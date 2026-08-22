use gpui::*;
use gpui_component::*;

/// Card corner radius.
pub const CARD_RADIUS: f32 = 20.0;
/// Dashboard page pad.
pub const PAGE_PAD: f32 = 12.0;
/// Home tile min height.
pub const TILE_MIN_H: f32 = 84.0;

pub fn page_frame(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
    cx: &mut App,
) -> Stateful<Div> {
    page_scroll(id)
        .child(page_title(title, cx))
        .child(muted(hint, cx))
}

pub fn page_scroll(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .size_full()
        .overflow_y_scroll()
        .p(px(PAGE_PAD))
        .flex()
        .flex_col()
        .gap(px(10.))
}

/// Non-scrolling page column so a child can pin to the remaining height.
pub fn page_fill(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .size_full()
        .min_h_0()
        .p(px(PAGE_PAD))
        .flex()
        .flex_col()
        .gap(px(10.))
}

pub fn page_title(title: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_xl()
        .font_bold()
        .text_color(cx.theme().foreground)
        .child(title.into())
}

/// Card: radius 20, surface container, hover → primary edge.
pub fn card(cx: &App) -> Div {
    let hover = cx.theme().primary.opacity(0.6);
    div()
        .p_4()
        .rounded(px(CARD_RADIUS))
        .bg(cx.theme().group_box)
        .border_1()
        .border_color(cx.theme().border.opacity(0.45))
        .hover(move |s| s.border_color(hover))
        .flex()
        .flex_col()
        .gap_2()
}

pub fn card_selected(cx: &App) -> Div {
    card(cx)
        .bg(cx.theme().secondary)
        .border_color(cx.theme().primary)
}

pub fn tile(cx: &App) -> Div {
    card(cx).min_h(px(TILE_MIN_H)).p_3()
}

/// Section label: 12 / w700 / muted.
pub fn section_header(title: impl Into<SharedString>) -> Div {
    div().text_xs().font_bold().child(title.into())
}

pub fn info_header(title: impl Into<SharedString>, cx: &App) -> Div {
    section_header(title).text_color(cx.theme().muted_foreground)
}

pub fn muted(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

#[derive(Clone, Copy, Default)]
pub enum ChipTone {
    #[default]
    Neutral,
    Primary,
    Success,
    Danger,
    Warning,
}

pub fn chip(label: impl Into<SharedString>, cx: &App) -> Div {
    chip_tone(label, ChipTone::Neutral, cx)
}

pub fn chip_tone(label: impl Into<SharedString>, tone: ChipTone, cx: &App) -> Div {
    let (bg, fg) = match tone {
        ChipTone::Neutral => (cx.theme().muted, cx.theme().muted_foreground),
        ChipTone::Primary => (cx.theme().secondary, cx.theme().secondary_foreground),
        ChipTone::Success => (cx.theme().success.opacity(0.16), cx.theme().success),
        ChipTone::Danger => (cx.theme().danger.opacity(0.14), cx.theme().danger),
        ChipTone::Warning => (cx.theme().warning.opacity(0.18), cx.theme().warning),
    };
    div()
        .px_2()
        .py_0p5()
        .rounded(px(8.))
        .text_xs()
        .font_bold()
        .bg(bg)
        .text_color(fg)
        .child(label.into())
}

pub fn delay_color(delay: Option<i64>, cx: &App) -> Hsla {
    match delay {
        None | Some(0) => cx.theme().muted_foreground,
        Some(d) if d < 0 => cx.theme().danger,
        Some(d) if d < 200 => cx.theme().success,
        Some(d) if d < 500 => cx.theme().warning,
        Some(_) => cx.theme().danger,
    }
}
