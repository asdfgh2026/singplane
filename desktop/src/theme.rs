//! Theme colors from seed `#047857`.
//! Replaces the default shadcn neutral (black/white primary).

use gpui::*;
use gpui_component::{Theme, ThemeMode, ThemeRegistry};

const APP_THEME: &str = include_str!("../themes/singpanel.json");

const LIGHT_NAME: &str = "SingPanel Light";
const DARK_NAME: &str = "SingPanel Dark";

pub fn install(cx: &mut App) {
    if let Err(err) = ThemeRegistry::global_mut(cx).load_themes_from_str(APP_THEME) {
        eprintln!("theme: {err}");
        return;
    }

    let (light, dark) = {
        let registry = ThemeRegistry::global(cx);
        (
            registry.themes().get(LIGHT_NAME).cloned(),
            registry.themes().get(DARK_NAME).cloned(),
        )
    };

    let theme = Theme::global_mut(cx);
    if let Some(light) = light {
        theme.light_theme = light;
    }
    if let Some(dark) = dark {
        theme.dark_theme = dark;
    }
}

pub fn apply_saved(window: &mut Window, cx: &mut App) {
    match crate::store::load_theme_mode().as_str() {
        "dark" => Theme::change(ThemeMode::Dark, Some(window), cx),
        "light" => Theme::change(ThemeMode::Light, Some(window), cx),
        _ => Theme::sync_system_appearance(Some(window), cx),
    }
}
