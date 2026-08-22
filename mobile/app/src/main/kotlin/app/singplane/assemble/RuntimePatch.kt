package app.singplane.assemble

import org.json.JSONArray
import org.json.JSONObject

/** Applied at start, does not mutate the profile. */
object RuntimePatch {
    data class Options(
        val forceMixedPort: Int? = null,
        val forceClashApi: String? = null,
        val forceListenLocalhost: Boolean = false,
        val stripTun: Boolean = false,
    ) {
        val isNoOp: Boolean
            get() = forceMixedPort == null &&
                forceClashApi.isNullOrEmpty() &&
                !forceListenLocalhost &&
                !stripTun
    }

    fun apply(config: JSONObject, options: Options): JSONObject {
        val cfg = JSONObject(config.toString())
        if (options.isNoOp) return cfg

        if (cfg.has("inbounds")) {
            val raw = cfg.getJSONArray("inbounds")
            val next = JSONArray()
            for (i in 0 until raw.length()) {
                val item = raw.optJSONObject(i) ?: continue
                val m = JSONObject(item.toString())
                val type = m.optString("type")
                if (options.stripTun && type == "tun") continue
                if (type == "mixed" || type == "http" || type == "socks") {
                    if (options.forceMixedPort != null) {
                        m.put("listen_port", options.forceMixedPort)
                    }
                    if (options.forceListenLocalhost) {
                        m.put("listen", "127.0.0.1")
                    }
                }
                next.put(m)
            }
            cfg.put("inbounds", next)
        }

        val api = options.forceClashApi
        if (!api.isNullOrEmpty()) {
            val experimental = cfg.optJSONObject("experimental")?.let { JSONObject(it.toString()) }
                ?: JSONObject()
            val clash = experimental.optJSONObject("clash_api")?.let { JSONObject(it.toString()) }
                ?: JSONObject()
            clash.put("external_controller", api)
            experimental.put("clash_api", clash)
            cfg.put("experimental", experimental)
        }
        return cfg
    }
}
