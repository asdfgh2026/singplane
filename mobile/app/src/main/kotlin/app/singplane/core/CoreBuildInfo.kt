package app.singplane.core

import app.singplane.BuildConfig

object CoreBuildInfo {
    val version: String = BuildConfig.SING_BOX_VERSION
    val displayName: String = displayName(version)

    fun displayName(version: String): String =
        if (version.isBlank() || version == "unknown") {
            "sing-box (version unknown)"
        } else {
            "sing-box ${version.removePrefix("v")}"
        }
}
