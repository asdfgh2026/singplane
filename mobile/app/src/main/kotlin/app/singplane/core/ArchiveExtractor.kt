package app.singplane.core

import org.apache.commons.compress.archivers.tar.TarArchiveInputStream
import org.apache.commons.compress.compressors.gzip.GzipCompressorInputStream
import java.io.File
import java.util.zip.ZipInputStream

object ArchiveExtractor {
    fun extractCore(archive: File, destDir: File, binaryName: String): File {
        destDir.mkdirs()
        val out = File(destDir, binaryName)
        val found = when {
            archive.name.endsWith(".zip", ignoreCase = true) -> extractZip(archive, binaryName)
            else -> extractTarGz(archive, binaryName)
        } ?: error("压缩包里没有 $binaryName")
        val tmp = File(destDir, "$binaryName.tmp")
        tmp.writeBytes(found)
        if (out.exists()) out.delete()
        if (!tmp.renameTo(out)) {
            tmp.copyTo(out, overwrite = true)
            tmp.delete()
        }
        out.setExecutable(true, false)
        return out
    }

    private fun extractTarGz(archive: File, binaryName: String): ByteArray? {
        TarArchiveInputStream(GzipCompressorInputStream(archive.inputStream().buffered())).use { tar ->
            while (true) {
                val e = tar.nextEntry ?: break
                if (e.isDirectory) continue
                if (!nameMatches(e.name, binaryName)) continue
                return tar.readBytes()
            }
        }
        return null
    }

    private fun extractZip(archive: File, binaryName: String): ByteArray? {
        ZipInputStream(archive.inputStream().buffered()).use { zip ->
            while (true) {
                val e = zip.nextEntry ?: break
                if (e.isDirectory) continue
                if (!nameMatches(e.name, binaryName)) continue
                return zip.readBytes()
            }
        }
        return null
    }

    private fun nameMatches(entry: String, binaryName: String): Boolean {
        val base = entry.replace('\\', '/').substringAfterLast('/')
        return base == binaryName && !entry.contains("..")
    }
}
