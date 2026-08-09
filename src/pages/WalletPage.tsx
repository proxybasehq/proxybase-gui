import { useState, useEffect } from "react";
import { walletCreate, walletImport, walletInfo, type WalletInfo, type CreateWalletResult } from "../api";
import PasswordInput from "../components/PasswordInput";
import { useI18n } from "../i18n";

export default function WalletPage() {
  const { t } = useI18n();
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
        <h1 className="page-title">{t("wallet.title")}</h1>
        <p className="page-description">{t("wallet.desc")}</p>
      </div>

      <div className="tabs">
        <button className={`tab ${activeTab === "info" ? "active" : ""}`} onClick={() => { setActiveTab("info"); handleInfo(); }}>
          {t("wallet.info")}
        </button>
        <button className={`tab ${activeTab === "create" ? "active" : ""}`} onClick={() => setActiveTab("create")}>
          {t("wallet.create")}
        </button>
        <button className={`tab ${activeTab === "import" ? "active" : ""}`} onClick={() => setActiveTab("import")}>
          {t("wallet.import")}
        </button>
      </div>

      {error && <div className="alert alert-error">{error}</div>}

      {activeTab === "info" && (
        <div className="card">
          <div className="card-title">{t("wallet.status")}</div>
          {result && "loaded" in result ? (
            result.loaded ? (
              <div className="flex flex-col gap-sm">
                <div>
                  <span className="text-muted">{t("wallet.addressLabel")}</span>{" "}
                  <code className="font-mono word-break">{result.address}</code>
                </div>
                <span className="badge badge-success">{t("wallet.loaded")}</span>
              </div>
            ) : (
              <p className="text-muted">{t("wallet.noWallet")}</p>
            )
          ) : (
            <p className="text-muted">{t("wallet.clickInfo")}</p>
          )}
        </div>
      )}

      {activeTab === "create" && (
        <div className="card">
          <div className="card-title">{t("wallet.createNew")}</div>
          <PasswordInput
            label={t("wallet.encryptionPassword")}
            value={password}
            onChange={setPassword}
            placeholder={t("wallet.passwordPlaceholder")}
          />
          <button className="btn btn-primary" onClick={handleCreate} disabled={loading}>
            {loading ? t("wallet.creating") : t("wallet.createWallet")}
          </button>
          {result && "mnemonic" in result && (
            <div className="mt-lg">
              <div className="form-label">{t("wallet.walletAddress")}</div>
              <code className="font-mono word-break">{result.address}</code>
              <div className="form-label mt-md">{t("wallet.mnemonic")}</div>
              <div className="mnemonic-display">
                {(result as CreateWalletResult).mnemonic.split(" ").map((word, i) => (
                  <div className="mnemonic-word" key={i}>
                    <span className="mnemonic-word-index">{i + 1}.</span>
                    <span>{word}</span>
                  </div>
                ))}
              </div>
              <p className="text-muted mt-md" style={{ fontSize: 12 }}>
                {t("wallet.mnemonicWarning")}
              </p>
            </div>
          )}
        </div>
      )}

      {activeTab === "import" && (
        <div className="card">
          <div className="card-title">{t("wallet.importFromMnemonic")}</div>
          <div className="form-group">
            <label className="form-label">{t("wallet.mnemonicPhrase")}</label>
            <textarea
              className="form-input"
              rows={3}
              value={importPhrase}
              onChange={(e) => setImportPhrase(e.target.value)}
              placeholder={t("wallet.mnemonicPlaceholder")}
            />
          </div>
          <PasswordInput
            label={t("wallet.encryptionPassword")}
            value={password}
            onChange={setPassword}
            placeholder={t("wallet.passwordPlaceholder")}
          />
          <button className="btn btn-primary" onClick={handleImport} disabled={loading || !importPhrase.trim()}>
            {loading ? t("wallet.importing") : t("wallet.importWallet")}
          </button>
          {result && "loaded" in result && result.loaded && (
            <div className="mt-md">
              <span className="badge badge-success">{t("wallet.imported")}</span>
              <code className="font-mono word-break" style={{ marginLeft: 8 }}>{result.address}</code>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
