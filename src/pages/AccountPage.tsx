import { useState, useEffect, useRef } from "react";
import { useOutletContext, Navigate, useNavigate } from "react-router-dom";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import { getBalance } from "../api";
import { formatUsd } from "../utils";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

interface AppInfo {
  version: string;
  git_hash: string;
  build_date: string;
  os: string;
  arch: string;
}

export default function AccountPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [appInfo, setAppInfo] = useState<AppInfo>({
    version: "0.1.0", git_hash: "dev", build_date: "local",
    os: "unknown", arch: "unknown",
  });

  useEffect(() => {
    invoke<AppInfo>("get_app_info")
      .then(setAppInfo)
      .catch(() => {});
  }, []);
  const { backendUrl } = useBackend();
  const {
    isAuthenticated, seller, openDeposit, handleLogout,
    walletAddr, walletLoaded,
  } = useOutletContext<AppContext>();

  if (!isAuthenticated) return <Navigate to="/login" replace />;

  const [balance, setBalance] = useState<Record<string, unknown> | null>(null);
  const [balanceLoading, setBalanceLoading] = useState(false);
  const [showBalance, setShowBalance] = useState(false);
  const [showInfo, setShowInfo] = useState(false);

  type UpdatePhase =
    | { kind: "idle" }
    | { kind: "checking" }
    | { kind: "uptodate" }
    | { kind: "available"; version: string }
    | { kind: "downloading"; progress: number | null }
    | { kind: "error" };
  const [updatePhase, setUpdatePhase] = useState<UpdatePhase>({ kind: "idle" });
  const updateRef = useRef<Update | null>(null);
  const downloadTotalRef = useRef<number>(0);

  async function handleCheckUpdate() {
    setUpdatePhase({ kind: "checking" });
    try {
      const update = await check();
      if (!update) {
        setUpdatePhase({ kind: "uptodate" });
        return;
      }
      updateRef.current = update;
      setUpdatePhase({ kind: "available", version: update.version });
    } catch (e) {
      console.error("Update check failed:", e);
      setUpdatePhase({ kind: "error" });
    }
  }

  async function handleInstallUpdate() {
    const update = updateRef.current;
    if (!update) return;
    setUpdatePhase({ kind: "downloading", progress: null });
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          downloadTotalRef.current = event.data?.contentLength ?? 0;
        } else if (event.event === "Progress" && event.data) {
          const total = downloadTotalRef.current;
          const pct = total > 0
            ? Math.min(100, Math.round((event.data.chunkLength / total) * 100))
            : null;
          setUpdatePhase({ kind: "downloading", progress: pct });
        }
      });
      await relaunch();
    } catch (e) {
      console.error("Update install failed:", e);
      setUpdatePhase({ kind: "error" });
    }
  }

  async function fetchBalance() {
    setShowBalance(true);
    setBalanceLoading(true);
    try { setBalance(await getBalance(backendUrl)); } catch (_) { setBalance(null); }
    setBalanceLoading(false);
  }

  function renderBalanceRows(data: Record<string, unknown>) {
    const mcFields: [string, string][] = [
      ["spendable_balance", t("account.spendable")], ["buyer_available", t("account.buyerAvailable")],
      ["buyer_reserved", t("account.buyerReserved")], ["buyer_spent", t("account.buyerSpent")],
      ["seller_pending", t("account.sellerPending")], ["seller_available", t("account.sellerAvailable")],
      ["seller_payout_locked", t("account.payoutLocked")],
    ];
    return (
      <table><tbody>
        {mcFields.map(([key, label]) => {
          const val = data[key];
          if (val === undefined || val === null) return null;
          return (<tr key={key}>
            <td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{label}</td>
            <td className="font-mono" style={{ fontSize: 13, textAlign: "right" }}>{formatUsd(val as number)}</td>
          </tr>);
        })}
      </tbody></table>
    );
  }

  return (
    <div>
      {/* ---- Wallet ---- */}
      <div className="card">
        <div className="card-title">{t("account.wallet")}</div>
        {walletLoaded ? (
          <>
            <div style={{ marginTop: "var(--space-sm)", marginBottom: "var(--space-md)" }}>
              <span className="text-muted" style={{ fontSize: 12 }}>{t("account.address")}</span>
              <div className="font-mono" style={{ fontSize: 13, wordBreak: "break-all", marginTop: 2 }}>
                {walletAddr}
              </div>
            </div>
            <div style={{ display: "flex", gap: "var(--space-sm)" }}>
              <button className="btn btn-secondary btn-sm" onClick={fetchBalance}>
                {balanceLoading ? t("common.loading") : t("account.viewBalance")}
              </button>
              <button className="btn btn-secondary btn-sm" onClick={() => navigate("/wallet")}>
                {t("account.manageWallet")}
              </button>
              <button className="btn btn-success btn-sm" onClick={openDeposit}>
                {t("account.addFunds")}
              </button>
            </div>
          </>
        ) : (
          <p className="text-muted" style={{ marginTop: "var(--space-sm)" }}>{t("account.noWallet")}</p>
        )}
      </div>

      {/* ---- Seller ---- */}
      <div className="card">
        <div className="card-title">{t("account.seller")}</div>
        <div style={{ display: "flex", alignItems: "center", gap: "var(--space-sm)", marginTop: "var(--space-sm)" }}>
          <span className={`status-dot ${seller.connected ? "status-dot-connected" : seller.running ? "status-dot-connected" : "status-dot-disconnected"}`}
            style={seller.running && !seller.connected ? { background: "#f5a623" } : undefined} />
          <span style={{ fontSize: 14, fontWeight: 500 }}>
            {seller.connected ? t("common.running") : seller.running ? t("common.reconnecting") : t("common.stopped")}
          </span>
        </div>
        {seller.streams.length > 0 && (
          <div className="font-mono" style={{ fontSize: 12, color: "#22c55e", marginTop: "var(--space-xs)" }}>
            {seller.streams.length === 1
              ? t("account.streamsOne", { count: seller.streams.length })
              : t("account.streamsOther", { count: seller.streams.length })}
          </div>
        )}
        {seller.error && (
          <div className="alert alert-error" style={{ marginTop: "var(--space-sm)" }}>{seller.error}</div>
        )}
        <div style={{ marginTop: "var(--space-sm)" }}>
          <button className="btn btn-secondary btn-sm" onClick={() => navigate("/seller")}>
            {t("account.sellerSettings")}
          </button>
        </div>
      </div>

      {/* ---- System Info ---- */}
      <div className="card">
        <div className="card-title">{t("account.system")}</div>
        <table style={{ marginTop: "var(--space-sm)" }}><tbody>
          <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{t("account.dataDir")}</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/</td></tr>
          <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{t("account.walletPath")}</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/wallet/keyfile.enc</td></tr>
          <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{t("account.sessionPath")}</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/session_token</td></tr>
          <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{t("account.configPath")}</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/config.toml</td></tr>
          <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{t("account.version")}</td><td className="font-mono" style={{ fontSize: 12 }}>v{appInfo.version} <span style={{ color: "var(--color-mute)" }}>({appInfo.git_hash})</span></td></tr>
          <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{t("common.support")}</td><td className="font-mono" style={{ fontSize: 12 }}>humanshere@proxybase.xyz</td></tr>
        </tbody></table>
      </div>

      {/* ---- Actions ---- */}
      <div className="card">
        <a href="https://discord.gg/7uedk7ajHD" target="_blank" rel="noopener noreferrer"
          className="btn btn-secondary btn-sm" style={{ textDecoration: "none", width: "100%" }}>
          {t("common.support")}
        </a>
        <button className="btn btn-secondary btn-sm" style={{ width: "100%", marginTop: "var(--space-xs)" }}
          onClick={handleCheckUpdate}
          disabled={updatePhase.kind === "checking" || updatePhase.kind === "downloading"}>
          {updatePhase.kind === "checking" ? t("account.checkingUpdate")
            : updatePhase.kind === "downloading"
              ? `${t("account.downloadingUpdate")}${updatePhase.progress !== null ? ` (${updatePhase.progress}%)` : ""}`
            : t("account.checkUpdate")}
        </button>
        {updatePhase.kind === "uptodate" && (
          <p className="text-muted" style={{ fontSize: 12, marginTop: "var(--space-xs)", textAlign: "center" }}>
            {t("account.upToDate")}
          </p>
        )}
        {updatePhase.kind === "error" && (
          <div className="alert alert-error" style={{ marginTop: "var(--space-xs)" }}>{t("account.updateError")}</div>
        )}
        <button className="btn btn-danger btn-sm" style={{ width: "100%", marginTop: "var(--space-xs)" }} onClick={handleLogout}>
          {t("account.logout")}
        </button>
        <p className="text-muted" style={{ fontSize: 12, marginTop: "var(--space-sm)", textAlign: "center" }}>
          {t("account.logoutWarning")}
        </p>
      </div>

      {/* ---- Update Modal ---- */}
      {updatePhase.kind === "available" && (
        <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setUpdatePhase({ kind: "idle" })}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="card-title">{t("account.checkUpdate")}</div>
            <p style={{ marginTop: "var(--space-sm)", fontSize: 14 }}>
              {t("account.updateAvailable", { version: updatePhase.version })}
            </p>
            <div style={{ display: "flex", gap: "var(--space-sm)", marginTop: "var(--space-md)" }}>
              <button className="btn btn-secondary btn-sm" style={{ flex: 1 }}
                onClick={() => setUpdatePhase({ kind: "idle" })}>
                {t("common.cancel")}
              </button>
              <button className="btn btn-success btn-sm" style={{ flex: 1 }} onClick={handleInstallUpdate}>
                {t("account.installUpdate")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---- Balance Modal ---- */}
      {showBalance && (
        <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setShowBalance(false)}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="card-title">{t("account.walletBalance")}</div>
            {balanceLoading ? (<p className="text-muted" style={{ marginTop: "var(--space-sm)" }}>{t("common.loading")}</p>)
            : balance ? (<div style={{ marginTop: "var(--space-sm)" }}>{renderBalanceRows(balance)}</div>)
            : (<p className="text-muted" style={{ marginTop: "var(--space-sm)" }}>{t("account.failedBalance")}</p>)}
            <button className="btn btn-secondary btn-sm" style={{ marginTop: "var(--space-lg)", width: "100%" }} onClick={() => setShowBalance(false)}>{t("common.close")}</button>
          </div>
        </div>
      )}

      {/* ---- Info Modal ---- */}
      {showInfo && (
        <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setShowInfo(false)}>
          <div style={{ background: "var(--color-canvas)", borderRadius: "var(--rounded-md)", padding: "var(--space-xl)", maxWidth: 380, width: "90%", boxShadow: "var(--shadow-card)" }}
            onClick={(e) => e.stopPropagation()}>
            <div className="card-title">{t("account.appInfo")}</div>
            <table style={{ marginTop: "var(--space-sm)" }}><tbody>
              <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{t("account.dataDir")}</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/</td></tr>
              <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{t("account.walletPath")}</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/wallet/keyfile.enc</td></tr>
              <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{t("account.sessionPath")}</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/session_token</td></tr>
              <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>{t("account.configPath")}</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/config.toml</td></tr>
            </tbody></table>
            <button className="btn btn-secondary btn-sm" style={{ marginTop: "var(--space-lg)", width: "100%" }} onClick={() => setShowInfo(false)}>{t("common.close")}</button>
          </div>
        </div>
      )}
    </div>
  );
}
