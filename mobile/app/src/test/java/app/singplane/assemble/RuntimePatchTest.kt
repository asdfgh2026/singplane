package app.singplane.assemble

import com.google.common.truth.Truth.assertThat
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Test

class RuntimePatchTest {
    private fun cfg(mixedPort: Int = 1234, withTun: Boolean = false, api: String = "127.0.0.1:1111"): JSONObject {
        val inbounds = JSONArray().put(
            JSONObject()
                .put("type", "mixed")
                .put("tag", "mixed-in")
                .put("listen", "0.0.0.0")
                .put("listen_port", mixedPort),
        )
        if (withTun) {
            inbounds.put(JSONObject().put("type", "tun").put("tag", "tun-in"))
        }
        return JSONObject()
            .put("inbounds", inbounds)
            .put(
                "experimental",
                JSONObject().put(
                    "clash_api",
                    JSONObject().put("external_controller", api).put("secret", ""),
                ),
            )
            .put("outbounds", JSONArray().put(JSONObject().put("type", "direct").put("tag", "direct")))
    }

    @Test
    fun forcesMixedPortAndLocalhost() {
        val out = RuntimePatch.apply(
            cfg(1234),
            RuntimePatch.Options(forceMixedPort = 7890, forceListenLocalhost = true),
        )
        val mixed = out.getJSONArray("inbounds").getJSONObject(0)
        assertThat(mixed.getInt("listen_port")).isEqualTo(7890)
        assertThat(mixed.getString("listen")).isEqualTo("127.0.0.1")
    }

    @Test
    fun forcesClashApi() {
        val out = RuntimePatch.apply(
            cfg(api = "0.0.0.0:1111"),
            RuntimePatch.Options(forceClashApi = "127.0.0.1:9090"),
        )
        val api = out.getJSONObject("experimental").getJSONObject("clash_api")
        assertThat(api.getString("external_controller")).isEqualTo("127.0.0.1:9090")
    }

    @Test
    fun stripsTun() {
        val out = RuntimePatch.apply(cfg(withTun = true), RuntimePatch.Options(stripTun = true))
        val types = (0 until out.getJSONArray("inbounds").length()).map {
            out.getJSONArray("inbounds").getJSONObject(it).getString("type")
        }
        assertThat(types).doesNotContain("tun")
        assertThat(types).contains("mixed")
    }

    @Test
    fun noOpKeepsPorts() {
        val out = RuntimePatch.apply(cfg(mixedPort = 5555, withTun = true), RuntimePatch.Options())
        val mixed = out.getJSONArray("inbounds").getJSONObject(0)
        assertThat(mixed.getInt("listen_port")).isEqualTo(5555)
        val types = (0 until out.getJSONArray("inbounds").length()).map {
            out.getJSONArray("inbounds").getJSONObject(it).getString("type")
        }
        assertThat(types).contains("tun")
    }
}
