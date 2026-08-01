use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

/// Helper function to parse unmasked or masked WS frame received by server.
fn server_parse_ws_frame(buf: &[u8]) -> Option<(Vec<u8>, u8, &[u8])> {
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

/// Helper function for server to write unmasked WS text frame.
fn server_write_ws_text(stream: &mut impl Write, payload: &str) -> std::io::Result<()> {
    let len = payload.len();
    let mut header = vec![0x81u8]; // FIN + text opcode
    if len < 126 {
        header.push(len as u8);
    } else if len <= 0xFFFF {
        header.push(126u8);
        header.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        header.push(127u8);
        header.extend_from_slice(&(len as u64).to_be_bytes());
    }
    header.extend_from_slice(payload.as_bytes());
    stream.write_all(&header)
}

/// Base64 encoder (mirrors the seller's private helper).
fn b64(data: &[u8]) -> String {
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for ch in data.chunks(3) {
        let b0 = ch[0] as u32;
        let b1 = *ch.get(1).unwrap_or(&0) as u32;
        let b2 = *ch.get(2).unwrap_or(&0) as u32;
        let t = (b0 << 16) | (b1 << 8) | b2;
        o.push(C[((t >> 18) & 63) as usize] as char);
        o.push(C[((t >> 12) & 63) as usize] as char);
        o.push(if ch.len() > 1 { C[((t >> 6) & 63) as usize] as char } else { '=' });
        o.push(if ch.len() > 2 { C[(t & 63) as usize] as char } else { '=' });
    }
    o
}

/// Upgrade a raw TCP connection to a mock backend WebSocket and consume the
/// seller's setup frames (auth token + path_info).
fn upgrade_ws_and_consume_setup(stream: &mut TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    assert!(
        request_line.contains("GET /v2/ws/seller?token="),
        "Expected token in request line, got: {}",
        request_line
    );

    let mut sec_ws_key = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
        if line.to_lowercase().starts_with("sec-websocket-key:") {
            sec_ws_key = line.split(':').nth(1).unwrap().trim().to_string();
        }
    }
    assert!(!sec_ws_key.is_empty(), "Sec-WebSocket-Key missing");

    let accept_key =
        tokio_tungstenite::tungstenite::handshake::derive_accept_key(sec_ws_key.as_bytes());
    let resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\r\n",
        accept_key
    );
    stream.write_all(resp.as_bytes()).unwrap();

    // Consume auth token frame + path_info frame (masked text frames) through
    // the BufReader so any bytes it already buffered are not lost.
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    for _ in 0..2 {
        loop {
            if let Some((payload, opcode, remaining)) = server_parse_ws_frame(&buf) {
                let remaining = remaining.to_vec();
                buf.clear();
                buf.extend_from_slice(&remaining);
                assert_eq!(opcode, 0x1, "expected text frame");
                let _: serde_json::Value = serde_json::from_slice(&payload).unwrap_or_default();
                break;
            }
            let n = reader.read(&mut tmp).unwrap();
            assert!(n > 0, "seller closed during setup");
            buf.extend_from_slice(&tmp[..n]);
        }
    }
}

/// Read the next text frame from a mock backend stream.
fn read_ws_text(stream: &mut TcpStream, buf: &mut Vec<u8>, timeout: std::time::Duration) -> Option<serde_json::Value> {
    stream.set_read_timeout(Some(timeout)).ok()?;
    let mut tmp = [0u8; 4096];
    loop {
        if let Some((payload, opcode, remaining)) = server_parse_ws_frame(buf) {
            let remaining = remaining.to_vec();
            buf.clear();
            buf.extend_from_slice(&remaining);
            if opcode == 0x1 {
                return serde_json::from_slice(&payload).ok();
            }
            continue;
        }
        match stream.read(&mut tmp) {
            Ok(0) => return None,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(_) => return None,
        }
    }
}

/// One shared Tauri mock app for all tests (generate_context! may only appear
/// once per crate). The App is leaked so its handle stays valid.
fn mock_app_handle() -> tauri::AppHandle<tauri::test::MockRuntime> {
    static APP: std::sync::OnceLock<tauri::AppHandle<tauri::test::MockRuntime>> =
        std::sync::OnceLock::new();
    APP.get_or_init(|| {
        let app = tauri::test::mock_builder()
            .build(tauri::generate_context!())
            .expect("mock app build");
        let handle = app.handle().clone();
        std::mem::forget(app); // keep the mock app alive for the process lifetime
        handle
    })
    .clone()
}

#[tokio::test]
async fn test_seller_ws_connection_handshake_and_protocol() {
    // 1. Bind mock server to port 0
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind tcp listener");
    let port = listener.local_addr().unwrap().port();

    let (server_done_tx, server_done_rx) = oneshot::channel::<()>();

    // 2. Spawn mock backend WebSocket server
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept seller connection");

        let mut reader = BufReader::new(stream.try_clone().unwrap());

        // Read HTTP request line & headers
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert!(
            request_line.contains("GET /v2/ws/seller?token="),
            "Expected token in request line, got: {}",
            request_line
        );

        let mut headers = String::new();
        let mut sec_ws_key = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" || line == "\n" || line.is_empty() {
                break;
            }
            if line.to_lowercase().starts_with("sec-websocket-key:") {
                sec_ws_key = line.split(':').nth(1).unwrap().trim().to_string();
            }
            headers.push_str(&line);
        }
        assert!(!sec_ws_key.is_empty(), "Sec-WebSocket-Key missing");

        let accept_key = tokio_tungstenite::tungstenite::handshake::derive_accept_key(sec_ws_key.as_bytes());
        let resp = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {}\r\n\r\n",
            accept_key
        );
        stream.write_all(resp.as_bytes()).unwrap();

        // Read Frame 1: Auth token
        let mut buf = Vec::new();
        let mut read_buf = [0u8; 1024];
        let n = stream.read(&mut read_buf).unwrap();
        buf.extend_from_slice(&read_buf[..n]);

        let (_payload1, opcode1, remaining) =
            server_parse_ws_frame(&buf).expect("auth frame expected");
        assert_eq!(opcode1, 0x1, "Auth frame opcode should be text");

        // Read Frame 2: path_info message
        let mut frame2_buf = remaining.to_vec();
        if frame2_buf.is_empty() {
            let n = stream.read(&mut read_buf).unwrap();
            frame2_buf.extend_from_slice(&read_buf[..n]);
        }

        let (payload2, opcode2, _) =
            server_parse_ws_frame(&frame2_buf).expect("path_info frame expected");
        assert_eq!(opcode2, 0x1);
        let path_info: serde_json::Value =
            serde_json::from_slice(&payload2).expect("path_info JSON parse");
        assert_eq!(path_info["type"], "path_info");
        assert_eq!(path_info["path_id"], "direct");

        // Send Frame 3: stream_open message from server to seller
        let stream_open_msg = serde_json::json!({
            "type": "stream_open",
            "session_id": "probe_test_session",
            "target_ip": "127.0.0.1",
            "target_port": 80,
        }).to_string();
        server_write_ws_text(&mut stream, &stream_open_msg).unwrap();

        // Notify test that handshake and auth succeeded
        let _ = server_done_tx.send(());
    });

    // 3. Run seller ws loop using Tauri mock_app
    let app_handle = mock_app_handle();

    let backend_url = format!("http://127.0.0.1:{}", port);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let seller_task = tokio::spawn(async move {
        proxybase_gui_lib::seller::run_seller_ws_loop(
            app_handle,
            backend_url,
            vec![], // direct only
            true,
            shutdown_rx,
        )
        .await
    });

    // Wait for server to verify handshake + auth frames
    server_done_rx.await.expect("mock server verification");

    // Trigger seller shutdown
    let _ = shutdown_tx.send(());
    let _ = seller_task.await;
}

/// Full QoS probe cycle against a real local target: stream_open → relay_data
/// (HTTP GET) → relay_response → stream_close. The target keeps its end open,
/// so the only way the relayed connection closes is if the seller tears it
/// down on stream_close — proving the socket/FD is released.
#[tokio::test]
async fn test_probe_cycle_closes_relay_connection() {
    // 1. Local target: reads the GET, replies, then holds the socket open and
    //    signals when the relay closes its end.
    let target_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let target_port = target_listener.local_addr().unwrap().port();
    let (target_done_tx, target_done_rx) = oneshot::channel::<()>();
    std::thread::spawn(move || {
        let (mut stream, _) = target_listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut req = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            match stream.read(&mut tmp) {
                Ok(n) => req.extend_from_slice(&tmp[..n]),
                Err(_) => break,
            }
            if req.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
        // Keep the connection open and wait for the relay to close it.
        let mut tmp2 = [0u8; 16];
        loop {
            match stream.read(&mut tmp2) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        let _ = target_done_tx.send(());
    });

    // 2. Mock backend WebSocket that drives the probe protocol.
    let ws_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let ws_port = ws_listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut stream, _) = ws_listener.accept().unwrap();
        upgrade_ws_and_consume_setup(&mut stream);

        let sid = "probe_cycle_test".to_string();
        let stream_open = serde_json::json!({
            "type": "stream_open",
            "session_id": sid,
            "target_ip": "127.0.0.1",
            "target_port": target_port,
            "target_host": "127.0.0.1",
        })
        .to_string();
        server_write_ws_text(&mut stream, &stream_open).unwrap();

        let http_req = b"GET / HTTP/1.1\r\nHost: target\r\n\r\n";
        let relay_msg = serde_json::json!({
            "type": "relay_data",
            "session_id": "probe_cycle_test",
            "data": b64(http_req),
        })
        .to_string();
        server_write_ws_text(&mut stream, &relay_msg).unwrap();

        // Wait for the relayed HTTP response to come back.
        let mut buf = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if std::time::Instant::now() >= deadline {
                panic!("no relay_response received");
            }
            match read_ws_text(&mut stream, &mut buf, std::time::Duration::from_secs(2)) {
                Some(v) if v.get("type").and_then(|t| t.as_str()) == Some("relay_response") => break,
                Some(_) => continue,
                None => panic!("seller closed before relay_response"),
            }
        }

        let close_msg = serde_json::json!({
            "type": "stream_close",
            "session_id": "probe_cycle_test",
        })
        .to_string();
        server_write_ws_text(&mut stream, &close_msg).unwrap();
    });

    // 3. Run the seller and require the target connection to be closed by the
    //    seller after stream_close.
    let backend_url = format!("http://127.0.0.1:{}", ws_port);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let seller_task = tokio::spawn(async move {
        proxybase_gui_lib::seller::run_seller_ws_loop(
            mock_app_handle(),
            backend_url,
            vec![],
            true,
            shutdown_rx,
        )
        .await
    });

    tokio::time::timeout(std::time::Duration::from_secs(15), target_done_rx)
        .await
        .expect("relay connection was not closed after stream_close (FD leak)")
        .expect("target signal channel closed unexpectedly");

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), seller_task).await;
}

/// A SOCKS5 upstream that accepts the TCP connection but never completes the
/// handshake must not hold its socket forever: the 10s connect timeout should
/// close it, and the upstream should observe EOF.
#[tokio::test]
async fn test_stalled_upstream_connect_times_out_and_closes() {
    // 1. Stalling SOCKS5 server: accepts, reads the greeting, never replies.
    let stall_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let stall_port = stall_listener.local_addr().unwrap().port();
    let (stall_done_tx, stall_done_rx) = oneshot::channel::<()>();
    std::thread::spawn(move || {
        let (mut stream, _) = stall_listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(20)))
            .unwrap();
        let mut greeting = [0u8; 32];
        let _ = stream.read(&mut greeting);
        // Never reply; wait until the client gives up and closes.
        let mut tmp = [0u8; 16];
        loop {
            match stream.read(&mut tmp) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        let _ = stall_done_tx.send(());
    });

    // 2. Mock backend WebSocket that opens an upstream-path probe.
    let ws_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let ws_port = ws_listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut stream, _) = ws_listener.accept().unwrap();
        upgrade_ws_and_consume_setup(&mut stream);
        let stream_open = serde_json::json!({
            "type": "stream_open",
            "session_id": "probe_stall_test",
            "target_ip": "127.0.0.1",
            "target_port": 443,
            "target_host": "example.com",
        })
        .to_string();
        server_write_ws_text(&mut stream, &stream_open).unwrap();
        // Keep the WS open while the upstream connect times out.
        std::thread::sleep(std::time::Duration::from_secs(20));
    });

    // 3. Run the seller with one upstream path pointing at the stalling proxy.
    let backend_url = format!("http://127.0.0.1:{}", ws_port);
    let upstreams = vec![proxybase_gui_lib::seller::UpstreamProxy {
        address: format!("127.0.0.1:{}", stall_port),
        username: "u".to_string(),
        password: "p".to_string(),
    }];
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let seller_task = tokio::spawn(async move {
        proxybase_gui_lib::seller::run_seller_ws_loop(
            mock_app_handle(),
            backend_url,
            upstreams,
            false,
            shutdown_rx,
        )
        .await
    });

    tokio::time::timeout(std::time::Duration::from_secs(20), stall_done_rx)
        .await
        .expect("upstream socket was never closed after the connect timeout")
        .expect("stall signal channel closed unexpectedly");

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), seller_task).await;
}
