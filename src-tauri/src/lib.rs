mod api;
pub mod bridge;
mod commands;
#[path = "commands/tunnel.rs"]
mod tunnel_commands;
#[cfg(desktop)]
pub mod tunnel_client;
#[cfg(target_os = "android")]
mod android_vpn_jni;
pub mod seller;

#[cfg(desktop)]
use libproxybase::tunnel::TunnelState;
use seller::SellerState;
use std::path::PathBuf;
use std::sync::OnceLock;

#[cfg(desktop)]
use tauri::menu::{MenuBuilder, MenuItemBuilder};
#[cfg(desktop)]
use tauri::tray::TrayIconBuilder;
use tauri::Manager;

static PROXYBASE_DIR: OnceLock<PathBuf> = OnceLock::new();
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Tracks the last frontend heartbeat so a wedged webview (e.g. after system
/// hibernation) can be auto-recovered by reloading the window.
pub struct UiWatchdog {
    pub last_heartbeat: std::sync::Mutex<std::time::Instant>,
}

impl UiWatchdog {
    pub fn new() -> Self {
        Self {
            last_heartbeat: std::sync::Mutex::new(std::time::Instant::now()),
        }
    }
}

/// Store a global AppHandle so background tasks can emit Tauri events
/// without threading `AppHandle` through every call chain.
pub fn app_handle() -> &'static tauri::AppHandle {
    APP_HANDLE.get().expect("APP_HANDLE not initialised")
}

/// Non-panicking variant for paths that can run before Tauri's setup, e.g. a
/// VpnService restarted by Android in a fresh process after a crash.
pub fn app_handle_opt() -> Option<&'static tauri::AppHandle> {
    APP_HANDLE.get()
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

/// Draw a green rounded border around the tray icon while the tunnel is
/// active. The icon is generated at runtime so no extra image assets (and no
/// platform-specific icon variants) are needed.
#[cfg(desktop)]
fn tunnel_bordered_icon(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = rgba.to_vec();
    let w = width as i64;
    let h = height as i64;
    if w < 12 || h < 12 || out.len() < (width as usize * height as usize * 4) {
        return out;
    }

    let border = (w.min(h) / 10).clamp(2, 12);
    let radius = (w.min(h) / 5).max(border + 2);

    let inside = |x: i64, y: i64, inset: i64, r: i64| -> bool {
        let left = inset;
        let top = inset;
        let right = w - 1 - inset;
        let bottom = h - 1 - inset;
        if x < left || x > right || y < top || y > bottom {
            return false;
        }
        let r = r.max(0);
        let cx = x.clamp(left + r, right - r);
        let cy = y.clamp(top + r, bottom - r);
        let dx = x - cx;
        let dy = y - cy;
        dx * dx + dy * dy <= r * r
    };

    // The same green used by the in-app "tunnel active" accent.
    let color = [34u8, 197, 94, 255];
    for y in 0..h {
        for x in 0..w {
            if inside(x, y, 0, radius) && !inside(x, y, border, radius - border) {
                let idx = ((y as u32 * width + x as u32) * 4) as usize;
                if idx + 3 < out.len() {
                    out[idx..idx + 4].copy_from_slice(&color);
                }
            }
        }
    }
    out
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let builder = tauri::Builder::default();

    let builder = builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
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
        ))
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

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
                    // Generate the tunnel-mode icon once: a green rounded ring
                    // around the normal tray icon. Switching to it is a cheap
                    // `set_icon` call, no assets are required.
                    let base_rgba = icon.rgba().to_vec();
                    let icon_width = icon.width();
                    let icon_height = icon.height();
                    let active_rgba =
                        tunnel_bordered_icon(&base_rgba, icon_width, icon_height);

                    let tunnel_status = MenuItemBuilder::with_id(
                        "tunnel_status",
                        "System tunnel: disconnected",
                    )
                    .enabled(false)
                    .build(_app)?;
                    let tunnel_disconnect = MenuItemBuilder::with_id(
                        "tunnel_disconnect",
                        "Disconnect System Tunnel",
                    )
                    .build(_app)?;
                    let show_item =
                        MenuItemBuilder::with_id("show_window", "Show ProxyBase").build(_app)?;
                    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(_app)?;
                    let tray_menu = MenuBuilder::new(_app)
                        .items(&[&tunnel_status, &tunnel_disconnect])
                        .separator()
                        .items(&[&show_item, &quit_item])
                        .build()?;

                    let tray = TrayIconBuilder::new()
                        .icon(icon_cloned)
                        .icon_as_template(false)
                        .tooltip("ProxyBase")
                        .menu(&tray_menu)
                        .show_menu_on_left_click(false)
                        .on_menu_event(|app, event| match event.id().as_ref() {
                            "show_window" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            "tunnel_disconnect" => {
                                tauri::async_runtime::spawn(async move {
                                    let _ = tunnel_client::stop().await;
                                });
                            }
                            "quit" => app.exit(0),
                            _ => {}
                        })
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
                        .build(_app)
                        .ok();

                    // Keep the tray label and the tunnel border in sync without
                    // waking the daemon more than necessary.
                    {
                        let label = tunnel_status.clone();
                        let tray = tray.clone();
                        tauri::async_runtime::spawn(async move {
                            let base_icon = (base_rgba, icon_width, icon_height);
                            let active_icon = (active_rgba, icon_width, icon_height);
                            let mut last_active: Option<bool> = None;
                            loop {
                                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                let state = tunnel_client::status()
                                    .await
                                    .unwrap_or(TunnelState::Disconnected);
                                let active = state.is_active();
                                let text = match state {
                                    TunnelState::Disconnected => {
                                        "System tunnel: disconnected".to_string()
                                    }
                                    TunnelState::Connected { exit_ip, .. } => {
                                        format!("System tunnel: connected ({exit_ip})")
                                    }
                                    TunnelState::Error { message, .. } => {
                                        format!("System tunnel: error — {message}")
                                    }
                                    _ => "System tunnel: connecting…".to_string(),
                                };
                                let _ = label.set_text(text);

                                if last_active != Some(active) {
                                    if let Some(tray) = &tray {
                                        let icon = if active { &active_icon } else { &base_icon };
                                        let _ = tray.set_icon(Some(tauri::image::Image::new(
                                            &icon.0, icon.1, icon.2,
                                        )));
                                        let _ = tray.set_tooltip(Some(if active {
                                            "ProxyBase — system tunnel active"
                                        } else {
                                            "ProxyBase"
                                        }));
                                    }
                                    last_active = Some(active);
                                }
                            }
                        });
                    }
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

            // UI watchdog: the frontend reports a heartbeat every 10s. If the
            // webview stops responding while its window is visible (a known
            // failure after system sleep/hibernation), reload it automatically
            // instead of forcing the user to kill and restart the process.
            #[cfg(desktop)]
            tunnel_client::init(_app.handle().clone());

            _app.manage(UiWatchdog::new());
            {
                let wd_app = _app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                        let stale = wd_app
                            .state::<UiWatchdog>()
                            .last_heartbeat
                            .lock()
                            .map(|g| g.elapsed())
                            .unwrap_or_default();
                        if stale > std::time::Duration::from_secs(120) {
                            if let Some(window) = wd_app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    eprintln!(
                                        "[ui-watchdog] UI unresponsive for {:.0}s — reloading webview",
                                        stale.as_secs_f64()
                                    );
                                    let _ = window.eval("window.location.reload()");
                                    if let Ok(mut g) =
                                        wd_app.state::<UiWatchdog>().last_heartbeat.lock()
                                    {
                                        *g = std::time::Instant::now();
                                    }
                                }
                            }
                        }
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
            commands::country_name,
            commands::country_code,
            commands::country_flag,
            commands::get_all_countries,
            commands::get_all_country_metadata,
            commands::create_session,
            commands::close_session,
            commands::rotate_session,
            commands::list_sessions,
            commands::keepalive_session,
            commands::list_deposits,
            commands::get_token,
            commands::logout,
            commands::list_payouts,
            commands::get_app_info,
            commands::ui_heartbeat,
            seller::start_seller,
            seller::stop_seller,
            bridge::bridge_start,
            bridge::bridge_stop,
            bridge::bridge_port,
            tunnel_commands::enable_system_tunnel,
            tunnel_commands::disable_system_tunnel,
            tunnel_commands::get_system_tunnel_status,
            tunnel_commands::get_tunnel_telemetry,
            tunnel_commands::get_tunnel_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(all(test, desktop))]
mod tray_icon_tests {
    #[test]
    fn tunnel_border_draws_a_green_ring_and_leaves_the_center_untouched() {
        let (w, h) = (32u32, 32u32);
        let rgba = vec![0u8; (w * h * 4) as usize];
        let out = super::tunnel_bordered_icon(&rgba, w, h);
        let green = [34u8, 197, 94, 255];

        // Middle of the left edge is on the ring.
        let edge = ((16 * w) * 4) as usize;
        assert_eq!(&out[edge..edge + 4], &green);

        // The center is untouched (still transparent).
        let center = ((16 * w + 16) * 4) as usize;
        assert_eq!(&out[center..center + 4], &[0, 0, 0, 0]);

        // The rounded top-left corner is outside the border.
        assert_eq!(&out[0..4], &[0, 0, 0, 0]);
    }
}
