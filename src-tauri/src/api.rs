use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// HTTP API client
// ---------------------------------------------------------------------------

pub struct BackendClient {
    http: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

/// Process an HTTP response, turning non-2xx into errors with the body's error message.
async fn response_json(resp: reqwest::Response) -> Result<serde_json::Value> {
    if resp.status().is_success() {
        return resp.json().await.context("Failed to parse response");
    }
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
    let msg = body
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown error");
    anyhow::bail!("{}", msg)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChallengeResponse {
    pub nonce: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VerifyResponse {
    pub session_token: String,
    pub wallet_address: String,
    pub role: String,
    pub buyer_available: i64,
    pub spendable_balance: i64,
}

impl BackendClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            token: Self::load_token(),
        }
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token.clone());
        Self::save_token(&token);
    }

    fn token_path() -> std::path::PathBuf {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".proxybase")
            .join("session_token")
    }

    fn load_token() -> Option<String> {
        std::fs::read_to_string(Self::token_path()).ok()
    }

    pub fn save_token(token: &str) {
        let path = Self::token_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, token);
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.token.as_deref().unwrap_or(""))
    }

    pub fn ws_url_for_seller(&self) -> String {
        let token = self.token.as_deref().unwrap_or("");
        format!(
            "{}/v2/ws/seller?token={}",
            self.base_url
                .replace("http://", "ws://")
                .replace("https://", "wss://"),
            token
        )
    }

    // --- Auth ---

    pub async fn auth_challenge(&self, wallet_address: &str) -> Result<ChallengeResponse> {
        let resp = self
            .http
            .post(format!("{}/v2/auth/challenge", self.base_url))
            .json(&serde_json::json!({"wallet_address": wallet_address}))
            .send()
            .await
            .context("Failed to request auth challenge")?;
        let json = response_json(resp).await?;
        serde_json::from_value(json).context("Failed to parse challenge response")
    }

    pub async fn auth_verify(
        &self,
        public_key_hex: &str,
        nonce: &str,
        timestamp: &str,
        signature_hex: &str,
    ) -> Result<VerifyResponse> {
        let resp = self
            .http
            .post(format!("{}/v2/auth/verify", self.base_url))
            .json(&serde_json::json!({
                "public_key_hex": public_key_hex,
                "nonce": nonce,
                "timestamp": timestamp,
                "signature_hex": signature_hex,
            }))
            .send()
            .await
            .context("Failed to verify auth")?;
        let json = response_json(resp).await?;
        serde_json::from_value(json).context("Failed to parse verify response")
    }

    // --- Wallet ---

    pub async fn get_balance(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/v2/wallet/balance", self.base_url))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to fetch balance")?;
        response_json(resp).await
    }

    pub async fn transfer(&self, amount: i64) -> Result<serde_json::Value> {
        let resp = self
            .http
            .post(format!("{}/v2/wallet/transfer", self.base_url))
            .header("Authorization", self.bearer())
            .json(&serde_json::json!({"amount_microcredits": amount}))
            .send()
            .await
            .context("Failed to transfer")?;
        response_json(resp).await
    }

    // --- Currencies ---

    pub async fn list_currencies(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/v2/currencies", self.base_url))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to fetch currencies")?;
        response_json(resp).await
    }

    // --- Deposits ---

    pub async fn create_deposit(&self, amount: i64, currency: &str) -> Result<serde_json::Value> {
        let resp = self
            .http
            .post(format!("{}/v2/deposits", self.base_url))
            .header("Authorization", self.bearer())
            .json(&serde_json::json!({
                "amount_microcredits": amount,
                "pay_currency": currency,
            }))
            .send()
            .await
            .context("Failed to create deposit")?;
        let json = response_json(resp).await?;
        if let Some(err) = json.get("error").and_then(|v| v.as_str()) {
            anyhow::bail!("{}", err);
        }
        Ok(json)
    }

    pub async fn get_deposit(&self, deposit_id: &str) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/v2/deposits/{}", self.base_url, deposit_id))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to fetch deposit")?;
        response_json(resp).await
    }

    pub async fn list_deposits(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/v2/deposits", self.base_url))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to fetch deposits")?;
        response_json(resp).await
    }

    // --- Seller ---

    pub async fn register_seller(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .post(format!("{}/v2/seller/register", self.base_url))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to register seller")?;
        response_json(resp).await
    }

    pub async fn seller_status(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/v2/seller/status", self.base_url))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to fetch seller status")?;
        response_json(resp).await
    }

    pub async fn list_payouts(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/v2/payouts", self.base_url))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to fetch payouts")?;
        response_json(resp).await
    }

    // --- Market ---

    pub async fn list_countries(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/v2/catalog/countries", self.base_url))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to fetch countries")?;
        response_json(resp).await
    }

    pub async fn list_pricing(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/v2/catalog/pricing", self.base_url))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to fetch pricing")?;
        response_json(resp).await
    }

    pub async fn create_session(
        &self,
        country: &str,
        network_type: &str,
        session_type: &str,
        spend_cap: Option<i64>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .http
            .post(format!("{}/v2/sessions", self.base_url))
            .header("Authorization", self.bearer())
            .json(&serde_json::json!({
                "country": country,
                "network_type": network_type,
                "session_type": session_type,
                "spend_cap_microcredits": spend_cap,
            }))
            .send()
            .await
            .context("Failed to create session")?;
        response_json(resp).await
    }

    pub async fn close_session(&self, session_id: &str) -> Result<serde_json::Value> {
        let resp = self
            .http
            .delete(format!("{}/v2/sessions/{}", self.base_url, session_id))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to close session")?;
        response_json(resp).await
    }

    pub async fn list_sessions(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/v2/sessions", self.base_url))
            .header("Authorization", self.bearer())
            .send()
            .await
            .context("Failed to fetch sessions")?;
        response_json(resp).await
    }
}
