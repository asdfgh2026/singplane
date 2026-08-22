//! AppData store.
#![allow(dead_code)]
//!
//! Settings live in `%APPDATA%/SingPanel/SingPanel/shared_preferences.json`
//! (legacy key prefix `flutter.` is kept so existing installs keep their settings).
//! Profiles / user templates are one JSON file each under `profiles/` and `templates/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use crate::host::{app_root, default_core_path, runtime_dir};

const SETTINGS_KEY: &str = "app_settings_v1";
const THEME_KEY: &str = "theme_mode_v1";
const DISCLAIMER_KEY: &str = "disclaimer_accepted_v1";
const PROFILES_INDEX_KEY: &str = "profiles_v1_index";
const TEMPLATES_INDEX_KEY: &str = "templates_v1_index";

pub fn profiles_dir() -> PathBuf {
    app_root().join("profiles")
}

pub fn templates_dir() -> PathBuf {
    app_root().join("templates")
}

pub fn prefs_path() -> PathBuf {
    app_root().join("shared_preferences.json")
}

pub fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

pub fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|s| s.to_str()).unwrap_or("json")
    ));
    fs::write(&tmp, content).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(&tmp, path).map_err(|e| format!("copy {}: {e}", path.display()))?;
            let _ = fs::remove_file(&tmp);
            Ok(())
        }
    }
}

fn load_prefs() -> Result<Map<String, Value>, String> {
    let path = prefs_path();
    if !path.is_file() {
        return Ok(Map::new());
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("read prefs: {e}"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("prefs json: {e}"))?;
    Ok(v.as_object().cloned().unwrap_or_default())
}

fn save_prefs(map: &Map<String, Value>) -> Result<(), String> {
    let text = serde_json::to_string(map).map_err(|e| e.to_string())?;
    atomic_write(&prefs_path(), &text)
}

fn pref_key(key: &str) -> String {
    format!("flutter.{key}")
}

pub fn prefs_get(key: &str) -> Option<Value> {
    load_prefs().ok()?.remove(&pref_key(key))
}

pub fn prefs_set(key: &str, value: Value) -> Result<(), String> {
    let mut map = load_prefs()?;
    map.insert(pref_key(key), value);
    save_prefs(&map)
}

pub fn prefs_get_string(key: &str) -> Option<String> {
    match prefs_get(key)? {
        Value::String(s) => Some(s),
        other => Some(other.to_string()),
    }
}

pub fn load_theme_mode() -> String {
    prefs_get_string(THEME_KEY).unwrap_or_else(|| "system".into())
}

pub fn save_theme_mode(mode: &str) -> Result<(), String> {
    prefs_set(THEME_KEY, Value::String(mode.to_string()))
}

pub const DISCLAIMER_TEXT: &str = "本软件为开源免费软件，仅供学习交流等非商业性质的个人测试使用，代理服务商的行为均与本软件无关，同意声明代表您已完全知晓并确认了这一点，如不同意，请选择退出！";

pub fn disclaimer_accepted() -> bool {
    match prefs_get(DISCLAIMER_KEY) {
        Some(Value::Bool(v)) => v,
        Some(Value::String(s)) => s == "true" || s == "1",
        _ => false,
    }
}

pub fn set_disclaimer_accepted(accepted: bool) -> Result<(), String> {
    prefs_set(DISCLAIMER_KEY, Value::Bool(accepted))
}

pub fn load_settings() -> Value {
    if let Some(raw) = prefs_get_string(SETTINGS_KEY) {
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            return v;
        }
    }
    default_settings()
}

pub fn default_settings() -> Value {
    serde_json::json!({
        "corePath": default_core_path().to_string_lossy(),
        "mixedPort": 7890,
        "clashApiPort": 9090,
        "clashApiHost": "127.0.0.1",
        "autoStartCore": false,
        "activeProfileId": Value::Null,
        "seedColorValue": 0xFF047857u32,
        "coreChannel": "beta",
        "closeToTray": true,
        "trayEnabled": true,
        "launchAtStartup": false,
        "autoUpdateSubscriptions": true,
        "autoUpdateIntervalMinutes": 60,
        "defaultAssembleOnImport": false,
        "defaultTemplateId": "builtin-mixed-direct",
        "forceAppPortsOnAssemble": true,
        "stripTunOnAssemble": false,
        "githubProxy": "",
        "language": "system",
        "systemProxyEnabled": true,
        "tunEnabled": false,
    })
}

pub fn save_settings(settings: &Value) -> Result<(), String> {
    let text = serde_json::to_string(settings).map_err(|e| e.to_string())?;
    prefs_set(SETTINGS_KEY, Value::String(text))
}

/// Shallow-merge `patch` into current settings and persist.
pub fn patch_settings(patch: &Value) -> Result<Value, String> {
    let mut current = load_settings();
    if let (Some(dst), Some(src)) = (current.as_object_mut(), patch.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    save_settings(&current)?;
    Ok(current)
}

pub fn settings_str(settings: &Value, key: &str) -> String {
    settings
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

pub fn settings_i64(settings: &Value, key: &str, default: i64) -> i64 {
    settings
        .get(key)
        .and_then(|v| v.as_i64())
        .unwrap_or(default)
}

pub fn settings_bool(settings: &Value, key: &str, default: bool) -> bool {
    settings
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

/// Secret the kernel is actually using. Settings win when set; otherwise
/// the active runtime `experimental.clash_api.secret` (subscriptions often
/// ship one; empty Bearer then 401s every `/proxies` call).
pub fn clash_secret_for_calls(settings: &Value) -> String {
    let from_settings = settings_str(settings, "clashApiSecret");
    if !from_settings.is_empty() {
        return from_settings;
    }
    read_runtime_json()
        .as_ref()
        .and_then(clash_secret_from_config)
        .unwrap_or_default()
}

pub fn clash_secret_from_config(cfg: &Value) -> Option<String> {
    let raw = cfg
        .get("experimental")
        .and_then(|e| e.get("clash_api"))
        .and_then(|c| c.get("secret"))?;
    let s = match raw {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

pub fn clash_base_from_settings(settings: &Value) -> String {
    let host = settings_str(settings, "clashApiHost");
    let host = if host.is_empty() {
        "127.0.0.1".into()
    } else if host == "0.0.0.0" || host == "::" {
        "127.0.0.1".into()
    } else {
        host
    };
    let port = settings_i64(settings, "clashApiPort", 9090);
    format!("http://{host}:{port}")
}

pub fn core_path_from_settings(settings: &Value) -> PathBuf {
    let raw = settings_str(settings, "corePath");
    if raw.is_empty() {
        crate::host::abs_path(default_core_path())
    } else {
        crate::host::abs_path(PathBuf::from(raw))
    }
}

pub fn load_profiles() -> Vec<Value> {
    let dir = profiles_dir();
    let mut by_id = Map::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                        by_id.insert(id.to_string(), v);
                    }
                }
            }
        }
    }
    let order: Vec<String> = prefs_get_string(PROFILES_INDEX_KEY)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let mut out = Vec::new();
    for id in order {
        if let Some(p) = by_id.remove(&id) {
            out.push(p);
        }
    }
    let mut rest: Vec<_> = by_id.into_values().collect();
    rest.sort_by(|a, b| {
        settings_str(a, "name").cmp(&settings_str(b, "name"))
    });
    out.extend(rest);
    out
}

pub fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("id 不能为空".into());
    }
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err("id 包含非法路径字符".into());
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("id 仅允许字母、数字、下划线及中划线".into());
    }
    Ok(())
}

pub fn save_profile(profile: &Value) -> Result<(), String> {
    let id = profile
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("profile 缺少 id")?;
    validate_id(id)?;
    fs::create_dir_all(profiles_dir()).map_err(|e| e.to_string())?;
    let path = profiles_dir().join(format!("{id}.json"));
    let text = serde_json::to_string_pretty(profile).map_err(|e| e.to_string())?;
    atomic_write(&path, &text)?;
    let mut ids: Vec<String> = prefs_get_string(PROFILES_INDEX_KEY)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if !ids.iter().any(|x| x == id) {
        ids.push(id.to_string());
        prefs_set(
            PROFILES_INDEX_KEY,
            Value::String(serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into())),
        )?;
    }
    Ok(())
}

pub fn delete_profile(id: &str) -> Result<(), String> {
    validate_id(id)?;
    let path = profiles_dir().join(format!("{id}.json"));
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let ids: Vec<String> = prefs_get_string(PROFILES_INDEX_KEY)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let ids: Vec<String> = ids.into_iter().filter(|x| x != id).collect();
    prefs_set(
        PROFILES_INDEX_KEY,
        Value::String(serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into())),
    )
}

pub fn load_user_templates() -> Vec<Value> {
    let dir = templates_dir();
    let mut by_id = Map::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    if v.get("builtin").and_then(|x| x.as_bool()).unwrap_or(false) {
                        continue;
                    }
                    if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                        if !id.is_empty() {
                            by_id.insert(id.to_string(), v);
                        }
                    }
                }
            }
        }
    }
    let order: Vec<String> = prefs_get_string(TEMPLATES_INDEX_KEY)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let mut out = Vec::new();
    for id in order {
        if let Some(t) = by_id.remove(&id) {
            out.push(t);
        }
    }
    out.extend(by_id.into_values());
    out
}

pub fn load_builtin_templates() -> Vec<Value> {
    const META: &[(&str, &str, &str, &str)] = &[
        (
            "builtin-mixed-direct",
            "桌面 · Mixed 直连基础模板",
            "mixed 127.0.0.1:7890，无 TUN，Clash API :9090；节点由模板注入。",
            "builtin-mixed-direct.json",
        ),
        (
            "builtin-mixed-rule",
            "桌面 · Mixed + 基础分流",
            "在直连模板上增加私有 IP / 本地域名直连规则，无远程 ruleset。",
            "builtin-mixed-rule.json",
        ),
    ];
    let mut out = Vec::new();
    for (id, name, desc, file) in META {
        let content = read_asset_template(file).unwrap_or_default();
        out.push(serde_json::json!({
            "id": id,
            "name": name,
            "description": desc,
            "builtin": true,
            "content": content,
        }));
    }
    out
}

pub fn load_all_templates() -> Vec<Value> {
    let mut all = load_builtin_templates();
    all.extend(load_user_templates());
    all
}

pub fn template_by_id(id: &str) -> Option<Value> {
    load_all_templates()
        .into_iter()
        .find(|t| t.get("id").and_then(|v| v.as_str()) == Some(id))
}

pub fn save_template(template: &Value) -> Result<(), String> {
    let id = template
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("template 缺少 id")?;
    validate_id(id)?;
    if template
        .get("builtin")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Err("内置模板只读".into());
    }
    fs::create_dir_all(templates_dir()).map_err(|e| e.to_string())?;
    let path = templates_dir().join(format!("{id}.json"));
    let text = serde_json::to_string_pretty(template).map_err(|e| e.to_string())?;
    atomic_write(&path, &text)?;
    let mut ids: Vec<String> = prefs_get_string(TEMPLATES_INDEX_KEY)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if !ids.iter().any(|x| x == id) {
        ids.push(id.to_string());
        prefs_set(
            TEMPLATES_INDEX_KEY,
            Value::String(serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into())),
        )?;
    }
    Ok(())
}

pub fn delete_template(id: &str) -> Result<(), String> {
    validate_id(id)?;
    if id.starts_with("builtin-") {
        return Err("内置模板不能删除".into());
    }
    let path = templates_dir().join(format!("{id}.json"));
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let ids: Vec<String> = prefs_get_string(TEMPLATES_INDEX_KEY)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let ids: Vec<String> = ids.into_iter().filter(|x| x != id).collect();
    prefs_set(
        TEMPLATES_INDEX_KEY,
        Value::String(serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into())),
    )
}

pub fn write_runtime_config(content: &str) -> Result<PathBuf, String> {
    let dir = runtime_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("config.runtime.json");
    atomic_write(&path, content)?;
    Ok(path)
}

pub fn active_profile(settings: &Value, profiles: &[Value]) -> Option<Value> {
    let id = settings.get("activeProfileId").and_then(|v| v.as_str())?;
    profiles
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(id))
        .cloned()
}

fn read_asset_template(file: &str) -> Option<String> {
    for dir in asset_template_dirs() {
        let path = dir.join(file);
        if let Ok(text) = fs::read_to_string(path) {
            return Some(text);
        }
    }
    None
}

fn asset_template_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(p) = std::env::var("SINGPANEL_ASSETS") {
        dirs.push(PathBuf::from(p).join("templates"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("assets").join("templates"));
        dirs.push(cwd.join("..").join("assets").join("templates"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(repo) = exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
        {
            dirs.push(repo.join("assets").join("templates"));
        }
    }
    dirs
}

/// Default assemble options object (camelCase, matches host).
pub fn default_assemble_options() -> Value {
    serde_json::json!({
        "include": "",
        "exclude": "",
        "addSourceTag": false,
        "disableDefaultGroups": false,
        "keepSourceGroups": false,
        "keepSourceDns": false,
        "keepSourceRoute": false,
    })
}

pub fn patch_from_settings(settings: &Value) -> Value {
    let mixed = settings_i64(settings, "mixedPort", 7890);
    let host = settings_str(settings, "clashApiHost");
    let host = if host.is_empty() {
        "127.0.0.1".into()
    } else {
        host
    };
    let port = settings_i64(settings, "clashApiPort", 9090);
    serde_json::json!({
        "forceMixedPort": mixed,
        "forceClashApi": format!("{host}:{port}"),
        "forceListenLocalhost": true,
        "stripTun": settings_bool(settings, "stripTunOnAssemble", false),
    })
}

pub fn json_has_tun(cfg: &Value) -> bool {
    cfg.get("inbounds")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| {
            arr.iter()
                .any(|ib| ib.get("type").and_then(|t| t.as_str()) == Some("tun"))
        })
}

pub fn runtime_has_tun() -> bool {
    read_runtime_json().is_some_and(|v| json_has_tun(&v))
}

/// Port the OS system proxy must use: the running mixed/http/socks inbound,
/// not the settings default. Profiles often listen on 2080 while mixedPort is 7890.
pub fn runtime_proxy_port(settings: &Value) -> u16 {
    if let Some(cfg) = read_runtime_json() {
        if let Some(port) = proxy_inbound_port(&cfg) {
            return port;
        }
    }
    settings_i64(settings, "mixedPort", 7890).clamp(1, 65535) as u16
}

pub fn proxy_inbound_port(cfg: &Value) -> Option<u16> {
    let inbounds = cfg.get("inbounds")?.as_array()?;
    let mut http = None;
    let mut socks = None;
    for ib in inbounds {
        let typ = ib.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let Some(port) = ib
            .get("listen_port")
            .and_then(|v| v.as_u64())
            .or_else(|| ib.get("listen_port").and_then(|v| v.as_i64()).map(|n| n as u64))
        else {
            continue;
        };
        if port == 0 || port > 65535 {
            continue;
        }
        let port = port as u16;
        match typ {
            "mixed" => return Some(port),
            "http" if http.is_none() => http = Some(port),
            "socks" if socks.is_none() => socks = Some(port),
            _ => {}
        }
    }
    http.or(socks)
}

fn read_runtime_json() -> Option<Value> {
    let path = crate::host::default_config_path();
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

/// Runtime overlay used at core start. Profile JSON on disk is never rewritten.
/// Runtime overlay: Tailscale + Clash API + optional ports.
pub fn for_runtime(user_config: &Value, settings: &Value) -> Value {
    let ts = crate::tailscale::TailscaleSettings::from_settings(settings);
    let core = core_path_from_settings(settings);
    let line = crate::tailscale::core_line_from_path(&core.to_string_lossy());
    let mut cfg = crate::tailscale::with_tailscale_for(user_config, &ts, line);
    crate::tailscale::ensure_remote_dns_detour(&mut cfg);
    let had_api = has_clash_api_config(&cfg);
    cfg = ensure_clash_api(cfg, settings, false);
    let force_ports = settings_bool(settings, "forceAppPortsOnAssemble", true);
    let tun_enabled = settings_bool(settings, "tunEnabled", false);
    let strip_tun = !tun_enabled;
    if force_ports || strip_tun || tun_enabled {
        cfg = apply_runtime_patch(
            cfg,
            if force_ports {
                Some(settings_i64(settings, "mixedPort", 7890) as u16)
            } else {
                None
            },
            if had_api {
                None
            } else {
                Some(clash_controller_from_settings(settings))
            },
            force_ports,
            strip_tun,
        );
        cfg = ensure_clash_api(cfg, settings, false);
    }
    if tun_enabled && !json_has_tun(&cfg) {
        cfg = inject_default_tun(cfg);
    }
    if tun_enabled {
        cfg = sanitize_tun_inbounds(cfg, cfg!(target_os = "macos"));
        cfg = ensure_tun_fakeip_dns(cfg);
    }
    cfg = ensure_cache_file(cfg);
    strip_legacy_domain_strategy(&mut cfg);
    cfg
}

/// sing-box 1.14+ fatals on leftover `domain_strategy` dial fields.
fn strip_legacy_domain_strategy(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.remove("domain_strategy");
            for child in map.values_mut() {
                strip_legacy_domain_strategy(child);
            }
        }
        Value::Array(arr) => {
            for child in arr {
                strip_legacy_domain_strategy(child);
            }
        }
        _ => {}
    }
}

fn tun_name_ok_on_macos(name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return true;
    }
    // Darwin native tun only accepts utun + digits (see sing-box "bad tun name").
    name.strip_prefix("utun")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

fn sanitize_tun_inbounds(mut cfg: Value, macos: bool) -> Value {
    if !macos {
        return cfg;
    }
    let Some(raw) = cfg.get("inbounds").and_then(Value::as_array).cloned() else {
        return cfg;
    };
    let mut inbounds = Vec::new();
    for item in raw {
        let Some(mut m) = item.as_object().cloned() else {
            continue;
        };
        if m.get("type").and_then(Value::as_str) == Some("tun") {
            let name = m
                .get("interface_name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if !tun_name_ok_on_macos(&name) {
                m.remove("interface_name");
            }
        }
        inbounds.push(Value::Object(m));
    }
    cfg["inbounds"] = Value::Array(inbounds);
    cfg
}

pub fn has_clash_api_config(cfg: &Value) -> bool {
    cfg.get("experimental")
        .and_then(|e| e.get("clash_api"))
        .and_then(|c| c.get("external_controller"))
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

fn clash_controller_from_settings(settings: &Value) -> String {
    let host = settings_str(settings, "clashApiHost");
    let host = if host.is_empty() {
        "127.0.0.1".into()
    } else {
        host
    };
    let port = settings_i64(settings, "clashApiPort", 9090);
    format!("{host}:{port}")
}

fn apply_clash_secret(clash: &mut serde_json::Map<String, Value>, settings: &Value) {
    let secret = settings_str(settings, "clashApiSecret");
    if !secret.is_empty() {
        clash.insert("secret".into(), Value::String(secret));
    }
}

fn ensure_clash_api(cfg: Value, settings: &Value, force: bool) -> Value {
    let mut cfg = cfg;
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
    let existing = clash
        .get("external_controller")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if force || existing.is_empty() {
        clash.insert(
            "external_controller".into(),
            Value::String(clash_controller_from_settings(settings)),
        );
    }
    clash
        .entry("default_mode".to_string())
        .or_insert_with(|| Value::String("rule".into()));
    apply_clash_secret(&mut clash, settings);
    experimental.insert("clash_api".into(), Value::Object(clash));
    cfg["experimental"] = Value::Object(experimental);
    cfg
}

/// sing-box official: fake-ip + Clash selected outbound live in cache_file.
/// Without it, TUN logs `missing fakeip record` and selector `now` resets.
fn ensure_cache_file(mut cfg: Value) -> Value {
    let mut experimental = cfg
        .get("experimental")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut cache = experimental
        .get("cache_file")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    cache.insert("enabled".into(), Value::Bool(true));
    let path = cache
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if path.is_empty() {
        let db = runtime_dir().join("cache.db");
        cache.insert(
            "path".into(),
            Value::String(db.to_string_lossy().into_owned()),
        );
    }
    cache.insert("store_fakeip".into(), Value::Bool(true));
    experimental.insert("cache_file".into(), Value::Object(cache));
    cfg["experimental"] = Value::Object(experimental);
    cfg
}

/// Official client: A/AAAA → fakeip (after CN/Tailscale exceptions).
/// LAN resolvers that also use 198.18/15 must not answer TUN queries.
fn fakeip_server_tag(cfg: &Value) -> Option<String> {
    cfg.get("dns")
        .and_then(|d| d.get("servers"))
        .and_then(Value::as_array)
        .and_then(|arr| {
            arr.iter().find_map(|s| {
                if s.get("type").and_then(Value::as_str) == Some("fakeip") {
                    s.get("tag").and_then(Value::as_str).map(str::to_string)
                } else {
                    None
                }
            })
        })
}

fn ensure_tun_fakeip_dns(mut cfg: Value) -> Value {
    let Some(tag) = fakeip_server_tag(&cfg) else {
        return cfg;
    };
    let mut dns = cfg
        .get("dns")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    dns.insert("independent_cache".into(), Value::Bool(true));
    let mut rules = dns
        .get("rules")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let already = rules.iter().any(|r| {
        r.get("query_type")
            .and_then(Value::as_array)
            .is_some_and(|q| q.iter().any(|v| v.as_str() == Some("A")))
            && r.get("server").and_then(Value::as_str) == Some(tag.as_str())
    });
    if !already {
        rules.push(serde_json::json!({
            "query_type": ["A", "AAAA"],
            "server": tag,
        }));
    }
    dns.insert("rules".into(), Value::Array(rules));
    cfg["dns"] = Value::Object(dns);
    cfg
}

fn inject_default_tun(mut cfg: Value) -> Value {
    let mut inbounds = cfg
        .get("inbounds")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // Official client TUN: address + auto_route + strict_route.
    // Tag must be `tun` so subscription dns.rules `inbound: tun` still matches.
    // sniff is a listen field; fake-ip needs the domain back from 198.18.x.
    inbounds.push(serde_json::json!({
        "type": "tun",
        "tag": "tun",
        "address": ["172.19.0.1/30"],
        "auto_route": true,
        "strict_route": true,
        "sniff": true,
    }));
    cfg["inbounds"] = Value::Array(inbounds);
    cfg
}

fn apply_runtime_patch(
    mut cfg: Value,
    force_mixed_port: Option<u16>,
    force_clash_api: Option<String>,
    force_listen_localhost: bool,
    strip_tun: bool,
) -> Value {
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
            if strip_tun && typ == "tun" {
                continue;
            }
            if typ == "mixed" || typ == "http" || typ == "socks" {
                if let Some(port) = force_mixed_port {
                    m.insert("listen_port".into(), serde_json::json!(port));
                }
                if force_listen_localhost {
                    m.insert("listen".into(), serde_json::json!("127.0.0.1"));
                }
            }
            inbounds.push(Value::Object(m));
        }
        cfg["inbounds"] = Value::Array(inbounds);
    }
    if let Some(ctrl) = force_clash_api.filter(|s| !s.is_empty()) {
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
        clash.insert("external_controller".into(), Value::String(ctrl));
        experimental.insert("clash_api".into(), Value::Object(clash));
        cfg["experimental"] = Value::Object(experimental);
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clash_secret_from_config_reads_experimental() {
        let cfg = serde_json::json!({
            "experimental": { "clash_api": { "secret": "888888", "external_controller": "127.0.0.1:9090" } }
        });
        assert_eq!(clash_secret_from_config(&cfg).as_deref(), Some("888888"));
        assert_eq!(clash_secret_from_config(&serde_json::json!({})), None);
        assert_eq!(
            clash_secret_from_config(&serde_json::json!({"experimental":{"clash_api":{"secret":""}}})),
            None
        );
    }

    #[test]
    fn clash_secret_prefers_settings_over_runtime() {
        let settings = serde_json::json!({ "clashApiSecret": "from-settings" });
        assert_eq!(clash_secret_for_calls(&settings), "from-settings");
    }

    #[test]
    fn runtime_keeps_subscription_clash_secret_for_non_loopback_controller() {
        let user = serde_json::json!({
            "outbounds": [{"type": "direct", "tag": "direct"}],
            "experimental": {
                "clash_api": {
                    "external_controller": "0.0.0.0:9090",
                    "secret": "888888"
                }
            }
        });
        let settings = serde_json::json!({
            "tunEnabled": false,
            "forceAppPortsOnAssemble": false,
            "tailscale": { "enabled": false }
        });
        let cfg = for_runtime(&user, &settings);
        assert_eq!(cfg["experimental"]["clash_api"]["secret"], "888888");
        assert_eq!(
            cfg["experimental"]["clash_api"]["external_controller"],
            "0.0.0.0:9090"
        );
    }

    #[test]
    fn runtime_applies_settings_clash_secret() {
        let user = serde_json::json!({
            "outbounds": [{"type": "direct", "tag": "direct"}],
            "experimental": {
                "clash_api": {
                    "external_controller": "127.0.0.1:9090",
                    "secret": "888888"
                }
            }
        });
        let settings = serde_json::json!({
            "clashApiSecret": "from-ui",
            "tunEnabled": false,
            "forceAppPortsOnAssemble": false,
            "tailscale": { "enabled": false }
        });
        let cfg = for_runtime(&user, &settings);
        assert_eq!(cfg["experimental"]["clash_api"]["secret"], "from-ui");
    }

    #[test]
    fn proxy_port_prefers_mixed() {
        let cfg = serde_json::json!({
            "inbounds": [
                {"type": "http", "listen_port": 8080},
                {"type": "mixed", "listen": "127.0.0.1", "listen_port": 2080},
                {"type": "socks", "listen_port": 1080}
            ]
        });
        assert_eq!(proxy_inbound_port(&cfg), Some(2080));
    }

    #[test]
    fn runtime_adds_detour_to_https_dns_without_tailscale() {
        let user = serde_json::json!({
            "outbounds": [
                {"type": "selector", "tag": "proxy", "outbounds": ["n1"]},
                {"type": "vless", "tag": "n1"},
                {"type": "direct", "tag": "direct"}
            ],
            "route": { "final": "proxy" },
            "dns": {
                "servers": [
                    {"type": "https", "tag": "cloudflare", "server": "1.1.1.1"},
                    {"type": "local", "tag": "local"}
                ],
                "final": "cloudflare"
            }
        });
        let settings = serde_json::json!({
            "tunEnabled": false,
            "forceAppPortsOnAssemble": false,
            "tailscale": { "enabled": false }
        });
        let cfg = for_runtime(&user, &settings);
        assert_eq!(cfg["dns"]["servers"][0]["detour"], "proxy");
        assert!(cfg["dns"]["servers"][1].get("detour").is_none());
    }

    #[test]
    fn runtime_strips_legacy_domain_strategy() {
        let user = serde_json::json!({
            "outbounds": [{
                "type": "direct",
                "tag": "direct",
                "domain_strategy": "prefer_ipv4"
            }],
            "route": { "final": "direct" }
        });
        let settings = serde_json::json!({
            "tunEnabled": false,
            "forceAppPortsOnAssemble": false,
            "tailscale": { "enabled": false }
        });
        let cfg = for_runtime(&user, &settings);
        assert!(
            cfg["outbounds"][0].get("domain_strategy").is_none(),
            "1.14 fatals on leftover domain_strategy: {}",
            cfg["outbounds"][0]
        );
    }

    #[test]
    fn runtime_enables_cache_file_for_fakeip_and_selected() {
        let user = serde_json::json!({
            "inbounds": [{"type": "mixed", "listen": "127.0.0.1", "listen_port": 2080}],
            "outbounds": [{"type": "direct", "tag": "direct"}],
            "experimental": {
                "clash_api": { "external_controller": "127.0.0.1:9090" }
            }
        });
        let settings = serde_json::json!({
            "tunEnabled": false,
            "forceAppPortsOnAssemble": false,
            "tailscale": { "enabled": false }
        });
        let cfg = for_runtime(&user, &settings);
        let cache = &cfg["experimental"]["cache_file"];
        assert_eq!(cache["enabled"], true);
        assert_eq!(cache["store_fakeip"], true);
        let path = cache["path"].as_str().unwrap();
        assert!(path.ends_with("cache.db"), "{path}");
        assert!(path.contains("runtime"), "{path}");
    }

    #[test]
    fn cache_file_keeps_existing_path() {
        let mut cfg = serde_json::json!({
            "experimental": {
                "cache_file": { "path": "/tmp/custom-cache.db" }
            }
        });
        cfg = ensure_cache_file(cfg);
        assert_eq!(cfg["experimental"]["cache_file"]["path"], "/tmp/custom-cache.db");
        assert_eq!(cfg["experimental"]["cache_file"]["enabled"], true);
        assert_eq!(cfg["experimental"]["cache_file"]["store_fakeip"], true);
    }

    #[test]
    fn tun_runtime_appends_official_aaaa_fakeip_rule() {
        let user = serde_json::json!({
            "inbounds": [{"type": "mixed", "listen_port": 2080}],
            "outbounds": [{"type": "direct", "tag": "direct"}],
            "dns": {
                "servers": [
                    {"type": "https", "tag": "local", "server": "223.5.5.5"},
                    {"type": "fakeip", "tag": "remote", "inet4_range": "198.18.0.0/15"}
                ],
                "rules": [
                    {"rule_set": ["geosite-cn"], "server": "local"}
                ]
            }
        });
        let settings = serde_json::json!({
            "tunEnabled": true,
            "forceAppPortsOnAssemble": false,
            "tailscale": { "enabled": false }
        });
        let cfg = for_runtime(&user, &settings);
        assert_eq!(cfg["dns"]["independent_cache"], true);
        let rules = cfg["dns"]["rules"].as_array().unwrap();
        assert!(rules.iter().any(|r| {
            r.get("query_type").is_some() && r["server"] == "remote"
        }));
    }

    #[test]
    fn inject_tun_uses_profile_tag_and_safe_macos_defaults() {
        let cfg = inject_default_tun(serde_json::json!({
            "inbounds": [{"type": "mixed", "listen_port": 2080}]
        }));
        let tun = cfg["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|ib| ib["type"] == "tun")
            .unwrap();
        assert_eq!(tun["tag"], "tun", "profile dns.rules inbound is tun");
        assert_eq!(tun["address"][0], "172.19.0.1/30");
        assert_eq!(tun["auto_route"], true);
        assert_eq!(tun["strict_route"], true);
        assert_eq!(tun["sniff"], true);
    }

    #[test]
    fn for_runtime_injects_tun_when_enabled() {
        let user = serde_json::json!({
            "inbounds": [{"type": "mixed", "listen": "127.0.0.1", "listen_port": 2080}],
            "outbounds": [{"type": "direct", "tag": "direct"}]
        });
        let settings = serde_json::json!({
            "tunEnabled": true,
            "forceAppPortsOnAssemble": false,
            "tailscale": {"enabled": false}
        });
        let cfg = for_runtime(&user, &settings);
        assert!(json_has_tun(&cfg));
        let tun = cfg["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|ib| ib["type"] == "tun")
            .unwrap();
        assert_eq!(tun["tag"], "tun");
    }

    #[test]
    fn proxy_port_skips_tun_without_listen_port() {
        let cfg = serde_json::json!({
            "inbounds": [
                {"type": "tun", "tag": "tun-in", "address": ["172.19.0.1/30"]},
                {"type": "mixed", "listen": "127.0.0.1", "listen_port": 2080}
            ]
        });
        assert_eq!(proxy_inbound_port(&cfg), Some(2080));
    }

    #[test]
    fn macos_rejects_linux_tun_device_names() {
        assert!(tun_name_ok_on_macos(""));
        assert!(tun_name_ok_on_macos("utun4"));
        assert!(!tun_name_ok_on_macos("utun"));
        assert!(!tun_name_ok_on_macos("sing-box-tun"));
        assert!(!tun_name_ok_on_macos("tun0"));
    }

    #[test]
    fn imported_linux_tun_name_is_cleared_for_macos() {
        let user = serde_json::json!({
            "inbounds": [{
                "type": "tun",
                "tag": "tun-in",
                "interface_name": "sing-box-tun",
                "address": "172.19.0.1/30",
                "auto_route": true,
                "strict_route": true
            }],
            "outbounds": [{"type": "direct", "tag": "direct"}]
        });
        let cfg = sanitize_tun_inbounds(user, true);
        let tun = cfg["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|ib| ib["type"] == "tun")
            .unwrap();
        assert_eq!(tun["tag"], "tun-in");
        assert!(
            tun.get("interface_name").is_none()
                || tun["interface_name"].as_str().unwrap_or("").is_empty(),
            "macOS cannot use sing-box-tun; leave name empty so Darwin picks utunN"
        );
    }

    #[test]
    fn test_validate_id() {
        assert!(validate_id("my-profile_123").is_ok());
        assert!(validate_id("default").is_ok());
        assert!(validate_id("").is_err());
        assert!(validate_id("   ").is_err());
        assert!(validate_id("../foo").is_err());
        assert!(validate_id("foo/bar").is_err());
        assert!(validate_id("foo\\bar").is_err());
        assert!(validate_id("../../etc/passwd").is_err());
        assert!(validate_id("profile with spaces").is_err());
        assert!(validate_id("profile;rm -rf").is_err());
    }

    #[test]
    fn test_save_profile_rejects_path_traversal() {
        let malicious_profile = serde_json::json!({
            "id": "../../traversal_test",
            "name": "Evil Profile",
            "type": "local"
        });
        let res = save_profile(&malicious_profile);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("非法路径字符"));
    }

    #[test]
    fn test_save_template_rejects_path_traversal() {
        let malicious_template = serde_json::json!({
            "id": "../../../traversal_template",
            "name": "Evil Template",
            "content": "{}"
        });
        let res = save_template(&malicious_template);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("非法路径字符"));
    }
}
