//! Windows Service Control Manager integration.
//!
//! The service runs as LocalSystem. The named-pipe DACL (see `peer_auth`) and
//! the client executable check ensure that only the interactive ProxyBase GUI
//! can control it.

use anyhow::Context;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;
use windows_service::service::{
    Service, ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl,
    ServiceExitCode, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

pub const SERVICE_NAME: &str = "ProxyBaseTunnelService";
const DISPLAY_NAME: &str = "ProxyBase System Tunnel";

windows_service::define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        tracing::error!(error = %e, "ProxyBase tunnel service stopped with an error");
        // Log to the Windows event log would require an extra dependency; the
        // service exits non-zero so the SCM restart policy applies.
        std::process::exit(1);
    }
}

fn run_service() -> anyhow::Result<()> {
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();
    let status_handle =
        service_control_handler::register(SERVICE_NAME, move |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        })?;

    let status =
        |state: ServiceState, accepted: ServiceControlAccept, checkpoint: u32| ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accepted,
            exit_code: ServiceExitCode::NO_ERROR,
            checkpoint,
            wait_hint: Duration::from_secs(30),
            process_id: None,
        };
    status_handle.set_service_status(status(
        ServiceState::StartPending,
        ServiceControlAccept::empty(),
        1,
    ))?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let result: anyhow::Result<()> = runtime.block_on(async {
        // Crash recovery MUST run before the IPC socket opens.
        libproxybase::tunnel::journal::recover_if_dirty()
            .await
            .map_err(|e| anyhow::anyhow!("journal recovery failed: {e}"))?;
        let engine = std::sync::Arc::new(libproxybase::tunnel::SystemTunnelEngine::new());
        let serve_engine = engine.clone();
        let serve = tokio::spawn(async move { crate::ipc_server::serve(serve_engine).await });
        // The SCM requires an explicit StartPending -> Running transition;
        // without it the service is killed with error 1053 after ~30 s and the
        // installer times out waiting for the Running state.
        status_handle.set_service_status(status(
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            0,
        ))?;
        let stop = tokio::task::spawn_blocking(move || {
            let _ = shutdown_rx.recv();
        });
        let outcome = tokio::select! {
            result = serve => result,
            _ = stop => Ok(Ok(())),
        };
        let _ = engine.stop().await;
        match outcome {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(anyhow::anyhow!("service task failed: {e}")),
        }
    });

    let _ = status_handle.set_service_status(status(
        ServiceState::Stopped,
        ServiceControlAccept::empty(),
        0,
    ));

    result
}

pub fn run_daemon() -> anyhow::Result<()> {
    windows_service::service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .context("failed to connect to the Windows Service Control Manager")
}

fn install_dir() -> PathBuf {
    let program_files = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    // Use a dedicated subdirectory. Tauri installs the sidecar next to the GUI
    // executable in `Program Files\ProxyBase`; installing the daemon there
    // would make the installer overwrite the very file it is running from,
    // which Windows forbids.
    program_files.join("ProxyBase").join("tunneld")
}

fn installed_binary() -> PathBuf {
    install_dir().join("proxybase-tunneld.exe")
}

fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn replace_file(tmp: &std::path::Path, dest: &std::path::Path) -> anyhow::Result<()> {
    // The previous service may still be releasing the executable/DLL for a
    // moment after reaching the Stopped state.
    let mut last_error = None;
    for _ in 0..25 {
        match std::fs::rename(tmp, dest) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_error = Some(e);
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
    Err(anyhow::anyhow!(
        "could not replace {}: {:?}",
        dest.display(),
        last_error
    ))
}

fn copy_binary() -> anyhow::Result<PathBuf> {
    let current = std::env::current_exe().context("cannot locate tunneld executable")?;
    let dest = installed_binary();
    std::fs::create_dir_all(install_dir())?;

    if !same_file(&current, &dest) {
        let tmp = install_dir().join(format!("proxybase-tunneld.exe.new.{}", std::process::id()));
        std::fs::copy(&current, &tmp).with_context(|| {
            format!("failed to copy {} -> {}", current.display(), tmp.display())
        })?;
        replace_file(&tmp, &dest)?;
    }

    // tun-rs loads wintun.dll from the executable directory. Tauri ships the
    // signed DLLs as resources; copy the matching architecture next to the
    // installed service binary.
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "amd64"
    };
    let exe_dir = current
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let candidates = [
        exe_dir.join("wintun.dll"),
        exe_dir
            .join("resources")
            .join("wintun")
            .join(arch)
            .join("wintun.dll"),
        exe_dir
            .join("..")
            .join("resources")
            .join("wintun")
            .join(arch)
            .join("wintun.dll"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            let target = install_dir().join("wintun.dll");
            std::fs::copy(&candidate, &target)?;
            break;
        }
    }
    Ok(dest)
}

/// Wait until the SCM reports the service stopped (or no longer exists).
/// Transient query errors are retried: deleting a service that is still
/// releasing its executable would make the following copy fail.
fn wait_until_stopped(service: &Service) {
    const ERROR_SERVICE_DOES_NOT_EXIST: i32 = 1060;
    let mut terminated = false;
    for attempt in 0..175 {
        match service.query_status() {
            Ok(status) if status.current_state == ServiceState::Stopped => return,
            Err(windows_service::Error::Winapi(e))
                if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST) =>
            {
                return
            }
            Ok(status) => {
                // A service stuck in StartPending does not accept Stop, and
                // the SCM would only kill it after its ~30 s startup timeout.
                // This is our own service, so terminate it directly instead of
                // blocking the installer.
                if !terminated && attempt >= 75 {
                    if let Some(pid) = status.process_id {
                        terminate_process(pid);
                    }
                    terminated = true;
                }
            }
            Err(_) => {}
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn terminate_process(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if handle.is_null() {
        return;
    }
    unsafe {
        let _ = TerminateProcess(handle, 1);
        CloseHandle(handle);
    }
}

pub fn install(allowed_uid: Option<u32>, gui_path: Option<&std::path::Path>) -> anyhow::Result<()> {
    // The interactive-user DACL makes a per-uid check unnecessary, but the
    // installer still records it for consistency with Unix builds.
    if let Some(uid) = allowed_uid {
        if uid == 0 {
            anyhow::bail!("refusing to authorize uid 0");
        }
    }
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    // Stop and delete the previous service BEFORE replacing its executable.
    // Windows locks the image of a running service, and an old service also
    // holds the named pipe, which would make the new instance fail to bind.
    if let Ok(existing) = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        let _ = existing.stop();
        wait_until_stopped(&existing);
        let _ = existing.delete();
        drop(existing);
    }

    let binary = copy_binary()?;
    if let Some(gui) = gui_path {
        let dir = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
            .join("ProxyBase");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("gui_path"), gui.to_string_lossy().as_bytes())?;
    }

    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: binary,
        launch_arguments: vec![OsString::from("--run-daemon")],
        dependencies: vec![],
        account_name: None, // LocalSystem
        account_password: None,
    };

    // Service deletion is asynchronous in the SCM. Retry creation until the old
    // registration is gone instead of failing with ERROR_SERVICE_MARKED_FOR_DELETE.
    let mut service = None;
    let mut create_error = None;
    for _ in 0..75 {
        match manager.create_service(
            &info,
            ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::QUERY_STATUS,
        ) {
            Ok(created) => {
                service = Some(created);
                break;
            }
            Err(e) => {
                create_error = Some(e);
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
    let service = service
        .ok_or_else(|| anyhow::anyhow!("could not create the tunnel service: {create_error:?}"))?;

    service
        .start(&[] as &[&std::ffi::OsStr])
        .context("failed to start ProxyBaseTunnelService")?;

    // Report startup failures now (the pipe server might never appear) instead
    // of leaving the GUI to poll for ten seconds and time out.
    let mut last_state = None;
    for _ in 0..150 {
        match service.query_status() {
            Ok(status) if status.current_state == ServiceState::Running => return Ok(()),
            Ok(status) if status.current_state == ServiceState::Stopped => {
                anyhow::bail!(
                    "ProxyBaseTunnelService stopped during startup: {:?}",
                    status.exit_code
                );
            }
            Ok(status) => last_state = Some(status.current_state),
            Err(e) => {
                create_error = Some(e);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    anyhow::bail!(
        "ProxyBaseTunnelService did not reach the Running state (last state: {last_state:?}): \
         {create_error:?}"
    )
}

pub fn uninstall() -> anyhow::Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    if let Ok(service) = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        let _ = service.stop();
        wait_until_stopped(&service);
        let _ = service.delete();
        drop(service);
    }
    std::thread::sleep(Duration::from_millis(300));
    let _ = std::fs::remove_file(installed_binary());
    let _ = std::fs::remove_dir_all(install_dir());
    Ok(())
}
