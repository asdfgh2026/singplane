//! Assemble the runtime config and (re)start the official core.

use crate::host::{abs_path, default_config_path, default_core_path, HostClient, Status};
use crate::store::{
    active_profile, core_path_from_settings, for_runtime, load_profiles, load_settings,
    patch_settings, settings_bool, settings_str, write_runtime_config,
};
use crate::tailscale;
use crate::tun_auth;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreReload {
    Skip,
    Restart,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSwitchOutcome {
    pub restarted: bool,
    pub message: String,
}

pub fn core_reload_after_switch(running: bool) -> CoreReload {
    if running {
        CoreReload::Restart
    } else {
        CoreReload::Skip
    }
}

pub fn profile_switch_message(restarted: bool) -> String {
    if restarted {
        "已设为当前，并重载内核".into()
    } else {
        "已设为当前".into()
    }
}

pub fn apply_core_reload(
    running: bool,
    stop: impl FnOnce(),
    launch: impl FnOnce() -> Result<(), String>,
) -> Result<CoreReload, String> {
    match core_reload_after_switch(running) {
        CoreReload::Skip => Ok(CoreReload::Skip),
        CoreReload::Restart => {
            stop();
            launch()?;
            Ok(CoreReload::Restart)
        }
    }
}

pub fn prepared_runtime(user_config: &Value, settings: &Value) -> Value {
    for_runtime(user_config, settings)
}

pub fn prepare_runtime() -> Result<(), String> {
    let settings = load_settings();
    let profiles = load_profiles();
    let profile = active_profile(&settings, &profiles)
        .ok_or_else(|| "请先在配置页添加并设为当前".to_string())?;
    let raw = profile
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if raw.is_empty() {
        return Err("当前配置内容为空".into());
    }
    let parsed: Value =
        serde_json::from_str(raw).map_err(|e| format!("当前配置不是 JSON: {e}"))?;
    let ts = tailscale::TailscaleSettings::from_settings(&settings);
    if ts.enabled {
        let core = core_path_from_settings(&settings);
        tailscale::ensure_core_version(&core.to_string_lossy())?;
        tailscale::ensure_state_dir(&ts)?;
    }
    let overlaid = prepared_runtime(&parsed, &settings);
    let text = serde_json::to_string_pretty(&overlaid).map_err(|e| e.to_string())?;
    write_runtime_config(&text)?;
    Ok(())
}

pub fn launch_core(host: &HostClient) -> Status {
    let settings = load_settings();
    let raw = settings_str(&settings, "corePath");
    let core = if raw.is_empty() {
        default_core_path()
    } else {
        core_path_from_settings(&settings)
    };
    let config = default_config_path();
    let tun = settings_bool(&settings, "tunEnabled", false);
    let core = abs_path(core);
    let config = abs_path(config);
    let core = if tun && !cfg!(windows) {
        match tun_auth::ensure_privileged(&core) {
            Ok(path) => {
                if path != core {
                    let _ = patch_settings(&json!({
                        "corePath": path.to_string_lossy(),
                    }));
                }
                path
            }
            Err(e) => {
                return Status {
                    host_ok: true,
                    error: Some(e),
                    ..Status::default()
                };
            }
        }
    } else {
        core
    };
    host.start(
        &core.to_string_lossy(),
        &config.to_string_lossy(),
        cfg!(windows) && tun,
    )
}

pub fn activate_profile(id: &str) -> Result<(), String> {
    patch_settings(&json!({ "activeProfileId": id }))?;
    prepare_runtime()
}

pub fn apply_profile_switch(host: &HostClient, id: &str) -> Result<ProfileSwitchOutcome, String> {
    activate_profile(id)?;
    let running = host.status().running;
    let reload = apply_core_reload(
        running,
        || {
            let _ = host.stop();
        },
        || {
            let st = launch_core(host);
            if st.running {
                Ok(())
            } else {
                Err(st
                    .error
                    .unwrap_or_else(|| "重载内核失败".into()))
            }
        },
    )?;
    let restarted = reload == CoreReload::Restart;
    Ok(ProfileSwitchOutcome {
        restarted,
        message: profile_switch_message(restarted),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn running_switch_requires_core_restart() {
        assert_eq!(core_reload_after_switch(true), CoreReload::Restart);
        assert_eq!(core_reload_after_switch(false), CoreReload::Skip);
    }

    #[test]
    fn switch_message_mentions_reload_when_restarted() {
        assert_eq!(profile_switch_message(true), "已设为当前，并重载内核");
        assert_eq!(profile_switch_message(false), "已设为当前");
    }

    #[test]
    fn apply_reload_stops_then_launches_when_running() {
        let stops = AtomicUsize::new(0);
        let launches = AtomicUsize::new(0);
        let result = apply_core_reload(
            true,
            || {
                stops.fetch_add(1, Ordering::SeqCst);
            },
            || {
                launches.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result, CoreReload::Restart);
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        assert_eq!(launches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn apply_reload_skips_when_stopped() {
        let stops = AtomicUsize::new(0);
        let launches = AtomicUsize::new(0);
        let result = apply_core_reload(
            false,
            || {
                stops.fetch_add(1, Ordering::SeqCst);
            },
            || {
                launches.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result, CoreReload::Skip);
        assert_eq!(stops.load(Ordering::SeqCst), 0);
        assert_eq!(launches.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn apply_reload_surfaces_launch_error() {
        let err = apply_core_reload(true, || {}, || Err("launch failed".into())).unwrap_err();
        assert_eq!(err, "launch failed");
    }

    #[test]
    fn prepared_runtime_reapplies_tailscale_overlay() {
        let user = serde_json::json!({
            "outbounds": [{"type": "direct", "tag": "direct"}],
            "route": {"final": "direct"}
        });
        let settings = serde_json::json!({
            "tunEnabled": false,
            "forceAppPortsOnAssemble": false,
            "tailscale": {
                "enabled": true,
                "tag": "ts-local",
                "injectDns": true,
                "injectRoutePreferredBy": true
            }
        });
        let cfg = prepared_runtime(&user, &settings);
        let endpoints = cfg.get("endpoints").and_then(|v| v.as_array());
        let has_ts = endpoints.is_some_and(|arr| {
            arr.iter().any(|e| e.get("type").and_then(|t| t.as_str()) == Some("tailscale"))
        }) || cfg
            .get("outbounds")
            .and_then(|v| v.as_array())
            .is_some_and(|arr| {
                arr.iter()
                    .any(|o| o.get("type").and_then(|t| t.as_str()) == Some("tailscale"))
            });
        assert!(has_ts, "tailscale overlay missing: {cfg}");
    }
}
