package app.singplane.clash

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class ClashConnectionTest {

    @Test
    fun parseConnectionsJson() {
        val json = """
            {
              "downloadTotal": 1048576,
              "uploadTotal": 524288,
              "connections": [
                {
                  "id": "c-001",
                  "metadata": {
                    "network": "tcp",
                    "type": "HTTP",
                    "sourceIP": "127.0.0.1",
                    "destinationIP": "104.21.56.78",
                    "sourcePort": "54321",
                    "destinationPort": "443",
                    "host": "api.github.com",
                    "processPath": "/data/app/com.github.android/base.apk"
                  },
                  "upload": 2048,
                  "download": 8192,
                  "start": "2026-08-17T06:00:00.000Z",
                  "chains": ["Proxy", "HK-Node"],
                  "rule": "Match",
                  "rulePayload": ""
                }
              ]
            }
        """.trimIndent()

        val snap = ClashConnectionParser.parse(json)
        assertThat(snap.downloadTotal).isEqualTo(1048576L)
        assertThat(snap.uploadTotal).isEqualTo(524288L)
        assertThat(snap.connections).hasSize(1)

        val conn = snap.connections[0]
        assertThat(conn.id).isEqualTo("c-001")
        assertThat(conn.host).isEqualTo("api.github.com")
        assertThat(conn.destination).isEqualTo("104.21.56.78:443")
        assertThat(conn.network).isEqualTo("tcp")
        assertThat(conn.process).isEqualTo("base.apk")
        assertThat(conn.chains).containsExactly("Proxy", "HK-Node").inOrder()
        assertThat(conn.upload).isEqualTo(2048L)
        assertThat(conn.download).isEqualTo(8192L)
    }

    @Test
    fun parseLiveTunConnectionWithUidProcess() {
        val json = """
            {
              "downloadTotal": 2376541,
              "uploadTotal": 221341,
              "connections": [
                {
                  "id": "2be8c7b2-57b3-486c-9d48-247839a34df8",
                  "chains": ["direct"],
                  "download": 290714,
                  "upload": 4096,
                  "rule": "ip_is_private=true => route(direct)",
                  "start": "2026-08-17T18:59:00.000+08:00",
                  "metadata": {
                    "destinationIP": "10.0.0.1",
                    "destinationPort": "8888",
                    "dnsMode": "normal",
                    "host": "video.twimg.com",
                    "network": "tcp",
                    "processPath": "10321",
                    "sourceIP": "172.19.0.1",
                    "sourcePort": "35422",
                    "type": "tun/tun"
                  }
                }
              ]
            }
        """.trimIndent()

        val snap = ClashConnectionParser.parse(json)
        assertThat(snap.connections).hasSize(1)
        val conn = snap.connections.single()
        assertThat(conn.host).isEqualTo("video.twimg.com")
        assertThat(conn.destination).isEqualTo("10.0.0.1:8888")
        assertThat(conn.process).isEqualTo("10321")
        assertThat(conn.chains).containsExactly("direct")
        assertThat(conn.rule).contains("ip_is_private")
    }

    @Test
    fun calculateSpeedDiff() {
        val prev = listOf(
            ClashConnection(id = "c-1", upload = 1000, download = 2000),
            ClashConnection(id = "c-2", upload = 500, download = 500),
        )
        val current = listOf(
            ClashConnection(id = "c-1", upload = 1500, download = 3000), // diff: up=500, down=1000
            ClashConnection(id = "c-3", upload = 800, download = 1200), // new: diff=0
        )

        val withSpeed = ClashConnectionParser.computeSpeeds(current, prev, intervalSec = 1.0)
        val c1 = withSpeed.find { it.id == "c-1" }
        val c3 = withSpeed.find { it.id == "c-3" }

        assertThat(c1?.uploadSpeed).isEqualTo(500L)
        assertThat(c1?.downloadSpeed).isEqualTo(1000L)
        assertThat(c3?.uploadSpeed).isEqualTo(0L)
        assertThat(c3?.downloadSpeed).isEqualTo(0L)
    }

    @Test
    fun filterAndSortConnections() {
        val items = listOf(
            ClashConnection(id = "1", host = "google.com", process = "chrome", uploadSpeed = 100, download = 1000),
            ClashConnection(id = "2", host = "github.com", process = "git", uploadSpeed = 500, download = 500),
            ClashConnection(id = "3", host = "youtube.com", process = "chrome", uploadSpeed = 50, download = 5000),
        )

        // Filter
        val filtered = ClashConnectionParser.filter(items, "git")
        assertThat(filtered.map { it.id }).containsExactly("2")

        // Sort by speed
        val bySpeed = ClashConnectionParser.sort(items, ConnSortMode.SPEED)
        assertThat(bySpeed.map { it.id }).containsExactly("2", "1", "3")

        // Sort by traffic
        val byTraffic = ClashConnectionParser.sort(items, ConnSortMode.TRAFFIC)
        assertThat(byTraffic.map { it.id }).containsExactly("3", "1", "2")
    }

    @Test
    fun shortensProcessAndRuleForCard() {
        assertThat(
            ClashConnectionParser.shortProcess("com.google.android.gms (com.google.android.gms)"),
        ).isEqualTo("gms")
        assertThat(ClashConnectionParser.shortProcess("/data/app/chrome/base.apk")).isEqualTo("base.apk")
        assertThat(ClashConnectionParser.shortProcess("10321")).isEqualTo("10321")
        assertThat(
            ClashConnectionParser.shortRule(
                "domain_suffix=[a.example b.example] => route(proxy)",
            ),
        ).isEqualTo("domain_suffix → proxy")
    }
}
