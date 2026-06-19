import { NavLink } from "react-router-dom";

interface BottomNavProps {
  authenticated: boolean;
  walletLoaded: boolean;
}

export default function BottomNav({ authenticated, walletLoaded }: BottomNavProps) {
  const tabs: Array<{ to: string; label: string; icon: React.FC }> = [];

  // Login only available when wallet exists but not authenticated
  if (!authenticated && walletLoaded) {
    tabs.push({ to: "/login", label: "Login", icon: KeyIcon });
  }

  // Gated tabs — only visible when authenticated
  if (authenticated) {
    tabs.push(
      { to: "/market", label: "Market", icon: GlobeIcon },
      { to: "/seller", label: "Seller", icon: RadioIcon },
      { to: "/faq", label: "FAQ", icon: FaqIcon },
    );
  }

  return (
    <nav className="bottom-nav">
      {tabs.map(({ to, label, icon: Icon }) => (
        <NavLink key={to} to={to} className="bottom-nav-item">
          <Icon />
          <span className="bottom-nav-label">{label}</span>
        </NavLink>
      ))}
    </nav>
  );
}

// ---- Inline SVG icons ----

function KeyIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="7.5" cy="15.5" r="5.5" />
      <path d="m21 2-9.6 9.6" />
      <path d="m15.5 7.5 3 3" />
    </svg>
  );
}

function GlobeIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" />
      <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" />
      <path d="M2 12h20" />
    </svg>
  );
}

function RadioIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4.9 19.1C1 15.2 1 8.8 4.9 4.9" />
      <path d="M7.8 16.2c-2.3-2.3-2.3-6.1 0-8.5" />
      <circle cx="12" cy="12" r="2" />
      <path d="M16.2 7.8c2.3 2.3 2.3 6.1 0 8.5" />
      <path d="M19.1 4.9C23 8.8 23 15.1 19.1 19" />
    </svg>
  );
}

function FaqIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" />
      <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
      <line x1="12" y1="17" x2="12.01" y2="17" />
    </svg>
  );
}
