//! The player's storage: named text documents (RON) in namespaces, each with a format version, behind one small
//! trait so the game's other crates stay free of I/O (`sim` and `data` never touch it). Written 13 Oct 2026 for
//! the drydock's designs (`designs/<id>`, the working design `drydock/working`) and the player's behaviors
//! (`behaviors/<id>`, `drydock/behaviors.rs`); save games since (`saves/<slot>`, ADR-0024: saves are versioned, with
//! migrations), the settings (`settings/controls`, `settings/display`, `settings.rs`, GDR-0090) and the evolution
//! lab's runs (`lab/<id>`, `evolution.rs`, GDR-0092).
//!
//! * A **key** is `namespace/name`; both parts are `a-z`, `0-9`, `_` and `-`, starting with a letter or digit, at
//!   most 64 characters (safe as file names and storage keys on every platform).
//! * A **document** ([`Document`]) is RON text. Stored, it gets one header line, a RON comment, so the rest stays
//!   valid RON (a saved design is a content ship file):
//!   `// space-game: designs, format 1, saved 1760351234` (namespace, format version, Unix seconds). Reading
//!   checks the namespace, refuses formats newer than the build knows ([`StorageError::Newer`]) and hands older
//!   ones to [`Document::from_ron`], where migrations live. A file without the header line (a ship file copied
//!   in by hand) reads as format 1.
//! * **Backends** ([`Backend`]): the browser's local storage (web, keys `space-game/<key>`), files in the user's
//!   data directory (desktop: `<data dir>/<namespace>/<name>.ron`), and memory (tests, and the fallback when the
//!   platform has no storage, e.g. Android until its backend lands: the files backend with the app's internal
//!   data path). [`Storage::open`] picks one.
//! * **Errors** ([`StorageError`]) say what went wrong in words the UI can show: storage unavailable or full
//!   (the browser's quota), unreadable documents, documents from a newer build.

use std::collections::BTreeMap;
use std::fmt;

use bevy::prelude::*;

/// The marker that starts a stored document's header line.
const HEADER: &str = "// space-game: ";
/// The longest namespace or name.
const MAX_NAME: usize = 64;

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

/// Where documents live: one per platform, plus memory. Keys are checked by [`Storage`] before they get here.
pub trait Backend: Send + Sync + 'static {
    /// The names of the documents in a namespace, sorted.
    fn list(&self, namespace: &str) -> Result<Vec<String>, StorageError>;
    /// A document's text, `None` if there is none.
    fn read(&self, namespace: &str, name: &str) -> Result<Option<String>, StorageError>;
    /// Creates or replaces a document.
    fn write(&mut self, namespace: &str, name: &str, text: &str) -> Result<(), StorageError>;
    /// Removes a document; removing a missing one is fine.
    fn delete(&mut self, namespace: &str, name: &str) -> Result<(), StorageError>;
    /// Where the documents are, for messages and the docs ("browser storage of http://…", a directory).
    fn location(&self) -> String;
    /// Whether this backend writes documents kept with [`Backend::keep_on_leave`] when the page goes away.
    fn keeps_on_leave(&self) -> bool {
        false
    }
    /// Keeps a document's text (made when it is needed) to write if the page goes away before the next write of
    /// that document (web). Other backends keep nothing.
    fn keep_on_leave(&mut self, _namespace: &str, _name: &str, _text: Box<dyn FnOnce() -> String>) {}
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

    /// The platform's storage: the browser's local storage on the web, files in the user's data directory on
    /// desktop. Falls back to memory (with [`Storage::fallback`] set) when the platform has none.
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

    /// The names of the documents in a namespace, sorted.
    pub fn list(&self, namespace: &str) -> Result<Vec<String>, StorageError> {
        check_name(namespace)?;
        self.backend.list(namespace)
    }

    /// Reads and migrates a document; `None` if there is none.
    pub fn read<D: Document>(&self, name: &str) -> Result<Option<Stored<D>>, StorageError> {
        check_name(name)?;
        let Some(text) = self.backend.read(D::NAMESPACE, name)? else { return Ok(None) };
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

    /// Writes a document in the current format, stamped with the time `saved` (Unix seconds).
    pub fn write<D: Document>(&mut self, name: &str, doc: &D, saved: u64) -> Result<(), StorageError> {
        check_name(name)?;
        self.backend.write(D::NAMESPACE, name, &stored_text(doc, saved))
    }

    /// Keeps the document `doc()` makes, to be written under `name` if the page goes away before its next write
    /// (web: a reload or a closed tab runs no more frames, so a write waiting for the autosave's second would be
    /// lost; the page's `pagehide` writes it). `doc` runs only where that is done; the text is made only then.
    /// Desktop needs none (it writes a waiting autosave when the app closes).
    pub fn keep_on_leave<D: Document + 'static>(&mut self, name: &str, doc: impl FnOnce() -> D) {
        if self.backend.keeps_on_leave() && valid_name(name) {
            let doc = doc();
            self.backend.keep_on_leave(D::NAMESPACE, name, Box::new(move || stored_text(&doc, now_unix())));
        }
    }

    /// Removes a document.
    pub fn delete<D: Document>(&mut self, name: &str) -> Result<(), StorageError> {
        check_name(name)?;
        self.backend.delete(D::NAMESPACE, name)
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
    fn list(&self, namespace: &str) -> Result<Vec<String>, StorageError> {
        Ok(self.docs.keys().filter(|(ns, _)| ns == namespace).map(|(_, n)| n.clone()).collect())
    }

    fn read(&self, namespace: &str, name: &str) -> Result<Option<String>, StorageError> {
        Ok(self.docs.get(&(namespace.to_string(), name.to_string())).cloned())
    }

    fn write(&mut self, namespace: &str, name: &str, text: &str) -> Result<(), StorageError> {
        self.docs.insert((namespace.into(), name.into()), text.into());
        Ok(())
    }

    fn delete(&mut self, namespace: &str, name: &str) -> Result<(), StorageError> {
        self.docs.remove(&(namespace.to_string(), name.to_string()));
        Ok(())
    }

    fn location(&self) -> String {
        "memory (gone when the game closes)".into()
    }
}

#[cfg(target_arch = "wasm32")]
fn platform_backend() -> Result<Box<dyn Backend>, StorageError> {
    web::LocalStorage::open().map(|b| Box::new(b) as Box<dyn Backend>)
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_backend() -> Result<Box<dyn Backend>, StorageError> {
    let dir = data_dir().ok_or_else(|| StorageError::Unavailable("no user data directory on this platform".into()))?;
    Ok(Box::new(FileBackend::new(dir)))
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

/// Documents as files: `<root>/<namespace>/<name>.ron`. Writes go to a temporary file first and are renamed
/// into place, so a crash never leaves half a document.
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

    fn path(&self, namespace: &str, name: &str) -> std::path::PathBuf {
        self.root.join(namespace).join(format!("{name}.ron"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn io_error(e: std::io::Error) -> StorageError {
    if e.kind() == std::io::ErrorKind::StorageFull { StorageError::Full } else { StorageError::Io(e.to_string()) }
}

#[cfg(not(target_arch = "wasm32"))]
impl Backend for FileBackend {
    fn list(&self, namespace: &str) -> Result<Vec<String>, StorageError> {
        let dir = match std::fs::read_dir(self.root.join(namespace)) {
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

    fn read(&self, namespace: &str, name: &str) -> Result<Option<String>, StorageError> {
        match std::fs::read_to_string(self.path(namespace, name)) {
            Ok(t) => Ok(Some(t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_error(e)),
        }
    }

    fn write(&mut self, namespace: &str, name: &str, text: &str) -> Result<(), StorageError> {
        let path = self.path(namespace, name);
        std::fs::create_dir_all(self.root.join(namespace)).map_err(io_error)?;
        let tmp = path.with_extension("ron.tmp");
        std::fs::write(&tmp, text).map_err(io_error)?;
        std::fs::rename(&tmp, &path).map_err(io_error)
    }

    fn delete(&mut self, namespace: &str, name: &str) -> Result<(), StorageError> {
        match std::fs::remove_file(self.path(namespace, name)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_error(e)),
            _ => Ok(()),
        }
    }

    fn location(&self) -> String {
        self.root.display().to_string()
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    use super::{Backend, StorageError};

    /// Every key starts with this, so other pages of the origin don't mix in.
    const PREFIX: &str = "space-game/";

    thread_local! {
        /// Documents to write if the page goes away before their next write: key → its text, made then.
        static ON_LEAVE: RefCell<BTreeMap<String, Box<dyn FnOnce() -> String>>> = RefCell::new(BTreeMap::new());
        static LISTENING: Cell<bool> = const { Cell::new(false) };
    }

    /// Writes the kept documents when the page goes away (`pagehide`: a reload, a closed tab, another page) or is
    /// hidden (a background tab gets no frames and may be discarded without `pagehide`).
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
            let kept = ON_LEAVE.with_borrow_mut(std::mem::take);
            if let Ok(s) = store() {
                for (key, text) in kept {
                    let _ = s.set_item(&key, &text());
                }
            }
        });
        for event in ["pagehide", "visibilitychange"] {
            let _ = window.add_event_listener_with_callback(event, on_leave.as_ref().unchecked_ref());
        }
        // The listener lives as long as the page.
        on_leave.forget();
    }

    /// The browser's local storage (`localStorage`). It belongs to the origin (scheme, host and port: the
    /// copies served on :8080 and :8081 don't share it) and holds about 5 MB per origin.
    pub struct LocalStorage;

    impl LocalStorage {
        pub fn open() -> Result<Self, StorageError> {
            store().map(|_| Self)
        }
    }

    fn store() -> Result<web_sys::Storage, StorageError> {
        let unavailable = || StorageError::Unavailable("the browser's local storage is blocked".into());
        let window = web_sys::window().ok_or_else(unavailable)?;
        window.local_storage().ok().flatten().ok_or_else(unavailable)
    }

    fn key(namespace: &str, name: &str) -> String {
        format!("{PREFIX}{namespace}/{name}")
    }

    fn js_error(e: wasm_bindgen::JsValue) -> StorageError {
        match e.dyn_ref::<web_sys::DomException>() {
            Some(d) if d.name() == "QuotaExceededError" => StorageError::Full,
            Some(d) => StorageError::Io(d.message()),
            None => StorageError::Io(format!("{e:?}")),
        }
    }

    impl Backend for LocalStorage {
        fn list(&self, namespace: &str) -> Result<Vec<String>, StorageError> {
            let s = store()?;
            let prefix = key(namespace, "");
            let n = s.length().map_err(js_error)?;
            let mut names: Vec<String> = (0..n)
                .filter_map(|i| s.key(i).ok().flatten())
                .filter_map(|k| k.strip_prefix(&prefix).map(str::to_string))
                .filter(|n| super::valid_name(n))
                .collect();
            names.sort();
            Ok(names)
        }

        fn read(&self, namespace: &str, name: &str) -> Result<Option<String>, StorageError> {
            store()?.get_item(&key(namespace, name)).map_err(js_error)
        }

        fn write(&mut self, namespace: &str, name: &str, text: &str) -> Result<(), StorageError> {
            let key = key(namespace, name);
            store()?.set_item(&key, text).map_err(js_error)?;
            // Written: nothing is waiting for the page to go away.
            ON_LEAVE.with_borrow_mut(|kept| kept.remove(&key));
            Ok(())
        }

        fn delete(&mut self, namespace: &str, name: &str) -> Result<(), StorageError> {
            store()?.remove_item(&key(namespace, name)).map_err(js_error)
        }

        fn location(&self) -> String {
            let origin = web_sys::window().and_then(|w| w.location().origin().ok()).unwrap_or_default();
            format!("the browser's local storage of {origin}")
        }

        fn keeps_on_leave(&self) -> bool {
            true
        }

        fn keep_on_leave(&mut self, namespace: &str, name: &str, text: Box<dyn FnOnce() -> String>) {
            listen();
            ON_LEAVE.with_borrow_mut(|kept| kept.insert(key(namespace, name), text));
        }
    }
}

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
        // Names are checked.
        for bad in ["", "../x", "A", "a/b", "a b", &"x".repeat(65)] {
            assert_eq!(storage.write(bad, &Note("x".into()), 0), Err(StorageError::BadName(bad.into())), "{bad}");
        }
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
}
