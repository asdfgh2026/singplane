package app.singplane.ui

import androidx.annotation.StringRes
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Article
import androidx.compose.material.icons.automirrored.outlined.CompareArrows
import androidx.compose.material.icons.outlined.Dashboard
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.Hub
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.SpaceDashboard
import androidx.compose.ui.graphics.vector.ImageVector
import app.singplane.R

enum class AppDest(
    @StringRes val labelRes: Int,
    val icon: ImageVector,
) {
    Home(R.string.tab_home, Icons.Outlined.SpaceDashboard),
    Proxies(R.string.tab_proxies, Icons.Outlined.Hub),
    Connections(R.string.tab_connections, Icons.AutoMirrored.Outlined.CompareArrows),
    Profiles(R.string.tab_profiles, Icons.Outlined.Folder),
    Templates(R.string.tab_templates, Icons.Outlined.Dashboard),
    Logs(R.string.tab_logs, Icons.AutoMirrored.Outlined.Article),
    Settings(R.string.tab_settings, Icons.Outlined.Settings);

    companion object {
        /** The 5 primary bottom navigation items (Templates & Logs are accessed via Settings). */
        val bottomNavItems = listOf(Home, Proxies, Connections, Profiles, Settings)
    }
}

