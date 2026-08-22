package app.singplane.model

import org.json.JSONObject

data class Profile(
    val id: String,
    val name: String,
    val sourceType: String = "local",
    val path: String? = null,
    val url: String? = null,
    val content: String = "",
    val updatedAtMs: Long = System.currentTimeMillis(),
    val upload: Long = 0,
    val download: Long = 0,
    val total: Long = 0,
    val expireMs: Long = 0,
    val runnable: Boolean = false,
    val lastError: String? = null,
    val assembleEnabled: Boolean = false,
    val templateId: String? = null,
    val sourceBody: String? = null,
    val contentKind: ContentKind = ContentKind.Unknown,
) {
    val trafficLabel: String
        get() {
            if (total <= 0 && upload <= 0 && download <= 0) return ""
            val used = upload + download
            return if (total > 0) "${fmtBytes(used)} / ${fmtBytes(total)}" else "已用 ${fmtBytes(used)}"
        }

    fun toJson(): JSONObject = JSONObject()
        .put("id", id)
        .put("name", name)
        .put("sourceType", sourceType)
        .put("path", path)
        .put("url", url)
        .put("content", content)
        .put("updatedAt", java.time.Instant.ofEpochMilli(updatedAtMs).toString())
        .put("upload", upload)
        .put("download", download)
        .put("total", total)
        .put("expireMs", expireMs)
        .put("runnable", runnable)
        .put("lastError", lastError)
        .put("assembleEnabled", assembleEnabled)
        .put("templateId", templateId)
        .put("sourceBody", sourceBody)
        .put("contentKind", contentKind.wireName())

    companion object {
        fun fromJson(json: JSONObject): Profile {
            val updated = json.optString("updatedAt", "")
            val updatedMs = runCatching { java.time.Instant.parse(updated).toEpochMilli() }
                .getOrDefault(System.currentTimeMillis())
            return Profile(
                id = json.getString("id"),
                name = json.getString("name"),
                sourceType = json.optString("sourceType", "local"),
                path = json.optString("path").ifEmpty { null },
                url = json.optString("url").ifEmpty { null },
                content = json.optString("content", ""),
                updatedAtMs = updatedMs,
                upload = json.optLong("upload", 0),
                download = json.optLong("download", 0),
                total = json.optLong("total", 0),
                expireMs = json.optLong("expireMs", 0),
                runnable = json.optBoolean("runnable", false),
                lastError = json.optString("lastError").ifEmpty { null },
                assembleEnabled = json.optBoolean("assembleEnabled", false),
                templateId = json.optString("templateId").ifEmpty { null },
                sourceBody = json.optString("sourceBody").ifEmpty { null },
                contentKind = ContentKind.fromWire(json.optString("contentKind")),
            )
        }

        fun prettyContent(raw: String): String {
            val t = raw.trim()
            if (t.isEmpty()) return ""
            runCatching { return prettyJson(JSONObject(t), 0) }
            runCatching { return prettyJson(org.json.JSONArray(t), 0) }
            return t
        }

        private fun prettyJson(value: Any?, indent: Int): String {
            val pad = " ".repeat(indent)
            val inner = " ".repeat(indent + 2)
            return when (value) {
                is JSONObject -> {
                    val keys = value.keys().asSequence().toList()
                    if (keys.isEmpty()) return "{}"
                    val body = keys.joinToString(",\n") { key ->
                        "$inner${JSONObject.quote(key)}: ${prettyJson(value.get(key), indent + 2)}"
                    }
                    "{\n$body\n$pad}"
                }
                is org.json.JSONArray -> {
                    if (value.length() == 0) return "[]"
                    val body = (0 until value.length()).joinToString(",\n") { i ->
                        "$inner${prettyJson(value.get(i), indent + 2)}"
                    }
                    "[\n$body\n$pad]"
                }
                is String -> JSONObject.quote(value)
                org.json.JSONObject.NULL, null -> "null"
                else -> value.toString()
            }
        }

        fun fmtBytes(n: Long): String {
            if (n < 1024) return "$n B"
            val kb = n / 1024.0
            if (kb < 1024) return String.format("%.1f KB", kb)
            val mb = kb / 1024.0
            if (mb < 1024) return String.format("%.1f MB", mb)
            return String.format("%.2f GB", mb / 1024.0)
        }
    }
}
