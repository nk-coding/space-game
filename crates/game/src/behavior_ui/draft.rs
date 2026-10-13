//! The behavior being edited: [`BehaviorDraft`], a [`BehaviorEditor`] (`data::behavior_edit`: the program, legal
//! edits, undo) plus the text view's text and its errors. The graph view shows [`BehaviorDraft::program`] and
//! edits it through [`BehaviorDraft::edit`]; the text view edits [`BehaviorDraft::text`]: a valid text replaces
//! the program, an invalid one keeps the last valid program and lists its errors.
//!
//! A draft starts as a copy of a behavior (the design's own `behavior`, else its faction's default; GDR-0057:
//! content behaviors are templates to copy).

use bevy::prelude::*;
use data::behavior::{Program, TextError};
use data::behavior_edit::BehaviorEditor;
use data::{Behavior, BehaviorNode, Blueprint, Content, NodeRole, Table};

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
    /// `editor.revision()` when the draft last caught up with it.
    seen: u64,
}

impl BehaviorDraft {
    /// A copy of a behavior. `None` if its rules don't check against the vocabulary (a content behavior always
    /// does; content is validated at load).
    pub fn from_behavior(b: &Behavior, nodes: &Table<BehaviorNode>) -> Option<Self> {
        let editor = BehaviorEditor::open(nodes, &b.program().ok()?).ok()?;
        Some(Self::with_editor(Some(b.id.clone()), b.name.clone(), editor))
    }

    /// The behavior a design's draft copies: `wanted` (a content id, e.g. from `?behavior=`) if it exists, else
    /// the behavior the design names, else its faction's default.
    pub fn source_for<'a>(content: &'a Content, design: &Blueprint, wanted: Option<&str>) -> Option<&'a Behavior> {
        wanted.and_then(|id| content.behaviors.get(id)).or_else(|| content.ship_behavior(design))
    }

    /// A new draft with one rule: `always ->` the vocabulary's first action (a behavior needs a rule). `None`
    /// if the vocabulary has no action.
    pub fn empty(nodes: &Table<BehaviorNode>) -> Option<Self> {
        let action = nodes.iter().find(|n| n.role == NodeRole::Action)?;
        let editor = BehaviorEditor::from_text(nodes, &format!("always -> {}", action.id)).ok()?;
        Some(Self::with_editor(None, "New behavior".into(), editor))
    }

    fn with_editor(source: Option<String>, name: String, editor: BehaviorEditor) -> Self {
        let text = editor.text();
        Self { source, name, seen: editor.revision(), editor: Some(editor), text, errors: Vec::new(), revision: 1 }
    }

    /// Replaces this draft with another one (opened, loaded or new); the revision keeps going up.
    pub fn replace(&mut self, mut next: BehaviorDraft) {
        next.revision = self.revision + 1;
        *self = next;
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

    /// Runs an edit on the editing model (the graph view and the inspector), then brings the text up to date:
    /// a changed program is printed canonically and the text view's errors go. `None` before the screen opens.
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut BehaviorEditor) -> R) -> Option<R> {
        let editor = self.editor.as_mut()?;
        let r = f(editor);
        self.sync();
        Some(r)
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

    /// After an edit through the model: a changed program becomes the text, the errors go.
    fn sync(&mut self) {
        let Some(editor) = &self.editor else { return };
        if editor.revision() != self.seen {
            self.seen = editor.revision();
            self.revision += 1;
            self.text = editor.text();
            self.errors.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_design_gets_its_faction_default() {
        let content = data::base_content();
        let runner = content.ships.get("halvex_runner").unwrap();
        let b = BehaviorDraft::source_for(&content, runner, None).unwrap();
        let d = BehaviorDraft::from_behavior(b, &content.behavior_nodes).unwrap();
        assert_eq!(d.source.as_deref(), Some("skirmisher"));
        assert_eq!(d.name, "Skirmisher");
        assert_eq!(d.program().rules.len(), 6);
        assert_eq!(d.text, d.program().to_text());
        assert!(d.text.starts_with("target nearest\nwhen hull < 25% -> flee"));
        assert!(d.errors.is_empty());
        // A design that names its own behavior gets that one; a wanted content behavior comes first.
        let mut own = runner.clone();
        own.behavior = Some("raider_basic".into());
        assert_eq!(BehaviorDraft::source_for(&content, &own, None).unwrap().id, "raider_basic");
        assert_eq!(BehaviorDraft::source_for(&content, &own, Some("skirmisher")).unwrap().id, "skirmisher");
        assert_eq!(BehaviorDraft::source_for(&content, &own, Some("nonsense")).unwrap().id, "raider_basic");
        // No behavior at all (no faction default): none.
        own.behavior = None;
        own.faction = "nobody".into();
        assert!(BehaviorDraft::source_for(&content, &own, None).is_none());
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
        d.errors.push(TextError { line: 1, col: 1, message: "x".into() });
        let r0 = d.revision();
        let first = d.editor().unwrap().rule_at(0).unwrap().id;
        assert_eq!(d.edit(|e| e.delete_rule(first)), Some(Ok(true)));
        assert_eq!(d.program().rules.len(), 5);
        assert_eq!(d.text, d.program().to_text());
        assert!(d.errors.is_empty());
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
