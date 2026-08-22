#![allow(dead_code)]

use serde::{Deserialize, Serialize};

pub const PIPE_NAME: &str = r"\\.\pipe\singpanel-helper-v1";
pub const SERVICE_NAME: &str = "SingPanelHelper";
pub const SERVICE_DISPLAY: &str = "SingPanel Helper";
pub const SERVICE_DESC: &str = "Elevated helper that starts/stops the sing-box core for TUN support.";
pub const PROTOCOL_VER: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub version: u32,
    pub id: String,
    pub method: String,
    pub token: String,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok<T: Serialize>(id: String, data: T) -> Self {
        Self {
            id,
            ok: true,
            data: serde_json::to_value(data).ok(),
            error: None,
        }
    }

    pub fn err(id: String, error: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StartBody {
    pub path: String,
    #[serde(default)]
    pub config: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, rename = "workDir")]
    pub work_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusData {
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_serialization() {
        let req = Request {
            version: PROTOCOL_VER,
            id: "123".into(),
            method: "ping".into(),
            token: "secret".into(),
            body: None,
        };
        let s = serde_json::to_string(&req).unwrap();
        let parsed: Request = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.id, "123");
        assert_eq!(parsed.method, "ping");
    }

    #[test]
    fn test_response_serialization() {
        let res = Response::ok("test".into(), StatusData { running: true, pid: Some(42) });
        let s = serde_json::to_string(&res).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(s.contains("\"pid\":42"));
    }
}
