//! The behavior being edited: [`BehaviorDraft`], the program as text plus its last valid parse. The graph view
//! shows [`BehaviorDraft::program`], the text view edits [`BehaviorDraft::text`]; a valid text replaces the
//! program, an invalid one keeps the last valid program and lists its errors.
//!
//! For now a draft is a copy of a content behavior (the design's own `behavior`, else its faction's default,
//! GDR-0057: content behaviors are templates to copy). The editing model (`data::behavior_edit`) and the player
//! storage take this resource's place or fill it; nothing on the screen reads more than the fields below.

use bevy::prelude::*;
use data::behavior::{Program, TextError, check_program};
use data::{Behavior, BehaviorNode, Blueprint, Content, Table};

/// The behavior on the BEHAVIOR screen.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BehaviorDraft {
    /// The content behavior this draft was copied from (`None`: none, an empty draft).
    pub source: Option<String>,
    /// The behavior's name (the header).
    pub name: String,
    /// The last valid program: what the graph shows and what would fly.
    pub program: Program,
    /// The text view's text: the canonical text of `program`, or the player's text while typing.
    pub text: String,
    /// Problems in `text` (syntax and vocabulary), by text line and column; empty when `text` is valid.
    pub errors: Vec<TextError>,
    /// Goes up whenever `program` changes (the graph rebuilds then).
    pub revision: u64,
}

impl BehaviorDraft {
    /// A copy of a content behavior.
    pub fn from_behavior(b: &Behavior) -> Self {
        // Content behaviors are validated at load, so they parse; a broken one opens empty rather than panic.
        let program = b.program().unwrap_or_default();
        Self { source: Some(b.id.clone()), name: b.name.clone(), text: program.to_text(), program, errors: Vec::new(), revision: 0 }
    }

    /// The draft for a design: the behavior it names, else its faction's default; empty if neither exists.
    pub fn for_design(content: &Content, design: &Blueprint) -> Self {
        content.ship_behavior(design).map_or_else(Self::empty, Self::from_behavior)
    }

    /// A draft without rules (a design whose faction has no default behavior).
    pub fn empty() -> Self {
        Self { name: "New behavior".into(), ..default() }
    }

    /// The text view changed: parse and check it. A valid text becomes the program (`true` if that changed
    /// it); an invalid one leaves the program as it was and fills `errors`.
    pub fn set_text(&mut self, text: &str, nodes: &Table<BehaviorNode>) -> bool {
        self.text = text.to_string();
        match check_text(text, nodes) {
            Ok(program) => {
                self.errors.clear();
                if program != self.program {
                    self.program = program;
                    self.revision += 1;
                    return true;
                }
                false
            }
            Err(errors) => {
                self.errors = errors;
                false
            }
        }
    }

    /// Replaces the program (the graph view's edits): the text becomes its canonical form, errors go.
    pub fn set_program(&mut self, program: Program) {
        self.text = program.to_text();
        self.errors.clear();
        if program != self.program {
            self.program = program;
            self.revision += 1;
        }
    }

    /// Leaving the text view: a valid text is printed canonically (`If hull≤ 30 %` becomes
    /// `when hull <= 30% -> …`); an invalid one stays as typed.
    pub fn tidy_text(&mut self) {
        if self.errors.is_empty() {
            self.text = self.program.to_text();
        }
    }
}

/// Parses a behavior text and checks it against the vocabulary; every problem with its text line and column.
pub fn check_text(text: &str, nodes: &Table<BehaviorNode>) -> Result<Program, Vec<TextError>> {
    let (program, lines) = Program::parse_with_lines(text)?;
    let errors = check_program(&program, nodes);
    if errors.is_empty() { Ok(program) } else { Err(errors.into_iter().map(|e| lines.to_text_error(e)).collect()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_design_gets_its_faction_default() {
        let content = data::base_content();
        let runner = content.ships.get("halvex_runner").unwrap();
        let d = BehaviorDraft::for_design(&content, runner);
        assert_eq!(d.source.as_deref(), Some("skirmisher"));
        assert_eq!(d.name, "Skirmisher");
        assert_eq!(d.program.rules.len(), 6);
        assert_eq!(d.text, d.program.to_text());
        assert!(d.text.starts_with("target nearest\nwhen hull < 25% -> flee"));
        // A design that names its own behavior gets that one.
        let mut own = runner.clone();
        own.behavior = Some("raider_basic".into());
        assert_eq!(BehaviorDraft::for_design(&content, &own).source.as_deref(), Some("raider_basic"));
    }

    #[test]
    fn a_valid_text_replaces_the_program_an_invalid_one_keeps_it() {
        let content = data::base_content();
        let nodes = &content.behavior_nodes;
        let mut d = BehaviorDraft::from_behavior(content.behaviors.get("raider_basic").unwrap());
        let before = d.program.clone();
        // A typo: the program stays, the error names line, column and the suggestion.
        assert!(!d.set_text("target nearest\nwhen sheild_down -> flee\n", nodes));
        assert_eq!(d.program, before);
        assert_eq!(d.revision, 0);
        assert_eq!(d.errors.len(), 1);
        assert_eq!((d.errors[0].line, d.errors[0].col), (2, 6));
        assert!(d.errors[0].message.contains("did you mean `shield_down`?"), "{}", d.errors[0].message);
        // Syntax errors too.
        assert!(!d.set_text("when hull < 30 -> flee", nodes));
        assert!(d.errors[0].message.contains('%'), "{}", d.errors[0].message);
        // Fixed: the program changes, the errors go, the typed text stays until it is tidied.
        assert!(d.set_text("If hull≤ 30 % -> flee", nodes));
        assert!(d.errors.is_empty());
        assert_eq!(d.revision, 1);
        assert_eq!(d.program.rules.len(), 1);
        assert_eq!(d.text, "If hull≤ 30 % -> flee");
        d.tidy_text();
        assert_eq!(d.text, "when hull <= 30% -> flee\n");
        // The same program again changes nothing.
        assert!(!d.set_text("when hull <= 30% -> flee", nodes));
        assert_eq!(d.revision, 1);
        // No rules at all is an error (a behavior needs one); the last program stays.
        assert!(!d.set_text("", nodes));
        assert_eq!(d.errors.len(), 1);
        assert_eq!(d.program.rules.len(), 1);
    }

    #[test]
    fn set_program_prints_the_canonical_text() {
        let content = data::base_content();
        let mut d = BehaviorDraft::empty();
        assert!(d.program.rules.is_empty());
        d.errors.push(TextError { line: 1, col: 1, message: "x".into() });
        let p = content.behaviors.get("skirmisher").unwrap().program().unwrap();
        d.set_program(p.clone());
        assert_eq!(d.program, p);
        assert_eq!(d.text, p.to_text());
        assert!(d.errors.is_empty());
        assert_eq!(d.revision, 1);
    }
}
