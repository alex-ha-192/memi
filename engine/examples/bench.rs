//! How long edits, saving and sync's encryption take with a large Memi.
//! cargo run --release --all-features --example bench
use memi_engine::core::model::*;
use memi_engine::{Data, sqlite::SqliteStore};
use std::time::Instant;

fn main() {
    let mut d = Data::new(Snapshot::default());
    let mut cats = vec![];
    for i in 0..300 {
        let p = d.add_person(Person::new(&format!("Person {i}")));
        cats.push((p.clone(), d.categories_of(&p)[0].uid.clone()));
    }
    for i in 0..6000 { let (p, c) = &cats[i % 300]; d.add_note(p, c, &format!("Note number {i} with a bit of text"), None); }
    let dir = std::env::temp_dir().join(format!("memi-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut store = SqliteStore::open(&dir.join("memi.db")).unwrap();
    store.write(&d.snap).unwrap();

    let n = 50;
    let t = Instant::now();
    for i in 0..n { let (p, c) = &cats[i % 300]; d.add_note(p, c, "One more", None); }
    let edit = t.elapsed() / n as u32;
    let t = Instant::now();
    for i in 0..n { let (p, c) = &cats[i % 300]; d.add_note(p, c, "And another", None); store.write(&d.snap).unwrap(); }
    let edit_and_save = t.elapsed() / n as u32;
    let t = Instant::now();
    let json = memi_engine::core::backup::export(&d.snap, |_| None);
    let export = t.elapsed();
    let key = memi_engine::core::crypto::new_key();
    let t = Instant::now();
    let sealed = memi_engine::core::crypto::seal(&key, &json).unwrap();
    let seal = t.elapsed();
    let t = Instant::now();
    let _ = Data::new(store.load());
    let load = t.elapsed();
    println!("notes {} | edit {:?} | edit+save {:?} | export {:?} ({} KB) | seal {:?} ({} KB) | load {:?}",
        d.snap.notes.len(), edit, edit_and_save, export, json.len() / 1024, seal, sealed.len() / 1024, load);
    std::fs::remove_dir_all(dir).ok();
}
