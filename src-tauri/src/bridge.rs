use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn bridge_start(
    session_id: String,
    upstream_addr: String,
    upstream_username: String,
    upstream_password: String,
    preferred_port: Option<u16>,
) -> Result<u16, String> {
    start_bridge(session_id, upstream_addr, upstream_username, upstream_password, preferred_port).await
}

#[tauri::command]
pub async fn bridge_stop(session_id: String) -> Result<(), String> {
    stop_bridge(&session_id).await;
    Ok(())
}

#[tauri::command]
pub async fn bridge_port(session_id: String) -> Result<Option<u16>, String> {
    Ok(bridge_port_inner(&session_id).await)
}

/// A running local bridge instance — unauthenticated SOCKS5 → authenticated upstream.
struct Bridge {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    local_port: u16,
}

/// Global registry of active bridges, keyed by session_id.
static BRIDGES: std::sync::LazyLock<Arc<Mutex<HashMap<String, Bridge>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Start a local unauthenticated SOCKS5 bridge for a session.
/// Returns the local port the bridge is listening on.
pub async fn start_bridge(
    session_id: String,
    upstream_addr: String,
    upstream_username: String,
    upstream_password: String,
    preferred_port: Option<u16>,
) -> Result<u16, String> {
    // Stop existing bridge for this session if any
    stop_bridge(&session_id).await;

    // Try preferred port first, fall back to random
    let listener = if let Some(port) = preferred_port {
        match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
            Ok(l) => l,
            Err(_) => TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|e| format!("Failed to bind bridge listener: {}", e))?,
        }
    } else {
        TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("Failed to bind bridge listener: {}", e))?
    };
    let local_port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local addr: {}", e))?
        .port();

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let sid = session_id.clone();
    tokio::spawn(async move {
        eprintln!("[bridge {}] Started on port {}", sid, local_port);

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    eprintln!("[bridge {}] Shutting down", sid);
                    break;
                }
                result = listener.accept() => {
                    match result {
                        Ok((client_stream, client_addr)) => {
                            let up_addr = upstream_addr.clone();
                            let up_user = upstream_username.clone();
                            let up_pass = upstream_password.clone();
                            eprintln!("[bridge {}] Accepted client {}", sid, client_addr);
                            tokio::spawn(async move {
                                relay_through_upstream(
                                    client_stream,
                                    &up_addr,
                                    &up_user,
                                    &up_pass,
                                ).await;
                            });
                        }
                        Err(e) => {
                            eprintln!("[bridge {}] Accept error: {}", sid, e);
                        }
                    }
                }
            }
        }
    });

    let mut bridges = BRIDGES.lock().await;
    bridges.insert(
        session_id,
        Bridge {
            shutdown_tx,
            local_port,
        },
    );

    Ok(local_port)
}

/// Stop the bridge for a given session.
pub async fn stop_bridge(session_id: &str) {
    let mut bridges = BRIDGES.lock().await;
    if let Some(bridge) = bridges.remove(session_id) {
        let _ = bridge.shutdown_tx.send(());
        eprintln!("[bridge {}] Stop signal sent", session_id);
    }
}

/// Get the local port for an active bridge.
async fn bridge_port_inner(session_id: &str) -> Option<u16> {
    let bridges = BRIDGES.lock().await;
    bridges.get(session_id).map(|b| b.local_port)
}

/// Relay a client connection through the authenticated upstream SOCKS5 proxy.
async fn relay_through_upstream(
    mut client: tokio::net::TcpStream,
    upstream_addr: &str,
    upstream_username: &str,
    upstream_password: &str,
) {
    // Accept SOCKS5 handshake from client (no auth)
    let target = match accept_socks5_noauth(&mut client).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Bridge SOCKS5 handshake failed: {}", e);
            return;
        }
    };

    // Connect to upstream with auth
    let mut cfg = fast_socks5::client::Config::default();
    cfg.set_skip_auth(false);
    let upstream = match fast_socks5::client::Socks5Stream::connect_with_password(
        upstream_addr,
        target.0,
        target.1,
        upstream_username.to_string(),
        upstream_password.to_string(),
        cfg,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Bridge upstream connect failed: {:?}", e);
            return;
        }
    };

    let (mut up_r, mut up_w) = tokio::io::split(upstream);
    let (mut cl_r, mut cl_w) = tokio::io::split(client);

    // Bidirectional relay
    let up_to_cl = tokio::spawn(async move {
        let mut buf = vec![0u8; 8192];
        loop {
            match up_r.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if cl_w.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let cl_to_up = tokio::spawn(async move {
        let mut buf = vec![0u8; 8192];
        loop {
            match cl_r.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if up_w.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Wait for either direction to finish
    tokio::select! {
        _ = up_to_cl => {}
        _ = cl_to_up => {}
    }
}

/// Minimal SOCKS5 connect accept (no auth, only CONNECT command).
async fn accept_socks5_noauth(
    client: &mut tokio::net::TcpStream,
) -> Result<(String, u16), String> {
    let mut greeting_hdr = [0u8; 2];
    client
        .read_exact(&mut greeting_hdr)
        .await
        .map_err(|e| format!("read greeting header: {}", e))?;

    if greeting_hdr[0] != 0x05 {
        return Err("not SOCKS5".to_string());
    }
    let nmethods = greeting_hdr[1] as usize;

    let mut methods = vec![0u8; nmethods];
    if nmethods > 0 {
        client
            .read_exact(&mut methods)
            .await
            .map_err(|e| format!("read methods: {}", e))?;
    }

    // Reply: no auth
    client
        .write_all(&[0x05, 0x00])
        .await
        .map_err(|e| format!("write auth reply: {}", e))?;

    // Read connect request
    let mut hdr = [0u8; 4];
    client
        .read_exact(&mut hdr)
        .await
        .map_err(|e| format!("read connect hdr: {}", e))?;

    if hdr[0] != 0x05 || hdr[1] != 0x01 {
        return Err("not CONNECT".to_string());
    }

    let host = match hdr[3] {
        0x01 => {
            // IPv4
            let mut ip = [0u8; 4];
            client
                .read_exact(&mut ip)
                .await
                .map_err(|e| format!("read ipv4: {}", e))?;
            std::net::Ipv4Addr::from(ip).to_string()
        }
        0x03 => {
            // Domain name
            let mut len = [0u8; 1];
            client
                .read_exact(&mut len)
                .await
                .map_err(|e| format!("read domain len: {}", e))?;
            let mut domain = vec![0u8; len[0] as usize];
            client
                .read_exact(&mut domain)
                .await
                .map_err(|e| format!("read domain: {}", e))?;
            String::from_utf8_lossy(&domain).to_string()
        }
        0x04 => {
            // IPv6
            let mut ip = [0u8; 16];
            client
                .read_exact(&mut ip)
                .await
                .map_err(|e| format!("read ipv6: {}", e))?;
            std::net::Ipv6Addr::from(ip).to_string()
        }
        _ => return Err("unsupported address type".to_string()),
    };

    // Read port
    let mut port_bytes = [0u8; 2];
    client
        .read_exact(&mut port_bytes)
        .await
        .map_err(|e| format!("read port: {}", e))?;
    let port = u16::from_be_bytes(port_bytes);

    // Send success reply
    let reply = [
        0x05, 0x00, 0x00, 0x01, // VER, REP, RSV, ATYP
        0x00, 0x00, 0x00, 0x00, // BND.ADDR (0.0.0.0)
        (port >> 8) as u8,       // BND.PORT hi
        (port & 0xFF) as u8,     // BND.PORT lo
    ];
    client
        .write_all(&reply)
        .await
        .map_err(|e| format!("write reply: {}", e))?;

    Ok((host, port))
}
