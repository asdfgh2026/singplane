//! Shared control plane. Desktop host speaks HTTP over this crate.
//! Android will later bind the same API (UniFFI / JNI). Official sing-box
//! stays the data plane.

pub mod assemble;
pub mod check;
pub mod clash;
pub mod convert;
pub mod engine;
pub mod fetch;
pub mod helper;

pub use assemble::{
    detect, run as assemble, AssembleOptions, AssembleOut, ContentKind, PatchOptions,
};
pub use check::check_content;
pub use convert::ConvertSidecar;
pub use engine::{Engine, EngineError, StartSpec, StatusSnap};
pub use fetch::{fetch_url, FetchOut};

#[cfg(test)]
mod api_tests {
    use super::*;

    #[test]
    fn detect_is_reexported() {
        assert_eq!(
            detect(r#"{"outbounds":[{"type":"direct","tag":"d"}]}"#),
            ContentKind::Singbox
        );
        assert_eq!(detect("ss://YWVzLTEyOC1nY206dGVzdA@1.2.3.4:8388#n1"), ContentKind::UriList);
    }

    #[test]
    fn missing_core_start_is_library_error() {
        let mut eng = Engine::new();
        let err = match eng.start(StartSpec {
            core_path: "C:\\missing\\sing-box.exe".into(),
            config_path: "C:\\missing\\c.json".into(),
            require_helper: false,
        }) {
            Ok(_) => panic!("expected core_missing"),
            Err(e) => e,
        };
        assert_eq!(err.code, "core_missing");
    }
}
