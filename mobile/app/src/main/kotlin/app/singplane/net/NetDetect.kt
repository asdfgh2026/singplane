package app.singplane.net

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.net.Inet4Address
import java.net.NetworkInterface
import java.util.concurrent.TimeUnit

import androidx.annotation.StringRes
import app.singplane.R

data class IpInfo(
    val ip: String,
    val countryCode: String,
    val flagEmoji: String,
)

enum class IpCheckSource(@StringRes val labelRes: Int) {
    AUTO(R.string.source_auto),
    INTERNATIONAL(R.string.source_international),
    DOMESTIC(R.string.source_domestic),
}

object NetDetect {
    const val MASKED_IP = "*** *** *** ***"

    val INTERNATIONAL_SOURCES = listOf(
        "https://cp.cloudflare.com/cdn-cgi/trace",
        "https://api.cloudflare.com/cdn-cgi/trace",
    )

    val DOMESTIC_SOURCES = listOf(
        "https://www.qualcomm.cn/cdn-cgi/trace",
        "https://www.cloudflare-cn.com/cdn-cgi/trace",
    )

    private val httpClient = OkHttpClient.Builder()
        .connectTimeout(4, TimeUnit.SECONDS)
        .readTimeout(4, TimeUnit.SECONDS)
        .build()

    fun parseTrace(text: String): IpInfo? {
        var ip: String? = null
        var loc: String? = null
        for (line in text.lineSequence()) {
            val parts = line.split('=', limit = 2)
            if (parts.size != 2) continue
            val k = parts[0].trim()
            val v = parts[1].trim()
            if (k == "ip" && v.isNotEmpty()) ip = v
            if (k == "loc" && v.isNotEmpty()) loc = v
        }
        val foundIp = ip ?: return null
        val foundLoc = loc ?: "UN"
        return IpInfo(
            ip = foundIp,
            countryCode = foundLoc,
            flagEmoji = flagEmoji(foundLoc),
        )
    }

    fun flagEmoji(countryCode: String): String {
        val code = countryCode.trim().uppercase()
        if (code.length != 2 || !code.all { it in 'A'..'Z' }) {
            return countryCode
        }
        val first = Character.toChars(0x1F1E6 + (code[0] - 'A'))
        val second = Character.toChars(0x1F1E6 + (code[1] - 'A'))
        return String(first) + String(second)
    }

    fun getLocalIpv4(): String? {
        runCatching {
            val interfaces = NetworkInterface.getNetworkInterfaces() ?: return null
            for (intf in interfaces) {
                if (intf.isLoopback || !intf.isUp) continue
                val addrs = intf.inetAddresses
                for (addr in addrs) {
                    if (addr is Inet4Address && !addr.isLoopbackAddress && !addr.isLinkLocalAddress) {
                        val host = addr.hostAddress ?: continue
                        if (host != "127.0.0.1" && !host.startsWith("172.19.0.")) {
                            return host
                        }
                    }
                }
            }
        }
        return null
    }

    suspend fun detect(source: IpCheckSource): Result<IpInfo> = withContext(Dispatchers.IO) {
        val urls = when (source) {
            IpCheckSource.INTERNATIONAL -> INTERNATIONAL_SOURCES
            IpCheckSource.DOMESTIC -> DOMESTIC_SOURCES
            IpCheckSource.AUTO -> INTERNATIONAL_SOURCES + DOMESTIC_SOURCES
        }

        var lastErr: Throwable? = null
        for (url in urls) {
            try {
                val req = Request.Builder().url(url).build()
                httpClient.newCall(req).execute().use { resp ->
                    if (resp.isSuccessful) {
                        val body = resp.body?.string().orEmpty()
                        val info = parseTrace(body)
                        if (info != null) return@withContext Result.success(info)
                    }
                }
            } catch (t: Throwable) {
                lastErr = t
            }
        }
        Result.failure(lastErr ?: RuntimeException("检测失败"))
    }
}
