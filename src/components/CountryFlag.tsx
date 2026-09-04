import React from "react";
import * as Flags from "country-flag-icons/react/3x2";
import { countryCode, countryFlag, countryName } from "../utils";

const flagMap = Flags as Record<string, React.FC<{ className?: string }>>;

// Alias mappings for historical, regional, or legacy ISO country codes
const FLAG_ALIASES: Record<string, string> = {
  UK: "GB",
  AN: "CW",
  BU: "MM",
  CS: "RS",
  DD: "DE",
  FX: "FR",
  SU: "RU",
  TP: "TL",
  ZR: "CD",
  YU: "RS",
  DY: "BJ",
  HV: "BF",
  NH: "VU",
  RH: "ZW",
  VD: "VN",
  YD: "YE",
  EA: "ES",
  CP: "FR",
  DG: "IO",
  EZ: "EU",
};

export function GlobeFlag({ className, style }: { className?: string; style?: React.CSSProperties }) {
  return (
    <span
      style={{
        display: "inline-flex",
        width: 18,
        height: 12,
        flexShrink: 0,
        marginRight: 6,
        verticalAlign: "middle",
        ...style,
      }}
      title="WorldWide"
    >
      <svg
        className={className}
        viewBox="0 0 18 12"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        style={{ width: "100%", height: "100%", display: "block", borderRadius: 2 }}
      >
        <rect width="18" height="12" rx="2" fill="#0070f3" fillOpacity="0.15" stroke="#0070f3" strokeWidth="0.8" />
        <circle cx="9" cy="6" r="4.2" stroke="#0070f3" strokeWidth="0.8" />
        <ellipse cx="9" cy="6" rx="2.1" ry="4.2" stroke="#0070f3" strokeWidth="0.7" />
        <line x1="4.8" y1="6" x2="13.2" y2="6" stroke="#0070f3" strokeWidth="0.7" />
        <line x1="5.6" y1="3.9" x2="12.4" y2="3.9" stroke="#0070f3" strokeWidth="0.6" />
        <line x1="5.6" y1="8.1" x2="12.4" y2="8.1" stroke="#0070f3" strokeWidth="0.6" />
      </svg>
    </span>
  );
}

export function UnknownFlag({ className, style }: { className?: string; style?: React.CSSProperties }) {
  return (
    <span
      style={{
        display: "inline-flex",
        width: 18,
        height: 12,
        flexShrink: 0,
        marginRight: 6,
        verticalAlign: "middle",
        ...style,
      }}
      title="Unknown"
    >
      <svg
        className={className}
        viewBox="0 0 18 12"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        style={{ width: "100%", height: "100%", display: "block", borderRadius: 2 }}
      >
        <rect width="18" height="12" rx="2" fill="#888888" fillOpacity="0.15" stroke="#888888" strokeWidth="0.8" strokeDasharray="1.5 1.5" />
        <text
          x="9"
          y="8.5"
          textAnchor="middle"
          fontSize="6.5"
          fontWeight="bold"
          fill="#666666"
          fontFamily="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
        >
          ?
        </text>
      </svg>
    </span>
  );
}

export function CountryFlag({
  code,
  className,
  style,
}: {
  code: string;
  className?: string;
  style?: React.CSSProperties;
}) {
  if (!code || !code.trim()) return null;
  const upper = countryCode(code.trim()).toUpperCase();
  if (upper === "WORLDWIDE") {
    return <GlobeFlag className={className} style={style} />;
  }
  if (upper === "UNKNOWN") {
    return <UnknownFlag className={className} style={style} />;
  }
  const resolvedCode = FLAG_ALIASES[upper] || upper;
  const Flag = flagMap[resolvedCode];
  if (!Flag) {
    const emoji = countryFlag(upper);
    if (emoji) {
      return (
        <span
          style={{
            fontSize: 13,
            marginRight: 6,
            verticalAlign: "middle",
            fontFamily: '"Twemoji Country Flags", "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji", sans-serif',
            ...style,
          }}
          title={countryName(upper) || upper}
        >
          {emoji}
        </span>
      );
    }
    return (
      <span
        style={{
          fontSize: 10,
          fontWeight: 600,
          marginRight: 6,
          verticalAlign: "middle",
          fontFamily: "var(--font-mono)",
          ...style,
        }}
      >
        {upper}
      </span>
    );
  }
  return (
    <span
      style={{
        display: "inline-flex",
        width: 18,
        height: 12,
        flexShrink: 0,
        marginRight: 6,
        verticalAlign: "middle",
        ...style,
      }}
      title={countryName(upper) || upper}
    >
      <Flag className={className || "w-full h-full object-cover rounded-sm"} />
    </span>
  );
}
