package app.singplane.core

import org.json.JSONObject

data class CoreReleaseInfo(
    val version: String,
    val assetName: String,
    val downloadUrl: String,
    val size: Long,
)

object GithubReleasePicker {
    fun pick(body: JSONObject, wantAsset: String): CoreReleaseInfo {
        val tag = body.optString("tag_name").removePrefix("v")
        if (tag.isEmpty()) error("Cannot parse release tag")
        val assets = body.optJSONArray("assets") ?: error("Asset not found: $wantAsset")
        for (i in 0 until assets.length()) {
            val a = assets.optJSONObject(i) ?: continue
            if (a.optString("name") == wantAsset) {
                val url = a.optString("browser_download_url")
                if (url.isEmpty()) error("Download URL is empty")
                return CoreReleaseInfo(
                    version = tag,
                    assetName = wantAsset,
                    downloadUrl = url,
                    size = a.optLong("size"),
                )
            }
        }
        val names = (0 until assets.length()).map { assets.optJSONObject(it)?.optString("name") }
        error("Asset not found: $wantAsset\navailable: ${names.joinToString()}")
    }
}
