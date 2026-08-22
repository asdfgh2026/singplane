package app.singplane.model

import org.json.JSONObject

data class AppSettings(
    val corePath: String = "",
    val coreChannel: String = "beta",
    val githubProxy: String = "",
    val mixedPort: Int = 7890,
    val clashApiPort: Int = 9090,
    val clashApiHost: String = "127.0.0.1",
    val activeProfileId: String? = null,
    val forceAppPortsOnAssemble: Boolean = true,
    val stripTunOnAssemble: Boolean = true,
    val defaultAssembleOnImport: Boolean = false,
    val defaultTemplateId: String = "builtin-mixed-direct",
    val seedColorValue: Int = 0xFF047857.toInt(),
    val themeMode: String = "system",
    val language: String = "system",
    val tailscale: TailscaleSettings = TailscaleSettings(),
    val disclaimerAccepted: Boolean = false,
    val autoUpdateIntervalMinutes: Int = 0,
) {
    val clashApiController: String get() = "$clashApiHost:$clashApiPort"

    fun toJson(): JSONObject = JSONObject()
        .put("corePath", corePath)
        .put("coreChannel", coreChannel)
        .put("githubProxy", githubProxy)
        .put("mixedPort", mixedPort)
        .put("clashApiPort", clashApiPort)
        .put("clashApiHost", clashApiHost)
        .put("activeProfileId", activeProfileId)
        .put("forceAppPortsOnAssemble", forceAppPortsOnAssemble)
        .put("stripTunOnAssemble", stripTunOnAssemble)
        .put("defaultAssembleOnImport", defaultAssembleOnImport)
        .put("defaultTemplateId", defaultTemplateId)
        .put("seedColorValue", seedColorValue)
        .put("themeMode", themeMode)
        .put("language", language)
        .put("tailscale", tailscale.toJson())
        .put("disclaimerAccepted", disclaimerAccepted)
        .put("disclaimer_accepted_v1", disclaimerAccepted)
        .put("autoUpdateIntervalMinutes", autoUpdateIntervalMinutes)

    companion object {
        fun fromJson(json: JSONObject): AppSettings = AppSettings(
            corePath = json.optString("corePath", ""),
            coreChannel = json.optString("coreChannel", "beta"),
            githubProxy = json.optString("githubProxy", ""),
            mixedPort = json.optInt("mixedPort", 7890),
            clashApiPort = json.optInt("clashApiPort", 9090),
            clashApiHost = json.optString("clashApiHost", "127.0.0.1"),
            activeProfileId = json.optString("activeProfileId").ifEmpty { null },
            forceAppPortsOnAssemble = json.optBoolean("forceAppPortsOnAssemble", true),
            stripTunOnAssemble = json.optBoolean("stripTunOnAssemble", true),
            defaultAssembleOnImport = json.optBoolean("defaultAssembleOnImport", false),
            defaultTemplateId = json.optString("defaultTemplateId", "builtin-mixed-direct"),
            seedColorValue = json.optInt("seedColorValue", 0xFF047857.toInt()),
            themeMode = json.optString("themeMode", json.optString("theme_mode_v1", "system")),
            language = json.optString("language", "system"),
            tailscale = json.optJSONObject("tailscale")?.let { TailscaleSettings.fromJson(it) } ?: TailscaleSettings(),
            disclaimerAccepted = json.optBoolean("disclaimerAccepted", json.optBoolean("disclaimer_accepted_v1", false)),
            autoUpdateIntervalMinutes = json.optInt("autoUpdateIntervalMinutes", 0),
        )
    }
}

