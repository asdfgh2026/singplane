package app.singplane.assemble

enum class CoreLine {
    V13, V14;

    fun atLeast(major: Int, minor: Int): Boolean {
        val (a, b) = when (this) {
            V13 -> 1 to 13
            V14 -> 1 to 14
        }
        return a > major || (a == major && b >= minor)
    }

    companion object {
        fun fromVersion(version: String?): CoreLine? {
            val (major, minor, _) = parseSemver(version ?: return null) ?: return null
            return if (major > 1 || (major == 1 && minor >= 14)) {
                V14
            } else if (major == 1 && minor >= 13) {
                V13
            } else {
                null
            }
        }

        fun meetsTailscaleCore(version: String?): Boolean {
            return fromVersion(version) != null
        }

        private fun parseSemver(raw: String): Triple<Int, Int, Int>? {
            var s = raw.trim()
            if (s.isEmpty()) return null
            if (s.startsWith("v", ignoreCase = true)) {
                s = s.substring(1)
            }
            val nums = s.split(Regex("[^0-9]+")).filter { it.isNotEmpty() }
            if (nums.size < 2) return null
            val major = nums[0].toIntOrNull() ?: return null
            val minor = nums[1].toIntOrNull() ?: return null
            val patch = nums.getOrNull(2)?.toIntOrNull() ?: 0
            return Triple(major, minor, patch)
        }
    }
}
