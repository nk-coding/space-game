//! The behavior text's lexer and parser: one line into tokens, then into a rule, the target line or a value; errors
//! carry a 1-based column.

use super::*;

// ---------------------------------------------------------------------------------------------------------
// Lexer

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tok {
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
pub(super) struct Token {
    tok: Tok,
    col: u32,
}

pub(super) fn describe(t: &Tok) -> String {
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

pub(super) type PResult<T> = Result<T, (u32, String)>;

pub(super) fn lex(line: &str) -> PResult<Vec<Token>> {
    if line.chars().nth(MAX_LINE_CHARS).is_some() {
        return Err((
            MAX_LINE_CHARS as u32 + 1,
            format!("line too long (at most {MAX_LINE_CHARS} characters)"),
        ));
    }
    let chars: Vec<char> = line.chars().collect();
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
            let note = note.trim();
            // A note is one line: the text form would split it (and the inspector refuses the same).
            if note.chars().any(char::is_control) {
                let skip = chars[i + 1..].iter().take_while(|c| c.is_whitespace()).count();
                let k = chars[i + 1 + skip..].iter().position(|c| c.is_control()).unwrap_or(0);
                return Err((
                    (i + 2 + skip + k) as u32,
                    "a note is one line of text: no line breaks, tabs or other control characters".into(),
                ));
            }
            i = n;
            Tok::Note(note.to_string())
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

pub(super) enum Line {
    Blank,
    Target(TargetLine),
    Rule(Rule),
}

pub(super) struct Parser {
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

pub(super) fn parse_line(line: &str) -> PResult<Line> {
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
pub(super) fn parse_target_part(s: &str) -> PResult<TargetLine> {
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
