//! The behavior editor's editing model: one behavior program (the target line and the rules, with their notes)
//! and the vocabulary it is written in, edited with small legal operations, with undo/redo and a revision
//! counter. Engine-free and wasm-safe: the counterpart of [`crate::design::edit`] for rules. The BEHAVIOR screen edits
//! through it in both views: the graph view and the inspector call the operations, the text view replaces the
//! program with [`BehaviorEditor::set_text`]. The rules are documented in `docs/behavior-format.md` ("Editing
//! model").
//!
//! The program is always legal: it passes [`check_list`] against the vocabulary for its [`ListKind`] (a behavior or
//! a design's pilot assists, GDR-0088), has at least one rule (a behavior; an assist list may have none) and
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
//! use data::behavior::edit::{BehaviorEditor, NodeRef};
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

mod edits;
mod state;

use std::collections::VecDeque;
use std::fmt;

use crate::behavior::{
    Arg, Call, CmpOp, Col, Compare, Cond, MAX_CONDITIONS, MAX_LINE_CHARS, MAX_REACH_PERCENT,
    MAX_RULES, ListKind, Program, Rule, TargetLine, TextError, Unit, Value, check_list, check_param_value,
    param_amount, parse_value, quantity_amount,
};
use crate::design::edit::UNDO_STEPS;
use crate::content::model::{BehaviorNode, NodeParam, NodeRole, ParamType, Quantity};
use crate::content::table::Table;

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
    /// The target selector; while the program has no `target` line, the default ([`DEFAULT_TARGET`]). Pilot
    /// assists have none (refused with [`Refusal::NotInList`]).
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
    /// The node doesn't belong in this kind of list ([`ListKind::allows`]): a pilot assist's node in a behavior,
    /// a behavior's maneuver in pilot assists.
    NotInList(String),
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
            Refusal::NotInList(id) => write!(f, "`{id}` doesn't belong in this list"),
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
    /// A custom condition written in Rhai (tagged SCRIPT in the editor).
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
    /// A behavior or a design's pilot assists: what may be written, and whether a rule is needed.
    kind: ListKind,
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
        BehaviorEditor::from_text_as(nodes, text, ListKind::Behavior)
    }

    /// [`BehaviorEditor::open`] for a list of either kind (a design's pilot assists may have no rule at all).
    pub fn open_as(
        nodes: &Table<BehaviorNode>,
        program: &Program,
        kind: ListKind,
    ) -> Result<BehaviorEditor, Vec<TextError>> {
        BehaviorEditor::from_text_as(nodes, &program.to_text(), kind)
    }

    /// [`BehaviorEditor::from_text`] for a list of either kind.
    pub fn from_text_as(
        nodes: &Table<BehaviorNode>,
        text: &str,
        kind: ListKind,
    ) -> Result<BehaviorEditor, Vec<TextError>> {
        let program = crate::behavior::check_text_as(text, nodes, kind)?;
        let mut ed = BehaviorEditor {
            nodes: nodes.clone(),
            kind,
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

    /// A behavior or a design's pilot assists.
    pub fn kind(&self) -> ListKind {
        self.kind
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

    /// The vocabulary of one role in content order that this kind of list may use (the add-node palette).
    pub fn palette(&self, role: NodeRole) -> impl Iterator<Item = &BehaviorNode> + '_ {
        self.nodes.iter().filter(move |n| n.role == role && self.kind.allows(n))
    }

    /// Parses and checks a text against the vocabulary without changing anything (the text view's live
    /// errors). Every error has its line and column.
    pub fn check_text(&self, text: &str) -> Result<Program, Vec<TextError>> {
        crate::behavior::check_text_as(text, &self.nodes, self.kind)
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
