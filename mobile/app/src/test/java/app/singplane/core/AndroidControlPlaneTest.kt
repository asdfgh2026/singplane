package app.singplane.core

import com.google.common.truth.Truth.assertThat
import app.singplane.clash.ProxyGroup
import app.singplane.fetch.FetchResult
import app.singplane.fetch.SubscriptionFetcher
import app.singplane.model.AppSettings
import app.singplane.store.ProfileStore
import app.singplane.store.SettingsStore
import app.singplane.store.TemplateStore
import app.singplane.vpn.RecordingVpnSession
import kotlinx.coroutines.test.runTest
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class AndroidControlPlaneTest {
    @get:Rule
    val tmp = TemporaryFolder()

    private fun plane(
        fetcher: SubscriptionFetcher = SubscriptionFetcher { error("no net") },
        vpn: RecordingVpnSession = RecordingVpnSession(),
        core: RecordingCoreProcess = RecordingCoreProcess(),
        clash: suspend (String) -> List<ProxyGroup> = { emptyList() },
    ): Triple<AndroidControlPlane, RecordingVpnSession, RecordingCoreProcess> {
        val tplDir = tmp.newFolder("templates_${System.nanoTime()}")
        val templateStore = TemplateStore(
            templatesDir = tplDir,
            builtinReader = { id: String ->
                if (id == "builtin-mixed-direct") """{"inbounds":[{"type":"mixed","listen_port":7890}],"outbounds":[{"type":"direct","tag":"direct"},{"type":"selector","tag":"select","outbounds":["direct"]}]}"""
                else null
            },
        )

        val p = AndroidControlPlane(
            profileStore = ProfileStore(tmp.newFolder("profiles_${System.nanoTime()}")),
            settingsStore = SettingsStore(tmp.newFile("settings_${System.nanoTime()}.json")).also {
                it.save(AppSettings(corePath = "/opt/sing-box"))
            },
            templateStore = templateStore,
            fetcher = fetcher,
            vpn = vpn,
            core = core,
            clashGroups = clash,
        )
        return Triple(p, vpn, core)
    }


    @Test
    fun startWithoutProfileFails() = runTest {
        val (p, vpn, core) = plane()
        p.start()
        assertThat(p.status.value.running).isFalse()
        assertThat(p.status.value.phase).isEqualTo(CorePhase.Stopped)
        assertThat(p.status.value.message).contains("配置")
        assertThat(vpn.started).isEmpty()
        assertThat(core.started).isEmpty()
    }

    @Test
    fun startWithoutCoreFails() = runTest {
        val (p, _, core) = plane()
        p.updateSettings(p.settings.value.copy(corePath = ""))
        p.importLocal(name = "l", content = """{"outbounds":[{"type":"direct","tag":"d"}]}""")
        p.setActiveProfile(p.profiles.value.single().id)
        p.start()
        assertThat(p.status.value.running).isFalse()
        assertThat(p.status.value.phase).isEqualTo(CorePhase.Stopped)
        assertThat(p.status.value.message).contains("内核")
        assertThat(core.started).isEmpty()
    }

    @Test
    fun startNonRunnableProfileFails() = runTest {
        val (p, vpn, core) = plane()
        p.importLocal(name = "clash_yaml", content = "proxies:\n  - name: node\n    type: ss", assembleEnabled = false)
        val profile = p.profiles.value.single().copy(runnable = false)
        p.upsertProfile(profile)
        p.setActiveProfile(profile.id)

        p.start()
        assertThat(p.status.value.running).isFalse()
        assertThat(p.status.value.phase).isEqualTo(CorePhase.Stopped)
        assertThat(p.status.value.message).contains("不可直接运行")
        assertThat(vpn.started).isEmpty()
        assertThat(core.started).isEmpty()
    }


    @Test
    fun startInvalidJsonProfileFails() = runTest {
        val (p, vpn, core) = plane()
        p.importLocal(name = "broken", content = "{ not-a-json")
        val profile = p.profiles.value.single().copy(runnable = true)
        p.upsertProfile(profile)
        p.setActiveProfile(profile.id)

        p.start()
        assertThat(p.status.value.running).isFalse()
        assertThat(p.status.value.phase).isEqualTo(CorePhase.Stopped)
        assertThat(p.status.value.message).contains("JSON")
        assertThat(vpn.started).isEmpty()
        assertThat(core.started).isEmpty()
    }

    @Test
    fun startWithNeedVpnConsentRollsBackAndRethrows() = runTest {
        val (p, vpn, core) = plane()
        val dummyIntent = android.content.Intent()
        vpn.throwOnStart = app.singplane.vpn.NeedVpnConsent(dummyIntent)

        p.importLocal(name = "l", content = """{"outbounds":[{"type":"direct","tag":"d"}]}""")
        p.setActiveProfile(p.profiles.value.single().id)

        try {
            p.start()
            error("Expected NeedVpnConsent")
        } catch (e: app.singplane.vpn.NeedVpnConsent) {
            assertThat(e.consentIntent).isEqualTo(dummyIntent)
        }

        assertThat(p.status.value.running).isFalse()
        assertThat(p.status.value.phase).isEqualTo(CorePhase.Stopped)
        assertThat(p.status.value.message).contains("VPN")
        assertThat(core.stopCount).isEqualTo(1)
        assertThat(vpn.stopCount).isEqualTo(1)
    }

    @Test
    fun startWithVpnExceptionSetsPhaseErrorAndCleansUp() = runTest {
        val (p, vpn, core) = plane()
        vpn.throwOnStart = RuntimeException("VPN permission revoked")

        p.importLocal(name = "l", content = """{"outbounds":[{"type":"direct","tag":"d"}]}""")
        p.setActiveProfile(p.profiles.value.single().id)

        p.start()

        assertThat(p.status.value.running).isFalse()
        assertThat(p.status.value.phase).isEqualTo(CorePhase.Error)
        assertThat(p.status.value.message).contains("revoked")
        assertThat(core.stopCount).isEqualTo(1)
        assertThat(vpn.stopCount).isEqualTo(1)
    }



    @Test
    fun startRunnableProfilePatchesAndStartsCoreAndVpn() = runTest {
        val (p, vpn, core) = plane()
        p.importLocal(
            name = "local",
            content = """
                {"inbounds":[{"type":"mixed","listen":"0.0.0.0","listen_port":1}],
                 "outbounds":[{"type":"direct","tag":"d"}]}
            """.trimIndent(),
        )
        p.setActiveProfile(p.profiles.value.single().id)
        p.start()
        assertThat(p.status.value.running).isTrue()
        assertThat(vpn.started).hasSize(1)
        assertThat(core.started).hasSize(1)
        assertThat(core.started[0].first).isEqualTo("/opt/sing-box")
        assertThat(core.started[0].second).contains("\"listen_port\":7890")
        assertThat(p.status.value.message).contains("运行中")
    }

    @Test
    fun stopClearsVpnAndCore() = runTest {
        val (p, vpn, core) = plane()
        p.importLocal(name = "l", content = """{"outbounds":[{"type":"direct","tag":"d"}]}""")
        p.setActiveProfile(p.profiles.value.single().id)
        p.start()
        p.stop()
        assertThat(p.status.value.running).isFalse()
        assertThat(vpn.stopCount).isEqualTo(1)
        assertThat(core.stopCount).isEqualTo(1)
    }

    @Test
    fun importUrlStoresBodyAndUserinfo() = runTest {
        val body = """{"outbounds":[{"type":"direct","tag":"d"}]}"""
        val (p, _, _) = plane(
            fetcher = SubscriptionFetcher {
                FetchResult(body = body, upload = 1, download = 2, total = 3, expireMs = 4)
            },
        )
        p.importUrl("https://ex/sub", "remote")
        val prof = p.profiles.value.single()
        assertThat(prof.sourceType).isEqualTo("url")
        assertThat(prof.url).isEqualTo("https://ex/sub")
        assertThat(prof.upload).isEqualTo(1)
        assertThat(prof.download).isEqualTo(2)
        assertThat(prof.runnable).isTrue()
    }

    @Test
    fun settingsRoundTrip() = runTest {
        val (p, _, _) = plane()
        p.updateSettings(p.settings.value.copy(mixedPort = 1088, activeProfileId = "x"))
        assertThat(p.settings.value.mixedPort).isEqualTo(1088)
        assertThat(p.settings.value.activeProfileId).isEqualTo("x")
    }

    @Test
    fun refreshProxiesUsesClashClient() = runTest {
        val (p, _, _) = plane(
            clash = {
                listOf(ProxyGroup("节点", "Selector", "hk", listOf("hk", "jp")))
            },
        )
        p.importLocal(name = "l", content = """{"outbounds":[{"type":"direct","tag":"d"}]}""")
        p.setActiveProfile(p.profiles.value.single().id)
        p.start()
        p.refreshProxies()
        assertThat(p.groups.value.single().name).isEqualTo("节点")
    }

    @Test
    fun refreshProxiesLogsClashFailureInsteadOfEmptySilent() = runTest {
        val (p, _, _) = plane(
            clash = { error("CLEARTEXT communication to 127.0.0.1 not permitted") },
        )
        p.importLocal(name = "l", content = """{"outbounds":[{"type":"direct","tag":"d"}]}""")
        p.setActiveProfile(p.profiles.value.single().id)
        p.start()
        p.refreshProxies()
        assertThat(p.status.value.running).isTrue()
        assertThat(p.groups.value).isEmpty()
        assertThat(p.logs.value).contains("clash proxies")
        assertThat(p.logs.value).contains("CLEARTEXT")
    }

    @Test
    fun onCoreLogAppendsToLogsAndClearLogs() = runTest {
        val (p, _, _) = plane()
        p.onCoreLog("line 1: sing-box started")
        p.onCoreLog("line 2: inbound listening on 7890")
        assertThat(p.logs.value).contains("line 1: sing-box started")
        assertThat(p.logs.value).contains("line 2: inbound listening on 7890")

        p.clearLogs()
        assertThat(p.logs.value).isEmpty()
    }

    @Test
    fun tailscaleOverlayOnStartUsesV13WithoutPreferredBy() = runTest {
        val (p, _, core) = plane()
        p.updateSettings(
            p.settings.value.copy(
                tailscale = app.singplane.model.TailscaleSettings(enabled = true),
            ),
        )
        p.importLocal(
            name = "ts",
            content = """
                {"inbounds":[{"type":"mixed","listen_port":7890}],
                 "outbounds":[{"type":"direct","tag":"d"}],
                 "dns":{"servers":[{"type":"https","tag":"local","server":"223.5.5.5"}],"rules":[]}}
            """.trimIndent(),
        )
        p.setActiveProfile(p.profiles.value.single().id)
        p.start()
        assertThat(core.started).hasSize(1)
        val json = core.started[0].second
        assertThat(json).contains("ts-local")
        assertThat(json).doesNotContain("preferred_by")
        assertThat(json).doesNotContain("accept_search_domain")
    }

    @Test
    fun startWithStripTunDefaultRemovesTunInbound() = runTest {
        val (p, _, core) = plane()
        p.importLocal(
            name = "with_tun",
            content = """
                {"inbounds":[{"type":"mixed","listen_port":7890},{"type":"tun","interface_name":"tun0"}],
                 "outbounds":[{"type":"direct","tag":"d"}]}
            """.trimIndent(),
        )
        p.setActiveProfile(p.profiles.value.single().id)
        p.start()
        assertThat(core.started).hasSize(1)
        val passedJson = core.started[0].second
        assertThat(passedJson).doesNotContain("\"type\":\"tun\"")
        assertThat(passedJson).contains("\"type\":\"mixed\"")
    }

    @Test
    fun templateManagementAndAssembly() = runTest {
        val (p, _, _) = plane()
        assertThat(p.templates.value.any { it.id == "builtin-mixed-direct" }).isTrue()

        val sourceOnlyNodes = """{"outbounds":[{"type":"vless","tag":"hk1"}]}"""
        p.importLocal(
            name = "assembled-profile",
            content = sourceOnlyNodes,
            assembleEnabled = true,
            templateId = "builtin-mixed-direct",
        )
        val prof = p.profiles.value.single()
        assertThat(prof.assembleEnabled).isTrue()
        assertThat(prof.runnable).isTrue()
        assertThat(prof.content).contains("mixed")
        assertThat(prof.content).contains("hk1")
        assertThat(prof.sourceBody).isEqualTo(sourceOnlyNodes)
    }

    @Test
    fun hotReloadWhenActiveProfileChangesWhileRunning() = runTest {
        val (p, _, core) = plane()
        p.importLocal(name = "p1", content = """{"outbounds":[{"type":"direct","tag":"d1"}]}""")
        p.importLocal(name = "p2", content = """{"outbounds":[{"type":"direct","tag":"d2"}]}""")
        val p1 = p.profiles.value.first { it.name == "p1" }
        val p2 = p.profiles.value.first { it.name == "p2" }

        p.setActiveProfile(p1.id)
        p.start()
        assertThat(p.status.value.running).isTrue()
        assertThat(core.started).hasSize(1)
        assertThat(core.started[0].second).contains("d1")

        // Switch to p2 while running -> should trigger hot-reload (stop and restart with p2)
        p.setActiveProfile(p2.id)
        assertThat(p.status.value.running).isTrue()
        assertThat(core.started).hasSize(2)
        assertThat(core.started[1].second).contains("d2")
    }
}




