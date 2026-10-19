import { useEffect, useState } from "react";
import { useSystemTunnel, type TunnelSettings } from "../../hooks/useSystemTunnel";
import { useI18n } from "../../i18n";

interface Props {
  open: boolean;
  onClose: () => void;
}

function ToggleRow({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label
      style={{
        display: "flex",
        alignItems: "flex-start",
        gap: 10,
        padding: "10px 0",
        borderBottom: "1px solid var(--color-border, #e5e5e5)",
        cursor: "pointer",
      }}
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        style={{ marginTop: 3 }}
      />
      <span>
        <span style={{ display: "block", fontSize: 13, fontWeight: 600 }}>{label}</span>
        <span style={{ display: "block", fontSize: 11, color: "var(--color-mute)", marginTop: 2 }}>
          {description}
        </span>
      </span>
    </label>
  );
}

export default function TunnelSettingsModal({ open, onClose }: Props) {
  const { t } = useI18n();
  const { settings, setSettings, state } = useSystemTunnel();
  const [draft, setDraft] = useState<TunnelSettings>(settings);
  const [cidrText, setCidrText] = useState(settings.bypassCidrs.join("\n"));

  useEffect(() => {
    if (!open) return;
    setDraft(settings);
    setCidrText(settings.bypassCidrs.join("\n"));
  }, [open, settings]);

  if (!open) return null;

  function save() {
    const bypassCidrs = cidrText
      .split(/[\n,]+/)
      .map((value) => value.trim())
      .filter(Boolean);
    setSettings({ ...draft, bypassCidrs });
    onClose();
  }

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.4)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1100,
      }}
      onClick={onClose}
    >
      <div
        className="modal-card"
        style={{ width: "min(420px, 92vw)", maxHeight: "86vh", overflowY: "auto" }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="card-title">{t("tunnel.settingsTitle")}</div>
        <p className="text-muted" style={{ fontSize: 12, marginTop: 4 }}>
          {t("tunnel.settingsDesc")}
        </p>

        <ToggleRow
          label={t("tunnel.killSwitch")}
          description={t("tunnel.killSwitchDesc")}
          checked={draft.killSwitch}
          onChange={(value) => setDraft({ ...draft, killSwitch: value })}
        />
        <ToggleRow
          label={t("tunnel.quic")}
          description={t("tunnel.quicDesc")}
          checked={draft.disableQuicUdp443}
          onChange={(value) => setDraft({ ...draft, disableQuicUdp443: value })}
        />
        <ToggleRow
          label={t("tunnel.stun")}
          description={t("tunnel.stunDesc")}
          checked={draft.blockWebrtcStun}
          onChange={(value) => setDraft({ ...draft, blockWebrtcStun: value })}
        />
        <ToggleRow
          label={t("tunnel.stopOnExit")}
          description={t("tunnel.stopOnExitDesc")}
          checked={draft.stopOnGuiExit}
          onChange={(value) => setDraft({ ...draft, stopOnGuiExit: value })}
        />

        <div className="form-group" style={{ marginTop: 14 }}>
          <label className="form-label">{t("tunnel.bypass")}</label>
          <textarea
            value={cidrText}
            onChange={(e) => setCidrText(e.target.value)}
            placeholder={"192.168.50.0/24\n10.20.0.0/16"}
            rows={3}
            style={{
              width: "100%",
              fontFamily: "JetBrains Mono, monospace",
              fontSize: 12,
              padding: 8,
              borderRadius: 6,
              border: "1px solid var(--color-border, #e5e5e5)",
              resize: "vertical",
            }}
          />
          <span style={{ fontSize: 11, color: "var(--color-mute)" }}>
            {t("tunnel.bypassDesc")}
          </span>
        </div>

        {state.status === "connected" && (
          <p className="text-muted" style={{ fontSize: 11, marginTop: 8 }}>
            {t("tunnel.appliesNext")}
          </p>
        )}

        <div style={{ display: "flex", gap: 8, marginTop: 18 }}>
          <button className="btn btn-primary" style={{ flex: 1 }} onClick={save}>
            {t("common.continue")}
          </button>
          <button className="btn btn-secondary" style={{ flex: 1 }} onClick={onClose}>
            {t("common.cancel")}
          </button>
        </div>
      </div>
    </div>
  );
}
