use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::api::BackendClient;

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
    task: tokio::task::JoinHandle<()>,
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
    // Idempotent start: if this session already has a healthy bridge, keep it.
    // Restarting it would drop the stable port (and any live tunnels) for no
    // benefit — the upstream token is already reloaded on every connection.
    // The lock is held across bind+insert so two concurrent starts for the
    // same session can never both bind and orphan a listener.
    let mut bridges = BRIDGES.lock().await;
    if let Some(bridge) = bridges.get(&session_id) {
        return Ok(bridge.local_port);
    }

    // If the preferred port is owned by ANOTHER session's bridge, leave that
    // bridge alone and fall back to an ephemeral port instead of killing it.
    let bind_addr = if let Some(port) = preferred_port {
        let owned_by_other = bridges.values().any(|b| b.local_port == port);
        if owned_by_other {
            "127.0.0.1:0".to_string()
        } else {
            format!("127.0.0.1:{}", port)
        }
    } else {
        "127.0.0.1:0".to_string()
    };

    // SO_REUSEADDR is required on macOS/BSD to rebind a stable port while old
    // relay connections are still in TIME_WAIT. Without it, a restart fails
    // with EADDRINUSE and the bridge port silently disappears.
    let socket = tokio::net::TcpSocket::new_v4()
        .map_err(|e| format!("Failed to create bridge socket: {}", e))?;
    socket
        .set_reuseaddr(true)
        .map_err(|e| format!("Failed to set SO_REUSEADDR: {}", e))?;
    socket
        .bind(
            bind_addr
                .parse::<std::net::SocketAddr>()
                .map_err(|e| format!("Invalid bridge bind addr {}: {}", bind_addr, e))?,
        )
        .map_err(|e| format!("Failed to bind bridge on {}: {}", bind_addr, e))?;
    let listener = socket
        .listen(1024)
        .map_err(|e| format!("Failed to listen on {}: {}", bind_addr, e))?;
    let local_port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local addr: {}", e))?
        .port();

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let sid = session_id.clone();
    let task = tokio::spawn(async move {
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
                            // Dynamically reload token on every connection so
                            // re-auth'd tokens propagate to the bridge instantly.
                            let up_pass = BackendClient::load_token()
                                .unwrap_or_default();
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

    bridges.insert(
        session_id,
        Bridge {
            shutdown_tx,
            task,
            local_port,
        },
    );

    Ok(local_port)
}

/// Stop the bridge for a given session.
pub async fn stop_bridge(session_id: &str) {
    let bridge = {
        let mut bridges = BRIDGES.lock().await;
        bridges.remove(session_id)
    };
    if let Some(bridge) = bridge {
        let _ = bridge.shutdown_tx.send(());
        // Wait (bounded) for the listener task to actually exit so the port is
        // guaranteed released before a caller rebinds it. Previously the stop
        // returned immediately and a restart could race the old listener's
        // teardown, failing with EADDRINUSE.
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            bridge.task,
        )
        .await;
        eprintln!("[bridge {}] Stopped", session_id);
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
    // Bound the client SOCKS5 handshake so a stalled client cannot hold a
    // socket (FD) forever.
    let target = match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        accept_socks5_noauth(&mut client),
    )
    .await
    {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            eprintln!("Bridge SOCKS5 handshake failed: {}", e);
            return;
        }
        Err(_) => {
            eprintln!("Bridge SOCKS5 handshake timed out");
            return;
        }
    };

    // Connect to upstream with auth. fast-socks5 has no built-in connect
    // timeout (Config::default() leaves connect_timeout as None), so bound the
    // whole upstream handshake to keep dead proxies from leaking sockets.
    let mut cfg = fast_socks5::client::Config::default();
    cfg.set_skip_auth(false);
    cfg.set_connect_timeout(std::time::Duration::from_secs(10));
    let upstream = match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        fast_socks5::client::Socks5Stream::connect_with_password(
            upstream_addr,
            target.0,
            target.1,
            upstream_username.to_string(),
            upstream_password.to_string(),
            cfg,
        ),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            eprintln!("Bridge upstream connect failed: {:?}", e);
            return;
        }
        Err(_) => {
            eprintln!("Bridge upstream connect timed out");
            return;
        }
    };

    let (mut up_r, mut up_w) = tokio::io::split(upstream);
    let (mut cl_r, mut cl_w) = tokio::io::split(client);

    // Idle timeout: traffic in either direction resets the clock. Abandoned
    // tunnels close after 60s (matches the seller relay's idle timeout) so a
    // client that never disconnects cannot hold sockets forever.
    const IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
    let deadline = Arc::new(std::sync::Mutex::new(
        tokio::time::Instant::now() + IDLE_TIMEOUT,
    ));

    // Bidirectional relay
    let mut up_to_cl = {
        let deadline = deadline.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            loop {
                match up_r.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        *deadline.lock().unwrap() =
                            tokio::time::Instant::now() + IDLE_TIMEOUT;
                        if cl_w.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    };

    let mut cl_to_up = {
        let deadline = deadline.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            loop {
                match cl_r.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        *deadline.lock().unwrap() =
                            tokio::time::Instant::now() + IDLE_TIMEOUT;
                        if up_w.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    };

    // Sleep until the current idle deadline, re-checking it on each wake so
    // any traffic keeps the tunnel alive indefinitely.
    let mut idle_task = tokio::spawn({
        let deadline = deadline.clone();
        async move {
            loop {
                let next = *deadline.lock().unwrap();
                if tokio::time::Instant::now() >= next {
                    break;
                }
                tokio::time::sleep_until(next).await;
            }
        }
    });

    tokio::select! {
        _ = &mut up_to_cl => {}
        _ = &mut cl_to_up => {}
        _ = &mut idle_task => {
            eprintln!("Bridge relay idle timeout — closing");
        }
    }
    // Abort whichever direction is still running so BOTH socket halves close
    // immediately (previously the other task kept running, leaking sockets).
    up_to_cl.abort();
    cl_to_up.abort();
    idle_task.abort();
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
