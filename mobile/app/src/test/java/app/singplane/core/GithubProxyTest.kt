package app.singplane.core

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class GithubProxyTest {
    @Test
    fun normalizeStripsSlashAndWhitespace() {
        assertThat(GithubProxy.normalize(" https://ghfast.top/ ")).isEqualTo("https://ghfast.top")
        assertThat(GithubProxy.normalize("")).isEqualTo("")
        assertThat(GithubProxy.normalize("   ")).isEqualTo("")
    }

    @Test
    fun applyEmptyIsDirect() {
        val url = "https://api.github.com/repos/SagerNet/sing-box/releases/latest"
        assertThat(GithubProxy.applyProxy(url, "")).isEqualTo(url)
        assertThat(GithubProxy.applyProxy(url, "   ")).isEqualTo(url)
    }

    @Test
    fun applyWrapsGithubHosts() {
        assertThat(
            GithubProxy.applyProxy(
                "https://github.com/SagerNet/sing-box/releases/download/v1.12.0/x.tar.gz",
                "https://ghfast.top/",
            ),
        ).isEqualTo("https://ghfast.top/https://github.com/SagerNet/sing-box/releases/download/v1.12.0/x.tar.gz")

        assertThat(
            GithubProxy.applyProxy(
                "https://api.github.com/repos/SagerNet/sing-box/releases?per_page=30",
                "https://gh-proxy.com",
            ),
        ).isEqualTo("https://gh-proxy.com/https://api.github.com/repos/SagerNet/sing-box/releases?per_page=30")
    }

    @Test
    fun applyDoesNotDoubleWrap() {
        val already = "https://ghfast.top/https://api.github.com/repos/x"
        assertThat(GithubProxy.applyProxy(already, "https://ghfast.top")).isEqualTo(already)
    }

    @Test
    fun applySkipsNonGithub() {
        assertThat(
            GithubProxy.applyProxy("https://example.com/a", "https://ghfast.top"),
        ).isEqualTo("https://example.com/a")
    }

    @Test
    fun matchingPreset() {
        assertThat(GithubProxy.findPreset("https://ghproxy.net/")?.id).isEqualTo("ghproxy-net")
        assertThat(GithubProxy.findPreset("")?.id).isEqualTo("direct")
        assertThat(GithubProxy.findPreset("https://unknown.domain")?.id).isNull()
    }
}
