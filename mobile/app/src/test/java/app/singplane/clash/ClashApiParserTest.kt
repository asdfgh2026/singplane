package app.singplane.clash

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class ClashApiParserTest {

    @Test
    fun parseMode() {
        val json = """{"mode":"rule","log-level":"info"}"""
        val mode = ClashApiParser.parseMode(json)
        assertThat(mode).isEqualTo("rule")
    }

    @Test
    fun parseMemoryInuse() {
        val json = """{"inuse":33554432,"os":67108864}"""
        val inuse = ClashApiParser.parseMemoryInuse(json)
        assertThat(inuse).isEqualTo(33554432L)
    }

    @Test
    fun parseProxiesWithDelay() {
        val json = """
            {
              "proxies": {
                "GLOBAL": { "name": "GLOBAL", "type": "Selector", "now": "DIRECT", "all": ["DIRECT", "HK-01", "JP-01"] },
                "DIRECT": { "name": "DIRECT", "type": "Direct", "history": [] },
                "HK-01": {
                  "name": "HK-01",
                  "type": "Vless",
                  "history": [
                    { "time": "2026-08-17T00:00:00Z", "delay": 85 }
                  ]
                },
                "JP-01": {
                  "name": "JP-01",
                  "type": "Shadowsocks",
                  "history": [
                    { "time": "2026-08-17T00:00:00Z", "delay": 150 }
                  ]
                },
                "PROXY": {
                  "name": "PROXY",
                  "type": "Selector",
                  "now": "HK-01",
                  "all": ["HK-01", "JP-01", "DIRECT"]
                }
              }
            }
        """.trimIndent()

        val groupMap = ClashApiParser.parseGroupNodes(json)
        val proxyGroup = groupMap.firstOrNull { it.group.name == "PROXY" }
        assertThat(proxyGroup).isNotNull()
        assertThat(proxyGroup?.group?.now).isEqualTo("HK-01")
        assertThat(proxyGroup?.nodes?.size).isEqualTo(3)

        val hkNode = proxyGroup?.nodes?.find { it.name == "HK-01" }
        assertThat(hkNode?.delayMs).isEqualTo(85)

        val jpNode = proxyGroup?.nodes?.find { it.name == "JP-01" }
        assertThat(jpNode?.delayMs).isEqualTo(150)
    }

    @Test
    fun sortAndFilterNodes() {
        val nodes = listOf(
            ProxyNode(name = "US-Node", type = "Vless", delayMs = 300),
            ProxyNode(name = "HK-Node", type = "Vless", delayMs = 50),
            ProxyNode(name = "JP-Node", type = "Vless", delayMs = null),
            ProxyNode(name = "SG-Node", type = "Vless", delayMs = 120),
        )

        // Filter
        val filtered = ClashApiParser.filterNodes(nodes, "HK")
        assertThat(filtered.map { it.name }).containsExactly("HK-Node")

        // Sort by Latency (unknown at end)
        val sortedByLatency = ClashApiParser.sortNodes(nodes, SortMode.LATENCY)
        assertThat(sortedByLatency.map { it.name }).containsExactly("HK-Node", "SG-Node", "US-Node", "JP-Node")

        // Sort by Name
        val sortedByName = ClashApiParser.sortNodes(nodes, SortMode.NAME)
        assertThat(sortedByName.map { it.name }).containsExactly("HK-Node", "JP-Node", "SG-Node", "US-Node")
    }

    @Test
    fun encodeNameUsesPercentTwentyNotPlus() {
        val encoded = ClashApiPath.encodeName("🇭🇰 [A] HK 2 家宽")
        assertThat(encoded).doesNotContain("+")
        assertThat(encoded).contains("%20")
        assertThat(encoded).contains("%5B")
        assertThat(encoded).contains("%5D")
        assertThat(ClashApiPath.encodeName("HK-01")).isEqualTo("HK-01")
    }

    @Test
    fun latencySortPutsTimeoutAndUntestedAfterLiveDelays() {
        val nodes = listOf(
            ProxyNode("HK", "Vless", delayMs = 80),
            ProxyNode("US", "Vless", delayMs = 0),
            ProxyNode("JP", "Vless", delayMs = null),
        )
        val sorted = ClashApiParser.sortNodes(nodes, SortMode.LATENCY)
        assertThat(sorted.map { it.name }).containsExactly("HK", "US", "JP").inOrder()
    }

    @Test
    fun mergeDelaysKeepsLocalResultWhenApiHistoryEmpty() {
        val prev = listOf(
            GroupWithNodes(
                group = ProxyGroup("自动选择", "URLTest", "HK", listOf("HK", "US")),
                nodes = listOf(
                    ProxyNode("HK", "Vless", delayMs = 217),
                    ProxyNode("US", "Vless", delayMs = 0),
                ),
            ),
        )
        val fresh = listOf(
            GroupWithNodes(
                group = ProxyGroup("自动选择", "URLTest", "HK", listOf("HK", "US")),
                nodes = listOf(
                    ProxyNode("HK", "Vless", delayMs = null),
                    ProxyNode("US", "Vless", delayMs = null),
                ),
            ),
        )
        val merged = ClashApiParser.mergeDelays(fresh, prev)
        assertThat(merged.single().nodes.first { it.name == "HK" }.delayMs).isEqualTo(217)
        assertThat(merged.single().nodes.first { it.name == "US" }.delayMs).isEqualTo(0)
    }
}
