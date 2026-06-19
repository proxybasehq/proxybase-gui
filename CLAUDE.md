# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

proxybase-gui — a Tauri v2 desktop app for [ProxyBase](https://proxybase.xyz), a decentralized peer-to-peer bandwidth marketplace. Sellers offer internet connections as proxy exits; buyers purchase SOCKS5 proxy sessions. Payments use microcredits (1,000,000 = $1.00 USD) backed by crypto deposits.

The app provides wallet management (BIP-39 mnemonic), auth via cryptographic challenge/verify against the backend, a market browser for buying proxy sessions by country/network type, and a background seller process that registers with the marketplace and relays traffic through a WebSocket connection.

## Commands

| Task | Command |
|------|---------|
| Frontend dev server | `pnpm dev` (port 1420) |
| TypeScript type-check + build | `pnpm build` |
| Tauri dev (app window) | `pnpm tauri dev` |
| Tauri production build | `pnpm tauri build` |
| Preview built frontend | `pnpm preview` |

There are no tests or linter configured yet.

## Architecture

### Frontend (`src/`)

- **Entry**: `src/main.tsx` mounts `<App />` into `#root`
- **Routing**: `src/App.tsx` — `HashRouter` from react-router-dom v7. Routes: `/wallet`, `/login`, `/market`, `/seller`, `/faq`, plus `/` redirects to `/wallet`. All routes render inside `<Layout />` which acts as the app shell.
- **App shell**: `src/components/Layout.tsx` — owns auth state, seller background state, wallet info, and the deposit modal. Provides `AppContext` via `<Outlet context>`. On mount, checks for existing wallet and auto-logins (if wallet has no password). Reads `proxybase-settings.json` store to auto-resume seller on launch. Manages all modals (info, balance, deposit).
- **Navigation**: `src/components/BottomNav.tsx` — conditionally shows tabs based on auth state. Unauthenticated: Login (only if wallet exists). Authenticated: Market, Seller, FAQ. The Wallet page is always accessible via the header icon.
- **API layer**: `src/api.ts` — typed wrappers around `invoke()` from `@tauri-apps/api/core`. Every backend command has a corresponding function here with TypeScript parameter types.
- **Backend URL**: `src/hooks/useBackend.ts` — `useBackend()` hook provides the backend URL, persisted to `localStorage`. Defaults to `http://localhost:8080` in dev, `https://api.proxybase.xyz` in production.
- **Utilities**: `src/utils.ts` — microcredit ↔ USD conversion (`mcToUsd`, `usdToMc`, `formatUsd`, `formatUsdPerGb`), country code → flag emoji (`countryFlag`), country code → name mapping (`countryName`).
- **Pages**: `src/pages/` contains `WalletPage`, `LoginPage`, `MarketPage`, `SellerPage`, `FaqPage`, and an unused `BuyerPage` (deposit functionality was moved into `Layout`'s modal).
- **Components**: `PasswordInput` (with show/hide toggle), `JsonView` (pretty-printed JSON), `StatusBar` (unused in current shell — status is inline in `Layout`'s header).

### Backend (`src-tauri/`)

- **Entry**: `src-tauri/src/main.rs` → calls `proxybase_gui_lib::run()`
- **App setup**: `src-tauri/src/lib.rs` — `tauri::Builder` with plugins (opener, store, autostart), a tray icon with Show/Hide + Quit menu, window close-prevention (closing hides to tray instead of quitting), and 24 registered commands. The `SellerState` (containing a shutdown oneshot channel) is managed via `tauri::manage`.
- **Commands**: `src-tauri/src/commands.rs` — all `#[tauri::command]` functions. Organized into groups:
  - **Wallet**: `wallet_create`, `wallet_import`, `wallet_info` — use `libproxybase::WalletManager`, keyfile stored at `~/.proxybase/wallet/keyfile.enc`
  - **Auth**: `login` — challenge/verify flow with ECDSA signature using `libproxybase`. Session token saved to `~/.proxybase/session_token`
  - **Buyer**: `get_balance`, `transfer`, `list_currencies`, `create_deposit`, `get_deposit`, `list_deposits`
  - **Seller**: `register_seller`, `seller_status`, `list_payouts`
  - **Market**: `list_countries`, `list_pricing`, `create_session`, `close_session`, `list_sessions`
  - **Session**: `get_token`, `logout`
- **API client**: `src-tauri/src/api.rs` — `BackendClient` wraps `reqwest::Client` for all API calls. Base URL from the frontend is passed per-command. Token loaded/saved from `~/.proxybase/session_token`. Includes a method to build the seller WebSocket URL.
- **Seller relay**: `src-tauri/src/seller.rs` — the background seller process. Connects to the backend via WebSocket (`tokio-tungstenite`). Handles `stream_open` messages by spawning TCP relay tasks (`run_stream_relay`), which bridge the WebSocket connection to an upstream target (SOCKS5 proxy or direct TCP). Uses `fast-socks5` for upstream proxy connections. Reconnects with exponential backoff (1s–60s, 20% jitter). Communicates with the frontend exclusively through Tauri events (`seller:connected`, `seller:disconnected`, `seller:error`, `seller:reconnecting`, `seller:stream-open`, `seller:stream-closed`) — no return values from `start_seller`/`stop_seller` commands.
- **Dependencies**: `libproxybase` (local path `../../libproxybase` — shared wallet/auth library), `fast-socks5` (local path `../../proxybase2-backend/fast-socks5` — custom SOCKS5 client with username/password auth), `reqwest`, `tokio-tungstenite`, `serde` + `serde_json`.

### File storage layout

Everything lives under `~/.proxybase/`:
- `wallet/keyfile.enc` — encrypted wallet keyfile
- `session_token` — bearer token for API auth
- `config.toml` — configuration

### Tauri store

The `tauri-plugin-store` plugin persists to `proxybase-settings.json`:
- `seller_running` (boolean) — whether the seller was running on last close (for auto-resume)
- `seller_config` (object) — `{ upstreams: UpstreamProxy[], includeDirect: boolean }` cached config

### Plugins

- `tauri-plugin-opener` — open URLs in external browser
- `tauri-plugin-store` — persistent key-value storage
- `tauri-plugin-autostart` — launch on system boot (macOS: LaunchAgent)

## Design system

The full design spec is in `design/design.md`. It defines a Vercel-inspired design language. Key points:

- **Fonts**: Geometric sans (Inter 400/500/600) for body/display; monospace (JetBrains Mono 400) for code/technical labels. Display weights never exceed 600. Headlines are sentence-case with negative letter-spacing.
- **Colors**: Ink-near-black `#171717` primary; near-white `#fafafa` page background; pure white `#ffffff` cards; a 200-step gray scale for dividers/borders. A multi-stop mesh gradient (cyan → blue → magenta → amber) is the only decoration, used at hero scale only.
- **Buttons**: Two pill scales — 100px marketing CTAs and 6px nav buttons. Never mix on the same screen.
- **Elevation**: Stacked shadows (multiple small offsets + inset hairline ring), never a single heavy drop-shadow.
- **Shapes**: Border radius tokens from 4px to 9999px (full round). Cards use 8-12px.
- **Spacing**: 4px base unit. Section padding 64-96px. Content max-width ~1400px.

Read `design/design.md` before implementing any UI — treat it as the source of truth for visual decisions.
