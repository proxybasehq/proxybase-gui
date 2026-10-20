import { useSystemTunnel } from "../../hooks/useSystemTunnel";
import { useI18n } from "../../i18n";

const EMOJI_FONT =
  "'Twemoji Country Flags', 'Apple Color Emoji', 'Segoe UI Emoji', 'Noto Color Emoji', sans-serif";

/** Shield = protected / tunneled traffic. */
const TUNNEL_EMOJI = "🛡️";
/** Used when this row would hot-swap an existing tunnel to another proxy. */
const SWITCH_EMOJI = "🔀";

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

  // Tooltip / accessible name always uses the full, descriptive label.
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
          ? {
              padding: 0,
              width: 26,
              height: 26,
              minWidth: 26,
              fontSize: 14,
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              borderRadius: 999,
            }
          : {
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 7,
            }
      }
      disabled={busy || inProgress}
      title={errorMessage || label}
      aria-label={label}
      onClick={(event) => {
        event.stopPropagation();
        if (sameProxy) {
          void disable();
        } else {
          void enable(proxyId);
        }
      }}
    >
      {inProgress ? (
        <svg
          width={compact ? 14 : 12}
          height={compact ? 14 : 12}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          strokeLinecap="round"
          style={{ animation: "spin 1s linear infinite" }}
        >
          <path d="M21 12a9 9 0 1 1-6.22-8.56" />
        </svg>
      ) : (
        <span
          aria-hidden="true"
          style={{
            fontFamily: EMOJI_FONT,
            fontSize: compact ? 16 : 15,
            lineHeight: 1,
          }}
        >
          {otherProxy ? SWITCH_EMOJI : TUNNEL_EMOJI}
        </span>
      )}
      {!compact && <span>{label}</span>}
    </button>
  );
}
