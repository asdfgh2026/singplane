package app.singplane.assemble

import app.singplane.model.TailscaleSettings
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.Base64
import java.util.Locale

enum class TsPhase {
    Disabled, Ready, Pending, Injected, NeedsLogin, Error
}

data class TsStatus(
    val phase: TsPhase,
    val title: String,
    val subtitle: String,
    val loginUrl: String?,
    val selfIp: String?,
    val hostname: String?,
)

data class TsIdentity(
    val loggedIn: Boolean = false,
    val displayName: String? = null,
    val hostname: String? = null,
    val magicDns: String? = null,
    val selfIp: String? = null,
) {
    val joined: Boolean get() = loggedIn || selfIp != null
    val label: String
        get() = selfIp ?: displayName ?: hostname ?: magicDns ?: "已加入 tailnet"
}

object TailscaleStatus {

    data class Hint(
        val kind: HintKind,
        val loginUrl: String?,
        val detail: String?
    )

    enum class HintKind {
        Connected, WaitingAuth, Error, None
    }

    fun isTailscaleIp(ip: String): Boolean {
        val parts = ip.split(".")
        if (parts.size != 4) return false
        val a = parts[0].toIntOrNull()
        val b = parts[1].toIntOrNull()
        return a == 100 && b != null && b in 64..127
    }

    fun latestHint(log: String): Hint {
        if (log.isEmpty()) return Hint(HintKind.None, null, null)
        val lines = log.lines()
        for (line in lines.reversed()) {
            val lower = line.lowercase(Locale.getDefault())
            val isTs = lower.contains("tailscale") || lower.contains("ts-local") || lower.contains("ts-local-dns")
            if (!isTs) continue

            val magicJoin = magicdnsJoinDetail(lower, line)
            if (magicJoin != null) {
                return Hint(HintKind.Connected, null, magicJoin)
            }
            if (lower.contains("backend: running") ||
                lower.contains("switching ipn state to running") ||
                lower.contains("logged in") ||
                lower.contains("connected to control")
            ) {
                return Hint(HintKind.Connected, null, "已加入 tailnet")
            }
            if (lower.contains("waiting for authentication")) {
                return Hint(HintKind.WaitingAuth, urlFromLine(line), null)
            }
            if (lower.contains("endpoint/tailscale") &&
                (lower.contains("error") || lower.contains("failed") || lower.contains("fatal") || lower.contains("denied"))
            ) {
                return Hint(HintKind.Error, null, line.trim().take(120))
            }
        }
        return Hint(HintKind.None, null, null)
    }

    private fun magicdnsJoinDetail(lower: String, line: String): String? {
        if (!lower.contains("updated")) return null
        if (!(lower.contains("routes") || lower.contains("hosts") || lower.contains("search domain"))) return null
        val routes = captureCount(lower, "routes")
        val hosts = captureCount(lower, "hosts")
        if ((routes ?: 0) == 0 && (hosts ?: 0) == 0) return null
        val parts = line.split("updated")
        return if (parts.size > 1) {
            "MagicDNS · ${parts[1].trim()}"
        } else {
            "已加入 tailnet"
        }
    }

    private fun captureCount(lower: String, label: String): Int? {
        val needle = " $label"
        val idx = lower.indexOf(needle)
        if (idx == -1) return null
        val before = lower.substring(0, idx)
        val num = before.split(Regex("[^0-9]")).lastOrNull { it.isNotEmpty() }
        return num?.toIntOrNull()
    }

    fun urlFromLine(line: String): String? {
        val lower = line.lowercase(Locale.getDefault())
        val idx = lower.indexOf("https://login.tailscale.com/")
        if (idx != -1) {
            val rest = line.substring(idx)
            val endIdx = rest.indexOfFirst { it.isWhitespace() || it == '"' || it == '\'' || it == ')' || it == ']' || it == ',' || it == ';' }
            val end = if (endIdx == -1) rest.length else endIdx
            return rest.substring(0, end).trimEnd('.', ',', ';')
        }
        val waitIdx = lower.indexOf("waiting for authentication:")
        if (waitIdx != -1) {
            val rest = line.substring(waitIdx)
            val parts = rest.split("\\s+".toRegex())
            if (parts.size >= 4 && parts[3].startsWith("http")) {
                return parts[3].trimEnd('.', ',', ';')
            }
        }
        return null
    }

    private fun urlAnywhere(log: String): String? {
        for (line in log.lines().reversed()) {
            val u = urlFromLine(line)
            if (u != null) return u
        }
        return null
    }

    fun pendingStatus(ts: TailscaleSettings, loginUrl: String?, hostname: String?): TsStatus {
        if (loginUrl != null) {
            return TsStatus(TsPhase.NeedsLogin, "等待授权", loginUrl, loginUrl, null, hostname)
        }
        return if (ts.usesDeviceAuth()) {
            TsStatus(TsPhase.Pending, "验证中", "无 Auth Key · 等 login.tailscale.com 链接", null, null, hostname)
        } else {
            TsStatus(TsPhase.Pending, "验证中", "Auth Key 登录中…", null, null, hostname)
        }
    }

    fun statusFromLog(
        ts: TailscaleSettings,
        running: Boolean,
        log: String,
        ident: TsIdentity? = null,
    ): TsStatus {
        if (!ts.enabled) {
            return TsStatus(TsPhase.Disabled, "未启用", "点卡片开启 · 官方内核 ≥1.13", null, null, null)
        }
        if (!running) {
            return TsStatus(TsPhase.Ready, "已开启", "下次启动内核时注入", null, null, null)
        }

        if (ident?.joined == true) {
            return TsStatus(
                TsPhase.Injected,
                "已加入",
                ident.label,
                null,
                ident.selfIp,
                ident.displayName ?: ident.hostname,
            )
        }

        val hint = latestHint(log)
        return when (hint.kind) {
            HintKind.Connected -> TsStatus(
                TsPhase.Injected,
                "已加入",
                hint.detail ?: "已加入 tailnet",
                null,
                ident?.selfIp,
                ident?.hostname,
            )
            HintKind.WaitingAuth -> TsStatus(
                TsPhase.NeedsLogin,
                "等待授权",
                hint.loginUrl ?: "日志里会有 login.tailscale.com 链接",
                hint.loginUrl,
                null,
                ident?.hostname,
            )
            HintKind.Error -> TsStatus(
                TsPhase.Error,
                "出错",
                hint.detail ?: "见内核日志",
                null,
                null,
                ident?.hostname,
            )
            HintKind.None -> pendingStatus(ts, hint.loginUrl, ident?.hostname)
        }
    }

    fun parseState(raw: String): TsIdentity {
        val root = runCatching { JSONObject(raw) }.getOrNull() ?: return TsIdentity()
        val blobs = mutableListOf(root)
        val keys = root.keys()
        while (keys.hasNext()) {
            val decoded = decodeB64Json(root.optString(keys.next(), ""))
            if (decoded != null) blobs.add(decoded)
        }
        var loggedOut: Boolean? = null
        var displayName: String? = null
        var hostname: String? = null
        var magicDns: String? = null
        var loginName: String? = null
        var selfIp: String? = null
        var nodeId: String? = null

        fun considerName(value: String?, slot: (String) -> Unit) {
            val t = value?.trim()?.trimEnd('.')?.takeIf { it.isNotEmpty() } ?: return
            if (t.startsWith("http") || t.startsWith("privkey:")) return
            slot(t)
        }

        fun walk(o: JSONObject, depth: Int) {
            if (depth > 8) return
            if (o.has("LoggedOut")) loggedOut = o.optBoolean("LoggedOut")
            considerName(o.optString("DisplayName")) { if (displayName == null) displayName = it }
            considerName(o.optString("LoginName")) { if (loginName == null) loginName = it }
            considerName(o.optString("Hostname")) { if (hostname == null && it != "localhost") hostname = it }
            considerName(o.optString("MagicDNSName")) { if (magicDns == null) magicDns = it }
            considerName(o.optString("NodeID")) { if (nodeId == null) nodeId = it }
            val addrs = o.optJSONArray("Addresses") ?: o.optJSONArray("TailscaleIPs")
            if (addrs != null && selfIp == null) {
                for (i in 0 until addrs.length()) {
                    val host = addrs.optString(i).substringBefore('/')
                    if (isTailscaleIp(host)) {
                        selfIp = host
                        break
                    }
                }
            }
            val it = o.keys()
            while (it.hasNext()) {
                when (val child = o.opt(it.next())) {
                    is JSONObject -> walk(child, depth + 1)
                    is JSONArray -> {
                        for (i in 0 until child.length()) {
                            val item = child.opt(i)
                            if (item is JSONObject) walk(item, depth + 1)
                        }
                    }
                }
            }
        }
        blobs.forEach { walk(it, 0) }
        val loggedIn = loggedOut == false &&
            (loginName != null || displayName != null || nodeId != null || magicDns != null)
        return TsIdentity(
            loggedIn = loggedIn,
            displayName = displayName ?: loginName,
            hostname = hostname,
            magicDns = magicDns,
            selfIp = selfIp,
        )
    }

    fun discoverSelf(stateDirs: List<File>): TsIdentity {
        var acc = TsIdentity()
        for (dir in stateDirs) {
            val state = File(dir, "tailscaled.state")
            if (state.isFile) {
                val parsed = parseState(state.readText())
                acc = acc.merge(parsed)
            }
            val netmap = readNetmapIdentity(dir)
            if (netmap != null) acc = acc.merge(netmap)
            if (acc.joined && acc.selfIp != null) break
        }
        return acc
    }

    fun stateDirs(filesDir: File, ts: TailscaleSettings): List<File> {
        val out = mutableListOf<File>()
        val configured = ts.stateDirectory.trim()
        if (configured.isNotEmpty()) {
            out.add(File(configured))
            if (!File(configured).isAbsolute) {
                out.add(File(filesDir, configured))
                out.add(File(File(filesDir, "cache"), configured))
            }
        }
        out.add(File(filesDir, "cache/tailscale"))
        out.add(File(filesDir, "runtime/tailscale"))
        out.add(File(filesDir, "tailscale"))
        return out.distinct()
    }

    private fun decodeB64Json(value: String): JSONObject? {
        if (value.isEmpty() || value.startsWith("{") || value.startsWith("[")) return null
        val bytes = runCatching { Base64.getDecoder().decode(value) }.getOrNull() ?: return null
        val text = runCatching { String(bytes, Charsets.UTF_8) }.getOrNull() ?: return null
        val trimmed = text.trim()
        if (!trimmed.startsWith("{")) return null
        return runCatching { JSONObject(trimmed) }.getOrNull()
    }

    private fun readNetmapIdentity(dir: File): TsIdentity? {
        val root = File(dir, "profile-data")
        if (!root.isDirectory) return null
        val profiles = root.listFiles() ?: return null
        for (prof in profiles) {
            val cache = File(prof, "netmap-cache")
            val selfFile = File(cache, "73656c66")
            val files = buildList {
                if (selfFile.isFile) add(selfFile)
                cache.listFiles()?.let { addAll(it) }
            }
            for (f in files) {
                if (!f.isFile) continue
                val ident = parseNetmapSelf(f.readText()) ?: continue
                if (ident.joined) return ident
            }
        }
        return null
    }

    private fun parseNetmapSelf(text: String): TsIdentity? {
        val root = runCatching { JSONObject(text) }.getOrNull() ?: return null
        val node = root.optJSONObject("Node") ?: return null
        val name = node.optString("Name").trim().trimEnd('.').ifEmpty { null }
        var ip: String? = null
        val addrs = node.optJSONArray("Addresses")
        if (addrs != null) {
            for (i in 0 until addrs.length()) {
                val host = addrs.optString(i).substringBefore('/')
                if (isTailscaleIp(host)) {
                    ip = host
                    break
                }
            }
        }
        if (name == null && ip == null) return null
        return TsIdentity(loggedIn = ip != null || name != null, displayName = name, hostname = name, selfIp = ip)
    }

    private fun TsIdentity.merge(other: TsIdentity): TsIdentity = TsIdentity(
        loggedIn = loggedIn || other.loggedIn,
        displayName = displayName ?: other.displayName,
        hostname = hostname ?: other.hostname,
        magicDns = magicDns ?: other.magicDns,
        selfIp = selfIp ?: other.selfIp,
    )
}
