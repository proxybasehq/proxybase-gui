use crate::api::BackendClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Emitter;

// ---------------------------------------------------------------------------
// Cached wallet password for silent re-authentication
// ---------------------------------------------------------------------------

/// Stores the last successfully used wallet password so that `reauth()` can
/// load password-protected wallets without user intervention. Set by `login()`
/// on success, cleared by `logout()`.
static WALLET_PASSWORD: std::sync::LazyLock<Mutex<String>> =
    std::sync::LazyLock::new(|| Mutex::new(String::new()));

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

fn require_auth(client: &BackendClient) -> Result<(), String> {
    if !client.is_authenticated() {
        Err("Not authenticated. Please login first.".to_string())
    } else {
        Ok(())
    }
}

/// Silently re-authenticate using the on-disk wallet.
/// Tries the cached password first (set during `login()`), then falls back
/// to an empty password for wallets without encryption.
pub(crate) async fn reauth(backend_url: &str) -> Result<(), String> {
    let data_dir = crate::proxybase_dir();
    let mut wm =
        libproxybase::WalletManager::new(data_dir).map_err(|e| e.to_string())?;

    // Try cached password first, fall back to empty
    let cached_pw = WALLET_PASSWORD.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let passwords_to_try: Vec<&str> = if cached_pw.is_empty() {
        vec![""]
    } else {
        vec![&cached_pw, ""]
    };

    let mut loaded = false;
    for pw in &passwords_to_try {
        if wm.load(pw).is_ok() {
            loaded = true;
            break;
        }
    }
    if !loaded {
        return Err("Failed to load wallet: wrong password or corrupted wallet".to_string());
    }

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
    if let Some(app) = crate::APP_HANDLE.get() {
        let _ = app.emit("auth:token-updated", &auth.session_token);
    }
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
                // Token may be stale (e.g. backend restarted) — re-auth silently if possible
                if reauth($backend_url).await.is_ok() {
                    let $client = BackendClient::new($backend_url);
                    $expr.await.map_err(|e2| e2.to_string())
                } else {
                    Err(_e.to_string())
                }
            }
        }
    }};
}

// ---------------------------------------------------------------------------
// Wallet commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn wallet_create(
    app_handle: tauri::AppHandle,
    password: String,
) -> Result<CreateWalletResult, String> {
    let data_dir = crate::ensure_proxybase_dir(&app_handle);
    let mut wm = libproxybase::WalletManager::new(data_dir).map_err(|e| e.to_string())?;
    let mnemonic = wm
        .create(if password.is_empty() { "" } else { &password })
        .map_err(|e| e.to_string())?;
    let address = wm.address().unwrap_or("unknown").to_string();
    Ok(CreateWalletResult { address, mnemonic })
}

#[tauri::command]
pub fn wallet_import(
    app_handle: tauri::AppHandle,
    phrase: String,
    password: String,
) -> Result<WalletInfo, String> {
    let data_dir = crate::ensure_proxybase_dir(&app_handle);
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
pub fn wallet_info(app_handle: tauri::AppHandle) -> Result<WalletInfo, String> {
    let data_dir = crate::ensure_proxybase_dir(&app_handle);
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
    app_handle: tauri::AppHandle,
    backend_url: String,
    password: String,
) -> Result<LoginResult, String> {
    let data_dir = crate::ensure_proxybase_dir(&app_handle);
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
    let _ = app_handle.emit("auth:token-updated", &auth.session_token);

    // Cache the password for silent re-authentication
    if let Ok(mut cached) = WALLET_PASSWORD.lock() {
        *cached = password.clone();
    }

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

#[tauri::command]
pub async fn keepalive_session(backend_url: String, session_id: String) -> Result<(), String> {
    let client = BackendClient::new(&backend_url);
    match client.keepalive_session(&session_id).await {
        Ok(()) => Ok(()),
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("401") || err_str.contains("unauthorized") {
                // Token expired — reauth and retry once
                let _ = reauth(&backend_url).await;
                let client2 = BackendClient::new(&backend_url);
                client2.keepalive_session(&session_id).await.map_err(|e2| e2.to_string())
            } else {
                Err(err_str)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Session token
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_token(app_handle: tauri::AppHandle) -> Result<String, String> {
    let path = crate::ensure_proxybase_dir(&app_handle).join("session_token");
    std::fs::read_to_string(&path).map_err(|e| format!("No session token: {}", e))
}

// ---------------------------------------------------------------------------
// Logout
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn logout(app_handle: tauri::AppHandle) -> Result<(), String> {
    let path = crate::ensure_proxybase_dir(&app_handle).join("session_token");
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to logout: {}", e))?;
    }
    // Clear cached wallet password
    if let Ok(mut cached) = WALLET_PASSWORD.lock() {
        cached.clear();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// App metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppInfo {
    pub version: String,
    pub git_hash: String,
    pub build_date: String,
    pub os: String,
    pub arch: String,
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_hash: option_env!("BUILD_GIT_HASH").unwrap_or("dev").to_string(),
        build_date: option_env!("BUILD_TIMESTAMP").unwrap_or("local").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}