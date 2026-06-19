interface StatusBarProps {
  backendUrl: string;
  authenticated: boolean;
}

export default function StatusBar({ backendUrl, authenticated }: StatusBarProps) {
  return (
    <footer className="app-statusbar">
      <span
        className={authenticated ? "status-dot status-dot-connected" : "status-dot status-dot-disconnected"}
      />
      <span>{authenticated ? "Authenticated" : "Not authenticated"}</span>
      <span style={{ marginLeft: "auto" }}>{backendUrl}</span>
    </footer>
  );
}
