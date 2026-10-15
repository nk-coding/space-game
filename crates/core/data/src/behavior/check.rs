//! Behavior programs against the vocabulary (the `behavior_nodes` content): typed values (`Amount`) from what a
//! rule wrote, the checks that give every error a line and column, suggestions for misspelt words.

use super::*;

// ---------------------------------------------------------------------------------------------------------
// Typed values

/// A value resolved against its parameter or quantity type.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Amount {
    /// Metres.
    Metres(f32),
    /// A share of the ship's weapon reach (`67%` on a distance = 0.67).
    Reach(f32),
    /// A share (`30%` = 0.3).
    Share(f32),
    Seconds(f32),
    Count(u32),
    /// Index into the parameter's options.
    Choice(usize),
}

/// Resolves a number against a quantity: distances take metres (`m` or bare) or `%` of weapon reach;
/// percentages need `%`; seconds take `s` or bare; counts are bare whole numbers.
pub fn quantity_amount(v: &Value, q: Quantity) -> Result<Amount, String> {
    let (x, unit) = match v {
        Value::Number(x, u) => (*x, *u),
        Value::Word(w) => return Err(format!("expected {}, found the word `{w}`", q.describe())),
    };
    match (q, unit) {
        (Quantity::Distance, None | Some(Unit::Metres)) => Ok(Amount::Metres(x)),
        (Quantity::Distance, Some(Unit::Percent)) => Ok(Amount::Reach(x / 100.0)),
        (Quantity::Percent, Some(Unit::Percent)) => Ok(Amount::Share(x / 100.0)),
        (Quantity::Percent, None) => Err(format!("write a percentage with `%`, e.g. {x}%")),
        (Quantity::Seconds, None | Some(Unit::Seconds)) => Ok(Amount::Seconds(x)),
        (Quantity::Count, None) => {
            if x.fract() != 0.0 || !(0.0..=1.0e6).contains(&x) {
                Err(format!("expected a whole number, found {x}"))
            } else {
                Ok(Amount::Count(x as u32))
            }
        }
        (_, Some(u)) => Err(format!(
            "{} cannot be in `{}`; {}",
            q.describe(),
            u.suffix(),
            q.unit_hint()
        )),
    }
}

/// Resolves an argument against its parameter type (numbers by quantity, words against the options).
pub fn param_amount(v: &Value, ty: &ParamType) -> Result<Amount, String> {
    match ty {
        ParamType::Choice(options) => match v {
            Value::Word(w) => options
                .iter()
                .position(|o| &o.id == w)
                .map(Amount::Choice)
                .ok_or_else(|| {
                    let ids: Vec<&str> = options.iter().map(|o| o.id.as_str()).collect();
                    format!("`{w}` is not an option; choose one of: {}", ids.join(", "))
                }),
            Value::Number(..) => {
                let ids: Vec<&str> = options.iter().map(|o| o.id.as_str()).collect();
                Err(format!("expected one of: {}", ids.join(", ")))
            }
        },
        other => quantity_amount(v, other.quantity().expect("numeric type")),
    }
}

/// The number as written (percent for `%`, metres, seconds, count) for range checks against `min`/`max`.
pub(super) fn written_number(v: &Value) -> f32 {
    match v {
        Value::Number(x, _) => *x,
        Value::Word(_) => 0.0,
    }
}

/// Checks a parameter value: type, then range (`min`/`max` apply to the number as written in the
/// parameter's own unit; reach-relative distances must lie within 0–1000%).
pub fn check_param_value(v: &Value, p: &NodeParam) -> Result<Amount, String> {
    let a = param_amount(v, &p.ty)?;
    let x = written_number(v);
    match a {
        Amount::Choice(_) => {}
        Amount::Reach(_) => {
            if !(0.0..=MAX_REACH_PERCENT).contains(&x) {
                return Err(format!(
                    "{} must be between 0% and {MAX_REACH_PERCENT}% of the weapon reach, got {x}%",
                    p.name
                ));
            }
        }
        _ => {
            if let Some(min) = p.min
                && x < min
            {
                return Err(format!(
                    "{} must be at least {min}{}, got {v}",
                    p.name,
                    p.ty.unit_suffix()
                ));
            }
            if let Some(max) = p.max
                && x > max
            {
                return Err(format!(
                    "{} must be at most {max}{}, got {v}",
                    p.name,
                    p.ty.unit_suffix()
                ));
            }
        }
    }
    Ok(a)
}

// ---------------------------------------------------------------------------------------------------------
// Checking against the vocabulary

/// Which rule list a program is (GDR-0088): a behavior, which flies a ship nobody pilots, or a design's PILOT
/// ASSISTS, which help the player fly it. Both are written in the same format with the same conditions; the nodes
/// marked `pilot` (`BehaviorNode::pilot`) belong to assist lists only, and an assist list's actions are those. An
/// assist list has no target line: its assists act on the pilot's lock, which `auto_lock` can set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ListKind {
    #[default]
    Behavior,
    Assists,
}

impl ListKind {
    /// May this node be written in such a list?
    pub fn allows(self, n: &BehaviorNode) -> bool {
        match self {
            ListKind::Behavior => !n.pilot,
            ListKind::Assists => n.pilot || n.role == NodeRole::Condition,
        }
    }

    /// A list needs a rule: a behavior does (a ship must do something), an assist list doesn't (none: the ship coasts).
    pub fn needs_a_rule(self) -> bool {
        self == ListKind::Behavior
    }
}

/// Parses a behavior text and checks it against the vocabulary: the program, or every syntax and vocabulary
/// error with its text line and column (what a text view shows as live errors).
pub fn check_text(text: &str, nodes: &Table<BehaviorNode>) -> Result<Program, Vec<TextError>> {
    check_text_as(text, nodes, ListKind::Behavior)
}

/// [`check_text`] for a list of either kind.
pub fn check_text_as(text: &str, nodes: &Table<BehaviorNode>, kind: ListKind) -> Result<Program, Vec<TextError>> {
    let (program, lines) = Program::parse_with_lines(text)?;
    let errors = check_list(&program, nodes, kind);
    if errors.is_empty() { Ok(program) } else { Err(errors.into_iter().map(|e| lines.to_text_error(e)).collect()) }
}

/// Checks a parsed program against the behavior nodes: every name exists and has the right role, value
/// conditions are compared (and flag conditions are not), arguments fit their parameters, required ones are
/// present, numbers are in range. Returns every problem found.
pub fn check_program(p: &Program, nodes: &Table<BehaviorNode>) -> Vec<LineError> {
    check_list(p, nodes, ListKind::Behavior)
}

/// [`check_program`] for a design's pilot assists: no rule at all is fine, and the actions are the assists'.
pub fn check_assists(p: &Program, nodes: &Table<BehaviorNode>) -> Vec<LineError> {
    check_list(p, nodes, ListKind::Assists)
}

/// [`check_program`] for a list of either kind.
pub fn check_list(p: &Program, nodes: &Table<BehaviorNode>, kind: ListKind) -> Vec<LineError> {
    let mut out = Vec::new();
    if p.rules.is_empty() && kind.needs_a_rule() {
        out.push(LineError {
            line: LineRef::Whole,
            col: 1,
            message: "a behavior needs at least one rule".into(),
        });
    }
    if p.rules.len() > MAX_RULES {
        out.push(too_many_rules());
    }
    match &p.target {
        Some(t) if kind == ListKind::Assists => out.push(LineError {
            line: LineRef::Target,
            col: t.call.col.0,
            message: "pilot assists have no target line: they act on the ship you lock".into(),
        }),
        Some(t) => {
            check_printed_length(t, LineRef::Target, &mut out);
            check_call(&t.call, NodeRole::Target, nodes, kind, LineRef::Target, &mut out);
        }
        None => {}
    }
    for (i, r) in p.rules.iter().enumerate() {
        let at = LineRef::Rule(i);
        check_printed_length(r, at, &mut out);
        for c in &r.when {
            let Some(node) = check_call(&c.call, NodeRole::Condition, nodes, kind, at, &mut out) else {
                continue;
            };
            match (node.value, &c.cmp) {
                (Some(q), None) => out.push(LineError {
                    line: at,
                    col: c.call.col.0,
                    message: format!(
                        "`{}` is a value ({}): compare it, e.g. `{} < {}`",
                        node.id,
                        q.describe(),
                        c.call,
                        q.example()
                    ),
                }),
                (None, Some(cmp)) => out.push(LineError {
                    line: at,
                    col: cmp.rhs.col.0,
                    message: format!("`{}` is true or false; it cannot be compared", node.id),
                }),
                (Some(q), Some(cmp)) => match quantity_amount(&cmp.rhs.value, q) {
                    Err(m) => out.push(LineError {
                        line: at,
                        col: cmp.rhs.col.0,
                        message: m,
                    }),
                    Ok(_) if written_number(&cmp.rhs.value) < 0.0 => out.push(LineError {
                        line: at,
                        col: cmp.rhs.col.0,
                        message: "compare with a number of at least 0".into(),
                    }),
                    Ok(_) => {}
                },
                (None, None) => {}
            }
        }
        check_call(&r.action, NodeRole::Action, nodes, kind, at, &mut out);
    }
    out
}

pub(super) fn too_many_rules() -> LineError {
    LineError {
        line: LineRef::Whole,
        col: 1,
        message: format!("at most {MAX_RULES} rules"),
    }
}

/// The canonical print of a line (spaces around `<`, `->`, `,` and `#`, `when` for `if`, ...) may be longer than
/// what was typed; it must stay within [`MAX_LINE_CHARS`] too, so the printed text always parses back.
pub(super) fn check_printed_length(line: &impl std::fmt::Display, at: LineRef, out: &mut Vec<LineError>) {
    let n = line.to_string().chars().count();
    if n > MAX_LINE_CHARS {
        out.push(LineError {
            line: at,
            col: 1,
            message: format!(
                "too long once written out ({n} characters, at most {MAX_LINE_CHARS}); shorten the note or drop a condition"
            ),
        });
    }
}

pub(super) fn role_word(r: NodeRole) -> &'static str {
    match r {
        NodeRole::Condition => "a condition",
        NodeRole::Action => "an action",
        NodeRole::Target => "a target selector",
    }
}

impl NodeRole {
    /// `condition`, `action`, `target selector`.
    pub fn noun(self) -> &'static str {
        &role_word(self)[role_word(self).find(' ').map_or(0, |k| k + 1)..]
    }
}

pub(super) fn check_call<'a>(
    call: &Call,
    role: NodeRole,
    nodes: &'a Table<BehaviorNode>,
    kind: ListKind,
    at: LineRef,
    out: &mut Vec<LineError>,
) -> Option<&'a BehaviorNode> {
    let err = |col: Col, message: String| LineError {
        line: at,
        col: col.0,
        message,
    };
    let Some(node) = nodes.get(&call.name) else {
        let mut msg = format!("unknown {} `{}`", role.noun(), call.name);
        if let Some(s) = suggest(
            &call.name,
            nodes
                .iter()
                .filter(|n| n.role == role && kind.allows(n))
                .map(|n| n.id.as_str()),
        ) {
            msg += &format!("; did you mean `{s}`?");
        }
        out.push(err(call.col, msg));
        return None;
    };
    if node.role != role {
        out.push(err(
            call.col,
            format!(
                "`{}` is {}, not {}",
                node.id,
                role_word(node.role),
                role_word(role)
            ),
        ));
        return None;
    }
    if !kind.allows(node) {
        let message = match kind {
            ListKind::Behavior => format!("`{}` is for pilot assists only", node.id),
            ListKind::Assists => {
                let assists: Vec<&str> = nodes.iter().filter(|n| n.pilot && n.role == role).map(|n| n.id.as_str()).collect();
                format!("`{}` is a behavior's maneuver; pilot assists use {}", node.id, assists.join(", "))
            }
        };
        out.push(err(call.col, message));
        return None;
    }
    if call.args.len() > node.params.len() {
        let names: Vec<&str> = node.params.iter().map(|p| p.id.as_str()).collect();
        let col = call.args[node.params.len()].col;
        out.push(err(
            col,
            if names.is_empty() {
                format!("`{}` takes no values", node.id)
            } else {
                format!(
                    "`{}` takes at most {} value(s): {}",
                    node.id,
                    names.len(),
                    names.join(", ")
                )
            },
        ));
    }
    for (a, p) in call.args.iter().zip(&node.params) {
        if let Err(m) = check_param_value(&a.value, p) {
            out.push(err(a.col, m));
        }
    }
    for p in node.params.iter().skip(call.args.len()) {
        if p.default.is_none() {
            out.push(err(
                call.col,
                format!("`{}` needs a value for {} (`{}`)", node.id, p.name, p.id),
            ));
            break;
        }
    }
    Some(node)
}

/// The closest candidate by edit distance (at most 2 edits, or 1 for short names).
pub fn suggest<'a>(word: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let limit = if word.len() <= 4 { 1 } else { 2 };
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        let d = edit_distance(word, c);
        if d <= limit && best.is_none_or(|b| d < b.0) {
            best = Some((d, c));
        }
    }
    best.map(|b| b.1)
}

pub(super) fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let sub = prev[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            cur[j] = sub.min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The arguments of a call with defaults filled in, resolved against the node's parameters. `None` if a
/// required value is missing or a value does not fit (impossible for a checked program).
pub fn resolve_args(call: &Call, node: &BehaviorNode) -> Option<Vec<Amount>> {
    node.params
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let v = match call.args.get(k) {
                Some(a) => a.value.clone(),
                None => parse_value(p.default.as_deref()?).ok()?,
            };
            param_amount(&v, &p.ty).ok()
        })
        .collect()
}
