package xyz.proxybase.desktop

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.VpnService
import java.lang.ref.WeakReference

/**
 * Bridges the Tauri Rust commands to VpnService.prepare() / startService().
 *
 * The permission dialog needs an Activity, so MainActivity attaches itself on
 * create and forwards onActivityResult here.
 */
object VpnController {
    init {
        // Tauri normally loads this, but the permission-result callback can be
        // invoked before the service class is touched.
        try {
            System.loadLibrary("proxybase_gui_lib")
        } catch (_: UnsatisfiedLinkError) {
        }
    }

    private const val REQUEST_CODE = 0x5650
    private var activityRef: WeakReference<Activity>? = null
    private var pendingConfig: String? = null

    @JvmStatic
    external fun nativePermissionResult(granted: Boolean)

    @JvmStatic
    external fun nativeAttach(context: Context)

    fun attach(activity: Activity) {
        activityRef = WeakReference(activity)
        // Capture the JVM and application Context for Rust JNI calls made from
        // tokio worker threads. Tauri does not initialise ndk_context, so Rust
        // cannot look them up on its own.
        try {
            nativeAttach(activity.applicationContext ?: activity)
        } catch (_: Throwable) {
            // Native library unavailable; tunnel start will report the error.
        }
    }

    @JvmStatic
    fun startVpn(context: Context, configJson: String): Boolean {
        pendingConfig = configJson
        val activity = activityRef?.get()
            ?: return startService(context, configJson)

        val prepareIntent = VpnService.prepare(activity)
        if (prepareIntent == null) {
            nativePermissionResult(true)
            return startService(context, configJson)
        }
        return try {
            // startVpn is called from a Rust tokio worker; the consent dialog
            // must be launched on the UI thread.
            activity.runOnUiThread {
                try {
                    activity.startActivityForResult(prepareIntent, REQUEST_CODE)
                } catch (_: Exception) {
                    nativePermissionResult(false)
                }
            }
            true
        } catch (_: Exception) {
            nativePermissionResult(false)
            false
        }
    }

    fun onActivityResult(requestCode: Int, resultCode: Int) {
        if (requestCode != REQUEST_CODE) return
        val config = pendingConfig
        pendingConfig = null
        val activity = activityRef?.get() ?: return
        if (resultCode == Activity.RESULT_OK && config != null) {
            nativePermissionResult(true)
            startService(activity, config)
        } else {
            nativePermissionResult(false)
        }
    }

    @JvmStatic
    fun stopVpn(context: Context) {
        try {
            context.stopService(Intent(context, ProxyBaseVpnService::class.java))
        } catch (_: Exception) {
        }
    }

    private fun startService(context: Context, configJson: String): Boolean {
        val intent = Intent(context, ProxyBaseVpnService::class.java).apply {
            action = ProxyBaseVpnService.ACTION_START
            putExtra(ProxyBaseVpnService.EXTRA_CONFIG_JSON, configJson)
        }
        return try {
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
            true
        } catch (_: Exception) {
            false
        }
    }
}
