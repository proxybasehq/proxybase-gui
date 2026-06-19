import { useState, useEffect } from "react";
import { useOutletContext, Navigate } from "react-router-dom";
import { createDeposit, getDeposit, listCurrencies } from "../api";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import JsonView from "../components/JsonView";
import { usdToMc } from "../utils";

export default function BuyerPage() {
  const { backendUrl } = useBackend();
  const { isAuthenticated } = useOutletContext<AppContext>();

  if (!isAuthenticated) return <Navigate to="/login" replace />;

  const [currencies, setCurrencies] = useState<string[]>(["usdcsol"]);
  const [depAmount, setDepAmount] = useState("");
  const [depCurrency, setDepCurrency] = useState("usdcsol");

  useEffect(() => {
    listCurrencies(backendUrl)
      .then((r) => {
        const arr = (r as any).currencies || [];
        if (arr.length > 0) {
          setCurrencies(arr);
          if (!arr.includes(depCurrency)) setDepCurrency(arr[0]);
        }
      })
      .catch(() => {});
  }, [backendUrl]);
  const [depResult, setDepResult] = useState<Record<string, unknown> | null>(null);
  const [depError, setDepError] = useState("");
  const [depLoading, setDepLoading] = useState(false);

  const [depId, setDepId] = useState("");
  const [depStatus, setDepStatus] = useState<Record<string, unknown> | null>(null);
  const [depStatusError, setDepStatusError] = useState("");

  async function handleCreateDeposit() {
    setDepError("");
    setDepLoading(true);
    try {
      const amount = parseFloat(depAmount);
      if (isNaN(amount) || amount <= 0) { setDepError("Invalid amount"); setDepLoading(false); return; }
      const r = await createDeposit(backendUrl, usdToMc(amount), depCurrency);
      setDepResult(r);
    } catch (e) {
      setDepError(String(e));
    }
    setDepLoading(false);
  }

  async function handleDepositStatus() {
    setDepStatusError("");
    try {
      const r = await getDeposit(backendUrl, depId);
      setDepStatus(r);
    } catch (e) {
      setDepStatusError(String(e));
    }
  }

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Deposits</h1>
        <p className="page-description">Create deposits and check their status.</p>
      </div>

      {/* Create Deposit */}
      <div className="card">
        <div className="card-title">Create Deposit</div>
        {depError && <div className="alert alert-error">{depError}</div>}
        <div className="form-row">
          <div className="form-group">
            <label className="form-label">Amount ($USD)</label>
            <input
              type="number"
              step="0.01"
              min="0.01"
              className="form-input"
              value={depAmount}
              onChange={(e) => setDepAmount(e.target.value)}
              placeholder="1.00"
            />
          </div>
          <div className="form-group">
            <label className="form-label">Currency</label>
            <select
              className="form-select"
              value={depCurrency}
              onChange={(e) => setDepCurrency(e.target.value)}
            >
              {currencies.map((c) => (
                <option key={c} value={c}>{c}</option>
              ))}
            </select>
          </div>
          <div className="form-group form-group-btn">
            <label className="form-label">&nbsp;</label>
            <button className="btn btn-primary" onClick={handleCreateDeposit} disabled={depLoading || !depAmount}>
              {depLoading ? "Creating..." : "Create"}
            </button>
          </div>
        </div>
        {depResult && <JsonView data={depResult} />}
      </div>

      {/* Deposit Status */}
      <div className="card">
        <div className="card-title">Check Deposit Status</div>
        {depStatusError && <div className="alert alert-error">{depStatusError}</div>}
        <div className="form-row">
          <div className="form-group">
            <label className="form-label">Deposit ID</label>
            <input
              className="form-input"
              value={depId}
              onChange={(e) => setDepId(e.target.value)}
              placeholder="Enter deposit ID..."
            />
          </div>
          <div className="form-group form-group-btn">
            <label className="form-label">&nbsp;</label>
            <button className="btn btn-secondary" onClick={handleDepositStatus} disabled={!depId}>
              Check Status
            </button>
          </div>
        </div>
        {depStatus && <JsonView data={depStatus} />}
      </div>

    </div>
  );
}
