// SOCKS5 proxy address — local in dev, remote in production
export const PROXY_ADDRESS = import.meta.env.PROD
  ? "api.proxybase.xyz:1082"
  : "127.0.0.1:1082";

// 1,000,000 microcredits = $1.00
const MC_PER_USD = 1_000_000;

export function mcToUsd(microcredits: number): string {
  return (microcredits / MC_PER_USD).toFixed(2);
}

export function usdToMc(usd: number): number {
  return Math.round(usd * MC_PER_USD);
}

/** Format as "$X.XX" from microcredits */
export function formatUsd(microcredits: number): string {
  return `$${mcToUsd(microcredits)}`;
}

/** Format as "$X.XX/GB" from microcredits per GB */
export function formatUsdPerGb(microcreditsPerGb: number): string {
  return `$${mcToUsd(microcreditsPerGb)}/GB`;
}

// ---- Country helpers ----

/** Common country names keyed by ISO 3166-1 alpha-2 code */
const COUNTRY_NAMES: Record<string, string> = {
  US: "United States", GB: "United Kingdom", DE: "Germany", FR: "France",
  JP: "Japan", KR: "South Korea", CN: "China", IN: "India",
  BR: "Brazil", CA: "Canada", AU: "Australia", NL: "Netherlands",
  SE: "Sweden", CH: "Switzerland", SG: "Singapore", HK: "Hong Kong",
  IT: "Italy", ES: "Spain", MX: "Mexico", RU: "Russia",
  PL: "Poland", TR: "Turkey", AR: "Argentina", CL: "Chile",
  CO: "Colombia", PE: "Peru", ZA: "South Africa", NG: "Nigeria",
  KE: "Kenya", EG: "Egypt", ID: "Indonesia", PH: "Philippines",
  VN: "Vietnam", TH: "Thailand", MY: "Malaysia", TW: "Taiwan",
  NZ: "New Zealand", IE: "Ireland", AT: "Austria", BE: "Belgium",
  DK: "Denmark", FI: "Finland", NO: "Norway", PT: "Portugal",
  CZ: "Czechia", RO: "Romania", UA: "Ukraine", AE: "United Arab Emirates",
  SA: "Saudi Arabia", IL: "Israel", PK: "Pakistan", BD: "Bangladesh",
};

export function countryName(code: string): string {
  return COUNTRY_NAMES[code.toUpperCase()] || code.toUpperCase();
}
