use serde::Serialize;

const UA: &str = "sing-box/SingPanel clash.meta";
const MAX_BODY: usize = 16 * 1024 * 1024;
const MAX_ERR: usize = 64 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchOut {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub upload: i64,
    pub download: i64,
    pub total: i64,
    pub expire_ms: i64,
    pub http_status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn fetch_url(url: &str) -> FetchOut {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return err("订阅 URL 不能为空");
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return err("URL 必须以 http:// 或 https:// 开头");
    }
    let mut parsed = match reqwest::Url::parse(trimmed) {
        Ok(u) => u,
        Err(e) => return err(&format!("URL 无效: {e}")),
    };
    let client = match reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::limited(8))
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(UA)
        .build()
    {
        Ok(c) => c,
        Err(e) => return err(&e.to_string()),
    };
    let mut req = client.get(parsed.clone());
    if !parsed.username().is_empty() || parsed.password().is_some() {
        let user = parsed.username().to_string();
        let pass = parsed.password().unwrap_or("").to_string();
        let b64 = data_encoding_base64(&format!("{user}:{pass}"));
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
        req = client
            .get(parsed)
            .header("Authorization", format!("Basic {b64}"));
    }
    let res = match req.header("Accept", "*/*").send().await {
        Ok(r) => r,
        Err(e) => return err(&e.to_string()),
    };
    let status = res.status().as_u16();
    let ctype = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let userinfo = res
        .headers()
        .iter()
        .find(|(k, _)| k.as_str().eq_ignore_ascii_case("subscription-userinfo"))
        .and_then(|(_, v)| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = match res.bytes().await {
        Ok(b) => b,
        Err(e) => return err(&e.to_string()),
    };
    if status != 200 {
        let snippet = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_ERR)]);
        return FetchOut {
            ok: false,
            body: None,
            upload: 0,
            download: 0,
            total: 0,
            expire_ms: 0,
            http_status: status,
            content_type: ctype,
            error: Some(format!(
                "HTTP {status}: {}",
                snippet.trim().if_empty("(empty body)")
            )),
        };
    }
    if bytes.len() > MAX_BODY {
        return err(&format!("响应超过 {MAX_BODY} 字节上限"));
    }
    let mut body = String::from_utf8_lossy(&bytes).into_owned();
    if body.starts_with('\u{feff}') {
        body = body.trim_start_matches('\u{feff}').to_string();
    }
    let body = body.trim().to_string();
    if body.is_empty() {
        return err(&format!("订阅内容为空 (HTTP {status})"));
    }
    let info = parse_userinfo(&userinfo);
    FetchOut {
        ok: true,
        body: Some(body),
        upload: info.0,
        download: info.1,
        total: info.2,
        expire_ms: info.3 * 1000,
        http_status: status,
        content_type: ctype,
        error: None,
    }
}

fn parse_userinfo(raw: &str) -> (i64, i64, i64, i64) {
    let mut upload = 0;
    let mut download = 0;
    let mut total = 0;
    let mut expire = 0;
    for part in raw.split(';') {
        let p = part.trim();
        let Some((k, v)) = p.split_once('=') else {
            continue;
        };
        let n = v.trim().parse::<i64>().unwrap_or(0);
        match k.trim().to_ascii_lowercase().as_str() {
            "upload" => upload = n,
            "download" => download = n,
            "total" => total = n,
            "expire" => expire = n,
            _ => {}
        }
    }
    (upload, download, total, expire)
}

fn err(msg: &str) -> FetchOut {
    FetchOut {
        ok: false,
        body: None,
        upload: 0,
        download: 0,
        total: 0,
        expire_ms: 0,
        http_status: 0,
        content_type: None,
        error: Some(msg.into()),
    }
}

fn data_encoding_base64(s: &str) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] } else { 0 };
        out.push(T[(b0 >> 2) as usize] as char);
        out.push(T[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if i + 1 < bytes.len() {
            out.push(T[(((b1 & 15) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < bytes.len() {
            out.push(T[(b2 & 63) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

trait IfEmpty {
    fn if_empty(self, other: &str) -> String;
}
impl IfEmpty for &str {
    fn if_empty(self, other: &str) -> String {
        if self.is_empty() {
            other.to_string()
        } else {
            self.to_string()
        }
    }
}
