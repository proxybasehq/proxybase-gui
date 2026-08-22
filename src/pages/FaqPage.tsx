import { useState } from "react";
import { PROXY_ADDRESS } from "../utils";
import { useI18n } from "../i18n";
import type { Messages } from "../i18n/translations";

// View-only topic icons; all glyphs are old enough to render on every
// desktop OS and Android WebView.
const EMOJI_FONT = "'Apple Color Emoji', 'Segoe UI Emoji', 'Noto Color Emoji', sans-serif";

const FAQ_KEYS: Array<{ q: keyof Messages; a: keyof Messages; emoji: string; vars?: Record<string, string> }> = [
  { q: "faq.q1", a: "faq.a1", emoji: "🌐" },
  { q: "faq.q2", a: "faq.a2", emoji: "💸" },
  { q: "faq.q3", a: "faq.a3", emoji: "📶" },
  { q: "faq.q4", a: "faq.a4", emoji: "🛒" },
  { q: "faq.q5", a: "faq.a5", emoji: "💵" },
  { q: "faq.q6", a: "faq.a6", emoji: "💳" },
  { q: "faq.q7", a: "faq.a7", emoji: "👛" },
  { q: "faq.q8", a: "faq.a8", emoji: "🚪" },
  { q: "faq.q9", a: "faq.a9", emoji: "🔑" },
  { q: "faq.q10", a: "faq.a10", emoji: "🔓" },
  { q: "faq.q11", a: "faq.a11", emoji: "💾" },
  { q: "faq.q12", a: "faq.a12", emoji: "📡" },
  { q: "faq.q13", a: "faq.a13", emoji: "🏆" },
  { q: "faq.q14", a: "faq.a14", emoji: "🔌", vars: { proxyAddress: PROXY_ADDRESS } },
  { q: "faq.q15", a: "faq.a15", emoji: "🔒" },
  { q: "faq.q16", a: "faq.a16", emoji: "🔗" },
  { q: "faq.q17", a: "faq.a17", emoji: "💬" },
  { q: "faq.q18", a: "faq.a18", emoji: "ℹ️" },
];

export default function FaqPage() {
  const { t } = useI18n();
  const [openIdx, setOpenIdx] = useState<number | null>(null);

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">{t("faq.title")}</h1>
        <p className="page-description">{t("faq.desc")}</p>
      </div>

      {FAQ_KEYS.map((faq, i) => (
        <div key={i} className="card" style={{ cursor: "pointer" }} onClick={() => setOpenIdx(openIdx === i ? null : i)}>
          <div className="flex justify-between items-center" style={{ marginBottom: openIdx === i ? "var(--space-sm)" : 0 }}>
            <span style={{ fontSize: 14, fontWeight: 600, color: "var(--color-ink)" }}>
              <span aria-hidden="true" style={{ marginRight: "var(--space-xs)", fontFamily: EMOJI_FONT }}>{faq.emoji}</span>
              {t(faq.q)}
            </span>
            <span style={{ fontSize: 12, color: "var(--color-mute)", transform: openIdx === i ? "rotate(180deg)" : "none", transition: "transform 0.15s" }}>
              ▼
            </span>
          </div>
          {openIdx === i && (
            <p style={{ fontSize: 14, color: "var(--color-body)", lineHeight: 1.7, marginTop: "var(--space-xs)" }}>
              <span aria-hidden="true" style={{ marginRight: "var(--space-xs)", fontFamily: EMOJI_FONT }}>{faq.emoji}</span>
              {t(faq.a, faq.vars)}
            </p>
          )}
        </div>
      ))}
    </div>
  );
}
