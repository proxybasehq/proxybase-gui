import { useState, useEffect } from "react";
import { useOutletContext, useNavigate } from "react-router-dom";
import { login, walletInfo } from "../api";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import PasswordInput from "../components/PasswordInput";
import JsonView from "../components/JsonView";
import type { LoginResult } from "../api";
import { formatUsd } from "../utils";
import { useI18n } from "../i18n";

export default function LoginPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const { backendUrl } = useBackend();
  const { onLoginSuccess } = useOutletContext<AppContext>();
  const [password, setPassword] = useState("");
  const [result, setResult] = useState<LoginResult | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [walletAddr, setWalletAddr] = useState("");
  const [walletLoaded, setWalletLoaded] = useState(false);

  useEffect(() => {
    walletInfo().then((info) => {
      setWalletLoaded(info.loaded);
      setWalletAddr(info.address);
    }).catch(() => {});
  }, []);

  async function handleLogin() {
    setError("");
    setLoading(true);
    try {
      const r = await login(backendUrl, password);
      setResult(r);
      onLoginSuccess();
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">{t("login.title")}</h1>
        <p className="page-description">{t("login.desc")}</p>
      </div>

      {error && <div className="alert alert-error">{error}</div>}

      {!walletLoaded ? (
        <div className="card">
          <div className="card-title">{t("login.noWallet")}</div>
          <p className="text-muted">{t("login.noWalletDesc")}</p>
          <button className="btn btn-primary btn-sm" style={{ width: "100%", marginTop: "var(--space-sm)" }}
            onClick={() => navigate("/wallet")}>
            {t("login.goToWallet")}
          </button>
        </div>
      ) : (
        <div className="card">
          <div className="card-title">{t("login.authenticate")}</div>
          {walletAddr && (
            <div className="form-group">
              <label className="form-label">{t("login.walletAddress")}</label>
              <code className="font-mono word-break" style={{ fontSize: 13 }}>{walletAddr}</code>
            </div>
          )}
          <PasswordInput
            label={t("login.walletPassword")}
            value={password}
            onChange={setPassword}
            placeholder={t("login.passwordPlaceholder")}
          />
          <button className="btn btn-primary" onClick={handleLogin} disabled={loading}>
            {loading ? t("login.authenticating") : t("login.title")}
          </button>
        </div>
      )}

      {result && (
        <div className="card mt-lg">
          <div className="card-title">{t("login.successful")}</div>
          <div className="flex flex-col gap-sm mb-md">
            <div><span className="text-muted">{t("login.role")}</span> <span className="badge badge-success">{result.role}</span></div>
            <div><span className="text-muted">{t("login.buyerAvailable")}</span> {formatUsd(result.buyer_available)}</div>
            <div><span className="text-muted">{t("login.spendableBalance")}</span> {formatUsd(result.spendable_balance)}</div>
          </div>
          <JsonView data={result} />
        </div>
      )}
    </div>
  );
}
