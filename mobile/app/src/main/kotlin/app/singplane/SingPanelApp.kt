package app.singplane

import android.app.Application
import android.os.Build
import app.singplane.clash.OkHttpClashClient
import app.singplane.core.AndroidControlPlane
import app.singplane.core.CorePlatform
import app.singplane.core.LibboxCoreProcess
import app.singplane.core.ProcessCoreProcess
import app.singplane.fetch.OkHttpSubscriptionFetcher
import app.singplane.store.ProfileStore
import app.singplane.store.SettingsStore
import app.singplane.store.TemplateStore
import app.singplane.vpn.AndroidVpnSession
import java.io.File

class SingPanelApp : Application() {
    lateinit var controlPlane: AndroidControlPlane
        private set

    val defaultCorePath: String
        get() = File(File(filesDir, "cores"), CorePlatform.binaryFileName("android")).absolutePath

    val androidArch: String
        get() = CorePlatform.androidArch(Build.SUPPORTED_ABIS.firstOrNull() ?: "arm64-v8a")

    override fun onCreate() {
        super.onCreate()
        val clash = OkHttpClashClient()
        val coresDir = File(filesDir, "cores")
        val coreProcess = LibboxCoreProcess(
            context = this,
            onLog = { line ->
                if (this@SingPanelApp::controlPlane.isInitialized) {
                    controlPlane.onCoreLog(line)
                }
            },
        )

        val templateStore = TemplateStore(
            templatesDir = File(filesDir, "templates"),
            builtinReader = { id: String ->
                runCatching {
                    assets.open("templates/$id.json").bufferedReader().use { it.readText() }
                }.getOrNull()
            },
        )

        val settingsStore = SettingsStore(File(filesDir, "settings.json"))
        val currentSettings = settingsStore.load()
        if (currentSettings.corePath.isEmpty()) {
            settingsStore.save(currentSettings.copy(corePath = defaultCorePath))
        }

        controlPlane = AndroidControlPlane(
            profileStore = ProfileStore(File(filesDir, "profiles")),
            settingsStore = settingsStore,
            templateStore = templateStore,
            fetcher = OkHttpSubscriptionFetcher(),
            vpn = AndroidVpnSession(this),
            core = coreProcess,
            coresDir = coresDir,
            arch = androidArch,
            clashClient = clash,
            clashGroups = { clash.groups(it) },
            clashSelect = { base, group, name -> clash.select(base, group, name) },
        )



    }
}


