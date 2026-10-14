//! A cheap structural check of icon SVGs (no XML library): the client rasterises them at runtime and the wiki
//! shows them as they are, so a broken or unsuitable file should fail validation, not the renderer. The
//! rasteriser reads the viewBox with [`view_box`], so it can use exactly the icons that pass [`check_svg`].

/// Elements an icon must not use (local names, compared in lower case, so `svg:script` and `SCRIPT` count too):
/// raster images (no fixed raster graphics, ADR-0021; `feImage` loads one as a filter input), text (the runtime
/// rasteriser has no fonts; convert text to paths), and scripts or foreign content (the wiki shows the SVGs in
/// a browser).
const FORBIDDEN: &[&str] = &["image", "feimage", "text", "script", "foreignobject"];

/// Largest viewBox side, in user units (20 units are one cell).
pub const MAX_VIEW_BOX: f32 = 10_000.0;

/// `Err` describes the first problem: not well-formed (unbalanced or unclosed tags, comments, quotes, malformed
/// attributes), a root element other than `<svg>` in the SVG namespace, no usable `viewBox` on the root, a
/// forbidden element, an event handler attribute (`onload`, ...), a reference outside the file (`href` or
/// `url(...)` not starting with `#`, `@import`) or a declaration other than a plain `<!DOCTYPE ...>`.
pub fn check_svg(src: &str) -> Result<(), String> {
    scan(src).map(drop)
}

/// The root `viewBox` (`[min_x, min_y, width, height]`) of an icon that passes [`check_svg`], read the same
/// way (either quote style, numbers separated by whitespace or a comma); `None` for any other text.
pub fn view_box(src: &str) -> Option<[f32; 4]> {
    scan(src).ok()
}

/// SVG user units per ship cell in all content art.
pub const UNITS_PER_CELL: f32 = 20.0;

/// The most a ship drawing may reach past its footprint at the stern, in cells (GDR-0069: engine nozzles).
pub const MAX_OVERHANG_CELLS: f32 = 1.0;

/// How far the ship drawing of a part with a `w × h` footprint reaches past its stern, in cells, from the
/// drawing's viewBox: `0 0 (w × 20) (h × 20 + overhang)` (the part unrotated, facing up, the stern at +y; see
/// `docs/content-model.md` "Ship drawings"). `Err` says why the viewBox does not fit the footprint.
pub fn ship_overhang(view_box: [f32; 4], (w, h): (u32, u32)) -> Result<f32, String> {
    let [x, y, vw, vh] = view_box;
    let (fw, fh) = (w as f32 * UNITS_PER_CELL, h as f32 * UNITS_PER_CELL);
    let over = (vh - fh) / UNITS_PER_CELL;
    if x != 0.0 || y != 0.0 || (vw - fw).abs() > 1e-3 || over < -1e-3 || over > MAX_OVERHANG_CELLS + 1e-3 {
        return Err(format!(
            "the ship drawing's viewBox must be `0 0 {fw} {fh}` for a {w}×{h} footprint, at most {} units taller \
             (an overhang at the stern); got `{x} {y} {vw} {vh}`",
            MAX_OVERHANG_CELLS * UNITS_PER_CELL
        ));
    }
    Ok(over.max(0.0))
}

/// [`check_svg`], returning the root viewBox.
fn scan(src: &str) -> Result<[f32; 4], String> {
    let lower = src.to_ascii_lowercase();
    if lower.contains("@import") {
        return Err("`@import` is not allowed in icons (references outside the file)".into());
    }
    for (k, _) in lower.match_indices("url(") {
        let target = lower[k + 4..].trim_start().trim_start_matches(['"', '\'']);
        if !target.starts_with('#') {
            return Err("`url(...)` may only point inside the file (`url(#id)`)".into());
        }
    }
    let mut stack: Vec<&str> = Vec::new();
    let mut view_box = None;
    let mut rest = src.trim_start_matches('\u{feff}');
    while let Some(lt) = rest.find('<') {
        if stack.is_empty() && view_box.is_some() && !rest[..lt].trim().is_empty() {
            return Err("text after the closing </svg>".into());
        }
        rest = &rest[lt..];
        if let Some(r) = rest.strip_prefix("<!--") {
            let end = r.find("-->").ok_or("unclosed comment")?;
            rest = &r[end + 3..];
        } else if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let end = r.find("]]>").ok_or("unclosed CDATA section")?;
            rest = &r[end + 3..];
        } else if let Some(r) = rest.strip_prefix("<?") {
            let end = r.find("?>").ok_or("unclosed processing instruction")?;
            rest = &r[end + 2..];
        } else if let Some(r) = rest.strip_prefix("<!") {
            let end = r.find('>').ok_or("unclosed declaration")?;
            // A plain DOCTYPE line is harmless; an internal subset declares entities, which icons never need.
            if !r[..end].starts_with("DOCTYPE") || r[..end].contains('[') {
                return Err(
                    "no entity or other declarations in icons (a plain <!DOCTYPE ...> is fine)".into(),
                );
            }
            rest = &r[end + 1..];
        } else if let Some(r) = rest.strip_prefix("</") {
            let end = r.find('>').ok_or("unclosed end tag")?;
            let name = r[..end].trim();
            match stack.pop() {
                Some(open) if open == name => {}
                Some(open) => return Err(format!("</{name}> closes <{open}>")),
                None => return Err(format!("</{name}> without an open element")),
            }
            rest = &r[end + 1..];
        } else {
            let r = &rest[1..];
            let end = tag_end(r).ok_or("unclosed tag")?;
            let tag = &r[..end];
            let self_closing = tag.ends_with('/');
            let tag = tag.trim_end_matches('/');
            let name_len = tag.find(|c: char| c.is_whitespace()).unwrap_or(tag.len());
            let name = &tag[..name_len];
            if name.is_empty() {
                return Err("a `<` that starts no tag".into());
            }
            let attrs = attributes(&tag[name_len..]).map_err(|e| format!("<{name}>: {e}"))?;
            if view_box.is_none() {
                if name != "svg" {
                    return Err(format!("the root element is <{name}>, not <svg>"));
                }
                let attr = |n: &str| attrs.iter().find(|(a, _)| *a == n).map(|(_, v)| *v);
                if attr("xmlns") != Some("http://www.w3.org/2000/svg") {
                    return Err("the root <svg> lacks xmlns=\"http://www.w3.org/2000/svg\"".into());
                }
                let v = attr("viewBox")
                    .ok_or("the root <svg> has no viewBox (icons are drawn at any size)")?;
                view_box = Some(parse_view_box(v).ok_or_else(|| {
                    format!(
                        "the root viewBox {v:?} is not four numbers with a width and height above 0 and at most {MAX_VIEW_BOX}"
                    )
                })?);
            } else if stack.is_empty() {
                return Err(format!("a second root element <{name}>"));
            }
            if FORBIDDEN.contains(&local(name).to_ascii_lowercase().as_str()) {
                return Err(format!(
                    "<{name}> is not allowed in icons (no raster images, text, scripts or foreign content)"
                ));
            }
            for (a, v) in &attrs {
                let a_local = local(a).to_ascii_lowercase();
                if a_local.starts_with("on") {
                    return Err(format!("<{name}>: event handler attributes (`{a}`) are not allowed"));
                }
                if a_local == "href" && !v.trim_start().starts_with('#') {
                    return Err(format!(
                        "<{name}>: `{a}` may only point inside the file (`#id`), not {v:?}"
                    ));
                }
            }
            if !self_closing {
                stack.push(name);
            }
            rest = &r[end + 1..];
        }
    }
    let Some(view_box) = view_box else {
        return Err("no <svg> element".into());
    };
    if let Some(open) = stack.last() {
        return Err(format!("<{open}> is never closed"));
    }
    if !rest.trim().is_empty() {
        return Err("text after the closing </svg>".into());
    }
    Ok(view_box)
}

/// The name after a namespace prefix (`svg:script` → `script`).
fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// The `name="value"` (or `name='value'`) attributes of a start tag, after its name.
fn attributes(mut s: &str) -> Result<Vec<(&str, &str)>, String> {
    let mut out = Vec::new();
    loop {
        s = s.trim_start();
        if s.is_empty() {
            return Ok(out);
        }
        let name_len = s
            .find(|c: char| c == '=' || c.is_whitespace())
            .ok_or("an attribute without a value")?;
        let name = &s[..name_len];
        let r = s[name_len..].trim_start();
        let r = r
            .strip_prefix('=')
            .ok_or("an attribute without a value")?
            .trim_start();
        let quote = r
            .chars()
            .next()
            .filter(|c| matches!(c, '"' | '\''))
            .ok_or_else(|| format!("the value of `{name}` is not quoted"))?;
        let end = r[1..].find(quote).ok_or("an unclosed attribute value")?;
        out.push((name, &r[1..1 + end]));
        s = &r[end + 2..];
    }
}

/// `min-x min-y width height`, separated by whitespace and/or a comma; finite, with a width and height above 0
/// and at most [`MAX_VIEW_BOX`].
fn parse_view_box(v: &str) -> Option<[f32; 4]> {
    let mut out = [0.0f32; 4];
    let mut rest = v.trim();
    for (k, slot) in out.iter_mut().enumerate() {
        if k > 0 {
            let r = rest.trim_start();
            rest = r.strip_prefix(',').unwrap_or(r).trim_start();
        }
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E')))
            .unwrap_or(rest.len());
        *slot = rest[..end].parse().ok()?;
        rest = &rest[end..];
    }
    let side = |x: f32| x > 0.0 && x <= MAX_VIEW_BOX;
    (rest.trim().is_empty() && out.iter().all(|x| x.is_finite()) && side(out[2]) && side(out[3]))
        .then_some(out)
}

/// Index of the `>` that ends a start tag (quotes respected), within `s` (which starts after the `<`).
fn tag_end(s: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '>') => return Some(i),
            (None, '<') => return None,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{check_svg, ship_overhang, view_box};

    /// A ship drawing covers its footprint from `0 0` and may reach past the stern by up to a cell.
    #[test]
    fn ship_drawings_fit_their_footprint() {
        assert_eq!(ship_overhang([0.0, 0.0, 20.0, 20.0], (1, 1)), Ok(0.0));
        assert!((ship_overhang([0.0, 0.0, 40.0, 49.0], (2, 2)).unwrap() - 0.45).abs() < 1e-6);
        assert_eq!(ship_overhang([0.0, 0.0, 20.0, 40.0], (1, 2)), Ok(0.0));
        for bad in [[1.0, 0.0, 20.0, 20.0], [0.0, -5.0, 20.0, 25.0], [0.0, 0.0, 40.0, 20.0], [0.0, 0.0, 20.0, 19.0], [0.0, 0.0, 20.0, 41.0]] {
            assert!(ship_overhang(bad, (1, 1)).is_err(), "{bad:?}");
        }
    }

    const OK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><filter id="g"><feGaussianBlur stdDeviation="1"/></filter></defs><!-- a > comment --><g filter="url(#g)" data-x="a>b"><rect x="1" y="1" width="2" height="2"></rect></g></svg>"#;

    /// An icon with `body` inside the root.
    fn icon(body: &str) -> String {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1">{body}</svg>"#)
    }

    #[test]
    fn accepts_a_plain_icon() {
        assert_eq!(check_svg(OK), Ok(()));
        assert_eq!(
            check_svg(&format!("<?xml version=\"1.0\"?>\n{OK}\n")),
            Ok(())
        );
        let doctype = r#"<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">"#;
        assert_eq!(check_svg(&format!("{doctype}\n{OK}")), Ok(()));
        assert_eq!(check_svg(&icon(r##"<use href="#a"/><g fill='url( "#b")'/>"##)), Ok(()));
    }

    #[test]
    fn reads_the_view_box_in_every_spelling() {
        for (root, want) in [
            (r#"viewBox="0 0 20 20""#, Some([0.0, 0.0, 20.0, 20.0])),
            ("viewBox='0 0 40 20'", Some([0.0, 0.0, 40.0, 20.0])),
            (r#"viewBox="0,0,20,20""#, Some([0.0, 0.0, 20.0, 20.0])),
            (r#"viewBox=" -1.5 , 2e1  3 4 ""#, Some([-1.5, 20.0, 3.0, 4.0])),
            (r#"viewBox = "0 0 20 20""#, Some([0.0, 0.0, 20.0, 20.0])),
            (r#"viewBox="0 0 20""#, None),
            (r#"viewBox="0 0 20 20 20""#, None),
            (r#"viewBox="0 0 0 20""#, None),
            (r#"viewBox="0 0 -20 20""#, None),
            (r#"viewBox="0 0 NaN 20""#, None),
            (r#"viewBox="0 0 inf 20""#, None),
            (r#"viewBox="0 0 1e30 20""#, None),
            (r#"viewBox="0 0 20,,20""#, None),
            (r#"viewBox="a b c d""#, None),
        ] {
            let src = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {root}></svg>"#);
            assert_eq!(view_box(&src), want, "{root}");
            assert_eq!(check_svg(&src).is_ok(), want.is_some(), "{root}");
        }
    }

    #[test]
    fn rejects_broken_or_unsuitable_files() {
        let open = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1">"#;
        let cases: Vec<(String, String)> = vec![
            ("".into(), "no <svg>".into()),
            ("<html></html>".into(), "root element is <html>".into()),
            (r#"<svg viewBox="0 0 1 1"></svg>"#.into(), "xmlns".into()),
            (r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#.into(), "viewBox".into()),
            (format!("{open}<g></svg>"), "</svg> closes <g>".into()),
            (format!("{open}<g>"), "never closed".into()),
            (format!("{open}<rect"), "unclosed tag".into()),
            (icon(r#"<image href="x.png"/>"#), "<image> is not allowed".into()),
            (icon("<text>A</text>"), "<text> is not allowed".into()),
            (
                format!("{open}</svg><svg/>"),
                "second root".into(),
            ),
            // Raster images and files through a filter, in any case or with a prefix.
            (icon(r#"<filter id="f"><feImage href="/dev/zero"/></filter>"#), "<feImage> is not allowed".into()),
            (icon(r##"<filter id="f"><FEIMAGE href="#a"/></filter>"##), "not allowed".into()),
            (icon("<svg:script>alert(1)</svg:script>"), "not allowed".into()),
            (icon("<SCRIPT>alert(1)</SCRIPT>"), "not allowed".into()),
            (icon("<foreignobject></foreignobject>"), "not allowed".into()),
            // Event handlers and references outside the file.
            (
                open.replace(">", r#" onload="alert(1)"></svg>"#),
                "event handler".into(),
            ),
            (icon(r#"<g ONCLICK="x()"/>"#), "event handler".into()),
            (icon(r#"<use href="other.svg#a"/>"#), "inside the file".into()),
            (icon(r#"<use xlink:href="file:///etc/passwd"/>"#), "inside the file".into()),
            (icon(r#"<a href="javascript:alert(1)"><g/></a>"#), "inside the file".into()),
            (icon(r#"<rect fill="url(http://example.com/p.svg#a)"/>"#), "url(".into()),
            (icon(r#"<rect style="fill: URL( 'x.svg#a' )"/>"#), "url(".into()),
            (icon("<style>@import 'x.css';</style>"), "@import".into()),
            // Entities and other declarations.
            (
                format!(r#"<!DOCTYPE svg [<!ENTITY a "aaaaaaaaaa">]>{OK}"#),
                "declarations".into(),
            ),
            (format!("<!ENTITY a 'b'>{OK}"), "declarations".into()),
            // Malformed attributes.
            (icon(r#"<rect x=1/>"#), "not quoted".into()),
            (icon(r#"<rect x="1/>"#), "unclosed".into()),
            (icon("<rect hidden/>"), "without a value".into()),
        ];
        for (src, needle) in cases {
            let err = check_svg(&src).expect_err(&src);
            assert!(err.contains(&needle), "{src}: {err}");
            assert_eq!(view_box(&src), None);
        }
    }
}
