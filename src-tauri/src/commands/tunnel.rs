//! Tauri commands for the one-click system-wide tunnel.

use libproxybase::tunnel::config::{TunnelConfig, TunnelState};
use serde::Serialize;
use std::net::IpAddr;

/// `enable_system_tunnel` requires the session's local SOCKS5 bridge to be
/// running: the bridge owns the authenticated upstream credentials, while the
/// tunnel only needs an unauthenticated loopback SOCKS5 endpoint.
#[tauri::command]
pub async fn enable_system_tunnel(
    proxy_id: String,
    kill_switch: Option<bool>,
    bypass_cidrs: Option<Vec<String>>,
    disable_quic_udp_443: Option<bool>,
    block_webrtc_stun: Option<bool>,
    stop_on_gui_exit: Option<bool>,
) -> Result<TunnelState, String> {
    if proxy_id.trim().is_empty() {
        let err = "proxy_id is required".to_string();
        #[cfg(desktop)]
        crate::tunnel_client::emit_error("internal_error", &err).await;
        return Err(err);
    }
    let Some((bridge_port, _upstream_addr, upstream_dial_addr)) =
        crate::bridge::bridge_info(&proxy_id).await
    else {
        let err =
            "Start the local SOCKS5 bridge for this session before enabling the system tunnel"
                .to_string();
        #[cfg(desktop)]
        crate::tunnel_client::emit_error("internal_error", &err).await;
        return Err(err);
    };
    let upstream_relay_ip = match resolve_relay_ip(&upstream_dial_addr).await {
        Ok(ip) => ip,
        Err(e) => {
            #[cfg(desktop)]
            crate::tunnel_client::emit_error("internal_error", &e).await;
            return Err(e);
        }
    };

    let mut config = TunnelConfig {
        proxy_id: proxy_id.clone(),
        socks5_bridge_addr: ([127, 0, 0, 1], bridge_port).into(),
        upstream_relay_ip,
        kill_switch: kill_switch.unwrap_or(true),
        disable_quic_udp_443: disable_quic_udp_443.unwrap_or(true),
        block_webrtc_stun: block_webrtc_stun.unwrap_or(true),
        custom_bypass_cidrs: bypass_cidrs.unwrap_or_default(),
        stop_on_gui_exit: stop_on_gui_exit.unwrap_or(true),
        ..TunnelConfig::default()
    };
    config.proxy_id = proxy_id;
    if let Err(e) = config.validate() {
        #[cfg(desktop)]
        crate::tunnel_client::emit_error("internal_error", &e).await;
        return Err(e);
    }

    #[cfg(desktop)]
    {
        crate::tunnel_client::start(config).await
    }
    #[cfg(target_os = "android")]
    {
        crate::android_vpn_jni::start(config).await
    }
    #[cfg(target_os = "ios")]
    {
        let _ = config;
        Err("the system tunnel is not supported on iOS".into())
    }
}

#[tauri::command]
pub async fn disable_system_tunnel() -> Result<TunnelState, String> {
    #[cfg(desktop)]
    {
        crate::tunnel_client::stop().await
    }
    #[cfg(target_os = "android")]
    {
        crate::android_vpn_jni::stop().await
    }
    #[cfg(target_os = "ios")]
    {
        Err("the system tunnel is not supported on iOS".into())
    }
}

#[tauri::command]
pub async fn get_system_tunnel_status() -> Result<TunnelState, String> {
    #[cfg(desktop)]
    {
        crate::tunnel_client::status().await
    }
    #[cfg(target_os = "android")]
    {
        crate::android_vpn_jni::status().await
    }
    #[cfg(target_os = "ios")]
    {
        Ok(TunnelState::Disconnected)
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct TunnelTelemetry {
    pub rx_bytes_total: u64,
    pub tx_bytes_total: u64,
    pub rx_bps: u64,
    pub tx_bps: u64,
    pub active_flows: u32,
}

#[tauri::command]
pub async fn get_tunnel_telemetry() -> Result<TunnelTelemetry, String> {
    #[cfg(desktop)]
    {
        let t = crate::tunnel_client::cached_telemetry().await;
        Ok(TunnelTelemetry {
            rx_bytes_total: t.rx_bytes_total,
            tx_bytes_total: t.tx_bytes_total,
            rx_bps: t.rx_bps,
            tx_bps: t.tx_bps,
            active_flows: t.active_flows,
        })
    }
    #[cfg(target_os = "android")]
    {
        let t = crate::android_vpn_jni::telemetry().await;
        Ok(TunnelTelemetry {
            rx_bytes_total: t.rx_bytes_total,
            tx_bytes_total: t.tx_bytes_total,
            rx_bps: t.rx_bps,
            tx_bps: t.tx_bps,
            active_flows: t.active_flows,
        })
    }
    #[cfg(target_os = "ios")]
    {
        Ok(TunnelTelemetry {
            rx_bytes_total: 0,
            tx_bytes_total: 0,
            rx_bps: 0,
            tx_bps: 0,
            active_flows: 0,
        })
    }
}

/// Collect the daemon log tail plus environment details for support.
///
/// The privileged daemon's log is the only place Windows route/DNS failures
/// are recorded (a LocalSystem service has no console), so surface it in the
/// GUI's error banner instead of making users hunt through ProgramData.
#[tauri::command]
pub async fn get_tunnel_diagnostics() -> Result<String, String> {
    let state = get_system_tunnel_status()
        .await
        .unwrap_or(TunnelState::Disconnected);
    let mut out = format!(
        "ProxyBase {}\nOS: {} {}\n\nTunnel state:\n{}\n\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        serde_json::to_string_pretty(&state).unwrap_or_else(|_| format!("{state:?}"))
    );
    out.push_str(&daemon_log_tail());
    Ok(out)
}

fn daemon_log_tail() -> String {
    const MAX_LINES: usize = 400;
    let mut out = String::new();

    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;
        let base = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
            .join("ProxyBase");
        let log = base.join("tunneld.log");
        out.push_str(&format!("Log file: {}\n", log.display()));
        out.push_str(
            &tail_file(&log, MAX_LINES).unwrap_or_else(|e| format!("<unavailable: {e}>\n")),
        );
        let install_error = base.join("last_install_error.txt");
        if let Ok(text) = std::fs::read_to_string(&install_error) {
            if !text.trim().is_empty() {
                out.push_str(&format!("\nLast install error:\n{}\n", text.trim()));
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let log = std::path::PathBuf::from("/var/log/proxybase-tunneld.log");
        out.push_str(&format!("Log file: {}\n", log.display()));
        out.push_str(
            &tail_file(&log, MAX_LINES).unwrap_or_else(|e| format!("<unavailable: {e}>\n")),
        );
    }

    #[cfg(target_os = "linux")]
    {
        out.push_str(
            "The tunnel daemon logs to the systemd journal. Run:\n  \
             journalctl -u proxybase-tunneld -n 400 --no-pager\n",
        );
    }

    out
}

fn tail_file(path: &std::path::Path, max_lines: usize) -> std::io::Result<String> {
    const MAX_BYTES: usize = 512 * 1024;
    let bytes = std::fs::read(path)?;
    let start = bytes.len().saturating_sub(MAX_BYTES);
    let text = String::from_utf8_lossy(&bytes[start..]);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

/// Resolve the upstream relay hostname once, in the unprivileged GUI process,
/// before any routes change. The daemon receives only a validated `IpAddr`.
async fn resolve_relay_ip(upstream: &str) -> Result<IpAddr, String> {
    let trimmed = upstream.trim();
    let authority = trimmed
        .strip_prefix("socks5h://")
        .or_else(|| trimmed.strip_prefix("socks5://"))
        .or_else(|| trimmed.strip_prefix("socks4://"))
        .or_else(|| trimmed.strip_prefix("http://"))
        .or_else(|| trimmed.strip_prefix("https://"))
        .unwrap_or(trimmed);
    let authority = authority.split('/').next().unwrap_or(authority);
    let (host, port) = split_host_port(authority)?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip);
    }
    let addrs = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|e| format!("cannot resolve upstream relay {host}: {e}"))?;
    let mut first: Option<IpAddr> = None;
    for addr in addrs {
        let ip = addr.ip();
        if ip.is_ipv4() {
            return Ok(ip);
        }
        first.get_or_insert(ip);
    }
    first.ok_or_else(|| format!("upstream relay {host} resolved to no addresses"))
}

fn split_host_port(authority: &str) -> Result<(String, u16), String> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, tail) = rest
            .split_once(']')
            .ok_or_else(|| format!("malformed IPv6 authority {authority:?}"))?;
        let port = tail
            .strip_prefix(':')
            .ok_or_else(|| format!("missing port in {authority:?}"))?
            .parse::<u16>()
            .map_err(|e| format!("invalid port in {authority:?}: {e}"))?;
        return Ok((host.to_string(), port));
    }
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| format!("missing port in {authority:?}"))?;
    let port = port
        .parse::<u16>()
        .map_err(|e| format!("invalid port in {authority:?}: {e}"))?;
    if host.is_empty() {
        return Err(format!("missing host in {authority:?}"));
    }
    Ok((host.to_string(), port))
}
