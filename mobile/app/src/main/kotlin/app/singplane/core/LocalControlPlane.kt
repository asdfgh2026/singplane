package app.singplane.core

import androidx.compose.runtime.staticCompositionLocalOf

val LocalControlPlane = staticCompositionLocalOf<ControlPlane> {
    error("ControlPlane not provided")
}
