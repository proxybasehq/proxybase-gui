import { useState, useEffect } from "react";
import { useOutletContext } from "react-router-dom";
import { login, walletInfo } from "../api";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import PasswordInput from "../components/PasswordInput";
import JsonView from "../components/JsonView";
import type { LoginResult } from "../api";
import { formatUsd } from "../utils";

export default function LoginPage() {
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
        <h1 className="page-title">Login</h1>
        <p className="page-description">Authenticate with your wallet to access the ProxyBase network.</p>
      </div>

      {error && <div className="alert alert-error">{error}</div>}

      {!walletLoaded ? (
        <div className="card">
          <div className="card-title">No Wallet Found</div>
          <p className="text-muted">Create or import a wallet first from the Wallet page.</p>
        </div>
      ) : (
        <div className="card">
          <div className="card-title">Authenticate</div>
          <div className="form-group">
            <label className="form-label">Wallet Address</label>
            <code className="font-mono word-break" style={{ fontSize: 13 }}>{walletAddr}</code>
          </div>
          <PasswordInput
            label="Wallet Password"
            value={password}
            onChange={setPassword}
            placeholder="Enter wallet encryption password (leave empty if none)"
          />
          <button className="btn btn-primary" onClick={handleLogin} disabled={loading}>
            {loading ? "Authenticating..." : "Login"}
          </button>
        </div>
      )}

      {result && (
        <div className="card mt-lg">
          <div className="card-title">Login Successful</div>
          <div className="flex flex-col gap-sm mb-md">
            <div><span className="text-muted">Role:</span> <span className="badge badge-success">{result.role}</span></div>
            <div><span className="text-muted">Buyer Available:</span> {formatUsd(result.buyer_available)}</div>
            <div><span className="text-muted">Spendable Balance:</span> {formatUsd(result.spendable_balance)}</div>
          </div>
          <JsonView data={result} />
        </div>
      )}
    </div>
  );
}
