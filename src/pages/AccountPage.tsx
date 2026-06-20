import { useState } from "react";
import { useOutletContext, Navigate, useNavigate } from "react-router-dom";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import { getBalance } from "../api";
import { formatUsd } from "../utils";

export default function AccountPage() {
  const navigate = useNavigate();
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

  async function fetchBalance() {
    setShowBalance(true);
    setBalanceLoading(true);
    try { setBalance(await getBalance(backendUrl)); } catch (_) { setBalance(null); }
    setBalanceLoading(false);
  }

  function renderBalanceRows(data: Record<string, unknown>) {
    const mcFields: [string, string][] = [
      ["spendable_balance", "Spendable"], ["buyer_available", "Buyer Available"],
      ["buyer_reserved", "Buyer Reserved"], ["buyer_spent", "Buyer Spent"],
      ["seller_pending", "Seller Pending"], ["seller_available", "Seller Available"],
      ["seller_payout_locked", "Payout Locked"],
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
        <div className="card-title">Wallet</div>
        {walletLoaded ? (
          <>
            <div style={{ marginTop: "var(--space-sm)", marginBottom: "var(--space-md)" }}>
              <span className="text-muted" style={{ fontSize: 12 }}>Address</span>
              <div className="font-mono" style={{ fontSize: 13, wordBreak: "break-all", marginTop: 2 }}>
                {walletAddr}
              </div>
            </div>
            <div style={{ display: "flex", gap: "var(--space-sm)" }}>
              <button className="btn btn-secondary btn-sm" onClick={fetchBalance}>
                {balanceLoading ? "Loading..." : "View Balance"}
              </button>
              <button className="btn btn-secondary btn-sm" onClick={() => navigate("/wallet")}>
                Manage Wallet
              </button>
              <button className="btn btn-success btn-sm" onClick={openDeposit}>
                Add Funds
              </button>
            </div>
          </>
        ) : (
          <p className="text-muted" style={{ marginTop: "var(--space-sm)" }}>No wallet loaded.</p>
        )}
      </div>

      {/* ---- Seller ---- */}
      <div className="card">
        <div className="card-title">Seller</div>
        <div style={{ display: "flex", alignItems: "center", gap: "var(--space-sm)", marginTop: "var(--space-sm)" }}>
          <span className={`status-dot ${seller.connected ? "status-dot-connected" : seller.running ? "status-dot-connected" : "status-dot-disconnected"}`}
            style={seller.running && !seller.connected ? { background: "#f5a623" } : undefined} />
          <span style={{ fontSize: 14, fontWeight: 500 }}>
            {seller.connected ? "Running" : seller.running ? "Reconnecting..." : "Stopped"}
          </span>
        </div>
        {seller.streams.length > 0 && (
          <div className="font-mono" style={{ fontSize: 12, color: "#22c55e", marginTop: "var(--space-xs)" }}>
            {seller.streams.length} active stream{seller.streams.length !== 1 ? "s" : ""}
          </div>
        )}
        {seller.error && (
          <div className="alert alert-error" style={{ marginTop: "var(--space-sm)" }}>{seller.error}</div>
        )}
        <div style={{ marginTop: "var(--space-sm)" }}>
          <button className="btn btn-secondary btn-sm" onClick={() => navigate("/seller")}>
            Seller Settings
          </button>
        </div>
      </div>

      {/* ---- System Info ---- */}
      <div className="card">
        <div className="card-title">System</div>
        <table style={{ marginTop: "var(--space-sm)" }}><tbody>
          <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>Data dir</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/</td></tr>
          <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>Wallet</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/wallet/keyfile.enc</td></tr>
          <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>Session</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/session_token</td></tr>
          <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>Config</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/config.toml</td></tr>
        </tbody></table>
      </div>

      {/* ---- Actions ---- */}
      <div className="card">
        <button className="btn btn-danger btn-sm" style={{ width: "100%" }} onClick={handleLogout}>
          Logout
        </button>
        <p className="text-muted" style={{ fontSize: 12, marginTop: "var(--space-sm)", textAlign: "center" }}>
          All active sessions will be closed and the seller will be stopped.
        </p>
      </div>

      {/* ---- Balance Modal ---- */}
      {showBalance && (
        <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setShowBalance(false)}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="card-title">Wallet Balance</div>
            {balanceLoading ? (<p className="text-muted" style={{ marginTop: "var(--space-sm)" }}>Loading...</p>)
            : balance ? (<div style={{ marginTop: "var(--space-sm)" }}>{renderBalanceRows(balance)}</div>)
            : (<p className="text-muted" style={{ marginTop: "var(--space-sm)" }}>Failed to load balance.</p>)}
            <button className="btn btn-secondary btn-sm" style={{ marginTop: "var(--space-lg)", width: "100%" }} onClick={() => setShowBalance(false)}>Close</button>
          </div>
        </div>
      )}

      {/* ---- Info Modal ---- */}
      {showInfo && (
        <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setShowInfo(false)}>
          <div style={{ background: "var(--color-canvas)", borderRadius: "var(--rounded-md)", padding: "var(--space-xl)", maxWidth: 380, width: "90%", boxShadow: "var(--shadow-card)" }}
            onClick={(e) => e.stopPropagation()}>
            <div className="card-title">App Info</div>
            <table style={{ marginTop: "var(--space-sm)" }}><tbody>
              <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>Data dir</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/</td></tr>
              <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>Wallet</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/wallet/keyfile.enc</td></tr>
              <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>Session</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/session_token</td></tr>
              <tr><td style={{ color: "var(--color-mute)", fontSize: 13, padding: "4px 12px 4px 0" }}>Config</td><td className="font-mono" style={{ fontSize: 12 }}>~/.proxybase/config.toml</td></tr>
            </tbody></table>
            <button className="btn btn-secondary btn-sm" style={{ marginTop: "var(--space-lg)", width: "100%" }} onClick={() => setShowInfo(false)}>Close</button>
          </div>
        </div>
      )}
    </div>
  );
}
