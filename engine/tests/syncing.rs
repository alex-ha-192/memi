//! Sync between devices, including the ways it goes wrong: clashing edits, deletions, no
//! connection, an expired sign-in, a full drive, a damaged or foreign copy, and every way a new
//! device gets the key.

mod common;
use common::*;
use memi_engine::core::model::*;
use memi_engine::core::pairing;
use memi_engine::sync::{self, Drive};
use memi_engine::{PhotoChange, Problem};

fn sync_now(h: &H, c: &Cloud) -> bool { block(sync::sync_now(h, c)) }

/// The text of the one note both devices have, after they've both synced.
fn both_say(a: &H, b: &H, c: &Cloud) -> (Vec<String>, Vec<String>) {
    sync_now(a, c);
    sync_now(b, c);
    sync_now(a, c);
    (notes(a), notes(b))
}

// --- Clashes -------------------------------------------------------------------------------------------

#[test]
fn when_both_edit_the_same_note_the_later_edit_wins_everywhere() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    let (maya, gifts) = someone(&a, "Maya");
    let n = on(&a, |m| m.change(|d| d.add_note(&maya, &gifts, "Blue mug", None)));
    two_in_step(&a, &b, &c);
    let edit = |h: &H, text: &str| on(h, |m| { let mut x = m.data().note(&n).cloned().unwrap(); x.text = text.into(); m.change(|d| d.update_note(x)) });
    edit(&a, "Blue mug, the big one");
    tick();
    edit(&b, "Green mug");
    let (x, y) = both_say(&a, &b, &c);
    assert_eq!((x, y), (vec!["Green mug".to_string()], vec!["Green mug".to_string()]));
}

#[test]
fn a_deletion_beats_an_older_edit_but_not_a_newer_one() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    let (maya, gifts) = someone(&a, "Maya");
    let older = on(&a, |m| m.change(|d| d.add_note(&maya, &gifts, "Older", None)));
    let newer = on(&a, |m| m.change(|d| d.add_note(&maya, &gifts, "Newer", None)));
    two_in_step(&a, &b, &c);
    let edit = |h: &H, uid: &str, text: &str| on(h, |m| { let mut x = m.data().note(uid).cloned().unwrap(); x.text = text.into(); m.change(|d| d.update_note(x)) });
    // "Older": edited on the laptop, then deleted on the phone → gone.
    edit(&b, &older, "Older, edited");
    tick();
    on(&a, |m| m.change(|d| d.remove_note(&older)));
    // "Newer": deleted on the phone, then edited on the laptop → kept, with the edit.
    on(&a, |m| m.change(|d| d.remove_note(&newer)));
    tick();
    edit(&b, &newer, "Newer, edited after");
    let (x, y) = both_say(&a, &b, &c);
    assert_eq!(x, ["Newer, edited after"]);
    assert_eq!(y, x);
}

#[test]
fn someone_removed_on_one_device_goes_from_the_others() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    let (maya, gifts) = someone(&a, "Maya");
    someone(&a, "Sam");
    on(&a, |m| m.change(|d| d.add_note(&maya, &gifts, "Blue mug", None)));
    two_in_step(&a, &b, &c);
    assert_eq!(people(&b), ["Maya", "Sam"]);
    on(&b, |m| m.remove_person(&maya));
    both_say(&a, &b, &c);
    assert_eq!((people(&a), notes(&a).len()), (vec!["Sam".to_string()], 0));
}

#[test]
fn photos_travel_and_changes_to_them_follow() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    let maya = on(&a, |m| m.add_person(Person::new("Maya"), Some(vec![1, 2, 3])));
    two_in_step(&a, &b, &c);
    let photo = |h: &H| on(h, |m| { let p = m.data().person(&maya).cloned().unwrap(); m.photo(&p) });
    assert_eq!(photo(&b), Some(vec![1, 2, 3]));
    tick();
    let p = on(&b, |m| m.data().person(&maya).cloned().unwrap());
    on(&b, |m| m.update_person(p, PhotoChange::Set(vec![7, 7])));
    both_say(&a, &b, &c);
    assert_eq!(photo(&a), Some(vec![7, 7]));
    assert_eq!(on(&a, |m| m.platform_ref().photos.len()), 1, "the old photo file was cleared away");
    tick();
    let p = on(&a, |m| m.data().person(&maya).cloned().unwrap());
    on(&a, |m| m.update_person(p, PhotoChange::Remove));
    both_say(&a, &b, &c);
    assert_eq!((photo(&b), on(&b, |m| m.platform_ref().photos.len())), (None, 0));
}

// --- Doing no more than needed -----------------------------------------------------------------------

#[test]
fn nothing_is_uploaded_or_downloaded_when_nothing_changed() {
    let (a, c) = (phone("Pixel 8"), Cloud::default());
    someone(&a, "Maya");
    assert!(block(sync::connect(&a, &c)));
    let (up, down) = (c.uploads.get(), c.downloads.get());
    for _ in 0..5 { assert!(sync_now(&a, &c)); }
    assert_eq!((c.uploads.get(), c.downloads.get()), (up, down));
    someone(&a, "Sam");
    assert!(sync_now(&a, &c));
    assert_eq!(c.uploads.get(), up + 1, "one change, one upload");
    assert_eq!(c.downloads.get(), down, "and no need to download our own copy");
}

// --- When things go wrong ----------------------------------------------------------------------------------

#[test]
fn with_no_connection_changes_wait_and_go_later() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    two_in_step(&a, &b, &c);
    c.offline.set(true);
    let (maya, gifts) = someone(&a, "Maya");
    on(&a, |m| m.change(|d| d.add_note(&maya, &gifts, "Written offline", None)));
    assert!(!sync_now(&a, &c));
    on(&a, |m| {
        assert!(m.sync.dirty, "still waiting to go");
        assert!(matches!(&m.status.problem, Some(Problem::Failed(e)) if e.contains("No connection")), "{:?}", m.status.problem);
        assert!(!m.status.busy, "and not stuck busy");
    });
    c.offline.set(false);
    assert!(sync_now(&a, &c));
    assert!(on(&a, |m| !m.sync.dirty && m.status.problem.is_none()));
    sync_now(&b, &c);
    assert_eq!(notes(&b), ["Written offline"]);
}

#[test]
fn an_expired_sign_in_asks_to_sign_in_again() {
    let (a, c) = (phone("Pixel 8"), Cloud::default());
    assert!(block(sync::connect(&a, &c)));
    c.signed_out.set(true);
    assert!(!sync_now(&a, &c));
    assert_eq!(on(&a, |m| m.status.problem.clone()), Some(Problem::NeedsSignIn));
    c.signed_out.set(false);
    assert!(sync_now(&a, &c));
    assert_eq!(on(&a, |m| m.status.problem.clone()), None);
}

#[test]
fn a_full_drive_keeps_the_change_queued() {
    let (a, c) = (phone("Pixel 8"), Cloud::default());
    assert!(block(sync::connect(&a, &c)));
    c.read_only.set(true);
    someone(&a, "Maya");
    assert!(!sync_now(&a, &c));
    assert!(on(&a, |m| m.sync.dirty), "the upload failed, so it's still to do");
    c.read_only.set(false);
    assert!(sync_now(&a, &c));
    assert!(on(&a, |m| !m.sync.dirty));
}

#[test]
fn a_damaged_copy_changes_nothing_here() {
    let (a, c) = (phone("Pixel 8"), Cloud::default());
    someone(&a, "Maya");
    assert!(block(sync::connect(&a, &c)));
    c.overwrite(sync::FILE, b"not a Memi backup at all");
    assert!(!sync_now(&a, &c));
    assert_eq!(people(&a), ["Maya"]);
    assert!(on(&a, |m| m.status.problem.is_some()));
}

#[test]
fn a_copy_made_with_another_key_needs_that_key() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    two_in_step(&a, &b, &c);
    // The laptop starts over (new key); the phone still has the old one.
    assert!(block(sync::start_over(&b, &c)));
    assert!(!sync_now(&a, &c));
    assert_eq!(on(&a, |m| m.status.problem.clone()), Some(Problem::NeedsKey));
    // The old recovery key doesn't open it; the new one does.
    let old = on(&a, |m| m.recovery_key()).unwrap();
    assert!(!block(sync::use_recovery_key(&a, &c, &old)));
    assert!(!block(sync::use_recovery_key(&a, &c, "not-a-key")));
    let new = on(&b, |m| m.recovery_key()).unwrap();
    assert_ne!(old, new);
    assert!(block(sync::use_recovery_key(&a, &c, &new)));
    assert!(sync_now(&a, &c));
}

#[test]
fn if_the_copy_disappears_the_next_sync_puts_it_back() {
    let (a, c) = (phone("Pixel 8"), Cloud::default());
    someone(&a, "Maya");
    assert!(block(sync::connect(&a, &c)));
    let id = block(c.find(sync::FILE)).unwrap().unwrap().id;
    block(c.delete(&id)).unwrap();
    assert!(sync_now(&a, &c));
    assert!(c.names().contains(&sync::FILE.to_string()));
}

#[test]
fn only_one_sync_runs_at_a_time() {
    let (a, c) = (phone("Pixel 8"), Cloud::default());
    assert!(block(sync::connect(&a, &c)));
    on(&a, |m| m.set_status(|s| s.busy = true));
    assert!(!sync_now(&a, &c), "a second one doesn't start");
    on(&a, |m| m.set_status(|s| s.busy = false));
    assert!(sync_now(&a, &c));
}

#[test]
fn sync_off_means_no_contact() {
    let (a, c) = (phone("Pixel 8"), Cloud::default());
    someone(&a, "Maya");
    assert!(!sync_now(&a, &c));
    assert!(c.names().is_empty());
    assert_eq!(c.uploads.get() + c.downloads.get(), 0);
}

// --- Turning it off and on -------------------------------------------------------------------------------

#[test]
fn turning_off_and_on_again_keeps_the_key_unless_asked() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    two_in_step(&a, &b, &c);
    on(&a, |m| m.turn_off_sync(false));
    assert!(on(&a, |m| !m.sync.enabled && m.key().is_some()));
    assert!(block(sync::connect(&a, &c)));
    assert!(on(&a, |m| m.sync.enabled && m.status.asking.is_none()), "back on without asking anyone");
    on(&a, |m| m.turn_off_sync(true));
    assert!(on(&a, |m| m.key().is_none()));
    assert!(block(sync::connect(&a, &c)));
    assert_eq!(on(&a, |m| m.status.problem.clone()), Some(Problem::NeedsKey), "without the key it has to ask");
}

// --- Bringing in a new device --------------------------------------------------------------------------

#[test]
fn turning_it_down_leaves_the_new_device_waiting() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    assert!(block(sync::connect(&a, &c)));
    assert!(block(sync::connect(&b, &c)));
    sync_now(&a, &c);
    let join = on(&a, |m| m.status.joins.first().cloned()).expect("a request");
    block(sync::answer_join(&a, &c, &join.file_id, false)).unwrap();
    assert!(on(&a, |m| m.status.joins.is_empty()));
    assert!(!block(sync::check_pairing(&b, &c)).unwrap());
    assert!(on(&b, |m| m.key().is_none() && !m.sync.enabled));
    assert!(c.names().iter().all(|n| !n.starts_with(pairing::JOIN_PREFIX)), "the request is gone: {:?}", c.names());
}

#[test]
fn a_phone_can_send_the_key_to_a_scanned_code() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    let (maya, gifts) = someone(&a, "Maya");
    on(&a, |m| m.change(|d| d.add_note(&maya, &gifts, "Blue mug", None)));
    assert!(block(sync::connect(&a, &c)));
    assert!(block(sync::connect(&b, &c)));
    let code = on(&b, |m| m.status.asking.clone()).unwrap().code;
    block(sync::send_key_to(&a, &c, &code)).unwrap();
    assert!(block(sync::check_pairing(&b, &c)).unwrap());
    assert_eq!(notes(&b), ["Blue mug"]);
    // Codes that aren't Memi's are turned away with a helpful message.
    let e = block(sync::send_key_to(&a, &c, "https://example.com")).unwrap_err();
    assert!(e.to_string().contains("isn't a Memi code"), "{e}");
    // A device without sync can't send anything.
    let lonely = phone("Tablet");
    assert!(block(sync::send_key_to(&lonely, &c, &code)).is_err());
}

#[test]
fn cancelling_takes_the_request_away() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    assert!(block(sync::connect(&a, &c)));
    assert!(block(sync::connect(&b, &c)));
    assert!(c.names().iter().any(|n| n.starts_with(pairing::JOIN_PREFIX)));
    block(sync::cancel_asking(&b, &c));
    assert!(on(&b, |m| m.status.asking.is_none()));
    assert!(c.names().iter().all(|n| !n.starts_with(pairing::JOIN_PREFIX)));
    sync_now(&a, &c);
    assert!(on(&a, |m| m.status.joins.is_empty()));
}

#[test]
fn old_and_broken_requests_are_cleared_away() {
    let (a, c) = (phone("Pixel 8"), Cloud::default());
    assert!(block(sync::connect(&a, &c)));
    let old = pairing::Offer::new();
    let an_hour_ago = now_millis() - 60 * 60_000;
    block(c.create(&old.join_file_name(), &old.join_request("Old phone", an_hour_ago))).unwrap();
    block(c.create(&format!("{}broken.json", pairing::JOIN_PREFIX), b"{nonsense")).unwrap();
    sync_now(&a, &c);
    assert!(on(&a, |m| m.status.joins.is_empty()), "neither is offered");
    assert!(c.names().iter().all(|n| !n.starts_with(pairing::JOIN_PREFIX)), "both deleted: {:?}", c.names());
}

#[test]
fn starting_over_replaces_the_copy_with_whats_here() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    someone(&b, "Only on the laptop");
    two_in_step(&a, &b, &c);
    someone(&a, "Only on the phone");
    on(&b, |m| m.turn_off_sync(true));
    assert!(block(sync::start_over(&a, &c)));
    let files: Vec<String> = c.names().into_iter().filter(|n| n == sync::FILE).collect();
    assert_eq!(files.len(), 1, "one copy");
    assert!(on(&a, |m| m.sync.enabled && !m.sync.key_saved), "a new key, not yet written down");
}

// --- Backup files --------------------------------------------------------------------------------------

#[test]
fn backup_files_from_newer_versions_are_refused_clearly() {
    let a = phone("Pixel 8");
    let e = on(&a, |m| m.import_file(br#"{"format": 99, "people": []}"#)).unwrap_err();
    assert!(e.to_string().contains("newer version of Memi"), "{e}");
    let e = on(&a, |m| m.import_file(b"\x89PNG")).unwrap_err();
    assert!(e.to_string().contains("isn't valid"), "{e}");
}

#[test]
fn a_backup_file_carries_photos_and_queues_them_for_sync() {
    let (a, b, c) = (phone("Pixel 8"), phone("Laptop"), Cloud::default());
    on(&a, |m| m.add_person(Person::new("Maya"), Some(vec![5, 6])));
    let file = on(&a, |m| m.export_file());
    assert!(block(sync::connect(&b, &c)));
    let got = on(&b, |m| m.import_file(&file)).unwrap();
    assert_eq!((got.new_people, got.changed), (1, true));
    let p = on(&b, |m| m.snapshot().people[0].clone());
    assert_eq!(on(&b, |m| m.photo(&p)), Some(vec![5, 6]));
    assert!(on(&b, |m| m.sync.dirty), "the import goes to sync too");
    assert!(sync_now(&b, &c));
}
