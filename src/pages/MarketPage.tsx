import { useState, useEffect, useRef } from "react";
import { useOutletContext, Navigate } from "react-router-dom";
import { listPricing, createSession, closeSession, listSessions, keepaliveSession, getToken, bridgeStart, bridgeStop } from "../api";
import { load } from "@tauri-apps/plugin-store";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import { formatUsdPerGb, PROXY_ADDRESS } from "../utils";
import { CountryFlag } from "../components/CountryFlag";
import { track, TrackEvent } from "../tracking";
import { useI18n } from "../i18n";

export default function MarketPage() {
  const { t } = useI18n();
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
          {copied === label && <span style={{ color: "#22c55e", marginLeft: 4, fontSize: 10 }}>{t("common.copied")}</span>}
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

  async function saveBridgePorts(ports: Record<string, number>) {
    try {
      const store = await load("proxybase-settings.json");
      await store.set("bridge_ports", ports);
      await store.save();
    } catch (_) {}
  }

  async function loadBridgePorts(): Promise<Record<string, number>> {
    try {
      const store = await load("proxybase-settings.json");
      return await store.get<Record<string, number>>("bridge_ports") || {};
    } catch (_) { return {}; }
  }

  // ── Stable port registry: country:type → port ──
  const PORT_REGISTRY_KEY = "port_registry";
  const nextPortRef = useRef(10800);

  async function loadPortRegistry(): Promise<Record<string, number>> {
    try {
      const store = await load("proxybase-settings.json");
      const registry = await store.get<Record<string, number>>(PORT_REGISTRY_KEY) || {};
      // Find highest port to continue from
      for (const p of Object.values(registry)) { if (p >= nextPortRef.current) nextPortRef.current = p + 1; }
      return registry;
    } catch (_) { return {}; }
  }

  async function savePortRegistry(registry: Record<string, number>) {
    try {
      const store = await load("proxybase-settings.json");
      await store.set(PORT_REGISTRY_KEY, registry);
      await store.save();
    } catch (_) {}
  }

  // ── Session keepalive timer (every 5 minutes) ──
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const r = await listSessions(backendUrl);
        const active: Array<Record<string, unknown>> = (r as any).sessions || [];
        for (const s of active) {
          const sid = (s as any).session_id;
          if (sid) {
            keepaliveSession(backendUrl, sid as string).catch(() => {});
          }
        }
      } catch (_) {}
    }, 5 * 60 * 1000);
    return () => clearInterval(interval);
  }, [backendUrl]);

  async function fetchSessions(currentPorts?: Record<string, number>) {
    try {
      const r = await listSessions(backendUrl);
      const active: Array<Record<string, unknown>> = (r as any).sessions || [];
      setSessions(active);

      // Restart local bridges for sessions that lost state after app restart
      let t = token;
      if (!t) {
        t = await getToken().catch(() => "");
        if (t) setToken(t);
      }
      if (t) {
        const savedPorts = await loadBridgePorts();
        const activeIds = new Set(active.map((s) => (s as any).session_id).filter(Boolean));
        const portsSource = currentPorts || bridgePorts;
        const newPorts: Record<string, number> = {};

        // Load stable port registry for country:type → port mapping
        const portRegistry = await loadPortRegistry();

        // Stop bridges for expired sessions BEFORE starting new ones
        for (const sid of Object.keys(savedPorts)) {
          if (!activeIds.has(sid)) {
            await bridgeStop(sid).catch(() => {});
          }
        }

        for (const sid of activeIds) {
          if (!portsSource[sid as string]) {
            try {
              // Prefer stable port from registry (country:type), fall back to session-specific
              const session = active.find((s) => (s as any).session_id === sid);
              const countryType = session ? `${(session as any).country}:${(session as any).network_type}` : null;
              const preferred = (countryType ? portRegistry[countryType] : undefined)
                || savedPorts[sid as string]
                || undefined;
              const port = await bridgeStart(sid as string, PROXY_ADDRESS, sid as string, t, preferred);
              newPorts[sid as string] = port;
              // Update port registry only if we got the expected stable port
              if (countryType && port === preferred) {
                portRegistry[countryType] = port;
              }
            } catch (_) { /* bridge start is best-effort */ }
          } else {
            newPorts[sid as string] = portsSource[sid as string];
          }
        }

        setBridgePorts(newPorts);
        await saveBridgePorts(newPorts);
        await savePortRegistry(portRegistry);
      }
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
      track(TrackEvent.SESSION_CLOSE, { sessionId });
      const nextPorts = { ...bridgePorts };
      delete nextPorts[sessionId];
      setBridgePorts(nextPorts);
      await saveBridgePorts(nextPorts);
      await fetchSessions(nextPorts);
    } catch (e) { setError(String(e)); }
    setClosingId(null);
  }

  async function handleBuyFromPrice(country: string, networkType: string) {
    setError("");
    setInsufficientFunds(false);
    const countryTypeKey = `${country}:${networkType}`;
    setPriceBuyLoading(countryTypeKey);
    try {
      const session = await createSession(backendUrl, country, networkType, "rotating", null);
      track(TrackEvent.SESSION_CREATE, { country, networkType });
      const sid = (session as any).session_id;
      if (sid && token) {
        try {
          // Stable port: reuse port from registry or allocate next
          const portRegistry = await loadPortRegistry();
          const preferredPort = portRegistry[countryTypeKey] || nextPortRef.current;
          if (!portRegistry[countryTypeKey]) {
            portRegistry[countryTypeKey] = preferredPort;
            nextPortRef.current += 1;
            await savePortRegistry(portRegistry);
          }
          const port = await bridgeStart(sid, PROXY_ADDRESS, sid, token, preferredPort);
          // Update registry only if we got the expected stable port
          if (port === preferredPort) {
            portRegistry[countryTypeKey] = port;
          }
          await savePortRegistry(portRegistry);
          const nextPorts = { ...bridgePorts, [sid]: port };
          setBridgePorts(nextPorts);
          await saveBridgePorts(nextPorts);
          await fetchSessions(nextPorts);
        } catch (_) {
          await fetchSessions();
        }
      } else {
        await fetchSessions();
      }
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

  // SSE Real-time Updates
  useEffect(() => {
    let es: EventSource | null = null;
    let active = true;
    let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
    // Exponential backoff (2s → 60s) with jitter. Previously this retried
    // every 2s forever on a stale token/network loss, hammering the backend
    // and draining energy.
    let backoffMs = 2000;
    const MAX_BACKOFF_MS = 60_000;
    let lastPriceFetch = 0;

    async function setupSse() {
      try {
        if (es) { es.close(); es = null; }
        const token = await getToken();
        if (!active) return;

        const url = `${backendUrl}/v2/events?token=${encodeURIComponent(token)}`;

        es = new EventSource(url);

        es.onopen = () => {
          backoffMs = 2000;
        };

        es.onmessage = (e) => {
          try {
            const evt = JSON.parse(e.data);
            if (evt.event === "PricingUpdate" || evt.event === "SellerPoolUpdate") {
              // Debounce: SSE can burst (seller churn, wake from sleep).
              const now = Date.now();
              if (now - lastPriceFetch > 10_000) {
                lastPriceFetch = now;
                fetchPrices();
              }
            }
            if (evt.event === "SessionUpdate") {
              fetchSessions();
            }
          } catch (_) { /* ignore parse errors */ }
        };

        es.onerror = () => {
          // Native EventSource reconnects with the same (stale) URL.
          // Instead, close it and reconnect with a fresh token.
          if (es) { es.close(); es = null; }
          if (active) {
            const delay = backoffMs + Math.floor(Math.random() * 500);
            backoffMs = Math.min(backoffMs * 2, MAX_BACKOFF_MS);
            reconnectTimeout = setTimeout(setupSse, delay);
          }
        };
      } catch (_) { /* retry */ }
    }

    setupSse();

    // Reconnect with a fresh token when the app wakes from sleep.
    const onResumed = () => { if (active) setupSse(); };
    window.addEventListener("app-resumed", onResumed);

    // Listen for token refresh — reconnect SSE with fresh token
    const unlistenPromises: Promise<() => void>[] = [];
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen<string>("auth:token-updated", () => {
        console.log("[SSE] Token refreshed, reconnecting...");
        setupSse();
      }).then(fn => unlistenPromises.push(Promise.resolve(fn)));
    });

    return () => {
      active = false;
      if (reconnectTimeout) clearTimeout(reconnectTimeout);
      if (es) es.close();
      window.removeEventListener("app-resumed", onResumed);
      unlistenPromises.forEach(p => p.then(fn => fn()));
    };
  }, [backendUrl]);

  // Auto-switch to Prices when all sessions are closed
  useEffect(() => {
    if (sessions.length === 0 && activeTab === "sessions") {
      setActiveTab("prices");
    }
  }, [sessions.length, activeTab]);

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">{t("market.title")}</h1>
        <p className="page-description">{t("market.desc")}</p>
      </div>

      <div className="tabs">
        <button className={`tab ${activeTab === "prices" ? "active" : ""}`} onClick={() => setActiveTab("prices")}>
          {t("market.prices")}
        </button>
        {sessions.length > 0 && (
          <button className={`tab ${activeTab === "sessions" ? "active" : ""}`} onClick={() => setActiveTab("sessions")}>
            {t("market.activeSessions")}
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
            {t("market.insufficientBalance")}
          </div>
          <p style={{ fontSize: 13, color: "var(--color-body)", margin: "0 0 var(--space-sm) 0", lineHeight: 1.5 }}>
            {t("market.insufficientDesc")}
          </p>
          <div style={{ display: "flex", gap: "var(--space-sm)", justifyContent: "center" }}>
            <button className="btn btn-success btn-sm" onClick={() => { setInsufficientFunds(false); openDeposit(); }}>
              {t("market.depositFunds")}
            </button>
            <button className="btn btn-secondary btn-sm" onClick={() => setInsufficientFunds(false)}>
              {t("market.dismiss")}
            </button>
          </div>
        </div>
      )}

      {/* ---- Connect Modal ---- */}
      {connectModal && (
        <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setConnectModal(null)}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="card-title">{t("market.connectionDetails")}</div>

            <div className="tabs" style={{ marginTop: "var(--space-sm)" }}>
              <button className={`tab ${connectTab === "remote" ? "active" : ""}`} onClick={() => setConnectTab("remote")}>
                {t("market.remote")}
              </button>
              <button className={`tab ${connectTab === "local" ? "active" : ""}`} onClick={() => setConnectTab("local")}>
                {t("market.localBridge")}
              </button>
            </div>

            {connectTab === "remote" && (
              <>
                <table style={{ marginTop: "var(--space-sm)" }}>
                  <tbody>
                    {copyTr({ label: t("market.proxyAddress"), value: PROXY_ADDRESS })}
                    {copyTr({ label: t("common.username"), value: (connectModal as any).session_id })}
                    {copyTr({ label: t("market.sessionId"), value: (connectModal as any).session_id })}
                    {copyTr({ label: t("market.password"), value: token.slice(0, 20) + "...", full: token })}
                    {copyTr({ label: t("market.country"), value: (connectModal as any).country })}
                    {copyTr({ label: t("market.type"), value: (connectModal as any).network_type || (connectModal as any).proxy_category })}
                  </tbody>
                </table>
                <div className="form-label" style={{ marginTop: "var(--space-md)" }}>{t("market.exampleCurl")}</div>
                <pre className="json-view" style={{ fontSize: 11, cursor: "pointer" }}
                  onClick={() => copyToClipboard(`${navigator.platform.includes("Win") ? "curl.exe" : "curl"} --socks5 ${PROXY_ADDRESS} --proxy-user ${(connectModal as any).session_id}:${token} http://api.proxybase.xyz/v2/ip`, "Example")}>
                  {navigator.platform.includes("Win") ? "curl.exe" : "curl"} --socks5 {PROXY_ADDRESS} \<br/>  --proxy-user {(connectModal as any).session_id}:{token} \<br/>  http://api.proxybase.xyz/v2/ip
                  {copied === "Example" && <span style={{ color: "#22c55e", marginLeft: 6, fontSize: 10 }}>{t("common.copied")}</span>}
                </pre>
              </>
            )}

            {connectTab === "local" && (
                <>
                <p style={{ fontSize: 12, color: "var(--color-body)", marginTop: "var(--space-sm)" }}>
                  {t("market.localBridgeDesc")}
                </p>
                <table style={{ marginTop: "var(--space-sm)" }}>
                  <tbody>
                    {copyTr({ label: t("market.proxyAddress"), value: "127.0.0.1:" + (bridgePorts[(connectModal as any).session_id] || "?") })}
                    {copyTr({ label: t("market.auth"), value: t("market.noneRequired") })}
                  </tbody>
                </table>
                {bridgePorts[(connectModal as any).session_id] ? (
                  <>
                    <div className="form-label" style={{ marginTop: "var(--space-md)" }}>{t("market.exampleCurlLocal")}</div>
                    <pre className="json-view" style={{ fontSize: 11, cursor: "pointer" }}
                      onClick={() => copyToClipboard(`${navigator.platform.includes("Win") ? "curl.exe" : "curl"} --socks5 127.0.0.1:${bridgePorts[(connectModal as any).session_id]} http://api.proxybase.xyz/v2/ip`, "Example (local)")}>
                      {navigator.platform.includes("Win") ? "curl.exe" : "curl"} --socks5 127.0.0.1:{bridgePorts[(connectModal as any).session_id]} http://api.proxybase.xyz/v2/ip
                      {copied === "Example (local)" && <span style={{ color: "#22c55e", marginLeft: 6, fontSize: 10 }}>{t("common.copied")}</span>}
                    </pre>
                  </>
                ) : (
                  <p className="text-muted" style={{ fontSize: 11, marginTop: "var(--space-md)" }}>
                    {t("market.bridgeNotRunning")}
                  </p>
                )}
              </>
            )}

            <button className="btn btn-secondary btn-sm" style={{ marginTop: "var(--space-md)", width: "100%" }} onClick={() => setConnectModal(null)}>{t("common.close")}</button>
          </div>
        </div>
      )}

      {/* ---- Prices Tab ---- */}
      {activeTab === "prices" && (
        <div className="card">
          <div className="flex justify-between items-center">
            <div className="card-title" style={{ marginBottom: 0 }}>{t("market.pricing")}</div>
            <button className="btn btn-sm btn-secondary" onClick={fetchPrices} disabled={pricesLoading}>{t("market.refresh")}</button>
          </div>
          {pricesLoading && allPricing.length === 0 ? (
            <div style={{ textAlign: "center", padding: "var(--space-xl) 0" }}>
              <div className="welcome-loader" />
              <p className="text-muted" style={{ marginTop: "var(--space-md)", fontSize: 13 }}>{t("market.loadingPricing")}</p>
            </div>
          ) : availablePrices.length > 0 ? (
            <div className="table-container" style={{ marginTop: "var(--space-sm)" }}>
              <table>
                <thead><tr><th>{t("market.country")}</th><th>{t("market.category")}</th><th>{t("market.price")}</th><th style={{ width: 80 }}></th></tr></thead>
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
                          {loading ? t("market.buying") : t("market.buy")}
                        </button>
                      </td>
                    </tr>
                  )})}
                </tbody>
              </table>
            </div>
          ) : (
            <p className="text-muted" style={{ marginTop: "var(--space-sm)" }}>{t("market.noSellers")}</p>
          )}
        </div>
      )}

      {/* ---- Active Sessions Tab ---- */}
      {activeTab === "sessions" && (
        <div className="card">
          <div className="flex justify-between items-center">
            <div className="card-title" style={{ marginBottom: 0 }}>{t("market.activeSessionsCount", { count: sessions.length })}</div>
            <button className="btn btn-sm btn-secondary" onClick={() => fetchSessions()}>{t("market.refresh")}</button>
          </div>
          {sessions.length === 0 ? (
            <p className="text-muted" style={{ marginTop: "var(--space-sm)" }}>{t("market.noActiveSessions")}</p>
          ) : (
            <div className="table-container" style={{ marginTop: "var(--space-sm)" }}>
              <table>
                <thead>
                  <tr>
                    <th>{t("market.country")}</th>
                    <th>{t("market.type")}</th>
                    <th>{t("market.mode")}</th>
                    <th>{t("common.status")}</th>
                    <th className="table-action" style={{ width: 40 }}></th>
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
                      <td className="table-action" onClick={(e) => e.stopPropagation()}>
                        <button className="btn btn-sm btn-danger" style={{ padding: "0 6px", height: 26, fontSize: 11 }}
                          onClick={() => handleClose((s as any).session_id)}
                          disabled={closingId === (s as any).session_id} title={t("market.closeSession")}>
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
