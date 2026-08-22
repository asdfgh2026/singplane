package app.singplane.vpn

class RecordingVpnSession : VpnSession {
    val started = mutableListOf<String>()
    var stopCount: Int = 0
    var throwOnStart: Throwable? = null

    override suspend fun start(runtimeConfig: String) {
        throwOnStart?.let { throw it }
        started += runtimeConfig
    }

    override suspend fun stop() {
        stopCount += 1
    }

}
