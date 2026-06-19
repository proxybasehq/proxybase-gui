import { useState, useEffect } from "react";
import { useOutletContext, Navigate } from "react-router-dom";
import { listPricing, createSession, closeSession, listSessions, getToken } from "../api";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import { formatUsdPerGb, countryFlag, countryName } from "../utils";

export default function MarketPage() {
  const { backendUrl } = useBackend();
  const { isAuthenticated, openDeposit } = useOutletContext<AppContext>();
  if (!isAuthenticated) return <Navigate to="/login" replace />;

  const [activeTab, setActiveTab] = useState<"prices" | "sessions">("prices");
  const [error, setError] = useState("");
  const [insufficientFunds, setInsufficientFunds] = useState(false);

  const [allPricing, setAllPricing] = useState<Array<Record<string, unknown>>>([]);
  const [pricesLoading, setPricesLoading] = useState(false);
  const [priceBuyLoading, setPriceBuyLoading] = useState<string | null>(null); // country+network_type key

  const [sessions, setSessions] = useState<Array<Record<string, unknown>>>([]);
  const [closingId, setClosingId] = useState<string | null>(null);
  const [connectModal, setConnectModal] = useState<Record<string, unknown> | null>(null);
  const [token, setToken] = useState("");

  async function fetchPrices() {
    setError("");
    setPricesLoading(true);
    try { setAllPricing(((await listPricing(backendUrl)) as any).pricing || []); }
    catch (e) { setError(String(e)); }
    setPricesLoading(false);
  }

  async function fetchSessions() {
    try {
      const r = await listSessions(backendUrl);
      setSessions((r as any).sessions || []);
    } catch (_) { /* ignore */ }
  }

  async function fetchToken() {
    try { setToken(await getToken()); } catch (_) {}
  }

  async function handleClose(sessionId: string) {
    setClosingId(sessionId);
    try {
      await closeSession(backendUrl, sessionId);
      await fetchSessions(); // refresh from backend — only active sessions are returned
    } catch (e) { setError(String(e)); }
    setClosingId(null);
  }

  async function handleBuyFromPrice(country: string, networkType: string) {
    setError("");
    setInsufficientFunds(false);
    const key = `${country}:${networkType}`;
    setPriceBuyLoading(key);
    try {
      await createSession(backendUrl, country, networkType, "rotating", null);
      await fetchSessions();
      setActiveTab("sessions");
    } catch (e) {
      const msg = String(e).toLowerCase();
      if (msg.includes("insufficient") || msg.includes("balance") || msg.includes("funds")) {
        setInsufficientFunds(true);
      } else {
        setError(String(e));
      }
    }
    setPriceBuyLoading(null);
  }


  const availablePrices = allPricing.filter((p) => ((p as any).available_sellers ?? 0) > 0);

  useEffect(() => { fetchToken(); }, []);
  useEffect(() => {
    if (activeTab === "sessions") fetchSessions();
    if (activeTab === "prices") fetchPrices();
  }, [activeTab]);

  // Auto-switch to Prices when all sessions are closed
  useEffect(() => {
    if (sessions.length === 0 && activeTab === "sessions") {
      setActiveTab("prices");
    }
  }, [sessions.length, activeTab]);

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Market</h1>
        <p className="page-description">Browse countries, pricing, and manage proxy sessions.</p>
      </div>

      <div className="tabs">
        <button className={`tab ${activeTab === "prices" ? "active" : ""}`} onClick={() => setActiveTab("prices")}>
          Prices
        </button>
        {sessions.length > 0 && (
          <button className={`tab ${activeTab === "sessions" ? "active" : ""}`} onClick={() => setActiveTab("sessions")}>
            Active Sessions
          </button>
        )}
      </div>

      {error && <div className="alert alert-error">{error}</div>}

      {insufficientFunds && (
        <div className="card" style={{
          border: "1px solid #f5a623",
          background: "#fffbeb",
          textAlign: "center",
          marginBottom: "var(--space-md)",
        }}>
          <div style={{ fontSize: 40, marginBottom: "var(--space-sm)" }}>&#x26A0;</div>
          <div style={{ fontSize: 18, fontWeight: 600, color: "var(--color-ink)", marginBottom: "var(--space-xs)" }}>
            Insufficient Balance
          </div>
          <p style={{ fontSize: 14, color: "var(--color-body)", margin: "0 0 var(--space-lg) 0", lineHeight: 1.6 }}>
            You don't have enough funds to purchase this session.
            Deposit crypto into your wallet to continue.
          </p>
          <div style={{ display: "flex", gap: "var(--space-sm)", justifyContent: "center" }}>
            <button className="btn btn-success" onClick={() => { setInsufficientFunds(false); openDeposit(); }}>
              Deposit Funds
            </button>
            <button className="btn btn-secondary" onClick={() => setInsufficientFunds(false)}>
              Dismiss
            </button>
          </div>
        </div>
      )}

      {/* ---- Connect Modal ---- */}
      {connectModal && (
        <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setConnectModal(null)}>
          <div style={{ background: "var(--color-canvas)", borderRadius: "var(--rounded-md)", padding: "var(--space-xl)", maxWidth: 480, width: "90%", boxShadow: "var(--shadow-card)" }}
            onClick={(e) => e.stopPropagation()}>
            <div className="card-title">Proxy Connection Details</div>
            <table style={{ marginTop: "var(--space-sm)" }}>
              <tbody>
                <tr><td style={{ color: "var(--color-mute)", fontSize: 12, padding: "4px 12px 4px 0", whiteSpace: "nowrap" }}>Proxy Address</td><td className="font-mono" style={{ fontSize: 12 }}>127.0.0.1:1082</td></tr>
                <tr><td style={{ color: "var(--color-mute)", fontSize: 12, padding: "4px 12px 4px 0", whiteSpace: "nowrap" }}>Username</td><td className="font-mono" style={{ fontSize: 12, wordBreak: "break-all" }}>{(connectModal as any).session_id}</td></tr>
                <tr><td style={{ color: "var(--color-mute)", fontSize: 12, padding: "4px 12px 4px 0", whiteSpace: "nowrap" }}>Password</td><td className="font-mono" style={{ fontSize: 12 }}>{token.slice(0, 20)}...</td></tr>
                <tr><td style={{ color: "var(--color-mute)", fontSize: 12, padding: "4px 12px 4px 0", whiteSpace: "nowrap" }}>Country</td><td className="font-mono" style={{ fontSize: 12 }}>{(connectModal as any).country}</td></tr>
                <tr><td style={{ color: "var(--color-mute)", fontSize: 12, padding: "4px 12px 4px 0", whiteSpace: "nowrap" }}>Type</td><td className="font-mono" style={{ fontSize: 12 }}>{(connectModal as any).network_type || (connectModal as any).proxy_category}</td></tr>
              </tbody>
            </table>
            <div className="form-label" style={{ marginTop: "var(--space-md)" }}>Example (curl)</div>
            <pre className="json-view" style={{ fontSize: 11 }}>curl --socks5 127.0.0.1:1082 \<br/>  --proxy-user {(connectModal as any).session_id}:{token} \<br/>  https://httpbin.org/ip</pre>
            <button className="btn btn-secondary btn-sm" style={{ marginTop: "var(--space-lg)", width: "100%" }} onClick={() => setConnectModal(null)}>Close</button>
          </div>
        </div>
      )}

      {/* ---- Prices Tab ---- */}
      {activeTab === "prices" && (
        <div className="card">
          <div className="flex justify-between items-center">
            <div className="card-title" style={{ marginBottom: 0 }}>Pricing</div>
            <button className="btn btn-sm btn-secondary" onClick={fetchPrices} disabled={pricesLoading}>Refresh</button>
          </div>
          {availablePrices.length > 0 ? (
            <div className="table-container" style={{ marginTop: "var(--space-sm)" }}>
              <table>
                <thead><tr><th>Country</th><th>Category</th><th>Buyer Price</th><th>Seller Credit</th><th style={{ width: 120 }}></th></tr></thead>
                <tbody>
                  {availablePrices.map((p, i) => {
                    const c = (p as any).country as string;
                    const nt = (p as any).network_type as string;
                    const key = `${c}:${nt}`;
                    const loading = priceBuyLoading === key;
                    return (
                    <tr key={i}>
                      <td>{countryFlag(c)} {countryName(c)}</td>
                      <td><span className="badge">{nt}</span></td>
                      <td className="font-mono">{formatUsdPerGb((p as any).buyer_price_microcredits_per_gb)}</td>
                      <td className="font-mono">{formatUsdPerGb((p as any).seller_credit_microcredits_per_gb)}</td>
                      <td>
                        <button
                          className="btn btn-success"
                          style={{ padding: "0 16px", height: 36, fontSize: 14, fontWeight: 600, width: "100%" }}
                          onClick={() => handleBuyFromPrice(c, nt)}
                          disabled={loading}
                        >
                          {loading ? "Buying..." : "Buy"}
                        </button>
                      </td>
                    </tr>
                  )})}
                </tbody>
              </table>
            </div>
          ) : (
            <p className="text-muted" style={{ marginTop: "var(--space-sm)" }}>No sellers available.</p>
          )}
        </div>
      )}

      {/* ---- Active Sessions Tab ---- */}
      {activeTab === "sessions" && (
        <div className="card">
          <div className="flex justify-between items-center">
            <div className="card-title" style={{ marginBottom: 0 }}>Active Sessions ({sessions.length})</div>
            <button className="btn btn-sm btn-secondary" onClick={fetchSessions}>Refresh</button>
          </div>
          {sessions.length === 0 ? (
            <p className="text-muted" style={{ marginTop: "var(--space-sm)" }}>No active sessions. Buy one from the Prices tab.</p>
          ) : (
            <div className="table-container" style={{ marginTop: "var(--space-sm)" }}>
              <table>
                <thead>
                  <tr>
                    <th>Session ID</th>
                    <th>Country</th>
                    <th>Type</th>
                    <th>Mode</th>
                    <th>Status</th>
                    <th style={{ width: 40 }}></th>
                  </tr>
                </thead>
                <tbody>
                  {sessions.map((s) => (
                    <tr key={(s as any).session_id} style={{ cursor: "pointer" }} onClick={() => setConnectModal(s)}>
                      <td className="font-mono" style={{ fontSize: 11 }}>{(s as any).session_id?.slice(0, 16)}...</td>
                      <td>{countryFlag((s as any).country)} {(s as any).country}</td>
                      <td><span className="badge">{(s as any).network_type || (s as any).proxy_category || "-"}</span></td>
                      <td><span className="badge">{(s as any).session_type || "-"}</span></td>
                      <td><span className={`badge ${(s as any).status === "active" ? "badge-success" : ""}`}>{(s as any).status || "-"}</span></td>
                      <td onClick={(e) => e.stopPropagation()}>
                        <button className="btn btn-sm btn-danger" style={{ padding: "0 6px", height: 26, fontSize: 11 }}
                          onClick={() => handleClose((s as any).session_id)}
                          disabled={closingId === (s as any).session_id} title="Close session">
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                            <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
                          </svg>
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

    </div>
  );
}
