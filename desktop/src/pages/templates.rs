use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::*;
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use gpui_component::*;
use serde_json::{json, Value};

use crate::host::HostClient;
use crate::store::{
    delete_template, load_all_templates, new_id, save_template, template_by_id,
};
use crate::i18n::tr;
use crate::widgets::{card, chip, muted, page_scroll, page_title, section_header};

const DEFAULT_CONTENT: &str = "{\n  \"inbounds\": [],\n  \"outbounds\": [\n    {\"type\": \"direct\", \"tag\": \"direct\"}\n  ],\n  \"route\": {\"final\": \"direct\"}\n}\n";


#[derive(Clone)]
struct TemplateItem {
    id: SharedString,
    name: SharedString,
    description: SharedString,
    content: SharedString,
    builtin: bool,
}

/// Tab: 模板. Only edit this file when implementing the templates page.
pub struct TemplatesPage {
    _host: Arc<HostClient>,
    templates: Vec<TemplateItem>,
    message: SharedString,
    message_error: bool,
    busy: bool,
    pending_delete: Option<SharedString>,
    viewing_id: Option<SharedString>,
    name_input: Entity<InputState>,
    content_input: Entity<TextareaState>,
}

impl TemplatesPage {
    pub fn new(host: Arc<HostClient>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("名称"));
        let content_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("JSON / JSONC")
                .rows(14)
                .default_value(DEFAULT_CONTENT)
        });
        let this = Self {
            _host: host,
            templates: Vec::new(),
            message: "正在加载…".into(),
            message_error: false,
            busy: true,
            pending_delete: None,
            viewing_id: None,
            name_input,
            content_input,
        };
        this.reload(cx);
        this
    }

    fn reload(&self, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async move { load_all_templates() });
        cx.spawn(async move |this, cx| {
            let raw = task.await;
            this.update(cx, |this, cx| {
                this.busy = false;
                this.apply_loaded(raw);
                if this.message.as_ref() == "正在加载…" {
                    this.set_message("", false);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn apply_loaded(&mut self, raw: Vec<Value>) {
        self.templates = raw.iter().filter_map(parse_item).collect();
        if let Some(id) = &self.viewing_id {
            if !self.templates.iter().any(|t| &t.id == id) {
                self.viewing_id = None;
            }
        }
    }

    fn set_message(&mut self, text: impl Into<SharedString>, error: bool) {
        self.message = text.into();
        self.message_error = error;
    }

    fn find(&self, id: &str) -> Option<&TemplateItem> {
        self.templates.iter().find(|t| t.id.as_ref() == id)
    }

    fn start_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.viewing_id = None;
        self.pending_delete = None;
        self.name_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.content_input
            .update(cx, |s, cx| s.set_value(DEFAULT_CONTENT, window, cx));
        self.open_editor_sheet(window, cx);
        cx.notify();
    }

    fn open_view(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_delete = None;
        let item = self.find(id.as_ref()).cloned();
        self.viewing_id = Some(id);
        if let Some(t) = item {
            if !t.builtin {
                let name = t.name.clone();
                let content = t.content.clone();
                self.name_input
                    .update(cx, |s, cx| s.set_value(name, window, cx));
                self.content_input
                    .update(cx, |s, cx| s.set_value(content, window, cx));
            }
        }
        self.open_editor_sheet(window, cx);
        cx.notify();
    }

    fn open_editor_sheet(&self, window: &mut Window, cx: &mut Context<Self>) {
        let viewing = self
            .viewing_id
            .as_ref()
            .and_then(|id| self.find(id.as_ref()).cloned());
        let viewing_builtin = viewing.as_ref().is_some_and(|t| t.builtin);
        let title = match &viewing {
            None => "新建模板",
            Some(t) if t.builtin => "查看模板",
            Some(_) => "编辑模板",
        };
        let name_input = self.name_input.clone();
        let content_input = self.content_input.clone();
        let entity = cx.entity().downgrade();
        let preview = viewing.clone();
        window.open_sheet(cx, move |sheet, _, _cx| {
            let body = if viewing_builtin {
                let t = preview.as_ref().unwrap();
                v_flex()
                    .gap_2()
                    .child(div().font_semibold().child(t.name.clone()))
                    .when(!t.description.is_empty(), |d| {
                        d.child(div().text_sm().text_color(_cx.theme().muted_foreground).child(t.description.clone()))
                    })
                    .child(
                        div()
                            .id("tpl-preview")
                            .max_h(px(420.))
                            .overflow_y_scroll()
                            .text_xs()
                            .text_color(_cx.theme().muted_foreground)
                            .child(preview_text(t.content.as_ref())),
                    )
                    .into_any_element()
            } else {
                v_flex()
                    .gap_2()
                    .child(div().text_xs().font_bold().child("名称"))
                    .child(Input::new(&name_input).w_full())
                    .child(div().text_xs().font_bold().child("JSON / JSONC"))
                    .child(Textarea::new(&content_input).w_full().h(px(360.)))
                    .into_any_element()
            };
            let footer = if viewing_builtin {
                h_flex().child(
                    Button::new("tpl-sheet-close")
                        .label("关闭")
                        .on_click(|_, window, cx| window.close_sheet(cx)),
                )
            } else {
                let save_e = entity.clone();
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("tpl-sheet-save")
                            .primary()
                            .label("保存")
                            .on_click(move |_, window, cx| {
                                if let Some(view) = save_e.upgrade() {
                                    view.update(cx, |this, cx| this.save(window, cx));
                                }
                            }),
                    )
                    .child(
                        Button::new("tpl-sheet-cancel")
                            .label("取消")
                            .on_click(|_, window, cx| window.close_sheet(cx)),
                    )
            };
            sheet
                .size(px(480.))
                .title(title)
                .child(body)
                .footer(footer)
        });
    }

    fn save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self
            .viewing_id
            .as_ref()
            .and_then(|id| self.find(id.as_ref()))
            .is_some_and(|t| t.builtin)
        {
            self.set_message("内置模板只读，请先复制", true);
            cx.notify();
            return;
        }

        let name_raw = self.name_input.read(cx).value().to_string();
        let content = self.content_input.read(cx).value().to_string();
        if content.trim().is_empty() {
            self.set_message("内容不能为空", true);
            cx.notify();
            return;
        }

        let name = {
            let trimmed = name_raw.trim();
            if trimmed.is_empty() {
                "未命名模板".to_string()
            } else {
                trimmed.to_string()
            }
        };

        let editing_id = self
            .viewing_id
            .as_ref()
            .and_then(|id| self.find(id.as_ref()).filter(|t| !t.builtin))
            .map(|t| t.id.to_string());
        let description = editing_id
            .as_ref()
            .and_then(|id| self.find(id).map(|t| t.description.to_string()))
            .unwrap_or_default();
        let id = editing_id.clone().unwrap_or_else(new_id);
        let json_ok = serde_json::from_str::<Value>(&content).is_ok();
        let item = json!({
            "id": id,
            "name": name,
            "content": content,
            "builtin": false,
            "description": description,
            "updatedAt": now_iso8601(),
        });

        self.busy = true;
        self.set_message("正在保存…", false);
        cx.notify();

        let task = cx.background_spawn(async move {
            save_template(&item)?;
            Ok::<_, String>(load_all_templates())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(raw) => {
                        this.apply_loaded(raw);
                        this.viewing_id = Some(id.into());
                        this.set_message(
                            if json_ok {
                                "模板已保存"
                            } else {
                                "模板已保存（内容不是严格 JSON）"
                            },
                            false,
                        );
                    }
                    Err(e) => this.set_message(format!("保存失败: {e}"), true),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn duplicate(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.set_message("正在复制…", false);
        cx.notify();

        let src_id = id.to_string();
        let fallback_name = self
            .find(src_id.as_str())
            .map(|t| t.name.to_string())
            .unwrap_or_else(|| "模板".into());

        let task = cx.background_spawn(async move {
            let src = template_by_id(&src_id)
                .ok_or_else(|| format!("找不到模板: {src_id}"))?;
            let src_name = src
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(fallback_name.as_str());
            let copied_name = format!("{src_name} 副本");
            let item = json!({
                "id": new_id(),
                "name": copied_name,
                "content": src.get("content").and_then(|v| v.as_str()).unwrap_or(""),
                "description": src.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                "builtin": false,
                "updatedAt": now_iso8601(),
            });
            save_template(&item)?;
            Ok::<_, String>((load_all_templates(), copied_name))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok((raw, copied_name)) => {
                        this.apply_loaded(raw);
                        this.set_message(format!("已复制为「{copied_name}」"), false);
                    }
                    Err(e) => this.set_message(format!("复制失败: {e}"), true),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn request_delete(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if id.as_ref().starts_with("builtin-") {
            self.set_message("内置模板不能删除", true);
            cx.notify();
            return;
        }
        self.pending_delete = Some(id);
        cx.notify();
    }

    fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.pending_delete = None;
        cx.notify();
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(id) = self.pending_delete.clone() else {
            return;
        };
        if id.as_ref().starts_with("builtin-") {
            self.pending_delete = None;
            self.set_message("内置模板不能删除", true);
            cx.notify();
            return;
        }

        self.busy = true;
        self.set_message("正在删除…", false);
        cx.notify();

        let delete_id = id.to_string();
        let viewing = self.viewing_id.clone();
        let task = cx.background_spawn(async move {
            delete_template(&delete_id)?;
            Ok::<_, String>(load_all_templates())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.busy = false;
                this.pending_delete = None;
                match result {
                    Ok(raw) => {
                        this.apply_loaded(raw);
                        if viewing.as_ref() == Some(&id) {
                            this.viewing_id = None;
                        }
                        this.set_message("已删除", false);
                    }
                    Err(e) => this.set_message(format!("删除失败: {e}"), true),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn render_card(
        &self,
        t: &TemplateItem,
        entity: WeakEntity<Self>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let confirming = self.pending_delete.as_ref() == Some(&t.id);
        let name = if t.name.is_empty() {
            SharedString::from("未命名模板")
        } else {
            t.name.clone()
        };

        let view_e = entity.clone();
        let view_id = t.id.clone();
        let copy_e = entity.clone();
        let copy_id = t.id.clone();
        let del_e = entity.clone();
        let del_id = t.id.clone();
        let confirm_e = entity.clone();
        let cancel_e = entity;

        card(cx)
            .id(ElementId::from((
                ElementId::from("tpl-card"),
                t.id.clone(),
            )))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().font_semibold().child(name))
                    .when(t.builtin, |d| d.child(chip("内置", cx))),
            )
            .when(!t.description.is_empty(), |d| {
                d.child(muted(t.description.clone(), cx))
            })
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new((ElementId::from("tpl-view"), t.id.clone()))
                            .label("查看")
                            .compact()
                            .disabled(self.busy)
                            .on_click(move |_, window, cx| {
                                if let Some(view) = view_e.upgrade() {
                                    let id = view_id.clone();
                                    view.update(cx, |this, cx| this.open_view(id, window, cx));
                                }
                            }),
                    )
                    .child(
                        Button::new((ElementId::from("tpl-copy"), t.id.clone()))
                            .label("复制")
                            .compact()
                            .disabled(self.busy)
                            .on_click(move |_, _, cx| {
                                if let Some(view) = copy_e.upgrade() {
                                    let id = copy_id.clone();
                                    view.update(cx, |this, cx| this.duplicate(id, cx));
                                }
                            }),
                    )
                    .when(!t.builtin, |d| {
                        d.when(confirming, |d| {
                            d.child(
                                Button::new((ElementId::from("tpl-confirm"), t.id.clone()))
                                    .danger()
                                    .label("确认")
                                    .compact()
                                    .disabled(self.busy)
                                    .on_click(move |_, _, cx| {
                                        if let Some(view) = confirm_e.upgrade() {
                                            view.update(cx, |this, cx| this.confirm_delete(cx));
                                        }
                                    }),
                            )
                            .child(
                                Button::new((ElementId::from("tpl-cancel"), t.id.clone()))
                                    .label("取消")
                                    .compact()
                                    .disabled(self.busy)
                                    .on_click(move |_, _, cx| {
                                        if let Some(view) = cancel_e.upgrade() {
                                            view.update(cx, |this, cx| this.cancel_delete(cx));
                                        }
                                    }),
                            )
                        })
                        .when(!confirming, |d| {
                            d.child(
                                Button::new((ElementId::from("tpl-del"), t.id.clone()))
                                    .danger()
                                    .label("删除")
                                    .compact()
                                    .disabled(self.busy)
                                    .on_click(move |_, _, cx| {
                                        if let Some(view) = del_e.upgrade() {
                                            let id = del_id.clone();
                                            view.update(cx, |this, cx| this.request_delete(id, cx));
                                        }
                                    }),
                            )
                        })
                    }),
            )
    }
}

impl Render for TemplatesPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let builtins: Vec<TemplateItem> = self
            .templates
            .iter()
            .filter(|t| t.builtin)
            .cloned()
            .collect();
        let users: Vec<TemplateItem> = self
            .templates
            .iter()
            .filter(|t| !t.builtin)
            .cloned()
            .collect();
        let new_e = entity.clone();

        let mut builtin_list = div().id("tpl-builtins").flex().flex_col().gap_2();
        for t in &builtins {
            builtin_list = builtin_list.child(self.render_card(t, entity.clone(), cx));
        }

        let mut user_list = div().id("tpl-users").flex().flex_col().gap_2();
        if users.is_empty() {
            user_list = user_list.child(card(cx).child(muted(
                tr("templates.empty"),
                cx,
            )));
        } else {
            for t in &users {
                user_list = user_list.child(self.render_card(t, entity.clone(), cx));
            }
        }

        page_scroll("page-templates")
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(page_title(tr("nav.templates"), cx))
                    .child(
                        Button::new("tpl-new")
                            .label(tr("templates.new"))
                            .disabled(self.busy)
                            .on_click(move |_, window, cx| {
                                if let Some(view) = new_e.upgrade() {
                                    view.update(cx, |this, cx| this.start_create(window, cx));
                                }
                            }),
                    ),
            )
            .when(!self.message.is_empty(), |d| {
                d.child(
                    div()
                        .text_sm()
                        .text_color(if self.message_error {
                            cx.theme().danger
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(self.message.clone()),
                )
            })
            .child(section_header(tr("templates.builtin")))
            .child(builtin_list)
            .child(section_header(tr("templates.custom")))
            .child(user_list)
    }
}

fn parse_item(v: &Value) -> Option<TemplateItem> {
    let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
    if id.is_empty() {
        return None;
    }
    Some(TemplateItem {
        id: id.to_string().into(),
        name: v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
            .into(),
        description: v
            .get("description")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
            .into(),
        content: v
            .get("content")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
            .into(),
        builtin: v
            .get("builtin")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
    })
}

fn preview_text(content: &str) -> SharedString {
    const MAX: usize = 2000;
    if content.chars().count() <= MAX {
        return content.to_string().into();
    }
    let mut out: String = content.chars().take(MAX).collect();
    out.push_str("\n…");
    out.into()
}

fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;
    let (year, month, day) = civil_from_unix_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Howard Hinnant civil_from_days: `z` is days since 1970-01-01.
fn civil_from_unix_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}
