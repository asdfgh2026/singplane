package app.singplane.model

import com.google.common.truth.Truth.assertThat
import org.json.JSONObject
import org.junit.Test

class ProfileJsonTest {
    @Test
    fun roundTripProfileKeys() {
        val p = Profile(
            id = "abc",
            name = "sub",
            sourceType = "url",
            url = "https://example.com/sub",
            content = """{"outbounds":[]}""",
            updatedAtMs = 1_700_000_000_000L,
            upload = 10,
            download = 20,
            total = 100,
            expireMs = 2_000L,
            runnable = true,
        )
        val again = Profile.fromJson(JSONObject(p.toJson().toString()))
        assertThat(again.id).isEqualTo("abc")
        assertThat(again.name).isEqualTo("sub")
        assertThat(again.sourceType).isEqualTo("url")
        assertThat(again.url).isEqualTo("https://example.com/sub")
        assertThat(again.upload).isEqualTo(10)
        assertThat(again.download).isEqualTo(20)
        assertThat(again.total).isEqualTo(100)
        assertThat(again.expireMs).isEqualTo(2_000L)
        assertThat(again.runnable).isTrue()
    }

    @Test
    fun trafficLabel() {
        val p = Profile(
            id = "1",
            name = "n",
            content = "{}",
            upload = 512,
            download = 512,
            total = 2048,
        )
        assertThat(p.trafficLabel).contains("KB")
        assertThat(p.trafficLabel).contains("/")
    }

    @Test
    fun prettyContentFormatsJson() {
        val out = Profile.prettyContent("""{"inbounds":[{"type":"tun"}]}""")
        assertThat(out).contains("\n")
        assertThat(out).contains("inbounds")
    }

    @Test
    fun prettyContentKeepsNonJson() {
        assertThat(Profile.prettyContent("proxies:\n  - a")).isEqualTo("proxies:\n  - a")
        assertThat(Profile.prettyContent("   ")).isEmpty()
    }
}
