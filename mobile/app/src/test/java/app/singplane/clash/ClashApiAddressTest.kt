package app.singplane.clash

import com.google.common.truth.Truth.assertThat
import org.junit.Test
import java.net.InetAddress

class ClashApiAddressTest {

    @Test
    fun loopbackHostsBecomeHttpOn127() {
        assertThat(ClashApiAddress.httpBase("127.0.0.1", 9090))
            .isEqualTo("http://127.0.0.1:9090")
        assertThat(ClashApiAddress.httpBase("0.0.0.0", 9090))
            .isEqualTo("http://127.0.0.1:9090")
        assertThat(ClashApiAddress.httpBase("::", 9090))
            .isEqualTo("http://127.0.0.1:9090")
        assertThat(ClashApiAddress.httpBase("localhost", 9090))
            .isEqualTo("http://127.0.0.1:9090")
    }

    @Test
    fun defaultOkHttpClientPinsLoopbackAndBypassesProxy() {
        val client = OkHttpClashClient.defaultClient()
        assertThat(client.socketFactory).isInstanceOf(LoopbackSocketFactory::class.java)
        assertThat(client.proxy).isEqualTo(java.net.Proxy.NO_PROXY)
    }

    @Test
    fun loopbackSocketFactoryBindsToLoopback() {
        val socket = LoopbackSocketFactory().createSocket()
        try {
            assertThat(socket.localAddress.isLoopbackAddress).isTrue()
            assertThat(socket.localAddress).isEqualTo(InetAddress.getByName("127.0.0.1"))
        } finally {
            socket.close()
        }
    }

    @Test
    fun parseLiveSubscriptionGroups() {
        val json = """
            {
              "proxies": {
                "GLOBAL": {
                  "name": "GLOBAL",
                  "type": "Fallback",
                  "now": "节点选择",
                  "all": ["节点选择", "自动选择", "DIRECT"]
                },
                "节点选择": {
                  "name": "节点选择",
                  "type": "Selector",
                  "now": "自动选择",
                  "all": ["自动选择", "🇭🇰 [H] HK 2 家宽", "DIRECT"]
                },
                "自动选择": {
                  "name": "自动选择",
                  "type": "URLTest",
                  "now": "🇭🇰 [H] HK 2 家宽",
                  "all": ["🇭🇰 [H] HK 2 家宽"]
                },
                "🇭🇰 [H] HK 2 家宽": {
                  "name": "🇭🇰 [H] HK 2 家宽",
                  "type": "AnyTLS",
                  "history": [{"delay": 120}]
                },
                "DIRECT": { "name": "DIRECT", "type": "Direct" }
              }
            }
        """.trimIndent()

        val groups = ClashApiParser.parseGroupNodes(json)
        assertThat(groups.map { it.group.name }).containsExactly("节点选择", "自动选择")
        val selector = groups.first { it.group.name == "节点选择" }
        assertThat(selector.group.now).isEqualTo("自动选择")
        assertThat(selector.nodes.map { it.name }).contains("🇭🇰 [H] HK 2 家宽")
        assertThat(selector.nodes.first { it.name == "🇭🇰 [H] HK 2 家宽" }.delayMs).isEqualTo(120)
    }
}
