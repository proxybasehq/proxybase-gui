//! `proxybase-tunneld` — privileged, headless sidecar for the ProxyBase
//! system-wide tunnel.
//!
//! The desktop GUI never runs as root. This daemon owns TUN creation, route
//! mutations, DNS overrides and crash recovery, and exposes an authenticated
//! local IPC channel (Unix socket / named pipe) to the GUI.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ipc_server;
mod peer_auth;
#[cfg(unix)]
mod service_unix;
#[cfg(windows)]
mod service_win;

use clap::Parser;
use std::sync::Arc;

#[derive(Debug, Parser)]
#[command(
    name = "proxybase-tunneld",
    version,
    about = "ProxyBase privileged system tunnel daemon"
)]
struct Cli {
    /// Run the IPC daemon (used by systemd/launchd/Windows SCM).
    #[arg(long)]
    run_daemon: bool,

    /// Install and start the systemd unit (Linux).
    #[arg(long)]
    install_systemd: bool,

    /// Install and start the launchd daemon (macOS).
    #[arg(long)]
    install_launchd: bool,

    /// Install and start the Windows service (Windows).
    #[arg(long)]
    install_service: bool,

    /// Install and start the appropriate service for this OS.
    #[arg(long)]
    install_and_start: bool,

    /// Remove the installed service and its daemon binary.
    #[arg(long)]
    uninstall: bool,

    /// UID allowed to connect to the IPC socket (written at install time).
    #[arg(long)]
    allowed_uid: Option<u32>,

    /// Absolute path of the GUI executable allowed to talk to the daemon.
    #[arg(long)]
    gui_path: Option<std::path::PathBuf>,
}

#[cfg(windows)]
fn program_data_dir() -> std::path::PathBuf {
    std::env::var_os("ProgramData")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"))
        .join("ProxyBase")
}

#[cfg(windows)]
fn install_error_path() -> std::path::PathBuf {
    program_data_dir().join("last_install_error.txt")
}

fn env_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
}

fn init_tracing() {
    // A Windows service has no console; stderr disappears. Persist logs next
    // to the journal so startup failures can be diagnosed.
    #[cfg(windows)]
    {
        let dir = program_data_dir();
        let appender = std::fs::create_dir_all(&dir)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                tracing_appender::rolling::Builder::new()
                    .rotation(tracing_appender::rolling::Rotation::NEVER)
                    .filename_prefix("tunneld.log")
                    .build(&dir)
                    .map_err(|e| e.to_string())
            });
        if let Ok(appender) = appender {
            // Write synchronously: the service must not lose its final error
            // line when it calls process::exit immediately afterwards.
            let initialized = tracing_subscriber::fmt()
                .with_env_filter(env_filter())
                .with_writer(appender)
                .with_ansi(false)
                .try_init()
                .is_ok();
            if initialized {
                return;
            }
        }
    }
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter())
        .with_writer(std::io::stderr)
        .try_init();
}

fn main() {
    let cli = Cli::parse();
    init_tracing();

    if let Some(uid) = cli.allowed_uid {
        std::env::set_var("PROXYBASE_ALLOWED_UID", uid.to_string());
    }

    if cli.uninstall {
        if let Err(e) = uninstall() {
            eprintln!("proxybase-tunneld: uninstall failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    if cli.install_systemd || cli.install_launchd || cli.install_service || cli.install_and_start {
        match install(&cli) {
            Ok(()) => {
                #[cfg(windows)]
                {
                    let _ = std::fs::remove_file(install_error_path());
                }
                println!("ProxyBase tunnel service installed and started.");
                return;
            }
            Err(e) => {
                tracing::error!(error = %e, "service installation failed");
                #[cfg(windows)]
                {
                    let _ = std::fs::create_dir_all(program_data_dir());
                    let _ = std::fs::write(install_error_path(), format!("{e}\n"));
                }
                eprintln!("proxybase-tunneld: installation failed: {e}");
                std::process::exit(1);
            }
        }
    }

    if !cli.run_daemon {
        eprintln!(
            "proxybase-tunneld: nothing to do. Use --run-daemon, --install-and-start or --uninstall."
        );
        std::process::exit(2);
    }

    if let Err(e) = run_daemon() {
        tracing::error!(error = %e, "daemon terminated");
        eprintln!("proxybase-tunneld: fatal: {e}");
        std::process::exit(1);
    }
}

fn install(cli: &Cli) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        let uid = cli
            .allowed_uid
            .or_else(peer_auth::allowed_uid)
            .ok_or_else(|| anyhow::anyhow!("--allowed-uid is required for installation"))?;
        let target = if cfg!(target_os = "macos") {
            cli.install_launchd || cli.install_and_start
        } else {
            cli.install_systemd || cli.install_and_start
        };
        if !target {
            anyhow::bail!("use --install-systemd on Linux or --install-launchd on macOS");
        }
        service_unix::install(uid, cli.gui_path.as_deref())?;
        Ok(())
    }
    #[cfg(windows)]
    {
        let target = cli.install_service || cli.install_and_start;
        if !target {
            anyhow::bail!("use --install-service on Windows");
        }
        service_win::install(cli.allowed_uid, cli.gui_path.as_deref())?;
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = cli;
        anyhow::bail!("unsupported platform")
    }
}

fn uninstall() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        service_unix::uninstall()?;
    }
    #[cfg(windows)]
    {
        service_win::uninstall()?;
    }
    println!("ProxyBase tunnel service removed.");
    Ok(())
}

fn run_daemon() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        // Creating a TUN device and mutating routes requires root. Refuse to
        // start early instead of surfacing a confusing EPERM later.
        let euid = unsafe { libc::geteuid() };
        if euid != 0 {
            anyhow::bail!("proxybase-tunneld must run as root (installed service does this)");
        }

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            // INVARIANT: recover before binding the IPC socket.
            libproxybase::tunnel::journal::recover_if_dirty()
                .await
                .map_err(|e| anyhow::anyhow!("crash-recovery rollback failed: {e}"))?;

            let engine = Arc::new(libproxybase::tunnel::SystemTunnelEngine::new());
            let serve_engine = engine.clone();
            let serve = tokio::spawn(async move { ipc_server::serve(serve_engine).await });

            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
            let mut sigint =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
            let outcome = tokio::select! {
                result = serve => result,
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received");
                    Ok(Ok(()))
                }
                _ = sigint.recv() => {
                    tracing::info!("SIGINT received");
                    Ok(Ok(()))
                }
            };

            // Always restore routes/DNS on a clean shutdown.
            let _ = engine.stop().await;
            match outcome {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(anyhow::anyhow!("IPC server task failed: {e}")),
            }
        })
    }
    #[cfg(windows)]
    {
        service_win::run_daemon()
    }
    #[cfg(not(any(unix, windows)))]
    {
        anyhow::bail!("unsupported platform")
    }
}
