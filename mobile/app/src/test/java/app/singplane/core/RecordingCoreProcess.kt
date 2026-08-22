package app.singplane.core

class RecordingCoreProcess : CoreProcess {
    val started = mutableListOf<Pair<String, String>>()
    var stopCount = 0
    var failMessage: String? = null

    override suspend fun start(binaryPath: String, configJson: String) {
        failMessage?.let { error(it) }
        started += binaryPath to configJson
    }

    override suspend fun stop() {
        stopCount += 1
    }
}
