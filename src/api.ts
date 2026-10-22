import { invoke } from "@tauri-apps/api/core";

// ---- Types ----

export interface WalletInfo {
  address: string;
  loaded: boolean;
}

export interface CreateWalletResult {
  address: string;
  mnemonic: string;
}

export interface LoginResult {
  session_token: string;
  wallet_address: string;
  role: string;
  buyer_available: number;
  spendable_balance: number;
}

export interface UpstreamProxy {
  address: string;
  username: string;
  password: string;
}

export interface StreamEvent {
  session_id: string;
  target_ip: string;
  target_port: number;
  route_index: number | null;
}

// ---- Wallet ----

export async function walletCreate(password: string): Promise<CreateWalletResult> {
  return invoke("wallet_create", { password });
}

export async function walletImport(phrase: string, password: string): Promise<WalletInfo> {
  return invoke("wallet_import", { phrase, password });
}

export async function walletInfo(): Promise<WalletInfo> {
  return invoke("wallet_info");
}

// ---- Auth ----

export async function login(backendUrl: string, password: string): Promise<LoginResult> {
  return invoke("login", { backendUrl, password });
}

export async function getToken(): Promise<string> {
  return invoke("get_token");
}

export async function logout(): Promise<void> {
  return invoke("logout");
}

// ---- Buyer ----

export async function getBalance(backendUrl: string): Promise<Record<string, unknown>> {
  return invoke("get_balance", { backendUrl });
}

export async function transfer(backendUrl: string, amount: number): Promise<Record<string, unknown>> {
  return invoke("transfer", { backendUrl, amount });
}

export async function listCurrencies(backendUrl: string): Promise<{ currencies: string[] }> {
  return invoke("list_currencies", { backendUrl });
}

export async function createDeposit(
  backendUrl: string,
  amount: number,
  currency: string
): Promise<Record<string, unknown>> {
  return invoke("create_deposit", { backendUrl, amount, currency });
}

export async function getDeposit(
  backendUrl: string,
  depositId: string
): Promise<Record<string, unknown>> {
  return invoke("get_deposit", { backendUrl, depositId });
}

// ---- Seller ----

export async function registerSeller(
  backendUrl: string,
  nodeType?: string
): Promise<Record<string, unknown>> {
  return invoke("register_seller", { backendUrl, nodeType: nodeType ?? null });
}

export async function sellerStatus(backendUrl: string): Promise<Record<string, unknown>> {
  return invoke("seller_status", { backendUrl });
}

export async function startSeller(
  backendUrl: string,
  upstreams: UpstreamProxy[],
  includeDirect: boolean
): Promise<void> {
  return invoke("start_seller", {
    backendUrl,
    upstreamsJson: JSON.stringify(upstreams),
    includeDirect,
  });
}

export async function stopSeller(): Promise<void> {
  return invoke("stop_seller");
}

// ---- Market ----

export async function listCountries(backendUrl: string): Promise<Record<string, unknown>> {
  return invoke("list_countries", { backendUrl });
}

export async function listPricing(backendUrl: string): Promise<Record<string, unknown>> {
  return invoke("list_pricing", { backendUrl });
}

export interface CountryMetadata {
  country: string;
  country_code: string;
  country_name: string;
  flag_emoji: string;
}

export async function getCountryName(codeOrName: string): Promise<string> {
  return invoke("country_name", { codeOrName });
}

export async function getCountryCode(codeOrName: string): Promise<string> {
  return invoke("country_code", { codeOrName });
}

export async function getCountryFlag(codeOrName: string): Promise<string> {
  return invoke("country_flag", { codeOrName });
}

export async function getAllCountries(): Promise<Array<[string, string]>> {
  return invoke("get_all_countries");
}

export async function getAllCountryMetadata(): Promise<CountryMetadata[]> {
  return invoke("get_all_country_metadata");
}

export async function createSession(
  backendUrl: string,
  country: string,
  networkType: string,
  sessionType: string,
  spendCap: number | null
): Promise<Record<string, unknown>> {
  return invoke("create_session", {
    backendUrl,
    country,
    networkType,
    sessionType,
    spendCap,
  });
}

export async function closeSession(
  backendUrl: string,
  sessionId: string
): Promise<Record<string, unknown>> {
  return invoke("close_session", { backendUrl, sessionId });
}

export async function rotateSession(
  backendUrl: string,
  sessionId: string
): Promise<Record<string, unknown>> {
  return invoke("rotate_session", { backendUrl, sessionId });
}

export async function keepaliveSession(backendUrl: string, sessionId: string): Promise<void> {
  return invoke("keepalive_session", { backendUrl, sessionId });
}

export async function listSessions(backendUrl: string): Promise<Record<string, unknown>> {
  return invoke("list_sessions", { backendUrl });
}

export async function listDeposits(backendUrl: string): Promise<Record<string, unknown>> {
  return invoke("list_deposits", { backendUrl });
}

export async function listPayouts(backendUrl: string): Promise<Record<string, unknown>> {
  return invoke("list_payouts", { backendUrl });
}

// ---- Bridge ----

export async function bridgeStart(
  sessionId: string,
  upstreamAddr: string,
  upstreamUsername: string,
  upstreamPassword: string,
  preferredPort?: number,
): Promise<number> {
  return invoke("bridge_start", {
    sessionId,
    upstreamAddr,
    upstreamUsername,
    upstreamPassword,
    preferredPort: preferredPort ?? null,
  });
}

export async function bridgeStop(sessionId: string): Promise<void> {
  return invoke("bridge_stop", { sessionId });
}

export async function bridgePort(sessionId: string): Promise<number | null> {
  return invoke("bridge_port", { sessionId });
}

// ---- System tunnel (VPN mode) ----

export type TunnelMode = "layer3_tun" | "os_system_proxy_fallback";

export type TunnelState =
  | { status: "disconnected" }
  | { status: "requesting_privileges" }
  | { status: "configuring_routes"; proxy_id: string }
  | { status: "verifying_exit_node"; proxy_id: string }
  | {
      status: "connected";
      proxy_id: string;
      exit_ip: string;
      country_code?: string | null;
      isp?: string | null;
      latency_ms: number;
      connected_since_unix: number;
      mode: TunnelMode;
    }
  | { status: "reconnecting"; proxy_id: string; attempt: number; last_error: string }
  | { status: "disconnecting" }
  | { status: "error"; code: string; message: string };

export interface TunnelTelemetry {
  rx_bytes_total: number;
  tx_bytes_total: number;
  rx_bps: number;
  tx_bps: number;
  active_flows: number;
}

export interface TunnelEnableOptions {
  killSwitch?: boolean;
  bypassCidrs?: string[];
  disableQuicUdp443?: boolean;
  blockWebrtcStun?: boolean;
  stopOnGuiExit?: boolean;
}

export async function enableSystemTunnel(
  proxyId: string,
  options: TunnelEnableOptions = {}
): Promise<TunnelState> {
  return invoke("enable_system_tunnel", {
    proxyId,
    killSwitch: options.killSwitch ?? null,
    bypassCidrs: options.bypassCidrs ?? null,
    disableQuicUdp443: options.disableQuicUdp443 ?? null,
    blockWebrtcStun: options.blockWebrtcStun ?? null,
    stopOnGuiExit: options.stopOnGuiExit ?? null,
  });
}

export async function disableSystemTunnel(): Promise<TunnelState> {
  return invoke("disable_system_tunnel");
}

export async function getSystemTunnelStatus(): Promise<TunnelState> {
  return invoke("get_system_tunnel_status");
}

/** Daemon log tail + environment details, for support and bug reports. */
export async function getTunnelDiagnostics(): Promise<string> {
  return invoke("get_tunnel_diagnostics");
}

export async function getTunnelTelemetry(): Promise<TunnelTelemetry> {
  return invoke("get_tunnel_telemetry");
}
