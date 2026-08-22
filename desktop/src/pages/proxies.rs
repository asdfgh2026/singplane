use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use serde_json::Value;

use crate::host::{clash_call_result, clash_encode_name, HostClient};
use crate::store::{clash_base_from_settings, clash_secret_for_calls, load_settings};
use crate::i18n::tr;
use crate::widgets::{
    card, chip, delay_color, muted, page_scroll, page_title, section_header, CARD_RADIUS,
};

const TEST_URL: &str = "https://www.gstatic.com/generate_204";
const DELAY_QUERY_TIMEOUT: &str = "5000";
const CLASH_TIMEOUT_MS: u64 = 8000;
const DELAY_CONCURRENCY: usize = 8;

/// Tab: 代理.
pub struct ProxiesPage {
    host: Arc<HostClient>,
    groups: Vec<ProxyGroup>,
    selected: Option<String>,
    delays: HashMap<String, i64>,
    sort: SortMode,
    search: Entity<InputState>,
    search_query: String,
    loading: bool,
    testing: bool,
    selecting: bool,
    core_running: bool,
    clash_base: String,
    clash_secret: String,
    error: Option<String>,
    notice: Option<String>,
    test_epoch: u64,
    test_queue: VecDeque<String>,
    in_flight: usize,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
struct ProxyGroup {
    name: String,
    kind: String,
    now: String,
    all: Vec<String>,
}

impl ProxyGroup {
    fn is_selector(&self) -> bool {
        self.kind.eq_ignore_ascii_case("selector")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortMode {
    Default,
    Delay,
    Name,
}

impl SortMode {
    fn label(self) -> &'static str {
        match self {
            Self::Default => tr("proxies.sort.default"),
            Self::Delay => tr("proxies.sort.delay"),
            Self::Name => tr("proxies.sort.name"),
        }
    }
}

struct Snapshot {
    running: bool,
    base: String,
    secret: String,
    groups: Vec<ProxyGroup>,
    delays: HashMap<String, i64>,
    error: Option<String>,
}

impl ProxiesPage {
    pub fn new(host: Arc<HostClient>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(tr("proxies.search_placeholder")));
        let _subscriptions = vec![cx.subscribe_in(&search, window, {
            move |this, state, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.search_query = state.read(cx).value().to_string();
                    cx.notify();
                }
            }
        })];

        let mut page = Self {
            host,
            groups: Vec::new(),
            selected: None,
            delays: HashMap::new(),
            sort: SortMode::Default,
            search,
            search_query: String::new(),
            loading: false,
            testing: false,
            selecting: false,
            core_running: false,
            clash_base: String::new(),
            clash_secret: String::new(),
            error: None,
            notice: None,
            test_epoch: 0,
            test_queue: VecDeque::new(),
            in_flight: 0,
            _subscriptions,
        };
        page.refresh(cx);
        page.start_status_loop(cx);
        page
    }

    fn start_status_loop(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(3))
                    .await;
                let busy = this
                    .update(cx, |page, _| page.loading || page.testing)
                    .ok()
                    .unwrap_or(true);
                if busy {
                    continue;
                }
                let Some(host) = this.update(cx, |page, _| page.host.clone()).ok() else {
                    break;
                };
                let snap = cx.background_spawn(async move { fetch_snapshot(&host) }).await;
                this.update(cx, |page, cx| {
                    if page.loading || page.testing {
                        return;
                    }
                    page.apply_snapshot(snap);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let host = self.host.clone();
        self.loading = true;
        self.notice = None;
        cx.notify();
        let task = cx.background_spawn(async move { fetch_snapshot(&host) });
        cx.spawn(async move |this, cx| {
            let snap = task.await;
            this.update(cx, |this, cx| {
                this.apply_snapshot(snap);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn apply_snapshot(&mut self, snap: Snapshot) {
        self.loading = false;
        self.selecting = false;
        self.core_running = snap.running;
        self.clash_base = snap.base;
        self.clash_secret = snap.secret;
        self.groups = snap.groups;
        for (name, delay) in snap.delays {
            if self.delays.get(&name) != Some(&0) {
                self.delays.insert(name, delay);
            }
        }
        self.error = snap.error;
        let still = self
            .selected
            .as_ref()
            .is_some_and(|s| self.groups.iter().any(|g| &g.name == s));
        if !still {
            self.selected = self.groups.first().map(|g| g.name.clone());
        }
    }

    fn current_group(&self) -> Option<&ProxyGroup> {
        let name = self.selected.as_ref()?;
        self.groups.iter().find(|g| &g.name == name)
    }

    fn select_group(&mut self, name: String, cx: &mut Context<Self>) {
        if self.selected.as_deref() == Some(name.as_str()) {
            return;
        }
        self.selected = Some(name);
        self.notice = None;
        cx.notify();
    }

    fn set_sort(&mut self, sort: SortMode, cx: &mut Context<Self>) {
        if self.sort == sort {
            return;
        }
        self.sort = sort;
        cx.notify();
    }

    fn visible_nodes(&self) -> Vec<String> {
        let Some(group) = self.current_group() else {
            return Vec::new();
        };
        let q = self.search_query.trim().to_lowercase();
        let mut names: Vec<String> = group
            .all
            .iter()
            .filter(|n| q.is_empty() || n.to_lowercase().contains(&q))
            .cloned()
            .collect();
        match self.sort {
            SortMode::Default => {}
            SortMode::Name => {
                names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
            }
            SortMode::Delay => {
                names.sort_by(|a, b| {
                    let da = self.delays.get(a).copied();
                    let db = self.delays.get(b).copied();
                    delay_rank(da)
                        .cmp(&delay_rank(db))
                        .then_with(|| match (da, db) {
                            (Some(x), Some(y)) if x > 0 && y > 0 => x.cmp(&y),
                            _ => a.to_lowercase().cmp(&b.to_lowercase()),
                        })
                });
            }
        }
        names
    }

    fn on_select_node(&mut self, node: String, cx: &mut Context<Self>) {
        let Some(group) = self.current_group().cloned() else {
            return;
        };
        if !group.is_selector() {
            self.notice = Some(if group.kind.eq_ignore_ascii_case("urltest") {
                "URLTest 组由内核自动选择，不可手动切换".into()
            } else {
                "当前组类型不支持手动切换".into()
            });
            cx.notify();
            return;
        }
        if group.now == node || self.selecting {
            return;
        }
        if let Some(g) = self.groups.iter_mut().find(|g| g.name == group.name) {
            g.now = node.clone();
        }
        self.selecting = true;
        self.notice = None;
        cx.notify();

        let host = self.host.clone();
        let base = self.clash_base.clone();
        let secret = self.clash_secret.clone();
        let group_name = group.name.clone();
        let task = cx.background_spawn(async move {
            let put = select_proxy(&host, &base, &secret, &group_name, &node);
            let snap = fetch_snapshot(&host);
            (put, snap)
        });
        cx.spawn(async move |this, cx| {
            let (put, snap) = task.await;
            this.update(cx, |this, cx| {
                this.apply_snapshot(snap);
                if let Err(err) = put {
                    this.notice = Some(err);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn test_current_group(&mut self, cx: &mut Context<Self>) {
        if self.testing || !self.core_running {
            return;
        }
        let Some(group) = self.current_group() else {
            return;
        };
        let names: Vec<String> = group
            .all
            .iter()
            .filter(|n| is_testable_node(n))
            .cloned()
            .collect();
        if names.is_empty() {
            self.notice = Some("当前组没有可测速节点".into());
            cx.notify();
            return;
        }
        self.test_epoch = self.test_epoch.wrapping_add(1);
        self.testing = true;
        self.notice = None;
        self.test_queue = names.iter().cloned().collect();
        self.in_flight = 0;
        for name in &names {
            self.delays.insert(name.clone(), 0);
        }
        cx.notify();
        self.pump_tests(cx);
    }

    fn pump_tests(&mut self, cx: &mut Context<Self>) {
        while self.in_flight < DELAY_CONCURRENCY {
            let Some(name) = self.test_queue.pop_front() else {
                break;
            };
            self.in_flight += 1;
            self.spawn_delay(name, self.test_epoch, cx);
        }
        if self.in_flight == 0 && self.test_queue.is_empty() {
            self.testing = false;
        }
    }

    fn spawn_delay(&mut self, name: String, epoch: u64, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let base = self.clash_base.clone();
        let secret = self.clash_secret.clone();
        let task = cx.background_spawn(async move { probe_delay(&host, &base, &secret, &name) });
        cx.spawn(async move |this, cx| {
            let (node, ms) = task.await;
            this.update(cx, |this, cx| {
                if this.test_epoch != epoch {
                    return;
                }
                this.delays.insert(node, ms);
                this.in_flight = this.in_flight.saturating_sub(1);
                this.pump_tests(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ProxiesPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let nodes = self.visible_nodes();
        let selected = self.current_group().cloned();
        let has_groups = !self.groups.is_empty();

        let refresh_e = entity.clone();
        let test_e = entity.clone();

        let mut toolbar = h_flex().gap_2().flex_wrap().child(
            Button::new("proxy-refresh")
                .small()
                .label(tr("common.refresh"))
                .loading(self.loading)
                .disabled(self.loading)
                .on_click(move |_, _, cx| {
                    if let Some(e) = refresh_e.upgrade() {
                        e.update(cx, |this, cx| this.refresh(cx));
                    }
                }),
        );
        toolbar = toolbar.child(
            Button::new("proxy-test-group")
                .small()
                .primary()
                .label(if self.testing {
                    tr("proxies.testing_delay")
                } else {
                    tr("proxies.test_delay")
                })
                .loading(self.testing)
                .disabled(self.testing || selected.is_none() || !self.core_running)
                .on_click(move |_, _, cx| {
                    if let Some(e) = test_e.upgrade() {
                        e.update(cx, |this, cx| this.test_current_group(cx));
                    }
                }),
        );

        let mut sorts = h_flex().gap_1();
        for (id, mode) in [
            ("sort-none", SortMode::Default),
            ("sort-delay", SortMode::Delay),
            ("sort-name", SortMode::Name),
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

        let mut groups_row = h_flex()
            .id("proxy-groups")
            .gap_1()
            .flex_wrap()
            .w_full()
            .pb_1()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.4));
        for (ix, g) in self.groups.iter().enumerate() {
            let active = self.selected.as_deref() == Some(g.name.as_str());
            let name = g.name.clone();
            let e = entity.clone();
            let fg = if active {
                cx.theme().primary
            } else {
                cx.theme().muted_foreground
            };
            groups_row = groups_row.child(
                div()
                    .id(("proxy-group", ix))
                    .px_3()
                    .py_2()
                    .rounded_t(px(10.))
                    .cursor_pointer()
                    .when(active, |d| {
                        d.bg(cx.theme().secondary)
                            .border_b_2()
                            .border_color(cx.theme().primary)
                    })
                    .when(!active, |d| d.hover(|s| s.bg(cx.theme().muted)))
                    .on_click(move |_, _, cx| {
                        if let Some(ent) = e.upgrade() {
                            ent.update(cx, |this, cx| this.select_group(name.clone(), cx));
                        }
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(fg)
                            .when(active, |d| d.font_bold())
                            .child(g.name.clone()),
                    ),
            );
        }

        let mut nodes_row = div()
            .id("proxy-nodes")
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_2();
        if has_groups && nodes.is_empty() {
            nodes_row = nodes_row.child(muted("无匹配节点", cx));
        }
        for (ix, name) in nodes.iter().enumerate() {
            let is_now = selected
                .as_ref()
                .is_some_and(|g| g.now.as_str() == name.as_str());
            let delay = self.delays.get(name).copied();
            let node = name.clone();
            let e = entity.clone();
            nodes_row = nodes_row.child(node_card(
                ix,
                name,
                delay,
                is_now,
                cx,
                move |_, _, app| {
                    if let Some(ent) = e.upgrade() {
                        ent.update(app, |this, cx| this.on_select_node(node.clone(), cx));
                    }
                },
            ));
        }

        page_scroll("page-proxies")
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_start()
                    .gap_3()
                    .child(page_title(tr("nav.proxies"), cx))
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
            .when_some(self.notice.clone(), |d, notice| d.child(muted(notice, cx)))
            .when(self.loading && self.groups.is_empty(), |d| {
                d.child(muted(tr("common.loading"), cx))
            })
            .when(!has_groups && !self.loading, |d| {
                d.child(empty_card(
                    self.core_running,
                    self.error.as_deref(),
                    cx,
                ))
            })
            .when(has_groups, |d| {
                d.child(section_header(tr("proxies.title")))
                    .child(groups_row)
                    .when_some(selected, |d, g| {
                        let now = if g.now.is_empty() {
                            "—".to_string()
                        } else {
                            g.now.clone()
                        };
                        d.child(
                            h_flex()
                                .gap_2()
                                .flex_wrap()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .child(format!("{} · {}", g.name, g.kind)),
                                )
                                .child(chip(g.kind.clone(), cx))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().primary)
                                        .child(format!("{} · {now}", tr("home.card.active_profile"))),
                                )
                                .child(muted(
                                    format!("{} · {}", nodes.len(), self.sort.label()),
                                    cx,
                                )),
                        )
                    })
                    .child(section_header(tr("proxies.title")))
                    .child(nodes_row)
            })
    }
}

fn empty_card(running: bool, error: Option<&str>, cx: &App) -> Div {
    let title = if running { tr("proxies.empty") } else { tr("home.status.stopped") };
    let detail = error
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if running {
                "API 可访问但未解析到 Selector / URLTest 组。".into()
            } else {
                tr("proxies.not_running_hint").into()
            }
        });
    card(cx)
        .child(section_header(title))
        .child(muted(detail, cx))
}

fn node_card(
    ix: usize,
    name: &str,
    delay: Option<i64>,
    selected: bool,
    cx: &App,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let border = if selected {
        cx.theme().primary
    } else {
        cx.theme().border
    };
    let bg = if selected {
        cx.theme().secondary
    } else {
        cx.theme().group_box
    };
    let hover = cx.theme().primary.opacity(0.6);
    div()
        .id(("proxy-node", ix))
        .flex_none()
        .w(px(200.))
        .min_w(px(160.))
        .p_3()
        .rounded(px(CARD_RADIUS))
        .border_1()
        .border_color(border)
        .bg(bg)
        .cursor_pointer()
        .hover(move |s| s.border_color(hover))
        .on_click(on_click)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .when(selected, |d| d.font_bold())
                        .when(!selected, |d| d.font_semibold())
                        .line_clamp(2)
                        .child(name.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(delay_color(delay, cx))
                        .child(format_delay(delay)),
                ),
        )
}

fn format_delay(delay: Option<i64>) -> SharedString {
    match delay {
        None => "—".into(),
        Some(0) => "…".into(),
        Some(d) if d < 0 => "超时".into(),
        Some(d) => format!("{d} ms").into(),
    }
}

fn delay_rank(d: Option<i64>) -> i32 {
    match d {
        Some(x) if x > 0 => 0,
        None => 1,
        Some(0) => 2,
        _ => 3,
    }
}

fn is_testable_node(name: &str) -> bool {
    let n = name.trim().to_ascii_uppercase();
    if n.is_empty() {
        return false;
    }
    !matches!(
        n.as_str(),
        "DIRECT" | "REJECT" | "REJECT-DROP" | "PASS" | "BLOCK" | "DNS" | "COMPATIBLE"
    )
}

fn is_group_type(ty: &str) -> bool {
    matches!(
        ty.to_ascii_lowercase().as_str(),
        "selector" | "urltest" | "fallback" | "loadbalance" | "load-balance" | "load_balance" | "relay"
    )
}

fn normalize_type(ty: &str) -> String {
    match ty.to_ascii_lowercase().as_str() {
        "selector" => "Selector".into(),
        "urltest" => "URLTest".into(),
        "fallback" => "Fallback".into(),
        "loadbalance" | "load-balance" | "load_balance" => "LoadBalance".into(),
        "relay" => "Relay".into(),
        _ => ty.to_string(),
    }
}

fn group_sort_priority(g: &ProxyGroup) -> i32 {
    let n = g.name.to_lowercase();
    let t = g.kind.to_lowercase();
    if n == "global" {
        100
    } else if n == "select" || n == "proxy" || n == "节点选择" {
        0
    } else if t == "selector" {
        1
    } else if t == "urltest" {
        2
    } else {
        10
    }
}

fn json_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_u64().map(|n| n as i64))
        .or_else(|| v.as_f64().map(|n| n as i64))
}

fn last_history_delay(map: &Value) -> Option<i64> {
    let hist = map.get("history")?.as_array()?;
    let last = hist.last()?;
    let d = last.get("delay").and_then(json_i64)?;
    (d > 0).then_some(d)
}

fn value_as_name(v: &Value) -> Option<String> {
    let s = match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => return None,
    };
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn read_all(map: &Value) -> Vec<String> {
    map.get("all")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(value_as_name).collect())
        .unwrap_or_default()
}

fn parse_groups(json: &Value) -> (Vec<ProxyGroup>, HashMap<String, i64>, Option<String>) {
    let proxies = json
        .get("proxies")
        .and_then(|v| v.as_object())
        .or_else(|| json.as_object());
    let Some(proxies) = proxies else {
        return (
            Vec::new(),
            HashMap::new(),
            Some("proxies 响应无法解析为 JSON 对象".into()),
        );
    };

    let mut delays = HashMap::new();
    let mut groups = Vec::new();
    let mut entries = 0usize;
    for (name, v) in proxies {
        if !v.is_object() {
            continue;
        }
        entries += 1;
        if let Some(d) = last_history_delay(v) {
            delays.insert(name.clone(), d);
        }
        let ty = v
            .get("type")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if !is_group_type(&ty) {
            continue;
        }
        let all = read_all(v);
        if all.is_empty() {
            continue;
        }
        groups.push(ProxyGroup {
            name: name.clone(),
            kind: normalize_type(&ty),
            now: v
                .get("now")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            all,
        });
    }
    groups.sort_by(|a, b| {
        group_sort_priority(a)
            .cmp(&group_sort_priority(b))
            .then_with(|| a.name.cmp(&b.name))
    });
    let error = if groups.is_empty() {
        Some(format!(
            "API 可访问但无 Selector/URLTest 组（proxies 条目 {entries}）"
        ))
    } else {
        None
    };
    (groups, delays, error)
}

fn fetch_snapshot(host: &HostClient) -> Snapshot {
    let settings = load_settings();
    let base = clash_base_from_settings(&settings);
    let secret = clash_secret_for_calls(&settings);
    let status = host.status();
    if !status.running {
        let err = status
            .error
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "内核未运行，启动后显示代理组。".into());
        return Snapshot {
            running: false,
            base,
            secret,
            groups: Vec::new(),
            delays: HashMap::new(),
            error: Some(err),
        };
    }

    match host.clash_json(
        &base,
        &secret,
        "GET",
        "/proxies",
        None,
        None,
        Some(CLASH_TIMEOUT_MS),
    ) {
        Ok(json) => {
            let (groups, delays, error) = parse_groups(&json);
            Snapshot {
                running: true,
                base,
                secret,
                groups,
                delays,
                error,
            }
        }
        Err(e) => Snapshot {
            running: true,
            base,
            secret,
            groups: Vec::new(),
            delays: HashMap::new(),
            error: Some(e),
        },
    }
}

fn select_proxy(
    host: &HostClient,
    base: &str,
    secret: &str,
    group: &str,
    node: &str,
) -> Result<(), String> {
    let path = format!("/proxies/{}", clash_encode_name(group));
    let body = serde_json::json!({ "name": node });
    let r = host.clash(
        base,
        secret,
        "PUT",
        &path,
        None,
        Some(body),
        Some(CLASH_TIMEOUT_MS),
    );
    clash_call_result(r).map(|_| ())
}

fn probe_delay(host: &HostClient, base: &str, secret: &str, name: &str) -> (String, i64) {
    let path = format!("/proxies/{}/delay", clash_encode_name(name));
    let query = serde_json::json!({
        "url": TEST_URL,
        "timeout": DELAY_QUERY_TIMEOUT,
    });
    let ms = match host.clash_json(
        base,
        secret,
        "GET",
        &path,
        Some(query),
        None,
        Some(CLASH_TIMEOUT_MS),
    ) {
        Ok(v) => {
            let d = v.get("delay").and_then(json_i64).unwrap_or(-1);
            if d > 0 { d } else { -1 }
        }
        Err(_) => -1,
    };
    (name.to_string(), ms)
}
