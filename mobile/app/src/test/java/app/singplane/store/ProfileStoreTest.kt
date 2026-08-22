package app.singplane.store

import com.google.common.truth.Truth.assertThat
import app.singplane.model.Profile
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class ProfileStoreTest {
    @get:Rule
    val tmp = TemporaryFolder()

    private fun store() = ProfileStore(tmp.newFolder("profiles"))

    @Test
    fun emptyOnFreshDir() {
        assertThat(store().loadAll()).isEmpty()
    }

    @Test
    fun saveLoadDeletePreservesOrder() {
        val s = store()
        val a = Profile(id = "a", name = "Alpha", content = "{}")
        val b = Profile(id = "b", name = "Beta", content = "{}")
        s.upsert(a)
        s.upsert(b)
        assertThat(s.loadAll().map { it.id }).containsExactly("a", "b").inOrder()
        s.delete("a")
        assertThat(s.loadAll().map { it.id }).containsExactly("b")
    }

    @Test
    fun upsertReplacesSameId() {
        val s = store()
        s.upsert(Profile(id = "a", name = "old", content = "1"))
        s.upsert(Profile(id = "a", name = "new", content = "2"))
        val all = s.loadAll()
        assertThat(all).hasSize(1)
        assertThat(all[0].name).isEqualTo("new")
        assertThat(all[0].content).isEqualTo("2")
    }

    @Test(expected = IllegalArgumentException::class)
    fun upsertRejectsPathTraversal() {
        val s = store()
        s.upsert(Profile(id = "../../malicious", name = "bad", content = "{}"))
    }

    @Test(expected = IllegalArgumentException::class)
    fun deleteRejectsPathTraversal() {
        val s = store()
        s.delete("../evil")
    }

    @Test(expected = IllegalArgumentException::class)
    fun upsertRejectsEmptyId() {
        val s = store()
        s.upsert(Profile(id = "   ", name = "empty", content = "{}"))
    }
}
