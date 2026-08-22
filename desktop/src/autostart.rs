//! OS login auto-start. Settings switch →
//! Windows `HKCU\...\Run`, macOS `~/Library/LaunchAgents` plist.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const AGENT_LABEL: &str = "app.singplane.gpui";
#[allow(dead_code)]
pub const RUN_VALUE: &str = "SingPanel";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchSpec {
    /// `open -a /Applications/SingPanel.app`
    OpenApp(PathBuf),
    /// raw binary / script
    Exec(PathBuf),
}

pub fn launch_spec_from_exe(exe: &Path) -> LaunchSpec {
    let s = exe.to_string_lossy().replace('\\', "/");
    if let Some(idx) = s.find(".app/Contents/MacOS/") {
        return LaunchSpec::OpenApp(PathBuf::from(&s[..=idx + 3]));
    }
    let installed = PathBuf::from("/Applications/SingPanel.app");
    if cfg!(target_os = "macos") && installed.is_dir() {
        return LaunchSpec::OpenApp(installed);
    }
    LaunchSpec::Exec(exe.to_path_buf())
}

pub fn macos_agent_plist(spec: &LaunchSpec) -> String {
    let args = match spec {
        LaunchSpec::OpenApp(app) => vec![
            "/usr/bin/open".into(),
            "-a".into(),
            app.to_string_lossy().into_owned(),
        ],
        LaunchSpec::Exec(bin) => vec![bin.to_string_lossy().into_owned()],
    };
    let args_xml: String = args
        .iter()
        .map(|a| format!("    <string>{}</string>\n", xml_escape(a)))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{AGENT_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
{args_xml}  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#
    )
}

pub fn windows_run_data(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn agent_plist_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(home)
        .join("Library/LaunchAgents")
        .join(format!("{AGENT_LABEL}.plist"))
}

pub fn is_enabled() -> bool {
    platform_is_enabled()
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    platform_set_enabled(on, &exe)
}

#[cfg(target_os = "macos")]
fn platform_is_enabled() -> bool {
    let path = agent_plist_path();
    path.is_file()
        && fs::read_to_string(&path)
            .map(|t| t.contains(AGENT_LABEL))
            .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn platform_set_enabled(on: bool, exe: &Path) -> Result<(), String> {
    let path = agent_plist_path();
    if on {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("LaunchAgents: {e}"))?;
        }
        let spec = launch_spec_from_exe(exe);
        fs::write(&path, macos_agent_plist(&spec)).map_err(|e| format!("写启动项: {e}"))?;
        let uid = users_uid();
        let id = format!("gui/{uid}/{AGENT_LABEL}");
        let _ = Command::new("launchctl").args(["bootout", &id]).status();
        let _ = Command::new("launchctl")
            .args(["bootstrap", &format!("gui/{uid}"), &path.to_string_lossy()])
            .status();
    } else {
        let uid = users_uid();
        let id = format!("gui/{uid}/{AGENT_LABEL}");
        let _ = Command::new("launchctl").args(["bootout", &id]).status();
        let _ = fs::remove_file(&path);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn users_uid() -> String {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "501".into())
}

#[cfg(target_os = "windows")]
fn platform_is_enabled() -> bool {
    Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            RUN_VALUE,
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn platform_set_enabled(on: bool, exe: &Path) -> Result<(), String> {
    if on {
        let data = windows_run_data(exe);
        let out = Command::new("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                RUN_VALUE,
                "/t",
                "REG_SZ",
                "/d",
                &data,
                "/f",
            ])
            .output()
            .map_err(|e| format!("reg: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
    } else {
        let _ = Command::new("reg")
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                RUN_VALUE,
                "/f",
            ])
            .status();
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_is_enabled() -> bool {
    false
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_set_enabled(_: bool, _: &Path) -> Result<(), String> {
    Err("此平台未实现开机自启动".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_unwraps_app_bundle() {
        let exe = Path::new("/Applications/SingPanel.app/Contents/MacOS/SingPanel");
        assert_eq!(
            launch_spec_from_exe(exe),
            LaunchSpec::OpenApp(PathBuf::from("/Applications/SingPanel.app"))
        );
    }

    #[test]
    fn spec_raw_binary_without_bundle_prefix() {
        let exe = Path::new("/tmp/singpanel-gpui");
        match launch_spec_from_exe(exe) {
            LaunchSpec::OpenApp(p) => {
                assert!(p.ends_with("SingPanel.app"));
            }
            LaunchSpec::Exec(p) => {
                assert_eq!(p, exe);
            }
        }
    }

    #[test]
    fn plist_opens_app_and_has_label() {
        let spec = LaunchSpec::OpenApp(PathBuf::from("/Applications/SingPanel.app"));
        let xml = macos_agent_plist(&spec);
        assert!(xml.contains(AGENT_LABEL));
        assert!(xml.contains("<string>/usr/bin/open</string>"));
        assert!(xml.contains("<string>-a</string>"));
        assert!(xml.contains("<string>/Applications/SingPanel.app</string>"));
        assert!(xml.contains("<key>RunAtLoad</key>"));
        assert!(!xml.contains("&lt;") || xml.contains("SingPanel"));
    }

    #[test]
    fn plist_escapes_xml() {
        let spec = LaunchSpec::Exec(PathBuf::from("/tmp/a&b<c>"));
        let xml = macos_agent_plist(&spec);
        assert!(xml.contains("/tmp/a&amp;b&lt;c&gt;"));
    }

    #[test]
    fn windows_run_quotes_path() {
        let data = windows_run_data(Path::new(r"C:\Program Files\SingPanel\singpanel-gpui.exe"));
        assert_eq!(
            data,
            r#""C:\Program Files\SingPanel\singpanel-gpui.exe""#
        );
    }
}
