package app.singplane.clash

import java.net.URLEncoder

/** Clash `/proxies/{name}` path encoding. `URLEncoder` uses `+` for space; the API needs `%20`. */
object ClashApiPath {
    fun encodeName(name: String): String =
        URLEncoder.encode(name, Charsets.UTF_8.name()).replace("+", "%20")
}
