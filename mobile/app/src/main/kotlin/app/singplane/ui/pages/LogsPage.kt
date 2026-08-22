package app.singplane.ui.pages

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.singplane.R
import app.singplane.core.LocalControlPlane
import androidx.annotation.StringRes
import kotlinx.coroutines.launch

enum class LogBranchFilter(val label: String, val keywords: List<String>) {
    ALL("全部", emptyList()),
    CONTROL("控制面", listOf("ControlPlane", "assemble", "profile", "template", "settings", "store", "start", "stop")),
    CORE("核心内核", listOf("libbox", "sing-box", "box", "inbound", "outbound")),
    VPN("VPN/隧道", listOf("vpn", "tun", "protect", "interface", "fd")),
    ROUTE_DNS("路由/DNS", listOf("dns", "route", "rule", "domain", "ip")),
    TAILSCALE("Tailscale", listOf("tailscale", "ts_", "derp", "magicdns")),
}

enum class LogLevelFilter(@StringRes val labelRes: Int, val pattern: String) {
    ALL(R.string.logs_level_all, ""),
    DEBUG(R.string.logs_level_debug, "DEBUG"),
    INFO(R.string.logs_level_info, "INFO"),
    WARN(R.string.logs_level_warn, "WARN"),
    ERROR(R.string.logs_level_error, "ERROR"),
}

@Composable
fun LogsPage() {
    val plane = LocalControlPlane.current
    val rawLogs by plane.logs.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    var searchQuery by remember { mutableStateOf("") }
    var selectedBranch by remember { mutableStateOf(LogBranchFilter.ALL) }
    var selectedLevel by remember { mutableStateOf(LogLevelFilter.ALL) }
    var autoScroll by remember { mutableStateOf(true) }

    val verticalScrollState = rememberScrollState()

    // Filter logs
    val filteredLogs = remember(rawLogs, searchQuery, selectedBranch, selectedLevel) {
        if (rawLogs.isBlank()) return@remember ""
        val lines = rawLogs.lines()
        val filtered = lines.filter { line ->
            val branchMatch = when (selectedBranch) {
                LogBranchFilter.ALL -> true
                else -> selectedBranch.keywords.any { line.contains(it, ignoreCase = true) }
            }
            val levelMatch = when (selectedLevel) {
                LogLevelFilter.ALL -> true
                else -> line.contains(selectedLevel.pattern, ignoreCase = true)
            }
            val searchMatch = if (searchQuery.isBlank()) true else line.contains(searchQuery, ignoreCase = true)
            branchMatch && levelMatch && searchMatch
        }
        filtered.joinToString("\n")
    }

    // Auto-scroll to bottom when new logs arrive and autoScroll is enabled
    LaunchedEffect(filteredLogs, autoScroll) {
        if (autoScroll && filteredLogs.isNotEmpty()) {
            verticalScrollState.scrollTo(verticalScrollState.maxValue)
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        // Search bar & actions
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = searchQuery,
                onValueChange = { searchQuery = it },
                placeholder = { Text(stringResource(R.string.logs_search_placeholder)) },
                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = stringResource(R.string.connections_search_desc)) },
                modifier = Modifier.weight(1f),
                singleLine = true,
            )

            // Auto-scroll toggle
            IconButton(
                onClick = { autoScroll = !autoScroll },
            ) {
                Icon(
                    if (autoScroll) Icons.Filled.Pause else Icons.Filled.PlayArrow,
                    contentDescription = if (autoScroll) stringResource(R.string.logs_autoscroll_pause) else stringResource(R.string.logs_autoscroll_resume),
                    tint = if (autoScroll) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            // Clear logs
            IconButton(
                onClick = { scope.launch { plane.clearLogs() } },
            ) {
                Icon(Icons.Outlined.Delete, contentDescription = stringResource(R.string.logs_clear_desc))
            }
        }

        // Branch Category Filter Chips
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            LogBranchFilter.entries.forEach { branch ->
                FilterChip(
                    selected = selectedBranch == branch,
                    onClick = { selectedBranch = branch },
                    label = { Text(branch.label) },
                )
            }
        }

        // Log Level Filter Chips
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            LogLevelFilter.entries.forEach { level ->
                FilterChip(
                    selected = selectedLevel == level,
                    onClick = { selectedLevel = level },
                    label = { Text(stringResource(level.labelRes)) },
                )
            }
        }

        // Logs terminal view
        Card(
            modifier = Modifier
                .fillMaxSize()
                .weight(1f),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f),
            ),
        ) {
            Text(
                text = filteredLogs.ifBlank { stringResource(R.string.logs_empty) },
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(verticalScrollState)
                    .horizontalScroll(rememberScrollState())
                    .padding(12.dp),
                style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
            )
        }
    }
}

