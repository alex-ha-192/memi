//! Small building blocks shared by every screen: text, icons, buttons, chips, photos.

use super::theme::{Colors, FONT};
use crate::model::Person;
use gpui::prelude::*;
use gpui::*;

pub fn t(text: impl Into<SharedString>, size: f32, weight: FontWeight, color: Hsla) -> Div {
    div().font_family(FONT).text_size(px(size)).line_height(px(size * 1.3)).font_weight(weight).text_color(color).child(text.into())
}

pub fn display(text: impl Into<SharedString>, c: &Colors) -> Div { t(text, 30., FontWeight::EXTRA_BOLD, c.ink) }
pub fn tile_title(text: impl Into<SharedString>, color: Hsla) -> Div { t(text, 21., FontWeight::EXTRA_BOLD, color) }
pub fn section(text: impl Into<SharedString>, c: &Colors) -> Div { t(text, 18., FontWeight::BOLD, c.ink) }
pub fn card_title(text: impl Into<SharedString>, color: Hsla) -> Div { t(text, 15., FontWeight::BOLD, color) }
pub fn item(text: impl Into<SharedString>, color: Hsla) -> Div { t(text, 14.5, FontWeight::SEMIBOLD, color) }
pub fn body(text: impl Into<SharedString>, color: Hsla) -> Div { t(text, 14., FontWeight::NORMAL, color) }
pub fn label(text: impl Into<SharedString>, color: Hsla) -> Div { t(text, 13., FontWeight::MEDIUM, color) }
pub fn caption(text: impl Into<SharedString>, color: Hsla) -> Div { t(text, 12., FontWeight::MEDIUM, color) }

pub fn icon(name: &str, size: f32, color: Hsla) -> Svg {
    svg().path(SharedString::from(format!("icons/{name}.svg"))).size(px(size)).flex_none().text_color(color)
}

/// A small coloured label: "Due Thu", "New · 2d".
pub fn tag(text: impl Into<SharedString>, bg: Hsla, fg: Hsla) -> Div {
    div().flex_none().px(px(9.)).py(px(4.)).rounded(px(9.)).bg(bg).child(t(text, 11.5, FontWeight::BOLD, fg))
}

/// The square header buttons (themes, search, add).
pub fn header_button(id: &'static str, icon_name: &str, selected: bool, c: &Colors) -> Stateful<Div> {
    div().id(id).size(px(44.)).flex_none().rounded(px(15.)).flex().items_center().justify_center()
        .bg(if selected { c.ink } else { c.card })
        .cursor_pointer()
        .hover(|s| s.opacity(0.85))
        .child(icon(icon_name, 19., if selected { c.bg } else { c.ink }))
}

pub fn primary_button(id: impl Into<ElementId>, text: &str, enabled: bool, c: &Colors) -> Stateful<Div> {
    div().id(id).h(px(48.)).px(px(20.)).rounded(px(16.)).flex().items_center().justify_center()
        .bg(if enabled { c.ink } else { c.track })
        .when(enabled, |d| d.cursor_pointer().hover(|s| s.opacity(0.9)))
        .child(card_title(text.to_string(), if enabled { c.bg } else { c.muted }))
}

pub fn secondary_button(id: impl Into<ElementId>, text: &str, danger: bool, c: &Colors) -> Stateful<Div> {
    div().id(id).h(px(48.)).px(px(20.)).rounded(px(16.)).flex().items_center().justify_center()
        .bg(c.track).border_1().border_color(c.line)
        .cursor_pointer().hover(|s| s.opacity(0.85))
        .child(card_title(text.to_string(), if danger { c.danger } else { c.ink }))
}

/// A choice chip; dark when picked.
pub fn chip(id: impl Into<ElementId>, text: &str, selected: bool, c: &Colors) -> Stateful<Div> {
    div().id(id).px(px(13.)).py(px(8.)).rounded(px(13.)).cursor_pointer()
        .bg(if selected { c.ink } else { c.track })
        .border_1().border_color(if selected { c.ink } else { c.line })
        .hover(|s| s.opacity(0.85))
        .child(t(text.to_string(), 12.5, FontWeight::BOLD, if selected { c.bg } else { c.ink }))
}

/// Someone's photo, or their colour with a silhouette when there isn't one.
pub fn photo(p: &Person, w: f32, h: f32, radius: f32, c: &Colors) -> Div {
    let base = div().w(px(w)).h(px(h)).flex_none().rounded(px(radius)).overflow_hidden().bg(c.tone(p.tone_index));
    match &p.photo_path {
        Some(path) => base.child(img(std::path::PathBuf::from(path)).size_full().rounded(px(radius)).object_fit(ObjectFit::Cover)),
        None => base.flex().items_end().justify_center().child(icon("user", w.min(h) * 0.72, hsla(0., 0., 1., 0.6))),
    }
}

pub fn field_label(text: &str, c: &Colors) -> Div {
    caption(text.to_string(), c.muted).pl(px(4.))
}
