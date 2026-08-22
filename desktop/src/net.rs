//! LAN IPv4 listing. Skip vEthernet / fake-ip / Tailscale.
#![allow(dead_code)]

use std::process::Command;

#[derive(Debug, Clone)]
pub struct LanAddr {
    pub iface: String,
    pub ip: String,
    pub preferred: bool,
    pub virtual_iface: bool,
}

pub fn list_lan_ipv4() -> Vec<LanAddr> {
    let mut addrs = parse_ipconfig();
    addrs.retain(|a| is_private_lan(&a.ip) && !is_skippable(&a.ip));
    addrs.sort_by_key(|a| {
        let pref = if a.preferred { 0 } else { 1 };
        let virt = if a.virtual_iface { 1 } else { 0 };
        (pref, virt, a.iface.clone())
    });
    addrs
}

pub fn primary_lan_ip() -> Option<String> {
    list_lan_ipv4().into_iter().next().map(|a| a.ip)
}

fn is_private_lan(ip: &str) -> bool {
    let mut it = ip.split('.');
    let a = it.next().and_then(|s| s.parse::<u32>().ok());
    let b = it.next().and_then(|s| s.parse::<u32>().ok());
    match (a, b) {
        (Some(10), _) => true,
        (Some(172), Some(b)) if (16..=31).contains(&b) => true,
        (Some(192), Some(168)) => true,
        _ => false,
    }
}

fn is_skippable(ip: &str) -> bool {
    ip.starts_with("127.")
        || ip.starts_with("169.254.")
        || ip.starts_with("198.18.")
        || ip.starts_with("198.19.")
        || is_tailscale(ip)
}

fn is_tailscale(ip: &str) -> bool {
    let mut it = ip.split('.');
    let a = it.next().and_then(|s| s.parse::<u32>().ok());
    let b = it.next().and_then(|s| s.parse::<u32>().ok());
    matches!((a, b), (Some(100), Some(b)) if (64..=127).contains(&b))
}

fn is_virtual(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "vethernet",
        "hyper-v",
        "wsl",
        "docker",
        "virtualbox",
        "vmware",
        "vbox",
        "tap",
        "tun",
        "vpn",
        "bluetooth",
        "loopback",
        "npcap",
        "pseudo",
        "default switch",
        "bridge",
        "vmenet",
        "awdl",
        "llw",
    ]
    .iter()
    .any(|needle| n.contains(needle))
}

fn is_preferred(name: &str) -> bool {
    if is_virtual(name) {
        return false;
    }
    let n = name.to_lowercase();
    n.contains("wlan")
        || n.contains("wi-fi")
        || n.contains("wifi")
        || n.contains("以太网")
        || n.contains("本地连接")
        || n.contains("ethernet")
        || is_bsd_en(&n)
        || is_linux_eth(&n)
}

fn is_bsd_en(n: &str) -> bool {
    n.len() >= 3
        && n.starts_with("en")
        && n[2..].bytes().all(|b| b.is_ascii_digit())
}

fn is_linux_eth(n: &str) -> bool {
    n.len() >= 4
        && n.starts_with("eth")
        && n[3..].bytes().all(|b| b.is_ascii_digit())
}

fn parse_ipconfig() -> Vec<LanAddr> {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("ipconfig");
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
        return match cmd.output() {
            Ok(o) => parse_ipconfig_text(&decode_console(&o.stdout)),
            Err(_) => Vec::new(),
        };
    }
    #[cfg(not(windows))]
    {
        let from_ip = parse_ip_addr();
        if !from_ip.is_empty() {
            return from_ip;
        }
        parse_ifconfig()
    }
}

fn parse_ifconfig() -> Vec<LanAddr> {
    match Command::new("ifconfig").arg("-a").output() {
        Ok(o) => parse_ifconfig_text(&String::from_utf8_lossy(&o.stdout)),
        Err(_) => Vec::new(),
    }
}

fn parse_ip_addr() -> Vec<LanAddr> {
    let Ok(out) = Command::new("ip")
        .args(["-4", "-o", "addr", "show"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_ip_addr_text(&String::from_utf8_lossy(&out.stdout))
}

/// `2: enp0s3    inet 10.0.2.15/24 brd 10.0.2.255 scope global enp0s3`
fn parse_ip_addr_text(out: &str) -> Vec<LanAddr> {
    let mut addrs = Vec::new();
    for line in out.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(iface) = words.get(1).map(|s| s.trim_end_matches(':').to_string()) else {
            continue;
        };
        let Some(i) = words.iter().position(|w| *w == "inet") else {
            continue;
        };
        let Some(ip) = words.get(i + 1).and_then(|c| c.split('/').next()) else {
            continue;
        };
        if ip.split('.').count() == 4 {
            addrs.push(LanAddr {
                preferred: is_preferred(&iface),
                virtual_iface: is_virtual(&iface),
                iface,
                ip: ip.to_string(),
            });
        }
    }
    addrs
}

/// macOS / Linux `ifconfig -a`.
fn parse_ifconfig_text(out: &str) -> Vec<LanAddr> {
    let mut addrs = Vec::new();
    let mut iface = String::new();
    for line in out.lines() {
        let headed = !line.starts_with([' ', '\t']) && line.contains(':');
        if headed {
            iface = line
                .split_once(':')
                .map(|(n, _)| n.trim().to_string())
                .unwrap_or_default();
            continue;
        }
        let trimmed = line.trim();
        let rest = if let Some(r) = trimmed.strip_prefix("inet addr:") {
            r
        } else if let Some(r) = trimmed.strip_prefix("inet ") {
            r
        } else {
            continue;
        };
        let ip = rest.split_whitespace().next().unwrap_or("").trim();
        if ip.split('.').count() == 4 {
            addrs.push(LanAddr {
                preferred: is_preferred(&iface),
                virtual_iface: is_virtual(&iface),
                iface: iface.clone(),
                ip: ip.to_string(),
            });
        }
    }
    addrs
}

fn parse_ipconfig_text(out: &str) -> Vec<LanAddr> {
    let mut addrs = Vec::new();
    let mut iface = String::new();
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.ends_with(':') {
            iface = normalize_iface(trimmed.trim_end_matches(':'));
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if !lower.contains("ipv4") {
            continue;
        }
        if let Some(ip) = trimmed.rsplit([':', '：']).next().map(str::trim) {
            if ip.split('.').count() == 4 {
                addrs.push(LanAddr {
                    preferred: is_preferred(&iface),
                    virtual_iface: is_virtual(&iface),
                    iface: iface.clone(),
                    ip: ip.to_string(),
                });
            }
        }
    }
    addrs
}

/// English: `Wireless LAN adapter WLAN`. Chinese: `无线局域网适配器 WLAN`.
fn normalize_iface(raw: &str) -> String {
    if let Some(rest) = raw.split_once(" adapter ") {
        return rest.1.trim().to_string();
    }
    if let Some(rest) = raw.split_once("适配器") {
        return rest.1.trim().to_string();
    }
    raw.trim().to_string()
}

fn decode_console(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }
    #[cfg(windows)]
    if let Some(s) = decode_windows_legacy(bytes) {
        return s;
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// `ipconfig` writes the OEM/ANSI code page (CP936 on Chinese Windows), not UTF-8.
#[cfg(windows)]
fn decode_windows_legacy(bytes: &[u8]) -> Option<String> {
    use std::ptr;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetACP() -> u32;
        fn GetOEMCP() -> u32;
        fn MultiByteToWideChar(
            code_page: u32,
            flags: u32,
            multi_byte_str: *const u8,
            cb_multi_byte: i32,
            wide_char_str: *mut u16,
            cch_wide_char: i32,
        ) -> i32;
    }

    unsafe {
        for cp in [GetOEMCP(), GetACP()] {
            if cp == 0 || cp == 65001 {
                continue;
            }
            let needed = MultiByteToWideChar(
                cp,
                0,
                bytes.as_ptr(),
                bytes.len() as i32,
                ptr::null_mut(),
                0,
            );
            if needed <= 0 {
                continue;
            }
            let mut wide = vec![0u16; needed as usize];
            let written = MultiByteToWideChar(
                cp,
                0,
                bytes.as_ptr(),
                bytes.len() as i32,
                wide.as_mut_ptr(),
                needed,
            );
            if written <= 0 {
                continue;
            }
            return Some(String::from_utf16_lossy(&wide[..written as usize]));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chinese_ipconfig() {
        let text = "\
无线局域网适配器 WLAN:\r\n\
   IPv4 地址 . . . . . . . . . . . . : 10.0.0.166\r\n\
\r\n\
以太网适配器 以太网:\r\n\
   IPv4 地址 . . . . . . . . . . . . : 192.168.1.8\r\n\
";
        let addrs = parse_ipconfig_text(text);
        assert_eq!(addrs.len(), 2);
        assert_eq!(addrs[0].iface, "WLAN");
        assert_eq!(addrs[0].ip, "10.0.0.166");
        assert!(addrs[0].preferred);
        assert!(!addrs[0].virtual_iface);
        assert_eq!(addrs[1].iface, "以太网");
        assert_eq!(addrs[1].ip, "192.168.1.8");
        assert!(addrs[1].preferred);
    }

    #[test]
    fn parses_english_ipconfig() {
        let text = "\
Wireless LAN adapter Wi-Fi:\r\n\
   IPv4 Address. . . . . . . . . . . : 192.168.0.12\r\n\
";
        let addrs = parse_ipconfig_text(text);
        assert_eq!(addrs[0].iface, "Wi-Fi");
        assert!(addrs[0].preferred);
    }

    #[test]
    fn parses_macos_ifconfig() {
        let text = "\
en1: flags=8863<UP,BROADCAST,SMART,RUNNING,SIMPLEX,MULTICAST> mtu 1500
	inet 10.0.0.100 netmask 0xffffff00 broadcast 10.0.0.255
lo0: flags=8049<UP,LOOPBACK,RUNNING,MULTICAST> mtu 16384
	inet 127.0.0.1 netmask 0xff000000
";
        let addrs = parse_ifconfig_text(text);
        assert_eq!(addrs.len(), 2);
        assert_eq!(addrs[0].iface, "en1");
        assert_eq!(addrs[0].ip, "10.0.0.100");
        assert!(addrs[0].preferred);
        assert!(!addrs[0].virtual_iface);
    }

    #[test]
    fn parses_linux_ip_addr() {
        let text = "2: enp0s3    inet 10.0.2.15/24 brd 10.0.2.255 scope global enp0s3\n";
        let addrs = parse_ip_addr_text(text);
        assert_eq!(addrs[0].iface, "enp0s3");
        assert_eq!(addrs[0].ip, "10.0.2.15");
    }
}
