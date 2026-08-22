package app.singplane.core

import com.google.common.truth.Truth.assertThat
import org.junit.Test
import java.net.ServerSocket

class ListenPortsTest {

    @Test
    fun extractsMixedAndClashPorts() {
        val json = """
            {
              "inbounds": [
                {"type": "mixed", "listen": "127.0.0.1", "listen_port": 2080},
                {"type": "tun", "address": ["172.19.0.1/30"]}
              ],
              "experimental": {
                "clash_api": { "external_controller": "127.0.0.1:9090" }
              }
            }
        """.trimIndent()
        assertThat(ListenPorts.fromConfig(json)).containsExactly(2080, 9090)
    }

    @Test
    fun parsesIpv6ControllerAndSkipsBadPorts() {
        val json = """
            {
              "inbounds": [
                {"type": "http", "listen_port": 0},
                {"type": "socks", "listen_port": 1080}
              ],
              "experimental": { "clash_api": { "external_controller": "[::1]:19090" } }
            }
        """.trimIndent()
        assertThat(ListenPorts.fromConfig(json)).containsExactly(1080, 19090)
    }

    @Test
    fun emptyOrInvalidConfigYieldsNoPorts() {
        assertThat(ListenPorts.fromConfig("{}")).isEmpty()
        assertThat(ListenPorts.fromConfig("not-json")).isEmpty()
    }

    @Test
    fun addressInUseLooksLikeLeftoverBind() {
        assertThat(ListenPorts.isAddressInUse("listen tcp 127.0.0.1:2080: bind: address already in use")).isTrue()
        assertThat(ListenPorts.isAddressInUse("bind: Only one usage of each socket address")).isTrue()
        assertThat(ListenPorts.isAddressInUse("openTun fd=7")).isFalse()
    }

    @Test
    fun waitUntilFreeSeesReleasedPort() {
        val server = ServerSocket(0)
        val port = server.localPort
        assertThat(LocalPorts.isOccupied(port)).isTrue()
        assertThat(LocalPorts.waitUntilFree(setOf(port), timeoutMs = 80, stepMs = 20)).isFalse()
        server.close()
        assertThat(LocalPorts.waitUntilFree(setOf(port), timeoutMs = 1000, stepMs = 20)).isTrue()
        assertThat(LocalPorts.isOccupied(port)).isFalse()
    }
}
