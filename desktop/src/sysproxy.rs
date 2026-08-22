//! OS system HTTP/HTTPS/SOCKS proxy → mixed inbound.
//! System proxy: not TUN, just the desktop proxy settings.

use std::process::Command;

/// Hosts / CIDRs that must not go through the mixed inbound.
/// Terminal and LAN (router, NAS, Docker) break if 10/8 is proxied.
pub fn proxy_bypass_domains() -> &'static [&'static str] {
    &[
        "127.0.0.1",
        "localhost",
        "*.local",
        "<local>",
        "169.254.0.0/16",
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
    ]
}

/// WinINet ProxyOverride matches host/IP wildcard patterns, not CIDR blocks.
pub fn windows_proxy_bypass_domains() -> &'static [&'static str] {
    &[
        "127.0.0.1",
        "localhost",
        "*.local",
        "<local>",
        "169.254.*",
        "10.*",
        "172.16.*",
        "172.17.*",
        "172.18.*",
        "172.19.*",
        "172.20.*",
        "172.21.*",
        "172.22.*",
        "172.23.*",
        "172.24.*",
        "172.25.*",
        "172.26.*",
        "172.27.*",
        "172.28.*",
        "172.29.*",
        "172.30.*",
        "172.31.*",
        "192.168.*",
    ]
}

pub fn apply(port: u16) -> Result<(), String> {
    if port == 0 {
        return Err("混合端口无效".into());
    }
    platform_apply(port)
}

pub fn clear() -> Result<(), String> {
    platform_clear()
}

/// Point macOS stub resolver at the TUN address so LAN fake-ip DNS
/// (e.g. a router on 10.0.0.1) cannot answer 198.18/15.
pub fn apply_tun_dns(dns: &str) -> Result<(), String> {
    platform_apply_dns(&[dns])
}

pub fn clear_tun_dns() -> Result<(), String> {
    platform_clear_dns()
}

pub fn flush_dns_cache() {
    let _ = Command::new("dscacheutil").arg("-flushcache").status();
    let _ = Command::new("killall")
        .args(["-HUP", "mDNSResponder"])
        .status();
}

#[cfg(target_os = "macos")]
fn platform_apply(port: u16) -> Result<(), String> {
    let port_s = port.to_string();
    for svc in network_services()? {
        run(&[
            "networksetup",
            "-setwebproxy",
            &svc,
            "127.0.0.1",
            &port_s,
        ])?;
        run(&[
            "networksetup",
            "-setsecurewebproxy",
            &svc,
            "127.0.0.1",
            &port_s,
        ])?;
        run(&[
            "networksetup",
            "-setsocksfirewallproxy",
            &svc,
            "127.0.0.1",
            &port_s,
        ])?;
        let mut bypass = vec![
            "networksetup",
            "-setproxybypassdomains",
            svc.as_str(),
        ];
        bypass.extend(proxy_bypass_domains().iter().copied());
        run(&bypass)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_clear() -> Result<(), String> {
    for svc in network_services()? {
        run(&["networksetup", "-setwebproxystate", &svc, "off"])?;
        run(&["networksetup", "-setsecurewebproxystate", &svc, "off"])?;
        run(&["networksetup", "-setsocksfirewallproxystate", &svc, "off"])?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_apply_dns(servers: &[&str]) -> Result<(), String> {
    for svc in network_services()? {
        let mut args = vec!["networksetup", "-setdnsservers", svc.as_str()];
        args.extend(servers.iter().copied());
        run(&args)?;
    }
    flush_dns_cache();
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_clear_dns() -> Result<(), String> {
    for svc in network_services()? {
        run(&["networksetup", "-setdnsservers", &svc, "Empty"])?;
    }
    flush_dns_cache();
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn platform_apply_dns(_: &[&str]) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn platform_clear_dns() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn network_services() -> Result<Vec<String>, String> {
    let out = Command::new("networksetup")
        .arg("-listallnetworkservices")
        .output()
        .map_err(|e| format!("networksetup: {e}"))?;
    if !out.status.success() {
        return Err("无法列出网络服务".into());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut services = Vec::new();
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('*') {
            continue;
        }
        services.push(line.to_string());
    }
    if services.is_empty() {
        return Err("没有可用的网络服务".into());
    }
    Ok(services)
}

#[cfg(target_os = "windows")]
fn platform_apply(port: u16) -> Result<(), String> {
    let bypass = windows_proxy_bypass_domains().join(";");
    let script = format!(
        r#"$path='HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'; Set-ItemProperty $path ProxyEnable 1; Set-ItemProperty $path ProxyServer '127.0.0.1:{port}'; Set-ItemProperty $path ProxyOverride '{bypass}'; Add-Type -TypeDefinition 'using System.Runtime.InteropServices; public class W {{ [DllImport("wininet.dll")] public static extern bool InternetSetOption(int h, int o, System.IntPtr b, int l); }}'; [W]::InternetSetOption(0,39,[IntPtr]::Zero,0)|Out-Null; [W]::InternetSetOption(0,37,[IntPtr]::Zero,0)|Out-Null"#
    );
    run_ps(&script)
}

#[cfg(target_os = "windows")]
fn platform_clear() -> Result<(), String> {
    let script = r#"$path='HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'; Set-ItemProperty $path ProxyEnable 0; Add-Type -TypeDefinition 'using System.Runtime.InteropServices; public class W { [DllImport("wininet.dll")] public static extern bool InternetSetOption(int h, int o, System.IntPtr b, int l); }'; [W]::InternetSetOption(0,39,[IntPtr]::Zero,0)|Out-Null; [W]::InternetSetOption(0,37,[IntPtr]::Zero,0)|Out-Null"#;
    run_ps(script)
}

#[cfg(target_os = "windows")]
fn run_ps(script: &str) -> Result<(), String> {
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| format!("powershell: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(target_os = "linux")]
fn platform_apply(port: u16) -> Result<(), String> {
    let host = "127.0.0.1";
    let p = port.to_string();
    let _ = Command::new("gsettings")
        .args(["set", "org.gnome.system.proxy", "mode", "manual"])
        .status();
    let _ = Command::new("gsettings")
        .args(["set", "org.gnome.system.proxy.http", "host", host])
        .status();
    let _ = Command::new("gsettings")
        .args(["set", "org.gnome.system.proxy.http", "port", &p])
        .status();
    let _ = Command::new("gsettings")
        .args(["set", "org.gnome.system.proxy.https", "host", host])
        .status();
    let _ = Command::new("gsettings")
        .args(["set", "org.gnome.system.proxy.https", "port", &p])
        .status();
    let _ = Command::new("gsettings")
        .args(["set", "org.gnome.system.proxy.socks", "host", host])
        .status();
    let _ = Command::new("gsettings")
        .args(["set", "org.gnome.system.proxy.socks", "port", &p])
        .status();
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_clear() -> Result<(), String> {
    let _ = Command::new("gsettings")
        .args(["set", "org.gnome.system.proxy", "mode", "none"])
        .status();
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_apply(_: u16) -> Result<(), String> {
    Err("此平台未实现系统代理".into())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_clear() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn run(args: &[&str]) -> Result<(), String> {
    let (bin, rest) = args.split_first().ok_or("empty")?;
    let out = Command::new(bin)
        .args(rest)
        .output()
        .map_err(|e| format!("{bin}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_keeps_loopback_and_rfc1918_off_proxy() {
        let list = proxy_bypass_domains();
        assert!(list.contains(&"127.0.0.1"));
        assert!(list.contains(&"localhost"));
        assert!(list.contains(&"<local>"));
        assert!(list.contains(&"10.0.0.0/8"));
        assert!(list.contains(&"172.16.0.0/12"));
        assert!(list.contains(&"192.168.0.0/16"));
    }

    #[test]
    fn windows_bypass_uses_ipv4_wildcards_instead_of_cidr() {
        let list = windows_proxy_bypass_domains();
        assert!(list.contains(&"127.0.0.1"));
        assert!(list.contains(&"localhost"));
        assert!(list.contains(&"<local>"));
        assert!(list.contains(&"*.local"));
        assert!(list.contains(&"10.*"));
        assert!(list.contains(&"192.168.*"));
        assert!(list.contains(&"169.254.*"));
        assert!(list.contains(&"172.16.*"));
        assert!(list.contains(&"172.31.*"));
        assert!(!list.iter().any(|entry| entry.contains('/')));
    }
}
