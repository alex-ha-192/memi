//! The window's layout: the list pane (header, next up, Still fresh, everyone) on the left and
//! the person on the right, plus menus, the theme panel and the toast.

use super::app::*;
use super::app::Menu;
use super::drag::{self, DragCategory, DragPerson};
use super::theme::{Colors, SCHEMES};
use super::widgets::*;
use crate::model::*;
use crate::{dates, insights};
use gpui::prelude::*;
use gpui::*;
use gpui_component::input::Input;

const LIST_WIDTH: f32 = 400.;
/// Below this width the window shows one screen at a time, like the phone app: the list, or one
/// person's page with a back button. That lets it sit in half of a screen.
const TWO_PANE_MIN: f32 = 820.;
/// How many things the Coming up tile lists.
const NEXT_EVENTS: usize = 3;

impl Render for MemiView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = self.colors();
        // The person's name shows in the title only while their page is on screen.
        let page_showing = f32::from(window.viewport_size().width) >= TWO_PANE_MIN || self.page_open;
        window.set_window_title(&match self.shown_person() {
            Some(p) if page_showing => format!("{} · Memi", p.first_name()),
            _ => "Memi".into(),
        });
        // Nothing focused (after a sheet closes, say): take focus so Escape and shortcuts reach us.
        if window.focused(cx).is_none() { self.focus.focus(window, cx); }
        div()
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .bg(c.bg)
            .font_family(super::theme::FONT)
            .text_color(c.ink)
            .key_context("Memi")
            .on_action(cx.listener(MemiView::back))
            .on_action(cx.listener(|this, _: &NewPerson, window, cx| if this.welcome.is_none() { this.open_person_editor(None, window, cx) }))
            .on_action(cx.listener(|this, _: &Find, window, cx| if this.welcome.is_none() { this.open_search(window, cx) }))
            .child({
                let width = f32::from(window.viewport_size().width);
                if let Some(step) = self.welcome {
                    // First launch: the questions fill the window (see welcome.rs).
                    self.welcome_view(step, &c, cx)
                } else if width >= TWO_PANE_MIN {
                    div().size_full().flex()
                        .child(self.list_pane(&c, false, window, cx))
                        .child(div().w(px(1.)).h_full().bg(c.line))
                        .child(self.detail_pane(&c, width - LIST_WIDTH - 1., false, cx))
                } else if self.page_open && self.shown_person().is_some() {
                    div().size_full().flex().child(self.detail_pane(&c, width, true, cx))
                } else {
                    div().size_full().flex().child(self.list_pane(&c, true, window, cx))
                }
            })
            .when(self.theme_open, |d| d.child(self.theme_panel(&c, cx)))
            .when(self.overlay.is_some(), |d| d.child(self.overlay_view(&c, window, cx)))
            .when(self.crop.is_some(), |d| d.child(self.crop_view(&c, window, cx)))
            .when(self.menu.is_some(), |d| d.child(self.menu_view(&c, cx)))
            .when(self.toast.is_some(), |d| d.child(self.toast_view(&c, cx)))
    }
}

impl MemiView {
    /// Sync in a word or two, above "Your people": Synced, Syncing…, Not synced or Sign in.
    /// Opens Sync. Routine checks in the background don't show as Syncing.
    fn sync_chip(&mut self, c: &Colors, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = &self.sync_ui;
        let (icon_name, label, bg, ink) = if matches!(s.problem, Some(crate::sync::Problem::NeedsSignIn)) {
            ("cloud-alert", "Sign in", c.gift(), c.gift_ink())
        } else if s.busy && (!s.settings.enabled || s.problem.is_some()) {
            ("refresh-cw", "Syncing…", c.tile(3), c.tile_ink(3))
        } else if s.settings.enabled && s.problem.is_none() {
            ("cloud-check", "Synced", c.tile(1), c.tile_ink(1))
        } else {
            ("cloud-off", "Not synced", c.card, c.ink)
        };
        div().id("sync-chip").flex().items_center().gap(px(5.)).pl(px(8.)).pr(px(11.)).py(px(3.)).rounded_full().bg(bg)
            .cursor_pointer()
            .child(icon(icon_name, 14., ink))
            .child(t(label.to_string(), 13., FontWeight::BOLD, ink))
            .on_click(cx.listener(|this, _, _, cx| { this.theme_open = false; this.overlay = Some(Overlay::Sync); cx.notify(); }))
    }

    // --- Left: the list pane ---------------------------------------------------------------------

    fn list_pane(&mut self, c: &Colors, narrow: bool, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let today = dates::today();
        let header = div().flex().items_center().gap(px(10.)).px(px(28.)).pt(px(26.)).pb(px(18.))
            .child(match &self.search {
                Some(input) => div().flex_1().min_w_0().child(Input::new(input).h(px(44.))),
                None => div().flex_1().min_w_0().overflow_hidden().flex().flex_col().gap(px(6.))
                    .child(div().flex().child(self.sync_chip(c, cx)))
                    .child(display("Your people", c).text_size(px(28.)).whitespace_nowrap().text_ellipsis()),
            })
            .when(self.search.is_none(), |d| d
                .child(header_button("themes", "settings", self.theme_open, c)
                    .on_click(cx.listener(|this, _, _, cx| { this.theme_open = !this.theme_open; cx.notify(); })))
                .child(header_button("search", "search", false, c)
                    .on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx))))
                .child(header_button("add", "plus", false, c)
                    .on_click(cx.listener(|this, _, window, cx| this.open_person_editor(None, window, cx)))))
            .when(self.search.is_some(), |d| d.child(header_button("close-search", "x", false, c)
                .on_click(cx.listener(|this, _, _, cx| { this.search = None; this.query.clear(); cx.notify(); }))));

        let mut body = div().id("list").flex_1().min_h_0().overflow_y_scroll().px(px(28.)).pb(px(40.)).flex().flex_col();

        if self.search.is_some() {
            body = body.child(self.search_results(c, cx));
        } else {
            // Another device asking for the key comes first: it's waiting.
            for j in self.sync_ui.joins.clone() {
                body = body.child(self.join_card(&j, c, cx).mb(px(14.)));
            }
            // The big tile: the next few things coming up, a welcome, or all quiet.
            let events = insights::next_events(&self.people, &self.notes, &self.cols, today, NEXT_EVENTS);
            if self.people.is_empty() {
                body = body.child(self.welcome_tile(c, cx));
            } else if !events.is_empty() {
                body = body.child(self.events_tile(&events, c, cx));
            } else {
                body = body.child(self.quiet_tile(c));
            }

            let fresh = insights::still_fresh(&self.notes, &self.people, &self.cols, today);
            if !fresh.is_empty() {
                let collapsed = self.prefs.fresh_collapsed;
                body = body.child(div().id("fresh-toggle").flex().items_center().justify_between().mt(px(28.)).mb(px(12.)).cursor_pointer()
                    .child(div().flex().items_center().gap(px(6.))
                        .child(section("Still fresh", c))
                        .child(icon(if collapsed { "chevron-right" } else { "chevron-down" }, 18., c.muted)))
                    .child(caption(if collapsed { format!("{} hidden", fresh.len().min(12)) } else { "new or due soon".into() }, c.muted))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mut p = this.prefs.clone();
                        p.fresh_collapsed = !p.fresh_collapsed;
                        this.set_prefs(p, cx);
                    })));
                if !collapsed {
                    for (i, f) in fresh.iter().take(12).enumerate() {
                        body = body.child(self.fresh_card(i, f, c, cx).mb(px(8.)));
                    }
                }
            }
            if !self.people.is_empty() {
                body = body.child(div().flex().items_baseline().justify_between().mt(px(28.)).mb(px(10.))
                    .child(section("Everyone", c))
                    .child(caption(format!("{}", self.people.len()), c.muted)));
                let shown = self.shown_person().map(|p| p.id);
                for p in self.people.clone() {
                    body = body.child(self.person_row(&p, shown == Some(p.id), c, cx));
                }
            }
        }
        let _ = window;
        let pane = if narrow { div().flex_1().min_w_0() } else { div().w(px(LIST_WIDTH)).flex_none() };
        pane.h_full().flex().flex_col().child(header).child(body)
    }

    fn join_card(&self, j: &crate::sync::Join, c: &Colors, cx: &mut Context<Self>) -> Stateful<Div> {
        let (allow_id, deny_id) = (j.file_id.clone(), j.file_id.clone());
        div().id(SharedString::from(format!("join-{}", j.file_id))).flex_none().rounded(px(24.)).bg(c.card).border_1().border_color(c.line).p(px(18.)).flex().flex_col().gap(px(8.))
            .child(card_title(format!("A {} wants to join your Memi", j.device.to_lowercase()), c.ink))
            .child(body("Only allow it if it's yours and it shows this number:", c.muted))
            .child(display(j.confirm.clone(), c).text_size(px(28.)))
            .child(div().flex().gap(px(8.)).mt(px(4.))
                .child(primary_button("join-allow", "Allow", true, c).flex_1()
                    .on_click(cx.listener(move |this, _, _, cx| this.answer_join(allow_id.clone(), true, cx))))
                .child(secondary_button("join-deny", "Not mine", false, c).flex_1()
                    .on_click(cx.listener(move |this, _, _, cx| this.answer_join(deny_id.clone(), false, cx)))))
    }

    fn welcome_tile(&self, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        div().id("welcome").flex_none().rounded(px(28.)).bg(c.gift()).p(px(20.)).flex().items_center().gap(px(8.)).cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| this.open_person_editor(None, window, cx)))
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(6.))
                .child(tile_title("Hi, I'm Memi", c.ink))
                .child(body("Add someone you care about, then keep their gift ideas, plans and little things to remember in one place.", c.gift_ink()))
                .child(div().flex().mt(px(8.)).child(tag("+ Add a person", c.card, c.ink))))
            .child(img("mascot.png").size(px(96.)).flex_none())
    }

    /// The next few birthdays, dated categories and dated plans or notes, soonest first.
    fn events_tile(&self, events: &[insights::Upcoming], c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let today = dates::today();
        let mut tile = div().flex_none().rounded(px(28.)).bg(c.gift()).p(px(18.)).flex().flex_col().gap(px(4.))
            .child(div().flex().items_center().justify_between().mb(px(4.))
                .child(caption("COMING UP", c.gift_ink()))
                .child(img("mascot.png").size(px(34.)).flex_none()));
        for (i, u) in events.iter().enumerate() {
            let (pid, cid) = (u.person.id, u.collection.as_ref().map(|c| c.id));
            let detail = match (&u.note, &u.collection) {
                (Some(_), Some(col)) => format!("{} · {}", u.person.first_name(), col.name),
                (Some(_), None) => u.person.first_name().to_string(),
                (None, col) => {
                    let what = col.as_ref().map(|c| c.name.to_lowercase()).unwrap_or_else(|| "ideas".into());
                    match u.open { 0 => format!("Nothing in {what} yet"), 1 => format!("1 thing in {what}"), n => format!("{n} things in {what}") }
                }
            };
            tile = tile.child(div().id(("event", i)).flex().items_center().gap(px(12.)).p(px(8.)).rounded(px(18.)).cursor_pointer()
                .hover(|s| s.bg(Hsla { a: 0.06, ..c.ink }))
                .on_click(cx.listener(move |this, _, window, cx| match cid {
                    Some(cid) => this.open_category(pid, cid, window, cx),
                    None => { this.selected = Some(pid); this.page_open = true; cx.notify(); }
                }))
                .child(div().w(px(58.)).flex_none().flex().flex_col().items_center().justify_center().py(px(6.)).rounded(px(14.)).bg(c.card)
                    .child(label(dates::short_day_label(u.date, today), c.ink).whitespace_nowrap())
                    .child(caption(dates::in_days(u.date, today), c.muted).whitespace_nowrap()))
                .child(div().flex_1().min_w_0().flex().flex_col()
                    .child(card_title(u.title.clone(), c.ink).truncate())
                    .child(label(detail, c.gift_ink()).truncate()))
                .child(photo(&u.person, 40., 40., 12., c)));
        }
        tile
    }

    fn quiet_tile(&self, c: &Colors) -> impl IntoElement {
        let fresh = self.notes.iter().filter(|n| n.status == status::OPEN).count();
        div().flex_none().rounded(px(28.)).bg(c.plan()).p(px(20.)).flex().items_center().gap(px(8.))
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(6.))
                .child(tile_title("Nothing coming up", c.ink))
                .child(body(format!("{} open across {} {}.", fresh, self.people.len(), if self.people.len() == 1 { "person" } else { "people" }), c.plan_ink())))
            .child(img("mascot.png").size(px(84.)).flex_none())
    }



    fn fresh_card(&self, i: usize, f: &insights::FreshItem, c: &Colors, cx: &mut Context<Self>) -> Stateful<Div> {
        let today = dates::today();
        let (tag_text, bg, fg) = match f.due {
            Some(d) => (format!("Due {}", dates::short_day_label(d, today)), c.gift(), c.gift_ink()),
            None => (format!("New · {}", dates::ago_short(f.note.mentioned_at, today)), c.plan(), c.plan_ink()),
        };
        let sub = format!("{} · {}", f.person.first_name(), f.collection.as_ref().map(|c| c.name.as_str()).unwrap_or("note"));
        let (pid, cid) = (f.person.id, f.note.collection_id);
        let note = f.note.clone();
        self.note_card(("fresh", i), &note, Some(&f.person), &sub, Some((tag_text, bg, fg)), c, cx)
            .on_click(cx.listener(move |this, _, window, cx| this.open_category(pid, cid, window, cx)))
    }

    /// One note: optional photo, the text, a subtitle, a tag, and a ⋯ button when hovered.
    pub fn note_card(&self, id: impl Into<ElementId>, n: &Note, person: Option<&Person>, subtitle: &str, tag_: Option<(String, Hsla, Hsla)>, c: &Colors, cx: &mut Context<Self>) -> Stateful<Div> {
        let id: ElementId = id.into();
        let group = SharedString::from(format!("note-{}", n.id));
        let n1 = n.clone();
        let n2 = n.clone();
        div().id(id).group(group.clone()).flex_none().rounded(px(18.)).bg(c.card).border_1().border_color(c.line).p(px(12.)).flex().items_center().gap(px(12.)).cursor_pointer()
            .hover(|s| s.border_color(Hsla { a: 0.18, ..c.ink }))
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                this.menu = Some(Menu { at: e.position, kind: MenuKind::Note(n1.clone()) });
                cx.stop_propagation();
                cx.notify();
            }))
            .when_some(person, |d, p| d.child(photo(p, 44., 44., 15., c)))
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(3.))
                .child(item(n.text.clone(), c.ink).line_clamp(2))
                .child(div().flex().items_center().gap(px(8.))
                    .child(caption(subtitle.to_string(), c.muted).truncate())
                    .when_some(tag_, |d, (t, bg, fg)| d.child(tag(t, bg, fg)))))
            .child(div().id(("more", n.id as usize)).size(px(30.)).flex_none().rounded(px(10.)).flex().items_center().justify_center()
                .invisible().group_hover(group, |s| s.visible())
                .hover(|s| s.bg(c.track))
                .child(icon("ellipsis", 18., c.ink))
                .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                    this.menu = Some(Menu { at: e.position(), kind: MenuKind::Note(n2.clone()) });
                    cx.stop_propagation();
                    cx.notify();
                })))
    }

    fn person_row(&self, p: &Person, selected: bool, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let open = insights::open_count(p.id, &self.notes);
        let (id, p2) = (p.id, p.clone());
        let (drag_bg, drag_fg, accent) = (c.ink, c.bg, c.ink);
        div().id(("person", p.id as usize)).flex_none().flex().items_center().gap(px(12.)).p(px(8.)).mb(px(4.)).rounded(px(18.)).cursor_pointer()
            .when(selected, |d| d.bg(c.card).border_1().border_color(c.line))
            .when(!selected, |d| d.border_1().border_color(transparent_black()).hover(|s| s.bg(c.card)))
            // Drag onto another person to put them in that place.
            .on_drag(DragPerson { id, label: p.name.clone().into() }, move |d, _, _, cx| drag::preview(d.label.clone(), drag_bg, drag_fg, cx))
            .drag_over::<DragPerson>(move |s, d, _, _| if d.id == id { s } else { s.border_color(accent) })
            .on_drop(cx.listener(move |this, d: &DragPerson, _, cx| {
                if d.id != id { this.change(cx, |s| s.place_person(d.id, id)); }
            }))
            .on_click(cx.listener(move |this, _, _, cx| { this.selected = Some(id); this.page_open = true; cx.notify(); }))
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                this.menu = Some(Menu { at: e.position, kind: MenuKind::Person(p2.clone()) });
                cx.notify();
            }))
            .child(photo(p, 44., 44., 15., c))
            .child(div().flex_1().min_w_0().flex().flex_col()
                .child(div().flex().items_center().gap(px(6.))
                    .child(item(p.name.clone(), c.ink).truncate())
                    .when(p.pinned, |d| d.child(icon("pin", 13., c.muted))))
                .when(!p.relationship.is_empty(), |d| d.child(caption(p.relationship.clone(), c.muted).truncate())))
            .when(open > 0, |d| d.child(tag(format!("{open}"), c.track, c.muted)))
    }

    fn search_results(&self, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let q = self.query.trim().to_lowercase();
        let mut col = div().flex().flex_col().gap(px(8.));
        if q.is_empty() {
            return col.child(caption("Type a name, or something you noted down.", c.muted).mt(px(8.)));
        }
        let people: Vec<Person> = self.people.iter().filter(|p| p.name.to_lowercase().contains(&q) || p.relationship.to_lowercase().contains(&q)).cloned().collect();
        let notes: Vec<Note> = self.notes.iter().filter(|n| n.text.to_lowercase().contains(&q)).take(30).cloned().collect();
        if people.is_empty() && notes.is_empty() {
            return col.child(body(format!("Nothing matches “{}”.", self.query.trim()), c.muted).mt(px(8.)));
        }
        for p in &people {
            col = col.child(self.person_row(p, false, c, cx));
        }
        for (i, n) in notes.iter().enumerate() {
            let Some(p) = self.person(n.person_id).cloned() else { continue };
            let cname = self.collection(n.collection_id).map(|c| c.name.clone()).unwrap_or_default();
            let (pid, cid) = (p.id, n.collection_id);
            col = col.child(self.note_card(("found", i), n, Some(&p), &format!("{} · {}", p.first_name(), cname), None, c, cx)
                .on_click(cx.listener(move |this, _, window, cx| this.open_category(pid, cid, window, cx))));
        }
        col
    }

    // --- Right: the person ----------------------------------------------------------------------

    /// One person's page. `width` is the space it has; `narrow` adds a back button and tightens it.
    fn detail_pane(&mut self, c: &Colors, width: f32, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let Some(p) = self.shown_person() else {
            return div().flex_1().h_full().flex().items_center().justify_center().flex_col().gap(px(10.))
                .child(img("mascot.png").size(px(140.)))
                .child(section("Memi", c))
                .child(body("People you add show up here.", c.muted))
                .into_any_element();
        };
        let today = dates::today();
        let cats = self.categories_of(p.id);
        let open: Vec<&Note> = self.notes.iter().filter(|n| n.person_id == p.id && n.status == status::OPEN).collect();
        let mut done: Vec<Note> = self.notes.iter().filter(|n| n.person_id == p.id && n.status == status::DONE).cloned().collect();
        done.sort_by_key(|n| std::cmp::Reverse(n.done_at.unwrap_or(0)));
        let pid = p.id;
        let p_edit = p.clone();

        let pad = if narrow { 20. } else { 36. };
        let content = (width.min(860.) - 2. * pad).max(240.);
        // The photo shrinks with the page so the name and dates keep room beside it.
        let photo_w = if content < 520. { 140. } else if content < 640. { 180. } else { 220. };
        let photo_card = div().relative().flex_none()
            .child(photo(&p, photo_w, photo_w * 1.25, 28., c))
            .child(div().id("edit-person").absolute().top(px(12.)).left(px(12.)).size(px(38.)).rounded(px(13.)).bg(c.sheet).flex().items_center().justify_center().cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .child(icon("pencil", 17., c.ink))
                .on_click(cx.listener(move |this, _, window, cx| this.open_person_editor(Some(p_edit.clone()), window, cx))));

        // Dates: their birthday, and any dated categories.
        let mut dates_tile = div().rounded(px(22.)).bg(c.card).border_1().border_color(c.line).p(px(16.)).flex().flex_col().gap(px(6.))
            .child(caption("DATES", c.muted));
        match dates::next_birthday(&p, today) {
            Some(d) => {
                let turning = dates::turning_age(&p, today).map(|a| format!(" · turns {a}")).unwrap_or_default();
                dates_tile = dates_tile.child(card_title(format!("Birthday · {}{}", dates::short_date(d), turning), c.ink))
                    .child(caption(dates::in_days(d, today), c.muted));
            }
            None => {
                let p3 = p.clone();
                dates_tile = dates_tile.child(div().id("add-birthday").cursor_pointer().child(card_title("+ Add a birthday", c.tile_ink(3)))
                    .on_click(cx.listener(move |this, _, window, cx| this.open_person_editor(Some(p3.clone()), window, cx))));
            }
        }
        for cat in cats.iter().filter(|c| c.occasion == occasion::DATE) {
            if let Some(d) = dates::next_occasion(cat, &p, today) {
                dates_tile = dates_tile.child(label(format!("{} · {}", cat.name, dates::short_date(d)), c.ink));
            }
        }

        let header = div().flex().gap(px(if narrow { 18. } else { 24. })).items_start()
            .child(photo_card)
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(14.)).pt(px(6.))
                .child(div().flex().flex_col().gap(px(4.))
                    .child(display(p.name.clone(), c).line_clamp(2))
                    .when(!p.relationship.is_empty(), |d| d.child(body(p.relationship.clone(), c.muted))))
                .child(dates_tile)
                .child(caption(format!("{} open · {} done", open.len(), done.len()), c.muted)));

        // Categories as tiles, three across (two when space is short), plus a dashed "New category" tile.
        let mut grid = div().grid().grid_cols(if content < 600. { 2 } else { 3 }).gap(px(12.));
        for cat in &cats {
            grid = grid.child(self.category_tile(&p, cat, c, cx));
        }
        grid = grid.child(div().id("new-category").h(px(132.)).rounded(px(24.)).border_2().border_dashed().border_color(Hsla { a: 0.2, ..c.ink })
            .flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(c.card))
            .child(item("+ New category", c.muted))
            .on_click(cx.listener(move |this, _, window, cx| this.open_category_editor(pid, None, window, cx))));

        let mut done_list = div().flex().flex_col().gap(px(6.));
        for n in done.iter().take(40) {
            let (mark, bg, fg) = match n.outcome {
                outcome::HIT => ("check", c.hit, c.hit_ink),
                outcome::MISS => ("x", c.miss, c.miss_ink),
                _ => ("check", c.track, c.muted),
            };
            let n2 = n.clone();
            let cname = self.collection(n.collection_id).map(|c| c.name.clone()).unwrap_or_default();
            done_list = done_list.child(div().id(("done", n.id as usize)).flex().items_center().gap(px(12.)).px(px(12.)).py(px(10.)).rounded(px(16.)).bg(c.card)
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    this.menu = Some(Menu { at: e.position, kind: MenuKind::Note(n2.clone()) });
                    cx.notify();
                }))
                .child(div().size(px(26.)).flex_none().rounded(px(9.)).bg(bg).flex().items_center().justify_center().child(icon(mark, 15., fg)))
                .child(div().flex_1().min_w_0().flex().flex_col()
                    .child(label(n.text.clone(), c.ink).line_clamp(2))
                    .child(caption(format!("{} · {}", cname, n.done_at.map(|d| dates::ago(d, today)).unwrap_or_default()), c.muted))));
        }

        let back = div().flex().items_center().gap(px(10.)).mb(px(18.))
            .child(header_button("back", "chevron-left", false, c)
                .on_click(cx.listener(|this, _, _, cx| { this.page_open = false; cx.notify(); })))
            .child(label("Your people", c.muted));
        div().id("detail").flex_1().min_w_0().h_full().overflow_y_scroll()
            .child(div().max_w(px(860.)).px(px(pad)).pt(px(if narrow { 20. } else { 32. })).pb(px(60.)).flex().flex_col()
                .when(narrow, |d| d.child(back))
                .child(header)
                .child(div().flex().items_baseline().justify_between().mt(px(32.)).mb(px(12.))
                    .child(section("Categories", c))
                    .child(caption("click to add · right-click for more", c.muted)))
                .child(grid)
                .when(!done.is_empty(), |d| d
                    .child(section("Done", c).mt(px(32.)).mb(px(12.)))
                    .child(done_list)))
            .into_any_element()
    }

    fn category_tile(&self, p: &Person, cat: &Collection, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let today = dates::today();
        let open: Vec<&Note> = self.notes.iter().filter(|n| n.collection_id == cat.id && n.status == status::OPEN).collect();
        let occasion_label = dates::next_occasion(cat, p, today).map(|d| {
            if dates::days_until(d, today) > 60 { format!("for {}", dates::short_date(d)) } else { {
                let day = dates::day_label(d, today);
                format!("for {}", if day == "Today" || day == "Tomorrow" { day.to_lowercase() } else { day })
            } }
        });
        let (pid, cid) = (p.id, cat.id);
        let cat2 = cat.clone();
        let (drag_bg, drag_fg, accent) = (c.ink, c.bg, c.ink);
        div().id(("cat", cat.id as usize)).h(px(132.)).rounded(px(24.)).bg(c.tile(cat.slot)).p(px(16.)).flex().flex_col().justify_between().cursor_pointer()
            .border_2().border_color(transparent_black())
            .hover(|s| s.opacity(0.92))
            // Drag onto another category to put it in that place.
            .on_drag(DragCategory { id: cid, label: cat.name.clone().into() }, move |d, _, _, cx| drag::preview(d.label.clone(), drag_bg, drag_fg, cx))
            .drag_over::<DragCategory>(move |s, d, _, _| if d.id == cid { s } else { s.border_color(accent) })
            .on_drop(cx.listener(move |this, d: &DragCategory, _, cx| {
                if d.id != cid { this.change(cx, |s| s.place_collection(d.id, cid)); }
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.open_category(pid, cid, window, cx)))
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                this.menu = Some(Menu { at: e.position, kind: MenuKind::Category(cat2.clone()) });
                cx.notify();
            }))
            .child(div().flex().flex_col().gap(px(2.))
                .child(card_title(cat.name.clone(), c.ink).truncate())
                .when_some(occasion_label, |d, l| d.child(caption(l, c.tile_ink(cat.slot)))))
            .child(match open.first() {
                Some(first) => div().flex().flex_col().gap(px(2.))
                    .child(label(first.text.clone(), c.ink).truncate())
                    .child(caption(if open.len() > 1 { format!("+ {} more", open.len() - 1) } else { "1 open".into() }, c.tile_ink(cat.slot))),
                None => div().child(label("+ Click to add", c.tile_ink(cat.slot))),
            })
    }

    // --- Menus -------------------------------------------------------------------------------------

    fn menu_view(&mut self, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = self.menu.as_ref().unwrap();
        let at = menu.at;
        let mut list = div().w(px(232.)).rounded(px(16.)).bg(c.sheet).border_1().border_color(c.line).shadow_lg().p(px(6.)).flex().flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        let row = |id: &'static str, text: &str, ic: &str, danger: bool| {
            div().id(id).flex().items_center().gap(px(10.)).px(px(10.)).py(px(9.)).rounded(px(10.)).cursor_pointer()
                .hover(|s| s.bg(c.track))
                .child(icon(ic, 16., if danger { c.danger } else { c.ink }))
                .child(label(text.to_string(), if danger { c.danger } else { c.ink }))
        };
        match menu.kind.clone() {
            MenuKind::Note(n) if n.status == status::OPEN => {
                let (a, b, d, e, f) = (n.clone(), n.clone(), n.clone(), n.clone(), n.clone());
                list = list
                    .child(div().flex().gap(px(6.)).p(px(4.)).pb(px(8.))
                        .child(div().id("hit").flex_1().py(px(10.)).rounded(px(12.)).bg(c.hit).flex().flex_col().items_center().gap(px(2.)).cursor_pointer()
                            .child(icon("check", 18., c.hit_ink)).child(caption("It was a hit", c.hit_ink))
                            .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.mark_done(&a, outcome::HIT, cx); })))
                        .child(div().id("miss").flex_1().py(px(10.)).rounded(px(12.)).bg(c.miss).flex().flex_col().items_center().gap(px(2.)).cursor_pointer()
                            .child(icon("x", 18., c.miss_ink)).child(caption("It was a miss", c.miss_ink))
                            .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.mark_done(&b, outcome::MISS, cx); }))))
                    .child(row("done", "Mark done, decide later", "circle-check", false)
                        .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.mark_done(&d, outcome::NONE, cx); })))
                    .child(row("move", "Move to…", "chevron-right", false)
                        .on_click(cx.listener(move |this, _, _, cx| { if let Some(m) = &mut this.menu { m.kind = MenuKind::MoveNote(e.clone()); } cx.notify(); })))
                    .child(row("edit", "Edit", "pencil", false)
                        .on_click(cx.listener(move |this, _, window, cx| this.open_note_editor(f.clone(), window, cx))))
                    .child(row("delete", "Delete", "trash-2", true)
                        .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.delete_note(&n, cx); })));
            }
            MenuKind::Note(n) => {
                let a = n.clone();
                list = list
                    .child(row("reopen", "Put back in the list", "refresh-cw", false)
                        .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.reopen(&a, cx); })))
                    .child(row("delete", "Delete", "trash-2", true)
                        .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.delete_note(&n, cx); })));
            }
            MenuKind::MoveNote(n) => {
                list = list.child(caption("Move to", c.muted).px(px(10.)).py(px(6.)));
                for (i, target) in self.categories_of(n.person_id).into_iter().filter(|t| t.id != n.collection_id).enumerate() {
                    let n2 = n.clone();
                    list = list.child(div().id(("to", i)).flex().items_center().gap(px(10.)).px(px(10.)).py(px(9.)).rounded(px(10.)).cursor_pointer()
                        .hover(|s| s.bg(c.track))
                        .child(div().size(px(12.)).rounded(px(4.)).bg(c.strips[target.slot.rem_euclid(5) as usize]))
                        .child(label(target.name.clone(), c.ink))
                        .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.move_note(&n2, &target, cx); })));
                }
            }
            MenuKind::Person(p) => {
                let (a, b, d, e) = (p.clone(), p.clone(), p.clone(), p.clone());
                list = list
                    .child(row("pin", if p.pinned { "Unpin" } else { "Pin to the front" }, "pin", false)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.menu = None;
                            this.change(cx, |s| s.set_pinned(a.id, !a.pinned));
                            this.toast(format!("{} {}", if a.pinned { "Unpinned" } else { "Pinned" }, a.first_name()), None, cx);
                        })))
                    .child(row("earlier", "Move earlier", "arrow-up", false)
                        .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.change(cx, |s| s.move_person(b.id, -1)); })))
                    .child(row("later", "Move later", "arrow-down", false)
                        .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.change(cx, |s| s.move_person(d.id, 1)); })))
                    .child(row("edit", "Edit", "pencil", false)
                        .on_click(cx.listener(move |this, _, window, cx| this.open_person_editor(Some(e.clone()), window, cx))))
                    .child(row("remove", "Remove", "trash-2", true)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.menu = None;
                            this.overlay = Some(Overlay::Confirm {
                                title: format!("Remove {}?", p.first_name()),
                                body: "Their notes and photo will be deleted from this computer, and from your other devices if you sync. This can't be undone.".into(),
                                confirm: "Remove".into(),
                                action: ConfirmAction::RemovePerson(p.id),
                            });
                            cx.notify();
                        })));
            }
            MenuKind::Category(cat) => {
                let (a, b, d) = (cat.clone(), cat.clone(), cat.clone());
                list = list
                    .child(row("edit", "Edit", "pencil", false)
                        .on_click(cx.listener(move |this, _, window, cx| this.open_category_editor(a.person_id.unwrap_or(0), Some(a.clone()), window, cx))))
                    .child(row("earlier", "Move earlier", "arrow-up", false)
                        .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.change(cx, |s| s.move_collection(&b, -1)); })))
                    .child(row("later", "Move later", "arrow-down", false)
                        .on_click(cx.listener(move |this, _, _, cx| { this.menu = None; this.change(cx, |s| s.move_collection(&d, 1)); })))
                    .child(row("remove", "Remove category", "trash-2", true)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.menu = None;
                            this.overlay = Some(Overlay::Confirm {
                                title: format!("Remove {}?", cat.name),
                                body: "Anything in it moves to another of their categories, so nothing is lost.".into(),
                                confirm: "Remove".into(),
                                action: ConfirmAction::RemoveCategory(cat.clone()),
                            });
                            cx.notify();
                        })));
            }
        }
        // A click anywhere else closes the menu.
        div().absolute().inset_0().occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.menu = None; cx.notify(); }))
            .on_mouse_down(MouseButton::Right, cx.listener(|this, _, _, cx| { this.menu = None; cx.notify(); }))
            .child(deferred(anchored().position(at).snap_to_window_with_margin(px(8.)).child(list)))
    }

    // --- Theme panel -----------------------------------------------------------------------------

    fn theme_panel(&mut self, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.prefs.clone();
        let mut previews = div().flex().gap(px(8.));
        for (i, s) in SCHEMES.iter().enumerate() {
            let p = super::theme::colors(s.name, c.is_dark);
            let selected = prefs.scheme == s.name;
            let name = s.name;
            previews = previews.child(div().id(("scheme", i)).flex_1().min_w_0().flex().flex_col().items_center().gap(px(6.)).cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| { let mut p = this.prefs.clone(); p.scheme = name.into(); this.set_prefs(p, cx); }))
                .child(div().w_full().h(px(80.)).rounded(px(16.)).p(px(if selected { 3. } else { 0. }))
                    .border_2().border_color(if selected { c.ink } else { transparent_black() })
                    .child(div().size_full().rounded(px(13.)).bg(p.bg).p(px(6.)).flex().flex_col().gap(px(4.))
                        .child(div().w_full().flex_1().rounded(px(8.)).bg(p.tiles[0]))
                        .child(div().w_full().h(px(18.)).flex().gap(px(4.))
                            .child(div().flex_1().rounded(px(6.)).bg(p.tiles[1]))
                            .child(div().flex_1().rounded(px(6.)).bg(p.tiles[2])))))
                .child(caption(s.name, c.ink)));
        }
        let mode = |id: &'static str, text: &str, ic: &str, a: Appearance| {
            let selected = prefs.appearance == a;
            div().id(id).flex_1().h(px(40.)).rounded(px(13.)).flex().items_center().justify_center().gap(px(6.)).cursor_pointer()
                .when(selected, |d| d.bg(c.sheet).shadow_sm())
                .child(icon(ic, 15., c.ink)).child(label(text.to_string(), c.ink))
                .on_click(cx.listener(move |this, _, _, cx| { let mut p = this.prefs.clone(); p.appearance = a; this.set_prefs(p, cx); }))
        };
        let reminder = |id: &'static str, text: &str, days: i64| {
            let on = prefs.birthday_reminders.contains(&days);
            div().id(id).flex_1().h(px(36.)).rounded(px(12.)).flex().items_center().justify_center().cursor_pointer()
                .when(on, |d| d.bg(c.sheet).shadow_sm())
                .child(label(text.to_string(), if on { c.ink } else { c.muted }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let mut p = this.prefs.clone();
                    if p.birthday_reminders.contains(&days) { p.birthday_reminders.retain(|d| *d != days); } else { p.birthday_reminders.push(days); }
                    p.birthday_reminders.sort();
                    this.set_prefs(p, cx);
                    this.check_reminders(cx);
                }))
        };
        let sync_detail = match (&self.sync_ui.problem, &self.sync_ui.settings) {
            (Some(_), s) if s.enabled => "Needs a look".to_string(),
            (_, s) if s.enabled => s.account.clone().unwrap_or_else(|| "On".into()),
            _ => "Off".into(),
        };
        let panel = div().absolute().left(px(16.)).bottom(px(16.)).w(px(LIST_WIDTH - 32.)).rounded(px(28.)).bg(c.sheet).border_1().border_color(c.line).shadow_lg().p(px(20.)).flex().flex_col().gap(px(14.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().flex().items_baseline().justify_between()
                .child(section("Theme", c))
                .child(caption(format!("{} · {}", prefs.scheme, match prefs.appearance { Appearance::Light => "light", Appearance::Dark => "dark", Appearance::System => "following your system" }), c.muted)))
            .child(previews)
            .child(div().flex().gap(px(4.)).p(px(4.)).rounded(px(16.)).bg(c.track)
                .child(mode("light", "Light", "sun", Appearance::Light))
                .child(mode("dark", "Dark", "moon", Appearance::Dark))
                .child(mode("system", "System", "monitor", Appearance::System)))
            .child(div().flex().flex_col().gap(px(8.))
                .child(div().flex().items_baseline().justify_between()
                    .child(card_title("Birthday reminders", c.ink))
                    .child(caption(if prefs.birthday_reminders.is_empty() { "Off" } else { "While Memi is open" }, c.muted)))
                .child(div().flex().gap(px(4.)).p(px(4.)).rounded(px(16.)).bg(c.track)
                    .child(reminder("remind-0", "On the day", 0))
                    .child(reminder("remind-1", "Day before", 1))
                    .child(reminder("remind-7", "Week before", 7))))
            .child(div().id("sync-row").flex().items_center().gap(px(12.)).px(px(16.)).py(px(13.)).rounded(px(16.)).bg(c.card).border_1().border_color(c.line).cursor_pointer()
                .hover(|s| s.opacity(0.9))
                .child(icon("cloud", 19., c.ink))
                .child(card_title("Sync", c.ink))
                .child(label(sync_detail, c.muted).flex_1().truncate())
                .child(icon("chevron-right", 17., c.muted))
                .on_click(cx.listener(|this, _, _, cx| { this.theme_open = false; this.overlay = Some(Overlay::Sync); cx.notify(); })));
        // Clicking anywhere outside closes the panel.
        div().absolute().inset_0()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.theme_open = false; cx.notify(); }))
            .child(panel)
    }

    // --- Toast -------------------------------------------------------------------------------------

    fn toast_view(&mut self, c: &Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let toast = self.toast.as_ref().unwrap();
        div().absolute().bottom(px(24.)).left_0().right_0().flex().justify_center()
            .child(div().flex().items_center().gap(px(14.)).pl(px(18.)).pr(px(8.)).py(px(8.)).rounded(px(16.)).bg(c.ink).shadow_lg()
                .child(label(toast.message.clone(), c.bg).py(px(4.)))
                .when(toast.undo.is_some(), |d| d.child(div().id("undo").px(px(12.)).py(px(6.)).rounded(px(10.)).cursor_pointer()
                    .hover(|s| s.bg(Hsla { a: 0.15, ..c.bg }))
                    .child(t("Undo", 13., FontWeight::BOLD, c.bg))
                    .on_click(cx.listener(|this, _, _, cx| this.undo(cx))))))
    }
}
