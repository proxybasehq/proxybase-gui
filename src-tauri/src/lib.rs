mod api;
mod commands;
mod seller;

use seller::SellerState;
use tauri::tray::TrayIconBuilder;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            #[cfg(target_os = "macos")]
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            #[cfg(not(target_os = "macos"))]
            tauri_plugin_autostart::MacosLauncher::default(),
            None::<Vec<&str>>,
        ))
        .manage(SellerState::new())
        .setup(|app| {
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::Arc;

            // Hide from dock on macOS
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let window_visible = Arc::new(AtomicBool::new(false));

            // ---- Tray icon: toggle window on click ----
            let vis = window_visible.clone();
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .show_menu_on_left_click(false)
                .on_tray_icon_event(move |tray, event| {
                    // Keep positioner plugin in sync with tray position
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
                                let _ = window.move_window(Position::TrayBottomCenter);
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)
                .expect("failed to build tray icon");

            // Hide instead of close — so closing the window sends it to tray
            if let Some(window) = app.get_webview_window("main") {
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
            commands::list_deposits,
            commands::get_token,
            commands::logout,
            commands::list_payouts,
            seller::start_seller,
            seller::stop_seller,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
