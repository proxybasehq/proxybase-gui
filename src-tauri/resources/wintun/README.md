# Wintun driver binaries

`proxybase-tunneld` loads `wintun.dll` from its own directory on Windows
(tun-rs requirement). Place the Microsoft-signed binaries here before building:

- `amd64/wintun.dll`
- `arm64/wintun.dll`

Download the current release from <https://wintun.net/>. The files are not
checked into this repository; release CI must vendor the signed DLLs so that
`pnpm tauri build` produces a working Windows installer. The Windows service
installer copies the matching DLL next to the installed daemon.
