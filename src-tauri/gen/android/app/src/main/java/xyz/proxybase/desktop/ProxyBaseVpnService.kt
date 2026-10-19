package xyz.proxybase.desktop

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import androidx.core.app.NotificationCompat
import java.math.BigInteger
import java.net.InetAddress

/**
 * Android VPN service. The TUN interface is created here by the OS API and the
 * detached descriptor is handed to the Rust packet engine via JNI. Android
 * forbids shell routing commands and sidecar daemons, so this is the only
 * supported path on this platform.
 */
class ProxyBaseVpnService : VpnService() {
    companion object {
        const val ACTION_START = "xyz.proxybase.desktop.START_VPN"
        const val ACTION_STOP = "xyz.proxybase.desktop.STOP_VPN"
        const val EXTRA_CONFIG_JSON = "config_json"
        private const val CHANNEL_ID = "proxybase_vpn_channel"
        private const val NOTIFICATION_ID = 4201

        init {
            System.loadLibrary("proxybase_gui_lib")
        }
    }

    private var vpnInterface: ParcelFileDescriptor? = null

    private external fun nativeStartTunEngine(tunFd: Int, configJson: String): Boolean
    private external fun nativeStopTunEngine()

    /** Called from Rust before every outbound relay socket is connected. */
    @Suppress("unused")
    fun protectSocketFromRust(socketFd: Int): Boolean = protect(socketFd)

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                stopVpnTunnel()
                return START_NOT_STICKY
            }
            ACTION_START -> {
                val configJson = intent.getStringExtra(EXTRA_CONFIG_JSON)
                    ?: return START_NOT_STICKY
                startForeground(NOTIFICATION_ID, buildNotification("Connecting to ProxyBase…"))
                establishAndRunTunnel(configJson)
                return START_REDELIVER_INTENT
            }
        }
        return START_NOT_STICKY
    }

    private fun establishAndRunTunnel(configJson: String) {
        val builder = Builder()
            .setSession("ProxyBase System Tunnel")
            .setMtu(1380)
            .addAddress("10.255.0.2", 30)
            .addAddress("fd00:255::2", 64)
            .addDnsServer("10.255.0.1")

        // Android's VpnService has no "bypass" concept below API 33, so the
        // route set is the complement of the private/link-local ranges we must
        // keep on the physical network. Never add 0.0.0.0/0 or ::/0.
        val excludedV4 = listOf(
            "127.0.0.0/8",
            "10.0.0.0/8",
            "172.16.0.0/12",
            "192.168.0.0/16",
            "100.64.0.0/10",
            "169.254.0.0/16",
            "224.0.0.0/4",
            "255.255.255.255/32",
        )
        val excludedV6 = listOf("::1/128", "fe80::/10", "ff00::/8")
        for ((network, prefix) in complementRoutes(32, excludedV4)) {
            builder.addRoute(network, prefix)
        }
        for ((network, prefix) in complementRoutes(128, excludedV6)) {
            builder.addRoute(network, prefix)
        }
        // 10.0.0.0/8 is excluded above; the virtual network and its DNS
        // listener live inside it, so re-include that specific subnet.
        builder.addRoute("10.255.0.0", 30)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            builder.setMetered(false)
        }
        // Exclude the app's own relay sockets from the VPN. The local bridge
        // must reach the ProxyBase relay through the physical network, not
        // back into this TUN device. `protectSocketFromRust` is the
        // per-descriptor defence in depth.
        try {
            builder.addDisallowedApplication(packageName)
        } catch (_: Exception) {
        }

        val pfd = try {
            builder.establish()
        } catch (_: Exception) {
            VpnController.nativePermissionResult(false)
            stopSelf()
            return
        } ?: run {
            VpnController.nativePermissionResult(false)
            stopSelf()
            return
        }

        // detachFd transfers ownership of the descriptor to Rust, which closes
        // it when the engine stops.
        val rawFd = pfd.detachFd()
        vpnInterface = null

        if (!nativeStartTunEngine(rawFd, configJson)) {
            // Rust did not adopt the descriptor; close it ourselves so the
            // VPN interface does not linger.
            try {
                ParcelFileDescriptor.adoptFd(rawFd).close()
            } catch (_: Exception) {
            }
            stopSelf()
        } else {
            val manager = getSystemService(NotificationManager::class.java)
            manager.notify(NOTIFICATION_ID, buildNotification("System tunnel active"))
        }
    }

    private fun stopVpnTunnel() {
        nativeStopTunEngine()
        vpnInterface?.close()
        vpnInterface = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onRevoke() {
        // The user revoked VPN permission or another VPN app took over.
        stopVpnTunnel()
        super.onRevoke()
    }

    override fun onDestroy() {
        nativeStopTunEngine()
        super.onDestroy()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "ProxyBase VPN Status",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Shown while the ProxyBase system tunnel is active"
            }
            getSystemService(NotificationManager::class.java)
                .createNotificationChannel(channel)
        }
    }

    private fun buildNotification(statusText: String): Notification {
        val stopIntent = Intent(this, ProxyBaseVpnService::class.java).apply {
            action = ACTION_STOP
        }
        val stopPendingIntent = PendingIntent.getService(
            this,
            0,
            stopIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val openApp = PendingIntent.getActivity(
            this,
            1,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("ProxyBase Tunnel")
            .setContentText(statusText)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentIntent(openApp)
            .setOngoing(true)
            .addAction(R.mipmap.ic_launcher, "Disconnect", stopPendingIntent)
            .build()
    }
}

/**
 * Compute the complement of [excluded] within the whole IPv4/IPv6 space as a
 * minimal list of CIDR blocks. Used because VpnService.Builder can only add
 * routes; it cannot subtract them (before API 33).
 */
private fun complementRoutes(
    bits: Int,
    excluded: List<String>
): List<Pair<InetAddress, Int>> {
    val addressSize = bits / 8
    val total = BigInteger.ONE.shiftLeft(bits)
    data class Range(val start: BigInteger, val end: BigInteger)

    val ranges = excluded.map { cidr ->
        val (host, prefixText) = cidr.split("/", limit = 2)
        val prefix = prefixText.toInt()
        val raw = InetAddress.getByName(host).address
        val value = BigInteger(1, raw)
        val size = BigInteger.ONE.shiftLeft(bits - prefix)
        Range(value and size.subtract(BigInteger.ONE).not().and(total.subtract(BigInteger.ONE)), value.add(size).subtract(BigInteger.ONE))
    }.sortedBy { it.start }

    val result = mutableListOf<Pair<InetAddress, Int>>()
    fun addRange(startIn: BigInteger, endIn: BigInteger) {
        var start = startIn
        val end = endIn
        while (start <= end) {
            val align = if (start == BigInteger.ZERO) bits else start.lowestSetBit
            val remaining = end.subtract(start).add(BigInteger.ONE)
            val byRemaining = remaining.bitLength() - 1
            val sizeExp = minOf(align, byRemaining, bits)
            val prefix = bits - sizeExp
            if (prefix == 0) {
                // Never install a default route: split it into the two
                // sub-default halves used on desktop as well.
                val half = BigInteger.ONE.shiftLeft(bits - 1)
                result.add(InetAddress.getByAddress(toBytes(start, addressSize)) to 1)
                result.add(InetAddress.getByAddress(toBytes(start.add(half), addressSize)) to 1)
            } else {
                result.add(InetAddress.getByAddress(toBytes(start, addressSize)) to prefix)
            }
            start = start.add(BigInteger.ONE.shiftLeft(sizeExp))
        }
    }

    var cursor = BigInteger.ZERO
    for (range in ranges) {
        if (range.start > cursor) addRange(cursor, range.start.subtract(BigInteger.ONE))
        if (range.end.add(BigInteger.ONE) > cursor) cursor = range.end.add(BigInteger.ONE)
    }
    if (cursor < total) addRange(cursor, total.subtract(BigInteger.ONE))
    return result
}

private fun toBytes(value: BigInteger, size: Int): ByteArray {
    val raw = value.toByteArray()
    val out = ByteArray(size)
    if (raw.size <= size) {
        System.arraycopy(raw, 0, out, size - raw.size, raw.size)
    } else {
        System.arraycopy(raw, raw.size - size, out, 0, size)
    }
    return out
}
