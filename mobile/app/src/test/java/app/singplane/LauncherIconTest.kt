package app.singplane

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.io.DataInputStream
import java.io.FileInputStream

class LauncherIconTest {

    private val resDir = File("src/main/res")
    private val manifestFile = File("src/main/AndroidManifest.xml")

    private fun getPngDimensions(file: File): Pair<Int, Int> {
        DataInputStream(FileInputStream(file)).use { input ->
            val header = ByteArray(8)
            input.readFully(header)
            // Skip chunk length (4) + chunk type "IHDR" (4)
            input.skipBytes(8)
            val width = input.readInt()
            val height = input.readInt()
            return Pair(width, height)
        }
    }

    @Test
    fun manifestDeclaresLauncherIcons() {
        assertTrue("AndroidManifest.xml should exist", manifestFile.exists())
        val content = manifestFile.readText()
        assertTrue("Manifest should reference @mipmap/ic_launcher", content.contains("android:icon=\"@mipmap/ic_launcher\""))
        assertTrue("Manifest should reference @mipmap/ic_launcher_round", content.contains("android:roundIcon=\"@mipmap/ic_launcher_round\""))
    }

    @Test
    fun mipmapDirectoriesContainValidIcons() {
        val expectedDensities = mapOf(
            "mipmap-mdpi" to 48,
            "mipmap-hdpi" to 72,
            "mipmap-xhdpi" to 96,
            "mipmap-xxhdpi" to 144,
            "mipmap-xxxhdpi" to 192
        )

        for ((folder, size) in expectedDensities) {
            val dir = File(resDir, folder)
            assertTrue("Directory $folder should exist", dir.isDirectory)

            val squareIcon = File(dir, "ic_launcher.png")
            assertTrue("Square icon in $folder should exist", squareIcon.isFile)
            val (sqW, sqH) = getPngDimensions(squareIcon)
            assertEquals("Square icon width in $folder", size, sqW)
            assertEquals("Square icon height in $folder", size, sqH)

            val roundIcon = File(dir, "ic_launcher_round.png")
            assertTrue("Round icon in $folder should exist", roundIcon.isFile)
            val (rdW, rdH) = getPngDimensions(roundIcon)
            assertEquals("Round icon width in $folder", size, rdW)
            assertEquals("Round icon height in $folder", size, rdH)
        }
    }
}
