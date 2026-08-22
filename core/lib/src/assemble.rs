use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentKind {
    Auto,
    Singbox,
    Clash,
    UriList,
    Unknown,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AssembleOptions {
    pub include: String,
    pub exclude: String,
    pub add_source_tag: bool,
    pub disable_default_groups: bool,
    pub keep_source_groups: bool,
    pub keep_source_dns: bool,
    pub keep_source_route: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PatchOptions {
    pub force_mixed_port: Option<i64>,
    pub force_clash_api: Option<String>,
    pub force_listen_localhost: bool,
    pub strip_tun: bool,
}

impl PatchOptions {
    fn is_noop(&self) -> bool {
        self.force_mixed_port.is_none()
            && self
                .force_clash_api
                .as_ref()
                .map(|s| s.is_empty())
                .unwrap_or(true)
            && !self.force_listen_localhost
            && !self.strip_tun
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub node: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssembleOut {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
    pub detected_kind: ContentKind,
    pub warnings: Vec<Warning>,
    pub stats: Stats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub input_nodes: usize,
    pub merged: usize,
    pub skipped: usize,
}

const GROUP_TYPES: &[&str] = &["selector", "urltest"];

pub fn detect(body: &str) -> ContentKind {
    let text = body.trim();
    if text.is_empty() {
        return ContentKind::Unknown;
    }
    if text.starts_with('{') {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) {
            if map.contains_key("outbounds")
                || map.contains_key("inbounds")
                || map.contains_key("endpoints")
            {
                return ContentKind::Singbox;
            }
        }
    }
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if !lines.is_empty() {
        let uri_like = lines.iter().filter(|l| looks_like_node_uri(l)).count();
        if uri_like >= 1 && uri_like >= lines.len().div_ceil(2) {
            return ContentKind::UriList;
        }
    }
    let lower = text.to_ascii_lowercase();
    if (lower.contains("proxies:")
        || lower.contains("proxy-groups:")
        || lower.contains("proxy-providers:")
        || (regex_is_match(r"(?m)^\s*port:\s*\d+", text) && lower.contains("type:")))
        && (lower.contains("proxies:") || regex_is_match(r"(?m)^\s*-\s*name\s*:", text))
    {
        return ContentKind::Clash;
    }
    ContentKind::Unknown
}

fn regex_is_match(pat: &str, text: &str) -> bool {
    Regex::new(pat).map(|re| re.is_match(text)).unwrap_or(false)
}

fn looks_like_node_uri(line: &str) -> bool {
    let s = line.trim().to_ascii_lowercase();
    const SCHEMES: &[&str] = &[
        "ss://",
        "ssr://",
        "vmess://",
        "vless://",
        "trojan://",
        "hysteria://",
        "hysteria2://",
        "hy2://",
        "tuic://",
        "wireguard://",
        "wg://",
        "anytls://",
        "socks://",
        "http://",
        "https://",
    ];
    for sc in SCHEMES {
        if s.starts_with(sc) {
            if *sc == "http://" || *sc == "https://" {
                return line.contains('@') || line.contains('#');
            }
            return true;
        }
    }
    false
}

pub fn run(
    source_body: &str,
    template_content: &str,
    options: &AssembleOptions,
    patch: &PatchOptions,
    kind: ContentKind,
) -> AssembleOut {
    let detected = if kind == ContentKind::Auto {
        detect(source_body)
    } else {
        kind
    };
    let template: Value = match serde_json::from_str(template_content) {
        Ok(v) => v,
        Err(e) => {
            return AssembleOut {
                ok: false,
                config: None,
                detected_kind: detected,
                warnings: vec![],
                stats: Stats::default(),
                error: Some(format!("模板无效: {e}")),
            };
        }
    };
    match detected {
        ContentKind::Singbox => from_singbox(source_body, template, options, patch, detected),
        ContentKind::Clash => AssembleOut {
            ok: false,
            config: None,
            detected_kind: detected,
            warnings: vec![],
            stats: Stats::default(),
            error: Some("Clash 转换尚未接入（需要本地 convert 服务）".into()),
        },
        ContentKind::UriList => AssembleOut {
            ok: false,
            config: None,
            detected_kind: detected,
            warnings: vec![],
            stats: Stats::default(),
            error: Some("节点 URI 列表转换尚未接入（需要本地 convert 服务）".into()),
        },
        ContentKind::Auto | ContentKind::Unknown => AssembleOut {
            ok: false,
            config: None,
            detected_kind: detected,
            warnings: vec![],
            stats: Stats::default(),
            error: Some(
                "无法识别订阅内容类型（需要完整 sing-box JSON，或后续支持 Clash/URI）".into(),
            ),
        },
    }
}

fn from_singbox(
    source_body: &str,
    template: Value,
    options: &AssembleOptions,
    patch: &PatchOptions,
    detected: ContentKind,
) -> AssembleOut {
    let source: Value = match serde_json::from_str(source_body.trim()) {
        Ok(Value::Object(map)) => Value::Object(map),
        Ok(_) => {
            return AssembleOut {
                ok: false,
                config: None,
                detected_kind: detected,
                warnings: vec![],
                stats: Stats::default(),
                error: Some("sing-box 配置根节点必须是对象".into()),
            };
        }
        Err(e) => {
            return AssembleOut {
                ok: false,
                config: None,
                detected_kind: detected,
                warnings: vec![],
                stats: Stats::default(),
                error: Some(format!("解析 sing-box JSON 失败: {e}")),
            };
        }
    };
    let (nodes, groups, endpoints, warnings) = extract(&source, options);
    let input = nodes.len();
    if input == 0 {
        return AssembleOut {
            ok: false,
            config: None,
            detected_kind: detected,
            warnings,
            stats: Stats {
                input_nodes: 0,
                merged: 0,
                skipped: 0,
            },
            error: Some("装配失败：提取到 0 个节点".into()),
        };
    }
    let merged = merge(template, &nodes, &groups, &endpoints, options, Some(&source));
    let patched = apply_patch(merged, patch);
    AssembleOut {
        ok: true,
        config: Some(patched),
        detected_kind: detected,
        warnings,
        stats: Stats {
            input_nodes: input,
            merged: input,
            skipped: 0,
        },
        error: None,
    }
}

fn extract(
    config: &Value,
    options: &AssembleOptions,
) -> (Vec<Value>, Vec<Value>, Vec<Value>, Vec<Warning>) {
    let mut warnings = Vec::new();
    let include_re = compile_re(&options.include, &mut warnings, "include");
    let exclude_re = compile_re(&options.exclude, &mut warnings, "exclude");
    let mut nodes = Vec::new();
    let mut groups = Vec::new();
    if let Some(raw) = config.get("outbounds").and_then(Value::as_array) {
        for item in raw {
            let Some(m) = item.as_object() else { continue };
            let typ = m.get("type").and_then(Value::as_str).unwrap_or("");
            let tag = m.get("tag").and_then(Value::as_str).unwrap_or("");
            if tag.is_empty() {
                warnings.push(Warning {
                    node: "(no tag)".into(),
                    reason: "outbound 缺少 tag".into(),
                });
                continue;
            }
            if GROUP_TYPES.contains(&typ) {
                if options.keep_source_groups {
                    groups.push(item.clone());
                }
                continue;
            }
            if typ == "direct" || typ == "block" || typ == "dns" {
                continue;
            }
            if typ == "relay" && !options.keep_source_groups {
                continue;
            }
            if let Some(re) = &include_re {
                if !re.is_match(tag) {
                    continue;
                }
            }
            if let Some(re) = &exclude_re {
                if re.is_match(tag) {
                    continue;
                }
            }
            nodes.push(item.clone());
        }
    }
    let mut endpoints = Vec::new();
    if let Some(raw) = config.get("endpoints").and_then(Value::as_array) {
        for item in raw {
            if item.is_object() {
                endpoints.push(item.clone());
            }
        }
    }
    (nodes, groups, endpoints, warnings)
}

fn compile_re(pat: &str, warnings: &mut Vec<Warning>, label: &str) -> Option<Regex> {
    let t = pat.trim();
    if t.is_empty() {
        return None;
    }
    match Regex::new(t) {
        Ok(re) => Some(re),
        Err(e) => {
            warnings.push(Warning {
                node: "*".into(),
                reason: format!("{label} 正则无效: {e}"),
            });
            None
        }
    }
}

fn merge(
    template: Value,
    nodes: &[Value],
    groups: &[Value],
    endpoints: &[Value],
    options: &AssembleOptions,
    source: Option<&Value>,
) -> Value {
    let mut cfg = template;
    if let Some(src) = source {
        if options.keep_source_dns {
            if let Some(dns) = src.get("dns") {
                cfg["dns"] = dns.clone();
            }
        }
        if options.keep_source_route {
            if let Some(src_route) = src.get("route").and_then(Value::as_object) {
                let mut rules = Vec::new();
                if let Some(Value::Array(a)) = src_route.get("rules") {
                    rules.extend(a.iter().cloned());
                }
                if let Some(Value::Array(a)) = cfg.get("route").and_then(|r| r.get("rules")) {
                    rules.extend(a.iter().cloned());
                }
                let mut route = src_route.clone();
                route.remove("rules");
                if let Some(tpl) = cfg.get("route").and_then(Value::as_object) {
                    for (k, v) in tpl {
                        if k != "rules" {
                            route.insert(k.clone(), v.clone());
                        }
                    }
                    if let Some(fin) = tpl.get("final") {
                        route.insert("final".into(), fin.clone());
                    }
                }
                route.insert("rules".into(), Value::Array(rules));
                cfg["route"] = Value::Object(route);
            }
        }
    }

    let mut reserved = vec![
        "direct".into(),
        "block".into(),
        "dns-out".into(),
        "dns".into(),
        "reject".into(),
    ];
    let mut base_outbounds = Vec::new();
    if let Some(raw) = cfg.get("outbounds").and_then(Value::as_array) {
        for item in raw {
            if let Some(tag) = item.get("tag").and_then(Value::as_str) {
                if !tag.is_empty() {
                    reserved.push(tag.to_string());
                }
            }
            base_outbounds.push(item.clone());
        }
    }
    let mut used: std::collections::HashSet<String> = reserved.into_iter().collect();
    let mut injected = Vec::new();
    let mut node_tags = Vec::new();
    for node in nodes {
        let Some(obj) = node.as_object() else { continue };
        let mut m = obj.clone();
        let mut tag = m
            .get("tag")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if tag.is_empty() {
            continue;
        }
        if used.contains(&tag) {
            tag = unique_tag(&tag, &used);
            m.insert("tag".into(), Value::String(tag.clone()));
        }
        used.insert(tag.clone());
        node_tags.push(tag);
        injected.push(Value::Object(m));
    }
    let mut outbounds = base_outbounds;
    outbounds.extend(injected);

    if options.keep_source_groups {
        for g in groups {
            let Some(obj) = g.as_object() else { continue };
            let mut m = obj.clone();
            let mut tag = m
                .get("tag")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if tag.is_empty() {
                continue;
            }
            if used.contains(&tag) {
                tag = unique_tag(&tag, &used);
                m.insert("tag".into(), Value::String(tag.clone()));
            }
            used.insert(tag);
            outbounds.push(Value::Object(m));
        }
    }

    if !options.disable_default_groups && !node_tags.is_empty() {
        let ut = if used.contains("urltest") {
            unique_tag("urltest", &used)
        } else {
            "urltest".into()
        };
        used.insert(ut.clone());
        let sel = if used.contains("select") {
            unique_tag("select", &used)
        } else {
            "select".into()
        };
        used.insert(sel.clone());
        let mut ut_out = vec![json!({"type":"urltest","tag":ut,"interval":"3m","tolerance":50})];
        if let Some(obj) = ut_out[0].as_object_mut() {
            obj.insert(
                "outbounds".into(),
                Value::Array(node_tags.iter().cloned().map(Value::String).collect()),
            );
        }
        let mut sel_outs = vec![Value::String(ut.clone())];
        sel_outs.extend(node_tags.iter().cloned().map(Value::String));
        sel_outs.push(json!("direct"));
        outbounds.push(ut_out.remove(0));
        outbounds.push(json!({
            "type": "selector",
            "tag": sel,
            "outbounds": sel_outs,
            "default": ut,
        }));
        let mut route = cfg
            .get("route")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        route.insert("final".into(), Value::String(sel));
        cfg["route"] = Value::Object(route);
    }
    cfg["outbounds"] = Value::Array(outbounds);

    if !endpoints.is_empty() {
        let mut existing = Vec::new();
        if let Some(raw) = cfg.get("endpoints").and_then(Value::as_array) {
            existing.extend(raw.iter().cloned());
        }
        existing.extend(endpoints.iter().cloned());
        cfg["endpoints"] = Value::Array(existing);
    }
    cfg
}

fn unique_tag(base: &str, used: &std::collections::HashSet<String>) -> String {
    let mut i = 1;
    loop {
        let cand = format!("{base}_{i}");
        if !used.contains(&cand) {
            return cand;
        }
        i += 1;
    }
}

fn apply_patch(config: Value, options: &PatchOptions) -> Value {
    if options.is_noop() {
        return config;
    }
    let mut cfg = config;
    if let Some(raw) = cfg.get("inbounds").and_then(Value::as_array).cloned() {
        let mut inbounds = Vec::new();
        for item in raw {
            let Some(mut m) = item.as_object().cloned() else {
                continue;
            };
            let typ = m
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if options.strip_tun && typ == "tun" {
                continue;
            }
            if typ == "mixed" || typ == "http" || typ == "socks" {
                if let Some(port) = options.force_mixed_port {
                    m.insert("listen_port".into(), json!(port));
                }
                if options.force_listen_localhost {
                    m.insert("listen".into(), json!("127.0.0.1"));
                }
            }
            inbounds.push(Value::Object(m));
        }
        cfg["inbounds"] = Value::Array(inbounds);
    }
    if let Some(ctrl) = options
        .force_clash_api
        .as_ref()
        .filter(|s| !s.is_empty())
    {
        let mut experimental = cfg
            .get("experimental")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let mut clash = experimental
            .get("clash_api")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        clash.insert("external_controller".into(), json!(ctrl));
        experimental.insert("clash_api".into(), Value::Object(clash));
        cfg["experimental"] = Value::Object(experimental);
    }
    cfg
}

#[allow(dead_code)]
fn _map(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_kinds() {
        assert_eq!(
            detect(r#"{"outbounds":[],"inbounds":[]}"#),
            ContentKind::Singbox
        );
        assert_eq!(
            detect("proxies:\n  - name: a\n    type: ss\n    server: 1.1.1.1\n    port: 443\n"),
            ContentKind::Clash
        );
        assert_eq!(
            detect("ss://YWVzLTEyOC1nY206dGVzdA@1.2.3.4:8388#n1\nvmess://eyJ2IjoiMiJ9\n"),
            ContentKind::UriList
        );
        assert_eq!(detect("hello world"), ContentKind::Unknown);
    }

    #[test]
    fn assemble_singbox_with_patch() {
        let template = r#"{
          "inbounds":[{"type":"mixed","tag":"mixed-in","listen":"127.0.0.1","listen_port":2080}],
          "outbounds":[{"type":"direct","tag":"direct"},{"type":"block","tag":"block"},{"type":"dns","tag":"dns-out"}],
          "route":{"final":"direct","rules":[]},
          "dns":{"servers":[{"tag":"local","address":"local"}],"final":"local"},
          "experimental":{"clash_api":{"external_controller":"127.0.0.1:1111"}}
        }"#;
        let source = r#"{
          "inbounds":[{"type":"tun","tag":"tun-in"}],
          "outbounds":[
            {"type":"direct","tag":"direct"},
            {"type":"shadowsocks","tag":"node-a","server":"1.1.1.1","server_port":443,"method":"aes-128-gcm","password":"p"}
          ],
          "route":{"final":"node-a"}
        }"#;
        let out = run(
            source,
            template,
            &AssembleOptions::default(),
            &PatchOptions {
                force_mixed_port: Some(7890),
                force_clash_api: Some("127.0.0.1:9090".into()),
                force_listen_localhost: true,
                strip_tun: true,
            },
            ContentKind::Auto,
        );
        assert!(out.ok, "{:?}", out.error);
        assert_eq!(out.stats.input_nodes, 1);
        let cfg = out.config.unwrap();
        let mixed = cfg["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["type"] == "mixed")
            .unwrap();
        assert_eq!(mixed["listen_port"], 7890);
        assert_eq!(
            cfg["experimental"]["clash_api"]["external_controller"],
            "127.0.0.1:9090"
        );
        let tags: Vec<&str> = cfg["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|o| o["tag"].as_str())
            .collect();
        assert!(tags.contains(&"node-a"));
        assert!(tags.contains(&"select"));
    }

    #[test]
    fn zero_nodes_fails() {
        let out = run(
            r#"{"outbounds":[{"type":"direct","tag":"direct"}]}"#,
            r#"{"outbounds":[{"type":"direct","tag":"direct"}]}"#,
            &AssembleOptions::default(),
            &PatchOptions::default(),
            ContentKind::Auto,
        );
        assert!(!out.ok);
        assert!(out.error.unwrap().contains('0'));
    }
}
