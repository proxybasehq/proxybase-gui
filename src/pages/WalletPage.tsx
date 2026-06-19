import { useState, useEffect } from "react";
import { walletCreate, walletImport, walletInfo, type WalletInfo, type CreateWalletResult } from "../api";
import PasswordInput from "../components/PasswordInput";

export default function WalletPage() {
  const [password, setPassword] = useState("");
  const [importPhrase, setImportPhrase] = useState("");
  const [result, setResult] = useState<WalletInfo | CreateWalletResult | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [activeTab, setActiveTab] = useState<"info" | "create" | "import">("info");

  // Auto-fetch wallet info on mount
  useEffect(() => {
    handleInfo();
  }, []);

  async function handleCreate() {
    setError("");
    setLoading(true);
    try {
      const r = await walletCreate(password);
      setResult(r);
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }

  async function handleImport() {
    setError("");
    setLoading(true);
    try {
      const r = await walletImport(importPhrase, password);
      setResult(r);
      setImportPhrase("");
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }

  async function handleInfo() {
    setError("");
    setLoading(true);
    try {
      const r = await walletInfo();
      setResult(r);
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Wallet</h1>
        <p className="page-description">Manage your ProxyBase wallet identity.</p>
      </div>

      <div className="tabs">
        <button className={`tab ${activeTab === "info" ? "active" : ""}`} onClick={() => { setActiveTab("info"); handleInfo(); }}>
          Info
        </button>
        <button className={`tab ${activeTab === "create" ? "active" : ""}`} onClick={() => setActiveTab("create")}>
          Create
        </button>
        <button className={`tab ${activeTab === "import" ? "active" : ""}`} onClick={() => setActiveTab("import")}>
          Import
        </button>
      </div>

      {error && <div className="alert alert-error">{error}</div>}

      {activeTab === "info" && (
        <div className="card">
          <div className="card-title">Wallet Status</div>
          {result && "loaded" in result ? (
            result.loaded ? (
              <div className="flex flex-col gap-sm">
                <div>
                  <span className="text-muted">Address:</span>{" "}
                  <code className="font-mono word-break">{result.address}</code>
                </div>
                <span className="badge badge-success">Loaded</span>
              </div>
            ) : (
              <p className="text-muted">No wallet found. Create or import one.</p>
            )
          ) : (
            <p className="text-muted">Click Info to check wallet status.</p>
          )}
        </div>
      )}

      {activeTab === "create" && (
        <div className="card">
          <div className="card-title">Create New Wallet</div>
          <PasswordInput
            label="Encryption Password (optional)"
            value={password}
            onChange={setPassword}
            placeholder="Leave empty for no password"
          />
          <button className="btn btn-primary" onClick={handleCreate} disabled={loading}>
            {loading ? "Creating..." : "Create Wallet"}
          </button>
          {result && "mnemonic" in result && (
            <div className="mt-lg">
              <div className="form-label">Wallet Address</div>
              <code className="font-mono word-break">{result.address}</code>
              <div className="form-label mt-md">Mnemonic — SAVE SECURELY</div>
              <div className="mnemonic-display">
                {(result as CreateWalletResult).mnemonic.split(" ").map((word, i) => (
                  <div className="mnemonic-word" key={i}>
                    <span className="mnemonic-word-index">{i + 1}.</span>
                    <span>{word}</span>
                  </div>
                ))}
              </div>
              <p className="text-muted mt-md" style={{ fontSize: 12 }}>
                Write these 12 words down in order. Anyone with this phrase can access your wallet.
              </p>
            </div>
          )}
        </div>
      )}

      {activeTab === "import" && (
        <div className="card">
          <div className="card-title">Import from Mnemonic</div>
          <div className="form-group">
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
          <button className="btn btn-primary" onClick={handleImport} disabled={loading || !importPhrase.trim()}>
            {loading ? "Importing..." : "Import Wallet"}
          </button>
          {result && "loaded" in result && result.loaded && (
            <div className="mt-md">
              <span className="badge badge-success">Imported</span>
              <code className="font-mono word-break" style={{ marginLeft: 8 }}>{result.address}</code>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
