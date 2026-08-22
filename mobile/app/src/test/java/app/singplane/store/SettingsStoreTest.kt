package app.singplane.store

import com.google.common.truth.Truth.assertThat
import app.singplane.model.AppSettings
import app.singplane.model.TailscaleSettings
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class SettingsStoreTest {
    @get:Rule
    val tmp = TemporaryFolder()

    @Test
    fun defaultsWhenMissing() {
        val s = SettingsStore(tmp.newFile("settings.json"))
        tmp.root.resolve("settings.json").delete()
        val loaded = SettingsStore(tmp.root.resolve("settings.json")).load()
        assertThat(loaded.mixedPort).isEqualTo(7890)
        assertThat(loaded.clashApiPort).isEqualTo(9090)
        assertThat(loaded.activeProfileId).isNull()
        assertThat(loaded.coreChannel).isEqualTo("beta")
        assertThat(loaded.githubProxy).isEqualTo("")
        assertThat(loaded.stripTunOnAssemble).isTrue()
        assertThat(loaded.themeMode).isEqualTo("system")
        assertThat(loaded.language).isEqualTo("system")
        assertThat(loaded.tailscale.enabled).isFalse()
        assertThat(loaded.tailscale.tag).isEqualTo("ts-local")
    }

    @Test
    fun persistActiveAndPorts() {
        val file = tmp.newFile("settings.json")
        val s = SettingsStore(file)
        s.save(
            AppSettings(
                mixedPort = 1080,
                clashApiPort = 9091,
                activeProfileId = "p1",
                stripTunOnAssemble = false,
                coreChannel = "stable",
                githubProxy = "https://ghfast.top",
                themeMode = "dark",
                language = "zh-Hant",
                tailscale = TailscaleSettings(
                    enabled = true,
                    authKey = "tskey-auth-123",
                    hostname = "my-phone",
                ),
            ),
        )
        val again = SettingsStore(file).load()
        assertThat(again.mixedPort).isEqualTo(1080)
        assertThat(again.clashApiPort).isEqualTo(9091)
        assertThat(again.activeProfileId).isEqualTo("p1")
        assertThat(again.stripTunOnAssemble).isFalse()
        assertThat(again.coreChannel).isEqualTo("stable")
        assertThat(again.githubProxy).isEqualTo("https://ghfast.top")
        assertThat(again.themeMode).isEqualTo("dark")
        assertThat(again.language).isEqualTo("zh-Hant")
        assertThat(again.tailscale.enabled).isTrue()
        assertThat(again.tailscale.authKey).isEqualTo("tskey-auth-123")
        assertThat(again.tailscale.hostname).isEqualTo("my-phone")
    }
}
