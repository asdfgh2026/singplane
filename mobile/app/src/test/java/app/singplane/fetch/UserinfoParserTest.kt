package app.singplane.fetch

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class UserinfoParserTest {
    @Test
    fun parsesSubscriptionUserinfo() {
        val info = UserinfoParser.parse(
            "upload=100; download=200; total=1000; expire=1700000000",
        )
        assertThat(info.upload).isEqualTo(100)
        assertThat(info.download).isEqualTo(200)
        assertThat(info.total).isEqualTo(1000)
        assertThat(info.expireMs).isEqualTo(1_700_000_000_000L)
    }

    @Test
    fun emptyOnNull() {
        val info = UserinfoParser.parse(null)
        assertThat(info.upload).isEqualTo(0)
        assertThat(info.expireMs).isEqualTo(0)
    }
}
