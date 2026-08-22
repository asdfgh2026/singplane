package app.singplane.core

interface CoreProcess {
    suspend fun start(binaryPath: String, configJson: String)
    suspend fun stop()
}
