package app.singplane.core

/**
 * Picks the underlying physical NIC for libbox auto_detect_interface.
 * VPN/tun must never be reported as default or DNS bind fails with
 * "no available network interface".
 */
data class PhysicalIface(
    val name: String,
    val index: Int,
    val type: Kind,
    val metered: Boolean,
    val constrained: Boolean = false,
    val hasInternet: Boolean = true,
    val isVpn: Boolean = false,
) {
    enum class Kind { Wifi, Ethernet, Cellular, Other }
}

object NetworkInterfacePicker {
    /** golang.org/x net.Flags — libbox copies this int onto net.Interface.Flags. */
    const val FLAG_UP = 1
    const val FLAG_BROADCAST = 2
    const val FLAG_LOOPBACK = 4
    const val FLAG_POINTOPOINT = 8
    const val FLAG_MULTICAST = 16
    const val FLAG_RUNNING = 32

    fun netFlags(
        isUp: Boolean,
        isLoopback: Boolean = false,
        isPointToPoint: Boolean = false,
        supportsMulticast: Boolean = true,
    ): Int {
        var flags = 0
        if (isUp) flags = flags or FLAG_UP or FLAG_RUNNING
        if (isLoopback) flags = flags or FLAG_LOOPBACK
        if (isPointToPoint) {
            flags = flags or FLAG_POINTOPOINT
        } else if (!isLoopback) {
            flags = flags or FLAG_BROADCAST
        }
        if (supportsMulticast) flags = flags or FLAG_MULTICAST
        return flags
    }

    fun isVirtualName(name: String): Boolean {
        val n = name.lowercase()
        return n.startsWith("tun") ||
            n.startsWith("dummy") ||
            n.startsWith("p2p") ||
            n.startsWith("wlanp2p") ||
            n == "lo" ||
            n.startsWith("rmnet_data") && n.contains("ims")
    }

    fun pickDefault(ifaces: List<PhysicalIface>): PhysicalIface? {
        val usable = ifaces.filter { iface ->
            !iface.isVpn &&
                iface.hasInternet &&
                iface.index > 0 &&
                !isVirtualName(iface.name)
        }
        return usable.firstOrNull { it.type == PhysicalIface.Kind.Wifi }
            ?: usable.firstOrNull { it.type == PhysicalIface.Kind.Ethernet }
            ?: usable.firstOrNull { it.type == PhysicalIface.Kind.Cellular }
            ?: usable.firstOrNull()
    }
}
