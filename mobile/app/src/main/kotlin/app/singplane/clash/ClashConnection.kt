package app.singplane.clash

import androidx.annotation.StringRes
import app.singplane.R
import org.json.JSONObject
import java.io.File
import kotlin.math.max

data class ClashConnection(
    val id: String,
    val host: String = "",
    val destination: String = "",
    val network: String = "",
    val process: String = "",
    val chains: List<String> = emptyList(),
    val rule: String = "",
    val upload: Long = 0L,
    val download: Long = 0L,
    val uploadSpeed: Long = 0L,
    val downloadSpeed: Long = 0L,
    val start: String = "",
)

data class ConnectionsSnapshot(
    val downloadTotal: Long = 0L,
    val uploadTotal: Long = 0L,
    val connections: List<ClashConnection> = emptyList(),
)

enum class ConnSortMode(@StringRes val labelRes: Int) {
    DEFAULT(R.string.connections_sort_default),
    SPEED(R.string.connections_sort_speed),
    TRAFFIC(R.string.connections_sort_traffic),
}

object ClashConnectionParser {

    fun parse(body: String): ConnectionsSnapshot {
        return runCatching {
            val root = JSONObject(body)
            val downTotal = root.optLong("downloadTotal", 0L)
            val upTotal = root.optLong("uploadTotal", 0L)
            val arr = root.optJSONArray("connections") ?: return ConnectionsSnapshot(downTotal, upTotal)

            val list = mutableListOf<ClashConnection>()
            for (i in 0 until arr.length()) {
                val item = arr.optJSONObject(i) ?: continue
                val id = item.optString("id")
                if (id.isEmpty()) continue

                val meta = item.optJSONObject("metadata")
                val host = meta?.optString("host").orEmpty().ifEmpty {
                    meta?.optString("destinationIP").orEmpty()
                }
                val dstIp = meta?.optString("destinationIP").orEmpty()
                val dstPort = meta?.optString("destinationPort").orEmpty()
                val destination = if (dstIp.isNotEmpty() && dstPort.isNotEmpty()) "$dstIp:$dstPort" else dstIp
                val network = meta?.optString("network").orEmpty()
                val rawProcess = meta?.optString("processPath").orEmpty()
                val process = if (rawProcess.isNotEmpty()) File(rawProcess).name else ""

                val chainsArr = item.optJSONArray("chains")
                val chains = if (chainsArr != null) {
                    (0 until chainsArr.length()).map { chainsArr.optString(it) }
                } else emptyList()

                val rule = item.optString("rule")
                val upload = item.optLong("upload", 0L)
                val download = item.optLong("download", 0L)
                val start = item.optString("start")

                list.add(
                    ClashConnection(
                        id = id,
                        host = host,
                        destination = destination,
                        network = network,
                        process = process,
                        chains = chains,
                        rule = rule,
                        upload = upload,
                        download = download,
                        start = start,
                    ),
                )
            }
            ConnectionsSnapshot(downTotal, upTotal, list)
        }.getOrDefault(ConnectionsSnapshot())
    }

    fun computeSpeeds(
        current: List<ClashConnection>,
        prev: List<ClashConnection>,
        intervalSec: Double = 1.0,
    ): List<ClashConnection> {
        val prevMap = prev.associateBy { it.id }
        val factor = if (intervalSec > 0) 1.0 / intervalSec else 1.0

        return current.map { cur ->
            val p = prevMap[cur.id]
            if (p != null) {
                val upDiff = max(0L, cur.upload - p.upload)
                val downDiff = max(0L, cur.download - p.download)
                cur.copy(
                    uploadSpeed = (upDiff * factor).toLong(),
                    downloadSpeed = (downDiff * factor).toLong(),
                )
            } else {
                cur.copy(uploadSpeed = 0L, downloadSpeed = 0L)
            }
        }
    }

    fun filter(items: List<ClashConnection>, query: String): List<ClashConnection> {
        if (query.isBlank()) return items
        val q = query.trim().lowercase()
        return items.filter {
            it.host.lowercase().contains(q) ||
                it.destination.lowercase().contains(q) ||
                it.process.lowercase().contains(q) ||
                it.rule.lowercase().contains(q) ||
                it.chains.any { c -> c.lowercase().contains(q) }
        }
    }

    fun sort(items: List<ClashConnection>, mode: ConnSortMode): List<ClashConnection> {
        return when (mode) {
            ConnSortMode.DEFAULT -> items
            ConnSortMode.SPEED -> items.sortedByDescending { it.uploadSpeed + it.downloadSpeed }
            ConnSortMode.TRAFFIC -> items.sortedByDescending { it.upload + it.download }
        }
    }

    fun shortProcess(raw: String): String {
        val base = raw.substringBefore('(').trim().ifEmpty { raw.trim() }
        val leaf = base.substringAfterLast('/').substringAfterLast('\\')
        val parts = leaf.split('.')
        val ext = parts.last().lowercase()
        return when {
            parts.size >= 2 && ext in setOf("apk", "so", "exe", "dll", "bin") -> leaf
            parts.size >= 3 && parts.all { seg -> seg.all { c -> c.isLetterOrDigit() || c == '_' } } -> parts.last()
            else -> leaf
        }
    }

    fun shortRule(rule: String): String {
        var s = rule
            .replace(Regex("=\\[[^\\]]*\\]"), "")
            .replace("route(", "")
            .replace(")", "")
            .replace("=>", "→")
            .replace(Regex("\\s+"), " ")
            .trim()
        if (s.length > 42) s = s.take(40).trimEnd() + "…"
        return s
    }
}
