import { useState, useEffect } from "react";
import { useOutletContext, Navigate } from "react-router-dom";
import { createDeposit, getDeposit, listCurrencies } from "../api";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import JsonView from "../components/JsonView";
import { usdToMc } from "../utils";
import { useI18n } from "../i18n";

export default function BuyerPage() {
  const { t } = useI18n();
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
      if (isNaN(amount) || amount <= 0) { setDepError(t("deposit.invalidAmount")); setDepLoading(false); return; }
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
        <h1 className="page-title">{t("deposit.title")}</h1>
        <p className="page-description">{t("deposit.desc")}</p>
      </div>

      {/* Create Deposit */}
      <div className="card">
        <div className="card-title">{t("deposit.create")}</div>
        {depError && <div className="alert alert-error">{depError}</div>}
        <div className="form-row">
          <div className="form-group">
            <label className="form-label">{t("deposit.amountUsd")}</label>
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
            <label className="form-label">{t("deposit.currency")}</label>
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
              {depLoading ? t("deposit.creating") : t("deposit.createAction")}
            </button>
          </div>
        </div>
        {depResult && <JsonView data={depResult} />}
      </div>

      {/* Deposit Status */}
      <div className="card">
        <div className="card-title">{t("deposit.checkStatus")}</div>
        {depStatusError && <div className="alert alert-error">{depStatusError}</div>}
        <div className="form-row">
          <div className="form-group">
            <label className="form-label">{t("deposit.depositId")}</label>
            <input
              className="form-input"
              value={depId}
              onChange={(e) => setDepId(e.target.value)}
              placeholder={t("deposit.enterDepositId")}
            />
          </div>
          <div className="form-group form-group-btn">
            <label className="form-label">&nbsp;</label>
            <button className="btn btn-secondary" onClick={handleDepositStatus} disabled={!depId}>
              {t("deposit.checkStatusAction")}
            </button>
          </div>
        </div>
        {depStatus && <JsonView data={depStatus} />}
      </div>

    </div>
  );
}
