// Umami analytics tracking helpers

declare global {
  interface Window {
    umami?: {
      track: (event: string, data?: Record<string, unknown>) => void;
    };
  }
}

export function track(event: string, data?: Record<string, unknown>) {
  try {
    window.umami?.track(event, data);
  } catch {
    // never crash on analytics failures
  }
}

// ---------- Event names ----------

export const TrackEvent = {
  // Wallet
  WALLET_CREATE: "wallet_create",
  WALLET_IMPORT: "wallet_import",

  // Auth
  LOGIN: "login",
  LOGOUT: "logout",

  // Seller
  SELLER_START: "seller_start",
  SELLER_STOP: "seller_stop",

  // Market
  SESSION_CREATE: "session_create",
  SESSION_CLOSE: "session_close",

  // Deposit
  DEPOSIT_CREATE: "deposit_create",

  // Navigation
  PAGE_VIEW: "page_view",
} as const;
