//! systemd (Linux) and launchd (macOS) installation.
//!
//! The sidecar is copied out of the (potentially user-writable) app directory
//! into a root-owned location before being registered as a root service. That
//! prevents a local user from replacing the binary and gaining root on the
//! next service restart.

use anyhow::{anyhow, Context};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "linux")]
const SERVICE_NAME: &str = "proxybase-tunneld";
#[cfg(target_os = "linux")]
const INSTALL_DIR: &str = "/usr/local/lib/proxybase";
#[cfg(target_os = "linux")]
const UNIT_PATH: &str = "/etc/systemd/system/proxybase-tunneld.service";

#[cfg(target_os = "macos")]
const LABEL: &str = "xyz.proxybase.tunneld";
#[cfg(target_os = "macos")]
const INSTALL_DIR: &str = "/Library/Application Support/ProxyBase";
#[cfg(target_os = "macos")]
const PLIST_PATH: &str = "/Library/LaunchDaemons/xyz.proxybase.tunneld.plist";

fn root_owned_binary_path() -> PathBuf {
    Path::new(INSTALL_DIR).join(binary_name())
}

fn binary_name() -> &'static str {
    "proxybase-tunneld"
}

fn install_binary() -> anyhow::Result<PathBuf> {
    let current = std::env::current_exe().context("cannot locate tunneld executable")?;
    let dest = root_owned_binary_path();
    std::fs::create_dir_all(INSTALL_DIR)?;
    let tmp = dest.with_extension("new");
    std::fs::copy(&current, &tmp)
        .with_context(|| format!("failed to copy {} -> {}", current.display(), tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&tmp, &dest)?;
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        if let Ok(c) = CString::new(dest.as_os_str().as_bytes()) {
            unsafe {
                libc::chown(c.as_ptr(), 0, 0);
            }
        }
    }
    Ok(dest)
}

pub fn install(allowed_uid: u32, gui_path: Option<&Path>) -> anyhow::Result<()> {
    if allowed_uid == 0 {
        anyhow::bail!("refusing to authorize uid 0 (run the GUI as a normal user)");
    }
    let binary = install_binary()?;
    crate::peer_auth::write_allowed_uid(allowed_uid)
        .context("failed to persist the allowed uid")?;
    if let Some(gui) = gui_path {
        let path = Path::new("/var/lib/proxybase/gui_path");
        std::fs::write(path, format!("{}\n", gui.display()))
            .context("failed to persist the GUI executable path")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644));
        }
    }

    #[cfg(target_os = "linux")]
    install_linux(&binary, allowed_uid)?;
    #[cfg(target_os = "macos")]
    install_macos(&binary, allowed_uid)?;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = binary;
        anyhow::bail!("service installation is not supported on this platform");
    }
    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("systemctl")
            .args(["disable", "--now", SERVICE_NAME])
            .status();
        let _ = std::fs::remove_file(UNIT_PATH);
        let _ = Command::new("systemctl").arg("daemon-reload").status();
        let _ = std::fs::remove_file(root_owned_binary_path());
        let _ = std::fs::remove_dir_all(INSTALL_DIR);
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("launchctl")
            .args(["bootout", &format!("system/{LABEL}")])
            .status();
        let _ = std::fs::remove_file(PLIST_PATH);
        let _ = std::fs::remove_file(root_owned_binary_path());
        let _ = std::fs::remove_dir_all(INSTALL_DIR);
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        anyhow::bail!("service removal is not supported on this platform")
    }
}

#[cfg(target_os = "linux")]
fn install_linux(binary: &Path, allowed_uid: u32) -> anyhow::Result<()> {
    let unit = format!(
        "[Unit]\n\
         Description=ProxyBase System Tunnel Daemon\n\
         Documentation=https://proxybase.xyz\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={} --run-daemon --allowed-uid {}\n\
         Restart=on-failure\n\
         RestartSec=1\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        binary.display(),
        allowed_uid
    );
    std::fs::write(UNIT_PATH, unit)?;
    run("systemctl", &["daemon-reload"])?;
    run("systemctl", &["enable", "--now", SERVICE_NAME])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn run(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to run {program}"))?;
    if !status.success() {
        anyhow::bail!("{program} {:?} failed with {status}", args);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn install_macos(binary: &Path, allowed_uid: u32) -> anyhow::Result<()> {
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>--run-daemon</string>
        <string>--allowed-uid</string>
        <string>{uid}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>StandardOutPath</key>
    <string>/var/log/proxybase-tunneld.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/proxybase-tunneld.log</string>
</dict>
</plist>
"#,
        label = LABEL,
        binary = xml_escape(&binary.display().to_string()),
        uid = allowed_uid
    );
    std::fs::write(PLIST_PATH, plist)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(PLIST_PATH, std::fs::Permissions::from_mode(0o644))?;
    }

    // Unload a previous version if present, then load the new one.
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("system/{LABEL}")])
        .status();
    let bootstrap = Command::new("launchctl")
        .args(["bootstrap", "system", PLIST_PATH])
        .status();
    match bootstrap {
        Ok(status) if status.success() => Ok(()),
        _ => {
            let status = Command::new("launchctl")
                .args(["load", "-w", PLIST_PATH])
                .status()
                .context("launchctl load failed")?;
            if !status.success() {
                return Err(anyhow!("launchctl could not load {PLIST_PATH}"));
            }
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
