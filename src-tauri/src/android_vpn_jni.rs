//! Android in-process tunnel entry points.
//!
//! `ProxyBaseVpnService` (Kotlin) creates the TUN interface with
//! `VpnService.Builder`, detaches the raw descriptor and calls
//! `nativeStartTunEngine`. The descriptor is adopted by `tun-rs` and handed to
//! the exact same packet engine used on desktop — no shell routing commands
//! and no sidecar process.

#![cfg(target_os = "android")]

use jni::objects::{GlobalRef, JObject, JString, JValue};
use jni::sys::{jboolean, jint, JNI_FALSE, JNI_TRUE};
use jni::{JNIEnv, JavaVM};
use libproxybase::tunnel::socket_bypass::AndroidSocketProtector;
use libproxybase::tunnel::{SystemTunnelEngine, Telemetry, TunnelConfig, TunnelErrorCode, TunnelState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tauri::Emitter;

static ENGINE: OnceLock<Arc<SystemTunnelEngine>> = OnceLock::new();
static PERMISSION_DENIED: AtomicBool = AtomicBool::new(false);
static KILL_SWITCH: AtomicBool = AtomicBool::new(true);

fn engine() -> Arc<SystemTunnelEngine> {
    ENGINE
        .get_or_init(|| {
            let engine = Arc::new(SystemTunnelEngine::new());
            bridge_events(engine.clone());
            engine
        })
        .clone()
}

fn bridge_events(engine: Arc<SystemTunnelEngine>) {
    let app = crate::app_handle().clone();
    let mut states = engine.subscribe_state();
    let mut telemetry = engine.subscribe_telemetry();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                state = states.recv() => {
                    match state {
                        Ok(state) => {
                            // With the kill switch disabled, a terminal proxy
                            // failure must also tear down the VpnService TUN,
                            // otherwise traffic would still be captured.
                            let terminal = matches!(
                                &state,
                                TunnelState::Error { code, .. }
                                    if matches!(
                                        code,
                                        TunnelErrorCode::QuotaExhausted
                                            | TunnelErrorCode::LeaseExpired
                                            | TunnelErrorCode::ExitVerificationFailed
                                    )
                            );
                            if terminal && !KILL_SWITCH.load(Ordering::SeqCst) {
                                let _ = stop_vpn_service();
                            }
                            let _ = app.emit("tunnel://state-changed", &state);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(_) => continue,
                    }
                }
                sample = telemetry.recv() => {
                    match sample {
                        Ok(t) => {
                            let _ = app.emit("tunnel://telemetry", &serde_json::json!({
                                "rx_bytes_total": t.rx_bytes_total,
                                "tx_bytes_total": t.tx_bytes_total,
                                "rx_bps": t.rx_bps,
                                "tx_bps": t.tx_bps,
                                "active_flows": t.active_flows,
                            }));
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(_) => continue,
                    }
                }
            }
        }
    });
}

struct JniProtector {
    vm: JavaVM,
    service: GlobalRef,
}

impl AndroidSocketProtector for JniProtector {
    fn protect_fd(&self, fd: i32) -> bool {
        let Ok(mut env) = self.vm.attach_current_thread() else {
            return false;
        };
        match env.call_method(
            self.service.as_obj(),
            "protectSocketFromRust",
            "(I)Z",
            &[JValue::Int(fd)],
        ) {
            Ok(value) => value.z().unwrap_or(false),
            Err(e) => {
                eprintln!("[android] VpnService.protect failed: {e}");
                false
            }
        }
    }
}

pub async fn start(config: TunnelConfig) -> Result<TunnelState, String> {
    PERMISSION_DENIED.store(false, Ordering::SeqCst);
    KILL_SWITCH.store(config.kill_switch, Ordering::SeqCst);

    // Switching proxies while the VpnService is active must not establish a
    // second TUN. Hot-swap the endpoint inside the running engine instead.
    let engine = engine();
    let state = engine.state().await;
    if matches!(
        state,
        TunnelState::Connected { .. } | TunnelState::Reconnecting { .. }
    ) {
        engine
            .hot_swap(config)
            .await
            .map_err(|e| e.message)?;
        return Ok(engine.state().await);
    }

    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    start_vpn_service(&json)?;
    Ok(TunnelState::RequestingPrivileges)
}

pub async fn stop() -> Result<TunnelState, String> {
    stop_vpn_service().ok();
    let result = engine().stop().await;
    match result {
        Ok(()) => Ok(TunnelState::Disconnected),
        Err(e) => Err(e.message),
    }
}

pub async fn status() -> Result<TunnelState, String> {
    if PERMISSION_DENIED.load(Ordering::SeqCst) {
        return Ok(TunnelState::Error {
            code: TunnelErrorCode::VpnPermissionDeniedOnAndroid,
            message: "Android VPN permission was denied".into(),
        });
    }
    Ok(engine().state().await)
}

pub async fn telemetry() -> Telemetry {
    engine().telemetry().await
}

fn start_vpn_service(config_json: &str) -> Result<(), String> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let context = unsafe { JObject::from_raw(ctx.context().cast()) };
    let class = env
        .find_class("xyz/proxybase/desktop/VpnController")
        .map_err(|e| format!("VpnController class not found: {e}"))?;
    let config = env.new_string(config_json).map_err(|e| e.to_string())?;
    let result = env
        .call_static_method(
            &class,
            "startVpn",
            "(Landroid/content/Context;Ljava/lang/String;)Z",
            &[
                JValue::Object(&context),
                JValue::Object(&config),
            ],
        )
        .map_err(|e| format!("VpnController.startVpn failed: {e}"))?;
    if result.z().unwrap_or(false) {
        Ok(())
    } else {
        Err("VpnController.startVpn returned false".into())
    }
}

fn stop_vpn_service() -> Result<(), String> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let context = unsafe { JObject::from_raw(ctx.context().cast()) };
    let class = env
        .find_class("xyz/proxybase/desktop/VpnController")
        .map_err(|e| format!("VpnController class not found: {e}"))?;
    env.call_static_method(
        &class,
        "stopVpn",
        "(Landroid/content/Context;)V",
        &[JValue::Object(&context)],
    )
    .map_err(|e| format!("VpnController.stopVpn failed: {e}"))?;
    Ok(())
}

/// Called by `ProxyBaseVpnService` after `VpnService.Builder.establish()`.
#[no_mangle]
pub extern "system" fn Java_xyz_proxybase_desktop_ProxyBaseVpnService_nativeStartTunEngine(
    mut env: JNIEnv,
    service: JObject,
    tun_fd: jint,
    config_json: JString,
) -> jboolean {
    let config_text: String = match env.get_string(&config_json) {
        Ok(text) => text.into(),
        Err(e) => {
            eprintln!("[android] invalid config string: {e}");
            return JNI_FALSE;
        }
    };
    let config: TunnelConfig = match serde_json::from_str(&config_text) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("[android] invalid tunnel config: {e}");
            return JNI_FALSE;
        }
    };
    KILL_SWITCH.store(config.kill_switch, Ordering::SeqCst);

    // Exempt our outbound relay sockets from the VPN interface.
    match (env.get_java_vm(), env.new_global_ref(&service)) {
        (Ok(vm), Ok(global)) => {
            let protector: Arc<dyn AndroidSocketProtector> =
                Arc::new(JniProtector { vm, service: global });
            libproxybase::tunnel::install_global_android_protector(protector);
        }
        (Err(e), _) => eprintln!("[android] JavaVM unavailable: {e}"),
        (_, Err(e)) => eprintln!("[android] global ref failed: {e}"),
    }

    let device = match unsafe {
        libproxybase::tunnel::platform::android::adopt_tun_fd(tun_fd)
    } {
        Ok(device) => device,
        Err(e) => {
            eprintln!("[android] could not adopt TUN fd {tun_fd}: {e}");
            return JNI_FALSE;
        }
    };

    let engine = engine();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = engine.start_with_device(config, device).await {
            eprintln!("[android] tunnel start failed: {e}");
            // Tear down the VpnService so the OS does not keep showing an
            // active VPN whose TUN device has already been dropped.
            let _ = stop_vpn_service();
        }
    });
    JNI_TRUE
}

#[no_mangle]
pub extern "system" fn Java_xyz_proxybase_desktop_ProxyBaseVpnService_nativeStopTunEngine(
    _env: JNIEnv,
    _service: JObject,
) {
    let engine = engine();
    tauri::async_runtime::spawn(async move {
        let _ = engine.stop().await;
    });
}

#[no_mangle]
pub extern "system" fn Java_xyz_proxybase_desktop_VpnController_nativePermissionResult(
    _env: JNIEnv,
    _class: JObject,
    granted: jboolean,
) {
    if granted == JNI_TRUE {
        PERMISSION_DENIED.store(false, Ordering::SeqCst);
    } else {
        PERMISSION_DENIED.store(true, Ordering::SeqCst);
        let app = crate::app_handle().clone();
        let _ = app.emit(
            "tunnel://state-changed",
            &TunnelState::Error {
                code: TunnelErrorCode::VpnPermissionDeniedOnAndroid,
                message: "Android VPN permission was denied".into(),
            },
        );
    }
}
