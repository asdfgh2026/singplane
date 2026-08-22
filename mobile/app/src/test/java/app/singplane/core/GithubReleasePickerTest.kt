package app.singplane.core

import com.google.common.truth.Truth.assertThat
import org.json.JSONObject
import org.junit.Test

class GithubReleasePickerTest {
    @Test
    fun picksMatchingAsset() {
        val body = JSONObject(
            """
            {
              "tag_name": "v1.12.4",
              "assets": [
                {"name":"sing-box-1.12.4-linux-amd64.tar.gz","browser_download_url":"http://x/linux","size":1},
                {"name":"sing-box-1.12.4-android-arm64.tar.gz","browser_download_url":"http://x/droid","size":42}
              ]
            }
            """.trimIndent(),
        )
        val info = GithubReleasePicker.pick(body, wantAsset = "sing-box-1.12.4-android-arm64.tar.gz")
        assertThat(info.version).isEqualTo("1.12.4")
        assertThat(info.downloadUrl).isEqualTo("http://x/droid")
        assertThat(info.size).isEqualTo(42)
        assertThat(info.assetName).isEqualTo("sing-box-1.12.4-android-arm64.tar.gz")
    }

    @Test
    fun missingAssetThrows() {
        val body = JSONObject("""{"tag_name":"v1.0.0","assets":[]}""")
        try {
            GithubReleasePicker.pick(body, wantAsset = "nope.tar.gz")
            throw AssertionError("expected")
        } catch (e: IllegalStateException) {
            assertThat(e.message).contains("nope.tar.gz")
        }
    }
}
