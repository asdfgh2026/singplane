package app.singplane.core

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class NetworkInterfacePickerTest {
    @Test
    fun prefersWlanOverCellularAndSkipsVpn() {
        val picked = NetworkInterfacePicker.pickDefault(
            listOf(
                PhysicalIface("ccmni1", 15, PhysicalIface.Kind.Cellular, metered = true, hasInternet = false),
                PhysicalIface("ccmni2", 16, PhysicalIface.Kind.Cellular, metered = true, hasInternet = true),
                PhysicalIface("wlan0", 36, PhysicalIface.Kind.Wifi, metered = false, hasInternet = true),
                PhysicalIface("tun0", 42, PhysicalIface.Kind.Other, metered = false, isVpn = true),
            ),
        )
        assertThat(picked?.name).isEqualTo("wlan0")
        assertThat(picked?.index).isEqualTo(36)
    }

    @Test
    fun skipsTunEvenIfListedFirst() {
        val picked = NetworkInterfacePicker.pickDefault(
            listOf(
                PhysicalIface("tun0", 10, PhysicalIface.Kind.Other, metered = false, hasInternet = true, isVpn = true),
                PhysicalIface("ccmni2", 16, PhysicalIface.Kind.Cellular, metered = true, hasInternet = true),
            ),
        )
        assertThat(picked?.name).isEqualTo("ccmni2")
    }

    @Test
    fun wifiUpFlagsMatchGoNetFlags() {
        val flags = NetworkInterfacePicker.netFlags(
            isUp = true,
            isLoopback = false,
            isPointToPoint = false,
            supportsMulticast = true,
        )
        assertThat(flags and NetworkInterfacePicker.FLAG_UP).isNotEqualTo(0)
        assertThat(flags and NetworkInterfacePicker.FLAG_RUNNING).isNotEqualTo(0)
        assertThat(flags and NetworkInterfacePicker.FLAG_BROADCAST).isNotEqualTo(0)
        assertThat(flags and NetworkInterfacePicker.FLAG_MULTICAST).isNotEqualTo(0)
        assertThat(flags).isEqualTo(1 or 2 or 16 or 32)
    }

    @Test
    fun emptyWhenOnlyVpn() {
        val picked = NetworkInterfacePicker.pickDefault(
            listOf(
                PhysicalIface("tun0", 10, PhysicalIface.Kind.Other, metered = false, isVpn = true),
            ),
        )
        assertThat(picked).isNull()
    }
}
