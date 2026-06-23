import { useState } from "react";
import { PROXY_ADDRESS } from "../utils";

const faqs = [
  {
    q: "What is ProxyBase Markets?",
    a: "ProxyBase Markets is a decentralized peer-to-peer bandwidth marketplace. Sellers offer their internet connections as proxy exits, and buyers purchase access to these proxies for web scraping, AI agent browsing, and other automated traffic. All payments are settled in microcredits backed by cryptocurrency deposits.",
  },
  {
    q: "How do I start selling my bandwidth?",
    a: "Go to the Seller tab, configure your upstream proxies (optional), and click 'Start Seller'. Your node will register with the marketplace and begin receiving QoS probes. After passing quality checks, your connection is classified by country and network type, and becomes available for buyers to purchase. You earn credits for every GB of traffic you relay.",
  },
  {
    q: "What are the network types?",
    a: "ProxyBase classifies seller connections into five types: Residential (home ISP), Mobile (cellular carrier), Datacenter (cloud/colo IPs), ISP (business/static IPs), and Burner (VPN/tor/proxy IPs). Burner IPs are flagged during QoS probing and may receive lower pricing or be restricted from certain buyer pools.",
  },
  {
    q: "How do I buy a proxy session?",
    a: "Go to the Market tab → Prices. Find a country and network type with available sellers, then click the green 'Buy' button on that row. A SOCKS5 proxy session is created and appears under the 'Active Sessions' tab. Your wallet balance is debited per GB of traffic used. Close a session anytime by clicking the X button on its row.",
  },
  {
    q: "How do microcredits and pricing work?",
    a: "1,000,000 microcredits = $1.00 USD. Pricing is set per country and network type (e.g. $0.50/GB for US residential). Sellers earn credits when buyers use their proxies. You can deposit funds using cryptocurrencies (BTC, USDC, USDT, SOL, etc.) via NOWPayments.",
  },
  {
    q: "What is a deposit and how do I create one?",
    a: "Open the Account page from the header icon, then tap 'Add Funds'. Choose an amount ($10, $20, $100, or custom) and a cryptocurrency. After creation you'll be taken to a deposit page showing the payment address, QR code, and a 9-minute countdown. Send exactly the shown amount to that address — your wallet balance updates automatically once confirmed.",
  },
  {
    q: "How do I check my wallet balance?",
    a: "Go to the Account page and tap 'View Balance'. Your balances are displayed, including spendable balance, buyer available/reserved/spent, and seller pending/available/payout-locked earnings.",
  },
  {
    q: "What happens when I close the app?",
    a: "The app continues running in the system tray. Closing the window hides it — it doesn't quit. Click the tray icon to show or hide. If you were selling bandwidth, your seller session persists and auto-restarts on next launch. The app also registers for autostart so it launches on system boot.",
  },
  {
    q: "How do I create a wallet?",
    a: "The welcome screen guides you through wallet creation on first launch. To manage your wallet later, go to the Account page → 'Manage Wallet'. Use the 'Create' tab to generate a new BIP-39 mnemonic (12 words), or 'Import' to restore from an existing phrase. Save the words securely — they are the only way to recover your wallet. Your wallet data is stored under ~/.proxybase/.",
  },
  {
    q: "Does the app auto-login?",
    a: "Yes. On launch the welcome screen detects your existing wallet and signs you in automatically if no password is set. If you set a wallet password, you'll need to enter it on the Login page.",
  },
  {
    q: "What data is stored on my machine?",
    a: "Everything is stored under ~/.proxybase/: your encrypted wallet keyfile (wallet/keyfile.enc), session token (session_token), and configuration (config.toml). No private keys are ever sent to the backend — authentication uses cryptographic signatures from your local wallet.",
  },
  {
    q: "What are QoS probes and how do they affect my seller status?",
    a: "Quality of Service probes are automated tests run by the marketplace to verify your connection's speed, latency, and reliability. Probes connect through your relay, measure latency and uptime, and classify your IP (country, ISP, network type). Sellers that fail probes are suspended. Consistent good performance leads to promotion from the Trial pool to Production.",
  },
  {
    q: "What are the seller pool tiers?",
    a: "Trial: New sellers undergoing QoS evaluation. Production: Verified sellers with proven reliability — these are prioritized for buyer sessions. Suspended: Sellers that failed probes or went offline — automatically re-evaluated after a holdback period.",
  },
  {
    q: "How do I use the SOCKS5 proxy after buying a session?",
    a: `Click a session row in the Active Sessions tab to see connection details. Configure your application to use a SOCKS5 proxy at ${PROXY_ADDRESS} with the session ID as username and your session token as password. Example: curl.exe --socks5 ${PROXY_ADDRESS} --proxy-user SESSION_ID:TOKEN https://example.com (use curl on macOS/Linux)`,
  },
  {
    q: "What happens to my sessions when I log out?",
    a: "Logging out from the Account page closes all active buyer proxy sessions and stops the seller connection if running. Your session token is deleted from disk.",
  },
  {
    q: "How do I connect through an upstream proxy for reselling?",
    a: "In the Seller tab, add upstream proxies in the 'Upstream Proxies' section. Each entry has a host:port, username, and password. The seller relays traffic through these upstream proxies instead of (or in addition to) your own connection. Use the 'Include direct' checkbox to also sell your own bandwidth. The backend distributes streams across all paths using a round-robin hash.",
  },
  {
    q: "What is the Discord for?",
    a: "The Discord icon in the header links to the ProxyBase community server (discord.gg/7uedk7ajHD). Join for support, announcements, and discussions with other sellers and buyers.",
  },
  {
    q: "Where can I see my seller status and system info?",
    a: "Everything is on the Account page — tap the person icon in the top-right header. You'll find your connection status, wallet address and balance, seller state with active stream count, system file paths, and the app version.",
  },
];

export default function FaqPage() {
  const [openIdx, setOpenIdx] = useState<number | null>(null);

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">FAQ</h1>
        <p className="page-description">Frequently asked questions about ProxyBase Markets.</p>
      </div>

      {faqs.map((faq, i) => (
        <div key={i} className="card" style={{ cursor: "pointer" }} onClick={() => setOpenIdx(openIdx === i ? null : i)}>
          <div className="flex justify-between items-center" style={{ marginBottom: openIdx === i ? "var(--space-sm)" : 0 }}>
            <span style={{ fontSize: 14, fontWeight: 600, color: "var(--color-ink)" }}>{faq.q}</span>
            <span style={{ fontSize: 12, color: "var(--color-mute)", transform: openIdx === i ? "rotate(180deg)" : "none", transition: "transform 0.15s" }}>
              ▼
            </span>
          </div>
          {openIdx === i && (
            <p style={{ fontSize: 14, color: "var(--color-body)", lineHeight: 1.7, marginTop: "var(--space-xs)" }}>
              {faq.a}
            </p>
          )}
        </div>
      ))}
    </div>
  );
}
