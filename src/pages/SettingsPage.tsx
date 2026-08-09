import { LANGUAGES, useI18n } from "../i18n";

export default function SettingsPage() {
  const { t, lang, language, setLanguage, setSystemLanguage, isSystemDefault } =
    useI18n();

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">{t("settings.title")}</h1>
        <p className="page-description">{t("settings.desc")}</p>
      </div>

      <div className="card">
        <div className="card-title">{t("settings.language")}</div>
        <p className="text-muted" style={{ fontSize: 13, lineHeight: 1.5 }}>
          {t("settings.languageDesc")}
        </p>

        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)", marginTop: "var(--space-md)" }}>
          <button
            className={isSystemDefault ? "btn btn-primary btn-sm" : "btn btn-secondary btn-sm"}
            style={{ justifyContent: "flex-start", textAlign: "start" }}
            onClick={setSystemLanguage}
          >
            <span style={{ fontWeight: 600 }}>{t("settings.followSystem")}</span>
            <span className="text-muted" style={{ fontSize: 11, marginLeft: 8 }}>
              {t("settings.systemLangLabel", { lang: language.name })}
            </span>
          </button>

          {LANGUAGES.map((l) => (
            <button
              key={l.code}
              className={lang === l.code && !isSystemDefault ? "btn btn-primary btn-sm" : "btn btn-secondary btn-sm"}
              style={{ justifyContent: "flex-start", textAlign: "start" }}
              onClick={() => setLanguage(l.code)}
            >
              <span style={{ fontWeight: 600 }}>{l.name}</span>
              <span className="text-muted" style={{ fontSize: 11, marginLeft: 8 }}>
                {l.code}
              </span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
