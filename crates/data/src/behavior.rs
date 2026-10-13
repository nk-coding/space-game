//! The behavior text format: a behavior is a priority list of rules, one line per rule, readable and typeable
//! on a phone. The same lines are stored in the RON content (`behaviors/*.ron`), shown in the editor's text
//! view and printed in the wiki. Player-facing description: `docs/behavior-format.md`.
//!
//! ```text
//! target weakest(engines)
//! when hull < 30% -> flee                       # run when badly hurt
//! when target_in_range(120%) and cycle(9s) >= 5s -> orbit(60%)
//! always -> approach(67%)
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

use serde::{Deserialize, Serialize};

use crate::model::{BehaviorNode, NodeParam, NodeRole, ParamType, Quantity};
use crate::table::Table;

/// Words with a meaning of their own; never valid as node or option ids.
pub const KEYWORDS: &[&str] = &["when", "if", "always", "and", "or", "not", "target"];
/// Hard limits that keep evaluation bounded (rule slots are the gameplay limit; these are sanity limits).
pub const MAX_RULES: usize = 64;
pub const MAX_CONDITIONS: usize = 8;
pub const MAX_ARGS: usize = 4;
pub const MAX_LINE_CHARS: usize = 240;
/// Largest reach-relative distance (`%` on a distance), in percent.
pub const MAX_REACH_PERCENT: f32 = 1000.0;

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
    pub fn parse_with_lines(text: &str) -> Result<(Program, LineMap), Vec<TextError>> {
        let mut prog = Program::default();
        let mut map = LineMap::default();
        let mut errors = Vec::new();
        for (k, raw) in text.lines().enumerate() {
            let line_no = k as u32 + 1;
            match parse_line(raw) {
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

// ---------------------------------------------------------------------------------------------------------
// Lexer

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Num(f32, Option<Unit>),
    LParen,
    RParen,
    Comma,
    Cmp(CmpOp),
    Arrow,
    Note(String),
    End,
}

#[derive(Clone, Debug)]
struct Token {
    tok: Tok,
    col: u32,
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(s) => format!("`{s}`"),
        Tok::Num(v, u) => format!("`{}`", Value::Number(*v, *u)),
        Tok::LParen => "`(`".into(),
        Tok::RParen => "`)`".into(),
        Tok::Comma => "`,`".into(),
        Tok::Cmp(op) => format!("`{}`", op.symbol()),
        Tok::Arrow => "`->`".into(),
        Tok::Note(_) => "a `#` note".into(),
        Tok::End => "the end of the line".into(),
    }
}

type PResult<T> = Result<T, (u32, String)>;

fn lex(line: &str) -> PResult<Vec<Token>> {
    let chars: Vec<char> = line.chars().collect();
    if chars.len() > MAX_LINE_CHARS {
        return Err((
            MAX_LINE_CHARS as u32 + 1,
            format!("line too long (at most {MAX_LINE_CHARS} characters)"),
        ));
    }
    let mut out = Vec::new();
    let mut i = 0;
    let n = chars.len();
    while i < n {
        let c = chars[i];
        let col = i as u32 + 1;
        if c == ' ' || c == '\t' || c == '\r' {
            i += 1;
            continue;
        }
        let tok = if c == '#' {
            let note: String = chars[i + 1..].iter().collect();
            i = n;
            Tok::Note(note.trim().to_string())
        } else if c.is_ascii_alphabetic() {
            let start = i;
            while i < n && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            out.push(Token {
                tok: Tok::Ident(word.to_ascii_lowercase()),
                col,
            });
            continue;
        } else if c.is_ascii_digit()
            || (c == '-' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            let start = i;
            i += 1;
            while i < n && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < n && chars[i] == '.' {
                if !chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()) {
                    return Err((
                        i as u32 + 2,
                        "expected a digit after the decimal point".into(),
                    ));
                }
                i += 1;
                while i < n && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let text: String = chars[start..i].iter().collect();
            let value: f32 = text
                .parse()
                .map_err(|_| (col, format!("`{text}` is not a number")))?;
            if !value.is_finite() {
                return Err((col, format!("`{text}` is too large")));
            }
            // Optional unit, attached or after spaces: `%`, `m` or `s`.
            let mut j = i;
            while j < n && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            let spaced = j > i;
            let mut unit = None;
            if j < n && chars[j] == '%' {
                unit = Some(Unit::Percent);
                i = j + 1;
            } else if j < n && chars[j].is_ascii_alphabetic() {
                let ws = j;
                let mut k = j;
                while k < n && (chars[k].is_ascii_alphanumeric() || chars[k] == '_') {
                    k += 1;
                }
                let word: String = chars[ws..k].iter().collect::<String>().to_ascii_lowercase();
                match word.as_str() {
                    "m" => {
                        unit = Some(Unit::Metres);
                        i = k;
                    }
                    "s" => {
                        unit = Some(Unit::Seconds);
                        i = k;
                    }
                    _ if !spaced => {
                        return Err((
                            ws as u32 + 1,
                            format!(
                                "unknown unit `{word}`; units are m (metres), s (seconds) and %"
                            ),
                        ));
                    }
                    _ => {}
                }
            } else if j < n && chars[j] == '.' {
                return Err((j as u32 + 1, "a number cannot end with `.`".into()));
            }
            Tok::Num(value, unit)
        } else {
            let next = chars.get(i + 1).copied();
            let (tok, len) = match (c, next) {
                ('(', _) => (Tok::LParen, 1),
                (')', _) => (Tok::RParen, 1),
                (',', _) => (Tok::Comma, 1),
                ('<', Some('=')) => (Tok::Cmp(CmpOp::Le), 2),
                ('>', Some('=')) => (Tok::Cmp(CmpOp::Ge), 2),
                ('<', _) => (Tok::Cmp(CmpOp::Lt), 1),
                ('>', _) => (Tok::Cmp(CmpOp::Gt), 1),
                ('≤', _) => (Tok::Cmp(CmpOp::Le), 1),
                ('≥', _) => (Tok::Cmp(CmpOp::Ge), 1),
                ('-', Some('>')) => (Tok::Arrow, 2),
                ('→', _) => (Tok::Arrow, 1),
                ('=', _) => {
                    return Err((
                        col,
                        "compare with <, <=, > or >= (there is no `=`); the arrow is `->`".into(),
                    ));
                }
                ('!', _) => return Err((col, "write `not` instead of `!`".into())),
                ('&', _) => return Err((col, "join conditions with `and`".into())),
                ('|', _) => {
                    return Err((
                        col,
                        "there is no `or`: write a second rule with the same action".into(),
                    ));
                }
                ('.', _) => {
                    return Err((col, "write numbers with a leading digit, e.g. 0.5".into()));
                }
                ('-', _) => return Err((col, "expected `->` (or a number after `-`)".into())),
                _ => return Err((col, format!("unexpected character `{c}`"))),
            };
            i += len;
            tok
        };
        out.push(Token { tok, col });
    }
    out.push(Token {
        tok: Tok::End,
        col: n as u32 + 1,
    });
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------
// Parser

enum Line {
    Blank,
    Target(TargetLine),
    Rule(Rule),
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.toks[self.pos]
    }

    fn next(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn is_ident(&self, word: &str) -> bool {
        matches!(&self.peek().tok, Tok::Ident(w) if w == word)
    }

    fn call(&mut self, what: &str) -> PResult<Call> {
        let t = self.next();
        let name = match t.tok {
            Tok::Ident(w) if KEYWORDS.contains(&w.as_str()) => {
                return Err((t.col, format!("expected {what}, found the keyword `{w}`")));
            }
            Tok::Ident(w) => w,
            other => {
                return Err((
                    t.col,
                    format!("expected {what}, found {}", describe(&other)),
                ));
            }
        };
        let mut call = Call {
            name,
            args: Vec::new(),
            col: Col(t.col),
        };
        if self.peek().tok != Tok::LParen {
            return Ok(call);
        }
        self.next();
        if self.peek().tok == Tok::RParen {
            self.next();
            return Ok(call);
        }
        loop {
            let t = self.next();
            let value = match t.tok {
                Tok::Num(v, u) => Value::Number(v, u),
                Tok::Ident(w) => Value::Word(w),
                Tok::End | Tok::Note(_) | Tok::Arrow => {
                    return Err((t.col, "missing `)`".into()));
                }
                other => {
                    return Err((
                        t.col,
                        format!("expected a value, found {}", describe(&other)),
                    ));
                }
            };
            if call.args.len() == MAX_ARGS {
                return Err((t.col, format!("at most {MAX_ARGS} values per node")));
            }
            call.args.push(Arg {
                value,
                col: Col(t.col),
            });
            let t = self.next();
            match t.tok {
                Tok::Comma => continue,
                Tok::RParen => break,
                Tok::End | Tok::Note(_) | Tok::Arrow => return Err((t.col, "missing `)`".into())),
                other => {
                    return Err((
                        t.col,
                        format!("expected `,` or `)`, found {}", describe(&other)),
                    ));
                }
            }
        }
        Ok(call)
    }

    fn cond(&mut self) -> PResult<Cond> {
        let not = if self.is_ident("not") {
            self.next();
            true
        } else {
            false
        };
        let call = self.call("a condition")?;
        let cmp = if let Tok::Cmp(op) = self.peek().tok {
            self.next();
            let t = self.next();
            match t.tok {
                Tok::Num(v, u) => Some(Compare {
                    op,
                    rhs: Arg {
                        value: Value::Number(v, u),
                        col: Col(t.col),
                    },
                }),
                other => {
                    return Err((
                        t.col,
                        format!(
                            "expected a number after `{}`, found {}",
                            op.symbol(),
                            describe(&other)
                        ),
                    ));
                }
            }
        } else {
            None
        };
        Ok(Cond { not, call, cmp })
    }

    fn note_and_end(&mut self) -> PResult<Option<String>> {
        let t = self.next();
        let note = match t.tok {
            Tok::Note(n) => {
                let end = self.next();
                debug_assert_eq!(end.tok, Tok::End);
                (!n.is_empty()).then_some(n)
            }
            Tok::End => None,
            other => {
                return Err((
                    t.col,
                    format!(
                        "expected the end of the line (or a `#` note), found {}",
                        describe(&other)
                    ),
                ));
            }
        };
        Ok(note)
    }

    fn arrow(&mut self) -> PResult<()> {
        let t = self.next();
        match t.tok {
            Tok::Arrow => Ok(()),
            Tok::Ident(w) if w == "or" => Err((
                t.col,
                "there is no `or`: write a second rule with the same action".into(),
            )),
            Tok::Comma => Err((t.col, "join conditions with `and`".into())),
            other => Err((
                t.col,
                format!("expected `and` or `->`, found {}", describe(&other)),
            )),
        }
    }
}

fn parse_line(line: &str) -> PResult<Line> {
    let toks = lex(line)?;
    let mut p = Parser { toks, pos: 0 };
    let first = p.peek().clone();
    match &first.tok {
        Tok::End => Ok(Line::Blank),
        Tok::Note(_) => Err((
            first.col,
            "a note goes at the end of a rule (`... -> flee # note`), so it survives the graph view".into(),
        )),
        Tok::Ident(w) if w == "target" => {
            p.next();
            let call = p.call("a target selector")?;
            let note = p.note_and_end()?;
            Ok(Line::Target(TargetLine { call, note }))
        }
        Tok::Ident(w) if w == "always" => {
            p.next();
            if p.is_ident("and") {
                return Err((p.peek().col, "`always` stands alone; drop it or the other conditions".into()));
            }
            p.arrow()?;
            let action = p.call("an action")?;
            let note = p.note_and_end()?;
            Ok(Line::Rule(Rule {
                when: Vec::new(),
                action,
                note,
            }))
        }
        Tok::Ident(w) if w == "when" || w == "if" => {
            p.next();
            let mut when = Vec::new();
            loop {
                let col = p.peek().col;
                if when.len() == MAX_CONDITIONS {
                    return Err((col, format!("at most {MAX_CONDITIONS} conditions per rule")));
                }
                when.push(p.cond()?);
                if p.is_ident("and") {
                    p.next();
                    continue;
                }
                p.arrow()?;
                break;
            }
            let action = p.call("an action")?;
            let note = p.note_and_end()?;
            Ok(Line::Rule(Rule { when, action, note }))
        }
        Tok::Ident(w) => Err((
            first.col,
            format!("a rule starts with `when` or `always` (or `target` for the target line), not `{w}`"),
        )),
        other => Err((
            first.col,
            format!("a rule starts with `when` or `always`, found {}", describe(other)),
        )),
    }
}

/// The RON `target` field: a selector call, optionally with the `target` keyword and a note.
fn parse_target_part(s: &str) -> PResult<TargetLine> {
    let trimmed = s.trim_start();
    let starts_with_kw = trimmed
        .get(..6)
        .is_some_and(|w| w.eq_ignore_ascii_case("target"))
        && trimmed[6..].starts_with([' ', '\t']);
    if starts_with_kw {
        return match parse_line(s)? {
            Line::Target(t) => Ok(t),
            _ => Err((1, "expected a target selector".into())),
        };
    }
    let toks = lex(s)?;
    let mut p = Parser { toks, pos: 0 };
    if p.peek().tok == Tok::End {
        return Err((1, "expected a target selector, e.g. `nearest`".into()));
    }
    let call = p.call("a target selector")?;
    let note = p.note_and_end()?;
    Ok(TargetLine { call, note })
}

/// Parses one value as written in content defaults (`67%`, `45m`, `cw`).
pub fn parse_value(s: &str) -> Result<Value, String> {
    let toks = lex(s).map_err(|(c, m)| format!("column {c}: {m}"))?;
    match toks.as_slice() {
        [
            Token {
                tok: Tok::Num(v, u),
                ..
            },
            Token { tok: Tok::End, .. },
        ] => Ok(Value::Number(*v, *u)),
        [
            Token {
                tok: Tok::Ident(w), ..
            },
            Token { tok: Tok::End, .. },
        ] => Ok(Value::Word(w.clone())),
        _ => Err(format!(
            "`{s}` is not a single value (a number with an optional unit, or a word)"
        )),
    }
}

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
fn written_number(v: &Value) -> f32 {
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

/// Parses a behavior text and checks it against the vocabulary: the program, or every syntax and vocabulary
/// error with its text line and column (what a text view shows as live errors).
pub fn check_text(text: &str, nodes: &Table<BehaviorNode>) -> Result<Program, Vec<TextError>> {
    let (program, lines) = Program::parse_with_lines(text)?;
    let errors = check_program(&program, nodes);
    if errors.is_empty() { Ok(program) } else { Err(errors.into_iter().map(|e| lines.to_text_error(e)).collect()) }
}

/// Checks a parsed program against the behavior nodes: every name exists and has the right role, value
/// conditions are compared (and flag conditions are not), arguments fit their parameters, required ones are
/// present, numbers are in range. Returns every problem found.
pub fn check_program(p: &Program, nodes: &Table<BehaviorNode>) -> Vec<LineError> {
    let mut out = Vec::new();
    if p.rules.is_empty() {
        out.push(LineError {
            line: LineRef::Whole,
            col: 1,
            message: "a behavior needs at least one rule".into(),
        });
    }
    if p.rules.len() > MAX_RULES {
        out.push(LineError {
            line: LineRef::Whole,
            col: 1,
            message: format!("at most {MAX_RULES} rules"),
        });
    }
    if let Some(t) = &p.target {
        check_call(&t.call, NodeRole::Target, nodes, LineRef::Target, &mut out);
    }
    for (i, r) in p.rules.iter().enumerate() {
        let at = LineRef::Rule(i);
        for c in &r.when {
            let Some(node) = check_call(&c.call, NodeRole::Condition, nodes, at, &mut out) else {
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
        check_call(&r.action, NodeRole::Action, nodes, at, &mut out);
    }
    out
}

fn role_word(r: NodeRole) -> &'static str {
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

fn check_call<'a>(
    call: &Call,
    role: NodeRole,
    nodes: &'a Table<BehaviorNode>,
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
                .filter(|n| n.role == role)
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

fn edit_distance(a: &str, b: &str) -> usize {
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
