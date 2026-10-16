//! The player's storage: named text documents (RON) in namespaces, each with a format version, behind one small
//! trait so the game's other crates stay free of I/O (`sim` and `data` never touch it). Written 13 Oct 2026 for
//! the drydock's designs (`designs/<id>`, the working design `drydock/working`) and the player's behaviors
//! (`behaviors/<id>`, `drydock/behaviors.rs`); save games since (`saves/<slot>`, ADR-0024: saves are versioned, with
//! migrations), the settings (`settings/controls`, `settings/display`, `settings.rs`, GDR-0090) and the evolution
//! lab's runs (`lab/<id>` and its parts, `evolution/store.rs`, GDR-0092).
//!
//! * A **key** is `namespace/name`; both parts are `a-z`, `0-9`, `_` and `-`, starting with a letter or digit, at
//!   most 64 characters (safe as file names and storage keys on every platform). A document may have **parts**,
//!   `namespace/name/part` with the same rules for the part (a lab run's generations, `lab/<id>/g12`): written on
//!   their own, listed with [`Storage::parts`] and not with the namespace's documents.
//! * A **document** ([`Document`]) is RON text. Stored, it gets one header line, a RON comment, so the rest stays
//!   valid RON (a saved design is a content ship file):
//!   `// space-game: designs, format 1, saved 1760351234` (namespace, format version, Unix seconds). Reading
//!   checks the namespace, refuses formats newer than the build knows ([`StorageError::Newer`]) and hands older
//!   ones to [`Document::from_ron`], where migrations live. A file without the header line (a ship file copied
//!   in by hand) reads as format 1.
//! * **Backends** ([`Backend`]): the browser's IndexedDB (web, ADR-0041: every document read into memory before the
//!   game starts, writes kept at once and written behind),
//!   files in the user's data directory (desktop: `<data dir>/<namespace>/<name>.ron`, a part
//!   `<data dir>/<namespace>/<name>/<part>.ron`), and memory (tests, and the fallback when the platform has no
//!   storage, e.g. Android until its backend lands: the files backend with the app's internal data path).
//!   [`Storage::open`] picks one.
//! * **Errors** ([`StorageError`]) say what went wrong in words the UI can show: storage unavailable or full
//!   (the browser's quota), unreadable documents, documents from a newer build. A write behind that fails comes
//!   back with the next write (and [`Storage::unsaved`] names what it left unsaved).

use std::collections::BTreeMap;
use std::fmt;

use bevy::prelude::*;

/// The marker that starts a stored document's header line.
const HEADER: &str = "// space-game: ";
/// The longest namespace or name.
const MAX_NAME: usize = 64;
/// The room left under which [`Usage::low`] warns (bytes; PLACEHOLDER: a dozen STANDARD lab runs).
pub const LOW_ROOM: u64 = 64 << 20;

/// Why a storage operation failed, in words for the status line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageError {
    /// The platform gives no storage (browser storage blocked, no data directory).
    Unavailable(String),
    /// The storage is full (the browser's per-origin quota, a full disk).
    Full,
    /// Reading or writing failed.
    Io(String),
    /// A document doesn't read (its key and why).
    Unreadable { key: String, why: String },
    /// A document written by a newer build.
    Newer { key: String, format: u32, supported: u32 },
    /// Not a valid key.
    BadName(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Unavailable(why) => write!(f, "no storage: {why}"),
            StorageError::Full => f.write_str("the storage is full"),
            StorageError::Io(why) => write!(f, "storage error: {why}"),
            StorageError::Unreadable { key, why } => write!(f, "{key} doesn't read: {why}"),
            StorageError::Newer { key, format, supported } => {
                write!(f, "{key} is from a newer version of the game (format {format}, this one reads up to {supported})")
            }
            StorageError::BadName(name) => write!(f, "not a valid name: {name:?}"),
        }
    }
}

/// How much the storage holds, for the lab's RUNS page (bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Usage {
    /// The game's documents: the length of their text.
    pub documents: u64,
    /// What the page's origin uses in all and may use, as the browser estimates them (`navigator.storage.estimate()`,
    /// which browsers give only on a secure address: https or localhost, not `http://10.77.9.2`).
    pub estimate: Option<(u64, u64)>,
}

impl Usage {
    /// Little room is left: under [`LOW_ROOM`], or over nine tenths of the quota used.
    pub fn low(&self) -> bool {
        self.estimate.is_some_and(|(used, quota)| quota.saturating_sub(used) < LOW_ROOM || used > quota / 10 * 9)
    }
}

/// Where documents live: one per platform, plus memory. Keys are checked by [`Storage`] before they get here; a
/// `folder` is a namespace, or `<namespace>/<name>` for a document's parts.
pub trait Backend: Send + Sync + 'static {
    /// The names of the documents in a folder, sorted.
    fn list(&self, folder: &str) -> Result<Vec<String>, StorageError>;
    /// A document's text, `None` if there is none.
    fn read(&self, folder: &str, name: &str) -> Result<Option<String>, StorageError>;
    /// Creates or replaces a document.
    fn write(&mut self, folder: &str, name: &str, text: &str) -> Result<(), StorageError>;
    /// Removes a document; removing a missing one is fine.
    fn delete(&mut self, folder: &str, name: &str) -> Result<(), StorageError>;
    /// Where the documents are, for messages and the docs ("the browser's IndexedDB of http://…", a directory).
    fn location(&self) -> String;
    /// Whether this backend writes documents kept with [`Backend::keep_on_leave`] when the page goes away.
    fn keeps_on_leave(&self) -> bool {
        false
    }
    /// Keeps a document's text (made when it is needed) to write if the page goes away before the next write of
    /// that document (web). Other backends keep nothing.
    fn keep_on_leave(&mut self, _folder: &str, _name: &str, _text: Box<dyn FnOnce() -> String>) {}
    /// Why the last write made behind the calls failed, until one commits again (web). Backends that write in the
    /// call fail there and have none.
    fn failure(&self) -> Option<StorageError> {
        None
    }
    /// Whether a document or one of its parts (`name/…`) waits for a failed write behind to be tried again.
    fn waiting(&self, _folder: &str, _name: &str) -> bool {
        false
    }
    /// How much it holds, when it knows (web).
    fn usage(&self) -> Option<Usage> {
        None
    }
}

/// A kind of stored document: its namespace, its current format and its RON text.
pub trait Document: Sized {
    /// The namespace its documents live in (`designs`, `drydock`, ...).
    const NAMESPACE: &'static str;
    /// The format version this build writes. Raise it when the text changes, and migrate older formats in
    /// [`Document::from_ron`].
    const FORMAT: u32;
    fn to_ron(&self) -> String;
    /// Reads a document of `format` (1 to [`Document::FORMAT`]); older formats are migrated here.
    fn from_ron(text: &str, format: u32) -> Result<Self, String>;
}

/// A document as read, with its header.
#[derive(Clone, Debug, PartialEq)]
pub struct Stored<D> {
    pub doc: D,
    /// When it was written (Unix seconds), if the header says.
    pub saved: Option<u64>,
}

/// The player's storage (a resource): one backend, the document header and the key rules.
#[derive(Resource)]
pub struct Storage {
    backend: Box<dyn Backend>,
    /// Why the storage only keeps documents in memory, when the platform's could not be opened.
    pub fallback: Option<String>,
}

impl Storage {
    /// A storage on one backend (the tests of the client crates, on a `MemoryBackend`).
    pub fn new(backend: impl Backend) -> Self {
        Self { backend: Box::new(backend), fallback: None }
    }

    /// The platform's storage: the browser's IndexedDB on the web (opened by `preload` before the game starts),
    /// files in the user's data directory on desktop. Falls back to memory (with [`Storage::fallback`] set) when the
    /// platform has none.
    pub fn open() -> Self {
        match platform_backend() {
            Ok(b) => Self { backend: b, fallback: None },
            Err(e) => Self { backend: Box::new(MemoryBackend::default()), fallback: Some(e.to_string()) },
        }
    }

    /// Where the documents are.
    pub fn location(&self) -> String {
        self.backend.location()
    }

    /// The names of the documents in a namespace, sorted (their parts aside).
    pub fn list(&self, namespace: &str) -> Result<Vec<String>, StorageError> {
        check_name(namespace)?;
        self.backend.list(namespace)
    }

    /// The names of a document's parts (`<name>/<part>`), sorted.
    pub fn parts<D: Document>(&self, name: &str) -> Result<Vec<String>, StorageError> {
        check_name(name)?;
        self.backend.list(&format!("{}/{name}", D::NAMESPACE))
    }

    /// Reads and migrates a document or a part; `None` if there is none.
    pub fn read<D: Document>(&self, name: &str) -> Result<Option<Stored<D>>, StorageError> {
        check_key(name)?;
        let (folder, leaf) = place(D::NAMESPACE, name);
        let Some(text) = self.backend.read(&folder, leaf)? else { return Ok(None) };
        let key = format!("{}/{name}", D::NAMESPACE);
        let unreadable = |why: String| StorageError::Unreadable { key: key.clone(), why };
        let (header, body) = split_header(&text);
        let (format, saved) = match header {
            Some(h) => parse_header(h, D::NAMESPACE).map_err(unreadable)?,
            None => (1, None),
        };
        if format > D::FORMAT {
            return Err(StorageError::Newer { key, format, supported: D::FORMAT });
        }
        let doc = D::from_ron(body, format).map_err(unreadable)?;
        Ok(Some(Stored { doc, saved }))
    }

    /// Writes a document or a part in the current format, stamped with the time `saved` (Unix seconds). Written
    /// behind (web), it is kept at once; while the last write behind failed (and nothing committed since), it gets
    /// that failure (the storage is full, ...), as it may not be kept either; it is tried all the same.
    pub fn write<D: Document>(&mut self, name: &str, doc: &D, saved: u64) -> Result<(), StorageError> {
        check_key(name)?;
        let (folder, leaf) = place(D::NAMESPACE, name);
        self.backend.write(&folder, leaf, &stored_text(doc, saved))?;
        self.backend.failure().map_or(Ok(()), Err)
    }

    /// Keeps the document `doc()` makes, to be written under `name` if the page goes away before its next write
    /// (web: a reload or a closed tab runs no more frames, so a write waiting for the autosave's second would be
    /// lost; the page's `pagehide` writes it). `doc` runs only where that is done; the text is made only then.
    /// Desktop needs none (it writes a waiting autosave when the app closes).
    pub fn keep_on_leave<D: Document + 'static>(&mut self, name: &str, doc: impl FnOnce() -> D) {
        if self.backend.keeps_on_leave() && check_key(name).is_ok() {
            let doc = doc();
            let (folder, leaf) = place(D::NAMESPACE, name);
            self.backend.keep_on_leave(&folder, leaf, Box::new(move || stored_text(&doc, now_unix())));
        }
    }

    /// Removes a document or a part (a document's parts stay: [`Storage::delete_parts`]). Never refused for a
    /// failed write behind: deleting makes room.
    pub fn delete<D: Document>(&mut self, name: &str) -> Result<(), StorageError> {
        check_key(name)?;
        let (folder, leaf) = place(D::NAMESPACE, name);
        self.backend.delete(&folder, leaf)
    }

    /// Removes every part of a document.
    pub fn delete_parts<D: Document>(&mut self, name: &str) -> Result<(), StorageError> {
        let folder = format!("{}/{name}", D::NAMESPACE);
        for part in self.parts::<D>(name)? {
            self.backend.delete(&folder, &part)?;
        }
        Ok(())
    }

    /// Whether a document or one of its parts isn't kept: written behind (web), its write failed and waits to be
    /// tried again with the next write. The failure, if so (the lab pauses a run whose checkpoint it is).
    pub fn unsaved<D: Document>(&self, name: &str) -> Option<StorageError> {
        self.backend.failure().filter(|_| self.backend.waiting(D::NAMESPACE, name))
    }

    /// How much the storage holds, when the platform says (web).
    pub fn usage(&self) -> Option<Usage> {
        self.backend.usage()
    }

    /// Copies a document's stored text as it is (header and all, unread) to `aside` in its namespace, so a
    /// document this build can't read (a newer format, a damaged one) survives being written over. Nothing to copy
    /// is fine.
    pub fn set_aside<D: Document>(&mut self, name: &str, aside: &str) -> Result<(), StorageError> {
        check_name(name)?;
        check_name(aside)?;
        match self.backend.read(D::NAMESPACE, name)? {
            Some(text) => self.backend.write(D::NAMESPACE, aside, &text),
            None => Ok(()),
        }
    }
}

/// A document's folder and its name there: `lab` + `run/g3` is (`lab/run`, `g3`).
fn place<'a>(namespace: &str, name: &'a str) -> (String, &'a str) {
    match name.split_once('/') {
        Some((doc, part)) => (format!("{namespace}/{doc}"), part),
        None => (namespace.to_string(), name),
    }
}

/// Is this a valid namespace or document name?
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        && name.len() <= MAX_NAME
}

fn check_name(name: &str) -> Result<(), StorageError> {
    if valid_name(name) { Ok(()) } else { Err(StorageError::BadName(name.into())) }
}

/// A document's name, or `<name>/<part>` for one of its parts.
fn check_key(name: &str) -> Result<(), StorageError> {
    match name.split_once('/') {
        Some((doc, part)) if valid_name(doc) && valid_name(part) => Ok(()),
        Some(_) => Err(StorageError::BadName(name.into())),
        None => check_name(name),
    }
}

/// A document as stored: the header line, then its RON.
fn stored_text<D: Document>(doc: &D, saved: u64) -> String {
    format!("{HEADER}{}, format {}, saved {saved}\n{}", D::NAMESPACE, D::FORMAT, doc.to_ron())
}

/// The header line (without the marker) and the rest.
fn split_header(text: &str) -> (Option<&str>, &str) {
    match text.strip_prefix(HEADER) {
        Some(rest) => match rest.split_once('\n') {
            Some((h, body)) => (Some(h.trim_end_matches('\r')), body),
            None => (Some(rest), ""),
        },
        None => (None, text),
    }
}

/// `designs, format 1, saved 1760351234` → (format, saved). Items after these are ignored, so later builds
/// may add some.
fn parse_header(h: &str, namespace: &str) -> Result<(u32, Option<u64>), String> {
    let mut items = h.split(',').map(str::trim);
    let ns = items.next().unwrap_or_default();
    if ns != namespace {
        return Err(format!("it is a {ns:?} document, not {namespace:?}"));
    }
    let (mut format, mut saved) = (None, None);
    for item in items {
        match item.split_once(' ') {
            Some(("format", v)) => format = v.parse::<u32>().ok(),
            Some(("saved", v)) => saved = v.parse::<u64>().ok(),
            _ => {}
        }
    }
    match format {
        Some(f) if f >= 1 => Ok((f, saved)),
        _ => Err("its header has no format version".into()),
    }
}

/// Now, in Unix seconds (the wall clock: the client only, never the simulation).
pub fn now_unix() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
    }
}

// ---------------------------------------------------------------------------------------------------------
// Backends

/// Documents in memory: tests, and the fallback when the platform has no storage (gone when the game closes).
#[derive(Default, Debug)]
pub struct MemoryBackend {
    docs: BTreeMap<(String, String), String>,
}

impl Backend for MemoryBackend {
    fn list(&self, folder: &str) -> Result<Vec<String>, StorageError> {
        Ok(self.docs.keys().filter(|(f, _)| f == folder).map(|(_, n)| n.clone()).collect())
    }

    fn read(&self, folder: &str, name: &str) -> Result<Option<String>, StorageError> {
        Ok(self.docs.get(&(folder.to_string(), name.to_string())).cloned())
    }

    fn write(&mut self, folder: &str, name: &str, text: &str) -> Result<(), StorageError> {
        self.docs.insert((folder.into(), name.into()), text.into());
        Ok(())
    }

    fn delete(&mut self, folder: &str, name: &str) -> Result<(), StorageError> {
        self.docs.remove(&(folder.to_string(), name.to_string()));
        Ok(())
    }

    fn location(&self) -> String {
        "memory (gone when the game closes)".into()
    }
}

/// The platform's backend, else why not.
#[cfg(target_arch = "wasm32")]
fn platform_backend() -> Result<Box<dyn Backend>, StorageError> {
    web::opened()
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_backend() -> Result<Box<dyn Backend>, StorageError> {
    match data_dir() {
        Some(dir) => Ok(Box::new(FileBackend::new(dir))),
        None => Err(StorageError::Unavailable("no user data directory on this platform".into())),
    }
}

/// The game's directory in the user's data directory: `$XDG_DATA_HOME/space-game` (else
/// `~/.local/share/space-game`) on Linux and the BSDs, `~/Library/Application Support/space-game` on macOS,
/// `%APPDATA%\space-game` on Windows. `None` elsewhere (Android: the app's internal data path, later).
#[cfg(not(target_arch = "wasm32"))]
pub fn data_dir() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(target_os = "android") {
        None
    } else if cfg!(target_os = "windows") {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        env("XDG_DATA_HOME").filter(|p| p.is_absolute()).or_else(|| env("HOME").map(|h| h.join(".local/share")))
    };
    base.map(|b| b.join("space-game"))
}

/// Documents as files: `<root>/<namespace>/<name>.ron`, a document's parts `<root>/<namespace>/<name>/<part>.ron`.
/// Writes go to a temporary file first and are renamed into place, so a crash never leaves half a document.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct FileBackend {
    root: std::path::PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileBackend {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn dir(&self, folder: &str) -> std::path::PathBuf {
        folder.split('/').fold(self.root.clone(), |dir, segment| dir.join(segment))
    }

    fn path(&self, folder: &str, name: &str) -> std::path::PathBuf {
        self.dir(folder).join(format!("{name}.ron"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn io_error(e: std::io::Error) -> StorageError {
    if e.kind() == std::io::ErrorKind::StorageFull { StorageError::Full } else { StorageError::Io(e.to_string()) }
}

#[cfg(not(target_arch = "wasm32"))]
impl Backend for FileBackend {
    fn list(&self, folder: &str) -> Result<Vec<String>, StorageError> {
        let dir = match std::fs::read_dir(self.dir(folder)) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io_error(e)),
        };
        let mut names = Vec::new();
        for entry in dir {
            let path = entry.map_err(io_error)?.path();
            if path.extension().is_some_and(|x| x == "ron")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                && valid_name(stem)
            {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    fn read(&self, folder: &str, name: &str) -> Result<Option<String>, StorageError> {
        match std::fs::read_to_string(self.path(folder, name)) {
            Ok(t) => Ok(Some(t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_error(e)),
        }
    }

    fn write(&mut self, folder: &str, name: &str, text: &str) -> Result<(), StorageError> {
        let path = self.path(folder, name);
        std::fs::create_dir_all(self.dir(folder)).map_err(io_error)?;
        let tmp = path.with_extension("ron.tmp");
        std::fs::write(&tmp, text).map_err(io_error)?;
        std::fs::rename(&tmp, &path).map_err(io_error)
    }

    fn delete(&mut self, folder: &str, name: &str) -> Result<(), StorageError> {
        match std::fs::remove_file(self.path(folder, name)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_error(e)),
            _ => {
                // A document's last part gone: its folder goes too (it fails while parts are left).
                if folder.contains('/') {
                    let _ = std::fs::remove_dir(self.dir(folder));
                }
                Ok(())
            }
        }
    }

    fn location(&self) -> String {
        self.root.display().to_string()
    }
}

/// Writing behind, platform-free (the web drives it with IndexedDB, the tests with a fake): every document in
/// memory, read once at start, so reads and lists never wait; a write or delete changes the memory at once and marks
/// its key; [`WriteBehind::take`] hands the marked keys out as batches to commit, and [`WriteBehind::done`] hears how
/// each ended. The platform commits batches in the order they were taken (IndexedDB runs read-write transactions on
/// one store in the order they were made), and a batch writes each key's text as it is when the batch is taken, so
/// the last write of a key wins and a delete after a write deletes. A failed batch's keys wait and go out again with
/// the next batch that deletes nothing, and after any batch that commits (a delete that made room), so the stored
/// documents catch up with memory once there is room again.
#[cfg(any(target_arch = "wasm32", test))]
mod behind {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{StorageError, valid_name};

    #[derive(Default)]
    pub(crate) struct WriteBehind {
        /// Every document: its key (`folder/name`) and its stored text.
        docs: BTreeMap<String, String>,
        /// Changed since the last batch was taken.
        dirty: BTreeSet<String>,
        /// Keys a failed batch left unsaved, written again with a later batch.
        unsaved: BTreeSet<String>,
        /// Batches out: their keys.
        flying: BTreeMap<u64, Vec<String>>,
        next: u64,
        /// Why the last batch that ended failed, if it did.
        failure: Option<StorageError>,
        /// The documents' text, in bytes.
        bytes: u64,
    }

    /// Keys to commit together: first the ones to remove (they make room), then the ones to write with their text.
    #[derive(Debug, PartialEq)]
    pub(crate) struct Batch {
        pub id: u64,
        pub deletes: Vec<String>,
        pub puts: Vec<(String, String)>,
    }

    pub(crate) fn key(folder: &str, name: &str) -> String {
        format!("{folder}/{name}")
    }

    impl WriteBehind {
        pub fn new(docs: BTreeMap<String, String>) -> Self {
            let bytes = docs.values().map(|t| t.len() as u64).sum();
            Self { docs, bytes, ..Self::default() }
        }

        /// The documents directly in a folder (a document's parts are in a folder of their own), sorted.
        pub fn list(&self, folder: &str) -> Vec<String> {
            let prefix = format!("{folder}/");
            self.docs
                .range(prefix.clone()..)
                .take_while(|(k, _)| k.starts_with(&prefix))
                .map(|(k, _)| &k[prefix.len()..])
                .filter(|name| valid_name(name))
                .map(str::to_string)
                .collect()
        }

        pub fn read(&self, key: &str) -> Option<&String> {
            self.docs.get(key)
        }

        pub fn write(&mut self, key: String, text: String) {
            self.bytes += text.len() as u64;
            if let Some(old) = self.docs.insert(key.clone(), text) {
                self.bytes -= old.len() as u64;
            }
            self.dirty.insert(key);
        }

        pub fn delete(&mut self, key: String) {
            if let Some(old) = self.docs.remove(&key) {
                self.bytes -= old.len() as u64;
            }
            self.dirty.insert(key);
        }

        /// The changes to commit, as at most two batches (the deletes, then the writes); nothing when nothing changed.
        /// Unless something is deleted, the writes take along the keys a failed batch left unsaved (and no batch out
        /// holds); a delete goes first on its own, so the room it makes is there when they are tried again
        /// ([`WriteBehind::retry_due`]).
        pub fn take(&mut self) -> Vec<Batch> {
            let deleting = self.dirty.iter().any(|k| !self.docs.contains_key(k));
            let retry: Vec<String> = if deleting { Vec::new() } else { self.retry() };
            let keys: BTreeSet<String> = std::mem::take(&mut self.dirty).into_iter().chain(retry).collect();
            let (puts, deletes): (Vec<String>, Vec<String>) = keys.into_iter().partition(|k| self.docs.contains_key(k));
            let puts: Vec<(String, String)> = puts.into_iter().map(|k| (k.clone(), self.docs[&k].clone())).collect();
            let mut batches = Vec::new();
            for (deletes, puts) in [(deletes, Vec::new()), (Vec::new(), puts)] {
                if deletes.is_empty() && puts.is_empty() {
                    continue;
                }
                let id = self.next;
                self.next += 1;
                self.flying.insert(id, deletes.iter().cloned().chain(puts.iter().map(|(k, _)| k.clone())).collect());
                batches.push(Batch { id, deletes, puts });
            }
            batches
        }

        /// The unsaved keys no batch out holds.
        fn retry(&self) -> Vec<String> {
            let out: BTreeSet<&String> = self.flying.values().flatten().collect();
            self.unsaved.iter().filter(|k| !out.contains(k)).cloned().collect()
        }

        /// A batch committed while keys wait: they are to go out again now (the platform takes a batch).
        pub fn retry_due(&self) -> bool {
            self.failure.is_none() && !self.retry().is_empty()
        }

        /// Batch `id` was committed, or failed: its keys wait for a later batch. A commit ends the failure (the
        /// storage takes writes again); what is still unsaved goes out with the next batch.
        pub fn done(&mut self, id: u64, result: Result<(), StorageError>) {
            let Some(keys) = self.flying.remove(&id) else { return };
            match result {
                Ok(()) => {
                    for k in &keys {
                        self.unsaved.remove(k);
                    }
                    self.failure = None;
                }
                Err(e) => {
                    self.unsaved.extend(keys);
                    self.failure = Some(e);
                }
            }
        }

        pub fn failure(&self) -> Option<&StorageError> {
            self.failure.as_ref()
        }

        /// Whether `key` or one of its parts (`key/…`) is unsaved.
        pub fn waiting(&self, key: &str) -> bool {
            let parts = format!("{key}/");
            self.unsaved.contains(key) || self.unsaved.range(parts.clone()..).next().is_some_and(|k| k.starts_with(&parts))
        }

        pub fn bytes(&self) -> u64 {
            self.bytes
        }
    }
}

/// The web's storage: IndexedDB (ADR-0041), database `space-game` with the documents in `docs` (keys
/// `namespace/name`, the text as it is stored). It belongs to the origin (scheme, host and port: the copies served
/// on :8080 and :8081 don't share it); the browser's quota is a share of the disk.
///
/// `preload` (awaited by `main` before the app is built) opens it and reads every document into memory
/// ([`behind`]). The game's calls then never wait: writes are
/// kept in memory at once and committed behind them, what a frame writes in one transaction (a microtask at the
/// end of the frame's task), so a lab generation's part and head are kept together or not at all. A failed
/// transaction (the quota: `QuotaExceededError`) comes back with the next write and through [`Backend::waiting`].
///
/// When the page goes away (`pagehide`) or is hidden (`visibilitychange`), the documents kept for leaving are
/// written and every change still in memory is committed right then. The browser finishes a transaction that was
/// committed before the page went away (Chromium: also on a reload or a closed tab, checked in the headless
/// browser); a crash or a killed tab loses what had not committed, at most the last frame's writes.
#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;

    use bevy::prelude::*;
    use js_sys::{Array, Promise, Reflect};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::{JsFuture, spawn_local};
    use web_sys::{IdbDatabase, IdbRequest, IdbTransaction, IdbTransactionMode};

    use super::behind::{Batch, WriteBehind, key};
    use super::{Backend, StorageError, Usage};

    const DB: &str = "space-game";
    const VERSION: u32 = 1;
    const DOCS: &str = "docs";

    /// The open database and what is written behind it.
    struct Open {
        db: IdbDatabase,
        docs: WriteBehind,
        /// Transactions committing, with their batch.
        flying: Vec<(IdbTransaction, u64)>,
        /// A flush is queued as a microtask.
        queued: bool,
        /// Documents to write if the page goes away before their next write: key → its text, made then.
        on_leave: BTreeMap<String, Box<dyn FnOnce() -> String>>,
        /// The browser's estimate (used, quota), and whether one is being asked for.
        estimate: Option<(u64, u64)>,
        estimating: bool,
    }

    /// The handlers every transaction and flush shares, made once.
    struct Handlers {
        complete: Closure<dyn FnMut(web_sys::Event)>,
        abort: Closure<dyn FnMut(web_sys::Event)>,
        flush: Closure<dyn FnMut(JsValue)>,
    }

    thread_local! {
        static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
        static HANDLERS: RefCell<Option<Handlers>> = const { RefCell::new(None) };
        /// Why the database didn't open.
        static FAILED: RefCell<Option<StorageError>> = const { RefCell::new(None) };
        static LISTENING: Cell<bool> = const { Cell::new(false) };
    }

    fn with_open<R>(f: impl FnOnce(&mut Open) -> R) -> Option<R> {
        OPEN.with_borrow_mut(|o| o.as_mut().map(f))
    }

    /// Opens the storage before the game starts: the database and every document into memory. If it fails, the
    /// game runs on memory (`Storage::fallback` says why).
    pub async fn preload() {
        match open().await {
            Ok(open) => {
                OPEN.with_borrow_mut(|o| *o = Some(open));
                HANDLERS.with_borrow_mut(|h| {
                    *h = Some(Handlers {
                        complete: Closure::new(|e: web_sys::Event| finished(e, true)),
                        abort: Closure::new(|e: web_sys::Event| finished(e, false)),
                        flush: Closure::new(|_: JsValue| {
                            with_open(flush);
                        }),
                    })
                });
                persist();
                with_open(estimate);
            }
            Err(e) => FAILED.with_borrow_mut(|f| *f = Some(e)),
        }
    }

    async fn open() -> Result<Open, StorageError> {
        let db = open_db().await?;
        let docs = read_all(&db).await?;
        Ok(Open { db, docs: WriteBehind::new(docs), flying: Vec::new(), queued: false, on_leave: BTreeMap::new(), estimate: None, estimating: false })
    }

    /// The backend `Storage::open` gets: the database `preload` opened, else why it didn't open.
    pub fn opened() -> Result<Box<dyn Backend>, StorageError> {
        if OPEN.with_borrow(Option::is_some) {
            return Ok(Box::new(IndexedDb));
        }
        Err(FAILED.with_borrow_mut(Option::take).unwrap_or_else(|| StorageError::Unavailable("the storage wasn't opened before the game started".into())))
    }

    fn js_error(e: JsValue) -> StorageError {
        match e.dyn_into::<web_sys::DomException>() {
            Ok(d) => dom_error(d),
            Err(e) => StorageError::Io(format!("{e:?}")),
        }
    }

    fn dom_error(d: web_sys::DomException) -> StorageError {
        match d.name().as_str() {
            "QuotaExceededError" => StorageError::Full,
            _ => StorageError::Io(d.message()),
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // Opening

    async fn open_db() -> Result<IdbDatabase, StorageError> {
        let unavailable = |why: &str| StorageError::Unavailable(format!("the browser's IndexedDB {why}"));
        let factory = web_sys::window().and_then(|w| w.indexed_db().ok().flatten()).ok_or_else(|| unavailable("is blocked"))?;
        let req = factory.open_with_u32(DB, VERSION).map_err(js_error)?;
        // A new database gets its store.
        let upgrade = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
            let db = e.target().and_then(|t| t.dyn_into::<web_sys::IdbOpenDbRequest>().ok()).and_then(|r| r.result().ok()).and_then(|r| r.dyn_into::<IdbDatabase>().ok());
            if let Some(db) = db.filter(|db| !db.object_store_names().contains(DOCS)) {
                let _ = db.create_object_store(DOCS);
            }
        });
        req.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
        let db = request(&req).await;
        req.set_onupgradeneeded(None);
        db?.dyn_into::<IdbDatabase>().map_err(|_| unavailable("gave no database"))
    }

    /// A request's result, once it succeeds.
    async fn request(req: &IdbRequest) -> Result<JsValue, StorageError> {
        let done = Promise::new(&mut |resolve, reject| {
            req.set_onsuccess(Some(&resolve));
            req.set_onerror(Some(&reject));
        });
        let ended = JsFuture::from(done).await;
        req.set_onsuccess(None);
        req.set_onerror(None);
        match ended {
            Ok(_) => req.result().map_err(js_error),
            Err(_) => Err(req.error().ok().flatten().map_or_else(|| StorageError::Io("a request failed".into()), dom_error)),
        }
    }

    /// Every document.
    async fn read_all(db: &IdbDatabase) -> Result<BTreeMap<String, String>, StorageError> {
        let tx = db.transaction_with_str_and_mode(DOCS, IdbTransactionMode::Readonly).map_err(js_error)?;
        let store = tx.object_store(DOCS).map_err(js_error)?;
        let keys = store.get_all_keys().map_err(js_error)?;
        let values = store.get_all().map_err(js_error)?;
        let (keys, values) = (Array::from(&request(&keys).await?), Array::from(&request(&values).await?));
        Ok(keys.iter().zip(values.iter()).filter_map(|(k, v)| Some((k.as_string()?, v.as_string()?))).collect())
    }

    // -----------------------------------------------------------------------------------------------------
    // Writing behind

    /// Something changed: a flush is queued as a microtask, which runs when the frame's task ends, so everything a
    /// frame writes commits in one transaction.
    fn changed(o: &mut Open) {
        if o.queued {
            return;
        }
        o.queued = true;
        HANDLERS.with_borrow(|h| {
            if let Some(h) = h {
                let _ = Promise::resolve(&JsValue::UNDEFINED).then(&h.flush);
            }
        });
    }

    /// Commits what changed, one transaction a batch.
    fn flush(o: &mut Open) {
        o.queued = false;
        for batch in o.docs.take() {
            match transaction(&o.db, &batch) {
                Ok(tx) => {
                    HANDLERS.with_borrow(|h| {
                        if let Some(h) = h {
                            tx.set_oncomplete(Some(h.complete.as_ref().unchecked_ref()));
                            tx.set_onabort(Some(h.abort.as_ref().unchecked_ref()));
                        }
                    });
                    o.flying.push((tx, batch.id));
                }
                Err(e) => {
                    warn!("storage: a write failed: {e}");
                    o.docs.done(batch.id, Err(e));
                }
            }
        }
    }

    fn transaction(db: &IdbDatabase, b: &Batch) -> Result<IdbTransaction, StorageError> {
        let tx = db.transaction_with_str_and_mode(DOCS, IdbTransactionMode::Readwrite).map_err(js_error)?;
        let fill = || -> Result<(), JsValue> {
            let store = tx.object_store(DOCS)?;
            for k in &b.deletes {
                store.delete(&k.into())?;
            }
            for (k, text) in &b.puts {
                store.put_with_key(&text.into(), &k.into())?;
            }
            Ok(())
        };
        if let Err(e) = fill() {
            let _ = tx.abort();
            return Err(js_error(e));
        }
        // Committed now, not when the task ends: a page going away has its last changes on their way. Called by
        // name (web-sys marks its binding deprecated); browsers without `commit` commit at the task's end anyway.
        if let Some(commit) = Reflect::get(&tx, &"commit".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()) {
            let _ = commit.call0(&tx);
        }
        Ok(tx)
    }

    /// A transaction committed or failed: its batch hears it.
    fn finished(e: web_sys::Event, ok: bool) {
        let Some(tx) = e.target().and_then(|t| t.dyn_into::<IdbTransaction>().ok()) else { return };
        with_open(|o| {
            let Some(k) = o.flying.iter().position(|(t, _)| t == &tx) else { return };
            let (_, id) = o.flying.remove(k);
            let result = if ok { Ok(()) } else { Err(tx.error().map_or_else(|| StorageError::Io("a write was cut off".into()), dom_error)) };
            match &result {
                Ok(()) => estimate(o),
                Err(e) => warn!("storage: a write failed: {e}"),
            }
            o.docs.done(id, result);
            if o.docs.retry_due() {
                changed(o);
            }
        });
    }

    /// Writes the kept documents and commits every change when the page goes away (`pagehide`: a reload, a closed
    /// tab, another page) or is hidden (a background tab gets no frames and may be discarded without `pagehide`).
    fn listen() {
        if LISTENING.replace(true) {
            return;
        }
        let Some(window) = web_sys::window() else { return };
        let on_leave = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
            let hidden = web_sys::window().and_then(|w| w.document()).is_some_and(|d| d.hidden());
            if e.type_() != "pagehide" && !hidden {
                return;
            }
            with_open(|o| {
                for (k, text) in std::mem::take(&mut o.on_leave) {
                    o.docs.write(k, text());
                }
                flush(o);
            });
        });
        for event in ["pagehide", "visibilitychange"] {
            let _ = window.add_event_listener_with_callback(event, on_leave.as_ref().unchecked_ref());
        }
        // The listener lives as long as the page.
        on_leave.forget();
    }

    /// The browser's storage manager, which only secure addresses (https, localhost) have.
    fn manager() -> Option<web_sys::StorageManager> {
        let navigator = web_sys::window()?.navigator();
        Reflect::get(&navigator, &"storage".into()).ok().filter(|m| !m.is_undefined() && !m.is_null()).map(JsCast::unchecked_into)
    }

    /// Asks the browser to keep the storage when the disk runs low (best effort: it may say no, and says nothing on
    /// an insecure address).
    fn persist() {
        let Some(p) = manager().and_then(|m| m.persist().ok()) else { return };
        spawn_local(async move {
            if let Ok(granted) = JsFuture::from(p).await {
                info!("storage: kept when the disk runs low: {}", if granted.as_bool() == Some(true) { "yes" } else { "no (the browser's choice)" });
            }
        });
    }

    /// Asks for the browser's estimate of what the origin uses and may use (after the start and every commit).
    fn estimate(o: &mut Open) {
        if o.estimating {
            return;
        }
        let Some(p) = manager().and_then(|m| m.estimate().ok()) else { return };
        o.estimating = true;
        spawn_local(async move {
            let got = JsFuture::from(p).await.ok().map(JsCast::unchecked_into::<web_sys::StorageEstimate>);
            with_open(|o| {
                o.estimating = false;
                if let Some(e) = got {
                    o.estimate = Some((e.get_usage().unwrap_or(0.0) as u64, e.get_quota().unwrap_or(0.0) as u64));
                }
            });
        });
    }

    /// The backend: the documents in memory, written behind into the database (see the module docs).
    pub struct IndexedDb;

    impl Backend for IndexedDb {
        fn list(&self, folder: &str) -> Result<Vec<String>, StorageError> {
            Ok(with_open(|o| o.docs.list(folder)).unwrap_or_default())
        }

        fn read(&self, folder: &str, name: &str) -> Result<Option<String>, StorageError> {
            Ok(with_open(|o| o.docs.read(&key(folder, name)).cloned()).flatten())
        }

        fn write(&mut self, folder: &str, name: &str, text: &str) -> Result<(), StorageError> {
            with_open(|o| {
                let k = key(folder, name);
                // Written: nothing is waiting for the page to go away.
                o.on_leave.remove(&k);
                o.docs.write(k, text.to_string());
                changed(o);
            });
            Ok(())
        }

        fn delete(&mut self, folder: &str, name: &str) -> Result<(), StorageError> {
            with_open(|o| {
                let k = key(folder, name);
                o.on_leave.remove(&k);
                o.docs.delete(k);
                changed(o);
            });
            Ok(())
        }

        fn location(&self) -> String {
            let origin = web_sys::window().and_then(|w| w.location().origin().ok()).unwrap_or_default();
            format!("the browser's IndexedDB of {origin}")
        }

        fn keeps_on_leave(&self) -> bool {
            true
        }

        fn keep_on_leave(&mut self, folder: &str, name: &str, text: Box<dyn FnOnce() -> String>) {
            listen();
            with_open(|o| o.on_leave.insert(key(folder, name), text));
        }

        fn failure(&self) -> Option<StorageError> {
            with_open(|o| o.docs.failure().cloned()).flatten()
        }

        fn waiting(&self, folder: &str, name: &str) -> bool {
            with_open(|o| o.docs.waiting(&key(folder, name))).unwrap_or(false)
        }

        fn usage(&self) -> Option<Usage> {
            with_open(|o| Usage { documents: o.docs.bytes(), estimate: o.estimate })
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::preload;

pub struct StoragePlugin;

impl Plugin for StoragePlugin {
    fn build(&self, app: &mut App) {
        let storage = Storage::open();
        match &storage.fallback {
            Some(why) => warn!("storage: {why}; documents are kept in memory only"),
            None => info!("storage: {}", storage.location()),
        }
        app.insert_resource(storage);
    }
}

/// The web's writing behind without the browser, for the tests of the crates over the storage.
#[cfg(test)]
pub(crate) mod testing {
    use std::collections::BTreeMap;

    use super::behind::{self, Batch, WriteBehind};
    use super::{Backend, StorageError};

    /// Commits batches into a fake database in order, the way IndexedDB runs them.
    pub fn commit_in(db: &mut BTreeMap<String, String>, wb: &mut WriteBehind, batches: Vec<Batch>) {
        for b in batches {
            for k in &b.deletes {
                db.remove(k);
            }
            db.extend(b.puts.iter().cloned());
            wb.done(b.id, Ok(()));
        }
    }

    /// The web's backend with a fake database (shared with the test, which commits or refuses its batches).
    #[derive(Default)]
    pub struct Behind(pub std::sync::Arc<std::sync::Mutex<(WriteBehind, BTreeMap<String, String>)>>);

    impl Behind {
        /// Commits what is out, and the retries a commit makes due; with `full` every batch that writes fails with
        /// the quota.
        pub fn commit(&self, full: bool) {
            let mut guard = self.0.lock().unwrap();
            let (wb, db) = &mut *guard;
            loop {
                let batches = wb.take();
                if batches.is_empty() {
                    break;
                }
                let mut failed = false;
                for b in batches {
                    if full && !b.puts.is_empty() {
                        wb.done(b.id, Err(StorageError::Full));
                        failed = true;
                    } else {
                        commit_in(db, wb, vec![b]);
                    }
                }
                // A failure takes no retry (the web's waits for a commit or a write).
                if failed {
                    break;
                }
            }
        }
    }

    impl Backend for Behind {
        fn list(&self, folder: &str) -> Result<Vec<String>, StorageError> {
            Ok(self.0.lock().unwrap().0.list(folder))
        }
        fn read(&self, folder: &str, name: &str) -> Result<Option<String>, StorageError> {
            Ok(self.0.lock().unwrap().0.read(&behind::key(folder, name)).cloned())
        }
        fn write(&mut self, folder: &str, name: &str, text: &str) -> Result<(), StorageError> {
            self.0.lock().unwrap().0.write(behind::key(folder, name), text.into());
            Ok(())
        }
        fn delete(&mut self, folder: &str, name: &str) -> Result<(), StorageError> {
            self.0.lock().unwrap().0.delete(behind::key(folder, name));
            Ok(())
        }
        fn location(&self) -> String {
            "behind".into()
        }
        fn failure(&self) -> Option<StorageError> {
            self.0.lock().unwrap().0.failure().cloned()
        }
        fn waiting(&self, folder: &str, name: &str) -> bool {
            self.0.lock().unwrap().0.waiting(&behind::key(folder, name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document that is its text; format 2 reads format 1 by upper-casing it (a stand-in migration).
    #[derive(Debug, PartialEq)]
    struct Note(String);

    impl Document for Note {
        const NAMESPACE: &'static str = "notes";
        const FORMAT: u32 = 2;
        fn to_ron(&self) -> String {
            format!("{:?}\n", self.0)
        }
        fn from_ron(text: &str, format: u32) -> Result<Self, String> {
            let s = text.trim().strip_prefix('"').and_then(|t| t.strip_suffix('"')).ok_or("not a string")?;
            Ok(Note(if format == 1 { s.to_uppercase() } else { s.to_string() }))
        }
    }

    fn exercise(storage: &mut Storage) {
        assert_eq!(storage.list("notes").unwrap(), Vec::<String>::new());
        assert_eq!(storage.read::<Note>("a").unwrap(), None);
        storage.write("b", &Note("two".into()), 20).unwrap();
        storage.write("a", &Note("one".into()), 10).unwrap();
        assert_eq!(storage.list("notes").unwrap(), ["a", "b"]);
        let a = storage.read::<Note>("a").unwrap().unwrap();
        assert_eq!(a, Stored { doc: Note("one".into()), saved: Some(10) });
        // Replace, delete, delete again.
        storage.write("a", &Note("uno".into()), 11).unwrap();
        assert_eq!(storage.read::<Note>("a").unwrap().unwrap().doc, Note("uno".into()));
        storage.delete::<Note>("a").unwrap();
        storage.delete::<Note>("a").unwrap();
        assert_eq!(storage.list("notes").unwrap(), ["b"]);
        // A document's parts: listed with it, not with the namespace; removed with `delete_parts`.
        storage.write("b/g1", &Note("p1".into()), 21).unwrap();
        storage.write("b/g0", &Note("p0".into()), 21).unwrap();
        storage.write("b-2/g0", &Note("other".into()), 21).unwrap();
        assert_eq!(storage.list("notes").unwrap(), ["b"]);
        assert_eq!(storage.parts::<Note>("b").unwrap(), ["g0", "g1"]);
        assert_eq!(storage.read::<Note>("b/g1").unwrap().unwrap().doc, Note("p1".into()));
        storage.delete_parts::<Note>("b").unwrap();
        storage.delete_parts::<Note>("b-2").unwrap();
        assert_eq!(storage.parts::<Note>("b").unwrap(), Vec::<String>::new());
        assert_eq!(storage.read::<Note>("b").unwrap().unwrap().doc, Note("two".into()));
        // Names are checked.
        for bad in ["", "../x", "A", "a/b/c", "a/", "/a", "a/B", "a b", &"x".repeat(65)] {
            assert_eq!(storage.write(bad, &Note("x".into()), 0), Err(StorageError::BadName(bad.into())), "{bad}");
        }
        assert!(storage.parts::<Note>("a/b").is_err());
    }

    #[test]
    fn memory_backend_lists_reads_writes_and_deletes() {
        exercise(&mut Storage::new(MemoryBackend::default()));
    }

    #[test]
    fn file_backend_lists_reads_writes_and_deletes() {
        let dir = std::env::temp_dir().join(format!("space-game-storage-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut storage = Storage::new(FileBackend::new(&dir));
        exercise(&mut storage);
        // One file per document, with the header line; no temporary files left.
        let text = std::fs::read_to_string(dir.join("notes/b.ron")).unwrap();
        assert_eq!(text, "// space-game: notes, format 2, saved 20\n\"two\"\n");
        let files: Vec<_> = std::fs::read_dir(dir.join("notes")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(files.len(), 1);
        // Files that are not documents are not listed.
        std::fs::write(dir.join("notes/README.txt"), "x").unwrap();
        std::fs::write(dir.join("notes/Bad Name.ron"), "x").unwrap();
        assert_eq!(storage.list("notes").unwrap(), ["b"]);
        assert!(storage.location().contains("space-game-storage-test"));
        // A part is a file in its document's folder.
        storage.write("b/g0", &Note("p0".into()), 22).unwrap();
        assert!(dir.join("notes/b/g0.ron").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn headers_migrate_old_formats_and_refuse_newer_ones() {
        let mut mem = MemoryBackend::default();
        mem.write("notes", "old", "// space-game: notes, format 1, saved 5\n\"quiet\"\n").unwrap();
        mem.write("notes", "plain", "\"no header\"").unwrap();
        mem.write("notes", "new", "// space-game: notes, format 3, saved 5, colour blue\n\"later\"\n").unwrap();
        mem.write("notes", "other", "// space-game: designs, format 1\n\"x\"\n").unwrap();
        mem.write("notes", "broken", "// space-game: notes, format 2\nnot ron").unwrap();
        mem.write("notes", "unversioned", "// space-game: notes, saved 5\n\"x\"").unwrap();
        let storage = Storage::new(mem);
        // Format 1 is migrated (upper-cased by the stand-in), a file without a header reads as format 1.
        assert_eq!(storage.read::<Note>("old").unwrap().unwrap(), Stored { doc: Note("QUIET".into()), saved: Some(5) });
        assert_eq!(storage.read::<Note>("plain").unwrap().unwrap(), Stored { doc: Note("NO HEADER".into()), saved: None });
        let newer = storage.read::<Note>("new").unwrap_err();
        assert_eq!(newer, StorageError::Newer { key: "notes/new".into(), format: 3, supported: 2 });
        assert!(newer.to_string().contains("newer version"));
        assert!(matches!(storage.read::<Note>("other"), Err(StorageError::Unreadable { .. })));
        let broken = storage.read::<Note>("broken").unwrap_err();
        assert_eq!(broken, StorageError::Unreadable { key: "notes/broken".into(), why: "not a string".into() });
        assert!(matches!(storage.read::<Note>("unversioned"), Err(StorageError::Unreadable { .. })));
    }

    /// A backend that keeps documents for the page going away (as the web's does): (namespace, name, text).
    #[derive(Default)]
    struct Keeping(std::sync::Arc<std::sync::Mutex<Vec<(String, String, String)>>>);

    impl Backend for Keeping {
        fn list(&self, _: &str) -> Result<Vec<String>, StorageError> {
            Ok(Vec::new())
        }
        fn read(&self, _: &str, _: &str) -> Result<Option<String>, StorageError> {
            Ok(None)
        }
        fn write(&mut self, _: &str, _: &str, _: &str) -> Result<(), StorageError> {
            Ok(())
        }
        fn delete(&mut self, _: &str, _: &str) -> Result<(), StorageError> {
            Ok(())
        }
        fn location(&self) -> String {
            "keeping".into()
        }
        fn keeps_on_leave(&self) -> bool {
            true
        }
        fn keep_on_leave(&mut self, namespace: &str, name: &str, text: Box<dyn FnOnce() -> String>) {
            self.0.lock().unwrap().push((namespace.into(), name.into(), text()));
        }
    }

    /// A document kept for the page going away is the text a write would store; only backends that keep
    /// anything make the document at all.
    #[test]
    fn documents_kept_for_leaving_are_stored_text() {
        let keeping = Keeping::default();
        let kept = keeping.0.clone();
        let mut storage = Storage::new(keeping);
        storage.keep_on_leave("a", || Note("later".into()));
        Storage::new(MemoryBackend::default()).keep_on_leave("a", || -> Note { panic!("a memory backend keeps nothing") });
        let kept = kept.lock().unwrap();
        let (ns, name, text) = &kept[0];
        assert_eq!((ns.as_str(), name.as_str()), ("notes", "a"));
        assert!(text.starts_with("// space-game: notes, format 2, saved ") && text.ends_with("\n\"later\"\n"), "{text}");
    }

    #[test]
    fn a_document_set_aside_keeps_its_text_as_stored() {
        let mut mem = MemoryBackend::default();
        mem.write("notes", "new", "// space-game: notes, format 3, saved 5\n\"later\"\n").unwrap();
        let mut storage = Storage::new(mem);
        storage.set_aside::<Note>("new", "new-unread").unwrap();
        // The copy is the same document: still from a newer build, unread.
        assert!(matches!(storage.read::<Note>("new-unread"), Err(StorageError::Newer { format: 3, .. })));
        storage.write("new", &Note("over".into()), 6).unwrap();
        assert!(matches!(storage.read::<Note>("new-unread"), Err(StorageError::Newer { .. })));
        // Nothing there: nothing to keep; names are checked.
        storage.set_aside::<Note>("missing", "missing-unread").unwrap();
        assert_eq!(storage.read::<Note>("missing-unread").unwrap(), None);
        assert!(matches!(storage.set_aside::<Note>("new", "../x"), Err(StorageError::BadName(_))));
    }

    // -----------------------------------------------------------------------------------------------------
    // Writing behind (the web's backend without the browser)

    use std::collections::BTreeMap;

    use super::behind::WriteBehind;
    use super::testing::{Behind, commit_in as commit};

    fn docs(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// Reads and lists come from memory at once; what goes out is each key's last state: the last write wins, a
    /// delete after a write deletes, a write after a delete writes; deletes go first, in a batch of their own.
    #[test]
    fn writes_behind_keep_the_last_state_of_each_key() {
        let mut db = docs(&[("designs/a", "1"), ("lab/r", "head"), ("lab/r/g0", "part")]);
        let mut wb = WriteBehind::new(db.clone());
        assert_eq!(wb.bytes(), 1 + 4 + 4);
        wb.write("designs/b".into(), "x".into());
        wb.write("designs/b".into(), "y".into());
        wb.delete("designs/a".into());
        wb.write("lab/r/g1".into(), "part 1".into());
        wb.write("lab/r-2".into(), "other".into());
        assert_eq!(wb.list("designs"), ["b"]);
        assert_eq!(wb.list("lab"), ["r", "r-2"], "a document's parts are not the namespace's");
        assert_eq!(wb.list("lab/r"), ["g0", "g1"]);
        assert_eq!(wb.read("designs/b").map(String::as_str), Some("y"));
        assert_eq!(wb.bytes(), 1 + 4 + 4 + 6 + 5);
        let batches = wb.take();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].deletes, ["designs/a"]);
        assert!(batches[0].puts.is_empty() && batches[1].deletes.is_empty());
        assert_eq!(batches[1].puts.iter().find(|(k, _)| k == "designs/b").unwrap().1, "y");
        commit(&mut db, &mut wb, batches);
        assert!(wb.take().is_empty(), "nothing changed since");
        // Written then deleted before it went out: a delete; deleted then written: a write.
        wb.write("designs/c".into(), "c".into());
        wb.delete("designs/c".into());
        wb.delete("designs/b".into());
        wb.write("designs/b".into(), "again".into());
        let b = wb.take();
        commit(&mut db, &mut wb, b);
        // Two batches out at once commit in order: the later one's text stays.
        wb.write("designs/b".into(), "first".into());
        let one = wb.take();
        wb.write("designs/b".into(), "second".into());
        let two = wb.take();
        commit(&mut db, &mut wb, one);
        commit(&mut db, &mut wb, two);
        let mem: BTreeMap<String, String> = ["designs", "lab", "lab/r"].iter().flat_map(|f| wb.list(f).into_iter().map(move |n| format!("{f}/{n}"))).map(|k| (k.clone(), wb.read(&k).unwrap().clone())).collect();
        assert_eq!(db, mem, "the database holds what memory does");
        assert_eq!(db["designs/b"], "second");
        assert!(!db.contains_key("designs/c") && !db.contains_key("designs/a"));
        assert!(wb.failure().is_none());
    }

    /// A failed batch (the quota) leaves its keys unsaved: they wait, are named, and go out again with the next
    /// batch (once, not while a retry is out); deletes still go first on their own, so deleting makes room; once
    /// everything is kept, the failure is gone.
    #[test]
    fn a_failed_batch_waits_and_goes_out_again() {
        let mut db = BTreeMap::new();
        let mut wb = WriteBehind::new(BTreeMap::new());
        wb.write("lab/old".into(), "old run".into());
        let b = wb.take();
        commit(&mut db, &mut wb, b);
        wb.write("lab/r/g0".into(), "a big part".into());
        wb.write("lab/r".into(), "head".into());
        let b = wb.take();
        assert_eq!(b.len(), 1);
        wb.done(b[0].id, Err(StorageError::Full));
        assert_eq!(wb.failure(), Some(&StorageError::Full));
        assert!(wb.waiting("lab/r") && !wb.waiting("lab/old") && !wb.waiting("lab/r-2"));
        assert!(!wb.retry_due(), "nothing committed since");
        // The player deletes a run: the delete goes on its own; once it committed (room made) the unsaved keys go
        // out again.
        wb.delete("lab/old".into());
        let b = wb.take();
        assert_eq!((b.len(), b[0].deletes.clone()), (1, vec!["lab/old".to_string()]));
        db.remove("lab/old");
        wb.done(b[0].id, Ok(()));
        assert!(wb.failure().is_none() && wb.waiting("lab/r") && wb.retry_due(), "the storage takes writes again; they wait");
        let b = wb.take();
        assert_eq!(b[0].puts.len(), 2);
        assert!(!wb.retry_due() && wb.take().is_empty(), "a retry out is not sent twice");
        db.extend(b[0].puts.iter().cloned());
        wb.done(b[0].id, Ok(()));
        assert!(wb.failure().is_none() && !wb.waiting("lab/r"));
        assert_eq!(db.keys().collect::<Vec<_>>(), ["lab/r", "lab/r/g0"]);
        // A write while keys wait takes them along (when it deletes nothing).
        wb.write("lab/r/g1".into(), "next".into());
        let b = wb.take();
        wb.done(b[0].id, Err(StorageError::Full));
        wb.write("designs/a".into(), "small".into());
        let b = wb.take();
        assert_eq!(b[0].puts.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["designs/a", "lab/r/g1"]);
    }

    /// Through `Storage`: a write behind that failed comes back with the next write, `unsaved` names what waits,
    /// deleting is never refused, and once a commit made room writes are fine and what waited is kept.
    #[test]
    fn a_failed_write_behind_reaches_the_caller() {
        let backend = Behind::default();
        let shared = Behind(backend.0.clone());
        let mut storage = Storage::new(backend);
        storage.write("a", &Note("one".into()), 1).unwrap();
        shared.commit(true);
        assert_eq!(storage.unsaved::<Note>("a"), Some(StorageError::Full));
        assert_eq!(storage.unsaved::<Note>("b"), None);
        assert_eq!(storage.write("b", &Note("two".into()), 2), Err(StorageError::Full), "the next write hears of it");
        assert_eq!(storage.read::<Note>("b").unwrap().unwrap().doc, Note("two".into()), "kept in memory all the same");
        storage.delete::<Note>("b").unwrap();
        shared.commit(false);
        assert_eq!(storage.unsaved::<Note>("a"), None);
        storage.write("c", &Note("three".into()), 3).unwrap();
        shared.commit(false);
        let db = shared.0.lock().unwrap().1.clone();
        assert_eq!(db.keys().collect::<Vec<_>>(), ["notes/a", "notes/c"]);
    }

    #[test]
    fn usage_warns_when_little_room_is_left() {
        let at = |used: u64, quota: u64| Usage { documents: 0, estimate: Some((used, quota)) };
        assert!(!at(10 << 20, 2 << 30).low());
        assert!(at((2 << 30) - (10 << 20), 2 << 30).low(), "under 64 MB left");
        assert!(at(95 << 30, 100 << 30).low(), "over nine tenths used");
        assert!(!Usage { documents: 5, estimate: None }.low(), "no quota known: no warning");
    }
}
