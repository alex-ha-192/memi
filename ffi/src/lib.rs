//! memi-engine for the Android app. Kotlin keeps its screens (Compose) and the parts only Android
//! has: Google sign-in through Play services, the key in the Android Keystore (and Block Store),
//! notifications and the photo picker. Everything else (the data, every edit, the SQLite file,
//! Google Drive, merging, encryption and pairing) is this engine, the same code the desktop and
//! web apps run.
//!
//! Every method blocks; Kotlin calls them off the main thread. Ids are this phone's row ids, as
//! the screens use them; the engine works by uid underneath.

uniffi::setup_scaffolding!();

use memi_engine::core::model as m;
use memi_engine::edits::Edits;
use memi_engine::any_drive::AnyDrive;
use memi_engine::native_drive::{block_on, NativeDrive};
use memi_engine::webdav::WebDavDrive;
use memi_engine::sqlite::SqliteStore;
use memi_engine::sync::{self as engine, Handle};
use memi_engine::{Memi, Platform, SyncSettings};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// --- What Kotlin provides ----------------------------------------------------------------------------

/// The Android pieces: where the key is kept, and what this phone is called.
#[uniffi::export(with_foreign)]
pub trait Host: Send + Sync {
    /// The sync key on this phone (Android Keystore), if there is one.
    fn key(&self) -> Option<Vec<u8>>;
    /// Keeps the key on this phone, and backs it up with Block Store where it can.
    fn save_key(&self, key: Vec<u8>);
    fn forget_key(&self);
    /// "Pixel 8": shown on your other devices when this phone asks them for the key.
    fn device_label(&self) -> String;
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MemiError {
    #[error("{message}")]
    Failed { message: String },
}

impl From<anyhow::Error> for MemiError {
    fn from(e: anyhow::Error) -> Self { MemiError::Failed { message: e.to_string() } }
}

type Res<T> = Result<T, MemiError>;

// --- Records, as the screens see them -----------------------------------------------------------------

#[derive(uniffi::Record, Clone)]
pub struct Person {
    pub id: i64,
    pub uid: String,
    pub updated_at: i64,
    pub name: String,
    pub relationship: String,
    pub photo_path: Option<String>,
    pub birthday_month: Option<i32>,
    pub birthday_day: Option<i32>,
    pub birth_year: Option<i32>,
    pub tone_index: i32,
    pub pinned: bool,
    pub sort_order: i32,
    pub created_at: i64,
}

#[derive(uniffi::Record, Clone)]
pub struct Collection {
    pub id: i64,
    pub uid: String,
    pub updated_at: i64,
    pub person_id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub slot: i32,
    pub sort_order: i32,
    pub occasion: String,
    pub occasion_day: Option<i64>,
    pub yearly: bool,
}

#[derive(uniffi::Record, Clone)]
pub struct Note {
    pub id: i64,
    pub uid: String,
    pub updated_at: i64,
    pub person_id: i64,
    pub collection_id: i64,
    pub text: String,
    pub mentioned_at: i64,
    pub first_mentioned_at: i64,
    pub mention_count: i32,
    pub due_at: Option<i64>,
    pub status: i32,
    pub outcome: i32,
    pub done_at: Option<i64>,
    pub created_at: i64,
    pub sort_order: i64,
}

#[derive(uniffi::Record)]
pub struct Everything {
    pub people: Vec<Person>,
    pub collections: Vec<Collection>,
    pub notes: Vec<Note>,
    /// Goes up with every change, here or from another device.
    pub version: u64,
}

#[derive(uniffi::Record, Clone, Default)]
pub struct SyncState {
    pub enabled: bool,
    pub account: Option<String>,
    pub remote_version: Option<String>,
    pub dirty: bool,
    pub last_sync: Option<i64>,
    #[uniffi(default = false)]
    pub key_saved: bool,
    #[uniffi(default = None)]
    pub key_nudge_after: Option<i64>,
}

#[derive(uniffi::Enum, Clone)]
pub enum Problem {
    NeedsSignIn,
    NeedsKey,
    Failed { message: String },
}

#[derive(uniffi::Record, Clone)]
pub struct Join {
    pub file_id: String,
    pub device: String,
    pub confirm: String,
}

#[derive(uniffi::Record, Clone)]
pub struct Asking {
    pub id: String,
    pub code: String,
    pub confirm: String,
}

#[derive(uniffi::Record, Clone)]
pub struct Status {
    pub busy: bool,
    pub problem: Option<Problem>,
    pub joins: Vec<Join>,
    pub asking: Option<Asking>,
    pub sync: SyncState,
}

fn person_out(p: &m::Person) -> Person {
    Person {
        id: p.id, uid: p.uid.clone(), updated_at: p.updated_at, name: p.name.clone(), relationship: p.relationship.clone(),
        photo_path: p.photo_path.clone(), birthday_month: p.birthday_month.map(|x| x as i32), birthday_day: p.birthday_day.map(|x| x as i32),
        birth_year: p.birth_year, tone_index: p.tone_index, pinned: p.pinned, sort_order: p.sort_order, created_at: p.created_at,
    }
}

fn person_in(p: Person) -> m::Person {
    m::Person {
        id: p.id, uid: p.uid, updated_at: p.updated_at, name: p.name, relationship: p.relationship, photo_path: p.photo_path,
        birthday_month: p.birthday_month.and_then(|x| u32::try_from(x).ok()), birthday_day: p.birthday_day.and_then(|x| u32::try_from(x).ok()),
        birth_year: p.birth_year, tone_index: p.tone_index, pinned: p.pinned, sort_order: p.sort_order, created_at: p.created_at,
    }
}

fn collection_out(c: &m::Collection) -> Collection {
    Collection {
        id: c.id, uid: c.uid.clone(), updated_at: c.updated_at, person_id: c.person_id, name: c.name.clone(), kind: c.kind.clone(),
        slot: c.slot, sort_order: c.sort_order, occasion: c.occasion.clone(), occasion_day: c.occasion_day, yearly: c.yearly,
    }
}

fn collection_in(c: Collection) -> m::Collection {
    m::Collection {
        id: c.id, uid: c.uid, updated_at: c.updated_at, person_id: c.person_id, person_uid: None, name: c.name, kind: c.kind,
        slot: c.slot, sort_order: c.sort_order, occasion: c.occasion, occasion_day: c.occasion_day, yearly: c.yearly,
    }
}

fn note_out(n: &m::Note) -> Note {
    Note {
        id: n.id, uid: n.uid.clone(), updated_at: n.updated_at, person_id: n.person_id, collection_id: n.collection_id, text: n.text.clone(),
        mentioned_at: n.mentioned_at, first_mentioned_at: n.first_mentioned_at, mention_count: n.mention_count, due_at: n.due_at,
        status: n.status, outcome: n.outcome, done_at: n.done_at, created_at: n.created_at, sort_order: n.sort_order,
    }
}

fn note_in(n: Note) -> m::Note {
    m::Note {
        id: n.id, uid: n.uid, updated_at: n.updated_at, person_id: n.person_id, person_uid: String::new(), collection_id: n.collection_id,
        collection_uid: String::new(), text: n.text, mentioned_at: n.mentioned_at, first_mentioned_at: n.first_mentioned_at,
        mention_count: n.mention_count, due_at: n.due_at, status: n.status, outcome: n.outcome, done_at: n.done_at,
        created_at: n.created_at, sort_order: n.sort_order,
    }
}

impl From<SyncSettings> for SyncState {
    fn from(s: SyncSettings) -> Self {
        SyncState { enabled: s.enabled, account: s.account, remote_version: s.remote_version, dirty: s.dirty, last_sync: s.last_sync, key_saved: s.key_saved, key_nudge_after: s.key_nudge_after }
    }
}

impl From<SyncState> for SyncSettings {
    fn from(s: SyncState) -> Self {
        // Settings from before 1.0 always meant Google Drive.
        SyncSettings { enabled: s.enabled, provider: Default::default(), account: s.account, remote_version: s.remote_version, dirty: s.dirty, last_sync: s.last_sync, key_saved: s.key_saved, key_nudge_after: s.key_nudge_after }
    }
}

// --- The phone's side of the engine -------------------------------------------------------------------

/// Photos are kept no larger than this (a cropped profile photo is far smaller).
const MAX_PHOTO_BYTES: usize = 8 * 1024 * 1024;

struct AndroidPlatform {
    store: SqliteStore,
    photo_dir: PathBuf,
    sync_path: PathBuf,
    host: Arc<dyn Host>,
}

impl Platform for AndroidPlatform {
    fn save(&mut self, snap: &m::Snapshot) -> anyhow::Result<()> { self.store.write(snap) }

    fn save_sync(&mut self, sync: &SyncSettings) {
        if let Ok(json) = serde_json::to_vec_pretty(sync) {
            let tmp = self.sync_path.with_extension("tmp");
            if std::fs::write(&tmp, json).is_ok() { let _ = std::fs::rename(tmp, &self.sync_path); }
        }
    }

    fn photo(&self, key: &str) -> Option<Vec<u8>> { std::fs::read(key).ok() }

    /// A photo from another device: kept as a JPEG file in the app's photo folder, as before.
    fn save_photo(&mut self, bytes: &[u8]) -> Option<String> {
        let is_image = bytes.starts_with(&[0xFF, 0xD8, 0xFF]) || bytes.starts_with(b"\x89PNG") || bytes.get(8..12) == Some(b"WEBP");
        if !is_image || bytes.len() > MAX_PHOTO_BYTES { return None; }
        std::fs::create_dir_all(&self.photo_dir).ok()?;
        let path = self.photo_dir.join(format!("{}.jpg", m::new_uid()));
        std::fs::write(&path, bytes).ok()?;
        Some(path.to_string_lossy().into_owned())
    }

    fn delete_photo(&mut self, key: &str) { delete_photo_in(&self.photo_dir, key); }

    fn key(&self) -> Option<[u8; 32]> { self.host.key()?.as_slice().try_into().ok() }
    fn save_key(&mut self, key: &[u8; 32]) { self.host.save_key(key.to_vec()); }
    fn forget_key(&mut self) { self.host.forget_key(); }
    fn device_label(&self) -> String { self.host.device_label() }
}

/// Deletes a photo, but only one in the app's own photo folder.
fn delete_photo_in(dir: &Path, key: &str) {
    let p = Path::new(key);
    if p.parent() == Some(dir) { let _ = std::fs::remove_file(p); }
}

// --- The engine, for Kotlin ---------------------------------------------------------------------------

#[derive(uniffi::Object)]
pub struct Engine {
    memi: Arc<Mutex<Memi<AndroidPlatform>>>,
    photo_dir: PathBuf,
    /// For WebDAV: the password, which the app keeps encrypted and hands over at start.
    webdav_password: Mutex<Option<String>>,
    /// For a folder: the app's access to the folder the person picked.
    host_folder: Mutex<Option<Arc<dyn HostFolder>>>,
}

// --- Where sync goes ------------------------------------------------------------------------------

/// Where sync keeps its files, as Sync shows it.
#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum SyncProvider {
    Google,
    WebDav { url: String, user: String },
    /// A folder the person picked; `path` is its content:// URI.
    Folder { path: String },
}

impl From<memi_engine::Provider> for SyncProvider {
    fn from(p: memi_engine::Provider) -> Self {
        match p {
            memi_engine::Provider::Google => SyncProvider::Google,
            memi_engine::Provider::WebDav { url, user } => SyncProvider::WebDav { url, user },
            memi_engine::Provider::Folder { path } => SyncProvider::Folder { path },
        }
    }
}

/// A file in the picked folder.
#[derive(uniffi::Record, Clone, Debug)]
pub struct HostFile {
    pub name: String,
    /// Milliseconds since 1970.
    pub modified: i64,
    pub size: i64,
}

/// The folder the person picked, which only the app can open (Android's storage access framework).
/// Kotlin implements this; sync reads and writes Memi's files through it.
#[uniffi::export(with_foreign)]
pub trait HostFolder: Send + Sync {
    /// Everything in the folder, or None if it can't be opened.
    fn list(&self) -> Option<Vec<HostFile>>;
    /// None if it can't be read.
    fn read(&self, name: String) -> Option<Vec<u8>>;
    /// Writes (making or replacing) a file. Returns what went wrong, or None.
    fn write(&self, name: String, bytes: Vec<u8>) -> Option<String>;
    /// Returns what went wrong, or None (also when it wasn't there).
    fn delete(&self, name: String) -> Option<String>;
    /// The folder's name, to show in Sync.
    fn label(&self) -> String;
}

struct HostFolderDrive(Arc<dyn HostFolder>);

/// Stands in when the app hasn't handed over the folder (access was taken away, say).
struct NoFolder;

impl HostFolder for NoFolder {
    fn list(&self) -> Option<Vec<HostFile>> { None }
    fn read(&self, _: String) -> Option<Vec<u8>> { None }
    fn write(&self, _: String, _: Vec<u8>) -> Option<String> { Some(FOLDER_GONE.into()) }
    fn delete(&self, _: String) -> Option<String> { Some(FOLDER_GONE.into()) }
    fn label(&self) -> String { "the folder".into() }
}

const FOLDER_GONE: &str = "Memi can't open the sync folder any more. Choose it again in Sync.";

impl HostFolderDrive {
    fn remote(f: HostFile) -> engine::RemoteFile {
        engine::RemoteFile { id: f.name.clone(), modified_time: Some(format!("{}-{}", f.modified, f.size)), name: f.name }
    }
    fn files(&self) -> anyhow::Result<Vec<HostFile>> { self.0.list().ok_or_else(|| anyhow::anyhow!(FOLDER_GONE)) }
}

impl engine::Drive for HostFolderDrive {
    async fn find(&self, name: &str) -> anyhow::Result<Option<engine::RemoteFile>> {
        Ok(self.files()?.into_iter().find(|f| f.name == name).map(Self::remote))
    }
    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<engine::RemoteFile>> {
        let mut v: Vec<HostFile> = self.files()?.into_iter().filter(|f| f.name.starts_with(prefix)).collect();
        v.sort_by(|a, b| b.modified.cmp(&a.modified));
        Ok(v.into_iter().map(Self::remote).collect())
    }
    async fn download(&self, id: &str) -> anyhow::Result<Vec<u8>> { self.0.read(id.to_string()).ok_or_else(|| anyhow::anyhow!("Couldn't read {id} from the sync folder")) }
    async fn create(&self, name: &str, bytes: &[u8]) -> anyhow::Result<engine::RemoteFile> { self.update(name, bytes).await }
    async fn update(&self, id: &str, bytes: &[u8]) -> anyhow::Result<engine::RemoteFile> {
        if let Some(e) = self.0.write(id.to_string(), bytes.to_vec()) { anyhow::bail!("Couldn't write to the sync folder: {e}"); }
        self.find(id).await?.ok_or_else(|| anyhow::anyhow!("The folder didn't keep {id}"))
    }
    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        match self.0.delete(id.to_string()) { Some(e) => anyhow::bail!("Couldn't delete from the sync folder: {e}"), None => Ok(()) }
    }
    async fn email(&self) -> Option<String> { Some(self.0.label()) }
}

/// Whichever place sync uses on the phone.
enum PhoneDrive {
    Native(AnyDrive),
    Host(HostFolderDrive),
}

impl engine::Drive for PhoneDrive {
    async fn find(&self, name: &str) -> anyhow::Result<Option<engine::RemoteFile>> { match self { PhoneDrive::Native(d) => d.find(name).await, PhoneDrive::Host(d) => d.find(name).await } }
    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<engine::RemoteFile>> { match self { PhoneDrive::Native(d) => d.list(prefix).await, PhoneDrive::Host(d) => d.list(prefix).await } }
    async fn download(&self, id: &str) -> anyhow::Result<Vec<u8>> { match self { PhoneDrive::Native(d) => d.download(id).await, PhoneDrive::Host(d) => d.download(id).await } }
    async fn create(&self, name: &str, bytes: &[u8]) -> anyhow::Result<engine::RemoteFile> { match self { PhoneDrive::Native(d) => d.create(name, bytes).await, PhoneDrive::Host(d) => d.create(name, bytes).await } }
    async fn update(&self, id: &str, bytes: &[u8]) -> anyhow::Result<engine::RemoteFile> { match self { PhoneDrive::Native(d) => d.update(id, bytes).await, PhoneDrive::Host(d) => d.update(id, bytes).await } }
    async fn delete(&self, id: &str) -> anyhow::Result<()> { match self { PhoneDrive::Native(d) => d.delete(id).await, PhoneDrive::Host(d) => d.delete(id).await } }
    async fn email(&self) -> Option<String> { match self { PhoneDrive::Native(d) => d.email().await, PhoneDrive::Host(d) => d.email().await } }
}

impl Engine {
    /// The place sync goes, from the saved provider. `token` only matters for Google.
    fn drive(&self, token: &str) -> PhoneDrive {
        match self.memi.with(|m| m.sync.provider.clone()) {
            memi_engine::Provider::Google => PhoneDrive::Native(AnyDrive::Google(NativeDrive::new(token))),
            memi_engine::Provider::WebDav { url, user } => {
                let pass = self.webdav_password.lock().unwrap().clone().unwrap_or_default();
                match WebDavDrive::new(&url, &user, &pass) {
                    Ok(d) => PhoneDrive::Native(AnyDrive::WebDav(d)),
                    // A saved address that no longer passes the checks: fail the way a bad token does.
                    Err(_) => PhoneDrive::Native(AnyDrive::Google(NativeDrive::new(""))),
                }
            }
            memi_engine::Provider::Folder { path } => match self.host_folder.lock().unwrap().clone() {
                Some(f) => PhoneDrive::Host(HostFolderDrive(f)),
                None => { let _ = path; PhoneDrive::Host(HostFolderDrive(Arc::new(NoFolder))) }
            },
        }
    }
}

fn missing() -> MemiError { MemiError::Failed { message: "That's no longer here.".into() } }

impl Engine {
    fn edit<R>(&self, f: impl FnOnce(&mut Edits) -> anyhow::Result<R>) -> Res<R> {
        self.memi.with(|m| m.change(|d| f(&mut Edits { d }))).map_err(Into::into)
    }

    fn note(&self, id: i64) -> Res<m::Note> {
        self.memi.with(|m| m.snapshot().notes.iter().find(|n| n.id == id).cloned()).ok_or_else(missing)
    }

    fn collection(&self, id: i64) -> Res<m::Collection> {
        self.memi.with(|m| m.snapshot().collections.iter().find(|c| c.id == id).cloned()).ok_or_else(missing)
    }

    fn problem(&self) -> Option<memi_engine::Problem> { self.memi.with(|m| m.status.problem.clone()) }
}

#[uniffi::export]
impl Engine {
    /// Opens Memi's database at `db_path`. Room's file from earlier versions opens in place; the
    /// first time, it's copied to `untouched_copy` exactly as it was, before anything changes.
    /// `legacy_sync` is the sync state Android kept in its preferences before 0.6.1, used only if
    /// there's no sync.json yet.
    #[uniffi::constructor]
    pub fn new(
        db_path: String,
        untouched_copy: String,
        photo_dir: String,
        sync_path: String,
        legacy_sync: Option<SyncState>,
        host: Arc<dyn Host>,
    ) -> Res<Arc<Self>> {
        SqliteStore::copy_untouched(Path::new(&db_path), Path::new(&untouched_copy))?;
        let store = SqliteStore::open(Path::new(&db_path))?;
        let snapshot = store.load();
        let sync_path = PathBuf::from(sync_path);
        let saved: Option<SyncSettings> = std::fs::read(&sync_path).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let migrated = saved.is_none() && legacy_sync.is_some();
        let sync: SyncSettings = saved.or(legacy_sync.map(Into::into)).unwrap_or_default();
        let photo_dir = PathBuf::from(photo_dir);
        let mut platform = AndroidPlatform { store, photo_dir: photo_dir.clone(), sync_path, host };
        if migrated { platform.save_sync(&sync); }
        Ok(Arc::new(Engine { memi: Arc::new(Mutex::new(Memi::new(snapshot, sync, platform))), photo_dir, webdav_password: Mutex::new(None), host_folder: Mutex::new(None) }))
    }

    /// Everything, photos included, as a backup file the person keeps (not encrypted).
    pub fn export_file(&self) -> Vec<u8> { self.memi.with(|m| m.export_file()) }

    /// Merges a backup file into what's here. Returns a line to show.
    pub fn import_file(&self, bytes: Vec<u8>) -> Result<String, MemiError> {
        Ok(self.memi.with(|m| m.import_file(&bytes))?.summary())
    }

    /// Everyone, their categories and notes, in the order the screens show them.
    pub fn everything(&self) -> Everything {
        self.memi.with(|m| {
            let s = m.snapshot();
            Everything {
                people: s.people.iter().map(person_out).collect(),
                collections: s.collections.iter().map(collection_out).collect(),
                notes: s.notes.iter().map(note_out).collect(),
                version: m.version,
            }
        })
    }

    /// Goes up with every change; cheap to check.
    pub fn version(&self) -> u64 { self.memi.with(|m| m.version) }

    // --- People ----------------------------------------------------------------------------------------

    /// Adds someone (their photo already saved in `photo_path`) with the starter categories.
    pub fn add_person(&self, p: Person) -> Res<i64> { self.edit(|e| e.add_person(&person_in(p))) }
    pub fn update_person(&self, p: Person) -> Res<()> { self.edit(|e| e.update_person(&person_in(p))) }

    /// Removes someone, their categories and notes, and their photo.
    pub fn delete_person(&self, id: i64) -> Res<()> {
        let photo = self.memi.with(|m| m.snapshot().people.iter().find(|p| p.id == id).and_then(|p| p.photo_path.clone()));
        self.edit(|e| e.delete_person(id))?;
        if let Some(p) = photo { delete_photo_in(&self.photo_dir, &p); }
        Ok(())
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Res<()> { self.edit(|e| e.set_pinned(id, pinned)) }
    pub fn move_person(&self, id: i64, delta: i32) -> Res<()> { self.edit(|e| e.move_person(id, delta)) }
    pub fn place_person(&self, id: i64, target: i64) -> Res<()> { self.edit(|e| e.place_person(id, target)) }

    /// A photo that's no longer used (replaced, or the edit was cancelled).
    pub fn delete_photo_file(&self, path: String) { delete_photo_in(&self.photo_dir, &path); }

    // --- Categories ------------------------------------------------------------------------------------

    pub fn add_collection(&self, person_id: i64, name: String, occasion: String, occasion_day: Option<i64>, yearly: bool) -> Res<i64> {
        self.edit(|e| e.add_collection(person_id, &name, &occasion, occasion_day, yearly))
    }
    pub fn update_collection(&self, c: Collection) -> Res<()> { self.edit(|e| e.update_collection(&collection_in(c))) }

    /// Removes a category; its notes move to another of the person's categories.
    pub fn delete_collection(&self, id: i64) -> Res<()> {
        let c = self.collection(id)?;
        self.edit(|e| e.delete_collection(&c))
    }

    pub fn move_collection(&self, id: i64, delta: i32) -> Res<()> {
        let c = self.collection(id)?;
        self.edit(|e| e.move_collection(&c, delta))
    }
    pub fn place_collection(&self, id: i64, target: i64) -> Res<()> { self.edit(|e| e.place_collection(id, target)) }

    // --- Notes -----------------------------------------------------------------------------------------

    pub fn add_note(&self, person_id: i64, collection_id: i64, text: String, due_at: Option<i64>) -> Res<i64> {
        self.edit(|e| e.add_note(person_id, collection_id, &text, due_at))
    }
    /// Saves a note as given, including a move to another category.
    pub fn update_note(&self, n: Note) -> Res<()> { self.edit(|e| e.update_note(&note_in(n))) }
    pub fn delete_note(&self, id: i64) -> Res<()> { self.edit(|e| e.delete_note(id)) }

    /// Undo a deletion: the note comes back as it was, the same note on every device.
    pub fn restore_note(&self, n: Note) -> Res<()> {
        let (person_uid, collection_uid) = self.memi.with(|m| {
            let s = m.snapshot();
            (s.people.iter().find(|p| p.id == n.person_id).map(|p| p.uid.clone()),
             s.collections.iter().find(|c| c.id == n.collection_id).map(|c| c.uid.clone()))
        });
        let (Some(person_uid), Some(collection_uid)) = (person_uid, collection_uid) else { return Err(missing()) };
        let note = m::Note { person_uid, collection_uid, ..note_in(n) };
        self.edit(|e| e.restore_note(&note))
    }

    pub fn mark_done(&self, id: i64, outcome: i32) -> Res<()> {
        let n = self.note(id)?;
        self.edit(|e| e.mark_done(&n, outcome))
    }
    pub fn reopen(&self, id: i64) -> Res<()> {
        let n = self.note(id)?;
        self.edit(|e| e.reopen(&n))
    }
    pub fn place_note(&self, id: i64, target: i64) -> Res<()> { self.edit(|e| e.place_note(id, target)) }

    // --- Sync ------------------------------------------------------------------------------------------
    //
    // Each step takes a Google access token (Play services gives Kotlin one without asking). When
    // Google turns a token down, the status says NeedsSignIn; Kotlin drops that token and tries once
    // more with a fresh one.

    pub fn status(&self) -> Status {
        self.memi.with(|m| Status {
            busy: m.status.busy,
            problem: m.status.problem.clone().map(|p| match p {
                memi_engine::Problem::NeedsSignIn => Problem::NeedsSignIn,
                memi_engine::Problem::NeedsKey => Problem::NeedsKey,
                memi_engine::Problem::Failed(message) => Problem::Failed { message },
            }),
            joins: m.status.joins.iter().map(|j| Join { file_id: j.file_id.clone(), device: j.device.clone(), confirm: j.confirm.clone() }).collect(),
            asking: m.status.asking.as_ref().map(|a| Asking { id: a.id.clone(), code: a.code.clone(), confirm: a.confirm.clone() }),
            sync: m.sync.clone().into(),
        })
    }

    /// Clears a problem before trying again with a fresh token.
    pub fn clear_problem(&self) { self.memi.with(|m| m.set_status(|s| s.problem = None)); }

    /// Right after you pick your Google account: turns sync on, or (with no key here) starts asking
    /// your other devices for it.
    pub fn connect(&self, token: String) {
        self.memi.with(|m| m.update_sync(|s| s.provider = memi_engine::Provider::Google));
        block_on(engine::connect(&self.memi, &self.drive(&token)));
    }

    /// Where sync goes.
    pub fn provider(&self) -> SyncProvider { self.memi.with(|m| m.sync.provider.clone()).into() }

    /// The WebDAV password (kept encrypted by the app), given at start and after connecting.
    pub fn set_webdav_password(&self, password: Option<String>) { *self.webdav_password.lock().unwrap() = password; }

    /// Access to the picked folder, given at start and after picking.
    pub fn set_host_folder(&self, folder: Option<Arc<dyn HostFolder>>) { *self.host_folder.lock().unwrap() = folder; }

    /// Checks a WebDAV address and password, then turns sync on there. A mistake comes back as an
    /// error to show in the form; nothing changes.
    pub fn connect_webdav(&self, url: String, user: String, password: String) -> Res<()> {
        let url = memi_engine::dav::normalise_url(&url)?;
        let user = user.trim().to_string();
        if user.is_empty() { return Err(MemiError::Failed { message: "Enter your user name".into() }); }
        let drive = WebDavDrive::new(&url, &user, &password)?;
        match block_on(engine::Drive::find(&drive, engine::FILE)) {
            Err(e) if e.is::<engine::Unauthorized>() => return Err(MemiError::Failed { message: "The server turned down that user name and password. On Nextcloud, use an app password (Settings › Security).".into() }),
            Err(e) => return Err(e.into()),
            Ok(_) => {}
        }
        self.set_webdav_password(Some(password));
        self.memi.with(|m| { m.update_sync(|s| s.provider = memi_engine::Provider::WebDav { url, user }); m.set_status(|s| s.problem = None) });
        let enabled = self.memi.with(|m| m.sync.enabled);
        let d = PhoneDrive::Native(AnyDrive::WebDav(drive));
        if enabled { block_on(engine::sync_now(&self.memi, &d)); } else { block_on(engine::connect(&self.memi, &d)); }
        Ok(())
    }

    /// Turns sync on through the folder the person picked (`uri`, kept to open it again later).
    pub fn connect_folder(&self, uri: String, folder: Arc<dyn HostFolder>) {
        self.set_host_folder(Some(folder.clone()));
        self.memi.with(|m| m.update_sync(|s| s.provider = memi_engine::Provider::Folder { path: uri }));
        block_on(engine::connect(&self.memi, &PhoneDrive::Host(HostFolderDrive(folder))));
    }

    /// Brings in changes from your other devices and sends this phone's.
    pub fn sync_now(&self, token: String) { block_on(engine::sync_now(&self.memi, &self.drive(&token))); }

    /// True if the key opens the backup in your account (sync is then on).
    pub fn use_recovery_key(&self, token: String, text: String) -> bool {
        block_on(engine::use_recovery_key(&self.memi, &self.drive(&token), &text))
    }

    pub fn ask_for_key(&self, token: String) { block_on(engine::ask_for_key(&self.memi, &self.drive(&token))); }

    /// Looks once for another device's answer. True once the key is in and sync is on.
    pub fn check_pairing(&self, token: String) -> Res<bool> {
        Ok(block_on(engine::check_pairing(&self.memi, &self.drive(&token)))?)
    }

    pub fn cancel_asking(&self, token: Option<String>) {
        match token {
            Some(t) => block_on(engine::cancel_asking(&self.memi, &self.drive(&t))),
            None => self.memi.with(|m| m.set_status(|s| s.asking = None)),
        }
    }

    /// Allow (sends the key, sealed for that device alone) or turn down another device's request.
    pub fn answer_join(&self, token: String, file_id: String, allow: bool) -> Res<()> {
        Ok(block_on(engine::answer_join(&self.memi, &self.drive(&token), &file_id, allow))?)
    }

    /// Sends this phone's key to the computer whose QR code was scanned.
    pub fn send_key_to(&self, token: String, code: String) -> Res<()> {
        Ok(block_on(engine::send_key_to(&self.memi, &self.drive(&token), &code))?)
    }

    /// Replaces the backup in your account with what's here, under a new key.
    pub fn start_over(&self, token: String) { block_on(engine::start_over(&self.memi, &self.drive(&token))); }

    pub fn turn_off(&self, forget_key: bool) { self.memi.with(|m| m.turn_off_sync(forget_key)); }

    pub fn recovery_key(&self) -> Option<String> { self.memi.with(|m| m.recovery_key()) }

    /// Whether Home should remind them to save the recovery key.
    pub fn wants_key_nudge(&self, now: i64) -> bool { self.memi.with(|m| m.wants_key_nudge(now)) }
    /// They've put the recovery key somewhere safe.
    pub fn key_saved(&self) { self.memi.with(|m| m.key_saved()); }
    /// "Not now": quiet the reminder for two weeks.
    pub fn snooze_key_nudge(&self, now: i64) { self.memi.with(|m| m.snooze_key_nudge(now)); }

    /// Whether the last step left a sign-in problem (to retry with a fresh token).
    pub fn needs_sign_in(&self) -> bool { self.problem() == Some(memi_engine::Problem::NeedsSignIn) }
}
