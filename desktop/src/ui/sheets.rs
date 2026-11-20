//! Everything that opens over the window: a category's list, the editors, confirmations, sync,
//! and the photo cropper. Each sits on a dimmed backdrop; clicking the backdrop or pressing
//! Escape closes it.

use super::app::*;
use super::drag::{self, DragNote};
use super::theme::Colors;
use super::widgets::*;
use crate::model::*;
use crate::sync::Problem;
use crate::{dates, photos};
use chrono::{Datelike, Duration as Days, NaiveDate, Weekday};
use gpui::prelude::*;
use gpui::*;
use gpui_component::input::Input;

const SHEET_WIDTH: f32 = 500.;

impl MemiView {
    pub fn overlay_view(&mut self, c: &Colors, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Each sheet: a body that scrolls if the window is short, and buttons that always show.
        let (content, footer): (AnyElement, Option<AnyElement>) = match self.overlay.as_ref().unwrap() {
            Overlay::Category { .. } => (self.category_sheet(c, cx).into_any_element(), None),
            Overlay::EditPerson(_) => self.person_sheet(c, cx),
            Overlay::EditNote(_) => self.note_sheet(c, cx),
            Overlay::EditCategory(_) => self.category_editor_sheet(c, cx),
            Overlay::Confirm { .. } => self.confirm_sheet(c, cx),
            Overlay::Sync => (self.sync_sheet(c, window, cx).into_any_element(), None),
        };
        // Taller than the window? The sheet scrolls; the backdrop always fills the window.
        div().absolute().inset_0().occlude().bg(c.scrim).flex().items_center().justify_center().p(px(24.))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if matches!(this.overlay, Some(Overlay::EditPerson(_))) { this.close_person_editor(None, cx); } else { this.overlay = None; }
                this.stop_pairing();
                cx.notify();
            }))
            .child(div().w(px(SHEET_WIDTH)).max_w_full().max_h_full().flex().flex_col()
                .rounded(px(28.)).bg(c.sheet).border_1().border_color(c.line).shadow_lg().overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().id("sheet-body").flex_shrink(1.).min_h_0().overflow_y_scroll()
                    .child(div().p(px(24.)).when(footer.is_some(), |d| d.pb(px(10.))).flex().flex_col().gap(px(14.)).child(content)))
                .when_some(footer, |d, f| d.child(div().flex_none().px(px(24.)).pb(px(22.)).pt(px(12.)).border_t_1().border_color(c.line).child(f))))
    }

    fn sheet_title(&self, text: impl Into<SharedString>, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().items_start().gap(px(12.))
            .child(tile_title(text, c.ink).flex_1().min_w_0())
            .child(div().id("close").size(px(34.)).flex_none().rounded(px(12.)).flex().items_center().justify_center().cursor_pointer()
                .hover(|s| s.bg(c.track))
                .child(icon("x", 18., c.muted))
                .on_click(cx.listener(|this, _, _, cx| {
                    if matches!(this.overlay, Some(Overlay::EditPerson(_))) { this.close_person_editor(None, cx); } else { this.overlay = None; }
                    this.stop_pairing();
                    cx.notify();
                })))
    }

    // --- A category: add to it, see what's in it ---------------------------------------------------

    fn category_sheet(&mut self, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(Overlay::Category { person_id, collection_id, input }) = &self.overlay else { unreachable!() };
        let (pid, cid, input) = (*person_id, *collection_id, input.clone());
        let Some(cat) = self.collection(cid).cloned() else { return div().child(body("This category was removed.", c.muted)) };
        let person = self.person(pid).cloned();
        let today = dates::today();
        let open: Vec<Note> = self.notes.iter().filter(|n| n.collection_id == cid && n.status == status::OPEN).cloned().collect();
        let occasion = person.as_ref().and_then(|p| dates::next_occasion(&cat, p, today))
            .map(|d| format!("For {} · {}", dates::day_label(d, today), dates::in_days(d, today)));
        let ready = !input.read(cx).value().trim().is_empty();
        let mut list = div().flex().flex_col().gap(px(8.));
        for (i, n) in open.iter().enumerate() {
            let sub = match n.due_at {
                Some(d) => format!("Added {} · due {}", dates::ago(n.mentioned_at, today), dates::short_day_label(dates::to_date(d), today)),
                None => format!("Added {}", dates::ago(n.mentioned_at, today)),
            };
            let n2 = n.clone();
            let (nid, drag_bg, drag_fg, accent) = (n.id, c.ink, c.bg, c.ink);
            list = list.child(self.note_card(("in-cat", i), n, None, &sub, None, c, cx)
                .on_click(cx.listener(move |this, _, window, cx| this.open_note_editor(n2.clone(), window, cx)))
                // Drag onto another note to put it in that place.
                .on_drag(DragNote { id: nid, collection_id: cid, label: n.text.clone().into() }, move |d, _, _, cx| drag::preview(d.label.clone(), drag_bg, drag_fg, cx))
                .drag_over::<DragNote>(move |s, d, _, _| if d.id == nid || d.collection_id != cid { s } else { s.border_color(accent) })
                .on_drop(cx.listener(move |this, d: &DragNote, _, cx| {
                    if d.id != nid && d.collection_id == cid { this.change(cx, |s| s.place_note(d.id, nid)); }
                })));
        }
        div().flex().flex_col().gap(px(14.))
            .child(div().flex().flex_col().gap(px(4.))
                .child(self.sheet_title(cat.name.clone(), c, cx))
                .child(caption(match (&person, &occasion) {
                    (Some(p), Some(o)) => format!("{} · {}", p.name, o),
                    (Some(p), None) => p.name.clone(),
                    _ => String::new(),
                }, c.muted)))
            .child(div().flex().gap(px(8.)).items_center()
                .child(div().flex_1().child(Input::new(&input).h(px(46.))))
                .child(div().id("add-note").size(px(46.)).flex_none().rounded(px(15.)).flex().items_center().justify_center()
                    .bg(if ready { c.ink } else { c.track })
                    .when(ready, |d| d.cursor_pointer())
                    .child(icon("plus", 20., if ready { c.bg } else { c.muted }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let text = input.read(cx).value().to_string();
                        if !text.trim().is_empty() {
                            this.add_note(pid, cid, &text, cx);
                            input.update(cx, |s, cx| s.set_value("", window, cx));
                        }
                    }))))
            .child(if open.is_empty() {
                div().py(px(10.)).child(body("Nothing here yet. Type above and press Enter.", c.muted))
            } else {
                list
            })
            .when(!open.is_empty(), |d| d.child(caption("Click to edit, drag to reorder. Right-click, or use ⋯, to mark done, a hit or a miss.", c.muted)))
    }

    // --- Add or edit a person --------------------------------------------------------------------

    fn person_sheet(&mut self, c: &Colors, cx: &mut Context<Self>) -> (AnyElement, Option<AnyElement>) {
        let Some(Overlay::EditPerson(e)) = &self.overlay else { unreachable!() };
        let editing = e.id.is_some();
        let title = if editing { format!("Edit {}", e.name.read(cx).value().split(' ').next().unwrap_or("")) } else { "New person".into() };
        let preview = Person { photo_path: e.photo.clone(), tone_index: e.id.and_then(|id| self.person(id)).map(|p| p.tone_index).unwrap_or(0), ..Person::new("") };
        let has_photo = e.photo.is_some();
        let busy = e.busy;
        let error = e.error.clone();
        let month = e.month;
        let (name, rel, day, year) = (e.name.clone(), e.relationship.clone(), e.day.clone(), e.year.clone());
        let can_save = !name.read(cx).value().trim().is_empty();
        let id = e.id;

        let mut months = div().flex().flex_wrap().gap(px(6.));
        for (i, m) in dates::MONTHS_LONG.iter().enumerate() {
            let mi = i as u32 + 1;
            months = months.child(chip(("month", i), &m[..3], month == Some(mi), c)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(Overlay::EditPerson(e)) = &mut this.overlay { e.month = if e.month == Some(mi) { None } else { Some(mi) }; }
                    cx.notify();
                })));
        }

        let body = div().flex().flex_col().gap(px(14.))
            .child(self.sheet_title(title, c, cx))
            .child(div().flex().items_center().gap(px(16.))
                .child(div().id("pick-photo").relative().cursor_pointer()
                    .child(photo(&preview, 104., 130., 26., c))
                    .child(div().absolute().bottom(px(8.)).right(px(8.)).size(px(32.)).rounded(px(11.)).bg(c.sheet).flex().items_center().justify_center()
                        .child(icon("image-plus", 16., c.ink)))
                    .on_click(cx.listener(|this, _, _, cx| this.pick_photo(cx))))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(4.))
                    .child(item(if busy { "Opening photo…" } else if has_photo { "Change photo" } else { "Add a photo" }, c.ink))
                    .child(caption("You'll crop it next. JPEG, PNG or WebP.", c.muted))
                    .when(has_photo, |d| d.child(div().flex().gap(px(6.)).mt(px(6.))
                        .child(chip("recrop", "Adjust crop", false, c).on_click(cx.listener(|this, _, _, cx| this.recrop(cx))))
                        .child(chip("no-photo", "Remove photo", false, c).on_click(cx.listener(|this, _, _, cx| {
                            if let Some(Overlay::EditPerson(e)) = &mut this.overlay { e.photo = None; }
                            cx.notify();
                        })))))
                    .when_some(error, |d, err| d.child(caption(err, c.danger)))))
            .child(field_label("Name", c)).child(Input::new(&name).h(px(46.)))
            .child(field_label("Who they are to you", c)).child(Input::new(&rel).h(px(46.)))
            .child(field_label("Birthday", c))
            .child(div().flex().gap(px(8.))
                .child(div().w(px(90.)).child(Input::new(&day).h(px(46.))))
                .child(div().flex_1().child(Input::new(&year).h(px(46.)))))
            .child(caption("Day, then the year if you know it (optional). Pick the month:", c.muted).pl(px(4.)))
            .child(months);
        let footer = div().flex().gap(px(8.))
                .when_some(id, |d, id| d.child(secondary_button("remove", "Remove", true, c).on_click(cx.listener(move |this, _, _, cx| {
                    let first = this.person(id).map(|p| p.first_name().to_string()).unwrap_or_default();
                    this.close_person_editor(None, cx);
                    this.overlay = Some(Overlay::Confirm {
                        title: format!("Remove {first}?"),
                        body: "Their notes and photo will be deleted from this computer, and from your other devices if you sync. This can't be undone.".into(),
                        confirm: "Remove".into(),
                        action: ConfirmAction::RemovePerson(id),
                    });
                    cx.notify();
                }))))
                .child(primary_button("save", "Save", can_save && !busy, c).flex_1()
                    .on_click(cx.listener(|this, _, _, cx| this.save_person(cx))));
        (body.into_any_element(), Some(footer.into_any_element()))
    }

    // --- Edit a note -------------------------------------------------------------------------------

    fn note_sheet(&mut self, c: &Colors, cx: &mut Context<Self>) -> (AnyElement, Option<AnyElement>) {
        let Some(Overlay::EditNote(e)) = &self.overlay else { unreachable!() };
        let today = dates::today();
        let (text, cid, due, pid) = (e.text.clone(), e.collection_id, e.due, e.note.person_id);
        let mut cats = div().flex().flex_wrap().gap(px(6.));
        for (i, cat) in self.categories_of(pid).into_iter().enumerate() {
            let id = cat.id;
            cats = cats.child(chip(("cat", i), &cat.name, cat.id == cid, c).on_click(cx.listener(move |this, _, _, cx| {
                if let Some(Overlay::EditNote(e)) = &mut this.overlay { e.collection_id = id; }
                cx.notify();
            })));
        }
        let saturday = (0..7).map(|d| today + Days::days(d)).find(|d| d.weekday() == Weekday::Sat).unwrap_or(today);
        let options: [(&str, Option<NaiveDate>); 5] = [
            ("No date", None),
            ("Tomorrow", Some(today + Days::days(1))),
            ("This weekend", Some(saturday)),
            ("Next week", Some(today + Days::days(7))),
            ("In a month", today.checked_add_months(chrono::Months::new(1))),
        ];
        let mut dues = div().flex().flex_wrap().gap(px(6.));
        for (i, (text_, date)) in options.into_iter().enumerate() {
            dues = dues.child(chip(("due", i), text_, due == date, c).on_click(cx.listener(move |this, _, _, cx| {
                if let Some(Overlay::EditNote(e)) = &mut this.overlay { e.due = date; }
                cx.notify();
            })));
        }
        let custom = due.filter(|d| !options.iter().any(|(_, o)| *o == Some(*d)));
        let can_save = !text.read(cx).value().trim().is_empty();
        let body = div().flex().flex_col().gap(px(14.))
            .child(self.sheet_title("Edit note", c, cx))
            .child(Input::new(&text).h(px(46.)))
            .child(field_label("Category", c)).child(cats)
            .child(field_label("Due", c)).child(dues)
            .when_some(custom, |d, date| d.child(caption(format!("Currently due {}", dates::short_date(date)), c.muted).pl(px(4.))));
        let footer = primary_button("save", "Save", can_save, c).on_click(cx.listener(|this, _, _, cx| this.save_note(cx)));
        (body.into_any_element(), Some(footer.into_any_element()))
    }

    // --- New or edited category, optionally tied to a date ---------------------------------------

    fn category_editor_sheet(&mut self, c: &Colors, cx: &mut Context<Self>) -> (AnyElement, Option<AnyElement>) {
        let Some(Overlay::EditCategory(e)) = &self.overlay else { unreachable!() };
        let person = self.person(e.person_id).cloned().unwrap_or_else(|| Person::new(""));
        let title = match &e.existing { Some(x) => format!("Edit {}", x.name), None => format!("New category for {}", person.first_name()) };
        let (occ, day, yearly, picking, name) = (e.occasion.clone(), e.day, e.yearly, e.picking, e.name.clone());
        let set = |f: fn(&mut CategoryEditor)| move |this: &mut MemiView, _: &ClickEvent, _: &mut Window, cx: &mut Context<MemiView>| {
            if let Some(Overlay::EditCategory(e)) = &mut this.overlay { f(e); }
            cx.notify();
        };
        let chosen = day.and_then(dates::from_epoch_day);
        let valid = !name.read(cx).value().trim().is_empty() && (occ != occasion::DATE || day.is_some());
        let body = div().flex().flex_col().gap(px(14.))
            .child(self.sheet_title(title, c, cx))
            .child(field_label("Name", c)).child(Input::new(&name).h(px(46.)))
            .child(field_label("Builds up to", c))
            .child(div().flex().flex_wrap().gap(px(6.))
                .child(chip("none", "Nothing", occ == occasion::NONE, c).on_click(cx.listener(set(|e| e.occasion = occasion::NONE.into()))))
                .child(chip("bday", &format!("{}'s birthday", person.first_name()), occ == occasion::BIRTHDAY, c).on_click(cx.listener(set(|e| e.occasion = occasion::BIRTHDAY.into()))))
                .child(chip("date", "A date", occ == occasion::DATE, c).on_click(cx.listener(set(|e| {
                    e.occasion = occasion::DATE.into();
                    if e.day.is_none() { e.picking = Some(dates::today()); }
                })))))
            .when(occ == occasion::BIRTHDAY && !person.has_birthday(), |d| d.child(caption(format!("{} doesn't have a birthday saved yet. Add one from their profile.", person.first_name()), c.muted).pl(px(4.))))
            .when(occ == occasion::DATE, |d| d
                .child(div().flex().flex_wrap().gap(px(6.))
                    .child(chip("pick", &chosen.map(|d| format!("{} {}", dates::short_date(d), d.year())).unwrap_or_else(|| "Pick a date".into()), chosen.is_some(), c)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(Overlay::EditCategory(e)) = &mut this.overlay { e.picking = Some(chosen.unwrap_or_else(dates::today)); }
                            cx.notify();
                        })))
                    .child(chip("once", "Just once", !yearly, c).on_click(cx.listener(set(|e| e.yearly = false))))
                    .child(chip("yearly", "Every year", yearly, c).on_click(cx.listener(set(|e| e.yearly = true)))))
                .when_some(picking, |d, month| d.child(self.calendar(month, chosen, c, cx))));
        let footer = primary_button("save", "Save", valid, c).on_click(cx.listener(|this, _, _, cx| this.save_category(cx)));
        (body.into_any_element(), Some(footer.into_any_element()))
    }

    /// A month of days to pick from.
    fn calendar(&self, shown: NaiveDate, chosen: Option<NaiveDate>, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let first = shown.with_day(1).unwrap();
        let today = dates::today();
        let lead = first.weekday().num_days_from_monday() as i64;
        let days_in_month = (first.checked_add_months(chrono::Months::new(1)).unwrap() - first).num_days();
        let step = |delta: i32| move |this: &mut MemiView, _: &ClickEvent, _: &mut Window, cx: &mut Context<MemiView>| {
            if let Some(Overlay::EditCategory(e)) = &mut this.overlay {
                if let Some(p) = e.picking {
                    let f = p.with_day(1).unwrap();
                    e.picking = if delta > 0 { f.checked_add_months(chrono::Months::new(1)) } else { f.checked_sub_months(chrono::Months::new(1)) };
                }
            }
            cx.notify();
        };
        let mut grid = div().grid().grid_cols(7).gap(px(4.));
        for w in ["M", "T", "W", "T", "F", "S", "S"] {
            grid = grid.child(caption(w, c.muted).flex().justify_center());
        }
        for _ in 0..lead { grid = grid.child(div()); }
        for d in 1..=days_in_month {
            let date = first.with_day(d as u32).unwrap();
            let selected = chosen == Some(date);
            grid = grid.child(div().id(("day", d as usize)).h(px(34.)).rounded(px(10.)).flex().items_center().justify_center().cursor_pointer()
                .when(selected, |x| x.bg(c.ink))
                .when(!selected && date == today, |x| x.border_1().border_color(c.ink))
                .when(!selected, |x| x.hover(|s| s.bg(c.track)))
                .child(label(format!("{d}"), if selected { c.bg } else { c.ink }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(Overlay::EditCategory(e)) = &mut this.overlay { e.day = Some(dates::epoch_day(date)); e.picking = None; }
                    cx.notify();
                })));
        }
        div().rounded(px(18.)).bg(c.track).p(px(12.)).flex().flex_col().gap(px(8.))
            .child(div().flex().items_center()
                .child(div().id("prev").size(px(30.)).rounded(px(10.)).flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(c.sheet))
                    .child(icon("chevron-left", 17., c.ink)).on_click(cx.listener(step(-1))))
                .child(card_title(format!("{} {}", dates::MONTHS_LONG[first.month0() as usize], first.year()), c.ink).flex_1().flex().justify_center())
                .child(div().id("next").size(px(30.)).rounded(px(10.)).flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(c.sheet))
                    .child(icon("chevron-right", 17., c.ink)).on_click(cx.listener(step(1)))))
            .child(grid)
    }

    // --- Are you sure? -----------------------------------------------------------------------------

    fn confirm_sheet(&mut self, c: &Colors, cx: &mut Context<Self>) -> (AnyElement, Option<AnyElement>) {
        let Some(Overlay::Confirm { title, body: text, confirm, .. }) = &self.overlay else { unreachable!() };
        let (title, text, confirm) = (title.clone(), text.clone(), confirm.clone());
        let content = div().flex().flex_col().gap(px(14.))
            .child(self.sheet_title(title, c, cx))
            .child(body(text, c.ink));
        let footer = div().flex().gap(px(8.))
                .child(secondary_button("cancel", "Cancel", false, c).flex_1().on_click(cx.listener(|this, _, _, cx| {
                    // Starting sync over is asked from the Sync sheet, so Cancel goes back there.
                    let back = matches!(&this.overlay, Some(Overlay::Confirm { action: ConfirmAction::StartSyncOver, .. }));
                    this.overlay = back.then_some(Overlay::Sync);
                    cx.notify();
                })))
                .child(div().id("confirm").flex_1().h(px(48.)).rounded(px(16.)).bg(c.danger).flex().items_center().justify_center().cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .child(card_title(confirm, hsla(0., 0., 1., 1.)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(Overlay::Confirm { action, .. }) = this.overlay.take() {
                            match action {
                                ConfirmAction::RemovePerson(id) => this.remove_person(id, cx),
                                ConfirmAction::RemoveCategory(cat) => {
                                    let name = cat.name.clone();
                                    this.change(cx, |s| s.delete_collection(&cat));
                                    this.toast(format!("Removed {name}"), None, cx);
                                }
                                ConfirmAction::StartSyncOver => {
                                    this.stop_pairing();
                                    this.sync_ui.typing_key = None;
                                    this.sync_ui.key_shown = None;
                                    this.overlay = Some(Overlay::Sync);
                                    this.run_sync(cx, |e| e.start_over());
                                }
                            }
                        }
                        cx.notify();
                    })));
        (content.into_any_element(), Some(footer.into_any_element()))
    }

    // --- Sync --------------------------------------------------------------------------------------

    fn sync_sheet(&mut self, c: &Colors, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.sync_ui.settings.clone();
        let busy = self.sync_ui.busy;
        let problem = self.sync_ui.problem.clone();
        let mut col = div().flex().flex_col().gap(px(14.)).child(self.sheet_title("Sync", c, cx));

        match problem.clone() {
            Some(Problem::NeedsKey) => {
                if self.sync_ui.pairing.is_none() && self.sync_ui.typing_key.is_none() { self.start_pairing(cx); }
                col = col
                    .child(body("Memi is already syncing on another device. To bring it here, that device hands this computer the key.", c.ink));
                if let Some(confirm) = self.sync_ui.pairing_confirm.clone() {
                    col = col.child(div().rounded(px(18.)).bg(c.card).border_1().border_color(c.line).p(px(16.)).flex().flex_col().gap(px(4.))
                        .child(card_title("On any device that's already syncing", c.ink))
                        .child(body("Open Memi there. It will ask whether to let this computer join; allow it if it shows:", c.muted))
                        .child(display(confirm, c).text_size(px(30.))));
                }
                col = col.child(body("Or, on your phone: Theme → Sync → Add a computer, then scan this.", c.ink));
                if let Some(code) = self.sync_ui.pairing_code.clone() {
                    col = col.child(div().flex().flex_col().items_center().gap(px(8.))
                        .child(qr(&code))
                        .child(caption("Waiting for your phone…", c.muted)));
                } else if self.sync_ui.typing_key.is_none() {
                    col = col.child(secondary_button("new-code", "Show a new code", false, c).on_click(cx.listener(|this, _, _, cx| this.start_pairing(cx))));
                }
                match self.sync_ui.typing_key.clone() {
                    None => {
                        col = col.child(div().id("type-key").cursor_pointer().child(label("Can't scan? Type the recovery key instead", c.ink))
                            .on_click(cx.listener(|this, _, window, cx| {
                                let input = cx.new(|cx| gpui_component::input::InputState::new(window, cx).placeholder("xxxxxxxx-xxxxxxxx-…"));
                                input.update(cx, |s, cx| s.focus(window, cx));
                                this.sync_ui.typing_key = Some(input);
                                cx.notify();
                            })));
                    }
                    Some(input) => {
                        let ready = !input.read(cx).value().trim().is_empty();
                        col = col
                            .child(Input::new(&input).h(px(46.)))
                            .child(primary_button("unlock", if busy { "Checking…" } else { "Unlock backup" }, ready && !busy, c)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let text = input.read(cx).value().to_string();
                                    this.stop_pairing();
                                    this.run_sync(cx, move |e| e.use_recovery_key(&text));
                                })));
                    }
                }
                col = col.child(secondary_button("cancel", "Cancel", false, c).on_click(cx.listener(|this, _, _, cx| {
                    this.stop_pairing();
                    this.sync_ui.typing_key = None;
                    this.sync_ui.problem = None;
                    this.run_sync(cx, |e| { e.turn_off(); Ok(None) });
                })));
                col = col.child(div().id("start-over").cursor_pointer().child(label("Lost your recovery key? Start over", c.danger))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overlay = Some(Overlay::Confirm {
                            title: "Start sync over?".into(),
                            body: "Without its recovery key, nobody can open the copy in your Google account, so Memi will delete it and make a new one from what's on this computer. You'll get a new recovery key. Your other devices will ask for it before they sync again, and anything that's only on them stays there until you set them up.".into(),
                            confirm: "Start over".into(),
                            action: ConfirmAction::StartSyncOver,
                        });
                        cx.notify();
                    })));
            }
            _ if !s.enabled => {
                col = col
                    .child(body("Keep a copy of everything in your own Google account, so your phone and this computer stay in step.", c.ink))
                    .child(body("It's encrypted on this computer before it leaves, so Google can't read it, and it sits in a hidden folder only Memi can open, away from your Drive files. Nobody else, including us, ever gets a copy.", c.ink))
                    .child(primary_button("connect", if busy { "Waiting for your browser…" } else { "Choose a Google account" }, !busy, c)
                        .on_click(cx.listener(|this, _, _, cx| this.run_sync(cx, |e| e.sign_in_and_connect()))))
                    .when(busy, |d| d.child(caption("Finish signing in in your browser, then come back here.", c.muted)));
            }
            _ => {
                let detail = if busy { "Syncing…".to_string() } else {
                    match s.last_sync { Some(t) => format!("Up to date · synced {}", since(t)), None => "Waiting to sync".into() }
                };
                col = col.child(div().rounded(px(18.)).bg(c.card).border_1().border_color(c.line).p(px(16.)).flex().flex_col().gap(px(2.))
                    .child(card_title(s.account.clone().map(|a| format!("Syncing with {a}")).unwrap_or_else(|| "Sync is on".into()), c.ink))
                    .child(label(detail, c.muted)));
                if let Some(Problem::NeedsSignIn) = problem {
                    col = col.child(body("Google needs you to sign in again before Memi can sync.", c.ink))
                        .child(primary_button("again", "Sign in again", !busy, c).on_click(cx.listener(|this, _, _, cx| this.run_sync(cx, |e| e.sign_in_and_connect()))));
                }
                if let Some(Problem::Failed(m)) = &problem {
                    col = col.child(body(format!("The last sync didn't work ({m}). Memi will try again by itself."), c.ink));
                }
                col = col.child(div().flex().gap(px(8.))
                    .child(secondary_button("now", "Sync now", false, c).flex_1().on_click(cx.listener(|this, _, _, cx| this.run_sync(cx, |e| e.sync()))))
                    .child(secondary_button("off", "Turn off", true, c).flex_1().on_click(cx.listener(|this, _, _, cx| this.run_sync(cx, |e| { e.turn_off(); Ok(None) })))));
                col = col.child(caption("To set up another computer, open Sync on it and scan its code with your phone, or type this recovery key there. Keep it somewhere safe and private.", c.muted));
                match self.sync_ui.key_shown.clone() {
                    None => col = col.child(secondary_button("show-key", "Show recovery key", false, c).on_click(cx.listener(|this, _, _, cx| {
                        this.sync_ui.key_shown = Some(this.sync.recovery_key().unwrap_or_else(|| "No key on this computer".into()));
                        cx.notify();
                    }))),
                    Some(key) => {
                        let k2 = key.clone();
                        col = col.child(div().rounded(px(14.)).bg(c.track).p(px(12.)).child(t(key, 13., FontWeight::MEDIUM, c.ink).font_family("monospace")))
                            .child(div().flex().gap(px(8.))
                                .child(secondary_button("copy", "Copy", false, c).flex_1().on_click(cx.listener(move |this, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(k2.clone()));
                                    this.toast("Copied. Paste it somewhere safe, then clear your clipboard.", None, cx);
                                })))
                                .child(secondary_button("hide", "Hide", false, c).flex_1().on_click(cx.listener(|this, _, _, cx| { this.sync_ui.key_shown = None; cx.notify(); }))));
                    }
                }
            }
        }
        let _ = window;
        col.child(div().id("privacy").cursor_pointer().child(label("How sync handles your data", c.ink))
            .on_click(|_, _, _| { let _ = webbrowser::open("https://yourmemi.app/privacy/"); }))
    }

    // --- The photo cropper ---------------------------------------------------------------------------

    pub fn crop_view(&mut self, c: &Colors, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vp = window.viewport_size();
        let viewport = Size { width: f32::from(vp.width), height: f32::from(vp.height) };
        let crop = self.crop.as_mut().unwrap();
        // The frame: as big as fits, leaving room for the title and buttons.
        let avail_h = (viewport.height - 200.).max(200.);
        let avail_w = (viewport.width - 80.).max(200.);
        let fw = avail_w.min(avail_h * photos::ASPECT);
        let fh = fw / photos::ASPECT;
        crop.frame = Size { width: fw, height: fh };
        crop.clamp();
        let s = crop.base_scale() * crop.zoom;
        let (iw, ih) = (crop.image.width() as f32 * s, crop.image.height() as f32 * s);
        let (left, top) = ((fw - iw) / 2. + crop.offset.x, (fh - ih) / 2. + crop.offset.y);
        let preview = crop.preview.clone();
        let white = hsla(0., 0., 1., 1.);

        let frame = div().id("crop-frame").relative().w(px(fw)).h(px(fh)).rounded(px(28.)).overflow_hidden().cursor_grab()
            .border_2().border_color(white)
            .child(img(ImageSource::Image(preview)).absolute().left(px(left)).top(px(top)).w(px(iw)).h(px(ih)))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, cx| {
                if let Some(c) = &mut this.crop { c.drag_from = Some((e.position, c.offset)); }
                cx.stop_propagation();
            }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if let Some(c) = &mut this.crop {
                    if let (Some((from, start)), true) = (c.drag_from, e.pressed_button == Some(MouseButton::Left)) {
                        c.offset = Point { x: start.x + f32::from(e.position.x - from.x), y: start.y + f32::from(e.position.y - from.y) };
                        c.clamp();
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| { if let Some(c) = &mut this.crop { c.drag_from = None; } }))
            .on_scroll(cx.listener(|this, e: &ScrollEvent, window, cx| {
                if let Some(c) = &mut this.crop {
                    let dy = f32::from(e.delta.pixel_delta(window.line_height()).y);
                    c.zoom = (c.zoom * (1. + dy * 0.004)).clamp(1., 5.);
                    c.clamp();
                    cx.notify();
                    cx.stop_propagation();
                }
            }));

        div().absolute().inset_0().occlude().bg(hsla(250. / 360., 0.12, 0.07, 0.97)).flex().flex_col().items_center().justify_center().gap(px(18.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(item("Drag to move, scroll to zoom", white))
            .child(frame)
            // Two equal buttons, exactly as wide as the frame.
            .child(div().w(px(fw)).flex().gap(px(10.))
                .child(div().id("crop-cancel").flex_1().h(px(50.)).rounded(px(16.)).bg(hsla(0., 0., 1., 0.12)).flex().items_center().justify_center().cursor_pointer()
                    .child(card_title("Cancel", white))
                    .on_click(cx.listener(|this, _, _, cx| { this.crop = None; cx.notify(); })))
                .child(div().id("crop-use").flex_1().h(px(50.)).rounded(px(16.)).bg(c.gift()).flex().items_center().justify_center().cursor_pointer()
                    .child(card_title("Use photo", c.gift_ink()))
                    .on_click(cx.listener(|this, _, _, cx| this.finish_crop(cx)))))
    }
}

/// A QR code, dark on white whatever the theme, so phone cameras read it easily.
fn qr(text: &str) -> impl IntoElement {
    let code = qrcode::QrCode::with_error_correction_level(text.as_bytes(), qrcode::EcLevel::M).expect("QR");
    let n = code.width();
    let cell = (200. / n as f32).floor().max(2.);
    let colors = code.to_colors();
    let mut rows = div().flex().flex_col();
    for y in 0..n {
        let mut row = div().flex();
        for x in 0..n {
            let dark = colors[y * n + x] == qrcode::Color::Dark;
            row = row.child(div().size(px(cell)).when(dark, |d| d.bg(rgb(0x1C1B2E))));
        }
        rows = rows.child(row);
    }
    div().p(px(16.)).rounded(px(20.)).bg(rgb(0xFFFFFF)).child(rows)
}

fn since(millis: i64) -> String {
    let mins = (crate::model::now_millis() - millis) / 60_000;
    match mins {
        i64::MIN..=0 => "just now".into(),
        1..=59 => format!("{mins} min ago"),
        60..=1439 => format!("{} h ago", mins / 60),
        _ => format!("{} d ago", mins / 1440),
    }
}
