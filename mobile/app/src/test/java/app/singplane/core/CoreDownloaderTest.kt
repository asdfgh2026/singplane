package app.singplane.core

import com.google.common.truth.Truth.assertThat
import org.apache.commons.compress.archivers.tar.TarArchiveEntry
import org.apache.commons.compress.archivers.tar.TarArchiveOutputStream
import org.apache.commons.compress.compressors.gzip.GzipCompressorOutputStream
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class CoreDownloaderTest {
    @get:Rule
    val tmp = TemporaryFolder()

    @Test
    fun pickReleaseInfoBeta() {
        val releases = JSONArray("""
            [
              {
                "tag_name": "v1.13.0-rc.1",
                "prerelease": true,
                "assets": [
                  {
                    "name": "sing-box-1.13.0-rc.1-android-arm64.tar.gz",
                    "browser_download_url": "https://github.com/SagerNet/sing-box/releases/download/v1.13.0-rc.1/sing-box-1.13.0-rc.1-android-arm64.tar.gz",
                    "size": 12345
                  }
                ]
              },
              {
                "tag_name": "v1.12.0",
                "prerelease": false,
                "assets": [
                  {
                    "name": "sing-box-1.12.0-android-arm64.tar.gz",
                    "browser_download_url": "https://github.com/SagerNet/sing-box/releases/download/v1.12.0/sing-box-1.12.0-android-arm64.tar.gz",
                    "size": 10000
                  }
                ]
              }
            ]
        """.trimIndent())

        val picker = CoreDownloader.findReleaseInReleases(releases, channel = "beta", arch = "arm64")
        assertThat(picker.version).isEqualTo("1.13.0-rc.1")
        assertThat(picker.assetName).isEqualTo("sing-box-1.13.0-rc.1-android-arm64.tar.gz")
    }

    @Test
    fun pickReleaseInfoStable() {
        val latest = JSONObject("""
            {
              "tag_name": "v1.12.0",
              "prerelease": false,
              "assets": [
                {
                  "name": "sing-box-1.12.0-android-amd64.tar.gz",
                  "browser_download_url": "https://github.com/SagerNet/sing-box/releases/download/v1.12.0/sing-box-1.12.0-android-amd64.tar.gz",
                  "size": 11000
                }
              ]
            }
        """.trimIndent())

        val info = GithubReleasePicker.pick(latest, "sing-box-1.12.0-android-amd64.tar.gz")
        assertThat(info.version).isEqualTo("1.12.0")
        assertThat(info.downloadUrl).contains("amd64")
    }

    @Test
    fun installExtractsTarGzAndSetsExecutable() {
        val coresDir = tmp.newFolder("cores")
        val archiveFile = tmp.newFile("sample.tar.gz")

        // Create a valid sing-box tar.gz
        TarArchiveOutputStream(GzipCompressorOutputStream(archiveFile.outputStream().buffered())).use { tar ->
            val bytes = "binary content".toByteArray()
            val entry = TarArchiveEntry("sing-box-1.12.0-android-arm64/sing-box").apply {
                size = bytes.size.toLong()
            }
            tar.putArchiveEntry(entry)
            tar.write(bytes)
            tar.closeArchiveEntry()
        }

        val out = CoreDownloader.installArchive(archiveFile, coresDir)
        assertThat(out.exists()).isTrue()
        assertThat(out.name).isEqualTo("sing-box")
        assertThat(out.readText()).isEqualTo("binary content")
    }

    @Test
    fun installRestoresBackupOnFailure() {
        val coresDir = tmp.newFolder("cores")
        val oldBinary = File(coresDir, "sing-box").apply { writeText("old binary") }
        val corruptArchive = tmp.newFile("corrupt.tar.gz").apply { writeText("not a tar") }

        val error = runCatching {
            CoreDownloader.installArchive(corruptArchive, coresDir)
        }.exceptionOrNull()

        assertThat(error).isNotNull()
        // Check old binary is restored
        assertThat(File(coresDir, "sing-box").exists()).isTrue()
        assertThat(File(coresDir, "sing-box").readText()).isEqualTo("old binary")
    }
}
