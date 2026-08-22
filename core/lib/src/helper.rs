use std::path::{Path, PathBuf};
use std::process::Command;

pub struct HelperStatus {
    pub running: bool,
    pub pid: Option<u32>,
}

pub fn is_available() -> bool {
    match run_ctl(&["ping"]) {
        Ok(out) => out.status.success() && out.stdout.contains("pong"),
        Err(_) => false,
    }
}

pub fn start(core: &Path, config: &Path, work: &Path) -> Result<(), String> {
    let out = run_ctl(&[
        "start",
        "--core",
        &core.to_string_lossy(),
        "--config",
        &config.to_string_lossy(),
        "--workdir",
        &work.to_string_lossy(),
    ])
    .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let msg = if out.stderr.is_empty() {
        out.stdout
    } else {
        out.stderr
    };
    Err(if msg.is_empty() {
        format!("helper start failed (exit {})", out.status)
    } else {
        msg
    })
}

pub fn stop() -> Result<(), String> {
    let out = run_ctl(&["stop"]).map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(out.stderr.if_empty(out.stdout))
    }
}

pub fn status() -> Result<HelperStatus, String> {
    let out = run_ctl(&["status"]).map_err(|e| e.to_string())?;
    Ok(parse_helper_status(&out.stdout, out.status.success()))
}

pub fn parse_helper_status(stdout: &str, ok_exit: bool) -> HelperStatus {
    if !ok_exit {
        return HelperStatus {
            running: false,
            pid: None,
        };
    }
    let v: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(_) => {
            return HelperStatus {
                running: stdout.contains("\"running\":true")
                    || stdout.contains("\"running\": true"),
                pid: None,
            };
        }
    };
    let running = v
        .pointer("/data/running")
        .and_then(|x| x.as_bool())
        .or_else(|| v.get("running").and_then(|x| x.as_bool()))
        .unwrap_or(false);
    let pid = v
        .pointer("/data/pid")
        .and_then(|x| x.as_u64())
        .or_else(|| v.get("pid").and_then(|x| x.as_u64()))
        .map(|n| n as u32);
    HelperStatus { running, pid }
}

struct CtlOut {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn run_ctl(args: &[&str]) -> std::io::Result<CtlOut> {
    let exe = discover_helper().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "singpanel-helper not found")
    })?;
    let mut cmd = Command::new(exe);
    cmd.arg("ctl").args(args);
    hide_window(&mut cmd);
    let out = cmd.output()?;
    Ok(CtlOut {
        status: out.status,
        stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
    })
}

fn discover_helper() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SINGPANEL_HELPER") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(p) = helper_from_service() {
        return Some(p);
    }
    let names = if cfg!(windows) {
        ["singpanel-helper.exe", "singpanel-helper"]
    } else {
        ["singpanel-helper", "singpanel-helper.exe"]
    };

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
        cur = dir.parent().map(Path::to_path_buf);
    }
}

/// ImagePath of the installed SingPanelHelper service (same exe used for `ctl`).
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
        if let Some(p) = exe_from_image_path(rest) {
            return Some(p);
        }
    }
    None
}

fn exe_from_image_path(raw: &str) -> Option<PathBuf> {
    let s = raw.trim().trim_matches('"');
    let lower = s.to_ascii_lowercase();
    let end = lower.find(".exe")? + 4;
    let path = s.get(..end)?.trim().trim_matches('"');
    let p = PathBuf::from(path);
    p.is_file().then_some(p)
}

fn hide_window(_cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        _cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

trait IfEmpty {
    fn if_empty(self, other: String) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, other: String) -> String {
        if self.is_empty() {
            other
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_helper_status;

    #[test]
    fn ok_true_is_not_running() {
        let raw = r#"{"id":"1","ok":true,"data":{"running":false}}"#;
        let st = parse_helper_status(raw, true);
        assert!(!st.running);
        assert!(st.pid.is_none());
    }

    #[test]
    fn nested_running_and_pid() {
        let raw = r#"{"id":"1","ok":true,"data":{"running":true,"pid":42}}"#;
        let st = parse_helper_status(raw, true);
        assert!(st.running);
        assert_eq!(st.pid, Some(42));
    }

    #[test]
    fn failed_ctl_is_stopped() {
        let st = parse_helper_status(r#"{"ok":true,"data":{"running":true}}"#, false);
        assert!(!st.running);
    }
}
