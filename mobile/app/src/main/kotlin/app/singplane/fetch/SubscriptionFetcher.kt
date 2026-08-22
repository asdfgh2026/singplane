package app.singplane.fetch

data class FetchResult(
    val body: String,
    val upload: Long = 0,
    val download: Long = 0,
    val total: Long = 0,
    val expireMs: Long = 0,
    val httpStatus: Int = 200,
)

fun interface SubscriptionFetcher {
    suspend fun fetch(url: String): FetchResult
}
