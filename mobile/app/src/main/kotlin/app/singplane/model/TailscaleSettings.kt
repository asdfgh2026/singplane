package app.singplane.model

import app.singplane.assemble.CoreLine
import org.json.JSONArray
import org.json.JSONObject

data class TailscaleSettings(
    var enabled: Boolean = false,
    var tag: String = "ts-local",
    var authKey: String = "",
    var controlUrl: String = "",
    var hostname: String = "",
    var stateDirectory: String = "",
    var acceptRoutes: Boolean = true,
    var advertiseExitNode: Boolean = false,
    var exitNodeAllowLanAccess: Boolean = false,
    var exitNode: String = "",
    var advertiseRoutes: String = "",
    var advertiseTags: String = "",
    var systemInterface: Boolean = false,
    var sshServer: Boolean = false,
    var replaceOtherTailscale: Boolean = true,
    var injectDns: Boolean = true,
    var acceptDefaultResolvers: Boolean = false,
    var acceptSearchDomain: Boolean = true,
    var injectRoutePreferredBy: Boolean = true,
    var routeDomainSuffix: String = ".ts.net",
    var routeIpCidr: String = ""
) {
    fun resolvedTag(): String {
        return if (tag.trim().isEmpty()) "ts-local" else tag.trim()
    }

    fun resolvedDnsTag(): String {
        return "${resolvedTag()}-dns"
    }

    fun usesDeviceAuth(): Boolean {
        return authKey.trim().isEmpty()
    }

    fun toJson(): JSONObject {
        val json = JSONObject()
        json.put("enabled", enabled)
        json.put("tag", tag)
        json.put("authKey", authKey)
        json.put("controlUrl", controlUrl)
        json.put("hostname", hostname)
        json.put("stateDirectory", stateDirectory)
        json.put("acceptRoutes", acceptRoutes)
        json.put("advertiseExitNode", advertiseExitNode)
        json.put("exitNodeAllowLanAccess", exitNodeAllowLanAccess)
        json.put("exitNode", exitNode)
        json.put("advertiseRoutes", advertiseRoutes)
        json.put("advertiseTags", advertiseTags)
        json.put("systemInterface", systemInterface)
        json.put("sshServer", sshServer)
        json.put("replaceOtherTailscale", replaceOtherTailscale)
        json.put("injectDns", injectDns)
        json.put("acceptDefaultResolvers", acceptDefaultResolvers)
        json.put("acceptSearchDomain", acceptSearchDomain)
        json.put("injectRoutePreferredBy", injectRoutePreferredBy)
        json.put("routeDomainSuffix", routeDomainSuffix)
        json.put("routeIpCidr", routeIpCidr)
        return json
    }

    fun toEndpointJson(line: CoreLine): JSONObject {
        val ep = JSONObject()
        ep.put("type", "tailscale")
        ep.put("tag", resolvedTag())
        if (authKey.trim().isNotEmpty()) {
            ep.put("auth_key", authKey.trim())
        }
        if (controlUrl.trim().isNotEmpty()) {
            ep.put("control_url", controlUrl.trim())
        }
        if (hostname.trim().isNotEmpty()) {
            ep.put("hostname", hostname.trim())
        }
        val sd = if (stateDirectory.trim().isNotEmpty()) stateDirectory.trim() else "tailscale"
        ep.put("state_directory", sd)
        if (acceptRoutes) {
            ep.put("accept_routes", true)
        }
        if (advertiseExitNode) {
            ep.put("advertise_exit_node", true)
        }
        if (exitNodeAllowLanAccess) {
            ep.put("exit_node_allow_lan_access", true)
        }
        if (exitNode.trim().isNotEmpty()) {
            ep.put("exit_node", exitNode.trim())
        }
        val routes = splitList(advertiseRoutes)
        if (routes.isNotEmpty()) {
            val arr = JSONArray()
            routes.forEach { arr.put(it) }
            ep.put("advertise_routes", arr)
        }
        if (line.atLeast(1, 13)) {
            val tags = splitList(advertiseTags)
            if (tags.isNotEmpty()) {
                val arr = JSONArray()
                tags.forEach { arr.put(it) }
                ep.put("advertise_tags", arr)
            }
            if (systemInterface) {
                ep.put("system_interface", true)
            }
        }
        if (line.atLeast(1, 14) && sshServer) {
            ep.put("ssh_server", true)
        }
        return ep
    }

    companion object {
        fun splitList(raw: String): List<String> {
            return raw.split(Regex("[\\s,;]+")).map { it.trim() }.filter { it.isNotEmpty() }
        }

        fun fromJson(json: JSONObject): TailscaleSettings {
            return TailscaleSettings(
                enabled = json.optBoolean("enabled", false),
                tag = json.optString("tag", "ts-local"),
                authKey = json.optString("authKey", ""),
                controlUrl = json.optString("controlUrl", ""),
                hostname = json.optString("hostname", ""),
                stateDirectory = json.optString("stateDirectory", ""),
                acceptRoutes = json.optBoolean("acceptRoutes", true),
                advertiseExitNode = json.optBoolean("advertiseExitNode", false),
                exitNodeAllowLanAccess = json.optBoolean("exitNodeAllowLanAccess", false),
                exitNode = json.optString("exitNode", ""),
                advertiseRoutes = json.optString("advertiseRoutes", ""),
                advertiseTags = json.optString("advertiseTags", ""),
                systemInterface = json.optBoolean("systemInterface", false),
                sshServer = json.optBoolean("sshServer", false),
                replaceOtherTailscale = json.optBoolean("replaceOtherTailscale", true),
                injectDns = json.optBoolean("injectDns", true),
                acceptDefaultResolvers = json.optBoolean("acceptDefaultResolvers", false),
                acceptSearchDomain = json.optBoolean("acceptSearchDomain", true),
                injectRoutePreferredBy = json.optBoolean("injectRoutePreferredBy", true),
                routeDomainSuffix = json.optString("routeDomainSuffix", ".ts.net"),
                routeIpCidr = json.optString("routeIpCidr", "")
            )
        }
    }
}
