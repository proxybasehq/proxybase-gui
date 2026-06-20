import { useState, useEffect, useRef } from "react";
import { useNavigate, Navigate } from "react-router-dom";
import { useOutletContext } from "react-router-dom";
import { getDeposit } from "../api";
import type { AppContext } from "../components/Layout";
import { useBackend } from "../hooks/useBackend";
import { formatUsd } from "../utils";

const DEPOSIT_TIMEOUT_SECS = 9 * 60;
const POLL_INTERVAL_MS = 10_000;

export interface DepositState {
  deposit_id: string;
  pay_address: string;
  pay_currency: string;
  pay_amount: number;
  amount_microcredits: number;
  qrDataUrl: string;
}

// Module-level store so state survives HashRouter navigation
let _pendingDeposit: DepositState | null = null;

export function setPendingDeposit(s: DepositState) { _pendingDeposit = s; }

export default function DepositPage() {
  const navigate = useNavigate();
  const { backendUrl } = useBackend();
  const { isAuthenticated } = useOutletContext<AppContext>();

  // Capture and consume the pending deposit on first render
  const [state] = useState<DepositState | null>(() => {
    const s = _pendingDeposit;
    _pendingDeposit = null;
    return s;
  });

  if (!isAuthenticated) return <Navigate to="/login" replace />;
  if (!state) return <Navigate to="/market" replace />;

  const [step, setStep] = useState<"created" | "completed" | "expired">("created");
  const [result, setResult] = useState<Record<string, unknown> | null>(null);
  const [countdown, setCountdown] = useState(DEPOSIT_TIMEOUT_SECS);
  const [copied, setCopied] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  async function copyToClipboard(value: string, label: string) {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(label);
      setTimeout(() => setCopied(null), 1500);
    } catch {
      // Fallback for non-secure contexts
      const ta = document.createElement("textarea");
      ta.value = value;
      ta.style.position = "fixed"; ta.style.opacity = "0";
      document.body.appendChild(ta); ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
      setCopied(label);
      setTimeout(() => setCopied(null), 1500);
    }
  }

  function clearTimers() {
    if (pollRef.current) { clearInterval(pollRef.current); pollRef.current = null; }
    if (timerRef.current) { clearInterval(timerRef.current); timerRef.current = null; }
  }

  useEffect(() => {
    timerRef.current = setInterval(() => {
      setCountdown((prev) => {
        if (prev <= 1) {
          clearTimers();
          setStep("expired");
          return 0;
        }
        return prev - 1;
      });
    }, 1000);

    pollRef.current = setInterval(async () => {
      try {
        const status = await getDeposit(backendUrl, state.deposit_id);
        const s = (status as any).status || "";
        if (s === "paid" || s === "completed" || s === "confirming") {
          clearTimers();
          setResult(status);
          setStep("completed");
        }
      } catch (_) { /* ignore */ }
    }, POLL_INTERVAL_MS);

    return clearTimers;
  }, []);

  function formatCountdown(secs: number) {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  return (
    <div>
      {step === "created" && (
        <div className="card" style={{ textAlign: "center" }}>
          <div className="alert alert-success" style={{ justifyContent: "center", marginBottom: 8 }}>
            Deposit created — send the exact amount shown
          </div>

          {state.qrDataUrl && (
            <div style={{ marginBottom: 8 }}>
              <img src={state.qrDataUrl} alt="Payment QR"
                style={{ border: "1px solid var(--color-hairline)", borderRadius: "var(--rounded-sm)", width: 120, height: 120 }} />
            </div>
          )}
          <table style={{ marginBottom: 8 }}><tbody>
            <tr onClick={() => copyToClipboard(state.pay_address, "Address")} style={{ cursor: "pointer" }}>
              <td style={{ color: "var(--color-mute)", fontSize: 10, padding: "1px 4px 1px 0", whiteSpace: "nowrap" }}>Address</td>
              <td className="font-mono" style={{ fontSize: 10, wordBreak: "break-all", padding: "1px 0" }}>
                {state.pay_address}
                {copied === "Address" && <span style={{ color: "#22c55e", marginLeft: 4, fontSize: 9 }}>Copied!</span>}
              </td>
            </tr>
            <tr onClick={() => copyToClipboard(state.pay_currency, "Currency")} style={{ cursor: "pointer" }}>
              <td style={{ color: "var(--color-mute)", fontSize: 10, padding: "1px 4px 1px 0" }}>Currency</td>
              <td className="font-mono" style={{ fontSize: 10, padding: "1px 0" }}>
                {state.pay_currency}
                {copied === "Currency" && <span style={{ color: "#22c55e", marginLeft: 4, fontSize: 9 }}>Copied!</span>}
              </td>
            </tr>
            {state.pay_amount != null && (
              <tr onClick={() => copyToClipboard(String(state.pay_amount), "Amount")} style={{ cursor: "pointer" }}>
                <td style={{ color: "var(--color-mute)", fontSize: 10, padding: "1px 4px 1px 0" }}>Amount</td>
                <td className="font-mono" style={{ fontSize: 10, padding: "1px 0" }}>
                  {state.pay_amount}
                  {copied === "Amount" && <span style={{ color: "#22c55e", marginLeft: 4, fontSize: 9 }}>Copied!</span>}
                </td>
              </tr>
            )}
            <tr onClick={() => copyToClipboard(state.deposit_id, "Deposit ID")} style={{ cursor: "pointer" }}>
              <td style={{ color: "var(--color-mute)", fontSize: 10, padding: "1px 4px 1px 0" }}>Deposit ID</td>
              <td className="font-mono" style={{ fontSize: 10, padding: "1px 0" }}>
                {state.deposit_id}
                {copied === "Deposit ID" && <span style={{ color: "#22c55e", marginLeft: 4, fontSize: 9 }}>Copied!</span>}
              </td>
            </tr>
          </tbody></table>

          <div style={{ marginTop: 8, marginBottom: 8 }}>
            <span style={{
              fontSize: 22,
              fontFamily: "var(--font-mono)",
              color: countdown < 60 ? "var(--color-error)" : "var(--color-ink)",
              fontWeight: 600,
            }}>
              {formatCountdown(countdown)}
            </span>
            <div className="text-muted" style={{ fontSize: 10 }}>time remaining</div>
          </div>

          <button className="btn btn-secondary btn-sm" onClick={() => navigate(-1)}>
            Back
          </button>
        </div>
      )}

      {step === "completed" && result && (
        <div className="card" style={{ textAlign: "center" }}>
          <div style={{ fontSize: 32, marginBottom: 4 }}>{'✅'}</div>
          <div className="card-title" style={{ fontSize: 14 }}>Deposit Complete</div>
          <table style={{ marginTop: 4, marginBottom: 8 }}><tbody>
            <tr>
              <td style={{ color: "var(--color-mute)", fontSize: 11, padding: "1px 6px 1px 0" }}>Status</td>
              <td className="font-mono" style={{ fontSize: 11, padding: "1px 0" }}>{(result as any).status || "completed"}</td>
            </tr>
            {(result as any).amount_microcredits != null && (
              <tr>
                <td style={{ color: "var(--color-mute)", fontSize: 11, padding: "1px 6px 1px 0" }}>Credited</td>
                <td className="font-mono" style={{ fontSize: 13, fontWeight: 600, padding: "1px 0" }}>{formatUsd((result as any).amount_microcredits)}</td>
              </tr>
            )}
          </tbody></table>
          <button className="btn btn-primary" onClick={() => navigate("/market", { replace: true })}>
            Done
          </button>
        </div>
      )}

      {step === "expired" && (
        <div className="card" style={{ textAlign: "center" }}>
          <div className="alert alert-error" style={{ justifyContent: "center" }}>
            Payment time expired. Please create a new deposit.
          </div>
          <div style={{ display: "flex", gap: "var(--space-sm)", marginTop: 8 }}>
            <button className="btn btn-primary" style={{ flex: 1 }}
              onClick={() => navigate(-1)}>
              Try Again
            </button>
            <button className="btn btn-secondary" style={{ flex: 1 }}
              onClick={() => navigate("/market", { replace: true })}>
              Close
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
