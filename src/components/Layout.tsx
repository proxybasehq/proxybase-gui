import { Outlet, useNavigate } from "react-router-dom";
import { useEffect, useState, useCallback, useRef } from "react";
import BottomNav from "./BottomNav";
import { useBackend } from "../hooks/useBackend";
import type { StreamEvent, UpstreamProxy } from "../api";
import {
  walletInfo,
  login as apiLogin,
  logout as apiLogout,
  registerSeller,
  startSeller as apiStartSeller,
  stopSeller as apiStopSeller,
  listSessions,
  closeSession,
  createDeposit,
  listCurrencies,
} from "../api";
import { listen } from "@tauri-apps/api/event";
import { load } from "@tauri-apps/plugin-store";
import { enable } from "@tauri-apps/plugin-autostart";
import QRCode from "qrcode";
import { setPendingDeposit } from "../pages/DepositPage";

export interface SellerState {
  running: boolean;
  connected: boolean;
  streams: StreamEvent[];
  error: string;
}

export interface AppContext {
  onLoginSuccess: () => void;
  isAuthenticated: boolean;
  seller: SellerState;
  startSeller: (backendUrl: string, upstreams: UpstreamProxy[], includeDirect: boolean) => Promise<void>;
  stopSeller: () => Promise<void>;
  openDeposit: () => void;
  handleLogout: () => Promise<void>;
  walletAddr: string;
  walletLoaded: boolean;
}

export default function Layout() {
  const { backendUrl } = useBackend();
  const navigate = useNavigate();
  const [walletAddr, setWalletAddr] = useState("");
  const [walletLoaded, setWalletLoaded] = useState(false);
  const [isAuth, setIsAuth] = useState(false);
  const didAutoLogin = useRef(false);

  // ---- Deposit modal state ----
  const [showDeposit, setShowDeposit] = useState(false);
  const [depError, setDepError] = useState("");
  const [depLoading, setDepLoading] = useState(false);
  const [depPreset, setDepPreset] = useState<number | "other">(10);
  const [depCustomAmount, setDepCustomAmount] = useState("");
  const [depCurrency, setDepCurrency] = useState("usdcsol");
  const [currencies, setCurrencies] = useState<string[]>(["usdcsol"]);

  const PRESETS = [10, 20, 100] as const;

  function depositAmount(): number | null {
    if (depPreset === "other") {
      const v = parseFloat(depCustomAmount);
      return isNaN(v) || v <= 0 ? null : v;
    }
    return depPreset;
  }

  function closeDeposit() {
    setShowDeposit(false);
    setDepError("");
    setDepPreset(10);
    setDepCustomAmount("");
  }

  // Load currencies when deposit modal opens
  async function openDeposit() {
    setShowDeposit(true);
    try {
      const r = await listCurrencies(backendUrl);
      const arr = (r as any).currencies || [];
      if (arr.length > 0) { setCurrencies(arr); setDepCurrency(arr[0]); }
    } catch (_) { /* keep default */ }
  }

  async function handleCreateDeposit() {
    setDepError("");
    const amount = depositAmount();
    if (amount === null) { setDepError("Enter a valid amount"); return; }
    setDepLoading(true);
    try {
      const r = await createDeposit(backendUrl, Math.round(amount * 1_000_000), depCurrency);
      const addr = (r as any).pay_address || "";
      let qrDataUrl = "";
      if (addr) {
        qrDataUrl = await QRCode.toDataURL(addr, { width: 200, margin: 1 });
      }
      // Close modal and navigate to dedicated deposit page
      setPendingDeposit({
        deposit_id: (r as any).deposit_id,
        pay_address: addr,
        pay_currency: (r as any).pay_currency,
        pay_amount: (r as any).pay_amount,
        amount_microcredits: Math.round(amount * 1_000_000),
        qrDataUrl,
      });
      setShowDeposit(false);
      setDepPreset(10);
      setDepCustomAmount("");
      navigate("/deposit");
    } catch (e) {
      const msg = String(e);
      if (msg.toLowerCase().includes("too small")) {
        setDepError("Amount is too small. The minimum is set by the payment provider. Try a larger amount.");
      } else {
        setDepError(msg);
      }
    }
    setDepLoading(false);
  }

  // ---- Seller background state ----
  const [sellerRunning, setSellerRunning] = useState(false);
  const [sellerConnected, setSellerConnected] = useState(false);
  const [sellerStreams, setSellerStreams] = useState<StreamEvent[]>([]);
  const [sellerError, setSellerError] = useState("");

  useEffect(() => {
    const unlistens: Array<() => void> = [];
    listen<StreamEvent>("seller:stream-open", (event) => {
      setSellerStreams((prev) => [...prev, event.payload]);
    }).then((fn) => unlistens.push(fn));
    listen<string>("seller:stream-closed", (event) => {
      setSellerStreams((prev) => prev.filter((s) => s.session_id !== event.payload));
    }).then((fn) => unlistens.push(fn));
    listen<string>("seller:connected", () => {
      setSellerConnected(true); setSellerRunning(true); setSellerError("");
    }).then((fn) => unlistens.push(fn));
    listen<string>("seller:disconnected", () => {
      setSellerConnected(false); setSellerRunning(false); setSellerStreams([]);
    }).then((fn) => unlistens.push(fn));
    listen<string>("seller:error", (event) => {
      setSellerError(event.payload); setSellerRunning(false); setSellerConnected(false); setSellerStreams([]);
    }).then((fn) => unlistens.push(fn));
    listen<string>("seller:reconnecting", (event) => {
      setSellerError(event.payload);
    }).then((fn) => unlistens.push(fn));
    return () => { unlistens.forEach((fn) => fn()); };
  }, []);

  async function handleStartSeller(beUrl: string, upstreams: UpstreamProxy[], includeDirect: boolean) {
    setSellerError(""); setSellerStreams([]);
    await registerSeller(beUrl);
    await apiStartSeller(beUrl, upstreams, includeDirect);
    setSellerRunning(true);
    try {
      const store = await load("proxybase-settings.json");
      await store.set("seller_config", { upstreams, includeDirect });
      await store.set("seller_running", true);
    } catch (_) { /* ignore */ }
  }

  async function handleStopSeller() {
    await apiStopSeller();
    setSellerRunning(false); setSellerConnected(false);
    try {
      const store = await load("proxybase-settings.json");
      await store.set("seller_running", false);
    } catch (_) { /* ignore */ }
  }

  const seller: SellerState = { running: sellerRunning, connected: sellerConnected, streams: sellerStreams, error: sellerError };

  // ---- Auth ----
  const checkAuth = useCallback(() => {
    walletInfo().then(async (info) => {
      setWalletLoaded(info.loaded); setWalletAddr(info.address);
      if (!info.loaded) { setIsAuth(false); return; }
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("get_balance", { backendUrl });
        setIsAuth(true);
        if (!didAutoLogin.current) { didAutoLogin.current = true; navigate("/market"); }
        return;
      } catch (_) { /* no session */ }
      try {
        await apiLogin(backendUrl, "");
        setIsAuth(true);
        if (!didAutoLogin.current) { didAutoLogin.current = true; navigate("/market"); }
      } catch (_) { setIsAuth(false); }
    }).catch(() => {});
  }, [backendUrl]);

  useEffect(() => { checkAuth(); }, [checkAuth]);
  useEffect(() => { enable().catch(() => {}); }, []);

  useEffect(() => {
    if (!isAuth || sellerRunning) return;
    load("proxybase-settings.json").then(async (store) => {
      const wasRunning = await store.get<boolean>("seller_running");
      if (!wasRunning) return;
      const cfg = await store.get<{ upstreams: UpstreamProxy[]; includeDirect: boolean }>("seller_config");
      if (!cfg) return;
      handleStartSeller(backendUrl, cfg.upstreams || [], cfg.includeDirect ?? true);
    }).catch(() => {});
  }, [isAuth]);

  async function handleLogout() {
    if (sellerRunning) { try { await apiStopSeller(); } catch (_) {} }
    try { const store = await load("proxybase-settings.json"); await store.set("seller_running", false); } catch (_) {}
    try {
      const r = await listSessions(backendUrl);
      const sessions = (r as any).sessions || [];
      for (const s of sessions) { try { await closeSession(backendUrl, s.session_id); } catch (_) {} }
    } catch (_) {}
    try { await apiLogout(); } catch (_) {}
    setIsAuth(false);
    navigate("/wallet");
  }

  function handleLoginSuccess() { setIsAuth(true); navigate("/market"); }

  const context: AppContext = {
    onLoginSuccess: handleLoginSuccess, isAuthenticated: isAuth, seller,
    startSeller: handleStartSeller, stopSeller: handleStopSeller,
    openDeposit, handleLogout, walletAddr, walletLoaded,
  };

  return (
    <div className="app-shell">
      <header className="app-header">
        <span style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <span className="app-header-logo" style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <img src="/logo.svg" alt="" style={{ width: 24, height: 24, borderRadius: 6 }} />
            ProxyBase
          </span>
          <a href="https://discord.gg/7uedk7ajHD" target="_blank" rel="noopener noreferrer" title="Discord"
            style={{ display: "flex", alignItems: "center", color: "var(--color-mute)" }}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
              <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.085 2.157 2.419 0 1.334-.956 2.419-2.157 2.419zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.085 2.157 2.419 0 1.334-.946 2.419-2.157 2.419z"/>
            </svg>
          </a>
        </span>
        <div className="app-header-status">
          <span
            style={{ cursor: "pointer", display: "flex", alignItems: "center", gap: 6 }}
            onClick={() => navigate("/account")} title="Account">
            <span className={`status-dot ${isAuth ? "status-dot-connected" : "status-dot-disconnected"}`} />
            <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="8" r="4" />
              <path d="M4 21v-1a6 6 0 0 1 6-6h4a6 6 0 0 1 6 6v1" />
            </svg>
          </span>
        </div>
      </header>

      <main className="app-content"><Outlet context={context} /></main>
      <BottomNav authenticated={isAuth} walletLoaded={walletLoaded} />

      {/* ---- Deposit modal ---- */}
      {showDeposit && (
        <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={closeDeposit}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="card-title">Create Deposit</div>

            <div className="form-group" style={{ marginTop: "var(--space-sm)" }}>
              <label className="form-label">Amount</label>
              <div style={{ display: "flex", gap: "var(--space-xs)", marginBottom: "var(--space-sm)" }}>
                {PRESETS.map((p) => (
                  <button
                    key={p}
                    className={depPreset === p ? "btn btn-primary btn-sm" : "btn btn-secondary btn-sm"}
                    style={{ flex: 1, height: 36, fontSize: 14, fontWeight: 600 }}
                    onClick={() => setDepPreset(p)}
                  >
                    ${p}
                  </button>
                ))}
                <button
                  className={depPreset === "other" ? "btn btn-primary btn-sm" : "btn btn-secondary btn-sm"}
                  style={{ flex: 1, height: 36, fontSize: 14, fontWeight: 600 }}
                  onClick={() => setDepPreset("other")}
                >
                  Other
                </button>
              </div>
              {depPreset === "other" && (
                <input
                  type="number" step="0.01" min="0.01"
                  className="form-input"
                  value={depCustomAmount}
                  onChange={(e) => setDepCustomAmount(e.target.value)}
                  placeholder="Enter amount..."
                />
              )}
            </div>
            <div className="form-group">
              <label className="form-label">Currency</label>
              <select className="form-select" value={depCurrency} onChange={(e) => setDepCurrency(e.target.value)}>
                {currencies.map((c) => (<option key={c} value={c}>{c}</option>))}
              </select>
            </div>
            {depError && <div className="alert alert-error">{depError}</div>}
            <div style={{ display: "flex", gap: "var(--space-sm)", marginTop: "var(--space-md)" }}>
              <button className="btn btn-primary" onClick={handleCreateDeposit}
                disabled={depLoading || depositAmount() === null}
                style={{ flex: 1 }}>{depLoading ? "Creating..." : "Create Deposit"}</button>
              <button className="btn btn-secondary" onClick={closeDeposit}>Cancel</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
