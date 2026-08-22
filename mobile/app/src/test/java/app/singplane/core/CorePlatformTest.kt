package app.singplane.core

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class CorePlatformTest {
    @Test
    fun androidArm64Asset() {
        assertThat(CorePlatform.assetFileName("1.12.0", os = "android", arch = "arm64"))
            .isEqualTo("sing-box-1.12.0-android-arm64.tar.gz")
    }

    @Test
    fun androidAmd64Asset() {
        assertThat(CorePlatform.assetFileName("v1.13.0", os = "android", arch = "amd64"))
            .isEqualTo("sing-box-1.13.0-android-amd64.tar.gz")
    }

    @Test
    fun windowsZip() {
        assertThat(CorePlatform.assetFileName("1.12.0", os = "windows", arch = "amd64"))
            .isEqualTo("sing-box-1.12.0-windows-amd64.zip")
    }

    @Test
    fun binaryName() {
        assertThat(CorePlatform.binaryFileName("android")).isEqualTo("sing-box")
        assertThat(CorePlatform.binaryFileName("windows")).isEqualTo("sing-box.exe")
    }
}
