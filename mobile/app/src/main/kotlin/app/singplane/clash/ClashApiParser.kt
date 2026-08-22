package app.singplane.clash

import androidx.annotation.StringRes
import app.singplane.R
import org.json.JSONObject

data class ProxyGroup(
    val name: String,
    val type: String,
    val now: String,
    val all: List<String>,
) {
    val selectable: Boolean get() = type.equals("Selector", ignoreCase = true)
}

data class ProxyNode(
    val name: String,
    val type: String,
    val delayMs: Int? = null,
)

data class GroupWithNodes(
    val group: ProxyGroup,
    val nodes: List<ProxyNode>,
)

enum class SortMode(@StringRes val labelRes: Int) {
    DEFAULT(R.string.proxies_sort_default),
    LATENCY(R.string.proxies_sort_latency),
    NAME(R.string.proxies_sort_name),
}

object ClashApiParser {
    private val skipNames = setOf("DIRECT", "REJECT", "GLOBAL", "COMPATIBLE", "PASS", "REJECT-DROP")
    private val noticeNeedles = listOf(
        "剩余流量", "套餐到期", "官网", "距離下次", "距离下次", "重置剩余", "重置剩餘", "到期时间",
    )

    fun parseMode(body: String): String {
        return runCatching {
            JSONObject(body).optString("mode", "rule")
        }.getOrDefault("rule")
    }

    fun parseMemoryInuse(body: String): Long {
        return runCatching {
            JSONObject(body).optLong("inuse", 0L)
        }.getOrDefault(0L)
    }

    fun groups(body: String): List<ProxyGroup> {
        val root = JSONObject(body)
        val proxies = root.optJSONObject("proxies") ?: return emptyList()
        val out = ArrayList<ProxyGroup>()
        val keys = proxies.keys()
        while (keys.hasNext()) {
            val name = keys.next()
            if (name.uppercase() in skipNames) continue
            val o = proxies.optJSONObject(name) ?: continue
            val type = o.optString("type")
            if (!isGroupType(type)) continue
            val allArr = o.optJSONArray("all")
            val all = if (allArr == null) emptyList() else (0 until allArr.length()).map { allArr.optString(it) }
            out.add(
                ProxyGroup(
                    name = o.optString("name").ifEmpty { name },
                    type = type,
                    now = o.optString("now"),
                    all = all,
                ),
            )
        }
        return out.sortedBy { it.name }
    }

    fun parseGroupNodes(body: String): List<GroupWithNodes> {
        val root = JSONObject(body)
        val proxies = root.optJSONObject("proxies") ?: return emptyList()
        val groupList = groups(body)

        return groupList.map { grp ->
            val nodes = grp.all.map { nodeName ->
                val nodeObj = proxies.optJSONObject(nodeName)
                val nodeType = nodeObj?.optString("type") ?: "Unknown"
                val history = nodeObj?.optJSONArray("history")
                var lastDelay: Int? = null
                if (history != null && history.length() > 0) {
                    val last = history.optJSONObject(history.length() - 1)
                    val d = last?.optInt("delay", 0) ?: 0
                    if (d > 0) lastDelay = d
                }
                ProxyNode(
                    name = nodeName,
                    type = nodeType,
                    delayMs = lastDelay,
                )
            }
            GroupWithNodes(group = grp, nodes = nodes)
        }
    }

    fun filterNodes(nodes: List<ProxyNode>, query: String): List<ProxyNode> {
        if (query.isBlank()) return nodes
        val q = query.trim().lowercase()
        return nodes.filter { it.name.lowercase().contains(q) || it.type.lowercase().contains(q) }
    }

    fun sortNodes(nodes: List<ProxyNode>, sortMode: SortMode): List<ProxyNode> {
        return when (sortMode) {
            SortMode.DEFAULT -> nodes
            SortMode.NAME -> nodes.sortedWith(compareBy(java.text.Collator.getInstance()) { it.name })
            SortMode.LATENCY -> nodes.sortedWith(
                compareBy<ProxyNode> { latencyRank(it.delayMs) }
                    .thenBy { it.delayMs?.takeIf { d -> d > 0 } ?: Int.MAX_VALUE }
                    .thenBy(java.text.Collator.getInstance()) { it.name },
            )
        }
    }

    fun isSubscriptionNotice(name: String): Boolean {
        val n = name.lowercase()
        return noticeNeedles.any { needle -> n.contains(needle.lowercase()) }
    }

    fun visibleNodes(nodes: List<ProxyNode>): List<ProxyNode> =
        nodes.filter { !isSubscriptionNotice(it.name) }

    fun mergeDelays(fresh: List<GroupWithNodes>, prev: List<GroupWithNodes>): List<GroupWithNodes> {
        val prevMap = prev.flatMap { it.nodes }.associate { it.name to it.delayMs }
        return fresh.map { gn ->
            gn.copy(
                nodes = gn.nodes.map { n ->
                    if (n.delayMs != null && n.delayMs > 0) n
                    else n.copy(delayMs = prevMap[n.name] ?: n.delayMs)
                },
            )
        }
    }

    private fun latencyRank(delayMs: Int?): Int = when {
        delayMs != null && delayMs > 0 -> 0
        delayMs != null -> 1
        else -> 2
    }

    private fun isGroupType(type: String): Boolean {
        val t = type.lowercase()
        return t == "selector" || t == "urltest" || t == "fallback" || t == "loadbalance"
    }
}
