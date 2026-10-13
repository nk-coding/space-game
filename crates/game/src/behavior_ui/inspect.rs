//! What the right column shows, as plain data: the inspector for the selected node (its role and kind, name,
//! summary and parameters as read-only rows; with nothing selected, the behavior itself) and the ADD NODE
//! palette (the content vocabulary, filtered by the search). The screen (`screen.rs`) draws these.

use data::behavior::{Call, Cond, MAX_REACH_PERCENT, Program, Unit, Value, parse_value};
use data::{Behavior, BehaviorNode, NodeParam, NodeRole, ParamType, Quantity, Table};

use super::graph::{DEFAULT_TARGET, NodeRef};

/// What the inspector shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Selection {
    /// A node of the program.
    Node(NodeRef),
    /// A vocabulary entry picked in the palette (content id), with its default values.
    Vocabulary(String),
}

/// The inspector's content.
#[derive(Clone, Debug, PartialEq)]
pub struct Inspection {
    /// The tracked line above the name: role and kind (`ACTION · MANEUVER`).
    pub kicker: String,
    pub name: String,
    /// Colours the name: conditions violet, actions teal, Rhai gold; `None` for the behavior itself (white).
    pub role: Option<NodeRole>,
    pub rhai: bool,
    pub summary: String,
    pub rows: Vec<Row>,
    /// The rule's `# note`.
    pub note: Option<String>,
    /// A Rhai condition's script.
    pub script: Option<String>,
}

/// One read-only parameter row.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// A number: `label` (`RANGE`), the value as written (`67%`), its range (`0 – 5000 M`) and where the value
    /// sits in that range (a bar), when the range is known.
    Value { label: String, value: String, range: String, share: Option<f32>, default: bool },
    /// One of a few words: the options' names, the chosen one marked.
    Choice { label: String, options: Vec<(String, bool)>, default: bool },
}

/// The behavior itself (nothing selected): its name, its content summary and a hint.
pub fn behavior_inspection(name: &str, source: Option<&Behavior>, program: &Program) -> Inspection {
    let rules = program.rules.len();
    Inspection {
        kicker: format!("BEHAVIOR  ·  {rules} RULE{}", if rules == 1 { "" } else { "S" }),
        name: name.to_string(),
        role: None,
        rhai: false,
        summary: source.map_or_else(String::new, |b| b.summary.clone()),
        rows: Vec::new(),
        note: None,
        script: None,
    }
}

/// The inspector for a selection; `None` if the program or the vocabulary does not have it.
pub fn inspect(program: &Program, nodes: &Table<BehaviorNode>, sel: &Selection) -> Option<Inspection> {
    match sel {
        Selection::Vocabulary(id) => {
            let node = nodes.get(id)?;
            Some(of_node(node, &Call { name: node.id.clone(), args: Vec::new(), col: Default::default() }, None, None))
        }
        Selection::Node(NodeRef::Target) => {
            let call = program.target.as_ref().map_or_else(
                || Call { name: DEFAULT_TARGET.into(), args: Vec::new(), col: Default::default() },
                |t| t.call.clone(),
            );
            let note = program.target.as_ref().and_then(|t| t.note.clone());
            Some(of_node(nodes.get(&call.name)?, &call, None, note))
        }
        Selection::Node(NodeRef::Cond { rule, cond }) => {
            let r = program.rules.get(*rule)?;
            let c = r.when.get(*cond)?;
            Some(of_node(nodes.get(&c.call.name)?, &c.call, Some(c), r.note.clone()))
        }
        Selection::Node(NodeRef::Action { rule }) => {
            let r = program.rules.get(*rule)?;
            Some(of_node(nodes.get(&r.action.name)?, &r.action, None, r.note.clone()))
        }
    }
}

fn of_node(node: &BehaviorNode, call: &Call, cond: Option<&Cond>, note: Option<String>) -> Inspection {
    let kind = match node.role {
        NodeRole::Condition if node.script.is_some() => "CONDITION  ·  RHAI",
        NodeRole::Condition if node.value.is_some() => "CONDITION  ·  VALUE",
        NodeRole::Condition => "CONDITION  ·  FLAG",
        NodeRole::Action => "ACTION  ·  MANEUVER",
        NodeRole::Target => "TARGET  ·  SELECTOR",
    };
    let mut rows = Vec::new();
    if node.role == NodeRole::Condition {
        let not = cond.is_some_and(|c| c.not);
        rows.push(Row::Choice { label: "WHEN".into(), options: vec![("TRUE".into(), !not), ("NOT TRUE".into(), not)], default: !not });
    }
    if let (Some(q), Some(cmp)) = (node.value, cond.and_then(|c| c.cmp.as_ref())) {
        let ops = ["<", "<=", ">", ">="];
        rows.push(Row::Choice {
            label: "COMPARE".into(),
            options: ops.iter().map(|o| (o.to_string(), *o == cmp.op.symbol())).collect(),
            default: false,
        });
        let (range, share) = compare_range(q, &cmp.rhs.value);
        rows.push(Row::Value { label: "THAN".into(), value: cmp.rhs.to_string(), range, share, default: false });
    }
    for (k, p) in node.params.iter().enumerate() {
        let written = call.args.get(k).map(|a| a.value.clone());
        let default = written.is_none();
        let Some(value) = written.or_else(|| p.default.as_deref().and_then(|d| parse_value(d).ok())) else {
            // A required parameter left out (the checks make this impossible for a valid program).
            rows.push(Row::Value { label: p.name.to_uppercase(), value: "—".into(), range: String::new(), share: None, default: false });
            continue;
        };
        rows.push(param_row(p, &value, default));
    }
    Inspection {
        kicker: kind.into(),
        name: node.name.clone(),
        role: Some(node.role),
        rhai: node.script.is_some(),
        summary: node.summary.clone(),
        rows,
        note,
        script: node.script.clone(),
    }
}

fn param_row(p: &NodeParam, value: &Value, default: bool) -> Row {
    let label = p.name.to_uppercase();
    match &p.ty {
        ParamType::Choice(options) => {
            let chosen = match value {
                Value::Word(w) => Some(w.as_str()),
                Value::Number(..) => None,
            };
            Row::Choice { label, options: options.iter().map(|o| (o.name.to_uppercase(), Some(o.id.as_str()) == chosen)).collect(), default }
        }
        ty => {
            let x = match value {
                Value::Number(x, _) => *x,
                Value::Word(_) => 0.0,
            };
            let reach = matches!((ty, value), (ParamType::Distance, Value::Number(_, Some(Unit::Percent))));
            let (range, share) = if reach {
                (format!("0 – {MAX_REACH_PERCENT} % OF WEAPON REACH"), None)
            } else {
                let unit = unit_word(ty.quantity());
                match (p.min, p.max) {
                    (Some(lo), Some(hi)) if hi > lo => (format!("{lo} – {hi}{unit}"), Some(((x - lo) / (hi - lo)).clamp(0.0, 1.0))),
                    (Some(lo), None) => (format!("FROM {lo}{unit}"), None),
                    (None, Some(hi)) => (format!("UP TO {hi}{unit}"), None),
                    _ => (String::new(), None),
                }
            };
            Row::Value { label, value: value.to_string(), range, share, default }
        }
    }
}

/// The range a value condition is compared in: percentages 0–100 %; other quantities have no fixed range.
fn compare_range(q: Quantity, v: &Value) -> (String, Option<f32>) {
    match (q, v) {
        (Quantity::Percent, Value::Number(x, _)) => ("0 – 100 %".into(), Some((x / 100.0).clamp(0.0, 1.0))),
        (Quantity::Distance, Value::Number(_, Some(Unit::Percent))) => ("% OF WEAPON REACH".into(), None),
        _ => (String::new(), None),
    }
}

/// The unit after a range (` M`, ` %`, ` S`, nothing for counts).
fn unit_word(q: Option<Quantity>) -> &'static str {
    match q {
        Some(Quantity::Distance) => " M",
        Some(Quantity::Percent) => " %",
        Some(Quantity::Seconds) => " S",
        _ => "",
    }
}

/// One entry of the ADD NODE palette.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteEntry {
    pub id: String,
    /// The pill's text: the name in capitals, `· RHAI` after a Rhai condition.
    pub label: String,
    pub role: NodeRole,
    pub rhai: bool,
}

/// The vocabulary for the palette: conditions, then actions, then target selectors, each in content order;
/// with a search, only the nodes whose name or id contains it (ignoring case).
pub fn palette(nodes: &Table<BehaviorNode>, search: &str) -> Vec<PaletteEntry> {
    let search = search.trim().to_lowercase();
    let wanted = |n: &BehaviorNode| search.is_empty() || n.name.to_lowercase().contains(&search) || n.id.contains(&search);
    [NodeRole::Condition, NodeRole::Action, NodeRole::Target]
        .into_iter()
        .flat_map(|role| nodes.iter().filter(move |n| n.role == role))
        .filter(|n| wanted(n))
        .map(|n| PaletteEntry {
            id: n.id.clone(),
            label: if n.script.is_some() { format!("{}  ·  RHAI", n.name.to_uppercase()) } else { n.name.to_uppercase() },
            role: n.role,
            rhai: n.script.is_some(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skirmisher(content: &data::Content) -> Program {
        content.behaviors.get("skirmisher").unwrap().program().unwrap()
    }

    #[test]
    fn an_action_shows_its_values_with_ranges_and_choices() {
        let content = data::base_content();
        let p = skirmisher(&content);
        // `orbit(80%, ccw)`
        let i = inspect(&p, &content.behavior_nodes, &Selection::Node(NodeRef::Action { rule: 1 })).unwrap();
        assert_eq!(i.kicker, "ACTION  ·  MANEUVER");
        assert_eq!(i.name, "Orbit");
        assert_eq!(i.role, Some(NodeRole::Action));
        assert_eq!(i.note.as_deref(), Some("dodge while the shields recharge"));
        assert_eq!(i.rows.len(), 2);
        assert_eq!(
            i.rows[0],
            Row::Value { label: "RADIUS".into(), value: "80%".into(), range: "0 – 1000 % OF WEAPON REACH".into(), share: None, default: false }
        );
        let Row::Choice { label, options, default } = &i.rows[1] else { panic!("{:?}", i.rows[1]) };
        assert_eq!(label, "DIRECTION");
        assert!(!default);
        assert_eq!(options, &[("AUTO".to_string(), false), ("COUNTER-CLOCKWISE".to_string(), true), ("CLOCKWISE".to_string(), false)]);
        // `flee` has no values.
        let flee = inspect(&p, &content.behavior_nodes, &Selection::Node(NodeRef::Action { rule: 0 })).unwrap();
        assert!(flee.rows.is_empty());
    }

    #[test]
    fn a_condition_shows_not_the_comparison_and_defaults() {
        let content = data::base_content();
        let p = skirmisher(&content);
        // `hull < 25%`
        let hull = inspect(&p, &content.behavior_nodes, &Selection::Node(NodeRef::Cond { rule: 0, cond: 0 })).unwrap();
        assert_eq!(hull.kicker, "CONDITION  ·  VALUE");
        assert_eq!(hull.rows[0], Row::Choice { label: "WHEN".into(), options: vec![("TRUE".into(), true), ("NOT TRUE".into(), false)], default: true });
        let Row::Choice { options, .. } = &hull.rows[1] else { panic!() };
        assert_eq!(options.iter().filter(|o| o.1).map(|o| o.0.as_str()).collect::<Vec<_>>(), ["<"]);
        assert_eq!(hull.rows[2], Row::Value { label: "THAN".into(), value: "25%".into(), range: "0 – 100 %".into(), share: Some(0.25), default: false });
        // `not target_in_range(100%)`
        let tir = inspect(&p, &content.behavior_nodes, &Selection::Node(NodeRef::Cond { rule: 4, cond: 1 })).unwrap();
        assert_eq!(tir.kicker, "CONDITION  ·  FLAG");
        assert_eq!(tir.rows[0], Row::Choice { label: "WHEN".into(), options: vec![("TRUE".into(), false), ("NOT TRUE".into(), true)], default: false });
        // `closing_in(2s)`: Rhai, its script, the time in its content range.
        let ci = inspect(&p, &content.behavior_nodes, &Selection::Node(NodeRef::Cond { rule: 4, cond: 0 })).unwrap();
        assert_eq!(ci.kicker, "CONDITION  ·  RHAI");
        assert!(ci.rhai && ci.script.as_deref().unwrap().contains("target.closing"));
        assert_eq!(ci.rows[1], Row::Value { label: "TIME".into(), value: "2s".into(), range: "0 – 60 S".into(), share: Some(2.0 / 60.0), default: false });
        // A value left out shows the default, marked so.
        let p2 = Program::parse("when target_in_range -> hold").unwrap();
        let d = inspect(&p2, &content.behavior_nodes, &Selection::Node(NodeRef::Cond { rule: 0, cond: 0 })).unwrap();
        assert_eq!(d.rows[1], Row::Value { label: "RANGE".into(), value: "100%".into(), range: "0 – 1000 % OF WEAPON REACH".into(), share: None, default: true });
        let m = Program::parse("when target_in_range(45m) -> hold").unwrap();
        let m = inspect(&m, &content.behavior_nodes, &Selection::Node(NodeRef::Cond { rule: 0, cond: 0 })).unwrap();
        assert_eq!(m.rows[1], Row::Value { label: "RANGE".into(), value: "45m".into(), range: "0 – 5000 M".into(), share: Some(0.009), default: false });
    }

    #[test]
    fn the_target_and_palette_picks() {
        let content = data::base_content();
        let p = skirmisher(&content);
        let t = inspect(&p, &content.behavior_nodes, &Selection::Node(NodeRef::Target)).unwrap();
        assert_eq!((t.kicker.as_str(), t.name.as_str()), ("TARGET  ·  SELECTOR", "Nearest"));
        let Row::Choice { label, options, default } = &t.rows[0] else { panic!() };
        assert_eq!(label, "AIM AT");
        assert!(default);
        assert_eq!(options.iter().find(|o| o.1).unwrap().0, "CENTRE");
        // A palette pick shows the node with its defaults.
        let k = inspect(&p, &content.behavior_nodes, &Selection::Vocabulary("kite".into())).unwrap();
        assert_eq!(k.name, "Keep distance");
        assert!(matches!(&k.rows[0], Row::Value { value, default: true, .. } if value == "90%"));
        // Gone or unknown: nothing.
        assert!(inspect(&p, &content.behavior_nodes, &Selection::Node(NodeRef::Action { rule: 9 })).is_none());
        assert!(inspect(&p, &content.behavior_nodes, &Selection::Vocabulary("warp".into())).is_none());
        // Nothing selected: the behavior.
        let b = behavior_inspection("Skirmisher", content.behaviors.get("skirmisher"), &p);
        assert_eq!(b.kicker, "BEHAVIOR  ·  6 RULES");
        assert!(b.summary.starts_with("Attack and strafe"));
    }

    #[test]
    fn the_palette_lists_the_vocabulary_by_role() {
        let content = data::base_content();
        let all = palette(&content.behavior_nodes, "");
        assert_eq!(all.len(), content.behavior_nodes.iter().count());
        let roles: Vec<NodeRole> = all.iter().map(|e| e.role).collect();
        let mut sorted = roles.clone();
        sorted.sort();
        assert_eq!(roles, sorted, "conditions, then actions, then targets");
        let rhai = all.iter().find(|e| e.id == "closing_in").unwrap();
        assert!(rhai.rhai);
        assert_eq!(rhai.label, "CLOSING IN  ·  RHAI");
        // Search by name or id, any case.
        let ids: Vec<String> = palette(&content.behavior_nodes, "Target").into_iter().map(|e| e.id).collect();
        assert!(ids.contains(&"target_in_range".to_string()) && ids.contains(&"face".to_string()), "{ids:?}");
        let ids: Vec<String> = palette(&content.behavior_nodes, "keep").into_iter().map(|e| e.id).collect();
        assert_eq!(ids, ["kite"]);
        assert!(palette(&content.behavior_nodes, "zzz").is_empty());
    }
}
