package app.singplane.model

import com.google.common.truth.Truth.assertThat
import app.singplane.assemble.CoreLine
import org.json.JSONObject
import org.junit.Test

class TailscaleSettingsTest {

    @Test
    fun testDefaultsAndResolution() {
        val settings = TailscaleSettings()
        assertThat(settings.resolvedTag()).isEqualTo("ts-local")
        assertThat(settings.resolvedDnsTag()).isEqualTo("ts-local-dns")
        assertThat(settings.usesDeviceAuth()).isTrue()
        
        settings.tag = "  my-tag  "
        assertThat(settings.resolvedTag()).isEqualTo("my-tag")
        assertThat(settings.resolvedDnsTag()).isEqualTo("my-tag-dns")
    }

    @Test
    fun testJsonSerialization() {
        val settings = TailscaleSettings(
            enabled = true,
            authKey = "my-key",
            routeDomainSuffix = ".abc.net"
        )
        val json = settings.toJson()
        val parsed = TailscaleSettings.fromJson(json)
        assertThat(parsed.enabled).isTrue()
        assertThat(parsed.authKey).isEqualTo("my-key")
        assertThat(parsed.routeDomainSuffix).isEqualTo(".abc.net")
    }

    @Test
    fun testEndpointJson() {
        val settings = TailscaleSettings(
            enabled = true,
            authKey = "my-key",
            controlUrl = "https://control.example.com",
            hostname = "my-host",
            advertiseRoutes = "192.168.1.0/24, 10.0.0.0/8",
            advertiseTags = "tag:a,tag:b",
            sshServer = true
        )

        // V13 tests
        val ep13 = settings.toEndpointJson(CoreLine.V13)
        assertThat(ep13.getString("type")).isEqualTo("tailscale")
        assertThat(ep13.getString("tag")).isEqualTo("ts-local")
        assertThat(ep13.getString("auth_key")).isEqualTo("my-key")
        assertThat(ep13.getString("control_url")).isEqualTo("https://control.example.com")
        assertThat(ep13.getString("hostname")).isEqualTo("my-host")
        assertThat(ep13.has("state_directory")).isTrue()
        assertThat(ep13.getBoolean("accept_routes")).isTrue()
        assertThat(ep13.getJSONArray("advertise_routes").length()).isEqualTo(2)
        assertThat(ep13.getJSONArray("advertise_routes").getString(0)).isEqualTo("192.168.1.0/24")
        
        // V13 supports advertise_tags
        assertThat(ep13.getJSONArray("advertise_tags").length()).isEqualTo(2)
        // V13 does not support ssh_server
        assertThat(ep13.has("ssh_server")).isFalse()
        // system_interface is always false on Android, so it won't be set
        assertThat(ep13.has("system_interface")).isFalse()

        // V14 tests
        val ep14 = settings.toEndpointJson(CoreLine.V14)
        assertThat(ep14.getBoolean("ssh_server")).isTrue()
    }
}
