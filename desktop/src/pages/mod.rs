//! One tab = one file. Subagents must only edit their own `pages/*.rs`.
//!
//! Shared (do not edit from a tab task):
//! - `crate::host::HostClient` — start/stop/status/check/fetch/convert/assemble/clash
//! - `crate::store` — AppData settings / profiles / templates
//! - `crate::net` — LAN IPv4
//! - `crate::net_detect` — exit-IP / 网页检测
//! - `crate::widgets` — page chrome
//!
//! `new(host, window, cx)` may create `InputState` entities. I/O goes through
//! `cx.background_spawn`, then `this.update` + `cx.notify()`. Do not spawn a
//! second host process.

pub mod connections;
pub mod home;
pub mod logs;
pub mod profiles;
pub mod proxies;
pub mod settings;
pub mod templates;

pub use connections::ConnectionsPage;
pub use home::HomePage;
pub use logs::LogsPage;
pub use profiles::ProfilesPage;
pub use proxies::ProxiesPage;
pub use settings::SettingsPage;
pub use templates::TemplatesPage;
