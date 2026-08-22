package app.singplane.assemble

import app.singplane.model.ContentKind
import org.json.JSONArray
import org.json.JSONObject
import java.util.regex.Pattern

data class AssembleOptions(
    val include: String = "",
    val exclude: String = "",
    val addSourceTag: Boolean = false,
    val disableDefaultGroups: Boolean = false,
    val keepSourceGroups: Boolean = false,
    val keepSourceDns: Boolean = false,
    val keepSourceRoute: Boolean = false,
)

data class AssembleResult(
    val ok: Boolean,
    val config: JSONObject? = null,
    val detectedKind: ContentKind,
    val warnings: List<String> = emptyList(),
    val error: String? = null,
)

object Assembler {
    private val GROUP_TYPES = setOf("selector", "urltest")
    private val RESERVED_DEFAULT_TAGS = setOf("direct", "block", "dns-out", "dns", "reject")

    fun assemble(
        sourceBody: String,
        templateContent: String,
        options: AssembleOptions = AssembleOptions(),
        patch: RuntimePatch.Options = RuntimePatch.Options(),
        kind: ContentKind = ContentKind.Unknown,
    ): AssembleResult {
        val detected = if (kind == ContentKind.Unknown) {
            ContentDetector.detect(sourceBody)
        } else {
            kind
        }


        val template = runCatching { JSONObject(templateContent) }.getOrElse { e ->
            return AssembleResult(
                ok = false,
                detectedKind = detected,
                error = "模板无效: ${e.message}",
            )
        }

        return when (detected) {
            ContentKind.Singbox -> fromSingbox(sourceBody, template, options, patch, detected)
            ContentKind.Clash -> AssembleResult(
                ok = false,
                detectedKind = detected,
                error = "Clash 转换尚未接入（需要完整 sing-box JSON）",
            )
            ContentKind.UriList -> AssembleResult(
                ok = false,
                detectedKind = detected,
                error = "节点 URI 列表转换尚未接入（需要完整 sing-box JSON）",
            )
            else -> AssembleResult(
                ok = false,
                detectedKind = detected,
                error = "无法识别订阅内容类型（需要完整 sing-box JSON）",
            )
        }
    }

    private fun fromSingbox(
        sourceBody: String,
        template: JSONObject,
        options: AssembleOptions,
        patch: RuntimePatch.Options,
        detected: ContentKind,
    ): AssembleResult {
        val source = runCatching { JSONObject(sourceBody.trim()) }.getOrElse { e ->
            return AssembleResult(
                ok = false,
                detectedKind = detected,
                error = "解析 sing-box JSON 失败: ${e.message}",
            )
        }

        val warnings = mutableListOf<String>()
        val (nodes, groups, endpoints) = extract(source, options, warnings)

        if (nodes.isEmpty()) {
            return AssembleResult(
                ok = false,
                detectedKind = detected,
                warnings = warnings,
                error = "装配失败：提取到 0 个节点",
            )
        }

        val merged = merge(template, nodes, groups, endpoints, options, source)
        val patched = RuntimePatch.apply(merged, patch)

        return AssembleResult(
            ok = true,
            config = patched,
            detectedKind = detected,
            warnings = warnings,
        )
    }

    private fun extract(
        config: JSONObject,
        options: AssembleOptions,
        warnings: MutableList<String>,
    ): Triple<List<JSONObject>, List<JSONObject>, List<JSONObject>> {
        val includePattern = compilePattern(options.include, warnings, "include")
        val excludePattern = compilePattern(options.exclude, warnings, "exclude")

        val nodes = mutableListOf<JSONObject>()
        val groups = mutableListOf<JSONObject>()
        val rawOutbounds = config.optJSONArray("outbounds") ?: JSONArray()

        for (i in 0 until rawOutbounds.length()) {
            val item = rawOutbounds.optJSONObject(i) ?: continue
            val typ = item.optString("type", "")
            val tag = item.optString("tag", "")

            if (tag.isEmpty()) {
                warnings.add("outbound 缺少 tag")
                continue
            }

            if (GROUP_TYPES.contains(typ)) {
                if (options.keepSourceGroups) {
                    groups.add(JSONObject(item.toString()))
                }
                continue
            }

            if (typ == "direct" || typ == "block" || typ == "dns") {
                continue
            }
            if (typ == "relay" && !options.keepSourceGroups) {
                continue
            }

            if (includePattern != null && !includePattern.matcher(tag).find()) {
                continue
            }
            if (excludePattern != null && excludePattern.matcher(tag).find()) {
                continue
            }

            nodes.add(JSONObject(item.toString()))
        }

        val endpoints = mutableListOf<JSONObject>()
        val rawEndpoints = config.optJSONArray("endpoints")
        if (rawEndpoints != null) {
            for (i in 0 until rawEndpoints.length()) {
                val ep = rawEndpoints.optJSONObject(i) ?: continue
                endpoints.add(JSONObject(ep.toString()))
            }
        }

        return Triple(nodes, groups, endpoints)
    }

    private fun merge(
        template: JSONObject,
        nodes: List<JSONObject>,
        groups: List<JSONObject>,
        endpoints: List<JSONObject>,
        options: AssembleOptions,
        source: JSONObject,
    ): JSONObject {
        val cfg = JSONObject(template.toString())

        if (options.keepSourceDns && source.has("dns")) {
            cfg.put("dns", source.get("dns"))
        }

        if (options.keepSourceRoute && source.has("route")) {
            val srcRoute = source.optJSONObject("route")
            if (srcRoute != null) {
                val rules = mutableListOf<Any>()
                val srcRules = srcRoute.optJSONArray("rules")
                if (srcRules != null) {
                    for (i in 0 until srcRules.length()) rules.add(srcRules.get(i))
                }
                val tplRoute = cfg.optJSONObject("route")
                val tplRules = tplRoute?.optJSONArray("rules")
                if (tplRules != null) {
                    for (i in 0 until tplRules.length()) rules.add(tplRules.get(i))
                }
                val mergedRoute = JSONObject(srcRoute.toString())
                mergedRoute.put("rules", JSONArray(rules))
                cfg.put("route", mergedRoute)
            }
        }

        val reserved = RESERVED_DEFAULT_TAGS.toMutableSet()
        val baseOutbounds = mutableListOf<JSONObject>()
        val rawTplOutbounds = cfg.optJSONArray("outbounds")
        if (rawTplOutbounds != null) {
            for (i in 0 until rawTplOutbounds.length()) {
                val obj = rawTplOutbounds.optJSONObject(i) ?: continue
                val tag = obj.optString("tag", "")
                if (tag.isNotEmpty()) reserved.add(tag)
                baseOutbounds.add(obj)
            }
        }

        val used = reserved.toMutableSet()
        val injected = mutableListOf<JSONObject>()
        val nodeTags = mutableListOf<String>()

        for (node in nodes) {
            var tag = node.optString("tag", "")
            if (tag.isEmpty()) continue
            if (used.contains(tag)) {
                tag = uniqueTag(tag, used)
                node.put("tag", tag)
            }
            used.add(tag)
            nodeTags.add(tag)
            injected.add(node)
        }

        val outbounds = mutableListOf<JSONObject>()
        outbounds.addAll(baseOutbounds)
        outbounds.addAll(injected)

        if (options.keepSourceGroups) {
            for (g in groups) {
                var tag = g.optString("tag", "")
                if (tag.isEmpty()) continue
                if (used.contains(tag)) {
                    tag = uniqueTag(tag, used)
                    g.put("tag", tag)
                }
                used.add(tag)
                outbounds.add(g)
            }
        }

        if (!options.disableDefaultGroups && nodeTags.isNotEmpty()) {
            for (out in outbounds) {
                val typ = out.optString("type", "")
                if (GROUP_TYPES.contains(typ)) {
                    val arr = out.optJSONArray("outbounds") ?: JSONArray()
                    val existingTags = (0 until arr.length()).map { arr.optString(it) }.toSet()
                    for (ntag in nodeTags) {
                        if (!existingTags.contains(ntag)) {
                            arr.put(ntag)
                        }
                    }
                    out.put("outbounds", arr)
                }
            }
        }

        cfg.put("outbounds", JSONArray(outbounds))

        if (endpoints.isNotEmpty()) {
            val epArr = cfg.optJSONArray("endpoints") ?: JSONArray()
            for (ep in endpoints) epArr.put(ep)
            cfg.put("endpoints", epArr)
        }

        return cfg
    }

    private fun uniqueTag(base: String, used: Set<String>): String {
        var idx = 2
        while (true) {
            val candidate = "$base-$idx"
            if (!used.contains(candidate)) return candidate
            idx++
        }
    }

    private fun compilePattern(pat: String, warnings: MutableList<String>, label: String): Pattern? {
        val t = pat.trim()
        if (t.isEmpty()) return null
        return runCatching { Pattern.compile(t) }.getOrElse { e ->
            warnings.add("$label 正则无效: ${e.message}")
            null
        }
    }
}
