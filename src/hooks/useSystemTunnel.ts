import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  disableSystemTunnel,
  enableSystemTunnel,
  getSystemTunnelStatus,
  getTunnelTelemetry,
  type TunnelState,
  type TunnelTelemetry,
} from "../api";

export interface TunnelSettings {
  killSwitch: boolean;
  disableQuicUdp443: boolean;
  blockWebrtcStun: boolean;
  stopOnGuiExit: boolean;
  bypassCidrs: string[];
}

export const DEFAULT_TUNNEL_SETTINGS: TunnelSettings = {
  killSwitch: true,
  disableQuicUdp443: true,
  blockWebrtcStun: true,
  stopOnGuiExit: true,
  bypassCidrs: [],
};

const STORAGE_KEY = "proxybase_tunnel_settings";

export function loadTunnelSettings(): TunnelSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_TUNNEL_SETTINGS;
    const parsed = JSON.parse(raw) as Partial<TunnelSettings>;
    return {
      ...DEFAULT_TUNNEL_SETTINGS,
      ...parsed,
      bypassCidrs: Array.isArray(parsed.bypassCidrs)
        ? parsed.bypassCidrs.filter((c): c is string => typeof c === "string")
        : [],
    };
  } catch {
    return DEFAULT_TUNNEL_SETTINGS;
  }
}

export function persistTunnelSettings(settings: TunnelSettings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Storage unavailable — settings still apply to this session.
  }
}

const ACTIVE_STATUSES = new Set([
  "requesting_privileges",
  "configuring_routes",
  "verifying_exit_node",
  "connected",
  "reconnecting",
  "disconnecting",
  "error",
]);

export function isTunnelActive(state: TunnelState): boolean {
  return ACTIVE_STATUSES.has(state.status);
}

export function useSystemTunnel() {
  const [state, setState] = useState<TunnelState>({ status: "disconnected" });
  const [telemetry, setTelemetry] = useState<TunnelTelemetry>({
    rx_bytes_total: 0,
    tx_bytes_total: 0,
    rx_bps: 0,
    tx_bps: 0,
    active_flows: 0,
  });
  const [settings, setSettingsState] = useState<TunnelSettings>(loadTunnelSettings);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mounted = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const next = await getSystemTunnelStatus();
      if (mounted.current) setState(next);
    } catch {
      // Daemon not installed yet or app shutting down; keep the last state.
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    let unlisteners: UnlistenFn[] = [];
    (async () => {
      try {
        unlisteners = await Promise.all([
          listen<TunnelState>("tunnel://state-changed", (event) => {
            if (mounted.current) setState(event.payload);
          }),
          listen<TunnelTelemetry>("tunnel://telemetry", (event) => {
            if (mounted.current) setTelemetry(event.payload);
          }),
        ]);
      } catch {
        // Events are unavailable outside Tauri; polling still works.
      }
      await refresh();
    })();

    // Poll as a fallback for platforms where events fire before the webview
    // subscribes (and for Android service transitions).
    const interval = setInterval(() => {
      void refresh();
      void getTunnelTelemetry()
        .then((t) => {
          if (mounted.current) setTelemetry(t);
        })
        .catch(() => {});
    }, 4000);

    return () => {
      mounted.current = false;
      clearInterval(interval);
      for (const unlisten of unlisteners) unlisten();
    };
  }, [refresh]);

  const enable = useCallback(async (proxyId: string) => {
    setBusy(true);
    setError(null);
    // Read from storage so a settings change made in another hook instance
    // (e.g. the settings modal) is honoured immediately.
    const s = loadTunnelSettings();
    try {
      const next = await enableSystemTunnel(proxyId, {
        killSwitch: s.killSwitch,
        bypassCidrs: s.bypassCidrs,
        disableQuicUdp443: s.disableQuicUdp443,
        blockWebrtcStun: s.blockWebrtcStun,
        stopOnGuiExit: s.stopOnGuiExit,
      });
      if (mounted.current) setState(next);
      return next;
    } catch (e) {
      const message = String(e);
      setError(message);
      // A daemon-side failure has already emitted a specific error state;
      // only synthesise one for transport/elevation failures.
      setState((prev) =>
        prev.status === "error"
          ? prev
          : { status: "error", code: "internal_error", message }
      );
      return null;
    } finally {
      if (mounted.current) setBusy(false);
    }
  }, []);

  const disable = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const next = await disableSystemTunnel();
      if (mounted.current) {
        setState(next);
        setTelemetry({
          rx_bytes_total: 0,
          tx_bytes_total: 0,
          rx_bps: 0,
          tx_bps: 0,
          active_flows: 0,
        });
      }
      return next;
    } catch (e) {
      const message = String(e);
      setError(message);
      setState((prev) =>
        prev.status === "error"
          ? prev
          : { status: "error", code: "internal_error", message }
      );
      return null;
    } finally {
      if (mounted.current) setBusy(false);
    }
  }, []);

  const dismissError = useCallback(() => {
    setError(null);
    setState({ status: "disconnected" });
    setTelemetry({
      rx_bytes_total: 0,
      tx_bytes_total: 0,
      rx_bps: 0,
      tx_bps: 0,
      active_flows: 0,
    });
  }, []);

  const setSettings = useCallback((next: TunnelSettings) => {
    setSettingsState(next);
    persistTunnelSettings(next);
  }, []);

  return {
    state,
    telemetry,
    settings,
    setSettings,
    enable,
    disable,
    dismissError,
    refresh,
    busy,
    error,
  };
}
