package app.singplane.vpn

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import app.singplane.MainActivity
import app.singplane.R
import app.singplane.SingPanelApp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * Android VPN service handling TUN establish, protection, and foreground notification.
 */
class SingPanelVpnService : VpnService() {
    private var tun: ParcelFileDescriptor? = null

    override fun onCreate() {
        super.onCreate()
        instance = this
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        instance = this
        if (intent?.action == ACTION_STOP) {
            closeTun()
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
            return START_NOT_STICKY
        }



        ensureChannel()
        val notif = notification()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIF_ID, notif, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIF_ID, notif)
        }

        establishTun()
        return START_STICKY
    }

    private fun establishTun() {
        var detachedFd = -1
        try {
            val runtimeFile = java.io.File(filesDir, "runtime/config.runtime.json")
            val runtimeJson = if (runtimeFile.exists()) runtimeFile.readText() else ""
            val params = if (runtimeJson.isNotBlank()) {
                VpnParams.fromSingBoxJson(runtimeJson, packageName = packageName)
            } else {
                VpnParams(packageName = packageName)
            }
            val builder = params.applyTo(Builder())
            // establish()?.detachFd() then hand the raw fd to the core.
            // Keeping ParcelFileDescriptor in Java while libbox also uses .fd
            // produces EBADF / "query tun name: bad file descriptor".
            val pfd = builder.establish()
            val fd = pfd?.detachFd() ?: -1
            detachedFd = fd
            tun = null
            android.util.Log.i("SingPanel", "establishTun detachFd=$fd")
            if (fd >= 0) {
                readyFd.set(fd)
                readyLatch.get()?.countDown()
            } else {
                android.util.Log.e("SingPanel", "establish() returned null")
            }
        } catch (e: Exception) {
            android.util.Log.e("SingPanel", "establishTun failed", e)
            if (detachedFd >= 0 && readyFd.get() != detachedFd) {
                closeFd(detachedFd)
            }
            closeTun()
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
        }
    }

    private fun closeTun() {
        // If readyFd was not yet consumed by libbox, close it now to prevent leak.
        val unconsumed = readyFd.getAndSet(-1)
        if (unconsumed >= 0) {
            closeFd(unconsumed)
        }
        runCatching { tun?.close() }
        tun = null
    }

    override fun onDestroy() {
        closeTun()
        if (instance == this) instance = null
        super.onDestroy()
    }


    private fun ensureChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val mgr = getSystemService(NotificationManager::class.java)
        if (mgr?.getNotificationChannel(CHANNEL_ID) == null) {
            mgr?.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    getString(R.string.vpn_notification_channel),
                    NotificationManager.IMPORTANCE_LOW,
                ),
            )
        }
    }

    private fun notification(): Notification {
        val openIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val openPendingIntent = PendingIntent.getActivity(
            this,
            0,
            openIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        val stopIntent = Intent(this, SingPanelVpnService::class.java).apply {
            action = ACTION_STOP
        }
        val stopPendingIntent = PendingIntent.getService(
            this,
            1,
            stopIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }

        return builder
            .setSmallIcon(android.R.drawable.stat_sys_warning)
            .setContentTitle(getString(R.string.vpn_notification_title))
            .setContentText(getString(R.string.vpn_notification_text))
            .setContentIntent(openPendingIntent)
            .addAction(
                Notification.Action.Builder(
                    null,
                    "停止",
                    stopPendingIntent,
                ).build(),
            )
            .setOngoing(true)
            .build()
    }

    companion object {
        const val CHANNEL_ID = "singpanel.core"
        const val NOTIF_ID = 1001
        const val ACTION_START = "app.singplane.vpn.START"
        const val ACTION_STOP = "app.singplane.vpn.STOP"

        @Volatile
        var instance: SingPanelVpnService? = null
            internal set

        private val readyLatch = AtomicReference(CountDownLatch(1))
        private val readyFd = AtomicReference(-1)

        fun closeFd(fd: Int) {
            if (fd >= 0) {
                runCatching {
                    ParcelFileDescriptor.adoptFd(fd).close()
                }
            }
        }

        fun resetReady() {
            val unconsumed = readyFd.getAndSet(-1)
            closeFd(unconsumed)
            readyLatch.set(CountDownLatch(1))
        }

        fun waitForTunFd(timeoutMs: Long = 8000): Int {
            val existing = instance?.tunFd() ?: readyFd.get()
            if (existing >= 0) return existing
            runCatching { readyLatch.get()?.await(timeoutMs, TimeUnit.MILLISECONDS) }
            return instance?.tunFd() ?: readyFd.get()
        }

        fun consumeTunFd(timeoutMs: Long = 8000): Int {
            var fd = readyFd.getAndSet(-1)
            if (fd >= 0) return fd
            runCatching { readyLatch.get()?.await(timeoutMs, TimeUnit.MILLISECONDS) }
            return readyFd.getAndSet(-1)
        }

        fun currentTunFd(): Int = instance?.tunFd() ?: readyFd.get()
    }

    fun tunFd(): Int = readyFd.get().takeIf { it >= 0 } ?: (tun?.fd ?: -1)

}
