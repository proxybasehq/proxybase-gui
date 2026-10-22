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
  `%ProgramFiles%\ProxyBase\tunneld`) before service registration. The Windows
  subdirectory matters: installing over the sidecar next to the GUI would
  replace the executable the elevated installer is currently running from.

## Install paths triggered on first use

- macOS: `osascript ... with administrator privileges` writes
  `/Library/LaunchDaemons/xyz.proxybase.tunneld.plist` and bootstraps it.
- Linux: `pkexec /bin/sh -c ...` streams the bundled daemon over stdin into
  `/usr/local/lib/proxybase/proxybase-tunneld` (write to a sibling, then
  rename, so an already-running daemon does not trigger `ETXTBSY`), then runs
  its `--install-systemd` mode. The stream is required for AppImages: their
  FUSE mount is only accessible to the mounting user, so root cannot exec the
  sidecar from `/tmp/.mount_*`. The installer enables and **restarts** the
  unit, so upgrades never leave the previous daemon running.
- AppImage peer authentication: the root daemon also cannot `stat()` the GUI
  binary inside `/tmp/.mount_*` (FUSE `allow_other` is off). An unreadable
  executable is treated as not writable by the peer check; the `SO_PEERCRED`
  uid and the `proxybase-gui` name checks still apply. Without this, every
  AppImage launch is rejected with "peer executable ... is writable by other
  users".
- Windows: `ShellExecuteExW` with `runas` installs `ProxyBaseTunnelService`
  (LocalSystem, auto-start).
- Android: `VpnService.prepare()` then a foreground `ProxyBaseVpnService`.

## Windows prerequisites

`tun-rs` loads `wintun.dll` from the daemon's executable directory. The
Microsoft-signed DLLs are intentionally not checked in: release CI downloads
the pinned Wintun 0.14.1 archive, verifies its SHA-256 and Authenticode
signature, and stages it in `src-tauri/resources/wintun/{amd64,arm64}/`. The
service installer copies the matching DLL next to the installed daemon. For a
local Windows build, download the same archive from <https://wintun.net/> and
copy the DLL manually.

## Windows troubleshooting

- `%ProgramData%\ProxyBase\tunneld.log` — service log (stderr is invisible to
  a LocalSystem service).
- `%ProgramData%\ProxyBase\last_install_error.txt` — written by the elevated
  installer when registration/startup fails; the GUI includes it in the
  elevation error and removes it after a successful install.
- The tunnel error banner has a **Diagnostics** button that returns the tail of
  the daemon log, the current tunnel state, and the last install error. Ask
  users for that output when reporting a Windows problem.
- The GUI waits up to 15 s for the named pipe after elevation. The installer
  itself stops and deletes any previous `ProxyBaseTunnelService` before
  replacing the binary, then waits for the SCM to report `Running`.
- The Windows service keeps running when the GUI is updated, so a warm IPC
  connection can point at an older daemon. On every enable, the GUI verifies
  both the daemon version and the installed binary hash against the bundled
  sidecar and re-runs the elevated installer when they differ.

### Windows route rules (error 87)

`CreateIpForwardEntry2` is much stricter than `route(8)` on macOS/Linux. The
Windows manager keeps these invariants; do not relax them:

- Destination prefixes are always canonical network addresses. Windows rejects
  a row like `10.255.0.2/30` (host bits set) with
  `ERROR_INVALID_PARAMETER` (87); it must be `10.255.0.0/30`.
- `SitePrefixLength` must be `0` — `InitializeIpForwardEntry` leaves it at 255,
  which is greater than most prefix lengths and therefore rejected.
- Next hop and destination families must match. An IPv6 relay can never be
  pinned through the IPv4 default gateway.
- The relay host route is derived from `GetBestRoute2`, so its interface and
  next hop are exactly what Windows itself would use (multi-homed hosts, VPNs,
  IPv6 relays). Do not assume the default gateway.
- `InitializeIpForwardEntry` also leaves `Metric`/`Protocol` at illegal values
  and sets `Loopback`, `AutoconfigureAddress`, and `Publish` to TRUE; the row
  builder fills every field explicitly.
- A fresh Wintun adapter can transiently reject routes with 87/1168 before the
  IP Helper provider publishes it; route creation waits for the interface and
  retries briefly.
- Restores delete only `MIB_IPPROTO_NETMGMT` routes, so DHCP/connected routes
  that existed before the tunnel survive a disconnect.

## Conflicting VPNs

A second **full-tunnel** VPN cannot coexist with the system tunnel: both want
the default route, and whichever loses silently breaks the tunnel or leaks
traffic. Before creating the TUN device, the daemon asks the platform manager
for a conflicting VPN and refuses to start with `conflicting_vpn`, naming the
interface (e.g. `utun4`).

Detection rules (implemented per platform in `tunnel/platform/*.rs`):

- macOS: up `utun*`/`ipsec*`/`ppp*` interfaces that own the IPv4 default route
  or one of its `/1` halves. IPv6-only defaults from Apple's built-in
  `utun0`-`utun3` system interfaces are ignored.
- Linux: up `tun*`/`tap*`/`wg*`/`ppp*`/`tailscale*`/`nordlynx`/... interfaces
  that own an IPv4 `default`, `0.0.0.0/1`, or `128.0.0.0/1` route in the main
  table.
- Windows: up `IF_TYPE_TUNNEL`/`IF_TYPE_PPP` or VPN-named adapters (WireGuard,
  Wintun, TAP-Windows, OpenVPN, Tailscale, NordLynx, Proton, Mullvad, WARP)
  that own an IPv4 default or `/1` route. ProxyBase's own adapter is excluded.

Split-tunnel VPNs (Tailscale without an exit node, corporate routes) leave the
default route alone and are not reported; they can run alongside the tunnel.

## Crash recovery

Every route/DNS mutation is preceded by an atomic journal write
(`/var/lib/proxybase/tunnel_journal.json`,
`%ProgramData%\ProxyBase\tunnel_journal.json`). The daemon calls
`recover_if_dirty()` before binding its IPC socket. Dynamic Windows WFP filters
are removed by the kernel if the process dies.

## CI / standalone repository builds

`scripts/prepare-sidecar.mjs` builds the daemon directly with
`--manifest-path crates/proxybase-tunneld/Cargo.toml`, so it works both in the
local monorepo (where the crate is a workspace member) and in the standalone
`proxybase-gui` checkout used by CI. It shares `src-tauri/target` with Tauri so
the Rust cache covers the daemon.

The workflow checks out `libproxybase` next to the GUI repository and links it
to `../libproxybase`; the daemon's `../../../libproxybase` path dependency
resolves through that link. **Both repositories must contain the tunnel
changes**: `libproxybase` (the `tunnel` module and its dependencies) and
`proxybase-gui` (the `crates/proxybase-tunneld` daemon, UI, and scripts).

The script also produces `proxybase-tunneld-universal-apple-darwin` (via
`lipo`) for `--target universal-apple-darwin`, and skips sidecar builds for
Android/iOS (which use the in-process VpnService engine).

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
