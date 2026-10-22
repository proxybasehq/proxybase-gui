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
