package app.singplane.ui

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class AppDestTest {
    @Test
    fun bottomNavItemsExcludesTemplatesAndLogs() {
        val items = AppDest.bottomNavItems
        assertThat(items).containsExactly(
            AppDest.Home,
            AppDest.Proxies,
            AppDest.Connections,
            AppDest.Profiles,
            AppDest.Settings,
        ).inOrder()

        assertThat(items).doesNotContain(AppDest.Templates)
        assertThat(items).doesNotContain(AppDest.Logs)
        assertThat(items).hasSize(5)
    }
}
