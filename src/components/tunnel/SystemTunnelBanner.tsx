import { useEffect, useState } from "react";
import { CountryFlag } from "../CountryFlag";
import { isTunnelActive, useSystemTunnel } from "../../hooks/useSystemTunnel";
import { useI18n } from "../../i18n";
import TunnelSettingsModal from "./TunnelSettingsModal";

function formatBytesPerSecond(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B/s";
  const units = ["B/s", "KB/s", "MB/s", "GB/s"];
  let v = value;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0s";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

/**
 * Persistent top-of-app status bar while the system tunnel is active.
 */
export default function SystemTunnelBanner({ onTopUp }: { onTopUp?: () => void }) {
  const { t } = useI18n();
  const { state, telemetry, disable, dismissError, busy } = useSystemTunnel();
  const [showSettings, setShowSettings] = useState(false);
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));

  useEffect(() => {
    if (state.status !== "connected") return;
    const interval = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(interval);
  }, [state.status]);

  if (!isTunnelActive(state)) return null;

  const connected = state.status === "connected";
  const errored = state.status === "error";
  const statusText = connected
    ? t("tunnel.connected")
    : errored
      ? t("tunnel.error")
      : state.status === "reconnecting"
        ? t("tunnel.reconnecting")
        : state.status === "disconnecting"
          ? t("tunnel.disconnecting")
          : t("tunnel.connecting");

  const connectedFor =
    connected ? Math.max(0, now - state.connected_since_unix) : 0;
  const quotaExhausted =
    state.status === "error" &&
    (state.code === "quota_exhausted" || state.code === "lease_expired");
  const permissionError =
    state.status === "error" &&
    (state.code === "privilege_elevation_denied" ||
      state.code === "vpn_permission_denied_on_android");

  return (
    <>
      <div className={`tunnel-banner ${errored ? "tunnel-banner-error" : connected ? "tunnel-banner-active" : ""}`}>
        <div className="tunnel-banner-main">
          <span className={`status-dot ${connected ? "status-dot-connected" : "status-dot-disconnected"}`} />
          <span className="tunnel-banner-title">{statusText}</span>
          {connected && (
            <span className="tunnel-banner-meta">
              {state.country_code ? <CountryFlag code={state.country_code} /> : null}
              <span className="tunnel-mono">{state.exit_ip}</span>
              <span className="tunnel-mono">{state.latency_ms} ms</span>
              <span className="tunnel-mono">{formatDuration(connectedFor)}</span>
              <span className="tunnel-mono">
                ↓ {formatBytesPerSecond(telemetry.rx_bps)} ↑ {formatBytesPerSecond(telemetry.tx_bps)}
              </span>
              <span className="tunnel-mono">
                {t("tunnel.flows", { count: telemetry.active_flows })}
              </span>
            </span>
          )}
          {errored && <span className="tunnel-banner-error-text">{state.message}</span>}
          {!connected && !errored && <span className="tunnel-banner-meta">{t("tunnel.pleaseWait")}</span>}
        </div>
        <div className="tunnel-banner-actions">
          {quotaExhausted && onTopUp && (
            <button
              className="btn btn-primary btn-sm"
              style={{ padding: "0 10px", height: 26, fontSize: 11 }}
              onClick={onTopUp}
            >
              {t("tunnel.topUp")}
            </button>
          )}
          <button
            className="btn btn-secondary btn-sm"
            style={{ padding: "0 10px", height: 26, fontSize: 11 }}
            onClick={() => setShowSettings(true)}
            title={t("tunnel.settings")}
          >
            {t("tunnel.settings")}
          </button>
          <button
            className="btn btn-danger btn-sm"
            style={{ padding: "0 10px", height: 26, fontSize: 11 }}
            disabled={busy}
            onClick={() => (permissionError ? dismissError() : void disable())}
          >
            {permissionError ? t("common.close") : t("tunnel.disconnect")}
          </button>
        </div>
      </div>
      <TunnelSettingsModal open={showSettings} onClose={() => setShowSettings(false)} />
    </>
  );
}
