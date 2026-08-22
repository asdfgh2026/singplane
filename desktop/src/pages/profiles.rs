use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::*;
use gpui_component::input::{Input, InputState};
use gpui_component::*;
use serde_json::{json, Value};

use crate::host::{HostClient, HostResult};
use crate::runtime::{activate_profile, apply_profile_switch, prepare_runtime};
use crate::store::{
    default_assemble_options, delete_profile, load_profiles, load_settings, new_id, patch_from_settings,
    patch_settings, save_profile, settings_bool, settings_str, template_by_id,
};
use crate::i18n::tr;
use crate::widgets::{card, card_selected, chip, muted, page_scroll, page_title, section_header};

#[derive(Clone, Copy, PartialEq, Eq)]
enum AddStep {
    Menu,
    Qr,
    Url,
    File,
}

/// Tab: 配置. List / import / refresh profiles.
pub struct ProfilesPage {
    host: Arc<HostClient>,
    url_input: Entity<InputState>,
    path_input: Entity<InputState>,
    profiles: Vec<Value>,
    active_id: Option<String>,
    pending_delete: Option<String>,
    updating: Vec<String>,
    busy: bool,
    assemble_on_import: bool,
    add_step: AddStep,
    message: SharedString,
}

enum Job {
    Fail {
        message: String,
        done_id: Option<String>,
    },
    Upsert {
        profile: Value,
        activate: bool,
        message: String,
        done_id: Option<String>,
    },
    Merged {
        profiles: Vec<Value>,
        message: String,
        done_ids: Vec<String>,
    },
    Deleted {
        id: String,
        next_id: Option<String>,
        message: String,
    },
    Done {
        message: String,
    },
}

impl ProfilesPage {
    pub fn new(host: Arc<HostClient>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = load_settings();
        let active_id = settings
            .get("activeProfileId")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        Self {
            host,
            url_input: cx.new(|cx| InputState::new(window, cx).placeholder("https://...")),
            path_input: cx.new(|cx| InputState::new(window, cx).placeholder(r"C:\path\config.json")),
            profiles: load_profiles(),
            active_id,
            pending_delete: None,
            updating: Vec::new(),
            busy: false,
            assemble_on_import: settings_bool(&settings, "defaultAssembleOnImport", false),
            add_step: AddStep::Menu,
            message: SharedString::default(),
        }
    }

    fn names(&self) -> Vec<String> {
        self.profiles.iter().map(|p| json_str(p, "name")).collect()
    }

    fn profile_by_id(&self, id: &str) -> Option<&Value> {
        self.profiles.iter().find(|p| json_str(p, "id") == id)
    }

    fn is_active(&self, id: &str, index: usize) -> bool {
        match &self.active_id {
            Some(a) => a == id,
            None => index == 0 && !self.profiles.is_empty(),
        }
    }

    fn is_updating(&self, id: &str) -> bool {
        self.updating.iter().any(|x| x == id)
    }

    fn has_url(&self) -> bool {
        self.profiles.iter().any(|p| {
            json_str(p, "sourceType") == "url" && !json_str(p, "url").is_empty()
        })
    }

    fn await_job(&mut self, task: Task<Job>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let job = task.await;
            this.update_in(cx, |this, window, cx| {
                this.apply_job(job, window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn apply_job(&mut self, job: Job, window: &mut Window, cx: &mut Context<Self>) {
        let close_add = matches!(&job, Job::Upsert { activate: true, .. });
        match job {
            Job::Done { message } => {
                self.busy = false;
                self.message = message.into();
            }
            Job::Fail { message, done_id } => {
                if let Some(id) = done_id {
                    self.updating.retain(|x| x != &id);
                    if self.updating.is_empty() {
                        self.busy = false;
                    }
                } else {
                    self.busy = false;
                }
                self.message = message.into();
            }
            Job::Upsert {
                profile,
                activate,
                message,
                done_id,
            } => {
                let id = json_str(&profile, "id");
                if let Some(slot) = self
                    .profiles
                    .iter_mut()
                    .find(|p| json_str(p, "id") == id)
                {
                    *slot = profile;
                } else {
                    self.profiles.push(profile);
                }
                if activate && !id.is_empty() {
                    self.active_id = Some(id.clone());
                }
                if let Some(did) = done_id {
                    self.updating.retain(|x| x != &did);
                } else {
                    self.updating.retain(|x| x != &id);
                }
                if self.updating.is_empty() {
                    self.busy = false;
                }
                self.message = message.into();
            }
            Job::Merged {
                profiles,
                message,
                done_ids,
            } => {
                for next in profiles {
                    let id = json_str(&next, "id");
                    if let Some(slot) = self
                        .profiles
                        .iter_mut()
                        .find(|p| json_str(p, "id") == id)
                    {
                        *slot = next;
                    }
                }
                for id in done_ids {
                    self.updating.retain(|x| x != &id);
                }
                if self.updating.is_empty() {
                    self.busy = false;
                }
                self.message = message.into();
            }
            Job::Deleted {
                id,
                next_id,
                message,
            } => {
                self.profiles.retain(|p| json_str(p, "id") != id);
                if self.pending_delete.as_deref() == Some(id.as_str()) {
                    self.pending_delete = None;
                }
                if self.active_id.as_deref() == Some(id.as_str()) {
                    self.active_id = next_id;
                }
                self.busy = false;
                self.message = message.into();
            }
        }
        if close_add {
            self.add_step = AddStep::Menu;
            window.close_sheet(cx);
        }
    }

    fn open_add_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_sheet(cx) {
            window.close_sheet(cx);
        }
        let step = self.add_step;
        let url_input = self.url_input.clone();
        let path_input = self.path_input.clone();
        let busy = self.busy;
        let assemble_hint = if self.assemble_on_import {
            "导入订阅时会按设置套用默认运行模板（失败仍保存原文）"
        } else {
            "导入订阅默认原样保存"
        };
        let entity = cx.entity().downgrade();
        window.open_sheet(cx, move |sheet, _, cx| {
            let body = match step {
                AddStep::Menu => add_menu(entity.clone(), cx),
                AddStep::Qr => add_url_form(
                    entity.clone(),
                    &url_input,
                    busy,
                    "把二维码里的订阅链接贴到下面，或先复制到剪贴板再点识别。",
                    "识别并导入",
                    true,
                    cx,
                ),
                AddStep::Url => add_url_form(
                    entity.clone(),
                    &url_input,
                    busy,
                    assemble_hint,
                    if busy { "导入中" } else { "导入" },
                    false,
                    cx,
                ),
                AddStep::File => add_file_form(entity.clone(), &path_input, busy, cx),
            };
            sheet
                .size(px(360.))
                .overlay(true)
                .overlay_closable(true)
                .title(match step {
                    AddStep::Menu => "添加配置",
                    AddStep::Qr => "二维码",
                    AddStep::Url => "URL",
                    AddStep::File => "文件",
                })
                .child(body)
        });
    }

    fn set_add_step(&mut self, step: AddStep, window: &mut Window, cx: &mut Context<Self>) {
        self.add_step = step;
        self.open_add_sheet(window, cx);
        cx.notify();
    }

    fn import_url(&mut self, cx: &mut Context<Self>) {
        let url = self.url_input.read(cx).value().to_string();
        let url = url.trim().to_string();
        if url.is_empty() {
            self.message = "请填写 URL".into();
            cx.notify();
            return;
        }
        let names = self.names();
        let host = self.host.clone();
        self.busy = true;
        self.message = "正在下载订阅…".into();
        cx.notify();
        let task = cx.background_spawn(async move { import_url_job(host, url, names) });
        self.await_job(task, cx);
    }

    fn import_file(&mut self, cx: &mut Context<Self>) {
        let path = self.path_input.read(cx).value().to_string();
        let path = path.trim().trim_matches('"').trim().to_string();
        if path.is_empty() {
            self.message = "请填写本地文件路径".into();
            cx.notify();
            return;
        }
        let names = self.names();
        self.busy = true;
        self.message = "正在读取文件…".into();
        cx.notify();
        let task = cx.background_spawn(async move { import_file_job(path, names) });
        self.await_job(task, cx);
    }

    fn import_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            self.message = "剪贴板是空的".into();
            cx.notify();
            return;
        };
        if let Some(path) = clipboard_file_path(&item) {
            self.path_input.update(cx, |s, cx| {
                s.set_value(path.clone(), window, cx);
            });
            self.import_file(cx);
            return;
        }
        let Some(text) = item.text() else {
            self.message = "剪贴板里没有文字或文件".into();
            cx.notify();
            return;
        };
        self.import_clipboard_text(&text, window, cx);
    }

    fn import_clipboard_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.trim();
        if text.is_empty() {
            self.message = "剪贴板是空的".into();
            cx.notify();
            return;
        }
        if let Some(url) = extract_http_url(text) {
            self.url_input.update(cx, |s, cx| {
                s.set_value(url.clone(), window, cx);
            });
            self.import_url(cx);
            return;
        }
        if looks_like_config(text) {
            let names = self.names();
            let body = text.to_string();
            self.busy = true;
            self.message = "正在从剪贴板导入配置…".into();
            cx.notify();
            let task = cx.background_spawn(async move { import_body_job(body, names) });
            self.await_job(task, cx);
            return;
        }
        self.message = "剪贴板不是订阅链接或配置内容".into();
        cx.notify();
    }

    fn browse_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match pick_local_file() {
            Some(path) => {
                self.path_input.update(cx, |s, cx| {
                    s.set_value(path.clone(), window, cx);
                });
                self.import_file(cx);
            }
            None => {
                self.message = "未选择文件".into();
                cx.notify();
            }
        }
    }

    fn set_active(&mut self, id: String, cx: &mut Context<Self>) {
        if self.profile_by_id(&id).is_none() {
            self.message = "找不到该配置".into();
            cx.notify();
            return;
        }
        self.active_id = Some(id.clone());
        self.message = "正在设为当前…".into();
        cx.notify();
        let host = self.host.clone();
        let task = cx.background_spawn(async move { set_active_job(host, id) });
        self.await_job(task, cx);
    }

    fn refresh_one(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(profile) = self.profile_by_id(&id).cloned() else {
            self.message = "找不到该配置".into();
            cx.notify();
            return;
        };
        if json_str(&profile, "sourceType") != "url" || json_str(&profile, "url").is_empty() {
            self.message = "仅 URL 订阅可更新".into();
            cx.notify();
            return;
        }
        if self.is_updating(&id) {
            return;
        }
        let host = self.host.clone();
        let active = self.active_id.clone();
        self.updating.push(id.clone());
        self.message = "正在更新订阅…".into();
        cx.notify();
        let task = cx.background_spawn(async move { refresh_one_job(host, profile, active) });
        self.await_job(task, cx);
    }

    fn refresh_all(&mut self, cx: &mut Context<Self>) {
        let jobs: Vec<Value> = self
            .profiles
            .iter()
            .filter(|p| json_str(p, "sourceType") == "url" && !json_str(p, "url").is_empty())
            .cloned()
            .collect();
        if jobs.is_empty() {
            self.message = "没有可更新的订阅".into();
            cx.notify();
            return;
        }
        let ids: Vec<String> = jobs.iter().map(|p| json_str(p, "id")).collect();
        let host = self.host.clone();
        let active = self.active_id.clone();
        self.busy = true;
        self.updating.extend(ids.iter().cloned());
        self.message = "正在更新全部订阅…".into();
        cx.notify();
        let task = cx.background_spawn(async move { refresh_all_job(host, jobs, active) });
        self.await_job(task, cx);
    }

    fn request_delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.pending_delete = Some(id);
        self.message = "再点「确认删除」以删除该配置".into();
        cx.notify();
    }

    fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.pending_delete = None;
        self.message = "已取消删除".into();
        cx.notify();
    }

    fn confirm_delete(&mut self, id: String, cx: &mut Context<Self>) {
        if self.pending_delete.as_deref() != Some(id.as_str()) {
            self.pending_delete = Some(id);
            cx.notify();
            return;
        }
        let implicit_first = self.active_id.is_none()
            && self
                .profiles
                .first()
                .is_some_and(|p| json_str(p, "id") == id);
        let was_active = self.active_id.as_deref() == Some(id.as_str()) || implicit_first;
        let next = if was_active {
            self.profiles.iter().find(|p| json_str(p, "id") != id)
        } else {
            None
        };
        let next_id = next.map(|p| json_str(p, "id"));
        self.busy = true;
        self.message = "正在删除…".into();
        cx.notify();
        let task = cx.background_spawn(async move {
            delete_job(id, next_id)
        });
        self.await_job(task, cx);
    }

    fn open_view_sheet(
        &self,
        name: String,
        content: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_sheet(cx) {
            window.close_sheet(cx);
        }
        let pretty = pretty_profile_content(&content);
        let empty = pretty.is_empty();
        let body = if empty {
            tr("profiles.view_empty").to_string()
        } else {
            pretty.clone()
        };
        let title = format!("{} · {name}", tr("profiles.view"));
        window.open_sheet(cx, move |sheet, _, cx| {
            let copy = pretty.clone();
            sheet
                .size(px(720.))
                .overlay(true)
                .overlay_closable(true)
                .title(title.clone())
                .child(
                    v_flex()
                        .id("profile-view-scroll")
                        .size_full()
                        .min_h_0()
                        .child(config_view_scroll(&body, cx)),
                )
                .footer(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("profile-view-copy")
                                .label(tr("common.copy"))
                                .disabled(empty)
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                                }),
                        )
                        .child(
                            Button::new("profile-view-close")
                                .label(tr("common.close"))
                                .on_click(|_, window, cx| window.close_sheet(cx)),
                        ),
                )
        });
    }

    fn render_card(
        &self,
        profile: &Value,
        index: usize,
        entity: WeakEntity<Self>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = json_str(profile, "id");
        let name = json_str(profile, "name");
        let source = json_str(profile, "sourceType");
        let url = json_opt_str(profile, "url");
        let path = json_opt_str(profile, "path");
        let last_error = json_opt_str(profile, "lastError");
        let assemble = json_bool(profile, "assembleEnabled", false);
        let runnable = json_bool(profile, "runnable", true);
        let traffic = traffic_label(
            json_i64(profile, "upload"),
            json_i64(profile, "download"),
            json_i64(profile, "total"),
        );
        let expire = expire_label(json_i64(profile, "expireMs"));
        let updated = fmt_updated(&json_str(profile, "updatedAt"));
        let warnings = warnings_brief(profile);
        let active = self.is_active(&id, index);
        let updating = self.is_updating(&id);
        let pending = self.pending_delete.as_deref() == Some(id.as_str());
        let set_e = entity.clone();
        let view_e = entity.clone();
        let refresh_e = entity.clone();
        let del_e = entity.clone();
        let confirm_e = entity.clone();
        let cancel_e = entity;
        let set_id = id.clone();
        let refresh_id = id.clone();
        let del_id = id.clone();
        let confirm_id = id.clone();
        let view_name = name.clone();
        let view_content = json_str(profile, "content");
        let card_id = SharedString::from(format!("profile-{id}"));
        let set_btn = SharedString::from(format!("set-active-{id}"));
        let view_btn = SharedString::from(format!("view-{id}"));
        let refresh_btn = SharedString::from(format!("refresh-{id}"));
        let delete_btn = SharedString::from(format!("delete-{id}"));
        let confirm_btn = SharedString::from(format!("confirm-delete-{id}"));
        let cancel_btn = SharedString::from(format!("cancel-delete-{id}"));

        let mut meta: Vec<String> = Vec::new();
        if !updated.is_empty() {
            meta.push(updated);
        }
        if !traffic.is_empty() {
            meta.push(traffic);
        }
        if !expire.is_empty() {
            meta.push(expire);
        }
        if updating {
            meta.push(tr("profiles.updating").to_string());
        }

        let base = if active { card_selected(cx) } else { card(cx) };
        base
            .id(card_id)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_base()
                            .font_semibold()
                            .child(SharedString::from(name)),
                    )
                    .child(chip(source_label(&source), cx))
                    .when(active, |d| d.child(chip(tr("profiles.badge_current"), cx)))
                    .when(assemble, |d| d.child(chip(tr("profiles.assembled_badge"), cx)))
                    .when(!runnable, |d| d.child(chip(tr("profiles.badge_download_only"), cx))),
            )
            .when(!meta.is_empty(), |d| {
                d.child(muted(meta.join(" · "), cx))
            })
            .when_some(url, |d, u| d.child(muted(ellipsize(&u, 88), cx)))
            .when_some(path, |d, p| d.child(muted(ellipsize(&p, 88), cx)))
            .when_some(last_error, |d, err| {
                d.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(SharedString::from(ellipsize(&err, 160))),
                )
            })
            .when_some(warnings, |d, w| d.child(muted(ellipsize(&w, 120), cx)))
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        Button::new(set_btn)
                            .compact()
                            .primary()
                            .label(if active {
                                tr("profiles.current")
                            } else {
                                tr("profiles.set_current")
                            })
                            .disabled(active || self.busy)
                            .on_click(move |_, _, cx| {
                                if let Some(e) = set_e.upgrade() {
                                    let id = set_id.clone();
                                    e.update(cx, |this, cx| this.set_active(id, cx));
                                }
                            }),
                    )
                    .child(
                        Button::new(view_btn)
                            .compact()
                            .label(tr("profiles.view"))
                            .on_click(move |_, window, cx| {
                                if let Some(e) = view_e.upgrade() {
                                    let name = view_name.clone();
                                    let content = view_content.clone();
                                    e.update(cx, |this, cx| {
                                        this.open_view_sheet(name, content, window, cx)
                                    });
                                }
                            }),
                    )
                    .when(source == "url", |row| {
                        row.child(
                            Button::new(refresh_btn)
                                .compact()
                                .label(if updating {
                                    tr("profiles.updating")
                                } else {
                                    tr("profiles.update_sub")
                                })
                                .disabled(updating || self.busy)
                                .on_click(move |_, _, cx| {
                                    if let Some(e) = refresh_e.upgrade() {
                                        let id = refresh_id.clone();
                                        e.update(cx, |this, cx| this.refresh_one(id, cx));
                                    }
                                }),
                        )
                    })
                    .when(!pending, |row| {
                        row.child(
                            Button::new(delete_btn)
                                .compact()
                                .label(tr("common.delete"))
                                .disabled(self.busy)
                                .on_click(move |_, _, cx| {
                                    if let Some(e) = del_e.upgrade() {
                                        let id = del_id.clone();
                                        e.update(cx, |this, cx| this.request_delete(id, cx));
                                    }
                                }),
                        )
                    })
                    .when(pending, |row| {
                        row.child(
                            Button::new(confirm_btn)
                                .compact()
                                .danger()
                                .label(tr("profiles.confirm_delete"))
                                .disabled(self.busy)
                                .on_click(move |_, _, cx| {
                                    if let Some(e) = confirm_e.upgrade() {
                                        let id = confirm_id.clone();
                                        e.update(cx, |this, cx| this.confirm_delete(id, cx));
                                    }
                                }),
                        )
                        .child(
                            Button::new(cancel_btn)
                                .compact()
                                .label(tr("common.cancel"))
                                .disabled(self.busy)
                                .on_click(move |_, _, cx| {
                                    if let Some(e) = cancel_e.upgrade() {
                                        e.update(cx, |this, cx| this.cancel_delete(cx));
                                    }
                                }),
                        )
                    }),
            )
    }
}

impl Render for ProfilesPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let add_e = entity.clone();
        let all_e = entity.clone();
        let empty = self.profiles.is_empty();
        let has_url = self.has_url();

        let mut page = page_scroll("page-profiles")
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(page_title(tr("nav.profiles"), cx))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("add-profile")
                                    .small()
                                    .primary()
                                    .icon(IconName::Plus)
                                    .label(tr("profiles.import_btn"))
                                    .disabled(self.busy)
                                    .on_click(move |_, window, cx| {
                                        if let Some(e) = add_e.upgrade() {
                                            e.update(cx, |this, cx| {
                                                this.add_step = AddStep::Menu;
                                                this.open_add_sheet(window, cx);
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("refresh-all")
                                    .small()
                                    .label(tr("profiles.update_all"))
                                    .disabled(self.busy || !has_url)
                                    .on_click(move |_, _, cx| {
                                        if let Some(e) = all_e.upgrade() {
                                            e.update(cx, |this, cx| this.refresh_all(cx));
                                        }
                                    }),
                            ),
                    ),
            )
            .child(muted(self.message.clone(), cx))
            .child(section_header(tr("profiles.title")));

        if empty {
            let empty_e = entity.clone();
            page = page.child(
                card(cx)
                    .child(div().font_semibold().child(tr("profiles.empty")))
                    .child(muted(tr("profiles.empty_hint"), cx))
                    .child(
                        Button::new("add-profile-empty")
                            .primary()
                            .label(tr("profiles.import_btn"))
                            .on_click(move |_, window, cx| {
                                if let Some(e) = empty_e.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.add_step = AddStep::Menu;
                                        this.open_add_sheet(window, cx);
                                    });
                                }
                            }),
                    ),
            );
        } else {
            for (index, profile) in self.profiles.iter().enumerate() {
                page = page.child(self.render_card(profile, index, entity.clone(), cx));
            }
        }

        page
    }
}

fn add_menu(entity: WeakEntity<ProfilesPage>, cx: &App) -> Div {
    v_flex()
        .w_full()
        .gap_1()
        .child(add_method_row(
            "add-qr",
            "icons/qr-code.svg",
            "二维码",
            "扫描二维码获取配置文件",
            entity.clone(),
            AddStep::Qr,
            cx,
        ))
        .child(add_method_row(
            "add-clip",
            "icons/clipboard.svg",
            "剪贴板",
            "自动获取剪贴板订阅链接",
            entity.clone(),
            AddStep::Menu,
            cx,
        ))
        .child(add_method_row(
            "add-file",
            "icons/file-up.svg",
            "文件",
            "直接上传配置文件",
            entity.clone(),
            AddStep::File,
            cx,
        ))
        .child(add_method_row(
            "add-url",
            "icons/link.svg",
            "URL",
            "通过 URL 获取配置文件",
            entity,
            AddStep::Url,
            cx,
        ))
}

fn add_method_row(
    id: &'static str,
    icon: &'static str,
    title: &'static str,
    hint: &'static str,
    entity: WeakEntity<ProfilesPage>,
    step: AddStep,
    cx: &App,
) -> impl IntoElement {
    let hover = cx.theme().muted;
    let fg = cx.theme().foreground;
    let muted_fg = cx.theme().muted_foreground;
    let clip = id == "add-clip";
    h_flex()
        .id(id)
        .w_full()
        .gap_3()
        .px_3()
        .py_3()
        .rounded(px(14.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .on_click(move |_, window, cx| {
            if let Some(view) = entity.upgrade() {
                view.update(cx, |this, cx| {
                    if clip {
                        this.import_clipboard(window, cx);
                    } else {
                        this.set_add_step(step, window, cx);
                    }
                });
            }
        })
        .child(
            Icon::empty()
                .path(icon)
                .text_color(fg)
                .with_size(px(22.)),
        )
        .child(
            v_flex()
                .gap_1()
                .min_w_0()
                .child(div().text_sm().font_semibold().text_color(fg).child(title))
                .child(div().text_xs().text_color(muted_fg).child(hint)),
        )
}

fn add_url_form(
    entity: WeakEntity<ProfilesPage>,
    url_input: &Entity<InputState>,
    busy: bool,
    hint: &str,
    action: &str,
    from_qr: bool,
    _cx: &App,
) -> Div {
    let back_e = entity.clone();
    let go_e = entity.clone();
    v_flex()
        .w_full()
        .gap_3()
        .child(muted_static(hint))
        .child(Input::new(url_input).w_full())
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("add-url-go")
                        .primary()
                        .label(action.to_string())
                        .disabled(busy)
                        .on_click(move |_, window, cx| {
                            if let Some(view) = go_e.upgrade() {
                                view.update(cx, |this, cx| {
                                    if from_qr {
                                        if this.url_input.read(cx).value().trim().is_empty() {
                                            this.import_clipboard(window, cx);
                                        } else {
                                            this.import_url(cx);
                                        }
                                    } else {
                                        this.import_url(cx);
                                    }
                                });
                            }
                        }),
                )
                .child(
                    Button::new("add-back")
                        .label("返回")
                        .on_click(move |_, window, cx| {
                            if let Some(view) = back_e.upgrade() {
                                view.update(cx, |this, cx| {
                                    this.set_add_step(AddStep::Menu, window, cx);
                                });
                            }
                        }),
                ),
        )
}

fn add_file_form(
    entity: WeakEntity<ProfilesPage>,
    path_input: &Entity<InputState>,
    busy: bool,
    _cx: &App,
) -> Div {
    let back_e = entity.clone();
    let pick_e = entity.clone();
    let go_e = entity;
    v_flex()
        .w_full()
        .gap_3()
        .child(muted_static("选择本机 JSON / YAML / TXT，按原文保存。"))
        .child(Input::new(path_input).w_full())
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .child(
                    Button::new("add-file-pick")
                        .primary()
                        .label("选择文件")
                        .disabled(busy)
                        .on_click(move |_, window, cx| {
                            if let Some(view) = pick_e.upgrade() {
                                view.update(cx, |this, cx| this.browse_file(window, cx));
                            }
                        }),
                )
                .child(
                    Button::new("add-file-go")
                        .label(if busy { "读取中" } else { "读取路径" })
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            if let Some(view) = go_e.upgrade() {
                                view.update(cx, |this, cx| this.import_file(cx));
                            }
                        }),
                )
                .child(
                    Button::new("add-file-back")
                        .label("返回")
                        .on_click(move |_, window, cx| {
                            if let Some(view) = back_e.upgrade() {
                                view.update(cx, |this, cx| {
                                    this.set_add_step(AddStep::Menu, window, cx);
                                });
                            }
                        }),
                ),
        )
}

fn muted_static(text: impl Into<SharedString>) -> Div {
    div().text_sm().text_color(rgb(0x6b7280)).child(text.into())
}

fn import_body_job(body: String, names: Vec<String>) -> Job {
    let name = unique_name("剪贴板", &names);
    let profile = new_profile(&name, "local", None, None, &body, 0, 0, 0, 0);
    let note = if detect_runnable(&body) {
        "已从剪贴板导入（可运行）".to_string()
    } else {
        "已从剪贴板导入".to_string()
    };
    if let Err(e) = save_profile(&profile) {
        return Job::Fail {
            message: format!("保存失败: {e}"),
            done_id: None,
        };
    }
    let id = json_str(&profile, "id");
    if let Err(e) = activate_profile(&id) {
        return Job::Upsert {
            profile,
            activate: true,
            message: format!("{note}（设为当前失败: {e}）"),
            done_id: None,
        };
    }
    Job::Upsert {
        profile,
        activate: true,
        message: note,
        done_id: None,
    }
}

fn extract_http_url(text: &str) -> Option<String> {
    for raw in text.split_whitespace() {
        let t = raw.trim_matches(|c: char| "<>\"'(),;".contains(c));
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Some(t.to_string());
        }
    }
    None
}

fn looks_like_config(text: &str) -> bool {
    let kind = detect_content_kind(text);
    kind != "unknown"
}

fn clipboard_file_path(item: &ClipboardItem) -> Option<String> {
    for entry in &item.entries {
        if let ClipboardEntry::ExternalPaths(paths) = entry {
            return paths.0.first().map(|p| p.display().to_string());
        }
    }
    None
}

fn pick_local_file() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("osascript")
            .args([
                "-e",
                "POSIX path of (choose file with prompt \"选择配置文件\")",
            ])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }
    #[cfg(target_os = "windows")]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$d = New-Object System.Windows.Forms.OpenFileDialog
$d.Filter = 'Config|*.json;*.yaml;*.yml;*.txt|All|*.*'
if ($d.ShowDialog() -eq 'OK') { $d.FileName }
"#;
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

fn import_url_job(host: Arc<HostClient>, url: String, names: Vec<String>) -> Job {
    let settings = load_settings();
    let assemble = settings_bool(&settings, "defaultAssembleOnImport", false);
    let fetched = match fetch_sub(host.as_ref(), &url) {
        Ok(f) => f,
        Err(e) => {
            return Job::Fail {
                message: format!("失败: {e}"),
                done_id: None,
            };
        }
    };
    let name = unique_name(&name_from_url(&url), &names);
    let mut profile = new_profile(
        &name,
        "url",
        None,
        Some(&url),
        &fetched.body,
        fetched.upload,
        fetched.download,
        fetched.total,
        fetched.expire_ms,
    );
    let mut note = if detect_runnable(&fetched.body) {
        "下载成功（完整 sing-box 配置，可运行）".to_string()
    } else {
        "下载成功（已原样保存）".to_string()
    };
    if assemble {
        match assemble_into(host.as_ref(), &settings, &mut profile, &fetched.body, None) {
            Ok(()) => note = "装配成功（模板已应用，可运行）".into(),
            Err(e) => {
                profile["lastError"] = json!(e);
                profile["sourceBody"] = json!(fetched.body);
                note = format!("已保存原文，装配失败: {e}");
            }
        }
    }
    if let Err(e) = save_profile(&profile) {
        return Job::Fail {
            message: format!("保存失败: {e}"),
            done_id: None,
        };
    }
    let id = json_str(&profile, "id");
    if let Err(e) = activate_profile(&id) {
        return Job::Upsert {
            profile,
            activate: true,
            message: format!("{note}（设为当前失败: {e}）"),
            done_id: None,
        };
    }
    Job::Upsert {
        profile,
        activate: true,
        message: note,
        done_id: None,
    }
}

fn import_file_job(path: String, names: Vec<String>) -> Job {
    let body = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            return Job::Fail {
                message: format!("导入失败: {e}"),
                done_id: None,
            };
        }
    };
    let body = body.trim().to_string();
    if body.is_empty() {
        return Job::Fail {
            message: "文件内容为空".into(),
            done_id: None,
        };
    }
    let name = unique_name(&name_from_path(&path), &names);
    let profile = new_profile(&name, "local", Some(&path), None, &body, 0, 0, 0, 0);
    let note = if detect_runnable(&body) {
        "已导入（原样保存，可运行）".to_string()
    } else {
        "已导入（原样保存）".to_string()
    };
    if let Err(e) = save_profile(&profile) {
        return Job::Fail {
            message: format!("保存失败: {e}"),
            done_id: None,
        };
    }
    let id = json_str(&profile, "id");
    if let Err(e) = activate_profile(&id) {
        return Job::Upsert {
            profile,
            activate: true,
            message: format!("{note}（设为当前失败: {e}）"),
            done_id: None,
        };
    }
    Job::Upsert {
        profile,
        activate: true,
        message: note,
        done_id: None,
    }
}

fn set_active_job(host: Arc<HostClient>, id: String) -> Job {
    match apply_profile_switch(host.as_ref(), &id) {
        Ok(out) => Job::Done {
            message: out.message,
        },
        Err(e) => Job::Fail {
            message: format!("设为当前失败: {e}"),
            done_id: None,
        },
    }
}

fn refresh_one_job(host: Arc<HostClient>, profile: Value, active: Option<String>) -> Job {
    let id = json_str(&profile, "id");
    let next = refresh_profile(host.as_ref(), profile);
    if let Err(e) = save_profile(&next) {
        return Job::Fail {
            message: format!("保存失败: {e}"),
            done_id: Some(id),
        };
    }
    let name = json_str(&next, "name");
    let err = json_opt_str(&next, "lastError");
    if active.as_deref() == Some(id.as_str()) {
        let _ = prepare_runtime();
    }
    let message = match err {
        Some(e) => format!("更新失败: {e}"),
        None => {
            if json_bool(&next, "assembleEnabled", false) {
                format!("已重新下载并装配 · {name}")
            } else {
                format!("已重新下载 · {name}")
            }
        }
    };
    Job::Upsert {
        profile: next,
        activate: false,
        message,
        done_id: Some(id),
    }
}

fn refresh_all_job(host: Arc<HostClient>, jobs: Vec<Value>, active: Option<String>) -> Job {
    let mut updated = Vec::new();
    let mut lines = Vec::new();
    let mut done_ids = Vec::new();
    for profile in jobs {
        let id = json_str(&profile, "id");
        let name = json_str(&profile, "name");
        let next = refresh_profile(host.as_ref(), profile);
        match save_profile(&next) {
            Ok(()) => {
                if json_opt_str(&next, "lastError").is_some() {
                    lines.push(format!(
                        "✗ {name}: {}",
                        json_str(&next, "lastError")
                    ));
                } else {
                    lines.push(format!("✓ {name}"));
                }
                if active.as_deref() == Some(id.as_str()) {
                    let _ = prepare_runtime();
                }
                updated.push(next);
            }
            Err(e) => lines.push(format!("✗ {name}: 保存失败 {e}")),
        }
        done_ids.push(id);
    }
    Job::Merged {
        profiles: updated,
        message: if lines.is_empty() {
            "没有可更新的订阅".into()
        } else {
            lines.join("  ")
        },
        done_ids,
    }
}

fn delete_job(id: String, next_id: Option<String>) -> Job {
    if let Err(e) = delete_profile(&id) {
        return Job::Fail {
            message: format!("删除失败: {e}"),
            done_id: None,
        };
    }
    match &next_id {
        Some(nid) => {
            let _ = activate_profile(nid);
        }
        None => {
            let _ = patch_settings(&json!({ "activeProfileId": Value::Null }));
        }
    }
    Job::Deleted {
        id,
        next_id,
        message: "已删除".into(),
    }
}

struct Fetched {
    body: String,
    upload: i64,
    download: i64,
    total: i64,
    expire_ms: i64,
}

fn fetch_sub(host: &HostClient, url: &str) -> Result<Fetched, String> {
    let r = host.fetch(url);
    if !r.ok {
        return Err(r.error.unwrap_or_else(|| "订阅下载失败".into()));
    }
    let body = r.str("body").unwrap_or_default();
    if body.trim().is_empty() {
        return Err("订阅内容为空".into());
    }
    Ok(Fetched {
        body,
        upload: host_i64(&r, "upload"),
        download: host_i64(&r, "download"),
        total: host_i64(&r, "total"),
        expire_ms: host_i64(&r, "expireMs"),
    })
}

fn host_i64(r: &HostResult, key: &str) -> i64 {
    r.i64(key)
        .or_else(|| r.data.get(key).and_then(value_i64))
        .unwrap_or(0)
}

fn refresh_profile(host: &HostClient, mut profile: Value) -> Value {
    let url = json_str(&profile, "url");
    let fetched = match fetch_sub(host, &url) {
        Ok(f) => f,
        Err(e) => {
            profile["lastError"] = json!(e);
            return profile;
        }
    };
    profile["upload"] = json!(fetched.upload);
    profile["download"] = json!(fetched.download);
    profile["total"] = json!(fetched.total);
    profile["expireMs"] = json!(fetched.expire_ms);
    profile["updatedAt"] = json!(now_iso8601());
    profile["sourceBody"] = json!(fetched.body);

    if json_bool(&profile, "assembleEnabled", false) {
        let settings = load_settings();
        let template_id = json_opt_str(&profile, "templateId");
        match assemble_into(
            host,
            &settings,
            &mut profile,
            &fetched.body,
            template_id.as_deref(),
        ) {
            Ok(()) => {
                profile["lastError"] = Value::Null;
            }
            Err(e) => {
                profile["lastError"] = json!(e);
            }
        }
        return profile;
    }

    profile["content"] = json!(fetched.body);
    profile["runnable"] = json!(detect_runnable(&fetched.body));
    profile["contentKind"] = json!(detect_content_kind(&fetched.body));
    profile["lastError"] = Value::Null;
    profile
}

fn assemble_into(
    host: &HostClient,
    settings: &Value,
    profile: &mut Value,
    source: &str,
    template_id: Option<&str>,
) -> Result<(), String> {
    let mut tid = template_id
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| settings_str(settings, "defaultTemplateId"));
    if tid.is_empty() {
        tid = "builtin-mixed-direct".into();
    }
    let template = template_by_id(&tid).ok_or_else(|| format!("找不到运行模板: {tid}"))?;
    let template_content = settings_str(&template, "content");
    if template_content.trim().is_empty() {
        return Err(format!("模板内容为空: {tid}"));
    }
    let options = match profile.get("assembleOptions") {
        Some(v) if v.is_object() => v.clone(),
        _ => default_assemble_options(),
    };
    let patch = patch_from_settings(settings);
    let r = host.assemble(source, &template_content, options.clone(), patch, None, true);
    profile["assembleEnabled"] = json!(true);
    profile["templateId"] = json!(tid);
    profile["sourceBody"] = json!(source);
    profile["assembleOptions"] = options;
    profile["updatedAt"] = json!(now_iso8601());

    if !r.ok {
        return Err(r.error.unwrap_or_else(|| "装配失败".into()));
    }
    let config = r
        .data
        .get("config")
        .cloned()
        .ok_or_else(|| "装配失败".to_string())?;
    let content = if let Some(s) = config.as_str() {
        s.to_string()
    } else {
        serde_json::to_string_pretty(&config).unwrap_or_else(|_| config.to_string())
    };
    if content.trim().is_empty() {
        return Err("装配结果为空".into());
    }
    let kind = r
        .data
        .get("detectedKind")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let warnings = r
        .data
        .get("warnings")
        .cloned()
        .unwrap_or_else(|| json!([]));
    profile["content"] = json!(content);
    profile["contentKind"] = json!(kind);
    profile["lastAssembleWarnings"] = warnings;
    profile["lastAssembleAt"] = json!(now_iso8601());
    profile["runnable"] = json!(detect_runnable(&json_str(profile, "content")));
    profile["lastError"] = Value::Null;
    Ok(())
}

fn new_profile(
    name: &str,
    source_type: &str,
    path: Option<&str>,
    url: Option<&str>,
    body: &str,
    upload: i64,
    download: i64,
    total: i64,
    expire_ms: i64,
) -> Value {
    json!({
        "id": new_id(),
        "name": name,
        "sourceType": source_type,
        "path": path,
        "url": url,
        "content": body,
        "updatedAt": now_iso8601(),
        "upload": upload,
        "download": download,
        "total": total,
        "expireMs": expire_ms,
        "runnable": detect_runnable(body),
        "lastError": Value::Null,
        "assembleEnabled": false,
        "templateId": Value::Null,
        "sourceBody": body,
        "contentKind": detect_content_kind(body),
        "assembleOptions": default_assemble_options(),
        "lastAssembleWarnings": [],
        "lastAssembleAt": Value::Null,
    })
}

fn detect_runnable(content: &str) -> bool {
    match serde_json::from_str::<Value>(content) {
        Ok(Value::Object(map)) => map.contains_key("outbounds") || map.contains_key("inbounds"),
        _ => false,
    }
}

fn detect_content_kind(body: &str) -> &'static str {
    let text = body.trim();
    if text.starts_with('{') {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) {
            if map.contains_key("outbounds")
                || map.contains_key("inbounds")
                || map.contains_key("endpoints")
            {
                return "singbox";
            }
        }
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("proxies:") || lower.contains("proxy-groups:") {
        return "clash";
    }
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if !lines.is_empty() {
        let uri_like = lines
            .iter()
            .filter(|l| {
                let s = l.to_ascii_lowercase();
                s.contains("://")
                    && (s.starts_with("ss://")
                        || s.starts_with("vmess://")
                        || s.starts_with("vless://")
                        || s.starts_with("trojan://")
                        || s.starts_with("hysteria")
                        || s.starts_with("hy2://")
                        || s.starts_with("tuic://")
                        || s.contains('@'))
            })
            .count();
        if uri_like >= 1 && uri_like * 2 >= lines.len() {
            return "uriList";
        }
    }
    "unknown"
}

fn unique_name(base: &str, names: &[String]) -> String {
    if !names.iter().any(|n| n == base) {
        return base.to_string();
    }
    let mut i = 1;
    loop {
        let cand = format!("{base} ({i})");
        if !names.iter().any(|n| n == &cand) {
            return cand;
        }
        i += 1;
    }
}

fn name_from_url(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim();
    if host.is_empty() {
        "订阅".into()
    } else {
        host.into()
    }
}

fn name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("本地配置")
        .to_string()
}

fn source_label(source: &str) -> &'static str {
    match source {
        "url" => tr("profiles.remote_badge"),
        "local" => tr("profiles.local_badge"),
        "sample" => tr("profiles.sample_badge"),
        _ => tr("nav.profiles"),
    }
}

fn config_view_lines(pretty: &str) -> Vec<String> {
    pretty.lines().map(str::to_string).collect()
}

fn config_view_scroll(text: &str, cx: &App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let lines = Arc::new(config_view_lines(text));
    let count = lines.len().max(1);
    let lines = lines.clone();
    // Virtual list: a flex column of every line does not report its full
    // height inside the sheet, so scroll stopped mid-JSON with a blank gap.
    uniform_list("profile-view-json", count, move |range, _, _| {
        range
            .map(|i| {
                let line = lines.get(i).cloned().unwrap_or_default();
                let shown = if line.is_empty() {
                    " ".to_string()
                } else {
                    line
                };
                div()
                    .id(("cfg-line", i as u64))
                    .w_full()
                    .px_1()
                    .font_family("Menlo")
                    .text_xs()
                    .text_color(muted)
                    .child(shown)
            })
            .collect()
    })
    .w_full()
    .flex_1()
    .min_h(px(120.))
}

fn json_str(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn json_opt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn json_bool(v: &Value, key: &str, default: bool) -> bool {
    v.get(key).and_then(|x| x.as_bool()).unwrap_or(default)
}

fn json_i64(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(value_i64).unwrap_or(0)
}

fn value_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_u64().map(|n| n as i64))
        .or_else(|| v.as_f64().map(|n| n as i64))
}

fn traffic_label(upload: i64, download: i64, total: i64) -> String {
    if total <= 0 && upload <= 0 && download <= 0 {
        return String::new();
    }
    let used = upload.saturating_add(download);
    if total > 0 {
        format!("{} / {}", fmt_bytes(used), fmt_bytes(total))
    } else {
        format!("已用 {}", fmt_bytes(used))
    }
}

fn fmt_bytes(n: i64) -> String {
    let n = n.max(0) as f64;
    if n < 1024.0 {
        return format!("{} B", n as i64);
    }
    let kb = n / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} KB");
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{mb:.1} MB");
    }
    format!("{:.2} GB", mb / 1024.0)
}

fn expire_label(expire_ms: i64) -> String {
    if expire_ms <= 0 {
        return String::new();
    }
    let (y, m, d, _, _, _) = civil_from_unix(expire_ms.div_euclid(1000));
    format!("到期 {y:04}-{m:02}-{d:02}")
}

fn fmt_updated(iso: &str) -> String {
    if iso.len() >= 16 && iso.as_bytes().get(10) == Some(&b'T') {
        format!("{} {}", &iso[5..10], &iso[11..16])
    } else {
        String::new()
    }
}

fn warnings_brief(p: &Value) -> Option<String> {
    let arr = p.get("lastAssembleWarnings")?.as_array()?;
    if arr.is_empty() {
        return None;
    }
    let first = arr.iter().find_map(|w| {
        let reason = w.get("reason").and_then(|x| x.as_str()).unwrap_or("").trim();
        if reason.is_empty() {
            None
        } else {
            let node = w.get("node").and_then(|x| x.as_str()).unwrap_or("").trim();
            Some(if node.is_empty() {
                reason.to_string()
            } else {
                format!("{node}: {reason}")
            })
        }
    });
    Some(match first {
        Some(s) => format!("跳过 {} · {s}", arr.len()),
        None => format!("跳过 {}", arr.len()),
    })
}

fn ellipsize(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

fn now_iso8601() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let (y, mo, d, h, mi, s) = civil_from_unix(dur.as_secs() as i64);
    format!(
        "{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{:03}Z",
        dur.subsec_millis()
    )
}

/// Howard Hinnant civil calendar from Unix seconds (UTC).
fn civil_from_unix(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400) as u32;
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d, hh, mm, ss)
}

fn pretty_profile_content(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        return String::new();
    }
    serde_json::from_str::<Value>(t)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::{config_view_lines, pretty_profile_content};

    #[test]
    fn pretty_profile_content_formats_json() {
        let out = pretty_profile_content(r#"{"inbounds":[{"type":"tun"}]}"#);
        assert!(out.contains('\n'), "{out}");
        assert!(out.contains("inbounds"), "{out}");
    }

    #[test]
    fn pretty_profile_content_keeps_non_json() {
        assert_eq!(pretty_profile_content("proxies:\n  - a"), "proxies:\n  - a");
        assert!(pretty_profile_content("   ").is_empty());
    }

    #[test]
    fn config_view_keeps_every_pretty_line() {
        let pretty = pretty_profile_content(
            r#"{"log":{"level":"info"},"dns":{"servers":[{"type":"https","tag":"cf","server":"1.1.1.1"}]},"inbounds":[{"type":"mixed","tag":"mixed-in","listen":"127.0.0.1","listen_port":7890}],"outbounds":[{"type":"direct","tag":"direct"}]}"#,
        );
        let lines = config_view_lines(&pretty);
        assert!(lines.len() > 8, "expected full pretty JSON, got {} lines:\n{pretty}", lines.len());
        assert!(lines.iter().any(|l| l.contains("listen_port")), "{pretty}");
        assert!(lines.iter().any(|l| l.contains("outbounds")), "{pretty}");
        assert_eq!(lines.join("\n"), pretty);
    }
}
