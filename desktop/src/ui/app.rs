//! The main window: state, data changes, sync, and the two-pane layout. The panes and the
//! sheets are drawn in panes.rs and sheets.rs.

use super::theme::{self, Colors, FONT};

use crate::model::*;
use memi_engine::edits::Edits;
use crate::sync::{self, Problem};
use crate::{dates, photos};
use memi_core::dates::Date;
use gpui::prelude::*;
use gpui::*;
use gpui_component::input::{InputEvent, InputState};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use memi_engine::sync::Handle;
use std::sync::Arc;
use std::time::Duration;

actions!(memi, [Back, NewPerson, Find]);

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Appearance { Light, Dark, System }

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub scheme: String,
    pub appearance: Appearance,
    /// Still fresh is folded away on Home.
    pub fresh_collapsed: bool,
    /// Birthday reminders: how many days before each birthday to remind you (0 = on the day).
    /// Empty means off.
    pub birthday_reminders: Vec<i64>,
    /// Reminders already shown, so each appears once: "<person uid>:<date>:<days before>".
    pub reminded: Vec<String>,
    /// The first-launch questions are answered (or were skipped: the computer already had people or sync).
    pub welcomed: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { scheme: "Bento".into(), appearance: Appearance::System, fresh_collapsed: false, birthday_reminders: vec![], reminded: vec![], welcomed: false }
    }
}

pub struct Dirs {
    pub data: PathBuf,
    pub photos: PathBuf,
}

/// Something the person can take back from the toast.
#[derive(Clone)]
pub enum Undo {
    RemoveNote(i64),
    RestoreNote(Note),
}

pub struct Toast {
    pub message: String,
    pub undo: Option<Undo>,
    id: u64,
}

pub enum ConfirmAction { RemovePerson(i64), RemoveCategory(Collection), StartSyncOver }

pub struct PersonEditor {
    pub id: Option<i64>,
    pub name: Entity<InputState>,
    pub relationship: Entity<InputState>,
    pub day: Entity<InputState>,
    pub year: Entity<InputState>,
    pub month: Option<u32>,
    pub photo: Option<String>,
    pub original_photo: Option<String>,
    /// Photos saved while this editor was open; the ones not kept are deleted.
    pub made_here: Vec<String>,
    pub busy: bool,
    pub error: Option<String>,
}

pub struct Crop {
    pub image: Arc<image::DynamicImage>,
    pub preview: Arc<gpui::Image>,
    pub zoom: f32,
    /// How far the photo has been dragged from centre, in screen pixels.
    pub offset: Point<f32>,
    pub drag_from: Option<(Point<Pixels>, Point<f32>)>,
    /// The frame's size on screen, from the last layout (used to turn the view into pixels).
    pub frame: Size<f32>,
}

pub struct NoteEditor {
    pub note: Note,
    pub text: Entity<InputState>,
    pub collection_id: i64,
    pub due: Option<Date>,
}

pub struct CategoryEditor {
    pub person_id: i64,
    pub existing: Option<Collection>,
    pub name: Entity<InputState>,
    pub occasion: String,
    pub day: Option<i64>,
    pub yearly: bool,
    /// When the calendar is open: the month it's showing.
    pub picking: Option<Date>,
}

pub enum Overlay {
    Category { person_id: i64, collection_id: i64, input: Entity<InputState> },
    EditPerson(PersonEditor),
    EditNote(NoteEditor),
    EditCategory(CategoryEditor),
    Confirm { title: String, body: String, confirm: String, action: ConfirmAction },
    Sync,
}

#[derive(Clone)]
pub enum MenuKind { Note(Note), Person(Person), Category(Collection), MoveNote(Note) }

pub struct Menu {
    pub at: Point<Pixels>,
    pub kind: MenuKind,
}

#[derive(Default)]
pub struct SyncUi {
    pub busy: bool,
    pub problem: Option<Problem>,
    pub settings: sync::Settings,
    /// While asking your other devices for the key: the request's id.
    pub pairing: Option<String>,
    pub pairing_code: Option<String>,
    /// The six digits another device shows when this one asks for the key.
    pub pairing_confirm: Option<String>,
    /// Other devices asking this one for the key.
    pub joins: Vec<sync::Join>,
    pub typing_key: Option<Entity<InputState>>,
    pub key_shown: Option<String>,
    pub message: Option<String>,
}

pub struct MemiView {
    /// The Memi (memi-engine), shared with the sync thread.
    pub memi: sync::Shared,
    pub sync: Arc<sync::Engine>,
    pub dirs: Dirs,
    pub people: Vec<Person>,
    pub cols: Vec<Collection>,
    pub notes: Vec<Note>,
    pub selected: Option<i64>,
    /// In the narrow, one-screen layout: a person's page is showing instead of the list.
    pub page_open: bool,
    pub search: Option<Entity<InputState>>,
    pub query: String,
    pub prefs: Prefs,
    pub theme_open: bool,
    pub overlay: Option<Overlay>,
    /// The person editor waits here while its photo is being cropped.
    pub crop: Option<Crop>,
    pub menu: Option<Menu>,
    pub toast: Option<Toast>,
    pub sync_ui: SyncUi,
    toast_seq: u64,
    push_seq: u64,
    pub system_dark: bool,
    _subs: Vec<Subscription>,
    /// Holds keyboard focus when no field does, so Escape and the shortcuts always work.
    pub focus: FocusHandle,
    /// The window is in front, so sync checks for changes every few seconds.
    pub active: bool,
    /// The first-launch questions, while they're showing.
    pub welcome: Option<super::welcome::Welcome>,
}

impl MemiView {
    pub fn new(memi: sync::Shared, sync: Arc<sync::Engine>, dirs: Dirs, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prefs = std::fs::read(dirs.data.join("settings.json")).ok()
            .and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let subs = vec![
            cx.observe_window_appearance(window, |this, window, cx| {
                this.system_dark = is_dark(window.appearance());
                this.apply_component_theme(cx);
                cx.notify();
            }),
            // Coming back to the window brings in changes made on your other devices.
            cx.observe_window_activation(window, |this, window, cx| {
                this.active = window.is_window_active();
                if this.active { this.pull(cx); }
            }),
        ];
        let mut this = MemiView {
            memi, sync, dirs, people: vec![], cols: vec![], notes: vec![], selected: None, page_open: false,
            search: None, query: String::new(), prefs, theme_open: false, overlay: None, crop: None,
            menu: None, toast: None, sync_ui: SyncUi::default(), toast_seq: 0, push_seq: 0,
            system_dark: is_dark(window.appearance()), _subs: subs, focus: cx.focus_handle(),
            active: window.is_window_active(), welcome: None,
        };
        this.sync_ui.settings = this.sync.snapshot();
        this.reload();
        this.settle_welcome(cx);
        this.apply_component_theme(cx);
        this.pull(cx);
        this.keep_in_step(cx);
        this.check_reminders(cx);
        this
    }

    pub fn colors(&self) -> Colors {
        let dark = match self.prefs.appearance {
            Appearance::Light => false,
            Appearance::Dark => true,
            Appearance::System => self.system_dark,
        };
        theme::colors(&self.prefs.scheme, dark)
    }

    /// Text fields come from gpui-component; give them Bento's colours and font.
    pub fn apply_component_theme(&self, cx: &mut App) {
        let c = self.colors();
        let t = gpui_component::Theme::global_mut(cx);
        t.font_family = FONT.into();
        t.font_size = px(14.5);
        t.radius = px(14.);
        t.colors.background = c.sheet;
        t.colors.foreground = c.ink;
        t.colors.muted_foreground = c.muted;
        t.colors.border = c.line;
        t.colors.input = c.line;
        t.colors.ring = c.ink;
        t.colors.caret = c.ink;
        t.colors.selection = Hsla { a: 0.25, ..c.tile_ink(3) };
    }

    pub fn set_prefs(&mut self, prefs: Prefs, cx: &mut Context<Self>) {
        self.prefs = prefs;
        if let Ok(json) = serde_json::to_vec_pretty(&self.prefs) {
            let _ = std::fs::write(self.dirs.data.join("settings.json"), json);
        }
        self.apply_component_theme(cx);
        cx.notify();
    }

    // --- Data ------------------------------------------------------------------------------

    /// Refreshes what the screens show from the engine.
    pub fn reload(&mut self) {
        (self.people, self.cols, self.notes) = self.memi.with(|m| {
            let s = m.snapshot();
            (s.people.clone(), s.collections.clone(), s.notes.clone())
        });
        if let Some(id) = self.selected {
            if !self.people.iter().any(|p| p.id == id) { self.selected = None; }
        }
    }

    /// Makes a change, refreshes, and queues an encrypted upload if sync is on.
    pub fn change<R>(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Edits) -> anyhow::Result<R>) -> Option<R> {
        let (result, save_error) = self.memi.with(|m| {
            let r = m.change(|d| f(&mut Edits { d }));
            (r, m.save_error.clone())
        });
        if let Some(e) = save_error { self.toast(e, None, cx); }
        match result {
            Ok(r) => {
                self.reload();
                self.schedule_push(cx);
                cx.notify();
                Some(r)
            }
            Err(e) => {
                self.toast(format!("That didn't work: {e}"), None, cx);
                None
            }
        }
    }

    pub fn person(&self, id: i64) -> Option<&Person> { self.people.iter().find(|p| p.id == id) }
    pub fn collection(&self, id: i64) -> Option<&Collection> { self.cols.iter().find(|c| c.id == id) }
    pub fn categories_of(&self, person_id: i64) -> Vec<Collection> {
        let mut v: Vec<Collection> = self.cols.iter().filter(|c| c.person_id == Some(person_id)).cloned().collect();
        v.sort_by_key(|c| c.sort_order);
        v
    }

    pub fn shown_person(&self) -> Option<Person> {
        self.selected.and_then(|id| self.person(id).cloned()).or_else(|| self.people.first().cloned())
    }

    // --- Toasts ------------------------------------------------------------------------------

    pub fn toast(&mut self, message: impl Into<String>, undo: Option<Undo>, cx: &mut Context<Self>) {
        self.toast_seq += 1;
        let id = self.toast_seq;
        self.toast = Some(Toast { message: message.into(), undo, id });
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            let _ = this.update(cx, |this, cx| {
                if this.toast.as_ref().map(|t| t.id) == Some(id) { this.toast = None; cx.notify(); }
            });
        }).detach();
        cx.notify();
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        let Some(undo) = self.toast.take().and_then(|t| t.undo) else { return };
        match undo {
            Undo::RemoveNote(id) => { self.change(cx, |s| s.delete_note(id)); }
            Undo::RestoreNote(n) => { self.change(cx, |s| s.restore_note(&n)); }
        }
    }

    // --- Note actions --------------------------------------------------------------------------

    pub fn add_note(&mut self, person_id: i64, collection_id: i64, text: &str, cx: &mut Context<Self>) {
        let text = text.trim();
        if text.is_empty() { return; }
        let name = self.collection(collection_id).map(|c| c.name.clone()).unwrap_or_default();
        if let Some(id) = self.change(cx, |s| s.add_note(person_id, collection_id, text, None)) {
            self.toast(format!("Added to {name}"), Some(Undo::RemoveNote(id)), cx);
        }
    }

    pub fn mark_done(&mut self, n: &Note, outcome_: i32, cx: &mut Context<Self>) {
        let before = n.clone();
        self.change(cx, |s| s.mark_done(n, outcome_));
        let msg = match outcome_ { outcome::HIT => "Marked as a hit", outcome::MISS => "Marked as a miss", _ => "Marked done" };
        self.toast(msg, Some(Undo::RestoreNote(before)), cx);
    }

    pub fn reopen(&mut self, n: &Note, cx: &mut Context<Self>) { self.change(cx, |s| s.reopen(n)); }

    pub fn move_note(&mut self, n: &Note, to: &Collection, cx: &mut Context<Self>) {
        let before = n.clone();
        self.change(cx, |s| s.update_note(&Note { collection_id: to.id, ..n.clone() }));
        self.toast(format!("Moved to {}", to.name), Some(Undo::RestoreNote(before)), cx);
    }

    pub fn delete_note(&mut self, n: &Note, cx: &mut Context<Self>) {
        let before = n.clone();
        self.change(cx, |s| s.delete_note(n.id));
        self.toast("Deleted", Some(Undo::RestoreNote(before)), cx);
    }

    // --- People ------------------------------------------------------------------------------

    pub fn open_person_editor(&mut self, p: Option<Person>, window: &mut Window, cx: &mut Context<Self>) {
        let field = |v: String, ph: &str, window: &mut Window, cx: &mut Context<Self>| {
            let ph = ph.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(ph).default_value(v))
        };
        let p0 = p.clone().unwrap_or_else(|| Person::new(""));
        let editor = PersonEditor {
            id: p.as_ref().map(|p| p.id),
            name: field(p0.name.clone(), "Maya Okafor", window, cx),
            relationship: field(p0.relationship.clone(), "Sister, friend from uni…", window, cx),
            day: field(p0.birthday_day.map(|d| d.to_string()).unwrap_or_default(), "22", window, cx),
            year: field(p0.birth_year.map(|y| y.to_string()).unwrap_or_default(), "1995", window, cx),
            month: p0.birthday_month,
            photo: p0.photo_path.clone(),
            original_photo: p0.photo_path.clone(),
            made_here: vec![],
            busy: false,
            error: None,
        };
        editor.name.update(cx, |s, cx| s.focus(window, cx));
        self.menu = None;
        self.overlay = Some(Overlay::EditPerson(editor));
        cx.notify();
    }

    /// Closes the person editor, cleaning up photos made during it that weren't kept.
    pub fn close_person_editor(&mut self, kept: Option<&str>, cx: &mut Context<Self>) {
        if let Some(Overlay::EditPerson(e)) = self.overlay.take() {
            for p in e.made_here.iter().filter(|p| Some(p.as_str()) != kept) {
                photos::delete(Some(p), &self.dirs.photos);
            }
            if let (Some(orig), Some(kept)) = (e.original_photo.as_deref(), kept) {
                if orig != kept { photos::delete(Some(orig), &self.dirs.photos); }
            }
            if kept.is_none() && e.original_photo.is_some() {
                // Removing the photo: the old file goes once the change is saved (see save_person).
            }
        }
        self.crop = None;
        cx.notify();
    }

    pub fn save_person(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::EditPerson(e)) = &self.overlay else { return };
        let name = e.name.read(cx).value().trim().to_string();
        if name.is_empty() { return; }
        let relationship = e.relationship.read(cx).value().trim().to_string();
        let day: Option<u32> = e.day.read(cx).value().trim().parse().ok().filter(|d| (1..=31).contains(d));
        let year: Option<i32> = e.year.read(cx).value().trim().parse().ok().filter(|y| (1900..=dates::year(dates::today())).contains(y));
        let has_birthday = day.is_some() && e.month.is_some();
        let base = e.id.and_then(|id| self.person(id).cloned()).unwrap_or_else(|| Person::new(&name));
        let person = Person {
            name, relationship, photo_path: e.photo.clone(),
            birthday_day: if has_birthday { day } else { None },
            birthday_month: if has_birthday { e.month } else { None },
            birth_year: if has_birthday { year } else { None },
            ..base
        };
        let removed_photo = e.original_photo.clone().filter(|o| person.photo_path.as_deref() != Some(o));
        let kept = person.photo_path.clone();
        let adding = e.id.is_none();
        if let Some(id) = self.change(cx, |s| if adding { s.add_person(&person) } else { s.update_person(&person).map(|_| person.id) }) {
            if adding { self.selected = Some(id); self.page_open = true; }
            if let Some(old) = removed_photo { photos::delete(Some(&old), &self.dirs.photos); }
            self.close_person_editor(kept.as_deref(), cx);
        }
    }

    pub fn remove_person(&mut self, id: i64, cx: &mut Context<Self>) {
        let photo = self.person(id).and_then(|p| p.photo_path.clone());
        let name = self.person(id).map(|p| p.first_name().to_string()).unwrap_or_default();
        if self.change(cx, |s| s.delete_person(id)).is_some() {
            photos::delete(photo.as_deref(), &self.dirs.photos);
            if self.selected == Some(id) { self.selected = None; self.page_open = false; }
            self.toast(format!("Removed {name}"), None, cx);
        }
    }

    pub fn pick_photo(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::EditPerson(e)) = &mut self.overlay { e.busy = true; e.error = None; }
        cx.notify();
        let dialog = rfd::AsyncFileDialog::new()
            .set_title("Choose a photo")
            .add_filter("Photos", &["jpg", "jpeg", "png", "webp", "bmp", "gif"]);
        cx.spawn(async move |this, cx| {
            let file = dialog.pick_file().await;
            let loaded = match file {
                None => None,
                Some(f) => {
                    let path = f.path().to_path_buf();
                    Some(cx.background_executor().spawn(async move { load_crop_source(&path) }).await)
                }
            };
            let _ = this.update(cx, |this, cx| {
                if let Some(Overlay::EditPerson(e)) = &mut this.overlay { e.busy = false; }
                match loaded {
                    Some(Ok(crop)) => this.crop = Some(crop),
                    Some(Err(err)) => if let Some(Overlay::EditPerson(e)) = &mut this.overlay { e.error = Some(err.to_string()); },
                    None => {}
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn recrop(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::EditPerson(e)) = &self.overlay else { return };
        let Some(path) = e.photo.clone() else { return };
        match load_crop_source(std::path::Path::new(&path)) {
            Ok(c) => self.crop = Some(c),
            Err(err) => if let Some(Overlay::EditPerson(e)) = &mut self.overlay { e.error = Some(err.to_string()); },
        }
        cx.notify();
    }

    pub fn finish_crop(&mut self, cx: &mut Context<Self>) {
        let Some(crop) = self.crop.take() else { return };
        let (x, y, w, h) = crop.area();
        let dir = self.dirs.photos.clone();
        let img = crop.image.clone();
        if let Some(Overlay::EditPerson(e)) = &mut self.overlay { e.busy = true; }
        cx.notify();
        cx.spawn(async move |this, cx| {
            let saved = cx.background_executor().spawn(async move { photos::save_crop(&img, x, y, w, h, &dir) }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(Overlay::EditPerson(e)) = &mut this.overlay {
                    e.busy = false;
                    match saved {
                        Ok(p) => {
                            let p = p.to_string_lossy().into_owned();
                            e.made_here.push(p.clone());
                            e.photo = Some(p);
                        }
                        Err(err) => e.error = Some(format!("Couldn't save the photo: {err}")),
                    }
                }
                cx.notify();
            });
        }).detach();
    }

    // --- Categories --------------------------------------------------------------------------

    pub fn open_category(&mut self, person_id: i64, collection_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.collection(collection_id).map(|c| c.name.clone()).unwrap_or_default();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(format!("Add to {name}…")));
        let sub = cx.subscribe_in(&input, window, move |this, input, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = input.read(cx).value().to_string();
                if !text.trim().is_empty() {
                    this.add_note(person_id, collection_id, &text, cx);
                    input.update(cx, |s, cx| s.set_value("", window, cx));
                }
            }
        });
        self._subs.push(sub);
        input.update(cx, |s, cx| s.focus(window, cx));
        self.selected = Some(person_id);
        self.page_open = true;
        self.search = None;
        self.menu = None;
        self.theme_open = false;
        self.overlay = Some(Overlay::Category { person_id, collection_id, input });
        cx.notify();
    }

    pub fn open_category_editor(&mut self, person_id: i64, existing: Option<Collection>, window: &mut Window, cx: &mut Context<Self>) {
        let value = existing.as_ref().map(|c| c.name.clone()).unwrap_or_default();
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Anniversary, Trip to Lisbon, Books…").default_value(value));
        name.update(cx, |s, cx| s.focus(window, cx));
        self.menu = None;
        self.overlay = Some(Overlay::EditCategory(CategoryEditor {
            person_id,
            occasion: existing.as_ref().map(|c| c.occasion.clone()).unwrap_or_else(|| occasion::NONE.into()),
            day: existing.as_ref().and_then(|c| c.occasion_day),
            yearly: existing.as_ref().map(|c| c.yearly).unwrap_or(false),
            existing, name, picking: None,
        }));
        cx.notify();
    }

    pub fn save_category(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::EditCategory(e)) = &self.overlay else { return };
        let name = e.name.read(cx).value().trim().to_string();
        if name.is_empty() || (e.occasion == occasion::DATE && e.day.is_none()) { return; }
        let (occ, day, yearly) = (e.occasion.clone(), if e.occasion == occasion::DATE { e.day } else { None }, e.occasion == occasion::DATE && e.yearly);
        let pid = e.person_id;
        let existing = e.existing.clone();
        let ok = match existing {
            None => self.change(cx, |s| s.add_collection(pid, &name, &occ, day, yearly).map(|_| ())),
            Some(c) => self.change(cx, |s| s.update_collection(&Collection { name: name.clone(), occasion: occ, occasion_day: day, yearly, ..c })),
        };
        if ok.is_some() { self.overlay = None; cx.notify(); }
    }

    // --- Edit note ---------------------------------------------------------------------------

    pub fn open_note_editor(&mut self, n: Note, window: &mut Window, cx: &mut Context<Self>) {
        let text = cx.new(|cx| InputState::new(window, cx).default_value(n.text.clone()));
        text.update(cx, |s, cx| s.focus(window, cx));
        self.menu = None;
        self.overlay = Some(Overlay::EditNote(NoteEditor { collection_id: n.collection_id, due: n.due_at.map(dates::to_date), note: n, text }));
        cx.notify();
    }

    pub fn save_note(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::EditNote(e)) = &self.overlay else { return };
        let text = e.text.read(cx).value().trim().to_string();
        if text.is_empty() { return; }
        let n = Note { text, collection_id: e.collection_id, due_at: e.due.map(dates::end_of_day), ..e.note.clone() };
        if self.change(cx, |s| s.update_note(&n)).is_some() { self.overlay = None; cx.notify(); }
    }

    // --- Search ------------------------------------------------------------------------------

    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search people and notes"));
            let sub = cx.subscribe_in(&input, window, |this, input, ev: &InputEvent, _, cx| {
                if let InputEvent::Change = ev { this.query = input.read(cx).value().to_string(); cx.notify(); }
            });
            self._subs.push(sub);
            self.search = Some(input);
            self.query.clear();
        }
        if let Some(s) = &self.search { s.update(cx, |s, cx| s.focus(window, cx)); }
        self.theme_open = false;
        cx.notify();
    }

    pub fn back(&mut self, _: &Back, _window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
        } else if self.crop.take().is_some() {
        } else if self.overlay.is_some() {
            if matches!(self.overlay, Some(Overlay::EditPerson(_))) { self.close_person_editor(None, cx); } else { self.overlay = None; }
        } else if self.theme_open {
            self.theme_open = false;
        } else if self.search.take().is_some() {
            self.query.clear();
        } else if self.page_open {
            // Narrow layout: from a person's page back to the list.
            self.page_open = false;
        } else if self.welcome == Some(super::welcome::Welcome::Choice) {
            self.welcome = Some(super::welcome::Welcome::Question);
        }
        cx.notify();
    }

    // --- Sync --------------------------------------------------------------------------------

    /// Runs a blocking sync step off the main thread, then refreshes everything.
    pub fn run_sync(&mut self, cx: &mut Context<Self>, op: impl FnOnce(&sync::Engine) -> anyhow::Result<Option<Problem>> + Send + 'static) {
        if self.sync_ui.busy { return; }
        self.sync_ui.busy = true;
        self.sync_ui.message = None;
        cx.notify();
        let engine = self.sync.clone();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { op(&engine) }).await;
            let _ = this.update(cx, |this, cx| {
                this.sync_ui.busy = false;
                match result {
                    Ok(problem) => this.sync_ui.problem = problem,
                    Err(e) => this.sync_ui.problem = Some(Problem::Failed(e.to_string())),
                }
                this.sync_ui.settings = this.sync.snapshot();
                this.sync_ui.joins = this.sync.joins();
                if let Some(a) = this.sync.asking() { this.show_asking(a, cx); }
                this.reload();
                cx.notify();
            });
        }).detach();
    }

    /// After a change: upload a few seconds later, batching quick edits together.
    fn schedule_push(&mut self, cx: &mut Context<Self>) {
        if !self.sync.snapshot().enabled { return; }
        self.push_seq += 1;
        let seq = self.push_seq;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = this.update(cx, |this, cx| {
                if this.push_seq == seq { this.run_sync(cx, |e| e.sync()); }
            });
        }).detach();
    }

    pub fn pull(&mut self, cx: &mut Context<Self>) {
        if self.sync.snapshot().enabled { self.run_sync(cx, |e| e.sync()); }
    }

    /// While the window is in front, checks for changes from your other devices every few seconds,
    /// so edits show up almost as they're made.
    fn keep_in_step(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(LIVE_SECONDS)).await;
            let Ok(()) = this.update(cx, |this, cx| {
                if this.active && !this.sync_ui.busy && this.sync_ui.problem.is_none() { this.pull(cx); }
                this.check_reminders(cx);
            }) else { return };
        }).detach();
    }

    /// Shows any birthday reminders that are due and haven't been shown yet (while the app is open).
    pub fn check_reminders(&mut self, cx: &mut Context<Self>) {
        if self.prefs.birthday_reminders.is_empty() { return; }
        let due = crate::insights::reminders_due(&self.people, &self.notes, &self.cols, dates::today(), &self.prefs.birthday_reminders);
        let new: Vec<_> = due.into_iter().filter(|r| !self.prefs.reminded.contains(&r.key)).collect();
        if new.is_empty() { return; }
        let mut prefs = self.prefs.clone();
        for r in new {
            prefs.reminded.push(r.key);
            let (title, body) = (r.title, r.body);
            // Showing a notification can wait on the system, so it happens off the UI thread.
            std::thread::spawn(move || {
                let mut n = notify_rust::Notification::new();
                n.appname("Memi").summary(&title).body(&body);
                #[cfg(target_os = "windows")]
                n.app_id("app.yourmemi.YourMemi");
                #[cfg(target_os = "linux")]
                n.icon("app.yourmemi.YourMemi");
                let _ = n.show();
            });
        }
        // Only recent ones matter; each key includes its date.
        let extra = prefs.reminded.len().saturating_sub(100);
        prefs.reminded.drain(..extra);
        self.set_prefs(prefs, cx);
    }

    /// Hands the key to a device that asked for it (see memi_core::pairing).
    pub fn answer_join(&mut self, file_id: String, allow: bool, cx: &mut Context<Self>) {
        self.sync_ui.joins.retain(|j| j.file_id != file_id);
        let device_allowed = allow;
        self.run_sync(cx, move |e| {
            if device_allowed { e.approve_join(&file_id)?; } else { e.decline_join(&file_id)?; }
            Ok(None)
        });
        if allow { self.toast("Sent. The other device will finish setting up in a few seconds.", None, cx); }
    }

    /// Asks your other devices for the key: a request they show with a six-digit code, and a QR
    /// code a phone can scan. Waits up to fifteen minutes for an answer.
    pub fn start_pairing(&mut self, cx: &mut Context<Self>) {
        let engine = self.sync.clone();
        cx.spawn(async move |this, cx| {
            let asking = cx.background_executor().spawn(async move { engine.ask_for_key() }).await;
            let _ = this.update(cx, |this, cx| match asking {
                Ok(Some(a)) => this.show_asking(a, cx),
                Ok(None) => {}
                Err(e) => { this.sync_ui.problem = Some(Problem::Failed(e.to_string())); cx.notify(); }
            });
        }).detach();
    }

    /// Shows the code another device will match, and waits for its answer.
    fn show_asking(&mut self, a: sync::Asking, cx: &mut Context<Self>) {
        if self.sync_ui.pairing.as_deref() == Some(a.id.as_str()) { return; }
        self.sync_ui.pairing = Some(a.id.clone());
        self.sync_ui.pairing_code = Some(a.code.clone());
        self.sync_ui.pairing_confirm = Some(a.confirm.clone());
        cx.notify();
        let engine = self.sync.clone();
        let id = a.id;
        cx.spawn(async move |this, cx| {
            for _ in 0..300 {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let still_waiting = this.update(cx, |this, _| this.sync_ui.pairing.as_deref() == Some(id.as_str())).unwrap_or(false);
                if !still_waiting { return; }
                let e = engine.clone();
                let done = cx.background_executor().spawn(async move { e.check_pairing() }).await;
                if let Ok(true) = done {
                    let _ = this.update(cx, |this, cx| {
                        this.sync_ui.pairing = None;
                        this.sync_ui.pairing_code = None;
                        this.sync_ui.pairing_confirm = None;
                        this.sync_ui.problem = None;
                        this.sync_ui.settings = this.sync.snapshot();
                        this.reload();
                        this.toast("This computer is syncing now", None, cx);
                    });
                    return;
                }
            }
            let _ = this.update(cx, |this, cx| {
                if this.sync_ui.pairing.as_deref() == Some(id.as_str()) { this.stop_pairing(); cx.notify(); }
            });
        }).detach();
    }

    pub fn stop_pairing(&mut self) {
        if self.sync_ui.pairing.take().is_some() {
            let engine = self.sync.clone();
            std::thread::spawn(move || engine.cancel_asking());
        }
        self.sync_ui.pairing_code = None;
        self.sync_ui.pairing_confirm = None;
    }
}

/// How often the window checks for changes from your other devices while it's in front.
const LIVE_SECONDS: u64 = 10;

fn is_dark(a: WindowAppearance) -> bool {
    matches!(a, WindowAppearance::Dark | WindowAppearance::VibrantDark)
}

/// Loads a photo for cropping, with a small preview the window can draw.
pub fn load_crop_source(path: &std::path::Path) -> anyhow::Result<Crop> {
    let img = photos::load_for_cropping(path)?;
    let mut png = Vec::new();
    img.thumbnail(1400, 1400).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)?;
    Ok(Crop {
        image: Arc::new(img),
        preview: Arc::new(gpui::Image::from_bytes(ImageFormat::Png, png)),
        zoom: 1.0,
        offset: Point::default(),
        drag_from: None,
        frame: Size { width: 0., height: 0. },
    })
}

impl Crop {
    /// The photo's scale on screen at zoom 1: just big enough to cover the frame.
    pub fn base_scale(&self) -> f32 {
        let (w, h) = (self.image.width() as f32, self.image.height() as f32);
        (self.frame.width / w).max(self.frame.height / h)
    }

    pub fn clamp(&mut self) {
        let s = self.base_scale() * self.zoom;
        let mx = ((self.image.width() as f32 * s - self.frame.width) / 2.).max(0.);
        let my = ((self.image.height() as f32 * s - self.frame.height) / 2.).max(0.);
        self.offset.x = self.offset.x.clamp(-mx, mx);
        self.offset.y = self.offset.y.clamp(-my, my);
    }

    /// The area inside the frame, in the photo's own pixels.
    pub fn area(&self) -> (u32, u32, u32, u32) {
        let s = self.base_scale() * self.zoom;
        if s <= 0. { return (0, 0, self.image.width(), self.image.height()); }
        let (w, h) = (self.image.width() as f32, self.image.height() as f32);
        let left = (w * s - self.frame.width) / 2. - self.offset.x;
        let top = (h * s - self.frame.height) / 2. - self.offset.y;
        let x = (left / s).round().max(0.) as u32;
        let y = (top / s).round().max(0.) as u32;
        let cw = (self.frame.width / s).round().max(1.) as u32;
        let ch = (self.frame.height / s).round().max(1.) as u32;
        (x, y, cw, ch)
    }
}
