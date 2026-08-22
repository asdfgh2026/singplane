//! Official SagerNet/sing-box GitHub Releases installer.
//! Official GitHub release asset names for sing-box.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::Value;

use crate::host::{app_root, default_core_path};

const UA: &str = "SingPanel";
const LATEST: &str = "https://api.github.com/repos/SagerNet/sing-box/releases/latest";
const RELEASES: &str = "https://api.github.com/repos/SagerNet/sing-box/releases?per_page=30";

/// Built-in GitHub reverse-proxy prefixes for core download.
/// Empty prefix = direct. User can edit or type a custom prefix.
pub struct GithubProxyPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub prefix: &'static str,
}

pub const GITHUB_PROXY_PRESETS: &[GithubProxyPreset] = &[
    GithubProxyPreset {
        id: "direct",
        label: "直连",
        prefix: "",
    },
    GithubProxyPreset {
        id: "ghfast",
        label: "ghfast",
        prefix: "https://ghfast.top",
    },
    GithubProxyPreset {
        id: "gh-proxy",
        label: "gh-proxy",
        prefix: "https://gh-proxy.com",
    },
    GithubProxyPreset {
        id: "ghproxy-net",
        label: "ghproxy.net",
        prefix: "https://ghproxy.net",
    },
    GithubProxyPreset {
        id: "llkk",
        label: "gh.llkk.cc",
        prefix: "https://gh.llkk.cc",
    },
];

pub fn normalize_github_proxy(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

pub fn matching_github_proxy_preset(raw: &str) -> Option<&'static GithubProxyPreset> {
    let norm = normalize_github_proxy(raw);
    GITHUB_PROXY_PRESETS
        .iter()
        .find(|p| normalize_github_proxy(p.prefix) == norm)
}

/// Wrap a GitHub URL with a ghproxy-style prefix: `{proxy}/{original_url}`.
pub fn apply_github_proxy(url: &str, proxy: &str) -> String {
    let proxy = normalize_github_proxy(proxy);
    if proxy.is_empty() || url.starts_with(&proxy) || !is_github_url(url) {
        return url.to_string();
    }
    format!("{proxy}/{url}")
}

fn is_github_url(url: &str) -> bool {
    const HOSTS: &[&str] = &[
        "https://github.com/",
        "https://api.github.com/",
        "https://objects.githubusercontent.com/",
        "https://release-assets.githubusercontent.com/",
        "https://codeload.github.com/",
        "https://raw.githubusercontent.com/",
        "https://gist.githubusercontent.com/",
        "https://gist.github.com/",
    ];
    HOSTS.iter().any(|h| url.starts_with(h))
}

#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    pub version: String,
    pub asset_name: String,
    pub download_url: String,
}

#[derive(Clone, Debug, Default)]
pub struct ChannelInfo {
    pub local: Option<String>,
    pub stable: Option<String>,
    pub beta: Option<String>,
    pub selected: Option<ReleaseInfo>,
}

pub fn cores_dir() -> PathBuf {
    app_root().join("cores")
}

pub fn binary_name() -> &'static str {
    if cfg!(windows) {
        "sing-box.exe"
    } else {
        "sing-box"
    }
}

pub fn platform_label() -> String {
    format!("{}-{}", os_name(), arch_name())
}

pub fn asset_file_name(version: &str) -> String {
    let v = version.strip_prefix('v').unwrap_or(version);
    let suffix = if cfg!(windows) { ".zip" } else { ".tar.gz" };
    format!("sing-box-{v}-{}-{}{suffix}", os_name(), arch_name())
}

pub fn local_version(core_path: &str) -> Option<String> {
    let path = if core_path.trim().is_empty() {
        default_core_path()
    } else {
        PathBuf::from(core_path.trim())
    };
    if !path.is_file() {
        return None;
    }
    let out = Command::new(&path).arg("version").output().ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    parse_version_output(&text)
}

pub fn inspect_channels(
    core_path: &str,
    channel: &str,
    github_proxy: &str,
) -> Result<ChannelInfo, String> {
    let local = local_version(core_path);
    let http = http_client()?;
    let stable = fetch_latest(&http, false, github_proxy).ok();
    let beta = fetch_latest(&http, true, github_proxy).ok();
    let selected = if channel == "beta" {
        beta.clone()
    } else {
        stable.clone()
    };
    Ok(ChannelInfo {
        local,
        stable: stable.as_ref().map(|r| r.version.clone()),
        beta: beta.as_ref().map(|r| r.version.clone()),
        selected,
    })
}

pub fn download_and_install(
    alpha: bool,
    github_proxy: &str,
) -> Result<(PathBuf, Option<String>), String> {
    let http = http_client()?;
    let info = fetch_latest(&http, alpha, github_proxy)?;
    let dir = cores_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir cores: {e}"))?;
    let cache = dir.join(".cache");
    fs::create_dir_all(&cache).map_err(|e| format!("mkdir cache: {e}"))?;

    let archive = cache.join(&info.asset_name);
    let url = apply_github_proxy(&info.download_url, github_proxy);
    download_file(&http, &url, &archive)?;

    let target = dir.join(binary_name());
    let bak = dir.join(format!("{}.bak", binary_name()));
    if target.is_file() {
        let _ = fs::remove_file(&bak);
        if fs::rename(&target, &bak).is_err() {
            let _ = fs::copy(&target, &bak);
            let _ = fs::remove_file(&target);
        }
    }

    let extract = if info.asset_name.ends_with(".zip") {
        extract_zip(&archive, &target)
    } else {
        extract_tar_gz(&archive, &target)
    };
    if let Err(e) = extract {
        if bak.is_file() && !target.is_file() {
            let _ = fs::rename(&bak, &target);
        }
        return Err(e);
    }

    let _ = fs::remove_file(&archive);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&target) {
            let mut perm = meta.permissions();
            perm.set_mode(0o755);
            let _ = fs::set_permissions(&target, perm);
        }
    }
    if !target.is_file() {
        return Err(format!("解压后找不到内核: {}", target.display()));
    }
    let ver = local_version(&target.to_string_lossy());
    Ok((target, ver))
}

fn fetch_latest(
    http: &reqwest::blocking::Client,
    alpha: bool,
    github_proxy: &str,
) -> Result<ReleaseInfo, String> {
    if alpha {
        let list: Value = get_json(http, &apply_github_proxy(RELEASES, github_proxy))?;
        let arr = list
            .as_array()
            .ok_or_else(|| "releases 响应不是数组".to_string())?;
        for item in arr {
            let tag = item
                .get("tag_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let pre = item
                .get("prerelease")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let looks_beta =
                tag.contains("beta") || tag.contains("rc") || tag.contains("alpha");
            if pre || looks_beta {
                return parse_release(item);
            }
        }
        return Err("未找到测试版（Beta / RC）发布".into());
    }
    let body = get_json(http, &apply_github_proxy(LATEST, github_proxy))?;
    parse_release(&body)
}

fn parse_release(body: &Value) -> Result<ReleaseInfo, String> {
    let tag = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();
    if tag.is_empty() {
        return Err("无法解析发布 tag".into());
    }
    let want = asset_file_name(&tag);
    let assets = body
        .get("assets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let asset = assets.iter().find(|a| {
        a.get("name")
            .and_then(|v| v.as_str())
            .is_some_and(|n| n == want)
    });
    let Some(asset) = asset else {
        let names: Vec<&str> = assets
            .iter()
            .filter_map(|a| a.get("name").and_then(|v| v.as_str()))
            .collect();
        return Err(format!(
            "没有匹配资源 {want}（平台 {}）。可用: {}",
            platform_label(),
            names.join(", ")
        ));
    };
    let url = asset
        .get("browser_download_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "下载地址为空".to_string())?;
    Ok(ReleaseInfo {
        version: tag,
        asset_name: want,
        download_url: url.to_string(),
    })
}

fn download_file(
    http: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    let mut resp = http
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("下载失败: {e}"))?;
    let mut tmp = dest.with_extension("part");
    if tmp.extension().is_none() {
        tmp = dest.with_file_name(format!(
            "{}.part",
            dest.file_name().and_then(|s| s.to_str()).unwrap_or("dl")
        ));
    }
    let mut file = fs::File::create(&tmp).map_err(|e| format!("写缓存: {e}"))?;
    std::io::copy(&mut resp, &mut file).map_err(|e| format!("写文件: {e}"))?;
    file.sync_all().ok();
    drop(file);
    if fs::rename(&tmp, dest).is_err() {
        fs::copy(&tmp, dest).map_err(|e| format!("保存安装包: {e}"))?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

fn extract_tar_gz(archive: &Path, target: &Path) -> Result<(), String> {
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(archive)
        .arg("-C")
        .arg(archive.parent().unwrap_or(Path::new(".")))
        .status()
        .map_err(|e| format!("解压 tar: {e}"))?;
    if !status.success() {
        return Err("tar 解压失败".into());
    }
    let root = archive.parent().unwrap_or(Path::new("."));
    let found = find_binary(root).ok_or_else(|| "压缩包里没有 sing-box".to_string())?;
    if found != target {
        fs::copy(&found, target).map_err(|e| format!("复制内核: {e}"))?;
    }
    Ok(())
}

fn extract_zip(archive: &Path, target: &Path) -> Result<(), String> {
    let dest_dir = archive.parent().unwrap_or(Path::new("."));
    let status = Command::new("unzip")
        .args(["-o", "-q"])
        .arg(archive)
        .arg("-d")
        .arg(dest_dir)
        .status();
    match status {
        Ok(s) if s.success() => {}
        _ => {
            let python = Command::new("python3")
                .args([
                    "-c",
                    &format!(
                        "import zipfile; zipfile.ZipFile(r'{}').extractall(r'{}')",
                        archive.display(),
                        dest_dir.display()
                    ),
                ])
                .status()
                .map_err(|e| format!("解压 zip: {e}"))?;
            if !python.success() {
                return Err("zip 解压失败".into());
            }
        }
    }
    let found = find_binary(dest_dir).ok_or_else(|| "压缩包里没有 sing-box".to_string())?;
    if found != target {
        fs::copy(&found, target).map_err(|e| format!("复制内核: {e}"))?;
    }
    Ok(())
}

fn find_binary(root: &Path) -> Option<PathBuf> {
    let want = binary_name();
    fn walk(dir: &Path, want: &str, depth: u8) -> Option<PathBuf> {
        if depth > 4 {
            return None;
        }
        let rd = fs::read_dir(dir).ok()?;
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = walk(&path, want, depth + 1) {
                    return Some(found);
                }
            } else {
                let name = path.file_name()?.to_string_lossy();
                if name == want || name == "sing-box" || name == "sing-box.exe" {
                    return Some(path);
                }
            }
        }
        None
    }
    walk(root, want, 0)
}

fn parse_version_output(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line
            .strip_prefix("sing-box version ")
            .or_else(|| line.strip_prefix("version "))
        {
            let ver = rest.split_whitespace().next().unwrap_or(rest).trim();
            if !ver.is_empty() {
                return Some(ver.to_string());
            }
        }
    }
    None
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

fn get_json(http: &reqwest::blocking::Client, url: &str) -> Result<Value, String> {
    http.get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|e| format!("GitHub API: {e}"))
}

fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}

fn arch_name() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "386"
    } else {
        "amd64"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_slash() {
        assert_eq!(normalize_github_proxy(" https://ghfast.top/ "), "https://ghfast.top");
        assert_eq!(normalize_github_proxy(""), "");
    }

    #[test]
    fn apply_empty_is_direct() {
        let url = "https://api.github.com/repos/SagerNet/sing-box/releases/latest";
        assert_eq!(apply_github_proxy(url, ""), url);
        assert_eq!(apply_github_proxy(url, "   "), url);
    }

    #[test]
    fn apply_wraps_github_hosts() {
        assert_eq!(
            apply_github_proxy(
                "https://github.com/SagerNet/sing-box/releases/download/v1.12.0/x.tar.gz",
                "https://ghfast.top/"
            ),
            "https://ghfast.top/https://github.com/SagerNet/sing-box/releases/download/v1.12.0/x.tar.gz"
        );
        assert_eq!(
            apply_github_proxy(
                "https://api.github.com/repos/SagerNet/sing-box/releases?per_page=30",
                "https://gh-proxy.com"
            ),
            "https://gh-proxy.com/https://api.github.com/repos/SagerNet/sing-box/releases?per_page=30"
        );
    }

    #[test]
    fn apply_does_not_double_wrap() {
        let already = "https://ghfast.top/https://api.github.com/repos/x";
        assert_eq!(apply_github_proxy(already, "https://ghfast.top"), already);
    }

    #[test]
    fn apply_skips_non_github() {
        assert_eq!(
            apply_github_proxy("https://example.com/a", "https://ghfast.top"),
            "https://example.com/a"
        );
    }

    #[test]
    fn preset_match() {
        assert_eq!(
            matching_github_proxy_preset("https://ghproxy.net/").map(|p| p.id),
            Some("ghproxy-net")
        );
        assert_eq!(matching_github_proxy_preset("").map(|p| p.id), Some("direct"));
        assert_eq!(
            matching_github_proxy_preset("https://my.example/gh").map(|p| p.id),
            None
        );
    }
}
