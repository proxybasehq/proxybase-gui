import { useState, useEffect } from "react";
import { useOutletContext, Navigate } from "react-router-dom";
import { listPricing, createSession, closeSession, listSessions, getToken, bridgeStart, bridgeStop } from "../api";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import { formatUsdPerGb, PROXY_ADDRESS } from "../utils";
import { CountryFlag } from "../components/CountryFlag";

export default function MarketPage() {
  const { backendUrl } = useBackend();
  const { isAuthenticated, openDeposit } = useOutletContext<AppContext>();
  if (!isAuthenticated) return <Navigate to="/login" replace />;

  const [activeTab, setActiveTab] = useState<"prices" | "sessions">("prices");
  const [error, setError] = useState("");
  const [insufficientFunds, setInsufficientFunds] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);

  async function copyToClipboard(value: string, label: string) {
    try {
      await navigator.clipboard.writeText(value);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = value;
      ta.style.position = "fixed"; ta.style.opacity = "0";
      document.body.appendChild(ta); ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
    }
    setCopied(label);
    setTimeout(() => setCopied(null), 1500);
  }

  function copyTr({ label, value, full }: { label: string; value: string; full?: string }) {
    const display = full ?? value;
    return (
      <tr onClick={() => copyToClipboard(full ?? value, label)} style={{ cursor: "pointer" }}>
        <td style={{ color: "var(--color-mute)", fontSize: 12, padding: "4px 12px 4px 0", whiteSpace: "nowrap" }}>{label}</td>
        <td className="font-mono" style={{ fontSize: 12, wordBreak: "break-all" }}>
          {display}
          {copied === label && <span style={{ color: "#22c55e", marginLeft: 4, fontSize: 10 }}>Copied!</span>}
        </td>
      </tr>
    );
  }

  const [allPricing, setAllPricing] = useState<Array<Record<string, unknown>>>([]);
  const [pricesLoading, setPricesLoading] = useState(false);
  const [priceBuyLoading, setPriceBuyLoading] = useState<string | null>(null); // country+network_type key

  const [sessions, setSessions] = useState<Array<Record<string, unknown>>>([]);
  const [closingId, setClosingId] = useState<string | null>(null);
  const [connectModal, setConnectModal] = useState<Record<string, unknown> | null>(null);
  const [connectTab, setConnectTab] = useState<"remote" | "local">("remote");
  const [bridgePorts, setBridgePorts] = useState<Record<string, number>>({});
  const [token, setToken] = useState("");

  async function fetchPrices() {
    setError("");
    setPricesLoading(true);
    fetchSessions(); // refresh sessions in background
    try {
      const r = await listPricing(backendUrl);
      setAllPricing(((r as any).pricing || []));
    } catch (e) { setError(String(e)); }
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
      await bridgeStop(sessionId);
      setBridgePorts((prev) => { const next = { ...prev }; delete next[sessionId]; return next; });
      await fetchSessions();
    } catch (e) { setError(String(e)); }
    setClosingId(null);
  }

  async function handleBuyFromPrice(country: string, networkType: string) {
    setError("");
    setInsufficientFunds(false);
    const key = `${country}:${networkType}`;
    setPriceBuyLoading(key);
    try {
      const session = await createSession(backendUrl, country, networkType, "rotating", null);
      const sid = (session as any).session_id;
      if (sid && token) {
        try {
          const port = await bridgeStart(sid, PROXY_ADDRESS, sid, token);
          setBridgePorts((prev) => ({ ...prev, [sid]: port }));
        } catch (_) { /* bridge start is best-effort */ }
      }
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
    // Always refresh sessions to keep the list accurate
    fetchSessions();
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
          marginBottom: "var(--space-sm)",
        }}>
          <div style={{ fontSize: 28, marginBottom: "var(--space-xs)", lineHeight: 1 }}>{'⚠'}</div>
          <div style={{ fontSize: 15, fontWeight: 600, color: "var(--color-ink)", marginBottom: 4 }}>
            Insufficient Balance
          </div>
          <p style={{ fontSize: 13, color: "var(--color-body)", margin: "0 0 var(--space-sm) 0", lineHeight: 1.5 }}>
            You don't have enough funds. Deposit crypto to continue.
          </p>
          <div style={{ display: "flex", gap: "var(--space-sm)", justifyContent: "center" }}>
            <button className="btn btn-success btn-sm" onClick={() => { setInsufficientFunds(false); openDeposit(); }}>
              Deposit Funds
            </button>
            <button className="btn btn-secondary btn-sm" onClick={() => setInsufficientFunds(false)}>
              Dismiss
            </button>
          </div>
        </div>
      )}

      {/* ---- Connect Modal ---- */}
      {connectModal && (
        <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setConnectModal(null)}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="card-title">Proxy Connection Details</div>

            <div className="tabs" style={{ marginTop: "var(--space-sm)" }}>
              <button className={`tab ${connectTab === "remote" ? "active" : ""}`} onClick={() => setConnectTab("remote")}>
                Remote
              </button>
              <button className={`tab ${connectTab === "local" ? "active" : ""}`} onClick={() => setConnectTab("local")}>
                Local Bridge
              </button>
            </div>

            {connectTab === "remote" && (
              <>
                <table style={{ marginTop: "var(--space-sm)" }}>
                  <tbody>
                    {copyTr({ label: "Proxy Address", value: PROXY_ADDRESS })}
                    {copyTr({ label: "Username", value: (connectModal as any).session_id })}
                    {copyTr({ label: "Session ID", value: (connectModal as any).session_id })}
                    {copyTr({ label: "Password", value: token.slice(0, 20) + "...", full: token })}
                    {copyTr({ label: "Country", value: (connectModal as any).country })}
                    {copyTr({ label: "Type", value: (connectModal as any).network_type || (connectModal as any).proxy_category })}
                  </tbody>
                </table>
                <div className="form-label" style={{ marginTop: "var(--space-md)" }}>Example (curl)</div>
                <pre className="json-view" style={{ fontSize: 11, cursor: "pointer" }}
                  onClick={() => copyToClipboard(`curl --socks5 ${PROXY_ADDRESS} --proxy-user ${(connectModal as any).session_id}:${token} http://api.proxybase.xyz/v2/ip`, "Example")}>
                  curl --socks5 {PROXY_ADDRESS} \<br/>  --proxy-user {(connectModal as any).session_id}:{token} \<br/>  http://api.proxybase.xyz/v2/ip
                  {copied === "Example" && <span style={{ color: "#22c55e", marginLeft: 6, fontSize: 10 }}>Copied!</span>}
                </pre>
              </>
            )}

            {connectTab === "local" && (
              <>
                <p style={{ fontSize: 12, color: "var(--color-body)", marginTop: "var(--space-sm)" }}>
                  Use the local bridge for apps like Chrome that don't support authenticated proxies.
                </p>
                <table style={{ marginTop: "var(--space-sm)" }}>
                  <tbody>
                    {copyTr({ label: "Proxy Address", value: "127.0.0.1:" + (bridgePorts[(connectModal as any).session_id] || "?") })}
                    {copyTr({ label: "Auth", value: "None required" })}
                  </tbody>
                </table>
                {bridgePorts[(connectModal as any).session_id] ? (
                  <>
                    <div className="form-label" style={{ marginTop: "var(--space-md)" }}>Example (curl • local)</div>
                    <pre className="json-view" style={{ fontSize: 11, cursor: "pointer" }}
                      onClick={() => copyToClipboard(`curl --socks5 127.0.0.1:${bridgePorts[(connectModal as any).session_id]} http://api.proxybase.xyz/v2/ip`, "Example (local)")}>
                      curl --socks5 127.0.0.1:{bridgePorts[(connectModal as any).session_id]} http://api.proxybase.xyz/v2/ip
                      {copied === "Example (local)" && <span style={{ color: "#22c55e", marginLeft: 6, fontSize: 10 }}>Copied!</span>}
                    </pre>
                  </>
                ) : (
                  <p className="text-muted" style={{ fontSize: 11, marginTop: "var(--space-md)" }}>
                    Bridge not running. The session may have been bought from another device.
                  </p>
                )}
              </>
            )}

            <button className="btn btn-secondary btn-sm" style={{ marginTop: "var(--space-md)", width: "100%" }} onClick={() => setConnectModal(null)}>Close</button>
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
                <thead><tr><th>Country</th><th>Category</th><th>Price</th><th style={{ width: 80 }}></th></tr></thead>
                <tbody>
                  {availablePrices.map((p, i) => {
                    const c = (p as any).country as string;
                    const nt = (p as any).network_type as string;
                    const key = `${c}:${nt}`;
                    const loading = priceBuyLoading === key;
                    return (
                    <tr key={i}>
                      <td style={{ whiteSpace: "nowrap" }}>
                        <CountryFlag code={c} />
                        <span style={{ fontSize: 12 }}>{c}</span>
                      </td>
                      <td><span className="badge">{nt}</span></td>
                      <td className="font-mono">{formatUsdPerGb((p as any).buyer_price_microcredits_per_gb)}</td>
                      <td>
                        <button
                          className="btn btn-success"
                          style={{ padding: "0 12px", height: 26, fontSize: 12, fontWeight: 600, width: "100%" }}
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
                      <td style={{ whiteSpace: "nowrap" }}>
                        <CountryFlag code={(s as any).country} />
                        <span style={{ fontSize: 12 }}>{(s as any).country}</span>
                      </td>
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
