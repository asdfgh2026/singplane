package app.singplane.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

val DarkColors = darkColorScheme(
    primary = SeedColor,
    onPrimary = Color.White,
    primaryContainer = DarkAccent,
    onPrimaryContainer = DarkOnAccent,
    secondary = DarkSecondary,
    onSecondary = DarkOnSecondary,
    secondaryContainer = DarkSecondary,
    onSecondaryContainer = DarkOnSecondary,
    background = DarkBackground,
    onBackground = DarkForeground,
    surface = DarkSurface,
    onSurface = DarkForeground,
    surfaceVariant = DarkSurfaceVariant,
    onSurfaceVariant = DarkForeground,
    outline = DarkBorder,
    error = DarkError,
    onError = Color(0xFF450A0A),
)

val LightColors = lightColorScheme(
    primary = SeedColor,
    onPrimary = Color.White,
    primaryContainer = LightAccent,
    onPrimaryContainer = LightOnAccent,
    secondary = LightSecondary,
    onSecondary = LightOnSecondary,
    secondaryContainer = LightSecondary,
    onSecondaryContainer = LightOnSecondary,
    background = LightBackground,
    onBackground = LightForeground,
    surface = LightSurface,
    onSurface = LightForeground,
    surfaceVariant = LightSurfaceVariant,
    onSurfaceVariant = LightForeground,
    outline = LightBorder,
    error = LightError,
    onError = Color.White,
)

val AppShapes = Shapes(
    extraSmall = RoundedCornerShape(8.dp),
    small = RoundedCornerShape(10.dp),
    medium = RoundedCornerShape(14.dp),
    large = RoundedCornerShape(16.dp),
    extraLarge = RoundedCornerShape(20.dp),
)

@Composable
fun SingPanelTheme(
    themeMode: String = "system",
    darkTheme: Boolean = when (themeMode) {
        "dark" -> true
        "light" -> false
        else -> isSystemInDarkTheme()
    },
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        shapes = AppShapes,
        content = content,
    )
}
