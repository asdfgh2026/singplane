package app.singplane.net

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class NetDetectTest {

    @Test
    fun parseCloudflareTraceSuccess() {
        val traceText = """
            fl=533f86
            h=cp.cloudflare.com
            ip=104.28.245.89
            ts=1723876000.123
            visit_scheme=https
            uag=Mozilla/5.0
            colo=HKG
            sliver=none
            http=http/2
            loc=HK
            tls=TLSv1.3
            sni=plaintext
            warp=off
            gateway=off
            rbi=off
            kex=X25519
        """.trimIndent()

        val info = NetDetect.parseTrace(traceText)
        assertThat(info).isNotNull()
        assertThat(info?.ip).isEqualTo("104.28.245.89")
        assertThat(info?.countryCode).isEqualTo("HK")
        assertThat(info?.flagEmoji).isEqualTo("🇭🇰")
    }

    @Test
    fun parseCloudflareTraceInvalid() {
        val invalidText = "random text without ip or loc"
        val info = NetDetect.parseTrace(invalidText)
        assertThat(info).isNull()
    }

    @Test
    fun countryCodeToFlagEmoji() {
        assertThat(NetDetect.flagEmoji("US")).isEqualTo("🇺🇸")
        assertThat(NetDetect.flagEmoji("CN")).isEqualTo("🇨🇳")
        assertThat(NetDetect.flagEmoji("JP")).isEqualTo("🇯🇵")
        assertThat(NetDetect.flagEmoji("SG")).isEqualTo("🇸🇬")
        assertThat(NetDetect.flagEmoji("UNKNOWN")).isEqualTo("UNKNOWN")
    }

    @Test
    fun maskedIpConstant() {
        assertThat(NetDetect.MASKED_IP).isEqualTo("*** *** *** ***")
    }
}
