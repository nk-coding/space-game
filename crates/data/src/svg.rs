//! A cheap structural check of icon SVGs (no XML library): the client rasterises them at runtime and the wiki
//! shows them as they are, so a broken or unsuitable file should fail validation, not the renderer.

/// Elements an icon must not use: raster images (no fixed raster graphics, ADR-0021), text (the runtime
/// rasteriser has no fonts; convert text to paths), and scripts or foreign content (the wiki shows the SVGs
/// in a browser).
const FORBIDDEN: &[&str] = &["image", "text", "script", "foreignObject"];

/// `Err` describes the first problem: not well-formed (unbalanced or unclosed tags, comments, quotes), a root
/// element other than `<svg>` in the SVG namespace, no `viewBox` on the root, or a forbidden element.
pub fn check_svg(src: &str) -> Result<(), String> {
    let mut stack: Vec<&str> = Vec::new();
    let mut root_seen = false;
    let mut rest = src.trim_start_matches('\u{feff}');
    while let Some(lt) = rest.find('<') {
        if stack.is_empty() && root_seen && !rest[..lt].trim().is_empty() {
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
            if !root_seen {
                if name != "svg" {
                    return Err(format!("the root element is <{name}>, not <svg>"));
                }
                let attrs = &tag[name_len..];
                if !attrs.contains("xmlns=\"http://www.w3.org/2000/svg\"")
                    && !attrs.contains("xmlns='http://www.w3.org/2000/svg'")
                {
                    return Err("the root <svg> lacks xmlns=\"http://www.w3.org/2000/svg\"".into());
                }
                if !attrs.contains("viewBox=") {
                    return Err(
                        "the root <svg> has no viewBox (icons are drawn at any size)".into(),
                    );
                }
                root_seen = true;
            } else if stack.is_empty() {
                return Err(format!("a second root element <{name}>"));
            }
            if FORBIDDEN.contains(&name) {
                return Err(format!(
                    "<{name}> is not allowed in icons (no raster images, text, scripts or foreign content)"
                ));
            }
            if !self_closing {
                stack.push(name);
            }
            rest = &r[end + 1..];
        }
    }
    if !root_seen {
        return Err("no <svg> element".into());
    }
    if let Some(open) = stack.last() {
        return Err(format!("<{open}> is never closed"));
    }
    if !rest.trim().is_empty() {
        return Err("text after the closing </svg>".into());
    }
    Ok(())
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
    use super::check_svg;

    const OK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><filter id="g"><feGaussianBlur stdDeviation="1"/></filter></defs><!-- a > comment --><g filter="url(#g)" data-x="a>b"><rect x="1" y="1" width="2" height="2"></rect></g></svg>"#;

    #[test]
    fn accepts_a_plain_icon() {
        assert_eq!(check_svg(OK), Ok(()));
        assert_eq!(
            check_svg(&format!("<?xml version=\"1.0\"?>\n{OK}\n")),
            Ok(())
        );
    }

    #[test]
    fn rejects_broken_or_unsuitable_files() {
        let cases = [
            ("", "no <svg>"),
            ("<html></html>", "root element is <html>"),
            (r#"<svg viewBox="0 0 1 1"></svg>"#, "xmlns"),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#,
                "viewBox",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><g></svg>"#,
                "</svg> closes <g>",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><g>"#,
                "never closed",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect"#,
                "unclosed tag",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><image href="x.png"/></svg>"#,
                "<image> is not allowed",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><text>A</text></svg>"#,
                "<text> is not allowed",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"></svg><svg/>"#,
                "second root",
            ),
        ];
        for (src, needle) in cases {
            let err = check_svg(src).expect_err(src);
            assert!(err.contains(needle), "{src}: {err}");
        }
    }
}
