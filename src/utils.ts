import { invoke } from "@tauri-apps/api/core";

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

let displayNames: Intl.DisplayNames | null = null;
const CODE_TO_NAME = new Map<string, string>();
const NAME_TO_CODE = new Map<string, string>();

// Seed common aliases and special cases
CODE_TO_NAME.set("WORLDWIDE", "WorldWide");
CODE_TO_NAME.set("UNKNOWN", "Unknown");
NAME_TO_CODE.set("worldwide", "WorldWide");
NAME_TO_CODE.set("unknown", "Unknown");

const IP_INTEL_ALIASES: Array<[string, string]> = [
  ["united states of america", "US"],
  ["usa", "US"],
  ["u.s.a.", "US"],
  ["venezuela, bolivarian republic of", "VE"],
  ["venezuela (bolivarian republic of)", "VE"],
  ["bolivarian republic of venezuela", "VE"],
  ["russian federation", "RU"],
  ["viet nam", "VN"],
  ["korea, republic of", "KR"],
  ["republic of korea", "KR"],
  ["south korea", "KR"],
  ["korea, democratic people's republic of", "KP"],
  ["north korea", "KP"],
  ["taiwan, province of china", "TW"],
  ["bolivia, plurinational state of", "BO"],
  ["plurinational state of bolivia", "BO"],
  ["iran, islamic republic of", "IR"],
  ["islamic republic of iran", "IR"],
  ["tanzania, united republic of", "TZ"],
  ["united republic of tanzania", "TZ"],
  ["moldova, republic of", "MD"],
  ["republic of moldova", "MD"],
  ["syrian arab republic", "SY"],
  ["united kingdom of great britain and northern ireland", "GB"],
  ["uk", "GB"],
  ["uae", "AE"],
  ["czech republic", "CZ"],
  ["lao people's democratic republic", "LA"],
  ["hong kong, special administrative region of china", "HK"],
  ["macao, special administrative region of china", "MO"],
  ["brunei darussalam", "BN"],
  ["congo, democratic republic of the", "CD"],
  ["congo, republic of the", "CG"],
  ["cote d'ivoire", "CI"],
  ["ivory coast", "CI"],
  ["falkland islands (malvinas)", "FK"],
  ["micronesia, federated states of", "FM"],
  ["palestine, state of", "PS"],
];

for (const [alias, code] of IP_INTEL_ALIASES) {
  NAME_TO_CODE.set(alias.toLowerCase(), code);
}

try {
  if (typeof Intl !== "undefined" && typeof (Intl as any).DisplayNames === "function") {
    displayNames = new Intl.DisplayNames(["en"], { type: "region" });
    const letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    for (const a of letters) {
      for (const b of letters) {
        const code = a + b;
        try {
          const name = displayNames.of(code);
          if (name && name !== code && !name.includes("Unknown Region")) {
            CODE_TO_NAME.set(code, name);
            NAME_TO_CODE.set(name.toLowerCase(), code);
          }
        } catch (_) {}
      }
    }
  }
} catch (_) {
  displayNames = null;
}

// Hydrate directly from libproxybase on startup
try {
  invoke<Array<[string, string]>>("get_all_countries")
    .then((list) => {
      if (Array.isArray(list)) {
        for (const [code, name] of list) {
          CODE_TO_NAME.set(code.toUpperCase(), name);
          NAME_TO_CODE.set(name.toLowerCase(), code.toUpperCase());
        }
      }
    })
    .catch(() => {});
} catch (_) {}

/**
 * Return full English display name for a country code or long name.
 * E.g. "US" -> "United States", "Venezuela" -> "Venezuela", "WorldWide" -> "WorldWide".
 */
export function countryName(codeOrName: string): string {
  if (!codeOrName || !codeOrName.trim()) return "";
  const trimmed = codeOrName.trim();
  const upper = trimmed.toUpperCase();
  if (upper === "WORLDWIDE") return "WorldWide";
  if (upper === "UNKNOWN") return "Unknown";

  const directCode = NAME_TO_CODE.get(trimmed.toLowerCase());
  if (directCode && CODE_TO_NAME.has(directCode)) {
    return CODE_TO_NAME.get(directCode)!;
  }

  if (/^[A-Z]{2}$/.test(upper)) {
    if (CODE_TO_NAME.has(upper)) return CODE_TO_NAME.get(upper)!;
    if (displayNames) {
      try {
        const name = displayNames.of(upper);
        if (name && name.toUpperCase() !== upper) return name;
      } catch (_) {}
    }
  }

  // Resolve prefix / composite match
  const resolvedCode = countryCode(trimmed);
  if (resolvedCode && resolvedCode !== upper && CODE_TO_NAME.has(resolvedCode)) {
    return CODE_TO_NAME.get(resolvedCode)!;
  }

  return trimmed;
}

/**
 * Return standard 2-letter ISO uppercase code (or "WorldWide" / "Unknown")
 * from either a country code or long country name.
 * E.g. "United States" -> "US", "us" -> "US", "Venezuela" -> "VE".
 */
export function countryCode(codeOrName: string): string {
  if (!codeOrName || !codeOrName.trim()) return "";
  const trimmed = codeOrName.trim();
  const upper = trimmed.toUpperCase();
  if (upper === "WORLDWIDE") return "WorldWide";
  if (upper === "UNKNOWN") return "Unknown";

  if (/^[A-Z]{2}$/.test(upper)) {
    return upper;
  }

  const code = NAME_TO_CODE.get(trimmed.toLowerCase());
  if (code) return code;

  // Prefix / composite matching for official variants (e.g. "Venezuela, Bolivarian...")
  const lower = trimmed.toLowerCase();
  for (const [c, name] of CODE_TO_NAME.entries()) {
    if (c.length === 2) {
      const nameLower = name.toLowerCase();
      if (lower.startsWith(nameLower)) {
        const rest = lower.slice(nameLower.length);
        if (rest.startsWith(",") || rest.startsWith(" (") || rest.startsWith(" of") || rest.startsWith(" -")) {
          return c;
        }
      }
    }
  }

  return upper;
}

/**
 * Return unicode flag emoji for a country code or long country name.
 * E.g. "US" or "United States" -> "🇺🇸", "VE" or "Venezuela" -> "🇻🇪", "WorldWide" -> "🌐", "Unknown" -> "❓".
 */
export function countryFlag(codeOrName: string): string {
  if (!codeOrName || !codeOrName.trim()) return "";
  const upper = countryCode(codeOrName).toUpperCase();
  if (upper === "WORLDWIDE") return "🌐";
  if (upper === "UNKNOWN") return "❓";
  if (/^[A-Z]{2}$/.test(upper)) {
    return String.fromCodePoint(
      127397 + upper.charCodeAt(0),
      127397 + upper.charCodeAt(1)
    );
  }
  return "";
}
