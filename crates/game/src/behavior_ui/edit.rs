//! Editing in the graph view, the inspector and the palette. Every change is an [`Edit`] carried out by the
//! editing model (`data::behavior_edit::BehaviorEditor`, through [`BehaviorDraft::edit`]): each edit is legal or
//! refused with a reason, each is one undo step, and the text view follows the program.
//!
//! * [`apply_edit`] runs an [`Edit`] and says what the selection becomes ([`After`]).
//! * [`palette_click`]: what a click on an ADD NODE entry does with the selection: a condition is added to the
//!   selected rule (or replaces the selected condition while REPLACE is armed), an action replaces the rule's
//!   action, a target selector sets the target line; with nothing selected the inspector shows the node.
//! * [`add_rule_edit`]: `+ RULE` adds `always ->` the palette's first action above a trailing `always` rule
//!   (the usual fallback stays last), else at the bottom; [`delete_edit`]: Delete removes the selected
//!   condition, deletes the selected rule, or clears the target line.
//! * [`apply`] (the system) handles the editing actions; refusals go to [`EditState::note`] (a coral note after
//!   the status line). [`sync_rings`] dims undo / redo when there is nothing to undo or redo.

use bevy::prelude::*;
use data::NodeRole;
use data::behavior::{CmpOp, Value};
use data::behavior_edit::{BehaviorEditor, CondId, NodeRef, Refusal, RuleId};

use crate::style::Style;
use crate::ui_icons::UiIcon;
use crate::ui_kit::Ring;

use super::actions::{BehaviorAction, BehaviorButton};
use super::draft::BehaviorDraft;
use super::inspect::Selection;
use super::BehaviorUi;

/// How long a refusal stays after the status line (real seconds), as in the drydock.
pub const NOTE_SECS: f32 = 3.0;

/// One change of the program, carried out by the editing model.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    SetParam { at: NodeRef, index: usize, value: Value },
    ResetParam { at: NodeRef, index: usize },
    /// A value condition's comparison: the comparator and the threshold.
    SetCompare { cond: CondId, op: CmpOp, rhs: Value },
    ToggleNot(CondId),
    AddCondition { rule: RuleId, node: String },
    RemoveCondition(CondId),
    /// Another node of the same role in its place.
    ReplaceNode { at: NodeRef, node: String },
    /// `always -> action` at a position (0 = top).
    AddRule { index: usize, action: String },
    MoveRule { rule: RuleId, to: usize },
    DuplicateRule(RuleId),
    DeleteRule(RuleId),
    /// The rule's `# note` (empty clears it).
    SetRuleNote { rule: RuleId, note: String },
    SetTargetNote(String),
    /// No `target` line: the default target (`nearest`).
    ClearTarget,
}

/// What the selection becomes after an edit.
#[derive(Clone, Debug, PartialEq)]
pub enum After {
    Keep,
    Select(Selection),
    Deselect,
}

/// Carries out an edit. Refused edits change nothing.
pub fn apply_edit(ed: &mut BehaviorEditor, edit: &Edit) -> Result<After, Refusal> {
    let keep = |r: Result<bool, Refusal>| r.map(|_| After::Keep);
    match edit {
        Edit::SetParam { at, index, value } => keep(ed.set_param(*at, *index, value.clone())),
        Edit::ResetParam { at, index } => keep(ed.reset_param(*at, *index)),
        Edit::SetCompare { cond, op, rhs } => keep(ed.set_compare(*cond, *op, rhs.clone())),
        Edit::ToggleNot(cond) => keep(ed.toggle_not(*cond)),
        Edit::AddCondition { rule, node } => {
            ed.add_condition(*rule, node).map(|c| After::Select(Selection::Node(NodeRef::Cond(c))))
        }
        Edit::RemoveCondition(cond) => {
            let rule = ed.rule_of(*cond);
            ed.remove_condition(*cond)?;
            Ok(rule.map_or(After::Deselect, |r| After::Select(Selection::Rule(r))))
        }
        Edit::ReplaceNode { at, node } => ed.replace_node(*at, node).map(|_| After::Select(Selection::Node(*at))),
        Edit::AddRule { index, action } => ed.add_rule(*index, action).map(|r| After::Select(Selection::Rule(r))),
        Edit::MoveRule { rule, to } => keep(ed.move_rule(*rule, *to)),
        Edit::DuplicateRule(rule) => ed.duplicate_rule(*rule).map(|r| After::Select(Selection::Rule(r))),
        Edit::DeleteRule(rule) => ed.delete_rule(*rule).map(|_| After::Deselect),
        Edit::SetRuleNote { rule, note } => keep(ed.set_rule_note(*rule, Some(note))),
        Edit::SetTargetNote(note) => keep(ed.set_target_note(Some(note))),
        Edit::ClearTarget => {
            ed.clear_target();
            Ok(After::Keep)
        }
    }
}

/// What a click on a palette entry does.
#[derive(Clone, Debug, PartialEq)]
pub enum PaletteClick {
    Edit(Edit),
    /// The inspector shows the node (nothing to add it to).
    Show,
}

/// A palette click with this selection: a condition goes into the selected rule (or one of its nodes'), or
/// replaces `replacing` (REPLACE armed on a condition); an action replaces the selected rule's action; a target
/// selector sets the target line while anything of the program is selected. Else the inspector shows the node.
pub fn palette_click(ed: &BehaviorEditor, sel: Option<&Selection>, replacing: Option<NodeRef>, id: &str) -> PaletteClick {
    let Some(n) = ed.nodes().get(id) else { return PaletteClick::Show };
    let rule = sel.and_then(|s| s.rule(ed));
    let node = n.id.clone();
    match n.role {
        NodeRole::Condition => match (replacing, rule) {
            (Some(at @ NodeRef::Cond(_)), _) => PaletteClick::Edit(Edit::ReplaceNode { at, node }),
            (_, Some(rule)) => PaletteClick::Edit(Edit::AddCondition { rule, node }),
            _ => PaletteClick::Show,
        },
        NodeRole::Action => match rule {
            Some(rule) => PaletteClick::Edit(Edit::ReplaceNode { at: NodeRef::Action(rule), node }),
            None => PaletteClick::Show,
        },
        NodeRole::Target => match sel {
            Some(Selection::Node(_) | Selection::Rule(_)) => PaletteClick::Edit(Edit::ReplaceNode { at: NodeRef::Target, node }),
            _ => PaletteClick::Show,
        },
    }
}

/// `+ RULE`: `always ->` the palette's first action, above the last rule when that one is a fallback (`always`,
/// so it stays last), else at the bottom. `None` if the vocabulary has no action.
pub fn add_rule_edit(ed: &BehaviorEditor) -> Option<Edit> {
    let action = ed.palette(NodeRole::Action).next()?.id.clone();
    let n = ed.rules().len();
    let fallback_last = ed.rule_at(n.checked_sub(1)?).is_some_and(|r| r.rule.when.is_empty());
    Some(Edit::AddRule { index: if fallback_last { n - 1 } else { n }, action })
}

/// Delete: the selected condition goes, a selected rule (or its action) is deleted, the target goes back to
/// the default.
pub fn delete_edit(ed: &BehaviorEditor, sel: Option<&Selection>) -> Option<Edit> {
    match sel? {
        Selection::Node(NodeRef::Cond(c)) => Some(Edit::RemoveCondition(*c)),
        Selection::Node(NodeRef::Target) => ed.program().target.is_some().then_some(Edit::ClearTarget),
        Selection::Node(NodeRef::Action(r)) | Selection::Rule(r) => Some(Edit::DeleteRule(*r)),
        Selection::Vocabulary(_) => None,
    }
}

/// The editing state outside the program.
#[derive(Resource, Default, Debug)]
pub struct EditState {
    /// REPLACE is armed on this condition: the next palette condition replaces it.
    pub replacing: Option<NodeRef>,
    /// The last refusal and when it happened (real seconds).
    pub note: Option<(String, f32)>,
}

impl EditState {
    /// The refusal while it is fresh.
    pub fn current_note(&self, now: f32) -> Option<&str> {
        self.note.as_ref().filter(|(_, at)| now - at < NOTE_SECS).map(|(t, _)| t.as_str())
    }
}

/// The editing actions: edits, palette clicks, `+ RULE`, Delete, undo and redo, REPLACE, rule selection.
pub(super) fn apply(
    mut actions: MessageReader<BehaviorAction>,
    mut draft: ResMut<BehaviorDraft>,
    mut ui: ResMut<BehaviorUi>,
    mut state: ResMut<EditState>,
    time: Res<Time<Real>>,
) {
    for a in actions.read() {
        let Some(ed) = draft.editor() else { return };
        let edit = match a {
            BehaviorAction::Edit(e) => Some(e.clone()),
            BehaviorAction::AddRule => add_rule_edit(ed),
            BehaviorAction::AddNode(id) => match palette_click(ed, ui.selected.as_ref(), state.replacing, id) {
                PaletteClick::Edit(e) => Some(e),
                PaletteClick::Show => {
                    ui.selected = Some(Selection::Vocabulary(id.clone()));
                    None
                }
            },
            BehaviorAction::Delete => delete_edit(ed, ui.selected.as_ref()),
            BehaviorAction::SelectRule(r) => {
                ui.selected = Some(Selection::Rule(*r));
                None
            }
            // Arms REPLACE on the selected condition, or disarms it (also Esc).
            BehaviorAction::Replace => {
                state.replacing = match (state.replacing, &ui.selected) {
                    (None, Some(Selection::Node(at @ NodeRef::Cond(_)))) => Some(*at),
                    _ => None,
                };
                None
            }
            BehaviorAction::Undo | BehaviorAction::Redo => {
                let undo = *a == BehaviorAction::Undo;
                draft.edit(|e| if undo { e.undo() } else { e.redo() });
                None
            }
            _ => None,
        };
        let Some(edit) = edit else { continue };
        match draft.edit(|ed| apply_edit(ed, &edit)) {
            Some(Ok(After::Keep)) | None => {}
            Some(Ok(After::Select(s))) => ui.selected = Some(s),
            Some(Ok(After::Deselect)) => ui.selected = None,
            Some(Err(why)) => state.note = Some((why.to_string(), time.elapsed_secs())),
        }
        state.replacing = None;
    }
    // A selection that no longer exists (after an undo or a text edit) goes; REPLACE follows the selection.
    if let Some(ed) = draft.editor()
        && let Some(sel) = ui.selected.as_ref()
        && !sel.exists(ed)
    {
        ui.selected = None;
    }
    if let Some(at) = state.replacing
        && ui.selected != Some(Selection::Node(at))
    {
        state.replacing = None;
    }
}

/// Undo and redo look disabled (dim ring and arrow) when there is nothing to undo or redo.
pub(super) fn sync_rings(
    draft: Res<BehaviorDraft>,
    style: Res<Style>,
    mut rings: Query<(&BehaviorButton, &mut Ring, &Children)>,
    mut icons: Query<&mut UiIcon>,
) {
    if !draft.is_changed() {
        return;
    }
    let ed = draft.editor();
    for (b, mut ring, children) in &mut rings {
        let disabled = match b.0 {
            BehaviorAction::Undo => !ed.is_some_and(|e| e.can_undo()),
            BehaviorAction::Redo => !ed.is_some_and(|e| e.can_redo()),
            _ => continue,
        };
        if ring.disabled != disabled {
            ring.disabled = disabled;
        }
        let want = if disabled { style.ui.dim } else { style.ui.number };
        for child in children {
            if let Ok(mut icon) = icons.get_mut(*child)
                && let UiIcon::Glyph { color, .. } = &mut *icon
                && *color != want
            {
                *color = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::behavior::Unit;

    fn editor(text: &str) -> BehaviorEditor {
        let content = data::base_content();
        BehaviorEditor::from_text(&content.behavior_nodes, text).unwrap()
    }

    fn skirmisher() -> BehaviorEditor {
        let content = data::base_content();
        BehaviorEditor::open(&content.behavior_nodes, &content.behaviors.get("skirmisher").unwrap().program().unwrap()).unwrap()
    }

    #[test]
    fn palette_clicks_follow_the_selection() {
        let ed = skirmisher();
        let r1 = ed.rule_at(1).unwrap();
        let (rule, cond) = (r1.id, r1.conds[0]);
        let show = PaletteClick::Show;
        // Nothing selected, or a palette node shown: the inspector shows the node.
        assert_eq!(palette_click(&ed, None, None, "heat"), show);
        assert_eq!(palette_click(&ed, Some(&Selection::Vocabulary("kite".into())), None, "heat"), show);
        assert_eq!(palette_click(&ed, None, None, "weakest"), show);
        // A rule, or any of its nodes: a condition is added to it, an action replaces its action.
        for sel in [Selection::Rule(rule), Selection::Node(NodeRef::Action(rule)), Selection::Node(NodeRef::Cond(cond))] {
            assert_eq!(palette_click(&ed, Some(&sel), None, "heat"), PaletteClick::Edit(Edit::AddCondition { rule, node: "heat".into() }));
            assert_eq!(
                palette_click(&ed, Some(&sel), None, "kite"),
                PaletteClick::Edit(Edit::ReplaceNode { at: NodeRef::Action(rule), node: "kite".into() })
            );
            // A target selector sets the target line.
            assert_eq!(
                palette_click(&ed, Some(&sel), None, "weakest"),
                PaletteClick::Edit(Edit::ReplaceNode { at: NodeRef::Target, node: "weakest".into() })
            );
        }
        // REPLACE armed on a condition: a palette condition replaces it.
        let at = NodeRef::Cond(cond);
        assert_eq!(palette_click(&ed, Some(&Selection::Node(at)), Some(at), "heat"), PaletteClick::Edit(Edit::ReplaceNode { at, node: "heat".into() }));
        // The target selected: selectors replace it; conditions and actions have no rule to go to.
        let t = Selection::Node(NodeRef::Target);
        assert_eq!(palette_click(&ed, Some(&t), None, "threat"), PaletteClick::Edit(Edit::ReplaceNode { at: NodeRef::Target, node: "threat".into() }));
        assert_eq!(palette_click(&ed, Some(&t), None, "heat"), show);
        assert_eq!(palette_click(&ed, Some(&t), None, "flee"), show);
        assert_eq!(palette_click(&ed, Some(&t), None, "warp"), show);
    }

    #[test]
    fn edits_run_through_the_model_and_move_the_selection() {
        let mut ed = skirmisher();
        let rule = ed.rule_at(1).unwrap().id;
        // Adding a condition selects it; the text has it.
        let After::Select(Selection::Node(NodeRef::Cond(c))) = apply_edit(&mut ed, &Edit::AddCondition { rule, node: "heat".into() }).unwrap() else { panic!() };
        assert!(ed.text().contains("when shield_down and target_on_me and heat < 30% -> orbit(80%, ccw)"), "{}", ed.text());
        // Threshold, comparator and `not`.
        apply_edit(&mut ed, &Edit::SetCompare { cond: c, op: CmpOp::Ge, rhs: Value::Number(85.0, Some(Unit::Percent)) }).unwrap();
        apply_edit(&mut ed, &Edit::ToggleNot(c)).unwrap();
        assert!(ed.text().contains("and not heat >= 85% ->"), "{}", ed.text());
        // A refusal changes nothing and says why.
        let r = apply_edit(&mut ed, &Edit::SetCompare { cond: c, op: CmpOp::Ge, rhs: Value::Number(85.0, None) }).unwrap_err();
        assert!(r.to_string().contains('%'), "{r}");
        // Removing it selects its rule; deleting the rule deselects; undo brings it back.
        assert_eq!(apply_edit(&mut ed, &Edit::RemoveCondition(c)).unwrap(), After::Select(Selection::Rule(rule)));
        assert_eq!(apply_edit(&mut ed, &Edit::DeleteRule(rule)).unwrap(), After::Deselect);
        assert!(ed.rule(rule).is_none());
        assert!(ed.undo());
        assert_eq!(ed.rule(rule).unwrap().index, 1);
        // The last rule cannot go.
        let mut one = editor("always -> hold");
        let only = one.rule_at(0).unwrap().id;
        assert_eq!(apply_edit(&mut one, &Edit::DeleteRule(only)), Err(Refusal::LastRule));
    }

    #[test]
    fn a_new_rule_goes_above_the_fallback() {
        let mut ed = skirmisher();
        let add = add_rule_edit(&ed).unwrap();
        assert_eq!(add, Edit::AddRule { index: 5, action: "approach".into() });
        let After::Select(Selection::Rule(r)) = apply_edit(&mut ed, &add).unwrap() else { panic!() };
        assert_eq!(ed.rule(r).unwrap().index, 5);
        assert_eq!(ed.program().rules[6].to_string(), "always -> approach(67%)");
        // Without a fallback at the end: at the bottom.
        let ed = editor("when hull < 30% -> flee");
        assert_eq!(add_rule_edit(&ed), Some(Edit::AddRule { index: 1, action: "approach".into() }));
    }

    #[test]
    fn delete_follows_the_selection() {
        let ed = skirmisher();
        let r = ed.rule_at(0).unwrap();
        assert_eq!(delete_edit(&ed, Some(&Selection::Node(NodeRef::Cond(r.conds[0])))), Some(Edit::RemoveCondition(r.conds[0])));
        assert_eq!(delete_edit(&ed, Some(&Selection::Node(NodeRef::Action(r.id)))), Some(Edit::DeleteRule(r.id)));
        assert_eq!(delete_edit(&ed, Some(&Selection::Rule(r.id))), Some(Edit::DeleteRule(r.id)));
        assert_eq!(delete_edit(&ed, Some(&Selection::Node(NodeRef::Target))), Some(Edit::ClearTarget));
        assert_eq!(delete_edit(&editor("always -> hold"), Some(&Selection::Node(NodeRef::Target))), None);
        assert_eq!(delete_edit(&ed, None), None);
    }
}
