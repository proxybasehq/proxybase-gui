import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
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

// ---------------------------------------------------------------------------
// Shared Module Store (Synchronizes Banner, Buttons, and Settings Modal)
// ---------------------------------------------------------------------------

let globalState: TunnelState = { status: "disconnected" };
let globalTelemetry: TunnelTelemetry = {
  rx_bytes_total: 0,
  tx_bytes_total: 0,
  rx_bps: 0,
  tx_bps: 0,
  active_flows: 0,
};
let globalSettings: TunnelSettings = loadTunnelSettings();
let globalBusy = false;
let globalError: string | null = null;
const listeners = new Set<() => void>();

function notify() {
  for (const listener of listeners) {
    listener();
  }
}

function setGlobalState(next: TunnelState) {
  globalState = next;
  notify();
}

function setGlobalTelemetry(next: TunnelTelemetry) {
  globalTelemetry = next;
  notify();
}

function setGlobalBusy(next: boolean) {
  globalBusy = next;
  notify();
}

function setGlobalError(next: string | null) {
  globalError = next;
  notify();
}

let eventsInitialized = false;

async function refreshTunnelState() {
  try {
    const next = await getSystemTunnelStatus();
    // If we are currently showing a user-visible error and the daemon says disconnected,
    // do not wipe the error banner until the user dismisses or retries.
    if (globalState.status === "error" && next.status === "disconnected") {
      return;
    }
    setGlobalState(next);
  } catch {
    // Daemon not installed yet or app shutting down; keep the last state.
  }
}

function initGlobalTunnelEvents() {
  if (eventsInitialized) return;
  eventsInitialized = true;

  (async () => {
    try {
      await Promise.all([
        listen<TunnelState>("tunnel://state-changed", (event) => {
          setGlobalState(event.payload);
        }),
        listen<TunnelTelemetry>("tunnel://telemetry", (event) => {
          setGlobalTelemetry(event.payload);
        }),
      ]);
    } catch {
      // Events are unavailable outside Tauri; polling still works.
    }
    await refreshTunnelState();
  })();

  // Poll as fallback for platforms where events fire before the webview subscribes
  setInterval(() => {
    void refreshTunnelState();
    void getTunnelTelemetry()
      .then((t) => {
        setGlobalTelemetry(t);
      })
      .catch(() => {});
  }, 4000);
}

export function useSystemTunnel() {
  const [, setTick] = useState(0);

  useEffect(() => {
    initGlobalTunnelEvents();
    const listener = () => setTick((t) => t + 1);
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  const refresh = useCallback(async () => {
    await refreshTunnelState();
  }, []);

  const enable = useCallback(async (proxyId: string) => {
    setGlobalBusy(true);
    setGlobalError(null);
    const s = loadTunnelSettings();
    try {
      console.log(`[tunnel] enabling system tunnel for proxy: ${proxyId}`);
      const next = await enableSystemTunnel(proxyId, {
        killSwitch: s.killSwitch,
        bypassCidrs: s.bypassCidrs,
        disableQuicUdp443: s.disableQuicUdp443,
        blockWebrtcStun: s.blockWebrtcStun,
        stopOnGuiExit: s.stopOnGuiExit,
      });
      setGlobalState(next);
      return next;
    } catch (e) {
      const message = String(e);
      console.error("[tunnel] enableSystemTunnel failed:", message);
      setGlobalError(message);
      // The daemon usually emits a structured error event (e.g.
      // `conflicting_vpn`) just before the invoke rejects. Preserve that code
      // so the banner can show the specific title; only synthesize a generic
      // error when no matching event arrived.
      const existing = globalState;
      if (existing.status === "error" && existing.message === message) {
        return null;
      }
      setGlobalState({
        status: "error",
        code: "internal_error",
        message,
      });
      return null;
    } finally {
      setGlobalBusy(false);
    }
  }, []);

  const disable = useCallback(async () => {
    setGlobalBusy(true);
    setGlobalError(null);
    try {
      console.log("[tunnel] disabling system tunnel");
      const next = await disableSystemTunnel();
      setGlobalState(next);
      setGlobalTelemetry({
        rx_bytes_total: 0,
        tx_bytes_total: 0,
        rx_bps: 0,
        tx_bps: 0,
        active_flows: 0,
      });
      return next;
    } catch (e) {
      const message = String(e);
      console.error("[tunnel] disableSystemTunnel failed:", message);
      setGlobalError(message);
      setGlobalState({
        status: "error",
        code: "internal_error",
        message,
      });
      return null;
    } finally {
      setGlobalBusy(false);
    }
  }, []);

  const dismissError = useCallback(() => {
    setGlobalError(null);
    setGlobalState({ status: "disconnected" });
    setGlobalTelemetry({
      rx_bytes_total: 0,
      tx_bytes_total: 0,
      rx_bps: 0,
      tx_bps: 0,
      active_flows: 0,
    });
  }, []);

  const setSettings = useCallback((next: TunnelSettings) => {
    globalSettings = next;
    persistTunnelSettings(next);
    notify();
  }, []);

  return {
    state: globalState,
    telemetry: globalTelemetry,
    settings: globalSettings,
    setSettings,
    enable,
    disable,
    dismissError,
    refresh,
    busy: globalBusy,
    error: globalError,
  };
}
