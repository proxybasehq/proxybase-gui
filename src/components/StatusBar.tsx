import { useI18n } from "../i18n";

interface StatusBarProps {
  backendUrl: string;
  authenticated: boolean;
}

export default function StatusBar({ backendUrl, authenticated }: StatusBarProps) {
  const { t } = useI18n();
  return (
    <footer className="app-statusbar">
      <span
        className={authenticated ? "status-dot status-dot-connected" : "status-dot status-dot-disconnected"}
      />
      <span>{authenticated ? t("status.authenticated") : t("status.notAuthenticated")}</span>
      <span style={{ marginLeft: "auto" }}>{backendUrl}</span>
    </footer>
  );
}
