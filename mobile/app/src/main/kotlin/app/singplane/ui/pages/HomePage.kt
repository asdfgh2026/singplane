package app.singplane.ui.pages

import app.singplane.core.CorePhase
import app.singplane.core.CoreBuildInfo
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PowerSettingsNew
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.singplane.core.LocalControlPlane
import app.singplane.net.IpCheckSource
import app.singplane.net.IpInfo
import app.singplane.net.NetDetect
import app.singplane.vpn.NeedVpnConsent
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.io.File
import androidx.compose.ui.res.stringResource
import app.singplane.R

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.ui.platform.LocalContext
import app.singplane.assemble.TailscaleStatus
import app.singplane.assemble.TsPhase

@Composable
fun HomePage(onNeedVpnConsent: (NeedVpnConsent) -> Unit = {}) {
    val plane = LocalControlPlane.current
    val context = LocalContext.current
    val snap by plane.status.collectAsStateWithLifecycle()
    val settings by plane.settings.collectAsStateWithLifecycle()
    val profiles by plane.profiles.collectAsStateWithLifecycle()
    val logs by plane.logs.collectAsStateWithLifecycle()
    val mode by plane.mode.collectAsStateWithLifecycle()
    val memoryBytes by plane.memoryBytes.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val active = profiles.firstOrNull { it.id == settings.activeProfileId }
    val coreName = CoreBuildInfo.displayName



    var tsTick by remember { mutableStateOf(0) }
    LaunchedEffect(snap.running, settings.tailscale.enabled) {
        tsTick++
        if (snap.running && settings.tailscale.enabled) {
            while (isActive) {
                delay(2000)
                tsTick++
            }
        }
    }
    val tsStatus = remember(settings.tailscale, snap.running, logs, tsTick) {
        val ident = TailscaleStatus.discoverSelf(
            TailscaleStatus.stateDirs(context.filesDir, settings.tailscale),
        )
        TailscaleStatus.statusFromLog(settings.tailscale, snap.running, logs, ident)
    }

    // Net detect state
    var ipInfo by remember { mutableStateOf<IpInfo?>(null) }
    var isDetectingIp by remember { mutableStateOf(false) }
    var ipMasked by remember { mutableStateOf(false) }
    var currentSource by remember { mutableStateOf(IpCheckSource.AUTO) }
    val lanIp = remember { NetDetect.getLocalIpv4() }

    // Running duration timer
    var uptimeSeconds by remember { mutableStateOf(0L) }
    LaunchedEffect(snap.running, snap.startedAtMs) {
        if (snap.running && snap.startedAtMs > 0) {
            while (isActive) {
                uptimeSeconds = (System.currentTimeMillis() - snap.startedAtMs) / 1000
                delay(1000)
            }
        } else {
            uptimeSeconds = 0L
        }
    }

    // Auto trigger net detect on running or manual refresh
    val triggerNetDetect = {
        scope.launch {
            isDetectingIp = true
            val res = NetDetect.detect(currentSource)
            ipInfo = res.getOrNull()
            isDetectingIp = false
        }
    }

    LaunchedEffect(snap.running) {
        if (snap.running) {
            delay(1000)
            triggerNetDetect()
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Spacer(Modifier.height(4.dp))
        Text("SingPanel", style = MaterialTheme.typography.headlineMedium)

        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                when (snap.phase) {
                    CorePhase.Running -> stringResource(R.string.home_status_running)
                    CorePhase.Starting -> "启动中..."
                    CorePhase.Stopping -> "停止中..."
                    CorePhase.Error -> "启动失败"
                    CorePhase.Stopped -> stringResource(R.string.home_status_stopped)
                },
                style = MaterialTheme.typography.titleLarge,
                color = when (snap.phase) {
                    CorePhase.Running -> MaterialTheme.colorScheme.primary
                    CorePhase.Error -> MaterialTheme.colorScheme.error
                    CorePhase.Starting -> MaterialTheme.colorScheme.primary
                    else -> MaterialTheme.colorScheme.onSurfaceVariant
                },
            )
            if (active != null) {
                Spacer(Modifier.height(4.dp))
                Text(
                    stringResource(R.string.home_current_profile, active.name),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (snap.message.isNotBlank() && snap.phase != CorePhase.Running) {
                Spacer(Modifier.height(4.dp))
                Text(
                    snap.message,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (snap.phase == CorePhase.Error) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }


        FilledIconButton(
            onClick = {
                scope.launch {
                    try {
                        if (snap.running) plane.stop() else plane.start()
                    } catch (e: NeedVpnConsent) {
                        onNeedVpnConsent(e)
                    } catch (_: Throwable) {
                    }

                }
            },
            modifier = Modifier.size(96.dp),
            shape = CircleShape,
            colors = IconButtonDefaults.filledIconButtonColors(
                containerColor = if (snap.running) {
                    MaterialTheme.colorScheme.primary
                } else {
                    MaterialTheme.colorScheme.surfaceVariant
                },
            ),
        ) {
            Icon(
                Icons.Filled.PowerSettingsNew,
                contentDescription = if (snap.running) stringResource(R.string.home_action_stop) else stringResource(R.string.home_action_start),
                modifier = Modifier.size(44.dp),
            )
        }

        // Mode Switcher Chips (Rule / Global / Direct)
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val modes = listOf(
                "rule" to stringResource(R.string.home_mode_rule),
                "global" to stringResource(R.string.home_mode_global),
                "direct" to stringResource(R.string.home_mode_direct),
            )
            modes.forEach { (mKey, mLabel) ->
                FilterChip(
                    selected = mode.equals(mKey, ignoreCase = true),
                    onClick = {
                        scope.launch {
                            plane.changeMode(mKey)
                        }
                    },
                    label = { Text(mLabel) },
                    modifier = Modifier.padding(horizontal = 4.dp),
                )
            }
        }

        // Metrics Card (When running)
        if (snap.running) {
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
                ),
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Text(stringResource(R.string.home_uptime), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        val h = uptimeSeconds / 3600
                        val m = (uptimeSeconds % 3600) / 60
                        val s = uptimeSeconds % 60
                        Text(String.format("%02d:%02d:%02d", h, m, s), style = MaterialTheme.typography.bodyMedium, fontFamily = FontFamily.Monospace)
                    }
                    if (memoryBytes > 0) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Text(stringResource(R.string.home_core_memory), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            val mb = memoryBytes.toDouble() / (1024 * 1024)
                            Text(String.format("%.1f MB", mb), style = MaterialTheme.typography.bodyMedium)
                        }
                    }
                    if (!lanIp.isNullOrBlank()) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Text(stringResource(R.string.home_lan_ip), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            Text(lanIp, style = MaterialTheme.typography.bodyMedium, fontFamily = FontFamily.Monospace)
                        }
                    }
                }
            }
        }

        // Outbound Network Detection Card
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
            ),
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(stringResource(R.string.home_outbound_network_check), style = MaterialTheme.typography.titleSmall)
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        IconButton(
                            onClick = { ipMasked = !ipMasked },
                            modifier = Modifier.size(32.dp),
                        ) {
                            Icon(
                                if (ipMasked) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                                contentDescription = stringResource(R.string.home_mask_ip_desc),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        IconButton(
                            onClick = { triggerNetDetect() },
                            modifier = Modifier.size(32.dp),
                        ) {
                            if (isDetectingIp) {
                                CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                            } else {
                                Icon(Icons.Filled.Refresh, contentDescription = stringResource(R.string.proxies_refresh_desc), modifier = Modifier.size(18.dp))
                            }
                        }
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(stringResource(R.string.home_outbound_ip), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    val displayIp = when {
                        ipInfo == null -> if (isDetectingIp) stringResource(R.string.home_ip_detecting) else stringResource(R.string.home_ip_tap_to_detect)
                        ipMasked -> NetDetect.MASKED_IP
                        else -> "${ipInfo?.flagEmoji} ${ipInfo?.ip} (${ipInfo?.countryCode})"
                    }
                    Text(
                        displayIp,
                        style = MaterialTheme.typography.bodyMedium,
                        fontFamily = FontFamily.Monospace,
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(stringResource(R.string.home_detect_source), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(
                        stringResource(currentSource.labelRes),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.clickable {
                            currentSource = when (currentSource) {
                                IpCheckSource.AUTO -> IpCheckSource.INTERNATIONAL
                                IpCheckSource.INTERNATIONAL -> IpCheckSource.DOMESTIC
                                IpCheckSource.DOMESTIC -> IpCheckSource.AUTO
                            }
                            triggerNetDetect()
                        },
                    )
                }
            }
        }

        // Tailscale Card — header + phase chip; subtitle on its own line (desktop home.rs)
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
            ),
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Row(
                        modifier = Modifier.weight(1f),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(stringResource(R.string.home_tailscale_title), style = MaterialTheme.typography.titleSmall)
                        TsPhaseBadge(tsStatus.phase, tsStatus.title)
                    }
                    Switch(
                        checked = settings.tailscale.enabled,
                        onCheckedChange = {
                            scope.launch {
                                plane.updateSettings(settings.copy(tailscale = settings.tailscale.copy(enabled = it)))
                            }
                        },
                    )
                }

                if (tsStatus.subtitle.isNotBlank()) {
                    Text(
                        tsStatus.subtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontFamily = if (tsStatus.loginUrl != null) FontFamily.Monospace else FontFamily.Default,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }

                if (tsStatus.selfIp != null) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            tsStatus.selfIp,
                            style = MaterialTheme.typography.bodyMedium,
                            fontFamily = FontFamily.Monospace,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        OutlinedButton(
                            onClick = {
                                val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                clipboard.setPrimaryClip(ClipData.newPlainText("Tailscale IP", tsStatus.selfIp))
                            },
                        ) {
                            Text(stringResource(R.string.home_tailscale_copy_ip))
                        }
                    }
                }

                if (tsStatus.loginUrl != null) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Button(
                            onClick = {
                                val intent = Intent(Intent.ACTION_VIEW, Uri.parse(tsStatus.loginUrl))
                                context.startActivity(intent)
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text(
                                stringResource(R.string.home_tailscale_open_login),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        OutlinedButton(
                            onClick = {
                                val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                clipboard.setPrimaryClip(ClipData.newPlainText("Tailscale Login", tsStatus.loginUrl))
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text(
                                stringResource(R.string.home_tailscale_copy_login),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }
            }
        }

        // Info bar
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
            ),
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(stringResource(R.string.home_mixed_port), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text("${settings.mixedPort}", style = MaterialTheme.typography.bodyMedium)
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(stringResource(R.string.home_clash_controller), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(settings.clashApiController, style = MaterialTheme.typography.bodyMedium)
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(stringResource(R.string.home_core_program), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(coreName, style = MaterialTheme.typography.bodyMedium)
                }
            }
        }

        // Message banner
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = if (snap.running) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant,
            ),
        ) {
            Text(
                snap.message,
                modifier = Modifier.padding(16.dp),
                style = MaterialTheme.typography.bodyMedium,
                color = if (snap.running) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun TsPhaseBadge(phase: TsPhase, title: String) {
    val (bg, fg) = when (phase) {
        TsPhase.Injected, TsPhase.Ready ->
            MaterialTheme.colorScheme.primaryContainer to MaterialTheme.colorScheme.onPrimaryContainer
        TsPhase.Pending, TsPhase.NeedsLogin ->
            MaterialTheme.colorScheme.tertiaryContainer to MaterialTheme.colorScheme.onTertiaryContainer
        TsPhase.Error ->
            MaterialTheme.colorScheme.errorContainer to MaterialTheme.colorScheme.onErrorContainer
        TsPhase.Disabled ->
            MaterialTheme.colorScheme.surfaceVariant to MaterialTheme.colorScheme.onSurfaceVariant
    }
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(8.dp))
            .background(bg)
            .padding(horizontal = 8.dp, vertical = 3.dp),
    ) {
        Text(title, style = MaterialTheme.typography.labelMedium, color = fg, maxLines = 1)
    }
}
