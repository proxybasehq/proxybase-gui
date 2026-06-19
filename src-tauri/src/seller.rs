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
    target_ip: &str,
    target_port: u16,
    upstream: Option<&UpstreamProxy>,
    relay_tx: &mpsc::UnboundedSender<Message>,
    mut tcp_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    sid: &str,
) {
    let sid = sid.to_string();
    let connect_timeout = Duration::from_secs(10);

    let connect_fut = async {
        match upstream {
            Some(proxy) => {
                match fast_socks5::client::Socks5Stream::connect_with_password(
                    &proxy.address,
                    target_ip.to_string(),
                    target_port,
                    proxy.username.clone(),
                    proxy.password.clone(),
                    fast_socks5::client::Config::default(),
                )
                .await
                {
                    Ok(stream) => {
                        let (r, w) = tokio::io::split(stream);
                        Ok((Box::new(r) as Box<dyn tokio::io::AsyncRead + Unpin + Send>, Box::new(w) as Box<dyn tokio::io::AsyncWrite + Unpin + Send>))
                    }
                    Err(e) => Err(anyhow::anyhow!("SOCKS5 connect failed: {:?}", e)),
                }
            }
            None => match tokio::net::TcpStream::connect(format!("{}:{}", target_ip, target_port)).await
            {
                Ok(tcp) => {
                    let (r, w) = tokio::io::split(tcp);
                    Ok((Box::new(r) as Box<dyn tokio::io::AsyncRead + Unpin + Send>, Box::new(w) as Box<dyn tokio::io::AsyncWrite + Unpin + Send>))
                }
                Err(e) => Err(anyhow::anyhow!("TCP connect failed: {}", e)),
            },
        }
    };

    let connect_result = match tokio::time::timeout(connect_timeout, connect_fut).await {
        Ok(Ok(streams)) => streams,
        Ok(Err(e)) => {
            eprintln!("[RELAY {}] {}:{} — connect error: {}", sid, target_ip, target_port, e);
            return;
        }
        Err(_elapsed) => {
            eprintln!("[RELAY {}] {}:{} — connect timed out after {:?}", sid, target_ip, target_port, connect_timeout);
            return;
        }
    };

    let (mut tcp_r, mut tcp_w) = connect_result;
    eprintln!("[RELAY {}] {}:{} — connected", sid, target_ip, target_port);

    let tx2 = relay_tx.clone();
    let sid2 = sid.clone();
    let (done_tx, mut done_rx) = tokio::sync::oneshot::channel::<()>();

    // TCP reads → WS relay_response
    let tcp_to_ws = tokio::spawn(async move {
        let mut buf = vec![0u8; 8192];
        let _done_tx = done_tx; // move into this task; dropped when task finishes
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
        // done_tx dropped here → signals the write loop to stop
    });

    // WS relay_data → TCP writes, stopping when either channel closes or TCP read side dies
    loop {
        tokio::select! {
            data = tcp_rx.recv() => {
                match data {
                    Some(d) => {
                        if tokio::io::AsyncWriteExt::write_all(&mut tcp_w, &d).await.is_err() {
                            eprintln!("[RELAY {}] TCP write error", sid);
                            break;
                        }
                    }
                    None => {
                        eprintln!("[RELAY {}] relay_data channel closed", sid);
                        break;
                    }
                }
            }
            _ = &mut done_rx => {
                eprintln!("[RELAY {}] TCP read side closed", sid);
                break; // TCP read side closed
            }
        }
    }
    tcp_to_ws.abort();
    eprintln!("[RELAY {}] closed", sid);
}

// ---------------------------------------------------------------------------
// Seller WebSocket loop with auto-reconnect (mirrors CLI)
// ---------------------------------------------------------------------------

pub async fn run_seller_ws_loop(
    app_handle: AppHandle,
    backend_url: String,
    upstreams: Vec<UpstreamProxy>,
    include_direct: bool,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    let pool: Vec<Option<UpstreamProxy>> = {
        let mut v: Vec<Option<UpstreamProxy>> = Vec::new();
        if include_direct || upstreams.is_empty() {
            v.push(None);
        }
        for u in upstreams {
            v.push(Some(u));
        }
        v
    };
    let pool = Arc::new(pool);

    let mut backoff_secs = 1u64;

    loop {
        let result = run_single_ws_session(
            app_handle.clone(),
            &backend_url,
            pool.clone(),
            &mut shutdown_rx,
        ).await;

        match result {
            WsDisconnect::Stopped => {
                let _ = app_handle.emit("seller:disconnected", "Seller stopped by user");
                return Ok(());
            }
            WsDisconnect::Error(msg) => {
                let _ = app_handle.emit("seller:error", msg);
                // Fall through to reconnect
            }
            WsDisconnect::Closed => {
                let _ = app_handle.emit("seller:disconnected", "Backend closed connection — reconnecting...");
                // Fall through to reconnect
            }
        }

        // Exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s, cap at 60s, with 20% jitter
        let jitter = (backoff_secs as f64 * 0.2 * (rand::random::<f64>() - 0.5)) as u64;
        let delay = Duration::from_secs(backoff_secs.saturating_add(jitter));
        backoff_secs = (backoff_secs * 2).min(60);

        let _ = app_handle.emit("seller:reconnecting", format!("Reconnecting in {}s...", delay.as_secs()));

        tokio::select! {
            _ = &mut shutdown_rx => {
                let _ = app_handle.emit("seller:disconnected", "Seller stopped by user");
                return Ok(());
            }
            _ = tokio::time::sleep(delay) => {
                // continue reconnect loop
            }
        }
    }
}

enum WsDisconnect {
    Stopped,
    Error(String),
    Closed,
}

async fn run_single_ws_session(
    app_handle: AppHandle,
    backend_url: &str,
    pool: Arc<Vec<Option<UpstreamProxy>>>,
    shutdown_rx: &mut tokio::sync::oneshot::Receiver<()>,
) -> WsDisconnect {
    let client = BackendClient::new(backend_url);
    let ws_url = client.ws_url_for_seller();

    let (ws, _resp) = match tokio_tungstenite::connect_async(&ws_url).await {
        Ok(c) => c,
        Err(e) => {
            return WsDisconnect::Error(format!("WebSocket connect failed: {}", e));
        }
    };

    let _ = app_handle.emit("seller:connected", "Connected to seller WebSocket");

    let (mut ws_sink, mut ws_stream) = ws.split();
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

    let mut ping_tick = interval(Duration::from_secs(30));
    let mut heartbeat_tick = interval(Duration::from_secs(60));
    let mut stream_count: u32 = 0;

    loop {
        tokio::select! {
            _ = &mut *shutdown_rx => {
                relay_drain.abort();
                return WsDisconnect::Stopped;
            }
            _ = ping_tick.tick() => {
                let _ = relay_tx.send(Message::Ping(vec![].into()));
            }
            _ = heartbeat_tick.tick() => {
                let hb = serde_json::json!({
                    "type": "heartbeat",
                    "active_streams": stream_count,
                    "version": "0.1.0"
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

                                    let route_idx = p.get("route_index")
                                        .and_then(|v| v.as_u64())
                                        .map(|i| i as usize);

                                    let streams = active.clone();
                                    let tx = relay_tx.clone();
                                    let idx = route_idx.unwrap_or_else(|| {
                                        let mut h: usize = 0;
                                        for b in sid.as_bytes() {
                                            h = h.wrapping_mul(31).wrapping_add(*b as usize);
                                        }
                                        h % pool.len()
                                    });
                                    let up = pool[idx].clone();
                                    stream_count += 1;

                                    let (tcp_tx, tcp_rx) = mpsc::unbounded_channel::<Vec<u8>>();
                                    streams.lock().await.insert(sid.clone(), tcp_tx);

                                    let _ = app_handle.emit("seller:stream-open", StreamEvent {
                                        session_id: sid.clone(),
                                        target_ip: tip.clone(),
                                        target_port: tport,
                                        route_index: route_idx,
                                    });

                                    let app_handle2 = app_handle.clone();
                                    let sid2 = sid.clone();
                                    tokio::spawn(async move {
                                        let up_ref: Option<&UpstreamProxy> = up.as_ref();
                                        run_stream_relay(&tip, tport, up_ref, &tx, tcp_rx, &sid2).await;
                                        streams.lock().await.remove(&sid2);
                                        let _ = app_handle2.emit("seller:stream-closed", &sid2);
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        relay_drain.abort();
                        // Emit close for all remaining active streams so frontend can reset
                        let remaining = active.lock().await;
                        for sid in remaining.keys() {
                            let _ = app_handle.emit("seller:stream-closed", sid);
                        }
                        drop(remaining);
                        return WsDisconnect::Closed;
                    }
                    Some(Err(e)) => {
                        relay_drain.abort();
                        let remaining = active.lock().await;
                        for sid in remaining.keys() {
                            let _ = app_handle.emit("seller:stream-closed", sid);
                        }
                        drop(remaining);
                        return WsDisconnect::Error(format!("WS error: {}", e));
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
    let upstreams: Vec<UpstreamProxy> =
        serde_json::from_str(&upstreams_json).map_err(|e| format!("Invalid upstreams: {}", e))?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    // Store shutdown handle
    {
        let mut shutdown = state.shutdown_tx.lock().map_err(|e| e.to_string())?;
        // Stop any existing seller first
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
