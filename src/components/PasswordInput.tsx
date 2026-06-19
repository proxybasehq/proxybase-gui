import { useState } from "react";

interface PasswordInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  label?: string;
}

export default function PasswordInput({ value, onChange, placeholder, label }: PasswordInputProps) {
  const [show, setShow] = useState(false);

  return (
    <div className="form-group">
      {label && <label className="form-label">{label}</label>}
      <div style={{ position: "relative" }}>
        <input
          type={show ? "text" : "password"}
          className="form-input"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          style={{ paddingRight: 48 }}
        />
        <button
          type="button"
          onClick={() => setShow(!show)}
          style={{
            position: "absolute",
            right: 8,
            top: "50%",
            transform: "translateY(-50%)",
            background: "none",
            border: "none",
            cursor: "pointer",
            fontSize: 12,
            color: "var(--color-mute)",
            fontFamily: "var(--font-mono)",
          }}
        >
          {show ? "HIDE" : "SHOW"}
        </button>
      </div>
    </div>
  );
}
