# System-wide tunnel (VPN mode)

One-click routing of all OS traffic through a purchased ProxyBase SOCKS5
session. The design spec is `../../one-click-systemwide-deploy.md`.

## Components

| Component | Location | Role |
|---|---|---|
| `libproxybase::tunnel` | `../../libproxybase/src/tunnel` | Packet engine, Fake-IP DNS, SOCKS5 relay, journal, platform route/DNS managers, health checks |
| `proxybase-tunneld` | `../crates/proxybase-tunneld` | Privileged headless daemon (root/LocalSystem). Owns TUN creation, routes, DNS, IPC, crash recovery |
| IPC client | `src-tauri/src/tunnel_client.rs` | Authenticated local IPC, request correlation, Tauri events, first-click privilege escalation |
| Commands | `src-tauri/src/commands/tunnel.rs` | `enable_system_tunnel`, `disable_system_tunnel`, `get_system_tunnel_status`, `get_tunnel_telemetry` |
| Android | `src-tauri/src/android_vpn_jni.rs`, `gen/android/.../ProxyBaseVpnService.kt`, `VpnController.kt` | `VpnService` creates the TUN fd; Rust runs the same engine in-process |
| UI | `src/components/tunnel/*`, `src/hooks/useSystemTunnel.ts` | Toggle button, persistent banner, settings modal |

## Data path (desktop)

```
apps -> sub-default routes 0.0.0.0/1 + 128.0.0.0/1 (+ IPv6 ::/1, 8000::/1)
     -> Wintun / utunN / proxybase0
     -> proxybase-tunneld: Fake-IP DNS + userspace TCP/IP + MSS clamp
     -> 127.0.0.1:<bridge> (GUI's unauthenticated local SOCKS5 bridge)
     -> authenticated ProxyBase relay -> exit node
```

The relay host is resolved to an IP *before* the tunnel starts and pinned with
a `/32` route via the physical gateway; the GUI bridge dials that IP, never the
hostname (which would resolve to a fake IP while the tunnel is active).

## Security

- The GUI never runs elevated. Only `proxybase-tunneld` runs as root.
- Unix IPC: `/var/run/proxybase-tunneld.sock`, mode `0600`, owned by the
  interactive user. `SO_PEERCRED`/`LOCAL_PEERCRED` + executable path check.
- Windows IPC: `\\.\pipe\proxybase-tunneld` with a DACL for SYSTEM,
  Administrators and Interactive Users, plus client PID/path verification.
- The installer records the exact GUI executable path
  (`/var/lib/proxybase/gui_path`, `%ProgramData%\ProxyBase\gui_path`) and the
  daemon rejects any other binary.
- The root daemon binary is copied to a root-owned directory
  (`/usr/local/lib/proxybase`, `/Library/Application Support/ProxyBase`,
  `%ProgramFiles%\ProxyBase`) before service registration.

## Install paths triggered on first use

- macOS: `osascript ... with administrator privileges` writes
  `/Library/LaunchDaemons/xyz.proxybase.tunneld.plist` and bootstraps it.
- Linux: `pkexec proxybase-tunneld --install-systemd` writes
  `/etc/systemd/system/proxybase-tunneld.service`.
- Windows: `ShellExecuteExW` with `runas` installs `ProxyBaseTunnelService`
  (LocalSystem, auto-start).
- Android: `VpnService.prepare()` then a foreground `ProxyBaseVpnService`.

## Windows prerequisites

`tun-rs` loads `wintun.dll` from the daemon's executable directory. Vendor the
Microsoft-signed DLLs into `src-tauri/resources/wintun/{amd64,arm64}/` before
building; the installer copies the matching one next to the service binary.
They are intentionally not checked in.

## Crash recovery

Every route/DNS mutation is preceded by an atomic journal write
(`/var/lib/proxybase/tunnel_journal.json`,
`%ProgramData%\ProxyBase\tunnel_journal.json`). The daemon calls
`recover_if_dirty()` before binding its IPC socket. Dynamic Windows WFP filters
are removed by the kernel if the process dies.

## Tests

```bash
cargo test -p libproxybase tunnel::        # packet/DNS/journal/SOCKS/hot-swap
cargo test -p proxybase-tunneld            # IPC state machine
cd proxybase-gui && pnpm typecheck && pnpm build
```

The specified tests are present under their required names, including
`test_fake_dns_allocation_and_lru_eviction`,
`test_tcp_mss_clamping_checksum_validity`,
`test_quic_udp_443_icmp_rejection`,
`test_webrtc_stun_magic_cookie_blocker`,
`test_rollback_journal_crash_recovery`,
`test_end_to_end_fake_ip_to_socks5_domain_atyp`, and
`test_zero_downtime_proxy_hot_swap`.

## Known limitations

- The unprivileged "OS system proxy" fallback (`TunnelMode::OsSystemProxyFallback`)
  is not implemented: declining elevation surfaces an error instead. The full
  tunnel is always used, per the architectural invariants.
- Raw IPv6 literals that the upstream cannot reach fall back through a TCP
  reset rather than a synthesised ICMPv6 unreachable; DNS `AAAA` is already
  NODATA, so normal hostnames always use IPv4 Fake-IPs.
- The tray menu exposes status and Disconnect; a node-switcher submenu is not
  included (switching is done from the Market page and hot-swaps in place).
- The bundled local bridge implements SOCKS5 CONNECT only (the v2 upstream
  gateway is TCP). The engine attempts UDP ASSOCIATE and drops datagrams if the
  upstream does not support it. DNS is handled locally and QUIC/STUN are
  filtered, so normal web browsing is unaffected; arbitrary UDP apps are not
  proxied.
- Remaining quota is not shown in the banner (the daemon does not receive the
  session's quota; quota exhaustion is detected via SOCKS CONNECT rejection and
  reported as `quota_exhausted`).
- Windows route/WFP/service code is type-checked in an isolated crate against
  `windows-sys`/`windows-service` but has not been runtime-tested on Windows in
  this environment.
- Android Kotlin + Rust compile and link; no device/emulator runtime test was
  performed here.
