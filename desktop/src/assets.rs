//! Lucide icons for the rail, plus the default gpui-component set.
//!
//! Nav SVGs live in `desktop/assets/icons/` (Lucide ISC). Other `IconName`
//! paths fall through to `gpui-component-assets`.

use std::borrow::Cow;

use anyhow::anyhow;
use gpui::{AssetSource, Result, SharedString};

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
struct LocalIcons;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        if let Some(file) = LocalIcons::get(path) {
            return Ok(Some(file.data));
        }
        match gpui_component_assets::Assets.load(path) {
            Ok(data) => Ok(data),
            Err(_) => Err(anyhow!("could not find asset at path \"{path}\"")),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out = gpui_component_assets::Assets.list(path).unwrap_or_default();
        out.extend(
            LocalIcons::iter().filter_map(|p| p.starts_with(path).then(|| SharedString::from(p))),
        );
        out.sort();
        out.dedup();
        Ok(out)
    }
}
