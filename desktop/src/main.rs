#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod assets;
mod autostart;
mod core_download;
mod host;
mod i18n;
mod net;
mod net_detect;
mod win_helper;
mod pages;
mod store;
mod runtime;
mod single_instance;
mod sysproxy;
mod tailscale;
mod tun_auth;
mod theme;
mod tray;
mod widgets;

use gpui::*;
use gpui_component::*;

use crate::app::AppShell;

fn main() {
    if let Err(_) = crate::single_instance::acquire() {
        crate::single_instance::focus_existing();
        return;
    }
    std::panic::set_hook(Box::new(|info| {
        let path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("singpanel-gpui-panic.txt")))
            .unwrap_or_else(|| std::path::PathBuf::from("singpanel-gpui-panic.txt"));
        let _ = std::fs::write(&path, format!("{info}\n{info:?}"));
        eprintln!("{info}");
    }));
    gpui_platform::application()
        .with_assets(crate::assets::Assets)
        .run(move |cx| {
        gpui_component::init(cx);
        crate::theme::install(cx);
        crate::tray::apply_dock_icon();
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() && !crate::tray::is_enabled() {
                cx.quit();
            }
        })
        .detach();
        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(20.), px(20.)),
                        size: size(px(1400.), px(900.)),
                    })),
                    window_min_size: Some(size(px(960.), px(640.))),
                    app_id: Some("singpanel".into()),
                    ..WindowOptions::default()
                },
                |window, cx| {
                    window.set_window_title("SingPanel");
                    crate::theme::apply_saved(window, cx);
                    let view = cx.new(|cx| AppShell::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
                },
            )
            .expect("open window");
        })
        .detach();
    });
}
