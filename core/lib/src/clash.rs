use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClashReq {
    pub base_url: String,
    #[serde(default)]
    pub secret: String,
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub query: Value,
    #[serde(default)]
    pub body: Option<Value>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

pub async fn proxy(req: ClashReq) -> Value {
    let base = req.base_url.trim_end_matches('/');
    let path = if req.path.starts_with('/') {
        req.path.clone()
    } else {
        format!("/{}", req.path)
    };
    let mut url = match reqwest::Url::parse(&format!("{base}{path}")) {
        Ok(u) => u,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": e.to_string()});
        }
    };
    if let Some(obj) = req.query.as_object() {
        let mut qp = url.query_pairs_mut();
        for (k, v) in obj {
            let s = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            qp.append_pair(k, &s);
        }
    }
    let timeout = std::time::Duration::from_millis(req.timeout_ms.unwrap_or(8000).max(1000));
    let client = match reqwest::Client::builder().no_proxy().timeout(timeout).build() {
        Ok(c) => c,
        Err(e) => return serde_json::json!({"ok": false, "error": e.to_string()}),
    };
    let method = req.method.to_uppercase();
    let mut builder = match method.as_str() {
        "GET" => client.get(url),
        "PUT" => client.put(url),
        "PATCH" => client.patch(url),
        "POST" => client.post(url),
        "DELETE" => client.delete(url),
        _ => {
            return serde_json::json!({"ok": false, "error": "unsupported method"});
        }
    };
    if !req.secret.is_empty() {
        builder = builder.bearer_auth(&req.secret);
    }
    if let Some(body) = req.body {
        builder = builder.json(&body);
    }
    match builder.send().await {
        Ok(res) => {
            let status = res.status().as_u16();
            let text = res.text().await.unwrap_or_default();
            let json: Value = serde_json::from_str(&text).unwrap_or(Value::String(text.clone()));
            serde_json::json!({
                "ok": (200..300).contains(&status),
                "status": status,
                "json": json,
                "raw": text,
            })
        }
        Err(e) => serde_json::json!({"ok": false, "error": e.to_string()}),
    }
}
