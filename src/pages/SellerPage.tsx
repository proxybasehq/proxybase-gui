import { useState } from "react";
import { useOutletContext, Navigate } from "react-router-dom";
import { sellerStatus } from "../api";
import type { UpstreamProxy } from "../api";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import JsonView from "../components/JsonView";

export default function SellerPage() {
  const { backendUrl } = useBackend();
  const { isAuthenticated, seller, startSeller, stopSeller } = useOutletContext<AppContext>();

  if (!isAuthenticated) return <Navigate to="/login" replace />;

  const [status, setStatus] = useState<Record<string, unknown> | null>(null);
  const [statusError, setStatusError] = useState("");
  const [statusLoading, setStatusLoading] = useState(false);

  // Page-local config (only relevant when setting up seller)
  const [upstreams, setUpstreams] = useState<UpstreamProxy[]>([]);
  const [includeDirect, setIncludeDirect] = useState(true);
  const [startError, setStartError] = useState("");

  async function handleStatus() {
    setStatusError("");
    setStatusLoading(true);
    try {
      const r = await sellerStatus(backendUrl);
      setStatus(r);
    } catch (e) {
      setStatusError(String(e));
    }
    setStatusLoading(false);
  }

  async function handleStart() {
    setStartError("");
    try {
      await startSeller(backendUrl, upstreams, includeDirect);
    } catch (e) {
      setStartError(String(e));
    }
  }

  async function handleStop() {
    try {
      await stopSeller();
    } catch (e) {
      setStartError(String(e));
    }
  }

  function addUpstream() {
    setUpstreams([...upstreams, { address: "", username: "", password: "" }]);
  }

  function removeUpstream(i: number) {
    setUpstreams(upstreams.filter((_, idx) => idx !== i));
  }

  function updateUpstream(i: number, field: keyof UpstreamProxy, value: string) {
    const updated = [...upstreams];
    updated[i] = { ...updated[i], [field]: value };
    setUpstreams(updated);
  }

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Seller</h1>
        <p className="page-description">Start selling your bandwidth on the ProxyBase marketplace.</p>
      </div>

      {/* Status card — reads from persistent background state */}
      <div className="card">
        <div className="flex justify-between items-center">
          <div className="card-title" style={{ marginBottom: 0 }}>Seller Status</div>
          <div className="flex gap-sm items-center">
            <span className={seller.connected ? "status-dot status-dot-connected" : "status-dot status-dot-disconnected"} />
            <span style={{ fontSize: 13, fontFamily: "var(--font-mono)" }}>
              {seller.connected ? "Running" : seller.running ? "Reconnecting..." : "Stopped"}
            </span>
          </div>
        </div>
        {statusError && <div className="alert alert-error mt-md">{statusError}</div>}
        {seller.error && <div className="alert alert-error mt-md">{seller.error}</div>}
        {startError && <div className="alert alert-error mt-md">{startError}</div>}
        <div className="flex gap-sm mt-md">
          <button className="btn btn-secondary btn-sm" onClick={handleStatus} disabled={statusLoading}>
            {statusLoading ? "Loading..." : "Refresh Status"}
          </button>
        </div>
        {status && <div className="mt-md"><JsonView data={status} /></div>}
      </div>

      {/* Controls */}
      <div className="card">
        <div className="card-title">Start / Stop Seller</div>

        <div className="form-group">
          <label className="form-label">
            <input
              type="checkbox"
              checked={includeDirect}
              onChange={(e) => setIncludeDirect(e.target.checked)}
              style={{ marginRight: 8 }}
            />
            Include direct (sell own bandwidth)
          </label>
        </div>

        <div className="form-label">Upstream Proxies (resell)</div>
        {upstreams.map((u, i) => (
          <div key={i} className="form-row mb-md" style={{ padding: "var(--space-sm)", border: "1px solid var(--color-hairline)", borderRadius: "var(--rounded-sm)" }}>
            <div className="form-group">
              <label className="form-label">Host:Port</label>
              <input className="form-input" value={u.address} onChange={(e) => updateUpstream(i, "address", e.target.value)} placeholder="proxy.example:1080" />
            </div>
            <div className="form-group">
              <label className="form-label">Username</label>
              <input className="form-input" value={u.username} onChange={(e) => updateUpstream(i, "username", e.target.value)} placeholder="user" />
            </div>
            <div className="form-group">
              <label className="form-label">Password</label>
              <input className="form-input" value={u.password} onChange={(e) => updateUpstream(i, "password", e.target.value)} placeholder="pass" />
            </div>
            <div className="form-group form-group-btn">
              <label className="form-label">&nbsp;</label>
              <button className="btn btn-danger btn-sm" onClick={() => removeUpstream(i)}>Remove</button>
            </div>
          </div>
        ))}
        <button className="btn btn-secondary btn-sm mb-md" onClick={addUpstream}>+ Add Upstream Proxy</button>

        <div className="flex gap-sm">
          {!seller.running ? (
            <button className="btn btn-primary" onClick={handleStart}>
              Start Seller
            </button>
          ) : (
            <button className="btn btn-danger" onClick={handleStop}>
              Stop Seller
            </button>
          )}
        </div>
      </div>

      {/* Stream Monitor — reads from persistent background state */}
      {seller.running && (
        <div className="card">
          <div className="card-title">Active Streams ({seller.streams.length})</div>
          {seller.streams.length === 0 ? (
            <p className="text-muted">No active streams. Waiting for connections...</p>
          ) : (
            <table>
              <thead>
                <tr>
                  <th>Session ID</th>
                  <th>Target</th>
                  <th>Route</th>
                </tr>
              </thead>
              <tbody>
                {seller.streams.map((s) => (
                  <tr key={s.session_id}>
                    <td className="font-mono" style={{ fontSize: 12 }}>{s.session_id.slice(0, 16)}...</td>
                    <td className="font-mono">{s.target_ip}:{s.target_port}</td>
                    <td>
                      <span className="badge">{s.route_index !== null ? `Proxy #${s.route_index}` : "Direct"}</span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}
    </div>
  );
}
