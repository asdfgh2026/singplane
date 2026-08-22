package app.singplane.ui.pages

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Speed
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.singplane.clash.ClashApiParser
import app.singplane.clash.GroupWithNodes
import app.singplane.clash.ProxyNode
import app.singplane.clash.SortMode
import app.singplane.core.LocalControlPlane
import kotlinx.coroutines.launch
import androidx.compose.ui.res.stringResource
import app.singplane.R

@Composable
fun ProxiesPage() {
    val plane = LocalControlPlane.current
    val groupsWithNodes by plane.groupsWithNodes.collectAsStateWithLifecycle()
    val running by plane.status.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    var searchQuery by remember { mutableStateOf("") }
    var sortMode by remember { mutableStateOf(SortMode.DEFAULT) }
    val testingMap = remember { mutableStateMapOf<String, Boolean>() }

    LaunchedEffect(running.running) {
        if (running.running) {
            runCatching { plane.refreshProxies() }
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // Search bar & Refresh
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = searchQuery,
                onValueChange = { searchQuery = it },
                placeholder = { Text(stringResource(R.string.proxies_search_placeholder)) },
                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = stringResource(R.string.proxies_search_desc)) },
                modifier = Modifier.weight(1f),
                singleLine = true,
            )
            IconButton(
                onClick = { scope.launch { runCatching { plane.refreshProxies() } } },
            ) {
                Icon(Icons.Filled.Refresh, contentDescription = stringResource(R.string.proxies_refresh_desc))
            }
        }

        // Sort switcher chips
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            SortMode.entries.forEach { mode ->
                FilterChip(
                    selected = sortMode == mode,
                    onClick = { sortMode = mode },
                    label = { Text(stringResource(mode.labelRes)) },
                )
            }
        }

        if (!running.running) {
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
                ),
            ) {
                Text(
                    stringResource(R.string.proxies_core_not_running),
                    modifier = Modifier.padding(16.dp),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        } else if (groupsWithNodes.isEmpty()) {
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
                ),
            ) {
                Text(
                    stringResource(R.string.proxies_no_groups),
                    modifier = Modifier.padding(16.dp),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        LazyColumn(
            verticalArrangement = Arrangement.spacedBy(16.dp),
            contentPadding = PaddingValues(bottom = 24.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            items(groupsWithNodes, key = { it.group.name }) { gn ->
                ProxyGroupCard(
                    groupWithNodes = gn,
                    searchQuery = searchQuery,
                    sortMode = sortMode,
                    isTestingGroup = testingMap[gn.group.name] == true,
                    onTestAll = {
                        scope.launch {
                            testingMap[gn.group.name] = true
                            plane.testAllDelays(gn.group.name)
                            testingMap[gn.group.name] = false
                        }
                    },
                    onSelectNode = { nodeName ->
                        if (gn.group.selectable) {
                            scope.launch { runCatching { plane.selectProxy(gn.group.name, nodeName) } }
                        }
                    },
                    onTestNode = { nodeName ->
                        scope.launch {
                            plane.testProxyDelay(gn.group.name, nodeName)
                        }
                    },
                )
            }
        }
    }
}

@Composable
private fun ProxyGroupCard(
    groupWithNodes: GroupWithNodes,
    searchQuery: String,
    sortMode: SortMode,
    isTestingGroup: Boolean,
    onTestAll: () -> Unit,
    onSelectNode: (String) -> Unit,
    onTestNode: (String) -> Unit,
) {
    val group = groupWithNodes.group
    val visible = ClashApiParser.visibleNodes(groupWithNodes.nodes)
    val filtered = ClashApiParser.filterNodes(visible, searchQuery)
    val sorted = ClashApiParser.sortNodes(filtered, sortMode)

    ElevatedCard(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.elevatedCardColors(
            containerColor = MaterialTheme.colorScheme.surface,
        ),
    ) {
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            // Group Header
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column {
                    Text(group.name, style = MaterialTheme.typography.titleMedium)
                    Spacer(Modifier.height(2.dp))
                    Text(
                        stringResource(R.string.proxies_group_node_count, group.type, sorted.size),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

                FilledTonalButton(
                    onClick = onTestAll,
                    enabled = !isTestingGroup,
                ) {
                    if (isTestingGroup) {
                        CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                        Spacer(Modifier.width(6.dp))
                        Text(stringResource(R.string.proxies_speed_testing))
                    } else {
                        Icon(Icons.Filled.Speed, contentDescription = null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(stringResource(R.string.proxies_speed_test_all))
                    }
                }
            }

            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                sorted.forEach { node ->
                    ProxyNodeRow(
                        node = node,
                        isSelected = node.name == group.now,
                        onClick = { onSelectNode(node.name) },
                        onTest = { onTestNode(node.name) },
                    )
                }
            }
        }
    }
}

@Composable
private fun ProxyNodeRow(
    node: ProxyNode,
    isSelected: Boolean,
    onClick: () -> Unit,
    onTest: () -> Unit,
) {
    val primaryColor = MaterialTheme.colorScheme.primary
    val bgColor = if (isSelected) {
        MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.45f)
    } else {
        Color.Transparent
    }
    val delayText = when {
        node.delayMs == null -> "—"
        node.delayMs <= 0 -> stringResource(R.string.proxies_timeout)
        else -> "${node.delayMs} ms"
    }
    val delayColor = when {
        node.delayMs == null -> MaterialTheme.colorScheme.onSurfaceVariant
        node.delayMs <= 0 -> MaterialTheme.colorScheme.onSurfaceVariant
        node.delayMs < 200 -> Color(0xFF10B981)
        node.delayMs < 600 -> Color(0xFFF59E0B)
        else -> Color(0xFFEF4444)
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(bgColor)
            .clickable { onClick() }
            .padding(horizontal = 10.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (isSelected) {
            Icon(
                Icons.Filled.Check,
                contentDescription = stringResource(R.string.proxies_current_selected),
                tint = primaryColor,
                modifier = Modifier.size(16.dp),
            )
        } else {
            Spacer(Modifier.size(16.dp))
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                node.name,
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (node.type.isNotBlank()) {
                Text(
                    node.type,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Text(
            delayText,
            style = MaterialTheme.typography.labelMedium,
            color = delayColor,
            fontFamily = FontFamily.Monospace,
            modifier = Modifier
                .clip(RoundedCornerShape(4.dp))
                .clickable { onTest() }
                .padding(horizontal = 6.dp, vertical = 4.dp),
        )
    }
}
