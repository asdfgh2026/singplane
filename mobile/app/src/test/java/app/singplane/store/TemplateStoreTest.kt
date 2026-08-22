package app.singplane.store

import com.google.common.truth.Truth.assertThat
import app.singplane.model.Template
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class TemplateStoreTest {
    @get:Rule
    val tmp = TemporaryFolder()

    private val directJson = """{"inbounds":[{"type":"mixed","listen_port":7890}],"outbounds":[{"type":"direct","tag":"direct"}]}"""
    private val ruleJson = """{"inbounds":[{"type":"mixed","listen_port":7890}],"outbounds":[{"type":"direct","tag":"direct"}],"route":{"rules":[]}}"""

    private fun store(dir: File = tmp.newFolder("templates")): TemplateStore {
        return TemplateStore(
            templatesDir = dir,
            builtinReader = { id ->
                when (id) {
                    "builtin-mixed-direct" -> directJson
                    "builtin-mixed-rule" -> ruleJson
                    else -> null
                }
            },
        )
    }

    @Test
    fun loadBuiltinTemplates() {
        val s = store()
        val all = s.loadAll()
        assertThat(all.map { it.id }).containsAtLeast("builtin-mixed-direct", "builtin-mixed-rule")
        val direct = all.first { it.id == "builtin-mixed-direct" }
        assertThat(direct.builtin).isTrue()
        assertThat(direct.content).contains("mixed")
    }

    @Test
    fun saveBuiltinTemplateThrows() {
        val s = store()
        val direct = s.loadAll().first { it.id == "builtin-mixed-direct" }
        val err = runCatching { s.save(direct.copy(name = "new name")) }.exceptionOrNull()
        assertThat(err).isNotNull()
        assertThat(err?.message).contains("内置模板只读")
    }

    @Test
    fun deleteBuiltinTemplateThrows() {
        val s = store()
        val err = runCatching { s.delete("builtin-mixed-direct") }.exceptionOrNull()
        assertThat(err).isNotNull()
        assertThat(err?.message).contains("内置模板不能删除")
    }

    @Test
    fun saveAndLoadUserTemplate() {
        val dir = tmp.newFolder("templates_user")
        val s1 = store(dir)
        val custom = Template(
            id = "custom-1",
            name = "我的分流模板",
            description = "自定义",
            builtin = false,
            content = """{"outbounds":[]}""",
        )
        s1.save(custom)

        val s2 = store(dir)
        val loaded = s2.loadAll()
        val found = loaded.firstOrNull { it.id == "custom-1" }
        assertThat(found).isNotNull()
        assertThat(found?.name).isEqualTo("我的分流模板")
        assertThat(found?.builtin).isFalse()
    }

    @Test
    fun deleteUserTemplate() {
        val dir = tmp.newFolder("templates_del")
        val s = store(dir)
        val custom = Template(id = "c1", name = "C1", content = "{}")
        s.save(custom)
        assertThat(s.loadAll().any { it.id == "c1" }).isTrue()

        s.delete("c1")
        assertThat(s.loadAll().any { it.id == "c1" }).isFalse()
    }

    @Test(expected = IllegalArgumentException::class)
    fun saveRejectsPathTraversal() {
        val s = store()
        s.save(Template(id = "../../bad_template", name = "bad", content = "{}"))
    }

    @Test(expected = IllegalArgumentException::class)
    fun deleteRejectsPathTraversal() {
        val s = store()
        s.delete("../bad_template")
    }
}
