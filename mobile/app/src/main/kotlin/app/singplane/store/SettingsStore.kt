package app.singplane.store

import app.singplane.model.AppSettings
import org.json.JSONObject
import java.io.File

class SettingsStore(private val file: File) {
    fun load(): AppSettings {
        if (!file.exists() || file.length() == 0L) return AppSettings()
        return runCatching { AppSettings.fromJson(JSONObject(file.readText())) }
            .getOrDefault(AppSettings())
    }

    fun save(settings: AppSettings) {
        file.writeAtomically(settings.toJson().toString())
    }
}
