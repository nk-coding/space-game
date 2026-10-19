//! What the right column shows, as plain data: the inspector for the selection (its role and kind, name and
//! summary, and the controls that edit it, each carrying the [`BehaviorAction`] it sends) and the ADD NODE
//! palette (the content vocabulary, filtered by the search). The screen (`screen.rs`, `controls.rs`) draws these;
//! the tests below pin the mapping.
//!
//! * **A node**: its values by kind ([`NumberRow`]: −/+ steppers and a slider within the node's range, metres ↔ %
//!   of the weapon reach for distances, reset to the default; [`ChoiceRow`]: the options as pills). A condition
//!   adds `not` ([`SwitchRow`]), a value condition its comparator pills and threshold, and REPLACE / REMOVE; an
//!   action says that a palette action replaces it. Both end with their rule's block (UP, DOWN, DUPLICATE,
//!   DELETE, the note). The target line: its selector, its aim point, back to the default, its note.
//! * **A rule** (its `always` chip or its row): the rule as text and its block.
//! * **A palette node** with nothing to add it to: its default values, read-only, and how to add it.

use data::behavior::{CmpOp, ListKind, MAX_RULES, Value, parse_value};
use data::behavior::edit::{BehaviorEditor, CondId, NodeRef, ParamView, Refusal, RuleId};
use data::{Behavior, BehaviorNode, NodeRole, ParamType, Table};
use kit::ui_kit::Tone;

use super::actions::BehaviorAction;
use super::edit::Edit;
use super::graph::{always_decides, never_reached};
use super::values::{Kind, Scale, reach_hint, switch_unit};

/// The tag of a condition written as a Rhai script (`scripts.ron`), and its tooltip: what the word means.
pub const SCRIPT_TAG: &str = "SCRIPT";
pub const SCRIPT_TIP: &str = "SCRIPT: A CONDITION WRITTEN AS A SMALL SCRIPT, NOT BUILT INTO THE GAME";

/// What the inspector shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Selection {
    /// A node of the program.
    Node(NodeRef),
    /// A rule (its `always` chip or its row).
    Rule(RuleId),
    /// A vocabulary entry picked in the palette (content id), with its default values.
    Vocabulary(String),
}

impl Selection {
    /// The rule this selection is in: a rule, or one of its nodes.
    pub fn rule(&self, ed: &BehaviorEditor) -> Option<RuleId> {
        match self {
            Selection::Rule(r) | Selection::Node(NodeRef::Action(r)) => ed.rule(*r).map(|v| v.id),
            Selection::Node(NodeRef::Cond(c)) => ed.rule_of(*c),
            Selection::Node(NodeRef::Target) | Selection::Vocabulary(_) => None,
        }
    }

    /// Whether it is still there (an undo or a text edit can remove it).
    pub fn exists(&self, ed: &BehaviorEditor) -> bool {
        match self {
            Selection::Node(at) => ed.node(*at).is_some(),
            Selection::Rule(r) => ed.rule(*r).is_some(),
            Selection::Vocabulary(id) => ed.nodes().get(id).is_some(),
        }
    }
}

/// What the inspector needs besides the program.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Context {
    /// The design's rule slots.
    pub slots: u32,
    /// The design's weapon reach in metres (the range of its longest clear gun), when known.
    pub reach: Option<f32>,
    /// REPLACE is armed on this condition.
    pub replacing: Option<NodeRef>,
}

/// The inspector's content.
#[derive(Clone, Debug, PartialEq)]
pub struct Inspection {
    /// The tracked line above the name: role and kind (`ACTION · MANEUVER`).
    pub kicker: String,
    pub name: String,
    /// Colours the name: conditions violet, actions teal, Rhai gold; `None` for the behavior or a rule (white).
    pub role: Option<NodeRole>,
    pub rhai: bool,
    pub summary: String,
    pub controls: Vec<Control>,
    /// A Rhai condition's script.
    pub script: Option<String>,
}

/// One block of the inspector.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    Number(NumberRow),
    Choice(ChoiceRow),
    Switch(SwitchRow),
    /// A row of buttons under a label (REPLACE / REMOVE; the rule's UP / DOWN / DUPLICATE / DELETE).
    Buttons { label: String, buttons: Vec<Button> },
    /// A one-line text field for a note.
    Note { target: NoteTarget, text: String },
    /// A line of help, or why a button is off.
    Hint(String),
}

/// A pill button; `action: None` is disabled.
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub text: String,
    pub action: Option<BehaviorAction>,
    pub tone: Tone,
    pub active: bool,
}

impl Button {
    fn new(text: &str, action: Option<BehaviorAction>) -> Self {
        Self { text: text.into(), action, tone: Tone::Teal, active: false }
    }
}

/// A number: `RANGE · 67%`, steppers and a slider when editable, a bar when not.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberRow {
    pub label: String,
    /// As written (`67%`, `45m`, `2s`, `3`).
    pub value: String,
    /// The metres a share of the weapon reach stands for on this design (`≈ 100 M`).
    pub hint: Option<String>,
    /// `0 – 100 %`; empty when the unit pills say it.
    pub range: String,
    /// The rule leaves it out: the default applies.
    pub default: bool,
    pub reset: Option<BehaviorAction>,
    pub minus: Option<BehaviorAction>,
    pub plus: Option<BehaviorAction>,
    pub slider: Option<SliderSpec>,
    /// Distances: `M` and `% REACH`, the active one marked.
    pub units: Vec<Button>,
}

/// A slider: where the value sits on its scale and what dragging it edits (`None`: read-only, a bar).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderSpec {
    pub scale: Scale,
    pub value: f32,
    pub target: Option<SliderTarget>,
}

/// What a slider edits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderTarget {
    Param { at: NodeRef, index: usize },
    /// A value condition's threshold, keeping its comparator.
    Threshold { cond: CondId, op: CmpOp },
}

impl SliderTarget {
    /// The edit that sets the number to `x`.
    pub fn edit(self, kind: Kind, x: f32) -> Edit {
        match self {
            SliderTarget::Param { at, index } => Edit::SetParam { at, index, value: kind.value(x) },
            SliderTarget::Threshold { cond, op } => Edit::SetCompare { cond, op, rhs: kind.value(x) },
        }
    }
}

/// One of a few words, as pills.
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceRow {
    pub label: String,
    pub default: bool,
    pub reset: Option<BehaviorAction>,
    pub options: Vec<Button>,
}

/// An on/off switch (`not`).
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchRow {
    pub label: String,
    pub hint: String,
    pub on: bool,
    pub action: Option<BehaviorAction>,
}

/// Whose note a note field edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteTarget {
    Rule(RuleId),
    Target,
}

fn edit(e: Edit) -> BehaviorAction {
    BehaviorAction::Edit(e)
}

fn number_of(v: &Value) -> Option<f32> {
    match v {
        Value::Number(x, _) => Some(*x),
        Value::Word(_) => None,
    }
}

/// The behavior itself (nothing selected): its name, its content summary and a hint.
/// The inspector with nothing selected on the PILOT ASSISTS tab (GDR-0088): what assists are and how they run.
pub fn assists_inspection(rules: usize) -> Inspection {
    Inspection {
        kicker: format!("PILOT ASSISTS  ·  {rules} RULE{}", if rules == 1 { "" } else { "S" }),
        name: super::draft::ASSISTS_NAME.into(),
        role: None,
        rhai: false,
        summary: "They help you fly this design. Each assist works one channel (your lock, thrust or a weapon group's trigger) whenever its rule matches and you leave that channel free; the first matching rule per channel decides. What needs a target acts on your lock. Its TOGGLE key switches it on and off in flight, ALL ASSISTS every one.".into(),
        controls: vec![Control::Hint("CLICK A NODE OR A RULE TO EDIT IT  ·  + ASSIST ADDS ONE  ·  SAVED WITH THE DESIGN".into())],
        script: None,
    }
}

pub fn behavior_inspection(name: &str, source: Option<&Behavior>, rules: usize) -> Inspection {
    Inspection {
        kicker: format!("BEHAVIOR  ·  {rules} RULE{}", if rules == 1 { "" } else { "S" }),
        name: name.to_string(),
        role: None,
        rhai: false,
        summary: source.map_or_else(String::new, |b| b.summary.clone()),
        controls: vec![Control::Hint("CLICK A NODE OR A RULE TO EDIT IT  ·  + RULE ADDS ONE".into())],
        script: None,
    }
}

/// The inspector for a selection; `None` if the program or the vocabulary does not have it.
pub fn inspect(ed: &BehaviorEditor, sel: &Selection, cx: &Context) -> Option<Inspection> {
    match sel {
        Selection::Vocabulary(id) => ed.nodes().get(id).map(|n| preview(n, cx)),
        Selection::Rule(r) => rule_inspection(ed, *r, cx),
        Selection::Node(at) => node_inspection(ed, *at, cx),
    }
}

fn kind_line(node: &BehaviorNode) -> &'static str {
    match node.role {
        NodeRole::Action if node.pilot => "ACTION  ·  PILOT ASSIST",
        NodeRole::Condition if node.pilot => "CONDITION  ·  THE PILOT",
        NodeRole::Condition if node.script.is_some() => "CONDITION  ·  SCRIPT",
        NodeRole::Condition if node.value.is_some() => "CONDITION  ·  VALUE",
        NodeRole::Condition => "CONDITION  ·  FLAG",
        NodeRole::Action if !sim::behavior::action_decides(&node.id) => "ACTION  ·  STEPS ASIDE",
        NodeRole::Action => "ACTION  ·  MANEUVER",
        NodeRole::Target => "TARGET  ·  SELECTOR",
    }
}

fn node_inspection(ed: &BehaviorEditor, at: NodeRef, cx: &Context) -> Option<Inspection> {
    let v = ed.node(at)?;
    let mut controls = Vec::new();
    match at {
        NodeRef::Target => {
            controls.push(Control::Choice(ChoiceRow {
                label: "SELECTOR".into(),
                default: !v.written,
                reset: None,
                options: ed
                    .palette(NodeRole::Target)
                    .map(|n| Button {
                        active: n.id == v.node.id,
                        ..Button::new(&n.name.to_uppercase(), Some(edit(Edit::ReplaceNode { at, node: n.id.clone() })))
                    })
                    .collect(),
            }));
            params(&mut controls, Some(at), &v.params, cx);
            if v.written {
                controls.push(Control::Buttons { label: String::new(), buttons: vec![Button::new("USE THE DEFAULT TARGET", Some(edit(Edit::ClearTarget)))] });
            }
            let note = ed.program().target.as_ref().and_then(|t| t.note.clone());
            controls.push(Control::Note { target: NoteTarget::Target, text: note.unwrap_or_default() });
        }
        NodeRef::Cond(cond) => {
            controls.push(Control::Switch(SwitchRow {
                label: "NOT".into(),
                hint: "TRUE WHEN IT DOES NOT HOLD".into(),
                on: v.not,
                action: Some(edit(Edit::ToggleNot(cond))),
            }));
            if let (Some(q), Some(cmp)) = (v.node.value, v.compare) {
                let rhs = &cmp.rhs.value;
                controls.push(Control::Choice(ChoiceRow {
                    label: "COMPARE".into(),
                    default: false,
                    reset: None,
                    options: [CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge]
                        .into_iter()
                        .map(|op| Button { active: op == cmp.op, ..Button::new(op.symbol(), Some(edit(Edit::SetCompare { cond, op, rhs: rhs.clone() }))) })
                        .collect(),
                }));
                // A time is compared within the node's own period (`cycle(9s) >= 5s`).
                let period = v.params.iter().find(|p| p.param.ty == ParamType::Seconds).and_then(|p| number_of(&p.value));
                if let (Some(scale), Some(x)) = (Scale::threshold(q, rhs, period), number_of(rhs)) {
                    let target = SliderTarget::Threshold { cond, op: cmp.op };
                    controls.push(Control::Number(NumberRow {
                        label: "THAN".into(),
                        value: rhs.to_string(),
                        hint: None,
                        range: scale.label(),
                        default: false,
                        reset: None,
                        minus: scale.down(x).map(|y| edit(target.edit(scale.kind, y))),
                        plus: scale.up(x).map(|y| edit(target.edit(scale.kind, y))),
                        slider: Some(SliderSpec { scale, value: x, target: Some(target) }),
                        units: Vec::new(),
                    }));
                }
            }
            params(&mut controls, Some(at), &v.params, cx);
            let armed = cx.replacing == Some(at);
            controls.push(Control::Buttons {
                label: String::new(),
                buttons: vec![
                    Button { active: armed, ..Button::new("REPLACE", Some(BehaviorAction::Replace)) },
                    Button { tone: Tone::Coral, ..Button::new("REMOVE", Some(edit(Edit::RemoveCondition(cond)))) },
                ],
            });
            if armed {
                controls.push(Control::Hint("PICK A CONDITION IN ADD NODE TO PUT IN ITS PLACE".into()));
            }
            rule_block(&mut controls, ed, ed.rule_of(cond)?, cx);
        }
        NodeRef::Action(rule) => {
            params(&mut controls, Some(at), &v.params, cx);
            controls.push(Control::Hint("AN ACTION PICKED IN ADD NODE REPLACES IT".into()));
            rule_block(&mut controls, ed, rule, cx);
        }
    }
    Some(Inspection {
        kicker: kind_line(v.node).into(),
        name: v.node.name.clone(),
        role: Some(v.node.role),
        rhai: v.is_rhai(),
        summary: v.node.summary.clone(),
        controls,
        script: v.node.script.clone(),
    })
}

fn rule_inspection(ed: &BehaviorEditor, rule: RuleId, cx: &Context) -> Option<Inspection> {
    let view = ed.rule(rule)?;
    let mut controls = vec![Control::Hint("A CONDITION PICKED IN ADD NODE IS ADDED TO THIS RULE; AN ACTION REPLACES ITS ACTION".into())];
    rule_block(&mut controls, ed, rule, cx);
    let state = if ed.kind() == ListKind::Assists {
        "DECIDES ITS CHANNEL UNLESS A RULE ABOVE DOES"
    } else if view.index >= ed.active_rules(cx.slots) {
        "NO RULE SLOT: DOES NOT RUN"
    } else if never_reached_in(ed, cx).contains(&view.index) {
        "NEVER REACHED: AN ALWAYS RULE ABOVE MATCHES FIRST"
    } else {
        "RUNS IN ITS TURN"
    };
    Some(Inspection {
        kicker: format!("RULE  ·  {state}"),
        name: format!("Rule {}", view.index + 1),
        role: None,
        rhai: false,
        summary: view.rule.to_string(),
        controls,
        script: None,
    })
}

/// The rules of the program that have a slot but sit below an `always` rule that always decides
/// (`graph::never_reached`); none in pilot assists, where each rule decides its own channel.
fn never_reached_in(ed: &BehaviorEditor, cx: &Context) -> std::ops::Range<usize> {
    if ed.kind() == ListKind::Assists {
        return 0..0;
    }
    never_reached(&always_decides(ed.program()), cx.slots)
}

/// A rule's block: its place, UP / DOWN / DUPLICATE / DELETE (the last rule cannot go: the hint says why), its
/// note.
fn rule_block(out: &mut Vec<Control>, ed: &BehaviorEditor, rule: RuleId, cx: &Context) {
    let Some(view) = ed.rule(rule) else { return };
    let (i, n) = (view.index, ed.rules().len());
    // A behavior keeps its last rule; pilot assists may have none (GDR-0088).
    let keeps_one = ed.kind().needs_a_rule();
    let slot = if keeps_one && i >= ed.active_rules(cx.slots) {
        "  ·  NO SLOT"
    } else if never_reached_in(ed, cx).contains(&i) {
        "  ·  NEVER REACHED"
    } else {
        ""
    };
    let mv = |to: usize| Some(edit(Edit::MoveRule { rule, to }));
    out.push(Control::Buttons {
        label: format!("RULE {} OF {n}{slot}", i + 1),
        buttons: vec![
            Button::new("UP", if i > 0 { mv(i - 1) } else { None }),
            Button::new("DOWN", if i + 1 < n { mv(i + 1) } else { None }),
            Button::new("DUPLICATE", (n < MAX_RULES).then(|| edit(Edit::DuplicateRule(rule)))),
            Button { tone: Tone::Coral, ..Button::new("DELETE", (n > 1 || !keeps_one).then(|| edit(Edit::DeleteRule(rule)))) },
        ],
    });
    if n == 1 && keeps_one {
        out.push(Control::Hint(format!("{}: THE LAST RULE STAYS", Refusal::LastRule.to_string().to_uppercase())));
    }
    out.push(Control::Note { target: NoteTarget::Rule(rule), text: view.rule.note.clone().unwrap_or_default() });
}

/// The rows of a node's parameters; editable when `at` is a node of the program.
fn params(out: &mut Vec<Control>, at: Option<NodeRef>, params: &[ParamView], cx: &Context) {
    for p in params {
        let index = p.index;
        let label = p.param.name.to_uppercase();
        let reset = at.filter(|_| p.written).map(|at| edit(Edit::ResetParam { at, index }));
        let set = |value: Value| at.map(|at| edit(Edit::SetParam { at, index, value }));
        if let ParamType::Choice(options) = &p.param.ty {
            out.push(Control::Choice(ChoiceRow {
                label,
                default: !p.written,
                reset,
                options: options
                    .iter()
                    .map(|o| Button {
                        active: matches!(&p.value, Value::Word(w) if *w == o.id),
                        ..Button::new(&o.name.to_uppercase(), set(Value::Word(o.id.clone())))
                    })
                    .collect(),
            }));
            continue;
        }
        let (Some(scale), Some(x)) = (Scale::param(p.param, &p.value), number_of(&p.value)) else { continue };
        let units = match (p.param.ty == ParamType::Distance, switch_unit(p.param, &p.value, cx.reach)) {
            (true, Some(other)) => {
                let reach = scale.kind == Kind::Reach;
                vec![
                    Button { active: !reach, ..Button::new("M", if reach { set(other.clone()) } else { None }) },
                    Button { active: reach, ..Button::new("% REACH", if reach { None } else { set(other) }) },
                ]
            }
            _ => Vec::new(),
        };
        let target = at.map(|at| SliderTarget::Param { at, index });
        let step = |y: Option<f32>| target.zip(y).map(|(t, y)| edit(t.edit(scale.kind, y)));
        out.push(Control::Number(NumberRow {
            label,
            value: p.value.to_string(),
            hint: reach_hint(&p.value, cx.reach),
            range: if units.is_empty() { scale.label() } else { String::new() },
            default: !p.written,
            reset,
            minus: step(scale.down(x)),
            plus: step(scale.up(x)),
            slider: Some(SliderSpec { scale, value: x, target }),
            units,
        }));
    }
}

/// A palette node shown with nothing to add it to: its default values, read-only, and how to use it.
fn preview(node: &BehaviorNode, cx: &Context) -> Inspection {
    let views: Vec<ParamView> = node
        .params
        .iter()
        .enumerate()
        .filter_map(|(index, param)| {
            let value = parse_value(param.default.as_deref()?).ok()?;
            Some(ParamView { index, param, value, written: false, is_default: true })
        })
        .collect();
    let mut controls = Vec::new();
    params(&mut controls, None, &views, cx);
    controls.push(Control::Hint(
        match node.role {
            NodeRole::Condition => "SELECT A RULE, THEN PICK THIS TO ADD IT",
            NodeRole::Action => "SELECT A RULE, THEN PICK THIS AS ITS ACTION",
            NodeRole::Target => "SELECT A RULE OR THE TARGET, THEN PICK THIS AS THE TARGET",
        }
        .into(),
    ));
    Inspection {
        kicker: kind_line(node).into(),
        name: node.name.clone(),
        role: Some(node.role),
        rhai: node.script.is_some(),
        summary: node.summary.clone(),
        controls,
        script: node.script.clone(),
    }
}

/// The line above the palette: where a pick goes.
pub fn palette_hint(ed: &BehaviorEditor, sel: Option<&Selection>, replacing: Option<NodeRef>) -> String {
    if let Some(v) = replacing.and_then(|at| ed.node(at)) {
        return format!("REPLACE {}  ·  PICK A CONDITION", v.node.name.to_uppercase());
    }
    match (sel, sel.and_then(|s| s.rule(ed)).and_then(|r| ed.rule(r))) {
        (_, Some(rule)) => format!("ADD NODE  ·  TO RULE {}", rule.index + 1),
        (Some(Selection::Node(NodeRef::Target)), _) => "ADD NODE  ·  A SELECTOR SETS THE TARGET".into(),
        _ => "ADD NODE".into(),
    }
}

/// One entry of the ADD NODE palette.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteEntry {
    pub id: String,
    /// The pill's text: the name in capitals, `· SCRIPT` after a Rhai condition.
    pub label: String,
    pub role: NodeRole,
    pub rhai: bool,
}

/// The vocabulary for the palette: conditions, then actions, then target selectors, each in content order, those
/// the list may use (a behavior's or the pilot assists', GDR-0088); with a search, only the nodes whose name or id
/// contains it (ignoring case).
pub fn palette(nodes: &Table<BehaviorNode>, kind: ListKind, search: &str) -> Vec<PaletteEntry> {
    let search = search.trim().to_lowercase();
    let wanted = |n: &BehaviorNode| search.is_empty() || n.name.to_lowercase().contains(&search) || n.id.contains(&search);
    [NodeRole::Condition, NodeRole::Action, NodeRole::Target]
        .into_iter()
        .flat_map(|role| nodes.iter().filter(move |n| n.role == role && kind.allows(n)))
        .filter(|n| wanted(n))
        .map(|n| PaletteEntry {
            id: n.id.clone(),
            label: if n.script.is_some() { format!("{}  ·  {SCRIPT_TAG}", n.name.to_uppercase()) } else { n.name.to_uppercase() },
            role: n.role,
            rhai: n.script.is_some(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::behavior::Unit;

    fn skirmisher() -> BehaviorEditor {
        let content = data::base_content();
        BehaviorEditor::open(&content.behavior_nodes, &content.behaviors.get("skirmisher").unwrap().program().unwrap()).unwrap()
    }

    fn cx() -> Context {
        Context { slots: 12, reach: Some(150.0), replacing: None }
    }

    fn number(c: &Control) -> &NumberRow {
        let Control::Number(n) = c else { panic!("{c:?}") };
        n
    }

    fn choice(c: &Control) -> &ChoiceRow {
        let Control::Choice(n) = c else { panic!("{c:?}") };
        n
    }

    fn set(at: NodeRef, index: usize, value: Value) -> Option<BehaviorAction> {
        Some(edit(Edit::SetParam { at, index, value }))
    }

    #[test]
    fn an_action_gets_steppers_a_slider_units_and_choices() {
        let ed = skirmisher();
        // `orbit(80%, ccw)`
        let rule = ed.rule_at(1).unwrap().id;
        let at = NodeRef::Action(rule);
        let i = inspect(&ed, &Selection::Node(at), &cx()).unwrap();
        assert_eq!((i.kicker.as_str(), i.name.as_str(), i.role), ("ACTION  ·  MANEUVER", "Orbit", Some(NodeRole::Action)));
        let radius = number(&i.controls[0]);
        assert_eq!((radius.label.as_str(), radius.value.as_str(), radius.default), ("RADIUS", "80%", false));
        assert_eq!(radius.hint.as_deref(), Some("≈ 120 M"));
        assert_eq!(radius.minus, set(at, 0, Value::Number(75.0, Some(Unit::Percent))));
        assert_eq!(radius.plus, set(at, 0, Value::Number(85.0, Some(Unit::Percent))));
        assert_eq!(radius.reset, Some(edit(Edit::ResetParam { at, index: 0 })));
        let slider = radius.slider.unwrap();
        assert_eq!((slider.scale.kind, slider.value, slider.target), (Kind::Reach, 80.0, Some(SliderTarget::Param { at, index: 0 })));
        // Metres ↔ % of the weapon reach: 80 % of 150 m is 120 m.
        assert_eq!(radius.units.iter().map(|b| (b.text.as_str(), b.active)).collect::<Vec<_>>(), [("M", false), ("% REACH", true)]);
        assert_eq!(radius.units[0].action, set(at, 0, Value::Number(120.0, Some(Unit::Metres))));
        assert_eq!(radius.units[1].action, None);
        // The direction as pills, the chosen one active, each one an edit.
        let dir = choice(&i.controls[1]);
        assert_eq!(dir.label, "DIRECTION");
        assert_eq!(dir.options.iter().map(|b| (b.text.as_str(), b.active)).collect::<Vec<_>>(), [("AUTO", false), ("COUNTER-CLOCKWISE", true), ("CLOCKWISE", false)]);
        assert_eq!(dir.options[2].action, set(at, 1, Value::Word("cw".into())));
        // The rule's block: the second of four, every button on, the note.
        let Control::Buttons { label, buttons } = &i.controls[3] else { panic!("{:?}", i.controls[3]) };
        assert_eq!(label, "RULE 2 OF 4");
        assert!(buttons.iter().all(|b| b.action.is_some()));
        assert_eq!(buttons[0].action, Some(edit(Edit::MoveRule { rule, to: 0 })));
        assert_eq!(i.controls[4], Control::Note { target: NoteTarget::Rule(rule), text: "dodge while the shields recharge (no use against homing missiles)".into() });
        // A value left out shows its default and has nothing to reset.
        let kite = BehaviorEditor::from_text(&data::base_content().behavior_nodes, "always -> kite").unwrap();
        let k = inspect(&kite, &Selection::Node(NodeRef::Action(kite.rule_at(0).unwrap().id)), &cx()).unwrap();
        let range = number(&k.controls[0]);
        assert_eq!((range.value.as_str(), range.default, range.reset.is_none()), ("90%", true, true));
        // The only rule cannot go, and the inspector says why.
        let Control::Buttons { buttons, .. } = &k.controls[2] else { panic!() };
        assert_eq!(buttons[3].action, None);
        assert_eq!(buttons[0].action, None);
        assert!(matches!(&k.controls[3], Control::Hint(h) if h.starts_with("A BEHAVIOR NEEDS AT LEAST ONE RULE")));
    }

    #[test]
    fn a_value_condition_gets_not_the_comparator_and_its_threshold() {
        let ed = skirmisher();
        // `hull < 25%`
        let c = ed.rule_at(0).unwrap().conds[0];
        let i = inspect(&ed, &Selection::Node(NodeRef::Cond(c)), &cx()).unwrap();
        assert_eq!(i.kicker, "CONDITION  ·  VALUE");
        assert_eq!(i.controls[0], Control::Switch(SwitchRow { label: "NOT".into(), hint: "TRUE WHEN IT DOES NOT HOLD".into(), on: false, action: Some(edit(Edit::ToggleNot(c))) }));
        let cmp = choice(&i.controls[1]);
        assert_eq!(cmp.options.iter().filter(|o| o.active).map(|o| o.text.as_str()).collect::<Vec<_>>(), ["<"]);
        let pct = |x| Value::Number(x, Some(Unit::Percent));
        assert_eq!(cmp.options[3].action, Some(edit(Edit::SetCompare { cond: c, op: CmpOp::Ge, rhs: pct(25.0) })));
        let than = number(&i.controls[2]);
        assert_eq!((than.label.as_str(), than.value.as_str(), than.range.as_str()), ("THAN", "25%", "0 – 100 %"));
        assert_eq!(than.minus, Some(edit(Edit::SetCompare { cond: c, op: CmpOp::Lt, rhs: pct(20.0) })));
        assert_eq!(than.plus, Some(edit(Edit::SetCompare { cond: c, op: CmpOp::Lt, rhs: pct(30.0) })));
        assert_eq!(than.slider.unwrap().scale.share(25.0), 0.25);
        // REPLACE and REMOVE; armed, REPLACE is active and the hint says how.
        let Control::Buttons { buttons, .. } = &i.controls[3] else { panic!() };
        assert_eq!((buttons[0].text.as_str(), buttons[0].active, buttons[1].action.clone()), ("REPLACE", false, Some(edit(Edit::RemoveCondition(c)))));
        let armed = inspect(&ed, &Selection::Node(NodeRef::Cond(c)), &Context { replacing: Some(NodeRef::Cond(c)), ..cx() }).unwrap();
        let Control::Buttons { buttons, .. } = &armed.controls[3] else { panic!() };
        assert!(buttons[0].active);
        assert!(matches!(&armed.controls[4], Control::Hint(_)));
        // `cycle(9s) >= 5s`: the threshold's slider ends at the period.
        let ed2 = BehaviorEditor::from_text(&data::base_content().behavior_nodes, "when not cycle(9s) >= 5s -> hold").unwrap();
        let c2 = ed2.rule_at(0).unwrap().conds[0];
        let i2 = inspect(&ed2, &Selection::Node(NodeRef::Cond(c2)), &cx()).unwrap();
        assert!(matches!(&i2.controls[0], Control::Switch(s) if s.on));
        assert_eq!(number(&i2.controls[2]).range, "0 – 9 S");
        let period = number(&i2.controls[3]);
        assert_eq!((period.label.as_str(), period.range.as_str()), ("PERIOD", "1 – 600 S"));
    }

    #[test]
    fn the_target_line_and_a_rule() {
        let ed = BehaviorEditor::from_text(&data::base_content().behavior_nodes, "when hull < 30% -> flee\nalways -> hold").unwrap();
        // No target line: `nearest` by default, every selector a pill.
        let t = inspect(&ed, &Selection::Node(NodeRef::Target), &cx()).unwrap();
        assert_eq!((t.kicker.as_str(), t.name.as_str()), ("TARGET  ·  SELECTOR", "Nearest"));
        let sel = choice(&t.controls[0]);
        assert!(sel.default);
        assert_eq!(sel.options.len(), 5);
        assert_eq!(sel.options[1].action, Some(edit(Edit::ReplaceNode { at: NodeRef::Target, node: "weakest".into() })));
        let aim = choice(&t.controls[1]);
        assert_eq!(aim.label, "AIM AT");
        assert_eq!(aim.options[1].action, set(NodeRef::Target, 0, Value::Word("engines".into())));
        // No default to go back to, only the note.
        assert_eq!(t.controls[2], Control::Note { target: NoteTarget::Target, text: String::new() });
        // A rule: its line, the hint, its block.
        let r = ed.rule_at(1).unwrap().id;
        let i = inspect(&ed, &Selection::Rule(r), &Context { slots: 1, ..cx() }).unwrap();
        assert_eq!((i.name.as_str(), i.summary.as_str()), ("Rule 2", "always -> hold"));
        assert_eq!(i.kicker, "RULE  ·  NO RULE SLOT: DOES NOT RUN");
        let Control::Buttons { label, buttons } = &i.controls[1] else { panic!() };
        assert_eq!(label, "RULE 2 OF 2  ·  NO SLOT");
        assert_eq!((buttons[0].action.is_some(), buttons[1].action.is_some()), (true, false));
        // Below an `always` rule whose action needs no target: never reached.
        let mut shadowed = ed.clone();
        shadowed.move_rule(r, 0).unwrap();
        let below = shadowed.rule_at(1).unwrap().id;
        let i = inspect(&shadowed, &Selection::Rule(below), &cx()).unwrap();
        assert_eq!(i.kicker, "RULE  ·  NEVER REACHED: AN ALWAYS RULE ABOVE MATCHES FIRST");
        let Control::Buttons { label, .. } = &i.controls[1] else { panic!() };
        assert_eq!(label, "RULE 2 OF 2  ·  NEVER REACHED");
        // Gone: nothing.
        let mut gone = ed.clone();
        gone.delete_rule(r).unwrap();
        assert!(inspect(&gone, &Selection::Rule(r), &cx()).is_none());
        assert!(!Selection::Rule(r).exists(&gone));
    }

    #[test]
    fn a_palette_pick_shows_its_defaults_read_only() {
        let ed = skirmisher();
        let k = inspect(&ed, &Selection::Vocabulary("kite".into()), &cx()).unwrap();
        assert_eq!(k.name, "Keep distance");
        let range = number(&k.controls[0]);
        assert_eq!((range.value.as_str(), range.default), ("90%", true));
        assert!(range.minus.is_none() && range.plus.is_none() && range.reset.is_none());
        assert!(range.slider.unwrap().target.is_none());
        assert!(range.units.iter().all(|b| b.action.is_none()));
        assert!(matches!(k.controls.last(), Some(Control::Hint(h)) if h.contains("ITS ACTION")));
        assert!(inspect(&ed, &Selection::Vocabulary("warp".into()), &cx()).is_none());
        // Nothing selected: the behavior.
        let b = behavior_inspection("Skirmisher", data::base_content().behaviors.get("skirmisher"), 4);
        assert_eq!(b.kicker, "BEHAVIOR  ·  4 RULES");
        assert!(b.summary.starts_with("Attack nose first"));
    }

    #[test]
    fn the_palette_hint_says_where_a_pick_goes() {
        let ed = skirmisher();
        let r = ed.rule_at(2).unwrap();
        assert_eq!(palette_hint(&ed, None, None), "ADD NODE");
        assert_eq!(palette_hint(&ed, Some(&Selection::Rule(r.id)), None), "ADD NODE  ·  TO RULE 3");
        assert_eq!(palette_hint(&ed, Some(&Selection::Node(NodeRef::Cond(r.conds[0]))), None), "ADD NODE  ·  TO RULE 3");
        assert_eq!(palette_hint(&ed, Some(&Selection::Node(NodeRef::Target)), None), "ADD NODE  ·  A SELECTOR SETS THE TARGET");
        let at = NodeRef::Cond(r.conds[0]);
        assert_eq!(palette_hint(&ed, Some(&Selection::Node(at)), Some(at)), "REPLACE CLOSING IN  ·  PICK A CONDITION");
    }

    #[test]
    fn the_palette_lists_the_vocabulary_by_role() {
        let content = data::base_content();
        let all = palette(&content.behavior_nodes, ListKind::Behavior, "");
        assert_eq!(all.len(), content.behavior_nodes.iter().filter(|n| !n.pilot).count());
        let roles: Vec<NodeRole> = all.iter().map(|e| e.role).collect();
        let mut sorted = roles.clone();
        sorted.sort();
        assert_eq!(roles, sorted, "conditions, then actions, then targets");
        let rhai = all.iter().find(|e| e.id == "closing_in").unwrap();
        assert!(rhai.rhai);
        assert_eq!(rhai.label, "CLOSING IN  ·  SCRIPT");
        // Search by name or id, any case.
        let ids: Vec<String> = palette(&content.behavior_nodes, ListKind::Behavior, "Target").into_iter().map(|e| e.id).collect();
        assert!(ids.contains(&"target_in_range".to_string()) && ids.contains(&"face".to_string()), "{ids:?}");
        let ids: Vec<String> = palette(&content.behavior_nodes, ListKind::Behavior, "keep").into_iter().map(|e| e.id).collect();
        assert_eq!(ids, ["kite"]);
        assert!(palette(&content.behavior_nodes, ListKind::Behavior, "zzz").is_empty());
        // Pilot assists (GDR-0088): the assists' actions only, every condition (the pilot's too), no target selector
        // (they act on the pilot's lock).
        let assists = palette(&content.behavior_nodes, ListKind::Assists, "");
        let actions: Vec<&str> = assists.iter().filter(|e| e.role == NodeRole::Action).map(|e| e.id.as_str()).collect();
        assert_eq!(actions, ["auto_lock", "brake", "hold_station", "avoid_collisions", "auto_flare", "auto_fire"]);
        assert!(!assists.iter().any(|e| e.role == NodeRole::Target));
        assert!(assists.iter().any(|e| e.id == "idle") && assists.iter().any(|e| e.id == "hull"));
        assert!(!all.iter().any(|e| e.id == "idle"));
    }

    /// A rule of the pilot assists: no rule slots and nothing "never reached" (each rule decides its own channel),
    /// and the last one can go.
    #[test]
    fn assist_rules_have_no_slots_and_the_last_can_go() {
        let content = data::base_content();
        let program = data::behavior::Program::parse("always -> auto_lock(nearest, toggle1)\nalways -> brake\n").unwrap();
        let ed = BehaviorEditor::open_as(&content.behavior_nodes, &program, ListKind::Assists).unwrap();
        let second = ed.rule_at(1).unwrap().id;
        let i = inspect(&ed, &Selection::Rule(second), &Context { slots: 0, ..cx() }).unwrap();
        assert_eq!(i.kicker, "RULE  ·  DECIDES ITS CHANNEL UNLESS A RULE ABOVE DOES");
        let Control::Buttons { label, buttons } = &i.controls[1] else { panic!("{:?}", i.controls) };
        assert_eq!(label, "RULE 2 OF 2");
        assert!(buttons.iter().find(|b| b.text == "DELETE").is_some_and(|b| b.action.is_some()));
        let lock = inspect(&ed, &Selection::Node(NodeRef::Action(ed.rule_at(0).unwrap().id)), &cx()).unwrap();
        assert_eq!(lock.kicker, "ACTION  ·  PILOT ASSIST");
        // Its pick (the target selectors that suit a pilot), toggle and start are pills, set like any other value.
        let choice = |label: &str| {
            lock.controls.iter().find_map(|c| match c {
                Control::Choice(ch) if ch.label.starts_with(label) => Some(ch.options.iter().map(|o| o.text.as_str()).collect::<Vec<_>>()),
                _ => None,
            })
        };
        assert_eq!(choice("PICKS"), Some(vec!["NEAREST", "WEAKEST", "BIGGEST THREAT", "ATTACKER"]), "{:?}", lock.controls);
        assert!(choice("TOGGLE").is_some_and(|t| t.len() == 5), "{:?}", lock.controls);
        // There is no target node to inspect: the assists act on the lock.
        assert!(inspect(&ed, &Selection::Node(NodeRef::Target), &cx()).is_none());
        // Hold station (step 4): what it keeps its place at is a pill pair of its own, before the toggle and the start.
        let program = data::behavior::Program::parse("always -> hold_station(ally)\n").unwrap();
        let ed = BehaviorEditor::open_as(&content.behavior_nodes, &program, ListKind::Assists).unwrap();
        let hold = inspect(&ed, &Selection::Node(NodeRef::Action(ed.rule_at(0).unwrap().id)), &cx()).unwrap();
        let rows: Vec<(&str, Vec<&str>)> = hold
            .controls
            .iter()
            .filter_map(|c| match c {
                Control::Choice(ch) => Some((ch.label.as_str(), ch.options.iter().filter(|o| o.active).map(|o| o.text.as_str()).collect())),
                _ => None,
            })
            .collect();
        assert_eq!(rows.len(), 3, "{rows:?}");
        assert!(rows[0].0.starts_with("STATION") && rows[0].1 == ["ALLY"], "{rows:?}");
        assert!(rows[1].0.starts_with("TOGGLE") && rows[2].0.starts_with("AT START"), "{rows:?}");
    }
}
