//! The player's behaviors on the storage layer (GDR-0057, written 13 Oct 2026): saved on their own like designs,
//! named by designs, carried by share codes. Plain functions and one resource ([`PlayerBehaviors`]), so the
//! behavior screen calls them and tests run without an app.
//!
//! * **A saved behavior** ([`BehaviorDoc`], `behaviors/<id>`, format 1): exactly a content behavior
//!   (`content/base/behaviors/*.ron`: id, name, summary, optional description, lore and tags, the `target` line
//!   without its keyword, one string per rule), so it can move into a pack as is.
//! * **Ids**: snake_case from the name on the first save ([`PlayerBehaviors::new_id`]), unique among saved
//!   behaviors and never a content behavior's id. Content behaviors (`skirmisher`, `raider_basic`) are templates:
//!   the player edits a copy ([`PlayerBehaviors::copy_of`]).
//! * **Checked against the vocabulary** (the `behavior_nodes` content, as content validation does) on load and on
//!   every save: a behavior with errors still loads, as its lines, so the player can fix it;
//!   [`PlayerBehavior::errors`] locates them in its text form ([`text_of`]). Only behaviors without errors fly.
//! * **Flights** ([`PlayerBehaviors::add_to`]): the flight's content gets every player behavior that runs, in id
//!   order after the content behaviors, so `sim::BehaviorLibrary::new` compiles them with handles in a fixed order
//!   (behaviors are inputs of the deterministic sim, and handles are part of the state hash) and a design that
//!   names one compiles with it. A design naming one that doesn't run flies its faction's ([`fallback`]).
//! * **Designs** name their behavior in the blueprint's `behavior` field (a content or a player behavior id; none:
//!   the faction's). [`validate_design`] checks a design with the player's behaviors known.

use std::collections::BTreeMap;

use bevy::prelude::*;
use data::behavior::{LineError, LineRef, Program, TextError, check_program};
use data::{Behavior, Blueprint, Content, Kind, Problem};

use crate::sim_bridge::GameContent;
use crate::storage::{Document, Storage, StorageError, valid_name};

use super::library::{fresh_name, slug_or, unique_id};

/// The summary behaviors get when they have none (content requires one; the editor has no field for it yet).
pub const NEW_SUMMARY: &str = "A behavior from the behavior editor.";

/// A saved behavior: a content behavior (see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct BehaviorDoc(pub Behavior);

impl Document for BehaviorDoc {
    const NAMESPACE: &'static str = "behaviors";
    const FORMAT: u32 = 1;

    fn to_ron(&self) -> String {
        behavior_to_ron(&self.0)
    }

    fn from_ron(text: &str, _format: u32) -> Result<Self, String> {
        parse_behavior(text).map(BehaviorDoc)
    }
}

/// A behavior as RON in the content format (`behaviors/*.ron`): the header fields, `target`, then `rules` with
/// one rule per line, four-space indent. Absent and empty fields are left out; `source` is never written.
pub fn behavior_to_ron(b: &Behavior) -> String {
    fn field(s: &mut String, name: &str, value: &str) {
        s.push_str(&format!("    {name}: {value},\n"));
    }
    let q = |t: &str| ron::to_string(t).unwrap_or_default();
    let mut s = String::from("(\n");
    field(&mut s, "id", &q(&b.id));
    field(&mut s, "name", &q(&b.name));
    field(&mut s, "summary", &q(&b.summary));
    if let Some(d) = &b.description {
        field(&mut s, "description", &q(d));
    }
    if let Some(l) = &b.lore {
        field(&mut s, "lore", &q(l));
    }
    if !b.tags.is_empty() {
        let tags: Vec<String> = b.tags.iter().map(|t| q(t)).collect();
        field(&mut s, "tags", &format!("[{}]", tags.join(", ")));
    }
    if let Some(t) = &b.target {
        field(&mut s, "target", &q(t));
    }
    s.push_str("    rules: [\n");
    for r in &b.rules {
        s.push_str(&format!("        {},\n", q(r)));
    }
    s.push_str("    ],\n)\n");
    s
}

/// Reads one behavior written as RON (by [`behavior_to_ron`] or by hand) with the pack loader's options. Only the
/// syntax and the schema are checked here; [`check`] checks the program.
pub fn parse_behavior(text: &str) -> Result<Behavior, String> {
    data::ron_options().from_str(text).map_err(|e| e.to_string())
}

/// The text form of a behavior, as the editor's TEXT view shows it: `target <selector>` (when it has a target
/// line), then one line per rule.
#[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
pub fn text_of(b: &Behavior) -> String {
    let mut s = String::new();
    if let Some(t) = &b.target {
        if !has_target_keyword(t) {
            s.push_str("target ");
        }
        s.push_str(t);
        s.push('\n');
    }
    for r in &b.rules {
        s.push_str(r);
        s.push('\n');
    }
    s
}

/// Sets a behavior's lines from the editor's text: the canonical print when it parses, else the lines as typed
/// (blank lines dropped, a first `target` line into the target field), so a program with errors is kept as
/// written and comes back the same.
#[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
pub fn set_text(b: &mut Behavior, text: &str) {
    if let Ok(p) = Program::parse(text) {
        return set_program(b, &p);
    }
    b.target = None;
    b.rules.clear();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if b.target.is_none() && b.rules.is_empty() && has_target_keyword(line) {
            b.target = Some(line[6..].trim().to_string());
        } else {
            b.rules.push(line.to_string());
        }
    }
}

/// Sets a behavior's lines from a program (the canonical form).
#[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
pub fn set_program(b: &mut Behavior, p: &Program) {
    (b.target, b.rules) = p.to_parts();
}

fn has_target_keyword(line: &str) -> bool {
    let t = line.trim_start();
    t.get(..6).is_some_and(|w| w.eq_ignore_ascii_case("target")) && t[6..].starts_with([' ', '\t'])
}

/// Checks a behavior against the vocabulary, as content validation does; errors are located in its text form
/// ([`text_of`]: line 1 is the target line when it has one). Empty: it runs.
pub fn check(b: &Behavior, content: &Content) -> Vec<TextError> {
    let errors = match b.program() {
        Ok(p) => check_program(&p, &content.behavior_nodes),
        Err(errors) => errors,
    };
    let target = b.target.as_deref();
    let shift = target.map_or(0, |t| if has_target_keyword(t) { 0 } else { "target ".len() as u32 });
    let to_text = |e: LineError| {
        let (line, col) = match e.line {
            LineRef::Whole => (1, e.col),
            LineRef::Target => (1, e.col + shift),
            LineRef::Rule(i) => (i as u32 + 1 + target.is_some() as u32, e.col),
        };
        TextError { line, col, message: e.message }
    };
    errors.into_iter().map(to_text).collect()
}

/// Parses and checks the editor's text (live errors, a test of a program not saved yet).
#[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
pub fn check_text(text: &str, content: &Content) -> Result<Program, Vec<TextError>> {
    let (program, map) = Program::parse_with_lines(text)?;
    let errors = check_program(&program, &content.behavior_nodes);
    if errors.is_empty() { Ok(program) } else { Err(errors.into_iter().map(|e| map.to_text_error(e)).collect()) }
}

/// "1 error: line 2, column 6: …" or "3 errors: …" with the first one.
pub fn errors_note(errors: &[TextError]) -> String {
    match errors {
        [] => "no errors".into(),
        [e] => format!("1 error: {e}"),
        [e, ..] => format!("{} errors: {e}", errors.len()),
    }
}

/// Is this a valid id for a player behavior? The content rule (snake_case: a lowercase letter, then lowercase
/// letters, digits and `_`), within the storage's name rules.
fn valid_id(id: &str) -> bool {
    valid_name(id) && id.starts_with(|c: char| c.is_ascii_lowercase()) && !id.contains('-')
}

/// One of the player's saved behaviors.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerBehavior {
    pub behavior: Behavior,
    /// When it was saved (Unix seconds).
    pub saved: Option<u64>,
    /// What is wrong with it, by line and column of its text form ([`text_of`]); empty: it runs.
    pub errors: Vec<TextError>,
}

impl PlayerBehavior {
    /// It has no errors: flights can fly it.
    pub fn runs(&self) -> bool {
        self.errors.is_empty()
    }
}

/// One row of a behavior list: a saved behavior, or a document that doesn't read (listed so it can be deleted).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen lists them (GDR-0057 wave 2)"))]
pub struct BehaviorEntry {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub rules: usize,
    /// When it was saved (Unix seconds).
    pub saved: Option<u64>,
    /// Why it doesn't run (its errors) or doesn't read, if so.
    pub problem: Option<String>,
}

/// The player's saved behaviors (a resource), read from the storage at start and kept in step with it by the
/// operations below. Every operation that changes something raises [`PlayerBehaviors::revision`].
#[derive(Resource, Clone, Debug, Default)]
pub struct PlayerBehaviors {
    by_id: BTreeMap<String, PlayerBehavior>,
    /// Documents that don't read (or use a content behavior's id), with why.
    unreadable: BTreeMap<String, String>,
    revision: u64,
}

impl PlayerBehaviors {
    /// Reads every saved behavior and checks it against `content`'s vocabulary.
    pub fn load(storage: &Storage, content: &Content) -> Result<Self, StorageError> {
        let mut out = Self::default();
        for id in storage.list(BehaviorDoc::NAMESPACE)? {
            match storage.read::<BehaviorDoc>(&id) {
                Ok(Some(_)) if content.behaviors.contains(&id) || !valid_id(&id) => {
                    out.unreadable.insert(id.clone(), format!("{id} is not a free behavior id (content uses it, or it is no snake_case id)"));
                }
                Ok(Some(s)) => {
                    // The name it is stored under is its id (a file copied in by hand may say otherwise).
                    let behavior = Behavior { id: id.clone(), ..s.doc.0 };
                    let errors = check(&behavior, content);
                    out.by_id.insert(id, PlayerBehavior { behavior, saved: s.saved, errors });
                }
                Ok(None) => {}
                Err(e) => {
                    out.unreadable.insert(id, e.to_string());
                }
            }
        }
        Ok(out)
    }

    /// Goes up with every change.
    #[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// A saved behavior.
    pub fn get(&self, id: &str) -> Option<&PlayerBehavior> {
        self.by_id.get(id)
    }

    /// Every saved behavior that reads, by id.
    pub fn iter(&self) -> impl Iterator<Item = &PlayerBehavior> {
        self.by_id.values()
    }

    /// A content behavior (a template) or a saved one, by id (they never share an id).
    #[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
    pub fn behavior<'a>(&'a self, content: &'a Content, id: &str) -> Option<&'a Behavior> {
        content.behaviors.get(id).or_else(|| self.by_id.get(id).map(|p| &p.behavior))
    }

    /// The saved behaviors, most recently saved first (then by name), with those that don't read.
    #[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
    pub fn list(&self) -> Vec<BehaviorEntry> {
        let mut out: Vec<BehaviorEntry> = self
            .by_id
            .values()
            .map(|p| BehaviorEntry {
                id: p.behavior.id.clone(),
                name: p.behavior.name.clone(),
                summary: p.behavior.summary.clone(),
                rules: p.behavior.rules.len(),
                saved: p.saved,
                problem: (!p.runs()).then(|| errors_note(&p.errors)),
            })
            .chain(self.unreadable.iter().map(|(id, why)| BehaviorEntry {
                id: id.clone(),
                name: id.clone(),
                summary: String::new(),
                rules: 0,
                saved: None,
                problem: Some(why.clone()),
            }))
            .collect();
        out.sort_by(|a, b| b.saved.cmp(&a.saved).then_with(|| a.name.cmp(&b.name)));
        out
    }

    /// The names of the saved behaviors (for fresh names).
    #[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
    pub fn names(&self) -> Vec<String> {
        self.by_id.values().map(|p| p.behavior.name.clone()).collect()
    }

    fn taken(&self, content: &Content, id: &str) -> bool {
        content.behaviors.contains(id) || self.by_id.contains_key(id) || self.unreadable.contains_key(id)
    }

    /// A new id for a behavior called `name`: snake_case, unique among saved behaviors, never a content
    /// behavior's id ("Skirmisher" → `skirmisher_2`).
    pub fn new_id(&self, content: &Content, name: &str) -> String {
        unique_id(&slug_or(name, "behavior"), |id| self.taken(content, id))
    }

    /// An unsaved copy of a content or saved behavior with a fresh name ("Skirmisher copy") and a new id.
    #[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
    pub fn copy_of(&self, content: &Content, id: &str) -> Result<Behavior, String> {
        let from = self.behavior(content, id).ok_or_else(|| format!("no behavior {id}"))?;
        let name = fresh_name(&format!("{} copy", from.name), &self.names());
        let id = self.new_id(content, &name);
        Ok(Behavior { id, name, source: Default::default(), ..from.clone() })
    }

    /// Saves a behavior under its id (a new one from its name when the id is empty) and checks it; returns the id.
    /// A content behavior's id is refused (save a copy).
    pub fn save(&mut self, storage: &mut Storage, content: &Content, behavior: &Behavior, now: u64) -> Result<String, String> {
        let mut b = Behavior { source: Default::default(), ..behavior.clone() };
        if b.id.is_empty() {
            b.id = self.new_id(content, &b.name);
        }
        if content.behaviors.contains(&b.id) {
            return Err(format!("{} is a content behavior; save a copy", b.id));
        }
        if !valid_id(&b.id) {
            return Err(format!("not a valid behavior id: {:?}", b.id));
        }
        if b.name.trim().is_empty() {
            return Err("a behavior needs a name".into());
        }
        if b.summary.trim().is_empty() {
            b.summary = NEW_SUMMARY.into();
        }
        storage.write(&b.id, &BehaviorDoc(b.clone()), now).map_err(|e| e.to_string())?;
        let id = b.id.clone();
        let errors = check(&b, content);
        self.unreadable.remove(&id);
        self.by_id.insert(id.clone(), PlayerBehavior { behavior: b, saved: Some(now), errors });
        self.revision += 1;
        Ok(id)
    }

    /// Renames a saved behavior (its id stays, so designs keep naming it).
    #[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
    pub fn rename(&mut self, storage: &mut Storage, id: &str, name: &str, now: u64) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("a behavior needs a name".into());
        }
        let p = self.by_id.get_mut(id).ok_or_else(|| format!("{id} is gone"))?;
        let b = Behavior { name: name.to_string(), ..p.behavior.clone() };
        storage.write(id, &BehaviorDoc(b.clone()), now).map_err(|e| e.to_string())?;
        p.behavior = b;
        p.saved = Some(now);
        self.revision += 1;
        Ok(())
    }

    /// Saves a copy of a content or saved behavior under a new id and a fresh name; returns the new id.
    #[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
    pub fn duplicate(&mut self, storage: &mut Storage, content: &Content, id: &str, now: u64) -> Result<String, String> {
        let copy = self.copy_of(content, id)?;
        self.save(storage, content, &copy, now)
    }

    /// Deletes a saved behavior (or a document that doesn't read). Designs naming it fly their faction's.
    #[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
    pub fn delete(&mut self, storage: &mut Storage, id: &str) -> Result<(), String> {
        storage.delete::<BehaviorDoc>(id).map_err(|e| e.to_string())?;
        self.by_id.remove(id);
        self.unreadable.remove(id);
        self.revision += 1;
        Ok(())
    }

    /// Adds a behavior that came with a shared design: under its own id when that is free, or when the behavior
    /// saved under it has the same name and lines (nothing is added then); else under a new id. Returns the id and
    /// a note for the status line.
    pub fn import(&mut self, storage: &mut Storage, content: &Content, behavior: Behavior, now: u64) -> Result<(String, String), String> {
        let mut b = behavior;
        if let Some(have) = self.by_id.get(&b.id)
            && (&have.behavior.name, &have.behavior.target, &have.behavior.rules) == (&b.name, &b.target, &b.rules)
        {
            return Ok((b.id.clone(), format!("behavior {} is here already", b.name)));
        }
        if b.name.trim().is_empty() {
            b.name = "Imported behavior".into();
        }
        if !valid_id(&b.id) || self.taken(content, &b.id) {
            b.id = self.new_id(content, &b.name);
        }
        let id = self.save(storage, content, &b, now)?;
        let p = &self.by_id[&id];
        let mut note = format!("behavior {} added", p.behavior.name);
        if !p.runs() {
            note.push_str(&format!(" ({}; it flies once fixed)", errors_note(&p.errors)));
        }
        Ok((id, note))
    }

    /// Adds every saved behavior that runs to a flight's content, in id order after its own behaviors (see the
    /// module docs).
    pub fn add_to(&self, content: &mut Content) {
        for p in self.by_id.values().filter(|p| p.runs()) {
            if !content.behaviors.contains(&p.behavior.id) {
                content.behaviors.upsert(p.behavior.clone());
            }
        }
    }

    /// The player behavior a design names, if it names one (a content behavior or none: `None`), for exports.
    pub fn named_by<'a>(&'a self, content: &Content, bp: &Blueprint) -> Option<&'a Behavior> {
        bp.behavior.as_deref().filter(|id| !content.behaviors.contains(id)).and_then(|id| self.by_id.get(id)).map(|p| &p.behavior)
    }
}

/// Checks a design like `data::validate_ship`, with the player's behaviors known: a design may name one (its rules
/// must fit the design's rule slots, as for content behaviors), and one with errors is a problem of its own.
pub fn validate_design(content: &Content, behaviors: &PlayerBehaviors, bp: &Blueprint) -> Vec<Problem> {
    let Some(p) = bp.behavior.as_deref().filter(|id| !content.behaviors.contains(id)).and_then(|id| behaviors.get(id)) else {
        return data::validate_ship(content, bp);
    };
    let mut with = content.clone();
    with.behaviors.upsert(p.behavior.clone());
    let mut problems = data::validate_ship(&with, bp);
    if !p.runs() {
        let msg = format!("its behavior `{}` has {}", p.behavior.id, errors_note(&p.errors));
        problems.push(Problem::error(msg).entry(Kind::Ship, &bp.id).field("behavior"));
    }
    problems
}

/// The design as a flight flies it, when it names a behavior the flight's content doesn't have (a player behavior
/// with errors, a deleted one): the blueprint with its faction's behavior instead, and why. `None`: as it is.
pub fn fallback(bp: &Blueprint, flight: &Content, behaviors: &PlayerBehaviors) -> Option<(Blueprint, String)> {
    let id = bp.behavior.as_deref()?;
    if flight.behaviors.contains(id) {
        return None;
    }
    let why = match behaviors.get(id) {
        Some(p) => format!("behavior {} has {}", p.behavior.name, errors_note(&p.errors)),
        None => format!("behavior {id} is gone"),
    };
    let bp = Blueprint { behavior: None, ..bp.clone() };
    let flies = flight.ship_behavior(&bp).map_or_else(|| "no behavior".to_string(), |b| b.name.clone());
    Some((bp, format!("{why}; flies {flies}")))
}

/// Reads the player's behaviors at start (after the storage and the content).
pub(super) struct BehaviorsPlugin;

impl Plugin for BehaviorsPlugin {
    fn build(&self, app: &mut App) {
        let world = app.world();
        let storage = world.get_resource::<Storage>().expect("StoragePlugin is added before DrydockPlugin");
        let behaviors = match PlayerBehaviors::load(storage, &world.resource::<GameContent>().0) {
            Ok(b) => b,
            Err(e) => {
                warn!("behaviors: {e}");
                PlayerBehaviors::default()
            }
        };
        let broken = behaviors.iter().filter(|p| !p.runs()).count();
        info!("behaviors: {} saved ({broken} with errors)", behaviors.iter().count());
        app.insert_resource(behaviors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryBackend;

    fn mem() -> Storage {
        Storage::new(MemoryBackend::default())
    }

    /// A copy of the skirmisher with its first rule changed (flees later), saved as "Careful".
    fn careful(content: &Content) -> Behavior {
        let mut b = content.behaviors.get("skirmisher").unwrap().clone();
        b.id = "careful".into();
        b.name = "Careful".into();
        b.source = Default::default();
        b.rules[0] = "when hull < 50% -> flee # run sooner".into();
        b
    }

    #[test]
    fn documents_round_trip_in_the_content_format() {
        let content = data::base_content();
        for b in content.behaviors.iter() {
            let doc = BehaviorDoc(Behavior { source: Default::default(), ..b.clone() });
            let back = BehaviorDoc::from_ron(&doc.to_ron(), 1).unwrap();
            assert_eq!(back, doc, "{}", doc.to_ron());
        }
        // Odd strings survive; a saved document reads back with its header and time.
        let mut b = careful(&content);
        b.name = "Läufer \"Mk\" \\ II ★".into();
        b.tags = vec!["a".into(), "b c".into()];
        b.lore = Some("line\nbreak".into());
        b.target = None;
        let mut storage = mem();
        storage.write("careful", &BehaviorDoc(b.clone()), 7).unwrap();
        let s = storage.read::<BehaviorDoc>("careful").unwrap().unwrap();
        assert_eq!((s.doc, s.saved), (BehaviorDoc(b), Some(7)));
        // The text is a content behavior file (the pack loader's options; `Some` may be left out).
        let text = behavior_to_ron(&careful(&content));
        assert!(text.contains("    target: \"nearest\",\n") && text.contains("        \"when hull < 50% -> flee # run sooner\",\n"), "{text}");
        assert!(parse_behavior("(id: \"x\", name: 3)").unwrap_err().contains("1:"));
    }

    #[test]
    fn ids_are_snake_case_unique_and_never_a_content_behavior() {
        let content = data::base_content();
        let mut storage = mem();
        let mut lib = PlayerBehaviors::default();
        assert_eq!(lib.new_id(&content, "Skirmisher"), "skirmisher_2");
        assert_eq!(lib.new_id(&content, "Raider basic"), "raider_basic_2");
        assert_eq!(lib.new_id(&content, "3 rules"), "behavior_3_rules");
        assert_eq!(lib.new_id(&content, "★"), "behavior");
        // An empty id gets one from the name; the next one with that name is numbered.
        let b = Behavior { id: String::new(), name: "Hit and run".into(), ..careful(&content) };
        assert_eq!(lib.save(&mut storage, &content, &b, 1).unwrap(), "hit_and_run");
        assert_eq!(lib.save(&mut storage, &content, &b, 2).unwrap(), "hit_and_run_2");
        // Content ids and bad ids are refused.
        let skirm = content.behaviors.get("skirmisher").unwrap().clone();
        assert!(lib.save(&mut storage, &content, &skirm, 3).unwrap_err().contains("content behavior"));
        let bad = Behavior { id: "Bad-Id".into(), ..careful(&content) };
        assert!(lib.save(&mut storage, &content, &bad, 3).is_err());
        assert_eq!(storage.list("behaviors").unwrap(), ["hit_and_run", "hit_and_run_2"]);
        // A document under a content behavior's id (a hand-copied file) is listed as a problem, never flown.
        storage.write("skirmisher", &BehaviorDoc(skirm), 4).unwrap();
        let lib = PlayerBehaviors::load(&storage, &content).unwrap();
        assert!(lib.get("skirmisher").is_none());
        assert!(lib.list().iter().any(|e| e.id == "skirmisher" && e.problem.is_some()));
    }

    #[test]
    fn save_list_rename_duplicate_delete_and_templates() {
        let content = data::base_content();
        let mut storage = mem();
        let mut lib = PlayerBehaviors::default();
        // A template is copied, not edited.
        let copy = lib.copy_of(&content, "skirmisher").unwrap();
        assert_eq!((copy.id.as_str(), copy.name.as_str()), ("skirmisher_copy", "Skirmisher copy"));
        assert_eq!(copy.rules, content.behaviors.get("skirmisher").unwrap().rules);
        let r0 = lib.revision();
        let id = lib.save(&mut storage, &content, &copy, 10).unwrap();
        assert!(lib.revision() > r0);
        assert!(lib.get(&id).unwrap().runs());
        // The next copy gets the next name; DUPLICATE saves it.
        let dup = lib.duplicate(&mut storage, &content, "skirmisher", 20).unwrap();
        assert_eq!((dup.as_str(), lib.get(&dup).unwrap().behavior.name.as_str()), ("skirmisher_copy_2", "Skirmisher copy 2"));
        let dup2 = lib.duplicate(&mut storage, &content, &id, 30).unwrap();
        assert_eq!(lib.get(&dup2).unwrap().behavior.name, "Skirmisher copy copy");
        // Renaming keeps the id.
        lib.rename(&mut storage, &id, "  Careful  ", 40).unwrap();
        assert_eq!(lib.get(&id).unwrap().behavior.name, "Careful");
        assert!(lib.rename(&mut storage, &id, " ", 41).is_err());
        // The list: most recently saved first; it reads back the same from the storage.
        let names: Vec<String> = lib.list().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["Careful", "Skirmisher copy copy", "Skirmisher copy 2"]);
        assert_eq!(PlayerBehaviors::load(&storage, &content).unwrap().list(), lib.list());
        assert_eq!(lib.behavior(&content, &id).unwrap().name, "Careful");
        assert_eq!(lib.behavior(&content, "raider_basic").unwrap().name, "Raider pack");
        lib.delete(&mut storage, &dup2).unwrap();
        assert!(lib.get(&dup2).is_none());
        assert_eq!(storage.list("behaviors").unwrap(), [id, dup]);
    }

    #[test]
    fn a_behavior_with_errors_loads_as_text_with_located_errors() {
        let content = data::base_content();
        let mut storage = mem();
        let mut lib = PlayerBehaviors::default();
        let mut b = careful(&content);
        // Typed in the TEXT view: a misspelt condition on text line 3 (rule 2), a percentage without `%`.
        let text = "target nearest\nwhen hull < 50% -> flee\nwhen sheild_down -> orbit(80%, ccw)\nwhen hull < 30 -> hold\nalways -> approach(67%)\n";
        set_text(&mut b, text);
        assert_eq!(text_of(&b), text);
        let id = lib.save(&mut storage, &content, &b, 1).unwrap();
        let p = lib.get(&id).unwrap();
        assert!(!p.runs());
        assert_eq!(p.errors.len(), 2, "{:?}", p.errors);
        assert_eq!((p.errors[0].line, p.errors[0].col), (3, 6));
        assert!(p.errors[0].message.contains("shield_down"), "{}", p.errors[0]);
        assert_eq!(p.errors[1].line, 4);
        // The same errors as the text check, which the editor runs live.
        assert_eq!(check_text(text, &content).unwrap_err(), p.errors);
        // It still loads (as its lines) and is listed with the first error; flights skip it.
        let back = PlayerBehaviors::load(&storage, &content).unwrap();
        assert_eq!(text_of(&back.get(&id).unwrap().behavior), text);
        assert!(back.list()[0].problem.as_deref().unwrap().starts_with("2 errors: line 3, column 6"));
        let mut flight = content.clone();
        back.add_to(&mut flight);
        assert!(!flight.behaviors.contains(&id));
        // Syntax errors too (an unfinished line), and a target error is located in the text's first line.
        set_text(&mut b, "target nearest(sideways)\nwhen hull <");
        assert_eq!(b.target.as_deref(), Some("nearest(sideways)"));
        assert_eq!(b.rules, ["when hull <"]);
        let errors = check(&b, &content);
        assert_eq!(errors.iter().map(|e| e.line).collect::<Vec<_>>(), [2]);
        b.rules = vec!["always -> hold".into()];
        let errors = check(&b, &content);
        assert_eq!((errors[0].line, errors[0].col), (1, 16), "{errors:?}");
        // A text that parses is stored canonically.
        set_text(&mut b, "If Hull ≤ 30% → flee\n\nalways -> hold");
        assert_eq!((b.target.as_deref(), b.rules.as_slice()), (None, ["when hull <= 30% -> flee".to_string(), "always -> hold".to_string()].as_slice()));
    }

    #[test]
    fn imports_reuse_the_same_behavior_and_rename_a_clash() {
        let content = data::base_content();
        let mut storage = mem();
        let mut lib = PlayerBehaviors::default();
        let b = careful(&content);
        let (id, note) = lib.import(&mut storage, &content, b.clone(), 1).unwrap();
        assert_eq!((id.as_str(), note.as_str()), ("careful", "behavior Careful added"));
        // The same behavior again: linked, nothing added.
        let (again, note) = lib.import(&mut storage, &content, b.clone(), 2).unwrap();
        assert_eq!(again, "careful");
        assert!(note.contains("here already"));
        // Another behavior under the same id, or under a content id: a new id.
        let other = Behavior { rules: vec!["always -> hold".into()], ..b.clone() };
        assert_eq!(lib.import(&mut storage, &content, other, 3).unwrap().0, "careful_2");
        let named_skirm = Behavior { id: "skirmisher".into(), name: "Skirmisher".into(), ..b };
        assert_eq!(lib.import(&mut storage, &content, named_skirm, 4).unwrap().0, "skirmisher_2");
        assert_eq!(storage.list("behaviors").unwrap(), ["careful", "careful_2", "skirmisher_2"]);
    }

    #[test]
    fn designs_name_player_behaviors() {
        let content = data::base_content();
        let mut storage = mem();
        let mut lib = PlayerBehaviors::default();
        let id = lib.save(&mut storage, &content, &careful(&content), 1).unwrap();
        let mut runner = content.ships.get("halvex_runner").unwrap().clone();
        runner.behavior = Some(id.clone());
        // Content validation alone doesn't know it; with the player's behaviors the design is clean.
        assert!(data::validate_ship(&content, &runner).iter().any(|p| p.message.contains("unknown behavior")));
        assert!(!validate_design(&content, &lib, &runner).iter().any(Problem::is_error));
        assert_eq!(lib.named_by(&content, &runner).unwrap().id, id);
        assert!(fallback(&runner, &{ let mut c = content.clone(); lib.add_to(&mut c); c }, &lib).is_none());
        // Too many rules for the runner's two logic cores (12 slots).
        let long = Behavior { id: String::new(), name: "Long".into(), rules: vec!["always -> hold".into(); 13], ..careful(&content) };
        let long = lib.save(&mut storage, &content, &long, 2).unwrap();
        runner.behavior = Some(long);
        let problems = validate_design(&content, &lib, &runner);
        assert!(problems.iter().any(|p| p.message.contains("rule slots")), "{problems:?}");
        // One with errors: a problem, and flights fly the faction's behavior instead.
        let mut broken = careful(&content);
        broken.rules[1] = "when sheild_down -> hold".into();
        let broken = lib.import(&mut storage, &content, broken, 3).unwrap().0;
        runner.behavior = Some(broken);
        let problems = validate_design(&content, &lib, &runner);
        assert!(problems.iter().any(|p| p.field.as_deref() == Some("behavior") && p.message.contains("1 error")), "{problems:?}");
        let mut flight = content.clone();
        lib.add_to(&mut flight);
        let (flown, why) = fallback(&runner, &flight, &lib).unwrap();
        assert_eq!(flown.behavior, None);
        assert!(why.ends_with("flies Skirmisher"), "{why}");
        // Content behaviors are no player behaviors.
        runner.behavior = Some("raider_basic".into());
        assert!(lib.named_by(&content, &runner).is_none() && fallback(&runner, &flight, &lib).is_none());
    }

    /// Flights compile the player's behaviors after the content's, in id order, whatever order they were saved
    /// in; the same behaviors give the same handles and the same fight, tick for tick.
    #[test]
    fn the_flight_library_is_built_in_a_fixed_order() {
        let content = data::base_content();
        let order = |ids: &[&str]| {
            let mut storage = mem();
            let mut lib = PlayerBehaviors::default();
            for (k, id) in ids.iter().enumerate() {
                let b = Behavior { id: id.to_string(), name: id.to_string(), ..careful(&content) };
                lib.save(&mut storage, &content, &b, k as u64).unwrap();
            }
            // Read back as at start.
            let lib = PlayerBehaviors::load(&storage, &content).unwrap();
            let mut flight = content.clone();
            lib.add_to(&mut flight);
            sim::BehaviorLibrary::new(&flight).unwrap()
        };
        let a = order(&["zeta", "alpha", "mid"]);
        let b = order(&["mid", "zeta", "alpha"]);
        let ids = |l: &sim::BehaviorLibrary| l.iter().map(|(h, b)| (h, b.id.clone(), b.fingerprint)).collect::<Vec<_>>();
        assert_eq!(ids(&a), ids(&b));
        let n = content.behaviors.len() as u32;
        assert_eq!(a.id("alpha"), Some(sim::BehaviorId(n)));
        assert_eq!(a.id("zeta"), Some(sim::BehaviorId(n + 2)));
        // A runner under the player's behavior against a raider: the same state hash every tick.
        let fight = |lib: sim::BehaviorLibrary| {
            let mut runner = content.ships.get("halvex_runner").unwrap().clone();
            runner.behavior = Some("mid".into());
            let mut flight = content.clone();
            flight.behaviors.upsert(Behavior { id: "mid".into(), name: "mid".into(), ..careful(&content) });
            let design = std::sync::Arc::new(sim::ShipDesign::compile(&flight, &runner).unwrap());
            let raider = std::sync::Arc::new(sim::ShipDesign::compile(&content, content.ships.get("automaton_raider").unwrap()).unwrap());
            let mut world = sim::Sim::new(7);
            world.set_behaviors(lib);
            let me = world.spawn_ship(design.clone(), sim::Owner::PLAYER, sim::Body::at(sim::Vec2::ZERO, 0.0));
            let them = world.spawn_ship(raider.clone(), sim::Owner(1), sim::Body::at(sim::Vec2::new(0.0, 90.0), 3.0));
            let mine = world.behaviors().default_for(&design);
            assert_eq!(mine, world.behaviors().id("mid"));
            world.command(me, sim::ShipCommand::Behavior(mine));
            world.command(them, sim::ShipCommand::Behavior(world.behaviors().default_for(&raider)));
            (0..300).map(|_| {
                world.step(&sim::TickInputs::default());
                world.state_hash()
            }).collect::<Vec<u64>>()
        };
        assert_eq!(fight(a), fight(b));
    }
}
