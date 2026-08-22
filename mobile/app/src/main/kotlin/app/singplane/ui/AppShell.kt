package app.singplane.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import app.singplane.R
import app.singplane.ui.pages.ConnectionsPage
import app.singplane.ui.pages.HomePage
import app.singplane.ui.pages.LogsPage
import app.singplane.ui.pages.ProfilesPage
import app.singplane.ui.pages.ProxiesPage
import app.singplane.ui.pages.SettingsPage
import app.singplane.ui.pages.TemplatesPage
import app.singplane.vpn.NeedVpnConsent

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AppShell(onNeedVpnConsent: (NeedVpnConsent) -> Unit = {}) {
    var dest by rememberSaveable { mutableStateOf(AppDest.Home) }

    val isSubPage = dest == AppDest.Templates || dest == AppDest.Logs

    BackHandler(enabled = dest != AppDest.Home) {
        if (isSubPage) {
            dest = AppDest.Settings
        } else {
            dest = AppDest.Home
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(dest.labelRes)) },
                navigationIcon = {
                    if (isSubPage) {
                        IconButton(onClick = { dest = AppDest.Settings }) {
                            Icon(
                                Icons.AutoMirrored.Filled.ArrowBack,
                                contentDescription = stringResource(R.string.back),
                            )
                        }
                    }
                },
            )
        },
        bottomBar = {
            NavigationBar {
                AppDest.bottomNavItems.forEach { item ->
                    val label = stringResource(item.labelRes)
                    val selected = dest == item || (item == AppDest.Settings && isSubPage)
                    NavigationBarItem(
                        selected = selected,
                        onClick = { dest = item },
                        icon = { Icon(item.icon, contentDescription = label) },
                        label = {
                            Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        },
                    )
                }
            }
        },
    ) { inner ->
        Box(Modifier.padding(inner)) {
            when (dest) {
                AppDest.Home -> HomePage(onNeedVpnConsent = onNeedVpnConsent)
                AppDest.Proxies -> ProxiesPage()
                AppDest.Connections -> ConnectionsPage()
                AppDest.Profiles -> ProfilesPage()
                AppDest.Templates -> TemplatesPage()
                AppDest.Logs -> LogsPage()
                AppDest.Settings -> SettingsPage(
                    onNavigateToTemplates = { dest = AppDest.Templates },
                    onNavigateToLogs = { dest = AppDest.Logs },
                )
            }
        }
    }
}


