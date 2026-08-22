use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct ConvertSidecar {
    child: Option<Child>,
    port: Option<u16>,
    token: Option<String>,
}

impl ConvertSidecar {
    pub fn new() -> Self {
        Self {
            child: None,
            port: None,
            token: None,
        }
    }

    pub fn ensure(&mut self) -> Result<(u16, String), String> {
        if let (Some(p), Some(t)) = (self.port, self.token.clone()) {
            if self
                .child
                .as_mut()
                .and_then(|c| c.try_wait().ok().flatten())
                .is_none()
            {
                return Ok((p, t));
            }
            self.child = None;
            self.port = None;
            self.token = None;
        }
        let exe = discover().ok_or("找不到 singpanel-convert")?;
        let mut cmd = Command::new(&exe);
        cmd.current_dir(exe.parent().unwrap_or(std::path::Path::new(".")))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_window(&mut cmd);
        let mut child = cmd.spawn().map_err(|e| format!("启动 convert: {e}"))?;
        let stdout = child.stdout.take().ok_or("convert 无 stdout")?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(rest) = line.strip_prefix("READY ") {
                    let _ = tx.send(Ok(rest.to_string()));
                    return;
                }
            }
            let _ = tx.send(Err("convert 未输出 READY".into()));
        });
        let line = match rx.recv_timeout(Duration::from_secs(8)) {
            Ok(res) => res.map_err(|e: String| e)?,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("启动 convert 超时 (8s 未输出 READY)".into());
            }
        };
        let port = line
            .split_whitespace()
            .find_map(|p| p.strip_prefix("port=")?.parse().ok())
            .ok_or_else(|| format!("无法解析 port: {line}"))?;
        let token = line
            .split_whitespace()
            .find_map(|p| p.strip_prefix("token=").map(|s| s.to_string()))
            .ok_or_else(|| format!("无法解析 token: {line}"))?;
        self.child = Some(child);
        self.port = Some(port);
        self.token = Some(token.clone());
        // give the listener a tick
        std::thread::sleep(Duration::from_millis(50));
        Ok((port, token))
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.port = None;
        self.token = None;
    }
}

pub async fn convert_body(
    sidecar: &mut ConvertSidecar,
    body: String,
    include: String,
    exclude: String,
) -> serde_json::Value {
    let (port, token) = match sidecar.ensure() {
        Ok(v) => v,
        Err(e) => return serde_json::json!({"ok": false, "error": e}),
    };
    let client = match reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(c) => c,
        Err(e) => return serde_json::json!({"ok": false, "error": e.to_string()}),
    };
    let url = format!("http://127.0.0.1:{port}/v1/convert");
    match client
        .post(url)
        .bearer_auth(token)
        .json(&serde_json::json!({
            "subscriptionBody": body,
            "include": include,
            "exclude": exclude,
        }))
        .send()
        .await
    {
        Ok(res) => {
            let text = res.text().await.unwrap_or_default();
            serde_json::from_str(&text).unwrap_or_else(|_| {
                serde_json::json!({"ok": false, "error": text})
            })
        }
        Err(e) => serde_json::json!({"ok": false, "error": e.to_string()}),
    }
}

fn discover() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SINGPANEL_CONVERT") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    let names = if cfg!(windows) {
        ["singpanel-convert.exe", "singpanel-convert"]
    } else {
        ["singpanel-convert", "singpanel-convert.exe"]
    };
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            dirs.push(dir.join("convert"));
            if let Some(parent) = dir.parent() {
                dirs.push(parent.join("convert"));
            }
        }
        if let Some(host_dir) = exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
        {
            dirs.push(host_dir.join("convert"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("core").join("convert"));
        dirs.push(cwd.join("convert"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidecar_new_and_stop() {
        let mut sidecar = ConvertSidecar::new();
        assert!(sidecar.child.is_none());
        assert!(sidecar.port.is_none());
        assert!(sidecar.token.is_none());
        sidecar.stop();
        assert!(sidecar.child.is_none());
    }
}
