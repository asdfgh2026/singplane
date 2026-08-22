use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct Status {
    pub host_ok: bool,
    pub running: bool,
    pub via_helper: bool,
    pub pid: Option<u32>,
    pub config_path: Option<String>,
    pub error: Option<String>,
    pub code: Option<String>,
}

/// Generic host JSON call. `data` is the full decoded object.
#[derive(Debug, Clone)]
pub struct HostResult {
    pub ok: bool,
    pub error: Option<String>,
    pub code: Option<String>,
    pub data: Value,
}

impl HostResult {
    pub fn err(msg: impl Into<String>) -> Self {
        let error = msg.into();
        Self {
            ok: false,
            error: Some(error.clone()),
            code: None,
            data: serde_json::json!({"ok": false, "error": error}),
        }
    }

    pub fn from_value(data: Value) -> Self {
        Self {
            ok: data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
            error: data
                .get("error")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            code: data.get("code").and_then(|v| v.as_str()).map(str::to_string),
            data,
        }
    }

    pub fn str(&self, key: &str) -> Option<String> {
        self.data.get(key).and_then(|v| v.as_str()).map(str::to_string)
    }

    pub fn i64(&self, key: &str) -> Option<i64> {
        self.data.get(key).and_then(|v| v.as_i64())
    }
}

pub(crate) fn clash_call_error(r: &HostResult) -> String {
    if let Some(e) = r.error.as_deref().filter(|s| !s.is_empty()) {
        return e.to_string();
    }
    let status = r.data.get("status").and_then(Value::as_u64);
    let message = r
        .data
        .get("json")
        .and_then(|j| j.get("message"))
        .and_then(Value::as_str)
        .or_else(|| r.data.get("raw").and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match (status, message) {
        (Some(401), _) => "Clash API 需要密钥（当前配置带了 secret）".into(),
        (Some(s), Some(m)) => format!("clash {s}: {m}"),
        (Some(s), None) => format!("clash 调用失败 ({s})"),
        (None, Some(m)) => m.to_string(),
        (None, None) => "clash 调用失败".into(),
    }
}

pub(crate) fn clash_call_result(r: HostResult) -> Result<HostResult, String> {
    if r.ok {
        Ok(r)
    } else {
        Err(clash_call_error(&r))
    }
}

#[cfg(test)]
mod clash_error_tests {
    use super::*;

    #[test]
    fn unauthorized_mentions_secret() {
        let r = HostResult::from_value(serde_json::json!({
            "ok": false,
            "status": 401,
            "json": { "message": "Unauthorized" },
            "raw": "{\"message\":\"Unauthorized\"}"
        }));
        assert!(clash_call_error(&r).contains("密钥"), "{}", clash_call_error(&r));
    }

    #[test]
    fn other_status_keeps_message() {
        let r = HostResult::from_value(serde_json::json!({
            "ok": false,
            "status": 500,
            "json": { "message": "boom" }
        }));
        assert_eq!(clash_call_error(&r), "clash 500: boom");
    }

    #[test]
    fn direct_clash_results_use_http_error_details() {
        let unauthorized = HostResult::from_value(serde_json::json!({
            "ok": false,
            "status": 401,
            "raw": "Unauthorized"
        }));
        assert!(clash_call_result(unauthorized).unwrap_err().contains("密钥"));

        let server_error = HostResult::from_value(serde_json::json!({
            "ok": false,
            "status": 500,
            "json": { "message": "controller failed" }
        }));
        assert_eq!(
            clash_call_result(server_error).unwrap_err(),
            "clash 500: controller failed"
        );
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusJson {
    ok: Option<bool>,
    running: Option<bool>,
    via_helper: Option<bool>,
    pid: Option<u32>,
    config_path: Option<String>,
    error: Option<String>,
    code: Option<String>,
}

pub struct HostClient {
    inner: Mutex<Inner>,
}

struct Inner {
    child: Option<Child>,
    port: Option<u16>,
    token: Option<String>,
    http: reqwest::blocking::Client,
}

impl HostClient {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                child: None,
                port: None,
                token: None,
                http: reqwest::blocking::Client::builder()
                    .no_proxy()
                    .timeout(Duration::from_secs(60))
                    .build()
                    .expect("http client"),
            }),
        }
    }

    pub fn ensure(&self) -> Result<(), String> {
        let mut g = self.inner.lock().map_err(|_| "host lock")?;
        if g.port.is_some() {
            if let Some(child) = g.child.as_mut() {
                if child.try_wait().ok().flatten().is_none() {
                    return Ok(());
                }
            }
            g.child = None;
            g.port = None;
            g.token = None;
        }
        let exe = discover_host().ok_or("找不到 singpanel-host.exe（先 cargo build -p 于 core/host）")?;
        reap_stale_hosts(&exe);
        let mut cmd = Command::new(&exe);
        cmd.current_dir(exe.parent().unwrap_or(Path::new(".")))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(helper) = crate::win_helper::discover_helper() {
            cmd.env("SINGPANEL_HELPER", helper);
        }
        hide_window(&mut cmd);
        let mut child = cmd.spawn().map_err(|e| format!("启动 host: {e}"))?;
        let stdout = child.stdout.take().ok_or("host 无 stdout")?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(rest) = line.strip_prefix("READY ") {
                    let _ = tx.send(Ok(rest.to_string()));
                    return;
                }
            }
            let _ = tx.send(Err("host 未输出 READY".into()));
        });
        let line = match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(res) => res.map_err(|e: String| e)?,
            Err(_) => {
                HostClient::shutdown_child(&mut child);
                return Err("启动 host 超时 (10s 未输出 READY)".into());
            }
        };
        let port = line
            .split_whitespace()
            .find_map(|p| p.strip_prefix("port=")?.parse().ok())
            .ok_or_else(|| format!("无法解析 port: {line}"))?;
        let token = line
            .split_whitespace()
            .find_map(|p| p.strip_prefix("token=").map(str::to_string))
            .ok_or_else(|| format!("无法解析 token: {line}"))?;
        g.child = Some(child);
        g.port = Some(port);
        g.token = Some(token);
        Ok(())
    }

    /// Kill the current host child so the next `ensure` picks up helper env/paths.
    fn shutdown_child(child: &mut Child) {
        #[cfg(unix)]
        {
            let _ = Command::new("kill")
                .args(["-TERM", &child.id().to_string()])
                .status();
            let started = std::time::Instant::now();
            while started.elapsed() < Duration::from_secs(2) {
                if child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for HostClient {
    fn drop(&mut self) {
        self.shutdown_now();
    }
}

fn reap_stale_hosts(exe: &Path) {
    let Ok(key) = exe.canonicalize() else {
        return;
    };
    let key = key.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
    let Ok(out) = Command::new("ps")
        .args(["-axww", "-o", "pid=,command="])
        .output()
    else {
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/IM", "singpanel-host.exe", "/F"])
                .status();
        }
        return;
    };
    let self_pid = std::process::id();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let line = line.trim();
        let Some((pid_s, cmd)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let Ok(pid) = pid_s.parse::<u32>() else {
            continue;
        };
        if pid == self_pid {
            continue;
        }
        let cmd_n = cmd.replace('\\', "/").to_ascii_lowercase();
        if !cmd_n.contains(&key) {
            continue;
        }
        #[cfg(unix)]
        {
            let _ = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
        }
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .status();
        }
    }
    std::thread::sleep(Duration::from_millis(250));
}

impl HostClient {
    pub fn recycle(&self) -> Result<(), String> {
        {
            let mut g = self.inner.lock().map_err(|_| "host lock")?;
            if let Some(mut child) = g.child.take() {
                HostClient::shutdown_child(&mut child);
            }
            g.port = None;
            g.token = None;
        }
        self.ensure()
    }

    pub fn ping(&self) -> HostResult {
        match self.call("GET", "/v1/ping", None, Duration::from_secs(8)) {
            Ok(v) => HostResult::from_value(v),
            Err(e) => HostResult::err(e),
        }
    }

    pub fn status(&self) -> Status {
        match self.call("GET", "/v1/status", None, Duration::from_secs(8)) {
            Ok(v) => status_from_value(v),
            Err(e) => Status {
                host_ok: false,
                error: Some(e),
                ..Status::default()
            },
        }
    }

    pub fn start(&self, core_path: &str, config_path: &str, require_helper: bool) -> Status {
        let body = serde_json::json!({
            "corePath": core_path,
            "configPath": config_path,
            "requireHelper": require_helper,
        });
        match self.call("POST", "/v1/start", Some(body), Duration::from_secs(15)) {
            Ok(v) => status_from_value(v),
            Err(e) => Status {
                host_ok: false,
                error: Some(e),
                ..Status::default()
            },
        }
    }

    pub fn stop(&self) -> Status {
        match self.call(
            "POST",
            "/v1/stop",
            Some(serde_json::json!({})),
            Duration::from_secs(20),
        ) {
            Ok(_) => self.status(),
            Err(e) => Status {
                host_ok: false,
                error: Some(e),
                ..Status::default()
            },
        }
    }

    /// Stop the core and the host process without restarting host (`call` would `ensure`).
    pub fn shutdown_now(&self) {
        let endpoint = self.inner.lock().ok().and_then(|g| {
            Some((g.http.clone(), g.port?, g.token.clone()?))
        });
        if let Some((http, port, token)) = endpoint {
            let url = format!("http://127.0.0.1:{port}/v1/stop");
            let _ = http
                .post(&url)
                .bearer_auth(token)
                .timeout(Duration::from_secs(2))
                .json(&serde_json::json!({}))
                .send();
        }
        if let Ok(mut g) = self.inner.lock() {
            if let Some(mut child) = g.child.take() {
                HostClient::shutdown_child(&mut child);
            }
            g.port = None;
            g.token = None;
        }
    }

    pub fn check(&self, core_path: &str, content: &str) -> HostResult {
        let body = serde_json::json!({
            "corePath": core_path,
            "content": content,
        });
        self.post("/v1/check", body, Duration::from_secs(45))
    }

    pub fn fetch(&self, url: &str) -> HostResult {
        self.post(
            "/v1/fetch",
            serde_json::json!({ "url": url }),
            Duration::from_secs(45),
        )
    }

    pub fn convert(&self, subscription_body: &str, include: &str, exclude: &str) -> HostResult {
        self.post(
            "/v1/convert",
            serde_json::json!({
                "subscriptionBody": subscription_body,
                "include": include,
                "exclude": exclude,
            }),
            Duration::from_secs(90),
        )
    }

    pub fn assemble(
        &self,
        source_body: &str,
        template_content: &str,
        options: Value,
        patch: Value,
        content_kind: Option<&str>,
        convert_if_needed: bool,
    ) -> HostResult {
        let mut body = serde_json::json!({
            "sourceBody": source_body,
            "templateContent": template_content,
            "options": options,
            "patch": patch,
            "convertIfNeeded": convert_if_needed,
        });
        if let Some(kind) = content_kind {
            body["contentKind"] = Value::String(kind.to_string());
        }
        self.post("/v1/assemble", body, Duration::from_secs(90))
    }

    /// Proxy a Clash API call through the host (`POST /v1/clash`).
    pub fn clash(
        &self,
        base_url: &str,
        secret: &str,
        method: &str,
        path: &str,
        query: Option<Value>,
        body: Option<Value>,
        timeout_ms: Option<u64>,
    ) -> HostResult {
        let payload = serde_json::json!({
            "baseUrl": base_url,
            "secret": secret,
            "method": method,
            "path": path,
            "query": query.unwrap_or(serde_json::json!({})),
            "body": body,
            "timeoutMs": timeout_ms.unwrap_or(8000),
        });
        let outer_timeout = Duration::from_millis(timeout_ms.unwrap_or(8000) + 4000);
        self.post("/v1/clash", payload, outer_timeout)
    }

    /// Clash inner JSON (`data.json`) when the proxy call succeeded.
    pub fn clash_json(
        &self,
        base_url: &str,
        secret: &str,
        method: &str,
        path: &str,
        query: Option<Value>,
        body: Option<Value>,
        timeout_ms: Option<u64>,
    ) -> Result<Value, String> {
        let r = clash_call_result(self.clash(base_url, secret, method, path, query, body, timeout_ms))?;
        if let Some(inner) = r.data.get("json").cloned() {
            return Ok(inner);
        }
        if let Some(raw) = r.data.get("raw").and_then(|v| v.as_str()) {
            if raw.trim().is_empty() {
                return Ok(Value::Null);
            }
            return serde_json::from_str(raw).map_err(|e| e.to_string());
        }
        Ok(r.data)
    }

    fn post(&self, path: &str, body: Value, timeout: Duration) -> HostResult {
        match self.call("POST", path, Some(body), timeout) {
            Ok(v) => HostResult::from_value(v),
            Err(e) => HostResult::err(e),
        }
    }

    fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        timeout: Duration,
    ) -> Result<Value, String> {
        self.ensure()?;
        let g = self.inner.lock().map_err(|_| "host lock")?;
        let port = g.port.ok_or("host 未就绪")?;
        let token = g.token.as_deref().ok_or("host 无 token")?;
        let url = format!("http://127.0.0.1:{port}{path}");
        let mut req = match method {
            "GET" => g.http.get(&url),
            _ => g.http.post(&url),
        };
        req = req.bearer_auth(token).timeout(timeout);
        if let Some(body) = body {
            req = req.json(&body);
        }
        let res = req.send().map_err(|e| e.to_string())?;
        let text = res.text().map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| format!("{e}: {text}"))
    }
}

fn status_from_value(v: Value) -> Status {
    let parsed: StatusJson = serde_json::from_value(v.clone()).unwrap_or(StatusJson {
        ok: v.get("ok").and_then(|x| x.as_bool()),
        running: v.get("running").and_then(|x| x.as_bool()),
        via_helper: v.get("viaHelper").and_then(|x| x.as_bool()),
        pid: v.get("pid").and_then(|x| x.as_u64()).map(|n| n as u32),
        config_path: v
            .get("configPath")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        error: v.get("error").and_then(|x| x.as_str()).map(str::to_string),
        code: v.get("code").and_then(|x| x.as_str()).map(str::to_string),
    });
    Status {
        host_ok: parsed.ok.unwrap_or(false),
        running: parsed.running.unwrap_or(false),
        via_helper: parsed.via_helper.unwrap_or(false),
        pid: parsed.pid,
        config_path: parsed.config_path,
        error: parsed.error,
        code: parsed.code.or_else(|| {
            v.get("code")
                .and_then(|x| x.as_str())
                .map(str::to_string)
        }),
    }
}

/// Percent-encode a Clash proxy name for `/proxies/{name}`.
pub fn clash_encode_name(name: &str) -> String {
    let mut out = String::new();
    for b in name.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn default_core_path() -> PathBuf {
    app_root().join("cores").join(if cfg!(windows) {
        "sing-box.exe"
    } else {
        "sing-box"
    })
}

pub fn default_config_path() -> PathBuf {
    runtime_dir().join("config.runtime.json")
}

pub fn core_log_path() -> PathBuf {
    runtime_dir().join("sing-box.core.log")
}

/// Last `max_bytes` of the kernel log (UTF-8 lossy). Empty if missing.
pub fn read_core_log_tail(max_bytes: usize) -> String {
    let path = core_log_path();
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let slice = if bytes.len() > max_bytes {
        &bytes[bytes.len() - max_bytes..]
    } else {
        &bytes
    };
    String::from_utf8_lossy(slice).into_owned()
}

pub fn runtime_dir() -> PathBuf {
    app_root().join("runtime")
}

pub fn app_root() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let root = base.join("SingPanel").join("SingPanel");
    std::fs::canonicalize(&root).unwrap_or_else(|_| {
        if root.is_absolute() {
            root
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&root))
                .unwrap_or(root)
        }
    })
}

/// Resolve a stored path against the UI process cwd. Host has a different cwd.
pub fn abs_path(p: impl AsRef<Path>) -> PathBuf {
    let p = p.as_ref();
    if p.as_os_str().is_empty() {
        return p.to_path_buf();
    }
    let candidate = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    };
    std::fs::canonicalize(&candidate).unwrap_or(candidate)
}

fn discover_host() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SINGPANEL_HOST") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    let names = if cfg!(windows) {
        ["singpanel-host.exe", "singpanel-host"]
    } else {
        ["singpanel-host", "singpanel-host.exe"]
    };
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            dirs.push(dir.join("host"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("core").join("host").join("target").join("debug"));
        dirs.push(cwd.join("core").join("host").join("target").join("release"));
        dirs.push(
            cwd.join("..")
                .join("core")
                .join("host")
                .join("target")
                .join("debug"),
        );
        dirs.push(
            cwd.join("..")
                .join("core")
                .join("host")
                .join("target")
                .join("release"),
        );
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(repo) = exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
        {
            dirs.push(repo.join("core").join("host").join("target").join("debug"));
            dirs.push(repo.join("core").join("host").join("target").join("release"));
        }
    }
    for dir in dirs {
        for name in names {
            let cand = dir.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

fn hide_window(_cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        _cmd.creation_flags(CREATE_NO_WINDOW);
    }
}
