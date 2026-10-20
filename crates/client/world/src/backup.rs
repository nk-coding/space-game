//! Backups of the player's documents (ADR-0050, 20 Oct 2026): EXPORT writes everything the storage holds for this
//! game into one file, IMPORT checks such a file in full and replaces the saves with it. The web build is a player
//! platform (ADR-0048) and a browser may wipe its IndexedDB; a file the player keeps is the way back.
//!
//! * **The file** is RON, one per export (`space-game-<save>-<date>.ron`): a comment, then
//!   `(backup: 1, game: "0.1.0", commit: Some("…"), exported: <Unix seconds>, date: "2026-10-20 14:03 UTC",
//!   save: Some("career"), left_out: ["lab"], documents: { "<namespace>/<name>": r#"<stored text>"#, … })`.
//!   Each document is its stored text as it is (its header line, then its RON) in a raw string, so the file reads and
//!   diffs like the documents themselves. `backup` is [`FORMAT`]; `left_out` names the namespaces the export left out
//!   (an import leaves them as they are).
//! * **What goes in** ([`gather`]): every document of a known kind ([`BackupKinds`]: designs, the drydock's drafts,
//!   behaviors, the save the game uses, settings, the lab's runs with their parts, THE LADDER's ratings) that reads
//!   in this build; a document that doesn't read (a newer format, a damaged one) is left out and named. Only the save
//!   the game is using goes in (`saves/career`, or the dev mode's `saves/dev`); the other is never touched. The lab's
//!   runs may be left out ([`Include::lab`]: they are large and can be bred again).
//! * **Reading a file** ([`Backup::read`]) checks everything before anything is touched: the size ([`MAX_FILE`]), the
//!   backup format (newer and older refused), every key, and every document against its kind (its header, format
//!   and RON, as the game reads it). Unknown documents, a second save and a dev-mode save for the career are refused.
//! * **Importing** ([`plan`], [`Importing`]): the storage is frozen first (the running game writes nothing more: its
//!   state is the old one), then every document of the file is written (on the web in one transaction, so a failure
//!   leaves the stored saves as they were); only once that has committed are the documents the file doesn't hold
//!   deleted (those of the namespaces it left out stay). Then the game starts again on the new saves (the web
//!   reloads the page; desktop asks the player to start it again). The screen saves a copy of what was there before
//!   (an export) first.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bevy::prelude::*;
use serde::Deserialize;

use crate::storage::{Document, Storage, StorageError, parse_stored};

/// The backup format this build writes and reads.
pub const FORMAT: u32 = 1;
/// The oldest backup format this build still reads.
pub const OLDEST: u32 = 1;
/// The largest file an import reads (bytes; PLACEHOLDER: about forty DEEP lab runs).
pub const MAX_FILE: usize = 256 << 20;
/// The namespace of the saves: only the one the game uses is exported or replaced.
const SAVES: &str = "saves";
/// The lab's runs, which an export may leave out.
pub const LAB: &str = "lab";

/// A kind of document a backup holds: where it lives (a namespace and a name pattern) and how it is checked.
#[derive(Clone)]
pub struct Kind {
    pub namespace: &'static str,
    /// The name: `*` any name, a fixed name, or `*/<part>` a document's part (`*/g*`: parts starting with `g`).
    pub pattern: &'static str,
    check: fn(&str, &str) -> Result<(), StorageError>,
}

impl Kind {
    /// A kind of document `D` under names matching `pattern`.
    pub fn of<D: Document>(pattern: &'static str) -> Self {
        Kind { namespace: D::NAMESPACE, pattern, check: |name, text| parse_stored::<D>(name, text).map(|_| ()) }
    }

    fn matches(&self, namespace: &str, name: &str) -> bool {
        if namespace != self.namespace {
            return false;
        }
        let seg = |pattern: &str, s: &str| match pattern.strip_suffix('*') {
            Some(prefix) => s.starts_with(prefix),
            None => pattern == s,
        };
        match (self.pattern.split_once('/'), name.split_once('/')) {
            (None, None) => seg(self.pattern, name),
            (Some((p0, p1)), Some((n0, n1))) => seg(p0, n0) && seg(p1, n1),
            _ => false,
        }
    }
}

/// The kinds of documents backups hold (a resource): the world's, and those the screens add (the BEHAVIOR screen's
/// draft) with [`BackupKinds::add`].
#[derive(Resource, Clone)]
pub struct BackupKinds(Vec<Kind>);

impl Default for BackupKinds {
    fn default() -> Self {
        Self::standard()
    }
}

impl BackupKinds {
    /// The world's documents.
    pub fn standard() -> Self {
        use crate::career::CareerDoc;
        use crate::evolution::ladder::RatingDoc;
        use crate::evolution::store::{ConfigDoc, PartDoc, RunDoc};
        use crate::player::behaviors::BehaviorDoc;
        use crate::player::library::{DesignDoc, WORKING, WorkingDoc};
        use crate::settings::{ControlsDoc, DisplaySettings};
        BackupKinds(vec![
            Kind::of::<CareerDoc>("*"),
            Kind::of::<DesignDoc>("*"),
            Kind::of::<WorkingDoc>(WORKING),
            Kind::of::<BehaviorDoc>("*"),
            Kind::of::<ControlsDoc>("controls"),
            Kind::of::<DisplaySettings>("display"),
            Kind::of::<RunDoc>("*"),
            Kind::of::<ConfigDoc>("*/config"),
            Kind::of::<PartDoc>("*/g*"),
            Kind::of::<RatingDoc>("*"),
        ])
    }

    pub fn add(&mut self, kind: Kind) {
        self.0.push(kind);
    }

    /// The kind of the document under `key` (`namespace/name[/part]`).
    fn find(&self, key: &str) -> Option<&Kind> {
        let (namespace, name) = key.split_once('/')?;
        self.0.iter().find(|k| k.matches(namespace, name))
    }

    /// The namespaces, in order.
    fn namespaces(&self) -> BTreeSet<&'static str> {
        self.0.iter().map(|k| k.namespace).collect()
    }

    /// Checks a stored text as the document its key names.
    fn check(&self, key: &str, text: &str) -> Result<(), BackupError> {
        let kind = self.find(key).ok_or_else(|| BackupError::Unknown(key.into()))?;
        let name = &key[kind.namespace.len() + 1..];
        (kind.check)(name, text).map_err(BackupError::Document)
    }
}

/// What to put in an export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Include {
    /// The lab's runs (large; the lab can breed them again).
    pub lab: bool,
}

/// Why a file doesn't import, in words for the status line.
#[derive(Clone, Debug, PartialEq)]
pub enum BackupError {
    /// Over [`MAX_FILE`].
    TooLarge,
    /// Not a backup at all (and why it doesn't read as one).
    NotABackup(String),
    /// A backup format this build doesn't read yet.
    Newer(u32),
    /// A backup format this build no longer reads.
    Older(u32),
    /// A document of no kind this build knows (written by a newer version?).
    Unknown(String),
    /// A document doesn't read as its kind.
    Document(StorageError),
    /// The file's saves don't fit what it says (none named, or more than one).
    Saves(String),
    /// The dev mode's save would become the career.
    DevIntoCareer,
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupError::TooLarge => write!(f, "the file is too large for a backup (over {} MB)", MAX_FILE >> 20),
            BackupError::NotABackup(why) => write!(f, "not a space-game backup ({why})"),
            BackupError::Newer(v) => write!(f, "a backup from a newer version of the game (backup format {v}, this one reads up to {FORMAT})"),
            BackupError::Older(v) => write!(f, "a backup from an older version of the game (backup format {v}, this one reads from {OLDEST})"),
            BackupError::Unknown(key) => write!(f, "{key} is no document this version of the game knows"),
            BackupError::Document(e) => e.fmt(f),
            BackupError::Saves(why) => f.write_str(why),
            BackupError::DevIntoCareer => f.write_str("it holds the dev mode's save: import it with ?dev=1"),
        }
    }
}

/// A backup: what a file holds, or what an export writes.
#[derive(Clone, Debug, PartialEq)]
pub struct Backup {
    /// The backup format ([`FORMAT`]).
    pub format: u32,
    /// The game's version that wrote it, and its commit when the build knew it.
    pub game: String,
    pub commit: Option<String>,
    /// When (Unix seconds).
    pub exported: u64,
    /// The save it holds (`career`, `dev`), if any.
    pub save: Option<String>,
    /// The namespaces it left out (an import leaves them as they are).
    pub left_out: Vec<String>,
    /// Every document: its key and its stored text.
    pub documents: BTreeMap<String, String>,
}

/// The file's form.
#[derive(Deserialize)]
struct FileForm {
    backup: u32,
    game: String,
    commit: Option<String>,
    exported: u64,
    #[allow(dead_code)]
    date: String,
    save: Option<String>,
    left_out: Vec<String>,
    documents: BTreeMap<String, String>,
}

/// Its first field alone, read first so a newer format is named as one.
#[derive(Deserialize)]
struct Probe {
    backup: u32,
}

/// An export and the documents it left out (each with why: it doesn't read in this build).
#[derive(Debug)]
pub struct Gathered {
    pub backup: Backup,
    pub skipped: Vec<(String, String)>,
}

/// This build's version, and its commit when the build was told it (`SPACE_GAME_COMMIT`, or GitHub Actions'
/// `GITHUB_SHA` for the published build).
pub fn this_build() -> (String, Option<String>) {
    let commit = option_env!("SPACE_GAME_COMMIT").or(option_env!("GITHUB_SHA")).filter(|c| !c.is_empty());
    (env!("CARGO_PKG_VERSION").to_string(), commit.map(|c| c.chars().take(12).collect()))
}

/// Everything the storage holds for the game using save `slot`: see the module docs.
pub fn gather(storage: &Storage, kinds: &BackupKinds, slot: &str, include: Include, now: u64) -> Result<Gathered, StorageError> {
    let mut documents = BTreeMap::new();
    let mut skipped = Vec::new();
    let mut left_out = Vec::new();
    for namespace in kinds.namespaces() {
        if namespace == LAB && !include.lab {
            left_out.push(LAB.to_string());
            continue;
        }
        for name in storage.names_with_parts(namespace)? {
            if namespace == SAVES && name != slot {
                continue;
            }
            let key = format!("{namespace}/{name}");
            // Documents of no kind (a working document set aside unread) aren't the player's to keep.
            if kinds.find(&key).is_none() {
                continue;
            }
            let Some(text) = storage.read_stored(&key)? else { continue };
            match kinds.check(&key, &text) {
                Ok(()) => {
                    documents.insert(key, text);
                }
                Err(e) => skipped.push((key, e.to_string())),
            }
        }
    }
    let save = documents.contains_key(&format!("{SAVES}/{slot}")).then(|| slot.to_string());
    let (game, commit) = this_build();
    Ok(Gathered { backup: Backup { format: FORMAT, game, commit, exported: now, save, left_out, documents }, skipped })
}

impl Backup {
    /// The file's text (see the module docs).
    pub fn to_text(&self) -> String {
        let q = |s: &str| ron::to_string(s).unwrap_or_default();
        let mut out = String::new();
        out.push_str("// space-game backup: the player's documents, exported by the game (SETTINGS › SAVES › EXPORT).\n");
        out.push_str("// IMPORT in SETTINGS › SAVES replaces the saves with them. Each document is as the game stores it.\n");
        out.push_str("(\n");
        out.push_str(&format!("    backup: {},\n", self.format));
        out.push_str(&format!("    game: {},\n", q(&self.game)));
        out.push_str(&format!("    commit: {},\n", self.commit.as_deref().map_or("None".into(), |c| format!("Some({})", q(c)))));
        out.push_str(&format!("    exported: {},\n", self.exported));
        out.push_str(&format!("    date: {},\n", q(&date(self.exported))));
        out.push_str(&format!("    save: {},\n", self.save.as_deref().map_or("None".into(), |s| format!("Some({})", q(s)))));
        out.push_str(&format!("    left_out: [{}],\n", self.left_out.iter().map(|n| q(n)).collect::<Vec<_>>().join(", ")));
        out.push_str("    documents: {\n");
        for (key, text) in &self.documents {
            out.push_str(&format!("        {}: {},\n", q(key), raw(text)));
        }
        out.push_str("    },\n)\n");
        out
    }

    /// Reads a file and checks all of it (see the module docs); nothing is touched.
    pub fn read(text: &str, kinds: &BackupKinds) -> Result<Backup, BackupError> {
        if text.len() > MAX_FILE {
            return Err(BackupError::TooLarge);
        }
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let options = ron::Options::default();
        let probe: Probe = options.from_str(text).map_err(|e| BackupError::NotABackup(e.to_string()))?;
        if probe.backup > FORMAT {
            return Err(BackupError::Newer(probe.backup));
        }
        if probe.backup < OLDEST {
            return Err(BackupError::Older(probe.backup));
        }
        let form: FileForm = options.from_str(text).map_err(|e| BackupError::NotABackup(e.to_string()))?;
        for key in &form.documents {
            kinds.check(key.0, key.1)?;
        }
        let saves: Vec<&str> = form.documents.keys().filter_map(|k| k.strip_prefix("saves/")).collect();
        match (saves.as_slice(), form.save.as_deref()) {
            ([], None) => {}
            ([one], Some(save)) if *one == save => {}
            _ => return Err(BackupError::Saves(format!("its saves ({}) aren't the one it names ({})", saves.join(", "), form.save.as_deref().unwrap_or("none")))),
        }
        Ok(Backup { format: form.backup, game: form.game, commit: form.commit, exported: form.exported, save: form.save, left_out: form.left_out, documents: form.documents })
    }

    /// What it holds, for the screen: groups by namespace with their count of documents (parts aside) and bytes.
    pub fn contents(&self) -> Vec<Group> {
        contents(self.documents.iter().map(|(k, t)| (k.as_str(), t.len())))
    }

    /// The file name an export gets: `space-game-<save>-2026-10-20-1403.ron` (`backup` without a save).
    pub fn file_name(&self, slot: &str) -> String {
        let stamp: String = date(self.exported).chars().take(16).map(|c| if c == ' ' { '-' } else { c }).filter(|c| *c != ':').collect();
        format!("space-game-{slot}-{stamp}.ron")
    }

    /// A line naming where it came from: `0.1.0 (936e2a6c1a2b), 2026-10-20 14:03 UTC`.
    pub fn origin(&self) -> String {
        let commit = self.commit.as_deref().map_or(String::new(), |c| format!(" ({})", &c[..c.len().min(7)]));
        format!("{}{commit}, {}", self.game, date(self.exported))
    }
}

/// A group of documents: a namespace's label, how many documents (parts aside) and their bytes (parts included).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub namespace: String,
    pub count: usize,
    pub bytes: usize,
}

impl Group {
    /// `6 DESIGNS`, `1 SAVE`, `3 LAB RUNS (5.4 MB)`.
    pub fn words(&self) -> String {
        let (one, many) = match self.namespace.as_str() {
            "saves" => ("SAVE", "SAVES"),
            "designs" => ("DESIGN", "DESIGNS"),
            "drydock" => ("DRAFT", "DRAFTS"),
            "behaviors" => ("BEHAVIOR", "BEHAVIORS"),
            "settings" => ("SETTING", "SETTINGS"),
            "lab" => ("LAB RUN", "LAB RUNS"),
            "ladder" => ("RATING", "RATINGS"),
            other => (other, other),
        };
        let size = if self.namespace == LAB { format!(" ({})", bytes_text(self.bytes)) } else { String::new() };
        format!("{} {}{size}", self.count, if self.count == 1 { one } else { many })
    }
}

/// Groups documents (key, bytes) by namespace, in a fixed order (the save first).
pub fn contents<'a>(docs: impl Iterator<Item = (&'a str, usize)>) -> Vec<Group> {
    let mut groups: BTreeMap<&str, Group> = BTreeMap::new();
    for (key, bytes) in docs {
        let Some((namespace, name)) = key.split_once('/') else { continue };
        let g = groups.entry(namespace).or_insert_with(|| Group { namespace: namespace.into(), count: 0, bytes: 0 });
        g.bytes += bytes;
        if !name.contains('/') {
            g.count += 1;
        }
    }
    const ORDER: [&str; 7] = ["saves", "designs", "drydock", "behaviors", "settings", "lab", "ladder"];
    let rank = |n: &str| ORDER.iter().position(|o| *o == n).unwrap_or(ORDER.len());
    let mut out: Vec<Group> = groups.into_values().collect();
    out.sort_by_key(|g| (rank(&g.namespace), g.namespace.clone()));
    out
}

/// What the storage holds for the game using save `slot`, by group (as an export with everything would hold, without
/// checking the documents).
pub fn stored(storage: &Storage, kinds: &BackupKinds, slot: &str) -> Vec<Group> {
    let mut docs = Vec::new();
    for namespace in kinds.namespaces() {
        for name in storage.names_with_parts(namespace).unwrap_or_default() {
            let key = format!("{namespace}/{name}");
            if (namespace == SAVES && name != slot) || kinds.find(&key).is_none() {
                continue;
            }
            let bytes = storage.read_stored(&key).ok().flatten().map_or(0, |t| t.len());
            docs.push((key, bytes));
        }
    }
    contents(docs.iter().map(|(k, b)| (k.as_str(), *b)))
}

/// `5.4 MB`, `820 kB`.
pub fn bytes_text(bytes: usize) -> String {
    if bytes >= 1 << 20 { format!("{:.1} MB", bytes as f64 / (1 << 20) as f64) } else { format!("{} kB", bytes.div_ceil(1024)) }
}

/// A stored text as a RON raw string, with as many `#` as it needs.
fn raw(text: &str) -> String {
    let mut most = 0;
    for (i, _) in text.match_indices('"') {
        most = most.max(text[i + 1..].chars().take_while(|c| *c == '#').count());
    }
    let hashes = "#".repeat(most + 1);
    format!("r{hashes}\"{text}\"{hashes}")
}

/// Unix seconds as `2026-10-20 14:03 UTC` (the wall clock, for people: no time zone database).
pub fn date(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02} UTC", secs / 3600, secs % 3600 / 60)
}

// ---------------------------------------------------------------------------------------------------------
// Importing

/// What an import does to the storage: the documents to write (the file's, its save under the game's slot), then
/// the documents to delete (those of a known kind the file doesn't hold, outside the namespaces it left out; the
/// other save stays).
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub writes: Vec<(String, String)>,
    pub deletes: Vec<String>,
}

/// The plan for importing `backup` into the game using save `slot`. A dev-mode save never becomes the career.
pub fn plan(storage: &Storage, kinds: &BackupKinds, backup: &Backup, slot: &str) -> Result<Plan, BackupError> {
    if backup.save.as_deref() == Some(crate::career::DEV_SLOT) && slot != crate::career::DEV_SLOT {
        return Err(BackupError::DevIntoCareer);
    }
    let from = backup.save.as_ref().map(|s| format!("{SAVES}/{s}"));
    let writes: Vec<(String, String)> = backup
        .documents
        .iter()
        .map(|(k, t)| if Some(k) == from.as_ref() { (format!("{SAVES}/{slot}"), t.clone()) } else { (k.clone(), t.clone()) })
        .collect();
    let written: BTreeSet<&str> = writes.iter().map(|(k, _)| k.as_str()).collect();
    let mut deletes = Vec::new();
    for namespace in kinds.namespaces() {
        if backup.left_out.iter().any(|n| n == namespace) {
            continue;
        }
        for name in storage.names_with_parts(namespace).map_err(BackupError::Document)? {
            let key = format!("{namespace}/{name}");
            if (namespace == SAVES && name != slot) || written.contains(key.as_str()) || kinds.find(&key).is_none() {
                continue;
            }
            deletes.push(key);
        }
    }
    Ok(Plan { writes, deletes })
}

/// Where an import is (a resource the screen starts and watches).
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub enum Importing {
    #[default]
    Idle,
    /// The file's documents are written; the deletes wait until they are kept.
    Writing { deletes: Vec<String> },
    /// The stale documents are deleted.
    Deleting,
    /// Done: the game must start again on the new saves.
    Done,
    /// It failed (why); on the web the stored saves are as they were if the writes failed.
    Failed(String),
}

/// The words a storage frozen by an import refuses writes with.
pub const FROZEN: &str = "the saves were just replaced by an import: start the game again";

impl Importing {
    /// Starts an import: freezes the storage and writes the file's documents.
    pub fn start(storage: &mut Storage, plan: Plan) -> Importing {
        storage.freeze(FROZEN);
        for (key, text) in &plan.writes {
            if let Err(e) = storage.write_stored(key, text) {
                return Importing::Failed(format!("not imported: {e}"));
            }
        }
        Importing::Writing { deletes: plan.deletes }
    }

    /// One step on (every frame): once the writes are kept, the deletes; once those are, done.
    pub fn advance(&mut self, storage: &mut Storage) {
        let (busy, failed) = storage.settling();
        match self {
            Importing::Writing { deletes } => {
                if let Some(e) = failed {
                    *self = Importing::Failed(format!("not imported: {e}"));
                } else if !busy {
                    for key in std::mem::take(deletes) {
                        if let Err(e) = storage.delete_stored(&key) {
                            *self = Importing::Failed(format!("imported, but {key} is left over: {e}"));
                            return;
                        }
                    }
                    *self = Importing::Deleting;
                }
            }
            Importing::Deleting if !busy => *self = Importing::Done,
            _ => {}
        }
    }

    pub fn running(&self) -> bool {
        matches!(self, Importing::Writing { .. } | Importing::Deleting)
    }
}

fn advance(mut importing: ResMut<Importing>, mut storage: ResMut<Storage>) {
    if importing.running() {
        importing.advance(&mut storage);
    }
}

/// Starts the game again on the web (the page reloads; `?newgame=1` is dropped, so the imported dev save isn't
/// started over).
#[cfg(target_arch = "wasm32")]
pub fn restart() {
    let Some(location) = web_sys::window().map(|w| w.location()) else { return };
    let search = location.search().unwrap_or_default();
    let kept: Vec<&str> = search.trim_start_matches('?').split('&').filter(|p| !p.is_empty() && !p.starts_with("newgame=") && *p != "newgame").collect();
    let query = if kept.is_empty() { String::new() } else { format!("?{}", kept.join("&")) };
    let path = location.pathname().unwrap_or_default();
    let _ = location.replace(&format!("{path}{query}"));
}

pub struct BackupPlugin;

impl Plugin for BackupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BackupKinds>().init_resource::<Importing>().add_systems(Update, advance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{MemoryBackend, testing::Behind};

    /// A settings document and a lab run's head kept as text.
    fn stored_text(namespace: &str, format: u32, body: &str) -> String {
        format!("// space-game: {namespace}, format {format}, saved 1760000000\n{body}")
    }

    fn sample(storage: &mut Storage) {
        let display = DisplaySettings { glow: false, bloom: true, ui_size: 0.9 };
        storage.write("display", &display, 1_760_000_000).unwrap();
        storage.write("controls", &ControlsDoc(BTreeMap::from([("thrust".into(), vec!["W".into()])])), 1_760_000_001).unwrap();
        // A text with quotes and `"#` inside, which the raw string must get around.
        let mut odd = ControlsDoc::default();
        odd.0.insert("a\"#b".into(), vec!["\"##".into()]);
        storage.write_stored("settings/odd", &stored_text("settings", 1, &odd.to_ron())).unwrap();
        // Documents of no kind are not the player's: left out and left alone.
        storage.write_stored("drydock/working-unread", "// space-game: drydock, format 9\n(?)").unwrap();
        // The other save is never touched.
        storage.write_stored("saves/dev", "dev save").unwrap();
    }

    use crate::settings::{ControlsDoc, DisplaySettings};

    fn kinds() -> BackupKinds {
        let mut k = BackupKinds::standard();
        // The odd settings document above reads as controls.
        k.add(Kind::of::<ControlsDoc>("odd"));
        k
    }

    #[test]
    fn kinds_match_names_and_parts() {
        let k = BackupKinds::standard();
        for key in ["designs/a", "drydock/working", "saves/career", "settings/display", "lab/run", "lab/run/config", "lab/run/g12", "ladder/a", "behaviors/x"] {
            assert!(k.find(key).is_some(), "{key}");
        }
        for key in ["drydock/working-unread", "drydock/behavior", "lab/run/x", "settings/other", "meta/x", "designs/a/b", "nothing"] {
            assert!(k.find(key).is_none(), "{key}");
        }
    }

    #[test]
    fn a_backup_round_trips_through_its_text() {
        let mut storage = Storage::new(MemoryBackend::default());
        sample(&mut storage);
        let k = kinds();
        let got = gather(&storage, &k, "career", Include { lab: false }, 1_792_506_180).unwrap();
        assert!(got.skipped.is_empty(), "{:?}", got.skipped);
        let b = got.backup;
        assert_eq!(b.documents.keys().collect::<Vec<_>>(), ["settings/controls", "settings/display", "settings/odd"]);
        assert_eq!((b.save.as_deref(), b.left_out.as_slice()), (None, &["lab".to_string()][..]));
        let text = b.to_text();
        assert!(text.contains("date: \"2026-10-20 14:23 UTC\""), "{text}");
        assert!(text.contains("r###\"// space-game: settings"), "a document holding `\"##` gets three hashes:\n{text}");
        assert!(text.contains("\n(\n    backup: 1,"), "{text}");
        let back = Backup::read(&text, &k).unwrap();
        assert_eq!(back, b);
        assert_eq!(b.file_name("career"), "space-game-career-2026-10-20-1423.ron");
        assert_eq!(b.contents(), vec![Group { namespace: "settings".into(), count: 3, bytes: b.documents.values().map(String::len).sum() }]);
        assert_eq!(b.contents()[0].words(), "3 SETTINGS");
    }

    #[test]
    fn newer_older_broken_and_unknown_files_are_refused() {
        let mut storage = Storage::new(MemoryBackend::default());
        sample(&mut storage);
        let k = kinds();
        let b = gather(&storage, &k, "career", Include { lab: true }, 5).unwrap().backup;
        let text = b.to_text();
        let newer = text.replacen("backup: 1,", "backup: 2,\n    later: (x: 1),", 1);
        assert_eq!(Backup::read(&newer, &k), Err(BackupError::Newer(2)));
        assert_eq!(Backup::read(&text.replacen("backup: 1,", "backup: 0,", 1), &k), Err(BackupError::Older(0)));
        assert!(matches!(Backup::read("(hello: 1)", &k), Err(BackupError::NotABackup(_))));
        assert!(matches!(Backup::read(&text[..text.len() / 2], &k), Err(BackupError::NotABackup(_))));
        // A document that doesn't read, of a newer format, or of no kind.
        let broken = text.replacen("glow: false", "glow: maybe", 1);
        assert!(matches!(Backup::read(&broken, &k), Err(BackupError::Document(StorageError::Unreadable { .. }))), "{:?}", Backup::read(&broken, &k));
        let newer_doc = text.replacen("settings, format 1, saved 1760000000\n(\n    glow", "settings, format 7, saved 1760000000\n(\n    glow", 1);
        assert!(matches!(Backup::read(&newer_doc, &k), Err(BackupError::Document(StorageError::Newer { format: 7, .. }))));
        let unknown = text.replacen("\"settings/odd\"", "\"meta/odd\"", 1);
        assert_eq!(Backup::read(&unknown, &k), Err(BackupError::Unknown("meta/odd".into())));
        // A save the file doesn't name.
        let mut two = b.clone();
        two.documents.insert("saves/other".into(), "x".into());
        assert!(Backup::read(&two.to_text(), &BackupKinds(vec![Kind { namespace: "saves", pattern: "*", check: |_, _| Ok(()) }, Kind::of::<ControlsDoc>("*"), Kind::of::<DisplaySettings>("display")])).is_err());
        // Too large.
        assert_eq!(Backup::read(&" ".repeat(MAX_FILE + 1), &k), Err(BackupError::TooLarge));
    }

    /// A file that doesn't read never reaches the storage: the screen only plans with a backup that read.
    #[test]
    fn a_broken_file_leaves_the_storage_untouched() {
        let mut storage = Storage::new(MemoryBackend::default());
        sample(&mut storage);
        let k = kinds();
        let before = gather(&storage, &k, "career", Include { lab: true }, 5).unwrap().backup;
        let broken = before.to_text().replacen("ui_size: 0.9", "ui_size: \"big\"", 1);
        assert!(Backup::read(&broken, &k).is_err());
        assert_eq!(gather(&storage, &k, "career", Include { lab: true }, 5).unwrap().backup, before);
        assert_eq!(storage.frozen(), None);
    }

    /// An import writes the file's documents (its save under the game's slot), deletes what the file doesn't hold
    /// once they are kept, leaves the namespaces the file left out, the other save and documents of no kind alone,
    /// and freezes the storage for the game's own writes.
    #[test]
    fn an_import_replaces_the_saves_and_freezes_the_storage() {
        let backend = Behind::default();
        let shared = Behind(backend.0.clone());
        let mut storage = Storage::new(backend);
        sample(&mut storage);
        storage.write_stored("saves/career", "old career").unwrap();
        storage.write_stored("lab/run", "a run").unwrap();
        storage.write_stored("designs/old", "an old design").unwrap();
        shared.commit(false);
        // The file: what reads (the old design and the old career don't), without the controls and the lab.
        let k = kinds();
        let mut file = gather(&storage, &k, "career", Include { lab: false }, 5).unwrap().backup;
        file.documents.remove("settings/controls");
        let plan = plan(&storage, &k, &file, "career").unwrap();
        assert_eq!(plan.deletes, ["designs/old", "saves/career", "settings/controls"], "lab is left out; saves/dev stays");
        let mut importing = Importing::start(&mut storage, plan);
        assert!(matches!(storage.write("x", &DisplaySettings::default(), 1), Err(StorageError::Unavailable(_))), "the game's writes are refused now");
        assert!(storage.delete::<DisplaySettings>("display").is_err());
        importing.advance(&mut storage);
        assert!(matches!(importing, Importing::Writing { .. }), "the writes are still on their way");
        shared.commit(false);
        importing.advance(&mut storage);
        assert_eq!(importing, Importing::Deleting);
        shared.commit(false);
        importing.advance(&mut storage);
        assert_eq!(importing, Importing::Done);
        let db = shared.0.lock().unwrap().1.clone();
        assert_eq!(db.keys().collect::<Vec<_>>(), ["drydock/working-unread", "lab/run", "saves/dev", "settings/display", "settings/odd"]);
    }

    /// A failed write (the quota) stops the import before anything is deleted: the stored saves are as they were.
    #[test]
    fn a_failed_import_deletes_nothing() {
        let backend = Behind::default();
        let shared = Behind(backend.0.clone());
        let mut storage = Storage::new(backend);
        sample(&mut storage);
        shared.commit(false);
        let before = shared.0.lock().unwrap().1.clone();
        let k = kinds();
        let mut file = gather(&storage, &k, "career", Include { lab: true }, 5).unwrap().backup;
        file.documents.remove("settings/controls");
        let plan = plan(&storage, &k, &file, "career").unwrap();
        assert_eq!(plan.deletes, ["settings/controls"]);
        let mut importing = Importing::start(&mut storage, plan);
        shared.commit(true);
        importing.advance(&mut storage);
        assert!(matches!(&importing, Importing::Failed(why) if why.contains("full")), "{importing:?}");
        assert_eq!(shared.0.lock().unwrap().1, before);
    }

    #[test]
    fn saves_follow_the_game_and_the_dev_save_stays_out_of_the_career() {
        let mut storage = Storage::new(MemoryBackend::default());
        storage.write_stored("saves/career", "career").unwrap();
        storage.write_stored("saves/dev", "dev").unwrap();
        let k = BackupKinds(vec![Kind { namespace: "saves", pattern: "*", check: |_, _| Ok(()) }]);
        let career = gather(&storage, &k, "career", Include { lab: true }, 5).unwrap().backup;
        assert_eq!((career.save.as_deref(), career.documents.keys().map(String::as_str).collect::<Vec<_>>()), (Some("career"), vec!["saves/career"]));
        let dev = gather(&storage, &k, "dev", Include { lab: true }, 5).unwrap().backup;
        assert_eq!(dev.documents.keys().collect::<Vec<_>>(), ["saves/dev"]);
        // The career's save goes into the dev mode's; the dev mode's never into the career.
        let p = plan(&storage, &k, &career, "dev").unwrap();
        assert_eq!(p.writes, [("saves/dev".to_string(), "career".to_string())]);
        assert!(p.deletes.is_empty());
        assert_eq!(plan(&storage, &k, &dev, "career"), Err(BackupError::DevIntoCareer));
    }

    #[test]
    fn dates_and_sizes_read() {
        assert_eq!(date(0), "1970-01-01 00:00 UTC");
        assert_eq!(date(1_792_506_180), "2026-10-20 14:23 UTC");
        assert_eq!(date(951_782_400), "2000-02-29 00:00 UTC");
        assert_eq!(bytes_text(5_662_310), "5.4 MB");
        assert_eq!(bytes_text(1500), "2 kB");
        assert_eq!(raw("a \"# b"), "r##\"a \"# b\"##");
        assert_eq!(raw("plain"), "r#\"plain\"#");
    }
}
