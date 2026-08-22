//! Dock icon + macOS menu-bar extra.
//!
//! The debug binary is not an `.app`, so the Dock would otherwise show the
//! generic `exec` glyph. We set `NSApplication.applicationIconImage` at
//! startup. The menu-bar item uses `desktop/assets/tray.png`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::sync::OnceLock;

use crate::store::{load_settings, settings_bool};
#[cfg(target_os = "macos")]
use crate::i18n::tr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    ToggleCore,
    Quit,
}

const TAG_SHOW: isize = 1;
const TAG_TOGGLE: isize = 2;
const TAG_QUIT: isize = 3;

static ENABLED: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);
static TX: Mutex<Option<Sender<TrayAction>>> = Mutex::new(None);
static RX: Mutex<Option<Receiver<TrayAction>>> = Mutex::new(None);

pub fn apply_dock_icon() {
    platform::apply_dock_icon();
}

pub fn set_enabled(on: bool) {
    ensure_channel();
    if on {
        platform::show_item(RUNNING.load(Ordering::Relaxed));
        ENABLED.store(true, Ordering::Relaxed);
    } else {
        platform::hide_item();
        ENABLED.store(false, Ordering::Relaxed);
    }
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_running(running: bool) {
    update_running(running);
}

pub fn update_running(running: bool) {
    if !should_refresh_menu(
        RUNNING.swap(running, Ordering::Relaxed),
        running,
        ENABLED.load(Ordering::Relaxed),
    ) {
        return;
    }
    platform::update_menu(running);
}

fn should_refresh_menu(prev: bool, next: bool, enabled: bool) -> bool {
    enabled && prev != next
}

pub fn close_hides() -> bool {
    is_enabled() && settings_bool(&load_settings(), "closeToTray", true)
}

pub fn poll() -> Option<TrayAction> {
    poll_action()
}

pub fn poll_action() -> Option<TrayAction> {
    let rx = RX.lock().ok()?;
    let rx = rx.as_ref()?;
    match rx.try_recv() {
        Ok(action) => Some(action),
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => None,
    }
}

fn ensure_channel() {
    let mut tx = TX.lock().unwrap();
    if tx.is_none() {
        let (sender, receiver) = mpsc::channel();
        *tx = Some(sender);
        *RX.lock().unwrap() = Some(receiver);
    }
}

fn emit(action: TrayAction) {
    if let Ok(guard) = TX.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(action);
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use cocoa::appkit::{
        NSImage, NSMenu, NSMenuItem, NSStatusBar, NSVariableStatusItemLength,
    };
    use cocoa::base::{id, nil, NO, YES};
    use cocoa::foundation::{NSAutoreleasePool, NSData, NSString};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    struct NativeTray {
        item: id,
        target: id,
    }

    unsafe impl Send for NativeTray {}
    unsafe impl Sync for NativeTray {}

    static TRAY: Mutex<Option<NativeTray>> = Mutex::new(None);
    static TARGET_CLASS: OnceLock<&'static Class> = OnceLock::new();

    pub fn apply_dock_icon() {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            const PNG: &[u8] = include_bytes!("../assets/app_icon.png");
            let data = NSData::dataWithBytes_length_(
                nil,
                PNG.as_ptr() as *const _,
                PNG.len() as u64,
            );
            let img: id = NSImage::initWithData_(NSImage::alloc(nil), data);
            if img != nil {
                let app = cocoa::appkit::NSApp();
                let _: () = msg_send![app, setApplicationIconImage: img];
            }
        }
    }

    pub fn show_item(running: bool) {
        let mut g = TRAY.lock().unwrap();
        if g.is_some() {
            update_menu_locked(&mut g, running);
            return;
        }
        unsafe {
            // statusItemWithLength: is autoreleased. Retain before the pool
            // drains or later setMenu: hits a freed pointer (macOS 26 crash:
            // -[_CUIThemeGradientRendition setMenu:]).
            let bar = NSStatusBar::systemStatusBar(nil);
            let item: id = msg_send![bar, statusItemWithLength: NSVariableStatusItemLength];
            if item == nil {
                return;
            }
            let item: id = msg_send![item, retain];
            let button: id = msg_send![item, button];
            let img = load_tray_image();
            if img != nil && button != nil {
                let _: () = msg_send![button, setImage: img];
                let _: () = msg_send![img, setTemplate: YES];
            }
            let target_cls = target_class();
            let target: id = msg_send![target_cls, alloc];
            let target: id = msg_send![target, init];
            let menu = build_menu(target, running);
            if responds_to_set_menu(item) {
                let _: () = msg_send![item, setMenu: menu];
            }
            *g = Some(NativeTray { item, target });
        }
    }

    pub fn hide_item() {
        let mut g = TRAY.lock().unwrap();
        if let Some(native) = g.take() {
            unsafe {
                let bar = NSStatusBar::systemStatusBar(nil);
                if responds_to_set_menu(native.item) {
                    let _: () = msg_send![native.item, setMenu: nil];
                }
                bar.removeStatusItem_(native.item);
                let _: () = msg_send![native.item, release];
                let _: () = msg_send![native.target, release];
            }
        }
    }

    pub fn update_menu(running: bool) {
        let mut g = TRAY.lock().unwrap();
        update_menu_locked(&mut g, running);
    }

    fn update_menu_locked(g: &mut Option<NativeTray>, running: bool) {
        let Some(native) = g.as_ref() else {
            return;
        };
        unsafe {
            if !responds_to_set_menu(native.item) {
                return;
            }
            let menu = build_menu(native.target, running);
            let _: () = msg_send![native.item, setMenu: menu];
        }
    }

    unsafe fn responds_to_set_menu(item: id) -> bool {
        if item == nil {
            return false;
        }
        let ok: bool = msg_send![item, respondsToSelector: sel!(setMenu:)];
        ok
    }

    unsafe fn load_tray_image() -> id {
        const PNG: &[u8] = include_bytes!("../assets/tray.png");
        let data = NSData::dataWithBytes_length_(
            nil,
            PNG.as_ptr() as *const _,
            PNG.len() as u64,
        );
        let img: id = NSImage::initWithData_(NSImage::alloc(nil), data);
        img
    }

    unsafe fn build_menu(target: id, running: bool) -> id {
        let menu = NSMenu::new(nil);
        let _: () = msg_send![menu, setAutoenablesItems: NO];

        add_item(menu, target, "SingPanel", 0, false);
        let _: () = msg_send![menu, addItem: NSMenuItem::separatorItem(nil)];
        add_item(
            menu,
            target,
            if running { tr("tray.stop_core") } else { tr("tray.start_core") },
            TAG_TOGGLE,
            true,
        );
        let _: () = msg_send![menu, addItem: NSMenuItem::separatorItem(nil)];
        add_item(menu, target, tr("tray.show_window"), TAG_SHOW, true);
        add_item(menu, target, tr("tray.quit"), TAG_QUIT, true);
        menu
    }

    unsafe fn add_item(menu: id, target: id, title: &str, tag: isize, enabled: bool) {
        let title = NSString::alloc(nil).init_str(title);
        let key = NSString::alloc(nil).init_str("");
        let item: id = msg_send![class!(NSMenuItem), alloc];
        let item: id = msg_send![item, initWithTitle:title action:sel!(onMenu:) keyEquivalent:key];
        if tag != 0 {
            let _: () = msg_send![item, setTarget: target];
            let _: () = msg_send![item, setTag: tag];
        }
        let _: () = msg_send![item, setEnabled: if enabled { YES } else { NO }];
        menu.addItem_(item);
    }

    fn target_class() -> &'static Class {
        *TARGET_CLASS.get_or_init(|| {
            let mut decl = ClassDecl::new("SingPanelTrayTarget", class!(NSObject))
                .expect("register SingPanelTrayTarget");
            unsafe {
                decl.add_method(
                    sel!(onMenu:),
                    on_menu as extern "C" fn(&Object, Sel, id),
                );
            }
            decl.register()
        })
    }

    extern "C" fn on_menu(_this: &Object, _cmd: Sel, sender: id) {
        let tag: isize = unsafe { msg_send![sender, tag] };
        let action = match tag {
            TAG_SHOW => TrayAction::Show,
            TAG_TOGGLE => TrayAction::ToggleCore,
            TAG_QUIT => TrayAction::Quit,
            _ => return,
        };
        emit(action);
    }
}

#[cfg(test)]
mod tests {
    use super::should_refresh_menu;

    #[test]
    fn menu_refresh_only_when_enabled_and_state_changes() {
        assert!(!should_refresh_menu(false, false, true));
        assert!(!should_refresh_menu(true, true, true));
        assert!(should_refresh_menu(false, true, true));
        assert!(should_refresh_menu(true, false, true));
        assert!(!should_refresh_menu(false, true, false));
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn apply_dock_icon() {}
    pub fn show_item(_running: bool) {}
    pub fn hide_item() {}
    pub fn update_menu(_running: bool) {}
}
