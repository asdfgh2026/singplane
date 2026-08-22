package app.singplane.fetch

data class Userinfo(
    val upload: Long = 0,
    val download: Long = 0,
    val total: Long = 0,
    val expireMs: Long = 0,
)

object UserinfoParser {
    fun parse(header: String?): Userinfo {
        if (header.isNullOrBlank()) return Userinfo()
        var upload = 0L
        var download = 0L
        var total = 0L
        var expireMs = 0L
        header.split(';').forEach { part ->
            val kv = part.trim().split('=', limit = 2)
            if (kv.size != 2) return@forEach
            val key = kv[0].trim().lowercase()
            val value = kv[1].trim().toLongOrNull() ?: return@forEach
            when (key) {
                "upload" -> upload = value
                "download" -> download = value
                "total" -> total = value
                "expire" -> expireMs = if (value < 10_000_000_000L) value * 1000 else value
            }
        }
        return Userinfo(upload, download, total, expireMs)
    }
}
