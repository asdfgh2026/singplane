//! Exit-IP / webpage detection.
//!
//! International sources are Cloudflare traces (typical proxy path when the
//! core is running). Domestic sources are CN traces (direct path).

use std::time::Duration;

pub const INTERNATIONAL_SOURCES: &[&str] = &[
    "https://cp.cloudflare.com/cdn-cgi/trace",
    "https://api.cloudflare.com/cdn-cgi/trace",
];

pub const DOMESTIC_SOURCES: &[&str] = &[
    "https://www.qualcomm.cn/cdn-cgi/trace",
    "https://www.cloudflare-cn.com/cdn-cgi/trace",
];

pub const MASKED_IP: &str = "*** *** *** ***";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpCheckSource {
    Auto,
    International,
    Domestic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpInfo {
    pub ip: String,
    pub country_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseTraceError;

impl IpInfo {
    pub fn from_cloudflare_trace(text: &str) -> Result<Self, ParseTraceError> {
        let mut ip = None;
        let mut country_code = None;
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "ip" => ip = Some(value.trim().to_string()),
                "loc" => country_code = Some(value.trim().to_string()),
                _ => {}
            }
        }
        match (ip, country_code) {
            (Some(ip), Some(country_code)) => Ok(Self { ip, country_code }),
            _ => Err(ParseTraceError),
        }
    }

    pub fn flag_emoji(&self) -> String {
        let code = self.country_code.to_ascii_uppercase();
        let bytes = code.as_bytes();
        if bytes.len() != 2 || !bytes.iter().all(|b| b.is_ascii_alphabetic()) {
            return self.country_code.clone();
        }
        let a = 0x1F1E6 + (bytes[0] - b'A') as u32;
        let b = 0x1F1E6 + (bytes[1] - b'A') as u32;
        format!(
            "{}{}",
            char::from_u32(a).unwrap_or('?'),
            char::from_u32(b).unwrap_or('?')
        )
    }

    pub fn masked_ip(&self) -> &'static str {
        MASKED_IP
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectionView {
    pub loading: bool,
    pub info: Option<IpInfo>,
    pub error: Option<String>,
    pub ip_masked: bool,
    pub last_source: IpCheckSource,
}

impl Default for DetectionView {
    fn default() -> Self {
        Self {
            loading: true,
            info: None,
            error: None,
            ip_masked: false,
            last_source: IpCheckSource::Auto,
        }
    }
}

impl DetectionView {
    pub fn display_ip(&self) -> Option<String> {
        let info = self.info.as_ref()?;
        Some(if self.ip_masked {
            info.masked_ip().to_string()
        } else {
            info.ip.clone()
        })
    }

    pub fn caption(&self) -> Option<String> {
        let info = self.info.as_ref()?;
        let mut parts = vec![info.country_code.to_ascii_uppercase()];
        if self.last_source == IpCheckSource::Domestic {
            parts.push("国内源".into());
        }
        if self.ip_masked {
            parts.push("已隐藏".into());
        }
        Some(parts.join(" · "))
    }

    pub fn begin_check(&mut self, source: IpCheckSource, force: bool) {
        self.loading = true;
        self.error = None;
        if force || self.info.is_none() {
            self.info = None;
        }
        self.ip_masked = false;
        self.last_source = source;
    }

    pub fn finish_ok(&mut self, info: IpInfo) {
        self.loading = false;
        self.info = Some(info);
        self.error = None;
    }

    pub fn finish_err(&mut self, message: impl Into<String>) {
        self.loading = false;
        self.info = None;
        self.error = Some(message.into());
    }

    pub fn toggle_privacy(&mut self) {
        if self.info.is_none() {
            return;
        }
        self.ip_masked = !self.ip_masked;
    }
}

/// Auto uses domestic when the core is down, international when it is up.
pub fn resolved_source(source: IpCheckSource, core_running: bool) -> IpCheckSource {
    match source {
        IpCheckSource::Auto => {
            if core_running {
                IpCheckSource::International
            } else {
                IpCheckSource::Domestic
            }
        }
        other => other,
    }
}

pub fn sources_for(source: IpCheckSource, core_running: bool) -> &'static [&'static str] {
    match resolved_source(source, core_running) {
        IpCheckSource::Domestic => DOMESTIC_SOURCES,
        IpCheckSource::International => INTERNATIONAL_SOURCES,
        IpCheckSource::Auto => unreachable!(),
    }
}

/// International checks go through the local mixed inbound when the core is
/// running so the result is the proxy exit IP. Domestic is always direct.
pub fn outbound_proxy(
    source: IpCheckSource,
    core_running: bool,
    mixed_port: i64,
) -> Option<String> {
    if mixed_port <= 0 || !core_running {
        return None;
    }
    match resolved_source(source, core_running) {
        IpCheckSource::International => Some(format!("http://127.0.0.1:{mixed_port}")),
        _ => None,
    }
}

/// First source whose body parses as a Cloudflare trace wins.
pub fn check_ip_with<F>(sources: &[&str], fetch: F) -> Option<IpInfo>
where
    F: Fn(&str) -> Option<String>,
{
    for url in sources {
        if let Some(body) = fetch(url) {
            if let Ok(info) = IpInfo::from_cloudflare_trace(&body) {
                return Some(info);
            }
        }
    }
    None
}

pub fn fetch_trace(url: &str, proxy: Option<&str>, timeout: Duration) -> Result<String, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(timeout)
        .connect_timeout(timeout)
        .no_proxy();
    if let Some(proxy) = proxy {
        let proxy = reqwest::Proxy::all(proxy).map_err(|e| e.to_string())?;
        builder = builder.proxy(proxy);
    }
    let client = builder.build().map_err(|e| e.to_string())?;
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("http {}", resp.status()));
    }
    resp.text().map_err(|e| e.to_string())
}

pub fn check_exit_ip(
    source: IpCheckSource,
    core_running: bool,
    mixed_port: i64,
) -> Option<IpInfo> {
    let sources = sources_for(source, core_running);
    let proxy = outbound_proxy(source, core_running, mixed_port);
    check_ip_with(sources, |url| {
        fetch_trace(url, proxy.as_deref(), Duration::from_secs(5)).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACE_US: &str = "\
fl=123f4
h=cp.cloudflare.com
ip=1.2.3.4
ts=1700000000.000
visit_scheme=https
uag=Mozilla
colo=SJC
sliver=none
http=http/2
loc=US
tls=TLSv1.3
sni=plaintext
warp=off
gateway=off
rbi=off
kex=X25519
";

    const TRACE_CN: &str = "ip=36.112.8.9\nloc=CN\n";

    #[test]
    fn parses_cloudflare_trace() {
        let info = IpInfo::from_cloudflare_trace(TRACE_US).unwrap();
        assert_eq!(info.ip, "1.2.3.4");
        assert_eq!(info.country_code, "US");
    }

    #[test]
    fn rejects_incomplete_trace() {
        assert!(IpInfo::from_cloudflare_trace("ip=1.2.3.4\n").is_err());
        assert!(IpInfo::from_cloudflare_trace("loc=US\n").is_err());
        assert!(IpInfo::from_cloudflare_trace("").is_err());
    }

    #[test]
    fn keeps_equals_inside_values() {
        let info = IpInfo::from_cloudflare_trace("ip=fe80::1\nloc=US\n").unwrap();
        assert_eq!(info.ip, "fe80::1");
    }

    #[test]
    fn flag_emoji_from_iso_code() {
        let us = IpInfo {
            ip: "1.1.1.1".into(),
            country_code: "us".into(),
        };
        assert_eq!(us.flag_emoji(), "🇺🇸");
        let cn = IpInfo {
            ip: "1.1.1.1".into(),
            country_code: "CN".into(),
        };
        assert_eq!(cn.flag_emoji(), "🇨🇳");
        let bad = IpInfo {
            ip: "1.1.1.1".into(),
            country_code: "XXX".into(),
        };
        assert_eq!(bad.flag_emoji(), "XXX");
    }

    #[test]
    fn masks_ip() {
        let info = IpInfo {
            ip: "1.2.3.4".into(),
            country_code: "US".into(),
        };
        assert_eq!(info.masked_ip(), "*** *** *** ***");
    }

    #[test]
    fn auto_source_follows_core_running() {
        assert_eq!(
            resolved_source(IpCheckSource::Auto, false),
            IpCheckSource::Domestic
        );
        assert_eq!(
            resolved_source(IpCheckSource::Auto, true),
            IpCheckSource::International
        );
        assert_eq!(
            resolved_source(IpCheckSource::Domestic, true),
            IpCheckSource::Domestic
        );
        assert_eq!(
            resolved_source(IpCheckSource::International, false),
            IpCheckSource::International
        );
    }

    #[test]
    fn source_lists_match_expected() {
        assert_eq!(
            sources_for(IpCheckSource::International, false),
            INTERNATIONAL_SOURCES
        );
        assert_eq!(
            sources_for(IpCheckSource::Domestic, true),
            DOMESTIC_SOURCES
        );
        assert_eq!(sources_for(IpCheckSource::Auto, false), DOMESTIC_SOURCES);
        assert_eq!(
            sources_for(IpCheckSource::Auto, true),
            INTERNATIONAL_SOURCES
        );
    }

    #[test]
    fn international_uses_local_mixed_proxy_when_running() {
        assert_eq!(
            outbound_proxy(IpCheckSource::International, true, 2080),
            Some("http://127.0.0.1:2080".into())
        );
        assert_eq!(
            outbound_proxy(IpCheckSource::Auto, true, 7890),
            Some("http://127.0.0.1:7890".into())
        );
        assert_eq!(outbound_proxy(IpCheckSource::Domestic, true, 2080), None);
        assert_eq!(outbound_proxy(IpCheckSource::International, false, 2080), None);
        assert_eq!(outbound_proxy(IpCheckSource::Auto, true, 0), None);
    }

    #[test]
    fn first_successful_source_wins() {
        let info = check_ip_with(
            &[
                "https://bad.example/trace",
                "https://good.example/trace",
                "https://later.example/trace",
            ],
            |url| match url {
                "https://bad.example/trace" => Some("not a trace".into()),
                "https://good.example/trace" => Some(TRACE_US.into()),
                "https://later.example/trace" => Some(TRACE_CN.into()),
                _ => None,
            },
        )
        .unwrap();
        assert_eq!(info.ip, "1.2.3.4");
        assert_eq!(info.country_code, "US");
    }

    #[test]
    fn all_sources_failing_returns_none() {
        assert!(check_ip_with(&["a", "b"], |_| None).is_none());
        assert!(check_ip_with(&["a"], |_| Some("nope".into())).is_none());
    }

    #[test]
    fn view_display_and_caption() {
        let mut view = DetectionView::default();
        view.finish_ok(IpInfo::from_cloudflare_trace(TRACE_US).unwrap());
        view.last_source = IpCheckSource::International;
        assert_eq!(view.display_ip().as_deref(), Some("1.2.3.4"));
        assert_eq!(view.caption().as_deref(), Some("US"));

        view.last_source = IpCheckSource::Domestic;
        view.toggle_privacy();
        assert_eq!(view.display_ip().as_deref(), Some(MASKED_IP));
        assert_eq!(view.caption().as_deref(), Some("US · 国内源 · 已隐藏"));

        view.toggle_privacy();
        assert_eq!(view.display_ip().as_deref(), Some("1.2.3.4"));
        assert_eq!(view.caption().as_deref(), Some("US · 国内源"));
    }

    #[test]
    fn toggle_privacy_without_info_is_noop() {
        let mut view = DetectionView::default();
        view.loading = false;
        view.toggle_privacy();
        assert!(!view.ip_masked);
        assert!(view.info.is_none());
    }

    #[test]
    fn begin_check_clears_on_force() {
        let mut view = DetectionView::default();
        view.finish_ok(IpInfo::from_cloudflare_trace(TRACE_CN).unwrap());
        view.toggle_privacy();
        view.begin_check(IpCheckSource::International, true);
        assert!(view.loading);
        assert!(view.info.is_none());
        assert!(view.error.is_none());
        assert!(!view.ip_masked);
        assert_eq!(view.last_source, IpCheckSource::International);
    }

    #[test]
    fn finish_err_sets_retry_copy() {
        let mut view = DetectionView::default();
        view.finish_err("检测失败，点击重试");
        assert!(!view.loading);
        assert!(view.info.is_none());
        assert_eq!(view.error.as_deref(), Some("检测失败，点击重试"));
        assert!(view.display_ip().is_none());
    }
}
