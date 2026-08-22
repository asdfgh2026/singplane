package app.singplane.store

import app.singplane.model.Template
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

class TemplateStore(
    private val templatesDir: File,
    private val builtinReader: (String) -> String? = { null },
) {
    companion object {
        val BUILTIN_META = listOf(
            Triple(
                "builtin-mixed-direct",
                "Mixed 直连基础模板",
                "mixed 127.0.0.1:7890，无 TUN，Clash API :9090；节点由模板注入。",
            ),
            Triple(
                "builtin-mixed-rule",
                "Mixed + 基础分流",
                "在直连模板上增加私有 IP / 本地域名直连规则，无远程 ruleset。",
            ),
        )
    }

    private val indexFile: File get() = File(templatesDir, "index.json")

    fun loadBuiltin(): List<Template> {
        return BUILTIN_META.map { (id, name, desc) ->
            val content = builtinReader(id) ?: ""
            Template(
                id = id,
                name = name,
                description = desc,
                builtin = true,
                content = content,
            )
        }
    }

    fun loadUser(): List<Template> {
        if (!templatesDir.exists()) return emptyList()
        val ids = loadIndex()
        val byId = mutableMapOf<String, Template>()
        templatesDir.listFiles { f -> f.isFile && f.extension == "json" && f.name != "index.json" }?.forEach { f ->
            runCatching {
                val json = JSONObject(f.readText())
                val t = Template.fromJson(json)
                if (!t.builtin && !t.id.startsWith("builtin-")) {
                    byId[t.id] = t
                }
            }
        }
        val ordered = mutableListOf<Template>()
        for (id in ids) {
            byId.remove(id)?.let { ordered.add(it) }
        }
        ordered.addAll(byId.values)
        return ordered
    }

    fun loadAll(): List<Template> {
        return loadBuiltin() + loadUser()
    }

    fun findById(id: String): Template? {
        return loadAll().firstOrNull { it.id == id }
    }

    fun save(template: Template) {
        if (template.builtin || template.id.startsWith("builtin-")) {
            error("内置模板只读，无法修改")
        }
        ProfileStore.validateId(template.id)
        templatesDir.mkdirs()
        val file = File(templatesDir, "${template.id}.json")
        file.writeAtomically(template.toJson().toString(2))

        val ids = loadIndex().toMutableList()
        if (!ids.contains(template.id)) {
            ids.add(template.id)
            saveIndex(ids)
        }
    }

    fun delete(id: String) {
        if (id.startsWith("builtin-")) {
            error("内置模板不能删除")
        }
        ProfileStore.validateId(id)
        val file = File(templatesDir, "$id.json")
        if (file.exists()) {
            file.delete()
        }
        val ids = loadIndex().filter { it != id }
        saveIndex(ids)
    }

    private fun loadIndex(): List<String> {
        if (!indexFile.exists()) return emptyList()
        return runCatching {
            val arr = JSONArray(indexFile.readText())
            (0 until arr.length()).mapNotNull { arr.optString(it).takeIf { s -> s.isNotEmpty() } }
        }.getOrDefault(emptyList())
    }

    private fun saveIndex(ids: List<String>) {
        val arr = JSONArray()
        ids.forEach { arr.put(it) }
        indexFile.writeAtomically(arr.toString())
    }
}
