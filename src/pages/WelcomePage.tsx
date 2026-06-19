import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { walletCreate, walletImport, walletInfo, login } from "../api";
import type { CreateWalletResult } from "../api";
import { useBackend } from "../hooks/useBackend";
import PasswordInput from "../components/PasswordInput";

type Step =
  | "checking"
  | "no-wallet"
  | "create"
  | "create-done"
  | "import"
  | "logging-in";

export default function WelcomePage() {
  const { backendUrl } = useBackend();
  const navigate = useNavigate();
  const [step, setStep] = useState<Step>("checking");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const [password, setPassword] = useState("");
  const [importPhrase, setImportPhrase] = useState("");
  const [mnemonic, setMnemonic] = useState("");
  const [walletAddr, setWalletAddr] = useState("");

  useEffect(() => {
    walletInfo()
      .then((info) => {
        if (info.loaded) {
          setWalletAddr(info.address);
          setStep("logging-in");
          handleAutoLogin();
        } else {
          setStep("no-wallet");
        }
      })
      .catch(() => setStep("no-wallet"));
  }, []);

  async function handleAutoLogin() {
    try {
      await login(backendUrl, "");
      navigate("/market", { replace: true });
    } catch (e) {
      setStep("no-wallet");
      setError("Auto-login failed. Create or import a wallet.");
    }
  }

  async function handleCreate() {
    setError("");
    setLoading(true);
    try {
      const r: CreateWalletResult = await walletCreate(password);
      setMnemonic(r.mnemonic);
      setWalletAddr(r.address);
      setStep("create-done");
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }

  async function handleImport() {
    setError("");
    if (!importPhrase.trim()) { setError("Enter your mnemonic phrase"); return; }
    setLoading(true);
    try {
      const r = await walletImport(importPhrase, password);
      setWalletAddr(r.address);
      // Auto-login after import
      await login(backendUrl, password || "");
      navigate("/market", { replace: true });
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }

  async function handleContinueAfterCreate() {
    setError("");
    setLoading(true);
    try {
      await login(backendUrl, password || "");
      navigate("/market", { replace: true });
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }

  // ---- Checking ----
  if (step === "checking") {
    return (
      <div className="welcome-screen">
        <div className="welcome-bg" />
        <div className="welcome-card" style={{ textAlign: "center" }}>
          <img src="/logo.svg" alt="" className="welcome-logo" />
          <div className="welcome-loader" />
          <p className="text-muted" style={{ marginTop: "var(--space-md)" }}>Loading...</p>
        </div>
      </div>
    );
  }

  // ---- Logging in ----
  if (step === "logging-in") {
    return (
      <div className="welcome-screen">
        <div className="welcome-bg" />
        <div className="welcome-card" style={{ textAlign: "center" }}>
          <img src="/logo.svg" alt="" className="welcome-logo" />
          <div className="welcome-loader" />
          <p style={{ marginTop: "var(--space-md)", fontWeight: 500 }}>Signing in...</p>
          <p className="text-muted" style={{ fontSize: 12 }}>{walletAddr.slice(0, 10)}...{walletAddr.slice(-6)}</p>
        </div>
      </div>
    );
  }

  // ---- No wallet ----
  if (step === "no-wallet") {
    return (
      <div className="welcome-screen">
        <div className="welcome-bg" />
        <div className="welcome-card">
          <img src="/logo.svg" alt="" className="welcome-logo" />
          <h1 className="welcome-title">Welcome to ProxyBase</h1>
          <p className="welcome-sub">
            A decentralized peer-to-peer bandwidth marketplace. Buy and sell proxy access using cryptocurrency deposits.
          </p>
          {error && <div className="alert alert-error" style={{ marginTop: "var(--space-md)" }}>{error}</div>}
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-sm)", marginTop: "var(--space-xl)" }}>
            <button className="btn btn-primary btn-lg" onClick={() => setStep("create")}>
              Create New Wallet
            </button>
            <button className="btn btn-secondary btn-lg" onClick={() => setStep("import")}>
              Import Existing Wallet
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ---- Create wallet ----
  if (step === "create") {
    return (
      <div className="welcome-screen">
        <div className="welcome-bg" />
        <div className="welcome-card">
          <img src="/logo.svg" alt="" className="welcome-logo" />
          <h1 className="welcome-title">Create Wallet</h1>
          <p className="welcome-sub">Generate a new BIP-39 wallet. Save your mnemonic securely.</p>
          {error && <div className="alert alert-error">{error}</div>}
          <PasswordInput
            label="Encryption Password (optional)"
            value={password}
            onChange={setPassword}
            placeholder="Leave empty for no password"
          />
          <div style={{ display: "flex", gap: "var(--space-sm)", marginTop: "var(--space-lg)" }}>
            <button className="btn btn-primary btn-lg" style={{ flex: 1 }} onClick={handleCreate} disabled={loading}>
              {loading ? "Creating..." : "Create"}
            </button>
            <button className="btn btn-secondary btn-lg" onClick={() => { setStep("no-wallet"); setError(""); }}>
              Back
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ---- Create done ----
  if (step === "create-done") {
    const words = mnemonic.split(" ");
    return (
      <div className="welcome-screen">
        <div className="welcome-bg" />
        <div className="welcome-card">
          <div className="badge badge-success" style={{ marginBottom: "var(--space-sm)" }}>Wallet Created</div>
          <h1 className="welcome-title" style={{ fontSize: 22 }}>Save Your Mnemonic</h1>
          <p className="welcome-sub">
            Write these 12 words down in order. Anyone with this phrase can access your wallet. Never share it.
          </p>
          {error && <div className="alert alert-error" style={{ marginTop: "var(--space-sm)" }}>{error}</div>}
          <div className="mnemonic-display" style={{ marginTop: "var(--space-md)" }}>
            {words.map((word, i) => (
              <div className="mnemonic-word" key={i}>
                <span className="mnemonic-word-index">{i + 1}.</span>
                <span>{word}</span>
              </div>
            ))}
          </div>
          <button className="btn btn-primary btn-lg" style={{ marginTop: "var(--space-xl)", width: "100%" }}
            onClick={handleContinueAfterCreate} disabled={loading}>
            {loading ? "Signing in..." : "Continue"}
          </button>
        </div>
      </div>
    );
  }

  // ---- Import wallet ----
  return (
    <div className="welcome-screen">
      <div className="welcome-bg" />
      <div className="welcome-card">
        <img src="/logo.svg" alt="" className="welcome-logo" />
        <h1 className="welcome-title">Import Wallet</h1>
        <p className="welcome-sub">Restore your wallet from a BIP-39 mnemonic phrase.</p>
        {error && <div className="alert alert-error">{error}</div>}
        <div className="form-group" style={{ marginTop: "var(--space-md)" }}>
          <label className="form-label">12-word Mnemonic Phrase</label>
          <textarea
            className="form-input"
            rows={3}
            value={importPhrase}
            onChange={(e) => setImportPhrase(e.target.value)}
            placeholder="Enter the 12 words separated by spaces..."
          />
        </div>
        <PasswordInput
          label="Encryption Password (optional)"
          value={password}
          onChange={setPassword}
          placeholder="Leave empty for no password"
        />
        <div style={{ display: "flex", gap: "var(--space-sm)", marginTop: "var(--space-lg)" }}>
          <button className="btn btn-primary btn-lg" style={{ flex: 1 }}
            onClick={handleImport} disabled={loading}>
            {loading ? "Importing..." : "Import & Login"}
          </button>
          <button className="btn btn-secondary btn-lg" onClick={() => { setStep("no-wallet"); setError(""); }}>
            Back
          </button>
        </div>
      </div>
    </div>
  );
}
