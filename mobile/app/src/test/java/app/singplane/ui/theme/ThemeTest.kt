package app.singplane.ui.theme

import androidx.compose.ui.graphics.Color
import com.google.common.truth.Truth.assertThat
import org.junit.Test

class ThemeTest {

    @Test
    fun seedColorMatchesDesktop() {
        assertThat(SeedColor).isEqualTo(Color(0xFF047857))
    }

    @Test
    fun darkColorsMatchDesktop() {
        assertThat(DarkColors.primary).isEqualTo(Color(0xFF047857))
        assertThat(DarkColors.background).isEqualTo(DarkBackground)
        assertThat(DarkColors.surface).isEqualTo(DarkSurface)
        assertThat(DarkColors.surfaceVariant).isEqualTo(DarkSurfaceVariant)
        assertThat(DarkColors.onSurface).isEqualTo(DarkForeground)
        assertThat(DarkColors.outline).isEqualTo(DarkBorder)
    }

    @Test
    fun lightColorsMatchDesktop() {
        assertThat(LightColors.primary).isEqualTo(Color(0xFF047857))
        assertThat(LightColors.background).isEqualTo(LightBackground)
        assertThat(LightColors.surface).isEqualTo(LightSurface)
        assertThat(LightColors.surfaceVariant).isEqualTo(LightSurfaceVariant)
        assertThat(LightColors.onSurface).isEqualTo(LightForeground)
        assertThat(LightColors.outline).isEqualTo(LightBorder)
    }
}
