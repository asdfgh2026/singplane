package app.singplane.vpn

/** Android-owned TUN / service lifecycle. Kernel attach comes later. */
interface VpnSession {
    suspend fun start(runtimeConfig: String)
    suspend fun stop()
}
