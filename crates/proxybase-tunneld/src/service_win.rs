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
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
    ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
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
    let status_handle = service_control_handler::register(SERVICE_NAME, move |control| {
        match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    })?;

    let status = |state: ServiceState, accepted: ServiceControlAccept, checkpoint: u32| {
        ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accepted,
            exit_code: ServiceExitCode::NO_ERROR,
            checkpoint,
            wait_hint: Duration::from_secs(30),
            process_id: None,
        }
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
    program_files.join("ProxyBase")
}

fn installed_binary() -> PathBuf {
    install_dir().join("proxybase-tunneld.exe")
}

fn copy_binary() -> anyhow::Result<PathBuf> {
    let current = std::env::current_exe().context("cannot locate tunneld executable")?;
    let dest = installed_binary();
    std::fs::create_dir_all(install_dir())?;
    let tmp = dest.with_extension("exe.new");
    std::fs::copy(&current, &tmp)?;
    std::fs::rename(&tmp, &dest)?;

    // tun-rs loads wintun.dll from the executable directory. Tauri ships the
    // signed DLLs as resources; copy the matching architecture next to the
    // installed service binary.
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "amd64"
    };
    let exe_dir = current.parent().unwrap_or_else(|| std::path::Path::new("."));
    let candidates = [
        exe_dir.join("wintun.dll"),
        exe_dir.join("resources").join("wintun").join(arch).join("wintun.dll"),
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

pub fn install(allowed_uid: Option<u32>, gui_path: Option<&std::path::Path>) -> anyhow::Result<()> {
    // The interactive-user DACL makes a per-uid check unnecessary, but the
    // installer still records it for consistency with Unix builds.
    if let Some(uid) = allowed_uid {
        if uid == 0 {
            anyhow::bail!("refusing to authorize uid 0");
        }
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
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    // Remove a previous installation if present.
    if let Ok(existing) = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        let _ = existing.stop();
        let _ = existing.delete();
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
    let service = manager.create_service(
        &info,
        ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::QUERY_STATUS,
    )?;
    service.start(&[] as &[&std::ffi::OsStr])?;
    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    if let Ok(service) = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        let _ = service.stop();
        let _ = service.delete();
    }
    let _ = std::fs::remove_file(installed_binary());
    let _ = std::fs::remove_dir(install_dir());
    Ok(())
}
