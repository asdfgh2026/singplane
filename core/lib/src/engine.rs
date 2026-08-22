use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use crate::helper::{self, HelperStatus};

const CORE_LOG: &str = "sing-box.core.log";
const SETTLE: Duration = Duration::from_millis(250);
/// Return once the process is up. Late FATAL (rule-set + TUN) is
/// watched by the UI so `/v1/start` is not blocked for tens of seconds.
const WAIT_CRASH: Duration = Duration::from_secs(2);

pub struct Engine {
    child: Option<Child>,
    via_helper: bool,
    pid: Option<u32>,
    config_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct EngineError {
    pub code: String,
    pub message: String,
}

#[derive(Clone)]
pub struct StartSpec {
    pub core_path: String,
    pub config_path: String,
    pub require_helper: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnap {
    pub ok: bool,
    pub running: bool,
    pub via_helper: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            child: None,
            via_helper: false,
            pid: None,
            config_path: None,
        }
    }

    pub fn snapshot(&mut self) -> StatusSnap {
        self.refresh();
        StatusSnap {
            ok: true,
            running: self.is_running(),
            via_helper: self.via_helper,
            pid: self.pid,
            config_path: self
                .config_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
        }
    }

    pub fn start(&mut self, spec: StartSpec) -> Result<StatusSnap, EngineError> {
        if self.is_running() {
            return Err(EngineError {
                code: "already_running".into(),
                message: "内核已在运行".into(),
            });
        }

        let core = PathBuf::from(spec.core_path.trim());
        let config = PathBuf::from(spec.config_path.trim());
        if !core.is_file() {
            return Err(EngineError {
                code: "core_missing".into(),
                message: format!("找不到内核文件: {}", core.display()),
            });
        }
        if !config.is_file() {
            return Err(EngineError {
                code: "config_missing".into(),
                message: format!("找不到配置: {}", config.display()),
            });
        }

        let needs_tun = config_has_tun(&config);
        let helper_ok = helper::is_available();
        // TUN + privileged helper is the Windows pattern.
        // macOS / Linux start the official binary directly; the kernel creates utun.
        if cfg!(windows) && (spec.require_helper || needs_tun) && !helper_ok {
            return Err(EngineError {
                code: "need_helper".into(),
                message: need_helper_message(needs_tun),
            });
        }

        // Previous GPUI/host kills leave the official core listening (EADDRINUSE).
        reclaim_previous(&core, &config);

        if helper_ok {
            self.start_via_helper(&core, &config)?;
        } else {
            self.start_direct(&core, &config)?;
        }

        self.wait_ready(needs_tun)
    }

    pub fn stop(&mut self) -> StatusSnap {
        if self.via_helper {
            let _ = helper::stop();
        }
        if let Some(child) = self.child.as_mut() {
            let pid = child.id();
            kill_tree(pid);
            let _ = child.kill();
            let _ = child.wait();
        }
        self.clear();
        self.snapshot()
    }

    fn start_via_helper(&mut self, core: &Path, config: &Path) -> Result<(), EngineError> {
        let work = config.parent().unwrap_or_else(|| Path::new("."));
        helper::start(core, config, work).map_err(|e| EngineError {
            code: "helper_start".into(),
            message: e,
        })?;
        thread::sleep(SETTLE);
        let st = helper::status().unwrap_or(HelperStatus {
            running: false,
            pid: None,
        });
        if !st.running {
            return Err(EngineError {
                code: "helper_start".into(),
                message: format!(
                    "{}{}",
                    "后台服务未能拉起连接。请到「设置 → 全局接管」检查是否已安装权限服务。",
                    log_suffix(Some(config)),
                ),
            });
        }
        self.via_helper = true;
        self.pid = st.pid;
        self.config_path = Some(config.to_path_buf());
        self.child = None;
        Ok(())
    }

    fn start_direct(&mut self, core: &Path, config: &Path) -> Result<(), EngineError> {
        let work = config.parent().unwrap_or_else(|| Path::new("."));
        let log_path = work.join(CORE_LOG);
        let log = File::create(&log_path).map_err(|e| EngineError {
            code: "log".into(),
            message: format!("无法写入内核日志: {e}"),
        })?;
        let log_err = log.try_clone().map_err(|e| EngineError {
            code: "log".into(),
            message: format!("无法复制日志句柄: {e}"),
        })?;

        let mut cmd = Command::new(core);
        cmd.args(["run", "-c"])
            .arg(config)
            .current_dir(work)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err));
        hide_window(&mut cmd);

        let child = cmd.spawn().map_err(|e| EngineError {
            code: "spawn".into(),
            message: format!("启动内核失败: {e}"),
        })?;
        self.pid = Some(child.id());
        self.child = Some(child);
        self.via_helper = false;
        self.config_path = Some(config.to_path_buf());
        Ok(())
    }

    fn refresh(&mut self) {
        if self.via_helper {
            match helper::status() {
                Ok(st) if st.running => {
                    self.pid = st.pid.or(self.pid);
                }
                _ => self.clear(),
            }
            return;
        }
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) | Err(_) => self.clear(),
                Ok(None) => {}
            }
        }
    }

    fn is_running(&self) -> bool {
        self.via_helper || self.child.is_some()
    }

    fn clear(&mut self) {
        self.child = None;
        self.via_helper = false;
        self.pid = None;
        self.config_path = None;
    }

    fn wait_ready(&mut self, _needs_tun: bool) -> Result<StatusSnap, EngineError> {
        let deadline = Instant::now() + WAIT_CRASH;
        let config = self.config_path.clone();
        loop {
            thread::sleep(SETTLE);
            self.refresh();
            if !self.is_running() {
                let msg = exited_message(config.as_deref());
                self.clear();
                return Err(EngineError {
                    code: "exited".into(),
                    message: msg,
                });
            }
            if let Some(fatal) = log_fatal_line(config.as_deref()) {
                thread::sleep(SETTLE);
                self.refresh();
                if !self.is_running() {
                    self.clear();
                    return Err(EngineError {
                        code: "exited".into(),
                        message: decorate_fatal(&fatal),
                    });
                }
            }
            if Instant::now() >= deadline {
                break;
            }
            if log_looks_started(config.as_deref()) && log_fatal_line(config.as_deref()).is_none() {
                thread::sleep(SETTLE);
                self.refresh();
                if !self.is_running() {
                    let msg = exited_message(config.as_deref());
                    self.clear();
                    return Err(EngineError {
                        code: "exited".into(),
                        message: msg,
                    });
                }
                break;
            }
        }
        Ok(self.snapshot())
    }
}

fn need_helper_message(needs_tun: bool) -> String {
    if needs_tun {
        "当前配置含 TUN 虚拟网卡。Windows 上必须用「全局接管」权限服务（SYSTEM）启动，请到设置安装。".into()
    } else {
        "当前配置需要「全局接管」网络（虚拟网卡）。请安装权限服务。".into()
    }
}

pub fn config_has_tun(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    json_has_tun(&v)
}

pub fn json_has_tun(v: &Value) -> bool {
    v.get("inbounds")
        .and_then(|x| x.as_array())
        .is_some_and(|arr| {
            arr.iter()
                .any(|ib| ib.get("type").and_then(|t| t.as_str()) == Some("tun"))
        })
}

fn core_log_path(config: Option<&Path>) -> PathBuf {
    config
        .and_then(|p| p.parent())
        .map(|d| d.join(CORE_LOG))
        .unwrap_or_else(|| PathBuf::from(CORE_LOG))
}

fn read_core_log(config: Option<&Path>) -> String {
    fs::read_to_string(core_log_path(config)).unwrap_or_default()
}

fn log_fatal_line(config: Option<&Path>) -> Option<String> {
    read_core_log(config)
        .lines()
        .rev()
        .find(|l| l.contains("FATAL"))
        .map(str::trim)
        .map(ToString::to_string)
}

fn log_looks_started(config: Option<&Path>) -> bool {
    let text = read_core_log(config);
    text.contains("sing-box started") || text.contains("inbound/tun") && text.contains("started")
}

fn log_suffix(config: Option<&Path>) -> String {
    if let Some(fatal) = log_fatal_line(config) {
        return format!("\n{fatal}");
    }
    let tail = last_log_lines(&read_core_log(config), 4);
    if tail.is_empty() {
        String::new()
    } else {
        format!("\n{tail}")
    }
}

fn last_log_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

fn decorate_fatal(fatal: &str) -> String {
    let lower = fatal.to_ascii_lowercase();
    if lower.contains("access is denied") || fatal.contains("拒绝访问") {
        format!(
            "{fatal}\nWindows 创建 TUN 需要管理员权限。请先安装权限服务，再开虚拟网卡。"
        )
    } else if lower.contains("address already in use") || lower.contains("only one usage of each socket address")
    {
        format!(
            "{fatal}\n入站端口已被占用。多半是上次面板没把内核停干净；再点一次启动即可，或先关掉占用该端口的 sing-box。"
        )
    } else if lower.contains("operation not permitted") || lower.contains("permission denied")
    {
        format!(
            "{fatal}\n创建虚拟网卡需要管理员权限。请打开「虚拟网卡」并在系统弹窗输入密码（会给内核加 setuid）。"
        )
    } else {
        fatal.to_string()
    }
}

fn exited_message(config: Option<&Path>) -> String {
    if let Some(fatal) = log_fatal_line(config) {
        return decorate_fatal(&fatal);
    }
    let tail = last_log_lines(&read_core_log(config), 4);
    if tail.is_empty() {
        "内核未能保持运行".into()
    } else {
        format!("内核未能保持运行\n{tail}")
    }
}

fn hide_window(_cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        _cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tun_inbound() {
        let v: Value = serde_json::json!({
            "inbounds": [
                {"type": "mixed", "tag": "mix"},
                {"type": "tun", "tag": "tun"}
            ]
        });
        assert!(json_has_tun(&v));
        let plain: Value = serde_json::json!({"inbounds": [{"type": "mixed"}]});
        assert!(!json_has_tun(&plain));
    }

    #[test]
    fn fatal_line_is_last_fatal() {
        let text = "\
INFO started something\n\
FATAL start inbound/tun[tun]: configure tun interface: Access is denied.\n\
";
        let line = text
            .lines()
            .rev()
            .find(|l| l.contains("FATAL"))
            .unwrap();
        assert!(line.contains("Access is denied"));
        assert!(decorate_fatal(line).contains("权限服务"));
    }

    #[test]
    fn matches_leftover_core_command() {
        let core = "/volumes/ssd/dev/singplane/singpanel/singpanel/cores/sing-box";
        let cfg = "/volumes/ssd/dev/singplane/singpanel/singpanel/runtime/config.runtime.json";
        assert!(command_is_our_core(
            "/Volumes/SSD/dev/singplane/SingPanel/SingPanel/cores/sing-box run -c /Volumes/SSD/dev/singplane/SingPanel/SingPanel/runtime/config.runtime.json",
            core,
            cfg
        ));
        assert!(!command_is_our_core(
            "/Volumes/SSD/dev/singplane/core/host/target/debug/singpanel-host",
            core,
            cfg
        ));
        assert_eq!(
            parse_ps_line(" 16259 /opt/sing-box run -c /tmp/a.json").unwrap().0,
            16259
        );
    }
}

fn reclaim_previous(core: &Path, config: &Path) {
    let mut pids = leftover_core_pids(core, config);
    if pids.is_empty() {
        return;
    }
    for pid in &pids {
        kill_tree(*pid);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(150));
        pids = leftover_core_pids(core, config);
        if pids.is_empty() {
            return;
        }
    }
    for pid in pids {
        kill_force(pid);
    }
    thread::sleep(Duration::from_millis(200));
}

fn leftover_core_pids(core: &Path, config: &Path) -> Vec<u32> {
    let self_pid = std::process::id();
    let core_key = path_key(core);
    let config_key = path_key(config);
    list_processes()
        .into_iter()
        .filter(|(pid, cmd)| {
            *pid != self_pid && command_is_our_core(cmd, &core_key, &config_key)
        })
        .map(|(pid, _)| pid)
        .collect()
}

fn path_key(path: &Path) -> String {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    normalize_cmd(&resolved.to_string_lossy())
}

fn normalize_cmd(s: &str) -> String {
    s.replace('\\', "/").to_ascii_lowercase()
}

fn command_is_our_core(cmd: &str, core_key: &str, config_key: &str) -> bool {
    let cmd = normalize_cmd(cmd);
    if cmd.contains("singpanel-host") || cmd.contains("singpanel-gpui") {
        return false;
    }
    let looks_like_run = cmd.contains(" run ") || cmd.ends_with(" run") || cmd.contains(" run\t");
    if !looks_like_run {
        return false;
    }
    let has_core = !core_key.is_empty() && cmd.contains(core_key);
    let has_cfg = !config_key.is_empty() && cmd.contains(config_key);
    has_core || (cmd.contains("sing-box") && has_cfg)
}

fn parse_ps_line(line: &str) -> Option<(u32, String)> {
    let line = line.trim();
    let (pid, cmd) = line.split_once(char::is_whitespace)?;
    let pid = pid.parse().ok()?;
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return None;
    }
    Some((pid, cmd.to_string()))
}

fn list_processes() -> Vec<(u32, String)> {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("wmic");
        cmd.args([
            "process",
            "get",
            "ProcessId,CommandLine",
            "/FORMAT:CSV",
        ]);
        hide_window(&mut cmd);
        let Ok(out) = cmd.output() else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(parse_wmic_csv_line)
            .collect()
    }
    #[cfg(not(windows))]
    {
        let Ok(out) = Command::new("ps")
            .args(["-axww", "-o", "pid=,command="])
            .output()
        else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(parse_ps_line)
            .collect()
    }
}

#[cfg(windows)]
fn parse_wmic_csv_line(line: &str) -> Option<(u32, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("Node,") {
        return None;
    }
    let mut cols: Vec<&str> = line.split(',').collect();
    if cols.len() < 3 {
        return None;
    }
    let pid = cols.pop()?.trim().parse().ok()?;
    let cmd = cols[1..].join(",").trim().to_string();
    if cmd.is_empty() {
        return None;
    }
    Some((pid, cmd))
}

fn kill_force(pid: u32) {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
        hide_window(&mut cmd);
        let _ = cmd.status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
}

fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
        hide_window(&mut cmd);
        let _ = cmd.status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}
