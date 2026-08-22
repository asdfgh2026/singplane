use std::path::{Path, PathBuf};
use crate::protocol::StartBody;

pub const CORE_EXACT_NAME: &str = "sing-box.exe";

pub fn is_remote_path(path: &str) -> bool {
    let clean = path.trim();
    let mut normalized = clean;
    if normalized.starts_with(r"\\?\") {
        let rest = &normalized[4..];
        if rest.len() >= 4 && rest[..4].eq_ignore_ascii_case("UNC\\") {
            return true;
        }
        normalized = rest;
    }
    if normalized.starts_with(r"\\") || normalized.starts_with("//") {
        return true;
    }

    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::path::Component;
        use windows::Win32::Storage::FileSystem::GetDriveTypeW;

        const DRIVE_NO_ROOT_DIR: u32 = 1;
        const DRIVE_REMOTE: u32 = 4;

        let p = Path::new(normalized);
        let prefix = p.components().next();
        if let Some(Component::Prefix(prefix_comp)) = prefix {
            use std::path::Prefix;
            match prefix_comp.kind() {
                Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _) => return true,
                Prefix::Disk(disk) | Prefix::VerbatimDisk(disk) => {
                    let root = format!("{}:\\\0", (disk as char).to_ascii_uppercase());
                    let wide: Vec<u16> = OsStr::new(&root).encode_wide().collect();
                    let dt = unsafe { GetDriveTypeW(windows::core::PCWSTR(wide.as_ptr())) };
                    return dt == DRIVE_REMOTE || dt == DRIVE_NO_ROOT_DIR;
                }
                _ => return true,
            }
        }
        return false;
    }

    #[cfg(not(windows))]
    {
        // Fallback for non-windows / unit test environment
        if normalized.starts_with(r"\\") || normalized.starts_with("//") {
            return true;
        }
        if let Some(first) = normalized.chars().next() {
            if first.is_ascii_alphabetic() && normalized.get(1..2) == Some(":") {
                // e.g. C:\...
                return false;
            }
        }
        false
    }
}

pub fn normalize_local_file(raw_path: &str) -> Result<PathBuf, String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err("path empty".to_string());
    }
    let p = Path::new(trimmed);
    let is_abs = p.is_absolute()
        || (trimmed.len() >= 3
            && trimmed.chars().next().map_or(false, |c| c.is_ascii_alphabetic())
            && &trimmed[1..3] == r":\");
    if !is_abs {
        return Err("path must be absolute".to_string());
    }
    if is_remote_path(trimmed) {
        return Err("remote / UNC path not allowed".to_string());
    }

    let resolved = if p.exists() {
        std::fs::canonicalize(p).map_err(|e| format!("resolve path: {}", e))?
    } else {
        return Err(format!("not found: {}", trimmed));
    };

    let resolved_str = resolved.to_string_lossy();
    if is_remote_path(&resolved_str) {
        return Err("resolved path is remote".to_string());
    }
    if resolved.is_dir() {
        return Err("path is a directory".to_string());
    }
    Ok(resolved)
}

pub fn extract_file_name(path: &Path) -> &str {
    let s = path.to_str().unwrap_or("");
    let last_sep = s.rfind(|c| c == '/' || c == '\\');
    match last_sep {
        Some(idx) => &s[idx + 1..],
        None => s,
    }
}

pub fn validate_core_name(path: &Path) -> Result<(), String> {
    let base = extract_file_name(path);
    if !base.eq_ignore_ascii_case(CORE_EXACT_NAME) {
        return Err(format!("refusing to run non sing-box binary: {}", base));
    }
    Ok(())
}

pub fn under_root(path: &Path, root: &Path) -> bool {
    let path_clean = strip_unc_prefix(path);
    let root_clean = strip_unc_prefix(root);
    path_clean.starts_with(&root_clean)
}

fn strip_unc_prefix(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if s.starts_with(r"\\?\") {
        PathBuf::from(&s[4..])
    } else {
        p.to_path_buf()
    }
}

pub fn require_allowed_location(path: &Path, roots: &[PathBuf]) -> Result<(), String> {
    if roots.is_empty() {
        // No allow-list written yet (upgrade / first start): do not refuse start.
        return Ok(());
    }
    let norm_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    for root in roots {
        if root.as_os_str().is_empty() {
            continue;
        }
        let norm_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        if under_root(&norm_path, &norm_root) {
            return Ok(());
        }
    }
    Err("path outside allowed directories".to_string())
}

pub fn sanitize_start(
    body: &StartBody,
    roots: &[PathBuf],
) -> Result<(PathBuf, Vec<String>, PathBuf), String> {
    let path = normalize_local_file(&body.path).map_err(|e| format!("core: {}", e))?;
    validate_core_name(&path)?;
    require_allowed_location(&path, roots).map_err(|e| format!("core: {}", e))?;

    let mut config_cand = body.config.as_deref().unwrap_or("").trim().to_string();
    if config_cand.is_empty()
        && body.args.len() == 3
        && body.args[0] == "run"
        && (body.args[1] == "-c" || body.args[1] == "--config")
    {
        config_cand = body.args[2].clone();
    }
    if config_cand.is_empty() {
        return Err("config path required".to_string());
    }

    let config = normalize_local_file(&config_cand).map_err(|e| format!("config: {}", e))?;
    require_allowed_location(&config, roots).map_err(|e| format!("config: {}", e))?;

    let work_dir = config
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let config_arg = strip_unc_prefix(&config).to_string_lossy().to_string();
    Ok((path, vec!["run".into(), "-c".into(), config_arg], work_dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_is_remote_path() {
        assert!(is_remote_path(r"\\attacker\share\sing-box.exe"));
        assert!(is_remote_path(r"\\?\UNC\attacker\share\sing-box.exe"));
        assert!(is_remote_path(r"//attacker/share/sing-box.exe"));
        assert!(!is_remote_path(r"C:\cores\sing-box.exe"));
    }

    #[test]
    fn test_validate_core_name_exact() {
        assert!(validate_core_name(Path::new(r"C:\a\sing-box.exe")).is_ok());
        assert!(validate_core_name(Path::new(r"C:\a\SING-BOX.EXE")).is_ok());
        assert!(validate_core_name(Path::new(r"/usr/local/bin/sing-box.exe")).is_ok());
        assert!(validate_core_name(Path::new(r"C:\a\evil-sing-box.exe")).is_err());
    }

    #[test]
    fn test_sanitize_start_forces_args() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let core = root.join("sing-box.exe");
        let cfg = root.join("config.runtime.json");
        fs::write(&core, b"x").unwrap();
        fs::write(&cfg, b"{}").unwrap();

        let roots = vec![root.to_path_buf()];
        let body = StartBody {
            path: core.to_string_lossy().to_string(),
            config: Some(cfg.to_string_lossy().to_string()),
            args: vec![
                "run".into(),
                "-c".into(),
                r"C:\Temp\pwn.json".into(),
                "--evil".into(),
            ],
            work_dir: Some(r"C:\Windows".into()),
        };

        let (path, args, work_dir) = sanitize_start(&body, &roots).unwrap();
        let base = extract_file_name(&path);
        assert!(base.eq_ignore_ascii_case("sing-box.exe"));
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "run");
        assert_eq!(args[1], "-c");
        assert!(args[2].ends_with("config.runtime.json"));
        let norm_cfg = fs::canonicalize(&cfg).unwrap();
        let expected_workdir = norm_cfg.parent().unwrap();
        assert_eq!(work_dir, expected_workdir);
    }

    #[test]
    fn test_sanitize_start_rejects_wrong_name_and_unc() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let evil = root.join("evil-sing-box.exe");
        fs::write(&evil, b"x").unwrap();

        let roots = vec![root.to_path_buf()];
        let body = StartBody {
            path: evil.to_string_lossy().to_string(),
            config: Some(evil.to_string_lossy().to_string()),
            args: vec![],
            work_dir: None,
        };
        assert!(sanitize_start(&body, &roots).is_err());

        let body_unc = StartBody {
            path: r"\\attacker\share\sing-box.exe".into(),
            config: Some(r"C:\Windows\win.ini".into()),
            args: vec![],
            work_dir: None,
        };
        assert!(sanitize_start(&body_unc, &roots).is_err());
    }

    #[test]
    fn test_sanitize_start_rejects_outside_roots() {
        let temp_dir1 = tempfile::tempdir().unwrap();
        let temp_dir2 = tempfile::tempdir().unwrap();
        let root1 = temp_dir1.path();
        let root2 = temp_dir2.path();

        let core = root1.join("sing-box.exe");
        let cfg = root2.join("config.runtime.json");
        fs::write(&core, b"x").unwrap();
        fs::write(&cfg, b"{}").unwrap();

        let roots = vec![root1.to_path_buf()];
        let body = StartBody {
            path: core.to_string_lossy().to_string(),
            config: Some(cfg.to_string_lossy().to_string()),
            args: vec![],
            work_dir: None,
        };
        let res = sanitize_start(&body, &roots);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("allowed"));
    }

    #[test]
    fn empty_roots_do_not_reject_location() {
        let temp_dir = tempfile::tempdir().unwrap();
        let core = temp_dir.path().join("sing-box.exe");
        fs::write(&core, b"x").unwrap();
        assert!(require_allowed_location(&core, &[]).is_ok());
    }

    #[test]
    fn empty_allow_list_still_starts_under_temp() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let core = root.join("sing-box.exe");
        let cfg = root.join("config.runtime.json");
        fs::write(&core, b"x").unwrap();
        fs::write(&cfg, b"{}").unwrap();

        let body = StartBody {
            path: core.to_string_lossy().to_string(),
            config: Some(cfg.to_string_lossy().to_string()),
            args: vec![],
            work_dir: None,
        };
        assert!(sanitize_start(&body, &[]).is_ok());
    }
}
