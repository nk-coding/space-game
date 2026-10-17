//! The player's design library on top of the storage layer (`crate::storage`): saved designs, the working
//! design, ids from names, fresh names for copies, the autosave timer. Plain functions over [`Storage`],
//! `Content` and [`PlayerDesign`], so they are tested without an app; `blueprints/` calls them from the UI.
//!
//! * **Saved designs** ([`DesignDoc`], `designs/<id>`): the content ship format (`data::blueprint_to_ron`) in
//!   the design's drydock frame: `size` = the frame, parts in frame coordinates, `id` = the design's id. Opening
//!   one puts it back in that frame. Format 2 (GDR-0092) may carry an evolved design's `origin`; format 1 reads as
//!   it is (it has none), and an older build refuses a format 2 design as newer instead of failing on the field.
//! * **Ids**: a snake_case slug of the name on the first save ([`slug`]), unique among saved designs and never a
//!   content ship's id ([`unique_id`]); the design keeps it when renamed.
//! * **The working design** ([`WorkingDoc`], `drydock/working`): the design being edited, written about a
//!   second after the last change ([`Autosave`]) and when a test flight starts, read back at start.

use data::design::edit::OpenError;
use data::{Blueprint, Content, Problem};

use crate::storage::{Document, Storage, StorageError, valid_name};

use super::PlayerDesign;

/// The longest id a slug makes (suffixes for uniqueness come on top).
const MAX_SLUG: usize = 40;
/// The longest design or behavior name (the name fields).
pub const MAX_NAME: usize = 40;
/// The name a new design gets ("New design", "New design 2", ...); also the starter's when content has no ship.
pub const NEW_NAME: &str = "New design";
/// The working design's name in its namespace.
pub const WORKING: &str = "working";
/// Where a working design that doesn't read or open is kept before the autosave writes over it.
pub const WORKING_ASIDE: &str = "working-unread";
/// The summary new and imported designs get when they have none (content requires one; the drydock has no
/// field for it yet).
pub const NEW_SUMMARY: &str = "A design from the drydock.";

/// A saved design: the blueprint in its frame (see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct DesignDoc(pub Blueprint);

impl Document for DesignDoc {
    const NAMESPACE: &'static str = "designs";
    const FORMAT: u32 = 2;

    fn to_ron(&self) -> String {
        data::blueprint_to_ron(&self.0)
    }

    fn from_ron(text: &str, _format: u32) -> Result<Self, String> {
        data::parse_blueprint(text).map(DesignDoc).map_err(|p| problems_text(&p))
    }
}

/// The working design: `(saved_as: "<id>" or None, unsaved: <bool>, design: (<the ship in its frame>))`, the
/// id it is saved under, whether it changed since it was saved or opened, and the blueprint in frame
/// coordinates (`size` = the frame).
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkingDoc {
    pub saved_as: Option<String>,
    pub unsaved: bool,
    pub design: Blueprint,
}

impl Document for WorkingDoc {
    const NAMESPACE: &'static str = "drydock";
    const FORMAT: u32 = 1;

    fn to_ron(&self) -> String {
        let saved_as = self.saved_as.as_ref().map_or("None".into(), |id| ron::to_string(id).unwrap_or_default());
        let design = data::blueprint_to_ron(&self.design).trim_end().replace('\n', "\n    ");
        format!("(\n    saved_as: {saved_as},\n    unsaved: {},\n    design: {design},\n)\n", self.unsaved)
    }

    fn from_ron(text: &str, _format: u32) -> Result<Self, String> {
        data::ron_options().from_str(text).map_err(|e| e.to_string())
    }
}

/// Problems as one line: the first one, and how many more.
pub fn problems_text(problems: &[Problem]) -> String {
    let first = |p: &Problem| {
        let mut s = String::new();
        if let Some((l, c)) = p.line {
            s.push_str(&format!("line {l}:{c}: "));
        }
        if let Some(f) = &p.field {
            s.push_str(&format!("{f}: "));
        }
        s.push_str(&p.message);
        s
    };
    match problems {
        [] => String::new(),
        [p] => first(p),
        [p, rest @ ..] => format!("{} (and {} more)", first(p), rest.len()),
    }
}

/// Why a design can't be opened, in words.
pub fn open_error_text(e: &OpenError) -> String {
    match e {
        OpenError::BadFrame => "the frame is not 1 to 1024 cells a side".into(),
        OpenError::UnknownPart { part, .. } => format!("unknown part {part:?}"),
        OpenError::BadRotation { index } => format!("part {index} has a rotation above 3"),
        OpenError::Overlap { index, other } => format!("parts {other} and {index} overlap"),
        OpenError::DoesNotFit { design } => format!("the design ({}×{}) is larger than the frame", design.0, design.1),
    }
}

/// A snake_case id from a design's name: lowercase ASCII letters and digits, German umlauts and ß spelled out,
/// everything else a single `_`; it starts with a letter (`design_` is put before a digit) and is at most
/// 40 characters. An empty result is `design`.
pub fn slug(name: &str) -> String {
    slug_or(name, "design")
}

/// [`slug`] with another word before a leading digit and for an empty result (`behavior` for behaviors).
pub fn slug_or(name: &str, word: &str) -> String {
    let mut s = String::new();
    let mut gap = false;
    for c in name.chars().flat_map(char::to_lowercase) {
        let spelled = match c {
            'ä' => "ae",
            'ö' => "oe",
            'ü' => "ue",
            'ß' => "ss",
            _ => "",
        };
        if c.is_ascii_alphanumeric() || !spelled.is_empty() {
            if gap && !s.is_empty() {
                s.push('_');
            }
            gap = false;
            if spelled.is_empty() {
                s.push(c);
            } else {
                s.push_str(spelled);
            }
        } else {
            gap = true;
        }
    }
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert_str(0, &format!("{word}_"));
    }
    if s.is_empty() {
        s.push_str(word);
    }
    s.truncate(MAX_SLUG);
    s.trim_end_matches('_').to_string()
}

/// `base`, or `base_2`, `base_3`, ... : the first that is not taken.
pub fn unique_id(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|k| format!("{base}_{k}")).find(|id| !taken(id)).expect("some suffix is free")
}

/// `base`, or `base 2`, `base 3`, ... : the first name no saved design has.
pub fn fresh_name(base: &str, names: &[String]) -> String {
    if !names.iter().any(|n| n == base) {
        return base.to_string();
    }
    (2..).map(|k| format!("{base} {k}")).find(|n| !names.iter().any(|m| m == n)).expect("some number is free")
}

/// A new id for a design called `name`: unique among saved designs, never a content ship's id.
pub fn new_id(storage: &Storage, content: &Content, name: &str) -> Result<String, StorageError> {
    let saved = storage.list(DesignDoc::NAMESPACE)?;
    Ok(unique_id(&slug(name), |id| content.ships.contains(id) || saved.iter().any(|s| s == id)))
}

/// "Just now", "5 min ago", "3 h ago", "2 d ago" (uppercase for the UI's labels).
pub fn ago(now: u64, then: u64) -> String {
    let s = now.saturating_sub(then);
    match s {
        0..60 => "JUST NOW".into(),
        60..3600 => format!("{} MIN AGO", s / 60),
        3600..86_400 => format!("{} H AGO", s / 3600),
        _ => format!("{} D AGO", s / 86_400),
    }
}

/// When to write the working design: [`Autosave::DELAY`] seconds after the last change (every change restarts
/// the wait), or at once when a test flight starts ([`Autosave::pending`]).
#[derive(Debug, Default)]
pub struct Autosave {
    /// The last revision noted.
    seen: Option<u64>,
    /// When the design last changed without being written since.
    changed_at: Option<f64>,
}

impl Autosave {
    pub const DELAY: f64 = 1.0;

    /// The design is at `revision` at time `now` (seconds). The first revision noted is the one stored (the
    /// restored or first design), not a change.
    pub fn note(&mut self, revision: u64, now: f64) {
        if self.seen != Some(revision) {
            if self.seen.is_some() {
                self.changed_at = Some(now);
            }
            self.seen = Some(revision);
        }
    }

    /// A change waits to be written.
    pub fn pending(&self) -> bool {
        self.changed_at.is_some()
    }

    /// The wait is over.
    pub fn due(&self, now: f64) -> bool {
        self.changed_at.is_some_and(|t| now - t >= Self::DELAY)
    }

    /// The design was written.
    pub fn written(&mut self) {
        self.changed_at = None;
    }
}

/// One saved design in the BLUEPRINTS list.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    /// The frame it was saved in.
    pub frame: (u32, u32),
    /// The parts' bounding box.
    pub size: (u32, u32),
    pub parts: usize,
    /// When it was saved (Unix seconds).
    pub saved: Option<u64>,
    /// Where an evolved design came from (the evolution lab's run): BLUEPRINTS tags it EVOLVED.
    pub origin: Option<data::Origin>,
    /// Why it can't be read (it is listed anyway, so it can be deleted).
    pub problem: Option<String>,
}

/// The saved designs, most recently saved first (then by name).
pub fn list_designs(storage: &Storage, content: &Content) -> Result<Vec<Entry>, StorageError> {
    let mut out = Vec::new();
    for id in storage.list(DesignDoc::NAMESPACE)? {
        out.push(match storage.read::<DesignDoc>(&id) {
            Ok(Some(s)) => {
                let bp = &s.doc.0;
                Entry {
                    id,
                    name: bp.name.clone(),
                    frame: bp.size,
                    size: parts_size(content, bp),
                    parts: bp.parts.len(),
                    saved: s.saved,
                    origin: bp.origin.clone(),
                    problem: None,
                }
            }
            Ok(None) => continue,
            Err(e) => Entry { name: id.clone(), id, frame: (0, 0), size: (0, 0), parts: 0, saved: None, origin: None, problem: Some(e.to_string()) },
        });
    }
    out.sort_by(|a, b| b.saved.cmp(&a.saved).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// Refits every saved design to the content (`data::design::refit_replaced`, GDR-0103: a design carrying
/// `automaton_cannon` from before the player's cannon became a part of its own) and writes back the ones that
/// changed, keeping their saved time. Returns their names; a design that doesn't read is left alone.
pub fn refit_saved(storage: &mut Storage, content: &Content) -> Result<Vec<String>, StorageError> {
    let mut refitted = Vec::new();
    for id in storage.list(DesignDoc::NAMESPACE)? {
        let Ok(Some(mut s)) = storage.read::<DesignDoc>(&id) else { continue };
        if data::design::refit_replaced(content, &mut s.doc.0) > 0 {
            storage.write(&id, &s.doc, s.saved.unwrap_or_default())?;
            refitted.push(s.doc.0.name);
        }
    }
    Ok(refitted)
}

/// The bounding box of a blueprint's parts (unknown parts left out). Any coordinates (a stored file is
/// untrusted): the sums are taken in `i64`, a side beyond `u32` reads as `u32::MAX`.
pub fn parts_size(content: &Content, bp: &Blueprint) -> (u32, u32) {
    let boxes = bp.parts.iter().filter_map(|p| content.placed_shape(p)).map(|s| {
        let (w, h) = s.dims();
        let (x, y) = (i64::from(s.x), i64::from(s.y));
        (x, y, x + i64::from(w), y + i64::from(h))
    });
    let b = boxes.reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)));
    let side = |d: i64| u32::try_from(d).unwrap_or(u32::MAX);
    b.map_or((0, 0), |(x0, y0, x1, y1)| (side(x1 - x0), side(y1 - y0)))
}

/// The id the working design is saved under and what it looked like then, to tell unsaved changes.
#[derive(bevy::prelude::Resource, Clone, Debug, Default)]
pub struct DesignSlot {
    /// `designs/<id>`; `None` before the first save.
    pub id: Option<String>,
    /// The design (frame coordinates) when it was last saved or opened.
    clean: Option<Blueprint>,
}

impl DesignSlot {
    /// A slot for a design just opened (or saved) under `id`.
    pub fn opened(id: Option<String>, design: &PlayerDesign) -> Self {
        Self { id, clean: Some(design.blueprint().clone()) }
    }

    /// The design differs from the one last saved or opened.
    pub fn dirty(&self, design: &PlayerDesign) -> bool {
        self.clean.as_ref() != Some(design.blueprint())
    }
}

/// The blueprint stored for a design: frame coordinates, `size` = the frame, under `id`.
fn stored(design: &PlayerDesign, id: &str) -> Blueprint {
    Blueprint { id: id.to_string(), ..design.blueprint().clone() }
}

/// Saves the working design under its id (a new one from its name on the first save); returns the id.
pub fn save(storage: &mut Storage, content: &Content, design: &PlayerDesign, slot: &mut DesignSlot, now: u64) -> Result<String, StorageError> {
    let id = match &slot.id {
        Some(id) => id.clone(),
        None => new_id(storage, content, &design.blueprint().name)?,
    };
    storage.write(&id, &DesignDoc(stored(design, &id)), now)?;
    *slot = DesignSlot::opened(Some(id.clone()), design);
    Ok(id)
}

/// Writes the working design (with the id it is saved under and whether it has unsaved changes). Its blueprint's
/// `id` is that id (empty before the first save): the header's own id is the one it was opened with (a template's
/// content id, say), which saving and exports never use.
pub fn write_working(storage: &mut Storage, design: &PlayerDesign, slot: &DesignSlot, now: u64) -> Result<(), StorageError> {
    storage.write(WORKING, &working_doc(design, slot), now)
}

/// The working design as it is written ([`write_working`]).
pub fn working_doc(design: &PlayerDesign, slot: &DesignSlot) -> WorkingDoc {
    let id = slot.id.clone().unwrap_or_default();
    WorkingDoc { saved_as: slot.id.clone(), unsaved: slot.dirty(design), design: Blueprint { id, ..design.blueprint().clone() } }
}

/// A working design back from storage: the design, its slot, and a note when part of it was dropped.
pub type Restored = (PlayerDesign, DesignSlot, Option<String>);

/// The working design of the last session, opened in its frame, with its slot (unsaved changes stay unsaved) and a
/// note when part of it was dropped. `None` on the first start.
///
/// The document is untrusted (a damaged browser storage entry, a hand-edited file): an id it is "saved as" that is
/// no valid design id is dropped (the design comes back unsaved, and SAVE gives it a new id). A document that
/// doesn't read or open (a newer build's, a damaged one, a design whose parts this content lacks) is copied aside
/// first ([`WORKING_ASIDE`]), so the autosave of the design that replaces it doesn't destroy it; the error says so.
pub fn restore_working(storage: &mut Storage, content: &Content) -> Option<Result<Restored, String>> {
    let opened = match storage.read::<WorkingDoc>(WORKING) {
        Ok(Some(mut s)) => {
            data::design::refit_replaced(content, &mut s.doc.design);
            PlayerDesign::open(content, &s.doc.design, s.doc.design.size).map(|d| (d, s.doc)).map_err(|e| open_error_text(&e))
        }
        Ok(None) => return None,
        Err(e) => Err(e.to_string()),
    };
    let (design, doc) = match opened {
        Ok(d) => d,
        Err(why) => {
            let kept = match storage.set_aside::<WorkingDoc>(WORKING, WORKING_ASIDE) {
                Ok(()) => format!("it is kept as {}/{WORKING_ASIDE}", WorkingDoc::NAMESPACE),
                Err(e) => format!("it could not be kept: {e}"),
            };
            return Some(Err(format!("{why}; {kept}")));
        }
    };
    let (saved_as, note) = match doc.saved_as {
        Some(id) if !valid_name(&id) || content.ships.contains(&id) => (None, Some(format!("its saved name {id:?} is no design id: it is unsaved now"))),
        other => (other, None),
    };
    // Unsaved changes stay unsaved: compared with the saved version (as the editor shows it), if there is one.
    let clean = match (&saved_as, doc.unsaved) {
        (None, _) if note.is_some() => None,
        (_, true) => saved_as.as_deref().and_then(|id| open_saved(storage, content, id).ok()).map(|d| d.blueprint().clone()),
        (_, false) => Some(design.blueprint().clone()),
    };
    Some(Ok((design, DesignSlot { id: saved_as, clean }, note)))
}

/// Opens a saved design in the frame it was saved in.
pub fn open_saved(storage: &Storage, content: &Content, id: &str) -> Result<PlayerDesign, String> {
    let doc = storage.read::<DesignDoc>(id).map_err(|e| e.to_string())?.ok_or_else(|| format!("{id} is gone"))?;
    PlayerDesign::open(content, &doc.doc.0, doc.doc.0.size).map_err(|e| open_error_text(&e))
}

/// Opens a blueprint in the first content frame it fits (the drydock's frame order); the error says why
/// none takes it.
pub fn open_in_first_frame(content: &Content, bp: &Blueprint) -> Result<PlayerDesign, String> {
    let mut last = None;
    for f in content.frames.iter() {
        match PlayerDesign::open(content, bp, f.size) {
            Ok(d) => return Ok(d),
            Err(OpenError::DoesNotFit { design }) => last = Some(design),
            Err(e) => return Err(open_error_text(&e)),
        }
    }
    let largest = content.frames.iter().last().map_or((0, 0), |f| f.size);
    Err(match last {
        Some((w, h)) => format!("the design ({w}×{h}) is larger than every frame (the largest is {}×{})", largest.0, largest.1),
        None => "there are no frames".into(),
    })
}

/// A copy of a content ship with a fresh name ("Halvex Runner copy"), in the first frame it fits; not saved.
pub fn open_template(content: &Content, ship: &str, names: &[String]) -> Result<PlayerDesign, String> {
    let bp = content.ships.get(ship).ok_or_else(|| format!("no ship {ship}"))?;
    let name = fresh_name(&format!("{} copy", bp.name), names);
    open_in_first_frame(content, &Blueprint { name, ..bp.clone() })
}

/// An empty design in a frame, with a fresh name and the faction of `like` (the player's).
pub fn new_design(content: &Content, frame: (u32, u32), like: &Blueprint, names: &[String]) -> Result<PlayerDesign, String> {
    let name = fresh_name(NEW_NAME, names);
    let bp = Blueprint {
        id: slug(&name),
        name,
        summary: NEW_SUMMARY.into(),
        description: None,
        lore: None,
        tags: Vec::new(),
        parts: Vec::new(),
        size: frame,
        behavior: None,
        assists: None,
        origin: None,
        ..like.clone()
    };
    PlayerDesign::open(content, &bp, frame).map_err(|e| open_error_text(&e))
}

/// Saves a copy of a saved design under a new id and a fresh name; returns the new id.
pub fn duplicate(storage: &mut Storage, content: &Content, id: &str, names: &[String], now: u64) -> Result<String, String> {
    let doc = storage.read::<DesignDoc>(id).map_err(|e| e.to_string())?.ok_or_else(|| format!("{id} is gone"))?;
    let name = fresh_name(&format!("{} copy", doc.doc.0.name), names);
    let new = new_id(storage, content, &name).map_err(|e| e.to_string())?;
    storage.write(&new, &DesignDoc(Blueprint { id: new.clone(), name, ..doc.doc.0 }), now).map_err(|e| e.to_string())?;
    Ok(new)
}

/// Renames a saved design (its id stays).
pub fn rename(storage: &mut Storage, id: &str, name: &str, now: u64) -> Result<(), String> {
    let doc = storage.read::<DesignDoc>(id).map_err(|e| e.to_string())?.ok_or_else(|| format!("{id} is gone"))?;
    storage.write(id, &DesignDoc(Blueprint { name: name.to_string(), ..doc.doc.0 }), now).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryBackend;

    fn mem() -> Storage {
        Storage::new(MemoryBackend::default())
    }

    #[test]
    fn slugs_are_snake_case_ids() {
        assert_eq!(slug("Halvex Runner copy"), "halvex_runner_copy");
        assert_eq!(slug("  Läufer  Mk. II!! "), "laeufer_mk_ii");
        assert_eq!(slug("Große Fähre"), "grosse_faehre");
        assert_eq!(slug("3rd wing"), "design_3rd_wing");
        assert_eq!(slug("★★★"), "design");
        assert_eq!(slug(""), "design");
        assert_eq!(slug("a__b--c"), "a_b_c");
        let long = slug(&"very long name ".repeat(10));
        assert!(long.len() <= MAX_SLUG && !long.ends_with('_'), "{long}");
        let content = data::base_content();
        for name in ["Halvex Runner copy", "Läufer", "3rd wing", "★", &"x y ".repeat(30)] {
            let s = slug(name);
            assert!(valid_name(&s), "{s}");
            // The same rule as content ids, so a design can move into a pack.
            let bp = Blueprint { id: s.clone(), ..content.ships.get("halvex_runner").unwrap().clone() };
            assert!(data::validate_ship(&content, &bp).iter().all(|p| !p.is_error()), "{s}");
        }
    }

    #[test]
    fn ids_are_unique_and_never_a_content_ship() {
        let content = data::base_content();
        let mut storage = mem();
        assert_eq!(new_id(&storage, &content, "Halvex Runner").unwrap(), "halvex_runner_2");
        assert_eq!(new_id(&storage, &content, "Halvex Runner copy").unwrap(), "halvex_runner_copy");
        let bp = content.ships.get("halvex_runner").unwrap().clone();
        storage.write("halvex_runner_copy", &DesignDoc(bp.clone()), 1).unwrap();
        storage.write("halvex_runner_copy_2", &DesignDoc(bp), 1).unwrap();
        assert_eq!(new_id(&storage, &content, "Halvex Runner copy").unwrap(), "halvex_runner_copy_3");
        assert_eq!(fresh_name("New design", &[]), "New design");
        let names = vec!["New design".to_string(), "New design 2".to_string()];
        assert_eq!(fresh_name("New design", &names), "New design 3");
    }

    #[test]
    fn autosave_waits_a_second_after_the_last_change() {
        let mut a = Autosave::default();
        // The first revision is what is stored already.
        a.note(5, 0.0);
        assert!(!a.pending() && !a.due(10.0));
        a.note(5, 0.5);
        assert!(!a.pending());
        // A change, then another before the second is over: the wait restarts.
        a.note(6, 1.0);
        assert!(a.pending() && !a.due(1.75));
        a.note(7, 1.75);
        assert!(!a.due(2.5) && a.due(2.75));
        a.written();
        assert!(!a.pending() && !a.due(5.0));
        // No change, nothing to write; the next change waits again.
        a.note(7, 6.0);
        assert!(!a.pending());
        a.note(8, 7.0);
        assert!(a.pending() && !a.due(7.5) && a.due(8.0));
    }

    #[test]
    fn ago_reads_like_a_clock() {
        assert_eq!(ago(100, 100), "JUST NOW");
        assert_eq!(ago(100, 200), "JUST NOW");
        assert_eq!(ago(1000, 100), "15 MIN AGO");
        assert_eq!(ago(4 * 3600 + 10, 0), "4 H AGO");
        assert_eq!(ago(3 * 86_400, 0), "3 D AGO");
    }

    /// GDR-0103: a saved design from before the player's cannon became a part of its own carries
    /// `automaton_cannon`; on load it is refitted with `reverse_engineered_cannon` and written back with its time.
    #[test]
    fn saved_designs_are_refitted_with_the_parts_that_replaced_theirs() {
        let content = data::base_content();
        let mut storage = mem();
        let mut bp = content.ships.get("rim_skiff").unwrap().clone();
        bp.id = "old_cannon".into();
        bp.name = "Old cannon".into();
        for p in bp.parts.iter_mut().filter(|p| p.part == "pulse_laser") {
            p.part = "automaton_cannon".into();
        }
        assert!(bp.parts.iter().any(|p| p.part == "automaton_cannon"));
        storage.write("old_cannon", &DesignDoc(bp.clone()), 42).unwrap();
        storage.write("plain", &DesignDoc(content.ships.get("rim_skiff").unwrap().clone()), 7).unwrap();
        assert_eq!(refit_saved(&mut storage, &content).unwrap(), ["Old cannon"]);
        let back = storage.read::<DesignDoc>("old_cannon").unwrap().unwrap();
        assert_eq!(back.saved, Some(42));
        assert!(back.doc.0.parts.iter().all(|p| p.part != "automaton_cannon"));
        let swapped = back.doc.0.parts.iter().filter(|p| p.part == "reverse_engineered_cannon").count();
        assert_eq!(swapped, bp.parts.iter().filter(|p| p.part == "automaton_cannon").count());
        // Once refitted, nothing more to do.
        assert!(refit_saved(&mut storage, &content).unwrap().is_empty());
    }

    #[test]
    fn save_open_rename_duplicate_and_delete() {
        let content = data::base_content();
        let mut storage = mem();
        let names: Vec<String> = Vec::new();
        let mut design = open_template(&content, "halvex_runner", &names).unwrap();
        assert_eq!(design.blueprint().name, "Halvex Runner copy");
        let mut slot = DesignSlot::opened(None, &design);
        assert!(!slot.dirty(&design));
        design.edit(|d| d.paint((0, 0), "steel")).unwrap();
        assert!(slot.dirty(&design));

        // The first save takes an id from the name; the stored file is a content ship in its frame.
        let id = save(&mut storage, &content, &design, &mut slot, 100).unwrap();
        assert_eq!(id, "halvex_runner_copy");
        assert!(!slot.dirty(&design));
        let doc = storage.read::<DesignDoc>(&id).unwrap().unwrap();
        assert_eq!(doc.saved, Some(100));
        assert_eq!(doc.doc.0.id, id);
        assert_eq!(doc.doc.0.size, design.frame());
        assert!(data::validate_ship(&content, &doc.doc.0).iter().all(|p| !p.is_error()));

        // Renaming keeps the id; saving again writes the same document.
        design.edit_header(|h| h.name = "Runner Mk II".into());
        assert!(slot.dirty(&design));
        assert_eq!(save(&mut storage, &content, &design, &mut slot, 200).unwrap(), id);
        assert_eq!(storage.list("designs").unwrap(), std::slice::from_ref(&id));

        // Opening it again gives the same design in the same frame (its parts centred in it, like every open).
        let back = open_saved(&storage, &content, &id).unwrap();
        assert_eq!(back.frame(), design.frame());
        assert_eq!(back.export(), Blueprint { id: id.clone(), source: Default::default(), ..design.export() });

        // The list: name, frame, parts' size, part count, time.
        let list = list_designs(&storage, &content).unwrap();
        assert_eq!(list.len(), 1);
        let e = &list[0];
        assert_eq!((e.name.as_str(), e.frame, e.parts, e.saved), ("Runner Mk II", (24, 24), design.blueprint().parts.len(), Some(200)));
        assert_eq!(e.size, parts_size(&content, &design.export()));
        assert_eq!(e.size, design.export().size);

        // Duplicate and rename in the list.
        let names = vec![e.name.clone()];
        let dup = duplicate(&mut storage, &content, &id, &names, 300).unwrap();
        assert_eq!(dup, "runner_mk_ii_copy");
        rename(&mut storage, &dup, "Second", 400).unwrap();
        let list = list_designs(&storage, &content).unwrap();
        assert_eq!(list.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Second", "Runner Mk II"]);
        storage.delete::<DesignDoc>(&dup).unwrap();
        assert_eq!(storage.list("designs").unwrap(), [id]);
    }

    #[test]
    fn the_working_design_comes_back_with_its_frame_and_slot() {
        let content = data::base_content();
        let mut storage = mem();
        assert!(restore_working(&mut storage, &content).is_none());
        // An unsaved new design in the cruiser frame.
        let like = content.ships.get("halvex_runner").unwrap();
        let mut design = new_design(&content, (40, 40), like, &[]).unwrap();
        let mut slot = DesignSlot::opened(None, &design);
        design.edit(|d| d.paint((3, 4), "steel")).unwrap();
        // Its header passes the content rules (id, summary), so only the ship itself can have problems.
        let header = |p: &Problem| matches!(p.field.as_deref(), Some("id" | "name" | "summary" | "faction"));
        assert!(!data::validate_ship(&content, &design.export()).iter().any(header));
        write_working(&mut storage, &design, &slot, 10).unwrap();
        let (back, back_slot, _) = restore_working(&mut storage, &content).unwrap().unwrap();
        assert_eq!(back.frame(), (40, 40));
        assert_eq!(back.export().parts, design.export().parts);
        assert_eq!(back.blueprint().name, "New design");
        assert_eq!(back_slot.id, None);
        assert!(back_slot.dirty(&back));
        // An untouched copy comes back untouched (it is not saved when another design replaces it).
        let copy = open_template(&content, "sample_mid", &[]).unwrap();
        write_working(&mut storage, &copy, &DesignSlot::opened(None, &copy), 15).unwrap();
        let (back, back_slot, _) = restore_working(&mut storage, &content).unwrap().unwrap();
        assert!(!back_slot.dirty(&back));
        // (Not saved yet: no id; the template's content id stays behind.)
        assert_eq!(back.export(), Blueprint { id: String::new(), source: Default::default(), ..copy.export() });
        // Saved, then changed: it comes back saved, with the change unsaved.
        save(&mut storage, &content, &design, &mut slot, 20).unwrap();
        design.edit(|d| d.paint((5, 4), "steel")).unwrap();
        write_working(&mut storage, &design, &slot, 30).unwrap();
        let (back, back_slot, _) = restore_working(&mut storage, &content).unwrap().unwrap();
        assert_eq!(back_slot.id.as_deref(), Some("new_design"));
        assert!(back_slot.dirty(&back));
        assert_eq!(back.export().id, "new_design");
        // A design that no longer opens (its part left the content) says why, and is kept aside.
        let mut broken = design.blueprint().clone();
        broken.parts[0].part = "gone_part".into();
        let broken = WorkingDoc { saved_as: None, unsaved: true, design: broken };
        storage.write(WORKING, &broken, 40).unwrap();
        let e = restore_working(&mut storage, &content).unwrap().err().unwrap();
        assert_eq!(e, "unknown part \"gone_part\"; it is kept as drydock/working-unread");
        assert_eq!(storage.read::<WorkingDoc>(WORKING_ASIDE).unwrap().unwrap().doc.design.parts, broken.design.parts);
    }

    #[test]
    fn a_working_design_from_a_newer_build_survives_the_autosave() {
        let content = data::base_content();
        let mut mem = MemoryBackend::default();
        let newer = "// space-game: drydock, format 9, saved 5\n(whatever: 1)\n";
        crate::storage::Backend::write(&mut mem, "drydock", WORKING, newer).unwrap();
        let mut storage = Storage::new(mem);
        let e = restore_working(&mut storage, &content).unwrap().err().unwrap();
        assert!(e.contains("newer version") && e.ends_with("kept as drydock/working-unread"), "{e}");
        // The starter's autosave writes the slot; the newer document is still there, unread.
        let starter = PlayerDesign::starter(&content, "halvex_runner");
        write_working(&mut storage, &starter, &DesignSlot::opened(None, &starter), 6).unwrap();
        assert!(matches!(storage.read::<WorkingDoc>(WORKING_ASIDE), Err(StorageError::Newer { format: 9, .. })));
        assert!(restore_working(&mut storage, &content).unwrap().is_ok());
    }

    #[test]
    fn a_working_design_saved_under_no_valid_id_comes_back_unsaved() {
        let content = data::base_content();
        let mut storage = mem();
        let design = open_template(&content, "halvex_runner", &[]).unwrap();
        for bad in ["../../x", "Bad Id", "", "halvex_runner"] {
            let doc = WorkingDoc { saved_as: Some(bad.into()), unsaved: false, design: design.blueprint().clone() };
            storage.write(WORKING, &doc, 1).unwrap();
            let (back, mut slot, note) = restore_working(&mut storage, &content).unwrap().unwrap();
            assert_eq!(slot.id, None, "{bad}");
            assert!(slot.dirty(&back) && note.is_some_and(|n| n.contains("no design id")), "{bad}");
            // SAVE gives it a new id, so it is not stuck.
            assert_eq!(save(&mut storage, &content, &back, &mut slot, 2).unwrap(), "halvex_runner_copy");
            storage.delete::<DesignDoc>("halvex_runner_copy").unwrap();
        }
    }

    #[test]
    fn listing_survives_extreme_coordinates() {
        let content = data::base_content();
        let mut storage = mem();
        // A hand-copied file with parts at the ends of the coordinate range.
        let mut far = content.ships.get("halvex_runner").unwrap().clone();
        far.parts[0].x = i32::MAX;
        far.parts[1].x = i32::MIN;
        far.parts[2].y = i32::MAX;
        far.parts[3].y = i32::MIN;
        assert_eq!(parts_size(&content, &far), (u32::MAX, u32::MAX));
        far.parts.truncate(1);
        assert_eq!(parts_size(&content, &far).0, content.placed_shape(&far.parts[0]).unwrap().dims().0);
        storage.write("far", &DesignDoc(far), 1).unwrap();
        assert_eq!(list_designs(&storage, &content).unwrap()[0].id, "far");
    }

    #[test]
    fn frames_refuse_designs_that_do_not_fit() {
        let content = data::base_content();
        let runner = content.ships.get("halvex_runner").unwrap();
        assert_eq!(open_in_first_frame(&content, runner).unwrap().frame(), (24, 24));
        // A design wider than every frame.
        let mut wide = runner.clone();
        let mut far = wide.parts[0].clone();
        far.x = 200;
        wide.parts.push(far);
        wide.size = (300, 20);
        let e = open_in_first_frame(&content, &wide).err().unwrap();
        assert!(e.contains("larger than every frame"), "{e}");
    }
}
