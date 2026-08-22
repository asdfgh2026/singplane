package app.singplane.vpn

import com.google.common.truth.Truth.assertThat
import org.junit.After
import org.junit.Before
import org.junit.Test

class SingPanelVpnServiceTest {
    @Before
    @After
    fun cleanup() {
        SingPanelVpnService.resetReady()
    }

    @Test
    fun resetReadyClearsFd() {
        SingPanelVpnService.resetReady()
        assertThat(SingPanelVpnService.currentTunFd()).isEqualTo(-1)
    }

    @Test
    fun consumeTunFdReturnsNegativeWhenNotReady() {
        val fd = SingPanelVpnService.consumeTunFd(timeoutMs = 50)
        assertThat(fd).isEqualTo(-1)
    }
}
