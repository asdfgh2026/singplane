package app.singplane

import android.content.res.Configuration
import android.os.Bundle
import android.os.LocaleList
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import app.singplane.core.LocalControlPlane
import app.singplane.ui.AppShell
import app.singplane.ui.dialogs.DisclaimerDialog
import app.singplane.ui.theme.SingPanelTheme
import kotlinx.coroutines.launch
import java.util.Locale

class MainActivity : ComponentActivity() {
    private val vpnConsent = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        val plane = (application as SingPanelApp).controlPlane
        if (result.resultCode == RESULT_OK) {
            lifecycleScope.launch {
                runCatching { plane.start() }
            }
        } else {
            plane.onVpnConsentRejected()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val plane = (application as SingPanelApp).controlPlane
        enableEdgeToEdge()
        setContent {
            val settings by plane.settings.collectAsStateWithLifecycle()
            val locale = remember(settings.language) {
                when (settings.language) {
                    "zh-Hans" -> Locale.SIMPLIFIED_CHINESE
                    "zh-Hant" -> Locale.TRADITIONAL_CHINESE
                    "en" -> Locale.ENGLISH
                    else -> Locale.getDefault()
                }
            }

            val baseContext = LocalContext.current
            val baseConfig = LocalConfiguration.current
            val localizedContext = remember(locale, baseContext) {
                val config = Configuration(baseContext.resources.configuration)
                config.setLocale(locale)
                config.setLocales(LocaleList(locale))
                baseContext.createConfigurationContext(config)
            }
            val localizedConfig = remember(locale, baseConfig) {
                Configuration(baseConfig).apply {
                    setLocale(locale)
                    setLocales(LocaleList(locale))
                }
            }

            SingPanelTheme(themeMode = settings.themeMode) {
                CompositionLocalProvider(
                    LocalControlPlane provides plane,
                    LocalContext provides localizedContext,
                    LocalConfiguration provides localizedConfig,
                ) {
                    AppShell(onNeedVpnConsent = { vpnConsent.launch(it.consentIntent) })
                    if (!settings.disclaimerAccepted) {
                        DisclaimerDialog(
                            onAccept = {
                                lifecycleScope.launch {
                                    plane.updateSettings(settings.copy(disclaimerAccepted = true))
                                }
                            },
                        )
                    }
                }
            }
        }
    }
}

