//! The behavior being edited: [`BehaviorDraft`], a [`BehaviorEditor`] (`data::behavior_edit`: the program, legal
//! edits, undo) plus the text view's text and its errors. The graph view shows [`BehaviorDraft::program`] and
//! edits it through [`BehaviorDraft::edit`]; the text view edits [`BehaviorDraft::text`]: a valid text replaces
//! the program, an invalid one keeps the last valid program and lists its errors.
//!
//! A draft starts as a copy of a behavior: the one the design flies ([`design_behavior`]: a content template or
//! one of the player's saved behaviors; GDR-0057), or one picked in the BEHAVIORS list. It remembers what it
//! looked like when it was opened or last saved ([`BehaviorDraft::dirty`]), so the screen can say UNSAVED.

use bevy::prelude::*;
use data::behavior::{Program, TextError, check_program};
use data::behavior_edit::BehaviorEditor;
use data::{Behavior, BehaviorNode, Blueprint, Content, NodeRole, Table};

use crate::drydock::behaviors::{PlayerBehaviors, text_of};

/// The name a new behavior gets ("New behavior", "New behavior 2", ... in the BEHAVIORS list).
pub const NEW_NAME: &str = "New behavior";

/// The behavior on the BEHAVIOR screen.
#[derive(Resource, Clone, Debug, Default)]
pub struct BehaviorDraft {
    /// The behavior this draft was copied from (`None`: a new, empty draft).
    pub source: Option<String>,
    /// The behavior's name (the header).
    pub name: String,
    /// The editing model; `None` only before the screen first opens.
    editor: Option<BehaviorEditor>,
    /// The text view's text: the canonical text of the program, or the player's text while typing.
    pub text: String,
    /// Problems in `text` (syntax and vocabulary), by text line and column; empty when `text` is valid.
    pub errors: Vec<TextError>,
    /// Goes up whenever the program changes or another draft opens (the graph rebuilds then); never goes back.
    revision: u64,
    /// Goes up when another draft replaces this one ([`BehaviorDraft::replace`]): the editor's ids (rules,
    /// conditions) mean nothing in the next one, so a selection made in this one must go.
    generation: u64,
    /// `editor.revision()` when the draft last caught up with it.
    seen: u64,
    /// The draft when it was opened or last saved (see [`BehaviorDraft::dirty`]).
    clean: Snapshot,
}

/// The parts of a draft that are saved: its name and its rules (the canonical text of a valid program, else the
/// text as typed), to tell unsaved edits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub name: String,
    pub body: String,
}

/// The lines of a behavior that check on their own (the target line too, if it does), opened in the editor: what
/// the graph shows while the behavior's text has errors. `None` if no rule checks.
fn checked_part(b: &Behavior, nodes: &Table<BehaviorNode>) -> Option<BehaviorEditor> {
    let checks = |target: Option<&str>, rules: &[String]| Program::from_parts(target, rules).ok().filter(|p| check_program(p, nodes).is_empty());
    let rules: Vec<String> = b.rules.iter().filter(|r| checks(None, std::slice::from_ref(*r)).is_some()).cloned().collect();
    let target = b.target.as_deref().filter(|t| checks(Some(t), &rules).is_some());
    BehaviorEditor::open(nodes, &checks(target, &rules)?).ok()
}

/// The behavior a design flies when nobody pilots it, for the editor: the one it names (a content template or one
/// of the player's saved behaviors, also one with errors, so it can be fixed), else its faction's default. A
/// name that is gone gives the faction's default, as flights do.
pub fn design_behavior<'a>(content: &'a Content, behaviors: &'a PlayerBehaviors, design: &Blueprint) -> Option<&'a Behavior> {
    design.behavior.as_deref().and_then(|id| behaviors.behavior(content, id)).or_else(|| {
        let faction = content.factions.get(&design.faction)?;
        content.behaviors.get(faction.behavior.as_deref()?)
    })
}

impl BehaviorDraft {
    /// A copy of a behavior (a content template or a saved one). Rules that don't check against the vocabulary
    /// (a saved behavior with errors) open as their text with the errors; the program (what the graph shows) is
    /// then the lines that check on their own until the text is fixed. `None` only when the vocabulary has no
    /// action.
    pub fn from_behavior(b: &Behavior, nodes: &Table<BehaviorNode>) -> Option<Self> {
        let mut d = match b.program().ok().and_then(|p| BehaviorEditor::open(nodes, &p).ok()) {
            Some(editor) => Self::with_editor(Some(b.id.clone()), b.name.clone(), editor),
            None => {
                let mut d = match checked_part(b, nodes) {
                    Some(editor) => Self::with_editor(None, String::new(), editor),
                    None => Self::empty(nodes)?,
                };
                d.source = Some(b.id.clone());
                d.name.clone_from(&b.name);
                d.set_text(&text_of(b));
                d
            }
        };
        d.mark_clean();
        Some(d)
    }

    /// The behavior a design's draft copies: `wanted` (a content or saved behavior id, e.g. from `?behavior=`) if
    /// it exists, else the one the design flies ([`design_behavior`]).
    pub fn source_for<'a>(content: &'a Content, behaviors: &'a PlayerBehaviors, design: &Blueprint, wanted: Option<&str>) -> Option<&'a Behavior> {
        wanted.and_then(|id| behaviors.behavior(content, id)).or_else(|| design_behavior(content, behaviors, design))
    }

    /// A new draft with one rule: `always ->` the vocabulary's first action (a behavior needs a rule). `None`
    /// if the vocabulary has no action.
    pub fn empty(nodes: &Table<BehaviorNode>) -> Option<Self> {
        let action = nodes.iter().find(|n| n.role == NodeRole::Action)?;
        let editor = BehaviorEditor::from_text(nodes, &format!("always -> {}", action.id)).ok()?;
        Some(Self::with_editor(None, NEW_NAME.into(), editor))
    }

    fn with_editor(source: Option<String>, name: String, editor: BehaviorEditor) -> Self {
        let text = editor.text();
        let mut d = Self {
            source,
            name,
            seen: editor.revision(),
            editor: Some(editor),
            text,
            errors: Vec::new(),
            revision: 1,
            generation: 0,
            clean: Snapshot::default(),
        };
        d.mark_clean();
        d
    }

    /// Replaces this draft with another one (opened, loaded or new); the revision and the generation keep going up.
    pub fn replace(&mut self, mut next: BehaviorDraft) {
        next.revision = self.revision + 1;
        next.generation = self.generation + 1;
        *self = next;
    }

    /// Goes up with every [`BehaviorDraft::replace`] (not with edits).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The last valid program: what the graph shows and what would fly.
    pub fn program(&self) -> &Program {
        static EMPTY: std::sync::OnceLock<Program> = std::sync::OnceLock::new();
        self.editor.as_ref().map_or_else(|| EMPTY.get_or_init(Program::default), |e| e.program())
    }

    /// The editing model, read-only (rule and node views, ids, palette, slots). Edit through
    /// [`BehaviorDraft::edit`].
    pub fn editor(&self) -> Option<&BehaviorEditor> {
        self.editor.as_ref()
    }

    /// Bumped by every change of the program and by opening another draft.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Runs an edit on the editing model (the graph view and the inspector), then brings the text up to date: a
    /// changed program is printed canonically. `None` before the screen opens, and while the text has errors
    /// ([`BehaviorDraft::locked`]): the model holds only the last valid program, so printing it would throw the
    /// typed lines away (undo could not bring them back).
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut BehaviorEditor) -> R) -> Option<R> {
        if self.locked().is_some() {
            return None;
        }
        let editor = self.editor.as_mut()?;
        let r = f(editor);
        self.sync();
        Some(r)
    }

    /// Why the graph and the inspector can't edit the program now: the text has errors (fix them in the TEXT view).
    pub fn locked(&self) -> Option<String> {
        let n = self.errors.len();
        (n > 0).then(|| format!("fix the text first ({n} error{} in the TEXT view)", if n == 1 { "" } else { "s" }))
    }

    /// Closes the editing model's open stroke (a slider drag), also while the text has errors: it changes no
    /// program.
    pub fn end_stroke(&mut self) {
        if let Some(editor) = self.editor.as_mut() {
            editor.end_stroke();
        }
    }

    /// The text view changed: parse and check it. A valid text becomes the program (`true` if that changed
    /// it; one undo step); an invalid one leaves the program as it was and fills `errors`.
    pub fn set_text(&mut self, text: &str) -> bool {
        self.text = text.to_string();
        let Some(editor) = self.editor.as_mut() else { return false };
        match editor.set_text(text) {
            Ok(changed) => {
                self.errors.clear();
                if editor.revision() != self.seen {
                    self.seen = editor.revision();
                    self.revision += 1;
                }
                changed
            }
            Err(errors) => {
                self.errors = errors;
                false
            }
        }
    }

    /// Leaving the text view: a valid text is printed canonically (`If hull≤ 30 %` becomes
    /// `when hull <= 30% -> …`); an invalid one stays as typed.
    pub fn tidy_text(&mut self) {
        if let (true, Some(editor)) = (self.errors.is_empty(), &self.editor) {
            self.text = editor.text();
        }
    }

    /// What would be saved now: the name and the rules (the canonical text of the program when the text is
    /// valid, else the text as typed).
    pub fn snapshot(&self) -> Snapshot {
        let body = if self.errors.is_empty() { self.program().to_text() } else { self.text.clone() };
        Snapshot { name: self.name.trim().to_string(), body }
    }

    /// The draft differs from the behavior it was opened from or last saved as.
    pub fn dirty(&self) -> bool {
        self.clean != self.snapshot()
    }

    /// The draft was just saved (or opened): nothing unsaved.
    pub fn mark_clean(&mut self) {
        self.clean = self.snapshot();
    }

    /// Unsaved from now on, whatever it looks like (a draft restored with unsaved edits whose source is gone).
    pub fn mark_unsaved(&mut self) {
        self.clean = Snapshot::default();
    }

    /// After an edit through the model (never while the text has errors): a changed program becomes the text.
    fn sync(&mut self) {
        let Some(editor) = &self.editor else { return };
        if editor.revision() != self.seen {
            self.seen = editor.revision();
            self.revision += 1;
            self.text = editor.text();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_design_gets_its_faction_default() {
        let content = data::base_content();
        let lib = PlayerBehaviors::default();
        let runner = content.ships.get("halvex_runner").unwrap();
        let b = BehaviorDraft::source_for(&content, &lib, runner, None).unwrap();
        let d = BehaviorDraft::from_behavior(b, &content.behavior_nodes).unwrap();
        assert_eq!(d.source.as_deref(), Some("skirmisher"));
        assert_eq!(d.name, "Skirmisher");
        assert_eq!(d.program().rules.len(), 6);
        assert_eq!(d.text, d.program().to_text());
        assert!(d.text.starts_with("target nearest\nwhen hull < 25% -> flee"));
        assert!(d.errors.is_empty());
        assert!(!d.dirty());
        // A design that names its own behavior gets that one; a wanted behavior comes first.
        let mut own = runner.clone();
        own.behavior = Some("raider_basic".into());
        assert_eq!(BehaviorDraft::source_for(&content, &lib, &own, None).unwrap().id, "raider_basic");
        assert_eq!(BehaviorDraft::source_for(&content, &lib, &own, Some("skirmisher")).unwrap().id, "skirmisher");
        assert_eq!(BehaviorDraft::source_for(&content, &lib, &own, Some("nonsense")).unwrap().id, "raider_basic");
        // A name that is gone: the faction's default, as flights fly it.
        own.behavior = Some("gone".into());
        assert_eq!(design_behavior(&content, &lib, &own).unwrap().id, "skirmisher");
        // No behavior at all (no faction default): none.
        own.behavior = None;
        own.faction = "nobody".into();
        assert!(BehaviorDraft::source_for(&content, &lib, &own, None).is_none());
    }

    #[test]
    fn player_behaviors_open_and_ones_with_errors_open_as_text() {
        use crate::storage::{MemoryBackend, Storage};
        let content = data::base_content();
        let mut storage = Storage::new(MemoryBackend::default());
        let mut lib = PlayerBehaviors::default();
        let mut careful = lib.copy_of(&content, "skirmisher").unwrap();
        careful.name = "Careful".into();
        careful.rules[0] = "when hull < 50% -> flee".into();
        let careful = lib.save(&mut storage, &content, &careful, 1).unwrap();
        let mut broken = lib.copy_of(&content, "raider_basic").unwrap();
        broken.rules[0] = "when sheild_down -> flee".into();
        let broken = lib.save(&mut storage, &content, &broken, 2).unwrap();
        let mut runner = content.ships.get("halvex_runner").unwrap().clone();
        runner.behavior = Some(careful.clone());
        let b = BehaviorDraft::source_for(&content, &lib, &runner, None).unwrap();
        let d = BehaviorDraft::from_behavior(b, &content.behavior_nodes).unwrap();
        assert_eq!((d.source.as_deref(), d.name.as_str()), (Some(careful.as_str()), "Careful"));
        assert!(d.text.contains("when hull < 50% -> flee") && d.errors.is_empty() && !d.dirty());
        // With errors: the saved lines and their errors, nothing unsaved; fixing the text is an edit.
        runner.behavior = Some(broken.clone());
        let b = BehaviorDraft::source_for(&content, &lib, &runner, None).unwrap();
        let mut d = BehaviorDraft::from_behavior(b, &content.behavior_nodes).unwrap();
        assert_eq!(d.source.as_deref(), Some(broken.as_str()));
        assert_eq!(d.text, text_of(b));
        assert_eq!((d.errors.len(), d.errors[0].line, d.errors[0].col), (1, 2, 6));
        // The graph shows the lines that check: the target and the other rule.
        assert_eq!(d.program().to_text(), format!("target nearest\n{}\n", b.rules[1]));
        assert!(!d.dirty());
        let fixed = d.text.replace("sheild", "shield");
        d.set_text(&fixed);
        assert!(d.errors.is_empty() && d.dirty());
        // Renaming is an edit too; saving (marking clean) ends it.
        d.mark_clean();
        d.name = "Fixed".into();
        assert!(d.dirty());
        d.mark_clean();
        assert!(!d.dirty());
    }

    /// While the typed text has errors, the graph and the inspector can't edit: printing the model's last valid
    /// program would replace the typed lines (the broken one is lost, and SAVE would store the loss).
    #[test]
    fn model_edits_wait_while_the_typed_text_has_errors() {
        let content = data::base_content();
        let nodes = &content.behavior_nodes;
        let mut d = BehaviorDraft::from_behavior(content.behaviors.get("skirmisher").unwrap(), nodes).unwrap();
        let typed = "target nearest\nwhen sheild_down -> flee\nalways -> approach(67%)\n";
        assert!(!d.set_text(typed));
        let (r0, program) = (d.revision(), d.program().clone());
        assert_eq!(d.locked().as_deref(), Some("fix the text first (1 error in the TEXT view)"));
        let first = d.editor().unwrap().rule_at(0).unwrap().id;
        assert_eq!(d.edit(|e| e.delete_rule(first)), None);
        assert_eq!(d.edit(|e| e.undo()), None);
        assert_eq!((d.text.as_str(), d.revision(), d.program()), (typed, r0, &program));
        // A slider's stroke still closes; once the text is fixed, edits go through again.
        d.end_stroke();
        assert!(d.set_text(&typed.replace("sheild", "shield")));
        assert!(d.locked().is_none());
        let first = d.editor().unwrap().rule_at(0).unwrap().id;
        assert_eq!(d.edit(|e| e.delete_rule(first)), Some(Ok(true)));
        assert_eq!(d.text, "target nearest\nalways -> approach(67%)\n");
    }

    #[test]
    fn a_valid_text_replaces_the_program_an_invalid_one_keeps_it() {
        let content = data::base_content();
        let nodes = &content.behavior_nodes;
        let mut d = BehaviorDraft::from_behavior(content.behaviors.get("raider_basic").unwrap(), nodes).unwrap();
        let before = d.program().clone();
        // A typo: the program stays, the error names line, column and the suggestion.
        let r0 = d.revision();
        assert!(!d.set_text("target nearest\nwhen sheild_down -> flee\n"));
        assert_eq!(*d.program(), before);
        assert_eq!(d.revision(), r0);
        assert_eq!(d.errors.len(), 1);
        assert_eq!((d.errors[0].line, d.errors[0].col), (2, 6));
        assert!(d.errors[0].message.contains("did you mean `shield_down`?"), "{}", d.errors[0].message);
        // Syntax errors too.
        assert!(!d.set_text("when hull < 30 -> flee"));
        assert!(d.errors[0].message.contains('%'), "{}", d.errors[0].message);
        // Fixed: the program changes, the errors go, the typed text stays until it is tidied.
        assert!(d.set_text("If hull≤ 30 % -> flee"));
        assert!(d.errors.is_empty());
        assert_eq!(d.revision(), r0 + 1);
        assert_eq!(d.program().rules.len(), 1);
        assert_eq!(d.text, "If hull≤ 30 % -> flee");
        d.tidy_text();
        assert_eq!(d.text, "when hull <= 30% -> flee\n");
        // The same program again changes nothing.
        assert!(!d.set_text("when hull <= 30% -> flee"));
        assert_eq!(d.revision(), r0 + 1);
        // No rules at all is an error (a behavior needs one); the last program stays.
        assert!(!d.set_text(""));
        assert_eq!(d.errors.len(), 1);
        assert_eq!(d.program().rules.len(), 1);
    }

    #[test]
    fn graph_edits_print_the_canonical_text() {
        let content = data::base_content();
        let nodes = &content.behavior_nodes;
        let mut d = BehaviorDraft::from_behavior(content.behaviors.get("skirmisher").unwrap(), nodes).unwrap();
        let r0 = d.revision();
        let first = d.editor().unwrap().rule_at(0).unwrap().id;
        assert_eq!(d.edit(|e| e.delete_rule(first)), Some(Ok(true)));
        assert_eq!(d.program().rules.len(), 5);
        assert_eq!(d.text, d.program().to_text());
        assert_eq!(d.revision(), r0 + 1);
        // Undo brings the rule back, again through the text.
        assert_eq!(d.edit(|e| e.undo()), Some(true));
        assert_eq!(d.program().rules.len(), 6);
        assert_eq!(d.revision(), r0 + 2);
        // Opening another draft keeps the revision going up.
        let next = BehaviorDraft::empty(nodes).unwrap();
        assert_eq!(next.program().rules.len(), 1);
        d.replace(next);
        assert_eq!(d.revision(), r0 + 3);
        assert_eq!(d.source, None);
    }
}
