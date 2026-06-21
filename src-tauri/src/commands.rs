use crate::api::BackendClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WalletInfo {
    pub address: String,
    pub loaded: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateWalletResult {
    pub address: String,
    pub mnemonic: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoginResult {
    pub session_token: String,
    pub wallet_address: String,
    pub role: String,
    pub buyer_available: i64,
    pub spendable_balance: i64,
}

// ---------------------------------------------------------------------------
// Helpers (must be defined before use by commands)
// ---------------------------------------------------------------------------

fn wallet_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".proxybase")
}

fn require_auth(client: &BackendClient) -> Result<(), String> {
    if !client.is_authenticated() {
        Err("Not authenticated. Please login first.".to_string())
    } else {
        Ok(())
    }
}

/// Silently re-authenticate using the on-disk wallet (no password).
pub(crate) async fn reauth(backend_url: &str) -> Result<(), String> {
    let data_dir = wallet_dir();
    let mut wm =
        libproxybase::WalletManager::new(data_dir).map_err(|e| e.to_string())?;
    wm.load("").map_err(|e| format!("Failed to load wallet: {}", e))?;

    let address = wm
        .address()
        .ok_or_else(|| "Wallet not loaded".to_string())?
        .to_string();
    let client = BackendClient::new(backend_url);

    let challenge = client
        .auth_challenge(&address)
        .await
        .map_err(|e| format!("Auth challenge failed: {}", e))?;
    let message = format!("{}:{}:{}", address, challenge.nonce, challenge.timestamp);
    let signature = wm.sign(message.as_bytes()).map_err(|e| e.to_string())?;
    let sig_hex = hex::encode(&signature);
    let public_key_hex = wm
        .public_key_hex()
        .ok_or_else(|| "Cannot get public key".to_string())?;

    let auth = client
        .auth_verify(
            &public_key_hex,
            &challenge.nonce,
            &challenge.timestamp,
            &sig_hex,
        )
        .await
        .map_err(|e| format!("Auth verify failed: {}", e))?;

    BackendClient::save_token(&auth.session_token);
    Ok(())
}

/// Call an API method through the client, silently re-authenticating once on failure.
macro_rules! call_api {
    ($backend_url:expr, $client:ident, $expr:expr) => {{
        let $client = BackendClient::new($backend_url);
        if !$client.is_authenticated() {
            return Err("Not authenticated. Please login first.".to_string());
        }
        match $expr.await {
            Ok(v) => Ok(v),
            Err(_e) => {
                // Token may be stale (e.g. backend restarted) — re-auth silently
                let _ = reauth($backend_url).await;
                let $client = BackendClient::new($backend_url);
                $expr.await.map_err(|e2| e2.to_string())
            }
        }
    }};
}

// ---------------------------------------------------------------------------
// Wallet commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn wallet_create(password: String) -> Result<CreateWalletResult, String> {
    let data_dir = wallet_dir();
    let mut wm = libproxybase::WalletManager::new(data_dir).map_err(|e| e.to_string())?;
    let mnemonic = wm
        .create(if password.is_empty() { "" } else { &password })
        .map_err(|e| e.to_string())?;
    let address = wm.address().unwrap_or("unknown").to_string();
    Ok(CreateWalletResult { address, mnemonic })
}

#[tauri::command]
pub fn wallet_import(phrase: String, password: String) -> Result<WalletInfo, String> {
    let data_dir = wallet_dir();
    let mut wm = libproxybase::WalletManager::new(data_dir).map_err(|e| e.to_string())?;
    wm.import(
        &phrase,
        if password.is_empty() { "" } else { &password },
    )
    .map_err(|e| e.to_string())?;
    Ok(WalletInfo {
        address: wm.address().unwrap_or("unknown").to_string(),
        loaded: true,
    })
}

#[tauri::command]
pub fn wallet_info() -> Result<WalletInfo, String> {
    let data_dir = wallet_dir();
    let mut wm = libproxybase::WalletManager::new(data_dir).map_err(|e| e.to_string())?;
    match wm.load("") {
        Ok(()) => Ok(WalletInfo {
            address: wm.address().unwrap_or("unknown").to_string(),
            loaded: true,
        }),
        Err(_) => Ok(WalletInfo {
            address: String::new(),
            loaded: false,
        }),
    }
}

// ---------------------------------------------------------------------------
// Auth command
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn login(
    backend_url: String,
    password: String,
) -> Result<LoginResult, String> {
    let data_dir = wallet_dir();
    let mut wm = libproxybase::WalletManager::new(data_dir).map_err(|e| e.to_string())?;
    wm.load(if password.is_empty() { "" } else { &password })
        .map_err(|e| format!("Failed to load wallet: {}", e))?;

    let address = wm
        .address()
        .ok_or_else(|| "Wallet not loaded".to_string())?
        .to_string();

    let client = BackendClient::new(&backend_url);

    let challenge = client
        .auth_challenge(&address)
        .await
        .map_err(|e| format!("Auth challenge failed: {}", e))?;

    let message = format!("{}:{}:{}", address, challenge.nonce, challenge.timestamp);
    let signature = wm.sign(message.as_bytes()).map_err(|e| e.to_string())?;
    let sig_hex = hex::encode(&signature);

    let public_key_hex = wm
        .public_key_hex()
        .ok_or_else(|| "Cannot get public key".to_string())?;

    let auth = client
        .auth_verify(&public_key_hex, &challenge.nonce, &challenge.timestamp, &sig_hex)
        .await
        .map_err(|e| format!("Auth verify failed: {}", e))?;

    BackendClient::save_token(&auth.session_token);

    Ok(LoginResult {
        session_token: auth.session_token,
        wallet_address: auth.wallet_address,
        role: auth.role,
        buyer_available: auth.buyer_available,
        spendable_balance: auth.spendable_balance,
    })
}

// ---------------------------------------------------------------------------
// Buyer commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_balance(backend_url: String) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.get_balance())
}

#[tauri::command]
pub async fn transfer(backend_url: String, amount: i64) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.transfer(amount))
}

#[tauri::command]
pub async fn list_currencies(backend_url: String) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.list_currencies())
}

#[tauri::command]
pub async fn create_deposit(
    backend_url: String,
    amount: i64,
    currency: String,
) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.create_deposit(amount, &currency))
}

#[tauri::command]
pub async fn get_deposit(
    backend_url: String,
    deposit_id: String,
) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.get_deposit(&deposit_id))
}

#[tauri::command]
pub async fn list_deposits(backend_url: String) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.list_deposits())
}

// ---------------------------------------------------------------------------
// Seller commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn register_seller(backend_url: String) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.register_seller())
}

#[tauri::command]
pub async fn seller_status(backend_url: String) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.seller_status())
}

#[tauri::command]
pub async fn list_payouts(backend_url: String) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.list_payouts())
}

// ---------------------------------------------------------------------------
// Market commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_countries(backend_url: String) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.list_countries())
}

#[tauri::command]
pub async fn list_pricing(backend_url: String) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.list_pricing())
}

#[tauri::command]
pub async fn create_session(
    backend_url: String,
    country: String,
    network_type: String,
    session_type: String,
    spend_cap: Option<i64>,
) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.create_session(&country, &network_type, &session_type, spend_cap))
}

#[tauri::command]
pub async fn close_session(
    backend_url: String,
    session_id: String,
) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.close_session(&session_id))
}

#[tauri::command]
pub async fn list_sessions(backend_url: String) -> Result<serde_json::Value, String> {
    call_api!(&backend_url, client, client.list_sessions())
}

// ---------------------------------------------------------------------------
// Session token
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_token() -> Result<String, String> {
    let path = dirs::home_dir()
        .unwrap_or_default()
        .join(".proxybase")
        .join("session_token");
    std::fs::read_to_string(&path).map_err(|e| format!("No session token: {}", e))
}

// ---------------------------------------------------------------------------
// Logout
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn logout() -> Result<(), String> {
    let path = dirs::home_dir()
        .unwrap_or_default()
        .join(".proxybase")
        .join("session_token");
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to logout: {}", e))?;
    }
    Ok(())
}