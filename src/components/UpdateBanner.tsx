import { useUpdater } from "../hooks/useUpdater";

export default function UpdateBanner() {
  const {
    update,
    downloading,
    progress,
    readyToRestart,
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
          ? `Update v${update?.version} ready. Restart to apply.`
          : downloading
          ? `Downloading update${progress ? ` (${progress.percentage}%)` : ""}...`
          : `v${update?.version} available`}
      </span>
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
            onClick={restartApp}
          >
            Restart
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
            Update
          </button>
        )}
      </span>
    </div>
  );
}
