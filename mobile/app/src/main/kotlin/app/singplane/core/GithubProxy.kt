package app.singplane.core

data class GithubProxyPreset(
    val id: String,
    val label: String,
    val prefix: String,
)

object GithubProxy {
    val PRESETS = listOf(
        GithubProxyPreset(id = "direct", label = "直连", prefix = ""),
        GithubProxyPreset(id = "ghfast", label = "加速 1", prefix = "https://ghfast.top"),
        GithubProxyPreset(id = "gh-proxy", label = "加速 2", prefix = "https://gh-proxy.com"),
        GithubProxyPreset(id = "ghproxy-net", label = "加速 3", prefix = "https://ghproxy.net"),
    )

    fun normalize(raw: String): String =
        raw.trim().trimEnd('/')

    fun findPreset(raw: String): GithubProxyPreset? {
        val norm = normalize(raw)
        return PRESETS.firstOrNull { normalize(it.prefix) == norm }
    }

    fun applyProxy(url: String, proxy: String): String {
        val p = normalize(proxy)
        if (p.isEmpty() || url.startsWith(p) || !isGithubUrl(url)) {
            return url
        }
        return "$p/$url"
    }

    fun isGithubUrl(url: String): Boolean {
        val hosts = listOf(
            "https://github.com/",
            "https://api.github.com/",
            "https://objects.githubusercontent.com/",
            "https://release-assets.githubusercontent.com/",
            "https://codeload.github.com/",
            "https://raw.githubusercontent.com/",
            "https://gist.githubusercontent.com/",
            "https://gist.github.com/",
        )
        return hosts.any { url.startsWith(it) }
    }
}
