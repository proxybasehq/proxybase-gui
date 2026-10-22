# ProxyBase GUI

Desktop GUI application for ProxyBase, built with React + Vite + Tauri.

## Prerequisites

- Node.js 20+
- pnpm 9+
- Rust stable toolchain
- Platform dependencies required by Tauri (WebKitGTK/GTK on Linux)

## Development

```bash
pnpm install
pnpm tauri:dev
```

## Production Build

```bash
pnpm install --frozen-lockfile
pnpm tauri:build
```

## Release Automation

This repository includes GitHub workflows for:

- Cross-platform CI builds on Linux x86_64/arm64, Windows, macOS Intel, and
  macOS Apple Silicon
- Tagged production release builds and artifact publishing to GitHub Releases
  (Linux arm64 `.deb`/`.rpm`/`.AppImage` + updater manifest entries)

Release tag format:

```bash
proxybase-gui-v0.1.0
```
