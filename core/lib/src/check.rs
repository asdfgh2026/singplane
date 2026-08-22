use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct TempFileGuard(pub PathBuf);

impl TempFileGuard {
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn create_secure_temp_config(content: &str) -> Result<TempFileGuard, String> {
    let mut rand_bytes = [0u8; 16];
    getrandom::getrandom(&mut rand_bytes)
        .map_err(|e| format!("生成随机文件名失败: {e}"))?;
    let rand_hex: String = rand_bytes.iter().map(|b| format!("{b:02x}")).collect();
    let file_name = format!("singpanel-check-{rand_hex}.json");
    let path = std::env::temp_dir().join(file_name);

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }

    let mut file = opts
        .open(&path)
        .map_err(|e| format!("创建临时配置文件失败: {e}"))?;

    file.write_all(content.as_bytes())
        .map_err(|e| format!("写入临时配置失败: {e}"))?;
    file.flush()
        .map_err(|e| format!("刷新临时配置失败: {e}"))?;

    Ok(TempFileGuard(path))
}

pub fn check_content(core_path: &str, content: &str) -> Result<(), String> {
    let exe = core_path.trim();
    if exe.is_empty() {
        return Ok(());
    }
    if !PathBuf::from(exe).is_file() {
        return Err(format!("找不到内核: {exe}"));
    }
    let body = content.trim();
    if body.is_empty() {
        return Err("配置内容为空".into());
    }

    let guard = create_secure_temp_config(body)?;

    let mut cmd = Command::new(exe);
    cmd.args(["check", "-c"]).arg(guard.path());
    hide_window(&mut cmd);
    let out = cmd.output().map_err(|e| format!("执行 check 失败: {e}"))?;

    // Drop temp file immediately
    drop(guard);

    if out.status.success() {
        return Ok(());
    }
    let msg = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .trim()
    .to_string();
    Err(if msg.is_empty() {
        format!("配置校验失败 (exit {})", out.status)
    } else {
        msg
    })
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
    fn test_temp_file_guard_creates_and_cleans_up() {
        let content = r#"{"log": {"level": "info"}}"#;
        let guard = create_secure_temp_config(content).expect("create secure temp file");
        let path = guard.path().to_path_buf();
        assert!(path.is_file(), "Temp file must exist while guard is active");

        let read_back = fs::read_to_string(&path).expect("read temp file");
        assert_eq!(read_back, content);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(&path).expect("get metadata");
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "Temp file must have 0600 permissions on Unix");
        }

        // Explicit drop triggers cleanup
        drop(guard);
        assert!(!path.exists(), "Temp file must be removed after guard is dropped");
    }

    #[test]
    fn test_check_content_skips_when_no_core_path() {
        assert!(check_content("", "").is_ok());
        assert!(check_content("", "   ").is_ok());
    }

    #[test]
    fn test_check_content_rejects_missing_core() {
        let err = check_content("/nonexistent/sing-box", "{}").unwrap_err();
        assert!(err.contains("找不到内核"), "Error: {err}");
    }
}
