import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { walletCreate, walletImport, walletInfo, login } from "../api";
import type { CreateWalletResult } from "../api";
import { useBackend } from "../hooks/useBackend";
import PasswordInput from "../components/PasswordInput";
import { track, TrackEvent } from "../tracking";
import { useI18n } from "../i18n";

type Step =
  | "checking"
  | "no-wallet"
  | "create"
  | "create-done"
  | "import"
  | "logging-in";

export default function WelcomePage() {
  const { t } = useI18n();
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
      // A wallet exists but couldn't be auto-logged-in (password-protected
      // wallet, or backend unreachable). Send the user to the login page —
      // showing create/import here would offer to overwrite their wallet.
      navigate("/login", { replace: true });
    }
  }

  async function handleCreate() {
    setError("");
    setLoading(true);
    try {
      const r: CreateWalletResult = await walletCreate(password);
      track(TrackEvent.WALLET_CREATE);
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
    if (!importPhrase.trim()) { setError(t("welcome.enterMnemonic")); return; }
    setLoading(true);
    try {
      const r = await walletImport(importPhrase, password);
      track(TrackEvent.WALLET_IMPORT);
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
          <p className="text-muted" style={{ marginTop: "var(--space-md)" }}>{t("common.loading")}</p>
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
          <p style={{ marginTop: "var(--space-md)", fontWeight: 500 }}>{t("welcome.signingIn")}</p>
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
          <h1 className="welcome-title">{t("welcome.title")}</h1>
          <p className="welcome-sub">
            {t("welcome.subtitle")}
          </p>
          {error && <div className="alert alert-error" style={{ marginTop: "var(--space-md)" }}>{error}</div>}
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-sm)", marginTop: "var(--space-xl)" }}>
            <button className="btn btn-primary btn-lg" onClick={() => setStep("create")}>
              {t("welcome.createWallet")}
            </button>
            <button className="btn btn-secondary btn-lg" onClick={() => setStep("import")}>
              {t("welcome.importWallet")}
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
          <h1 className="welcome-title">{t("welcome.createTitle")}</h1>
          <p className="welcome-sub">{t("welcome.createDesc")}</p>
          {error && <div className="alert alert-error">{error}</div>}
          <PasswordInput
            label={t("welcome.encryptionPassword")}
            value={password}
            onChange={setPassword}
            placeholder={t("welcome.passwordPlaceholder")}
          />
          <div style={{ display: "flex", gap: "var(--space-sm)", marginTop: "var(--space-lg)" }}>
            <button className="btn btn-primary btn-lg" style={{ flex: 1 }} onClick={handleCreate} disabled={loading}>
              {loading ? t("common.creating") : t("common.create")}
            </button>
            <button className="btn btn-secondary btn-lg" onClick={() => { setStep("no-wallet"); setError(""); }}>
              {t("common.back")}
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
          <div className="badge badge-success" style={{ marginBottom: "var(--space-sm)" }}>{t("welcome.walletCreated")}</div>
          <h1 className="welcome-title" style={{ fontSize: 22 }}>{t("welcome.saveMnemonic")}</h1>
          <p className="welcome-sub">
            {t("welcome.saveMnemonicDesc")}
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
            {loading ? t("welcome.signingIn") : t("welcome.continue")}
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
        <h1 className="welcome-title">{t("welcome.importTitle")}</h1>
        <p className="welcome-sub">{t("welcome.importDesc")}</p>
        {error && <div className="alert alert-error">{error}</div>}
        <div className="form-group" style={{ marginTop: "var(--space-md)" }}>
          <label className="form-label">{t("welcome.mnemonicPhrase")}</label>
          <textarea
            className="form-input"
            rows={3}
            value={importPhrase}
            onChange={(e) => setImportPhrase(e.target.value)}
            placeholder={t("welcome.mnemonicPlaceholder")}
          />
        </div>
        <PasswordInput
          label={t("welcome.encryptionPassword")}
          value={password}
          onChange={setPassword}
          placeholder={t("welcome.passwordPlaceholder")}
        />
        <div style={{ display: "flex", gap: "var(--space-sm)", marginTop: "var(--space-lg)" }}>
          <button className="btn btn-primary btn-lg" style={{ flex: 1 }}
            onClick={handleImport} disabled={loading}>
            {loading ? t("welcome.importing") : t("welcome.importAndLogin")}
          </button>
          <button className="btn btn-secondary btn-lg" onClick={() => { setStep("no-wallet"); setError(""); }}>
            {t("common.back")}
          </button>
        </div>
      </div>
    </div>
  );
}
