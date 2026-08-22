//! Windows elevated helper (`singpanel-helper`) — SYSTEM service.
//! GUI stays non-admin; one UAC installs the service so TUN can start.

use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct HelperSnap {
    pub exe: Option<PathBuf>,
    pub installed: bool,
    pub available: bool,
}

impl HelperSnap {
    pub fn probe() -> Self {
        let exe = discover_helper();
        let installed = service_installed();
        let available = exe
            .as_ref()
            .is_some_and(|p| ctl(p, &["ping"]).is_some_and(|s| s.contains("pong")));
        Self {
            exe,
            installed,
            available,
        }
    }

    pub fn label(&self) -> &'static str {
        if self.available {
            "可用"
        } else if self.installed {
            "已安装未就绪"
        } else if self.exe.is_some() {
            "未安装"
        } else {
            "找不到 helper"
        }
    }
}

pub fn install_elevated() -> Result<HelperSnap, String> {
    let exe = discover_helper().ok_or("找不到 singpanel-helper.exe，请先编译 core/helper")?;
    let ps_exe = exe.to_string_lossy().replace('\'', "''");
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
try {{
  $p = Start-Process -FilePath '{ps_exe}' -ArgumentList @('install') -Verb RunAs -Wait -PassThru -WindowStyle Hidden
  if ($null -eq $p) {{ exit 2 }}
  exit $p.ExitCode
}} catch {{
  Write-Error $_.Exception.Message
  exit 1
}}
"#
    );
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &script,
    ]);
    hide_window(&mut cmd);
    let out = cmd.output().map_err(|e| format!("UAC 启动失败: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let detail = if err.is_empty() { stdout } else { err };
        return Err(if detail.is_empty() {
            format!("安装未完成（exit {}）", out.status)
        } else {
            detail
        });
    }
    let snap = HelperSnap::probe();
    if !snap.available {
        return Err("安装后服务不可用，请到设置查看或重试".into());
    }
    Ok(snap)
}

fn service_installed() -> bool {
    let mut cmd = Command::new("sc");
    cmd.args(["query", "SingPanelHelper"]);
    hide_window(&mut cmd);
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

fn ctl(exe: &std::path::Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(exe);
    cmd.arg("ctl").args(args);
    hide_window(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn discover_helper() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SINGPANEL_HELPER") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(p) = helper_from_service() {
        return Some(p);
    }
    let names = ["singpanel-helper.exe", "singpanel-helper"];
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            push_search_roots(dir.to_path_buf(), &mut dirs);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        push_search_roots(cwd, &mut dirs);
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

fn push_search_roots(start: PathBuf, dirs: &mut Vec<PathBuf>) {
    let mut cur = Some(start);
    let mut hops = 0;
    while let Some(dir) = cur {
        dirs.push(dir.clone());
        dirs.push(dir.join("helper"));
        dirs.push(dir.join("core").join("helper"));
        hops += 1;
        if hops > 8 {
            break;
        }
        cur = dir.parent().map(PathBuf::from);
    }
}

fn helper_from_service() -> Option<PathBuf> {
    let mut cmd = Command::new("sc");
    cmd.args(["qc", "SingPanelHelper"]);
    hide_window(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let marker = line.to_ascii_lowercase();
        if !(marker.contains("binary_path_name") || marker.contains("binpath")) {
            continue;
        }
        let rest = line.split_once(':').map(|(_, r)| r).unwrap_or(line);
        let s = rest.trim().trim_matches('"');
        let lower = s.to_ascii_lowercase();
        let end = lower.find(".exe").map(|i| i + 4)?;
        let path = s.get(..end)?.trim().trim_matches('"');
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
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
