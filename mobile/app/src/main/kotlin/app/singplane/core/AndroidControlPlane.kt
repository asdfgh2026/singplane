package app.singplane.core

import app.singplane.assemble.Assembler
import app.singplane.assemble.ContentDetector
import app.singplane.assemble.CoreLine
import app.singplane.assemble.RuntimePatch
import app.singplane.assemble.TailscaleOverlay
import app.singplane.clash.ClashApiParser
import app.singplane.clash.GroupWithNodes
import app.singplane.clash.OkHttpClashClient
import app.singplane.clash.ProxyGroup
import app.singplane.fetch.SubscriptionFetcher
import app.singplane.model.AppSettings
import app.singplane.model.Profile
import app.singplane.model.Template
import app.singplane.store.ProfileStore
import app.singplane.store.SettingsStore
import app.singplane.store.TemplateStore
import app.singplane.vpn.NeedVpnConsent
import app.singplane.vpn.VpnSession
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import org.json.JSONObject
import java.io.File
import java.util.UUID

class AndroidControlPlane(
    private val profileStore: ProfileStore,
    private val settingsStore: SettingsStore,
    private val templateStore: TemplateStore? = null,
    private val fetcher: SubscriptionFetcher,
    private val vpn: VpnSession,
    private val core: CoreProcess,
    private val coresDir: File? = null,
    private val arch: String = "arm64",
    private val downloader: CoreDownloader = CoreDownloader(),
    private val clashClient: OkHttpClashClient = OkHttpClashClient(),
    private val clashGroups: suspend (String) -> List<ProxyGroup> = { clashClient.groups(it) },
    private val clashSelect: suspend (String, String, String) -> Unit = { base, group, name -> clashClient.select(base, group, name) },
) : ControlPlane {
    private val _status = MutableStateFlow(CoreSnapshot())
    private val _profiles = MutableStateFlow<List<Profile>>(emptyList())
    private val _settings = MutableStateFlow(AppSettings())
    private val _templates = MutableStateFlow<List<Template>>(emptyList())
    private val _logs = MutableStateFlow("")
    private val _groups = MutableStateFlow<List<ProxyGroup>>(emptyList())
    private val _groupsWithNodes = MutableStateFlow<List<GroupWithNodes>>(emptyList())
    private val _mode = MutableStateFlow("rule")
    private val _memoryBytes = MutableStateFlow(0L)
    private val _connections = MutableStateFlow(app.singplane.clash.ConnectionsSnapshot())
    private var lastConnectionsPollMs = 0L

    override val status: StateFlow<CoreSnapshot> = _status.asStateFlow()
    override val profiles: StateFlow<List<Profile>> = _profiles.asStateFlow()
    override val settings: StateFlow<AppSettings> = _settings.asStateFlow()
    override val templates: StateFlow<List<Template>> = _templates.asStateFlow()
    override val logs: StateFlow<String> = _logs.asStateFlow()
    override val groups: StateFlow<List<ProxyGroup>> = _groups.asStateFlow()
    override val groupsWithNodes: StateFlow<List<GroupWithNodes>> = _groupsWithNodes.asStateFlow()
    override val mode: StateFlow<String> = _mode.asStateFlow()
    override val memoryBytes: StateFlow<Long> = _memoryBytes.asStateFlow()
    override val connections: StateFlow<app.singplane.clash.ConnectionsSnapshot> = _connections.asStateFlow()


    init {
        reload()
    }

    private fun reload() {
        _profiles.value = profileStore.loadAll()
        _settings.value = settingsStore.load()
        _templates.value = templateStore?.loadAll() ?: emptyList()
    }

    /**
     * Assemble runtime config JSON from profile content, runtime patch, and tailscale overlay.
     */
    fun assembleRuntimeConfig(profile: Profile, settings: AppSettings): Result<String> = runCatching {
        if (!profile.runnable && !ContentDetector.isRunnable(profile.content)) {
            error("当前配置不可直接运行（需要转换 / 模板）")
        }
        val parsed = runCatching { JSONObject(profile.content) }.getOrElse {
            error("配置不是合法 JSON")
        }
        val patched = RuntimePatch.apply(
            parsed,
            RuntimePatch.Options(
                forceMixedPort = if (settings.forceAppPortsOnAssemble) settings.mixedPort else null,
                forceClashApi = if (settings.forceAppPortsOnAssemble) settings.clashApiController else null,
                forceListenLocalhost = settings.forceAppPortsOnAssemble,
                stripTun = settings.stripTunOnAssemble,
            ),
        )
        val finalConfig = if (settings.tailscale.enabled) {
            TailscaleOverlay.apply(patched, settings.tailscale, CoreLine.V13)
        } else {
            patched
        }
        finalConfig.toString()
    }

    private suspend fun startCoreAndVpn(corePath: String, json: String) {
        try {
            vpn.start(json)
            core.start(corePath, json)
        } catch (e: NeedVpnConsent) {
            runCatching { core.stop() }
            runCatching { vpn.stop() }
            throw e
        } catch (e: Exception) {
            runCatching { core.stop() }
            runCatching { vpn.stop() }
            throw e
        }
    }

    override suspend fun start() {
        val settings = _settings.value
        val activeId = settings.activeProfileId
        val profile = _profiles.value.firstOrNull { it.id == activeId }
        if (profile == null) {
            val msg = "请先选择一份可运行的配置"
            appendLog("start failed: $msg")
            _status.value = CoreSnapshot(
                phase = CorePhase.Stopped,
                message = msg,
            )
            return
        }

        val json = assembleRuntimeConfig(profile, settings).getOrElse { err ->
            val msg = err.message ?: "配置处理失败"
            appendLog("assemble failed: $msg")
            _status.value = CoreSnapshot(
                phase = CorePhase.Stopped,
                message = msg,
                activeProfileName = profile.name,
            )
            return
        }

        val corePath = settings.corePath.trim().ifEmpty {
            if (core is LibboxCoreProcess) {
                "embedded-libbox"
            } else {
                File(coresDir, CorePlatform.binaryFileName("android")).takeIf { it.isFile }?.absolutePath ?: ""
            }
        }
        if (corePath.isEmpty()) {
            val msg = "请先在设置中下载或指定官方内核"
            appendLog("start failed: $msg")
            _status.value = CoreSnapshot(
                phase = CorePhase.Stopped,
                message = msg,
                activeProfileName = profile.name,
            )
            return
        }




        _status.value = CoreSnapshot(
            phase = CorePhase.Starting,
            message = "启动中...",
            activeProfileName = profile.name,
        )
        appendLog("starting core at $corePath for profile ${profile.name}...")

        try {
            startCoreAndVpn(corePath, json)
        } catch (e: NeedVpnConsent) {
            appendLog("waiting for VPN permission...")
            _status.value = CoreSnapshot(
                phase = CorePhase.Stopped,
                message = "等待 VPN 权限",
                activeProfileName = profile.name,
            )
            throw e
        } catch (e: Exception) {
            val raw = e.message ?: "启动失败"
            val msg = if (ListenPorts.isAddressInUse(raw)) {
                "$raw（上次退出未释放端口，已尝试关掉占用的内核，请再点一次启动）"
            } else {
                raw
            }
            appendLog("start error: $msg")
            _status.value = CoreSnapshot(
                phase = CorePhase.Error,
                message = msg,
                activeProfileName = profile.name,
            )
            return
        }

        appendLog("start ${profile.name} successfully")
        _status.value = CoreSnapshot(
            phase = CorePhase.Running,
            message = "运行中",
            viaVpn = true,
            activeProfileName = profile.name,
            startedAtMs = System.currentTimeMillis(),
        )

        runCatching { refreshProxies() }
            .onFailure { appendLog("clash proxies: ${it.message}") }
    }

    private fun clashBase(): String {
        val s = _settings.value
        return app.singplane.clash.ClashApiAddress.httpBase(s.clashApiHost, s.clashApiPort)
    }

    override suspend fun refreshProxies() {
        val base = clashBase()
        runCatching {
            _groups.value = clashGroups(base)
            val list = clashClient.groupsWithNodes(base)
            if (list.isNotEmpty()) {
                _groupsWithNodes.value = ClashApiParser.mergeDelays(list, _groupsWithNodes.value)
            }
            _mode.value = clashClient.getMode(base)
            _memoryBytes.value = clashClient.getMemory(base)
        }.onFailure { appendLog("clash proxies: ${it.message}") }
    }

    override suspend fun refreshConnections() {
        val base = clashBase()
        val now = System.currentTimeMillis()
        val elapsed = if (lastConnectionsPollMs > 0) (now - lastConnectionsPollMs) / 1000.0 else 1.0
        lastConnectionsPollMs = now
        runCatching {
            val snap = clashClient.getConnections(base)
            val computed = app.singplane.clash.ClashConnectionParser.computeSpeeds(
                current = snap.connections,
                prev = _connections.value.connections,
                intervalSec = elapsed,
            )
            _connections.value = snap.copy(connections = computed)
        }.onFailure { appendLog("clash connections: ${it.message}") }
    }

    override suspend fun closeConnection(id: String) {
        val base = clashBase()
        clashClient.closeConnection(base, id)
        refreshConnections()
    }

    override suspend fun closeAllConnections() {
        val base = clashBase()
        clashClient.closeAllConnections(base)
        refreshConnections()
    }


    override suspend fun selectProxy(group: String, name: String) {
        val base = clashBase()
        clashSelect(base, group, name)
        refreshProxies()
    }

    override suspend fun changeMode(mode: String) {
        val base = clashBase()
        clashClient.changeMode(base, mode)
        _mode.value = mode
    }

    override suspend fun testProxyDelay(groupName: String, proxyName: String): Int? {
        val base = clashBase()
        applyNodeDelay(proxyName, 0)
        val delay = clashClient.testDelay(base, proxyName)
        applyNodeDelay(proxyName, delay)
        return delay
    }

    override suspend fun testAllDelays(groupName: String) {
        val base = clashBase()
        val group = _groupsWithNodes.value.firstOrNull { it.group.name == groupName } ?: return
        val sem = Semaphore(5)
        coroutineScope {
            group.nodes.map { node ->
                async {
                    sem.withPermit {
                        applyNodeDelay(node.name, 0)
                        val delay = runCatching { clashClient.testDelay(base, node.name) }.getOrDefault(0)
                        applyNodeDelay(node.name, delay)
                    }
                }
            }.awaitAll()
        }
        runCatching { refreshProxies() }
    }

    private fun applyNodeDelay(name: String, delay: Int?) {
        val shown = if (delay != null && delay > 0) delay else 0
        _groupsWithNodes.value = _groupsWithNodes.value.map { gn ->
            gn.copy(
                nodes = gn.nodes.map { n ->
                    if (n.name == name) n.copy(delayMs = shown) else n
                },
            )
        }
    }

    override suspend fun stop() {
        runCatching { core.stop() }
        vpn.stop()
        appendLog("stop")
        _status.value = CoreSnapshot(
            phase = CorePhase.Stopped,
            message = "已停止",
            activeProfileName = activeName(),
        )
    }


    override suspend fun upsertProfile(profile: Profile) {
        profileStore.upsert(profile)
        reload()
    }

    override suspend fun deleteProfile(id: String) {
        profileStore.delete(id)
        val settings = _settings.value
        if (settings.activeProfileId == id) {
            settingsStore.save(settings.copy(activeProfileId = null))
        }
        reload()
    }

    override suspend fun setActiveProfile(id: String) {
        val wasRunning = _status.value.running
        settingsStore.save(_settings.value.copy(activeProfileId = id))
        reload()
        if (wasRunning) {
            stop()
            start()
        }
    }

    override suspend fun saveTemplate(template: Template) {
        templateStore?.save(template)
        reload()
    }

    override suspend fun deleteTemplate(id: String) {
        templateStore?.delete(id)
        reload()
    }

    private fun assembleIfEnabled(
        content: String,
        assembleEnabled: Boolean,
        templateId: String?,
    ): Pair<String, Boolean> {
        if (!assembleEnabled) return Pair(content, ContentDetector.isRunnable(content))
        val tid = templateId ?: _settings.value.defaultTemplateId
        val template = _templates.value.firstOrNull { it.id == tid } ?: _templates.value.firstOrNull()
        if (template == null) return Pair(content, ContentDetector.isRunnable(content))
        val res = Assembler.assemble(
            sourceBody = content,
            templateContent = template.content,
        )
        return if (res.ok && res.config != null) {
            Pair(res.config.toString(), true)
        } else {
            Pair(content, ContentDetector.isRunnable(content))
        }
    }

    override suspend fun importLocal(name: String, content: String, assembleEnabled: Boolean, templateId: String?) {
        val (finalContent, runnable) = assembleIfEnabled(content, assembleEnabled, templateId)
        val p = Profile(
            id = UUID.randomUUID().toString(),
            name = name.ifBlank { "本地配置" },
            sourceType = "local",
            content = finalContent,
            sourceBody = content,
            runnable = runnable,
            assembleEnabled = assembleEnabled,
            templateId = if (assembleEnabled) templateId ?: _settings.value.defaultTemplateId else null,
        )
        upsertProfile(p)
        if (_settings.value.activeProfileId == null) {
            setActiveProfile(p.id)
        }
    }

    override suspend fun importUrl(url: String, name: String, assembleEnabled: Boolean, templateId: String?) {
        val res = fetcher.fetch(url)
        val body = res.body
        val (finalContent, runnable) = assembleIfEnabled(body, assembleEnabled, templateId)
        val p = Profile(
            id = UUID.randomUUID().toString(),
            name = name.ifBlank { url.substringAfterLast('/').substringBefore('?').ifBlank { "远程订阅" } },
            sourceType = "url",
            url = url,
            content = finalContent,
            sourceBody = body,
            runnable = runnable,
            assembleEnabled = assembleEnabled,
            templateId = if (assembleEnabled) templateId ?: _settings.value.defaultTemplateId else null,
            upload = res.upload,
            download = res.download,
            total = res.total,
            expireMs = res.expireMs,
        )

        upsertProfile(p)
        if (_settings.value.activeProfileId == null) {
            setActiveProfile(p.id)
        }
    }

    override suspend fun refreshProfile(id: String) {
        val p = _profiles.value.firstOrNull { it.id == id } ?: return
        val url = p.url ?: return
        val res = fetcher.fetch(url)
        val body = res.body
        val (finalContent, runnable) = assembleIfEnabled(body, p.assembleEnabled, p.templateId)
        val updated = p.copy(
            content = finalContent,
            sourceBody = body,
            runnable = runnable,
            upload = res.upload,
            download = res.download,
            total = res.total,
            expireMs = res.expireMs,
        )
        upsertProfile(updated)
    }

    override suspend fun updateSettings(settings: AppSettings) {
        settingsStore.save(settings)
        reload()
    }

    override suspend fun downloadCore(onProgress: (String) -> Unit): Result<String> {
        val targetDir = coresDir ?: return Result.failure(IllegalStateException("coresDir not configured"))
        val proxy = _settings.value.githubProxy
        val channel = _settings.value.coreChannel
        return runCatching {
            val file = downloader.downloadAndInstall(
                channel = channel,
                githubProxy = proxy,
                coresDir = targetDir,
                arch = arch,
                onProgress = onProgress,
            )
            val settings = _settings.value.copy(corePath = file.absolutePath)
            updateSettings(settings)
            file.absolutePath
        }
    }

    override suspend fun clearLogs() {
        _logs.value = ""
    }

    private fun appendLog(line: String) {
        val cur = _logs.value
        _logs.value = if (cur.isEmpty()) line else "$cur\n$line"
    }

    fun onCoreLog(line: String) {
        appendLog(line)
    }

    fun onVpnConsentRejected() {
        _status.value = CoreSnapshot(
            phase = CorePhase.Stopped,
            message = "用户取消了 VPN 授权",
            activeProfileName = activeName(),
        )
    }


    private fun activeName(): String? {
        val id = _settings.value.activeProfileId
        return _profiles.value.firstOrNull { it.id == id }?.name
    }
}
