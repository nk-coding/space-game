//! The wiki's hand-written half (`wiki/`: the guide, mechanics and behavior pages, the theme): reading it from disk
//! and checking it against the content, so it stays complete and consistent (`docs/wiki.md` "Checks").
//!
//! [`check`] fails a page that references an entry, a value or a sim constant that does not exist, a rule example
//! (a ```` ```behavior ```` or ```` ```assists ```` block) that the game's parser or vocabulary refuses, a component
//! that names missing content, a page without a title or description, a page missing from the navigation
//! (`nav.json`) or a navigation entry without a page, and any raster image (a raster file anywhere in the wiki, an
//! `<img>`, a raster `url(...)` or `src`): the wiki is vector only. It also fails when an entry of the evolution lab's
//! documented lists ([`pages::LAB_DOCUMENTED`]: every trial kind, trial, goal, gene, pilot, size and preset) is named
//! on no hand-written page.

use std::path::Path;

use data::behavior::{ListKind, check_text_as};
use data::input::Section;

use super::*;
use refs::Refs;

/// A file of the wiki's hand-written half: its path relative to `wiki/` and, for text files, its contents.
pub struct HandFile {
    pub path: String,
    pub text: Option<String>,
}

/// Directories of `wiki/` that are not hand-written: dependencies, the generated data, VitePress's cache and output.
const SKIP: [&str; 4] = ["node_modules", "gen", ".vitepress/cache", ".vitepress/dist"];

/// Raster image formats: none may appear in the wiki.
const RASTER: [&str; 12] = [
    "png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "ico", "tif", "tiff", "heic", "jxl",
];

/// File types read as text (the others are only checked by name).
const TEXT: [&str; 11] = [
    "md", "vue", "css", "ts", "mts", "js", "mjs", "json", "svg", "html", "txt",
];

/// The components the site's theme registers that a page may use, with what their attributes name.
const COMPONENTS: [&str; 11] = [
    "HomePage",
    "ScrollBox",
    "PartCard",
    "PartGrid",
    "ShipCard",
    "ControlsTable",
    "NodeTable",
    "StatTable",
    "PartsTable",
    "KindGrid",
    "ShipGallery",
];

/// Reads every file of the wiki directory except [`SKIP`], sorted by path.
pub fn read(dir: &Path) -> Result<Vec<HandFile>, String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<HandFile>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        for e in entries {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let rel = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if SKIP.contains(&rel.as_str()) {
                continue;
            }
            let ty = e.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                walk(root, &path, out)?;
            } else if ty.is_file() {
                let ext = rel.rsplit_once('.').map_or("", |(_, x)| x).to_ascii_lowercase();
                let text = if TEXT.contains(&ext.as_str()) {
                    Some(std::fs::read_to_string(&path).map_err(|e| format!("cannot read {rel}: {e}"))?)
                } else {
                    None
                };
                out.push(HandFile { path: rel, text });
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// What [`check`] found: the problems, and every rule example rendered (its text → its figure).
pub(super) struct Checked {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub examples: BTreeMap<String, Value>,
}

/// A page's Markdown lines outside fenced code blocks, with their line numbers, and the fenced blocks with their info
/// string and first line.
struct Split<'a> {
    prose: Vec<(usize, &'a str)>,
    fences: Vec<(usize, &'a str, String)>,
}

fn split(text: &str) -> Split<'_> {
    let mut prose = Vec::new();
    let mut fences = Vec::new();
    let mut open: Option<(usize, &str, &str, Vec<&str>)> = None;
    for (k, line) in text.lines().enumerate() {
        let n = k + 1;
        let t = line.trim_start();
        match &mut open {
            Some((_, marker, _, body)) => {
                if t.starts_with(*marker) && t.trim_end().trim_start_matches(marker.chars().next().unwrap()).is_empty() {
                    let (first, _, info, body) = open.take().unwrap();
                    fences.push((first, info, body.join("\n")));
                } else {
                    body.push(line);
                }
            }
            None => {
                let marker = if t.starts_with("```") {
                    Some("```")
                } else if t.starts_with("~~~") {
                    Some("~~~")
                } else {
                    None
                };
                match marker {
                    Some(m) => open = Some((n, m, t.trim_start_matches(m.chars().next().unwrap()).trim(), Vec::new())),
                    None => prose.push((n, line)),
                }
            }
        }
    }
    if let Some((first, _, info, body)) = open {
        fences.push((first, info, body.join("\n")));
    }
    Split { prose, fences }
}

/// A line without its inline code spans (`` `[[part:x]]` `` shows the syntax and is no reference).
fn without_code(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(k) = rest.find('`') {
        out.push_str(&rest[..k]);
        let run = rest[k..].chars().take_while(|&c| c == '`').count();
        let after = &rest[k + run..];
        let fence = "`".repeat(run);
        match after.find(&fence) {
            Some(e) => rest = &after[e + run..],
            None => {
                out.push_str(&rest[k..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The `name="value"` attributes of the component tags `<Name …>` on a line.
fn components(line: &str) -> Vec<(String, BTreeMap<String, String>)> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(k) = rest.find('<') {
        rest = &rest[k + 1..];
        let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
            continue;
        }
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[name.len()..end];
        let mut attrs = BTreeMap::new();
        let mut a = tag;
        while let Some(eq) = a.find("=\"") {
            let key = a[..eq].rsplit(|c: char| c.is_whitespace()).next().unwrap_or("").to_string();
            let v = &a[eq + 2..];
            let close = v.find('"').unwrap_or(v.len());
            attrs.insert(key, v[..close].to_string());
            a = &v[(close + 1).min(v.len())..];
        }
        // Flag attributes (`pilot`).
        for word in a.split_whitespace().chain(tag.split_whitespace()) {
            let w = word.trim_end_matches('/');
            if !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase()) {
                attrs.entry(w.to_string()).or_default();
            }
        }
        out.push((name, attrs));
        rest = &rest[end..];
    }
    out
}

/// The navigation (`nav.json`): sections of page paths without extension (`mechanics/flight`).
fn nav_pages(files: &[HandFile], errors: &mut Vec<String>) -> Vec<String> {
    let Some(text) = files.iter().find(|f| f.path == "nav.json").and_then(|f| f.text.as_deref()) else {
        errors.push("nav.json: missing (the navigation of the hand-written pages)".into());
        return Vec::new();
    };
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("nav.json: {e}"));
            return Vec::new();
        }
    };
    let mut pages = Vec::new();
    for section in v.as_array().into_iter().flatten() {
        if section["text"].as_str().is_none() {
            errors.push("nav.json: a section without `text`".into());
        }
        for item in section["items"].as_array().into_iter().flatten() {
            match item.as_str() {
                Some(p) => pages.push(p.to_string()),
                None => errors.push(format!("nav.json: an item that is not a page path: {item}")),
            }
        }
    }
    pages
}

/// Checks the hand-written pages against the content and renders their rule examples.
pub(super) fn check(w: &Wiki, refs: &Refs, files: &[HandFile]) -> Checked {
    let c = w.c;
    let mut errors = Vec::new();
    let mut examples = BTreeMap::new();
    let nav = nav_pages(files, &mut errors);
    // Every reference the pages make (without its label), for the lab's documented lists.
    let mut named: BTreeSet<String> = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for p in &nav {
        if !seen.insert(p.as_str()) {
            errors.push(format!("nav.json: `{p}` is listed twice"));
        }
        if !files.iter().any(|f| f.path == format!("{p}.md")) {
            errors.push(format!("nav.json: `{p}` has no page ({p}.md)"));
        }
    }
    for f in files {
        let lower = f.path.to_ascii_lowercase();
        let ext = lower.rsplit_once('.').map_or("", |(_, x)| x);
        if RASTER.contains(&ext) {
            errors.push(format!("{}: a raster image; the wiki is vector only (SVG, CSS, fonts)", f.path));
        }
        let Some(text) = &f.text else { continue };
        // Raster references in what the site renders: pages, components and styles.
        if ["md", "vue", "css"].contains(&ext) {
            for (k, line) in text.lines().enumerate() {
                let l = line.to_ascii_lowercase();
                let raster_ref = RASTER.iter().any(|x| {
                    l.contains(&format!(".{x}\"")) || l.contains(&format!(".{x})")) || l.contains(&format!(".{x}'"))
                        || l.contains(&format!("data:image/{x}"))
                });
                if l.contains("<img") || raster_ref {
                    errors.push(format!("{}:{}: a raster image; the wiki is vector only", f.path, k + 1));
                }
            }
        }
        if ext != "md" || f.path == "README.md" {
            continue;
        }
        let page = f.path.trim_end_matches(".md");
        if page != "index" && !nav.iter().any(|p| p == page) {
            errors.push(format!("{}: not in nav.json, so no reader finds it", f.path));
        }
        // Front matter: a title and a description.
        let front = text
            .strip_prefix("---\n")
            .and_then(|t| t.split_once("\n---").map(|(fm, _)| fm))
            .unwrap_or("");
        for key in ["title:", "description:"] {
            match front.lines().find(|l| l.starts_with(key)).map(|l| l[key.len()..].trim()) {
                Some(v) if !v.is_empty() => {
                    // YAML reads `a: b` or ` #` in a plain value as structure or a comment.
                    if !v.starts_with(['"', '\'']) && (v.contains(": ") || v.contains(" #")) {
                        errors.push(format!("{}: quote the front matter's `{key}` value", f.path));
                    }
                }
                _ => errors.push(format!("{}: the front matter needs `{key}`", f.path)),
            }
        }
        let parts = split(text);
        for (n, line) in &parts.prose {
            let line = without_code(line);
            let mut rest = line.as_str();
            while let Some(k) = rest.find("[[") {
                let after = &rest[k + 2..];
                let Some(e) = after.find("]]") else { break };
                if let Err(why) = refs.check(&after[..e]) {
                    errors.push(format!("{}:{n}: [[{}]]: {why}", f.path, &after[..e]));
                }
                named.insert(after[..e].split('|').next().unwrap_or_default().to_string());
                rest = &after[e + 2..];
            }
            for (name, attrs) in components(&line) {
                let problem = component_problem(c, &name, &attrs);
                if let Some(why) = problem {
                    errors.push(format!("{}:{n}: <{name}>: {why}", f.path));
                }
            }
        }
        for (n, info, body) in &parts.fences {
            let kind = match info.split_whitespace().next() {
                Some("behavior") => ListKind::Behavior,
                Some("assists") => ListKind::Assists,
                _ => continue,
            };
            match check_text_as(body, &c.behavior_nodes, kind) {
                Ok(program) => {
                    let what = if kind == ListKind::Assists { "assists" } else { "behavior" };
                    examples.insert(
                        body.trim_end().to_string(),
                        json!({ "kind": what, "html": w.rules_figure(ROOT, &program, kind) }),
                    );
                }
                Err(errs) => {
                    for e in errs {
                        errors.push(format!("{}:{}: rule example: {e}", f.path, n + e.line as usize));
                    }
                }
            }
        }
    }
    for e in w.lab_entries(ROOT) {
        let list = e.key.split_once('.').map_or("", |(l, _)| l);
        let key = format!("lab:{}", e.key);
        let documented = named.iter().any(|r| *r == key || r.strip_prefix(&key).is_some_and(|f| f.starts_with('.')));
        if pages::LAB_DOCUMENTED.contains(&list) && !documented {
            errors.push(format!("[[{key}]]: no hand-written page names it (the evolution lab's pages document every {list})"));
        }
    }
    Checked {
        errors,
        warnings: Vec::new(),
        examples,
    }
}

/// What is wrong with a component's attributes, if anything.
fn component_problem(c: &Content, name: &str, attrs: &BTreeMap<String, String>) -> Option<String> {
    if !COMPONENTS.contains(&name) {
        // VitePress's own (`<Badge>`, `<Content>`) and Vue's are fine; anything else is a typo.
        return (!["Badge", "Content", "ClientOnly"].contains(&name))
            .then(|| format!("unknown component; the theme has {}", COMPONENTS.join(", ")));
    }
    let get = |k: &str| attrs.get(k).map(String::as_str);
    match name {
        "PartCard" => match get("id") {
            Some(id) if c.parts.contains(id) => None,
            Some(id) => Some(format!("no part `{id}`")),
            None => Some("needs id=\"<part>\"".into()),
        },
        "ShipCard" => match get("id") {
            Some(id) if c.ships.contains(id) => None,
            Some(id) => Some(format!("no ship `{id}`")),
            None => Some("needs id=\"<ship>\"".into()),
        },
        "PartGrid" => get("category").filter(|k| !c.categories.contains(k)).map(|k| format!("no category `{k}`")),
        "ControlsTable" => match get("section") {
            Some(s) if Section::ALL.iter().any(|x| section_key(*x) == s) => None,
            Some(s) => Some(format!("no controls section `{s}`")),
            None => Some("needs section=\"<section>\"".into()),
        },
        "NodeTable" => {
            if let Some(r) = get("role").filter(|r| !["target", "condition", "action"].contains(r)) {
                return Some(format!("role `{r}` is none of target, condition, action"));
            }
            get("tag")
                .filter(|t| !c.behavior_nodes.iter().any(|n| n.tags.iter().any(|x| x == t)))
                .map(|t| format!("no node is tagged `{t}`"))
        }
        "StatTable" => {
            let stats = get("stats")?;
            stats.split(',').map(str::trim).find_map(|s| {
                if !c.stats.contains(s) {
                    Some(format!("no stat `{s}`"))
                } else if !c.parts.iter().any(|p| c.part_stats(p, None).iter().any(|l| l.stat == s && l.bonus.is_none())) {
                    Some(format!("no part has the stat `{s}`"))
                } else {
                    None
                }
            })
        }
        _ => None,
    }
}

/// A controls section as `controls.ron` writes it (`flight`, `mission`, …).
pub(super) fn section_key(s: Section) -> String {
    serde_json::to_value(s).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}
