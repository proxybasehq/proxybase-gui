/// Android foreground service integration.
///
/// Starts/stops a native `SellerForegroundService` that shows a persistent
/// notification, keeping the app process alive while the seller runs.
/// All failures are silently swallowed — the seller runs regardless.

#[cfg(target_os = "android")]
pub fn start_foreground_service() {
    let _ = std::panic::catch_unwind(|| {
        let _ = try_start();
    });
}

#[cfg(target_os = "android")]
pub fn stop_foreground_service() {
    let _ = std::panic::catch_unwind(|| {
        let _ = try_stop();
    });
}

#[cfg(target_os = "android")]
fn try_start() -> Result<(), String> {
    let ctx = ndk_context::android_context();
    let vm =
        unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| format!("JavaVM: {}", e))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {}", e))?;
    let context = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };

    let service_class = env
        .find_class("xyz/proxybase/desktop/SellerForegroundService")
        .map_err(|e| format!("find class: {}", e))?;

    let intent = env
        .new_object(
            "android/content/Intent",
            "(Landroid/content/Context;Ljava/lang/Class;)V",
            &[
                jni::objects::JValue::Object(&context),
                jni::objects::JValue::Object(&service_class),
            ],
        )
        .map_err(|e| format!("Intent: {}", e))?;

    env.call_method(
        &context,
        "startService",
        "(Landroid/content/Intent;)Landroid/content/ComponentName;",
        &[jni::objects::JValue::Object(&intent)],
    )
    .map_err(|e| format!("startService: {}", e))?;

    Ok(())
}

#[cfg(target_os = "android")]
fn try_stop() -> Result<(), String> {
    let ctx = ndk_context::android_context();
    let vm =
        unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| format!("JavaVM: {}", e))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {}", e))?;
    let context = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };

    let service_class = env
        .find_class("xyz/proxybase/desktop/SellerForegroundService")
        .map_err(|e| format!("find class: {}", e))?;

    let intent = env
        .new_object(
            "android/content/Intent",
            "(Landroid/content/Context;Ljava/lang/Class;)V",
            &[
                jni::objects::JValue::Object(&context),
                jni::objects::JValue::Object(&service_class),
            ],
        )
        .map_err(|e| format!("Intent: {}", e))?;

    env.call_method(
        &context,
        "stopService",
        "(Landroid/content/Intent;)Z",
        &[jni::objects::JValue::Object(&intent)],
    )
    .map_err(|e| format!("stopService: {}", e))?;

    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn start_foreground_service() {}

#[cfg(not(target_os = "android"))]
pub fn stop_foreground_service() {}
