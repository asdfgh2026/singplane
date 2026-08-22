package app.singplane.ui.pages

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.DeleteSweep
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.singplane.R
import app.singplane.clash.ClashConnection
import app.singplane.clash.ClashConnectionParser
import app.singplane.clash.ConnSortMode
import app.singplane.core.LocalControlPlane
import app.singplane.model.Profile.Companion.fmtBytes
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ConnectionsPage() {
    val plane = LocalControlPlane.current
    val connectionsSnap by plane.connections.collectAsStateWithLifecycle()
    val runningSnap by plane.status.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    var searchQuery by remember { mutableStateOf("") }
    var sortMode by remember { mutableStateOf(ConnSortMode.DEFAULT) }

    // 1-second interval live polling when running
    LaunchedEffect(runningSnap.running) {
        if (runningSnap.running) {
            while (isActive) {
                plane.refreshConnections()
                delay(1000)
            }
        }
    }

    val filtered = ClashConnectionParser.filter(connectionsSnap.connections, searchQuery)
    val sorted = ClashConnectionParser.sort(filtered, sortMode)

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = searchQuery,
                onValueChange = { searchQuery = it },
                placeholder = {
                    Text(
                        stringResource(R.string.connections_search_placeholder),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
                leadingIcon = {
                    Icon(
                        Icons.Filled.Search,
                        contentDescription = stringResource(R.string.connections_search_desc),
                    )
                },
                trailingIcon = if (searchQuery.isNotEmpty()) {
                    {
                        IconButton(onClick = { searchQuery = "" }) {
                            Icon(Icons.Filled.Close, contentDescription = null, modifier = Modifier.size(20.dp))
                        }
                    }
                } else {
                    null
                },
                shape = RoundedCornerShape(12.dp),
                modifier = Modifier.weight(1f),
                singleLine = true,
                maxLines = 1,
            )
            IconButton(
                onClick = { scope.launch { plane.refreshConnections() } },
            ) {
                Icon(
                    Icons.Filled.Refresh,
                    contentDescription = stringResource(R.string.connections_refresh_desc),
                )
            }
        }

        // Summary Header Card
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f),
            ),
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 14.dp, vertical = 10.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        stringResource(R.string.connections_active_count, sorted.size),
                        style = MaterialTheme.typography.titleSmall,
                    )
                    if (connectionsSnap.downloadTotal > 0 || connectionsSnap.uploadTotal > 0) {
                        Text(
                            stringResource(
                                R.string.connections_total_traffic,
                                fmtBytes(connectionsSnap.uploadTotal),
                                fmtBytes(connectionsSnap.downloadTotal),
                            ),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                if (sorted.isNotEmpty()) {
                    OutlinedButton(
                        onClick = { scope.launch { plane.closeAllConnections() } },
                        colors = ButtonDefaults.outlinedButtonColors(
                            contentColor = MaterialTheme.colorScheme.error,
                        ),
                        contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                    ) {
                        Icon(Icons.Filled.DeleteSweep, contentDescription = null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(4.dp))
                        Text(stringResource(R.string.connections_close_all), style = MaterialTheme.typography.labelMedium)
                    }
                }
            }
        }

        // Sort switcher chips
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ConnSortMode.entries.forEach { mode ->
                FilterChip(
                    selected = sortMode == mode,
                    onClick = { sortMode = mode },
                    label = { Text(stringResource(mode.labelRes)) },
                    modifier = Modifier.weight(1f),
                )
            }
        }

        if (!runningSnap.running) {
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
                ),
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        stringResource(R.string.connections_not_running),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        } else if (sorted.isEmpty()) {
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
                ),
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        if (searchQuery.isNotBlank()) stringResource(R.string.connections_no_match) else stringResource(R.string.connections_empty),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }

        LazyColumn(
            verticalArrangement = Arrangement.spacedBy(8.dp),
            contentPadding = PaddingValues(bottom = 24.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            items(sorted, key = { it.id }) { conn ->
                ConnectionCard(
                    conn = conn,
                    onClose = { scope.launch { plane.closeConnection(conn.id) } },
                )
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ConnectionCard(
    conn: ClashConnection,
    onClose: () -> Unit,
) {
    val title = conn.host.ifEmpty { conn.destination }
    val process = ClashConnectionParser.shortProcess(conn.process)
    val rule = ClashConnectionParser.shortRule(conn.rule)
    val chain = conn.chains.joinToString(" → ")
    val hasSpeed = conn.uploadSpeed > 0 || conn.downloadSpeed > 0
    val speedColor = if (hasSpeed) Color(0xFF10B981) else MaterialTheme.colorScheme.onSurfaceVariant

    ElevatedCard(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.elevatedCardColors(containerColor = MaterialTheme.colorScheme.surface),
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        title,
                        style = MaterialTheme.typography.bodyMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (conn.destination.isNotEmpty() && conn.destination != title) {
                        Text(
                            conn.destination,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            fontFamily = FontFamily.Monospace,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                IconButton(
                    onClick = onClose,
                    modifier = Modifier.size(28.dp),
                ) {
                    Icon(
                        Icons.Filled.Close,
                        contentDescription = stringResource(R.string.connections_disconnect_desc),
                        modifier = Modifier.size(16.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                if (conn.network.isNotEmpty()) {
                    Badge(text = conn.network.uppercase())
                }
                if (process.isNotEmpty()) {
                    Badge(text = process, bg = MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.5f))
                }
                if (chain.isNotEmpty()) {
                    Badge(text = chain, bg = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.5f))
                }
            }

            if (rule.isNotEmpty()) {
                Text(
                    rule,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    "↑ ${fmtSpeed(conn.uploadSpeed)}  ↓ ${fmtSpeed(conn.downloadSpeed)}",
                    style = MaterialTheme.typography.labelSmall,
                    color = speedColor,
                    fontFamily = FontFamily.Monospace,
                    maxLines = 1,
                )
                Text(
                    "${fmtBytes(conn.upload)} / ${fmtBytes(conn.download)}",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontFamily = FontFamily.Monospace,
                    maxLines = 1,
                )
            }
        }
    }
}

@Composable
private fun Badge(
    text: String,
    bg: Color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.6f),
) {
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(4.dp))
            .background(bg)
            .padding(horizontal = 6.dp, vertical = 2.dp),
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

private fun fmtSpeed(bytesPerSec: Long): String {
    if (bytesPerSec <= 0) return "0 B/s"
    if (bytesPerSec < 1024) return "$bytesPerSec B/s"
    val kb = bytesPerSec / 1024.0
    if (kb < 1024) return String.format("%.1f KB/s", kb)
    val mb = kb / 1024.0
    return String.format("%.1f MB/s", mb)
}
