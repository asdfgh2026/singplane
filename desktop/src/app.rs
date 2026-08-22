use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{Icon, *};

use crate::host::HostClient;
use crate::store::{load_settings, settings_bool};
use crate::tray::{self, TrayAction};
use crate::pages::{
    ConnectionsPage, HomePage, LogsPage, ProfilesPage, ProxiesPage, SettingsPage, TemplatesPage,
};
use crate::store::disclaimer_accepted;
use crate::i18n::{self, tr};

pub const NAV_KEYS: [&str; 7] = [
    "nav.home",
    "nav.proxies",
    "nav.connections",
    "nav.profiles",
    "nav.templates",
    "nav.logs",
    "nav.settings",
];

/// Lucide icons (ISC) — paths in `desktop/assets/icons/`.
const NAV_ICONS: [&str; 7] = [
    "icons/layout-dashboard.svg",
    "icons/network.svg",
    "icons/activity.svg",
    "icons/folder.svg",
    "icons/layout-template.svg",
    "icons/scroll-text.svg",
    "icons/settings.svg",
];

const RAIL_WIDE: f32 = 200.0;

/// Shell only. Do not put page UI here — each tab lives in `pages/*.rs`.
/// Tabs other than 首页 are created on first visit so the window paints faster.
pub struct AppShell {
    tab: usize,
    host: Arc<HostClient>,
    home: Entity<HomePage>,
    proxies: Option<Entity<ProxiesPage>>,
    connections: Option<Entity<ConnectionsPage>>,
    profiles: Option<Entity<ProfilesPage>>,
    templates: Option<Entity<TemplatesPage>>,
    logs: Option<Entity<LogsPage>>,
    settings: Option<Entity<SettingsPage>>,
}

impl AppShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let host = Arc::new(HostClient::new());
        let settings = load_settings();
        let lang_str = settings.get("language").and_then(serde_json::Value::as_str).unwrap_or("system");
        if lang_str == "system" {
            i18n::set_current_lang(i18n::detect_system_language());
        } else {
            i18n::set_current_lang(i18n::Language::from_code(lang_str));
        }

        let this = Self {
            tab: 0,
            home: cx.new(|cx| HomePage::new(host.clone(), window, cx)),
            host,
            proxies: None,
            connections: None,
            profiles: None,
            templates: None,
            logs: None,
            settings: None,
        };
        if settings_bool(&settings, "trayEnabled", true) {
            tray::set_enabled(true);
        }
        this.spawn_tray_pump(cx);
        if !disclaimer_accepted() {
            cx.spawn_in(window, async move |this, cx| {
                this.update_in(cx, |_, window, cx| {
                    crate::pages::settings::show_disclaimer_dialog(window, cx, true);
                })
                .ok();
            })
            .detach();
        }
        this
    }

    fn spawn_tray_pump(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let keep = this
                    .update(cx, |shell, cx| {
                        shell.drain_tray(cx);
                        true
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

    fn drain_tray(&mut self, cx: &mut Context<Self>) {
        while let Some(action) = tray::poll() {
            match action {
                TrayAction::Show => {
                    cx.activate(true);
                    if let Some(handle) = cx.windows().first().cloned() {
                        handle
                            .update(cx, |_, window, _| window.activate_window())
                            .ok();
                    }
                }
                TrayAction::ToggleCore => {
                    self.home.update(cx, |home, cx| home.toggle(cx));
                }
                TrayAction::Quit => {
                    self.home.update(cx, |home, _| home.release_takeover());
                    cx.quit();
                }
            }
        }
    }

    fn select_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= NAV_KEYS.len() {
            return;
        }
        self.ensure_tab(ix, window, cx);
        self.tab = ix;
        cx.notify();
    }

    fn ensure_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host.clone();
        match ix {
            1 if self.proxies.is_none() => {
                self.proxies = Some(cx.new(|cx| ProxiesPage::new(host, window, cx)));
            }
            2 if self.connections.is_none() => {
                self.connections = Some(cx.new(|cx| ConnectionsPage::new(host, window, cx)));
            }
            3 if self.profiles.is_none() => {
                self.profiles = Some(cx.new(|cx| ProfilesPage::new(host, window, cx)));
            }
            4 if self.templates.is_none() => {
                self.templates = Some(cx.new(|cx| TemplatesPage::new(host, window, cx)));
            }
            5 if self.logs.is_none() => {
                self.logs = Some(cx.new(|cx| LogsPage::new(host, window, cx)));
            }
            6 if self.settings.is_none() => {
                self.settings = Some(cx.new(|cx| SettingsPage::new(host, window, cx)));
            }
            _ => {}
        }
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab.min(NAV_KEYS.len() - 1);
        let rail_w = px(RAIL_WIDE);
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .id("shell-root")
            .relative()
            .size_full()
            .child(
                h_flex()
                    .id("shell")
                    .size_full()
                    .bg(cx.theme().background)
                    .child(self.render_rail(tab, rail_w, cx))
                    .child(
                        div()
                            .id(("page", tab))
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .min_h_0()
                            .when(tab == 0, |d| d.child(self.home.clone()))
                            .when_some(self.proxies.clone().filter(|_| tab == 1), |d, p| d.child(p))
                            .when_some(self.connections.clone().filter(|_| tab == 2), |d, p| {
                                d.child(p)
                            })
                            .when_some(self.profiles.clone().filter(|_| tab == 3), |d, p| {
                                d.child(p)
                            })
                            .when_some(self.templates.clone().filter(|_| tab == 4), |d, p| {
                                d.child(p)
                            })
                            .when_some(self.logs.clone().filter(|_| tab == 5), |d, p| d.child(p))
                            .when_some(self.settings.clone().filter(|_| tab == 6), |d, p| {
                                d.child(p)
                            }),
                    ),
            )
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

impl AppShell {
    fn render_rail(
        &self,
        tab: usize,
        rail_w: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let sidebar = cx.theme().sidebar;
        let mut items = v_flex().id("nav-items").w_full().gap_1().px_2();
        for (ix, (key, icon)) in NAV_KEYS.iter().zip(NAV_ICONS).enumerate() {
            let label = tr(key);
            items = items.child(nav_item(ix, label, icon, ix == tab, cx));
        }

        v_flex()
            .id("nav-rail")
            .w(rail_w)
            .min_w(rail_w)
            .max_w(rail_w)
            .h_full()
            .flex_shrink_0()
            .bg(sidebar)
            .pt_4()
            .pb_3()
            .child(brand_row(cx))
            .child(div().h_5())
            .child(items.flex_1())
    }
}

fn brand_row(cx: &App) -> impl IntoElement {
    h_flex()
        .id("nav-brand")
        .w_full()
        .px_3()
        .gap_3()
        .items_center()
        .child(
            div()
                .size(px(28.))
                .rounded(px(8.))
                .bg(cx.theme().primary)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::empty()
                        .path("icons/zap.svg")
                        .text_color(cx.theme().primary_foreground)
                        .with_size(px(16.)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0()
                .min_w_0()
                .child(
                    div()
                        .text_xl()
                        .font_bold()
                        .text_color(cx.theme().foreground)
                        .child("SingPanel"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
                ),
        )
}

fn nav_item(
    ix: usize,
    label: &'static str,
    icon: &'static str,
    selected: bool,
    cx: &mut Context<AppShell>,
) -> impl IntoElement {
    let accent = cx.theme().sidebar_accent;
    let hover = cx.theme().muted;
    let fg = if selected {
        cx.theme().sidebar_accent_foreground
    } else {
        cx.theme().muted_foreground
    };

    div()
        .id(("nav-item", ix))
        .w_full()
        .rounded(px(14.))
        .cursor_pointer()
        .px_3()
        .py_2()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .when(selected, |d| d.bg(accent))
        .when(!selected, |d| d.hover(move |s| s.bg(hover)))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.select_tab(ix, window, cx);
        }))
        .child(
            Icon::empty()
                .path(icon)
                .text_color(fg)
                .with_size(px(18.)),
        )
        .child(
            div()
                .text_sm()
                .text_color(fg)
                .when(selected, |d| d.font_bold())
                .child(label),
        )
}
