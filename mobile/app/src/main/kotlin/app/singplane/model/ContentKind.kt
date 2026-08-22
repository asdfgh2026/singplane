package app.singplane.model

/** Content kind names — keep wire format stable. */
enum class ContentKind {
    Unknown,
    Singbox,
    UriList,
    Clash,
    ;

    fun wireName(): String = when (this) {
        Unknown -> "unknown"
        Singbox -> "singbox"
        UriList -> "uriList"
        Clash -> "clash"
    }

    companion object {
        fun fromWire(raw: String?): ContentKind = when (raw) {
            "singbox" -> Singbox
            "uriList" -> UriList
            "clash" -> Clash
            else -> Unknown
        }
    }
}
