#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};


pub fn program_data_dir() -> Result<PathBuf, String> {
    let base = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    let dir = PathBuf::from(base).join("SingPanel");
    fs::create_dir_all(&dir).map_err(|e| format!("create program data dir: {}", e))?;
    Ok(dir)
}

pub fn token_path() -> Result<PathBuf, String> {
    Ok(program_data_dir()?.join("helper.token"))
}

pub fn owner_path() -> Result<PathBuf, String> {
    Ok(program_data_dir()?.join("helper.owner"))
}

pub fn allow_path() -> Result<PathBuf, String> {
    Ok(program_data_dir()?.join("helper.allow"))
}

pub fn generate_and_save_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("generate token: {}", e))?;
    let tok = hex::encode(bytes);

    let path = token_path()?;
    persist_restricted(&path, tok.as_bytes()).map_err(|e| format!("token acl: {e}"))?;
    Ok(tok)
}

pub fn current_user_sid_string() -> Result<String, String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
        use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
        use windows::Win32::Security::{
            GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER,
        };
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
        use windows::core::PWSTR;

        unsafe {
            let mut token_handle = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle)
                .map_err(|e| format!("open process token: {}", e))?;

            let mut return_length = 0u32;
            let _ = GetTokenInformation(token_handle, TokenUser, None, 0, &mut return_length);
            if return_length == 0 {
                let _ = CloseHandle(token_handle);
                return Err("zero token info length".to_string());
            }

            let mut buf = vec![0u8; return_length as usize];
            let res = GetTokenInformation(
                token_handle,
                TokenUser,
                Some(buf.as_mut_ptr() as _),
                return_length,
                &mut return_length,
            );
            let _ = CloseHandle(token_handle);
            res.map_err(|e| format!("get token info: {}", e))?;

            let token_user = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut sid_string = PWSTR::null();
            ConvertSidToStringSidW(token_user.User.Sid, &mut sid_string)
                .map_err(|e| format!("convert sid to string: {}", e))?;

            let s = sid_string.to_string().map_err(|e| format!("sid string conversion: {}", e))?;
            let _ = LocalFree(HLOCAL(sid_string.0 as _));
            Ok(s)
        }
    }

    #[cfg(not(windows))]
    {
        Ok("S-1-5-21-0-0-0-1000".to_string())
    }
}

pub fn save_owner_sid() -> Result<(), String> {
    let sid = current_user_sid_string()?;
    let path = owner_path()?;
    persist_restricted(&path, format!("{sid}\n").as_bytes()).map_err(|e| format!("owner acl: {e}"))?;
    Ok(())
}

pub fn load_owner_sid_string() -> Option<String> {
    let path = owner_path().ok()?;
    let b = fs::read_to_string(path).ok()?;
    let s = b.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

pub fn default_allowed_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for key in ["USERPROFILE", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(v) = std::env::var(key) {
            if !v.trim().is_empty() {
                roots.push(PathBuf::from(v.trim()));
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
    }
    roots
}

pub fn save_allowed_roots() -> Result<(), String> {
    let path = allow_path()?;
    let roots = default_allowed_roots();
    let mut content = String::new();
    for r in roots {
        content.push_str(&r.to_string_lossy());
        content.push('\n');
    }
    persist_restricted(&path, content.as_bytes()).map_err(|e| format!("allow list acl: {e}"))?;
    Ok(())
}

pub fn coalesce_allowed_roots(from_file: Vec<PathBuf>) -> Vec<PathBuf> {
    if from_file.is_empty() {
        default_allowed_roots()
    } else {
        from_file
    }
}

pub fn load_allowed_roots() -> Vec<PathBuf> {
    let from_file = match allow_path().ok().and_then(|p| fs::read_to_string(p).ok()) {
        Some(content) => content
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect(),
        None => Vec::new(),
    };
    coalesce_allowed_roots(from_file)
}

pub(crate) fn persist_restricted(path: &Path, data: &[u8]) -> Result<(), String> {
    persist_restricted_with(path, data, restrict_file_read_acl)
}

pub(crate) fn persist_restricted_with(
    path: &Path,
    data: &[u8],
    restrict: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    fs::write(path, data).map_err(|e| format!("write restricted file: {e}"))?;
    if let Err(e) = restrict(path) {
        let _ = fs::remove_file(path);
        return Err(e);
    }
    Ok(())
}

pub fn restrict_file_read_acl(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let sid = current_user_sid_string()?;
        let mut cmd = Command::new("icacls");
        cmd.arg(path.as_os_str())
            .arg("/inheritance:r")
            .arg("/grant:r")
            .arg("*S-1-5-18:(R)")
            .arg("/grant:r")
            .arg(format!("*{}:(R)", sid))
            .creation_flags(CREATE_NO_WINDOW);

        let out = cmd.output().map_err(|e| format!("run icacls: {}", e))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            return Err(format!("icacls failed: {} {}", stdout, stderr).trim().to_string());
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
    }
    Ok(())
}

pub fn restrict_existing_token_acl() -> Result<(), String> {
    let path = token_path()?;
    restrict_file_read_acl(&path)
}

pub fn pipe_security_descriptor() -> String {
    if let Some(sid) = load_owner_sid_string() {
        format!("D:P(A;;GA;;;BA)(A;;GA;;;SY)(A;;GRGW;;;{})", sid)
    } else {
        "D:P(A;;GA;;;BA)(A;;GA;;;SY)(A;;GRGW;;;AU)".to_string()
    }
}

pub fn load_token() -> Result<String, String> {
    let path = token_path()?;
    let content = fs::read_to_string(&path).map_err(|e| format!("read token file: {}", e))?;
    let tok = content.trim().to_string();
    if tok.len() < 16 {
        return Err("token too short".to_string());
    }
    Ok(tok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipe_security_descriptor() {
        let desc = pipe_security_descriptor();
        assert!(desc.starts_with("D:P"));
    }

    #[test]
    fn persist_restricted_removes_file_when_acl_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helper.token");
        let err = persist_restricted_with(&path, b"secret", |_| Err("acl failed".into()))
            .expect_err("ACL failure must surface");
        assert!(err.contains("acl"), "{err}");
        assert!(!path.exists(), "world-readable token must not be left behind");
    }

    #[test]
    fn persist_restricted_keeps_file_when_acl_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helper.token");
        persist_restricted_with(&path, b"secret", |_| Ok(())).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"secret");
    }

    #[test]
    fn empty_allow_file_falls_back_to_defaults() {
        let defaults = default_allowed_roots();
        assert_eq!(coalesce_allowed_roots(Vec::new()), defaults);
        let explicit = vec![PathBuf::from("/only/this")];
        assert_eq!(coalesce_allowed_roots(explicit.clone()), explicit);
    }
}
