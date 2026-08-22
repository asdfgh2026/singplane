package app.singplane.core

import app.singplane.store.writeAtomically
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

class ProcessCoreProcess(
    private val workDir: File,
    private val onLog: (String) -> Unit = {},
) : CoreProcess {
    private var process: Process? = null

    override suspend fun start(binaryPath: String, configJson: String) = withContext(Dispatchers.IO) {
        stop()
        val bin = File(binaryPath)
        if (!bin.isFile) error("找不到内核文件: $binaryPath")
        bin.setExecutable(true, false)
        workDir.mkdirs()
        val cfg = File(workDir, "config.runtime.json")
        cfg.writeAtomically(configJson)
        val pb = ProcessBuilder(bin.absolutePath, "run", "-c", cfg.absolutePath)
            .directory(workDir)
            .redirectErrorStream(true)
        val p = pb.start()
        process = p
        Thread({
            p.inputStream.bufferedReader().useLines { lines ->
                lines.forEach { onLog(it) }
            }
        }, "sing-box-log").apply { isDaemon = true }.start()
        Thread.sleep(200)
        if (!p.isAlive) {
            val code = p.exitValue()
            error("内核立刻退出 (code=$code)")
        }
    }

    override suspend fun stop() = withContext(Dispatchers.IO) {
        process?.let { p ->
            p.destroy()
            val died = runCatching { p.waitFor(1, java.util.concurrent.TimeUnit.SECONDS) }.getOrDefault(false)
            if (!died && p.isAlive) {
                p.destroyForcibly()
                runCatching { p.waitFor(1, java.util.concurrent.TimeUnit.SECONDS) }
            }
        }
        process = null
    }
}
