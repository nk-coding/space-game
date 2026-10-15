//! Blueprint text: one blueprint as RON, in the style of `content/base/ships/*.ron`. Saved drydock designs and
//! exported blueprints use exactly this format, so an exported design can go into a pack's `ships/` folder as is.

use std::fmt::Write as _;

use crate::content::kind::Kind;
use crate::content::limits::MAX_DOCUMENT_BYTES;
use crate::content::load::{parse_error, ron_options, strip_bom};
use crate::content::model::{Blueprint, Placement, PowerOverride};
use crate::content::problem::Problem;

/// Writes a blueprint as RON: the header fields, `size`, then `parts` with one placement per line, four-space
/// indent. Defaults are left out: `rot: 0`, `flip: false`, a placement's `material` and `power` when absent,
/// `description`, `lore`, `behavior` and `assists` when absent, `tags` when empty; a `power` override writes only the
/// fields it sets. `source` is never written. Strings escape `"`, `\` and control characters (`\n`, `\t`,
/// `\u{..}`); other characters, unicode included, are written as they are.
///
/// The output is deterministic and reads back with [`parse_blueprint`] to an equal blueprint (its `source`
/// aside).
pub fn blueprint_to_ron(bp: &Blueprint) -> String {
    let mut s = String::from("(\n");
    field(&mut s, "id", &string(&bp.id));
    field(&mut s, "name", &string(&bp.name));
    field(&mut s, "summary", &string(&bp.summary));
    if let Some(d) = &bp.description {
        field(&mut s, "description", &string(d));
    }
    if let Some(l) = &bp.lore {
        field(&mut s, "lore", &string(l));
    }
    if !bp.tags.is_empty() {
        let tags: Vec<String> = bp.tags.iter().map(|t| string(t)).collect();
        field(&mut s, "tags", &format!("[{}]", tags.join(", ")));
    }
    field(&mut s, "faction", &string(&bp.faction));
    if let Some(b) = &bp.behavior {
        field(&mut s, "behavior", &string(b));
    }
    if let Some(a) = &bp.assists {
        s.push_str("    assists: (\n        rules: [\n");
        for r in &a.rules {
            let _ = writeln!(s, "            {},", string(r));
        }
        s.push_str("        ],\n    ),\n");
    }
    field(&mut s, "size", &format!("({}, {})", bp.size.0, bp.size.1));
    if bp.parts.is_empty() {
        field(&mut s, "parts", "[]");
    } else {
        s.push_str("    parts: [\n");
        for p in &bp.parts {
            s.push_str("        ");
            placement(&mut s, p);
            s.push_str(",\n");
        }
        s.push_str("    ],\n");
    }
    s.push_str(")\n");
    s
}

/// Reads one blueprint written as RON (by [`blueprint_to_ron`] or by hand) with the pack loader's options:
/// `Some(..)` may be left out, unknown fields are errors, comments are fine. `source` stays empty.
///
/// Only the syntax and the schema are checked here; [`crate::validate_ship`] checks the blueprint against the
/// content. Limits for text from outside: at most [`MAX_DOCUMENT_BYTES`], coordinates within
/// ±[`crate::content::limits::MAX_COORD`]; a leading byte order mark is skipped. `Err` holds the parse error (kind `ship`,
/// line and column).
pub fn parse_blueprint(text: &str) -> Result<Blueprint, Vec<Problem>> {
    if text.len() > MAX_DOCUMENT_BYTES {
        let mut p = Problem::error(format!(
            "the text is too long for a blueprint ({} bytes, at most {MAX_DOCUMENT_BYTES})",
            text.len()
        ));
        p.kind = Some(Kind::Ship);
        return Err(vec![p]);
    }
    ron_options().from_str(strip_bom(text)).map_err(|e| {
        let mut p = parse_error(&e);
        p.kind = Some(Kind::Ship);
        vec![p]
    })
}

fn field(s: &mut String, name: &str, value: &str) {
    let _ = writeln!(s, "    {name}: {value},");
}

fn placement(s: &mut String, p: &Placement) {
    let _ = write!(s, "(part: {}, x: {}, y: {}", string(&p.part), p.x, p.y);
    if p.rot != 0 {
        let _ = write!(s, ", rot: {}", p.rot);
    }
    if p.flip {
        s.push_str(", flip: true");
    }
    if let Some(m) = &p.material {
        let _ = write!(s, ", material: {}", string(m));
    }
    if let Some(o) = &p.power {
        s.push_str(", power: ");
        power(s, o);
    }
    if let Some(g) = p.group {
        let _ = write!(s, ", group: {g}");
    }
    s.push(')');
}

fn power(s: &mut String, o: &PowerOverride) {
    let mut fields = Vec::new();
    if let Some(t) = o.tier {
        // The variant name, exactly as serde reads it back.
        fields.push(format!(
            "tier: {}",
            ron::to_string(&t).expect("a unit variant")
        ));
    }
    if let Some(w) = o.weight {
        // `Display` of an f32 is the shortest text that reads back to the same value (`2`, `0.1`, `inf`).
        fields.push(format!("weight: {w}"));
    }
    let _ = write!(s, "({})", fields.join(", "));
}

/// A RON string literal.
fn string(text: &str) -> String {
    let mut s = String::with_capacity(text.len() + 2);
    s.push('"');
    for c in text.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(s, "\\u{{{:x}}}", c as u32);
            }
            c => s.push(c),
        }
    }
    s.push('"');
    s
}
