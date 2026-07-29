use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
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
    let mock_app = tauri::test::mock_builder()
        .build(tauri::generate_context!())
        .expect("mock app build");
    let app_handle = mock_app.handle().clone();

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
