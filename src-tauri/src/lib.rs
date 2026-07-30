mod api;
pub mod bridge;
mod commands;
pub mod seller;

use seller::SellerState;
use std::path::PathBuf;
use std::sync::OnceLock;

#[cfg(desktop)]
use tauri::tray::TrayIconBuilder;
use tauri::Manager;

static PROXYBASE_DIR: OnceLock<PathBuf> = OnceLock::new();
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Store a global AppHandle so background tasks can emit Tauri events
/// without threading `AppHandle` through every call chain.
pub fn app_handle() -> &'static tauri::AppHandle {
    APP_HANDLE.get().expect("APP_HANDLE not initialised")
}

/// Ensure the OnceLock is set (first call wins). Returns the resolved path.
pub fn ensure_proxybase_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    let dir = app_handle
        .path()
        .app_data_dir()
        .or_else(|_| app_handle.path().app_config_dir())
        .unwrap_or_default()
        .join(".proxybase");
    let _ = PROXYBASE_DIR.set(dir);
    PROXYBASE_DIR.get().cloned().unwrap()
}

/// Get the proxybase data directory. On desktop falls back to `~/.proxybase`
/// if not yet initialised; on mobile, `ensure_proxybase_dir` must be called first.
pub fn proxybase_dir() -> PathBuf {
    PROXYBASE_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".proxybase"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let builder = tauri::Builder::default();

    let builder = builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build());

    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            #[cfg(target_os = "macos")]
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            #[cfg(not(target_os = "macos"))]
            tauri_plugin_autostart::MacosLauncher::default(),
            None::<Vec<&str>>,
        ));

    builder
        .manage(SellerState::new())
        .setup(|_app| {
            let _ = APP_HANDLE.set(_app.handle().clone());

            // Hide from dock on macOS
            #[cfg(target_os = "macos")]
            _app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(desktop)]
            {
                use std::sync::atomic::{AtomicBool, Ordering};
                use std::sync::Arc;

                let window_visible = Arc::new(AtomicBool::new(false));

                if let Some(icon) = _app.default_window_icon() {
                    let vis = window_visible.clone();
                    let icon_cloned = icon.clone();
                    let _ = TrayIconBuilder::new()
                        .icon(icon_cloned)
                        .show_menu_on_left_click(false)
                        .on_tray_icon_event(move |tray, event| {
                            let app = tray.app_handle();
                            tauri_plugin_positioner::on_tray_event(app, &event);
                            if let tauri::tray::TrayIconEvent::Click { button_state, .. } = event {
                                if button_state != tauri::tray::MouseButtonState::Up {
                                    return;
                                }
                                if let Some(window) = app.get_webview_window("main") {
                                    if vis.fetch_xor(true, Ordering::SeqCst) {
                                        let _ = window.hide();
                                    } else {
                                        use tauri_plugin_positioner::{Position, WindowExt};
                                        #[cfg(target_os = "macos")]
                                        let _ = window.move_window_constrained(Position::TrayBottomCenter);
                                        #[cfg(not(target_os = "macos"))]
                                        let _ = window.move_window_constrained(Position::TrayCenter);
                                        let _ = window.show();
                                        let _ = window.set_focus();
                                    }
                                }
                            }
                        })
                        .build(_app);
                }

                // Hide instead of close — so closing the window sends it to tray
                if let Some(window) = _app.get_webview_window("main") {
                    let vis = window_visible.clone();
                    let w = window.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            vis.store(false, Ordering::SeqCst);
                            let _ = w.hide();
                            api.prevent_close();
                        }
                    });
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::wallet_create,
            commands::wallet_import,
            commands::wallet_info,
            commands::login,
            commands::get_balance,
            commands::transfer,
            commands::list_currencies,
            commands::create_deposit,
            commands::get_deposit,
            commands::register_seller,
            commands::seller_status,
            commands::list_countries,
            commands::list_pricing,
            commands::create_session,
            commands::close_session,
            commands::list_sessions,
            commands::keepalive_session,
            commands::list_deposits,
            commands::get_token,
            commands::logout,
            commands::list_payouts,
            seller::start_seller,
            seller::stop_seller,
            bridge::bridge_start,
            bridge::bridge_stop,
            bridge::bridge_port,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
