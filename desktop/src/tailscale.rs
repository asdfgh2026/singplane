//! App-level Tailscale overlay for GPUI.
//! Tailscale overlay fields + log/status hints.
//! Profile JSON on disk is never rewritten.

use std::fs;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::host::{app_root, core_log_path, read_core_log_tail};
use crate::store::{settings_bool, settings_str};

const LOG_TAIL: usize = 256 * 1024;

#[derive(Clone, Debug)]
pub struct TailscaleSettings {
    pub enabled: bool,
    pub tag: String,
    pub auth_key: String,
    pub control_url: String,
    pub hostname: String,
    pub state_directory: String,
    pub accept_routes: bool,
    pub advertise_exit_node: bool,
    pub exit_node_allow_lan_access: bool,
    pub exit_node: String,
    pub advertise_routes: String,
    pub advertise_tags: String,
    pub system_interface: bool,
    pub ssh_server: bool,
    pub replace_other: bool,
    pub inject_dns: bool,
    pub accept_default_resolvers: bool,
    pub accept_search_domain: bool,
    pub inject_route_preferred_by: bool,
    pub route_domain_suffix: String,
    pub route_ip_cidr: String,
}

impl TailscaleSettings {
    pub fn from_settings(settings: &Value) -> Self {
        let ts = settings.get("tailscale").cloned().unwrap_or_else(|| json!({}));
        let tag = settings_str(&ts, "tag");
        Self {
            enabled: settings_bool(&ts, "enabled", false),
            tag: if tag.is_empty() {
                "ts-local".into()
            } else {
                tag
            },
            auth_key: settings_str(&ts, "authKey"),
            control_url: settings_str(&ts, "controlUrl"),
            hostname: settings_str(&ts, "hostname"),
            state_directory: settings_str(&ts, "stateDirectory"),
            accept_routes: settings_bool(&ts, "acceptRoutes", true),
            advertise_exit_node: settings_bool(&ts, "advertiseExitNode", false),
            exit_node_allow_lan_access: settings_bool(&ts, "exitNodeAllowLanAccess", false),
            exit_node: settings_str(&ts, "exitNode"),
            advertise_routes: settings_str(&ts, "advertiseRoutes"),
            advertise_tags: settings_str(&ts, "advertiseTags"),
            system_interface: settings_bool(&ts, "systemInterface", false),
            ssh_server: settings_bool(&ts, "sshServer", false),
            replace_other: settings_bool(&ts, "replaceOtherTailscale", true),
            inject_dns: settings_bool(&ts, "injectDns", true),
            accept_default_resolvers: settings_bool(&ts, "acceptDefaultResolvers", false),
            accept_search_domain: settings_bool(&ts, "acceptSearchDomain", true),
            inject_route_preferred_by: settings_bool(&ts, "injectRoutePreferredBy", true),
            route_domain_suffix: {
                let s = settings_str(&ts, "routeDomainSuffix");
                if s.is_empty() {
                    ".ts.net".into()
                } else {
                    s
                }
            },
            route_ip_cidr: settings_str(&ts, "routeIpCidr"),
        }
    }

    pub fn resolved_tag(&self) -> &str {
        if self.tag.trim().is_empty() {
            "ts-local"
        } else {
            self.tag.trim()
        }
    }

    pub fn resolved_dns_tag(&self) -> String {
        format!("{}-dns", self.resolved_tag())
    }

    /// Empty auth key → device login URL, not pre-auth.
    pub fn uses_device_auth(&self) -> bool {
        self.auth_key.trim().is_empty()
    }

    pub fn resolved_state_dir(&self) -> PathBuf {
        let configured = self.state_directory.trim();
        if !configured.is_empty() {
            return PathBuf::from(configured);
        }
        app_root().join("runtime").join("tailscale")
    }

    fn split_list(raw: &str) -> Vec<String> {
        raw.split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn to_endpoint_json(&self, line: CoreLine) -> Value {
        let mut ep = Map::new();
        ep.insert("type".into(), json!("tailscale"));
        ep.insert("tag".into(), json!(self.resolved_tag()));
        if !self.auth_key.trim().is_empty() {
            ep.insert("auth_key".into(), json!(self.auth_key.trim()));
        }
        if !self.control_url.trim().is_empty() {
            ep.insert("control_url".into(), json!(self.control_url.trim()));
        }
        if !self.hostname.trim().is_empty() {
            ep.insert("hostname".into(), json!(self.hostname.trim()));
        }
        let sd = self.resolved_state_dir();
        ep.insert("state_directory".into(), json!(sd.to_string_lossy()));
        if self.accept_routes {
            ep.insert("accept_routes".into(), json!(true));
        }
        if self.advertise_exit_node {
            ep.insert("advertise_exit_node".into(), json!(true));
        }
        if self.exit_node_allow_lan_access {
            ep.insert("exit_node_allow_lan_access".into(), json!(true));
        }
        if !self.exit_node.trim().is_empty() {
            ep.insert("exit_node".into(), json!(self.exit_node.trim()));
        }
        let routes = Self::split_list(&self.advertise_routes);
        if !routes.is_empty() {
            ep.insert("advertise_routes".into(), json!(routes));
        }
        // advertise_tags / system_interface: since 1.13
        if line.at_least(1, 13) {
            let tags = Self::split_list(&self.advertise_tags);
            if !tags.is_empty() {
                ep.insert("advertise_tags".into(), json!(tags));
            }
            if self.system_interface {
                ep.insert("system_interface".into(), json!(true));
            }
        }
        // ssh_server: since 1.14
        if line.at_least(1, 14) && self.ssh_server {
            ep.insert("ssh_server".into(), json!(true));
        }
        Value::Object(ep)
    }
}

/// Overlay Tailscale onto a profile config. Does not persist the profile.
///
/// Official floor is **1.13**. DNS `preferred_by` /
/// `accept_search_domain` are 1.14-only — 1.13 uses `ip_accept_any`.
pub fn with_tailscale(user_config: &Value, ts: &TailscaleSettings) -> Value {
    with_tailscale_for(user_config, ts, CoreLine::V14)
}

pub fn with_tailscale_for(
    user_config: &Value,
    ts: &TailscaleSettings,
    line: CoreLine,
) -> Value {
    let mut cfg = user_config.clone();
    if !ts.enabled {
        return cfg;
    }
    if !cfg.is_object() {
        return cfg;
    }
    ensure_direct_outbound(&mut cfg);
    ensure_bootstrap_dns(&mut cfg);
    inject_endpoint(&mut cfg, ts, line, BOOTSTRAP_DNS_TAG);
    if ts.inject_dns {
        inject_dns(&mut cfg, ts, line, BOOTSTRAP_DNS_TAG);
    }
    if ts.inject_route_preferred_by
        || !TailscaleSettings::split_list(&ts.route_domain_suffix).is_empty()
        || !TailscaleSettings::split_list(&ts.route_ip_cidr).is_empty()
    {
        inject_route(&mut cfg, ts, line);
    }
    ensure_remote_dns_detour(&mut cfg);
    cfg
}

const BOOTSTRAP_DNS_TAG: &str = "ts-bootstrap";

fn outbound_tag(o: &Value) -> Option<&str> {
    o.get("tag").and_then(Value::as_str)
}

fn outbound_type(o: &Value) -> &str {
    o.get("type").and_then(Value::as_str).unwrap_or("")
}

fn is_dead_end_outbound(o: &Value) -> bool {
    matches!(
        outbound_type(o),
        "direct" | "block" | "dns" | "blackhole"
    ) || outbound_tag(o).is_some_and(|tag| {
        matches!(
            tag.to_ascii_lowercase().as_str(),
            "direct" | "block" | "dns" | "reject" | "blackhole"
        )
    })
}

fn is_group_outbound(o: &Value) -> bool {
    matches!(outbound_type(o), "selector" | "urltest")
}

fn find_outbound<'a>(arr: &'a [Value], tag: &str) -> Option<&'a Value> {
    arr.iter().find(|o| outbound_tag(o) == Some(tag))
}

/// Detour for remote DNS / `ts-bootstrap` on the *current* profile.
///
/// Recomputed on every overlay (profile switch / core start). Does not
/// assume Clash tags `proxy` / `auto` — other subscriptions use
/// `Proxy`, `节点选择`, or a single node as `route.final`.
pub(crate) fn proxy_detour_tag(cfg: &Value) -> Option<String> {
    let arr = cfg.get("outbounds")?.as_array()?;
    if let Some(final_tag) = cfg
        .get("route")
        .and_then(|r| r.get("final"))
        .and_then(Value::as_str)
    {
        if let Some(ob) = find_outbound(arr, final_tag) {
            if !is_dead_end_outbound(ob) {
                return Some(final_tag.to_string());
            }
        }
    }
    arr.iter()
        .find(|o| is_group_outbound(o) && !is_dead_end_outbound(o))
        .and_then(outbound_tag)
        .map(str::to_string)
        .or_else(|| {
            arr.iter()
                .find(|o| !is_dead_end_outbound(o))
                .and_then(outbound_tag)
                .map(str::to_string)
        })
}

fn ensure_bootstrap_dns(cfg: &mut Value) {
    let mut dns = cfg
        .get("dns")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut servers = dns
        .get("servers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    servers.retain(|s| s.get("tag").and_then(Value::as_str) != Some(BOOTSTRAP_DNS_TAG));
    let mut server = json!({
        "type": "https",
        "tag": BOOTSTRAP_DNS_TAG,
        "server": "1.1.1.1",
    });
    // Cloudflare DoH without detour dials 1.1.1.1 on the TUN interface and times out.
    if let Some(detour) = proxy_detour_tag(cfg) {
        server["detour"] = json!(detour);
    }
    servers.insert(0, server);
    dns.insert("servers".into(), Value::Array(servers));
    cfg["dns"] = Value::Object(dns);
}

/// SOCKS5h / TUN DNS uses `dns.final` (often Cloudflare DoH). Without
/// `detour` that dials 1.1.1.1 on the physical NIC and times out in CN,
/// so Terminal follows system SOCKS but never resolves.
pub(crate) fn ensure_remote_dns_detour(cfg: &mut Value) {
    let Some(detour) = proxy_detour_tag(cfg) else {
        return;
    };
    let Some(servers) = cfg
        .get_mut("dns")
        .and_then(|d| d.get_mut("servers"))
        .and_then(|s| s.as_array_mut())
    else {
        return;
    };
    for server in servers {
        let typ = server.get("type").and_then(Value::as_str).unwrap_or("");
        if !matches!(typ, "https" | "h3" | "http3" | "tls" | "quic") {
            continue;
        }
        if server
            .get("detour")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
        {
            continue;
        }
        server["detour"] = json!(detour.clone());
    }
}

fn inject_endpoint(cfg: &mut Value, ts: &TailscaleSettings, line: CoreLine, resolver: &str) {
    let tag = ts.resolved_tag().to_string();
    let mut list = Vec::new();
    if let Some(raw) = cfg.get("endpoints").and_then(Value::as_array) {
        for item in raw {
            let Some(m) = item.as_object() else { continue };
            let typ = m.get("type").and_then(Value::as_str).unwrap_or("");
            let tg = m.get("tag").and_then(Value::as_str).unwrap_or("");
            if tg == tag {
                continue;
            }
            if ts.replace_other && typ == "tailscale" {
                continue;
            }
            list.push(item.clone());
        }
    }
    let mut ep = ts.to_endpoint_json(line);
    if let Some(obj) = ep.as_object_mut() {
        // Do not use system/local DNS: TUN hijack-dns loops on "local".
        // sing-box 1.14: strategy lives on domain_resolver; sibling
        // domain_strategy is a hard FATAL unless a deprecated env is set.
        obj.insert(
            "domain_resolver".into(),
            json!({
                "server": resolver,
                "strategy": "ipv4_only",
            }),
        );
    }
    list.push(ep);
    cfg["endpoints"] = Value::Array(list);
}

fn ensure_direct_outbound(cfg: &mut Value) {
    let mut list = cfg
        .get("outbounds")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has_direct = list.iter().any(|o| {
        o.get("tag").and_then(Value::as_str) == Some("direct")
            && o.get("type").and_then(Value::as_str) == Some("direct")
    });
    if !has_direct {
        list.push(json!({"type": "direct", "tag": "direct"}));
        cfg["outbounds"] = Value::Array(list);
    }
}

fn inject_dns(cfg: &mut Value, ts: &TailscaleSettings, line: CoreLine, resolver: &str) {
    let dns_tag = ts.resolved_dns_tag();
    let mut dns = cfg
        .get("dns")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut servers = Vec::new();
    if let Some(raw) = dns.get("servers").and_then(Value::as_array) {
        for item in raw {
            let Some(m) = item.as_object() else { continue };
            let typ = m.get("type").and_then(Value::as_str).unwrap_or("");
            let tg = m.get("tag").and_then(Value::as_str).unwrap_or("");
            let ep = m.get("endpoint").and_then(Value::as_str).unwrap_or("");
            if tg == dns_tag {
                continue;
            }
            if ts.replace_other && typ == "tailscale" {
                continue;
            }
            if typ == "tailscale" && ep == ts.resolved_tag() {
                continue;
            }
            servers.push(item.clone());
        }
    }
    let mut server = Map::new();
    server.insert("type".into(), json!("tailscale"));
    server.insert("tag".into(), json!(dns_tag));
    server.insert("endpoint".into(), json!(ts.resolved_tag()));
    if ts.accept_default_resolvers {
        server.insert("accept_default_resolvers".into(), json!(true));
    }
    // accept_search_domain: since 1.14
    if line.at_least(1, 14) && ts.accept_search_domain {
        server.insert("accept_search_domain".into(), json!(true));
    }
    servers.push(Value::Object(server));
    dns.insert("servers".into(), Value::Array(servers));

    let suffixes = {
        let s = TailscaleSettings::split_list(&ts.route_domain_suffix);
        if s.is_empty() {
            vec![".ts.net".into()]
        } else {
            s
        }
    };
    // Official Tailscale DNS: preferred_by (1.14+) plus MagicDNS suffix.
    // Never add a bare `ip_accept_any` rule — that matches every query and
    // swallows fake-ip / TUN / CLI traffic. Tailscale + TUN coexist when
    // only .ts.net / preferred names go to ts-local-dns.
    // Resolve control-plane names via local/system DNS, not MagicDNS / proxy.
    let mut rules = vec![json!({
        "domain_suffix": ["tailscale.com", "tailscale.io"],
        "action": "route",
        "server": resolver,
    })];
    if line.at_least(1, 14) {
        rules.push(json!({"preferred_by": [dns_tag.as_str()], "action": "route", "server": dns_tag.as_str()}));
        rules.push(json!({"domain_suffix": suffixes, "action": "route", "server": dns_tag.as_str()}));
    } else {
        rules.push(json!({"domain_suffix": suffixes, "server": dns_tag.as_str()}));
    }
    if let Some(raw) = dns.get("rules").and_then(Value::as_array) {
        for item in raw {
            let Some(m) = item.as_object() else { continue };
            if preferred_contains(m.get("preferred_by"), &dns_tag) {
                continue;
            }
            if m.get("server").and_then(Value::as_str) == Some(dns_tag.as_str()) {
                continue;
            }
            rules.push(item.clone());
        }
    }
    dns.insert("rules".into(), Value::Array(rules));
    cfg["dns"] = Value::Object(dns);
}

fn inject_route(cfg: &mut Value, ts: &TailscaleSettings, line: CoreLine) {
    let tag = ts.resolved_tag().to_string();
    let mut route = cfg
        .get("route")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut rules = Vec::new();
    if ts.inject_route_preferred_by && line.at_least(1, 14) {
        rules.push(json!({"preferred_by": [tag], "outbound": tag}));
    }
    let suffixes = TailscaleSettings::split_list(&ts.route_domain_suffix);
    if !suffixes.is_empty() {
        rules.push(json!({"domain_suffix": suffixes, "outbound": tag}));
    }
    let cidrs = TailscaleSettings::split_list(&ts.route_ip_cidr);
    if !cidrs.is_empty() {
        rules.push(json!({"ip_cidr": cidrs, "outbound": tag}));
    }
    if let Some(raw) = route.get("rules").and_then(Value::as_array) {
        for item in raw {
            let Some(m) = item.as_object() else { continue };
            if preferred_contains(m.get("preferred_by"), &tag)
                && m.get("outbound").and_then(Value::as_str) == Some(tag.as_str())
            {
                continue;
            }
            rules.push(item.clone());
        }
    }
    route.insert("rules".into(), Value::Array(rules));
    cfg["route"] = Value::Object(route);
}

fn preferred_contains(v: Option<&Value>, tag: &str) -> bool {
    match v {
        Some(Value::String(s)) => s == tag,
        Some(Value::Array(arr)) => arr.iter().any(|x| x.as_str() == Some(tag)),
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TsPhase {
    Disabled,
    Ready,
    /// Core running, no 100.x / login URL yet (key or browser auth in flight).
    Pending,
    /// Local login leftover, but control plane is not live (admin shows offline).
    Offline,
    Injected,
    NeedsLogin,
    Error,
}

#[derive(Clone, Debug)]
pub struct TsStatus {
    pub phase: TsPhase,
    pub title: SharedLike,
    pub subtitle: SharedLike,
    pub login_url: Option<String>,
    pub self_ip: Option<String>,
    pub hostname: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SharedLike(pub String);

impl From<&str> for SharedLike {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for SharedLike {
    fn from(s: String) -> Self {
        Self(s)
    }
}

pub fn status_view(ts: &TailscaleSettings, running: bool) -> TsStatus {
    if !ts.enabled {
        return TsStatus {
            phase: TsPhase::Disabled,
            title: "未启用".into(),
            subtitle: "".into(),
            login_url: None,
            self_ip: None,
            hostname: None,
        };
    }
    if !running {
        return TsStatus {
            phase: TsPhase::Ready,
            title: "已开启".into(),
            subtitle: "启动后生效".into(),
            login_url: None,
            self_ip: None,
            hostname: None,
        };
    }

    let log = read_core_log_tail(LOG_TAIL);
    let hint = latest_hint(&log);
    let ident = discover_self(ts);
    // A leftover LoggedOut=false / NodeID is not "online". Admin only
    // lights up after a live 100.x or MagicDNS hosts > 0.
    let live = ident.ip.is_some()
        || matches!(hint.kind, HintKind::Connected);
    if live {
        return TsStatus {
            phase: TsPhase::Injected,
            title: "已加入".into(),
            subtitle: ident
                .ip
                .clone()
                .or_else(|| ident.hostname.clone())
                .unwrap_or_default()
                .into(),
            login_url: None,
            self_ip: ident.ip,
            hostname: ident.hostname,
        };
    }
    if ident.joined() {
        return TsStatus {
            phase: TsPhase::Offline,
            title: "未在线".into(),
            subtitle: ident.hostname.clone().unwrap_or_default().into(),
            login_url: hint.login_url,
            self_ip: None,
            hostname: ident.hostname,
        };
    }

    match hint.kind {
        HintKind::WaitingAuth => TsStatus {
            phase: TsPhase::NeedsLogin,
            title: "等待授权".into(),
            subtitle: ident.hostname.clone().unwrap_or_default().into(),
            login_url: hint.login_url,
            self_ip: None,
            hostname: ident.hostname,
        },
        HintKind::Error => TsStatus {
            phase: TsPhase::Error,
            title: "出错".into(),
            subtitle: ident.hostname.clone().unwrap_or_default().into(),
            login_url: None,
            self_ip: None,
            hostname: ident.hostname,
        },
        HintKind::Connected => TsStatus {
            phase: TsPhase::Injected,
            title: "已加入".into(),
            subtitle: ident
                .ip
                .clone()
                .or_else(|| ident.hostname.clone())
                .unwrap_or_default()
                .into(),
            login_url: None,
            self_ip: ident.ip,
            hostname: ident.hostname,
        },
        HintKind::None => pending_status(ts, hint.login_url, ident.hostname),
    }
}

fn pending_status(ts: &TailscaleSettings, login_url: Option<String>, hostname: Option<String>) -> TsStatus {
    if let Some(url) = login_url {
        return TsStatus {
            phase: TsPhase::NeedsLogin,
            title: "等待授权".into(),
            subtitle: hostname.clone().unwrap_or_default().into(),
            login_url: Some(url),
            self_ip: None,
            hostname,
        };
    }
    if ts.uses_device_auth() {
        TsStatus {
            phase: TsPhase::Pending,
            title: "验证中".into(),
            subtitle: hostname.clone().unwrap_or_default().into(),
            login_url: None,
            self_ip: None,
            hostname,
        }
    } else {
        TsStatus {
            phase: TsPhase::Pending,
            title: "验证中".into(),
            subtitle: hostname.clone().unwrap_or_default().into(),
            login_url: None,
            self_ip: None,
            hostname,
        }
    }
}

struct Hint {
    kind: HintKind,
    login_url: Option<String>,
    detail: Option<String>,
}

enum HintKind {
    Connected,
    WaitingAuth,
    Error,
    None,
}

fn latest_hint(log: &str) -> Hint {
    if log.is_empty() {
        return Hint {
            kind: HintKind::None,
            login_url: None,
            detail: None,
        };
    }
    let lines: Vec<&str> = log.lines().collect();
    for line in lines.iter().rev() {
        let lower = line.to_ascii_lowercase();
        let is_ts = lower.contains("tailscale")
            || lower.contains("ts-local")
            || lower.contains("ts-local-dns");
        if !is_ts {
            continue;
        }
        if let Some(detail) = magicdns_join_detail(&lower, line) {
            return Hint {
                kind: HintKind::Connected,
                login_url: None,
                detail: Some(detail),
            };
        }
        if lower.contains("backend: running")
            || lower.contains("switching ipn state to running")
            || lower.contains("logged in")
            || lower.contains("connected to control")
        {
            return Hint {
                kind: HintKind::Connected,
                login_url: None,
                detail: Some("已加入 Tailscale".into()),
            };
        }
        if lower.contains("waiting for authentication") {
            return Hint {
                kind: HintKind::WaitingAuth,
                login_url: url_from_line(line),
                detail: None,
            };
        }
        if lower.contains("endpoint/tailscale")
            && (lower.contains("error")
                || lower.contains("failed")
                || lower.contains("fatal")
                || lower.contains("denied"))
        {
            return Hint {
                kind: HintKind::Error,
                login_url: None,
                detail: Some(line.trim().chars().take(120).collect()),
            };
        }
    }
    Hint {
        kind: HintKind::None,
        login_url: None,
        detail: None,
    }
}

/// `updated 0 routes, 0 hosts` is just MagicDNS starting empty — not a join.
fn magicdns_join_detail(lower: &str, line: &str) -> Option<String> {
    if !lower.contains("updated") {
        return None;
    }
    if !(lower.contains("routes") || lower.contains("hosts") || lower.contains("search domain")) {
        return None;
    }
    let hosts = capture_count(lower, "hosts");
    // Routes-only (e.g. "67 routes, 0 hosts") is not a live node on the tailnet.
    if hosts.unwrap_or(0) == 0 {
        return None;
    }
    Some(
        line.split("updated")
            .nth(1)
            .map(|s| format!("MagicDNS · {}", s.trim()))
            .unwrap_or_else(|| "已加入 Tailscale".into()),
    )
}

fn capture_count(lower: &str, label: &str) -> Option<u32> {
    let needle = format!(" {label}");
    let idx = lower.find(&needle)?;
    let before = &lower[..idx];
    let num = before
        .rsplit(|c: char| !c.is_ascii_digit())
        .find(|s| !s.is_empty())?;
    num.parse().ok()
}

fn url_anywhere(log: &str) -> Option<String> {
    for line in log.lines().rev() {
        if let Some(u) = url_from_line(line) {
            return Some(u);
        }
    }
    None
}

fn url_from_line(line: &str) -> Option<String> {
    if let Some(idx) = line.to_ascii_lowercase().find("https://login.tailscale.com/") {
        let rest = &line[idx..];
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ')' | ']' | ',' | ';'))
            .unwrap_or(rest.len());
        return Some(rest[..end].trim_end_matches(['.', ',', ';']).to_string());
    }
    if let Some(idx) = line.to_ascii_lowercase().find("waiting for authentication:") {
        let rest = line[idx..].split_whitespace().nth(3)?;
        if rest.starts_with("http") {
            return Some(rest.trim_end_matches(['.', ',', ';']).to_string());
        }
    }
    None
}

struct SelfIdent {
    ip: Option<String>,
    hostname: Option<String>,
    logged_in: bool,
}

impl SelfIdent {
    fn joined(&self) -> bool {
        self.logged_in || self.ip.is_some()
    }

    fn label(&self) -> String {
        self.ip
            .clone()
            .or_else(|| self.hostname.clone())
            .unwrap_or_else(|| "已加入 Tailscale".into())
    }
}

fn discover_self(ts: &TailscaleSettings) -> SelfIdent {
    let mut hostname = None;
    let mut ip = list_tailscale_ips().into_iter().next();
    let mut logged_in = false;
    for dir in state_candidates(ts) {
        if let Some(info) = read_netmap_identity(&dir) {
            if info.0.is_some() || info.1.is_some() {
                logged_in = true;
            }
            if hostname.is_none() {
                hostname = info.0;
            }
            if ip.is_none() {
                ip = info.1;
            }
        }
        if let Some(st) = read_state_identity(&dir) {
            if hostname.is_none() {
                hostname = st.hostname;
            }
            if ip.is_none() {
                ip = st.ip;
            }
            logged_in = logged_in || st.logged_in;
        }
        if hostname.is_some() && ip.is_some() {
            break;
        }
    }
    SelfIdent {
        ip,
        hostname,
        logged_in,
    }
}

/// Userspace Tailscale (no utun) writes the node to `profile-data/*/netmap-cache/self`.
fn read_netmap_identity(dir: &Path) -> Option<(Option<String>, Option<String>)> {
    let root = dir.join("profile-data");
    let profiles = fs::read_dir(&root).ok()?;
    for prof in profiles.flatten() {
        let cache = prof.path().join("netmap-cache");
        let self_path = cache.join("73656c66"); // hex("self")
        if let Ok(text) = fs::read_to_string(&self_path) {
            if let Some(id) = parse_netmap_self(&text) {
                return Some(id);
            }
        }
        let Ok(files) = fs::read_dir(&cache) else {
            continue;
        };
        for file in files.flatten() {
            let Ok(text) = fs::read_to_string(file.path()) else {
                continue;
            };
            if let Some(id) = parse_netmap_self(&text) {
                return Some(id);
            }
        }
    }
    None
}

fn parse_netmap_self(text: &str) -> Option<(Option<String>, Option<String>)> {
    let v: Value = serde_json::from_str(text).ok()?;
    let node = v.get("Node")?;
    let name = node
        .get("Name")
        .and_then(Value::as_str)
        .map(|s| s.trim().trim_end_matches('.').to_string())
        .filter(|s| !s.is_empty());
    let mut ip = None;
    if let Some(arr) = node.get("Addresses").and_then(Value::as_array) {
        for item in arr {
            let Some(s) = item.as_str() else {
                continue;
            };
            let host = s.split('/').next().unwrap_or(s);
            if is_tailscale_ip(host) {
                ip = Some(host.to_string());
                break;
            }
        }
    }
    if name.is_none() && ip.is_none() {
        None
    } else {
        Some((name, ip))
    }
}

fn state_candidates(ts: &TailscaleSettings) -> Vec<PathBuf> {
    let mut dirs = vec![ts.resolved_state_dir()];
    dirs.push(app_root().join("tailscale"));
    dirs.push(app_root().join("runtime").join("tailscale"));
    dirs
}

fn read_state_identity(dir: &Path) -> Option<SelfIdent> {
    let path = dir.join("tailscaled.state");
    let raw = fs::read_to_string(path).ok()?;
    parse_tailscaled_state(&raw)
}

fn parse_tailscaled_state(raw: &str) -> Option<SelfIdent> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let mut blobs = vec![v.clone()];
    if let Some(obj) = v.as_object() {
        for val in obj.values() {
            if let Some(s) = val.as_str() {
                if let Some(decoded) = decode_b64_json(s) {
                    blobs.push(decoded);
                }
            }
        }
    }
    let mut hostname = None;
    let mut display = None;
    let mut ip = None;
    let mut logged_out = None;
    let mut has_user = false;
    for blob in &blobs {
        walk_state_json(blob, &mut hostname, &mut display, &mut ip, &mut logged_out, &mut has_user, 0);
    }
    let logged_in = logged_out == Some(false) && (has_user || display.is_some() || hostname.is_some());
    let hostname = display.or(hostname);
    if !logged_in && hostname.is_none() && ip.is_none() {
        return None;
    }
    Some(SelfIdent {
        ip,
        hostname,
        logged_in,
    })
}

fn decode_b64_json(value: &str) -> Option<Value> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with('{') || trimmed.starts_with('[') {
        return None;
    }
    let bytes = decode_b64(trimmed)?;
    let text = String::from_utf8(bytes).ok()?;
    let t = text.trim();
    if !t.starts_with('{') {
        return None;
    }
    serde_json::from_str(t).ok()
}

fn decode_b64(input: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let a = val(*chunk.first()?)?;
        let b = val(*chunk.get(1)?)?;
        out.push((a << 2) | (b >> 4));
        if let Some(c) = chunk.get(2).copied().and_then(val) {
            out.push((b << 4) | (c >> 2));
            if let Some(d) = chunk.get(3).copied().and_then(val) {
                out.push((c << 6) | d);
            }
        }
    }
    Some(out)
}

fn walk_state_json(
    v: &Value,
    hostname: &mut Option<String>,
    display: &mut Option<String>,
    ip: &mut Option<String>,
    logged_out: &mut Option<bool>,
    has_user: &mut bool,
    depth: u8,
) {
    if depth > 8 {
        return;
    }
    match v {
        Value::Object(map) => {
            if logged_out.is_none() {
                if let Some(b) = map.get("LoggedOut").and_then(Value::as_bool) {
                    *logged_out = Some(b);
                }
            }
            if display.is_none() {
                if let Some(s) = map.get("DisplayName").and_then(Value::as_str) {
                    let t = s.trim();
                    if !t.is_empty() && !t.starts_with("http") {
                        *display = Some(t.to_string());
                    }
                }
            }
            if hostname.is_none() {
                for key in ["LoginName", "MagicDNSName", "HostName"] {
                    if let Some(s) = map.get(key).and_then(Value::as_str) {
                        let t = s.trim().trim_end_matches('.');
                        if !t.is_empty() && t != "localhost" && !t.starts_with("http") {
                            *hostname = Some(t.to_string());
                            break;
                        }
                    }
                }
            }
            if map.contains_key("UserProfile") || map.contains_key("NodeID") || map.contains_key("LoginName") {
                *has_user = true;
            }
            if ip.is_none() {
                for key in ["Addresses", "addresses", "TailscaleIPs"] {
                    if let Some(arr) = map.get(key).and_then(Value::as_array) {
                        for item in arr {
                            if let Some(s) = item.as_str() {
                                let host = s.split('/').next().unwrap_or(s);
                                if is_tailscale_ip(host) {
                                    *ip = Some(host.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            for val in map.values() {
                walk_state_json(val, hostname, display, ip, logged_out, has_user, depth + 1);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                walk_state_json(item, hostname, display, ip, logged_out, has_user, depth + 1);
            }
        }
        _ => {}
    }
}

fn walk_json(v: &Value, hostname: &mut Option<String>, ip: &mut Option<String>, depth: u8) {
    if depth > 8 {
        return;
    }
    match v {
        Value::Object(map) => {
            if hostname.is_none() {
                for key in ["Name", "HostName", "hostname", "DNSName"] {
                    if let Some(s) = map.get(key).and_then(Value::as_str) {
                        let t = s.trim().trim_end_matches('.');
                        if !t.is_empty() {
                            *hostname = Some(t.to_string());
                            break;
                        }
                    }
                }
            }
            if ip.is_none() {
                for key in ["Addresses", "addresses", "TailscaleIPs"] {
                    if let Some(arr) = map.get(key).and_then(Value::as_array) {
                        for item in arr {
                            if let Some(s) = item.as_str() {
                                let host = s.split('/').next().unwrap_or(s);
                                if is_tailscale_ip(host) {
                                    *ip = Some(host.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            for val in map.values() {
                walk_json(val, hostname, ip, depth + 1);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                walk_json(item, hostname, ip, depth + 1);
            }
        }
        Value::String(s) => {
            if hostname.is_none() && s.contains(".ts.net") {
                *hostname = Some(s.trim().trim_end_matches('.').to_string());
            }
        }
        _ => {}
    }
}

pub fn is_tailscale_ip(ip: &str) -> bool {
    let mut it = ip.split('.');
    let a = it.next().and_then(|s| s.parse::<u32>().ok());
    let b = it.next().and_then(|s| s.parse::<u32>().ok());
    matches!((a, b), (Some(100), Some(b)) if (64..=127).contains(&b))
}

pub fn list_tailscale_ips() -> Vec<String> {
    let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(local) = {
        let _ = sock.connect("1.1.1.1:80");
        sock.local_addr()
    } {
        let ip = local.ip().to_string();
        if is_tailscale_ip(&ip) {
            out.push(ip);
        }
    }
    // /proc/net/fib_trie is noisy; parse `ip -4 addr` if present.
    if let Ok(text) = ipv4_addr_output() {
        for token in text.split_whitespace() {
            let host = token.split('/').next().unwrap_or(token);
            if is_tailscale_ip(host) && !out.iter().any(|x| x == host) {
                out.push(host.to_string());
            }
        }
    }
    out
}

fn ipv4_addr_output() -> Result<String, ()> {
    if let Ok(out) = std::process::Command::new("ip")
        .args(["-4", "-o", "addr", "show"])
        .output()
    {
        if out.status.success() && !out.stdout.is_empty() {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
    }
    let out = std::process::Command::new("ifconfig")
        .arg("-a")
        .output()
        .map_err(|_| ())?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn ensure_state_dir(ts: &TailscaleSettings) -> Result<PathBuf, String> {
    let dir = ts.resolved_state_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("创建 Tailscale 状态目录: {e}"))?;
    Ok(dir)
}

/// Official floor we support: sing-box 1.13 (endpoint extras + MagicDNS).
pub const MIN_CORE_VERSION_LABEL: &str = "1.13.0";

/// Feature line used when writing runtime JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreLine {
    /// 1.13.x — advertise_tags / system_interface; DNS uses ip_accept_any.
    V13,
    /// ≥1.14 — preferred_by, accept_search_domain, ssh_server.
    V14,
}

impl CoreLine {
    pub fn from_version(version: Option<&str>) -> Option<Self> {
        let (major, minor, _) = parse_semver(version?)?;
        if major > 1 || (major == 1 && minor >= 14) {
            Some(Self::V14)
        } else if major == 1 && minor >= 13 {
            Some(Self::V13)
        } else {
            None
        }
    }

    pub fn at_least(self, major: u32, minor: u32) -> bool {
        let (a, b) = match self {
            Self::V13 => (1, 13),
            Self::V14 => (1, 14),
        };
        a > major || (a == major && b >= minor)
    }
}

/// Supported from 1.13.x. Unknown parse → false.
pub fn meets_tailscale_core(version: Option<&str>) -> bool {
    CoreLine::from_version(version).is_some()
}

pub fn core_line_from_path(core_path: &str) -> CoreLine {
    let ver = crate::core_download::local_version(core_path);
    CoreLine::from_version(ver.as_deref()).unwrap_or(CoreLine::V13)
}

fn parse_semver(raw: &str) -> Option<(u32, u32, u32)> {
    let mut s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix('v').or_else(|| s.strip_prefix('V')) {
        s = rest;
    }
    let mut nums = s.split(|c: char| !c.is_ascii_digit());
    let major = nums.next()?.parse().ok()?;
    let minor = nums.next()?.parse().ok()?;
    let patch = nums.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

/// Refuse when older than 1.13.
pub fn ensure_core_version(core_path: &str) -> Result<String, String> {
    let ver = crate::core_download::local_version(core_path);
    if meets_tailscale_core(ver.as_deref()) {
        return Ok(ver.unwrap_or_default());
    }
    let current = ver.unwrap_or_else(|| "未安装".into());
    Err(format!(
        "当前内核 {current} 不支持 Tailscale（需要 ≥{MIN_CORE_VERSION_LABEL}，见官方 endpoint/tailscale 文档）"
    ))
}

pub fn login_url_from_log() -> Option<String> {
    let log = read_core_log_tail(LOG_TAIL);
    latest_hint(&log).login_url.or_else(|| {
        // also scan whole tail for any login URL
        for line in log.lines().rev() {
            if let Some(u) = url_from_line(line) {
                return Some(u);
            }
        }
        None
    })
}

#[allow(dead_code)]
pub fn core_log_exists() -> bool {
    core_log_path().is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base_cfg() -> Value {
        json!({
            "inbounds": [{"type":"http","tag":"http-in","listen":"127.0.0.1","listen_port":7890}],
            "outbounds": [{"type":"direct","tag":"direct"}],
            "endpoints": [
                {"type":"tailscale","tag":"from-sub"},
                {"type":"wireguard","tag":"wg-keep"}
            ],
            "dns": {
                "servers": [{"type":"local","tag":"local"}],
                "rules": []
            },
            "route": {"rules": [{"clash_mode":"Direct","outbound":"direct"}], "final":"direct"}
        })
    }

    fn enabled_ts() -> TailscaleSettings {
        TailscaleSettings::from_settings(&json!({
            "tailscale": {
                "enabled": true,
                "tag": "ts-local",
                "authKey": "tskey-auth-test",
                "hostname": "orb",
                "acceptRoutes": true,
                "injectDns": true,
                "injectRoutePreferredBy": true,
                "replaceOtherTailscale": true,
                "routeDomainSuffix": ".ts.net",
                "routeIpCidr": "100.64.0.0/10",
                "advertiseRoutes": "192.168.1.0/24"
            }
        }))
    }

    #[test]
    fn disabled_does_not_inject() {
        let ts = TailscaleSettings::from_settings(&json!({"tailscale":{"enabled":false}}));
        let cfg = with_tailscale(&base_cfg(), &ts);
        let eps = cfg["endpoints"].as_array().unwrap();
        assert_eq!(eps.len(), 2);
        assert!(eps.iter().all(|e| e["tag"] != "ts-local"));
        assert!(cfg["dns"]["servers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["type"] != "tailscale"));
    }

    #[test]
    fn enabled_injects_endpoint_dns_and_route() {
        let cfg = with_tailscale_for(&base_cfg(), &enabled_ts(), CoreLine::V14);
        let eps = cfg["endpoints"].as_array().unwrap();
        assert!(eps.iter().any(|e| e["tag"] == "ts-local" && e["type"] == "tailscale"));
        assert!(eps.iter().any(|e| e["tag"] == "wg-keep"));
        assert!(eps.iter().all(|e| e["tag"] != "from-sub"));
        let ep = eps.iter().find(|e| e["tag"] == "ts-local").unwrap();
        assert_eq!(ep["auth_key"], "tskey-auth-test");
        assert_eq!(ep["hostname"], "orb");
        assert_eq!(ep["advertise_routes"][0], "192.168.1.0/24");
        assert_eq!(ep["domain_resolver"]["server"], "ts-bootstrap");
        assert_eq!(ep["domain_resolver"]["strategy"], "ipv4_only");
        assert!(ep.get("domain_strategy").is_none());
        assert!(ep.get("detour").is_none());

        let servers = cfg["dns"]["servers"].as_array().unwrap();
        let ts = servers.iter().find(|s| s["tag"] == "ts-local-dns").unwrap();
        assert_eq!(ts["type"], "tailscale");
        assert_eq!(ts["accept_search_domain"], true);
        let rules = cfg["dns"]["rules"].as_array().unwrap();
        assert!(rules.iter().any(|r| r["server"] == "ts-local-dns" && r.get("preferred_by").is_some()));
        assert!(
            rules.iter().all(|r| {
                r["server"] != "ts-local-dns"
                    || r.get("preferred_by").is_some()
                    || r.get("domain_suffix").is_some()
            }),
            "tailscale DNS must not swallow every query (breaks TUN / CLI)"
        );

        let route = cfg["route"]["rules"].as_array().unwrap();
        assert!(route.iter().any(|r| r["outbound"] == "ts-local" && r.get("preferred_by").is_some()));
        assert!(route.iter().any(|r| r["outbound"] == "ts-local" && r.get("domain_suffix").is_some()));
        assert!(route.iter().any(|r| r["outbound"] == "ts-local" && r.get("ip_cidr").is_some()));
        assert!(route.iter().any(|r| r["clash_mode"] == "Direct"));
        assert!(route.iter().all(|r| {
            !(r["outbound"] == "direct"
                && r.get("domain_suffix")
                    .and_then(|v| v.as_array())
                    .is_some_and(|a| a.iter().any(|x| x == "tailscale.com")))
        }));
        assert!(rules.iter().any(|r| {
            r["server"] == "ts-bootstrap"
                && r.get("domain_suffix")
                    .and_then(|v| v.as_array())
                    .is_some_and(|a| a.iter().any(|x| x == "tailscale.com"))
        }));
        assert!(servers.iter().any(|s| s["tag"] == "ts-bootstrap" && s["type"] == "https"));
    }

    fn bootstrap_detour_of(cfg: &Value) -> Option<String> {
        cfg["dns"]["servers"]
            .as_array()?
            .iter()
            .find(|s| s["tag"] == "ts-bootstrap")?
            .get("detour")
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    fn overlay_with_outbounds(outbounds: Value, final_tag: Option<&str>) -> Value {
        let mut base = base_cfg();
        base["outbounds"] = outbounds;
        if let Some(tag) = final_tag {
            base["route"]["final"] = json!(tag);
        } else {
            base["route"].as_object_mut().unwrap().remove("final");
        }
        with_tailscale_for(&base, &enabled_ts(), CoreLine::V14)
    }

    #[test]
    fn bootstrap_dns_uses_proxy_detour() {
        let cfg = overlay_with_outbounds(
            json!([
                {"type": "selector", "tag": "proxy", "outbounds": ["auto", "direct"]},
                {"type": "urltest", "tag": "auto", "outbounds": ["direct"]},
                {"type": "direct", "tag": "direct"}
            ]),
            Some("proxy"),
        );
        assert_eq!(bootstrap_detour_of(&cfg).as_deref(), Some("proxy"));
        let ep = cfg["endpoints"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["tag"] == "ts-local")
            .unwrap();
        assert_eq!(ep["domain_resolver"]["server"], "ts-bootstrap");
    }

    #[test]
    fn bootstrap_follows_route_final_not_leftover_proxy() {
        let cfg = overlay_with_outbounds(
            json!([
                {"type": "selector", "tag": "proxy", "outbounds": ["direct"]},
                {"type": "selector", "tag": "Japan", "outbounds": ["jp-1"]},
                {"type": "vless", "tag": "jp-1"},
                {"type": "direct", "tag": "direct"}
            ]),
            Some("Japan"),
        );
        assert_eq!(bootstrap_detour_of(&cfg).as_deref(), Some("Japan"));
    }

    #[test]
    fn bootstrap_uses_chinese_selector_when_no_final() {
        let cfg = overlay_with_outbounds(
            json!([
                {"type": "selector", "tag": "节点选择", "outbounds": ["自动选择"]},
                {"type": "urltest", "tag": "自动选择", "outbounds": ["hk-1"]},
                {"type": "vless", "tag": "hk-1"},
                {"type": "direct", "tag": "direct"}
            ]),
            None,
        );
        assert_eq!(bootstrap_detour_of(&cfg).as_deref(), Some("节点选择"));
    }

    #[test]
    fn bootstrap_uses_capital_proxy_from_final() {
        let cfg = overlay_with_outbounds(
            json!([
                {"type": "selector", "tag": "Proxy", "outbounds": ["sg_urltest"]},
                {"type": "urltest", "tag": "sg_urltest", "outbounds": ["sg-1"]},
                {"type": "vless", "tag": "sg-1"},
                {"type": "direct", "tag": "direct"}
            ]),
            Some("Proxy"),
        );
        assert_eq!(bootstrap_detour_of(&cfg).as_deref(), Some("Proxy"));
    }

    #[test]
    fn bootstrap_uses_single_node_final() {
        let cfg = overlay_with_outbounds(
            json!([
                {"type": "vless", "tag": "jp-1"},
                {"type": "direct", "tag": "direct"}
            ]),
            Some("jp-1"),
        );
        assert_eq!(bootstrap_detour_of(&cfg).as_deref(), Some("jp-1"));
    }

    #[test]
    fn bootstrap_skips_direct_final_and_uses_first_group() {
        let cfg = overlay_with_outbounds(
            json!([
                {"type": "selector", "tag": "节点选择", "outbounds": ["hk-1"]},
                {"type": "vless", "tag": "hk-1"},
                {"type": "direct", "tag": "direct"}
            ]),
            Some("direct"),
        );
        assert_eq!(bootstrap_detour_of(&cfg).as_deref(), Some("节点选择"));
    }

    #[test]
    fn bootstrap_no_detour_when_only_direct() {
        let cfg = with_tailscale_for(&base_cfg(), &enabled_ts(), CoreLine::V14);
        assert_eq!(bootstrap_detour_of(&cfg), None);
    }

    #[test]
    fn cloudflare_https_final_gets_proxy_detour() {
        let mut base = base_cfg();
        base["outbounds"] = json!([
            {"type": "selector", "tag": "proxy", "outbounds": ["n1"]},
            {"type": "vless", "tag": "n1"},
            {"type": "direct", "tag": "direct"}
        ]);
        base["route"]["final"] = json!("proxy");
        base["dns"] = json!({
            "servers": [
                {"type": "https", "tag": "cloudflare", "server": "1.1.1.1"},
                {"type": "local", "tag": "local"}
            ],
            "final": "cloudflare",
            "rules": []
        });
        let cfg = with_tailscale_for(&base, &enabled_ts(), CoreLine::V14);
        let servers = cfg["dns"]["servers"].as_array().unwrap();
        let cf = servers.iter().find(|s| s["tag"] == "cloudflare").unwrap();
        assert_eq!(cf["detour"], "proxy");
        let boot = servers.iter().find(|s| s["tag"] == "ts-bootstrap").unwrap();
        assert_eq!(boot["detour"], "proxy");
        let local = servers.iter().find(|s| s["tag"] == "local").unwrap();
        assert!(local.get("detour").is_none());
    }

    #[test]
    fn bootstrap_detour_follows_each_profile() {
        let clash = overlay_with_outbounds(
            json!([
                {"type": "selector", "tag": "proxy", "outbounds": ["auto"]},
                {"type": "urltest", "tag": "auto", "outbounds": ["n1"]},
                {"type": "hysteria2", "tag": "n1"},
                {"type": "direct", "tag": "direct"}
            ]),
            Some("proxy"),
        );
        let sub = overlay_with_outbounds(
            json!([
                {"type": "selector", "tag": "节点选择", "outbounds": ["自动选择"]},
                {"type": "urltest", "tag": "自动选择", "outbounds": ["hk-1"]},
                {"type": "vless", "tag": "hk-1"},
                {"type": "direct", "tag": "direct"}
            ]),
            None,
        );
        assert_eq!(bootstrap_detour_of(&clash).as_deref(), Some("proxy"));
        assert_eq!(bootstrap_detour_of(&sub).as_deref(), Some("节点选择"));
    }

    #[test]
    fn v13_uses_ip_accept_any_not_preferred_by() {
        let cfg = with_tailscale_for(&base_cfg(), &enabled_ts(), CoreLine::V13);
        let servers = cfg["dns"]["servers"].as_array().unwrap();
        let ts = servers.iter().find(|s| s["tag"] == "ts-local-dns").unwrap();
        assert!(ts.get("accept_search_domain").is_none());
        let rules = cfg["dns"]["rules"].as_array().unwrap();
        assert!(rules.iter().any(|r| r["server"] == "ts-local-dns" && r.get("domain_suffix").is_some()));
        assert!(rules.iter().all(|r| r.get("preferred_by").is_none()));
        assert!(
            rules
                .iter()
                .all(|r| r.get("ip_accept_any").is_none() || r.get("domain_suffix").is_some()),
            "bare ip_accept_any hijacks all DNS"
        );
        let route = cfg["route"]["rules"].as_array().unwrap();
        assert!(route.iter().all(|r| r.get("preferred_by").is_none()));
        assert!(route.iter().any(|r| r["outbound"] == "ts-local" && r.get("domain_suffix").is_some()));
    }

    #[test]
    fn replace_other_false_keeps_subscription_endpoint() {
        let mut ts = enabled_ts();
        ts.replace_other = false;
        let cfg = with_tailscale(&base_cfg(), &ts);
        let tags: Vec<_> = cfg["endpoints"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["tag"].as_str().unwrap().to_string())
            .collect();
        assert!(tags.contains(&"from-sub".into()));
        assert!(tags.contains(&"ts-local".into()));
    }

    #[test]
    fn meets_core_floor() {
        assert!(!meets_tailscale_core(None));
        assert!(!meets_tailscale_core(Some("1.11.0")));
        assert!(!meets_tailscale_core(Some("1.12.0")));
        assert!(meets_tailscale_core(Some("1.13.18")));
        assert!(meets_tailscale_core(Some("1.14.0")));
        assert!(meets_tailscale_core(Some("v1.14.0-beta.3")));
        assert!(meets_tailscale_core(Some("1.15.0")));
        assert!(meets_tailscale_core(Some("2.0.0")));
        assert_eq!(CoreLine::from_version(Some("1.13.18")), Some(CoreLine::V13));
        assert_eq!(CoreLine::from_version(Some("1.14.0-beta.15")), Some(CoreLine::V14));
        assert!(!CoreLine::V13.at_least(1, 14));
        assert!(CoreLine::V13.at_least(1, 13));
        assert!(CoreLine::V14.at_least(1, 14));
    }

    #[test]
    fn parse_login_url_and_connected() {
        let waiting = "INFO endpoint/tailscale[ts-local]: waiting for authentication: https://login.tailscale.com/a/abc123";
        let hint = latest_hint(waiting);
        assert!(matches!(hint.kind, HintKind::WaitingAuth));
        assert_eq!(
            hint.login_url.as_deref(),
            Some("https://login.tailscale.com/a/abc123")
        );

        let log = format!(
            "{waiting}\nINFO dns/tailscale[ts-local-dns]: updated 67 routes, 23 hosts"
        );
        let hint = latest_hint(&log);
        assert!(matches!(hint.kind, HintKind::Connected));

        let empty_hosts = "INFO dns/tailscale[ts-local-dns]: updated 67 routes, 0 hosts, 1 search domains";
        let hint = latest_hint(empty_hosts);
        assert!(
            !matches!(hint.kind, HintKind::Connected),
            "0 hosts is not online"
        );
    }

    #[test]
    fn netmap_self_has_cg_nat_ip() {
        let raw = r#"{
            "Node": {
                "Name": "ppdemac-mini-1.tailafc5c3.ts.net.",
                "Addresses": ["100.93.134.114/32", "fd7a:115c:a1e0::ac34:8673/128"]
            }
        }"#;
        let (name, ip) = parse_netmap_self(raw).unwrap();
        assert_eq!(name.as_deref(), Some("ppdemac-mini-1.tailafc5c3.ts.net"));
        assert_eq!(ip.as_deref(), Some("100.93.134.114"));
    }

    #[test]
    fn logged_in_state_beats_stale_login_url() {
        let profiles = r#"{"8465":{"Name":"openmindw@github","UserProfile":{"DisplayName":"openmindw","LoginName":"openmindw@github"},"NodeID":"n1"}}"#;
        let prefs = r#"{"LoggedOut":false,"WantRunning":true,"Hostname":"localhost","Config":{"UserProfile":{"DisplayName":"openmindw"}}}"#;
        let state = serde_json::json!({
            "_profiles": b64(profiles),
            "profile-8465": b64(prefs),
        })
        .to_string();
        let ident = parse_tailscaled_state(&state).unwrap();
        assert!(ident.logged_in);
        assert_eq!(ident.hostname.as_deref(), Some("openmindw"));
    }

    fn b64(s: &str) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let bytes = s.as_bytes();
        let mut out = String::new();
        let mut i = 0;
        while i < bytes.len() {
            let b0 = bytes[i] as u32;
            let b1 = if i + 1 < bytes.len() {
                bytes[i + 1] as u32
            } else {
                0
            };
            let b2 = if i + 2 < bytes.len() {
                bytes[i + 2] as u32
            } else {
                0
            };
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(T[((n >> 18) & 63) as usize] as char);
            out.push(T[((n >> 12) & 63) as usize] as char);
            if i + 1 < bytes.len() {
                out.push(T[((n >> 6) & 63) as usize] as char);
            } else {
                out.push('=');
            }
            if i + 2 < bytes.len() {
                out.push(T[(n & 63) as usize] as char);
            } else {
                out.push('=');
            }
            i += 3;
        }
        out
    }

    #[test]
    fn empty_magicdns_is_not_joined() {
        let log = "INFO dns/tailscale[ts-local-dns]: updated 0 routes, 0 hosts, 0 search domains";
        let hint = latest_hint(log);
        assert!(matches!(hint.kind, HintKind::None));

        let ts = TailscaleSettings::from_settings(&json!({
            "tailscale": { "enabled": true, "authKey": "" }
        }));
        let st = pending_status(&ts, None, None);
        assert_eq!(st.phase, TsPhase::Pending);
        assert_eq!(st.title.0, "验证中");

        let with_key = TailscaleSettings::from_settings(&json!({
            "tailscale": { "enabled": true, "authKey": "tskey-auth-x" }
        }));
        let st = pending_status(&with_key, None, None);
        assert_eq!(st.phase, TsPhase::Pending);
        assert!(st.subtitle.0.is_empty());
    }

    #[test]
    fn tailscale_ip_range() {
        assert!(is_tailscale_ip("100.64.0.1"));
        assert!(is_tailscale_ip("100.127.1.2"));
        assert!(!is_tailscale_ip("100.63.0.1"));
        assert!(!is_tailscale_ip("10.0.0.1"));
    }
}
