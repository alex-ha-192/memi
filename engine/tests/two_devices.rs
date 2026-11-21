//! Two devices sharing one fake Google Drive, through the whole life of sync: turning it on,
//! a second device joining with the six-digit code, edits on both, deletions, and the SQLite store
//! underneath (including opening an old Android (Room) database).

use anyhow::Result;
use futures::executor::block_on;
use memi_engine::core::model::*;
use memi_engine::sync::{self, Drive, RemoteFile};
use memi_engine::{Memi, PhotoChange, Platform, Problem, SyncSettings};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// --- A Google Drive app folder, in memory ------------------------------------------------------------

#[derive(Default)]
struct Files { files: Vec<(RemoteFile, Vec<u8>)>, clock: u64 }

#[derive(Clone, Default)]
struct FakeDrive(Rc<RefCell<Files>>);

impl Drive for FakeDrive {
    async fn find(&self, name: &str) -> Result<Option<RemoteFile>> {
        Ok(self.0.borrow().files.iter().rev().find(|(f, _)| f.name == name).map(|(f, _)| f.clone()))
    }
    async fn list(&self, prefix: &str) -> Result<Vec<RemoteFile>> {
        Ok(self.0.borrow().files.iter().filter(|(f, _)| f.name.starts_with(prefix)).map(|(f, _)| f.clone()).collect())
    }
    async fn download(&self, id: &str) -> Result<Vec<u8>> {
        self.0.borrow().files.iter().find(|(f, _)| f.id == id).map(|(_, b)| b.clone()).ok_or_else(|| anyhow::anyhow!("no such file"))
    }
    async fn create(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> {
        let mut d = self.0.borrow_mut();
        d.clock += 1;
        let f = RemoteFile { id: format!("f{}", d.clock), name: name.into(), modified_time: Some(format!("t{}", d.clock)) };
        d.files.push((f.clone(), bytes.to_vec()));
        Ok(f)
    }
    async fn update(&self, id: &str, bytes: &[u8]) -> Result<RemoteFile> {
        let mut d = self.0.borrow_mut();
        d.clock += 1;
        let t = d.clock;
        let (f, b) = d.files.iter_mut().find(|(f, _)| f.id == id).ok_or_else(|| anyhow::anyhow!("no such file"))?;
        f.modified_time = Some(format!("t{t}"));
        *b = bytes.to_vec();
        Ok(f.clone())
    }
    async fn delete(&self, id: &str) -> Result<()> {
        self.0.borrow_mut().files.retain(|(f, _)| f.id != id);
        Ok(())
    }
    async fn email(&self) -> Option<String> { Some("maya@example.com".into()) }
}

// --- A device: SQLite in memory, photos and key in maps ------------------------------------------------

struct Device {
    store: memi_engine::sqlite::SqliteStore,
    photos: HashMap<String, Vec<u8>>,
    key: Option<[u8; 32]>,
    name: &'static str,
}

impl Platform for Device {
    fn save(&mut self, snap: &Snapshot) -> Result<()> { self.store.write(snap) }
    fn save_sync(&mut self, _: &SyncSettings) {}
    fn photo(&self, key: &str) -> Option<Vec<u8>> { self.photos.get(key).cloned() }
    fn save_photo(&mut self, bytes: &[u8]) -> Option<String> {
        let k = format!("photo-{}", new_uid());
        self.photos.insert(k.clone(), bytes.to_vec());
        Some(k)
    }
    fn delete_photo(&mut self, key: &str) { self.photos.remove(key); }
    fn key(&self) -> Option<[u8; 32]> { self.key }
    fn save_key(&mut self, key: &[u8; 32]) { self.key = Some(*key); }
    fn forget_key(&mut self) { self.key = None; }
    fn device_label(&self) -> String { self.name.into() }
}

type H = Arc<Mutex<Memi<Device>>>;

fn device(name: &'static str) -> H {
    let store = memi_engine::sqlite::SqliteStore::in_memory().unwrap();
    let snap = store.load();
    Arc::new(Mutex::new(Memi::new(snap, SyncSettings::default(), Device { store, photos: HashMap::new(), key: None, name })))
}

fn names(h: &H) -> Vec<String> {
    let mut v: Vec<String> = h.lock().unwrap().snapshot().notes.iter().map(|n| n.text.clone()).collect();
    v.sort();
    v
}

#[test]
fn a_second_device_joins_and_both_stay_in_step() {
    let drive = FakeDrive::default();
    let phone = device("Pixel 8");
    let laptop = device("Linux computer");

    // The phone has been used on its own for a while, with a photo.
    let maya = phone.lock().unwrap().add_person(Person::new("Maya Okafor"), Some(vec![1, 2, 3]));
    let gifts = phone.lock().unwrap().data().categories_of(&maya)[0].uid.clone();
    phone.lock().unwrap().change(|d| d.add_note(&maya, &gifts, "Blue mug", None));

    // It turns on sync: a new key, and the first upload.
    assert!(block_on(sync::connect(&phone, &drive)));
    assert!(phone.lock().unwrap().sync.enabled);
    assert_eq!(phone.lock().unwrap().sync.account.as_deref(), Some("maya@example.com"));

    // The laptop signs in to the same account. It has no key, so it asks for one.
    assert!(block_on(sync::connect(&laptop, &drive)));
    let asking = laptop.lock().unwrap().status.asking.clone().expect("asking for the key");
    assert_eq!(laptop.lock().unwrap().status.problem, Some(Problem::NeedsKey));

    // The phone sees the request, with the same code, and allows it.
    assert!(block_on(sync::sync_now(&phone, &drive)));
    let join = phone.lock().unwrap().status.joins.first().cloned().expect("a join request");
    assert_eq!(join.device, "Linux computer");
    assert_eq!(join.confirm, asking.confirm);
    block_on(sync::answer_join(&phone, &drive, &join.file_id, true)).unwrap();

    // The laptop picks up the key and everything the phone had, photo included.
    assert!(block_on(sync::check_pairing(&laptop, &drive)).unwrap());
    {
        let l = laptop.lock().unwrap();
        assert!(l.sync.enabled);
        assert!(l.status.asking.is_none() && l.status.problem.is_none());
        let p = l.data().person(&maya).cloned().unwrap();
        assert_eq!(l.photo(&p), Some(vec![1, 2, 3]));
    }
    assert_eq!(names(&laptop), vec!["Blue mug"]);
    assert!(drive.0.borrow().files.iter().all(|(f, _)| !f.name.starts_with("memi-join-")), "requests are cleaned up");

    // Both edit at once: both edits survive.
    phone.lock().unwrap().change(|d| d.add_note(&maya, &gifts, "Pottery class", None));
    laptop.lock().unwrap().change(|d| d.add_note(&maya, &gifts, "Signed novel", None));
    for _ in 0..2 {
        assert!(block_on(sync::sync_now(&phone, &drive)));
        assert!(block_on(sync::sync_now(&laptop, &drive)));
    }
    assert_eq!(names(&phone), vec!["Blue mug", "Pottery class", "Signed novel"]);
    assert_eq!(names(&laptop), names(&phone));

    // A deletion on one reaches the other.
    let mug = phone.lock().unwrap().snapshot().notes.iter().find(|n| n.text == "Blue mug").unwrap().uid.clone();
    phone.lock().unwrap().change(|d| d.remove_note(&mug));
    block_on(sync::sync_now(&phone, &drive));
    block_on(sync::sync_now(&laptop, &drive));
    assert_eq!(names(&laptop), vec!["Pottery class", "Signed novel"]);

    // A new photo on the laptop replaces the phone's.
    let p = laptop.lock().unwrap().data().person(&maya).cloned().unwrap();
    laptop.lock().unwrap().update_person(p, PhotoChange::Set(vec![9, 9]));
    block_on(sync::sync_now(&laptop, &drive));
    block_on(sync::sync_now(&phone, &drive));
    let ph = phone.lock().unwrap();
    let p = ph.data().person(&maya).cloned().unwrap();
    assert_eq!(ph.photo(&p), Some(vec![9, 9]));
    assert_eq!(ph.platform_ref().photos.len(), 1, "the old photo was removed");
}

#[test]
fn the_recovery_key_works_and_a_wrong_one_doesnt() {
    let drive = FakeDrive::default();
    let a = device("A");
    a.lock().unwrap().add_person(Person::new("Sam"), None);
    assert!(block_on(sync::connect(&a, &drive)));
    let key = a.lock().unwrap().recovery_key().unwrap();

    let b = device("B");
    block_on(sync::connect(&b, &drive));
    assert!(!block_on(sync::use_recovery_key(&b, &drive, "not a key")));
    assert!(block_on(sync::use_recovery_key(&b, &drive, &key)));
    assert_eq!(b.lock().unwrap().snapshot().people.len(), 1);
    assert!(b.lock().unwrap().sync.enabled);
}

#[test]
fn starting_over_makes_other_devices_ask_again() {
    let drive = FakeDrive::default();
    let a = device("A");
    let b = device("B");
    assert!(block_on(sync::connect(&a, &drive)));
    let key = a.lock().unwrap().recovery_key().unwrap();
    block_on(sync::connect(&b, &drive));
    assert!(block_on(sync::use_recovery_key(&b, &drive, &key)));

    assert!(block_on(sync::start_over(&a, &drive)));
    b.lock().unwrap().change(|d| { d.add_person(Person::new("Made offline")); });
    assert!(!block_on(sync::sync_now(&b, &drive)), "b can't open the new backup");
    assert_eq!(b.lock().unwrap().status.problem, Some(Problem::NeedsKey));
}

#[test]
fn the_sqlite_store_keeps_everything_across_restarts() {
    let dir = std::env::temp_dir().join(format!("memi-engine-test-{}", new_uid()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("memi.db");
    let open = || {
        let store = memi_engine::sqlite::SqliteStore::open(&path).unwrap();
        let snap = store.load();
        Memi::new(snap, SyncSettings::default(), Device { store, photos: HashMap::new(), key: None, name: "x" })
    };
    let mut m = open();
    let maya = m.add_person(Person::new("Maya"), None);
    let cols = m.data().categories_of(&maya);
    let n = m.change(|d| d.add_note(&maya, &cols[1].uid, "Pottery", None));
    m.change(|d| d.place_collection(&cols[2].uid, &cols[0].uid));
    let sam = m.add_person(Person::new("Sam"), None);
    m.remove_person(&sam);
    let before = m.snapshot().clone();
    drop(m);

    let m = open();
    let after = m.snapshot();
    let strip = |s: &Snapshot| -> Snapshot {
        let mut s = s.clone();
        for p in &mut s.people { p.id = 0; }
        for c in &mut s.collections { c.id = 0; c.person_id = None; }
        for n in &mut s.notes { n.id = 0; n.person_id = 0; n.collection_id = 0; }
        s.tombstones.sort_by(|a, b| a.uid.cmp(&b.uid));
        s
    };
    assert_eq!(strip(after), strip(&before));
    assert_eq!(after.tombstones.len(), 4, "Sam and their three categories");
    assert!(m.data().note(&n).is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_android_room_database_from_0_5_opens_with_the_shared_uids() {
    // Room version 3 (Memi 0.5): no uids, no tombstones, Room's own bookkeeping table.
    let dir = std::env::temp_dir().join(format!("memi-room-test-{}", new_uid()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("memi.db");
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(r#"
            CREATE TABLE room_master_table (id INTEGER PRIMARY KEY, identity_hash TEXT);
            CREATE TABLE people (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name TEXT NOT NULL, relationship TEXT NOT NULL, photoPath TEXT, birthdayMonth INTEGER, birthdayDay INTEGER, birthYear INTEGER, toneIndex INTEGER NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, sortOrder INTEGER NOT NULL DEFAULT 0, createdAt INTEGER NOT NULL);
            CREATE TABLE collections (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, personId INTEGER, name TEXT NOT NULL, kind TEXT NOT NULL, slot INTEGER NOT NULL, sortOrder INTEGER NOT NULL, occasion TEXT NOT NULL DEFAULT 'NONE', occasionDay INTEGER, yearly INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE notes (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, personId INTEGER NOT NULL, collectionId INTEGER NOT NULL, text TEXT NOT NULL, mentionedAt INTEGER NOT NULL, firstMentionedAt INTEGER NOT NULL, mentionCount INTEGER NOT NULL, dueAt INTEGER, status INTEGER NOT NULL, outcome INTEGER NOT NULL, doneAt INTEGER, createdAt INTEGER NOT NULL, FOREIGN KEY(personId) REFERENCES people(id) ON UPDATE NO ACTION ON DELETE CASCADE);
            INSERT INTO people (id, name, relationship, toneIndex, createdAt) VALUES (3, 'Maya', 'Sister', 0, 1700000000000);
            INSERT INTO collections (id, personId, name, kind, slot, sortOrder) VALUES (7, 3, 'Notes', 'NOTE', 2, 2);
            INSERT INTO notes (id, personId, collectionId, text, mentionedAt, firstMentionedAt, mentionCount, status, outcome, createdAt) VALUES (12, 3, 7, 'Hi', 1700000000123, 1700000000123, 1, 0, 0, 1700000000123);
            PRAGMA user_version = 3;
        "#).unwrap();
    }
    let store = memi_engine::sqlite::SqliteStore::open(&path).unwrap();
    let s = store.load();
    assert_eq!(s.people[0].uid, "a1e4984a7e39eea1336fa23191d15348", "the same uid desktop and Android give it");
    assert_eq!(s.collections[0].uid, "4fd968f882732d432b8ffa516af1ea10");
    assert_eq!(s.notes[0].person_uid, s.people[0].uid);
    assert_eq!(s.notes[0].sort_order, -1700000000123);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_first_android_databases_upgrade_like_room_did() {
    // Room version 1 (the first Android releases): no pinning, and categories shared by everyone.
    let dir = std::env::temp_dir().join(format!("memi-room1-test-{}", new_uid()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("memi.db");
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(r#"
            CREATE TABLE room_master_table (id INTEGER PRIMARY KEY, identity_hash TEXT);
            CREATE TABLE people (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name TEXT NOT NULL, relationship TEXT NOT NULL, photoPath TEXT, birthdayMonth INTEGER, birthdayDay INTEGER, birthYear INTEGER, toneIndex INTEGER NOT NULL, createdAt INTEGER NOT NULL);
            CREATE TABLE collections (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL, slot INTEGER NOT NULL, sortOrder INTEGER NOT NULL);
            CREATE TABLE notes (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, personId INTEGER NOT NULL, collectionId INTEGER NOT NULL, text TEXT NOT NULL, mentionedAt INTEGER NOT NULL, firstMentionedAt INTEGER NOT NULL, mentionCount INTEGER NOT NULL, dueAt INTEGER, status INTEGER NOT NULL, outcome INTEGER NOT NULL, doneAt INTEGER, createdAt INTEGER NOT NULL, FOREIGN KEY(personId) REFERENCES people(id) ON UPDATE NO ACTION ON DELETE CASCADE);
            INSERT INTO people (id, name, relationship, toneIndex, createdAt) VALUES (1, 'Maya', 'Sister', 0, 1700000000000), (2, 'Sam', 'Friend', 1, 1700000001000);
            INSERT INTO collections (id, name, kind, slot, sortOrder) VALUES (1, 'Gift ideas', 'GIFT', 0, 0), (2, 'Plans', 'PLAN', 1, 1), (3, 'Notes', 'NOTE', 2, 2);
            INSERT INTO notes (personId, collectionId, text, mentionedAt, firstMentionedAt, mentionCount, status, outcome, createdAt) VALUES
                (1, 1, 'Mug', 1700000000100, 1700000000100, 1, 0, 0, 1700000000100),
                (2, 3, 'Black coffee', 1700000000200, 1700000000200, 1, 0, 0, 1700000000200);
            PRAGMA user_version = 1;
        "#).unwrap();
    }
    let s = memi_engine::sqlite::SqliteStore::open(&path).unwrap().load();
    assert_eq!(s.people.len(), 2);
    assert_eq!(s.collections.len(), 6, "each person gets their own three categories");
    let maya = s.people.iter().find(|p| p.name == "Maya").unwrap();
    let gifts = s.collections.iter().find(|c| c.person_uid.as_deref() == Some(maya.uid.as_str()) && c.kind == "GIFT").unwrap();
    assert_eq!(gifts.occasion, "BIRTHDAY");
    let mug = s.notes.iter().find(|n| n.text == "Mug").unwrap();
    assert_eq!(mug.collection_uid, gifts.uid, "notes move to the person's own copy");
    let coffee = s.notes.iter().find(|n| n.text == "Black coffee").unwrap();
    let sams_notes = s.collections.iter().find(|c| c.uid == coffee.collection_uid).unwrap();
    assert_eq!((sams_notes.name.as_str(), sams_notes.person_uid.as_deref()), ("Notes", Some(coffee.person_uid.as_str())));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_save_your_key_reminder() {
    let drive = FakeDrive::default();
    let phone = device("Pixel 8");
    let now = 1_795_262_400_000;
    assert!(!phone.lock().unwrap().wants_key_nudge(now), "not before sync is on");

    // Turning sync on makes a new key nobody has written down: remind them.
    assert!(block_on(sync::connect(&phone, &drive)));
    assert!(block_on(sync::sync_now(&phone, &drive)));
    assert!(phone.lock().unwrap().wants_key_nudge(now));

    // "Not now" quiets it for two weeks, then it comes back.
    phone.lock().unwrap().snooze_key_nudge(now);
    assert!(!phone.lock().unwrap().wants_key_nudge(now + 1000));
    assert!(phone.lock().unwrap().wants_key_nudge(now + Memi::<Device>::KEY_NUDGE_SNOOZE));

    // "I've saved it" ends it for good.
    phone.lock().unwrap().key_saved();
    assert!(!phone.lock().unwrap().wants_key_nudge(now + Memi::<Device>::KEY_NUDGE_SNOOZE * 10));

    // A device that's given the key by typing it in already has it somewhere.
    let laptop = device("Linux computer");
    assert!(block_on(sync::connect(&laptop, &drive)));
    let text = phone.lock().unwrap().recovery_key().unwrap();
    assert!(block_on(sync::use_recovery_key(&laptop, &drive, &text)));
    assert!(!laptop.lock().unwrap().wants_key_nudge(now));

    // Starting over makes a new key, so the reminder is back.
    assert!(block_on(sync::start_over(&laptop, &drive)));
    assert!(laptop.lock().unwrap().wants_key_nudge(now));
}

#[test]
fn a_backup_file_merges_into_another_device() {
    let phone = device("Pixel 8");
    let maya = phone.lock().unwrap().change(|d| d.add_person(Person::new("Maya")));
    let gifts = phone.lock().unwrap().snapshot().collections.iter().find(|c| c.person_uid.as_deref() == Some(maya.as_str())).unwrap().uid.clone();
    phone.lock().unwrap().change(|d| d.add_note(&maya, &gifts, "Blue mug", None));
    phone.lock().unwrap().change(|d| d.add_note(&maya, &gifts, "Pottery class", None));
    let file = phone.lock().unwrap().export_file();

    // Another device with someone of its own: the backup adds to it, nothing is lost.
    let laptop = device("Linux computer");
    laptop.lock().unwrap().change(|d| d.add_person(Person::new("Sam")));
    let got = laptop.lock().unwrap().import_file(&file).unwrap();
    assert_eq!((got.new_people, got.new_notes, got.changed), (1, 2, true));
    assert_eq!(got.summary(), "Added 1 person and 2 notes.");
    let names: Vec<String> = laptop.lock().unwrap().snapshot().people.iter().map(|p| p.name.clone()).collect();
    assert!(names.contains(&"Maya".into()) && names.contains(&"Sam".into()), "{names:?}");

    // Importing it again changes nothing.
    let again = laptop.lock().unwrap().import_file(&file).unwrap();
    assert!(!again.changed);
    assert_eq!(again.summary(), "Everything in that backup was already here.");

    // Not a backup: a clear error, nothing touched.
    assert!(laptop.lock().unwrap().import_file(b"hello").is_err());
}

/// The whole journey over a real place: a phone turns sync on, a laptop joins, both edit, both see
/// everything. `phone_drive` and `laptop_drive` are two connections to the same place.
#[cfg(feature = "native-drive")]
fn journey<D: Drive>(phone_drive: &D, laptop_drive: &D) {
    let phone = device("Pixel 8");
    let laptop = device("Linux computer");
    let maya = phone.lock().unwrap().add_person(Person::new("Maya Okafor"), Some(vec![1, 2, 3]));
    let gifts = phone.lock().unwrap().data().categories_of(&maya)[0].uid.clone();
    phone.lock().unwrap().change(|d| d.add_note(&maya, &gifts, "Blue mug", None));

    assert!(block_on(sync::connect(&phone, phone_drive)));
    assert!(block_on(sync::connect(&laptop, laptop_drive)));
    let asking = laptop.lock().unwrap().status.asking.clone().expect("asking for the key");
    assert!(block_on(sync::sync_now(&phone, phone_drive)));
    let join = phone.lock().unwrap().status.joins.first().cloned().expect("a join request");
    assert_eq!(join.confirm, asking.confirm);
    block_on(sync::answer_join(&phone, phone_drive, &join.file_id, true)).unwrap();
    assert!(block_on(sync::check_pairing(&laptop, laptop_drive)).unwrap());
    {
        let l = laptop.lock().unwrap();
        let p = l.data().person(&maya).cloned().unwrap();
        assert_eq!(l.photo(&p), Some(vec![1, 2, 3]));
    }
    assert_eq!(names(&laptop), vec!["Blue mug"]);

    phone.lock().unwrap().change(|d| d.add_note(&maya, &gifts, "Pottery class", None));
    laptop.lock().unwrap().change(|d| d.add_note(&maya, &gifts, "Signed novel", None));
    assert!(block_on(sync::sync_now(&phone, phone_drive)));
    assert!(block_on(sync::sync_now(&laptop, laptop_drive)));
    assert!(block_on(sync::sync_now(&phone, phone_drive)));
    assert_eq!(names(&phone), names(&laptop));
    assert_eq!(names(&phone).len(), 3);
    assert!(block_on(laptop_drive.list("memi-join-")).unwrap().is_empty(), "requests are cleaned up");
}

#[cfg(feature = "native-drive")]
#[test]
fn syncing_through_a_folder() {
    use memi_engine::folder_drive::FolderDrive;
    let dir = tempfile::tempdir().unwrap();
    let shared = dir.path().join("Memi");
    journey(&FolderDrive::new(&shared), &FolderDrive::new(&shared));
    // Only Memi's own encrypted file is left, and no half-written ones.
    let left: Vec<String> = std::fs::read_dir(&shared).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(left, vec!["memi-backup.bin"]);
}

/// Against a real WebDAV server, when one is given:
/// MEMI_WEBDAV_URL=http://localhost:8800/Memi/ MEMI_WEBDAV_USER=u MEMI_WEBDAV_PASSWORD=p cargo test --all-features webdav -- --ignored
#[cfg(feature = "native-drive")]
#[test]
#[ignore]
fn syncing_through_webdav() {
    use memi_engine::webdav::WebDavDrive;
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (url, user, pass) = (env("MEMI_WEBDAV_URL"), env("MEMI_WEBDAV_USER"), env("MEMI_WEBDAV_PASSWORD"));
    let a = WebDavDrive::new(&url, &user, &pass).unwrap();
    let b = WebDavDrive::new(&url, &user, &pass).unwrap();
    journey(&a, &b);
    // Wrong password: sign in again.
    let bad = WebDavDrive::new(&url, &user, "nope").unwrap();
    let e = block_on(bad.find(sync::FILE)).unwrap_err();
    assert!(e.is::<sync::Unauthorized>(), "{e}");
}
