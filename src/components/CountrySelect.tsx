import React, { useState, useRef, useEffect, useMemo } from "react";
import { CountryFlag } from "./CountryFlag";
import { countryName } from "../utils";

export interface CountrySelectProps {
  value: string;
  onChange: (val: string) => void;
  countries: string[];
  allLabel?: string;
  style?: React.CSSProperties;
  className?: string;
}

export function CountrySelect({
  value,
  onChange,
  countries,
  allLabel = "All Countries",
  style,
  className,
}: CountrySelectProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [query, setQuery] = useState("");
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Close on outside click
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    }
    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
      // Auto-focus search input if open
      setTimeout(() => inputRef.current?.focus(), 50);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  // Close on Escape key
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape" && isOpen) {
        setIsOpen(false);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen]);

  // Filter countries by query
  const filteredCountries = useMemo(() => {
    if (!query.trim()) return countries;
    const q = query.trim().toLowerCase();
    return countries.filter((c) => {
      const name = countryName(c).toLowerCase();
      const code = c.toLowerCase();
      return name.includes(q) || code.includes(q);
    });
  }, [countries, query]);

  const selectedName = useMemo(() => {
    if (value === "all") return allLabel;
    return countryName(value);
  }, [value, allLabel]);

  return (
    <div
      ref={containerRef}
      className={className}
      style={{
        position: "relative",
        display: "inline-block",
        userSelect: "none",
        ...style,
      }}
    >
      {/* Trigger Button */}
      <button
        type="button"
        onClick={() => {
          setIsOpen((prev) => !prev);
          setQuery("");
        }}
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 6,
          height: 28,
          padding: "0 8px",
          fontSize: 12,
          fontWeight: 500,
          borderRadius: "var(--rounded-sm)",
          border: "1px solid var(--color-hairline)",
          background: "var(--color-canvas)",
          color: "var(--color-ink)",
          cursor: "pointer",
          maxWidth: 180,
          width: "100%",
          textAlign: "left",
          boxShadow: "0 1px 2px rgba(0,0,0,0.04)",
        }}
      >
        <span style={{ display: "inline-flex", alignItems: "center", flexShrink: 0 }}>
          <CountryFlag code={value === "all" ? "WORLDWIDE" : value} />
        </span>
        <span
          style={{
            flex: 1,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
          title={selectedName}
        >
          {selectedName}
        </span>
        <svg
          width="10"
          height="10"
          viewBox="0 0 10 10"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
          style={{
            flexShrink: 0,
            transform: isOpen ? "rotate(180deg)" : "none",
            transition: "transform 0.15s ease",
            opacity: 0.6,
          }}
        >
          <path
            d="M2.5 3.75L5 6.25L7.5 3.75"
            stroke="currentColor"
            strokeWidth="1.2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>

      {/* Dropdown Menu */}
      {isOpen && (
        <div
          style={{
            position: "absolute",
            top: "calc(100% + 4px)",
            right: 0,
            zIndex: 1100,
            minWidth: 200,
            maxWidth: 260,
            background: "var(--color-canvas)",
            border: "1px solid var(--color-hairline)",
            borderRadius: "var(--rounded-md)",
            boxShadow: "0 4px 16px rgba(0,0,0,0.12)",
            overflow: "hidden",
            display: "flex",
            flexDirection: "column",
          }}
        >
          {/* Search input if multiple countries */}
          {countries.length > 5 && (
            <div style={{ padding: "6px 8px", borderBottom: "1px solid var(--color-hairline)" }}>
              <input
                ref={inputRef}
                type="text"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search country..."
                style={{
                  width: "100%",
                  height: 24,
                  fontSize: 11,
                  padding: "0 6px",
                  borderRadius: "var(--rounded-xs)",
                  border: "1px solid var(--color-hairline)",
                  background: "var(--color-canvas-soft)",
                  color: "var(--color-ink)",
                  outline: "none",
                }}
              />
            </div>
          )}

          {/* Options List */}
          <div
            style={{
              maxHeight: 220,
              overflowY: "auto",
              padding: "4px 0",
            }}
          >
            {/* "All Countries" Option (if no filter query or query matches) */}
            {(!query.trim() || allLabel.toLowerCase().includes(query.toLowerCase())) && (
              <div
                onClick={() => {
                  onChange("all");
                  setIsOpen(false);
                }}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  padding: "6px 10px",
                  fontSize: 12,
                  cursor: "pointer",
                  background: value === "all" ? "var(--color-canvas-soft-2)" : "transparent",
                  color: value === "all" ? "var(--color-link)" : "var(--color-ink)",
                  fontWeight: value === "all" ? 600 : 400,
                }}
                onMouseEnter={(e) => {
                  if (value !== "all") e.currentTarget.style.background = "var(--color-canvas-soft)";
                }}
                onMouseLeave={(e) => {
                  if (value !== "all") e.currentTarget.style.background = "transparent";
                }}
              >
                <CountryFlag code="WORLDWIDE" />
                <span style={{ flex: 1 }}>{allLabel}</span>
                {value === "all" && <span style={{ fontSize: 11 }}>✓</span>}
              </div>
            )}

            {/* Country Options */}
            {filteredCountries.map((c) => {
              const name = countryName(c);
              const isSelected = value.toUpperCase() === c.toUpperCase();
              return (
                <div
                  key={c}
                  onClick={() => {
                    onChange(c);
                    setIsOpen(false);
                  }}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    padding: "6px 10px",
                    fontSize: 12,
                    cursor: "pointer",
                    background: isSelected ? "var(--color-canvas-soft-2)" : "transparent",
                    color: isSelected ? "var(--color-link)" : "var(--color-ink)",
                    fontWeight: isSelected ? 600 : 400,
                  }}
                  onMouseEnter={(e) => {
                    if (!isSelected) e.currentTarget.style.background = "var(--color-canvas-soft)";
                  }}
                  onMouseLeave={(e) => {
                    if (!isSelected) e.currentTarget.style.background = "transparent";
                  }}
                >
                  <CountryFlag code={c} />
                  <span
                    style={{
                      flex: 1,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {name}
                  </span>
                  <span
                    style={{
                      fontSize: 10,
                      fontFamily: "var(--font-mono)",
                      color: "var(--color-mute)",
                      marginLeft: 4,
                    }}
                  >
                    {c.toUpperCase()}
                  </span>
                  {isSelected && <span style={{ fontSize: 11, marginLeft: 4 }}>✓</span>}
                </div>
              );
            })}

            {filteredCountries.length === 0 && query.trim() && (
              <div
                style={{
                  padding: "12px 10px",
                  fontSize: 11,
                  color: "var(--color-mute)",
                  textAlign: "center",
                }}
              >
                No matching countries
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
