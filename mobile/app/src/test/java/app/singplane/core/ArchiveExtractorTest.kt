package app.singplane.core

import com.google.common.truth.Truth.assertThat
import org.apache.commons.compress.archivers.tar.TarArchiveEntry
import org.apache.commons.compress.archivers.tar.TarArchiveOutputStream
import org.apache.commons.compress.compressors.gzip.GzipCompressorOutputStream
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

class ArchiveExtractorTest {
    @get:Rule
    val tmp = TemporaryFolder()

    @Test
    fun extractsSingBoxFromTarGz() {
        val archive = tmp.newFile("c.tar.gz")
        TarArchiveOutputStream(GzipCompressorOutputStream(archive.outputStream())).use { tar ->
            val bytes = "bin".toByteArray()
            val e = TarArchiveEntry("sing-box-1.0.0-android-arm64/sing-box")
            e.size = bytes.size.toLong()
            tar.putArchiveEntry(e)
            tar.write(bytes)
            tar.closeArchiveEntry()
        }
        val dest = tmp.newFolder("out")
        val bin = ArchiveExtractor.extractCore(archive, dest, "sing-box")
        assertThat(bin.readText()).isEqualTo("bin")
        assertThat(bin.name).isEqualTo("sing-box")
    }

    @Test
    fun extractsSingBoxFromZip() {
        val archive = tmp.newFile("c.zip")
        ZipOutputStream(archive.outputStream()).use { zip ->
            zip.putNextEntry(ZipEntry("foo/sing-box.exe"))
            zip.write("exe".toByteArray())
            zip.closeEntry()
        }
        val dest = tmp.newFolder("out")
        val bin = ArchiveExtractor.extractCore(archive, dest, "sing-box.exe")
        assertThat(bin.readText()).isEqualTo("exe")
    }
}
