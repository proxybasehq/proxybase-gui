use crate::api::BackendClient;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::{interval, Duration};
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

async fn run_stream_relay(
    app_handle: AppHandle,
    target_dest: &str, // domain/IP for SOCKS5 routing
    target_ip: &str,   // IP for direct TCP
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
            match tokio::net::TcpStream::connect(format!("{}:{}", target_ip, target_port)).await {
                Ok(tcp) => {
                    let (r, w) = tokio::io::split(tcp);
                    Ok((Box::new(r), Box::new(w)))
                }
                Err(e) => Err(anyhow::anyhow!("TCP connect failed: {}", e)),
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

    // Race TCP→WS and WS→TCP via tokio::select! to prevent CLOSE_WAIT leaks.
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
                        .send(Message::Text(serde_json::to_string(&m).unwrap_or_default()))
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
// Seller: per-path WebSocket connections (mirrors CLI)
// ---------------------------------------------------------------------------

/// Build the list of paths: direct (None) + each upstream proxy.
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

pub async fn run_seller_ws_loop(
    app_handle: AppHandle,
    backend_url: String,
    upstreams: Vec<UpstreamProxy>,
    include_direct: bool,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    let paths = build_paths(&upstreams, include_direct);
    let base_url = backend_url.clone();

    let path_ids: Vec<String> = paths.iter().map(|(id, _)| id.clone()).collect();
    let _ = app_handle.emit(
        "seller:connected",
        format!("Starting {} path(s): {:?}", paths.len(), path_ids),
    );

    // Spawn one connection per path — each runs independently with its own reconnect loop
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

    // Wait for shutdown signal
    let _ = &mut shutdown_rx;

    // Drain handles: send shutdown to all, then await all
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

/// Single-path WebSocket connection loop with auto-reconnect and re-auth.
async fn run_single_path_loop(
    app_handle: AppHandle,
    backend_url: &str,
    path_id: &str,
    upstream: Option<&UpstreamProxy>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) {
    let upstream_owned = upstream.cloned();
    let pid = path_id.to_string();
    let mut backoff_secs = 1u64;

    loop {
        // Always rebuild WS URL with fresh token from disk
        let client = BackendClient::new(backend_url);
        let token = client.token().unwrap_or("").to_string();
        let ws_base = backend_url
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        let ws_url = format!("{}/v2/ws/seller?token={}", ws_base, token);

        let app = app_handle.clone();
        let up = upstream_owned.clone();
        let p = pid.clone();

        match try_single_path_connection(app.clone(), &ws_url, &token, &p, up.as_ref()).await {
            Ok(()) => {
                backoff_secs = 1;
                let _ = app.emit(
                    "seller:disconnected",
                    format!("[{}] Disconnected. Reconnecting...", p),
                );
            }
            Err(e) if e.contains("AUTH_EXPIRED") => {
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

        // Check for shutdown between reconnects
        tokio::select! {
            _ = &mut shutdown_rx => {
                return;
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
}

/// Establish one WebSocket connection for a single path and relay until disconnect.
async fn try_single_path_connection(
    app_handle: AppHandle,
    ws_url: &str,
    token: &str,
    path_id: &str,
    upstream: Option<&UpstreamProxy>,
) -> Result<(), String> {
    let (ws, _resp) = tokio_tungstenite::connect_async(ws_url)
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    let conn_id = uuid::Uuid::new_v4().to_string();
    let _ = app_handle.emit(
        "seller:connected",
        format!("[{}] Connected (conn={})", path_id, &conn_id[..8]),
    );

    let (mut ws_sink, mut ws_stream) = ws.split();

    // Send auth token as first message (required by backend WS listener)
    ws_sink
        .send(Message::Text(token.to_string()))
        .await
        .map_err(|e| format!("Failed to send auth token: {}", e))?;

    // Send path_info to identify this connection's path
    let path_info = serde_json::json!({"type": "path_info", "path_id": path_id});
    ws_sink
        .send(Message::Text(
            serde_json::to_string(&path_info).unwrap_or_default(),
        ))
        .await
        .map_err(|e| format!("Failed to send path_info: {}", e))?;

    let (relay_tx, mut relay_rx) = mpsc::unbounded_channel::<Message>();
    let active: Arc<TokioMutex<HashMap<String, mpsc::UnboundedSender<Vec<u8>>>>> =
        Arc::new(TokioMutex::new(HashMap::new()));

    let relay_drain = tokio::spawn(async move {
        while let Some(msg) = relay_rx.recv().await {
            if ws_sink.send(msg).await.is_err() {
                break;
            }
        }
    });

    let upstream_owned = upstream.cloned();
    let mut ping_tick = interval(Duration::from_secs(30));
    let mut heartbeat_tick = interval(Duration::from_secs(60));
    let mut stream_count: u32 = 0;

    loop {
        tokio::select! {
            _ = ping_tick.tick() => {
                let _ = relay_tx.send(Message::Ping(vec![].into()));
            }
            _ = heartbeat_tick.tick() => {
                let hb = serde_json::json!({
                    "type": "heartbeat",
                    "active_streams": stream_count,
                    "version": "0.1.0",
                    "conn_id": conn_id,
                });
                let _ = relay_tx.send(Message::Text(serde_json::to_string(&hb).unwrap_or_default()));
            }
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(Message::Ping(d))) => {
                        let _ = relay_tx.send(Message::Pong(d));
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(p) = serde_json::from_str::<serde_json::Value>(&text) {
                            // Detect auth-token rejection from the server
                            if p.get("error").and_then(|v| v.as_str()) == Some("invalid_token") {
                                relay_drain.abort();
                                return Err("AUTH_EXPIRED".to_string());
                            }
                            match p.get("type").and_then(|v| v.as_str()) {
                                Some("relay_data") => {
                                    if let Some(enc) = p.get("data").and_then(|v| v.as_str()) {
                                        if let Some(dec) = base64_decode(enc) {
                                            let streams = active.lock().await;
                                            let sid = p.get("session_id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("");
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
                                Some("stream_open") => {
                                    let sid = p.get("session_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("?")
                                        .to_string();
                                    let tip = p.get("target_ip")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("127.0.0.1")
                                        .to_string();
                                    let tport = p.get("target_port")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(443) as u16;
                                    let thost = p.get("target_host")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());
                                    let dest = thost.unwrap_or_else(|| tip.clone());

                                    let streams = active.clone();
                                    let tx = relay_tx.clone();
                                    let up = upstream_owned.clone();
                                    stream_count += 1;

                                    let (tcp_tx, tcp_rx) =
                                        mpsc::unbounded_channel::<Vec<u8>>();
                                    streams.lock().await.insert(sid.clone(), tcp_tx);

                                    // Don't emit stream-open for QoS probes — they're transient
                                    if !sid.starts_with("probe_") {
                                        let _ = app_handle.emit("seller:stream-open", StreamEvent {
                                            session_id: sid.clone(),
                                            target_ip: tip.clone(),
                                            target_port: tport,
                                            route_index: None,
                                        });
                                    }

                                    let app_handle2 = app_handle.clone();
                                    let sid2 = sid.clone();
                                    tokio::spawn(async move {
                                        let up_ref: Option<&UpstreamProxy> = up.as_ref();
                                        run_stream_relay(
                                            app_handle2.clone(),
                                            &dest,
                                            &tip,
                                            tport,
                                            up_ref,
                                            &tx,
                                            tcp_rx,
                                            &sid2,
                                        )
                                        .await;
                                        streams.lock().await.remove(&sid2);
                                        if !sid2.starts_with("probe_") {
                                            let _ = app_handle2.emit("seller:stream-closed", &sid2);
                                        }
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        relay_drain.abort();
                        return Ok(());
                    }
                    Some(Err(e)) => {
                        relay_drain.abort();
                        return Err(format!("WS error: {}", e));
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
    let upstreams: Vec<UpstreamProxy> = serde_json::from_str(&upstreams_json)
        .map_err(|e| format!("Invalid upstreams: {}", e))?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    // Store shutdown handle
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
