package app.singplane.ui.pages

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
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
import app.singplane.SingPanelApp
import app.singplane.model.Template
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.util.UUID
import androidx.compose.ui.res.stringResource
import app.singplane.R

@Composable
fun TemplatesPage() {
    val context = LocalContext.current
    val app = context.applicationContext as SingPanelApp
    val plane = app.controlPlane
    val templates by plane.templates.collectAsState()
    val scope = rememberCoroutineScope()

    var editingTemplate by remember { mutableStateOf<Template?>(null) }
    var isNewTemplate by remember { mutableStateOf(false) }

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(
                onClick = {
                    editingTemplate = Template(
                        id = "user-${UUID.randomUUID().toString().take(8)}",
                        name = context.getString(R.string.templates_custom_name),
                        description = context.getString(R.string.templates_custom_desc),
                        builtin = false,
                        content = "{\n  \"inbounds\": [\n    {\n      \"type\": \"mixed\",\n      \"listen\": \"127.0.0.1\",\n      \"listen_port\": 7890\n    }\n  ],\n  \"outbounds\": [\n    {\n      \"type\": \"direct\",\n      \"tag\": \"direct\"\n    }\n  ]\n}",
                    )
                    isNewTemplate = true
                },
            ) {
                Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.templates_new_desc))
            }
        },
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            items(templates, key = { it.id }) { tpl ->
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
                    ),
                ) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                text = tpl.name,
                                style = MaterialTheme.typography.titleMedium,
                            )
                            AssistChip(
                                onClick = {},
                                label = {
                                    Text(
                                        if (tpl.builtin) stringResource(R.string.templates_type_builtin) else stringResource(R.string.templates_type_custom),
                                        style = MaterialTheme.typography.labelSmall,
                                    )
                                },
                            )
                        }

                        if (tpl.description.isNotBlank()) {
                            Spacer(Modifier.height(4.dp))
                            Text(
                                text = tpl.description,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }

                        Spacer(Modifier.height(12.dp))
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.End,
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            OutlinedButton(
                                onClick = {
                                    editingTemplate = tpl
                                    isNewTemplate = false
                                },
                            ) {
                                Icon(
                                    if (tpl.builtin) Icons.Outlined.Info else Icons.Outlined.Edit,
                                    contentDescription = null,
                                )
                                Spacer(Modifier.width(4.dp))
                                Text(if (tpl.builtin) stringResource(R.string.templates_action_view_json) else stringResource(R.string.templates_action_edit))
                            }

                            if (!tpl.builtin) {
                                Spacer(Modifier.width(8.dp))
                                IconButton(
                                    onClick = {
                                        scope.launch { plane.deleteTemplate(tpl.id) }
                                    },
                                ) {
                                    Icon(
                                        Icons.Outlined.Delete,
                                        contentDescription = stringResource(R.string.templates_action_delete_desc),
                                        tint = MaterialTheme.colorScheme.error,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    editingTemplate?.let { tpl ->
        var name by remember { mutableStateOf(tpl.name) }
        var desc by remember { mutableStateOf(tpl.description) }
        var content by remember { mutableStateOf(tpl.content) }
        var errorMsg by remember { mutableStateOf<String?>(null) }

        AlertDialog(
            onDismissRequest = { editingTemplate = null },
            title = {
                Text(
                    if (tpl.builtin) stringResource(R.string.templates_title_view, tpl.name)
                    else if (isNewTemplate) stringResource(R.string.templates_new_desc)
                    else stringResource(R.string.templates_title_edit),
                )
            },
            text = {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    if (!tpl.builtin) {
                        OutlinedTextField(
                            value = name,
                            onValueChange = { name = it },
                            label = { Text(stringResource(R.string.templates_field_name)) },
                            modifier = Modifier.fillMaxWidth(),
                        )
                        OutlinedTextField(
                            value = desc,
                            onValueChange = { desc = it },
                            label = { Text(stringResource(R.string.templates_field_desc)) },
                            modifier = Modifier.fillMaxWidth(),
                        )
                    }

                    OutlinedTextField(
                        value = content,
                        onValueChange = {
                            if (!tpl.builtin) {
                                content = it
                                errorMsg = null
                            }
                        },
                        readOnly = tpl.builtin,
                        label = { Text(stringResource(R.string.templates_field_json)) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(280.dp),
                        textStyle = MaterialTheme.typography.bodySmall.copy(
                            fontFamily = FontFamily.Monospace,
                            fontSize = 12.sp,
                        ),
                    )

                    errorMsg?.let {
                        Text(
                            text = it,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
            },
            confirmButton = {
                if (!tpl.builtin) {
                    Button(
                        onClick = {
                            try {
                                JSONObject(content)
                                scope.launch {
                                    plane.saveTemplate(
                                        tpl.copy(
                                            name = name.ifBlank { context.getString(R.string.templates_unnamed) },
                                            description = desc,
                                            content = content,
                                        ),
                                    )
                                    editingTemplate = null
                                }
                            } catch (e: Exception) {
                                errorMsg = context.getString(R.string.templates_json_invalid, e.message ?: "")
                            }
                        },
                    ) {
                        Text(stringResource(R.string.templates_action_save))
                    }
                } else {
                    Button(onClick = { editingTemplate = null }) {
                        Text(stringResource(R.string.templates_action_close))
                    }
                }
            },
            dismissButton = {
                if (!tpl.builtin) {
                    TextButton(onClick = { editingTemplate = null }) {
                        Text(stringResource(R.string.templates_action_cancel))
                    }
                }
            },
        )
    }
}
