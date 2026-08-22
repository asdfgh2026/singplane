package app.singplane.model

import org.json.JSONObject

data class Template(
    val id: String,
    val name: String,
    val description: String = "",
    val builtin: Boolean = false,
    val content: String = "",
) {
    fun toJson(): JSONObject = JSONObject()
        .put("id", id)
        .put("name", name)
        .put("description", description)
        .put("builtin", builtin)
        .put("content", content)

    companion object {
        fun fromJson(json: JSONObject): Template = Template(
            id = json.optString("id"),
            name = json.optString("name"),
            description = json.optString("description", ""),
            builtin = json.optBoolean("builtin", false),
            content = json.optString("content", ""),
        )
    }
}
