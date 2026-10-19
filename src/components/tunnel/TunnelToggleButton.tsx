import { useSystemTunnel } from "../../hooks/useSystemTunnel";
import { useI18n } from "../../i18n";

interface Props {
  proxyId: string;
  /** Compact variant for table rows. */
  compact?: boolean;
  className?: string;
}

/**
 * One-click "Tunnel System Traffic" control for a purchased proxy.
 *
 * - disconnected → connect
 * - connected to this proxy → disconnect
 * - connected to another proxy → hot-swap without recreating the TUN adapter
 */
export default function TunnelToggleButton({ proxyId, compact = false, className = "" }: Props) {
  const { t } = useI18n();
  const { state, enable, disable, busy } = useSystemTunnel();

  const sameProxy =
    state.status === "connected" && state.proxy_id === proxyId;
  const otherProxy =
    (state.status === "connected" ||
      state.status === "reconnecting" ||
      state.status === "configuring_routes" ||
      state.status === "verifying_exit_node") &&
    "proxy_id" in state &&
    state.proxy_id !== proxyId;
  const inProgress =
    state.status === "requesting_privileges" ||
    state.status === "configuring_routes" ||
    state.status === "verifying_exit_node" ||
    state.status === "reconnecting" ||
    state.status === "disconnecting";

  const label = sameProxy
    ? t("tunnel.active")
    : otherProxy
      ? t("tunnel.switchHere")
      : inProgress
        ? t("tunnel.connecting")
        : t("tunnel.enable");

  const errorMessage = state.status === "error" ? state.message : undefined;

  return (
    <button
      className={`btn ${sameProxy ? "btn-success" : "btn-secondary"} ${compact ? "btn-sm" : ""} ${className}`}
      style={
        compact
          ? { padding: "0 8px", height: 26, fontSize: 11, fontWeight: 600, whiteSpace: "nowrap" }
          : undefined
      }
      disabled={busy || inProgress}
      title={errorMessage || label}
      onClick={(event) => {
        event.stopPropagation();
        if (sameProxy) {
          void disable();
        } else {
          void enable(proxyId);
        }
      }}
    >
      {inProgress && (
        <svg
          width="11"
          height="11"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          strokeLinecap="round"
          style={{ marginRight: 5, animation: "spin 1s linear infinite" }}
        >
          <path d="M21 12a9 9 0 1 1-6.22-8.56" />
        </svg>
      )}
      {label}
    </button>
  );
}
