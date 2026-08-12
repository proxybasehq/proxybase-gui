use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Spawn a fake SOCKS5 server that accepts auth and echoes back the target address.
async fn fake_socks5_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        loop {
            let (mut stream, _) = match listener.accept().await {
                Ok(c) => c,
                Err(_) => break,
            };

            tokio::spawn(async move {
                let mut greeting_hdr = [0u8; 2];
                if stream.read_exact(&mut greeting_hdr).await.is_err() { return; }
                if greeting_hdr[0] != 0x05 { return; }
                let n = greeting_hdr[1] as usize;
                let mut methods = vec![0u8; n];
                if n > 0 { let _ = stream.read_exact(&mut methods).await; }

                // Accept user/pass auth
                stream.write_all(&[0x05, 0x02]).await.unwrap();

                // Read auth
                let mut auth_hdr = [0u8; 2];
                stream.read_exact(&mut auth_hdr).await.unwrap();
                let ulen = auth_hdr[1] as usize;
                let mut uname = vec![0u8; ulen];
                stream.read_exact(&mut uname).await.unwrap();
                let mut plen = [0u8; 1];
                stream.read_exact(&mut plen).await.unwrap();
                let mut pass = vec![0u8; plen[0] as usize];
                stream.read_exact(&mut pass).await.unwrap();
                let uname = String::from_utf8_lossy(&uname);
                let pass = String::from_utf8_lossy(&pass);

                // Auth success
                stream.write_all(&[0x01, 0x00]).await.unwrap();

                // Read CONNECT request
                let mut hdr = [0u8; 4];
                stream.read_exact(&mut hdr).await.unwrap();
                let host = match hdr[3] {
                    0x01 => {
                        let mut ip = [0u8; 4];
                        stream.read_exact(&mut ip).await.unwrap();
                        std::net::Ipv4Addr::from(ip).to_string()
                    }
                    0x03 => {
                        let mut len = [0u8; 1];
                        stream.read_exact(&mut len).await.unwrap();
                        let mut domain = vec![0u8; len[0] as usize];
                        stream.read_exact(&mut domain).await.unwrap();
                        String::from_utf8_lossy(&domain).to_string()
                    }
                    _ => return,
                };
                let mut pb = [0u8; 2];
                stream.read_exact(&mut pb).await.unwrap();
                let port = u16::from_be_bytes(pb);

                // Echo the connected target back as data
                let reply = format!(
                    "OK uname={} pass={} target={}:{}",
                    uname, pass, host, port
                );
                let reply_bytes = reply.into_bytes();

                // Send SOCKS5 success
                let socks_reply = [
                    0x05, 0x00, 0x00, 0x01,
                    0x00, 0x00, 0x00, 0x00,
                    (port >> 8) as u8, (port & 0xFF) as u8,
                ];
                stream.write_all(&socks_reply).await.unwrap();

                // Echo data back
                stream.write_all(&reply_bytes).await.unwrap();
                stream.shutdown().await.unwrap();
            });
        }
    });

    port
}

#[tokio::test]
async fn test_bridge_full_flow() {
    // 1. Start fake upstream
    let upstream_port = fake_socks5_server().await;

    // 2. Start bridge connected to fake upstream
    let port = proxybase_gui_lib::bridge::start_bridge(
        "test-session".to_string(),
        format!("127.0.0.1:{}", upstream_port),
        "myuser".to_string(),
        "mypass".to_string(),
        None,
    )
    .await
    .expect("bridge should start");

    // 3. Connect a SOCKS5 client to the bridge and request example.com:443
    let mut client = TcpStream::connect(format!("127.0.0.1:{}", port))
        .await
        .expect("connect to bridge");

    // SOCKS5 greeting: no auth
    client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
    let mut resp = [0u8; 2];
    client.read_exact(&mut resp).await.unwrap();
    assert_eq!(resp, [0x05, 0x00], "should accept no auth");

    // CONNECT to example.com:443 (domain name)
    let domain = b"example.com";
    let port = 443u16;
    let mut req = vec![
        0x05, // VER
        0x01, // CMD = CONNECT
        0x00, // RSV
        0x03, // ATYP = domain
        domain.len() as u8,
    ];
    req.extend_from_slice(domain);
    req.extend_from_slice(&port.to_be_bytes());
    client.write_all(&req).await.unwrap();

    // Read SOCKS5 reply (10 bytes: VER REP RSV ATYP [4] BND.PORT [2])
    let mut reply = [0u8; 10];
    client.read_exact(&mut reply).await.unwrap();
    assert_eq!(reply[0], 0x05, "SOCKS5 version");
    assert_eq!(reply[1], 0x00, "SOCKS5 reply should be success");

    // Read echoed data from fake upstream
    let mut data = vec![0u8; 1024];
    let n = client.read(&mut data).await.unwrap();
    let response = String::from_utf8_lossy(&data[..n]);
    assert!(
        response.contains("uname=myuser"),
        "should pass username: {}",
        response
    );
    // The bridge intentionally reloads the session token from disk on every
    // connection (so re-auth'd tokens propagate to existing bridges),
    // ignoring the password passed at start. Expect the current on-disk token.
    let expected_pass =
        std::fs::read_to_string(proxybase_gui_lib::proxybase_dir().join("session_token"))
            .unwrap_or_default();
    assert!(
        response.contains(&format!("pass={}", expected_pass)),
        "should pass current session token: {}",
        response
    );
    assert!(
        response.contains("target=example.com:443"),
        "should pass target: {}",
        response
    );

    // 4. Stop the bridge
    proxybase_gui_lib::bridge::stop_bridge("test-session").await;
}

#[tokio::test]
async fn test_bridge_start_stop() {
    let upstream_port = fake_socks5_server().await;

    let port = proxybase_gui_lib::bridge::start_bridge(
        "test-2".to_string(),
        format!("127.0.0.1:{}", upstream_port),
        "u".to_string(),
        "p".to_string(),
        None,
    )
    .await
    .unwrap();

    assert!(port > 0);
    assert!(proxybase_gui_lib::bridge::bridge_port("test-2".to_string()).await.unwrap().is_some());

    proxybase_gui_lib::bridge::stop_bridge("test-2").await;

    // Give it a moment to shut down
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Port should no longer be tracked
    assert!(proxybase_gui_lib::bridge::bridge_port("test-2".to_string()).await.unwrap().is_none());
}

#[tokio::test]
async fn test_bridge_start_is_idempotent() {
    let upstream_port = fake_socks5_server().await;

    let first = proxybase_gui_lib::bridge::start_bridge(
        "idempotent".to_string(),
        format!("127.0.0.1:{}", upstream_port),
        "u".to_string(),
        "p".to_string(),
        None,
    )
    .await
    .unwrap();

    // A second start for the same session must return the existing port and
    // must NOT stop/restart the healthy bridge (that is what used to make the
    // port disappear).
    let second = proxybase_gui_lib::bridge::start_bridge(
        "idempotent".to_string(),
        format!("127.0.0.1:{}", upstream_port),
        "u".to_string(),
        "p".to_string(),
        Some(first),
    )
    .await
    .unwrap();

    assert_eq!(first, second, "bridge must not be restarted");
    assert_eq!(
        proxybase_gui_lib::bridge::bridge_port("idempotent".to_string()).await.unwrap(),
        Some(first)
    );

    proxybase_gui_lib::bridge::stop_bridge("idempotent").await;
}

#[tokio::test]
async fn test_bridge_restart_same_port_after_use() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let upstream_port = fake_socks5_server().await;
    let preferred = 23107u16; // fixed test port (distinct from other tests)

    let port = proxybase_gui_lib::bridge::start_bridge(
        "restart-port".to_string(),
        format!("127.0.0.1:{}", upstream_port),
        "u".to_string(),
        "p".to_string(),
        Some(preferred),
    )
    .await
    .expect("first bridge start");
    assert_eq!(port, preferred);

    // Use the bridge so relay sockets exist on the bridge port. After they
    // close, macOS leaves them in TIME_WAIT, which used to make an immediate
    // rebind of the same port fail with EADDRINUSE.
    let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).await.unwrap();
    client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
    let mut resp = [0u8; 2];
    client.read_exact(&mut resp).await.unwrap();
    assert_eq!(resp, [0x05, 0x00]);

    let domain = b"example.com";
    let mut req = vec![0x05, 0x01, 0x00, 0x03, domain.len() as u8];
    req.extend_from_slice(domain);
    req.extend_from_slice(&443u16.to_be_bytes());
    client.write_all(&req).await.unwrap();
    let mut reply = [0u8; 10];
    client.read_exact(&mut reply).await.unwrap();
    assert_eq!(reply[1], 0x00, "SOCKS5 connect should succeed");

    // Close the client and give the relay a moment to tear its sockets down.
    drop(client);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Stop and IMMEDIATELY restart on the same port. With SO_REUSEADDR the
    // rebind succeeds even while old connections are in TIME_WAIT (macOS).
    proxybase_gui_lib::bridge::stop_bridge("restart-port").await;
    let restarted = proxybase_gui_lib::bridge::start_bridge(
        "restart-port".to_string(),
        format!("127.0.0.1:{}", upstream_port),
        "u".to_string(),
        "p".to_string(),
        Some(preferred),
    )
    .await
    .expect("restart on same port must succeed");
    assert_eq!(restarted, preferred);

    proxybase_gui_lib::bridge::stop_bridge("restart-port").await;
}
