mod api;
mod commands;
mod seller;

use seller::SellerState;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            #[cfg(target_os = "macos")]
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            #[cfg(not(target_os = "macos"))]
            tauri_plugin_autostart::MacosLauncher::default(),
            None::<Vec<&str>>,
        ))
        .manage(SellerState::new())
        .setup(|app| {
            // ---- Tray icon + menu ----
            let show_hide = MenuItemBuilder::with_id("show_hide", "Show/Hide")
                .build(app)
                .expect("failed to create Show/Hide menu item");
            let quit = MenuItemBuilder::with_id("quit", "Quit")
                .build(app)
                .expect("failed to create Quit menu item");
            let menu = MenuBuilder::new(app)
                .item(&show_hide)
                .item(&quit)
                .build()
                .expect("failed to build tray menu");

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show_hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)
                .expect("failed to build tray icon");

            // Hide instead of close — so closing the window sends it to tray
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
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
