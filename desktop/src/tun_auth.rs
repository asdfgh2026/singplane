//! TUN privilege: setuid the official sing-box binary.
//!
//! macOS: `osascript` password dialog → `chown root:admin && chmod u+s`
//! Linux: `pkexec` → `chown root:root && chmod u+s`
//! Windows: no-op (use singpanel-helper).
//!
//! External / data volumes are often mounted `noowners,nosuid` (this machine's
//! `/Volumes/SSD` is). `chown` then fails with "Operation not permitted" and
//! setuid would not work even if it succeeded. In that case we copy the core
//! onto the boot volume (`~/Library/Application Support/SingPanel/cores`) and
//! authorize the copy.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn is_privileged(core: &Path) -> bool {
    if !core.is_file() {
        return false;
    }
    platform_is_privileged(core)
}

/// Make the core setuid-root if needed. Returns the path that should be
/// launched (may be a copy on a volume that allows setuid).
pub fn ensure_privileged(core: &Path) -> Result<PathBuf, String> {
    if !core.is_file() {
        return Err(format!("找不到内核文件: {}", core.display()));
    }
    // Windows: TUN is the SYSTEM helper, never setuid the core.
    #[cfg(windows)]
    {
        return Ok(fs::canonicalize(core).unwrap_or_else(|_| core.to_path_buf()));
    }
    #[allow(unreachable_code)]
    {
    let src = fs::canonicalize(core).unwrap_or_else(|_| core.to_path_buf());
    if !src.is_file() {
        return Err(format!("找不到内核文件: {}", src.display()));
    }
    let mount = read_mount_table();
    let home = boot_home();
    let mut target = authorize_dest(&src, &mount, &home);
    if is_privileged(&target) && same_len(&src, &target) {
        return Ok(target);
    }
    if let Err(e) = platform_authorize(&src, &target) {
        let boot = boot_volume_core_under(&home);
        if should_retry_boot_copy(&target, &src, &e) {
            platform_authorize(&src, &boot)?;
            if is_privileged(&boot) {
                return Ok(boot);
            }
            return Err(format!(
                "授权后内核仍无管理员权限（{}）。请重试并输入正确密码",
                boot.display()
            ));
        }
        return Err(e);
    }
    if is_privileged(&target) {
        return Ok(target);
    }
    let boot = boot_volume_core_under(&home);
    if should_retry_boot_copy(&target, &src, "授权后仍无 setuid") {
        platform_authorize(&src, &boot)?;
        if is_privileged(&boot) {
            return Ok(boot);
        }
        target = boot;
    }
    Err(format!(
        "授权后内核仍无管理员权限（{}）。请重试并输入正确密码",
        target.display()
    ))
    }
}

fn same_len(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(x), Ok(y)) => x.len() == y.len(),
        _ => false,
    }
}

fn boot_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/tmp"))
}

fn boot_volume_core_under(home: &Path) -> PathBuf {
    home.join("Library/Application Support/SingPanel/cores/sing-box")
}

fn authorize_dest(src: &Path, mount_text: &str, home: &Path) -> PathBuf {
    if volume_allows_setuid_from(mount_text, src) {
        fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf())
    } else {
        boot_volume_core_under(home)
    }
}

fn volume_allows_setuid_from(mount_text: &str, path: &Path) -> bool {
    let flags = volume_flags_from(mount_text, path);
    if flags.is_empty() {
        return false;
    }
    let flags = flags.to_ascii_lowercase();
    !flags.contains("nosuid") && !flags.contains("noowners")
}

fn read_mount_table() -> String {
    Command::new("/sbin/mount")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_default()
}

fn volume_flags_from(mount_text: &str, path: &Path) -> String {
    let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut best_len = 0usize;
    let mut flags = String::new();
    for line in mount_text.lines() {
        if let Some((mp, rest)) = parse_mount_line(line) {
            if path_on_mount(&canon, &mp) && mp.len() >= best_len {
                best_len = mp.len();
                flags = rest;
            }
        }
    }
    flags
}

fn should_retry_boot_copy(tried: &Path, src: &Path, err: &str) -> bool {
    let retryable = err.contains("not permitted") || err.contains("授权后仍无");
    retryable && (tried == src || paths_eq(tried, src))
}

fn path_on_mount(path: &Path, mount: &str) -> bool {
    let p = normalize_cmd(&path.to_string_lossy());
    let m = normalize_cmd(mount);
    if m == "/" {
        return p.starts_with('/');
    }
    p == m || p.starts_with(&(m + "/"))
}

fn normalize_cmd(s: &str) -> String {
    let s = s.replace('\\', "/");
    if s == "/" {
        return "/".into();
    }
    s.trim_end_matches('/').to_string()
}

/// ` /dev/disk7s1 on /Volumes/SSD (apfs, local, nosuid, noowners)`
fn parse_mount_line(line: &str) -> Option<(String, String)> {
    let rest = line.split_once(" on ")?.1;
    let (mp, flags) = rest.rsplit_once(" (")?;
    Some((
        mp.trim().to_string(),
        flags.trim().trim_end_matches(')').to_string(),
    ))
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthorizeErrorKind {
    Canceled,
    ExternalVolume,
    Other,
}

fn classify_authorize_error(err: &str) -> AuthorizeErrorKind {
    if err.contains("User canceled") || err.contains("-128") {
        AuthorizeErrorKind::Canceled
    } else if err.contains("Operation not permitted") || err.contains("not permitted") {
        AuthorizeErrorKind::ExternalVolume
    } else {
        AuthorizeErrorKind::Other
    }
}

fn stage_core_path() -> PathBuf {
    std::env::temp_dir().join("singpanel-sing-box.stage")
}

fn auth_script_path() -> PathBuf {
    std::env::temp_dir().join("singpanel-tun-auth.sh")
}

/// Root only touches the boot-volume stage + dest. Reading `/Volumes/SSD`
/// from `do shell script … administrator privileges` is TCC-blocked.
fn privileged_authorize_shell(stage: &Path, dest: &Path) -> String {
    let stage_s = shell_escape(&stage.to_string_lossy());
    let dest_s = shell_escape(&dest.to_string_lossy());
    let dest_dir = dest
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/tmp".into());
    let dest_dir_s = shell_escape(&dest_dir);
    format!(
        "set -e\n\
         mkdir -p {dest_dir_s}\n\
         cp -f {stage_s} {dest_s}\n\
         (xattr -d com.apple.quarantine {dest_s} >/dev/null 2>&1 || true)\n\
         chown root:admin {dest_s}\n\
         chmod 4755 {dest_s}\n"
    )
}

fn authorize_applescript(script_path: &Path) -> String {
    format!(
        r#"do shell script "/bin/bash {}" with administrator privileges"#,
        script_path.display()
    )
}

fn parse_privileged_stat(text: &str) -> bool {
    let mut parts = text.split_whitespace();
    let owner = parts.next().unwrap_or("");
    let mode = parts.next().unwrap_or("");
    let root_owned = owner == "root:admin" || owner == "root:wheel" || owner.starts_with("root:");
    root_owned && mode.contains('s')
}

#[cfg(target_os = "macos")]
fn platform_is_privileged(core: &Path) -> bool {
    let out = Command::new("stat")
        .args(["-f", "%Su:%Sg %Sp"])
        .arg(core)
        .output()
        .ok();
    let Some(out) = out else {
        return false;
    };
    parse_privileged_stat(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(target_os = "macos")]
fn platform_authorize(src: &Path, dest: &Path) -> Result<(), String> {
    let _ = Command::new("/usr/bin/xattr")
        .args(["-d", "com.apple.quarantine"])
        .arg(src)
        .status();

    let stage = stage_core_path();
    if let Some(dir) = stage.parent() {
        let _ = fs::create_dir_all(dir);
    }
    fs::copy(src, &stage).map_err(|e| {
        format!(
            "无法把内核从 {} 拷到启动盘缓存: {e}",
            src.display()
        )
    })?;

    let sh_path = auth_script_path();
    fs::write(&sh_path, privileged_authorize_shell(&stage, dest))
        .map_err(|e| format!("无法写授权脚本: {e}"))?;

    let script = authorize_applescript(&sh_path);
    let out = Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| format!("无法弹出授权窗口: {e}"))?;
    let _ = fs::remove_file(&stage);
    let _ = fs::remove_file(&sh_path);
    if out.status.success() {
        return Ok(());
    }
    let err = format!(
        "{} {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    match classify_authorize_error(&err) {
        AuthorizeErrorKind::Canceled => {
            Err("已取消授权。开启虚拟网卡需要管理员密码。".into())
        }
        AuthorizeErrorKind::ExternalVolume => Err(format!(
            "授权命令被系统拒绝（TCC/磁盘权限）。已改为先拷到启动盘再授权仍失败: {}",
            err.trim()
        )),
        AuthorizeErrorKind::Other => Err(format!(
            "授权失败: {}",
            err.trim().if_empty("请输入正确的管理员密码")
        )),
    }
}

fn paths_eq(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

#[cfg(target_os = "linux")]
fn platform_is_privileged(core: &Path) -> bool {
    let out = Command::new("stat")
        .args(["-c", "%U:%G %A"])
        .arg(core)
        .output()
        .ok();
    let Some(out) = out else {
        return false;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    let owner = parts.next().unwrap_or("");
    let mode = parts.next().unwrap_or("");
    owner.starts_with("root:") && mode.contains('s')
}

#[cfg(target_os = "linux")]
fn platform_authorize(_src: &Path, core: &Path) -> Result<(), String> {
    let path = shell_escape(&core.to_string_lossy());
    let sh = format!("chown root:root {path} && chmod u+s {path} && sync");
    let out = Command::new("pkexec")
        .args(["sh", "-c", &sh])
        .output()
        .map_err(|e| format!("pkexec: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    if out.status.code() == Some(127) {
        return Err("未找到 pkexec，无法授权虚拟网卡".into());
    }
    Err("授权失败。开启虚拟网卡需要管理员密码。".into())
}

#[cfg(windows)]
fn platform_is_privileged(_: &Path) -> bool {
    true
}

#[cfg(windows)]
fn platform_authorize(_: &Path, _: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_is_privileged(_: &Path) -> bool {
    false
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_authorize(_: &Path, _: &Path) -> Result<(), String> {
    Err("此平台未实现虚拟网卡授权".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNT_TABLE: &str = "\
/dev/disk3s1s1 on / (apfs, sealed, local, read-only, journaled)
devfs on /dev (devfs, local, nobrowse)
/dev/disk3s5 on /System/Volumes/Data (apfs, local, journaled, nobrowse, protect, root data)
/dev/disk7s1 on /Volumes/SSD (apfs, local, nodev, nosuid, journaled, noowners)
";

    #[test]
    fn parse_ssd_nosuid() {
        let line = "/dev/disk7s1 on /Volumes/SSD (apfs, local, nodev, nosuid, journaled, noowners)";
        let (mp, flags) = parse_mount_line(line).unwrap();
        assert_eq!(mp, "/Volumes/SSD");
        assert!(flags.contains("nosuid"));
        assert!(flags.contains("noowners"));
        assert!(path_on_mount(
            Path::new("/Volumes/SSD/dev/singplane/SingPanel/cores/sing-box"),
            "/Volumes/SSD"
        ));
        assert!(!path_on_mount(
            Path::new("/Users/box/Library/Application Support/x"),
            "/Volumes/SSD"
        ));
    }

    #[test]
    fn root_mount_point_stays_slash() {
        assert_eq!(normalize_cmd("/"), "/");
        assert!(path_on_mount(Path::new("/usr/bin/sing-box"), "/"));
    }

    #[test]
    fn empty_mount_table_does_not_allow_setuid() {
        let path = Path::new("/Volumes/SSD/dev/singplane/SingPanel/cores/sing-box");
        assert!(!volume_allows_setuid_from("", path));
    }

    #[test]
    fn ssd_loses_to_longest_mount_not_root() {
        let path = Path::new("/Volumes/SSD/dev/singplane/SingPanel/SingPanel/cores/sing-box");
        let flags = volume_flags_from(MOUNT_TABLE, path);
        assert!(
            flags.contains("nosuid"),
            "expected SSD flags, got {flags:?}"
        );
        assert!(!volume_allows_setuid_from(MOUNT_TABLE, path));
    }

    #[test]
    fn users_library_on_data_volume_allows_setuid() {
        let path = Path::new("/Users/box/Library/Application Support/SingPanel/cores/sing-box");
        assert!(volume_allows_setuid_from(MOUNT_TABLE, path));
    }

    #[test]
    fn nosuid_core_relocates_to_boot_volume() {
        let src = Path::new("/Volumes/SSD/dev/singplane/SingPanel/SingPanel/cores/sing-box");
        let dest = authorize_dest(src, MOUNT_TABLE, Path::new("/Users/box"));
        assert_eq!(
            dest,
            PathBuf::from("/Users/box/Library/Application Support/SingPanel/cores/sing-box")
        );
    }

    #[test]
    fn boot_volume_core_stays_put() {
        let src = Path::new("/Users/box/Library/Application Support/SingPanel/cores/sing-box");
        let dest = authorize_dest(src, MOUNT_TABLE, Path::new("/Users/box"));
        assert_eq!(dest, src);
    }

    #[test]
    fn privileged_stat_accepts_root_admin_or_wheel() {
        assert!(parse_privileged_stat("root:admin -rwsr-xr-x\n"));
        assert!(parse_privileged_stat("root:wheel -rwsr-xr-x\n"));
        assert!(!parse_privileged_stat("box:staff -rwxr-xr-x\n"));
        assert!(!parse_privileged_stat("root:admin -rwxr-xr-x\n"));
        assert!(!parse_privileged_stat(""));
    }

    #[test]
    fn privileged_shell_never_touches_ssd() {
        let stage = Path::new("/tmp/singpanel-sing-box.stage");
        let dest =
            Path::new("/Users/box/Library/Application Support/SingPanel/cores/sing-box");
        let sh = privileged_authorize_shell(stage, dest);
        assert!(sh.contains("/tmp/singpanel-sing-box.stage"));
        assert!(sh.contains("/Users/box/Library/Application Support/SingPanel/cores/sing-box"));
        assert!(
            !sh.contains("/Volumes/SSD"),
            "root must not read the external volume: {sh}"
        );
    }

    #[test]
    fn applescript_only_runs_staged_script() {
        let script = authorize_applescript(Path::new("/tmp/singpanel-tun-auth.sh"));
        assert_eq!(
            script,
            r#"do shell script "/bin/bash /tmp/singpanel-tun-auth.sh" with administrator privileges"#
        );
        assert!(script.len() < 200);
    }

    #[test]
    fn osascript_range_prefix_is_not_nosuid() {
        let err = "0:543: execution error: cp: /Volumes/SSD/x: Operation not permitted (1)";
        assert_eq!(
            classify_authorize_error(err),
            AuthorizeErrorKind::ExternalVolume
        );
        assert_eq!(
            classify_authorize_error("User canceled. (-128)"),
            AuthorizeErrorKind::Canceled
        );
    }

    #[test]
    fn windows_keep_core_path_without_setuid() {
        if !cfg!(windows) {
            return;
        }
        let p = std::env::temp_dir().join("singpanel-tun-auth-win.bin");
        fs::write(&p, b"x").unwrap();
        let out = ensure_privileged(&p).expect("windows authorize");
        assert_eq!(
            fs::canonicalize(&out).unwrap(),
            fs::canonicalize(&p).unwrap()
        );
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn in_place_not_permitted_retries_boot_copy() {
        let src = Path::new("/Volumes/SSD/dev/singplane/x/sing-box");
        let dest = src;
        assert!(should_retry_boot_copy(
            dest,
            src,
            "chown: Operation not permitted"
        ));
        let boot = Path::new("/Users/box/Library/Application Support/SingPanel/cores/sing-box");
        assert!(!should_retry_boot_copy(boot, src, "chown: Operation not permitted"));
    }
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for &str {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self.to_string()
        }
    }
}
