//! Every edit the apps can make, through the engine as they make them: by uid (web) and by row id
//! (desktop and Android, through Edits), with saving, photos, ordering, deletions and undo.

mod common;
use common::*;
use memi_engine::core::model::*;
use memi_engine::edits::Edits;
use memi_engine::{Data, PhotoChange};

// --- People ------------------------------------------------------------------------------------------

#[test]
fn a_new_person_gets_three_starter_categories() {
    let h = phone("Pixel 8");
    let (maya, _) = someone(&h, "Maya");
    on(&h, |m| {
        let cats = m.data().categories_of(&maya);
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Gift ideas", "Plans", "Notes"]);
        assert_eq!(cats[0].kind, kind::GIFT);
        assert_eq!(cats[0].occasion, occasion::BIRTHDAY, "gift ideas build up to the birthday");
        assert_eq!(cats.iter().map(|c| c.slot).collect::<Vec<_>>(), [0, 1, 2], "one colour each");
        let p = m.data().person(&maya).unwrap();
        assert!(p.id > 0 && p.created_at > 0 && p.updated_at == p.created_at);
        assert!(cats.iter().all(|c| c.person_id == Some(p.id)), "categories point at their person's id");
    });
}

#[test]
fn people_take_turns_at_tones() {
    let h = phone("Pixel 8");
    for i in 0..9 { someone(&h, &format!("P{i}")); }
    let tones: Vec<i32> = on(&h, |m| m.snapshot().people.iter().map(|p| p.tone_index).collect());
    assert_eq!(tones, [0, 1, 2, 3, 4, 5, 6, 7, 0]);
}

#[test]
fn pinned_people_come_first_and_keep_their_own_order() {
    let h = phone("Pixel 8");
    let (a, _) = someone(&h, "Ann");
    let (_b, _) = someone(&h, "Ben");
    let (c, _) = someone(&h, "Cal");
    on(&h, |m| m.change(|d| { d.set_pinned(&c, true); d.set_pinned(&a, true); }));
    assert_eq!(people(&h), ["Cal", "Ann", "Ben"], "pinned in the order they were pinned, then the rest");
    on(&h, |m| m.change(|d| d.set_pinned(&c, false)));
    assert_eq!(people(&h), ["Ann", "Ben", "Cal"], "unpinned goes to the end of the rest");
}

#[test]
fn dragging_people_into_place() {
    let h = phone("Pixel 8");
    for n in ["Ann", "Ben", "Cal", "Dee"] { someone(&h, n); }
    let id = |name: &str| on(&h, |m| m.snapshot().people.iter().find(|p| p.name == name).unwrap().id);
    let (ann, cal, dee) = (id("Ann"), id("Cal"), id("Dee"));
    // Down: Ann onto Cal puts her after him.
    on(&h, |m| m.change(|d| Edits { d }.place_person(ann, cal))).unwrap();
    assert_eq!(people(&h), ["Ben", "Cal", "Ann", "Dee"]);
    // Up: Dee onto Ben puts her before him.
    let ben = id("Ben");
    on(&h, |m| m.change(|d| Edits { d }.place_person(dee, ben))).unwrap();
    assert_eq!(people(&h), ["Dee", "Ben", "Cal", "Ann"]);
    // One step at a time, and nothing happens past the ends.
    on(&h, |m| m.change(|d| Edits { d }.move_person(dee, -1))).unwrap();
    assert_eq!(people(&h)[0], "Dee");
    on(&h, |m| m.change(|d| Edits { d }.move_person(dee, 1))).unwrap();
    assert_eq!(people(&h), ["Ben", "Dee", "Cal", "Ann"]);
    // Dropping a pinned person among the unpinned unpins them, and the reverse.
    on(&h, |m| m.change(|d| Edits { d }.set_pinned(cal, true))).unwrap();
    on(&h, |m| m.change(|d| Edits { d }.place_person(ann, cal))).unwrap();
    let pinned: Vec<String> = on(&h, |m| m.snapshot().people.iter().filter(|p| p.pinned).map(|p| p.name.clone()).collect());
    assert_eq!(pinned.len(), 2, "Ann joined the pinned group: {pinned:?}");
}

#[test]
fn editing_someone_keeps_their_id_and_stamps_the_time() {
    let h = phone("Pixel 8");
    let (maya, _) = someone(&h, "Maya");
    let before = on(&h, |m| m.data().person(&maya).cloned().unwrap());
    tick();
    let mut p = before.clone();
    p.name = "Maya Okafor".into();
    p.relationship = "Sister".into();
    p.birthday_month = Some(11);
    p.birthday_day = Some(26);
    on(&h, |m| m.update_person(p, PhotoChange::Keep));
    let after = on(&h, |m| m.data().person(&maya).cloned().unwrap());
    assert_eq!((after.id, after.uid.as_str()), (before.id, before.uid.as_str()));
    assert_eq!((after.name.as_str(), after.relationship.as_str(), after.birthday_month), ("Maya Okafor", "Sister", Some(11)));
    assert!(after.updated_at > before.updated_at);
}

#[test]
fn photos_are_kept_replaced_and_removed() {
    let h = phone("Pixel 8");
    let maya = on(&h, |m| m.add_person(Person::new("Maya"), Some(vec![1, 2, 3])));
    let photo = |h: &H| on(h, |m| { let p = m.data().person(&maya).cloned().unwrap(); m.photo(&p) });
    let stored = |h: &H| on(h, |m| m.platform_ref().photos.len());
    assert_eq!(photo(&h), Some(vec![1, 2, 3]));

    let p = on(&h, |m| m.data().person(&maya).cloned().unwrap());
    on(&h, |m| m.update_person(p.clone(), PhotoChange::Keep));
    assert_eq!((photo(&h), stored(&h)), (Some(vec![1, 2, 3]), 1), "kept");

    on(&h, |m| m.update_person(p.clone(), PhotoChange::Set(vec![9, 9])));
    assert_eq!((photo(&h), stored(&h)), (Some(vec![9, 9]), 1), "replaced, and the old file is gone");

    let p = on(&h, |m| m.data().person(&maya).cloned().unwrap());
    on(&h, |m| m.update_person(p, PhotoChange::Remove));
    assert_eq!((photo(&h), stored(&h)), (None, 0), "removed");

    let sam = on(&h, |m| m.add_person(Person::new("Sam"), Some(vec![4])));
    on(&h, |m| m.remove_person(&sam));
    assert_eq!(stored(&h), 0, "removing someone removes their photo");
}

#[test]
fn removing_someone_removes_their_categories_and_notes_and_remembers_it() {
    let h = phone("Pixel 8");
    let (maya, gifts) = someone(&h, "Maya");
    let (sam, sam_gifts) = someone(&h, "Sam");
    let n = on(&h, |m| m.change(|d| d.add_note(&maya, &gifts, "Blue mug", None)));
    on(&h, |m| m.change(|d| d.add_note(&sam, &sam_gifts, "Board game", None)));
    let cats: Vec<String> = on(&h, |m| m.data().categories_of(&maya).iter().map(|c| c.uid.clone()).collect());
    on(&h, |m| m.remove_person(&maya));
    on(&h, |m| {
        let s = m.snapshot();
        assert_eq!(s.people.len(), 1);
        assert_eq!(s.collections.len(), 3, "only Sam's categories are left");
        let gone: Vec<&str> = s.tombstones.iter().map(|t| t.uid.as_str()).collect();
        for u in cats.iter().chain([&maya, &n]) { assert!(gone.contains(&u.as_str()), "{u} is recorded as deleted, so other devices delete it too"); }
    });
    assert_eq!(notes(&h), ["Board game"]);
}


// --- Notes -------------------------------------------------------------------------------------------

#[test]
fn notes_are_trimmed_open_and_newest_first() {
    let h = phone("Pixel 8");
    let (maya, gifts) = someone(&h, "Maya");
    on(&h, |m| m.change(|d| d.add_note(&maya, &gifts, "  Blue mug \n", None)));
    tick();
    on(&h, |m| m.change(|d| d.add_note(&maya, &gifts, "Pottery class", Some(1_800_000_000_000))));
    on(&h, |m| {
        let open = m.data().open_in(&gifts);
        assert_eq!(open.iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), ["Pottery class", "Blue mug"]);
        assert_eq!(open[0].due_at, Some(1_800_000_000_000));
        assert!(open.iter().all(|n| n.status == status::OPEN && n.mention_count == 1 && n.outcome == outcome::NONE));
    });
}

#[test]
fn done_hit_miss_and_reopen() {
    let h = phone("Pixel 8");
    let (maya, gifts) = someone(&h, "Maya");
    let n = on(&h, |m| m.change(|d| d.add_note(&maya, &gifts, "Blue mug", None)));
    on(&h, |m| m.change(|d| d.mark_done(&n, outcome::HIT)));
    let done = on(&h, |m| m.data().note(&n).cloned().unwrap());
    assert_eq!((done.status, done.outcome), (status::DONE, outcome::HIT));
    let first_done = done.done_at.expect("done time");
    tick();
    on(&h, |m| m.change(|d| d.mark_done(&n, outcome::MISS)));
    let again = on(&h, |m| m.data().note(&n).cloned().unwrap());
    assert_eq!((again.outcome, again.done_at), (outcome::MISS, Some(first_done)), "changing the outcome keeps when it was done");
    assert!(on(&h, |m| m.data().open_in(&gifts).is_empty()));
    on(&h, |m| m.change(|d| d.reopen(&n)));
    let open = on(&h, |m| m.data().note(&n).cloned().unwrap());
    assert_eq!((open.status, open.outcome, open.done_at), (status::OPEN, outcome::NONE, None));
}

#[test]
fn undo_brings_a_deleted_note_back_as_it_was() {
    let h = phone("Pixel 8");
    let (maya, gifts) = someone(&h, "Maya");
    let n = on(&h, |m| m.change(|d| d.add_note(&maya, &gifts, "Blue mug", None)));
    let before = on(&h, |m| m.data().note(&n).cloned().unwrap());
    on(&h, |m| m.change(|d| d.remove_note(&n)));
    assert!(on(&h, |m| m.data().note(&n).is_none() && m.snapshot().tombstones.iter().any(|t| t.uid == n)));
    tick();
    on(&h, |m| m.change(|d| d.restore_note(before.clone())));
    let back = on(&h, |m| m.data().note(&n).cloned().unwrap());
    assert_eq!((back.uid.as_str(), back.id, back.text.as_str(), back.created_at), (n.as_str(), before.id, "Blue mug", before.created_at));
    assert!(back.updated_at > before.updated_at, "the undo is a newer change, so it beats the deletion elsewhere");
    assert!(on(&h, |m| !m.snapshot().tombstones.iter().any(|t| t.uid == n)), "and it's no longer recorded as deleted");
}

#[test]
fn moving_and_ordering_notes() {
    let h = phone("Pixel 8");
    let (maya, gifts) = someone(&h, "Maya");
    let plans = on(&h, |m| m.data().categories_of(&maya)[1].uid.clone());
    let mut ids = vec![];
    for t in ["A", "B", "C"] { ids.push(on(&h, |m| m.change(|d| d.add_note(&maya, &gifts, t, None)))); tick(); }
    let order = |h: &H, c: &str| on(h, |m| m.data().open_in(c).iter().map(|n| n.text.clone()).collect::<Vec<_>>());
    assert_eq!(order(&h, &gifts), ["C", "B", "A"]);
    on(&h, |m| m.change(|d| d.place_note(&ids[0], &ids[2])));
    assert_eq!(order(&h, &gifts), ["A", "C", "B"], "A dragged onto C takes its place");
    on(&h, |m| m.change(|d| d.place_note(&ids[1], &ids[0])));
    assert_eq!(order(&h, &gifts), ["B", "A", "C"]);
    on(&h, |m| m.change(|d| d.move_note(&ids[2], &plans)));
    assert_eq!((order(&h, &gifts), order(&h, &plans)), (vec!["B".to_string(), "A".into()], vec!["C".to_string()]));
    // Dragging onto a note in another category does nothing.
    on(&h, |m| m.change(|d| d.place_note(&ids[0], &ids[2])));
    assert_eq!(order(&h, &gifts), ["B", "A"]);
}

// --- Categories --------------------------------------------------------------------------------------

#[test]
fn categories_get_free_colours_and_dates() {
    let h = phone("Pixel 8");
    let (maya, _) = someone(&h, "Maya");
    let c = on(&h, |m| m.change(|d| d.add_collection(&maya, "  Anniversary ", occasion::DATE, Some(20_000), true)));
    let made = on(&h, |m| m.data().collection(&c).cloned().unwrap());
    assert_eq!((made.name.as_str(), made.kind.as_str(), made.slot, made.occasion_day, made.yearly), ("Anniversary", kind::CUSTOM, 3, Some(20_000), true));
    let more: Vec<i32> = (0..3).map(|i| on(&h, |m| { let u = m.change(|d| d.add_collection(&maya, &format!("X{i}"), occasion::NONE, None, false)); m.data().collection(&u).unwrap().slot })).collect();
    assert!(more.iter().all(|s| (0..SLOT_COUNT).contains(s)), "{more:?}");
    let mut renamed = made.clone();
    renamed.name = " Wedding anniversary ".into();
    on(&h, |m| m.change(|d| d.update_collection(renamed)));
    assert_eq!(on(&h, |m| m.data().collection(&c).unwrap().name.clone()), "Wedding anniversary");
}

#[test]
fn removing_a_category_keeps_its_notes() {
    let h = phone("Pixel 8");
    let (maya, gifts) = someone(&h, "Maya");
    on(&h, |m| m.change(|d| d.add_note(&maya, &gifts, "Blue mug", None)));
    on(&h, |m| m.change(|d| d.remove_collection(&gifts)));
    let note_cat = on(&h, |m| m.snapshot().notes[0].collection_uid.clone());
    let kind = on(&h, |m| m.data().collection(&note_cat).unwrap().kind.clone());
    assert_eq!(kind, kind::NOTE, "it moved to Notes");
    assert_eq!(notes(&h), ["Blue mug"]);
    // With no categories left at all, its notes go with the last one.
    let rest: Vec<String> = on(&h, |m| m.data().categories_of(&maya).iter().map(|c| c.uid.clone()).collect());
    for c in rest { on(&h, |m| m.change(|d| d.remove_collection(&c))); }
    assert!(notes(&h).is_empty());
    assert!(on(&h, |m| m.data().categories_of(&maya).is_empty()));
}

#[test]
fn reordering_categories() {
    let h = phone("Pixel 8");
    let (maya, _) = someone(&h, "Maya");
    let names = |h: &H| on(h, |m| m.data().categories_of(&maya).iter().map(|c| c.name.clone()).collect::<Vec<_>>());
    let cats: Vec<Collection> = on(&h, |m| m.data().categories_of(&maya));
    on(&h, |m| m.change(|d| d.place_collection(&cats[2].uid, &cats[0].uid)));
    assert_eq!(names(&h), ["Notes", "Gift ideas", "Plans"]);
    let notes_cat = on(&h, |m| m.data().categories_of(&maya)[0].clone());
    on(&h, |m| m.change(|d| Edits { d }.move_collection(&notes_cat, 1))).unwrap();
    assert_eq!(names(&h), ["Gift ideas", "Notes", "Plans"]);
    // A category from someone else isn't moved among Maya's.
    let (_, sam_gifts) = someone(&h, "Sam");
    on(&h, |m| m.change(|d| d.place_collection(&sam_gifts, &cats[0].uid)));
    assert_eq!(names(&h), ["Gift ideas", "Notes", "Plans"]);
}

// --- By row id (desktop and Android) -------------------------------------------------------------------

#[test]
fn ids_stay_the_same_and_missing_ones_are_errors() {
    let h = phone("Pixel 8");
    let maya = on(&h, |m| m.change(|d| Edits { d }.add_person(&Person::new("Maya")))).unwrap();
    let cats: Vec<i64> = on(&h, |m| m.snapshot().collections.iter().map(|c| c.id).collect());
    let n = on(&h, |m| m.change(|d| Edits { d }.add_note(maya, cats[0], "Blue mug", None))).unwrap();
    for _ in 0..3 { someone(&h, "Someone"); }
    on(&h, |m| m.change(|d| Edits { d }.set_pinned(maya, true))).unwrap();
    assert_eq!(on(&h, |m| m.data().person_by_id(maya).unwrap().name.clone()), "Maya", "ids don't move when the order does");
    assert_eq!(on(&h, |m| m.snapshot().notes.iter().find(|x| x.id == n).unwrap().text.clone()), "Blue mug");

    let gone = 9_999;
    let e = on(&h, |m| m.change(|d| Edits { d }.delete_note(gone))).unwrap_err();
    assert_eq!(e.to_string(), "That's no longer here.");
    assert!(on(&h, |m| m.change(|d| Edits { d }.delete_person(gone))).is_err());
    assert!(on(&h, |m| m.change(|d| Edits { d }.place_person(maya, gone))).is_err());
    assert!(on(&h, |m| m.change(|d| Edits { d }.add_note(maya, gone, "x", None))).is_err());
    assert_eq!(notes(&h), ["Blue mug"], "a failed edit changes nothing");
}

#[test]
fn loaded_ids_are_kept_and_new_ones_come_after() {
    let mut d = Data::new(Snapshot::default());
    let a = d.add_person(Person::new("Ann"));
    let snap = d.snap.clone();
    let a_id = d.person(&a).unwrap().id;
    let mut again = Data::new(snap);
    assert_eq!(again.person(&a).unwrap().id, a_id, "a reopened store keeps its row ids");
    let b = again.add_person(Person::new("Ben"));
    let max_before = again.snap.collections.iter().map(|c| c.id).chain([a_id]).max().unwrap();
    assert!(again.person(&b).unwrap().id > a_id && again.snap.collections.iter().all(|c| c.id <= max_before + 3));
}

#[test]
fn orphans_are_tidied_away() {
    let mut d = Data::new(Snapshot::default());
    let maya = d.add_person(Person::new("Maya"));
    let gifts = d.categories_of(&maya)[0].uid.clone();
    d.add_note(&maya, &gifts, "Blue mug", None);
    // A copy where the person is missing (as after a bad merge): their things go too.
    let mut s = d.snap.clone();
    s.people.clear();
    let tidy = Data::new(s);
    assert!(tidy.snap.collections.is_empty() && tidy.snap.notes.is_empty());
}

// --- Saving and the sync queue ----------------------------------------------------------------------------

#[test]
fn every_change_is_saved_and_moves_the_version_on() {
    let h = phone("Pixel 8");
    let v0 = on(&h, |m| m.version);
    let (maya, gifts) = someone(&h, "Maya");
    on(&h, |m| m.change(|d| d.add_note(&maya, &gifts, "Blue mug", None)));
    on(&h, |m| {
        assert!(m.version >= v0 + 2);
        assert_eq!(m.platform_ref().saves, 2);
        assert_eq!(m.platform_ref().saved.as_ref().unwrap().notes.len(), 1, "the saved copy is the latest");
        assert!(!m.sync.dirty, "sync is off, so nothing is queued");
        assert!(m.save_error.is_none());
    });
}

#[test]
fn a_failed_save_is_reported_and_cleared_by_the_next_good_one() {
    let h = phone("Pixel 8");
    on(&h, |m| m.platform().fail_saves = true);
    let (maya, gifts) = someone(&h, "Maya");
    let e = on(&h, |m| m.save_error.clone()).expect("reported");
    assert!(e.starts_with("Couldn't save on this device"), "{e}");
    assert_eq!(people(&h), ["Maya"], "the change still shows, so nothing typed is lost");
    on(&h, |m| m.platform().fail_saves = false);
    on(&h, |m| m.change(|d| d.add_note(&maya, &gifts, "Blue mug", None)));
    assert!(on(&h, |m| m.save_error.is_none()));
}

#[test]
fn with_sync_on_a_change_queues_an_upload() {
    let h = phone("Pixel 8");
    let cloud = Cloud::default();
    assert!(block(memi_engine::sync::connect(&h, &cloud)));
    assert!(on(&h, |m| !m.sync.dirty));
    someone(&h, "Maya");
    on(&h, |m| {
        assert!(m.sync.dirty);
        assert_eq!(m.platform_ref().sync_saved.as_ref().map(|s| s.dirty), Some(true), "and the queue is saved too");
    });
}
