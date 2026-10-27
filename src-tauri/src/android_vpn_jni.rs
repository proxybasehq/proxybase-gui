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
use libproxybase::tunnel::{
    SystemTunnelEngine, Telemetry, TunnelConfig, TunnelErrorCode, TunnelState,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tauri::Emitter;

static ENGINE: OnceLock<Arc<SystemTunnelEngine>> = OnceLock::new();
static PERMISSION_DENIED: AtomicBool = AtomicBool::new(false);
static KILL_SWITCH: AtomicBool = AtomicBool::new(true);

// Tauri's Android glue never initialises `ndk_context` (that is an
// ndk-glue/tao convention), so the JVM and application context are captured
// explicitly from `VpnController.attach()` on the Android main thread. Calls
// from tokio workers later on cannot use `ndk_context::android_context()` —
// it panics with "android context was not initialized" and leaves the JS
// `enableSystemTunnel` promise pending forever (the spinner).
static ANDROID_VM: OnceLock<JavaVM> = OnceLock::new();
static ANDROID_CONTEXT: OnceLock<GlobalRef> = OnceLock::new();
static VPN_CONTROLLER_CLASS: OnceLock<GlobalRef> = OnceLock::new();

/// Capture the JVM, application context (and the `VpnController` class) from a
/// thread that has the correct Android class loader — normally the activity's
/// main thread. Idempotent; later calls are ignored.
fn remember_android_env(env: &mut JNIEnv, context: Option<&JObject>) {
    if ANDROID_VM.get().is_none() {
        if let Ok(vm) = env.get_java_vm() {
            let _ = ANDROID_VM.set(vm);
        }
    }
    if ANDROID_CONTEXT.get().is_none() {
        if let Some(context) = context {
            if let Ok(global) = env.new_global_ref(context) {
                if ANDROID_CONTEXT.set(global).is_ok() {
                    eprintln!("[android] JNI bridge attached to the application context");
                }
            }
        }
    }
    if VPN_CONTROLLER_CLASS.get().is_none() {
        if let Ok(class) = env.find_class("xyz/proxybase/desktop/VpnController") {
            if let Ok(global) = env.new_global_ref(class) {
                let _ = VPN_CONTROLLER_CLASS.set(global);
            }
        }
    }
}

fn vpn_controller_class(env: &mut JNIEnv) -> Result<&'static GlobalRef, String> {
    if let Some(class) = VPN_CONTROLLER_CLASS.get() {
        return Ok(class);
    }
    // Only reachable if Kotlin never called `nativeAttach`, e.g. a VpnService
    // restarted in a fresh process.
    let class = env
        .find_class("xyz/proxybase/desktop/VpnController")
        .map_err(|e| format!("VpnController class not found: {e}"))?;
    let global = env.new_global_ref(class).map_err(|e| e.to_string())?;
    let _ = VPN_CONTROLLER_CLASS.set(global);
    VPN_CONTROLLER_CLASS
        .get()
        .ok_or_else(|| "VpnController class unavailable".to_string())
}

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
    // Android can restart the VpnService in a fresh process before Tauri's
    // setup has run; the engine must still start, just without UI events.
    let Some(app) = crate::app_handle_opt().cloned() else {
        eprintln!("[android] Tauri app handle not initialised; tunnel events disabled");
        return;
    };
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
        engine.hot_swap(config).await.map_err(|e| e.message)?;
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
    let not_ready = || "the Android application context is not ready yet".to_string();
    let vm = ANDROID_VM.get().ok_or_else(not_ready)?;
    let context = ANDROID_CONTEXT.get().ok_or_else(not_ready)?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let class = vpn_controller_class(&mut env)?;
    let config = env.new_string(config_json).map_err(|e| e.to_string())?;
    let result = env
        .call_static_method(
            class,
            "startVpn",
            "(Landroid/content/Context;Ljava/lang/String;)Z",
            &[JValue::Object(context.as_obj()), JValue::Object(&config)],
        )
        .map_err(|e| format!("VpnController.startVpn failed: {e}"))?;
    if result.z().unwrap_or(false) {
        Ok(())
    } else {
        Err("VpnController.startVpn returned false".into())
    }
}

fn stop_vpn_service() -> Result<(), String> {
    let not_ready = || "the Android application context is not ready yet".to_string();
    let vm = ANDROID_VM.get().ok_or_else(not_ready)?;
    let context = ANDROID_CONTEXT.get().ok_or_else(not_ready)?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let class = vpn_controller_class(&mut env)?;
    env.call_static_method(
        class,
        "stopVpn",
        "(Landroid/content/Context;)V",
        &[JValue::Object(context.as_obj())],
    )
    .map_err(|e| format!("VpnController.stopVpn failed: {e}"))?;
    Ok(())
}

/// Report a startup failure that happened before the engine had a chance to
/// publish a state of its own (e.g. the VpnService descriptor could not be
/// adopted). Without this the UI would only see the toggle flip back with no
/// explanation; Rust's stderr is not visible in logcat on Android.
fn emit_start_error(code: TunnelErrorCode, message: impl Into<String>) {
    let state = TunnelState::Error {
        code,
        message: message.into(),
    };
    if let Some(app) = crate::app_handle_opt() {
        let _ = app.emit("tunnel://state-changed", &state);
    }
}

/// Called by `ProxyBaseVpnService` after `VpnService.Builder.establish()`.
#[no_mangle]
pub extern "system" fn Java_xyz_proxybase_desktop_ProxyBaseVpnService_nativeStartTunEngine(
    mut env: JNIEnv,
    service: JObject,
    tun_fd: jint,
    config_json: JString,
) -> jboolean {
    // Seeds the JVM/class cache if `nativeAttach` has not run (the OS can
    // restart the VpnService in a fresh process). The context is not captured
    // here: the service instance would be leaked by a static global ref.
    remember_android_env(&mut env, None);
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
            let protector: Arc<dyn AndroidSocketProtector> = Arc::new(JniProtector {
                vm,
                service: global,
            });
            libproxybase::tunnel::install_global_android_protector(protector);
        }
        (Err(e), _) => eprintln!("[android] JavaVM unavailable: {e}"),
        (_, Err(e)) => eprintln!("[android] global ref failed: {e}"),
    }

    // `AsyncDevice::from_fd` registers the descriptor with tokio's reactor.
    // That panics when no runtime is entered, and this JNI callback runs on
    // Android's main thread, not a tokio worker. Adopt the fd inside the
    // runtime instead; on failure the fd has already been closed by the
    // dropped device wrapper, so only the VpnService needs tearing down.
    let engine = engine();
    tauri::async_runtime::spawn(async move {
        let device = match unsafe { libproxybase::tunnel::platform::android::adopt_tun_fd(tun_fd) }
        {
            Ok(device) => device,
            Err(e) => {
                let message = format!("could not adopt the Android TUN descriptor: {e}");
                eprintln!("[android] {message}");
                emit_start_error(TunnelErrorCode::TunDeviceCreationFailed, message);
                let _ = stop_vpn_service();
                return;
            }
        };
        if let Err(e) = engine.start_with_device(config, device).await {
            eprintln!("[android] tunnel start failed: {}", e.message);
            // The engine usually publishes the error itself, but the event
            // bridge is skipped when the VpnService was restarted before
            // Tauri's setup, so emit once here as well.
            emit_start_error(e.code, e.message);
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

/// Called by `VpnController.attach()` from `MainActivity.onCreate`. Captures
/// the JVM and application context on the main thread, where the app class
/// loader is guaranteed to be available. Without this, JNI calls from tokio
/// workers panic in `ndk_context` and the tunnel toggle spins forever.
#[no_mangle]
pub extern "system" fn Java_xyz_proxybase_desktop_VpnController_nativeAttach(
    mut env: JNIEnv,
    _class: JObject,
    context: JObject,
) {
    remember_android_env(&mut env, Some(&context));
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
        if let Some(app) = crate::app_handle_opt() {
            let _ = app.emit(
                "tunnel://state-changed",
                &TunnelState::Error {
                    code: TunnelErrorCode::VpnPermissionDeniedOnAndroid,
                    message: "Android VPN permission was denied".into(),
                },
            );
        }
    }
}
