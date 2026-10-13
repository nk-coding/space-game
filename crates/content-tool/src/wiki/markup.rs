//! Text to HTML: escaping, the inline marks of content texts and the small Markdown subset of wiki prose.
//!
//! Everything that comes from content is escaped; the only markup the output contains is what this module
//! writes. Supported (see `docs/wiki.md`):
//!
//! * inline, in every text field and in prose: `[[kind:id]]` / `[[kind:id|label]]` links, `` `code` ``,
//!   `**bold**`, `*italic*`;
//! * blocks, in prose only: `#`–`######` headings (the first `# ` line is the title and is left out),
//!   paragraphs, `-`/`*` and `1.` lists (one level; indented lines continue an item), `>` quotes, `---` rules and
//!   fenced code blocks.
//!
//! Anything else (raw HTML, Markdown links, tables) shows as text.

use std::fmt::Write as _;

/// Escapes text for HTML content and double-quoted attribute values.
pub fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Turns a `[[kind:id|label]]` link into HTML: `(kind, id, label)` → anchor (or plain text).
pub type LinkFn<'a> = &'a dyn Fn(&str, &str, Option<&str>) -> String;

/// Renders one line or paragraph of text with links, code spans, bold and italic.
pub fn inline(text: &str, link: LinkFn) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    inline_into(text, link, &mut out);
    out
}

fn inline_into(text: &str, link: LinkFn, out: &mut String) {
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if rest.starts_with("[[")
            && let Some(end) = rest.find("]]")
            && let Some(Ok(l)) = data::parse_links(&rest[..end + 2]).into_iter().next()
        {
            out.push_str(&link(l.kind, l.id, l.label));
            i += end + 2;
            continue;
        }
        if let Some(r) = rest.strip_prefix('`')
            && let Some(end) = r.find('`')
            && end > 0
        {
            write!(out, "<code>{}</code>", esc(&r[..end])).unwrap();
            i += end + 2;
            continue;
        }
        if let Some(r) = rest.strip_prefix("**")
            && let Some(end) = closer(r, "**")
        {
            out.push_str("<strong>");
            inline_into(&r[..end], link, out);
            out.push_str("</strong>");
            i += end + 4;
            continue;
        }
        if let Some(r) = rest.strip_prefix('*')
            && !r.starts_with('*')
            && let Some(end) = closer(r, "*")
        {
            out.push_str("<em>");
            inline_into(&r[..end], link, out);
            out.push_str("</em>");
            i += end + 2;
            continue;
        }
        let c = rest.chars().next().unwrap();
        let mut buf = [0u8; 4];
        out.push_str(&esc(c.encode_utf8(&mut buf)));
        i += c.len_utf8();
    }
}

/// Where an emphasis that opened just before `s` closes: the first `marker` that follows a non-space character
/// (a single `*` must not be part of `**`). `None` if the opener is followed by a space or nothing closes it.
fn closer(s: &str, marker: &str) -> Option<usize> {
    if s.starts_with(char::is_whitespace) || s.is_empty() {
        return None;
    }
    let mut from = 1;
    while let Some(k) = s.get(from..)?.find(marker).map(|k| k + from) {
        let before = s[..k].chars().next_back();
        let doubled = marker == "*" && (s[k + 1..].starts_with('*') || s[..k].ends_with('*'));
        if before.is_some_and(|c| !c.is_whitespace()) && !doubled {
            return Some(k);
        }
        from = k + marker.len();
    }
    None
}

/// Renders wiki prose (the Markdown subset above) to HTML blocks. Headings start at `<h2>`: the page has its
/// own `<h1>`, so the first `# ` line (the title of a free page) is left out. Problems (an unclosed code fence)
/// go to `warnings`.
pub fn markdown(text: &str, link: LinkFn, warnings: &mut Vec<String>) -> String {
    let mut out = String::new();
    let mut para: Vec<&str> = Vec::new();
    // Open list: (`ul` or `ol`, items so far, the current item's lines).
    let mut list: Option<(&str, Vec<String>, Vec<&str>)> = None;
    let mut quote: Vec<&str> = Vec::new();
    let mut seen_block = false;
    let mut lines = text.lines().map(|l| l.trim_end_matches('\r'));

    fn flush_para(para: &mut Vec<&str>, out: &mut String, link: LinkFn) {
        if !para.is_empty() {
            writeln!(out, "<p>{}</p>", inline(&para.join("\n"), link)).unwrap();
            para.clear();
        }
    }
    fn flush_quote(quote: &mut Vec<&str>, out: &mut String, link: LinkFn) {
        if !quote.is_empty() {
            writeln!(
                out,
                "<blockquote><p>{}</p></blockquote>",
                inline(&quote.join("\n"), link)
            )
            .unwrap();
            quote.clear();
        }
    }
    fn flush_list(
        list: &mut Option<(&str, Vec<String>, Vec<&str>)>,
        out: &mut String,
        link: LinkFn,
    ) {
        if let Some((tag, mut items, item)) = list.take() {
            if !item.is_empty() {
                items.push(inline(&item.join("\n"), link));
            }
            writeln!(out, "<{tag}>").unwrap();
            for i in items {
                writeln!(out, "<li>{i}</li>").unwrap();
            }
            writeln!(out, "</{tag}>").unwrap();
        }
    }

    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        // Fenced code block.
        if let Some(lang) = trimmed.strip_prefix("```") {
            flush_para(&mut para, &mut out, link);
            flush_quote(&mut quote, &mut out, link);
            flush_list(&mut list, &mut out, link);
            let mut code = Vec::new();
            let mut closed = false;
            for l in lines.by_ref() {
                if l.trim_start().starts_with("```") {
                    closed = true;
                    break;
                }
                code.push(l);
            }
            if !closed {
                warnings.push("unclosed ``` code block (it runs to the end of the file)".into());
            }
            let class = lang.trim();
            let class = if class.is_empty() {
                String::new()
            } else {
                format!(" class=\"lang-{}\"", esc(class))
            };
            writeln!(
                out,
                "<pre><code{class}>{}</code></pre>",
                esc(&code.join("\n"))
            )
            .unwrap();
            seen_block = true;
            continue;
        }
        if trimmed.is_empty() {
            flush_para(&mut para, &mut out, link);
            flush_quote(&mut quote, &mut out, link);
            flush_list(&mut list, &mut out, link);
            continue;
        }
        // Heading.
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
            flush_para(&mut para, &mut out, link);
            flush_quote(&mut quote, &mut out, link);
            flush_list(&mut list, &mut out, link);
            if hashes == 1 && !seen_block {
                seen_block = true;
                continue;
            }
            let level = hashes.max(2);
            writeln!(
                out,
                "<h{level}>{}</h{level}>",
                inline(trimmed[hashes..].trim(), link)
            )
            .unwrap();
            seen_block = true;
            continue;
        }
        seen_block = true;
        if trimmed == "---" || trimmed == "***" {
            flush_para(&mut para, &mut out, link);
            flush_quote(&mut quote, &mut out, link);
            flush_list(&mut list, &mut out, link);
            out.push_str("<hr>\n");
            continue;
        }
        if let Some(q) = trimmed.strip_prefix('>') {
            flush_para(&mut para, &mut out, link);
            flush_list(&mut list, &mut out, link);
            quote.push(q.trim_start());
            continue;
        }
        // List item: `- x`, `* x`, `1. x`.
        let bullet = ["- ", "* "]
            .iter()
            .find_map(|b| trimmed.strip_prefix(b))
            .map(|t| ("ul", t));
        let numbered = || {
            let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
            (digits > 0)
                .then(|| trimmed[digits..].strip_prefix(". "))
                .flatten()
                .map(|t| ("ol", t))
        };
        if let Some((tag, item)) = bullet.or_else(numbered) {
            flush_para(&mut para, &mut out, link);
            flush_quote(&mut quote, &mut out, link);
            if list.as_ref().is_some_and(|(t, _, _)| *t != tag) {
                flush_list(&mut list, &mut out, link);
            }
            let (_, items, current) = list.get_or_insert_with(|| (tag, Vec::new(), Vec::new()));
            if !current.is_empty() {
                items.push(inline(&current.join("\n"), link));
                current.clear();
            }
            current.push(item.trim());
            continue;
        }
        // An indented line continues the open list item; anything else continues the paragraph or quote.
        if let Some((_, _, current)) = list.as_mut()
            && line.starts_with([' ', '\t'])
        {
            current.push(trimmed);
            continue;
        }
        flush_list(&mut list, &mut out, link);
        if !quote.is_empty() {
            quote.push(trimmed);
        } else {
            para.push(trimmed);
        }
    }
    flush_para(&mut para, &mut out, link);
    flush_quote(&mut quote, &mut out, link);
    flush_list(&mut list, &mut out, link);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchor(kind: &str, id: &str, label: Option<&str>) -> String {
        format!(
            "<a href=\"{kind}/{id}.html\">{}</a>",
            esc(label.unwrap_or(id))
        )
    }
    const LINK: LinkFn = &anchor;

    #[test]
    fn escapes_everything_from_content() {
        assert_eq!(
            esc(r#"<a href="x">&'"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        assert_eq!(
            inline("1 < 2 & <b>bold</b>", LINK),
            "1 &lt; 2 &amp; &lt;b&gt;bold&lt;/b&gt;"
        );
        assert_eq!(
            inline("`<x>` and [[part:a|<b>]]", LINK),
            "<code>&lt;x&gt;</code> and <a href=\"part/a.html\">&lt;b&gt;</a>"
        );
    }

    #[test]
    fn inline_marks() {
        assert_eq!(
            inline("a **b** *c* `d` [[part:e]]", LINK),
            "a <strong>b</strong> <em>c</em> <code>d</code> <a href=\"part/e.html\">e</a>"
        );
        assert_eq!(
            inline("**see [[part:x|it]]**", LINK),
            "<strong>see <a href=\"part/x.html\">it</a></strong>"
        );
        assert_eq!(
            inline("*a **b** c*", LINK),
            "<em>a <strong>b</strong> c</em>"
        );
        // Not emphasis: a lone star, a star before a space, an unclosed marker, a malformed link.
        assert_eq!(
            inline("2 * 3 and *open and [[bad]]", LINK),
            "2 * 3 and *open and [[bad]]"
        );
        assert_eq!(inline("cycle(9s) >= 5s", LINK), "cycle(9s) &gt;= 5s");
    }

    #[test]
    fn markdown_blocks() {
        let md = "# Title\n\nFirst *line*\nsecond line.\n\n## Part\n\n- one\n- two\n  continued\n\n1. a\n2. b\n\n> quoted\n\n```rhai\nlet x = a < b;\n```\n\n---\n<div>raw</div>\n";
        let mut w = Vec::new();
        let html = markdown(md, LINK, &mut w);
        assert!(w.is_empty());
        assert_eq!(
            html,
            "<p>First <em>line</em>\nsecond line.</p>\n<h2>Part</h2>\n<ul>\n<li>one</li>\n<li>two\ncontinued</li>\n</ul>\n\
             <ol>\n<li>a</li>\n<li>b</li>\n</ol>\n<blockquote><p>quoted</p></blockquote>\n\
             <pre><code class=\"lang-rhai\">let x = a &lt; b;</code></pre>\n<hr>\n<p>&lt;div&gt;raw&lt;/div&gt;</p>\n"
        );
    }

    #[test]
    fn markdown_keeps_a_later_h1_and_warns_on_an_open_fence() {
        let mut w = Vec::new();
        assert_eq!(
            markdown("Intro.\n\n# Big\n", LINK, &mut w),
            "<p>Intro.</p>\n<h2>Big</h2>\n"
        );
        assert_eq!(
            markdown("```\ncode", LINK, &mut w),
            "<pre><code>code</code></pre>\n"
        );
        assert_eq!(w.len(), 1);
    }
}
