package app.singplane.core

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.TimeUnit

class CoreDownloader(
    private val client: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .build(),
) {
    companion object {
        const val LATEST_URL = "https://api.github.com/repos/SagerNet/sing-box/releases/latest"
        const val RELEASES_URL = "https://api.github.com/repos/SagerNet/sing-box/releases?per_page=30"
        const val USER_AGENT = "SingPanel"

        fun findReleaseInReleases(releases: JSONArray, channel: String, arch: String): CoreReleaseInfo {
            val wantBeta = channel.equals("beta", ignoreCase = true)
            for (i in 0 until releases.length()) {
                val item = releases.optJSONObject(i) ?: continue
                val tag = item.optString("tag_name")
                val isPre = item.optBoolean("prerelease", false)
                val isBeta = isPre || tag.contains("beta", ignoreCase = true) ||
                    tag.contains("rc", ignoreCase = true) ||
                    tag.contains("alpha", ignoreCase = true)

                if (wantBeta) {
                    if (isBeta) {
                        val ver = tag.removePrefix("v")
                        val wantAsset = CorePlatform.assetFileName(ver, "android", arch)
                        return GithubReleasePicker.pick(item, wantAsset)
                    }
                } else {
                    if (!isBeta) {
                        val ver = tag.removePrefix("v")
                        val wantAsset = CorePlatform.assetFileName(ver, "android", arch)
                        return GithubReleasePicker.pick(item, wantAsset)
                    }
                }
            }
            error("未找到匹配 ${if (wantBeta) "Beta" else "Stable"} 通道发布版本 (arch=$arch)")
        }

        fun installArchive(archive: File, coresDir: File, binaryName: String = "sing-box"): File {
            coresDir.mkdirs()
            val target = File(coresDir, binaryName)
            val bak = File(coresDir, "$binaryName.bak")

            if (target.exists()) {
                if (bak.exists()) bak.delete()
                if (!target.renameTo(bak)) {
                    target.copyTo(bak, overwrite = true)
                    target.delete()
                }
            }

            try {
                val extracted = ArchiveExtractor.extractCore(archive, coresDir, binaryName)
                if (bak.exists()) bak.delete()
                extracted.setExecutable(true, false)
                return extracted
            } catch (e: Exception) {
                if (bak.exists() && !target.exists()) {
                    bak.renameTo(target)
                }
                throw e
            }
        }
    }

    suspend fun downloadAndInstall(
        channel: String,
        githubProxy: String,
        coresDir: File,
        arch: String,
        onProgress: (String) -> Unit = {},
    ): File = withContext(Dispatchers.IO) {
        onProgress("正在获取发布信息...")
        val isBeta = channel.equals("beta", ignoreCase = true)
        val apiUrl = if (isBeta) RELEASES_URL else LATEST_URL
        val effectiveApiUrl = GithubProxy.applyProxy(apiUrl, githubProxy)

        val apiReq = Request.Builder()
            .url(effectiveApiUrl)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/vnd.github+json")
            .build()

        val apiResp = client.newCall(apiReq).execute()
        if (!apiResp.isSuccessful) {
            error("获取 GitHub 发布失败 (HTTP ${apiResp.code})")
        }
        val rawBody = apiResp.body?.string() ?: error("GitHub 响应内容为空")

        val releaseInfo = if (isBeta) {
            val arr = JSONArray(rawBody)
            findReleaseInReleases(arr, channel, arch)
        } else {
            val obj = JSONObject(rawBody)
            val ver = obj.optString("tag_name").removePrefix("v")
            val wantAsset = CorePlatform.assetFileName(ver, "android", arch)
            GithubReleasePicker.pick(obj, wantAsset)
        }

        onProgress("正在下载 ${releaseInfo.assetName}...")
        val rawDownloadUrl = releaseInfo.downloadUrl
        val effectiveDownloadUrl = GithubProxy.applyProxy(rawDownloadUrl, githubProxy)

        val dlReq = Request.Builder()
            .url(effectiveDownloadUrl)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/octet-stream")
            .build()

        val dlResp = client.newCall(dlReq).execute()
        if (!dlResp.isSuccessful) {
            error("下载内核失败 (HTTP ${dlResp.code})")
        }

        val cacheDir = File(coresDir, ".cache").apply { mkdirs() }
        val archiveFile = File(cacheDir, releaseInfo.assetName)
        val tmpFile = File(cacheDir, "${releaseInfo.assetName}.part")

        try {
            val body = dlResp.body ?: error("下载内容为空")
            body.byteStream().use { input ->
                FileOutputStream(tmpFile).use { output ->
                    input.copyTo(output)
                }
            }
            if (archiveFile.exists()) archiveFile.delete()
            if (!tmpFile.renameTo(archiveFile)) {
                tmpFile.copyTo(archiveFile, overwrite = true)
                tmpFile.delete()
            }

            onProgress("正在解压并安装内核...")
            val installed = installArchive(archiveFile, coresDir, "sing-box")
            archiveFile.delete()
            onProgress("内核安装成功: ${releaseInfo.version}")
            installed
        } finally {
            if (tmpFile.exists()) tmpFile.delete()
        }
    }
}
