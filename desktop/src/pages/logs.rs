use std::sync::Arc;
use std::time::Duration;

use gpui::*;
use gpui_component::button::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::host::{core_log_path, read_core_log_tail, HostClient};
use crate::i18n::tr;
use crate::widgets::{card, chip_tone, muted, page_fill, page_title, ChipTone};

/// Last bytes of `sing-box.core.log` shown in the pane (matches host helper).
const LOG_TAIL_BYTES: usize = 256 * 1024;
const POLL_EVERY: Duration = Duration::from_millis(1500);

/// Tab: 日志. Kernel log tail from the runtime file (direct-start only).
pub struct LogsPage {
    host: Arc<HostClient>,
    running: bool,
    /// Last file tail (full, before 清空显示).
    raw: String,
    /// What the pane shows (may be a suffix after 清空显示).
    text: SharedString,
    path: SharedString,
    search: Entity<InputState>,
    search_query: String,
    /// File snapshot at last 清空显示; auto-refresh only reveals appended bytes.
    cleared_prefix: Option<String>,
    _poll: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl LogsPage {
    pub fn new(host: Arc<HostClient>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let path: SharedString = core_log_path().display().to_string().into();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(tr("logs.search_placeholder")));
        let _subscriptions = vec![cx.subscribe_in(&search, window, {
            move |this, state, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.search_query = state.read(cx).value().to_string();
                    cx.notify();
                }
            }
        })];
        let _poll = Self::spawn_poll(cx);
        Self {
            host,
            running: false,
            raw: String::new(),
            text: SharedString::default(),
            path,
            search,
            search_query: String::new(),
            cleared_prefix: None,
            _poll,
            _subscriptions,
        }
    }

    /// Foreground loop: `WeakEntity` dies with the page; file/status I/O is off-thread.
    fn spawn_poll(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            loop {
                if !Self::reload(&this, cx, false).await {
                    return;
                }
                cx.background_executor().timer(POLL_EVERY).await;
            }
        })
    }

    /// `reset_clear`: 刷新 shows the full tail; auto-poll keeps 清空显示.
    async fn reload(this: &WeakEntity<Self>, cx: &mut AsyncApp, reset_clear: bool) -> bool {
        let host = match this.read_with(cx, |page, _| page.host.clone()) {
            Ok(host) => host,
            Err(_) => return false,
        };
        let (running, tail) = cx
            .background_spawn(async move { snapshot(host) })
            .await;
        this.update(cx, |page, cx| {
            if page.apply(running, tail, reset_clear) {
                cx.notify();
            }
        })
        .is_ok()
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            Self::reload(&this, cx, true).await;
        })
        .detach();
    }

    fn copy_all(&mut self, cx: &mut Context<Self>) {
        let body = self.text.to_string();
        if body.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(body));
    }

    fn clear_display(&mut self, cx: &mut Context<Self>) {
        self.cleared_prefix = Some(self.raw.clone());
        if !self.text.is_empty() {
            self.text = SharedString::default();
        }
        cx.notify();
    }

    /// Returns whether the view should redraw.
    fn apply(&mut self, running: bool, raw: String, reset_clear: bool) -> bool {
        let mut changed = self.running != running;
        self.running = running;
        if reset_clear {
            self.cleared_prefix = None;
        }
        if self.raw != raw {
            self.raw = raw;
            changed = true;
        }

        let displayed = match &self.cleared_prefix {
            Some(prefix) if self.raw == *prefix => String::new(),
            Some(prefix) => match self.raw.strip_prefix(prefix.as_str()) {
                Some(suffix) => suffix.trim_start_matches(['\r', '\n']).to_string(),
                None => {
                    self.cleared_prefix = None;
                    self.raw.clone()
                }
            },
            None => self.raw.clone(),
        };
        if self.text.as_ref() != displayed.as_str() {
            self.text = displayed.into();
            changed = true;
        }
        changed
    }
}

fn snapshot(host: Arc<HostClient>) -> (bool, String) {
    (host.status().running, read_core_log_tail(LOG_TAIL_BYTES))
}

fn log_lines_view(text: &str, query: &str, cx: &App) -> Stateful<Div> {
    let q = query.trim().to_ascii_lowercase();
    let mut col = div()
        .id("logs-text")
        .w_full()
        .flex()
        .flex_col()
        .gap_0()
        .text_xs()
        .font_family("Consolas");
    let lines: Vec<&str> = text.lines().collect();
    let filtered: Vec<&str> = if q.is_empty() {
        lines
    } else {
        lines
            .into_iter()
            .filter(|line| line.to_ascii_lowercase().contains(&q))
            .collect()
    };
    let start = filtered.len().saturating_sub(400);
    for (i, line) in filtered[start..].iter().enumerate() {
        let color = log_line_color(line, cx);
        col = col.child(
            div()
                .id(("log-line", i))
                .text_color(color)
                .child((*line).to_string()),
        );
    }
    col
}

fn log_line_color(line: &str, cx: &App) -> Hsla {
    let l = line.to_ascii_lowercase();
    if l.contains("error") || l.contains("fatal") || l.contains("panic") {
        cx.theme().danger
    } else if l.contains("warn") {
        cx.theme().warning
    } else if l.contains("info") {
        cx.theme().muted_foreground
    } else {
        cx.theme().foreground
    }
}

impl Render for LogsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let empty = self.text.trim().is_empty();
        let status_label = if self.running { tr("home.status.running") } else { tr("home.status.stopped") };
        let empty_hint = if self.running {
            tr("logs.connecting")
        } else {
            tr("logs.empty")
        };

        let pane = card(cx)
            .id("logs-pane")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(if empty {
                div()
                    .id("logs-empty")
                    .w_full()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .py_8()
                    .child(muted(empty_hint, cx))
                    .into_any_element()
            } else {
                log_lines_view(&self.text, &self.search_query, cx).into_any_element()
            });

        page_fill("page-logs")
            .child(
                div()
                    .id("logs-header")
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(page_title(tr("logs.title"), cx))
                    .child(chip_tone(
                        status_label,
                        if self.running {
                            ChipTone::Success
                        } else {
                            ChipTone::Neutral
                        },
                        cx,
                    )),
            )
            .child(muted(self.path.clone(), cx))
            .child(
                div()
                    .id("logs-actions")
                    .flex()
                    .gap_3()
                    .items_center()
                    .flex_wrap()
                    .child(div().w(px(260.)).child(Input::new(&self.search).cleanable(true)))
                    .child(
                        Button::new("logs-refresh")
                            .small()
                            .label(tr("common.refresh"))
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(
                        Button::new("logs-copy")
                            .small()
                            .label(tr("logs.copy"))
                            .disabled(empty)
                            .on_click(cx.listener(|this, _, _, cx| this.copy_all(cx))),
                    )
                    .child(
                        Button::new("logs-clear")
                            .small()
                            .label(tr("logs.clear"))
                            .disabled(empty)
                            .on_click(cx.listener(|this, _, _, cx| this.clear_display(cx))),
                    ),
            )
            .child(pane)
    }
}
