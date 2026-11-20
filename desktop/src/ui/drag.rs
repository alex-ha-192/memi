//! Drag to reorder: people in the list, categories on a person's page, notes in a category.
//! Each item carries one of these while it's dragged; dropping it on another of the same kind
//! puts it in that one's place.

use super::widgets::*;
use gpui::*;

#[derive(Clone)]
pub struct DragPerson { pub id: i64, pub label: SharedString }
#[derive(Clone)]
pub struct DragCategory { pub id: i64, pub label: SharedString }
#[derive(Clone)]
pub struct DragNote { pub id: i64, pub collection_id: i64, pub label: SharedString }

/// What follows the pointer while dragging: the item's name on a small card.
pub struct DragPreview {
    label: SharedString,
    bg: Hsla,
    fg: Hsla,
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().max_w(px(260.)).px(px(14.)).py(px(9.)).rounded(px(14.)).bg(self.bg).shadow_lg().opacity(0.95)
            .child(item(self.label.clone(), self.fg).truncate())
    }
}

/// Makes the preview for `on_drag`.
pub fn preview(label: SharedString, bg: Hsla, fg: Hsla, cx: &mut App) -> Entity<DragPreview> {
    cx.new(|_| DragPreview { label, bg, fg })
}
