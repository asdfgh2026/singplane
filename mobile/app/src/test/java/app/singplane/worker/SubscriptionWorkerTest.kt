package app.singplane.worker

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class SubscriptionWorkerTest {

    @Test
    fun clampIntervalMinutes() {
        fun effectiveMinutes(interval: Int): Long {
            return if (interval <= 0) 0L else if (interval < 15) 15L else interval.toLong()
        }

        assertThat(effectiveMinutes(0)).isEqualTo(0L)
        assertThat(effectiveMinutes(3)).isEqualTo(15L)
        assertThat(effectiveMinutes(14)).isEqualTo(15L)
        assertThat(effectiveMinutes(15)).isEqualTo(15L)
        assertThat(effectiveMinutes(60)).isEqualTo(60L)
        assertThat(effectiveMinutes(1440)).isEqualTo(1440L)
    }
}
