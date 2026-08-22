use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::*;
use gpui_component::dialog::DialogButtonProps;
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::switch::Switch;
use gpui_component::*;
use serde_json::Value;

use crate::host::{clash_call_result, default_core_path, HostClient, Status};
use crate::net::{list_lan_ipv4, primary_lan_ip, LanAddr};
use crate::net_detect::{self, DetectionView, IpCheckSource};
use crate::runtime::{launch_core, prepare_runtime};
use crate::store::{
    active_profile, clash_base_from_settings, core_path_from_settings, load_profiles,
    load_settings, patch_settings, runtime_proxy_port, settings_bool, settings_str,
};
use crate::sysproxy;
use crate::tun_auth;
use crate::tailscale::{self, TsPhase, TsStatus};
use crate::win_helper;
use crate::i18n::tr;
use crate::widgets::{
    chip, chip_tone, info_header, muted, page_scroll, page_title, tile, ChipTone,
};

#[derive(Clone)]
struct LanLine {
    iface: SharedString,
    ip: SharedString,
    virtual_iface: bool,
    preferred: bool,
}

pub struct HomePage {
    host: Arc<HostClient>,
    status: Status,
    busy: bool,
    message: SharedString,

    profile_name: SharedString,
    mixed_port: i64,
    clash_display: SharedString,
    clash_base: String,
    core_name: SharedString,
    tailscale_enabled: bool,
    ts_busy: bool,
    ts_status: TsStatus,
    ts_error: Option<SharedString>,

    lan: Vec<LanLine>,
    primary_lan: Option<SharedString>,
    lan_loading: bool,

    net_detect: DetectionView,
    net_detect_req: u64,

    up_total: u64,
    down_total: u64,
    up_speed: u64,
    down_speed: u64,
    last_up: u64,
    last_down: u64,
    last_sample: Option<Instant>,
    traffic_error: Option<SharedString>,

    proxy_mode: SharedString,
    mode_busy: bool,
    memory_inuse: u64,
    clash_secret: String,

    poll_gen: u64,
    traffic_loop_live: bool,
    need_helper: bool,
    helper_ready: bool,
    helper_busy: bool,

    system_proxy: bool,
    tun_enabled: bool,
    tun_error: Option<SharedString>,
    sysproxy_applied: bool,
    running_since: Option<Instant>,
    uptime_tick_live: bool,
    _on_quit: Subscription,
}

impl HomePage {
    pub fn new(host: Arc<HostClient>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let snap = load_settings_snapshot();
        let running = false;
        let on_quit = cx.on_app_quit(|this, _cx| {
            this.release_takeover();
            async {}
        });
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            if crate::tray::close_hides() {
                cx.hide();
                return false;
            }
            if let Some(home) = weak.upgrade() {
                home.update(cx, |this, _| this.release_takeover());
            }
            true
        });
        let mut page = Self {
            host,
            status: Status::default(),
            busy: false,
            message: "正在连接控制面…".into(),
            profile_name: snap.profile_name.into(),
            mixed_port: snap.mixed_port,
            clash_display: snap.clash_display.into(),
            clash_base: snap.clash_base,
            core_name: snap.core_name.into(),
            tailscale_enabled: snap.tailscale_enabled,
            ts_busy: false,
            ts_status: tailscale::status_view(
                &tailscale::TailscaleSettings::from_settings(&load_settings()),
                running,
            ),
            ts_error: None,
            lan: Vec::new(),
            primary_lan: None,
            lan_loading: true,
            net_detect: DetectionView::default(),
            net_detect_req: 0,
            up_total: 0,
            down_total: 0,
            up_speed: 0,
            down_speed: 0,
            last_up: 0,
            last_down: 0,
            last_sample: None,
            traffic_error: None,
            proxy_mode: "rule".into(),
            mode_busy: false,
            memory_inuse: 0,
            clash_secret: snap.clash_secret,
            poll_gen: 0,
            traffic_loop_live: false,
            need_helper: false,
            helper_ready: false,
            helper_busy: false,
            system_proxy: snap.system_proxy,
            tun_enabled: snap.tun_enabled,
            tun_error: None,
            sysproxy_applied: false,
            running_since: None,
            uptime_tick_live: false,
            _on_quit: on_quit,
        };
        // Last run may have left OS proxy on after a crash / kill.
        if page.system_proxy {
            let _ = sysproxy::clear();
        }
        let _ = sysproxy::clear_tun_dns();
        page.spawn_lan(cx);
        page.probe_helper(cx);
        page.connect_host(cx);
        page.refresh_net_detect(IpCheckSource::Auto, true, cx);
        page
    }

    /// Undo system proxy and stop the core. Safe to call more than once.
    pub(crate) fn release_takeover(&mut self) {
        let _ = sysproxy::clear();
        self.sysproxy_applied = false;
        let _ = sysproxy::clear_tun_dns();
        self.host.shutdown_now();
    }

    fn connect_host(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let task = cx.background_spawn(async move {
            match host.ensure() {
                Ok(()) => host.status(),
                Err(e) => Status {
                    error: Some(e),
                    ..Status::default()
                },
            }
        });
        cx.spawn(async move |this, cx| {
            let status = task.await;
            this.update(cx, |this, cx| {
                this.apply(status, cx);
                this.sync_traffic_loop(cx);
                this.ensure_uptime_tick(cx);
                this.refresh_ts_status();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.busy = true;
        self.lan_loading = true;
        cx.notify();
        let task = cx.background_spawn(async move {
            let status = host.status();
            let snap = load_settings_snapshot();
            let lan = collect_lan();
            (status, snap, lan)
        });
        cx.spawn(async move |this, cx| {
            let (status, snap, lan) = task.await;
            this.update(cx, |this, cx| {
                this.apply_settings(snap);
                this.apply_lan(lan);
                this.apply(status, cx);
                this.sync_traffic_loop(cx);
                this.refresh_ts_status();
                this.probe_helper(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn probe_helper(&mut self, cx: &mut Context<Self>) {
        if !cfg!(windows) {
            return;
        }
        let task = cx.background_spawn(async move { win_helper::HelperSnap::probe() });
        cx.spawn(async move |this, cx| {
            let snap = task.await;
            this.update(cx, |this, cx| {
                this.helper_ready = snap.available;
                if snap.available {
                    this.need_helper = false;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.busy = true;
        self.message = "正在启动…".into();
        cx.notify();
        let task = cx.background_spawn(async move {
            match prepare_runtime() {
                Ok(()) => (launch_core(&host), load_settings_snapshot()),
                Err(e) => (
                    Status {
                        host_ok: true,
                        error: Some(e),
                        ..Status::default()
                    },
                    load_settings_snapshot(),
                ),
            }
        });
        cx.spawn(async move |this, cx| {
            let (status, snap) = task.await;
            this.update(cx, |this, cx| {
                this.apply_settings(snap);
                this.apply(status, cx);
                this.sync_traffic_loop(cx);
                this.ensure_uptime_tick(cx);
                this.refresh_ts_status();
                if this.status.running {
                    this.watch_after_start(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_system_proxy(&mut self, on: bool, cx: &mut Context<Self>) {
        self.system_proxy = on;
        let _ = patch_settings(&serde_json::json!({ "systemProxyEnabled": on }));
        if self.status.running {
            self.sync_sysproxy();
        }
        cx.notify();
    }

    fn set_tun_enabled(&mut self, on: bool, cx: &mut Context<Self>) {
        if !on {
            self.tun_enabled = false;
            self.tun_error = None;
            let _ = patch_settings(&serde_json::json!({ "tunEnabled": false }));
            cx.notify();
            if self.status.running && !self.busy {
                self.restart(cx);
            }
            return;
        }
        if self.busy {
            return;
        }
        if cfg!(windows) {
            self.enable_tun_windows(cx);
            return;
        }
        let settings = load_settings();
        let raw = settings_str(&settings, "corePath");
        let core = if raw.is_empty() {
            default_core_path()
        } else {
            core_path_from_settings(&settings)
        };
        self.busy = true;
        self.message = "请在弹出的窗口输入密码，授权虚拟网卡…".into();
        cx.notify();
        let original = core.clone();
        let task = cx.background_spawn(async move { tun_auth::ensure_privileged(&core) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(path) => {
                        this.tun_enabled = true;
                        this.tun_error = None;
                        let mut patch = serde_json::json!({ "tunEnabled": true });
                        if path != original {
                            patch["corePath"] =
                                serde_json::Value::String(path.to_string_lossy().into_owned());
                        }
                        let _ = patch_settings(&patch);
                        this.message = "虚拟网卡已授权".into();
                        if this.status.running {
                            this.restart(cx);
                        } else {
                            cx.notify();
                        }
                    }
                    Err(e) => {
                        this.tun_enabled = false;
                        this.tun_error = Some(e.clone().into());
                        let _ = patch_settings(&serde_json::json!({ "tunEnabled": false }));
                        this.message = e.into();
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// GUI stays unelevated; one UAC installs SYSTEM helper.
    fn enable_tun_windows(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.message = "Windows 虚拟网卡走权限服务。若弹出 UAC，请点「是」…".into();
        cx.notify();
        let task = cx.background_spawn(async move {
            let snap = win_helper::HelperSnap::probe();
            if snap.available {
                return Ok(snap);
            }
            win_helper::install_elevated()
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(_) => {
                        this.tun_enabled = true;
                        this.tun_error = None;
                        this.need_helper = false;
                        this.helper_ready = true;
                        let _ = patch_settings(&serde_json::json!({ "tunEnabled": true }));
                        this.message = "权限服务已就绪".into();
                        if let Err(e) = this.host.recycle() {
                            this.message = format!("控制面重连失败: {e}").into();
                            cx.notify();
                            return;
                        }
                        if this.status.running {
                            this.restart(cx);
                        } else {
                            cx.notify();
                        }
                    }
                    Err(e) => {
                        this.tun_enabled = false;
                        this.need_helper = true;
                        this.tun_error = Some(e.clone().into());
                        let _ = patch_settings(&serde_json::json!({ "tunEnabled": false }));
                        this.message = e.into();
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.busy = true;
        self.message = "正在重载内核…".into();
        cx.notify();
        let task = cx.background_spawn(async move {
            host.stop();
            match prepare_runtime() {
                Ok(()) => (launch_core(&host), load_settings_snapshot()),
                Err(e) => (
                    Status {
                        host_ok: true,
                        error: Some(e),
                        ..Status::default()
                    },
                    load_settings_snapshot(),
                ),
            }
        });
        cx.spawn(async move |this, cx| {
            let (status, snap) = task.await;
            this.update(cx, |this, cx| {
                this.apply_settings(snap);
                this.apply(status, cx);
                this.sync_traffic_loop(cx);
                this.ensure_uptime_tick(cx);
                this.refresh_ts_status();
                if this.status.running {
                    this.watch_after_start(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn sync_sysproxy(&mut self) {
        if self.system_proxy && self.status.running {
            let port = runtime_proxy_port(&load_settings());
            self.mixed_port = port as i64;
            match sysproxy::apply(port) {
                Ok(()) => self.sysproxy_applied = true,
                Err(e) => {
                    self.sysproxy_applied = false;
                    self.message = format!("系统代理失败: {e}").into();
                }
            }
        } else {
            self.drop_sysproxy();
        }
    }

    fn ensure_uptime_tick(&mut self, cx: &mut Context<Self>) {
        if self.uptime_tick_live || !self.status.running {
            return;
        }
        self.uptime_tick_live = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(1))
                    .await;
                let keep = this
                    .update(cx, |page, cx| {
                        if page.status.running && page.running_since.is_some() {
                            cx.notify();
                            true
                        } else {
                            page.uptime_tick_live = false;
                            false
                        }
                    })
                    .ok()
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
    }

    fn drop_sysproxy(&mut self) {
        if !self.sysproxy_applied {
            return;
        }
        let _ = sysproxy::clear();
        self.sysproxy_applied = false;
    }

    pub(crate) fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self.status.running {
            self.stop(cx);
        } else {
            self.start(cx);
        }
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.busy = true;
        self.message = "正在停止…".into();
        cx.notify();
        let task = cx.background_spawn(async move { host.stop() });
        cx.spawn(async move |this, cx| {
            let status = task.await;
            this.update(cx, |this, cx| {
                this.apply(status, cx);
                this.sync_traffic_loop(cx);
                this.refresh_ts_status();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn restart_core(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let host = self.host.clone();
        self.busy = true;
        self.message = "正在重载配置…".into();
        cx.notify();
        let task = cx.background_spawn(async move {
            let _ = host.stop();
            std::thread::sleep(Duration::from_millis(300));
            match prepare_runtime() {
                Ok(()) => launch_core(&host),
                Err(e) => Status {
                    host_ok: true,
                    error: Some(e),
                    ..Status::default()
                },
            }
        });
        cx.spawn(async move |this, cx| {
            let status = task.await;
            this.update(cx, |this, cx| {
                this.apply(status, cx);
                this.apply_settings(load_settings_snapshot());
                this.sync_traffic_loop(cx);
                this.refresh_ts_status();
                if this.status.running {
                    this.watch_after_start(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_tailscale_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.ts_busy {
            return;
        }
        self.ts_busy = true;
        cx.notify();
        let running = self.status.running;
        let task = cx.background_spawn(async move {
            if enabled {
                let settings = load_settings();
                let core = core_path_from_settings(&settings);
                tailscale::ensure_core_version(&core.to_string_lossy())?;
            }
            persist_tailscale_enabled(enabled)?;
            Ok::<bool, String>(running)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.ts_busy = false;
                match result {
                    Ok(was_running) => {
                        this.tailscale_enabled = enabled;
                        this.ts_error = None;
                        this.refresh_ts_status();
                        if was_running {
                            this.restart_core(cx);
                        } else {
                            cx.notify();
                        }
                    }
                    Err(e) => {
                        this.message = e.clone().into();
                        this.ts_error = Some(e.into());
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn refresh_ts_status(&mut self) {
        let settings = load_settings();
        let ts = tailscale::TailscaleSettings::from_settings(&settings);
        self.tailscale_enabled = ts.enabled;
        self.ts_status = tailscale::status_view(&ts, self.status.running);
    }

    fn copy_ts_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        self.message = "已复制".into();
        cx.notify();
    }

    fn copy_ts_login(&mut self, cx: &mut Context<Self>) {
        let Some(url) = self.ts_login_url() else {
            self.message = "还没有授权链接，等日志出现 login.tailscale.com".into();
            cx.notify();
            return;
        };
        self.copy_ts_text(&url, cx);
        self.message = "已复制授权链接".into();
        cx.notify();
    }

    fn open_ts_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(url) = self.ts_login_url() else {
            window.open_alert_dialog(cx, |alert, _, _| {
                alert
                    .title("还没有授权链接")
                    .description("等内核日志出现 login.tailscale.com，或先点「复制授权链接」。")
            });
            cx.notify();
            return;
        };
        let opened = open_http_url(&url);
        let copy = url.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let desc = match &opened {
                Ok(()) => format!("已尝试打开浏览器。若没有跳转，请复制：\n{copy}"),
                Err(e) => format!("无法自动打开浏览器（{e}）。请复制：\n{copy}"),
            };
            let clip = copy.clone();
            alert
                .title("Tailscale 授权")
                .description(desc)
                .button_props(DialogButtonProps::default().ok_text("复制链接"))
                .on_ok(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(clip.clone()));
                    true
                })
        });
        cx.notify();
    }

    fn ts_login_url(&self) -> Option<String> {
        self.ts_status
            .login_url
            .clone()
            .or_else(tailscale::login_url_from_log)
    }

    fn apply(&mut self, status: Status, cx: &mut Context<Self>) {
        self.busy = false;
        self.need_helper = status.code.as_deref() == Some("need_helper")
            || status.error.as_deref().is_some_and(|e| {
                e.contains("权限服务") || e.contains("全局接管") || e.contains("Access is denied")
            });
        if status.via_helper {
            self.helper_ready = true;
        }
        if let Some(err) = &status.error {
            self.message = err.clone().into();
        } else if status.running {
            self.message = SharedString::default();
        } else if !status.host_ok {
            self.message = "控制面未连接".into();
        } else {
            self.message = SharedString::default();
        }
        let now_running = status.running;
        if now_running && self.running_since.is_none() {
            self.running_since = Some(Instant::now());
        } else if !now_running {
            self.running_since = None;
        }
        let running_changed = now_running != self.status.running;
        self.status = status;
        crate::tray::set_running(self.status.running);
        if self.status.running && self.system_proxy {
            self.sync_sysproxy();
        } else if !self.status.running {
            self.drop_sysproxy();
        }
        if self.status.running && self.tun_enabled {
            if let Err(e) = sysproxy::apply_tun_dns("172.19.0.1") {
                self.message = format!("TUN DNS 失败: {e}").into();
            }
        } else {
            let _ = sysproxy::clear_tun_dns();
        }
        if running_changed {
            self.refresh_net_detect(IpCheckSource::Auto, true, cx);
        }
    }

    fn apply_settings(&mut self, snap: SettingsSnapshot) {
        self.profile_name = snap.profile_name.into();
        self.mixed_port = snap.mixed_port;
        self.clash_display = snap.clash_display.into();
        self.clash_base = snap.clash_base;
        self.core_name = snap.core_name.into();
        self.tailscale_enabled = snap.tailscale_enabled;
        self.clash_secret = snap.clash_secret;
        self.system_proxy = snap.system_proxy;
        self.tun_enabled = snap.tun_enabled;
    }

    fn apply_lan(&mut self, snap: LanSnapshot) {
        self.lan = snap
            .addrs
            .into_iter()
            .map(|a| LanLine {
                iface: a.iface.into(),
                ip: a.ip.into(),
                virtual_iface: a.virtual_iface,
                preferred: a.preferred,
            })
            .collect();
        self.primary_lan = snap.primary.map(Into::into);
        self.lan_loading = false;
    }

    fn spawn_lan(&mut self, cx: &mut Context<Self>) {
        self.lan_loading = true;
        let task = cx.background_spawn(async { collect_lan() });
        cx.spawn(async move |this, cx| {
            let snap = task.await;
            this.update(cx, |this, cx| {
                this.apply_lan(snap);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn sync_traffic_loop(&mut self, cx: &mut Context<Self>) {
        if self.status.running {
            self.start_traffic_loop(cx);
        } else {
            self.reset_traffic();
        }
    }

    fn start_traffic_loop(&mut self, cx: &mut Context<Self>) {
        if self.traffic_loop_live {
            return;
        }
        self.traffic_loop_live = true;
        let gen = self.poll_gen;
        cx.spawn(async move |this, cx| {
            loop {
                let Some((host, base, secret)) = this
                    .update(cx, |page, _| {
                        if page.poll_gen != gen || !page.status.running {
                            None
                        } else {
                            Some((
                                page.host.clone(),
                                page.clash_base.clone(),
                                page.clash_secret.clone(),
                            ))
                        }
                    })
                    .ok()
                    .flatten()
                else {
                    break;
                };

                let task = cx.background_spawn(async move {
                    fetch_runtime_stats(&host, &base, &secret)
                });
                let result = task.await;
                let keep = this
                    .update(cx, |page, cx| {
                        if page.poll_gen != gen || !page.status.running {
                            return false;
                        }
                        page.apply_runtime(result);
                        page.refresh_ts_status();
                        cx.notify();
                        true
                    })
                    .ok()
                    .unwrap_or(false);
                if !keep {
                    break;
                }

                cx.background_executor()
                    .timer(Duration::from_secs(2))
                    .await;
            }
            this.update(cx, |page, _| {
                if page.poll_gen == gen {
                    page.traffic_loop_live = false;
                }
            })
            .ok();
        })
        .detach();
    }

    fn reset_traffic(&mut self) {
        self.poll_gen = self.poll_gen.wrapping_add(1);
        self.traffic_loop_live = false;
        self.up_total = 0;
        self.down_total = 0;
        self.up_speed = 0;
        self.down_speed = 0;
        self.last_up = 0;
        self.last_down = 0;
        self.last_sample = None;
        self.traffic_error = None;
        self.memory_inuse = 0;
        if !self.status.running {
            self.proxy_mode = "rule".into();
        }
    }

    fn watch_after_start(&mut self, cx: &mut Context<Self>) {
        let gen = self.poll_gen;
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            for _ in 0..20 {
                cx.background_executor()
                    .timer(Duration::from_secs(2))
                    .await;
                let still = this
                    .update(cx, |page, _| page.poll_gen == gen && page.status.running)
                    .ok()
                    .unwrap_or(false);
                if !still {
                    return;
                }
                let host = host.clone();
                let status = cx.background_spawn(async move { host.status() }).await;
                let keep = this
                    .update(cx, |page, cx| {
                        if page.poll_gen != gen {
                            return false;
                        }
                        if !status.running {
                            let mut status = status;
                            if status.error.is_none() {
                                let tail = crate::host::read_core_log_tail(1200);
                                if let Some(fatal) = tail.lines().rev().find(|l| l.contains("FATAL"))
                                {
                                    status.error = Some(fatal.trim().to_string());
                                } else if !tail.trim().is_empty() {
                                    status.error = Some("内核已退出".into());
                                }
                            }
                            page.apply(status, cx);
                            page.sync_traffic_loop(cx);
                            page.refresh_ts_status();
                            cx.notify();
                            return false;
                        }
                        true
                    })
                    .ok()
                    .unwrap_or(false);
                if !keep {
                    return;
                }
            }
        })
        .detach();
    }

    fn install_helper(&mut self, cx: &mut Context<Self>) {
        if self.helper_busy {
            return;
        }
        self.helper_busy = true;
        self.message = "请在弹出的权限窗口中点「是」…".into();
        cx.notify();
        let task = cx.background_spawn(async move { win_helper::install_elevated() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.helper_busy = false;
                match result {
                    Ok(_) => {
                        this.need_helper = false;
                        this.helper_ready = true;
                        this.message = "权限服务已就绪，正在重连控制面…".into();
                        if let Err(e) = this.host.recycle() {
                            this.message = format!("控制面重连失败: {e}").into();
                            cx.notify();
                            return;
                        }
                        this.start(cx);
                    }
                    Err(e) => {
                        this.need_helper = true;
                        this.message = e.into();
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn apply_runtime(&mut self, result: Result<RuntimeStats, String>) {
        match result {
            Ok(stats) => {
                if let Some(mode) = stats.mode {
                    self.proxy_mode = mode.into();
                }
                self.memory_inuse = stats.memory;
                let up = stats.up_total;
                let down = stats.down_total;
                let now = Instant::now();
                if let Some(prev) = self.last_sample {
                    let dt = now.duration_since(prev).as_secs_f64();
                    if dt > 0.0 {
                        self.up_speed =
                            ((up.saturating_sub(self.last_up) as f64) / dt).max(0.0) as u64;
                        self.down_speed =
                            ((down.saturating_sub(self.last_down) as f64) / dt).max(0.0) as u64;
                    }
                }
                self.last_up = up;
                self.last_down = down;
                self.up_total = up;
                self.down_total = down;
                self.last_sample = Some(now);
                self.traffic_error = None;
            }
            Err(e) => {
                self.traffic_error = Some(e.into());
            }
        }
    }

    fn set_proxy_mode(&mut self, mode: &'static str, cx: &mut Context<Self>) {
        if self.mode_busy || !self.status.running {
            return;
        }
        if self.proxy_mode.as_ref() == mode {
            return;
        }
        self.mode_busy = true;
        self.proxy_mode = mode.into();
        cx.notify();
        let host = self.host.clone();
        let base = self.clash_base.clone();
        let secret = self.clash_secret.clone();
        let task = cx.background_spawn(async move { patch_proxy_mode(&host, &base, &secret, mode) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |page, cx| {
                page.mode_busy = false;
                match result {
                    Ok(applied) => page.proxy_mode = applied.into(),
                    Err(e) => page.traffic_error = Some(e.into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Drop for HomePage {
    fn drop(&mut self) {
        let _ = sysproxy::clear();
        self.sysproxy_applied = false;
    }
}

impl Render for HomePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.status.running;
        let busy = self.busy;
        let entity = cx.entity().downgrade();
        let toggle_e = entity.clone();
        let refresh_e = entity.clone();
        let helper_e = entity.clone();
        let show_helper = self.need_helper || self.helper_busy;

        let status_label = if busy {
            if running {
                tr("home.status.stopping")
            } else {
                tr("home.status.starting")
            }
        } else if running {
            tr("home.status.running")
        } else if self.status.host_ok {
            tr("home.status.stopped")
        } else {
            tr("home.status.stopped")
        };

        let lan_card = self.render_lan_card(cx);

        page_scroll("page-home")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(page_title(tr("nav.home"), cx))
                    .child(chip_tone(
                        if running { tr("common.enabled") } else { tr("common.disabled") },
                        if running {
                            ChipTone::Success
                        } else {
                            ChipTone::Neutral
                        },
                        cx,
                    ))
                    .child(
                        Button::new("refresh")
                            .small()
                            .label(tr("common.refresh"))
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                if let Some(e) = refresh_e.upgrade() {
                                    e.update(cx, |this, cx| this.refresh(cx));
                                }
                            }),
                    )
            )
            .child(
                card_row()
                    .child(self.render_takeover_card(
                        entity.clone(),
                        "tile-sysproxy",
                        "icons/globe.svg",
                        tr("home.card.system_proxy"),
                        "选项",
                        self.system_proxy,
                        !busy,
                        cx,
                        |this, on, cx| this.set_system_proxy(on, cx),
                    ))
                    .child(self.render_takeover_card(
                        entity.clone(),
                        "tile-tun",
                        "icons/network.svg",
                        tr("home.card.tun_mode"),
                        tun_card_hint(
                            cfg!(windows),
                            self.helper_ready,
                            self.tun_enabled,
                            self.tun_error.as_deref(),
                        ),
                        self.tun_enabled,
                        !busy,
                        cx,
                        |this, on, cx| this.set_tun_enabled(on, cx),
                    )),
            )
            .when(show_helper, |d| {
                d.child(
                    tile(cx)
                        .child(info_header("权限服务", cx))
                        .child(muted("Windows 虚拟网卡需要安装一次 SYSTEM 权限服务", cx))
                        .child(
                            Button::new("install-helper")
                                .small()
                                .primary()
                                .label(if self.helper_busy {
                                    "安装中…"
                                } else {
                                    "安装权限服务"
                                })
                                .disabled(self.helper_busy || busy)
                                .on_click(move |_, _, cx| {
                                    if let Some(e) = helper_e.upgrade() {
                                        e.update(cx, |this, cx| this.install_helper(cx));
                                    }
                                }),
                        ),
                )
            })
            .child(
                card_row()
                    .child(
                        tile(cx)
                            .id("tile-power")
                            .flex_1()
                            .min_w(px(280.))
                            .cursor_pointer()
                            .when(running, |d| d.border_color(cx.theme().primary))
                            .on_click(move |_, _, cx| {
                                if let Some(e) = toggle_e.upgrade() {
                                    e.update(cx, |this, cx| this.toggle(cx));
                                }
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(info_header(
                                        if running { "运行时长" } else { "电源开关" },
                                        cx,
                                    ))
                                    .child(chip_tone(
                                        status_label,
                                        if running {
                                            ChipTone::Success
                                        } else if self.status.host_ok {
                                            ChipTone::Neutral
                                        } else {
                                            ChipTone::Danger
                                        },
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .text_color(if self.status.error.is_some() && !running {
                                        cx.theme().danger
                                    } else {
                                        cx.theme().foreground
                                    })
                                    .child(if running {
                                        SharedString::from(
                                            self.running_since
                                                .map(format_uptime)
                                                .unwrap_or_else(|| "00:00".into()),
                                        )
                                    } else if busy || self.status.error.is_some() {
                                        self.message.clone()
                                    } else {
                                        SharedString::from(tr("home.btn.start"))
                                    }),
                            )
                            .child(muted(self.profile_name.clone(), cx)),
                    )
                    .child(self.render_net_detect_card(entity.clone(), cx))
                    .child(self.render_ts_card(entity.clone(), cx)),
            )
            .when(self.traffic_error.is_some(), |d| {
                d.child(muted(
                    format!(
                        "流量暂不可用 · {}",
                        self.traffic_error
                            .as_ref()
                            .map(|s| s.as_ref())
                            .unwrap_or("")
                    ),
                    cx,
                ))
            })
            .child(
                card_row()
                    .child(lan_card.flex_1().min_w(px(240.)))
                    .child(traffic_card(
                        cx,
                        tr("home.card.upload"),
                        self.up_total,
                        self.up_speed,
                        running,
                        cx.theme().magenta,
                    ))
                    .child(traffic_card(
                        cx,
                        tr("home.card.download"),
                        self.down_total,
                        self.down_speed,
                        running,
                        cx.theme().primary,
                    )),
            )
            .child(
                card_row()
                    .child(self.render_mode_card(running, entity.clone(), cx))
                    .child(
                        tile(cx)
                            .flex_1()
                            .min_w(px(260.))
                            .child(info_header("运行信息", cx))
                            .child(
                                div()
                                    .flex()
                                    .gap_4()
                                    .child(info_cell(cx, "配置", self.profile_name.clone()))
                                    .child(info_cell(cx, "端口", format!("{}", self.mixed_port))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_4()
                                    .child(info_cell(cx, "API", self.clash_display.clone()))
                                    .child(info_cell(cx, "内核", self.core_name.clone())),
                            ),
                    )
                    .child(memory_card(cx, self.memory_inuse, running)),
            )
    }
}

impl HomePage {
    fn refresh_net_detect(
        &mut self,
        source: IpCheckSource,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        self.net_detect.begin_check(source, force);
        self.net_detect_req = self.net_detect_req.wrapping_add(1);
        let id = self.net_detect_req;
        let running = self.status.running;
        let port = self.mixed_port;
        cx.notify();
        let task = cx.background_spawn(async move {
            net_detect::check_exit_ip(source, running, port)
        });
        cx.spawn(async move |this, cx| {
            let info = task.await;
            this.update(cx, |this, cx| {
                if this.net_detect_req != id {
                    return;
                }
                match info {
                    Some(info) => this.net_detect.finish_ok(info),
                    None => this.net_detect.finish_err("检测失败，点击重试"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn render_net_detect_card(
        &self,
        entity: WeakEntity<Self>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = &self.net_detect;
        let click_e = entity.clone();
        let menu_refresh = entity.clone();
        let menu_cn = entity.clone();
        let menu_hide = entity.clone();
        let hide_label = if view.ip_masked {
            "显示IP"
        } else {
            "隐藏IP显示"
        };
        let value = if view.loading {
            SharedString::from("检测中…")
        } else if let Some(ip) = view.display_ip() {
            SharedString::from(ip)
        } else {
            SharedString::from(
                view.error
                    .clone()
                    .unwrap_or_else(|| "检测失败，点击重试".into()),
            )
        };
        let value_color = if view.loading {
            cx.theme().muted_foreground
        } else if view.info.is_none() {
            cx.theme().danger
        } else {
            cx.theme().foreground
        };
        let flag = view
            .info
            .as_ref()
            .filter(|_| !view.loading)
            .map(|info| info.flag_emoji());
        let caption = view
            .info
            .as_ref()
            .filter(|_| !view.loading)
            .and_then(|_| view.caption());

        tile(cx)
            .id("tile-net-detect")
            .flex_1()
            .min_w(px(220.))
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                if let Some(view) = click_e.upgrade() {
                    view.update(cx, |this, cx| {
                        this.refresh_net_detect(IpCheckSource::Auto, true, cx)
                    });
                }
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when_some(flag, |d, emoji| {
                                d.child(div().text_base().child(SharedString::from(emoji)))
                            })
                            .child(info_header("网络检测", cx)),
                    )
                    .child(
                        Button::new("net-detect-menu")
                            .ghost()
                            .small()
                            .icon(IconName::Ellipsis)
                            .dropdown_menu(move |menu, _, _| {
                                let refresh = menu_refresh.clone();
                                let cn = menu_cn.clone();
                                let hide = menu_hide.clone();
                                menu.item(
                                    PopupMenuItem::new("重新获取IP").on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(view) = refresh.upgrade() {
                                            view.update(cx, |this, cx| {
                                                this.refresh_net_detect(
                                                    IpCheckSource::Auto,
                                                    true,
                                                    cx,
                                                )
                                            });
                                        }
                                    }),
                                )
                                .item(
                                    PopupMenuItem::new("获取国内IP").on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(view) = cn.upgrade() {
                                            view.update(cx, |this, cx| {
                                                this.refresh_net_detect(
                                                    IpCheckSource::Domestic,
                                                    true,
                                                    cx,
                                                )
                                            });
                                        }
                                    }),
                                )
                                .item(PopupMenuItem::new(hide_label).on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(view) = hide.upgrade() {
                                        view.update(cx, |this, cx| {
                                            this.net_detect.toggle_privacy();
                                            cx.notify();
                                        });
                                    }
                                }))
                            }),
                    ),
            )
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .text_color(value_color)
                    .child(value),
            )
            .when_some(caption, |d, text| d.child(muted(text, cx)))
    }

    fn render_takeover_card(
        &self,
        entity: WeakEntity<Self>,
        id: &'static str,
        icon: &'static str,
        title: &'static str,
        hint: impl Into<SharedString>,
        on: bool,
        enabled: bool,
        cx: &mut Context<Self>,
        apply: fn(&mut Self, bool, &mut Context<Self>),
    ) -> Stateful<Div> {
        let hint = hint.into();
        let fg = cx.theme().foreground;
        let muted_fg = cx.theme().muted_foreground;
        tile(cx)
            .id(id)
            .flex_1()
            .min_w(px(220.))
            .when(on, |d| d.border_color(cx.theme().primary))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Icon::empty()
                                    .path(icon)
                                    .text_color(fg)
                                    .with_size(px(20.)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_sm().font_semibold().text_color(fg).child(title))
                                    .child(div().text_xs().text_color(muted_fg).child(hint)),
                            ),
                    )
                    .child(
                        Switch::new(SharedString::from(format!("sw-{id}")))
                            .checked(on)
                            .disabled(!enabled)
                            .on_click(move |checked, _, cx| {
                                let next = *checked;
                                if let Some(view) = entity.upgrade() {
                                    view.update(cx, |this, cx| apply(this, next, cx));
                                }
                            }),
                    ),
            )
    }

    fn render_ts_card(
        &self,
        entity: WeakEntity<Self>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let st = &self.ts_status;
        let tone = match st.phase {
            TsPhase::Injected => ChipTone::Success,
            TsPhase::NeedsLogin | TsPhase::Pending | TsPhase::Offline => ChipTone::Warning,
            TsPhase::Error => ChipTone::Danger,
            TsPhase::Ready => ChipTone::Primary,
            TsPhase::Disabled => ChipTone::Neutral,
        };
        let toggle_e = entity.clone();
        let copy_ip = st.self_ip.clone();
        let auth_url = st
            .login_url
            .clone()
            .or_else(tailscale::login_url_from_log);
        let show_auth_actions = matches!(st.phase, TsPhase::NeedsLogin | TsPhase::Pending)
            || auth_url.is_some();
        let enabled = self.tailscale_enabled;
        let can_toggle = !self.ts_busy && !self.busy;
        tile(cx)
            .id("tile-tailscale")
            .flex_1()
            .min_w(px(280.))
            .when(enabled, |d| d.border_color(cx.theme().primary))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(info_header("Tailscale", cx))
                    .child(chip_tone(st.title.0.clone(), tone, cx)),
            )
            .when(!st.subtitle.0.is_empty(), |d| {
                d.child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(match st.phase {
                            TsPhase::Injected | TsPhase::Ready => cx.theme().primary,
                            TsPhase::Pending | TsPhase::NeedsLogin | TsPhase::Offline => {
                                cx.theme().warning
                            }
                            TsPhase::Error => cx.theme().danger,
                            _ => cx.theme().foreground,
                        })
                        .child(st.subtitle.0.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("ts-toggle")
                            .small()
                            .label(if self.ts_busy {
                                "处理中…"
                            } else if enabled {
                                "关闭"
                            } else {
                                "开启"
                            })
                            .disabled(!can_toggle)
                            .when(enabled, |b| b.primary())
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                if !can_toggle {
                                    return;
                                }
                                if let Some(view) = toggle_e.upgrade() {
                                    view.update(cx, |this, cx| {
                                        this.set_tailscale_enabled(!enabled, cx)
                                    });
                                }
                            }),
                    )
                    .when_some(copy_ip, |d, ip| {
                        let e = entity.clone();
                        let ip2 = ip.clone();
                        d.child(
                            Button::new("ts-copy-ip")
                                .small()
                                .label("复制 IP")
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(view) = e.upgrade() {
                                        view.update(cx, |this, cx| this.copy_ts_text(&ip2, cx));
                                    }
                                }),
                        )
                    })
                    .when(show_auth_actions, |d| {
                        let copy_e = entity.clone();
                        let open_e = entity.clone();
                        d.child(
                            Button::new("ts-copy-url")
                                .small()
                                .label("复制授权链接")
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(view) = copy_e.upgrade() {
                                        view.update(cx, |this, cx| this.copy_ts_login(cx));
                                    }
                                }),
                        )
                        .child(
                            Button::new("ts-open-url")
                                .small()
                                .primary()
                                .label("打开授权页")
                                .on_click(move |_, window, cx| {
                                    cx.stop_propagation();
                                    if let Some(view) = open_e.upgrade() {
                                        view.update(cx, |this, cx| this.open_ts_login(window, cx));
                                    }
                                }),
                        )
                    }),
            )
            .when_some(
                st.hostname
                    .clone()
                    .filter(|h| st.subtitle.0 != *h),
                |d, host| d.child(muted(host, cx)),
            )
            .when_some(self.ts_error.clone(), |d, err| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(err),
                )
            })

    }

    fn render_mode_card(
        &self,
        running: bool,
        entity: WeakEntity<Self>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let current = normalize_mode(self.proxy_mode.as_ref());
        let mut body = tile(cx)
            .id("tile-mode")
            .flex_1()
            .min_w(px(200.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(info_header("代理模式", cx))
                    .child(chip_tone(
                        mode_label(current),
                        if running {
                            ChipTone::Primary
                        } else {
                            ChipTone::Neutral
                        },
                        cx,
                    )),
            );
        for (id, mode, label) in [
            ("mode-rule", "rule", "规则"),
            ("mode-global", "global", "全局"),
            ("mode-direct", "direct", "直连"),
        ] {
            let active = current == mode;
            let e = entity.clone();
            let enabled = running && !self.mode_busy;
            body = body.child(
                div()
                    .id(id)
                    .w_full()
                    .px_1()
                    .py_1()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .when(active, |d| d.bg(cx.theme().secondary))
                    .when(enabled && !active, |d| {
                        d.hover(|s| s.bg(cx.theme().muted))
                    })
                    .when(enabled, |d| {
                        d.on_click(move |_, _, cx| {
                            if let Some(view) = e.upgrade() {
                                view.update(cx, |this, cx| this.set_proxy_mode(mode, cx));
                            }
                        })
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .when(active, |d| d.font_bold())
                                    .text_color(if active {
                                        cx.theme().primary
                                    } else if enabled {
                                        cx.theme().foreground
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .child(if active { "●" } else { "○" }),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .when(active, |d| d.font_bold())
                                    .text_color(if enabled {
                                        cx.theme().foreground
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .child(label),
                            ),
                    ),
            );
        }
        if !running {
            body = body.child(muted("启动后可切换", cx));
        }
        body
    }

    fn render_lan_card(&self, cx: &mut Context<Self>) -> Div {
        let body = tile(cx).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(info_header("内网 IP", cx))
                .when(self.primary_lan.is_some(), |d| {
                    d.child(chip(
                        format!(
                            "主地址 {}",
                            self.primary_lan
                                .as_ref()
                                .map(|s| s.as_ref())
                                .unwrap_or("—")
                        ),
                        cx,
                    ))
                }),
        );

        if self.lan_loading && self.lan.is_empty() {
            return body.child(muted("检测中…", cx));
        }
        if self.lan.is_empty() {
            return body.child(muted("无可用内网地址", cx));
        }

        let shown = self.lan.iter().take(3);
        let extra = self.lan.len().saturating_sub(3);
        let mut list = div().flex().flex_col().gap_1();
        for (i, row) in shown.enumerate() {
            let mut line = div()
                .id(("lan-row", i))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .child(row.ip.clone()),
                )
                .child(muted(row.iface.clone(), cx));
            if row.preferred {
                line = line.child(chip_tone("首选", ChipTone::Primary, cx));
            }
            if row.virtual_iface {
                line = line.child(chip("虚拟", cx));
            }
            list = list.child(line);
        }
        if extra > 0 {
            list = list.child(muted(format!("另有 {extra} 个地址"), cx));
        }
        body.child(list)
    }
}

fn card_row() -> Div {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_stretch()
        .gap_3()
}

fn traffic_card(
    cx: &App,
    title: &'static str,
    total: u64,
    speed: u64,
    running: bool,
    accent: Hsla,
) -> Div {
    tile(cx)
        .flex_1()
        .min_w(px(180.))
        .child(info_header(title, cx))
        .child(
            div()
                .text_xl()
                .font_semibold()
                .text_color(accent)
                .child(format_bytes(total)),
        )
        .child(muted(
            if running {
                format!("{}/s", format_bytes(speed))
            } else {
                "未运行".into()
            },
            cx,
        ))
}

fn memory_card(cx: &App, inuse: u64, running: bool) -> Stateful<Div> {
    tile(cx)
        .id("tile-memory")
        .flex_1()
        .min_w(px(180.))
        .child(info_header("内存", cx))
        .child(
            div()
                .text_lg()
                .font_bold()
                .text_color(if running {
                    cx.theme().foreground
                } else {
                    cx.theme().muted_foreground
                })
                .child(if running {
                    format_bytes(inuse)
                } else {
                    "—".into()
                }),
        )
        .child(muted(
            if running {
                "Clash HeapInuse"
            } else {
                "启动后显示"
            },
            cx,
        ))
}

fn info_cell(cx: &App, label: &'static str, value: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .flex_1()
        .child(muted(label, cx))
        .child(div().text_sm().font_semibold().child(value.into()))
}

fn format_uptime(since: Instant) -> String {
    let secs = since.elapsed().as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    let n = n as f64;
    if n < KB {
        format!("{n:.0} B")
    } else if n < KB * KB {
        format!("{:.1} KB", n / KB)
    } else if n < KB * KB * KB {
        format!("{:.2} MB", n / (KB * KB))
    } else {
        format!("{:.2} GB", n / (KB * KB * KB))
    }
}

fn json_u64(v: &Value, key: &str) -> u64 {
    let Some(val) = v.get(key) else {
        return 0;
    };
    if let Some(n) = val.as_u64() {
        return n;
    }
    if let Some(n) = val.as_i64() {
        return n.max(0) as u64;
    }
    if let Some(n) = val.as_f64() {
        return n.max(0.0) as u64;
    }
    0
}

struct SettingsSnapshot {
    profile_name: String,
    mixed_port: i64,
    clash_display: String,
    clash_base: String,
    clash_secret: String,
    core_name: String,
    tailscale_enabled: bool,
    system_proxy: bool,
    tun_enabled: bool,
}

struct RuntimeStats {
    up_total: u64,
    down_total: u64,
    memory: u64,
    mode: Option<String>,
}

struct LanSnapshot {
    addrs: Vec<LanAddr>,
    primary: Option<String>,
}

fn load_settings_snapshot() -> SettingsSnapshot {
    let settings = load_settings();
    let profiles = load_profiles();
    let profile = active_profile(&settings, &profiles);
    let profile_name = profile
        .as_ref()
        .and_then(|p| p.get("name").and_then(|v| v.as_str()))
        .filter(|s| !s.is_empty())
        .unwrap_or("未选择配置")
        .to_string();
    let mixed_port = runtime_proxy_port(&settings) as i64;
    let clash_base = clash_base_from_settings(&settings);
    let clash_display = clash_base
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string();
    let raw = settings_str(&settings, "corePath");
    let core = if raw.is_empty() {
        default_core_path()
    } else {
        core_path_from_settings(&settings)
    };
    let core_name = core
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未设置".into());
    let tailscale_enabled = settings
        .get("tailscale")
        .and_then(|v| v.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let clash_secret = crate::store::clash_secret_for_calls(&settings);
    SettingsSnapshot {
        profile_name,
        mixed_port,
        clash_display,
        clash_base,
        clash_secret,
        core_name,
        tailscale_enabled,
        system_proxy: settings_bool(&settings, "systemProxyEnabled", true),
        tun_enabled: settings_bool(&settings, "tunEnabled", false),
    }
}

fn fetch_runtime_stats(host: &HostClient, base: &str, secret: &str) -> Result<RuntimeStats, String> {
    let conn = host.clash_json(base, secret, "GET", "/connections", None, None, Some(5000))?;
    let configs = host.clash_json(base, secret, "GET", "/configs", None, None, Some(4000));
    let mode = configs.ok().and_then(|v| {
        v.get("mode")
            .and_then(|m| m.as_str())
            .map(normalize_mode)
            .map(str::to_string)
    });
    Ok(RuntimeStats {
        up_total: json_u64(&conn, "uploadTotal"),
        down_total: json_u64(&conn, "downloadTotal"),
        memory: json_u64(&conn, "memory"),
        mode,
    })
}

fn patch_proxy_mode(
    host: &HostClient,
    base: &str,
    secret: &str,
    mode: &str,
) -> Result<String, String> {
    let body = serde_json::json!({ "mode": mode });
    let r = host.clash(
        base,
        secret,
        "PATCH",
        "/configs",
        None,
        Some(body),
        Some(5000),
    );
    clash_call_result(r)?;
    if let Ok(cfg) = host.clash_json(base, secret, "GET", "/configs", None, None, Some(4000)) {
        if let Some(m) = cfg.get("mode").and_then(|v| v.as_str()) {
            return Ok(normalize_mode(m).to_string());
        }
    }
    Ok(normalize_mode(mode).to_string())
}

fn normalize_mode(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "global" => "global",
        "direct" => "direct",
        _ => "rule",
    }
}

fn mode_label(mode: &str) -> &'static str {
    match mode {
        "global" => "全局",
        "direct" => "直连",
        _ => "规则",
    }
}

fn persist_tailscale_enabled(enabled: bool) -> Result<(), String> {
    let current = load_settings();
    let mut ts = current
        .get("tailscale")
        .cloned()
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = ts.as_object_mut() {
        obj.insert("enabled".into(), serde_json::json!(enabled));
        if !obj.contains_key("tag") {
            obj.insert("tag".into(), serde_json::json!("ts-local"));
        }
    }
    patch_settings(&serde_json::json!({ "tailscale": ts }))?;
    Ok(())
}

fn tun_card_hint(
    windows: bool,
    helper_ready: bool,
    tun_enabled: bool,
    tun_error: Option<&str>,
) -> String {
    if let Some(err) = tun_error {
        if !err.is_empty() {
            return err.to_string();
        }
    }
    if !windows {
        return "首次开启需管理员密码".into();
    }
    if helper_ready || tun_enabled {
        "权限服务已就绪".into()
    } else {
        "首次开启安装权限服务（UAC 一次）".into()
    }
}

#[cfg(test)]
mod tun_hint_tests {
    use super::tun_card_hint;

    #[test]
    fn windows_first_open_asks_for_helper() {
        assert_eq!(
            tun_card_hint(true, false, false, None),
            "首次开启安装权限服务（UAC 一次）"
        );
    }

    #[test]
    fn windows_ready_after_install() {
        assert_eq!(
            tun_card_hint(true, true, false, None),
            "权限服务已就绪"
        );
        assert_eq!(
            tun_card_hint(true, false, true, None),
            "权限服务已就绪"
        );
    }

    #[test]
    fn error_wins() {
        assert_eq!(
            tun_card_hint(true, true, true, Some("安装未完成")),
            "安装未完成"
        );
    }
}

fn collect_lan() -> LanSnapshot {
    let addrs = list_lan_ipv4();
    let primary = addrs
        .first()
        .map(|a| a.ip.clone())
        .or_else(primary_lan_ip);
    LanSnapshot { addrs, primary }
}

fn open_http_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("不是 http(s) 链接".into());
    }
    #[cfg(windows)]
    {
        return open_http_url_windows(url);
    }
    #[cfg(not(windows))]
    {
        let status = if cfg!(target_os = "macos") {
            std::process::Command::new("open").arg(url).status()
        } else {
            std::process::Command::new("xdg-open").arg(url).status()
        }
        .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("退出码 {}", status.code().unwrap_or(-1)))
        }
    }
}

/// `cmd /C start` often no-ops from a windowed GPUI process. ShellExecute is the OS contract.
#[cfg(windows)]
fn open_http_url_windows(url: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut std::ffi::c_void,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> isize;
    }

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let op = wide("open");
    let file = wide(url);
    let ret = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            op.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if ret > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecute {ret}"))
    }
}
