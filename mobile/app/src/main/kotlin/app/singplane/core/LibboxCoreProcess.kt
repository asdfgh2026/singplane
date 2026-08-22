package app.singplane.core

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.os.Build
import android.os.Handler
import android.os.Looper
import java.net.InetSocketAddress
import io.nekohasekai.libbox.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.net.Inet4Address
import java.net.Inet6Address

class StringArrayIterator(private val list: List<String>) : StringIterator {
    private var index = 0
    override fun len(): Int = list.size
    override fun hasNext(): Boolean = index < list.size
    override fun next(): String = if (index < list.size) list[index++] else ""
}

class NetworkInterfaceArrayIterator(private val list: List<io.nekohasekai.libbox.NetworkInterface>) : NetworkInterfaceIterator {
    private var index = 0
    override fun hasNext(): Boolean = index < list.size
    override fun next(): io.nekohasekai.libbox.NetworkInterface? = if (index < list.size) list[index++] else null
}

class LibboxCoreProcess(
    private val context: Context,
    private val onLog: (String) -> Unit = {},
) : CoreProcess {
    private var server: CommandServer? = null
    private var defaultIfaceCallback: ConnectivityManager.NetworkCallback? = null
    private var defaultIfaceListener: InterfaceUpdateListener? = null

    private fun collectNetworkInterfaces(): List<io.nekohasekai.libbox.NetworkInterface> {
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager ?: return emptyList()
        val result = mutableListOf<io.nekohasekai.libbox.NetworkInterface>()

        try {
            val networks = cm.allNetworks
            for (network in networks) {
                val lp = cm.getLinkProperties(network) ?: continue
                val caps = cm.getNetworkCapabilities(network) ?: continue
                val ifName = lp.interfaceName ?: continue
                if (ifName.startsWith("tun") || ifName.startsWith("dummy") || ifName.startsWith("p2p")) continue

                val isVpn = caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN)
                if (isVpn) continue

                val iface = runCatching { java.net.NetworkInterface.getByName(ifName) }.getOrNull() ?: continue

                val addresses = mutableListOf<String>()
                for (linkAddr in lp.linkAddresses) {
                    val addr = linkAddr.address
                    val prefix = linkAddr.prefixLength
                    val ipStr = when (addr) {
                        is Inet4Address -> addr.hostAddress
                        is Inet6Address -> addr.hostAddress?.substringBefore("%")
                        else -> null
                    }
                    val validPrefix = when (addr) {
                        is Inet4Address -> if (prefix in 1..32) prefix else 24
                        is Inet6Address -> if (prefix in 1..128) prefix else 64
                        else -> 24
                    }
                    if (!ipStr.isNullOrEmpty()) {
                        addresses.add("$ipStr/$validPrefix")
                    }
                }
                if (addresses.isEmpty()) continue

                val isWifi = caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)
                val isCell = caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR)
                val isEth = caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)
                val type = when {
                    isWifi -> Libbox.InterfaceTypeWIFI
                    isCell -> Libbox.InterfaceTypeCellular
                    isEth -> Libbox.InterfaceTypeEthernet
                    else -> Libbox.InterfaceTypeOther
                }

                val dnsList = lp.dnsServers.mapNotNull { it.hostAddress }.ifEmpty { listOf("223.5.5.5", "1.1.1.1") }
                val metered = !caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)

                val netIf = io.nekohasekai.libbox.NetworkInterface().apply {
                    this.index = iface.index
                    this.mtu = iface.mtu.coerceAtLeast(1500)
                    this.name = ifName
                    this.addresses = StringArrayIterator(addresses)
                    this.flags = NetworkInterfacePicker.netFlags(
                        isUp = iface.isUp,
                        isLoopback = iface.isLoopback,
                        isPointToPoint = iface.isPointToPoint,
                        supportsMulticast = runCatching { iface.supportsMulticast() }.getOrDefault(true),
                    )
                    this.type = type
                    this.dnsServer = StringArrayIterator(dnsList)
                    this.metered = metered
                }
                result.add(netIf)
            }

            // Fallback: If cm.allNetworks didn't return any physical interfaces, query java.net.NetworkInterface
            if (result.isEmpty()) {
                val ifaces = java.net.NetworkInterface.getNetworkInterfaces() ?: return emptyList()
                for (iface in ifaces) {
                    if (!iface.isUp || iface.isLoopback || iface.name.startsWith("tun") || iface.name.startsWith("dummy")) continue
                    val addresses = mutableListOf<String>()
                    for (ia in iface.interfaceAddresses) {
                        val addr = ia.address
                        val prefix = ia.networkPrefixLength.toInt()
                        val ipStr = when (addr) {
                            is Inet4Address -> addr.hostAddress
                            is Inet6Address -> addr.hostAddress?.substringBefore("%")
                            else -> null
                        }
                        val validPrefix = when (addr) {
                            is Inet4Address -> if (prefix in 1..32) prefix else 24
                            is Inet6Address -> if (prefix in 1..128) prefix else 64
                            else -> 24
                        }
                        if (!ipStr.isNullOrEmpty()) {
                            addresses.add("$ipStr/$validPrefix")
                        }
                    }
                    if (addresses.isEmpty()) continue
                    val netIf = io.nekohasekai.libbox.NetworkInterface().apply {
                        this.index = iface.index
                        this.mtu = iface.mtu.coerceAtLeast(1500)
                        this.name = iface.name
                        this.addresses = StringArrayIterator(addresses)
                        this.flags = NetworkInterfacePicker.netFlags(
                            isUp = iface.isUp,
                            isLoopback = iface.isLoopback,
                            isPointToPoint = iface.isPointToPoint,
                            supportsMulticast = runCatching { iface.supportsMulticast() }.getOrDefault(true),
                        )
                        this.type = Libbox.InterfaceTypeWIFI
                        this.dnsServer = StringArrayIterator(listOf("223.5.5.5", "1.1.1.1"))
                        this.metered = false
                    }
                    result.add(netIf)
                }
            }
        } catch (t: Throwable) {
            onLog("collectNetworkInterfaces warning: ${t.message}")
            android.util.Log.e("SingPanel", "collectNetworkInterfaces error", t)
        }
        android.util.Log.i(
            "SingPanel",
            "collectNetworkInterfaces found ${result.size}: ${result.map { "${it.name}(${it.index}, type=${it.type}, flags=${it.flags})" }}",
        )
        onLog("collectNetworkInterfaces found ${result.size}: ${result.map { "${it.name}(${it.index}, flags=${it.flags})" }}")
        return result
    }

    private fun connectivity(): ConnectivityManager? =
        context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager

    private fun physicalIfacesFromConnectivity(): List<PhysicalIface> {
        val cm = connectivity() ?: return emptyList()
        val out = mutableListOf<PhysicalIface>()
        for (network in cm.allNetworks) {
            val lp = cm.getLinkProperties(network) ?: continue
            val caps = cm.getNetworkCapabilities(network) ?: continue
            val ifName = lp.interfaceName ?: continue
            val isVpn = caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN)
            val hasInternet = caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            val iface = runCatching { java.net.NetworkInterface.getByName(ifName) }.getOrNull()
            val index = iface?.index ?: continue
            val kind = when {
                caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> PhysicalIface.Kind.Wifi
                caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> PhysicalIface.Kind.Ethernet
                caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> PhysicalIface.Kind.Cellular
                else -> PhysicalIface.Kind.Other
            }
            val metered = !caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)
            val constrained = caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_CONGESTED).not()
            out.add(
                PhysicalIface(
                    name = ifName,
                    index = index,
                    type = kind,
                    metered = metered,
                    constrained = constrained,
                    hasInternet = hasInternet,
                    isVpn = isVpn,
                ),
            )
        }
        return out
    }

    private fun notifyDefaultInterface(listener: InterfaceUpdateListener) {
        val picked = NetworkInterfacePicker.pickDefault(physicalIfacesFromConnectivity())
        if (picked == null) {
            onLog("default interface: none (only VPN/virtual?)")
            android.util.Log.w("SingPanel", "no physical default interface")
            return
        }
        onLog("default interface → ${picked.name}(${picked.index})")
        android.util.Log.i("SingPanel", "updateDefaultInterface ${picked.name} idx=${picked.index}")
        listener.updateDefaultInterface(picked.name, picked.index, picked.metered, picked.constrained)
    }

    private fun startPhysicalDefaultMonitor(listener: InterfaceUpdateListener) {
        stopPhysicalDefaultMonitor()
        defaultIfaceListener = listener
        notifyDefaultInterface(listener)
        val cm = connectivity() ?: return
        val request = NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
            .build()
        val cb = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                defaultIfaceListener?.let { notifyDefaultInterface(it) }
            }
            override fun onLost(network: Network) {
                defaultIfaceListener?.let { notifyDefaultInterface(it) }
            }
            override fun onCapabilitiesChanged(network: Network, networkCapabilities: NetworkCapabilities) {
                defaultIfaceListener?.let { notifyDefaultInterface(it) }
            }
            override fun onLinkPropertiesChanged(network: Network, linkProperties: android.net.LinkProperties) {
                defaultIfaceListener?.let { notifyDefaultInterface(it) }
            }
        }
        defaultIfaceCallback = cb
        runCatching {
            cm.registerNetworkCallback(request, cb, Handler(Looper.getMainLooper()))
        }.onFailure { t ->
            onLog("registerDefaultNetworkCallback failed: ${t.message}")
        }
    }

    private fun stopPhysicalDefaultMonitor() {
        val cm = connectivity()
        defaultIfaceCallback?.let { cb ->
            runCatching { cm?.unregisterNetworkCallback(cb) }
        }
        defaultIfaceCallback = null
        defaultIfaceListener = null
    }

    private var lastListenPorts: Set<Int> = emptySet()

    override suspend fun start(binaryPath: String, configJson: String) = withContext(Dispatchers.IO) {
        val ports = ListenPorts.fromConfig(configJson).ifEmpty { setOf(2080, 9090) }
        lastListenPorts = ports
        reclaimLeftover(ports)

        val baseDir = File(context.filesDir, "runtime").apply { mkdirs() }
        val workDir = File(context.filesDir, "cache").apply { mkdirs() }
        val tempDir = File(context.cacheDir, "libbox").apply { mkdirs() }

        val setupOpts = SetupOptions().apply {
            this.basePath = baseDir.absolutePath
            this.workingPath = workDir.absolutePath
            this.tempPath = tempDir.absolutePath
            this.debug = true
            this.logMaxLines = 300
        }
        Libbox.setup(setupOpts)

        val handler = object : CommandServerHandler {
            override fun serviceStop() {}
            override fun serviceReload() {}
            override fun getSystemProxyStatus(): SystemProxyStatus? = null
            override fun setSystemProxyEnabled(enabled: Boolean) {}
            override fun writeDebugMessage(message: String) {
                onLog(message)
            }
        }

        val platformInterface = object : PlatformInterface {
            override fun localDNSTransport(): LocalDNSTransport? = null
            override fun usePlatformAutoDetectInterfaceControl(): Boolean = true
            override fun autoDetectInterfaceControl(fd: Int) {
                val vpn = app.singplane.vpn.SingPanelVpnService.instance
                if (vpn == null) {
                    onLog("protect($fd) skipped: VpnService not ready")
                    return
                }
                val ok = vpn.protect(fd)
                if (!ok) onLog("protect($fd) failed")
            }
            override fun openTun(options: TunOptions): Int {
                val fd = app.singplane.vpn.SingPanelVpnService.consumeTunFd()
                if (fd < 0) {
                    onLog("openTun: VPN fd not ready")
                    throw IllegalStateException("VPN tun fd not ready")
                }
                onLog("openTun fd=$fd")
                android.util.Log.i("SingPanel", "openTun fd=$fd")
                return fd
            }
            override fun useProcFS(): Boolean = false
            override fun findConnectionOwner(
                ipProtocol: Int,
                sourceAddress: String,
                sourcePort: Int,
                destinationAddress: String,
                destinationPort: Int,
            ): ConnectionOwner {
                // libbox panics on a null ConnectionOwner (SFA always returns an object).
                val owner = ConnectionOwner()
                var uid = -1
                if (Build.VERSION.SDK_INT >= 29) {
                    val cm = connectivity()
                    uid = runCatching {
                        cm?.getConnectionOwnerUid(
                            ipProtocol,
                            InetSocketAddress(sourceAddress, sourcePort),
                            InetSocketAddress(destinationAddress, destinationPort),
                        ) ?: -1
                    }.getOrDefault(-1)
                }
                owner.userId = uid
                val pkgs = if (uid > 0) {
                    context.packageManager.getPackagesForUid(uid)?.toList().orEmpty()
                } else {
                    emptyList()
                }
                owner.userName = pkgs.firstOrNull().orEmpty()
                owner.processPath = pkgs.firstOrNull().orEmpty()
                owner.setAndroidPackageNames(StringArrayIterator(pkgs))
                return owner
            }
            override fun startDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {
                startPhysicalDefaultMonitor(listener)
            }
            override fun closeDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {
                stopPhysicalDefaultMonitor()
            }
            override fun getInterfaces(): NetworkInterfaceIterator {
                return NetworkInterfaceArrayIterator(collectNetworkInterfaces())
            }

            override fun underNetworkExtension(): Boolean = false
            override fun includeAllNetworks(): Boolean = false
            override fun readWIFIState(): WIFIState? = null
            override fun systemCertificates(): StringIterator? = null
            override fun clearDNSCache() {}
            override fun sendNotification(notification: Notification?) {}
        }

        try {
            startBox(handler, platformInterface, configJson)
        } catch (e: Exception) {
            if (!ListenPorts.isAddressInUse(e.message)) throw e
            onLog("port in use, reclaim leftover and retry: ${e.message}")
            reclaimLeftover(ports)
            startBox(handler, platformInterface, configJson)
        }
        onLog("${CoreBuildInfo.displayName} libbox core started")
    }

    private fun startBox(
        handler: CommandServerHandler,
        platformInterface: PlatformInterface,
        configJson: String,
    ) {
        val srv = Libbox.newCommandServer(handler, platformInterface)
        try {
            srv.start()
            srv.startOrReloadService(configJson, OverrideOptions())
            server = srv
        } catch (t: Throwable) {
            runCatching { srv.closeService() }
            runCatching { srv.close() }
            if (server === srv) server = null
            throw t
        }
    }

    private fun closeHeldServer() {
        val s = server ?: return
        server = null
        runCatching { s.closeService() }
        runCatching { s.close() }
    }

    private fun tryStandaloneServiceClose() {
        runCatching {
            val client = Libbox.newStandaloneCommandClient()
            runCatching { client.connect() }
            client.serviceClose()
            runCatching { client.disconnect() }
        }
    }

    private fun reclaimLeftover(ports: Set<Int>) {
        stopPhysicalDefaultMonitor()
        closeHeldServer()
        if (ports.any { LocalPorts.isOccupied(it) }) {
            onLog("listen ports still held ${LocalPorts.busy(ports)}, closing leftover service")
            tryStandaloneServiceClose()
        }
        if (!LocalPorts.waitUntilFree(ports, timeoutMs = 2_000)) {
            val busy = LocalPorts.busy(ports)
            onLog("listen ports still busy after close: $busy")
        }
    }

    override suspend fun stop() = withContext(Dispatchers.IO) {
        val ports = lastListenPorts.ifEmpty { setOf(2080, 9090) }
        reclaimLeftover(ports)
    }
}
