//! Mandatory IPC peer authentication.
//!
//! `proxybase-tunneld` runs as root / LocalSystem. Any local process could
//! otherwise connect to the socket and redirect all system traffic to a
//! malicious proxy. Every connection is verified to come from the expected
//! interactive user and the `proxybase-gui` executable before a single byte of
//! application data is accepted.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub uid: u32,
    pub pid: u32,
    pub executable: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("could not read peer credentials: {0}")]
    Credentials(String),
    #[error("peer uid {peer} is not the allowed user {allowed}")]
    WrongUid { peer: u32, allowed: u32 },
    #[error("privileged clients are not accepted (uid 0)")]
    RootClient,
    #[error("could not determine the peer executable: {0}")]
    UnknownExecutable(String),
    #[error("peer executable {0} is not the ProxyBase GUI")]
    WrongExecutable(String),
    #[error("peer executable {0} is writable by other users")]
    WritableExecutable(String),
}

/// The uid permitted to talk to the daemon.
///
/// Sources, in order: `PROXYBASE_ALLOWED_UID`, the installer-written file,
/// `PKEXEC_UID`, then the owner of `/dev/console`.
pub fn allowed_uid() -> Option<u32> {
    if let Ok(v) = std::env::var("PROXYBASE_ALLOWED_UID") {
        if let Ok(uid) = v.parse() {
            return Some(uid);
        }
    }
    if let Ok(v) = std::fs::read_to_string("/var/lib/proxybase/allowed_uid") {
        if let Ok(uid) = v.trim().parse() {
            return Some(uid);
        }
    }
    if let Ok(v) = std::env::var("PKEXEC_UID") {
        if let Ok(uid) = v.parse() {
            return Some(uid);
        }
    }
    console_owner()
}

#[cfg(unix)]
fn console_owner() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/dev/console").ok().map(|m| m.uid())
}

#[cfg(not(unix))]
fn console_owner() -> Option<u32> {
    None
}

/// Path of the GUI executable recorded at install time. When set, only that
/// exact binary (canonical path) may connect — a malware binary that merely
/// renames itself `proxybase-gui` is rejected.
pub fn configured_gui_path() -> Option<PathBuf> {
    #[cfg(unix)]
    let path = PathBuf::from("/var/lib/proxybase/gui_path");
    #[cfg(windows)]
    let path = std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("ProxyBase")
        .join("gui_path");
    let value = std::fs::read_to_string(path).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

/// Persist the allowed uid during installation.
pub fn write_allowed_uid(uid: u32) -> std::io::Result<()> {
    let dir = std::path::Path::new("/var/lib/proxybase");
    std::fs::create_dir_all(dir)?;
    let path = dir.join("allowed_uid");
    std::fs::write(&path, format!("{uid}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

fn dev_allow_any() -> bool {
    std::env::var_os("PROXYBASE_TUNNELD_ALLOW_ANY").is_some()
}

#[cfg(unix)]
pub fn verify_unix_peer(stream: &tokio::net::UnixStream) -> Result<PeerInfo, AuthError> {
    use std::os::fd::AsRawFd;

    let fd = stream.as_raw_fd();

    #[cfg(target_os = "linux")]
    let (uid, pid) = {
        let mut cred = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let ret = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut cred as *mut _ as *mut libc::c_void,
                &mut len,
            )
        };
        if ret != 0 {
            return Err(AuthError::Credentials(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        (cred.uid, cred.pid as u32)
    };

    #[cfg(target_os = "macos")]
    let (uid, pid) = {
        let mut cred: libc::xucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::xucred>() as libc::socklen_t;
        let ret = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_LOCAL,
                libc::LOCAL_PEERCRED,
                &mut cred as *mut _ as *mut libc::c_void,
                &mut len,
            )
        };
        if ret != 0 {
            return Err(AuthError::Credentials(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        let mut pid: libc::pid_t = 0;
        let mut pid_len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
        let pid_ret = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                &mut pid as *mut _ as *mut libc::c_void,
                &mut pid_len,
            )
        };
        (cred.cr_uid, if pid_ret == 0 { pid as u32 } else { 0 })
    };

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let (uid, pid) = (u32::MAX, 0u32);

    if !dev_allow_any() {
        let allowed = allowed_uid()
            .ok_or_else(|| AuthError::Credentials("allowed uid is not configured".into()))?;
        if uid == 0 {
            return Err(AuthError::RootClient);
        }
        if uid != allowed {
            return Err(AuthError::WrongUid { peer: uid, allowed });
        }
    }

    let executable = if pid != 0 { process_path(pid) } else { None };

    if !dev_allow_any() {
        let exe = executable
            .as_ref()
            .ok_or_else(|| AuthError::UnknownExecutable(format!("pid {pid}")))?;
        match configured_gui_path() {
            Some(expected) => {
                if !same_executable(&expected, exe) {
                    return Err(AuthError::WrongExecutable(format!(
                        "{} (expected {})",
                        exe.display(),
                        expected.display()
                    )));
                }
            }
            None => {
                if !is_gui_executable(exe) {
                    return Err(AuthError::WrongExecutable(exe.display().to_string()));
                }
            }
        }
        if is_other_writable(exe) {
            return Err(AuthError::WritableExecutable(exe.display().to_string()));
        }
    }

    Ok(PeerInfo {
        uid,
        pid,
        executable,
    })
}

#[cfg(target_os = "linux")]
fn process_path(pid: u32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/exe")).ok()
}

#[cfg(target_os = "macos")]
fn process_path(pid: u32) -> Option<PathBuf> {
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let len = unsafe {
        libc::proc_pidpath(
            pid as libc::c_int,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len() as u32,
        )
    };
    if len <= 0 {
        return None;
    }
    buf.truncate(len as usize);
    String::from_utf8(buf).ok().map(PathBuf::from)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn process_path(_pid: u32) -> Option<PathBuf> {
    None
}

fn same_executable(expected: &std::path::Path, actual: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        let canonical =
            |p: &std::path::Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        canonical(expected) == canonical(actual)
    }
    #[cfg(windows)]
    {
        let normalize = |p: &std::path::Path| {
            std::fs::canonicalize(p)
                .unwrap_or_else(|_| p.to_path_buf())
                .to_string_lossy()
                .to_ascii_lowercase()
        };
        normalize(expected) == normalize(actual)
    }
    #[cfg(not(any(unix, windows)))]
    {
        expected == actual
    }
}

pub fn is_gui_executable(path: &std::path::Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    // release bundle: Contents/MacOS/proxybase-gui; Windows: proxybase-gui.exe
    name == "proxybase-gui"
        || name == "proxybase-gui.exe"
        || name == "proxybase"
        || name == "proxybase.exe"
}

#[cfg(unix)]
fn is_other_writable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .map(|m| m.mode() & 0o022 != 0)
        .unwrap_or(true)
}

#[cfg(not(unix))]
fn is_other_writable(_path: &std::path::Path) -> bool {
    false
}

// ---------------------------------------------------------------------------
// Windows named-pipe peer verification / DACL
// ---------------------------------------------------------------------------

#[cfg(windows)]
const PIPE_SDDL: &str = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;IU)";

/// Restrict a named-pipe handle to LocalSystem, Administrators and interactive
/// users. Call before the first client can connect.
#[cfg(windows)]
pub fn apply_pipe_dacl(handle: windows_sys::Win32::Foundation::HANDLE) -> std::io::Result<()> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetSecurityInfo, SDDL_REVISION_1,
        SE_KERNEL_OBJECT,
    };
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorDacl, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    };

    let mut sd: PSECURITY_DESCRIPTOR = null_mut();
    let sddl: Vec<u16> = PIPE_SDDL.encode_utf16().chain(std::iter::once(0)).collect();
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let result = (|| {
        let mut dacl_present = 0i32;
        let mut dacl = std::ptr::null_mut();
        let mut dacl_defaulted = 0i32;
        let ok = unsafe {
            GetSecurityDescriptorDacl(sd, &mut dacl_present, &mut dacl, &mut dacl_defaulted)
        };
        if ok == 0 || dacl_present == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let status = unsafe {
            SetSecurityInfo(
                handle,
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                dacl,
                std::ptr::null_mut(),
            )
        };
        if status != 0 {
            return Err(std::io::Error::from_raw_os_error(status as i32));
        }
        Ok(())
    })();
    unsafe {
        LocalFree(sd);
    }
    result
}

#[cfg(windows)]
pub fn verify_windows_pipe_client(
    pipe: &tokio::net::windows::named_pipe::NamedPipeServer,
) -> Result<PeerInfo, AuthError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let mut pid: u32 = 0;
    let ok = unsafe {
        windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId(
            pipe.as_raw_handle() as _,
            &mut pid,
        )
    };
    if ok == 0 {
        return Err(AuthError::Credentials(
            std::io::Error::last_os_error().to_string(),
        ));
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(AuthError::UnknownExecutable(format!(
            "OpenProcess({pid}): {}",
            std::io::Error::last_os_error()
        )));
    }
    let mut buf = vec![0u16; 32768];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut len) };
    unsafe { CloseHandle(process) };
    if ok == 0 {
        return Err(AuthError::UnknownExecutable(format!(
            "pid {pid}: {}",
            std::io::Error::last_os_error()
        )));
    }
    let path = PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
    if !dev_allow_any() {
        match configured_gui_path() {
            Some(expected) => {
                if !same_executable(&expected, &path) {
                    return Err(AuthError::WrongExecutable(format!(
                        "{} (expected {})",
                        path.display(),
                        expected.display()
                    )));
                }
            }
            None => {
                if !is_gui_executable(&path) {
                    return Err(AuthError::WrongExecutable(path.display().to_string()));
                }
            }
        }
    }
    Ok(PeerInfo {
        uid: 0,
        pid,
        executable: Some(path),
    })
}
