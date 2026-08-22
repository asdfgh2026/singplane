package app.singplane.assemble

import com.google.common.truth.Truth.assertThat
import app.singplane.model.TailscaleSettings
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Test

class TailscaleOverlayTest {

    private fun baseCfg(): JSONObject {
        return JSONObject("""
        {
            "inbounds": [{"type":"http","tag":"http-in","listen":"127.0.0.1","listen_port":7890}],
            "outbounds": [{"type":"direct","tag":"direct"}],
            "endpoints": [
                {"type":"tailscale","tag":"from-sub"},
                {"type":"wireguard","tag":"wg-keep"}
            ],
            "dns": {
                "servers": [{"type":"local","tag":"local"}],
                "rules": []
            },
            "route": {"rules": [{"clash_mode":"Direct","outbound":"direct"}], "final":"direct"}
        }
        """.trimIndent())
    }

    private fun enabledTs(): TailscaleSettings {
        return TailscaleSettings(
            enabled = true,
            tag = "ts-local",
            authKey = "tskey-auth-test",
            hostname = "orb",
            acceptRoutes = true,
            injectDns = true,
            injectRoutePreferredBy = true,
            replaceOtherTailscale = true,
            routeDomainSuffix = ".ts.net",
            routeIpCidr = "100.64.0.0/10",
            advertiseRoutes = "192.168.1.0/24"
        )
    }

    @Test
    fun disabledDoesNotInject() {
        val ts = TailscaleSettings(enabled = false)
        val cfg = TailscaleOverlay.apply(baseCfg(), ts)
        val eps = cfg.getJSONArray("endpoints")
        assertThat(eps.length()).isEqualTo(2)
        for (i in 0 until eps.length()) {
            assertThat(eps.getJSONObject(i).getString("tag")).isNotEqualTo("ts-local")
        }
        val dnsServers = cfg.getJSONObject("dns").getJSONArray("servers")
        for (i in 0 until dnsServers.length()) {
            assertThat(dnsServers.getJSONObject(i).getString("type")).isNotEqualTo("tailscale")
        }
    }

    @Test
    fun enabledInjectsEndpointDnsAndRoute() {
        val cfg = TailscaleOverlay.apply(baseCfg(), enabledTs(), CoreLine.V14)
        val eps = cfg.getJSONArray("endpoints")
        var hasTsLocal = false
        var hasWgKeep = false
        var hasFromSub = false
        var tsLocalEp: JSONObject? = null
        for (i in 0 until eps.length()) {
            val e = eps.getJSONObject(i)
            val tag = e.optString("tag")
            if (tag == "ts-local" && e.optString("type") == "tailscale") {
                hasTsLocal = true
                tsLocalEp = e
            }
            if (tag == "wg-keep") hasWgKeep = true
            if (tag == "from-sub") hasFromSub = true
        }
        assertThat(hasTsLocal).isTrue()
        assertThat(hasWgKeep).isTrue()
        assertThat(hasFromSub).isFalse()

        assertThat(tsLocalEp!!.getString("auth_key")).isEqualTo("tskey-auth-test")
        assertThat(tsLocalEp.getString("hostname")).isEqualTo("orb")
        assertThat(tsLocalEp.getJSONArray("advertise_routes").getString(0)).isEqualTo("192.168.1.0/24")

        val servers = cfg.getJSONObject("dns").getJSONArray("servers")
        var tsDns: JSONObject? = null
        for (i in 0 until servers.length()) {
            val s = servers.getJSONObject(i)
            if (s.optString("tag") == "ts-local-dns") {
                tsDns = s
            }
        }
        assertThat(tsDns!!.getString("type")).isEqualTo("tailscale")
        assertThat(tsDns.getBoolean("accept_search_domain")).isTrue()

        val rules = cfg.getJSONObject("dns").getJSONArray("rules")
        var hasPreferredByRule = false
        for (i in 0 until rules.length()) {
            val r = rules.getJSONObject(i)
            if (r.optString("server") == "ts-local-dns" && r.has("preferred_by")) {
                hasPreferredByRule = true
            }
        }
        assertThat(hasPreferredByRule).isTrue()

        val routeRules = cfg.getJSONObject("route").getJSONArray("rules")
        var hasRoutePreferredBy = false
        var hasRouteDomainSuffix = false
        var hasRouteIpCidr = false
        var hasClashMode = false
        for (i in 0 until routeRules.length()) {
            val r = routeRules.getJSONObject(i)
            val ob = r.optString("outbound")
            if (ob == "ts-local" && r.has("preferred_by")) hasRoutePreferredBy = true
            if (ob == "ts-local" && r.has("domain_suffix")) hasRouteDomainSuffix = true
            if (ob == "ts-local" && r.has("ip_cidr")) hasRouteIpCidr = true
            if (r.optString("clash_mode") == "Direct") hasClashMode = true
        }
        assertThat(hasRoutePreferredBy).isTrue()
        assertThat(hasRouteDomainSuffix).isTrue()
        assertThat(hasRouteIpCidr).isTrue()
        assertThat(hasClashMode).isTrue()
    }

    @Test
    fun v13UsesDomainSuffixNotPreferredBy() {
        val cfg = TailscaleOverlay.apply(baseCfg(), enabledTs(), CoreLine.V13)
        val servers = cfg.getJSONObject("dns").getJSONArray("servers")
        var tsDns: JSONObject? = null
        for (i in 0 until servers.length()) {
            val s = servers.getJSONObject(i)
            if (s.optString("tag") == "ts-local-dns") {
                tsDns = s
            }
        }
        assertThat(tsDns!!.has("accept_search_domain")).isFalse()

        val rules = cfg.getJSONObject("dns").getJSONArray("rules")
        var hasDomainSuffixRule = false
        var anyHasPreferredBy = false
        for (i in 0 until rules.length()) {
            val r = rules.getJSONObject(i)
            if (r.optString("server") == "ts-local-dns" && r.has("domain_suffix")) {
                hasDomainSuffixRule = true
            }
            if (r.has("preferred_by")) {
                anyHasPreferredBy = true
            }
        }
        assertThat(hasDomainSuffixRule).isTrue()
        assertThat(anyHasPreferredBy).isFalse()

        val routeRules = cfg.getJSONObject("route").getJSONArray("rules")
        var anyRouteHasPreferredBy = false
        var hasRouteDomainSuffix = false
        for (i in 0 until routeRules.length()) {
            val r = routeRules.getJSONObject(i)
            if (r.has("preferred_by")) anyRouteHasPreferredBy = true
            if (r.optString("outbound") == "ts-local" && r.has("domain_suffix")) hasRouteDomainSuffix = true
        }
        assertThat(anyRouteHasPreferredBy).isFalse()
        assertThat(hasRouteDomainSuffix).isTrue()
    }

    @Test
    fun replaceOtherFalseKeepsSubscriptionEndpoint() {
        val ts = enabledTs()
        ts.replaceOtherTailscale = false
        val cfg = TailscaleOverlay.apply(baseCfg(), ts)
        val eps = cfg.getJSONArray("endpoints")
        var hasFromSub = false
        var hasTsLocal = false
        for (i in 0 until eps.length()) {
            val tag = eps.getJSONObject(i).optString("tag")
            if (tag == "from-sub") hasFromSub = true
            if (tag == "ts-local") hasTsLocal = true
        }
        assertThat(hasFromSub).isTrue()
        assertThat(hasTsLocal).isTrue()
    }

    @Test
    fun replaceOtherTrueRemovesExistingTailscaleEndpoint() {
        val ts = enabledTs()
        ts.replaceOtherTailscale = true
        val cfg = TailscaleOverlay.apply(baseCfg(), ts)
        val eps = cfg.getJSONArray("endpoints")
        var hasFromSub = false
        var hasTsLocal = false
        for (i in 0 until eps.length()) {
            val tag = eps.getJSONObject(i).optString("tag")
            if (tag == "from-sub") hasFromSub = true
            if (tag == "ts-local") hasTsLocal = true
        }
        assertThat(hasFromSub).isFalse()
        assertThat(hasTsLocal).isTrue()
    }
}

