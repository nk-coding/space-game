//! What the graph view shows, as plain data: the editing model's program becomes the target chip and one row per
//! rule (violet condition chips → a teal action chip), each chip with its content name, its values as written in
//! the text and the comparison of a value condition; rules without a rule slot are inactive. Chips and rows carry
//! the model's stable ids ([`NodeRef`], [`RuleId`]), so a selection keeps naming the same node while rules move.
//! The screen (`screen.rs`) turns this into nodes; the tests below pin the mapping.

use data::behavior::{Call, Cond};
use data::behavior::edit::{BehaviorEditor, DEFAULT_TARGET, NodeRef, RuleId};
use data::{BehaviorNode, NodeRole, Table};

/// One chip of the graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    pub node: NodeRef,
    pub role: NodeRole,
    /// `not` in front of a condition.
    pub not: bool,
    /// The node's content name (its id when the vocabulary does not know it).
    pub name: String,
    /// The values in brackets, as written in the text (`120%`, `80% · ccw`), shown dim after the name.
    pub args: String,
    /// The comparison of a value condition, as written in the text (`< 25%`, `>= 3`).
    pub compare: String,
    /// A condition written in Rhai: dashed outline and a gold RHAI tag.
    pub rhai: bool,
}

/// One rule: its conditions (none for `always`), its action, its note, whether it has a rule slot and whether it
/// can ever run (not below an `always` rule).
#[derive(Clone, Debug, PartialEq)]
pub struct RuleRow {
    pub id: RuleId,
    pub conds: Vec<Chip>,
    pub action: Chip,
    pub note: Option<String>,
    pub active: bool,
    pub reached: bool,
}

/// The whole graph.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphView {
    pub target: Chip,
    /// The program has no target line: the chip shows the default (`nearest`).
    pub default_target: bool,
    pub rules: Vec<RuleRow>,
    /// The design's rule slots.
    pub slots: u32,
}

impl GraphView {
    pub fn of(ed: &BehaviorEditor, slots: u32) -> Self {
        let nodes = ed.nodes();
        let program = ed.program();
        let (target, default_target) = match &program.target {
            Some(t) => (chip(&t.call, NodeRef::Target, NodeRole::Target, nodes), false),
            None => {
                let call = Call { name: DEFAULT_TARGET.into(), args: Vec::new(), col: Default::default() };
                (chip(&call, NodeRef::Target, NodeRole::Target, nodes), true)
            }
        };
        let active = ed.active_rules(slots);
        let always: Vec<bool> = program.rules.iter().map(|r| r.when.is_empty()).collect();
        let never = never_reached(&always, slots);
        let rules = ed
            .rules()
            .map(|r| RuleRow {
                id: r.id,
                conds: r.rule.when.iter().zip(r.conds).map(|(c, id)| cond_chip(c, NodeRef::Cond(*id), nodes)).collect(),
                action: chip(&r.rule.action, NodeRef::Action(r.id), NodeRole::Action, nodes),
                note: r.rule.note.clone(),
                active: r.index < active,
                reached: !never.contains(&r.index),
            })
            .collect();
        Self { target, default_target, rules, slots }
    }

    /// Where the slot line goes: the index of the first rule without a slot (`None`: every rule has one).
    pub fn slot_line(&self) -> Option<usize> {
        let slots = self.slots as usize;
        (self.rules.len() > slots).then_some(slots)
    }

    /// Where the never-reached line goes: the index of the first rule with a slot below an `always` rule.
    pub fn never_line(&self) -> Option<usize> {
        self.rules.iter().position(|r| !r.reached)
    }
}

/// The rules that have a slot but never run, by index: those below the first `always` rule (`always[i]`: rule `i`
/// has no conditions). It matches every time, and the first rule that matches decides.
pub fn never_reached(always: &[bool], slots: u32) -> std::ops::Range<usize> {
    let with_slot = always.len().min(slots as usize);
    match always[..with_slot].iter().position(|a| *a) {
        Some(i) => i + 1..with_slot,
        None => with_slot..with_slot,
    }
}

fn chip(call: &Call, node: NodeRef, role: NodeRole, nodes: &Table<BehaviorNode>) -> Chip {
    let known = nodes.get(&call.name);
    let args: Vec<String> = call.args.iter().map(|a| a.to_string()).collect();
    Chip {
        node,
        role,
        not: false,
        name: known.map_or_else(|| call.name.clone(), |n| n.name.clone()),
        args: args.join(" · "),
        compare: String::new(),
        rhai: known.is_some_and(|n| n.script.is_some()),
    }
}

fn cond_chip(c: &Cond, node: NodeRef, nodes: &Table<BehaviorNode>) -> Chip {
    let mut chip = chip(&c.call, node, NodeRole::Condition, nodes);
    chip.not = c.not;
    if let Some(cmp) = &c.cmp {
        chip.compare = format!("{} {}", cmp.op.symbol(), cmp.rhs);
    }
    chip
}

/// The header pill: `6 / 12 RULE SLOTS` (rules / slots).
pub fn slots_pill(rules: usize, slots: u32) -> String {
    format!("{rules} / {slots} RULE SLOT{}", if slots == 1 { "" } else { "S" })
}

/// The pill under the rules: `+ RULE · 6 SLOTS LEFT`.
pub fn add_rule_label(rules: usize, slots: u32) -> String {
    match u64::from(slots).saturating_sub(rules as u64) {
        0 => "+ RULE  ·  NO SLOT LEFT".into(),
        1 => "+ RULE  ·  1 SLOT LEFT".into(),
        n => format!("+ RULE  ·  {n} SLOTS LEFT"),
    }
}

/// The label on the slot line, above the first rule without a slot.
pub fn slot_line_label(slots: u32) -> String {
    match slots {
        0 => "NO LOGIC CORE  ·  NO RULE RUNS".into(),
        1 => "1 RULE SLOT  ·  THE RULES BELOW DO NOT RUN".into(),
        n => format!("{n} RULE SLOTS  ·  THE RULES BELOW DO NOT RUN"),
    }
}

/// The label on the never-reached line, under an `always` rule with rules below it.
pub const NEVER_LINE_LABEL: &str = "ALWAYS MATCHES  ·  THE RULES BELOW ARE NEVER REACHED";

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(content: &data::Content, text: &str) -> BehaviorEditor {
        BehaviorEditor::from_text(&content.behavior_nodes, text).unwrap()
    }

    fn skirmisher(content: &data::Content) -> BehaviorEditor {
        BehaviorEditor::open(&content.behavior_nodes, &content.behaviors.get("skirmisher").unwrap().program().unwrap()).unwrap()
    }

    #[test]
    fn rules_become_condition_chips_and_an_action() {
        let content = data::base_content();
        let ed = skirmisher(&content);
        let g = GraphView::of(&ed, 12);
        assert_eq!(g.target.name, "Nearest");
        assert!(!g.default_target);
        assert_eq!(g.rules.len(), 4);
        assert!(g.rules.iter().all(|r| r.active));
        assert_eq!(g.slot_line(), None);

        // `when hull < 25% -> flee # too hurt to fight on`
        let r0 = &g.rules[0];
        let v0 = ed.rule_at(0).unwrap();
        assert_eq!(r0.id, v0.id);
        assert_eq!(r0.conds.len(), 1);
        let hull = &r0.conds[0];
        assert_eq!((hull.name.as_str(), hull.args.as_str(), hull.compare.as_str()), ("Hull", "", "< 25%"));
        assert_eq!(hull.node, NodeRef::Cond(v0.conds[0]));
        assert_eq!(hull.role, NodeRole::Condition);
        assert_eq!(r0.action.name, "Flee");
        assert_eq!(r0.action.node, NodeRef::Action(v0.id));
        assert_eq!(r0.note.as_deref(), Some("too hurt to fight on"));

        // `when shield_down and target_on_me -> orbit(80%, ccw)`: two flags, the action's values dim.
        let r1 = &g.rules[1];
        let names: Vec<&str> = r1.conds.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Shield down", "Target on me"]);
        assert!(r1.conds.iter().all(|c| c.compare.is_empty() && c.args.is_empty()));
        assert_eq!((r1.action.name.as_str(), r1.action.args.as_str()), ("Orbit", "80% · ccw"));

        // `when closing_in(2s) and not target_in_range(100%) -> face`: the Rhai condition and a `not`.
        let r2 = &g.rules[2];
        assert!(r2.conds[0].rhai && !r2.conds[0].not);
        assert_eq!((r2.conds[0].name.as_str(), r2.conds[0].args.as_str()), ("Closing in", "2s"));
        assert!(r2.conds[1].not && !r2.conds[1].rhai);
        assert_eq!((r2.conds[1].name.as_str(), r2.conds[1].args.as_str()), ("Target in range", "100%"));
        assert_eq!(r2.action.name, "Face target");

        // `always -> approach(67%)`: no conditions.
        assert!(g.rules[3].conds.is_empty());
        assert_eq!((g.rules[3].action.name.as_str(), g.rules[3].action.args.as_str()), ("Approach", "67%"));

        // A value with arguments and a comparison: `enemies_near(100m) >= 3`.
        let crowd = GraphView::of(&editor(&content, "when enemies_near(100m) >= 3 -> orbit(90%)"), 12);
        let near = &crowd.rules[0].conds[0];
        assert_eq!((near.name.as_str(), near.args.as_str(), near.compare.as_str()), ("Enemies near", "100m", ">= 3"));
    }

    #[test]
    fn ids_follow_the_rules_when_they_move() {
        let content = data::base_content();
        let mut ed = skirmisher(&content);
        let before = GraphView::of(&ed, 12);
        let moved = before.rules[2].id;
        ed.move_rule(moved, 0).unwrap();
        let after = GraphView::of(&ed, 12);
        // The same rule and its chips, now on top; the others shifted down with their ids.
        assert_eq!(after.rules[0].id, moved);
        assert_eq!(after.rules[0].conds[0].node, before.rules[2].conds[0].node);
        assert_eq!(after.rules[1].id, before.rules[0].id);
        assert_eq!(after.rules[3].action.node, before.rules[3].action.node);
    }

    #[test]
    fn rules_below_the_slots_are_inactive() {
        let content = data::base_content();
        let ed = skirmisher(&content);
        let g = GraphView::of(&ed, 2);
        let active: Vec<bool> = g.rules.iter().map(|r| r.active).collect();
        assert_eq!(active, [true, true, false, false]);
        assert_eq!(g.slot_line(), Some(2));
        // No core: nothing runs, the line is above the first rule.
        let none = GraphView::of(&ed, 0);
        assert!(none.rules.iter().all(|r| !r.active));
        assert_eq!(none.slot_line(), Some(0));
        // Exactly full: no line.
        assert_eq!(GraphView::of(&ed, 4).slot_line(), None);
    }

    #[test]
    fn rules_below_an_always_rule_are_never_reached() {
        let content = data::base_content();
        // The skirmisher ends on its `always` fallback: every rule can run.
        let g = GraphView::of(&skirmisher(&content), 12);
        assert!(g.rules.iter().all(|r| r.reached));
        assert_eq!(g.never_line(), None);
        // `always -> approach` above two rules (what + RULE or a drag makes): those two never run.
        let ed = editor(&content, "when hull < 30% -> flee\nalways -> approach\nwhen shield_down -> face\nalways -> hold");
        let g = GraphView::of(&ed, 6);
        assert_eq!(g.rules.iter().map(|r| r.reached).collect::<Vec<_>>(), [true, true, false, false]);
        assert_eq!(g.never_line(), Some(2));
        // Rules without a slot are the slot line's: only those with a slot are marked.
        let g = GraphView::of(&ed, 3);
        assert_eq!(g.rules.iter().map(|r| (r.active, r.reached)).collect::<Vec<_>>(), [(true, true), (true, true), (true, false), (false, true)]);
        assert_eq!(never_reached(&[false, true, false], 1), 1..1);
        assert_eq!(never_reached(&[true, false, false], 0), 0..0);
        assert_eq!(never_reached(&[true, false, false], 12), 1..3);
    }

    #[test]
    fn a_missing_target_line_shows_the_default() {
        let content = data::base_content();
        let g = GraphView::of(&editor(&content, "when hull < 30% -> flee"), 6);
        assert!(g.default_target);
        assert_eq!(g.target.name, "Nearest");
        assert_eq!(g.target.node, NodeRef::Target);
        let g = GraphView::of(&editor(&content, "target weakest(engines)\nalways -> hold"), 6);
        assert_eq!((g.target.name.as_str(), g.target.args.as_str()), ("Weakest", "engines"));
        assert!(!g.default_target);
    }

    #[test]
    fn labels() {
        assert_eq!(slots_pill(6, 12), "6 / 12 RULE SLOTS");
        assert_eq!(slots_pill(1, 1), "1 / 1 RULE SLOT");
        assert_eq!(add_rule_label(6, 12), "+ RULE  ·  6 SLOTS LEFT");
        assert_eq!(add_rule_label(5, 6), "+ RULE  ·  1 SLOT LEFT");
        assert_eq!(add_rule_label(6, 6), "+ RULE  ·  NO SLOT LEFT");
        assert_eq!(add_rule_label(8, 6), "+ RULE  ·  NO SLOT LEFT");
        assert_eq!(slot_line_label(6), "6 RULE SLOTS  ·  THE RULES BELOW DO NOT RUN");
        assert_eq!(slot_line_label(0), "NO LOGIC CORE  ·  NO RULE RUNS");
    }
}
