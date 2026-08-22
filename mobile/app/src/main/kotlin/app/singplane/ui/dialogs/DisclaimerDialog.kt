package app.singplane.ui.dialogs

import android.app.Activity
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.window.DialogProperties
import app.singplane.R

const val DISCLAIMER_TEXT = "本软件为开源免费软件，仅供学习交流等非商业性质的个人测试使用，代理服务商的行为均与本软件无关，同意声明代表您已完全知晓并确认了这一点，如不同意，请选择退出！"

@Composable
fun DisclaimerDialog(
    onAccept: () -> Unit,
    onDismiss: (() -> Unit)? = null,
) {
    val context = LocalContext.current
    val isMandatory = onDismiss == null

    AlertDialog(
        onDismissRequest = { onDismiss?.invoke() },
        properties = DialogProperties(dismissOnBackPress = !isMandatory, dismissOnClickOutside = !isMandatory),
        title = {
            Text(stringResource(R.string.disclaimer_title), style = MaterialTheme.typography.titleMedium)
        },
        text = {
            Text(stringResource(R.string.disclaimer_text), style = MaterialTheme.typography.bodyMedium)
        },
        confirmButton = {
            Button(onClick = {
                onAccept()
                onDismiss?.invoke()
            }) {
                Text(stringResource(R.string.disclaimer_agree))
            }
        },
        dismissButton = {
            OutlinedButton(
                onClick = {
                    if (onDismiss != null) {
                        onDismiss()
                    } else {
                        (context as? Activity)?.finishAffinity()
                    }
                },
            ) {
                Text(if (isMandatory) stringResource(R.string.disclaimer_exit) else stringResource(R.string.templates_action_close))
            }
        },
    )
}
