//! What the graph view shows, as plain data: a program becomes the target chip and one row per rule (violet
//! condition chips → a teal action chip), each chip with its content name, its values as written in the text
//! and the comparison of a value condition; rules without a rule slot are inactive. The screen
//! (`screen.rs`) turns this into nodes; the tests below pin the mapping.

use data::behavior::{Call, Cond, Program};
use data::{BehaviorNode, NodeRole, Table};

/// A node of the program: the target selector, a condition of a rule or a rule's action. Rule and condition
/// indices are those of [`Program::rules`] and [`data::behavior::Rule::when`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeRef {
    Target,
    Cond { rule: usize, cond: usize },
    Action { rule: usize },
}

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

/// One rule: its conditions (none for `always`), its action, its note and whether it has a rule slot.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleRow {
    pub conds: Vec<Chip>,
    pub action: Chip,
    pub note: Option<String>,
    pub active: bool,
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

/// The selector a program without a `target` line uses (`docs/behavior-format.md`).
pub const DEFAULT_TARGET: &str = "nearest";

impl GraphView {
    pub fn of(program: &Program, nodes: &Table<BehaviorNode>, slots: u32) -> Self {
        let (target, default_target) = match &program.target {
            Some(t) => (chip(&t.call, NodeRef::Target, NodeRole::Target, nodes), false),
            None => {
                let call = Call { name: DEFAULT_TARGET.into(), args: Vec::new(), col: Default::default() };
                (chip(&call, NodeRef::Target, NodeRole::Target, nodes), true)
            }
        };
        let rules = program
            .rules
            .iter()
            .enumerate()
            .map(|(rule, r)| RuleRow {
                conds: r.when.iter().enumerate().map(|(cond, c)| cond_chip(c, NodeRef::Cond { rule, cond }, nodes)).collect(),
                action: chip(&r.action, NodeRef::Action { rule }, NodeRole::Action, nodes),
                note: r.note.clone(),
                active: (rule as u64) < u64::from(slots),
            })
            .collect();
        Self { target, default_target, rules, slots }
    }

    /// Where the slot line goes: the index of the first rule without a slot (`None`: every rule has one).
    pub fn slot_line(&self) -> Option<usize> {
        let slots = self.slots as usize;
        (self.rules.len() > slots).then_some(slots)
    }

    /// The chip of a node, if the program has it.
    pub fn chip(&self, node: NodeRef) -> Option<&Chip> {
        match node {
            NodeRef::Target => Some(&self.target),
            NodeRef::Cond { rule, cond } => self.rules.get(rule)?.conds.get(cond),
            NodeRef::Action { rule } => self.rules.get(rule).map(|r| &r.action),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn skirmisher(content: &data::Content) -> Program {
        content.behaviors.get("skirmisher").unwrap().program().unwrap()
    }

    #[test]
    fn rules_become_condition_chips_and_an_action() {
        let content = data::base_content();
        let g = GraphView::of(&skirmisher(&content), &content.behavior_nodes, 12);
        assert_eq!(g.target.name, "Nearest");
        assert!(!g.default_target);
        assert_eq!(g.rules.len(), 6);
        assert!(g.rules.iter().all(|r| r.active));
        assert_eq!(g.slot_line(), None);

        // `when hull < 25% -> flee # too hurt to fight on`
        let r0 = &g.rules[0];
        assert_eq!(r0.conds.len(), 1);
        let hull = &r0.conds[0];
        assert_eq!((hull.name.as_str(), hull.args.as_str(), hull.compare.as_str()), ("Hull", "", "< 25%"));
        assert_eq!(hull.node, NodeRef::Cond { rule: 0, cond: 0 });
        assert_eq!(hull.role, NodeRole::Condition);
        assert_eq!(r0.action.name, "Flee");
        assert_eq!(r0.action.node, NodeRef::Action { rule: 0 });
        assert_eq!(r0.note.as_deref(), Some("too hurt to fight on"));

        // `when shield_down and target_on_me -> orbit(80%, ccw)`: two flags, the action's values dim.
        let r1 = &g.rules[1];
        let names: Vec<&str> = r1.conds.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Shield down", "Target on me"]);
        assert!(r1.conds.iter().all(|c| c.compare.is_empty() && c.args.is_empty()));
        assert_eq!((r1.action.name.as_str(), r1.action.args.as_str()), ("Orbit", "80% · ccw"));

        // A value with arguments and a comparison: `enemies_near(100m) >= 3`.
        let near = &g.rules[2].conds[0];
        assert_eq!((near.name.as_str(), near.args.as_str(), near.compare.as_str()), ("Enemies near", "100m", ">= 3"));

        // `when closing_in(2s) and not target_in_range(100%) -> face`: the Rhai condition and a `not`.
        let r4 = &g.rules[4];
        assert!(r4.conds[0].rhai && !r4.conds[0].not);
        assert_eq!((r4.conds[0].name.as_str(), r4.conds[0].args.as_str()), ("Closing in", "2s"));
        assert!(r4.conds[1].not && !r4.conds[1].rhai);
        assert_eq!((r4.conds[1].name.as_str(), r4.conds[1].args.as_str()), ("Target in range", "100%"));
        assert_eq!(r4.action.name, "Face target");

        // `always -> approach(67%)`: no conditions.
        assert!(g.rules[5].conds.is_empty());
        assert_eq!((g.rules[5].action.name.as_str(), g.rules[5].action.args.as_str()), ("Approach", "67%"));
        assert_eq!(g.chip(NodeRef::Action { rule: 5 }).unwrap().name, "Approach");
        assert!(g.chip(NodeRef::Cond { rule: 5, cond: 0 }).is_none());
    }

    #[test]
    fn rules_below_the_slots_are_inactive() {
        let content = data::base_content();
        let g = GraphView::of(&skirmisher(&content), &content.behavior_nodes, 2);
        let active: Vec<bool> = g.rules.iter().map(|r| r.active).collect();
        assert_eq!(active, [true, true, false, false, false, false]);
        assert_eq!(g.slot_line(), Some(2));
        // No core: nothing runs, the line is above the first rule.
        let none = GraphView::of(&skirmisher(&content), &content.behavior_nodes, 0);
        assert!(none.rules.iter().all(|r| !r.active));
        assert_eq!(none.slot_line(), Some(0));
        // Exactly full: no line.
        assert_eq!(GraphView::of(&skirmisher(&content), &content.behavior_nodes, 6).slot_line(), None);
    }

    #[test]
    fn a_missing_target_line_shows_the_default() {
        let content = data::base_content();
        let p = Program::parse("when hull < 30% -> flee").unwrap();
        let g = GraphView::of(&p, &content.behavior_nodes, 6);
        assert!(g.default_target);
        assert_eq!(g.target.name, "Nearest");
        let p = Program::parse("target weakest(engines)\nalways -> hold").unwrap();
        let g = GraphView::of(&p, &content.behavior_nodes, 6);
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
