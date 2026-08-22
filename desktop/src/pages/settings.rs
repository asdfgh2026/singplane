use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputState};
use gpui_component::switch::Switch;
use gpui_component::dialog::DialogButtonProps;
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _, StyledExt as _,
    Theme, ThemeMode, WindowExt,
};
use serde_json::{json, Value};

use crate::core_download::{self, ChannelInfo};
use crate::host::{default_core_path, HostClient};
use crate::store::{
    load_settings, load_theme_mode, patch_settings, save_theme_mode, set_disclaimer_accepted,
    settings_bool, settings_i64, settings_str, DISCLAIMER_TEXT,
};
use crate::tailscale;
use crate::i18n::{self, tr};
use crate::widgets::{card, chip, muted, page_scroll, page_title, section_header};

/// Tab: 设置. Only edit this file when implementing the settings page.
pub struct SettingsPage {
    _host: Arc<HostClient>,
    ready: bool,
    saving: bool,
    save_ok: bool,
    save_message: Option<SharedString>,

    core_path: Entity<InputState>,
    mixed_port: Entity<InputState>,
    clash_api_host: Entity<InputState>,
    clash_api_port: Entity<InputState>,
    interval: Entity<InputState>,
    template_id: Entity<InputState>,
    ts_tag: Entity<InputState>,
    ts_hostname: Entity<InputState>,
    ts_auth: Entity<InputState>,
    ts_exit: Entity<InputState>,
    ts_routes: Entity<InputState>,
    ts_domain: Entity<InputState>,
    github_proxy: Entity<InputState>,

    core_channel: String,
    theme_mode: String,
    language: String,
    close_to_tray: bool,
    tray_enabled: bool,
    launch_at_startup: bool,
    auto_update_subscriptions: bool,
    default_assemble_on_import: bool,
    force_app_ports: bool,
    ts_enabled: bool,
    ts_accept_routes: bool,
    ts_inject_dns: bool,
    ts_preferred_route: bool,
    ts_replace_other: bool,
    ts_system_interface: bool,

    core_dl_busy: bool,
    core_dl_message: SharedString,
    core_dl_error: bool,
    local_core_version: Option<SharedString>,
    stable_version: Option<SharedString>,
    beta_version: Option<SharedString>,
}

struct SettingsEdits {
    core_path: String,
    mixed_port: i64,
    clash_api_host: String,
    clash_api_port: i64,
    core_channel: String,
    close_to_tray: bool,
    tray_enabled: bool,
    launch_at_startup: bool,
    auto_update_subscriptions: bool,
    auto_update_interval_minutes: i64,
    default_assemble_on_import: bool,
    force_app_ports_on_assemble: bool,
    default_template_id: String,
    ts_enabled: bool,
    ts_tag: String,
    ts_hostname: String,
    ts_auth_key: String,
    ts_exit_node: String,
    ts_advertise_routes: String,
    ts_route_domain: String,
    ts_accept_routes: bool,
    ts_inject_dns: bool,
    ts_preferred_route: bool,
    ts_replace_other: bool,
    ts_system_interface: bool,
    theme_mode: String,
    language: String,
    github_proxy: String,
}

impl SettingsPage {
    pub fn new(host: Arc<HostClient>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let core_placeholder = default_core_path().to_string_lossy().into_owned();
        let this = Self {
            _host: host,
            ready: false,
            saving: false,
            save_ok: false,
            save_message: None,
            core_path: cx.new(|cx| InputState::new(window, cx).placeholder(core_placeholder)),
            mixed_port: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("7890")
                    .default_value("7890")
            }),
            clash_api_host: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("127.0.0.1")
                    .default_value("127.0.0.1")
            }),
            clash_api_port: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("9090")
                    .default_value("9090")
            }),
            interval: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("60")
                    .default_value("60")
            }),
            template_id: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("builtin-mixed-direct")
                    .default_value("builtin-mixed-direct")
            }),
            ts_tag: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("ts-local")
                    .default_value("ts-local")
            }),
            ts_hostname: cx.new(|cx| InputState::new(window, cx).placeholder("可留空")),
            ts_auth: cx.new(|cx| {
                InputState::new(window, cx).placeholder("tskey-auth-… 可留空")
            }),
            ts_exit: cx.new(|cx| InputState::new(window, cx).placeholder("可留空")),
            ts_routes: cx.new(|cx| InputState::new(window, cx).placeholder("192.168.1.0/24")),
            ts_domain: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(".ts.net")
                    .default_value(".ts.net")
            }),
            github_proxy: cx.new(|cx| {
                InputState::new(window, cx).placeholder("留空 = 直连 GitHub")
            }),
            core_channel: "stable".into(),
            theme_mode: "system".into(),
            language: "system".into(),
            close_to_tray: true,
            tray_enabled: true,
            launch_at_startup: false,
            auto_update_subscriptions: true,
            default_assemble_on_import: false,
            force_app_ports: true,
            ts_enabled: false,
            ts_accept_routes: true,
            ts_inject_dns: true,
            ts_preferred_route: true,
            ts_replace_other: true,
            ts_system_interface: false,
            core_dl_busy: false,
            core_dl_message: SharedString::default(),
            core_dl_error: false,
            local_core_version: None,
            stable_version: None,
            beta_version: None,
        };
        this.queue_load(window, cx);
        this
    }

    fn queue_load(&self, window: &mut Window, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async move { (load_settings(), load_theme_mode()) });
        cx.spawn_in(window, async move |this, cx| {
            let (settings, theme) = task.await;
            this.update_in(cx, |this, window, cx| {
                this.apply_loaded(settings, theme, window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn apply_loaded(
        &mut self,
        settings: Value,
        theme: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let core = settings_str(&settings, "corePath");
        write_input(&self.core_path, core, window, cx);

        write_input(
            &self.mixed_port,
            settings_i64(&settings, "mixedPort", 7890).to_string(),
            window,
            cx,
        );

        let mut host = settings_str(&settings, "clashApiHost");
        if host.is_empty() {
            host = "127.0.0.1".into();
        }
        write_input(&self.clash_api_host, host, window, cx);
        write_input(
            &self.clash_api_port,
            settings_i64(&settings, "clashApiPort", 9090).to_string(),
            window,
            cx,
        );

        let mut interval = settings_i64(&settings, "autoUpdateIntervalMinutes", 60);
        if interval < 15 {
            interval = 15;
        }
        write_input(&self.interval, interval.to_string(), window, cx);

        let mut template = settings_str(&settings, "defaultTemplateId");
        if template.is_empty() {
            template = "builtin-mixed-direct".into();
        }
        write_input(&self.template_id, template, window, cx);

        write_input(
            &self.github_proxy,
            core_download::normalize_github_proxy(&settings_str(&settings, "githubProxy")),
            window,
            cx,
        );

        let channel = settings_str(&settings, "coreChannel");
        self.core_channel = if channel == "beta" {
            "beta".into()
        } else {
            "stable".into()
        };
        self.language = settings_str(&settings, "language");
        if self.language.is_empty() {
            self.language = "system".into();
        }
        self.close_to_tray = settings_bool(&settings, "closeToTray", true);
        self.tray_enabled = settings_bool(&settings, "trayEnabled", true);
        self.launch_at_startup = crate::autostart::is_enabled()
            || settings_bool(&settings, "launchAtStartup", false);
        self.auto_update_subscriptions = settings_bool(&settings, "autoUpdateSubscriptions", true);
        self.default_assemble_on_import =
            settings_bool(&settings, "defaultAssembleOnImport", false);
        self.force_app_ports = settings_bool(&settings, "forceAppPortsOnAssemble", true);

        let ts = settings
            .get("tailscale")
            .cloned()
            .unwrap_or_else(|| json!({}));
        self.ts_enabled = settings_bool(&ts, "enabled", false);
        self.ts_accept_routes = settings_bool(&ts, "acceptRoutes", true);
        self.ts_inject_dns = settings_bool(&ts, "injectDns", true);
        self.ts_preferred_route = settings_bool(&ts, "injectRoutePreferredBy", true);
        self.ts_replace_other = settings_bool(&ts, "replaceOtherTailscale", true);
        self.ts_system_interface = settings_bool(&ts, "systemInterface", false);
        let mut tag = settings_str(&ts, "tag");
        if tag.is_empty() {
            tag = "ts-local".into();
        }
        write_input(&self.ts_tag, tag, window, cx);
        write_input(&self.ts_hostname, settings_str(&ts, "hostname"), window, cx);
        write_input(&self.ts_auth, settings_str(&ts, "authKey"), window, cx);
        write_input(&self.ts_exit, settings_str(&ts, "exitNode"), window, cx);
        write_input(
            &self.ts_routes,
            settings_str(&ts, "advertiseRoutes"),
            window,
            cx,
        );
        let domain = settings_str(&ts, "routeDomainSuffix");
        write_input(
            &self.ts_domain,
            if domain.is_empty() {
                ".ts.net".into()
            } else {
                domain
            },
            window,
            cx,
        );

        self.theme_mode = match theme.as_str() {
            "light" | "dark" => theme,
            _ => "system".into(),
        };
        apply_theme_mode(&self.theme_mode, window, cx);

        self.ready = true;
        cx.notify();
        self.refresh_core_versions(window, cx);
    }

    fn collect_edits(&self, window: &mut Window, cx: &mut Context<Self>) -> Result<SettingsEdits, String> {
        let mixed_port = parse_port(&input_text(&self.mixed_port, cx), tr("settings.mixed_port"))?;
        let clash_api_port = parse_port(&input_text(&self.clash_api_port, cx), tr("settings.clash_api_port"))?;
        let interval_raw = input_text(&self.interval, cx);
        let interval = parse_interval(&interval_raw)?;
        if interval.to_string() != interval_raw.trim() {
            write_input(&self.interval, interval.to_string(), window, cx);
        }

        let mut clash_api_host = input_text(&self.clash_api_host, cx);
        clash_api_host = clash_api_host.trim().to_string();
        if clash_api_host.is_empty() {
            clash_api_host = "127.0.0.1".into();
        }

        Ok(SettingsEdits {
            core_path: input_text(&self.core_path, cx).trim().to_string(),
            mixed_port,
            clash_api_host,
            clash_api_port,
            core_channel: self.core_channel.clone(),
            close_to_tray: self.close_to_tray,
            tray_enabled: self.tray_enabled,
            launch_at_startup: self.launch_at_startup,
            auto_update_subscriptions: self.auto_update_subscriptions,
            auto_update_interval_minutes: interval,
            default_assemble_on_import: self.default_assemble_on_import,
            force_app_ports_on_assemble: self.force_app_ports,
            default_template_id: input_text(&self.template_id, cx).trim().to_string(),
            ts_enabled: self.ts_enabled,
            ts_tag: input_text(&self.ts_tag, cx).trim().to_string(),
            ts_hostname: input_text(&self.ts_hostname, cx).trim().to_string(),
            ts_auth_key: input_text(&self.ts_auth, cx).trim().to_string(),
            ts_exit_node: input_text(&self.ts_exit, cx).trim().to_string(),
            ts_advertise_routes: input_text(&self.ts_routes, cx).trim().to_string(),
            ts_route_domain: input_text(&self.ts_domain, cx).trim().to_string(),
            ts_accept_routes: self.ts_accept_routes,
            ts_inject_dns: self.ts_inject_dns,
            ts_preferred_route: self.ts_preferred_route,
            ts_replace_other: self.ts_replace_other,
            ts_system_interface: self.ts_system_interface,
            theme_mode: self.theme_mode.clone(),
            language: self.language.clone(),
            github_proxy: core_download::normalize_github_proxy(&input_text(
                &self.github_proxy,
                cx,
            )),
        })
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let edits = match self.collect_edits(window, cx) {
            Ok(edits) => edits,
            Err(err) => {
                self.save_ok = false;
                self.save_message = Some(err.into());
                cx.notify();
                return;
            }
        };
        let mut edits = edits;
        let mut gate_err = None;
        if edits.ts_enabled {
            let core = if edits.core_path.trim().is_empty() {
                default_core_path()
            } else {
                std::path::PathBuf::from(edits.core_path.trim())
            };
            if let Err(err) = tailscale::ensure_core_version(&core.to_string_lossy()) {
                self.ts_enabled = false;
                edits.ts_enabled = false;
                gate_err = Some(err);
            }
        }
        self.saving = true;
        self.save_message = None;
        cx.notify();
        let task = cx.background_spawn(async move { persist_settings(edits) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        if let Some(err) = gate_err {
                            this.save_ok = false;
                            this.save_message = Some(err.into());
                        } else {
                            this.save_ok = true;
                            this.save_message = Some("已保存".into());
                        }
                    }
                    Err(err) => {
                        this.save_ok = false;
                        this.save_message = Some(format!("保存失败：{err}").into());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_theme(&mut self, mode: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_mode = mode.to_string();
        apply_theme_mode(mode, window, cx);
        self.save(window, cx);
    }

    fn set_language(&mut self, lang: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.language = lang.to_string();
        let lang_enum = if lang == "system" {
            i18n::detect_system_language()
        } else {
            i18n::Language::from_code(lang)
        };
        i18n::set_current_lang(lang_enum);
        self.save(window, cx);
        cx.notify();
    }

    fn set_channel(&mut self, channel: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.core_channel = channel.to_string();
        self.save(window, cx);
        self.refresh_core_versions(window, cx);
    }

    fn set_github_proxy_preset(
        &mut self,
        prefix: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        write_input(
            &self.github_proxy,
            core_download::normalize_github_proxy(prefix),
            window,
            cx,
        );
        self.save(window, cx);
        self.refresh_core_versions(window, cx);
    }

    fn apply_channel_info(&mut self, info: ChannelInfo) {
        self.local_core_version = info.local.map(Into::into);
        self.stable_version = info.stable.map(Into::into);
        self.beta_version = info.beta.map(Into::into);
        let channel = if self.core_channel == "beta" {
            "测试版"
        } else {
            "稳定版"
        };
        self.core_dl_message = match (
            self.local_core_version.as_deref(),
            if self.core_channel == "beta" {
                info.selected.as_ref().map(|r| r.version.as_str())
            } else {
                info.selected.as_ref().map(|r| r.version.as_str())
            },
        ) {
            (None, Some(remote)) => format!("可下载{channel} {remote}").into(),
            (Some(local), Some(remote)) if local == remote => {
                format!("已是{channel}最新 {local}").into()
            }
            (Some(local), Some(remote)) => {
                format!("可更新 {local} → {remote}（{channel}）").into()
            }
            (Some(local), None) => format!("已安装 {local}").into(),
            (None, None) => "未安装内核".into(),
        };
        self.core_dl_error = false;
    }

    fn refresh_core_versions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.core_dl_busy {
            return;
        }
        let path = input_text(&self.core_path, cx);
        let channel = self.core_channel.clone();
        let proxy = core_download::normalize_github_proxy(&input_text(&self.github_proxy, cx));
        self.core_dl_message = "正在检查版本…".into();
        self.core_dl_error = false;
        cx.notify();
        let task = cx.background_spawn(async move {
            core_download::inspect_channels(&path, &channel, &proxy)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(info) => this.apply_channel_info(info),
                    Err(e) => {
                        this.core_dl_error = true;
                        this.core_dl_message = format!("检查失败：{e}").into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn download_core(&mut self, channel: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.core_dl_busy {
            return;
        }
        if channel != self.core_channel {
            self.core_channel = channel.to_string();
        }
        let alpha = channel == "beta";
        let label = if alpha { "测试版" } else { "稳定版" };
        let proxy = core_download::normalize_github_proxy(&input_text(&self.github_proxy, cx));
        self.core_dl_busy = true;
        self.core_dl_error = false;
        self.core_dl_message = if proxy.is_empty() {
            format!("准备下载{label}…").into()
        } else {
            format!("准备经代理下载{label}…").into()
        };
        cx.notify();
        let task =
            cx.background_spawn(async move { core_download::download_and_install(alpha, &proxy) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.core_dl_busy = false;
                match result {
                    Ok((path, ver)) => {
                        write_input(&this.core_path, path.display().to_string(), window, cx);
                        this.local_core_version = ver.clone().map(Into::into);
                        this.core_dl_error = false;
                        this.core_dl_message = format!(
                            "已安装 {}（{}）",
                            ver.as_deref().unwrap_or("sing-box"),
                            if alpha { "测试版" } else { "稳定版" }
                        )
                        .into();
                        this.save(window, cx);
                        this.refresh_core_versions(window, cx);
                    }
                    Err(e) => {
                        this.core_dl_error = true;
                        this.core_dl_message = format!("下载失败：{e}").into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_flag(
        &mut self,
        apply: impl FnOnce(&mut Self),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        apply(self);
        self.save(window, cx);
    }

    fn open_about(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.open_sheet(cx, |sheet, _, cx| {
            sheet
                .size(px(420.))
                .overlay(true)
                .overlay_closable(true)
                .title("关于")
                .child(about_body(cx))
        });
        cx.notify();
    }

    fn open_disclaimer(&self, window: &mut Window, cx: &mut Context<Self>) {
        show_disclaimer_dialog(window, cx, false);
        cx.notify();
    }
}

impl Render for SettingsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let channel_label = if self.core_channel == "beta" {
            tr("settings.channel.beta")
        } else {
            tr("settings.channel.stable")
        };
        let theme_label = match self.theme_mode.as_str() {
            "light" => tr("settings.theme.light"),
            "dark" => tr("settings.theme.dark"),
            _ => tr("settings.theme.system"),
        };
        let save_color = if self.save_ok {
            cx.theme().success
        } else {
            cx.theme().danger
        };

        page_scroll("page-settings")
            .child(page_title(tr("settings.title"), cx))
            .child(section_header(tr("settings.core")))
            .child(
                card(cx)
                    .child(muted(
                        tr("settings.core_channel_desc"),
                        cx,
                    ))
                    .child(div().text_sm().child(tr("settings.core_channel")))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(choice_btn(
                                "btn-channel-stable",
                                tr("settings.channel.stable"),
                                self.core_channel == "stable",
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_channel("stable", window, cx);
                            })))
                            .child(choice_btn(
                                "btn-channel-beta",
                                tr("settings.channel.beta"),
                                self.core_channel == "beta",
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_channel("beta", window, cx);
                            })))
                            .child(chip(channel_label, cx)),
                    )
                    .child(field_block(
                        tr("settings.core_path"),
                        Input::new(&self.core_path).w_full(),
                    ))
                    .child(self.render_github_proxy(cx))
                    .child(self.render_core_download(cx)),
            )
            .child(section_header(tr("settings.inbounds")))
            .child(
                card(cx)
                    .child(muted(
                        tr("settings.inbounds_desc"),
                        cx,
                    ))
                    .child(field_block(tr("settings.mixed_port"), Input::new(&self.mixed_port).w_full()))
                    .child(field_block(
                        tr("settings.clash_api_host"),
                        Input::new(&self.clash_api_host).w_full(),
                    ))
                    .child(field_block(
                        tr("settings.clash_api_port"),
                        Input::new(&self.clash_api_port).w_full(),
                    )),
            )
            .child(section_header(tr("settings.ui")))
            .child(
                card(cx)
                    .child(div().text_sm().font_semibold().child(tr("settings.language")))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(choice_btn(
                                "btn-lang-system",
                                tr("settings.lang.system"),
                                self.language == "system",
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_language("system", window, cx);
                            })))
                            .child(choice_btn(
                                "btn-lang-zh-hans",
                                tr("settings.lang.zh_hans"),
                                self.language == "zh-Hans" || self.language == "zh-CN",
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_language("zh-Hans", window, cx);
                            })))
                            .child(choice_btn(
                                "btn-lang-zh-hant",
                                tr("settings.lang.zh_hant"),
                                self.language == "zh-Hant" || self.language == "zh-TW" || self.language == "zh-HK",
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_language("zh-Hant", window, cx);
                            })))
                            .child(choice_btn(
                                "btn-lang-en",
                                tr("settings.lang.en"),
                                self.language == "en" || self.language == "en-US",
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_language("en", window, cx);
                            }))),
                    )
                    .child(div().text_sm().font_semibold().child(tr("settings.theme")))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(choice_btn(
                                "btn-theme-system",
                                tr("settings.theme.system"),
                                self.theme_mode == "system",
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_theme("system", window, cx);
                            })))
                            .child(choice_btn(
                                "btn-theme-light",
                                tr("settings.theme.light"),
                                self.theme_mode == "light",
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_theme("light", window, cx);
                            })))
                            .child(choice_btn(
                                "btn-theme-dark",
                                tr("settings.theme.dark"),
                                self.theme_mode == "dark",
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_theme("dark", window, cx);
                            })))
                            .child(chip(theme_label, cx)),
                    )
                    .child(
                        Switch::new("sw-tray-enabled")
                            .label(tr("settings.tray_enabled"))
                            .checked(self.tray_enabled)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                let on = *checked;
                                this.set_flag(|s| s.tray_enabled = on, window, cx);
                                crate::tray::set_enabled(on);
                            })),
                    )
                    .child(muted(tr("settings.tray_enabled_desc"), cx))
                    .child(
                        Switch::new("sw-close-to-tray")
                            .label(tr("settings.close_to_tray"))
                            .checked(self.close_to_tray)
                            .disabled(!self.tray_enabled)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.set_flag(|s| s.close_to_tray = *checked, window, cx);
                            })),
                    )
                    .child(muted(tr("settings.close_to_tray_desc"), cx))
                    .child(
                        Switch::new("sw-launch-at-startup")
                            .label(tr("settings.launch_at_startup"))
                            .checked(self.launch_at_startup)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                let on = *checked;
                                this.launch_at_startup = on;
                                if let Err(e) = crate::autostart::set_enabled(on) {
                                    this.launch_at_startup = crate::autostart::is_enabled();
                                    this.save_message = Some(format!("{}: {e}", tr("common.failed")).into());
                                    this.save_ok = false;
                                    cx.notify();
                                    return;
                                }
                                this.set_flag(|s| s.launch_at_startup = on, window, cx);
                            })),
                    )
                    .child(muted(tr("settings.launch_at_startup_desc"), cx)),
            )
            .child(section_header(tr("settings.subscription")))
            .child(
                card(cx)
                    .child(
                        Switch::new("sw-auto-update-subs")
                            .label(tr("settings.auto_update_subs"))
                            .checked(self.auto_update_subscriptions)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.set_flag(
                                    |s| s.auto_update_subscriptions = *checked,
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .child(muted(
                        tr("settings.auto_update_subs_desc"),
                        cx,
                    ))
                    .when(self.auto_update_subscriptions, |d| {
                        d.child(field_block(
                            tr("settings.auto_update_interval"),
                            Input::new(&self.interval).w_full(),
                        ))
                    }),
            )
            .child(section_header(tr("settings.assemble")))
            .child(
                card(cx)
                    .child(muted(
                        tr("settings.assemble_desc"),
                        cx,
                    ))
                    .child(
                        Switch::new("sw-default-assemble")
                            .label(tr("settings.default_assemble"))
                            .checked(self.default_assemble_on_import)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.set_flag(
                                    |s| s.default_assemble_on_import = *checked,
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .child(muted(tr("settings.default_assemble_desc"), cx))
                    .child(
                        Switch::new("sw-force-ports")
                            .label(tr("settings.force_ports"))
                            .checked(self.force_app_ports)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.set_flag(|s| s.force_app_ports = *checked, window, cx);
                            })),
                    )
                    .child(muted(tr("settings.force_ports_desc"), cx))
                    .child(field_block(
                        tr("settings.default_template"),
                        Input::new(&self.template_id).w_full(),
                    )),
            )
            .child(section_header(tr("settings.tailscale")))
            .child(
                card(cx)
                    .child(muted(
                        tr("settings.tailscale_desc"),
                        cx,
                    ))
                    .child(field_block(
                        tr("settings.ts_tag"),
                        Input::new(&self.ts_tag).w_full(),
                    ))
                    .child(field_block(
                        tr("settings.ts_auth"),
                        Input::new(&self.ts_auth).w_full(),
                    ))
                    .child(muted(
                        tr("settings.ts_auth_hint"),
                        cx,
                    ))
                    .child(field_block(
                        tr("settings.ts_hostname"),
                        Input::new(&self.ts_hostname).w_full(),
                    ))
                    .child(field_block(
                        tr("settings.ts_exit"),
                        Input::new(&self.ts_exit).w_full(),
                    ))
                    .child(field_block(
                        tr("settings.ts_routes"),
                        Input::new(&self.ts_routes).w_full(),
                    ))
                    .child(field_block(
                        tr("settings.ts_domain"),
                        Input::new(&self.ts_domain).w_full(),
                    ))
                    .child(
                        Switch::new("sw-ts-accept-routes")
                            .label(tr("settings.ts_accept_routes"))
                            .checked(self.ts_accept_routes)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.set_flag(|s| s.ts_accept_routes = *checked, window, cx);
                            })),
                    )
                    .child(
                        Switch::new("sw-ts-inject-dns")
                            .label(tr("settings.ts_inject_dns"))
                            .checked(self.ts_inject_dns)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.set_flag(|s| s.ts_inject_dns = *checked, window, cx);
                            })),
                    )
                    .child(muted(tr("settings.ts_inject_dns_desc"), cx))
                    .child(
                        Switch::new("sw-ts-preferred")
                            .label(tr("settings.ts_preferred"))
                            .checked(self.ts_preferred_route)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.set_flag(|s| s.ts_preferred_route = *checked, window, cx);
                            })),
                    )
                    .child(
                        Switch::new("sw-ts-replace")
                            .label(tr("settings.ts_replace"))
                            .checked(self.ts_replace_other)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.set_flag(|s| s.ts_replace_other = *checked, window, cx);
                            })),
                    )
                    .child(
                        Switch::new("sw-ts-sysif")
                            .label(tr("settings.ts_sysif"))
                            .checked(self.ts_system_interface)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.set_flag(|s| s.ts_system_interface = *checked, window, cx);
                            })),
                    )
                    .child(muted(tr("settings.ts_sysif_desc"), cx)),
            )
            .child(section_header(tr("settings.other")))
            .child(
                card(cx)
                    .gap_1()
                    .child(other_row(
                        "row-disclaimer",
                        IconName::TriangleAlert,
                        tr("settings.disclaimer"),
                        tr("settings.disclaimer_desc"),
                        cx,
                        cx.listener(|this, _, window, cx| {
                            this.open_disclaimer(window, cx);
                        }),
                    ))
                    .child(other_row(
                        "row-about",
                        IconName::Info,
                        tr("settings.about"),
                        tr("settings.about_desc"),
                        cx,
                        cx.listener(|this, _, window, cx| {
                            this.open_about(window, cx);
                        }),
                    )),
            )
            .child(
                card(cx)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Button::new("btn-save")
                                    .primary()
                                    .label(if self.saving { tr("common.saving") } else { tr("common.save") })
                                    .disabled(self.saving || !self.ready)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save(window, cx);
                                    })),
                            )
                            .when_some(self.save_message.clone(), |d, msg| {
                                d.child(
                                    div()
                                        .text_sm()
                                        .text_color(save_color)
                                        .child(msg),
                                )
                            }),
                    )
                    .when(!self.ready, |d| d.child(muted(tr("settings.loading_prefs"), cx))),
            )
    }
}

impl SettingsPage {
    fn render_github_proxy(&self, cx: &mut Context<Self>) -> Div {
        let current = core_download::normalize_github_proxy(&input_text(&self.github_proxy, cx));
        let matched = core_download::matching_github_proxy_preset(&current);
        let custom = !current.is_empty() && matched.is_none();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_sm().font_semibold().child(tr("settings.github_proxy")))
            .child(muted(
                tr("settings.github_proxy_desc"),
                cx,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_wrap()
                    .children(core_download::GITHUB_PROXY_PRESETS.iter().map(|preset| {
                        let selected = matched.is_some_and(|p| p.id == preset.id);
                        let prefix = preset.prefix;
                        choice_btn(preset.id, preset.label, selected).on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.set_github_proxy_preset(prefix, window, cx);
                            },
                        ))
                    }))
                    .when(custom, |d| d.child(chip(tr("common.custom"), cx))),
            )
            .child(field_block(
                tr("settings.github_proxy_hint"),
                Input::new(&self.github_proxy).w_full(),
            ))
            .child(muted(
                "格式：https://ghfast.top  — 请求会变成 代理/https://github.com/…",
                cx,
            ))
    }

    fn render_core_download(&self, cx: &mut Context<Self>) -> Div {
        let not_installed = self.local_core_version.is_none();
        let remote = if self.core_channel == "beta" {
            self.beta_version.as_deref()
        } else {
            self.stable_version.as_deref()
        };
        let need_update = match (self.local_core_version.as_deref(), remote) {
            (Some(local), Some(remote)) => local != remote,
            _ => false,
        };
        let channel = if self.core_channel == "beta" {
            "测试版"
        } else {
            "稳定版"
        };
        let primary_label = if self.core_dl_busy {
            "下载中…".to_string()
        } else if not_installed {
            format!("下载{channel}")
        } else if need_update {
            format!("更新到{channel}")
        } else {
            format!("重新下载{channel}")
        };
        let msg_color = if self.core_dl_error {
            cx.theme().danger
        } else {
            cx.theme().muted_foreground
        };
        let platform = core_download::platform_label();
        let versions = format!(
            "本机 {} · 稳定 {} · 测试 {} · {}",
            self.local_core_version.as_deref().unwrap_or("未安装"),
            self.stable_version.as_deref().unwrap_or("—"),
            self.beta_version.as_deref().unwrap_or("—"),
            platform
        );

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_sm().font_semibold().child("官方内核"))
            .child(
                div()
                    .text_sm()
                    .text_color(msg_color)
                    .child(if self.core_dl_message.is_empty() {
                        SharedString::from("来自 SagerNet/sing-box GitHub Releases")
                    } else {
                        self.core_dl_message.clone()
                    }),
            )
            .child(muted(versions, cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("btn-dl-channel")
                            .small()
                            .primary()
                            .label(primary_label)
                            .disabled(self.core_dl_busy || !self.ready)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let ch = this.core_channel.clone();
                                this.download_core(&ch, window, cx);
                            })),
                    )
                    .child(
                        Button::new("btn-dl-stable")
                            .small()
                            .label("装稳定版")
                            .disabled(self.core_dl_busy || !self.ready)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.download_core("stable", window, cx);
                            })),
                    )
                    .child(
                        Button::new("btn-dl-beta")
                            .small()
                            .label("装测试版")
                            .disabled(self.core_dl_busy || !self.ready)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.download_core("beta", window, cx);
                            })),
                    )
                    .child(
                        Button::new("btn-dl-check")
                            .small()
                            .label("检查更新")
                            .disabled(self.core_dl_busy || !self.ready)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.refresh_core_versions(window, cx);
                            })),
                    ),
            )
            .child(muted(
                "来自官方 SagerNet/sing-box，按本机平台自动匹配安装包。切换通道后请重新启动连接。",
                cx,
            ))
    }
}

fn persist_settings(edits: SettingsEdits) -> Result<(), String> {
    let current = load_settings();
    let mut ts = current
        .get("tailscale")
        .cloned()
        .filter(|v| v.is_object())
        .unwrap_or_else(|| json!({}));
    if let Some(obj) = ts.as_object_mut() {
        obj.insert("enabled".into(), json!(edits.ts_enabled));
        obj.insert("tag".into(), json!(edits.ts_tag));
        obj.insert("hostname".into(), json!(edits.ts_hostname));
        obj.insert("authKey".into(), json!(edits.ts_auth_key));
        obj.insert("exitNode".into(), json!(edits.ts_exit_node));
        obj.insert("advertiseRoutes".into(), json!(edits.ts_advertise_routes));
        obj.insert("routeDomainSuffix".into(), json!(edits.ts_route_domain));
        obj.insert("acceptRoutes".into(), json!(edits.ts_accept_routes));
        obj.insert("injectDns".into(), json!(edits.ts_inject_dns));
        obj.insert(
            "injectRoutePreferredBy".into(),
            json!(edits.ts_preferred_route),
        );
        obj.insert(
            "replaceOtherTailscale".into(),
            json!(edits.ts_replace_other),
        );
        obj.insert("systemInterface".into(), json!(edits.ts_system_interface));
    }

    patch_settings(&json!({
        "corePath": edits.core_path,
        "mixedPort": edits.mixed_port,
        "clashApiHost": edits.clash_api_host,
        "clashApiPort": edits.clash_api_port,
        "coreChannel": edits.core_channel,
        "closeToTray": edits.close_to_tray,
        "trayEnabled": edits.tray_enabled,
        "launchAtStartup": edits.launch_at_startup,
        "autoUpdateSubscriptions": edits.auto_update_subscriptions,
        "autoUpdateIntervalMinutes": edits.auto_update_interval_minutes,
        "defaultAssembleOnImport": edits.default_assemble_on_import,
        "defaultTemplateId": edits.default_template_id,
        "forceAppPortsOnAssemble": edits.force_app_ports_on_assemble,
        "language": edits.language,
        "githubProxy": edits.github_proxy,
        "tailscale": ts,
    }))?;
    save_theme_mode(&edits.theme_mode)?;
    Ok(())
}

fn apply_theme_mode(mode: &str, window: &mut Window, cx: &mut App) {
    let resolved = match mode {
        "light" => ThemeMode::Light,
        "dark" => ThemeMode::Dark,
        _ => ThemeMode::from(window.appearance()),
    };
    Theme::change(resolved, Some(window), cx);
}

fn write_input(
    state: &Entity<InputState>,
    value: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) {
    state.update(cx, |state, cx| state.set_value(value, window, cx));
}

fn input_text(state: &Entity<InputState>, cx: &App) -> String {
    state.read(cx).value().to_string()
}

fn parse_port(raw: &str, label: &str) -> Result<i64, String> {
    let n = raw
        .trim()
        .parse::<i64>()
        .map_err(|_| format!("{label} 不是有效数字"))?;
    if n <= 0 || n > 65535 {
        return Err(format!("{label} 需在 1–65535"));
    }
    Ok(n)
}

fn parse_interval(raw: &str) -> Result<i64, String> {
    let n = raw
        .trim()
        .parse::<i64>()
        .map_err(|_| "更新间隔 不是有效数字".to_string())?;
    if n <= 0 {
        return Err("更新间隔 需为正整数".into());
    }
    Ok(if n < 15 { 15 } else { n })
}

fn field_block(label: impl Into<SharedString>, child: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_sm().child(label.into()))
        .child(child)
}

fn choice_btn(id: &'static str, label: &'static str, selected: bool) -> Button {
    let btn = Button::new(id).small().label(label).selected(selected);
    if selected {
        btn.primary()
    } else {
        btn
    }
}

pub fn show_disclaimer_dialog(window: &mut Window, cx: &mut App, quit_on_decline: bool) {
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title("免责声明")
            .description(DISCLAIMER_TEXT)
            .show_cancel(true)
            .keyboard(!quit_on_decline)
            .button_props(
                DialogButtonProps::default()
                    .ok_text("同意")
                    .cancel_text(if quit_on_decline { "退出" } else { "关闭" })
                    .show_cancel(true),
            )
            .on_ok(|_, _, _| {
                let _ = set_disclaimer_accepted(true);
                true
            })
            .on_cancel(move |_, _, cx| {
                if quit_on_decline {
                    cx.quit();
                    false
                } else {
                    true
                }
            })
    });
}

fn other_row(
    id: &'static str,
    icon: IconName,
    title: &'static str,
    hint: &'static str,
    cx: &App,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let hover = cx.theme().muted;
    let fg = cx.theme().foreground;
    let muted_fg = cx.theme().muted_foreground;
    div()
        .id(id)
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_2()
        .py_2()
        .rounded(px(12.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .on_click(on_click)
        .child(
            div()
                .w(px(20.))
                .h(px(20.))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).text_color(fg).with_size(px(18.))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .flex_1()
                .min_w_0()
                .child(div().text_sm().font_semibold().text_color(fg).child(title))
                .child(div().text_xs().text_color(muted_fg).child(hint)),
        )
}

fn about_body(cx: &App) -> Div {
    let version = option_env!("SINGPANEL_VERSION")
        .filter(|s| !s.is_empty())
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    let fg = cx.theme().foreground;
    let muted_fg = cx.theme().muted_foreground;
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .size(px(48.))
                        .rounded(px(16.))
                        .bg(cx.theme().primary)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Icon::empty()
                                .path("icons/zap.svg")
                                .text_color(cx.theme().primary_foreground)
                                .with_size(px(22.)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_lg()
                                .font_bold()
                                .text_color(fg)
                                .child("SingPanel"),
                        )
                        .child(div().text_xs().text_color(muted_fg).child(version)),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(muted_fg)
                .child(
                    "SingPanel 是基于 sing-box 打造的原生图形客户端。界面采用 Zed GPUI 构建，交互参考 FlClash，致力于提供轻量、快速、开箱即用的代理与组网体验。",
                ),
        )
        .child(div().text_xs().font_bold().text_color(fg).child("查看"))
        .child(about_link_row(
            "link-singbox",
            "sing-box",
            "SagerNet/sing-box",
            "https://github.com/SagerNet/sing-box",
            cx,
        ))
        .child(about_link_row(
            "link-flclash",
            "FlClash",
            "界面交互参考 · chen08209/FlClash",
            "https://github.com/chen08209/FlClash",
            cx,
        ))
        .child(about_link_row(
            "link-zed",
            "Zed / GPUI",
            "本面板 UI 框架 · zed-industries/zed",
            "https://github.com/zed-industries/zed",
            cx,
        ))
        .child(about_link_row(
            "link-gpui-component",
            "GPUI Component",
            "组件库 · longbridge/gpui-component",
            "https://github.com/longbridge/gpui-component",
            cx,
        ))
}

fn about_link_row(
    id: &'static str,
    title: &'static str,
    hint: &'static str,
    url: &'static str,
    cx: &App,
) -> impl IntoElement {
    let hover = cx.theme().muted;
    let fg = cx.theme().foreground;
    let muted_fg = cx.theme().muted_foreground;
    div()
        .id(id)
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .px_2()
        .py_2()
        .rounded(px(12.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .on_click(move |_, _, _| {
            let _ = open_http_url(url);
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .min_w_0()
                .child(div().text_sm().font_semibold().text_color(fg).child(title))
                .child(div().text_xs().text_color(muted_fg).child(hint)),
        )
        .child(
            Icon::new(IconName::ExternalLink)
                .text_color(muted_fg)
                .with_size(px(14.)),
        )
}

fn open_http_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("不是 http(s) 链接".into());
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .status();
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let cmd = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        std::process::Command::new(cmd)
            .arg(url)
            .status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
