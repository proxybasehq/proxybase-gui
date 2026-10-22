import { useEffect, useState } from "react";
import { CountryFlag } from "../CountryFlag";
import { getTunnelDiagnostics } from "../../api";
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
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const [diagnostics, setDiagnostics] = useState<string | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [diagnosticsCopied, setDiagnosticsCopied] = useState(false);
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));

  async function openDiagnostics() {
    setDiagnosticsOpen(true);
    setDiagnostics(null);
    setDiagnosticsError(null);
    setDiagnosticsCopied(false);
    try {
      setDiagnostics(await getTunnelDiagnostics());
    } catch (e) {
      setDiagnosticsError(String(e));
    }
  }

  async function copyDiagnostics() {
    if (!diagnostics) return;
    try {
      await navigator.clipboard.writeText(diagnostics);
      setDiagnosticsCopied(true);
    } catch {
      // Clipboard may be unavailable; the text remains selectable.
    }
  }

  useEffect(() => {
    if (state.status !== "connected") return;
    const interval = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(interval);
  }, [state.status]);

  if (!isTunnelActive(state)) return null;

  const connected = state.status === "connected";
  const errored = state.status === "error";
  const vpnConflict = errored && state.code === "conflicting_vpn";
  const statusText = connected
    ? t("tunnel.connected")
    : errored
      ? vpnConflict
        ? t("tunnel.vpnConflict")
        : t("tunnel.error")
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
        <div className="tunnel-banner-row">
          <div className="tunnel-banner-heading">
            <span className={`status-dot ${connected ? "status-dot-connected" : "status-dot-disconnected"}`} />
            <span className="tunnel-banner-title">{statusText}</span>
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
            {!errored && (
              <button
                className="btn btn-secondary btn-sm"
                style={{ padding: "0 10px", height: 26, fontSize: 11 }}
                onClick={() => setShowSettings(true)}
                title={t("tunnel.settings")}
              >
                {t("tunnel.settings")}
              </button>
            )}
            {errored && (
              <button
                className="btn btn-secondary btn-sm"
                style={{ padding: "0 10px", height: 26, fontSize: 11 }}
                onClick={() => void openDiagnostics()}
                title={t("tunnel.diagnosticsDesc")}
              >
                {t("tunnel.diagnostics")}
              </button>
            )}
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

        {connected && (
          <>
            <div className="tunnel-banner-meta-row">
              {state.country_code ? <CountryFlag code={state.country_code} /> : null}
              <span className="tunnel-mono">{state.exit_ip}</span>
              <span className="tunnel-mono">{state.latency_ms} ms</span>
              <span className="tunnel-mono">{formatDuration(connectedFor)}</span>
            </div>
            <div className="tunnel-banner-meta-row">
              <span className="tunnel-mono">↓ {formatBytesPerSecond(telemetry.rx_bps)}</span>
              <span className="tunnel-mono">↑ {formatBytesPerSecond(telemetry.tx_bps)}</span>
              <span className="tunnel-mono">
                {t("tunnel.flows", { count: telemetry.active_flows })}
              </span>
            </div>
          </>
        )}

        {errored && (
          <div className="tunnel-banner-error-text" title={state.message}>
            {state.message}
          </div>
        )}

        {!connected && !errored && (
          <div className="tunnel-banner-meta-row">{t("tunnel.pleaseWait")}</div>
        )}
      </div>
      <TunnelSettingsModal open={showSettings} onClose={() => setShowSettings(false)} />

      {diagnosticsOpen && (
        <div
          style={{
            position: "fixed",
            inset: 0,
            background: "rgba(0,0,0,0.4)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 1100,
          }}
          onClick={() => setDiagnosticsOpen(false)}
        >
          <div
            className="modal-card"
            style={{
              width: "min(640px, 94vw)",
              maxHeight: "86vh",
              display: "flex",
              flexDirection: "column",
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="card-title">{t("tunnel.diagnosticsTitle")}</div>
            <p className="text-muted" style={{ fontSize: 12, marginTop: 4 }}>
              {t("tunnel.diagnosticsDesc")}
            </p>
            {diagnosticsError ? (
              <p style={{ fontSize: 12, color: "var(--color-danger, #dc2626)" }}>
                {diagnosticsError}
              </p>
            ) : diagnostics === null ? (
              <p className="text-muted" style={{ fontSize: 12 }}>
                {t("tunnel.pleaseWait")}
              </p>
            ) : (
              <textarea
                readOnly
                value={diagnostics}
                rows={18}
                onFocus={(e) => e.currentTarget.select()}
                style={{
                  width: "100%",
                  flex: 1,
                  minHeight: 240,
                  fontFamily: "JetBrains Mono, monospace",
                  fontSize: 11,
                  padding: 8,
                  borderRadius: 6,
                  border: "1px solid var(--color-border, #e5e5e5)",
                  resize: "vertical",
                  whiteSpace: "pre",
                }}
              />
            )}
            <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
              <button
                className="btn btn-secondary"
                style={{ flex: 1 }}
                disabled={!diagnostics}
                onClick={() => void copyDiagnostics()}
              >
                {diagnosticsCopied ? t("tunnel.diagnosticsCopied") : t("tunnel.diagnosticsCopy")}
              </button>
              <button
                className="btn btn-primary"
                style={{ flex: 1 }}
                onClick={() => setDiagnosticsOpen(false)}
              >
                {t("common.close")}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
