//! Tab: 连接. Live Clash `/connections` list.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui::*;
use gpui_component::button::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Selectable as _, *};
use serde_json::Value;

use crate::host::{clash_call_result, clash_encode_name, HostClient};
use crate::store::{clash_base_from_settings, clash_secret_for_calls, load_settings};
use crate::i18n::tr;
use crate::widgets::{card, chip, muted, page_fill, page_title, section_header, CARD_RADIUS};

const POLL_EVERY: Duration = Duration::from_secs(1);
const MAX_ROWS: usize = 200;

/// Tab: 连接.
pub struct ConnectionsPage {
    host: Arc<HostClient>,
    running: bool,
    items: Vec<ConnRow>,
    search: Entity<InputState>,
    search_query: String,
    sort: SortMode,
    loading: bool,
    closing: bool,
    error: Option<String>,
    clash_base: String,
    clash_secret: String,
    _poll: Task<()>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortMode {
    Default,
    Speed,
    Traffic,
}

impl SortMode {
    fn label(self) -> &'static str {
        match self {
            Self::Default => tr("connections.sort.default"),
            Self::Speed => tr("connections.sort.speed"),
            Self::Traffic => tr("connections.sort.traffic"),
        }
    }
}

#[derive(Clone)]
struct ConnRow {
    id: String,
    host: String,
    dest: String,
    network: String,
    process: String,
    chains: Vec<String>,
    rule: String,
    upload: u64,
    download: u64,
    upload_speed: u64,
    download_speed: u64,
    start: String,
}

impl ConnectionsPage {
    pub fn new(host: Arc<HostClient>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索主机 / 进程 / 节点"));
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
            items: Vec::new(),
            search,
            search_query: String::new(),
            sort: SortMode::Default,
            loading: false,
            closing: false,
            error: None,
            clash_base: String::new(),
            clash_secret: String::new(),
            _poll,
            _subscriptions,
        }
    }

    fn spawn_poll(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            loop {
                if !Self::reload(&this, cx).await {
                    return;
                }
                cx.background_executor().timer(POLL_EVERY).await;
            }
        })
    }

    async fn reload(this: &WeakEntity<Self>, cx: &mut AsyncApp) -> bool {
        let Some((host, prev)) = this
            .update(cx, |page, _| {
                (page.host.clone(), page.prev_totals())
            })
            .ok()
        else {
            return false;
        };
        let snap = cx
            .background_spawn(async move { fetch_snapshot(&host, &prev) })
            .await;
        this.update(cx, |page, cx| {
            page.apply(snap);
            cx.notify();
        })
        .is_ok()
    }

    fn prev_totals(&self) -> HashMap<String, (u64, u64)> {
        self.items
            .iter()
            .map(|c| (c.id.clone(), (c.upload, c.download)))
            .collect()
    }

    fn apply(&mut self, snap: Snapshot) {
        self.loading = false;
        self.running = snap.running;
        self.clash_base = snap.base;
        self.clash_secret = snap.secret;
        self.items = snap.items;
        self.error = snap.error;
    }

    fn visible(&self) -> Vec<ConnRow> {
        let q = self.search_query.trim().to_lowercase();
        let mut rows: Vec<ConnRow> = self
            .items
            .iter()
            .filter(|c| {
                if q.is_empty() {
                    return true;
                }
                c.host.to_lowercase().contains(&q)
                    || c.dest.to_lowercase().contains(&q)
                    || c.process.to_lowercase().contains(&q)
                    || c.rule.to_lowercase().contains(&q)
                    || c.chains.iter().any(|n| n.to_lowercase().contains(&q))
            })
            .cloned()
            .collect();
        match self.sort {
            SortMode::Default => {}
            SortMode::Speed => {
                rows.sort_by(|a, b| {
                    (b.upload_speed + b.download_speed).cmp(&(a.upload_speed + a.download_speed))
                });
            }
            SortMode::Traffic => {
                rows.sort_by(|a, b| (b.upload + b.download).cmp(&(a.upload + a.download)));
            }
        }
        rows.truncate(MAX_ROWS);
        rows
    }

    fn set_sort(&mut self, sort: SortMode, cx: &mut Context<Self>) {
        if self.sort == sort {
            return;
        }
        self.sort = sort;
        cx.notify();
    }

    fn close_one(&mut self, id: String, cx: &mut Context<Self>) {
        if self.closing || id.is_empty() {
            return;
        }
        self.closing = true;
        cx.notify();
        let host = self.host.clone();
        let base = self.clash_base.clone();
        let secret = self.clash_secret.clone();
        let path = format!("/connections/{}", clash_encode_name(&id));
        let task = cx.background_spawn(async move {
            host.clash(&base, &secret, "DELETE", &path, None, None, Some(4000))
        });
        cx.spawn(async move |this, cx| {
            let result = clash_call_result(task.await).map(|_| ());
            if result.is_ok() {
                let _ = Self::reload(&this, cx).await;
            }
            this.update(cx, |page, cx| {
                page.closing = false;
                if let Err(error) = result {
                    page.error = Some(error);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn close_all(&mut self, cx: &mut Context<Self>) {
        if self.closing || !self.running {
            return;
        }
        self.closing = true;
        cx.notify();
        let host = self.host.clone();
        let base = self.clash_base.clone();
        let secret = self.clash_secret.clone();
        let task = cx.background_spawn(async move {
            host.clash(&base, &secret, "DELETE", "/connections", None, None, Some(5000))
        });
        cx.spawn(async move |this, cx| {
            let result = clash_call_result(task.await).map(|_| ());
            if result.is_ok() {
                let _ = Self::reload(&this, cx).await;
            }
            this.update(cx, |page, cx| {
                page.closing = false;
                if let Err(error) = result {
                    page.error = Some(error);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ConnectionsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let rows = self.visible();
        let total = self.items.len();
        let busy = self.closing;

        let mut sorts = div().flex().items_center().gap_1();
        for (mode, id) in [
            (SortMode::Default, "conn-sort-default"),
            (SortMode::Speed, "conn-sort-speed"),
            (SortMode::Traffic, "conn-sort-traffic"),
        ] {
            let e = entity.clone();
            sorts = sorts.child(
                Button::new(id)
                    .small()
                    .label(mode.label())
                    .selected(self.sort == mode)
                    .on_click(move |_, _, cx| {
                        if let Some(ent) = e.upgrade() {
                            ent.update(cx, |this, cx| this.set_sort(mode, cx));
                        }
                    }),
            );
        }

        let close_e = entity.clone();
        let toolbar = div()
            .flex()
            .items_center()
            .gap_2()
            .child(chip(format!("{total}"), cx))
            .child(
                Button::new("conn-close-all")
                    .small()
                    .label(tr("connections.close_all"))
                    .disabled(busy || !self.running || total == 0)
                    .on_click(move |_, _, cx| {
                        if let Some(ent) = close_e.upgrade() {
                            ent.update(cx, |this, cx| this.close_all(cx));
                        }
                    }),
            );

        let mut list = div()
            .id("conn-list")
            .flex()
            .flex_col()
            .gap_2()
            .w_full();
        if !self.running {
            list = list.child(empty_card(tr("home.status.stopped"), tr("proxies.not_running_hint"), cx));
        } else {
            if let Some(error) = &self.error {
                list = list.child(empty_card(tr("common.failed"), error, cx));
            }
            if rows.is_empty() && self.error.is_none() {
                list = list.child(empty_card(
                    tr("connections.empty"),
                    tr("connections.empty"),
                    cx,
                ));
            }
            for (ix, row) in rows.iter().enumerate() {
                list = list.child(conn_card(ix, row, busy, entity.clone(), cx));
            }
        }

        page_fill("page-connections")
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_start()
                    .gap_3()
                    .child(page_title(tr("nav.connections"), cx))
                    .child(toolbar),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .flex_wrap()
                    .items_center()
                    .child(div().w(px(280.)).child(Input::new(&self.search).cleanable(true)))
                    .child(sorts),
            )
            .child(
                div()
                    .id("conn-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list),
            )
    }
}

fn conn_card(
    ix: usize,
    row: &ConnRow,
    busy: bool,
    entity: WeakEntity<ConnectionsPage>,
    cx: &App,
) -> impl IntoElement {
    let id = row.id.clone();
    let host = if row.host.is_empty() {
        row.dest.clone()
    } else {
        row.host.clone()
    };
    let speed = format!(
        "{}↑  {}↓",
        format_bytes(row.upload_speed),
        format_bytes(row.download_speed)
    );
    let total = format!(
        "{}↑  {}↓",
        format_bytes(row.upload),
        format_bytes(row.download)
    );
    let meta = [
        row.network.as_str(),
        row.dest.as_str(),
        row.process.as_str(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");

    let mut chains = div().flex().flex_wrap().gap_1().items_center();
    for chain in &row.chains {
        let q = chain.clone();
        let e = entity.clone();
        chains = chains.child(
            div()
                .id(SharedString::from(format!("conn-chain-{ix}-{chain}")))
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    if let Some(ent) = e.upgrade() {
                        ent.update(cx, |this, cx| {
                            this.search_query = q.clone();
                            cx.notify();
                        });
                    }
                })
                .child(chip(chain.clone(), cx)),
        );
    }
    if !row.rule.is_empty() {
        chains = chains.child(muted(row.rule.clone(), cx));
    }

    card(cx)
        .id(("conn-row", ix))
        .p_3()
        .rounded(px(CARD_RADIUS))
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .min_w_0()
                                        .child(host),
                                )
                                .child(muted(row.start.clone(), cx)),
                        )
                        .child(muted(meta, cx))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{total}  ·  {speed}/s")),
                        )
                        .child(chains),
                )
                .child(
                    Button::new(("conn-close", ix))
                        .small()
                        .label(tr("common.close"))
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            if let Some(ent) = entity.upgrade() {
                                ent.update(cx, |this, cx| this.close_one(id.clone(), cx));
                            }
                        }),
                ),
        )
}

fn empty_card(title: &str, detail: &str, cx: &App) -> Div {
    card(cx)
        .child(section_header(title))
        .child(muted(detail, cx))
}

struct Snapshot {
    running: bool,
    base: String,
    secret: String,
    items: Vec<ConnRow>,
    error: Option<String>,
}

fn fetch_snapshot(host: &HostClient, prev: &HashMap<String, (u64, u64)>) -> Snapshot {
    let settings = load_settings();
    let base = clash_base_from_settings(&settings);
    let secret = clash_secret_for_calls(&settings);
    let running = host.status().running;
    if !running {
        return Snapshot {
            running: false,
            base,
            secret,
            items: Vec::new(),
            error: None,
        };
    }
    match host.clash_json(&base, &secret, "GET", "/connections", None, None, Some(5000)) {
        Ok(json) => Snapshot {
            running: true,
            base,
            secret,
            items: parse_connections(&json, prev),
            error: None,
        },
        Err(e) => Snapshot {
            running: true,
            base,
            secret,
            items: Vec::new(),
            error: Some(e),
        },
    }
}

fn parse_connections(json: &Value, prev: &HashMap<String, (u64, u64)>) -> Vec<ConnRow> {
    let Some(arr) = json.get("connections").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| parse_one(v, prev))
        .collect()
}

fn parse_one(v: &Value, prev: &HashMap<String, (u64, u64)>) -> Option<ConnRow> {
    let id = v.get("id").and_then(|x| x.as_str())?.to_string();
    let meta = v.get("metadata").cloned().unwrap_or(Value::Null);
    let host = first_str(&meta, &["host", "sniffHost", "destinationIP"]);
    let dest_ip = json_str(&meta, "destinationIP");
    let dest_port = json_str(&meta, "destinationPort");
    let dest = if dest_ip.is_empty() {
        String::new()
    } else if dest_port.is_empty() {
        dest_ip
    } else {
        format!("{dest_ip}:{dest_port}")
    };
    let src_ip = json_str(&meta, "sourceIP");
    let src_port = json_str(&meta, "sourcePort");
    let src = if src_ip.is_empty() {
        String::new()
    } else if src_port.is_empty() {
        src_ip
    } else {
        format!("{src_ip}:{src_port}")
    };
    let process = first_str(&meta, &["process", "processPath"]);
    let process = file_name(&process);
    let network = json_str(&meta, "network");
    let chains = v
        .get("chains")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .rev()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let rule = {
        let r = json_str(v, "rule");
        let p = json_str(v, "rulePayload");
        if r.is_empty() {
            p
        } else if p.is_empty() {
            r
        } else {
            format!("{r} {p}")
        }
    };
    let upload = json_u64(v, "upload");
    let download = json_u64(v, "download");
    let (up_s, down_s) = match prev.get(&id) {
        Some((pu, pd)) => (upload.saturating_sub(*pu), download.saturating_sub(*pd)),
        None => (0, 0),
    };
    let title = if host.is_empty() { dest.clone() } else { host };
    Some(ConnRow {
        id,
        host: title,
        dest: if src.is_empty() { dest } else { format!("{src} → {dest}") },
        network,
        process,
        chains,
        rule,
        upload,
        download,
        upload_speed: up_s,
        download_speed: down_s,
        start: short_start(&json_str(v, "start")),
    })
}

fn first_str(v: &Value, keys: &[&str]) -> String {
    for k in keys {
        let s = json_str(v, k);
        if !s.is_empty() {
            return s;
        }
    }
    String::new()
}

fn json_str(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn json_u64(v: &Value, key: &str) -> u64 {
    let Some(val) = v.get(key) else {
        return 0;
    };
    val.as_u64()
        .or_else(|| val.as_i64().map(|n| n.max(0) as u64))
        .or_else(|| val.as_f64().map(|n| n.max(0.0) as u64))
        .unwrap_or(0)
}

fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_string()
}

fn short_start(raw: &str) -> String {
    if raw.len() >= 19 {
        raw[11..19].to_string()
    } else {
        raw.to_string()
    }
}

fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    let n = n as f64;
    if n < KB {
        format!("{n:.0} B")
    } else if n < KB * KB {
        format!("{:.1} KB", n / KB)
    } else if n < KB * KB * KB {
        format!("{:.2} MB", n / (KB * KB))
    } else {
        format!("{:.2} GB", n / (KB * KB * KB))
    }
}
