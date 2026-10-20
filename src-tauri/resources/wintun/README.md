# Wintun driver binaries

`proxybase-tunneld` loads `wintun.dll` from its own directory on Windows
(tun-rs requirement). Place the Microsoft-signed binaries here before building:

- `amd64/wintun.dll`
- `arm64/wintun.dll`

The files are not checked into this repository. The release workflow downloads
the pinned, signed Wintun 0.14.1 archive from <https://wintun.net/>, verifies
its SHA-256 and Authenticode signature, and copies the DLLs here before
`pnpm tauri build`, so release installers are self-contained. For a local
Windows build, download the same archive and copy `bin/amd64/wintun.dll` (or
`bin/arm64/wintun.dll`) into the matching directory. At install time the
Windows service installer copies the DLL next to the installed daemon, where
tun-rs loads it from.
