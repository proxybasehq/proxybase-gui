//! Desktop IPC client for `proxybase-tunneld`.
//!
//! Maintains one authenticated connection to the privileged daemon, correlates
//! requests by id, streams state/telemetry as Tauri events, and performs the
//! first-click privilege bootstrap (UAC / osascript / pkexec).

#![cfg(not(target_os = "android"))]

#[cfg(windows)]
use libproxybase::tunnel::config::TUNNELD_PIPE_NAME;
#[cfg(unix)]
use libproxybase::tunnel::config::TUNNELD_SOCKET_PATH;
use libproxybase::tunnel::config::{
    encode_ipc_frame, TunnelIpcEnvelope, TunnelIpcRequest, TunnelIpcResponse, MAX_IPC_FRAME,
};
use libproxybase::tunnel::{TunnelConfig, TunnelErrorCode, TunnelState};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{oneshot, Mutex, RwLock};

type BoxedRead = Box<dyn AsyncRead + Unpin + Send>;
type BoxedWrite = Box<dyn AsyncWrite + Unpin + Send>;

#[derive(Debug, Clone, Copy, Serialize, Default)]
pub struct TelemetryView {
    pub rx_bytes_total: u64,
    pub tx_bytes_total: u64,
    pub rx_bps: u64,
    pub tx_bps: u64,
    pub active_flows: u32,
}

static CLIENT: OnceLock<Arc<ClientState>> = OnceLock::new();

struct ClientState {
    app: AppHandle,
    writer: Mutex<Option<BoxedWrite>>,
    pending: Mutex<HashMap<u64, oneshot::Sender<TunnelIpcResponse>>>,
    next_id: AtomicU64,
    generation: AtomicU64,
    state: RwLock<TunnelState>,
    telemetry: RwLock<TelemetryView>,
    connecting: Mutex<()>,
    elevating: std::sync::atomic::AtomicBool,
}

/// Start the background connection manager. Safe to call once during setup.
pub fn init(app: AppHandle) {
    let state = Arc::new(ClientState {
        app,
        writer: Mutex::new(None),
        pending: Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(1),
        generation: AtomicU64::new(0),
        state: RwLock::new(TunnelState::Disconnected),
        telemetry: RwLock::new(TelemetryView::default()),
        connecting: Mutex::new(()),
        elevating: std::sync::atomic::AtomicBool::new(false),
    });
    if CLIENT.set(state.clone()).is_err() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        // Try once immediately (without elevating), then keep the connection
        // alive so the banner reflects daemon-side state changes.
        let _ = state.ensure_connected(false).await;
        let _ = state.refresh_state().await;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if state.writer.lock().await.is_none() {
                if state.ensure_connected(false).await.is_ok() {
                    let _ = state.refresh_state().await;
                }
            }
        }
    });
}

fn client() -> Result<Arc<ClientState>, String> {
    CLIENT
        .get()
        .cloned()
        .ok_or_else(|| "tunnel client is not initialized".to_string())
}

pub async fn emit_error(code: &str, message: &str) {
    if let Ok(c) = client() {
        c.emit_error(code, message).await;
    }
}

impl ClientState {
    async fn cached_state(&self) -> TunnelState {
        self.state.read().await.clone()
    }

    async fn emit_state(&self, state: TunnelState) {
        *self.state.write().await = state.clone();
        let _ = self.app.emit("tunnel://state-changed", &state);
    }

    async fn emit_error(&self, code: &str, message: &str) {
        let err_code = match code {
            "privilege_elevation_denied" => TunnelErrorCode::PrivilegeElevationDenied,
            "quota_exhausted" => TunnelErrorCode::QuotaExhausted,
            "lease_expired" => TunnelErrorCode::LeaseExpired,
            _ => TunnelErrorCode::InternalError,
        };
        self.emit_state(TunnelState::Error {
            code: err_code,
            message: message.to_string(),
        })
        .await;
    }

    async fn emit_telemetry(&self, value: TelemetryView) {
        *self.telemetry.write().await = value;
        let _ = self.app.emit("tunnel://telemetry", &value);
    }

    async fn reset(&self) {
        *self.writer.lock().await = None;
    }

    async fn refresh_state(&self) -> Result<(), String> {
        match self
            .send_request_with(TunnelIpcRequest::GetState, false)
            .await?
        {
            TunnelIpcResponse::State { state } => {
                self.emit_state(state).await;
                Ok(())
            }
            TunnelIpcResponse::Error { message, .. } => Err(message),
            other => Err(format!("unexpected response: {other:?}")),
        }
    }

    async fn send_ping(&self, timeout: Duration) -> Result<String, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let frame = encode_ipc_frame(&TunnelIpcEnvelope::Request {
            id,
            request: TunnelIpcRequest::Ping,
        })
        .map_err(|e| e.to_string())?;

        let sent = {
            let mut guard = self.writer.lock().await;
            match guard.as_mut() {
                Some(writer) => {
                    writer.write_all(&frame).await.is_ok() && writer.flush().await.is_ok()
                }
                None => false,
            }
        };
        if !sent {
            self.pending.lock().await.remove(&id);
            return Err("failed to write ping to IPC socket".into());
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(TunnelIpcResponse::Pong { version })) => Ok(version),
            Ok(Ok(TunnelIpcResponse::Error { message, .. })) => Err(message),
            Ok(Ok(other)) => Err(format!("unexpected ping response: {other:?}")),
            Ok(Err(_)) => {
                self.pending.lock().await.remove(&id);
                Err("IPC connection closed while waiting for ping response".into())
            }
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err("ping timed out".into())
            }
        }
    }

    async fn ensure_connected(&self, allow_elevation: bool) -> Result<(), String> {
        if self.writer.lock().await.is_some() {
            if !allow_elevation {
                return Ok(());
            }
            // A warm connection is not proof the daemon is current: the
            // Windows service keeps running across GUI updates, so the
            // connected process can be an older build. Verify it before using
            // it, otherwise route/DNS fixes never reach the service.
            let ping_res = self.send_ping(Duration::from_millis(2000)).await;
            return self.handle_ping_result(ping_res, allow_elevation).await;
        }
        let _guard = self.connecting.lock().await;
        if self.writer.lock().await.is_some() {
            if !allow_elevation {
                return Ok(());
            }
            let ping_res = self.send_ping(Duration::from_millis(2000)).await;
            return self.handle_ping_result(ping_res, allow_elevation).await;
        }
        match open_stream().await {
            Ok((reader, writer)) => {
                self.install(reader, writer).await;
                // Verify the connection with a fast ping. If ping fails or version differs,
                // the daemon is stale, crashed, or unauthorized.
                let ping_res = self.send_ping(Duration::from_millis(2000)).await;
                self.handle_ping_result(ping_res, allow_elevation).await
            }
            Err(e) if allow_elevation && daemon_absent(&e) => self.elevate_and_poll().await,
            Err(e) => Err(format!("cannot reach proxybase-tunneld: {e}")),
        }
    }

    async fn handle_ping_result(
        &self,
        ping_res: Result<String, String>,
        allow_elevation: bool,
    ) -> Result<(), String> {
        match ping_res {
            Ok(version) => {
                let app_version = env!("CARGO_PKG_VERSION");
                // Version equality is not enough: development builds and
                // rebuilt sidecars keep the same version. Compare the
                // installed service binary with the bundled sidecar so a
                // stale daemon cannot keep running old route/DNS code after
                // an app update.
                if allow_elevation && (version != app_version || daemon_binary_is_stale()) {
                    eprintln!(
                        "[tunnel] upgrading tunneld: running {version}, expected {app_version}"
                    );
                    self.reset().await;
                    self.elevate_and_poll().await
                } else {
                    Ok(())
                }
            }
            Err(e) if allow_elevation => {
                eprintln!("[tunnel] daemon ping failed ({e}); reinstalling service via elevation");
                self.reset().await;
                self.elevate_and_poll().await
            }
            Err(e) => {
                self.reset().await;
                Err(format!("cannot reach proxybase-tunneld: {e}"))
            }
        }
    }

    async fn install(&self, reader: BoxedRead, writer: BoxedWrite) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *self.writer.lock().await = Some(writer);
        let state = CLIENT.get().expect("client initialized").clone();
        tokio::spawn(async move {
            read_loop(reader, state, generation).await;
        });
    }

    async fn send_request(&self, request: TunnelIpcRequest) -> Result<TunnelIpcResponse, String> {
        self.send_request_with(request, true).await
    }

    async fn send_request_with(
        &self,
        request: TunnelIpcRequest,
        allow_elevation: bool,
    ) -> Result<TunnelIpcResponse, String> {
        for attempt in 0..2 {
            self.ensure_connected(allow_elevation).await?;
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let (tx, rx) = oneshot::channel();
            self.pending.lock().await.insert(id, tx);

            let frame = encode_ipc_frame(&TunnelIpcEnvelope::Request {
                id,
                request: clone_request(&request),
            })
            .map_err(|e| e.to_string())?;
            let sent = {
                let mut guard = self.writer.lock().await;
                match guard.as_mut() {
                    Some(writer) => {
                        writer.write_all(&frame).await.is_ok() && writer.flush().await.is_ok()
                    }
                    None => false,
                }
            };
            if !sent {
                self.pending.lock().await.remove(&id);
                self.reset().await;
                if attempt == 0 {
                    continue;
                }
                return Err("IPC connection closed".into());
            }

            match tokio::time::timeout(Duration::from_secs(90), rx).await {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(_)) => {
                    self.pending.lock().await.remove(&id);
                    self.reset().await;
                    if attempt == 0 {
                        continue;
                    }
                    return Err("proxybase-tunneld connection was closed".into());
                }
                Err(_) => {
                    self.pending.lock().await.remove(&id);
                    self.reset().await;
                    if attempt == 0 {
                        continue;
                    }
                    return Err("proxybase-tunneld did not respond in 90 seconds".into());
                }
            }
        }
        Err("IPC request failed".into())
    }

    async fn elevate_and_poll(&self) -> Result<(), String> {
        if self.elevating.swap(true, Ordering::SeqCst) {
            return Err("privilege prompt already in progress".into());
        }
        self.emit_state(TunnelState::RequestingPrivileges).await;
        let result = elevate().await;
        self.elevating.store(false, Ordering::SeqCst);
        if let Err(e) = result {
            self.emit_state(TunnelState::Error {
                code: TunnelErrorCode::PrivilegeElevationDenied,
                message: e.clone(),
            })
            .await;
            return Err(e);
        }

        // The installer starts the service; wait for the socket. Windows
        // service creation can take a few seconds on first install.
        for _ in 0..150 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if let Ok((reader, writer)) = open_stream().await {
                self.install(reader, writer).await;
                if self.send_ping(Duration::from_millis(2000)).await.is_ok() {
                    return Ok(());
                }
                self.reset().await;
            }
        }
        #[cfg_attr(not(target_os = "windows"), allow(unused_mut))]
        let mut message = "proxybase-tunneld did not start after elevation".to_string();
        #[cfg(target_os = "windows")]
        if let Some(detail) = read_install_error() {
            message.push_str(": ");
            message.push_str(&detail);
        }
        self.emit_state(TunnelState::Error {
            code: TunnelErrorCode::PrivilegeElevationDenied,
            message: message.clone(),
        })
        .await;
        Err(message)
    }
}

fn clone_request(request: &TunnelIpcRequest) -> TunnelIpcRequest {
    request.clone()
}

async fn read_loop(mut reader: BoxedRead, state: Arc<ClientState>, generation: u64) {
    loop {
        let frame = match read_frame(&mut reader).await {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(e) => {
                eprintln!("[tunnel] IPC read error: {e}");
                break;
            }
        };
        let envelope: TunnelIpcEnvelope = match serde_json::from_slice(&frame) {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[tunnel] malformed IPC frame: {e}");
                break;
            }
        };
        match envelope {
            TunnelIpcEnvelope::Response { id, response } => {
                if let Some(tx) = state.pending.lock().await.remove(&id) {
                    let _ = tx.send(response);
                }
            }
            TunnelIpcEnvelope::Event { response } => match response {
                TunnelIpcResponse::State { state: next } => state.emit_state(next).await,
                TunnelIpcResponse::Telemetry {
                    rx_bytes_total,
                    tx_bytes_total,
                    rx_bps,
                    tx_bps,
                    active_flows,
                } => {
                    state
                        .emit_telemetry(TelemetryView {
                            rx_bytes_total,
                            tx_bytes_total,
                            rx_bps,
                            tx_bps,
                            active_flows,
                        })
                        .await
                }
                TunnelIpcResponse::Error { code, message } => {
                    state.emit_state(TunnelState::Error { code, message }).await
                }
                TunnelIpcResponse::Pong { .. } => {}
            },
            TunnelIpcEnvelope::Request { .. } => {}
        }
    }

    // Only the connection that is still current may clear the writer or
    // report a disconnect (avoids a stale reader clobbering a new connection).
    if state.generation.load(Ordering::SeqCst) == generation {
        *state.writer.lock().await = None;
        state.pending.lock().await.clear();
        let current = state.cached_state().await;
        if current.is_active() {
            // An unexpected daemon exit may leave the kill-switch routes in
            // place. Never tell the user the tunnel is cleanly off.
            state
                .emit_state(TunnelState::Error {
                    code: TunnelErrorCode::InternalError,
                    message: "Lost connection to the tunnel daemon. System traffic may be blocked until the service is restarted or the tunnel is disconnected.".into(),
                })
                .await;
        }
    }
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

#[cfg(unix)]
async fn open_stream() -> std::io::Result<(BoxedRead, BoxedWrite)> {
    use tokio::net::UnixStream;
    let stream = UnixStream::connect(TUNNELD_SOCKET_PATH).await?;
    let (reader, writer) = stream.into_split();
    Ok((Box::new(reader), Box::new(writer)))
}

#[cfg(windows)]
async fn open_stream() -> std::io::Result<(BoxedRead, BoxedWrite)> {
    use tokio::net::windows::named_pipe::ClientOptions;
    const ERROR_PIPE_BUSY: i32 = 231;
    let mut last_err = None;
    for _ in 0..20 {
        match ClientOptions::new().open(TUNNELD_PIPE_NAME) {
            Ok(client) => {
                let (reader, writer) = tokio::io::split(client);
                return Ok((Box::new(reader), Box::new(writer)));
            }
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::from_raw_os_error(ERROR_PIPE_BUSY)))
}

fn daemon_absent(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
    )
}

// ---------------------------------------------------------------------------
// Public command surface
// ---------------------------------------------------------------------------

pub async fn start(config: TunnelConfig) -> Result<TunnelState, String> {
    let client = client()?;
    match client
        .send_request(TunnelIpcRequest::StartTunnel { config })
        .await?
    {
        TunnelIpcResponse::State { state } => {
            client.emit_state(state.clone()).await;
            Ok(state)
        }
        TunnelIpcResponse::Error { code, message } => {
            client
                .emit_state(TunnelState::Error {
                    code,
                    message: message.clone(),
                })
                .await;
            Err(message)
        }
        other => Err(format!("unexpected response: {other:?}")),
    }
}

pub async fn stop() -> Result<TunnelState, String> {
    let client = client()?;
    // Stopping must never trigger a privilege prompt: either the daemon is
    // already reachable, or there is no tunnel to stop.
    match client
        .send_request_with(TunnelIpcRequest::StopTunnel, false)
        .await?
    {
        TunnelIpcResponse::State { state } => {
            client.emit_state(state.clone()).await;
            Ok(state)
        }
        TunnelIpcResponse::Error { message, .. } => Err(message),
        other => Err(format!("unexpected response: {other:?}")),
    }
}

pub async fn status() -> Result<TunnelState, String> {
    let client = client()?;
    match client
        .send_request_with(TunnelIpcRequest::GetState, false)
        .await
    {
        Ok(TunnelIpcResponse::State { state }) => {
            client.emit_state(state.clone()).await;
            Ok(state)
        }
        Ok(TunnelIpcResponse::Error { message, .. }) => Err(message),
        Ok(other) => Err(format!("unexpected response: {other:?}")),
        Err(_) => Ok(client.cached_state().await),
    }
}

pub async fn cached_telemetry() -> TelemetryView {
    match CLIENT.get() {
        Some(c) => *c.telemetry.read().await,
        None => TelemetryView::default(),
    }
}

// ---------------------------------------------------------------------------
// Privilege bootstrap
// ---------------------------------------------------------------------------

/// True when the running service is a different binary than the one bundled
/// with this GUI (same version, rebuilt locally or via an app update).
///
/// Version equality alone is not enough: a dev build or a hot-fixed sidecar
/// keeps the same version string, and leaving the old daemon running would
/// silently keep old route/DNS code.
fn daemon_binary_is_stale() -> bool {
    let Some(bundled) = sidecar_path() else {
        return false;
    };

    #[cfg(target_os = "windows")]
    let installed = std::env::var_os("ProgramFiles")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Program Files"))
        .join("ProxyBase")
        .join("tunneld")
        .join("proxybase-tunneld.exe");
    #[cfg(target_os = "macos")]
    let installed =
        std::path::PathBuf::from("/Library/Application Support/ProxyBase/proxybase-tunneld");
    #[cfg(target_os = "linux")]
    let installed = std::path::PathBuf::from("/usr/local/lib/proxybase/proxybase-tunneld");
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let installed = std::path::PathBuf::new();

    binary_differs(&bundled, &installed)
}

fn binary_differs(bundled: &std::path::Path, installed: &std::path::Path) -> bool {
    use std::hash::{Hash, Hasher};

    let Ok(bundled_meta) = std::fs::metadata(bundled) else {
        // No bundled sidecar to install; leave the running service alone.
        return false;
    };
    if !installed.exists() {
        // Bundled sidecar exists but nothing is installed yet.
        return true;
    }
    let Ok(installed_meta) = std::fs::metadata(installed) else {
        return false;
    };
    if bundled_meta.len() != installed_meta.len() {
        return true;
    }
    let hash = |path: &std::path::Path| -> Option<u64> {
        let bytes = std::fs::read(path).ok()?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        Some(hasher.finish())
    };
    match (hash(bundled), hash(installed)) {
        (Some(bundled), Some(installed)) => bundled != installed,
        _ => false,
    }
}

fn sidecar_path() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("PROXYBASE_TUNNELD_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(windows) {
        "proxybase-tunneld.exe"
    } else {
        "proxybase-tunneld"
    };

    let direct_candidates = [
        dir.join(name),
        dir.join("binaries").join(name),
        dir.join("resources").join(name),
        dir.join("resources").join("binaries").join(name),
        dir.join("..").join("resources").join(name),
    ];
    for candidate in &direct_candidates {
        if candidate.exists() {
            return Some(candidate.clone());
        }
    }

    #[cfg(windows)]
    {
        let program_files = std::env::var_os("ProgramFiles")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Program Files"));
        let installed = program_files.join("ProxyBase").join("tunneld").join(name);
        if installed.exists() {
            return Some(installed);
        }
        if let Some(pf86) = std::env::var_os("ProgramFiles(x86)").map(std::path::PathBuf::from) {
            let installed86 = pf86.join("ProxyBase").join("tunneld").join(name);
            if installed86.exists() {
                return Some(installed86);
            }
        }
    }

    // Check dev/build binaries directory if present (e.g. debug or release cargo builds)
    let dev_binaries = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
    if let Ok(entries) = std::fs::read_dir(&dev_binaries) {
        for entry in entries.flatten() {
            let candidate = entry.path();
            if candidate
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("proxybase-tunneld-"))
                .unwrap_or(false)
            {
                return Some(candidate);
            }
        }
    }

    // Tauri externalBin layout (target-triple suffixed) in runtime folders
    let search_dirs = [
        dir.to_path_buf(),
        dir.join("binaries"),
        dir.join("resources"),
        dir.join("..").join("resources"),
    ];
    for d in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(d) {
            for entry in entries.flatten() {
                let candidate = entry.path();
                if candidate
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("proxybase-tunneld-"))
                    .unwrap_or(false)
                {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

async fn elevate() -> Result<(), String> {
    let sidecar = sidecar_path()
        .ok_or_else(|| "proxybase-tunneld sidecar not found next to the application".to_string())?;
    if !sidecar.exists() {
        return Err(format!("sidecar not found at {}", sidecar.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata(&sidecar) {
            if meta.mode() & 0o022 != 0 {
                return Err(format!(
                    "refusing to elevate {}: it is writable by other users",
                    sidecar.display()
                ));
            }
        }
    }

    #[cfg(unix)]
    let uid = unsafe { libc::getuid() } as u32;
    #[cfg(unix)]
    let uid_arg = uid.to_string();
    let gui_path =
        std::env::current_exe().map_err(|e| format!("cannot locate GUI executable: {e}"))?;
    let gui_path_text = gui_path.display().to_string();

    #[cfg(target_os = "macos")]
    {
        let shell_path = shell_single_quote(&sidecar.display().to_string());
        let shell_gui = shell_single_quote(&gui_path_text);
        let script = format!(
            "do shell script \"{} --install-launchd --allowed-uid {} --gui-path {}\" with administrator privileges",
            apple_script_escape(&shell_path),
            uid_arg,
            apple_script_escape(&shell_gui)
        );
        let output = tokio::process::Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(script)
            .output()
            .await
            .map_err(|e| format!("failed to launch osascript: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("-128") {
                return Err("administrator privileges were declined".into());
            }
            return Err(format!("privileged install failed: {}", stderr.trim()));
        }
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Stdio;

        // pkexec executes the installer as root. When the GUI runs from an
        // AppImage, the daemon lives on a FUSE mount that only the mounting
        // user can access, so root gets EACCES trying to exec it. Stream the
        // binary over stdin into a root-owned path via trusted system tools,
        // then exec the root-owned copy. This also avoids executing a
        // user-writable staging file as root and works when /tmp is noexec.
        // Write to a sibling and rename over the destination: opening a
        // currently-running daemon binary for writing fails with ETXTBSY, but
        // rename(2) is allowed because the running process keeps the old inode.
        let script = format!(
            "/bin/mkdir -p /usr/local/lib/proxybase && \
             /bin/cat > /usr/local/lib/proxybase/proxybase-tunneld.new && \
             /bin/chmod 0755 /usr/local/lib/proxybase/proxybase-tunneld.new && \
             /bin/mv -f /usr/local/lib/proxybase/proxybase-tunneld.new \
                      /usr/local/lib/proxybase/proxybase-tunneld && \
             exec /usr/local/lib/proxybase/proxybase-tunneld \
             --install-systemd --allowed-uid {} --gui-path {}",
            shell_single_quote(&uid_arg),
            shell_single_quote(&gui_path_text)
        );

        let mut child = tokio::process::Command::new("pkexec")
            .arg("/bin/sh")
            .arg("-c")
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to launch pkexec: {e}"))?;

        let write_result = async {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| "pkexec stdin unavailable".to_string())?;
            let bytes = tokio::fs::read(&sidecar)
                .await
                .map_err(|e| format!("cannot read {}: {e}", sidecar.display()))?;
            stdin
                .write_all(&bytes)
                .await
                .map_err(|e| format!("cannot stream the daemon to the installer: {e}"))?;
            Ok::<(), String>(())
        }
        .await;
        // The block above ends with stdin dropped, sending EOF so `cat`
        // completes. If the user dismisses the polkit prompt, pkexec may close
        // the pipe first; prefer its stderr over the write error.

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| format!("failed to wait for pkexec: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = stderr.trim();
            if stderr.is_empty() {
                return Err(format!(
                    "privileged install failed (exit status {})",
                    output.status
                ));
            }
            return Err(format!("privileged install failed: {stderr}"));
        }
        write_result?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        // ShellExecuteExW + WaitForSingleObject block; keep them off the async
        // runtime threads.
        return tokio::task::spawn_blocking(move || windows_elevate(&sidecar, &gui_path))
            .await
            .map_err(|e| format!("elevation task failed: {e}"))?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (sidecar, uid_arg);
        Err("unsupported platform".into())
    }
}

#[cfg(unix)]
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
fn apple_script_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "windows")]
fn install_error_path() -> std::path::PathBuf {
    std::env::var_os("ProgramData")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"))
        .join("ProxyBase")
        .join("last_install_error.txt")
}

#[cfg(target_os = "windows")]
fn read_install_error() -> Option<String> {
    std::fs::read_to_string(install_error_path())
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

#[cfg(target_os = "windows")]
fn windows_elevate(sidecar: &std::path::Path, gui_path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    // Remove any stale diagnostic from a previous attempt so a failure below
    // is attributable to this run.
    let _ = std::fs::remove_file(install_error_path());

    let file = wide(sidecar.as_os_str());
    let verb = wide(std::ffi::OsStr::new("runas"));
    let params = wide(&std::ffi::OsStr::new(&format!(
        "--install-and-start --gui-path \"{}\"",
        gui_path.display()
    )));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_HIDE;

    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok == 0 {
        let code = unsafe { GetLastError() };
        if code == 1223 {
            return Err("administrator privileges were declined".into());
        }
        return Err(format!("elevation failed with Win32 error {code}"));
    }
    let process: HANDLE = info.hProcess;
    if process.is_null() {
        return Err("elevation started but Windows returned no process handle".into());
    }

    // Installation is quick (copy + service registration); never hang forever
    // if UAC is dismissed or the process is stuck.
    let wait = unsafe { WaitForSingleObject(process, 120_000) };
    if wait == WAIT_TIMEOUT {
        unsafe { CloseHandle(process) };
        return Err("timed out waiting for the privileged installer".into());
    }
    let mut exit_code = 1u32;
    let got_exit = unsafe { GetExitCodeProcess(process, &mut exit_code) };
    unsafe { CloseHandle(process) };
    if wait != WAIT_OBJECT_0 || got_exit == 0 {
        return Err("could not determine the privileged installer result".into());
    }
    if exit_code != 0 {
        if let Some(detail) = read_install_error() {
            return Err(format!("privileged install failed: {detail}"));
        }
        return Err(format!("privileged install failed (exit code {exit_code})"));
    }
    Ok(())
}
