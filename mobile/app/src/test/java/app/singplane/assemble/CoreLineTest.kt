package app.singplane.assemble

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class CoreLineTest {

    @Test
    fun meetsCoreFloor() {
        assertThat(CoreLine.meetsTailscaleCore(null)).isFalse()
        assertThat(CoreLine.meetsTailscaleCore("1.11.0")).isFalse()
        assertThat(CoreLine.meetsTailscaleCore("1.12.0")).isFalse()
        assertThat(CoreLine.meetsTailscaleCore("1.13.18")).isTrue()
        assertThat(CoreLine.meetsTailscaleCore("1.14.0")).isTrue()
        assertThat(CoreLine.meetsTailscaleCore("v1.14.0-beta.3")).isTrue()
        assertThat(CoreLine.meetsTailscaleCore("1.15.0")).isTrue()
        assertThat(CoreLine.meetsTailscaleCore("2.0.0")).isTrue()

        assertThat(CoreLine.fromVersion("1.13.18")).isEqualTo(CoreLine.V13)
        assertThat(CoreLine.fromVersion("1.14.0-beta.15")).isEqualTo(CoreLine.V14)

        assertThat(CoreLine.V13.atLeast(1, 14)).isFalse()
        assertThat(CoreLine.V13.atLeast(1, 13)).isTrue()
        assertThat(CoreLine.V14.atLeast(1, 14)).isTrue()
    }
}
