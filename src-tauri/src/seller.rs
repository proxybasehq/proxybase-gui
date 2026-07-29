use crate::api::BackendClient;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, Mutex};
use tokio::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamProxy {
    pub address: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub session_id: String,
    pub target_ip: String,
    pub target_port: u16,
    pub route_index: Option<usize>,
}

// ---------------------------------------------------------------------------
// Seller state managed by Tauri
// ---------------------------------------------------------------------------

pub struct SellerState {
    pub shutdown_tx: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl SellerState {
    pub fn new() -> Self {
        Self {
            shutdown_tx: std::sync::Mutex::new(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Base64 helpers (mirrors CLI)
// ---------------------------------------------------------------------------

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            CHARS[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            CHARS[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn base64_decode(encoded: &str) -> Option<Vec<u8>> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for &b in encoded.as_bytes() {
        if b == b'=' {
            break;
        }
        let val = CHARS.iter().position(|&c| c == b)? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Stream relay (mirrors CLI run_stream_relay)
// ---------------------------------------------------------------------------

async fn run_stream_relay<R: tauri::Runtime + 'static>(
    app_handle: AppHandle<R>,
    target_dest: &str,
    target_ip: &str,
    target_port: u16,
    upstream: Option<&UpstreamProxy>,
    relay_tx: &mpsc::UnboundedSender<Message>,
    mut tcp_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    sid: &str,
) {
    let sid = sid.to_string();
    let using_upstream = upstream.is_some();

    let connect_result: anyhow::Result<(
        Box<dyn tokio::io::AsyncRead + Unpin + Send>,
        Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
    )> = match upstream {
        Some(proxy) => {
            match fast_socks5::client::Socks5Stream::connect_with_password(
                &proxy.address,
                target_dest.to_string(),
                target_port,
                proxy.username.clone(),
                proxy.password.clone(),
                fast_socks5::client::Config::default(),
            )
            .await
            {
                Ok(stream) => {
                    let (r, w) = tokio::io::split(stream);
                    Ok((Box::new(r), Box::new(w)))
                }
                Err(e) => Err(anyhow::anyhow!("SOCKS5 upstream connect failed: {:?}", e)),
            }
        }
        None => {
            match tokio::time::timeout(
                Duration::from_secs(10),
                tokio::net::TcpStream::connect(format!("{}:{}", target_ip, target_port)),
            )
            .await
            {
                Ok(Ok(tcp)) => {
                    let (r, w) = tokio::io::split(tcp);
                    Ok((Box::new(r), Box::new(w)))
                }
                Ok(Err(e)) => Err(anyhow::anyhow!("TCP connect failed: {}", e)),
                Err(_) => Err(anyhow::anyhow!("TCP connect timed out after 10s")),
            }
        }
    };

    let (mut tcp_r, mut tcp_w) = match connect_result {
        Ok(streams) => streams,
        Err(e) => {
            let _ = app_handle.emit(
                "seller:stream-error",
                serde_json::json!({
                    "session_id": sid,
                    "target": format!("{}:{}", target_ip, target_port),
                    "error": format!("{}/{}:{} — {}", target_dest, target_ip, target_port, e),
                    "upstream": using_upstream,
                }),
            );
            return;
        }
    };

    let tx2 = relay_tx.clone();
    let sid2 = sid.clone();

    let tcp_to_ws = async {
        let mut buf = vec![0u8; 8192];
        loop {
            match tokio::io::AsyncReadExt::read(&mut tcp_r, &mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let enc = base64_encode(&buf[..n]);
                    let m = serde_json::json!({
                        "type": "relay_response",
                        "session_id": &sid2,
                        "data": enc
                    });
                    if tx2
                        .send(Message::Text(serde_json::to_string(&m).unwrap_or_default().into()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    let ws_to_tcp = async {
        while let Some(data) = tcp_rx.recv().await {
            if tokio::io::AsyncWriteExt::write_all(&mut tcp_w, &data)
                .await
                .is_err()
            {
                break;
            }
        }
    };

    tokio::select! {
        _ = tcp_to_ws => {}
        _ = ws_to_tcp => {}
    }
}

// ---------------------------------------------------------------------------
// Seller: per-path WebSocket connections
// ---------------------------------------------------------------------------

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                format!("{}", b as char)
            } else {
                format!("%{:02X}", b)
            }
        })
        .collect()
}

fn build_paths(
    upstreams: &[UpstreamProxy],
    include_direct: bool,
) -> Vec<(String, Option<UpstreamProxy>)> {
    let mut paths: Vec<(String, Option<UpstreamProxy>)> = Vec::new();
    if include_direct {
        paths.push(("direct".to_string(), None));
    }
    for (i, u) in upstreams.iter().enumerate() {
        paths.push((format!("upstream_{}", i), Some(u.clone())));
    }
    if paths.is_empty() {
        paths.push(("direct".to_string(), None));
    }
    paths
}

pub async fn run_seller_ws_loop<R: tauri::Runtime + 'static>(
    app_handle: AppHandle<R>,
    backend_url: String,
    upstreams: Vec<UpstreamProxy>,
    include_direct: bool,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    let paths = build_paths(&upstreams, include_direct);
    let base_url = backend_url.clone();

    let path_ids: Vec<String> = paths.iter().map(|(id, _)| id.clone()).collect();
    let _ = app_handle.emit(
        "seller:connected",
        format!("Starting {} path(s): {:?}", paths.len(), path_ids),
    );

    let mut handles = Vec::new();
    for (path_id, upstream) in paths {
        let app = app_handle.clone();
        let url = base_url.clone();
        let (shutdown_child_tx, shutdown_child_rx) = tokio::sync::oneshot::channel::<()>();
        handles.push((
            shutdown_child_tx,
            tokio::spawn(async move {
                run_single_path_loop(app, &url, &path_id, upstream.as_ref(), shutdown_child_rx)
                    .await;
            }),
        ));
    }

    // Wait for external shutdown signal before stopping children
    let _ = shutdown_rx.await;

    let (senders, joins): (Vec<_>, Vec<_>) = handles.into_iter().unzip();
    for tx in senders {
        let _ = tx.send(());
    }
    for h in joins {
        let _ = h.await;
    }
    let _ = app_handle.emit("seller:disconnected", "Seller stopped by user");
    Ok(())
}

async fn run_single_path_loop<R: tauri::Runtime + 'static>(
    app_handle: AppHandle<R>,
    backend_url: &str,
    path_id: &str,
    upstream: Option<&UpstreamProxy>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) {
    let upstream_owned = upstream.cloned();
    let pid = path_id.to_string();
    let mut backoff_secs = 1u64;
    let shutdown_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

    loop {
        let client = BackendClient::new(backend_url);
        let token = client.token().unwrap_or("").to_string();
        let ws_base = backend_url
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        let ws_url = format!(
            "{}/v2/ws/seller?token={}",
            ws_base,
            percent_encode(&token)
        );

        let app = app_handle.clone();
        let up = upstream_owned.clone();
        let p = pid.clone();
        let flag = shutdown_flag.clone();

        match try_single_path_connection(
            app.clone(), &ws_url, &token, &p, up.as_ref(), flag,
        )
        .await
        {
            Ok(()) => {
                backoff_secs = 1;
                let _ = app.emit(
                    "seller:disconnected",
                    format!("[{}] Disconnected. Reconnecting...", p),
                );
            }
            Err(e) if e.contains("AUTH_EXPIRED") || e.contains("401") => {
                let _ = app.emit(
                    "seller:reconnecting",
                    format!("[{}] Token expired. Re-authenticating...", p),
                );
                if crate::commands::reauth(backend_url).await.is_ok() {
                    backoff_secs = 1;
                    let _ = app.emit("seller:connected", format!("[{}] Re-authenticated", p));
                } else {
                    let _ = app.emit(
                        "seller:error",
                        format!("[{}] Re-auth failed. Retrying...", p),
                    );
                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    backoff_secs = (backoff_secs * 2).min(60);
                }
            }
            Err(e) => {
                let _ = app.emit(
                    "seller:error",
                    format!("[{}] {} — retrying in {}s", p, e, backoff_secs),
                );
                tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(60);
            }
        }

        tokio::select! {
            _ = &mut shutdown_rx => {
                shutdown_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
}

async fn try_single_path_connection<R: tauri::Runtime + 'static>(
    app_handle: AppHandle<R>,
    ws_url: &str,
    token: &str,
    path_id: &str,
    upstream: Option<&UpstreamProxy>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    if token.is_empty() {
        return Err("No session token — please login first.".to_string());
    }

    let (ws, _) = connect_async(ws_url)
        .await
        .map_err(|e| format!("WS connect {}: {}", ws_url, e))?;

    let (mut ws_sink, mut ws_stream) = ws.split();

    // 1. Send auth token
    ws_sink
        .send(Message::Text(token.to_string().into()))
        .await
        .map_err(|e| format!("Send auth: {}", e))?;

    // 2. Send path_info
    let path_info = serde_json::json!({"type": "path_info", "path_id": path_id});
    ws_sink
        .send(Message::Text(serde_json::to_string(&path_info).unwrap_or_default().into()))
        .await
        .map_err(|e| format!("Send path_info: {}", e))?;

    let _ = app_handle.emit("seller:connected", format!("[{}] Connected", path_id));

    let (relay_tx, mut relay_rx) = mpsc::unbounded_channel::<Message>();
    let active: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Vec<u8>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let relay_drain = tokio::spawn(async move {
        while let Some(msg) = relay_rx.recv().await {
            if ws_sink.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut ping_tick = tokio::time::interval(Duration::from_secs(20));
    let mut heartbeat_tick = tokio::time::interval(Duration::from_secs(15));
    let mut watchdog = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(90),
        Duration::from_secs(90),
    );
    let upstream_owned = upstream.cloned();
    const MAX_STREAMS: usize = 100;

    loop {
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            relay_drain.abort();
            return Ok(());
        }

        tokio::select! {
            _ = watchdog.tick() => {
                // 90s silence — connection dead
                relay_drain.abort();
                return Err("Connection watchdog: no message in 90s".to_string());
            }
            _ = ping_tick.tick() => {
                let _ = relay_tx.send(Message::Ping(vec![].into()));
            }
            _ = heartbeat_tick.tick() => {
                let count = active.lock().await.len() as u32;
                let hb = serde_json::json!({
                    "type": "heartbeat",
                    "active_streams": count,
                    "version": "0.1.0"
                });
                let _ = relay_tx.send(Message::Text(serde_json::to_string(&hb).unwrap_or_default().into()));
            }
            msg = ws_stream.next() => {
                watchdog.reset();
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let p: serde_json::Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        if p.get("error").and_then(|v| v.as_str()) == Some("invalid_token") {
                            relay_drain.abort();
                            return Err("AUTH_EXPIRED".to_string());
                        }
                        match p.get("type").and_then(|v| v.as_str()) {
                            Some("relay_data") => {
                                if let Some(enc) = p.get("data").and_then(|v| v.as_str()) {
                                    if let Some(dec) = base64_decode(enc) {
                                        let streams = active.lock().await;
                                        let sid = p.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
                                        if let Some(s) = streams.get(sid) {
                                            let _ = s.send(dec);
                                        } else {
                                            for (_, s) in streams.iter() {
                                                let _ = s.send(dec.clone());
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            Some("stream_close") => {
                                let sid = p.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
                                active.lock().await.remove(sid);
                            }
                            Some("stream_open") => {
                                // Enforce concurrent stream limit
                                if active.lock().await.len() >= MAX_STREAMS {
                                    continue;
                                }
                                let sid = p.get("session_id").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                let tip = p.get("target_ip").and_then(|v| v.as_str()).unwrap_or("127.0.0.1").to_string();
                                let tport = p.get("target_port").and_then(|v| v.as_u64()).unwrap_or(443) as u16;
                                let thost = p.get("target_host").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let dest = thost.unwrap_or_else(|| tip.clone());

                                let (tcp_tx, tcp_rx) = mpsc::unbounded_channel::<Vec<u8>>();
                                active.lock().await.insert(sid.clone(), tcp_tx);

                                if !sid.starts_with("probe_") {
                                    let _ = app_handle.emit(
                                        "seller:stream-open",
                                        StreamEvent {
                                            session_id: sid.clone(),
                                            target_ip: tip.clone(),
                                            target_port: tport,
                                            route_index: None,
                                        },
                                    );
                                }

                                let app_cloned = app_handle.clone();
                                let up_cloned = upstream_owned.clone();
                                let sid_cloned = sid.clone();
                                let active_cloned = active.clone();
                                let tx_cloned = relay_tx.clone();

                                tokio::spawn(async move {
                                    run_stream_relay(
                                        app_cloned.clone(),
                                        &dest,
                                        &tip,
                                        tport,
                                        up_cloned.as_ref(),
                                        &tx_cloned,
                                        tcp_rx,
                                        &sid_cloned,
                                    ).await;

                                    active_cloned.lock().await.remove(&sid_cloned);
                                    if !sid_cloned.starts_with("probe_") {
                                        let _ = app_cloned.emit("seller:stream-closed", &sid_cloned);
                                    }
                                });
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = relay_tx.send(Message::Pong(payload));
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) | None => {
                        relay_drain.abort();
                        return Ok(());
                    }
                    Some(Err(e)) => {
                        relay_drain.abort();
                        return Err(format!("WS read error: {}", e));
                    }
                    _ => {}
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri commands for seller lifecycle
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn start_seller(
    app_handle: AppHandle,
    state: State<'_, SellerState>,
    backend_url: String,
    upstreams_json: String,
    include_direct: bool,
) -> Result<(), String> {
    let _ = stop_seller(state.clone()).await;

    let upstreams: Vec<UpstreamProxy> = serde_json::from_str(&upstreams_json)
        .map_err(|e| format!("Invalid upstreams: {}", e))?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    {
        let mut shutdown = state.shutdown_tx.lock().map_err(|e| e.to_string())?;
        *shutdown = Some(tx);
    }

    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        if let Err(e) =
            run_seller_ws_loop(app_handle_clone, backend_url, upstreams, include_direct, rx).await
        {
            eprintln!("Seller loop error: {}", e);
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_seller(state: State<'_, SellerState>) -> Result<(), String> {
    let mut shutdown = state.shutdown_tx.lock().map_err(|e| e.to_string())?;
    if let Some(tx) = shutdown.take() {
        let _ = tx.send(());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests for seller functionality
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode_decode_roundtrip() {
        // Empty
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_decode(""), Some(vec![]));

        // 1 byte padding (==)
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_decode("Zg=="), Some(b"f".to_vec()));

        // 2 bytes padding (=)
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_decode("Zm8="), Some(b"fo".to_vec()));

        // 3 bytes (no padding)
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_decode("Zm9v"), Some(b"foo".to_vec()));

        // Binary data (all 256 byte values)
        let binary_data: Vec<u8> = (0..=255).collect();
        let encoded = base64_encode(&binary_data);
        let decoded = base64_decode(&encoded);
        assert_eq!(decoded, Some(binary_data));
    }

    #[test]
    fn test_base64_decode_invalid() {
        // Invalid character
        assert_eq!(base64_decode("Zg!="), None);
    }

    #[test]
    fn test_percent_encode() {
        // Alphanumerics & unreserved chars preserved
        assert_eq!(percent_encode("abcXYZ123-._~"), "abcXYZ123-._~");

        // Special characters encoded
        assert_eq!(percent_encode("foo/bar?baz=1&token=a+b"), "foo%2Fbar%3Fbaz%3D1%26token%3Da%2Bb");

        // Space encoded as %20
        assert_eq!(percent_encode("hello world"), "hello%20world");
    }

    #[test]
    fn test_build_paths_direct_only() {
        let upstreams = vec![];
        let paths = build_paths(&upstreams, true);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].0, "direct");
        assert!(paths[0].1.is_none());
    }

    #[test]
    fn test_build_paths_no_direct_empty_upstreams_fallback() {
        // When include_direct is false and upstreams empty, falls back to direct
        let upstreams = vec![];
        let paths = build_paths(&upstreams, false);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].0, "direct");
        assert!(paths[0].1.is_none());
    }

    #[test]
    fn test_build_paths_with_upstreams() {
        let upstreams = vec![
            UpstreamProxy {
                address: "1.1.1.1:1080".to_string(),
                username: "u1".to_string(),
                password: "p1".to_string(),
            },
            UpstreamProxy {
                address: "2.2.2.2:1080".to_string(),
                username: "u2".to_string(),
                password: "p2".to_string(),
            },
        ];

        // With direct included
        let paths_with_direct = build_paths(&upstreams, true);
        assert_eq!(paths_with_direct.len(), 3);
        assert_eq!(paths_with_direct[0].0, "direct");
        assert_eq!(paths_with_direct[1].0, "upstream_0");
        assert_eq!(paths_with_direct[1].1.as_ref().unwrap().address, "1.1.1.1:1080");
        assert_eq!(paths_with_direct[2].0, "upstream_1");

        // Without direct
        let paths_no_direct = build_paths(&upstreams, false);
        assert_eq!(paths_no_direct.len(), 2);
        assert_eq!(paths_no_direct[0].0, "upstream_0");
        assert_eq!(paths_no_direct[1].0, "upstream_1");
    }

    #[test]
    fn test_seller_state_management() {
        let state = SellerState::new();
        assert!(state.shutdown_tx.lock().unwrap().is_none());

        let (tx, _rx) = tokio::sync::oneshot::channel();
        *state.shutdown_tx.lock().unwrap() = Some(tx);
        assert!(state.shutdown_tx.lock().unwrap().is_some());

        let taken = state.shutdown_tx.lock().unwrap().take();
        assert!(taken.is_some());
        assert!(state.shutdown_tx.lock().unwrap().is_none());
    }

    #[test]
    fn test_stream_event_serialization() {
        let event = StreamEvent {
            session_id: "sess_123".to_string(),
            target_ip: "192.168.1.1".to_string(),
            target_port: 8080,
            route_index: Some(2),
        };

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["session_id"], "sess_123");
        assert_eq!(json["target_ip"], "192.168.1.1");
        assert_eq!(json["target_port"], 8080);
        assert_eq!(json["route_index"], 2);
    }
}
