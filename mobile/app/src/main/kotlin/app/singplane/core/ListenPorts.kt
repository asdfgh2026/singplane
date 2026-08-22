package app.singplane.core

import org.json.JSONObject

/** Listen ports from a sing-box runtime config (inbounds + clash_api). */
object ListenPorts {
    fun fromConfig(json: String): Set<Int> {
        val root = runCatching { JSONObject(json) }.getOrNull() ?: return emptySet()
        val out = linkedSetOf<Int>()
        val inbounds = root.optJSONArray("inbounds")
        if (inbounds != null) {
            for (i in 0 until inbounds.length()) {
                val port = inbounds.optJSONObject(i)?.optInt("listen_port", 0) ?: 0
                if (port in 1..65535) out.add(port)
            }
        }
        val controller = root.optJSONObject("experimental")
            ?.optJSONObject("clash_api")
            ?.optString("external_controller")
            .orEmpty()
        parseControllerPort(controller)?.let { out.add(it) }
        return out
    }

    fun parseControllerPort(controller: String): Int? {
        val s = controller.trim()
        if (s.isEmpty()) return null
        val portPart = s.substringAfterLast(':').trim().trimStart(']')
        return portPart.toIntOrNull()?.takeIf { it in 1..65535 }
    }

    fun isAddressInUse(message: String?): Boolean {
        val lower = message.orEmpty().lowercase()
        return lower.contains("address already in use") ||
            lower.contains("only one usage of each socket address") ||
            lower.contains("bind: address already in use") ||
            lower.contains("端口已占用") ||
            (lower.contains("bind") && lower.contains("in use"))
    }
}
