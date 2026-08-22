package app.singplane.core

import app.singplane.clash.GroupWithNodes
import app.singplane.clash.ProxyGroup
import app.singplane.model.AppSettings
import app.singplane.model.Profile
import app.singplane.model.Template
import kotlinx.coroutines.flow.StateFlow

enum class CorePhase {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

data class CoreSnapshot(
    val phase: CorePhase = CorePhase.Stopped,
    val message: String = "已停止",
    val viaVpn: Boolean = false,
    val activeProfileName: String? = null,
    val startedAtMs: Long = 0L,
) {
    val running: Boolean get() = phase == CorePhase.Running
}


/**
 * Android control API.
 *
 * This track is **Kotlin-only**. A later Rust library can implement the same
 * interface; do not call UniFFI from UI.
 */
interface ControlPlane {
    val status: StateFlow<CoreSnapshot>
    val profiles: StateFlow<List<Profile>>
    val settings: StateFlow<AppSettings>
    val templates: StateFlow<List<Template>>
    val logs: StateFlow<String>
    val groups: StateFlow<List<ProxyGroup>>
    val groupsWithNodes: StateFlow<List<GroupWithNodes>>
    val mode: StateFlow<String>
    val memoryBytes: StateFlow<Long>
    val connections: StateFlow<app.singplane.clash.ConnectionsSnapshot>

    suspend fun start()
    suspend fun stop()
    suspend fun refreshProxies()
    suspend fun refreshConnections()
    suspend fun closeConnection(id: String)
    suspend fun closeAllConnections()
    suspend fun selectProxy(group: String, name: String)
    suspend fun changeMode(mode: String)
    suspend fun testProxyDelay(groupName: String, proxyName: String): Int?
    suspend fun testAllDelays(groupName: String)
    suspend fun upsertProfile(profile: Profile)
    suspend fun deleteProfile(id: String)
    suspend fun setActiveProfile(id: String)
    suspend fun importLocal(name: String, content: String, assembleEnabled: Boolean = false, templateId: String? = null)
    suspend fun importUrl(url: String, name: String, assembleEnabled: Boolean = false, templateId: String? = null)
    suspend fun refreshProfile(id: String)
    suspend fun saveTemplate(template: Template)
    suspend fun deleteTemplate(id: String)
    suspend fun updateSettings(settings: AppSettings)
    suspend fun downloadCore(onProgress: (String) -> Unit = {}): Result<String>
    suspend fun clearLogs()
}

