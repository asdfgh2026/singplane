package app.singplane.assemble

import app.singplane.model.TailscaleSettings
import org.json.JSONArray
import org.json.JSONObject

object TailscaleOverlay {

    fun apply(userConfig: JSONObject, ts: TailscaleSettings, line: CoreLine = CoreLine.V13): JSONObject {
        val cfg = JSONObject(userConfig.toString()) // deep copy
        if (!ts.enabled) {
            return cfg
        }
        injectEndpoint(cfg, ts, line)
        if (ts.injectDns) {
            injectDns(cfg, ts, line)
        }
        if (ts.injectRoutePreferredBy || TailscaleSettings.splitList(ts.routeDomainSuffix).isNotEmpty() || TailscaleSettings.splitList(ts.routeIpCidr).isNotEmpty()) {
            injectRoute(cfg, ts, line)
        }
        return cfg
    }

    private fun injectEndpoint(cfg: JSONObject, ts: TailscaleSettings, line: CoreLine) {
        val tag = ts.resolvedTag()
        val list = JSONArray()
        if (cfg.has("endpoints")) {
            val raw = cfg.optJSONArray("endpoints")
            if (raw != null) {
                for (i in 0 until raw.length()) {
                    val m = raw.optJSONObject(i) ?: continue
                    val typ = m.optString("type", "")
                    val tg = m.optString("tag", "")
                    if (tg == tag) continue
                    if (ts.replaceOtherTailscale && typ == "tailscale") continue
                    list.put(m)
                }
            }
        }
        list.put(ts.toEndpointJson(line))
        cfg.put("endpoints", list)
    }

    private fun injectDns(cfg: JSONObject, ts: TailscaleSettings, line: CoreLine) {
        val dnsTag = ts.resolvedDnsTag()
        val dns = cfg.optJSONObject("dns") ?: JSONObject()
        val servers = JSONArray()
        if (dns.has("servers")) {
            val raw = dns.optJSONArray("servers")
            if (raw != null) {
                for (i in 0 until raw.length()) {
                    val m = raw.optJSONObject(i) ?: continue
                    val typ = m.optString("type", "")
                    val tg = m.optString("tag", "")
                    val ep = m.optString("endpoint", "")
                    if (tg == dnsTag) continue
                    if (ts.replaceOtherTailscale && typ == "tailscale") continue
                    if (typ == "tailscale" && ep == ts.resolvedTag()) continue
                    servers.put(m)
                }
            }
        }
        val server = JSONObject()
        server.put("type", "tailscale")
        server.put("tag", dnsTag)
        server.put("endpoint", ts.resolvedTag())
        if (ts.acceptDefaultResolvers) {
            server.put("accept_default_resolvers", true)
        }
        if (line.atLeast(1, 14) && ts.acceptSearchDomain) {
            server.put("accept_search_domain", true)
        }
        servers.put(server)
        dns.put("servers", servers)

        var suffixes = TailscaleSettings.splitList(ts.routeDomainSuffix)
        if (suffixes.isEmpty()) {
            suffixes = listOf(".ts.net")
        }

        val rules = JSONArray()
        if (line.atLeast(1, 14)) {
            val r1 = JSONObject()
            r1.put("preferred_by", JSONArray().put(dnsTag))
            r1.put("action", "route")
            r1.put("server", dnsTag)
            rules.put(r1)

            val r2 = JSONObject()
            val sArr = JSONArray()
            suffixes.forEach { sArr.put(it) }
            r2.put("domain_suffix", sArr)
            r2.put("action", "route")
            r2.put("server", dnsTag)
            rules.put(r2)
        } else {
            val r = JSONObject()
            val sArr = JSONArray()
            suffixes.forEach { sArr.put(it) }
            r.put("domain_suffix", sArr)
            r.put("server", dnsTag)
            rules.put(r)
        }

        if (dns.has("rules")) {
            val raw = dns.optJSONArray("rules")
            if (raw != null) {
                for (i in 0 until raw.length()) {
                    val m = raw.optJSONObject(i) ?: continue
                    if (preferredContains(m.optJSONArray("preferred_by"), dnsTag) ||
                        m.optString("preferred_by") == dnsTag) continue
                    if (m.optString("server") == dnsTag) continue
                    rules.put(m)
                }
            }
        }
        dns.put("rules", rules)
        cfg.put("dns", dns)
    }

    private fun injectRoute(cfg: JSONObject, ts: TailscaleSettings, line: CoreLine) {
        val tag = ts.resolvedTag()
        val route = cfg.optJSONObject("route") ?: JSONObject()
        val rules = JSONArray()

        if (ts.injectRoutePreferredBy && line.atLeast(1, 14)) {
            val r = JSONObject()
            r.put("preferred_by", JSONArray().put(tag))
            r.put("outbound", tag)
            rules.put(r)
        }

        val suffixes = TailscaleSettings.splitList(ts.routeDomainSuffix)
        if (suffixes.isNotEmpty()) {
            val r = JSONObject()
            val sArr = JSONArray()
            suffixes.forEach { sArr.put(it) }
            r.put("domain_suffix", sArr)
            r.put("outbound", tag)
            rules.put(r)
        }

        val cidrs = TailscaleSettings.splitList(ts.routeIpCidr)
        if (cidrs.isNotEmpty()) {
            val r = JSONObject()
            val cArr = JSONArray()
            cidrs.forEach { cArr.put(it) }
            r.put("ip_cidr", cArr)
            r.put("outbound", tag)
            rules.put(r)
        }

        if (route.has("rules")) {
            val raw = route.optJSONArray("rules")
            if (raw != null) {
                for (i in 0 until raw.length()) {
                    val m = raw.optJSONObject(i) ?: continue
                    val hasPref = preferredContains(m.optJSONArray("preferred_by"), tag) || m.optString("preferred_by") == tag
                    if (hasPref && m.optString("outbound") == tag) continue
                    rules.put(m)
                }
            }
        }
        route.put("rules", rules)
        cfg.put("route", route)
    }

    private fun preferredContains(arr: JSONArray?, tag: String): Boolean {
        if (arr == null) return false
        for (i in 0 until arr.length()) {
            if (arr.optString(i) == tag) return true
        }
        return false
    }
}
