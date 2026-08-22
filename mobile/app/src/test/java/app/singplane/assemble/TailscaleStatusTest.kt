package app.singplane.assemble

import com.google.common.truth.Truth.assertThat
import app.singplane.model.TailscaleSettings
import org.junit.Test

class TailscaleStatusTest {

    @Test
    fun parseLoginUrlFromLogLine() {
        val waiting = "INFO endpoint/tailscale[ts-local]: waiting for authentication: https://login.tailscale.com/a/abc123"
        val hint = TailscaleStatus.latestHint(waiting)
        assertThat(hint.kind).isEqualTo(TailscaleStatus.HintKind.WaitingAuth)
        assertThat(hint.loginUrl).isEqualTo("https://login.tailscale.com/a/abc123")
    }

    @Test
    fun connectedHintFromMagicdns() {
        val waiting = "INFO endpoint/tailscale[ts-local]: waiting for authentication: https://login.tailscale.com/a/abc123"
        val log = "$waiting\nINFO dns/tailscale[ts-local-dns]: updated 67 routes, 23 hosts"
        val hint = TailscaleStatus.latestHint(log)
        assertThat(hint.kind).isEqualTo(TailscaleStatus.HintKind.Connected)
    }

    @Test
    fun emptyMagicdnsIsNotJoined() {
        val log = "INFO dns/tailscale[ts-local-dns]: updated 0 routes, 0 hosts, 0 search domains"
        val hint = TailscaleStatus.latestHint(log)
        assertThat(hint.kind).isEqualTo(TailscaleStatus.HintKind.None)

        val ts = TailscaleSettings(enabled = true, authKey = "")
        val st = TailscaleStatus.pendingStatus(ts, null, null)
        assertThat(st.phase).isEqualTo(TsPhase.Pending)
        assertThat(st.title).isEqualTo("验证中")

        val withKey = TailscaleSettings(enabled = true, authKey = "tskey-auth-x")
        val st2 = TailscaleStatus.pendingStatus(withKey, null, null)
        assertThat(st2.phase).isEqualTo(TsPhase.Pending)
        assertThat(st2.subtitle).contains("Auth Key")
    }

    @Test
    fun tailscaleIpRange() {
        assertThat(TailscaleStatus.isTailscaleIp("100.64.0.1")).isTrue()
        assertThat(TailscaleStatus.isTailscaleIp("100.127.1.2")).isTrue()
        assertThat(TailscaleStatus.isTailscaleIp("100.63.0.1")).isFalse()
        assertThat(TailscaleStatus.isTailscaleIp("10.0.0.1")).isFalse()
    }

    @Test
    fun disabledStatus() {
        val ts = TailscaleSettings(enabled = false)
        val st = TailscaleStatus.statusFromLog(ts, false, "")
        assertThat(st.phase).isEqualTo(TsPhase.Disabled)
    }

    @Test
    fun connectedHintIsInjectedNotNeedsLogin() {
        val waiting = "INFO endpoint/tailscale[ts-local]: waiting for authentication: https://login.tailscale.com/a/abc123"
        val log = "$waiting\nINFO dns/tailscale[ts-local-dns]: updated 67 routes, 23 hosts"
        val ts = TailscaleSettings(enabled = true)
        val st = TailscaleStatus.statusFromLog(ts, true, log)
        assertThat(st.phase).isEqualTo(TsPhase.Injected)
        assertThat(st.title).isEqualTo("已加入")
        assertThat(st.loginUrl).isNull()
    }

    @Test
    fun stateLoggedInWinsOverStaleLoginUrl() {
        val profiles = """{"8465":{"Name":"openmindw@github","NetworkProfile":{"MagicDNSName":"tailafc5c3.ts.net"},"UserProfile":{"LoginName":"openmindw@github","DisplayName":"openmindw"},"NodeID":"n5GJ8oEanC21CNTRL"}}"""
        val prefs = """{"LoggedOut":false,"WantRunning":true,"Hostname":"localhost","Config":{"UserProfile":{"DisplayName":"openmindw","LoginName":"openmindw@github"},"NodeID":"n5GJ8oEanC21CNTRL"}}"""
        val state = org.json.JSONObject()
            .put("_profiles", java.util.Base64.getEncoder().encodeToString(profiles.toByteArray()))
            .put("profile-8465", java.util.Base64.getEncoder().encodeToString(prefs.toByteArray()))
            .toString()
        val ident = TailscaleStatus.parseState(state)
        assertThat(ident.loggedIn).isTrue()
        assertThat(ident.displayName).isEqualTo("openmindw")

        val ts = TailscaleSettings(enabled = true)
        val stale = "INFO endpoint/tailscale[ts-local]: waiting for authentication: https://login.tailscale.com/a/old"
        val st = TailscaleStatus.statusFromLog(ts, true, stale, ident)
        assertThat(st.phase).isEqualTo(TsPhase.Injected)
        assertThat(st.title).isEqualTo("已加入")
        assertThat(st.subtitle).contains("openmindw")
        assertThat(st.loginUrl).isNull()
    }

    @Test
    fun loggedOutStateIsNotJoined() {
        val prefs = """{"LoggedOut":true,"WantRunning":false}"""
        val state = org.json.JSONObject()
            .put("profile-1", java.util.Base64.getEncoder().encodeToString(prefs.toByteArray()))
            .toString()
        val ident = TailscaleStatus.parseState(state)
        assertThat(ident.loggedIn).isFalse()
        assertThat(ident.joined).isFalse()
    }
}
