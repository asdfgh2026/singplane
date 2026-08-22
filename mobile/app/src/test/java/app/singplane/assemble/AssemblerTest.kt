package app.singplane.assemble

import com.google.common.truth.Truth.assertThat
import app.singplane.model.ContentKind
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Test

class AssemblerTest {
    private val directTemplate = """
        {
          "inbounds": [
            {
              "type": "mixed",
              "listen": "127.0.0.1",
              "listen_port": 7890
            }
          ],
          "outbounds": [
            {
              "type": "direct",
              "tag": "direct"
            },
            {
              "type": "selector",
              "tag": "select",
              "outbounds": ["direct"]
            }
          ]
        }
    """.trimIndent()

    @Test
    fun assembleSingboxNodesIntoTemplate() {
        val source = """
            {
              "outbounds": [
                { "type": "vless", "tag": "HK-Node", "server": "1.2.3.4", "server_port": 443 },
                { "type": "shadowsocks", "tag": "JP-Node", "server": "5.6.7.8", "server_port": 8388 },
                { "type": "direct", "tag": "direct" },
                { "type": "block", "tag": "block" }
              ]
            }
        """.trimIndent()

        val res = Assembler.assemble(source, directTemplate)
        assertThat(res.ok).isTrue()
        assertThat(res.detectedKind).isEqualTo(ContentKind.Singbox)

        val cfg = res.config!!
        val outbounds = cfg.getJSONArray("outbounds")
        val tags = (0 until outbounds.length()).map { outbounds.getJSONObject(it).getString("tag") }
        assertThat(tags).containsAtLeast("direct", "select", "HK-Node", "JP-Node")
        assertThat(tags).doesNotContain("block")

        // Selector group includes the injected nodes
        val selector = (0 until outbounds.length())
            .map { outbounds.getJSONObject(it) }
            .first { it.getString("tag") == "select" }
        val selOutbounds = selector.getJSONArray("outbounds")
        val selList = (0 until selOutbounds.length()).map { selOutbounds.getString(it) }
        assertThat(selList).containsAtLeast("HK-Node", "JP-Node")
    }

    @Test
    fun assembleZeroNodesFails() {
        val source = """
            {
              "outbounds": [
                { "type": "direct", "tag": "direct" },
                { "type": "block", "tag": "block" }
              ]
            }
        """.trimIndent()

        val res = Assembler.assemble(source, directTemplate)
        assertThat(res.ok).isFalse()
        assertThat(res.error).contains("0 个节点")
    }

    @Test
    fun assembleInvalidTemplateFails() {
        val source = """{"outbounds":[{"type":"vless","tag":"n1"}]}"""
        val res = Assembler.assemble(source, "not json")
        assertThat(res.ok).isFalse()
        assertThat(res.error).contains("模板无效")
    }

    @Test
    fun assembleIncludeExcludeRegex() {
        val source = """
            {
              "outbounds": [
                { "type": "vless", "tag": "HK-01" },
                { "type": "vless", "tag": "HK-02" },
                { "type": "vless", "tag": "US-01" }
              ]
            }
        """.trimIndent()

        val res = Assembler.assemble(
            source,
            directTemplate,
            options = AssembleOptions(include = "HK-.*", exclude = ".*02"),
        )
        assertThat(res.ok).isTrue()
        val outbounds = res.config!!.getJSONArray("outbounds")
        val tags = (0 until outbounds.length()).map { outbounds.getJSONObject(it).getString("tag") }
        assertThat(tags).contains("HK-01")
        assertThat(tags).doesNotContain("HK-02")
        assertThat(tags).doesNotContain("US-01")
    }

    @Test
    fun assembleUniqueTagDeduplication() {
        val source = """
            {
              "outbounds": [
                { "type": "vless", "tag": "direct" },
                { "type": "vless", "tag": "node" },
                { "type": "vless", "tag": "node" }
              ]
            }
        """.trimIndent()

        val res = Assembler.assemble(source, directTemplate)
        assertThat(res.ok).isTrue()
        val outbounds = res.config!!.getJSONArray("outbounds")
        val tags = (0 until outbounds.length()).map { outbounds.getJSONObject(it).getString("tag") }
        // "direct" already in template, so source node becomes "direct-2"
        assertThat(tags).contains("direct-2")
        assertThat(tags).contains("node")
        assertThat(tags).contains("node-2")
    }
}
