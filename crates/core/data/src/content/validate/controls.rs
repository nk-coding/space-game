//! Controls (`controls.ron`, GDR-0090): each one of the game's actions with readable inputs; every action has one
//! once the content has any; no two actions sharing a screen share an input.

use super::Validator;
use crate::content::model::Control;
use crate::content::problem::Problem;
use crate::input::{Action, Chord, MAX_INPUTS, clashes};

impl<'a> Validator<'a> {
    pub(super) fn control(&mut self, e: &'a Control) {
        if Action::from_id(&e.id).is_none() {
            self.err(e, "id", format!("`{}` is not an action the game has (`data::input::Action`)", e.id));
        }
        if e.contexts.is_empty() {
            self.err(e, "contexts", "a control acts somewhere: name at least one context");
        }
        if e.inputs.len() > MAX_INPUTS {
            self.err(e, "inputs", format!("{} inputs: at most {MAX_INPUTS}", e.inputs.len()));
        }
        let mut seen: Vec<Chord> = Vec::new();
        for (k, s) in e.inputs.iter().enumerate() {
            match s.parse::<Chord>() {
                Ok(c) if seen.contains(&c) => self.err(e, format!("inputs[{k}]"), format!("`{s}` is listed twice")),
                Ok(c) => seen.push(c),
                Err(why) => self.err(e, format!("inputs[{k}]"), why),
            }
        }
    }

    /// Every action has a control (once there are any: a pack of other content needs none), and no input is taken
    /// twice on one screen.
    pub(super) fn controls(&mut self) {
        let c = self.c;
        if !c.controls.is_empty() {
            for a in Action::ALL {
                if !c.controls.contains(a.id()) {
                    self.out.push(Problem::error(format!("the action `{}` has no control (`controls.ron`): it could not be bound", a.id())).field("controls"));
                }
            }
        }
        for (a, b, chord) in clashes(c.controls.as_slice()) {
            if let Some(e) = c.controls.get(&b) {
                self.err(e, "inputs", format!("`{chord}` is also `{a}`'s, on a screen both act on"));
            }
        }
    }
}
