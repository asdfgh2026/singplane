package app.singplane.ui.pages

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.singplane.model.Profile
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.singplane.SingPanelApp
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import androidx.compose.ui.res.stringResource
import app.singplane.R

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun ProfilesPage() {
    val app = LocalContext.current.applicationContext as SingPanelApp
    val plane = app.controlPlane
    val profiles by plane.profiles.collectAsStateWithLifecycle()
    val settings by plane.settings.collectAsStateWithLifecycle()
    val templates by plane.templates.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    var showImportUrl by remember { mutableStateOf(false) }
    var showImportLocal by remember { mutableStateOf(false) }
    var viewing by remember { mutableStateOf<Profile?>(null) }
    var err by remember { mutableStateOf<String?>(null) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Button(
                onClick = { showImportUrl = true },
                modifier = Modifier.weight(1f),
            ) {
                Text(stringResource(R.string.profiles_import_url))
            }
            OutlinedButton(
                onClick = { showImportLocal = true },
                modifier = Modifier.weight(1f),
            ) {
                Text(stringResource(R.string.profiles_paste_json))
            }
        }

        if (profiles.isEmpty()) {
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.3f),
                ),
            ) {
                Column(modifier = Modifier.padding(24.dp)) {
                    Text(
                        stringResource(R.string.profiles_empty_title),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Spacer(Modifier.height(4.dp))
                    Text(
                        stringResource(R.string.profiles_empty_desc),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }

        LazyColumn(
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.weight(1f),
        ) {
            items(profiles, key = { it.id }) { p ->
                val active = p.id == settings.activeProfileId
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(
                        containerColor = if (active) {
                            MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.35f)
                        } else {
                            MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
                        },
                    ),
                ) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                text = p.name,
                                style = MaterialTheme.typography.titleMedium,
                            )
                            if (active) {
                                AssistChip(
                                    onClick = {},
                                    label = { Text(stringResource(R.string.profiles_tag_current)) },
                                    leadingIcon = {
                                        Icon(Icons.Outlined.Check, contentDescription = null)
                                    },
                                )
                            }
                        }

                        Spacer(Modifier.height(4.dp))
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            AssistChip(
                                onClick = {},
                                label = { Text(if (p.sourceType == "url") stringResource(R.string.profiles_tag_subscription) else stringResource(R.string.profiles_tag_local)) },
                            )
                            if (p.assembleEnabled) {
                                AssistChip(
                                    onClick = {},
                                    label = { Text(stringResource(R.string.profiles_tag_assembled)) },
                                )
                            }
                            AssistChip(
                                onClick = {},
                                label = { Text(if (p.runnable) stringResource(R.string.profiles_tag_runnable) else stringResource(R.string.profiles_tag_unrunnable)) },
                            )
                        }

                        if (p.trafficLabel.isNotEmpty() || p.expireMs > 0) {
                            Spacer(Modifier.height(6.dp))
                            val details = buildList {
                                if (p.trafficLabel.isNotEmpty()) add(stringResource(R.string.profiles_traffic, p.trafficLabel))
                                if (p.expireMs > 0) {
                                    val dt = Instant.ofEpochMilli(p.expireMs)
                                        .atZone(ZoneId.systemDefault())
                                        .format(DateTimeFormatter.ofPattern("yyyy-MM-dd"))
                                    add(stringResource(R.string.profiles_expiry, dt))
                                }
                            }.joinToString(" · ")
                            Text(
                                text = details,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }

                        Spacer(Modifier.height(12.dp))
                        FlowRow(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            if (!active) {
                                FilledTonalButton(
                                    onClick = { scope.launch { plane.setActiveProfile(p.id) } },
                                ) {
                                    Text(stringResource(R.string.profiles_action_select))
                                }
                            }

                            OutlinedButton(onClick = { viewing = p }) {
                                Text(stringResource(R.string.profiles_action_view))
                            }

                            if (p.sourceType == "url" && p.url != null) {
                                OutlinedButton(
                                    onClick = {
                                        scope.launch {
                                            runCatching { plane.refreshProfile(p.id) }
                                                .onFailure { err = it.message }
                                        }
                                    },
                                ) {
                                    Icon(Icons.Outlined.Refresh, contentDescription = null)
                                    Spacer(Modifier.width(4.dp))
                                    Text(stringResource(R.string.proxies_refresh_desc))
                                }
                            }

                            IconButton(
                                onClick = { scope.launch { plane.deleteProfile(p.id) } },
                            ) {
                                Icon(
                                    Icons.Outlined.Delete,
                                    contentDescription = stringResource(R.string.profiles_action_delete_desc),
                                    tint = MaterialTheme.colorScheme.error,
                                )
                            }
                        }
                    }
                }
            }
        }

        err?.let {
            Text(
                text = it,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
            )
        }
    }

    viewing?.let { profile ->
        val pretty = remember(profile.id, profile.content) { Profile.prettyContent(profile.content) }
        val display = pretty.ifEmpty { stringResource(R.string.profiles_view_empty) }
        AlertDialog(
            onDismissRequest = { viewing = null },
            title = { Text(stringResource(R.string.profiles_view_title, profile.name)) },
            text = {
                OutlinedTextField(
                    value = display,
                    onValueChange = {},
                    readOnly = true,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(320.dp),
                    textStyle = MaterialTheme.typography.bodySmall.copy(
                        fontFamily = FontFamily.Monospace,
                        fontSize = 12.sp,
                    ),
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        val cm = app.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                        cm.setPrimaryClip(ClipData.newPlainText("sing-box", pretty.ifEmpty { display }))
                    },
                ) {
                    Text(stringResource(R.string.profiles_action_copy))
                }
            },
            dismissButton = {
                TextButton(onClick = { viewing = null }) {
                    Text(stringResource(R.string.profiles_action_cancel))
                }
            },
        )
    }

    if (showImportUrl) {
        var name by remember { mutableStateOf("") }
        var url by remember { mutableStateOf("") }
        var assemble by remember { mutableStateOf(settings.defaultAssembleOnImport) }
        var selectedTemplateId by remember {
            mutableStateOf(settings.defaultTemplateId.ifEmpty { "builtin-mixed-direct" })
        }
        var dropdownExpanded by remember { mutableStateOf(false) }

        AlertDialog(
            onDismissRequest = { showImportUrl = false },
            title = { Text(stringResource(R.string.profiles_import_url)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    OutlinedTextField(
                        value = name,
                        onValueChange = { name = it },
                        label = { Text(stringResource(R.string.profiles_field_name_optional)) },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    OutlinedTextField(
                        value = url,
                        onValueChange = { url = it },
                        label = { Text(stringResource(R.string.profiles_field_sub_url)) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(stringResource(R.string.profiles_assemble_with_template), style = MaterialTheme.typography.bodyMedium)
                        Switch(checked = assemble, onCheckedChange = { assemble = it })
                    }

                    if (assemble) {
                        ExposedDropdownMenuBox(
                            expanded = dropdownExpanded,
                            onExpandedChange = { dropdownExpanded = it },
                        ) {
                            val curName = templates.firstOrNull { it.id == selectedTemplateId }?.name ?: selectedTemplateId
                            OutlinedTextField(
                                value = curName,
                                onValueChange = {},
                                readOnly = true,
                                label = { Text(stringResource(R.string.profiles_field_assemble_template)) },
                                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = dropdownExpanded) },
                                modifier = Modifier
                                    .menuAnchor()
                                    .fillMaxWidth(),
                            )
                            ExposedDropdownMenu(
                                expanded = dropdownExpanded,
                                onDismissRequest = { dropdownExpanded = false },
                            ) {
                                templates.forEach { t ->
                                    DropdownMenuItem(
                                        text = { Text(t.name) },
                                        onClick = {
                                            selectedTemplateId = t.id
                                            dropdownExpanded = false
                                        },
                                    )
                                }
                            }
                        }
                    }
                }
            },
            confirmButton = {
                Button(
                    onClick = {
                        val trimmedUrl = url.trim()
                        if (trimmedUrl.isNotEmpty()) {
                            showImportUrl = false
                            scope.launch {
                                runCatching {
                                    plane.importUrl(
                                        url = trimmedUrl,
                                        name = name.trim(),
                                        assembleEnabled = assemble,
                                        templateId = if (assemble) selectedTemplateId else null,
                                    )
                                }.onFailure { err = it.message }
                            }
                        }
                    },
                ) {
                    Text(stringResource(R.string.profiles_action_import))
                }
            },
            dismissButton = {
                TextButton(onClick = { showImportUrl = false }) { Text(stringResource(R.string.templates_action_cancel)) }
            },
        )
    }

    if (showImportLocal) {
        var name by remember { mutableStateOf("") }
        var body by remember { mutableStateOf("") }
        var assemble by remember { mutableStateOf(settings.defaultAssembleOnImport) }
        var selectedTemplateId by remember {
            mutableStateOf(settings.defaultTemplateId.ifEmpty { "builtin-mixed-direct" })
        }
        var dropdownExpanded by remember { mutableStateOf(false) }

        AlertDialog(
            onDismissRequest = { showImportLocal = false },
            title = { Text(stringResource(R.string.profiles_title_paste_json)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    OutlinedTextField(
                        value = name,
                        onValueChange = { name = it },
                        label = { Text(stringResource(R.string.profiles_field_name)) },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    OutlinedTextField(
                        value = body,
                        onValueChange = { body = it },
                        label = { Text(stringResource(R.string.profiles_field_json_content)) },
                        minLines = 4,
                        maxLines = 8,
                        modifier = Modifier.fillMaxWidth(),
                    )

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(stringResource(R.string.profiles_assemble_with_template), style = MaterialTheme.typography.bodyMedium)
                        Switch(checked = assemble, onCheckedChange = { assemble = it })
                    }

                    if (assemble) {
                        ExposedDropdownMenuBox(
                            expanded = dropdownExpanded,
                            onExpandedChange = { dropdownExpanded = it },
                        ) {
                            val curName = templates.firstOrNull { it.id == selectedTemplateId }?.name ?: selectedTemplateId
                            OutlinedTextField(
                                value = curName,
                                onValueChange = {},
                                readOnly = true,
                                label = { Text(stringResource(R.string.profiles_field_assemble_template)) },
                                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = dropdownExpanded) },
                                modifier = Modifier
                                    .menuAnchor()
                                    .fillMaxWidth(),
                            )
                            ExposedDropdownMenu(
                                expanded = dropdownExpanded,
                                onDismissRequest = { dropdownExpanded = false },
                            ) {
                                templates.forEach { t ->
                                    DropdownMenuItem(
                                        text = { Text(t.name) },
                                        onClick = {
                                            selectedTemplateId = t.id
                                            dropdownExpanded = false
                                        },
                                    )
                                }
                            }
                        }
                    }
                }
            },
            confirmButton = {
                Button(
                    onClick = {
                        val trimmedBody = body.trim()
                        if (trimmedBody.isNotEmpty()) {
                            showImportLocal = false
                            scope.launch {
                                runCatching {
                                    plane.importLocal(
                                        name = name.trim(),
                                        content = trimmedBody,
                                        assembleEnabled = assemble,
                                        templateId = if (assemble) selectedTemplateId else null,
                                    )
                                }.onFailure { err = it.message }
                            }
                        }
                    },
                ) {
                    Text(stringResource(R.string.profiles_action_import))
                }
            },
            dismissButton = {
                TextButton(onClick = { showImportLocal = false }) { Text(stringResource(R.string.templates_action_cancel)) }
            },
        )
    }
}
