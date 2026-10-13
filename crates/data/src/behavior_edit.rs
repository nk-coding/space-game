//! The behavior editor's editing model: one behavior program (the target line and the rules, with their notes)
//! and the vocabulary it is written in, edited with small legal operations, with undo/redo and a revision
//! counter. Engine-free and wasm-safe: the counterpart of [`crate::edit`] for rules. The BEHAVIOR screen edits
//! through it in both views: the graph view and the inspector call the operations, the text view replaces the
//! program with [`BehaviorEditor::set_text`]. The rules are documented in `docs/behavior-format.md` ("Editing
//! model").
//!
//! The program is always legal: it passes [`check_program`] against the vocabulary, has at least one rule and
//! keeps the limits of the text format ([`MAX_RULES`] rules, [`MAX_CONDITIONS`] conditions per rule,
//! [`MAX_LINE_CHARS`] characters per line). An operation that would break that is refused with a [`Refusal`]
//! and changes nothing. The canonical text gives the program back: [`BehaviorEditor::from_text`] of
//! [`BehaviorEditor::text`] holds an equal program, so the graph and the text view always agree.
//!
//! Rules and conditions have stable ids ([`RuleId`], [`CondId`]): an id names the same rule or condition
//! across every edit, undo and redo until it is deleted, and undoing the delete brings it back under the same
//! id. Indices ([`RuleView::index`]) are priority positions and shift when rules are added, deleted or moved.
//!
//! ```no_run
//! use data::behavior::Value;
//! use data::behavior_edit::{BehaviorEditor, NodeRef};
//! let content = data::base_content();
//! let template = content.behaviors.get("skirmisher").unwrap().program().unwrap();
//! let mut ed = BehaviorEditor::open(&content.behavior_nodes, &template).unwrap();
//! let rule = ed.add_rule(0, "flee").unwrap(); // `always -> flee` on top
//! ed.add_condition(rule, "heat").unwrap(); // `when heat < 30% -> flee`
//! ed.set_param(NodeRef::Target, 0, Value::Word("engines".into())).unwrap(); // `target nearest(engines)`
//! ed.undo();
//! let slots = content.ship_rule_slots(content.ships.get("halvex_runner").unwrap());
//! let running = ed.active_rules(slots); // rules below this index are inactive
//! println!("{}", ed.text());
//! ```

use std::collections::VecDeque;
use std::fmt;

use crate::behavior::{
    Arg, Call, CmpOp, Col, Compare, Cond, MAX_CONDITIONS, MAX_LINE_CHARS, MAX_REACH_PERCENT,
    MAX_RULES, Program, Rule, TargetLine, TextError, Unit, Value, check_param_value, check_program,
    param_amount, parse_value, quantity_amount,
};
use crate::edit::UNDO_STEPS;
use crate::model::{BehaviorNode, NodeParam, NodeRole, ParamType, Quantity};
use crate::table::Table;

/// The target selector of a program without a `target` line (`docs/behavior-format.md`).
pub const DEFAULT_TARGET: &str = "nearest";

/// Names one rule for as long as it exists (also across undo and redo). Ids belong to the editor that handed
/// them out (and its clones).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(u32);

/// Names one condition of a rule for as long as it exists (also across undo and redo). Ids belong to the editor
/// that handed them out (and its clones).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CondId(u32);

/// A node of the program: the target selector, a rule's action or one of its conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeRef {
    /// The target selector; while the program has no `target` line, the default ([`DEFAULT_TARGET`]).
    Target,
    /// The action of a rule.
    Action(RuleId),
    /// A condition of a rule.
    Cond(CondId),
}

/// Why an edit is not possible. The edit changed nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No rule with this id (it was deleted).
    NoSuchRule(RuleId),
    /// No condition with this id (it was removed, or its rule deleted).
    NoSuchCondition(CondId),
    /// The vocabulary has no node with this id.
    UnknownNode(String),
    /// The node has another role than the place takes (an action where a condition goes, ...).
    WrongRole { node: String, expected: NodeRole },
    /// The node has no parameter with this index.
    NoSuchParam { node: String, index: usize },
    /// The value does not fit the parameter or the comparison (type, unit or range); the message says why.
    BadValue(String),
    /// A required parameter has no default to reset to.
    NoDefault { node: String, param: String },
    /// Only value conditions are compared; this one is a flag.
    NotAValue(String),
    /// A behavior has at most [`MAX_RULES`] rules.
    TooManyRules,
    /// A rule has at most [`MAX_CONDITIONS`] conditions.
    TooManyConditions,
    /// The line would be longer than [`MAX_LINE_CHARS`] characters.
    LineTooLong,
    /// A behavior keeps at least one rule.
    LastRule,
    /// No such position in the rule list.
    BadIndex(usize),
    /// A note is one line of text: no line breaks or other control characters.
    BadNote,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NoSuchRule(_) => f.write_str("that rule no longer exists"),
            Refusal::NoSuchCondition(_) => f.write_str("that condition no longer exists"),
            Refusal::UnknownNode(id) => write!(f, "unknown node `{id}`"),
            Refusal::WrongRole { node, expected } => {
                let noun = expected.noun();
                let article = if noun.starts_with('a') { "an" } else { "a" };
                write!(f, "`{node}` is not {article} {noun}")
            }
            Refusal::NoSuchParam { node, index } => {
                write!(f, "`{node}` has no value number {}", index + 1)
            }
            Refusal::BadValue(m) => f.write_str(m),
            Refusal::NoDefault { node, param } => {
                write!(f, "`{node}` needs a value for `{param}`: it has no default")
            }
            Refusal::NotAValue(node) => {
                write!(f, "`{node}` is true or false; it cannot be compared")
            }
            Refusal::TooManyRules => write!(f, "at most {MAX_RULES} rules"),
            Refusal::TooManyConditions => {
                write!(f, "at most {MAX_CONDITIONS} conditions per rule")
            }
            Refusal::LineTooLong => write!(f, "a line holds at most {MAX_LINE_CHARS} characters"),
            Refusal::LastRule => f.write_str("a behavior needs at least one rule"),
            Refusal::BadIndex(i) => write!(f, "there is no place {i} in the rule list"),
            Refusal::BadNote => f.write_str("a note is one line of text"),
        }
    }
}

/// A rule as the editor shows it.
#[derive(Clone, Copy, Debug)]
pub struct RuleView<'a> {
    pub id: RuleId,
    /// Priority position, 0 = top (the first rule whose conditions hold decides).
    pub index: usize,
    pub rule: &'a Rule,
    /// The ids of `rule.when`, in the same order.
    pub conds: &'a [CondId],
}

impl RuleView<'_> {
    /// The rule's nodes in reading order: its conditions, then its action.
    pub fn nodes(&self) -> impl Iterator<Item = NodeRef> + '_ {
        let action = NodeRef::Action(self.id);
        self.conds
            .iter()
            .map(|&c| NodeRef::Cond(c))
            .chain(std::iter::once(action))
    }
}

/// A node as the inspector shows it.
#[derive(Clone, Debug)]
pub struct NodeView<'a> {
    pub at: NodeRef,
    /// The vocabulary entry: role, display name, summary, tags, the quantity of a value condition, the Rhai
    /// script of a custom condition.
    pub node: &'a BehaviorNode,
    /// False only for the default target selector of a program without a `target` line.
    pub written: bool,
    /// Conditions: `not` in front.
    pub not: bool,
    /// Value conditions: the comparison (`< 30%`).
    pub compare: Option<&'a Compare>,
    /// Every parameter of the node, in order.
    pub params: Vec<ParamView<'a>>,
}

impl NodeView<'_> {
    /// A custom condition written in Rhai (tagged "Rhai" in the editor).
    pub fn is_rhai(&self) -> bool {
        self.node.script.is_some()
    }
}

/// A parameter of a node with the value in effect.
#[derive(Clone, Debug)]
pub struct ParamView<'a> {
    pub index: usize,
    /// The parameter: name, summary, type (with the options of a choice), default, `min`, `max`.
    pub param: &'a NodeParam,
    /// The value in effect: as written in the rule, else the default.
    pub value: Value,
    /// Written in the rule. False: the default applies (and follows the content if the default changes).
    pub written: bool,
    /// The value in effect equals the default (`45` and `45m` count as equal).
    pub is_default: bool,
}

impl ParamView<'_> {
    /// The allowed range of the number as written in the unit of [`ParamView::value`]: 0 to
    /// [`MAX_REACH_PERCENT`] for a share of the weapon reach (`%` on a distance), else the parameter's `min`
    /// and `max`. `(None, None)` for a choice.
    pub fn range(&self) -> (Option<f32>, Option<f32>) {
        match (&self.param.ty, &self.value) {
            (ParamType::Distance, Value::Number(_, Some(Unit::Percent))) => {
                (Some(0.0), Some(MAX_REACH_PERCENT))
            }
            (ParamType::Choice(_), _) => (None, None),
            _ => (self.param.min, self.param.max),
        }
    }
}

/// The ids of one rule and its conditions.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RuleIds {
    rule: RuleId,
    conds: Vec<CondId>,
}

/// A rule with its ids.
#[derive(Clone, Debug)]
struct Entry {
    ids: RuleIds,
    rule: Rule,
}

/// One primitive change; an undo step is a list of them in the order they were applied.
#[derive(Clone, Debug)]
enum Change {
    Insert {
        index: usize,
        entry: Entry,
    },
    Remove {
        index: usize,
        entry: Entry,
    },
    Set {
        index: usize,
        old: Entry,
        new: Entry,
    },
    /// The rule at `from` moves to `to` (the others close up).
    Move {
        from: usize,
        to: usize,
    },
    Target {
        old: Option<TargetLine>,
        new: Option<TargetLine>,
    },
}

impl Change {
    fn inverse(&self) -> Change {
        match self {
            Change::Insert { index, entry } => Change::Remove {
                index: *index,
                entry: entry.clone(),
            },
            Change::Remove { index, entry } => Change::Insert {
                index: *index,
                entry: entry.clone(),
            },
            Change::Set { index, old, new } => Change::Set {
                index: *index,
                old: new.clone(),
                new: old.clone(),
            },
            Change::Move { from, to } => Change::Move {
                from: *to,
                to: *from,
            },
            Change::Target { old, new } => Change::Target {
                old: new.clone(),
                new: old.clone(),
            },
        }
    }
}

/// Where a node sits: the target line, rule `i`'s action, or condition `j` of rule `i`.
#[derive(Clone, Copy)]
enum Place {
    Target,
    Action(usize),
    Cond(usize, usize),
}

impl Place {
    fn role(self) -> NodeRole {
        match self {
            Place::Target => NodeRole::Target,
            Place::Action(_) => NodeRole::Action,
            Place::Cond(..) => NodeRole::Condition,
        }
    }
}

/// The behavior editor: one program and the vocabulary (copied, so the editor owns its data).
#[derive(Clone, Debug)]
pub struct BehaviorEditor {
    nodes: Table<BehaviorNode>,
    program: Program,
    /// The ids of `program.rules`, same order.
    ids: Vec<RuleIds>,
    /// The next id to hand out; ids are never reused.
    next_id: u32,
    undo: VecDeque<Vec<Change>>,
    redo: Vec<Vec<Change>>,
    /// The open stroke's changes (one undo step), if a stroke is open.
    stroke: Option<Vec<Change>>,
    revision: u64,
}

impl BehaviorEditor {
    /// Opens a program (e.g. a content behavior's `program()`, as a template). It is read through its
    /// canonical text, so errors locate lines of [`Program::to_text`]; refused if it does not check against
    /// the vocabulary.
    pub fn open(
        nodes: &Table<BehaviorNode>,
        program: &Program,
    ) -> Result<BehaviorEditor, Vec<TextError>> {
        BehaviorEditor::from_text(nodes, &program.to_text())
    }

    /// Opens the text form of a program; refused with every syntax and vocabulary error (line and column).
    pub fn from_text(
        nodes: &Table<BehaviorNode>,
        text: &str,
    ) -> Result<BehaviorEditor, Vec<TextError>> {
        let program = crate::behavior::check_text(text, nodes)?;
        let mut ed = BehaviorEditor {
            nodes: nodes.clone(),
            program: Program::default(),
            ids: Vec::new(),
            next_id: 0,
            undo: VecDeque::new(),
            redo: Vec::new(),
            stroke: None,
            revision: 0,
        };
        let ids = program.rules.iter().map(|r| ed.fresh_ids(r)).collect();
        ed.ids = ids;
        ed.program = program;
        Ok(ed)
    }

    // ---- reading ----

    /// The program, always legal (see the module docs).
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The canonical text form ([`Program::to_text`]): what the text view shows.
    pub fn text(&self) -> String {
        self.program.to_text()
    }

    /// The vocabulary the program is written in.
    pub fn nodes(&self) -> &Table<BehaviorNode> {
        &self.nodes
    }

    /// Goes up by one with every change of the program (edits, undo, redo); the client caches views per
    /// revision. Starts at 0.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The rules in priority order.
    pub fn rules(&self) -> impl ExactSizeIterator<Item = RuleView<'_>> + '_ {
        (0..self.program.rules.len()).map(|i| self.view(i))
    }

    pub fn rule(&self, id: RuleId) -> Option<RuleView<'_>> {
        self.index_of(id).ok().map(|i| self.view(i))
    }

    /// The rule at a priority position.
    pub fn rule_at(&self, index: usize) -> Option<RuleView<'_>> {
        (index < self.program.rules.len()).then(|| self.view(index))
    }

    /// The rule a condition belongs to.
    pub fn rule_of(&self, cond: CondId) -> Option<RuleId> {
        self.find_cond(cond).ok().map(|(i, _)| self.ids[i].rule)
    }

    /// A node with its vocabulary entry and its parameters. `None` for a deleted rule or condition.
    pub fn node(&self, at: NodeRef) -> Option<NodeView<'_>> {
        let place = self.place(at).ok()?;
        let call = self.call(place);
        let (not, compare) = match place {
            Place::Cond(i, j) => {
                let c = &self.program.rules[i].when[j];
                (c.not, c.cmp.as_ref())
            }
            _ => (false, None),
        };
        let written = !matches!(place, Place::Target) || self.program.target.is_some();
        let node = self.nodes.get(&call.name)?;
        // Every parameter has a written value or a default in a legal program.
        let params = node
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let default = default_of(param);
                let (value, written) = match call.args.get(index) {
                    Some(a) => (a.value.clone(), true),
                    None => (default.clone()?, false),
                };
                let is_default = default.is_some_and(|d| same_amount(&value, &d, param));
                Some(ParamView {
                    index,
                    param,
                    value,
                    written,
                    is_default,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(NodeView {
            at,
            node,
            written,
            not,
            compare,
            params,
        })
    }

    /// How many rules run with `slots` rule slots (GDR-0035): the rules at positions below this number are
    /// active, the rest are below the line (inactive). `slots` comes from the design's logic cores, e.g.
    /// [`crate::Content::ship_rule_slots`].
    pub fn active_rules(&self, slots: u32) -> usize {
        self.program.rules.len().min(slots as usize)
    }

    /// The vocabulary of one role in content order (the add-node palette).
    pub fn palette(&self, role: NodeRole) -> impl Iterator<Item = &BehaviorNode> + '_ {
        self.nodes.iter().filter(move |n| n.role == role)
    }

    /// The tags of one role's vocabulary, each once, in content order (palette filters).
    pub fn palette_tags(&self, role: NodeRole) -> Vec<&str> {
        let mut tags: Vec<&str> = Vec::new();
        for t in self.palette(role).flat_map(|n| &n.tags) {
            if !tags.contains(&t.as_str()) {
                tags.push(t);
            }
        }
        tags
    }

    /// Parses and checks a text against the vocabulary without changing anything (the text view's live
    /// errors). Every error has its line and column.
    pub fn check_text(&self, text: &str) -> Result<Program, Vec<TextError>> {
        crate::behavior::check_text(text, &self.nodes)
    }

    // ---- edits ----

    /// Replaces the program with a text (the text view). Refused with every syntax and vocabulary error.
    /// Unchanged rules and conditions keep their ids: the leading and trailing rules equal to the old ones,
    /// and rules edited in place between them, by position (a condition likewise within its rule); added
    /// ones get new ids. One undo step; `Ok(false)` if the text gives the same program.
    pub fn set_text(&mut self, text: &str) -> Result<bool, Vec<TextError>> {
        let new = self.check_text(text)?;
        let mut log = Vec::new();
        if new.target != self.program.target {
            let old = self.program.target.clone();
            self.exec(
                Change::Target {
                    old,
                    new: new.target,
                },
                &mut log,
            );
        }
        let (pre, suf) = common_ends(&self.program.rules, &new.rules);
        let old_mid = self.program.rules.len() - pre - suf;
        let new_mid = new.rules.len() - pre - suf;
        let paired = old_mid.min(new_mid);
        let mut incoming = new.rules.into_iter().skip(pre);
        for (k, rule) in (0..paired).zip(incoming.by_ref()) {
            let index = pre + k;
            if self.program.rules[index] != rule {
                let old = self.entry(index);
                let conds = self.cond_ids(&old.rule.when, &old.ids.conds, &rule.when);
                let ids = RuleIds {
                    rule: old.ids.rule,
                    conds,
                };
                let new = Entry { ids, rule };
                self.exec(Change::Set { index, old, new }, &mut log);
            }
        }
        for _ in paired..old_mid {
            let index = pre + paired;
            let entry = self.entry(index);
            self.exec(Change::Remove { index, entry }, &mut log);
        }
        for (k, rule) in (paired..new_mid).zip(incoming) {
            let ids = self.fresh_ids(&rule);
            let entry = Entry { ids, rule };
            self.exec(
                Change::Insert {
                    index: pre + k,
                    entry,
                },
                &mut log,
            );
        }
        Ok(self.finish(log))
    }

    /// Adds `always -> action` at a priority position (0 = top, the number of rules = bottom). Required
    /// parameters of the action get starting values (see [`BehaviorEditor::add_condition`]).
    pub fn add_rule(&mut self, index: usize, action: &str) -> Result<RuleId, Refusal> {
        if index > self.program.rules.len() {
            return Err(Refusal::BadIndex(index));
        }
        if self.program.rules.len() >= MAX_RULES {
            return Err(Refusal::TooManyRules);
        }
        let action = new_call(self.lookup(action, NodeRole::Action)?)?;
        let rule = Rule {
            when: Vec::new(),
            action,
            note: None,
        };
        fits_line(&rule)?;
        let ids = self.fresh_ids(&rule);
        let id = ids.rule;
        let mut log = Vec::new();
        self.exec(
            Change::Insert {
                index,
                entry: Entry { ids, rule },
            },
            &mut log,
        );
        self.finish(log);
        Ok(id)
    }

    /// Deletes a rule; refused for the last one.
    pub fn delete_rule(&mut self, id: RuleId) -> Result<bool, Refusal> {
        let index = self.index_of(id)?;
        if self.program.rules.len() == 1 {
            return Err(Refusal::LastRule);
        }
        let entry = self.entry(index);
        let mut log = Vec::new();
        self.exec(Change::Remove { index, entry }, &mut log);
        Ok(self.finish(log))
    }

    /// Copies a rule (with its note) right below itself, with new ids. Returns the copy's id.
    pub fn duplicate_rule(&mut self, id: RuleId) -> Result<RuleId, Refusal> {
        let index = self.index_of(id)?;
        if self.program.rules.len() >= MAX_RULES {
            return Err(Refusal::TooManyRules);
        }
        let rule = self.program.rules[index].clone();
        let ids = self.fresh_ids(&rule);
        let copy = ids.rule;
        let mut log = Vec::new();
        self.exec(
            Change::Insert {
                index: index + 1,
                entry: Entry { ids, rule },
            },
            &mut log,
        );
        self.finish(log);
        Ok(copy)
    }

    /// Moves a rule to a priority position (the others close up): up is `index - 1`, down `index + 1`.
    pub fn move_rule(&mut self, id: RuleId, to: usize) -> Result<bool, Refusal> {
        let from = self.index_of(id)?;
        if to >= self.program.rules.len() {
            return Err(Refusal::BadIndex(to));
        }
        let mut log = Vec::new();
        if from != to {
            self.exec(Change::Move { from, to }, &mut log);
        }
        Ok(self.finish(log))
    }

    /// Sets or clears a rule's `# note`. The note is trimmed; an empty one clears it.
    pub fn set_rule_note(&mut self, id: RuleId, note: Option<&str>) -> Result<bool, Refusal> {
        let index = self.index_of(id)?;
        let mut e = self.entry(index);
        e.rule.note = clean_note(note)?;
        self.commit_rule(index, e)
    }

    /// Adds a condition at the end of a rule's conditions (`always -> x` becomes `when c -> x`). Its parameters
    /// are left to their defaults; a required one (no default) gets a starting value: the first option of a
    /// choice, else the parameter's `min` (or 0) in its unit. A value condition is compared with `<` and the
    /// example of its quantity (`hull < 30%`, `enemies_near < 2`, `cycle < 5s`).
    pub fn add_condition(&mut self, rule: RuleId, node: &str) -> Result<CondId, Refusal> {
        let index = self.index_of(rule)?;
        let node = self.lookup(node, NodeRole::Condition)?;
        let cond = Cond {
            not: false,
            call: new_call(node)?,
            cmp: node.value.map(|q| default_compare(q, CmpOp::Lt)),
        };
        let mut e = self.entry(index);
        e.rule.when.push(cond);
        let id = CondId(self.fresh());
        e.ids.conds.push(id);
        self.commit_rule(index, e)?;
        Ok(id)
    }

    /// Removes a condition (a rule without conditions is `always -> x`).
    pub fn remove_condition(&mut self, cond: CondId) -> Result<bool, Refusal> {
        let (i, j) = self.find_cond(cond)?;
        let mut e = self.entry(i);
        e.rule.when.remove(j);
        e.ids.conds.remove(j);
        self.commit_rule(i, e)
    }

    /// Turns a condition around: `not` in front, or not.
    pub fn toggle_not(&mut self, cond: CondId) -> Result<bool, Refusal> {
        let (i, j) = self.find_cond(cond)?;
        let mut e = self.entry(i);
        e.rule.when[j].not ^= true;
        self.commit_rule(i, e)
    }

    /// Sets the comparison of a value condition (`hull < 30%`): the comparator and a threshold of the
    /// condition's quantity, at least 0 (a percentage needs `%`, a count no unit, see
    /// [`crate::behavior::quantity_amount`]).
    pub fn set_compare(&mut self, cond: CondId, op: CmpOp, rhs: Value) -> Result<bool, Refusal> {
        let (i, j) = self.find_cond(cond)?;
        let name = &self.program.rules[i].when[j].call.name;
        let q = self
            .nodes
            .get(name)
            .and_then(|n| n.value)
            .ok_or_else(|| Refusal::NotAValue(name.clone()))?;
        check_threshold(&rhs, q)?;
        let mut e = self.entry(i);
        e.rule.when[j].cmp = Some(Compare { op, rhs: arg(rhs) });
        self.commit_rule(i, e)
    }

    /// Replaces a node by another of the same role (a condition, an action or the target selector). The
    /// leading values that fit the new node's parameters are kept, the rest are dropped (defaults apply);
    /// required parameters past them get starting values. A condition keeps its `not`; a value condition keeps
    /// a comparison whose threshold fits its quantity, else gets the default threshold (the comparator stays);
    /// a flag drops the comparison. The target line keeps its note.
    pub fn replace_node(&mut self, at: NodeRef, node: &str) -> Result<bool, Refusal> {
        let place = self.place(at)?;
        let node = self.lookup(node, place.role())?;
        let old = self.call(place);
        let kept: Vec<Arg> = old
            .args
            .iter()
            .zip(&node.params)
            .take_while(|(a, p)| check_param_value(&a.value, p).is_ok())
            .map(|(a, _)| a.clone())
            .collect();
        let call = complete_call(node, kept)?;
        let Place::Cond(i, j) = place else {
            return self.commit_call(place, call);
        };
        let value = node.value;
        let mut e = self.entry(i);
        let c = &mut e.rule.when[j];
        c.cmp = match (value, c.cmp.take()) {
            (None, _) => None,
            (Some(q), Some(cmp)) if check_threshold(&cmp.rhs.value, q).is_ok() => Some(cmp),
            (Some(q), Some(cmp)) => Some(default_compare(q, cmp.op)),
            (Some(q), None) => Some(default_compare(q, CmpOp::Lt)),
        };
        c.call = call;
        self.commit_rule(i, e)
    }

    /// Sets parameter `index` of a node, checked against its type, unit and range
    /// ([`check_param_value`]). Values before it that were left to their defaults are written out (values
    /// are positional). On the default target selector this writes the `target` line.
    pub fn set_param(&mut self, at: NodeRef, index: usize, value: Value) -> Result<bool, Refusal> {
        let place = self.place(at)?;
        let mut call = self.call(place);
        let node = self.node_of(&call)?;
        let param = param_at(node, index)?;
        check_value(&value, param)?;
        for p in &node.params[call.args.len().min(index)..index] {
            let default = default_of(p).ok_or_else(|| no_default(node, p))?;
            call.args.push(arg(default));
        }
        match call.args.get_mut(index) {
            Some(a) => a.value = value,
            None => call.args.push(arg(value)),
        }
        self.commit_call(place, call)
    }

    /// Resets parameter `index` of a node to its default: the default is written in its place, then the
    /// trailing values that equal their defaults are dropped (the shortest text with the same meaning), so
    /// resetting the last written value removes it. `Ok(false)` if the value was not written. Refused for a
    /// required parameter (no default).
    pub fn reset_param(&mut self, at: NodeRef, index: usize) -> Result<bool, Refusal> {
        let place = self.place(at)?;
        let mut call = self.call(place);
        let node = self.node_of(&call)?;
        let param = param_at(node, index)?;
        let default = default_of(param).ok_or_else(|| no_default(node, param))?;
        let Some(a) = call.args.get_mut(index) else {
            return Ok(false);
        };
        a.value = default;
        while let Some(last) = call.args.last() {
            let p = &node.params[call.args.len() - 1];
            if !default_of(p).is_some_and(|d| same_amount(&last.value, &d, p)) {
                break;
            }
            call.args.pop();
        }
        self.commit_call(place, call)
    }

    /// Sets or clears the target line's `# note` (writing the `target` line if the program has none). The
    /// note is trimmed; an empty one clears it.
    pub fn set_target_note(&mut self, note: Option<&str>) -> Result<bool, Refusal> {
        let note = clean_note(note)?;
        if self.program.target.is_none() && note.is_none() {
            return Ok(false);
        }
        let call = self.call(Place::Target);
        self.node_of(&call)?;
        self.commit_target(Some(TargetLine { call, note }))
    }

    /// Removes the `target` line (and its note): the rules are about the default target, `nearest`.
    pub fn clear_target(&mut self) -> bool {
        let mut log = Vec::new();
        if let Some(old) = self.program.target.clone() {
            self.exec(
                Change::Target {
                    old: Some(old),
                    new: None,
                },
                &mut log,
            );
        }
        self.finish(log)
    }

    // ---- undo ----

    /// Opens a stroke: every edit until [`BehaviorEditor::end_stroke`] becomes one undo step (a slider drag
    /// in the inspector). An open stroke is closed first; undo and redo close it too.
    pub fn begin_stroke(&mut self) {
        self.end_stroke();
        self.stroke = Some(Vec::new());
    }

    /// Closes the open stroke, if any.
    pub fn end_stroke(&mut self) {
        if let Some(step) = self.stroke.take()
            && !step.is_empty()
        {
            self.push_undo(step);
        }
    }

    /// Undoes the last step (closing an open stroke first). False if there is nothing to undo. Keeps the last
    /// [`UNDO_STEPS`] steps.
    pub fn undo(&mut self) -> bool {
        self.end_stroke();
        let Some(step) = self.undo.pop_back() else {
            return false;
        };
        for c in step.iter().rev() {
            self.apply(&c.inverse());
        }
        self.redo.push(step);
        self.revision += 1;
        debug_assert!(self.is_legal());
        true
    }

    /// Redoes the last undone step. False if there is nothing to redo (any new edit clears the redo list).
    pub fn redo(&mut self) -> bool {
        self.end_stroke();
        let Some(step) = self.redo.pop() else {
            return false;
        };
        for c in &step {
            self.apply(c);
        }
        self.push_undo(step);
        self.revision += 1;
        debug_assert!(self.is_legal());
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.stroke.as_ref().is_some_and(|s| !s.is_empty())
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // ---- internals ----

    fn view(&self, index: usize) -> RuleView<'_> {
        RuleView {
            id: self.ids[index].rule,
            index,
            rule: &self.program.rules[index],
            conds: &self.ids[index].conds,
        }
    }

    fn entry(&self, index: usize) -> Entry {
        Entry {
            ids: self.ids[index].clone(),
            rule: self.program.rules[index].clone(),
        }
    }

    fn index_of(&self, id: RuleId) -> Result<usize, Refusal> {
        self.ids
            .iter()
            .position(|r| r.rule == id)
            .ok_or(Refusal::NoSuchRule(id))
    }

    fn find_cond(&self, cond: CondId) -> Result<(usize, usize), Refusal> {
        self.ids
            .iter()
            .enumerate()
            .find_map(|(i, r)| r.conds.iter().position(|&c| c == cond).map(|j| (i, j)))
            .ok_or(Refusal::NoSuchCondition(cond))
    }

    fn place(&self, at: NodeRef) -> Result<Place, Refusal> {
        match at {
            NodeRef::Target => Ok(Place::Target),
            NodeRef::Action(r) => self.index_of(r).map(Place::Action),
            NodeRef::Cond(c) => self.find_cond(c).map(|(i, j)| Place::Cond(i, j)),
        }
    }

    /// A copy of the call at a place; the default selector's (no values) for a program without a target line.
    fn call(&self, place: Place) -> Call {
        match place {
            Place::Target => self.program.target.as_ref().map_or_else(
                || Call {
                    name: DEFAULT_TARGET.to_string(),
                    args: Vec::new(),
                    col: Col::default(),
                },
                |t| t.call.clone(),
            ),
            Place::Action(i) => self.program.rules[i].action.clone(),
            Place::Cond(i, j) => self.program.rules[i].when[j].call.clone(),
        }
    }

    /// The vocabulary entry of a call in the program (only the default target can be missing).
    fn node_of(&self, call: &Call) -> Result<&BehaviorNode, Refusal> {
        self.nodes
            .get(&call.name)
            .ok_or_else(|| Refusal::UnknownNode(call.name.clone()))
    }

    fn lookup(&self, id: &str, role: NodeRole) -> Result<&BehaviorNode, Refusal> {
        let node = self
            .nodes
            .get(id)
            .ok_or_else(|| Refusal::UnknownNode(id.to_string()))?;
        if node.role != role {
            return Err(Refusal::WrongRole {
                node: id.to_string(),
                expected: role,
            });
        }
        Ok(node)
    }

    fn fresh(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn fresh_ids(&mut self, rule: &Rule) -> RuleIds {
        RuleIds {
            rule: RuleId(self.fresh()),
            conds: rule.when.iter().map(|_| CondId(self.fresh())).collect(),
        }
    }

    /// Ids for the conditions `new` of a rule that had `old` with `ids`: unchanged leading and trailing
    /// conditions keep theirs, changed ones between them keep theirs by position, added ones get new ids.
    fn cond_ids(&mut self, old: &[Cond], ids: &[CondId], new: &[Cond]) -> Vec<CondId> {
        let (pre, suf) = common_ends(old, new);
        let old_mid = old.len() - pre - suf;
        (0..new.len())
            .map(|j| {
                if j < pre || (j < pre + old_mid && j < new.len() - suf) {
                    ids[j]
                } else if j >= new.len() - suf {
                    ids[old.len() - (new.len() - j)]
                } else {
                    CondId(self.fresh())
                }
            })
            .collect()
    }

    /// Stores a call at its place as one edit (the target line keeps its note).
    fn commit_call(&mut self, place: Place, call: Call) -> Result<bool, Refusal> {
        match place {
            // The default selector stays implicit until it changes.
            Place::Target if self.program.target.is_none() && call == self.call(place) => Ok(false),
            Place::Target => {
                let note = self.program.target.as_ref().and_then(|t| t.note.clone());
                self.commit_target(Some(TargetLine { call, note }))
            }
            Place::Action(i) => {
                let mut e = self.entry(i);
                e.rule.action = call;
                self.commit_rule(i, e)
            }
            Place::Cond(i, j) => {
                let mut e = self.entry(i);
                e.rule.when[j].call = call;
                self.commit_rule(i, e)
            }
        }
    }

    /// Replaces rule `index` by an edited copy as one edit, if it keeps the limits.
    fn commit_rule(&mut self, index: usize, new: Entry) -> Result<bool, Refusal> {
        if new.rule.when.len() > MAX_CONDITIONS {
            return Err(Refusal::TooManyConditions);
        }
        fits_line(&new.rule)?;
        let mut log = Vec::new();
        if new.rule != self.program.rules[index] || new.ids != self.ids[index] {
            let old = self.entry(index);
            self.exec(Change::Set { index, old, new }, &mut log);
        }
        Ok(self.finish(log))
    }

    fn commit_target(&mut self, new: Option<TargetLine>) -> Result<bool, Refusal> {
        if let Some(t) = &new {
            fits_line(t)?;
        }
        let mut log = Vec::new();
        if new != self.program.target {
            let old = self.program.target.clone();
            self.exec(Change::Target { old, new }, &mut log);
        }
        Ok(self.finish(log))
    }

    fn exec(&mut self, change: Change, log: &mut Vec<Change>) {
        self.apply(&change);
        log.push(change);
    }

    /// Records the applied changes of one edit (into the open stroke, or as a new undo step). Returns whether
    /// anything changed.
    fn finish(&mut self, log: Vec<Change>) -> bool {
        let changed = !log.is_empty();
        if changed {
            self.revision += 1;
            self.redo.clear();
            match &mut self.stroke {
                Some(stroke) => stroke.extend(log),
                None => self.push_undo(log),
            }
            debug_assert!(self.is_legal());
        }
        changed
    }

    fn push_undo(&mut self, step: Vec<Change>) {
        if self.undo.len() == UNDO_STEPS {
            self.undo.pop_front();
        }
        self.undo.push_back(step);
    }

    fn apply(&mut self, change: &Change) {
        match change {
            Change::Insert { index, entry } => {
                self.program.rules.insert(*index, entry.rule.clone());
                self.ids.insert(*index, entry.ids.clone());
            }
            Change::Remove { index, .. } => {
                self.program.rules.remove(*index);
                self.ids.remove(*index);
            }
            Change::Set { index, new, .. } => {
                self.program.rules[*index] = new.rule.clone();
                self.ids[*index] = new.ids.clone();
            }
            Change::Move { from, to } => {
                let rule = self.program.rules.remove(*from);
                self.program.rules.insert(*to, rule);
                let ids = self.ids.remove(*from);
                self.ids.insert(*to, ids);
            }
            Change::Target { new, .. } => self.program.target = new.clone(),
        }
    }

    /// The invariant (checked after every change in debug builds): the program checks against the
    /// vocabulary, survives its text form, and the ids match its shape.
    fn is_legal(&self) -> bool {
        check_program(&self.program, &self.nodes).is_empty()
            && Program::parse(&self.program.to_text()).is_ok_and(|p| p == self.program)
            && self.ids.len() == self.program.rules.len()
            && self
                .ids
                .iter()
                .zip(&self.program.rules)
                .all(|(ids, r)| ids.conds.len() == r.when.len())
    }
}

/// How many leading and how many (non-overlapping) trailing items two lists share.
fn common_ends<T: PartialEq>(a: &[T], b: &[T]) -> (usize, usize) {
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..]
        .iter()
        .rev()
        .zip(b[pre..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    (pre, suf)
}

fn arg(value: Value) -> Arg {
    Arg {
        value,
        col: Col::default(),
    }
}

fn param_at(node: &BehaviorNode, index: usize) -> Result<&NodeParam, Refusal> {
    node.params.get(index).ok_or_else(|| Refusal::NoSuchParam {
        node: node.id.clone(),
        index,
    })
}

fn no_default(node: &BehaviorNode, p: &NodeParam) -> Refusal {
    Refusal::NoDefault {
        node: node.id.clone(),
        param: p.id.clone(),
    }
}

fn default_of(p: &NodeParam) -> Option<Value> {
    parse_value(p.default.as_deref()?).ok()
}

/// Do two values of a parameter mean the same (`45` and `45m`)?
fn same_amount(a: &Value, b: &Value, p: &NodeParam) -> bool {
    matches!((param_amount(a, &p.ty), param_amount(b, &p.ty)), (Ok(x), Ok(y)) if x == y)
}

/// A new call of `node` without values: defaults apply.
fn new_call(node: &BehaviorNode) -> Result<Call, Refusal> {
    complete_call(node, Vec::new())
}

/// A call of `node` with these leading values, plus starting values for required parameters after them.
fn complete_call(node: &BehaviorNode, mut args: Vec<Arg>) -> Result<Call, Refusal> {
    for p in node.params[args.len()..]
        .iter()
        .take_while(|p| p.default.is_none())
    {
        let v = starting_value(p).ok_or_else(|| no_default(node, p))?;
        check_value(&v, p)?;
        args.push(arg(v));
    }
    Ok(Call {
        name: node.id.clone(),
        args,
        col: Col::default(),
    })
}

/// The value a new node gets for a required parameter: the first option of a choice, else `min` (or 0, but
/// at most `max`) in the parameter's unit (metres for distances).
fn starting_value(p: &NodeParam) -> Option<Value> {
    let unit = match &p.ty {
        ParamType::Choice(options) => return options.first().map(|o| Value::Word(o.id.clone())),
        ParamType::Distance => Some(Unit::Metres),
        ParamType::Percent => Some(Unit::Percent),
        ParamType::Seconds => Some(Unit::Seconds),
        ParamType::Count => None,
    };
    let lo = p.min.unwrap_or(0.0);
    Some(Value::Number(p.max.map_or(lo, |hi| lo.min(hi)), unit))
}

/// The comparison a value condition starts with: `op` and the example of its quantity (`30%`, `2`, `5s`).
fn default_compare(q: Quantity, op: CmpOp) -> Compare {
    let rhs = parse_value(q.example()).expect("quantity examples are single values");
    Compare { op, rhs: arg(rhs) }
}

fn finite(v: &Value) -> Result<(), Refusal> {
    match v {
        Value::Number(x, _) if !x.is_finite() => {
            Err(Refusal::BadValue(format!("`{x}` is not a number")))
        }
        _ => Ok(()),
    }
}

/// A parameter value: finite, and of the parameter's type, unit and range.
fn check_value(v: &Value, p: &NodeParam) -> Result<(), Refusal> {
    finite(v)?;
    check_param_value(v, p).map(drop).map_err(Refusal::BadValue)
}

/// A comparison threshold, as [`check_program`] wants it: finite, of the quantity, at least 0.
fn check_threshold(v: &Value, q: Quantity) -> Result<(), Refusal> {
    finite(v)?;
    quantity_amount(v, q).map_err(Refusal::BadValue)?;
    match v {
        Value::Number(x, _) if *x < 0.0 => Err(Refusal::BadValue(
            "compare with a number of at least 0".into(),
        )),
        _ => Ok(()),
    }
}

/// A note as the text form keeps it: trimmed, `None` when empty, one line.
fn clean_note(note: Option<&str>) -> Result<Option<String>, Refusal> {
    let Some(n) = note.map(str::trim).filter(|n| !n.is_empty()) else {
        return Ok(None);
    };
    if n.chars().any(char::is_control) {
        return Err(Refusal::BadNote);
    }
    Ok(Some(n.to_string()))
}

/// A printed line within [`MAX_LINE_CHARS`] characters.
fn fits_line(line: &impl fmt::Display) -> Result<(), Refusal> {
    match line.to_string().chars().count() <= MAX_LINE_CHARS {
        true => Ok(()),
        false => Err(Refusal::LineTooLong),
    }
}
