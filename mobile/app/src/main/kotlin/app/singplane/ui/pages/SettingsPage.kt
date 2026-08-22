package app.singplane.ui.pages

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowForwardIos
import androidx.compose.material.icons.automirrored.outlined.Article
import androidx.compose.material.icons.outlined.Dashboard
import androidx.compose.material.icons.outlined.Gavel
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.singplane.R
import app.singplane.core.CoreBuildInfo
import app.singplane.core.LocalControlPlane
import app.singplane.ui.dialogs.AboutDialog
import app.singplane.ui.dialogs.DisclaimerDialog
import kotlinx.coroutines.launch

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun SettingsPage(
    onNavigateToTemplates: () -> Unit = {},
    onNavigateToLogs: () -> Unit = {},
) {
    val plane = LocalControlPlane.current
    val settings by plane.settings.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val context = LocalContext.current

    var showAboutDialog by remember { mutableStateOf(false) }
    var showDisclaimerDialog by remember { mutableStateOf(false) }

    if (showAboutDialog) {
        AboutDialog(onDismiss = { showAboutDialog = false })
    }

    if (showDisclaimerDialog) {
        DisclaimerDialog(
            onAccept = {
                scope.launch {
                    plane.updateSettings(settings.copy(disclaimerAccepted = true))
                }
            },
            onDismiss = { showDisclaimerDialog = false },
        )
    }


    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        // 1. Core Info Card
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)),
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(stringResource(R.string.home_core_program), style = MaterialTheme.typography.titleMedium)
                Text(CoreBuildInfo.displayName, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.primary)
            }
        }

        // 2. Appearance & General Settings
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)),
        ) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Text(stringResource(R.string.settings_appearance_theme), style = MaterialTheme.typography.titleMedium)

                // Theme Mode
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(stringResource(R.string.settings_theme_mode), style = MaterialTheme.typography.bodyMedium)
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        val themes = listOf(
                            "system" to R.string.settings_theme_system,
                            "light" to R.string.settings_theme_light,
                            "dark" to R.string.settings_theme_dark,
                        )
                        themes.forEach { (mode, nameRes) ->
                            FilterChip(
                                selected = settings.themeMode == mode,
                                onClick = { scope.launch { plane.updateSettings(settings.copy(themeMode = mode)) } },
                                label = { Text(stringResource(nameRes), style = MaterialTheme.typography.labelSmall) },
                                modifier = Modifier.weight(1f),
                            )
                        }
                    }
                }

                // Language (2x2 grid to prevent text wrapping)
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(stringResource(R.string.settings_language_mode), style = MaterialTheme.typography.bodyMedium)
                    
                    val langsRow1 = listOf(
                        "system" to R.string.settings_lang_system,
                        "zh-Hans" to R.string.settings_lang_zh_hans,
                    )
                    val langsRow2 = listOf(
                        "zh-Hant" to R.string.settings_lang_zh_hant,
                        "en" to R.string.settings_lang_en,
                    )

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        langsRow1.forEach { (code, nameRes) ->
                            FilterChip(
                                selected = settings.language == code,
                                onClick = { scope.launch { plane.updateSettings(settings.copy(language = code)) } },
                                label = {
                                    Text(
                                        stringResource(nameRes),
                                        modifier = Modifier.fillMaxWidth(),
                                        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                                        maxLines = 1,
                                    )
                                },
                                modifier = Modifier.weight(1f),
                            )
                        }
                    }

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        langsRow2.forEach { (code, nameRes) ->
                            FilterChip(
                                selected = settings.language == code,
                                onClick = { scope.launch { plane.updateSettings(settings.copy(language = code)) } },
                                label = {
                                    Text(
                                        stringResource(nameRes),
                                        modifier = Modifier.fillMaxWidth(),
                                        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                                        maxLines = 1,
                                    )
                                },
                                modifier = Modifier.weight(1f),
                            )
                        }
                    }
                }

            }
        }

        // 3. Subscription Auto Update
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)),
        ) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(stringResource(R.string.settings_auto_update_sub), style = MaterialTheme.typography.titleMedium)
                Text(stringResource(R.string.settings_auto_update_sub_desc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)

                androidx.compose.foundation.layout.FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    val intervals = listOf(
                        0 to R.string.interval_off,
                        15 to R.string.interval_15m,
                        60 to R.string.interval_1h,
                        360 to R.string.interval_6h,
                        720 to R.string.interval_12h,
                        1440 to R.string.interval_24h,
                    )
                    intervals.forEach { (mins, resId) ->
                        FilterChip(
                            selected = settings.autoUpdateIntervalMinutes == mins,
                            onClick = {
                                scope.launch {
                                    plane.updateSettings(settings.copy(autoUpdateIntervalMinutes = mins))
                                    app.singplane.worker.SubscriptionWorker.schedule(context, mins)
                                }
                            },
                            label = { Text(stringResource(resId)) },
                        )
                    }
                }
            }
        }

        // 4. Network & Ports Configuration
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)),
        ) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.settings_network_ports), style = MaterialTheme.typography.titleMedium)

                OutlinedTextField(
                    value = settings.mixedPort.toString(),
                    onValueChange = {
                        val port = it.toIntOrNull() ?: return@OutlinedTextField
                        scope.launch { plane.updateSettings(settings.copy(mixedPort = port)) }
                    },
                    label = { Text(stringResource(R.string.settings_mixed_inbound_port)) },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                )

                OutlinedTextField(
                    value = settings.clashApiPort.toString(),
                    onValueChange = {
                        val port = it.toIntOrNull() ?: return@OutlinedTextField
                        scope.launch { plane.updateSettings(settings.copy(clashApiPort = port)) }
                    },
                    label = { Text(stringResource(R.string.settings_clash_api_port)) },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                )

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(stringResource(R.string.settings_strip_tun_on_assemble))
                        Text(stringResource(R.string.settings_strip_tun_on_assemble_desc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Switch(
                        checked = settings.stripTunOnAssemble,
                        onCheckedChange = {
                            scope.launch { plane.updateSettings(settings.copy(stripTunOnAssemble = it)) }
                        },
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(stringResource(R.string.settings_force_app_ports))
                        Text(stringResource(R.string.settings_force_app_ports_desc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Switch(
                        checked = settings.forceAppPortsOnAssemble,
                        onCheckedChange = {
                            scope.launch { plane.updateSettings(settings.copy(forceAppPortsOnAssemble = it)) }
                        },
                    )
                }
            }
        }

        // 5. Tailscale (Overlay Configuration)
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)),
        ) {
            val ts = settings.tailscale
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.settings_tailscale_title), style = MaterialTheme.typography.titleMedium)

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(stringResource(R.string.settings_ts_enable))
                        Text(stringResource(R.string.settings_ts_enable_desc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Switch(
                        checked = ts.enabled,
                        onCheckedChange = {
                            scope.launch { plane.updateSettings(settings.copy(tailscale = ts.copy(enabled = it))) }
                        },
                    )
                }

                if (ts.enabled) {
                    OutlinedTextField(
                        value = ts.authKey,
                        onValueChange = {
                            scope.launch { plane.updateSettings(settings.copy(tailscale = ts.copy(authKey = it.trim()))) }
                        },
                        label = { Text(stringResource(R.string.settings_ts_auth_key)) },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )

                    OutlinedTextField(
                        value = ts.hostname,
                        onValueChange = {
                            scope.launch { plane.updateSettings(settings.copy(tailscale = ts.copy(hostname = it.trim()))) }
                        },
                        label = { Text(stringResource(R.string.settings_ts_hostname)) },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            stringResource(R.string.settings_ts_accept_routes),
                            modifier = Modifier.weight(1f),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        Switch(
                            checked = ts.acceptRoutes,
                            onCheckedChange = {
                                scope.launch { plane.updateSettings(settings.copy(tailscale = ts.copy(acceptRoutes = it))) }
                            },
                        )
                    }

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            stringResource(R.string.settings_ts_inject_dns),
                            modifier = Modifier.weight(1f),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        Switch(
                            checked = ts.injectDns,
                            onCheckedChange = {
                                scope.launch { plane.updateSettings(settings.copy(tailscale = ts.copy(injectDns = it))) }
                            },
                        )
                    }

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            stringResource(R.string.settings_ts_preferred_route),
                            modifier = Modifier.weight(1f),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        Switch(
                            checked = ts.injectRoutePreferredBy,
                            onCheckedChange = {
                                scope.launch { plane.updateSettings(settings.copy(tailscale = ts.copy(injectRoutePreferredBy = it))) }
                            },
                        )
                    }

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            stringResource(R.string.settings_ts_replace_other),
                            modifier = Modifier.weight(1f),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        Switch(
                            checked = ts.replaceOtherTailscale,
                            onCheckedChange = {
                                scope.launch { plane.updateSettings(settings.copy(tailscale = ts.copy(replaceOtherTailscale = it))) }
                            },
                        )
                    }

                }
            }
        }

        // 6. Advanced Tools & Diagnostic
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)),
        ) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(stringResource(R.string.settings_tools_title), style = MaterialTheme.typography.titleMedium)

                Surface(
                    onClick = onNavigateToTemplates,
                    shape = RoundedCornerShape(8.dp),
                    color = MaterialTheme.colorScheme.surface,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        modifier = Modifier.padding(14.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(Icons.Outlined.Dashboard, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                            Column {
                                Text(stringResource(R.string.tab_templates), style = MaterialTheme.typography.bodyLarge)
                                Text(stringResource(R.string.settings_templates_desc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                        Icon(Icons.AutoMirrored.Outlined.ArrowForwardIos, contentDescription = null, modifier = Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }

                Surface(
                    onClick = onNavigateToLogs,
                    shape = RoundedCornerShape(8.dp),
                    color = MaterialTheme.colorScheme.surface,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        modifier = Modifier.padding(14.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(Icons.AutoMirrored.Outlined.Article, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                            Column {
                                Text(stringResource(R.string.tab_logs), style = MaterialTheme.typography.bodyLarge)
                                Text(stringResource(R.string.settings_logs_desc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                        Icon(Icons.AutoMirrored.Outlined.ArrowForwardIos, contentDescription = null, modifier = Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }

        // 7. About & Legal
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)),
        ) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(stringResource(R.string.tab_settings), style = MaterialTheme.typography.titleMedium)

                Surface(
                    onClick = { showAboutDialog = true },
                    shape = RoundedCornerShape(8.dp),
                    color = MaterialTheme.colorScheme.surface,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        modifier = Modifier.padding(14.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(Icons.Outlined.Info, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                            Column {
                                Text(stringResource(R.string.settings_about_title), style = MaterialTheme.typography.bodyLarge)
                                Text(stringResource(R.string.settings_about_desc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                        Icon(Icons.AutoMirrored.Outlined.ArrowForwardIos, contentDescription = null, modifier = Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }

                Surface(
                    onClick = { showDisclaimerDialog = true },
                    shape = RoundedCornerShape(8.dp),
                    color = MaterialTheme.colorScheme.surface,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        modifier = Modifier.padding(14.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(Icons.Outlined.Gavel, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                            Column {
                                Text(stringResource(R.string.settings_disclaimer_title), style = MaterialTheme.typography.bodyLarge)
                                Text(stringResource(R.string.settings_disclaimer_desc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                        Icon(Icons.AutoMirrored.Outlined.ArrowForwardIos, contentDescription = null, modifier = Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }

        Spacer(Modifier.height(8.dp))
        Text(
            stringResource(R.string.settings_disclaimer),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
