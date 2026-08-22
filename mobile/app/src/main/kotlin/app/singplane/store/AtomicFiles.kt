package app.singplane.store

import java.io.File

internal fun File.writeAtomically(text: String) {
    parentFile?.mkdirs()
    val tmp = File(parentFile, "$name.tmp")
    tmp.writeText(text)
    if (!tmp.renameTo(this)) {
        tmp.copyTo(this, overwrite = true)
        tmp.delete()
    }
}
