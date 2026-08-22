package app.singplane.clash

/** Client URL for sing-box `experimental.clash_api`. Listen-all hosts are not connectable. */
object ClashApiAddress {
    fun httpBase(host: String, port: Int): String {
        val trimmed = host.trim()
        val loopback = when (trimmed.lowercase()) {
            "", "0.0.0.0", "::", "[::]", "localhost", "::1", "[::1]" -> true
            else -> false
        }
        val h = if (loopback) "127.0.0.1" else trimmed
        return "http://$h:$port"
    }
}
