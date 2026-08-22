package app.singplane.clash

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.net.URLEncoder
import java.util.concurrent.TimeUnit

class OkHttpClashClient(
    private val client: OkHttpClient = defaultClient(),
) {
    companion object {
        fun defaultClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(12, TimeUnit.SECONDS)
            .proxy(java.net.Proxy.NO_PROXY)
            .socketFactory(LoopbackSocketFactory())
            .build()
    }
    suspend fun groups(baseUrl: String): List<ProxyGroup> = withContext(Dispatchers.IO) {
        val url = baseUrl.trimEnd('/') + "/proxies"
        val req = Request.Builder().url(url).get().build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful) error("Clash API ${resp.code}: $body")
            ClashApiParser.groups(body)
        }
    }

    suspend fun groupsWithNodes(baseUrl: String): List<GroupWithNodes> = withContext(Dispatchers.IO) {
        val url = baseUrl.trimEnd('/') + "/proxies"
        val req = Request.Builder().url(url).get().build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful) error("Clash API ${resp.code}: $body")
            ClashApiParser.parseGroupNodes(body)
        }
    }

    suspend fun select(baseUrl: String, group: String, name: String) = withContext(Dispatchers.IO) {
        val url = baseUrl.trimEnd('/') + "/proxies/" + ClashApiPath.encodeName(group)
        val json = """{"name":${JSONObject.quote(name)}}"""
        val req = Request.Builder()
            .url(url)
            .put(json.toRequestBody("application/json".toMediaType()))
            .build()
        client.newCall(req).execute().use { resp ->
            if (!resp.isSuccessful && resp.code != 204) {
                error("切换失败 HTTP ${resp.code}")
            }
        }
    }

    suspend fun getMode(baseUrl: String): String = withContext(Dispatchers.IO) {
        val url = baseUrl.trimEnd('/') + "/configs"
        val req = Request.Builder().url(url).get().build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful) return@withContext "rule"
            ClashApiParser.parseMode(body)
        }
    }

    suspend fun changeMode(baseUrl: String, mode: String) = withContext(Dispatchers.IO) {
        val url = baseUrl.trimEnd('/') + "/configs"
        val json = """{"mode":"$mode"}"""
        val req = Request.Builder()
            .url(url)
            .patch(json.toRequestBody("application/json".toMediaType()))
            .build()
        client.newCall(req).execute().use { resp ->
            if (!resp.isSuccessful && resp.code != 204) {
                error("修改模式失败 HTTP ${resp.code}")
            }
        }
    }

    suspend fun getMemory(baseUrl: String): Long = withContext(Dispatchers.IO) {
        val url = baseUrl.trimEnd('/') + "/memory"
        val req = Request.Builder().url(url).get().build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful) return@withContext 0L
            ClashApiParser.parseMemoryInuse(body)
        }
    }

    suspend fun testDelay(
        baseUrl: String,
        proxyName: String,
        testUrl: String = "https://www.gstatic.com/generate_204",
        timeoutMs: Int = 5000,
    ): Int? = withContext(Dispatchers.IO) {
        val enc = ClashApiPath.encodeName(proxyName)
        val url = baseUrl.trimEnd('/') + "/proxies/$enc/delay?url=" + URLEncoder.encode(testUrl, Charsets.UTF_8.name()) + "&timeout=$timeoutMs"
        val req = Request.Builder().url(url).get().build()
        try {
            client.newCall(req).execute().use { resp ->
                val body = resp.body?.string().orEmpty()
                if (resp.isSuccessful) {
                    val d = JSONObject(body).optInt("delay", -1)
                    if (d > 0) return@withContext d
                    return@withContext 0
                }
                0
            }
        } catch (_: Throwable) {
            0
        }
    }

    suspend fun getConnections(baseUrl: String): ConnectionsSnapshot = withContext(Dispatchers.IO) {
        val url = baseUrl.trimEnd('/') + "/connections"
        val req = Request.Builder().url(url).get().build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful) error("Clash API ${resp.code}: $body")
            ClashConnectionParser.parse(body)
        }
    }

    suspend fun closeConnection(baseUrl: String, id: String) = withContext(Dispatchers.IO) {
        val url = baseUrl.trimEnd('/') + "/connections/" + ClashApiPath.encodeName(id)
        val req = Request.Builder().url(url).delete().build()
        client.newCall(req).execute().use { resp ->
            if (!resp.isSuccessful && resp.code != 204) {
                error("关闭连接失败 HTTP ${resp.code}")
            }
        }
    }

    suspend fun closeAllConnections(baseUrl: String) = withContext(Dispatchers.IO) {
        val url = baseUrl.trimEnd('/') + "/connections"
        val req = Request.Builder().url(url).delete().build()
        client.newCall(req).execute().use { resp ->
            if (!resp.isSuccessful && resp.code != 204) {
                error("关闭全部连接失败 HTTP ${resp.code}")
            }
        }
    }
}

