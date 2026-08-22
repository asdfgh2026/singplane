package app.singplane.assemble

import app.singplane.model.ContentKind
import org.json.JSONObject

object ContentDetector {
    private val uriSchemes = listOf(
        "ss://", "ssr://", "vmess://", "vless://", "trojan://",
        "hysteria://", "hysteria2://", "hy2://", "tuic://",
        "wireguard://", "wg://", "anytls://", "socks://",
    )

    fun detect(body: String): ContentKind {
        val text = body.trim()
        if (text.isEmpty()) return ContentKind.Unknown

        if (text.startsWith("{")) {
            runCatching {
                val v = JSONObject(text)
                if (v.has("outbounds") || v.has("inbounds") || v.has("endpoints")) {
                    return ContentKind.Singbox
                }
            }
        }

        val lines = text.split('\n', '\r').map { it.trim() }.filter { it.isNotEmpty() }
        if (lines.isNotEmpty()) {
            val uriLike = lines.count { looksLikeNodeUri(it) }
            if (uriLike >= 1 && uriLike >= (lines.size + 1) / 2) {
                return ContentKind.UriList
            }
        }

        val lower = text.lowercase()
        if (lower.contains("proxies:") || lower.contains("proxy-groups:") ||
            lower.contains("proxy-providers:")
        ) {
            if (lower.contains("proxies:") || Regex("""^\s*-\s*name\s*:""", RegexOption.MULTILINE).containsMatchIn(text)) {
                return ContentKind.Clash
            }
        }
        return ContentKind.Unknown
    }

    fun isRunnable(content: String): Boolean {
        return runCatching {
            val v = JSONObject(content)
            v.has("outbounds") || v.has("inbounds")
        }.getOrDefault(false)
    }

    private fun looksLikeNodeUri(line: String): Boolean {
        val s = line.trim().lowercase()
        if (s.startsWith("http://") || s.startsWith("https://")) {
            return s.contains('@') || s.contains('#')
        }
        return uriSchemes.any { s.startsWith(it) }
    }
}
