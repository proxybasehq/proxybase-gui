import * as Flags from "country-flag-icons/react/3x2";

const flagMap = Flags as Record<string, React.FC<{ className?: string }>>;

export function CountryFlag({ code }: { code: string }) {
  const upper = code.toUpperCase();
  const Flag = flagMap[upper];
  if (!Flag) {
    return <span style={{ fontSize: 10, fontWeight: 600 }}>{upper}</span>;
  }
  return (
    <span style={{ display: "inline-flex", width: 18, height: 12, flexShrink: 0 }}>
      <Flag className="w-full h-full object-cover rounded-sm" />
    </span>
  );
}
