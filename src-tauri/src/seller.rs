use crate::api::BackendClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
use tokio::time::Duration;
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

    let _ = &mut shutdown_rx;

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

        tokio::select! {
            _ = &mut shutdown_rx => {
                shutdown_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Raw WebSocket connection (no tungstenite — works around Android handshake bug)
// ---------------------------------------------------------------------------

/// Write a masked WebSocket text frame to a TCP stream.
fn ws_write_text(tcp: &mut std::net::TcpStream, payload: &str) -> std::io::Result<()> {
    let len = payload.len();
    let mut header = vec![0x81u8]; // FIN + text opcode
    if len < 126 {
        header.push((len as u8) | 0x80);
    } else if len <= 0xFFFF {
        header.push(126u8 | 0x80);
        header.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        header.push(127u8 | 0x80);
        header.extend_from_slice(&(len as u64).to_be_bytes());
    }
    let mask: [u8; 4] = rand::random();
    header.extend_from_slice(&mask);
    let masked: Vec<u8> = payload
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ mask[i % 4])
        .collect();
    header.extend_from_slice(&masked);
    tcp.write_all(&header)
}

/// Write a masked WebSocket ping frame (empty payload).
fn ws_write_ping(tcp: &mut std::net::TcpStream) -> std::io::Result<()> {
    let mask: [u8; 4] = rand::random();
    let frame = [0x89u8, 0x80, mask[0], mask[1], mask[2], mask[3]];
    tcp.write_all(&frame)
}

/// Write a masked WebSocket pong frame.
fn ws_write_pong(tcp: &mut std::net::TcpStream, payload: &[u8]) -> std::io::Result<()> {
    let len = payload.len();
    let mut header = vec![0x8Au8]; // FIN + pong opcode
    if len < 126 {
        header.push((len as u8) | 0x80);
    } else if len <= 0xFFFF {
        header.push(126u8 | 0x80);
        header.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        header.push(127u8 | 0x80);
        header.extend_from_slice(&(len as u64).to_be_bytes());
    }
    let mask: [u8; 4] = rand::random();
    header.extend_from_slice(&mask);
    let masked: Vec<u8> = payload
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ mask[i % 4])
        .collect();
    header.extend_from_slice(&masked);
    tcp.write_all(&header)
}

/// Try to extract a complete WebSocket frame from `buf`.
/// Returns `Some((payload, opcode, remaining))` if a frame was parsed,
/// or `None` if more data is needed.
fn ws_parse_frame(buf: &[u8]) -> Option<(Vec<u8>, u8, &[u8])> {
    if buf.len() < 2 {
        return None;
    }
    let opcode = buf[0] & 0x0F;
    let masked = (buf[1] & 0x80) != 0;
    let mut len = (buf[1] & 0x7F) as usize;
    let mut pos = 2;

    if len == 126 {
        if buf.len() < 4 { return None; }
        len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
        pos = 4;
    } else if len == 127 {
        if buf.len() < 10 { return None; }
        len = u64::from_be_bytes([
            buf[2], buf[3], buf[4], buf[5], buf[6], buf[7], buf[8], buf[9],
        ]) as usize;
        pos = 10;
    }

    let mask_key_pos = pos;
    if masked {
        pos += 4;
    }
    let payload_end = pos + len;
    if buf.len() < payload_end {
        return None;
    }

    let mut payload = Vec::from(&buf[pos..payload_end]);
    if masked {
        let mk = &buf[mask_key_pos..mask_key_pos + 4];
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mk[i % 4];
        }
    }

    Some((payload, opcode, &buf[payload_end..]))
}

async fn try_single_path_connection(
    app_handle: AppHandle,
    ws_url: &str,
    token: &str,
    path_id: &str,
    upstream: Option<&UpstreamProxy>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    if token.is_empty() {
        return Err("No session token — please login first.".to_string());
    }

    let url = ws_url.to_string();
    let t = token.to_string();
    let pid = path_id.to_string();
    let app = app_handle.clone();
    let up_cloned = upstream.cloned();

    // Channel for relay responses → raw WS text frames
    let (ws_write_tx, ws_write_rx) = std::sync::mpsc::channel::<String>();

    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let shutdown_flag = shutdown.clone();
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
        use std::collections::HashMap;
        use std::io::{BufRead, Read, Write};
        use std::net::TcpStream;
        use std::sync::atomic::Ordering;
        use std::sync::Mutex;
        use std::time::Instant;

        // Parse URL
        let without_scheme = url
            .strip_prefix("ws://")
            .or_else(|| url.strip_prefix("wss://"))
            .ok_or_else(|| format!("Invalid WS URL: {}", url))?;
        let slash = without_scheme.find('/').unwrap_or(without_scheme.len());
        let hp = &without_scheme[..slash];
        let pq = if slash < without_scheme.len() {
            &without_scheme[slash..]
        } else {
            "/"
        };
        let (host, port) = hp.split_once(':').map_or((hp, 8080u16), |(h, p)| {
            (h, p.parse::<u16>().unwrap_or(8080))
        });

        // TCP connect + WebSocket handshake
        let addr = format!("{}:{}", host, port);
        let mut tcp =
            TcpStream::connect(&addr).map_err(|e| format!("TCP {}: {}", addr, e))?;

        let key_bytes: [u8; 16] = rand::random();
        let key = base64_encode(&key_bytes);
        let req = format!(
            "GET {} HTTP/1.1\r\nHost: {}:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {}\r\n\r\n",
            pq, host, port, key
        );
        tcp.write_all(req.as_bytes())
            .map_err(|e| format!("Write upgrade: {}", e))?;

        // Read response
        let mut reader = std::io::BufReader::new(&mut tcp);
        let mut status = String::new();
        reader.read_line(&mut status).map_err(|e| format!("Read status: {}", e))?;
        let mut headers = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).map_err(|e| format!("Read header: {}", e))?;
            if line == "\r\n" || line == "\n" || line.is_empty() {
                break;
            }
            headers.push_str(&line);
        }
        if !status.contains("101") {
            return Err(format!("Expected 101, got: {} | {}", status.trim(), headers.trim()));
        }
        drop(reader);

        let _ = app.emit("seller:connected", format!("[{}] Connected (raw WS)", pid));

        // Auth + path_info
        ws_write_text(&mut tcp, &t).map_err(|e| format!("Send auth: {}", e))?;
        let path_info = serde_json::json!({"type": "path_info", "path_id": &pid});
        ws_write_text(&mut tcp, &serde_json::to_string(&path_info).unwrap_or_default())
            .map_err(|e| format!("Send path_info: {}", e))?;

        tcp.set_nonblocking(true).map_err(|e| e.to_string())?;

        let active: Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<Vec<u8>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let mut stream_count: u32 = 0;
        let mut last_ping = Instant::now();
        let mut last_hb = Instant::now();
        let mut read_buf = Vec::new();
        let mut frame_buf = vec![0u8; 65536];

        loop {
            if shutdown_flag.load(Ordering::Relaxed) {
                return Ok(());
            }

            // Drain write channel
            while let Ok(msg) = ws_write_rx.try_recv() {
                let _ = ws_write_text(&mut tcp, &msg);
            }

            // Timers
            if last_ping.elapsed() >= std::time::Duration::from_secs(30) {
                let _ = ws_write_ping(&mut tcp);
                last_ping = Instant::now();
            }
            if last_hb.elapsed() >= std::time::Duration::from_secs(30) {
                let hb = serde_json::json!({
                    "type": "heartbeat",
                    "active_streams": stream_count,
                    "version": "0.1.0",
                });
                let _ = ws_write_text(
                    &mut tcp,
                    &serde_json::to_string(&hb).unwrap_or_default(),
                );
                last_hb = Instant::now();
            }

            // Read
            match tcp.read(&mut frame_buf) {
                Ok(0) => return Ok(()),
                Ok(n) => read_buf.extend_from_slice(&frame_buf[..n]),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    // Continue to parse what we already have
                }
                Err(e) => return Err(format!("TCP read: {}", e)),
            }

            // Parse frames from accumulated buffer (every iteration, not just WouldBlock)
            while let Some((payload, opcode, remaining)) = ws_parse_frame(&read_buf) {
                read_buf = remaining.to_vec();
                match opcode {
                    0x1 => {
                        // Text frame
                        let text = String::from_utf8_lossy(&payload);
                        let p: serde_json::Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        if p.get("error").and_then(|v| v.as_str()) == Some("invalid_token") {
                            return Err("AUTH_EXPIRED".to_string());
                        }
                        match p.get("type").and_then(|v| v.as_str()) {
                            Some("relay_data") => {
                                if let Some(enc) = p.get("data").and_then(|v| v.as_str()) {
                                    if let Some(dec) = base64_decode(enc) {
                                        let streams = active.lock().unwrap();
                                        let sid = p
                                            .get("session_id")
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
                                let sid = p
                                    .get("session_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?")
                                    .to_string();
                                let tip = p
                                    .get("target_ip")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("127.0.0.1")
                                    .to_string();
                                let tport = p
                                    .get("target_port")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(443) as u16;
                                let thost = p
                                    .get("target_host")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let dest = thost.unwrap_or_else(|| tip.clone());
                                stream_count += 1;

                                let (tcp_tx, tcp_rx) =
                                    std::sync::mpsc::channel::<Vec<u8>>();
                                active.lock().unwrap().insert(sid.clone(), tcp_tx);

                                if !sid.starts_with("probe_") {
                                    let _ = app.emit(
                                        "seller:stream-open",
                                        StreamEvent {
                                            session_id: sid.clone(),
                                            target_ip: tip.clone(),
                                            target_port: tport,
                                            route_index: None,
                                        },
                                    );
                                }

                                // Bridge std → tokio channel for run_stream_relay
                                let (tokio_tx, tokio_rx) =
                                    mpsc::unbounded_channel::<Vec<u8>>();
                                std::thread::spawn(move || {
                                    while let Ok(data) = tcp_rx.recv() {
                                        if tokio_tx.send(data).is_err() {
                                            break;
                                        }
                                    }
                                });

                                // Relay output → ws_write_tx
                                let (relay_tx, mut relay_drain) =
                                    mpsc::unbounded_channel::<Message>();
                                let ws_out = ws_write_tx.clone();
                                std::thread::spawn(move || {
                                    while let Some(msg) = relay_drain.blocking_recv() {
                                        if let Message::Text(t) = msg {
                                            if ws_out.send(t.to_string()).is_err() {
                                                break;
                                            }
                                        }
                                    }
                                });

                                let app2 = app.clone();
                                let up = up_cloned.clone();
                                let sid2 = sid.clone();
                                let active2 = active.clone();
                                std::thread::spawn(move || {
                                    let rt = tokio::runtime::Builder::new_current_thread()
                                        .enable_io()
                                        .enable_time()
                                        .build()
                                        .unwrap();
                                    let app_emit = app2.clone();
                                    rt.block_on(async {
                                        let up_ref: Option<&UpstreamProxy> = up.as_ref();
                                        run_stream_relay(
                                            app2, &dest, &tip, tport, up_ref,
                                            &relay_tx, tokio_rx, &sid2,
                                        )
                                        .await;
                                        active2.lock().unwrap().remove(&sid2);
                                        if !sid2.starts_with("probe_") {
                                            let _ = app_emit
                                                .emit("seller:stream-closed", &sid2);
                                        }
                                    });
                                });
                            }
                            _ => {}
                        }
                    }
                    0x8 => return Ok(()), // Close
                    0x9 => {
                        // Ping → Pong
                        let _ = ws_write_pong(&mut tcp, &payload);
                    }
                    _ => {} // Pong (0xA) and others ignored
                }
            }
        }
        })();
        let _ = done_tx.send(result);
    });

    done_rx.await.map_err(|_| "WS thread panicked".to_string())?
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
