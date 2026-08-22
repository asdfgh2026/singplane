package app.singplane.assemble

import com.google.common.truth.Truth.assertThat
import app.singplane.model.ContentKind
import org.junit.Test

class ContentDetectorTest {
    @Test
    fun emptyIsUnknown() {
        assertThat(ContentDetector.detect("")).isEqualTo(ContentKind.Unknown)
        assertThat(ContentDetector.detect("   ")).isEqualTo(ContentKind.Unknown)
    }

    @Test
    fun singboxJson() {
        val body = """{"inbounds":[],"outbounds":[{"type":"direct","tag":"d"}]}"""
        assertThat(ContentDetector.detect(body)).isEqualTo(ContentKind.Singbox)
    }

    @Test
    fun uriList() {
        val body = """
            ss://aaaa@host:1#n1
            vmess://bbbb
        """.trimIndent()
        assertThat(ContentDetector.detect(body)).isEqualTo(ContentKind.UriList)
    }

    @Test
    fun clashYaml() {
        val body = """
            proxies:
              - name: a
                type: ss
            proxy-groups:
              - name: g
                type: select
        """.trimIndent()
        assertThat(ContentDetector.detect(body)).isEqualTo(ContentKind.Clash)
    }

    @Test
    fun runnableSingbox() {
        assertThat(ContentDetector.isRunnable("""{"outbounds":[]}""")).isTrue()
        assertThat(ContentDetector.isRunnable("ss://x")).isFalse()
        assertThat(ContentDetector.isRunnable("{")).isFalse()
    }
}
