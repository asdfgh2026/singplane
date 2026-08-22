package app.singplane.fetch

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.util.concurrent.TimeUnit

class OkHttpSubscriptionFetcher(
    private val client: OkHttpClient = defaultClient(),
) : SubscriptionFetcher {
    override suspend fun fetch(url: String): FetchResult = withContext(Dispatchers.IO) {
        val req = Request.Builder()
            .url(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "*/*")
            .build()
        client.newCall(req).execute().use { resp ->
            val bytes = resp.body?.bytes() ?: ByteArray(0)
            if (bytes.size > MAX_BYTES) {
                error("订阅过大（>${MAX_BYTES / 1024 / 1024} MiB）")
            }
            if (!resp.isSuccessful) {
                val err = bytes.decodeToString().take(64 * 1024)
                error("HTTP ${resp.code}: $err")
            }
            val info = UserinfoParser.parse(resp.header("Subscription-Userinfo"))
            FetchResult(
                body = bytes.decodeToString(),
                upload = info.upload,
                download = info.download,
                total = info.total,
                expireMs = info.expireMs,
                httpStatus = resp.code,
            )
        }
    }

    companion object {
        const val USER_AGENT = "sing-box/SingPanel clash.meta"
        const val MAX_BYTES = 16 * 1024 * 1024

        fun defaultClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(30, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .followRedirects(true)
            .build()
    }
}
