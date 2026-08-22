package app.singplane.store

import app.singplane.model.Profile
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

class ProfileStore(private val dir: File) {
    private val indexFile get() = File(dir, "index.json")

    fun loadAll(): List<Profile> {
        if (!dir.exists()) return emptyList()
        val byId = dir.listFiles { f -> f.isFile && f.name.endsWith(".json") && f.name != "index.json" }
            ?.mapNotNull { file ->
                runCatching { Profile.fromJson(JSONObject(file.readText())) }.getOrNull()
            }
            ?.associateBy { it.id }
            .orEmpty()
            .toMutableMap()

        val order = loadIndex()
        if (order.isEmpty()) return byId.values.sortedBy { it.name }

        val result = ArrayList<Profile>(byId.size)
        for (id in order) {
            val p = byId.remove(id) ?: continue
            result.add(p)
        }
        result.addAll(byId.values)
        return result
    }

    fun upsert(profile: Profile) {
        validateId(profile.id)
        dir.mkdirs()
        File(dir, "${profile.id}.json").writeAtomically(profile.toJson().toString())
        val order = loadIndex().toMutableList()
        if (!order.contains(profile.id)) order.add(profile.id)
        saveIndex(order)
    }

    fun delete(id: String) {
        validateId(id)
        File(dir, "$id.json").delete()
        saveIndex(loadIndex().filterNot { it == id })
    }

    companion object {
        fun validateId(id: String) {
            require(id.isNotBlank()) { "id 不能为空" }
            require(!id.contains('/') && !id.contains('\\') && !id.contains("..")) { "id 包含非法路径字符" }
            require(id.all { it in 'a'..'z' || it in 'A'..'Z' || it in '0'..'9' || it == '-' || it == '_' }) { "id 仅允许字母、数字、下划线及中划线" }
        }
    }

    private fun loadIndex(): List<String> {
        if (!indexFile.exists()) return emptyList()
        return runCatching {
            val arr = JSONObject(indexFile.readText()).optJSONArray("ids") ?: JSONArray()
            (0 until arr.length()).map { arr.getString(it) }
        }.getOrDefault(emptyList())
    }

    private fun saveIndex(ids: List<String>) {
        val arr = JSONArray()
        ids.forEach { arr.put(it) }
        indexFile.writeAtomically(JSONObject().put("ids", arr).toString())
    }
}
