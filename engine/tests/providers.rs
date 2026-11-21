//! The places sync can keep its files, on their own: a folder, WebDAV's answers and addresses,
//! and the SQLite store the native apps save into.

mod common;
use common::*;
use memi_engine::core::model::*;
use memi_engine::dav;
use memi_engine::folder_drive::FolderDrive;
use memi_engine::sync::Drive;

#[test]
fn a_folder_lists_newest_first_and_only_what_was_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let f = FolderDrive::new(dir.path().join("Memi"));
    block(f.create("memi-join-a.json", b"a")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    block(f.create("memi-join-b.json", b"b")).unwrap();
    block(f.create("memi-backup.bin", b"x")).unwrap();
    std::fs::write(dir.path().join("Memi").join("someone-elses.txt"), "hi").unwrap();
    let joins: Vec<String> = block(f.list("memi-join-")).unwrap().into_iter().map(|r| r.name).collect();
    assert_eq!(joins, ["memi-join-b.json", "memi-join-a.json"]);
    assert_eq!(block(f.download("memi-join-a.json")).unwrap(), b"a");
    assert!(block(f.find("missing.bin")).unwrap().is_none());
}

#[test]
fn a_folder_notices_every_rewrite_and_leaves_nothing_half_written() {
    let dir = tempfile::tempdir().unwrap();
    let f = FolderDrive::new(dir.path());
    let one = block(f.create("memi-backup.bin", b"first")).unwrap();
    let two = block(f.update(&one.id, b"second!")).unwrap();
    let three = block(f.update(&one.id, b"third!!")).unwrap();
    assert_ne!(one.modified_time, two.modified_time);
    assert_ne!(two.modified_time, three.modified_time, "same size, same moment: still a new version");
    let names: Vec<String> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["memi-backup.bin"], "no .part files left");
    block(f.delete(&one.id)).unwrap();
    block(f.delete(&one.id)).unwrap();
    assert!(block(f.find("memi-backup.bin")).unwrap().is_none(), "deleting twice is fine");
}

#[test]
fn a_folder_refuses_names_that_reach_outside_it() {
    let dir = tempfile::tempdir().unwrap();
    let f = FolderDrive::new(dir.path().join("inner"));
    for bad in ["../escape.bin", "a/b.bin", "..", "", "a\\b"] {
        assert!(block(f.create(bad, b"x")).is_err(), "{bad:?}");
        assert!(block(f.download(bad)).is_err(), "{bad:?}");
    }
    assert!(!dir.path().join("escape.bin").exists());
}

#[test]
fn webdav_answers_in_other_shapes() {
    // No ETag, an encoded space, an empty folder answer, and a server using no prefix at all.
    let xml = r#"<multistatus xmlns="DAV:"><response><href>/dav/Memi/my%20file.bin</href><propstat><prop>
        <getlastmodified>Sat, 21 Nov 2026 10:00:00 GMT</getlastmodified><resourcetype/></prop></propstat></response></multistatus>"#;
    let e = dav::parse_propfind(xml);
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].name.as_str(), e[0].etag.as_deref(), e[0].folder), ("my file.bin", None, false));
    assert!(e[0].modified > 0);
    assert!(dav::parse_propfind("").is_empty());
    assert!(dav::parse_propfind("<d:multistatus xmlns:d=\"DAV:\"></d:multistatus>").is_empty());
    let bad_date = r#"<d:response><d:href>/x.bin</d:href><d:getlastmodified>yesterday</d:getlastmodified></d:response>"#;
    assert_eq!(dav::parse_propfind(bad_date)[0].modified, 0, "an unreadable date counts as oldest, not an error");
}

#[test]
fn webdav_addresses() {
    assert_eq!(dav::normalise_url("https://cloud.example.com").unwrap(), "https://cloud.example.com/");
    assert!(dav::normalise_url("HTTPS://Cloud.Example.com/dav/").is_ok());
    for local in ["http://localhost:8080/dav", "http://127.0.0.1/dav", "http://10.0.0.2/dav", "http://nas.local/dav"] {
        assert!(dav::normalise_url(local).is_ok(), "{local}");
    }
    for bad in ["ftp://example.com", "http://example.com/dav", "https://", "", "javascript:alert(1)"] {
        assert!(dav::normalise_url(bad).is_err(), "{bad}");
    }
    assert_eq!(dav::percent_encode("a b/c"), "a%20b%2Fc");
}

#[test]
fn the_store_keeps_deletions_and_photos_across_restarts() {
    use memi_engine::sqlite::SqliteStore;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memi.db");
    let mut d = memi_engine::Data::new(Snapshot::default());
    let maya = d.add_person(Person { photo_path: Some("/photos/maya.jpg".into()), ..Person::new("Maya") });
    let gifts = d.categories_of(&maya)[0].uid.clone();
    let n = d.add_note(&maya, &gifts, "Blue mug", Some(1_800_000_000_000));
    d.mark_done(&n, outcome::HIT);
    let sam = d.add_person(Person::new("Sam"));
    d.remove_person(&sam);
    {
        let mut s = SqliteStore::open(&path).unwrap();
        s.write(&d.snap).unwrap();
    }
    let s = SqliteStore::open(&path).unwrap();
    let back = s.load();
    assert_eq!(back.people.len(), 1);
    assert_eq!(back.people[0].photo_path.as_deref(), Some("/photos/maya.jpg"));
    let note = back.notes.iter().find(|x| x.uid == n).unwrap();
    assert_eq!((note.status, note.outcome, note.due_at), (status::DONE, outcome::HIT, Some(1_800_000_000_000)));
    assert!(back.tombstones.iter().any(|t| t.uid == sam), "deletions are remembered, so sync can pass them on");
    // Writing the same thing again is harmless.
    let mut s = s;
    s.write(&back).unwrap();
    assert_eq!(SqliteStore::open(&path).unwrap().load().notes.len(), 1);
}
