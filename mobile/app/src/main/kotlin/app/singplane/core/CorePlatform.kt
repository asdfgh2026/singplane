package app.singplane.core

object CorePlatform {
    fun assetFileName(version: String, os: String, arch: String): String {
        val v = version.removePrefix("v")
        val suffix = if (os == "windows") ".zip" else ".tar.gz"
        return "sing-box-$v-$os-$arch$suffix"
    }

    fun binaryFileName(os: String): String =
        if (os == "windows") "sing-box.exe" else "sing-box"

    fun androidArch(abi: String): String = when {
        abi.contains("arm64") -> "arm64"
        abi.contains("armeabi") -> "armv7"
        abi.contains("x86_64") -> "amd64"
        abi.contains("x86") -> "386"
        else -> "arm64"
    }
}
