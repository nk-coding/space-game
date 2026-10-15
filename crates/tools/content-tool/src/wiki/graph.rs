//! Rule lists drawn as the BEHAVIOR screen's GRAPH view draws them (`docs/client.md` "Behavior screen"): the target
//! line on top (for pilot assists `YOUR LOCK`: they have no target line and act on the pilot's lock), the root (`Priority · first match`, for assists `Assists · each channel`), the trunk with a dot per
//! rule, each rule's condition chips (violet; Rhai conditions dashed gold) joined by `and`, a connector, and the action
//! chip (teal) in one column, the rule's note after it. Chips show the nodes' content names and the values as written. Colours are CSS classes (`rg-*`), which the site maps to the style tokens.

use std::fmt::Write as _;

use data::behavior::{Call, ListKind, Program};
use data::Content;

use super::markup::esc;

/// Layout, in pixels: the font size, the width of a character (Barlow at that size, a little generous so labels never
/// overflow their chip), and the chips' height and padding.
const FONT: f32 = 12.0;
const CHAR: f32 = 6.5;
const CHIP_H: f32 = 24.0;
const PAD: f32 = 12.0;
const ROW: f32 = 40.0;
const TRUNK_X: f32 = 10.0;
const FIRST_X: f32 = 30.0;
const AND_GAP: f32 = 34.0;
const CONNECTOR: f32 = 44.0;

/// One chip: its label, the node it links to, and how it is drawn.
struct Chip {
    label: String,
    node: Option<String>,
    class: &'static str,
    /// A small tag over the chip (`RHAI`).
    tag: Option<&'static str>,
}

impl Chip {
    fn width(&self) -> f32 {
        self.label.chars().count() as f32 * CHAR + 2.0 * PAD
    }
}

/// `Orbit 58% · ccw`: a call as the graph labels it, with the node's content name.
fn call_label(c: &Content, call: &Call) -> String {
    let name = c
        .behavior_nodes
        .get(&call.name)
        .map_or(call.name.as_str(), |n| n.name.as_str());
    let args: Vec<String> = call.args.iter().map(|a| a.to_string()).collect();
    if args.is_empty() {
        name.to_string()
    } else {
        format!("{name} {}", args.join(" · "))
    }
}

fn node_chip(c: &Content, call: &Call, class: &'static str) -> Chip {
    let node = c.behavior_nodes.get(&call.name);
    let rhai = node.is_some_and(|n| n.script.is_some());
    Chip {
        label: call_label(c, call),
        node: node.map(|n| n.id.clone()),
        class: if rhai { "rg-chip rg-rhai" } else { class },
        tag: rhai.then_some("RHAI"),
    }
}

fn text(s: &mut String, x: f32, y: f32, class: &str, t: &str) {
    write!(
        s,
        "<text x=\"{x:.1}\" y=\"{y:.1}\" class=\"{class}\">{}</text>",
        esc(t)
    )
    .unwrap();
}

fn chip(s: &mut String, ch: &Chip, x: f32, y: f32) -> f32 {
    let w = ch.width();
    let body = format!(
        "<rect class=\"{}\" x=\"{x:.1}\" y=\"{:.1}\" width=\"{w:.1}\" height=\"{CHIP_H}\" rx=\"{}\"/><text x=\"{:.1}\" y=\"{:.1}\" class=\"rg-chip-label\">{}</text>",
        ch.class,
        y - CHIP_H / 2.0,
        CHIP_H / 2.0,
        x + w / 2.0,
        y + FONT * 0.36,
        esc(&ch.label)
    );
    // No links inside the SVG (the site prefetches every `<a>` on a page and cannot read an SVG one); the text under
    // the graph links every node, and a chip names its node in its tooltip.
    match &ch.node {
        Some(id) => write!(s, "<g><title>{}</title>{body}</g>", esc(id)).unwrap(),
        None => s.push_str(&body),
    }
    if let Some(tag) = ch.tag {
        text(s, x + w - 4.0, y - CHIP_H / 2.0 - 3.0, "rg-tag", tag);
    }
    w
}

/// The graph of a rule list as an SVG.
pub fn draw(c: &Content, p: &Program, kind: ListKind) -> String {
    // The chips of every rule: conditions (with `not` and the comparison as written), then the action.
    let rows: Vec<(Vec<Chip>, Chip, Option<&str>)> = p
        .rules
        .iter()
        .map(|r| {
            let conds: Vec<Chip> = if r.when.is_empty() {
                vec![Chip {
                    label: "always".into(),
                    node: None,
                    class: "rg-chip rg-always",
                    tag: None,
                }]
            } else {
                r.when
                    .iter()
                    .map(|cond| {
                        let mut ch = node_chip(c, &cond.call, "rg-chip rg-cond");
                        if cond.not {
                            ch.label = format!("not {}", ch.label);
                        }
                        if let Some(cmp) = &cond.cmp {
                            write!(ch.label, " {} {}", cmp.op.symbol(), cmp.rhs).unwrap();
                        }
                        ch
                    })
                    .collect()
            };
            (conds, node_chip(c, &r.action, "rg-chip rg-act"), r.note.as_deref())
        })
        .collect();
    let chain = |conds: &[Chip]| -> f32 {
        conds.iter().map(Chip::width).sum::<f32>() + AND_GAP * (conds.len().saturating_sub(1)) as f32
    };
    let act_x = FIRST_X + rows.iter().map(|(cs, _, _)| chain(cs)).fold(0.0, f32::max) + CONNECTOR;
    let notes = |n: Option<&str>| n.map_or(0.0, |n| 16.0 + n.chars().count() as f32 * CHAR);
    let target = match &p.target {
        _ if kind == ListKind::Assists => None,
        Some(t) => Some(node_chip(c, &t.call, "rg-chip rg-tgt")),
        None => Some(Chip {
            label: format!("{} · default", call_label(c, &Call { name: "nearest".into(), args: Vec::new(), col: Default::default() })),
            node: c.behavior_nodes.get("nearest").map(|n| n.id.clone()),
            class: "rg-chip rg-tgt",
            tag: None,
        }),
    };
    let target_x = 70.0;
    let lock_label = "YOUR LOCK";
    let target_w = target.as_ref().map_or(lock_label.len() as f32 * CHAR, Chip::width);
    let width = rows
        .iter()
        .map(|(_, a, n)| act_x + a.width() + notes(*n))
        .fold(target_x + target_w, f32::max)
        + 8.0;
    let top = 20.0;
    let root_y = top + 36.0;
    let first = root_y + 32.0;
    let height = first + (rows.len().max(1) as f32 - 1.0) * ROW + CHIP_H / 2.0 + 10.0;
    let label = if kind == ListKind::Assists {
        "Pilot assists"
    } else {
        "Behavior rules"
    };
    let mut s = format!(
        "<svg class=\"rule-graph\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width:.0} {height:.0}\" width=\"{width:.0}\" height=\"{height:.0}\" font-size=\"{FONT}\" role=\"img\" aria-label=\"{label}\">"
    );
    text(&mut s, 0.0, top + FONT * 0.36, "rg-kicker", "TARGET");
    match &target {
        Some(t) => {
            chip(&mut s, t, target_x, top);
        }
        None => text(&mut s, target_x, top + FONT * 0.36, "rg-kicker", lock_label),
    }
    // The root and the trunk.
    write!(s, "<circle class=\"rg-root\" cx=\"{TRUNK_X}\" cy=\"{root_y:.1}\" r=\"4\"/>").unwrap();
    let root_label = if kind == ListKind::Assists {
        "ASSISTS · EACH CHANNEL"
    } else {
        "PRIORITY · FIRST MATCH"
    };
    text(&mut s, FIRST_X - 8.0, root_y + FONT * 0.36, "rg-kicker", root_label);
    if !rows.is_empty() {
        let last = first + (rows.len() - 1) as f32 * ROW;
        write!(
            s,
            "<path class=\"rg-trunk\" d=\"M{TRUNK_X} {:.1}V{last:.1}\"/>",
            root_y + 4.0
        )
        .unwrap();
    }
    for (k, (conds, action, note)) in rows.iter().enumerate() {
        let y = first + k as f32 * ROW;
        write!(
            s,
            "<g class=\"rg-rule\"><circle class=\"rg-dot\" cx=\"{TRUNK_X}\" cy=\"{y:.1}\" r=\"3\"/><path class=\"rg-trunk\" d=\"M{TRUNK_X} {y:.1}H{FIRST_X}\"/>"
        )
        .unwrap();
        let mut x = FIRST_X;
        for (i, ch) in conds.iter().enumerate() {
            if i > 0 {
                text(&mut s, x - AND_GAP / 2.0 - 9.0, y + FONT * 0.36, "rg-and", "AND");
            }
            x += chip(&mut s, ch, x, y) + AND_GAP;
        }
        let end = x - AND_GAP;
        write!(
            s,
            "<path class=\"rg-connector\" marker-end=\"url(#arrow-teal)\" d=\"M{end:.1} {y:.1}H{:.1}\"/>",
            act_x - 6.0
        )
        .unwrap();
        let w = chip(&mut s, action, act_x, y);
        if let Some(n) = note {
            text(&mut s, act_x + w + 16.0, y + FONT * 0.36, "rg-note", &format!("# {n}"));
        }
        s.push_str("</g>");
    }
    if rows.is_empty() {
        text(&mut s, FIRST_X, first + FONT * 0.36, "rg-note", "No rules: the ship coasts when you let go.");
    }
    s.push_str("</svg>\n");
    s
}
