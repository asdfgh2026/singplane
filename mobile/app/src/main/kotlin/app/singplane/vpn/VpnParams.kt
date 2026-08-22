package app.singplane.vpn

import android.net.VpnService
import org.json.JSONObject

data class VpnRoute(
    val address: String,
    val prefixLength: Int,
)

data class VpnParams(
    val packageName: String,
    val sessionName: String = "SingPanel",
    val ipv4Address: String = "172.19.0.1",
    val ipv4Prefix: Int = 30,
    val ipv6Address: String? = null,
    val ipv6Prefix: Int? = null,
    val mtu: Int = 1500,
    val dnsServers: List<String> = listOf("8.8.8.8", "1.1.1.1"),
    val routes: List<VpnRoute> = listOf(VpnRoute("0.0.0.0", 0)),
    val disallowedPackages: List<String> = listOf(packageName),
) {
    fun applyTo(builder: VpnService.Builder): VpnService.Builder {
        builder.setSession(sessionName)
        builder.setMtu(mtu)
        builder.setBlocking(false)
        builder.addAddress(ipv4Address, ipv4Prefix)

        val addr6 = ipv6Address
        val prefix6 = ipv6Prefix
        if (!addr6.isNullOrBlank() && prefix6 != null) {
            runCatching { builder.addAddress(addr6, prefix6) }
        }
        for (route in routes) {
            builder.addRoute(route.address, route.prefixLength)
        }

        for (dns in dnsServers) {
            builder.addDnsServer(dns)
        }
        for (pkg in disallowedPackages) {
            runCatching { builder.addDisallowedApplication(pkg) }
        }
        return builder
    }

    companion object {
        fun fromSingBoxJson(json: String, packageName: String): VpnParams {
            var ipv4Addr = "172.19.0.1"
            var ipv4Prefix = 30
            var ipv6Addr: String? = null
            var ipv6Prefix: Int? = null
            var mtu = 1500
            val dnsList = mutableListOf<String>()

            runCatching {
                val root = JSONObject(json)
                val inbounds = root.optJSONArray("inbounds")
                if (inbounds != null) {
                    for (i in 0 until inbounds.length()) {
                        val inb = inbounds.optJSONObject(i) ?: continue
                        if (inb.optString("type") == "tun") {
                            val inet4 = inb.optString("inet4_address")
                            if (inet4.isNotEmpty()) {
                                val parts = inet4.split('/')
                                ipv4Addr = parts[0]
                                if (parts.size > 1) {
                                    ipv4Prefix = parts[1].toIntOrNull() ?: 30
                                }
                            }
                            val addrArr = inb.optJSONArray("address")
                            if (addrArr != null) {
                                for (j in 0 until addrArr.length()) {
                                    val item = addrArr.optString(j)
                                    if (item.contains('.') && !item.contains(':')) {
                                        val parts = item.split('/')
                                        ipv4Addr = parts[0]
                                        if (parts.size > 1) {
                                            ipv4Prefix = parts[1].toIntOrNull() ?: 30
                                        }
                                    } else if (item.contains(':')) {
                                        val parts = item.split('/')
                                        ipv6Addr = parts[0]
                                        if (parts.size > 1) {
                                            ipv6Prefix = parts[1].toIntOrNull() ?: 126
                                        }
                                    }
                                }
                            }
                            val inet6 = inb.optString("inet6_address")
                            if (inet6.isNotEmpty()) {
                                val parts = inet6.split('/')
                                ipv6Addr = parts[0]
                                if (parts.size > 1) {
                                    ipv6Prefix = parts[1].toIntOrNull() ?: 126
                                }
                            }
                            val m = inb.optInt("mtu", 0)
                            if (m > 0) mtu = m
                        }
                    }
                }

                val dnsObj = root.optJSONObject("dns")
                val servers = dnsObj?.optJSONArray("servers")
                if (servers != null) {
                    for (i in 0 until servers.length()) {
                        val s = servers.optJSONObject(i)
                        val addr = s?.optString("address")?.substringBefore(':')
                        if (!addr.isNullOrEmpty() && (addr.count { it == '.' } == 3 || addr.contains(':'))) {
                            if (!dnsList.contains(addr)) {
                                dnsList.add(addr)
                            }
                        }
                    }
                }
            }

            if (dnsList.isEmpty()) {
                dnsList.addAll(listOf("8.8.8.8", "1.1.1.1"))
            }

            return VpnParams(
                packageName = packageName,
                ipv4Address = ipv4Addr,
                ipv4Prefix = ipv4Prefix,
                ipv6Address = ipv6Addr,
                ipv6Prefix = ipv6Prefix,
                mtu = mtu,
                dnsServers = dnsList,
                disallowedPackages = listOf(packageName),
            )
        }
    }
}
