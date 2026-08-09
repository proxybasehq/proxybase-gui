import { useUpdater } from "../hooks/useUpdater";
import { useI18n } from "../i18n";

export default function UpdateBanner() {
  const { t } = useI18n();
  const {
    update,
    downloading,
    progress,
    readyToRestart,
    error,
    downloadAndInstall,
    restartApp,
  } = useUpdater();

  if (!update && !readyToRestart) return null;

  return (
    <div
      style={{
        background: "linear-gradient(90deg, #0070f3, #7928ca)",
        color: "#fff",
        padding: "6px 16px",
        fontSize: 12,
        fontWeight: 500,
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        flexShrink: 0,
      }}
    >
      <span>
        {readyToRestart
          ? t("update.ready", { version: update?.version ?? "" })
          : downloading
          ? t("update.downloading", { percent: progress ? progress.percentage : "" })
          : t("update.available", { version: update?.version ?? "" })}
      </span>
      {error && (
        <span style={{ fontSize: 11, opacity: 0.9 }} title={error}>
          {t("update.restartFailed")}
        </span>
      )}
      <span>
        {readyToRestart ? (
          <button
            className="btn btn-sm"
            style={{
              background: "rgba(255,255,255,0.2)",
              color: "#fff",
              border: "none",
              fontSize: 12,
              padding: "2px 10px",
              borderRadius: "var(--rounded-pill)",
              cursor: "pointer",
            }}
            onClick={() => { restartApp(); }}
          >
            {t("update.restart")}
          </button>
        ) : downloading ? null : (
          <button
            className="btn btn-sm"
            style={{
              background: "rgba(255,255,255,0.2)",
              color: "#fff",
              border: "none",
              fontSize: 12,
              padding: "2px 10px",
              borderRadius: "var(--rounded-pill)",
              cursor: "pointer",
            }}
            onClick={downloadAndInstall}
          >
            {t("update.update")}
          </button>
        )}
      </span>
    </div>
  );
}
