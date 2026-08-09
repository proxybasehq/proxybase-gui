import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  DEFAULT_LANGUAGE,
  LANGUAGES,
  matchLanguage,
  type Language,
} from "./languages";
import { translations, type Messages } from "./translations";

const STORAGE_KEY = "proxybase_language";
const SETTINGS_STORE = "proxybase-settings.json";
const SETTINGS_STORE_KEY = "language";
const SYSTEM_PLACEHOLDER = "system";

interface I18nContextValue {
  lang: string;
  language: Language;
  /** Translate a message key, optionally interpolating {placeholder}s. */
  t: (key: keyof Messages, vars?: Record<string, string | number>) => string;
  /** Set a concrete language and remember the choice. */
  setLanguage: (code: string) => void;
  /** Follow the system language again (re-detected on every launch). */
  setSystemLanguage: () => void;
  /** True when no explicit language has been chosen yet. */
  isSystemDefault: boolean;
}

const I18nContext = createContext<I18nContextValue | null>(null);

function readStoredLanguage(): string | null {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored || stored === SYSTEM_PLACEHOLDER) return stored || null;
    return LANGUAGES.some((l) => l.code === stored) ? stored : null;
  } catch {
    return null;
  }
}

/** Fast synchronous guess for the very first render; refined by the OS locale. */
function initialLanguage(): string {
  const stored = readStoredLanguage();
  if (stored && stored !== SYSTEM_PLACEHOLDER) return stored;
  return matchLanguage(navigator.language);
}

/**
 * Resolve the system locale through the Tauri OS plugin, falling back to the
 * browser locale outside of Tauri (e.g. plain `vite dev` in a web browser).
 */
async function detectSystemLanguage(): Promise<string> {
  try {
    const { locale } = await import("@tauri-apps/plugin-os");
    const sysLocale = await locale();
    if (sysLocale) return matchLanguage(sysLocale);
  } catch {
    // Not running inside Tauri, or plugin not registered yet.
  }
  return matchLanguage(navigator.language);
}

function persistLanguage(code: string) {
  try {
    localStorage.setItem(STORAGE_KEY, code);
  } catch {
    // Storage unavailable — keep the in-memory language for this session.
  }
  // Best-effort mirror into the Tauri settings store so the choice survives
  // webview data resets and is available to other components.
  import("@tauri-apps/plugin-store")
    .then(({ load }) => load(SETTINGS_STORE))
    .then(async (store) => {
      await store.set(SETTINGS_STORE_KEY, code);
      await store.save();
    })
    .catch(() => {});
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLang] = useState<string>(initialLanguage);
  const [isSystemDefault, setIsSystemDefault] = useState<boolean>(() => {
    const stored = readStoredLanguage();
    return stored === null || stored === SYSTEM_PLACEHOLDER;
  });

  // First launch (or "follow system"): auto-detect and remember the language.
  useEffect(() => {
    const stored = readStoredLanguage();
    if (stored && stored !== SYSTEM_PLACEHOLDER) return;
    detectSystemLanguage().then((code) => {
      setLang(code);
      // "Not set before" → save the detected language so later starts reuse it.
      if (!stored) persistLanguage(code);
    });
  }, []);

  // Apply language + direction to the document.
  useEffect(() => {
    const language = LANGUAGES.find((l) => l.code === lang) ?? LANGUAGES[0];
    document.documentElement.lang = language.code;
    document.documentElement.dir = language.dir;
  }, [lang]);

  const setLanguage = useCallback((code: string) => {
    if (!LANGUAGES.some((l) => l.code === code)) return;
    setLang(code);
    setIsSystemDefault(false);
    persistLanguage(code);
  }, []);

  const setSystemLanguage = useCallback(() => {
    setIsSystemDefault(true);
    persistLanguage(SYSTEM_PLACEHOLDER);
    detectSystemLanguage().then(setLang);
  }, []);

  const t = useCallback<I18nContextValue["t"]>(
    (key, vars) => {
      const dict = translations[lang] ?? translations[DEFAULT_LANGUAGE];
      let text = dict[key] ?? translations[DEFAULT_LANGUAGE][key] ?? String(key);
      if (vars) {
        for (const [name, value] of Object.entries(vars)) {
          text = text.split(`{${name}}`).join(String(value));
        }
      }
      return text;
    },
    [lang],
  );

  const value = useMemo<I18nContextValue>(() => {
    const language =
      LANGUAGES.find((l) => l.code === lang) ??
      LANGUAGES.find((l) => l.code === DEFAULT_LANGUAGE)!;
    return {
      lang,
      language,
      t,
      setLanguage,
      setSystemLanguage,
      isSystemDefault,
    };
  }, [lang, t, setLanguage, setSystemLanguage, isSystemDefault]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used inside <I18nProvider>");
  return ctx;
}

export { LANGUAGES };
