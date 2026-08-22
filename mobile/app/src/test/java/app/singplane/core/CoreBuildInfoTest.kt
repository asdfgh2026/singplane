package app.singplane.core

import app.singplane.BuildConfig
import com.google.common.truth.Truth.assertThat
import org.junit.Test

class CoreBuildInfoTest {
    @Test
    fun displayNameUsesGeneratedBuildConfigVersion() {
        assertThat(BuildConfig.SING_BOX_VERSION)
            .isEqualTo(System.getProperty("singBoxVersion"))
        assertThat(CoreBuildInfo.displayName).isEqualTo("sing-box ${BuildConfig.SING_BOX_VERSION}")
    }

    @Test
    fun unknownVersionDoesNotClaimDefaultVersion() {
        assertThat(CoreBuildInfo.displayName("unknown")).isEqualTo("sing-box (version unknown)")
    }
}
