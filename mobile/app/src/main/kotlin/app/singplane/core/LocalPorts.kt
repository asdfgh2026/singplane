package app.singplane.core

import java.net.InetSocketAddress
import java.net.Socket

/** Loopback occupancy for leftover mixed / Clash API listeners. */
object LocalPorts {
    fun isOccupied(port: Int, host: String = "127.0.0.1"): Boolean {
        if (port !in 1..65535) return false
        return try {
            Socket().use { socket ->
                socket.connect(InetSocketAddress(host, port), 150)
                true
            }
        } catch (_: Exception) {
            false
        }
    }

    fun busy(ports: Set<Int>): Set<Int> = ports.filterTo(linkedSetOf()) { isOccupied(it) }

    fun waitUntilFree(ports: Set<Int>, timeoutMs: Long = 2_000, stepMs: Long = 100): Boolean {
        if (ports.isEmpty()) return true
        val deadline = System.currentTimeMillis() + timeoutMs
        while (true) {
            if (busy(ports).isEmpty()) return true
            if (System.currentTimeMillis() >= deadline) return false
            Thread.sleep(stepMs.coerceAtLeast(10))
        }
    }
}
