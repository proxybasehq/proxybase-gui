export interface Language {
  /** Stable language code used as the translation dictionary key. */
  code: string;
  /** Language name in its own language, shown in the settings picker. */
  name: string;
  /** Text direction. */
  dir: "ltr" | "rtl";
  /** BCP-47 tags (lowercase) that map to this language, e.g. "en", "pt-BR". */
  tags: string[];
}

export const LANGUAGES: Language[] = [
  { code: "en", name: "English", dir: "ltr", tags: ["en"] },
  { code: "es", name: "Español", dir: "ltr", tags: ["es"] },
  { code: "fr", name: "Français", dir: "ltr", tags: ["fr"] },
  { code: "de", name: "Deutsch", dir: "ltr", tags: ["de"] },
  { code: "it", name: "Italiano", dir: "ltr", tags: ["it"] },
  { code: "pt", name: "Português", dir: "ltr", tags: ["pt"] },
  { code: "ru", name: "Русский", dir: "ltr", tags: ["ru"] },
  { code: "zh", name: "简体中文", dir: "ltr", tags: ["zh"] },
  { code: "ja", name: "日本語", dir: "ltr", tags: ["ja"] },
  { code: "ko", name: "한국어", dir: "ltr", tags: ["ko"] },
  { code: "ar", name: "العربية", dir: "rtl", tags: ["ar"] },
  { code: "tr", name: "Türkçe", dir: "ltr", tags: ["tr"] },
  { code: "pl", name: "Polski", dir: "ltr", tags: ["pl"] },
  { code: "uk", name: "Українська", dir: "ltr", tags: ["uk"] },
  { code: "vi", name: "Tiếng Việt", dir: "ltr", tags: ["vi"] },
  { code: "id", name: "Bahasa Indonesia", dir: "ltr", tags: ["id"] },
];

export const DEFAULT_LANGUAGE = "en";

/**
 * Match an OS/Browser locale tag (e.g. "pt-BR", "zh-Hans-CN", "en_GB")
 * to the closest supported language. Unknown locales fall back to English.
 */
export function matchLanguage(locale: string | null | undefined): string {
  if (!locale) return DEFAULT_LANGUAGE;
  const normalized = locale.trim().toLowerCase().replace(/_/g, "-");
  const base = normalized.split("-")[0];

  const exact = LANGUAGES.find((l) => l.tags.some((tag) => tag === normalized));
  if (exact) return exact.code;

  const byBase = LANGUAGES.find((l) => l.tags.some((tag) => tag === base));
  if (byBase) return byBase.code;

  return DEFAULT_LANGUAGE;
}
