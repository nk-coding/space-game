//! SVG for the wiki, all vector: the pack's icons as page-level `<symbol>`s, and style-2a layouts of ships and
//! single parts drawn from the blueprint geometry (`data::design::geometry`) in the colours of `art/tokens.json`.

use std::fmt::Write as _;

use data::design::geometry::HullLoop;
use data::{GridPoint, PlacedShape};
use serde_json::Value;

use super::markup::esc;

/// The style tokens (`art/tokens.json`, embedded at build time like the `art` crate does).
pub struct Style {
    tokens: Value,
}

impl Style {
    pub fn embedded() -> Style {
        Style::from_json(include_str!("../../../../../art/tokens.json"))
    }

    pub fn from_json(json: &str) -> Style {
        Style {
            tokens: serde_json::from_str(json).expect("art/tokens.json parses"),
        }
    }

    /// A token by dotted path (`ship.machine_tile`), as text. Panics if it is missing: the tokens are embedded,
    /// and the tests render every page.
    pub fn get(&self, path: &str) -> String {
        let pointer = format!("/{}", path.replace('.', "/"));
        match self.tokens.pointer(&pointer) {
            Some(Value::String(s)) => s.clone(),
            Some(v @ Value::Number(_)) => v.to_string(),
            _ => panic!("style token `{path}` missing in art/tokens.json"),
        }
    }

    fn num(&self, path: &str) -> f32 {
        self.get(path)
            .parse()
            .unwrap_or_else(|_| panic!("style token `{path}` is not a number"))
    }

    /// Hull glow colour of a faction glow key (`data::content::art_keys::GLOWS`).
    pub fn glow(&self, key: &str) -> String {
        self.get(if key == "enemy" {
            "ship.hull_glow.enemy_color"
        } else {
            "ship.hull_glow.color"
        })
    }
}

/// A number for SVG coordinates: at most 3 decimals, trailing zeros trimmed.
fn n(v: f32) -> String {
    data::design::stats::format_number(v, 3)
}

/// The `>` that ends the tag starting at `s[0] == '<'`, quotes respected.
fn tag_end(s: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in s.char_indices().skip(1) {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// The value of attribute `name` in a start tag.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(k) = tag[from..].find(name).map(|k| k + from) {
        let before_ok = tag[..k].ends_with(char::is_whitespace);
        let rest = tag[k + name.len()..].trim_start();
        if before_ok && let Some(v) = rest.strip_prefix('=') {
            let v = v.trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                return v[1..].find(q).map(|e| &v[1..1 + e]);
            }
        }
        from = k + name.len();
    }
    None
}

/// The symbol id of an icon key.
pub fn icon_id(key: &str) -> String {
    format!("icon-{key}")
}

/// An icon SVG as a `<symbol id="icon-<key>">` for the page's sprite. The root's `viewBox` (and
/// `preserveAspectRatio`) move to the symbol; every id inside gets the prefix `icon-<key>-` and every reference
/// (`url(#..)`, `href="#.."`) follows, because all icons use the same ids (`gl`, `gCore`, ...) and share a page.
/// The rest of the file is kept as it is (`data::content::svg::check_svg` has vetted it). `Err` for a file without a
/// usable root.
pub fn icon_symbol(key: &str, svg: &str) -> Result<String, String> {
    let mut start = 0;
    let open = loop {
        let lt = svg[start..].find('<').ok_or("no <svg> element")? + start;
        let rest = &svg[lt..];
        if let Some(r) = rest.strip_prefix("<!--") {
            start = lt + 4 + r.find("-->").ok_or("unclosed comment")? + 3;
        } else if rest.starts_with("<?") || rest.starts_with("<!") {
            start = lt + tag_end(rest).ok_or("unclosed declaration")? + 1;
        } else if rest.starts_with("<svg") {
            break lt;
        } else {
            return Err("the root element is not <svg>".into());
        }
    };
    let open_end = open + tag_end(&svg[open..]).ok_or("unclosed <svg> tag")?;
    let root = &svg[open..=open_end];
    let view_box = attr(root, "viewBox").ok_or("the root <svg> has no viewBox")?;
    let inner = if root.ends_with("/>") {
        ""
    } else {
        let close = svg.rfind("</svg>").ok_or("no closing </svg>")?;
        &svg[open_end + 1..close]
    };
    let aspect = attr(root, "preserveAspectRatio")
        .map(|a| format!(" preserveAspectRatio=\"{}\"", esc(a)))
        .unwrap_or_default();
    Ok(format!(
        "<symbol id=\"{}\" viewBox=\"{}\"{aspect}>{}</symbol>",
        icon_id(key),
        esc(view_box),
        prefix_ids(inner.trim(), &format!("icon-{key}-"))
    ))
}

/// Prefixes every `id` attribute and every local reference in the start tags of an SVG fragment.
fn prefix_ids(src: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(src.len() + src.len() / 8);
    let mut rest = src;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        rest = &rest[lt..];
        let end = if let Some(r) = rest.strip_prefix("<!--") {
            r.find("-->").map_or(rest.len(), |e| e + 7)
        } else {
            tag_end(rest).map_or(rest.len(), |e| e + 1)
        };
        let tag = &rest[..end];
        if tag.starts_with("</") || tag.starts_with("<!") || tag.starts_with("<?") {
            out.push_str(tag);
        } else {
            prefix_tag(tag, prefix, &mut out);
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn prefix_tag(tag: &str, prefix: &str, out: &mut String) {
    let mut s = tag;
    while let Some(eq) = s.find('=') {
        let after = &s[eq + 1..];
        let Some(q) = after.chars().next().filter(|&q| q == '"' || q == '\'') else {
            out.push_str(&s[..eq + 1]);
            s = after;
            continue;
        };
        let Some(close) = after[1..].find(q) else {
            break;
        };
        let name = s[..eq]
            .trim_end()
            .rsplit(char::is_whitespace)
            .next()
            .unwrap_or("");
        let value = &after[1..1 + close];
        let value = match name {
            "id" => format!("{prefix}{value}"),
            "href" | "xlink:href" if value.starts_with('#') => format!("#{prefix}{}", &value[1..]),
            _ => value.replace("url(#", &format!("url(#{prefix}")),
        };
        out.push_str(&s[..eq + 1]);
        out.push(q);
        out.push_str(&value);
        out.push(q);
        s = &after[close + 2..];
    }
    out.push_str(s);
}

/// The page sprite: shared filters plus one symbol per icon the page shows.
pub fn sprite(style: &Style, symbols: &[String]) -> String {
    let mut s = String::from(
        "<svg class=\"sprite\" xmlns=\"http://www.w3.org/2000/svg\" width=\"0\" height=\"0\" aria-hidden=\"true\"><defs>",
    );
    write!(
        s,
        "<filter id=\"hull-blur\" x=\"-0.5\" y=\"-0.5\" width=\"2\" height=\"2\"><feGaussianBlur stdDeviation=\"{}\"/></filter>",
        n(style.num("ship.hull_glow.halo_blur_std_cells"))
    )
    .unwrap();
    for sym in symbols {
        s.push_str(sym);
    }
    s.push_str("</defs></svg>\n");
    s
}

/// One tile of a layout: a placed footprint, its fill colour and the icon drawn on it.
pub struct Tile {
    pub shape: PlacedShape,
    pub fill: String,
    pub icon: Option<String>,
}

/// The hull glow along a ship's outline: chained loops and their colour.
pub struct Glow {
    pub loops: Vec<HullLoop>,
    pub color: String,
}

fn path_of(points: &[GridPoint], closed: bool, d: &mut String) {
    for (k, p) in points.iter().enumerate() {
        write!(d, "{}{} {}", if k == 0 { "M" } else { "L" }, p.x, p.y).unwrap();
    }
    if closed {
        d.push('Z');
    }
}

/// A style-2a drawing: flat tiles (armour tones, machine tiles), dark seams, glowing icons turned and mirrored like
/// the placements, and the faint hull glow (halo plus crisp line) when given. 1 SVG unit = 1 cell; the drawing is
/// `px_per_cell` pixels per cell, with `margin` cells around the tiles (room for the glow).
pub fn layout(
    style: &Style,
    tiles: &[Tile],
    glow: Option<&Glow>,
    px_per_cell: f32,
    margin: f32,
    class: &str,
    label: &str,
) -> String {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for t in tiles {
        for p in t.shape.polygon() {
            (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
        }
    }
    if tiles.is_empty() {
        (x0, y0, x1, y1) = (0, 0, 1, 1);
    }
    let m = margin;
    let (w, h) = ((x1 - x0) as f32 + 2.0 * m, (y1 - y0) as f32 + 2.0 * m);
    let mut s = format!(
        "<svg class=\"{class}\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" width=\"{}\" height=\"{}\" role=\"img\" aria-label=\"{}\">",
        n(x0 as f32 - m),
        n(y0 as f32 - m),
        n(w),
        n(h),
        n(w * px_per_cell),
        n(h * px_per_cell),
        esc(label)
    );
    // Fills, one path per colour (in order of first use), then all seams in one path on top.
    let mut fills: Vec<(&str, String)> = Vec::new();
    let mut seams = String::new();
    for t in tiles {
        let poly = t.shape.polygon();
        let k = match fills.iter().position(|(c, _)| *c == t.fill) {
            Some(k) => k,
            None => {
                fills.push((t.fill.as_str(), String::new()));
                fills.len() - 1
            }
        };
        path_of(&poly, true, &mut fills[k].1);
        path_of(&poly, true, &mut seams);
    }
    for (color, d) in &fills {
        write!(s, "<path fill=\"{}\" d=\"{d}\"/>", esc(color)).unwrap();
    }
    write!(
        s,
        "<path fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\" d=\"{seams}\"/>",
        esc(&style.get("ship.seam")),
        n(2.0 * style.num("ship.seam_width_cells"))
    )
    .unwrap();
    // Icons at the unrotated footprint size, centred, mirrored first if flipped, then turned clockwise.
    for t in tiles {
        let Some(key) = &t.icon else { continue };
        let (fw, fh) = (t.shape.footprint.w as f32, t.shape.footprint.h as f32);
        let [cx, cy] = t.shape.center();
        let mut xf = format!("translate({} {})", n(cx), n(cy));
        if t.shape.rot % 4 != 0 {
            write!(xf, " rotate({})", (t.shape.rot % 4) as u32 * 90).unwrap();
        }
        if t.shape.flip {
            xf.push_str(" scale(-1 1)");
        }
        write!(
            s,
            "<use href=\"#{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" transform=\"{xf}\"/>",
            icon_id(key),
            n(-fw / 2.0),
            n(-fh / 2.0),
            n(fw),
            n(fh)
        )
        .unwrap();
    }
    if let Some(g) = glow.filter(|g| !g.loops.is_empty()) {
        let mut d = String::new();
        for l in &g.loops {
            path_of(&l.points, l.closed, &mut d);
        }
        write!(
            s,
            "<g fill=\"none\" stroke=\"{}\" stroke-linejoin=\"round\" stroke-linecap=\"round\">\
             <path stroke-width=\"{}\" opacity=\"{}\" filter=\"url(#hull-blur)\" d=\"{d}\"/>\
             <path stroke-width=\"{}\" opacity=\"{}\" d=\"{d}\"/></g>",
            esc(&g.color),
            n(style.num("ship.hull_glow.halo_width_cells")),
            n(style.num("ship.hull_glow.halo_opacity")),
            n(style.num("ship.hull_glow.line_width_cells")),
            n(style.num("ship.hull_glow.line_opacity")),
        )
        .unwrap();
    }
    s.push_str("</svg>");
    s
}

/// An icon on its own (a material swatch, a category icon): the symbol drawn into a `w × h` box.
pub fn icon(key: &str, w: u32, h: u32, class: &str) -> String {
    format!(
        "<svg class=\"{class}\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" aria-hidden=\"true\"><use href=\"#{}\" width=\"{w}\" height=\"{h}\"/></svg>",
        icon_id(key)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_ids_get_a_prefix_and_references_follow() {
        let svg = r##"<?xml version="1.0"?><!-- an <svg> comment --><svg xmlns="http://www.w3.org/2000/svg" width="80" viewBox="0 0 20 20"><defs><filter id="gl"><feGaussianBlur stdDeviation="1"/></filter><radialGradient id="g"/></defs><g filter="url(#gl)" opacity="0.5"><circle fill="url(#g)" r="2"/></g><use xlink:href="#g" href="#gl"/><path d="M0 0" data-x="a>b"/></svg>"##;
        let sym = icon_symbol("pulse-laser", svg).unwrap();
        assert_eq!(
            sym,
            r##"<symbol id="icon-pulse-laser" viewBox="0 0 20 20"><defs><filter id="icon-pulse-laser-gl"><feGaussianBlur stdDeviation="1"/></filter><radialGradient id="icon-pulse-laser-g"/></defs><g filter="url(#icon-pulse-laser-gl)" opacity="0.5"><circle fill="url(#icon-pulse-laser-g)" r="2"/></g><use xlink:href="#icon-pulse-laser-g" href="#icon-pulse-laser-gl"/><path d="M0 0" data-x="a>b"/></symbol>"##
        );
        assert!(icon_symbol("x", "<g/>").is_err());
        assert!(icon_symbol("x", r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#).is_err());
    }

    #[test]
    fn every_token_the_wiki_uses_exists() {
        let s = Style::embedded();
        for tone in data::content::art_keys::ARMOR_TONES {
            s.get(&format!("ship.armor.{tone}"));
        }
        for key in data::content::art_keys::SYSTEM_COLORS {
            s.get(&format!("ship.system_colors.{key}"));
        }
        for key in data::content::art_keys::GLOWS {
            s.glow(key);
        }
    }
}
