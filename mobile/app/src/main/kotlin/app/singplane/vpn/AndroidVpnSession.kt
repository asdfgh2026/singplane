package app.singplane.vpn

import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.os.Build
import app.singplane.store.writeAtomically
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

class AndroidVpnSession(private val context: Context) : VpnSession {
    override suspend fun start(runtimeConfig: String) {
        val prepare = VpnService.prepare(context)
        if (prepare != null) throw NeedVpnConsent(prepare)
        val file = File(context.filesDir, "runtime/config.runtime.json")
        file.writeAtomically(runtimeConfig)
        val intent = Intent(context, SingPanelVpnService::class.java)
            .setAction(SingPanelVpnService.ACTION_START)
        SingPanelVpnService.resetReady()
        android.util.Log.i("SingPanel", "starting VpnService")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            context.startForegroundService(intent)
        } else {
            context.startService(intent)
        }
        // SFA/SagerNet: never block the main thread here. VpnService.onStartCommand
        // and Builder.establish() also run on main — a latch on Main deadlocks
        // and times out ("VPN tun 未建立") just before establish finishes.
        val fd = withContext(Dispatchers.IO) { SingPanelVpnService.waitForTunFd() }
        android.util.Log.i("SingPanel", "waitForTunFd=$fd")
        if (fd < 0) {
            error("VPN tun 未建立")
        }
    }

    override suspend fun stop() {
        runCatching {
            context.startService(
                Intent(context, SingPanelVpnService::class.java)
                    .setAction(SingPanelVpnService.ACTION_STOP),
            )
        }
    }

}
