package app.singplane.vpn

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class VpnParamsTest {
    @Test
    fun defaultVpnParamsValues() {
        val params = VpnParams(packageName = "app.singplane")
        assertThat(params.ipv4Address).isEqualTo("172.19.0.1")
        assertThat(params.ipv4Prefix).isEqualTo(30)
        assertThat(params.mtu).isEqualTo(1500)
        assertThat(params.dnsServers).contains("8.8.8.8")
        assertThat(params.routes).contains(VpnRoute("0.0.0.0", 0))
        assertThat(params.disallowedPackages).contains("app.singplane")
    }

    @Test
    fun parsesFromSingBoxJsonConfig() {
        val singBoxJson = """
            {
              "inbounds": [
                {
                  "type": "mixed",
                  "listen_port": 7890
                },
                {
                  "type": "tun",
                  "tag": "tun-in",
                  "inet4_address": "198.18.0.1/16",
                  "inet6_address": "fdfe:dcba:9876::1/126",
                  "mtu": 9000
                }
              ],
              "dns": {
                "servers": [
                  { "tag": "remote", "address": "1.1.1.1" },
                  { "tag": "local", "address": "223.5.5.5" }
                ]
              }
            }
        """.trimIndent()

        val params = VpnParams.fromSingBoxJson(singBoxJson, packageName = "app.singplane")
        assertThat(params.ipv4Address).isEqualTo("198.18.0.1")
        assertThat(params.ipv4Prefix).isEqualTo(16)
        assertThat(params.ipv6Address).isEqualTo("fdfe:dcba:9876::1")
        assertThat(params.ipv6Prefix).isEqualTo(126)
        assertThat(params.mtu).isEqualTo(9000)
        assertThat(params.dnsServers).contains("1.1.1.1")
        assertThat(params.disallowedPackages).contains("app.singplane")
    }

    @Test
    fun parsesModernAddressArrayOnTun() {
        val json = """
            {"inbounds":[{"type":"tun","tag":"tun","address":["172.19.0.1/30"],"mtu":1400}]}
        """.trimIndent()
        val params = VpnParams.fromSingBoxJson(json, packageName = "app.singplane")
        assertThat(params.ipv4Address).isEqualTo("172.19.0.1")
        assertThat(params.ipv4Prefix).isEqualTo(30)
        assertThat(params.mtu).isEqualTo(1400)
    }

    @Test
    fun fallbackWhenConfigHasNoTunInbound() {
        val nodeOnlyJson = """
            {
              "outbounds": [{ "type": "vless", "tag": "node-1" }]
            }
        """.trimIndent()

        val params = VpnParams.fromSingBoxJson(nodeOnlyJson, packageName = "app.singplane")
        assertThat(params.ipv4Address).isEqualTo("172.19.0.1")
        assertThat(params.ipv4Prefix).isEqualTo(30)
        assertThat(params.mtu).isEqualTo(1500)
        assertThat(params.disallowedPackages).contains("app.singplane")
    }
}
