//! The behavior text format: a behavior is a priority list of rules, one line per rule, readable and typeable
//! on a phone. The same lines are stored in the RON content (`behaviors/*.ron`), shown in the editor's text
//! view and printed in the wiki. Player-facing description: `docs/behavior-format.md`.
//!
//! ```text
//! target weakest(engines)
//! when hull < 30% -> flee                       # run when badly hurt
//! when target_in_range(120%) and cycle(9s) >= 5s -> orbit(67%)
//! always -> approach(85%)
//! ```
//!
//! * [`Program::parse`] reads the text form, [`Program::from_parts`] the RON form (the `target` field and the
//!   `rules` list hold exactly the same lines). [`Program::to_text`] / [`Program::to_parts`] print the
//!   canonical form. Parse → print → parse gives an equal [`Program`] (positions are not part of equality).
//! * Names are case-insensitive (phones capitalise the first letter of a line) and printed in lower case;
//!   `if` is accepted for `when`, `→` for `->`, `≤`/`≥` for `<=`/`>=`.
//! * [`check_program`] checks a parsed program against the vocabulary (the `behavior_nodes` content).
//!
//! Errors carry a 1-based column (in characters) and say what was expected.
//!
//! The program types and their printing are here, the lexer and parser in `parse.rs`, the checks against the
//! vocabulary and typed values in `check.rs`, the editing model in [`edit`].

pub mod edit;
mod check;
mod parse;

use serde::{Deserialize, Serialize};

use crate::content::model::{BehaviorNode, NodeParam, NodeRole, ParamType, Quantity};
use crate::content::table::Table;

pub use check::{Amount, ListKind, check_assists, check_list, check_param_value, check_program, check_text, check_text_as, param_amount, quantity_amount, resolve_args, suggest};
use check::too_many_rules;
pub use parse::parse_value;
use parse::{Line, parse_line, parse_target_part};

/// Words with a meaning of their own; never valid as node or option ids.
pub const KEYWORDS: &[&str] = &["when", "if", "always", "and", "or", "not", "target"];
/// Hard limits that keep evaluation bounded (rule slots are the gameplay limit; these are sanity limits).
pub const MAX_RULES: usize = 64;
pub const MAX_CONDITIONS: usize = 8;
pub const MAX_ARGS: usize = 4;
/// Longest line, in characters: as typed (the parser refuses longer lines) and as printed in canonical form
/// ([`check_program`] refuses a line that prints longer, e.g. compact input that gains spaces).
pub const MAX_LINE_CHARS: usize = 240;
/// Largest reach-relative distance (`%` on a distance), in percent.
pub const MAX_REACH_PERCENT: f32 = 1000.0;

/// A behavior as RON in the content format (`behaviors/*.ron`): the header fields, `target`, then `rules` with
/// one rule per line, four-space indent. Absent and empty fields are left out; `source` is never written.
pub fn behavior_to_ron(b: &crate::content::model::Behavior) -> String {
    fn field(s: &mut String, name: &str, value: &str) {
        s.push_str(&format!("    {name}: {value},\n"));
    }
    let q = |t: &str| ron::to_string(t).unwrap_or_default();
    let mut s = String::from("(\n");
    field(&mut s, "id", &q(&b.id));
    field(&mut s, "name", &q(&b.name));
    field(&mut s, "summary", &q(&b.summary));
    if let Some(d) = &b.description {
        field(&mut s, "description", &q(d));
    }
    if let Some(l) = &b.lore {
        field(&mut s, "lore", &q(l));
    }
    if !b.tags.is_empty() {
        let tags: Vec<String> = b.tags.iter().map(|t| q(t)).collect();
        field(&mut s, "tags", &format!("[{}]", tags.join(", ")));
    }
    if let Some(t) = &b.target {
        field(&mut s, "target", &q(t));
    }
    s.push_str("    rules: [\n");
    for r in &b.rules {
        s.push_str(&format!("        {},\n", q(r)));
    }
    s.push_str("    ],\n)\n");
    s
}

/// A 1-based column (in characters), kept for messages. Columns never take part in equality, so a program
/// equals its re-parsed print however it was spaced.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Col(pub u32);

impl PartialEq for Col {
    fn eq(&self, _: &Col) -> bool {
        true
    }
}

/// Unit written after a number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    /// `m`: metres.
    Metres,
    /// `%`: a percentage (of the value itself, or of the weapon reach for distances).
    Percent,
    /// `s`: seconds.
    Seconds,
}

impl Unit {
    pub fn suffix(self) -> &'static str {
        match self {
            Unit::Metres => "m",
            Unit::Percent => "%",
            Unit::Seconds => "s",
        }
    }
}

/// A literal value: a number with an optional unit, or a word (a choice option such as `cw`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Number(f32, Option<Unit>),
    Word(String),
}

/// A value and where it was written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arg {
    pub value: Value,
    #[serde(skip)]
    pub col: Col,
}

/// `name` or `name(arg, arg)`: a node with positional arguments (trailing ones may be left to defaults).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub name: String,
    pub args: Vec<Arg>,
    #[serde(skip)]
    pub col: Col,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    pub fn symbol(self) -> &'static str {
        match self {
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }

    /// `lhs op rhs`.
    #[inline]
    pub fn test(self, lhs: f32, rhs: f32) -> bool {
        match self {
            CmpOp::Lt => lhs < rhs,
            CmpOp::Le => lhs <= rhs,
            CmpOp::Gt => lhs > rhs,
            CmpOp::Ge => lhs >= rhs,
        }
    }
}

/// `op value` after a value condition (`hull < 30%`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Compare {
    pub op: CmpOp,
    pub rhs: Arg,
}

/// One condition of a rule: `[not] call [op value]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cond {
    pub not: bool,
    pub call: Call,
    pub cmp: Option<Compare>,
}

/// `when c1 and c2 -> action # note`, or `always -> action` (empty `when`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub when: Vec<Cond>,
    pub action: Call,
    pub note: Option<String>,
}

/// `target selector # note`: who the rules are about.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TargetLine {
    pub call: Call,
    pub note: Option<String>,
}

/// A whole behavior: an optional target line (default: the `nearest` selector) and the rules in priority
/// order. The first rule whose conditions all hold decides.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub target: Option<TargetLine>,
    pub rules: Vec<Rule>,
}

/// Which line of a program a problem is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LineRef {
    /// The program as a whole (e.g. no rules at all).
    Whole,
    /// The target line (RON field `target`).
    Target,
    /// Rule `i` (0-based; RON field `rules[i]`).
    Rule(usize),
}

impl LineRef {
    /// The RON field path of this line.
    pub fn field(self) -> String {
        match self {
            LineRef::Whole => "rules".into(),
            LineRef::Target => "target".into(),
            LineRef::Rule(i) => format!("rules[{i}]"),
        }
    }
}

/// A problem in one line of a program (syntax or vocabulary).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineError {
    pub line: LineRef,
    /// 1-based column in characters (1 for whole-line problems).
    pub col: u32,
    pub message: String,
}

impl std::fmt::Display for LineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            LineRef::Whole => write!(f, "{}", self.message),
            LineRef::Target => write!(f, "target, column {}: {}", self.col, self.message),
            LineRef::Rule(i) => write!(f, "rule {}, column {}: {}", i + 1, self.col, self.message),
        }
    }
}

/// A problem in the text form, located by 1-based line and column.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextError {
    pub line: u32,
    pub col: u32,
    pub message: String,
}

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line {}, column {}: {}",
            self.line, self.col, self.message
        )
    }
}

/// Where each program line came from in a text (1-based text line numbers).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineMap {
    pub target: Option<u32>,
    pub rules: Vec<u32>,
}

impl LineMap {
    pub fn text_line(&self, l: LineRef) -> u32 {
        match l {
            LineRef::Whole => 1,
            LineRef::Target => self.target.unwrap_or(1),
            LineRef::Rule(i) => self.rules.get(i).copied().unwrap_or(1),
        }
    }

    pub fn to_text_error(&self, e: LineError) -> TextError {
        TextError {
            line: self.text_line(e.line),
            col: e.col,
            message: e.message,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Printing (canonical form)

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // f32's Display is the shortest text that parses back to the same bits.
            Value::Number(v, unit) => write!(f, "{v}{}", unit.map_or("", Unit::suffix)),
            Value::Word(w) => f.write_str(w),
        }
    }
}

impl std::fmt::Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl std::fmt::Display for Call {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)?;
        if !self.args.is_empty() {
            f.write_str("(")?;
            for (k, a) in self.args.iter().enumerate() {
                if k > 0 {
                    f.write_str(", ")?;
                }
                a.fmt(f)?;
            }
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for Cond {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.not {
            f.write_str("not ")?;
        }
        self.call.fmt(f)?;
        if let Some(c) = &self.cmp {
            write!(f, " {} {}", c.op.symbol(), c.rhs)?;
        }
        Ok(())
    }
}

fn write_note(f: &mut std::fmt::Formatter<'_>, note: &Option<String>) -> std::fmt::Result {
    match note {
        Some(n) => write!(f, " # {n}"),
        None => Ok(()),
    }
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.when.is_empty() {
            f.write_str("always")?;
        } else {
            f.write_str("when ")?;
            for (k, c) in self.when.iter().enumerate() {
                if k > 0 {
                    f.write_str(" and ")?;
                }
                c.fmt(f)?;
            }
        }
        write!(f, " -> {}", self.action)?;
        write_note(f, &self.note)
    }
}

impl std::fmt::Display for TargetLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "target {}", self.call)?;
        write_note(f, &self.note)
    }
}

impl Program {
    /// The canonical text form: the target line (if any), then one line per rule, each ending in `\n`.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        if let Some(t) = &self.target {
            s += &t.to_string();
            s.push('\n');
        }
        for r in &self.rules {
            s += &r.to_string();
            s.push('\n');
        }
        s
    }

    /// The RON form: the target field (the line without the `target` keyword) and one string per rule.
    pub fn to_parts(&self) -> (Option<String>, Vec<String>) {
        let target = self.target.as_ref().map(|t| {
            let line = t.to_string();
            line.strip_prefix("target ").unwrap_or(&line).to_string()
        });
        (target, self.rules.iter().map(Rule::to_string).collect())
    }

    /// Parses the text form. Blank lines are ignored; a `target` line may come first.
    pub fn parse(text: &str) -> Result<Program, Vec<TextError>> {
        Program::parse_with_lines(text).map(|(p, _)| p)
    }

    /// [`Program::parse`], plus the text line of every program line (to locate later vocabulary errors).
    /// Reading stops after the target line and [`MAX_RULES`] rules (or lines with errors), with an error at the
    /// next one, so a huge text costs no more than a full program.
    pub fn parse_with_lines(text: &str) -> Result<(Program, LineMap), Vec<TextError>> {
        let mut prog = Program::default();
        let mut map = LineMap::default();
        let mut errors = Vec::new();
        let mut lines = 0;
        for (k, raw) in text.lines().enumerate() {
            let line_no = (k + 1).try_into().unwrap_or(u32::MAX);
            let parsed = parse_line(raw);
            if !matches!(parsed, Ok(Line::Blank)) {
                if lines == MAX_RULES + 1 {
                    errors.push(TextError {
                        line: line_no,
                        col: 1,
                        message: format!("at most {MAX_RULES} rules"),
                    });
                    break;
                }
                lines += 1;
            }
            match parsed {
                Ok(Line::Blank) => {}
                Ok(Line::Target(t)) => {
                    if prog.target.is_some() {
                        errors.push(TextError {
                            line: line_no,
                            col: 1,
                            message: "only one `target` line is allowed".into(),
                        });
                    } else if !prog.rules.is_empty() {
                        errors.push(TextError {
                            line: line_no,
                            col: 1,
                            message: "the `target` line comes before the rules".into(),
                        });
                    } else {
                        prog.target = Some(t);
                        map.target = Some(line_no);
                    }
                }
                Ok(Line::Rule(r)) => {
                    prog.rules.push(r);
                    map.rules.push(line_no);
                }
                Err((col, message)) => errors.push(TextError {
                    line: line_no,
                    col,
                    message,
                }),
            }
        }
        if errors.is_empty() {
            Ok((prog, map))
        } else {
            Err(errors)
        }
    }

    /// Parses the RON form: `target` holds the selector (`nearest(engines)`, without the keyword), `rules` one
    /// rule per string.
    pub fn from_parts(target: Option<&str>, rules: &[String]) -> Result<Program, Vec<LineError>> {
        if rules.len() > MAX_RULES {
            return Err(vec![too_many_rules()]);
        }
        let mut prog = Program::default();
        let mut errors = Vec::new();
        if let Some(t) = target {
            match parse_target_part(t) {
                Ok(t) => prog.target = Some(t),
                Err((col, message)) => errors.push(LineError {
                    line: LineRef::Target,
                    col,
                    message,
                }),
            }
        }
        for (i, r) in rules.iter().enumerate() {
            let at = LineRef::Rule(i);
            match parse_line(r) {
                Ok(Line::Rule(rule)) => prog.rules.push(rule),
                Ok(Line::Blank) => errors.push(LineError {
                    line: at,
                    col: 1,
                    message: "empty rule".into(),
                }),
                Ok(Line::Target(_)) => errors.push(LineError {
                    line: at,
                    col: 1,
                    message: "the target goes in the `target` field, not in `rules`".into(),
                }),
                Err((col, message)) => errors.push(LineError {
                    line: at,
                    col,
                    message,
                }),
            }
        }
        if errors.is_empty() {
            Ok(prog)
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(s: &str) -> Rule {
        match parse_line(s) {
            Ok(Line::Rule(r)) => r,
            other => panic!("{s}: {:?}", other.err()),
        }
    }

    fn err(s: &str) -> (u32, String) {
        match parse_line(s) {
            Err(e) => e,
            Ok(_) => panic!("{s} parsed"),
        }
    }

    #[test]
    fn rule_shapes() {
        let r = rule("when hull < 30% and not target_in_range(60m) -> orbit(45, cw) # strafe");
        assert_eq!(r.when.len(), 2);
        assert_eq!(r.when[0].call.name, "hull");
        let cmp = r.when[0].cmp.as_ref().unwrap();
        assert_eq!(cmp.op, CmpOp::Lt);
        assert_eq!(cmp.rhs.value, Value::Number(30.0, Some(Unit::Percent)));
        assert!(r.when[1].not);
        assert_eq!(
            r.when[1].call.args[0].value,
            Value::Number(60.0, Some(Unit::Metres))
        );
        assert_eq!(r.action.name, "orbit");
        assert_eq!(r.action.args[1].value, Value::Word("cw".into()));
        assert_eq!(r.note.as_deref(), Some("strafe"));
        assert_eq!(
            r.to_string(),
            "when hull < 30% and not target_in_range(60m) -> orbit(45, cw) # strafe"
        );
        let a = rule("Always → Flee()");
        assert!(a.when.is_empty());
        assert_eq!(a.to_string(), "always -> flee");
        assert_eq!(
            rule("If hull≤ 30 % -> hold").to_string(),
            "when hull <= 30% -> hold"
        );
    }

    #[test]
    fn columns_point_at_the_problem() {
        assert_eq!(
            err("when hull < 30% -> "),
            (20, "expected an action, found the end of the line".into())
        );
        assert_eq!(err("when hull = 30% -> flee").0, 11);
        assert_eq!(err("when hull < 30%, shield_down -> flee").0, 16);
        assert_eq!(err("when a or b -> flee").0, 8);
        assert_eq!(err("when orbit(45 -> flee"), (15, "missing `)`".into()));
        assert_eq!(err("when x < 5km -> flee").0, 11);
        assert_eq!(err("flee").0, 1);
        assert_eq!(err("# note").0, 1);
    }
}
