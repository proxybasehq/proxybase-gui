//! Authenticated local IPC server.
//!
//! Protocol: 4-byte big-endian length + JSON `TunnelIpcEnvelope`. Requests and
//! responses carry an id; state and telemetry are pushed as `Event` frames on
//! the same connection.
//!
//! If the connection that issued `StartTunnel` closes (GUI crash, `kill -9`,
//! task-manager force quit), the tunnel is torn down within milliseconds when
//! `config.stop_on_gui_exit` is set.

use crate::peer_auth;
use libproxybase::tunnel::config::{
    encode_ipc_frame, TunnelIpcEnvelope, TunnelIpcRequest, TunnelIpcResponse, MAX_IPC_FRAME,
};
use libproxybase::tunnel::{SystemTunnelEngine, TunnelConfig, TunnelErrorCode, TunnelState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex};

#[cfg(unix)]
pub const SOCKET_PATH: &str = "/var/run/proxybase-tunneld.sock";
#[cfg(windows)]
pub const PIPE_NAME: &str = r"\\.\pipe\proxybase-tunneld";

pub async fn serve(engine: Arc<SystemTunnelEngine>) -> anyhow::Result<()> {
    let service = Arc::new(Service::new(engine.clone()));

    // Forward engine state and telemetry to every connected GUI.
    {
        let service = service.clone();
        let mut states = engine.subscribe_state();
        let mut telemetry = engine.subscribe_telemetry();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = states.recv() => {
                        match result {
                            Ok(state) => service.broadcast(TunnelIpcResponse::State { state }),
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            Err(_) => continue,
                        }
                    }
                    result = telemetry.recv() => {
                        match result {
                            Ok(t) => service.broadcast(TunnelIpcResponse::Telemetry {
                                rx_bytes_total: t.rx_bytes_total,
                                tx_bytes_total: t.tx_bytes_total,
                                rx_bps: t.rx_bps,
                                tx_bps: t.tx_bps,
                                active_flows: t.active_flows,
                            }),
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            Err(_) => continue,
                        }
                    }
                }
            }
        });
    }

    #[cfg(unix)]
    {
        serve_unix(service).await
    }
    #[cfg(windows)]
    {
        serve_windows(service).await
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = service;
        anyhow::bail!("unsupported platform")
    }
}

struct Service {
    engine: Arc<SystemTunnelEngine>,
    connections: std::sync::Mutex<HashMap<u64, mpsc::UnboundedSender<TunnelIpcEnvelope>>>,
    next_id: AtomicU64,
    owner: Mutex<Option<(u64, TunnelConfig)>>,
}

impl Service {
    fn new(engine: Arc<SystemTunnelEngine>) -> Self {
        Self {
            engine,
            connections: std::sync::Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            owner: Mutex::new(None),
        }
    }

    fn register(&self, id: u64, tx: mpsc::UnboundedSender<TunnelIpcEnvelope>) {
        if let Ok(mut conns) = self.connections.lock() {
            conns.insert(id, tx);
        }
    }

    fn broadcast(&self, response: TunnelIpcResponse) {
        if let Ok(conns) = self.connections.lock() {
            for tx in conns.values() {
                let _ = tx.send(TunnelIpcEnvelope::Event {
                    response: response.clone(),
                });
            }
        }
    }

    async fn disconnect(&self, id: u64) {
        if let Ok(mut conns) = self.connections.lock() {
            conns.remove(&id);
        }
        let mut owner = self.owner.lock().await;
        let is_owner = owner.as_ref().map(|(o, _)| *o) == Some(id);
        if is_owner {
            let config = owner.take().map(|(_, c)| c);
            if config.map(|c| c.stop_on_gui_exit).unwrap_or(false) {
                tracing::info!(connection = id, "GUI disconnected; tearing down tunnel");
                if let Err(e) = self.engine.stop().await {
                    tracing::warn!(error = %e, "teardown after GUI exit failed");
                }
            }
        }
    }

    async fn handle(&self, id: u64, request: TunnelIpcRequest) -> TunnelIpcResponse {
        match request {
            TunnelIpcRequest::Ping => TunnelIpcResponse::Pong {
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            TunnelIpcRequest::GetState => TunnelIpcResponse::State {
                state: self.engine.state().await,
            },
            TunnelIpcRequest::StartTunnel { config } => {
                if let Err(e) = config.validate() {
                    return TunnelIpcResponse::error(TunnelErrorCode::InternalError, e);
                }
                let state = self.engine.state().await;
                if matches!(state, TunnelState::Error { .. }) {
                    // The kill switch may still hold routes; tear down before
                    // starting a fresh session.
                    let _ = self.engine.stop().await;
                }
                let state = self.engine.state().await;
                let result = if matches!(
                    state,
                    TunnelState::Connected { .. } | TunnelState::Reconnecting { .. }
                ) {
                    // Clicking "Tunnel System Traffic" on another proxy is a
                    // hot swap, not a reconnect.
                    self.engine.hot_swap(config.clone()).await
                } else if state.is_active() {
                    Err(libproxybase::tunnel::TunnelError {
                        code: TunnelErrorCode::InternalError,
                        message: "tunnel is already starting".into(),
                    })
                } else {
                    self.engine.start(config.clone()).await
                };
                match result {
                    Ok(()) => {
                        *self.owner.lock().await = Some((id, config));
                        TunnelIpcResponse::State {
                            state: self.engine.state().await,
                        }
                    }
                    Err(e) => TunnelIpcResponse::error(e.code, e.message),
                }
            }
            TunnelIpcRequest::HotSwapProxy { config } => {
                if let Err(e) = config.validate() {
                    return TunnelIpcResponse::error(TunnelErrorCode::InternalError, e);
                }
                match self.engine.hot_swap(config.clone()).await {
                    Ok(()) => {
                        let mut owner = self.owner.lock().await;
                        if owner.as_ref().map(|(o, _)| *o) == Some(id) {
                            *owner = Some((id, config));
                        }
                        TunnelIpcResponse::State {
                            state: self.engine.state().await,
                        }
                    }
                    Err(e) => TunnelIpcResponse::error(e.code, e.message),
                }
            }
            TunnelIpcRequest::StopTunnel => {
                let result = self.engine.stop().await;
                *self.owner.lock().await = None;
                match result {
                    Ok(()) => TunnelIpcResponse::State {
                        state: TunnelState::Disconnected,
                    },
                    Err(e) => TunnelIpcResponse::error(e.code, e.message),
                }
            }
        }
    }
}

async fn handle_io<R, W>(
    mut reader: R,
    mut writer: W,
    conn_id: u64,
    service: Arc<Service>,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (tx, mut rx) = mpsc::unbounded_channel::<TunnelIpcEnvelope>();
    service.register(conn_id, tx.clone());

    let writer_task = tokio::spawn(async move {
        while let Some(envelope) = rx.recv().await {
            match encode_ipc_frame(&envelope) {
                Ok(frame) => {
                    if writer.write_all(&frame).await.is_err() {
                        break;
                    }
                    let _ = writer.flush().await;
                }
                Err(_) => break,
            }
        }
    });

    loop {
        let frame = match read_frame(&mut reader).await {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(e) => {
                tracing::debug!(connection = conn_id, error = %e, "IPC read failed");
                break;
            }
        };
        let envelope: TunnelIpcEnvelope = match serde_json::from_slice(&frame) {
            Ok(env) => env,
            Err(e) => {
                tracing::warn!(connection = conn_id, error = %e, "malformed IPC frame");
                break;
            }
        };
        let TunnelIpcEnvelope::Request { id, request } = envelope else {
            continue;
        };
        let response = service.handle(conn_id, request).await;
        let reply = TunnelIpcEnvelope::Response { id, response };
        if tx.send(reply).is_err() {
            break;
        }
    }

    service.disconnect(conn_id).await;
    writer_task.abort();
    Ok(())
}

async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e)
            if e.kind() == std::io::ErrorKind::UnexpectedEof
                || e.kind() == std::io::ErrorKind::BrokenPipe
                || e.kind() == std::io::ErrorKind::ConnectionReset =>
        {
            return Ok(None);
        }
        Err(e) => return Err(e),
    }
    let n = u32::from_be_bytes(len) as usize;
    if n == 0 || n > MAX_IPC_FRAME {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid IPC frame length",
        ));
    }
    let mut buf = vec![0u8; n];
    match reader.read_exact(&mut buf).await {
        Ok(_) => {}
        Err(e)
            if e.kind() == std::io::ErrorKind::UnexpectedEof
                || e.kind() == std::io::ErrorKind::BrokenPipe
                || e.kind() == std::io::ErrorKind::ConnectionReset =>
        {
            return Ok(None);
        }
        Err(e) => return Err(e),
    }
    Ok(Some(buf))
}

// ---------------------------------------------------------------------------
// Unix
// ---------------------------------------------------------------------------

#[cfg(unix)]
async fn serve_unix(service: Arc<Service>) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use tokio::net::{UnixListener, UnixStream};

    let path = std::path::Path::new(SOCKET_PATH);
    if path.exists() {
        match UnixStream::connect(path).await {
            Ok(_) => anyhow::bail!(
                "another proxybase-tunneld is already listening on {}",
                path.display()
            ),
            Err(_) => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    if let Some(uid) = peer_auth::allowed_uid() {
        chown(path, uid)?;
    }
    tracing::info!(socket = %path.display(), "proxybase-tunneld IPC server listening");

    loop {
        let (stream, _) = listener.accept().await?;
        let peer = match peer_auth::verify_unix_peer(&stream) {
            Ok(peer) => peer,
            Err(e) => {
                tracing::warn!(error = %e, "rejected IPC peer");
                continue;
            }
        };
        let id = service.next_id.fetch_add(1, Ordering::Relaxed);
        tracing::info!(
            connection = id,
            uid = peer.uid,
            pid = peer.pid,
            exe = ?peer.executable,
            "accepted authenticated IPC connection"
        );
        let (reader, writer) = stream.into_split();
        let service = service.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_io(reader, writer, id, service).await {
                tracing::debug!(connection = id, error = %e, "IPC connection ended");
            }
        });
    }
}

#[cfg(unix)]
fn chown(path: &std::path::Path, uid: u32) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes())?;
    let ret = unsafe { libc::chown(c.as_ptr(), uid as libc::uid_t, u32::MAX) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(windows)]
async fn serve_windows(service: Arc<Service>) -> anyhow::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;

    tracing::info!(pipe = %PIPE_NAME, "proxybase-tunneld IPC server listening");
    loop {
        let mut options = ServerOptions::new();
        // Do not use first_pipe_instance(true): a stale daemon or a previous
        // service instance can still hold an instance and make creation fail
        // with ERROR_ACCESS_DENIED. The DACL + peer executable check are the
        // security boundary, not the instance count.
        options.reject_remote_clients(true);
        let server = match create_pipe_instance(&options) {
            Ok(server) => server,
            Err(e) => {
                // A transient instance conflict must not kill the service:
                // without a server the GUI could never connect again.
                tracing::warn!(error = %e, "could not create IPC pipe instance; retrying");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        };
        if let Err(e) = server.connect().await {
            // The client may have closed between open and connect.
            tracing::debug!(error = %e, "IPC pipe connect failed");
            continue;
        }
        let peer = match peer_auth::verify_windows_pipe_client(&server) {
            Ok(peer) => peer,
            Err(e) => {
                tracing::warn!(error = %e, "rejected IPC peer");
                let reply = TunnelIpcEnvelope::Response {
                    id: 0,
                    response: TunnelIpcResponse::Error {
                        code: TunnelErrorCode::InternalError,
                        message: format!("peer authentication failed: {e}"),
                    },
                };
                if let Ok(frame) = encode_ipc_frame(&reply) {
                    let mut server = server;
                    let _ = server.write_all(&frame).await;
                    let _ = server.flush().await;
                }
                continue;
            }
        };
        let id = service.next_id.fetch_add(1, Ordering::Relaxed);
        tracing::info!(
            connection = id,
            pid = peer.pid,
            exe = ?peer.executable,
            "accepted authenticated IPC connection"
        );
        let (reader, writer) = tokio::io::split(server);
        let service = service.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_io(reader, writer, id, service).await {
                tracing::debug!(connection = id, error = %e, "IPC connection ended");
            }
        });
    }
}

#[cfg(windows)]
fn create_pipe_instance(
    options: &tokio::net::windows::named_pipe::ServerOptions,
) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    // The DACL must be supplied at creation time: the handle returned by
    // CreateNamedPipe has no WRITE_DAC, so SetSecurityInfo would fail with
    // ERROR_ACCESS_DENIED. The kernel copies the descriptor during the call,
    // so `security` only needs to live until this function returns.
    let mut security = peer_auth::PipeSecurity::new()?;
    // SAFETY: `security` points at a valid SECURITY_ATTRIBUTES for the
    // duration of the call.
    unsafe { options.create_with_security_attributes_raw(PIPE_NAME, security.as_mut_ptr()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ping_get_state_and_stop_are_well_formed() {
        let engine = Arc::new(SystemTunnelEngine::new());
        let service = Service::new(engine);

        match service.handle(1, TunnelIpcRequest::Ping).await {
            TunnelIpcResponse::Pong { version } => assert!(!version.is_empty()),
            other => panic!("unexpected response: {other:?}"),
        }
        match service.handle(1, TunnelIpcRequest::GetState).await {
            TunnelIpcResponse::State {
                state: TunnelState::Disconnected,
            } => {}
            other => panic!("unexpected response: {other:?}"),
        }
        match service.handle(1, TunnelIpcRequest::StopTunnel).await {
            TunnelIpcResponse::State {
                state: TunnelState::Disconnected,
            } => {}
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn invalid_config_is_rejected_without_starting() {
        let engine = Arc::new(SystemTunnelEngine::new());
        let service = Service::new(engine);
        let mut config = TunnelConfig::default();
        config.proxy_id = "session".into();
        config.custom_bypass_cidrs = vec!["nonsense".into()];
        match service
            .handle(1, TunnelIpcRequest::StartTunnel { config })
            .await
        {
            TunnelIpcResponse::Error { code, .. } => {
                assert_eq!(code, TunnelErrorCode::InternalError)
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }
}
