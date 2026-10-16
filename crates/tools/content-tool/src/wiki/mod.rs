//! `content-tool wiki-data`: the generated half of the wiki (`wiki/`, a VitePress site; ADR-0039, `docs/wiki.md`).
//!
//! [`generate`] is pure: content and the wiki's hand-written pages in, files out (`path → bytes`, sorted) for
//! `wiki/gen/`, so two runs give identical bytes and the tests need no disk; [`write`] puts them into the directory.
//! What it writes:
//!
//! * one page per entry of every kind, `<kind dir>/<id>.md` (`parts/fission_reactor.md`; free articles under
//!   `pages/`), and one list per kind, `<kind dir>/index.md`, plus `reference/index.md` (every kind, the packs).
//!   The evolution lab's page, `reference/lab.md`, shows `lab.ron`.
//!   The site serves them at the same paths (`parts/fission_reactor.html`). They are Markdown files that hold
//!   only HTML blocks, which Vue leaves alone (`v-pre`), with the headings the site's outline and search use;
//! * `refs.json`: what hand-written pages may reference (`[[kind:id]]`, `[[kind:id.field]]`, `[[sim:name]]`,
//!   `[[lab:list.id]]`) and
//!   the rendered rule examples, for the site's Markdown plugin at build time;
//! * `ui.json` (what the site's components show: cards, the parts table, controls, nodes) and `sidebar.json`;
//! * `sprite.svg` (the pack's icons as symbols), `tokens.css` (`art/tokens.json` as CSS variables) and `fonts/`.
//!
//! It also checks the hand-written pages ([`hand`]): every reference resolves, every rule example parses and
//! checks against the vocabulary, every component names existing content, every page is in the navigation and
//! no raster image is used anywhere. Those problems are [`Site::errors`]: the command fails on them.
//!
//! Every name, summary, stat label and unit comes from the content (the keyed English text, ADR-0018); colours
//! come from `art/tokens.json`; icons are the pack's SVGs; ships, parts and rule graphs are drawn as SVG from the
//! data. The headings and words around them ("Used by", "Locked") are the tool's own, like the game's UI strings.

mod draw;
mod graph;
pub mod hand;
mod markup;
mod pages;
mod refs;
#[cfg(test)]
mod tests;
mod ui;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use data::design::geometry::hull_loops;
use data::design::stats::{SHIP_STATS, format_number, with_unit};
use data::design::tone::{HullTones, armor_tone};
use data::{
    Behavior, BehaviorNode, Better, Blueprint, Content, Delivery, Encounter, Entry, Family, Kind,
    Material, NodeRole, ParamType, Part, PlacedShape, ResearchKind, ResearchNode, Selector, Start,
    StatLine, Unlock, Voyage,
};
use serde_json::{Value, json};
use sim::DesignReport;

use draw::{Art, Glow, Style, Tile};
use markup::{esc, inline, markdown};

/// The generated files: path (relative to `wiki/gen/`, `/`-separated) → contents, sorted by path.
pub struct Site {
    pub files: BTreeMap<String, Vec<u8>>,
    /// Problems found while generating (an unclosed code fence in a prose file, a broken icon). The base pack
    /// has none.
    pub warnings: Vec<String>,
    /// Problems of the hand-written pages: an unknown reference, a rule example that does not check, a page
    /// missing from the navigation, a raster image. The command fails on any.
    pub errors: Vec<String>,
}

/// The kinds in navigation order, with the titles of their list pages. Every [`Kind`] is here (a test checks).
pub const KINDS: [(Kind, &str); 17] = [
    (Kind::Part, "Parts"),
    (Kind::Material, "Materials"),
    (Kind::Category, "Categories"),
    (Kind::Ship, "Ships"),
    (Kind::Frame, "Frames"),
    (Kind::Faction, "Factions"),
    (Kind::Research, "Research"),
    (Kind::Family, "Families"),
    (Kind::Facility, "Facilities"),
    (Kind::Start, "Starts"),
    (Kind::Voyage, "Voyages"),
    (Kind::Encounter, "Encounters"),
    (Kind::Behavior, "Behaviors"),
    (Kind::BehaviorNode, "Behavior nodes"),
    (Kind::Stat, "Stats"),
    (Kind::Control, "Controls"),
    (Kind::Page, "Articles"),
];

/// Stat lines as `(stat id, value text)`.
type Lines = Vec<(String, String)>;

/// Generated pages sit one directory down (`parts/x.md`) and link to each other through this.
const UP: &str = "../";
/// The data files (`refs.json`, `ui.json`) link through this placeholder: the site replaces it with its base, so the
/// same HTML works on a page at any depth.
pub const ROOT: &str = "@@/";
/// Pixels per cell of the drawings: ship pages and ship cards.
const PX_SHIP: f32 = 34.0;
const PX_CARD: f32 = 14.0;
/// Table chips fit a square of this many pixels, whatever the part's size.
const CHIP_BOX: f32 = 34.0;
/// Margin around a drawing, in cells: room for the hull glow around ships, just the seams around parts.
const MARGIN_SHIP: f32 = 0.5;
const MARGIN_PART: f32 = 0.06;

/// Where the file written by [`write`] lists the files it made, so the next run can remove stale ones.
pub const MANIFEST: &str = ".wiki-files";

/// The directory of a kind's pages: its pack directory (`parts`), `pages` for the free articles.
pub fn kind_dir(kind: Kind) -> &'static str {
    kind.dir().unwrap_or("pages")
}

/// Where an entry's page is served, relative to the site root.
pub fn item_path(kind: Kind, id: &str) -> String {
    format!("{}/{id}.html", kind_dir(kind))
}

fn list_path(kind: Kind) -> String {
    format!("{}/index.html", kind_dir(kind))
}

fn kind_title(kind: Kind) -> &'static str {
    KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or("", |(_, t)| t)
}

/// One generated page: its front matter and its body (HTML blocks).
struct Page {
    title: String,
    description: String,
    /// The entry it shows (`None` for lists).
    entry: Option<(Kind, String)>,
    /// Leave out the outline column, for a page whose table needs the width.
    wide: bool,
    body: String,
}

impl Page {
    /// The Markdown file: front matter (JSON strings are valid YAML scalars), then the blocks.
    fn markdown(&self) -> String {
        let q = |s: &str| serde_json::to_string(s).unwrap();
        let mut s = format!(
            "---\ntitle: {}\ndescription: {}\n",
            q(&self.title),
            q(&self.description)
        );
        if let Some((kind, id)) = &self.entry {
            writeln!(
                s,
                "entry:\n  kind: {}\n  id: {}\n  dir: {}\n  kindTitle: {}",
                kind.key(),
                q(id),
                kind_dir(*kind),
                q(kind_title(*kind))
            )
            .unwrap();
        }
        if self.wide {
            s.push_str("aside: false\n");
        }
        s.push_str("---\n\n");
        s.push_str(&self.body);
        s
    }
}

/// Generates every file of `wiki/gen/` from validated content, and checks the hand-written pages against it.
pub fn generate(c: &Content, hand: &[hand::HandFile]) -> Site {
    let w = Wiki::new(c);
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut put = |path: String, page: Page| {
        files.insert(path, page.markdown().into_bytes());
    };

    for (kind, title) in KINDS {
        put(
            format!("{}/index.md", kind_dir(kind)),
            Page {
                title: title.to_string(),
                description: format!("Every entry of the kind “{title}” in the loaded content."),
                entry: None,
                wide: kind == Kind::Part,
                body: w.list(kind),
            },
        );
    }
    put(
        "reference/index.md".into(),
        Page {
            title: "Reference".into(),
            description: "Every kind of game content, generated from the content packs.".into(),
            entry: None,
            wide: false,
            body: w.reference(),
        },
    );
    put(
        format!("{}.md", pages::LAB_PAGE),
        Page {
            title: "Evolution lab".into(),
            description: "The evolution lab's goals, trials, genes, stand-in pilots, sizes and measures, from the content.".into(),
            entry: None,
            wide: false,
            body: w.lab_page(),
        },
    );
    // One page per entry, below the list of its kind.
    let mut entry = |kind: Kind, id: &str, name: &str, summary: &str, body: String| {
        put(
            format!("{}/{id}.md", kind_dir(kind)),
            Page {
                title: name.to_string(),
                description: c.render_plain(summary),
                entry: Some((kind, id.to_string())),
                wide: false,
                body,
            },
        );
    };
    for e in &c.stats {
        entry(Kind::Stat, &e.id, &e.name, &e.summary, w.stat_page(e));
    }
    for e in &c.categories {
        entry(Kind::Category, &e.id, &e.name, &e.summary, w.category_page(e));
    }
    for e in &c.factions {
        entry(Kind::Faction, &e.id, &e.name, &e.summary, w.faction_page(e));
    }
    for e in &c.materials {
        entry(Kind::Material, &e.id, &e.name, &e.summary, w.material_page(e));
    }
    for e in &c.parts {
        entry(Kind::Part, &e.id, &e.name, &e.summary, w.part_page(e));
    }
    for e in &c.frames {
        entry(Kind::Frame, &e.id, &e.name, &e.summary, w.frame_page(e));
    }
    for (k, e) in c.ships.iter().enumerate() {
        entry(Kind::Ship, &e.id, &e.name, &e.summary, w.ship_page(k, e));
    }
    for e in &c.behavior_nodes {
        entry(Kind::BehaviorNode, &e.id, &e.name, &e.summary, w.node_page(e));
    }
    for e in &c.behaviors {
        entry(Kind::Behavior, &e.id, &e.name, &e.summary, w.behavior_page(e));
    }
    for e in &c.families {
        entry(Kind::Family, &e.id, &e.name, &e.summary, w.family_page(e));
    }
    for e in &c.research {
        entry(Kind::Research, &e.id, &e.name, &e.summary, w.research_page(e));
    }
    for e in &c.facilities {
        entry(Kind::Facility, &e.id, &e.name, &e.summary, w.facility_page(e));
    }
    for e in &c.starts {
        entry(Kind::Start, &e.id, &e.name, &e.summary, w.start_page(e));
    }
    for e in &c.voyages {
        entry(Kind::Voyage, &e.id, &e.name, &e.summary, w.voyage_page(e));
    }
    for e in &c.encounters {
        entry(Kind::Encounter, &e.id, &e.name, &e.summary, w.encounter_page(e));
    }
    for e in &c.controls {
        entry(Kind::Control, &e.id, &e.name, &e.summary, w.control_page(e));
    }
    for p in c.wiki.values().filter(|p| p.kind == Kind::Page) {
        let title = c.name_of(Kind::Page, &p.id).unwrap_or(&p.id);
        entry(Kind::Page, &p.id, title, "", w.article(&p.id));
    }

    let refs = refs::Refs::new(&w);
    let checked = hand::check(&w, &refs, hand);
    let json = |v: &Value| (serde_json::to_string_pretty(v).unwrap() + "\n").into_bytes();
    files.insert("refs.json".into(), json(&refs.to_json(&checked.examples)));
    files.insert("ui.json".into(), json(&w.ui()));
    files.insert("sidebar.json".into(), json(&w.sidebar()));
    files.insert("sprite.svg".into(), w.sprite().into_bytes());
    files.insert("tokens.css".into(), w.tokens_css().into_bytes());
    for (name, bytes) in FONTS {
        files.insert(format!("fonts/{name}"), bytes.to_vec());
    }
    let mut warnings = w.warnings.into_inner();
    warnings.extend(checked.warnings);
    Site {
        files,
        warnings,
        errors: checked.errors,
    }
}

/// The game's UI font (style guide: Barlow), with its licence, so the site looks like the game offline.
const FONTS: [(&str, &[u8]); 3] = [
    (
        "Barlow-Regular.ttf",
        include_bytes!("../../../../client/kit/assets/fonts/Barlow-Regular.ttf"),
    ),
    (
        "Barlow-Medium.ttf",
        include_bytes!("../../../../client/kit/assets/fonts/Barlow-Medium.ttf"),
    ),
    (
        "OFL.txt",
        include_bytes!("../../../../client/kit/assets/fonts/OFL.txt"),
    ),
];

/// Writes the files into `dir` (created if needed). A directory that is not empty must hold the file list of an
/// earlier run ([`MANIFEST`]); files of that run that are gone now are removed, nothing else is touched.
pub fn write(site: &Site, dir: &Path) -> Result<(), String> {
    let manifest = dir.join(MANIFEST);
    let old: Vec<String> = match std::fs::read_to_string(&manifest) {
        Ok(text) => text.lines().map(str::to_string).collect(),
        Err(_) => {
            let empty = std::fs::read_dir(dir).map_or(true, |mut d| d.next().is_none());
            if !empty {
                return Err(format!(
                    "{} is not empty and holds no wiki data made by content-tool ({MANIFEST}); pick an empty or new directory",
                    dir.display()
                ));
            }
            Vec::new()
        }
    };
    for path in old.iter().filter(|p| !site.files.contains_key(*p)) {
        // Only plain relative paths of the shape this tool writes.
        let safe = !path.is_empty()
            && path
                .split('/')
                .all(|s| !s.is_empty() && s != "." && s != "..")
            && !path.contains('\\');
        if safe {
            let file = dir.join(path);
            let _ = std::fs::remove_file(&file);
            if let Some(parent) = file.parent().filter(|p| *p != dir) {
                let _ = std::fs::remove_dir(parent); // only succeeds once it is empty
            }
        }
    }
    for (path, bytes) in &site.files {
        let file = dir.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        // Unchanged files keep their time stamp, so the site's dev server reloads only what changed.
        if std::fs::read(&file).is_ok_and(|old| old == *bytes) {
            continue;
        }
        std::fs::write(&file, bytes)
            .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    }
    let list: Vec<&str> = site.files.keys().map(String::as_str).collect();
    std::fs::write(&manifest, list.join("\n") + "\n")
        .map_err(|e| format!("cannot write {}: {e}", manifest.display()))
}

// ---------------------------------------------------------------------------------------------------------
// HTML blocks of a page

/// An id for a heading: lowercase letters, digits and dashes.
fn slug(text: &str) -> String {
    let mut s = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    s.trim_end_matches('-').to_string()
}

/// A heading as the site writes it (an id and a permalink anchor, which its outline and search read), as raw HTML
/// that Vue leaves alone: `text` may be content (a name), which must not be read as a template.
fn heading(level: u8, html: &str, id: &str) -> String {
    format!(
        "<h{level} id=\"{id}\" tabindex=\"-1\" v-pre>{html} <a class=\"header-anchor\" href=\"#{id}\" aria-hidden=\"true\">&#8203;</a></h{level}>\n\n"
    )
}

/// An HTML block of a page: one `<div>` that Vue leaves alone (`v-pre`, content text must not be read as a template),
/// without blank lines (one would end the block for the Markdown reader). Line breaks inside `<pre>` become
/// `&#10;`, and every table gets its `<tbody>` (the browser would insert one, and the page would not match what the
/// server rendered).
fn block(class: &str, html: &str) -> String {
    format!("<div class=\"{class}\" v-pre>\n{}\n</div>\n\n", tidy(html))
}

/// A [`block`] that may be wider than the page (tables, rule graphs): it scrolls sideways in the site's ScrollBox.
fn wide_block(class: &str, html: &str) -> String {
    format!(
        "<ScrollBox x>\n<div class=\"{class}\" v-pre>\n{}\n</div>\n</ScrollBox>\n\n",
        tidy(html)
    )
}

fn tidy(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 64);
    let mut rest = html;
    while let Some(k) = rest.find("<pre") {
        out.push_str(&rest[..k]);
        let end = rest[k..].find("</pre>").map_or(rest.len(), |e| k + e + 6);
        out.push_str(&rest[k..end].replace('\n', "&#10;"));
        rest = &rest[end..];
    }
    out.push_str(rest);
    let mut s = String::with_capacity(out.len() + 64);
    let mut rest = out.as_str();
    while let Some(k) = rest.find("<table") {
        let open = rest[k..].find('>').map_or(rest.len(), |e| k + e + 1);
        s.push_str(&rest[..open]);
        if !rest[open..].starts_with("<tbody>") && !rest[open..].starts_with("\n<tbody>") {
            s.push_str("<tbody>");
        }
        rest = &rest[open..];
        if let Some(close) = rest.find("</table>") {
            let inner = &rest[..close];
            s.push_str(inner);
            if !inner.trim_end().ends_with("</tbody>") {
                s.push_str("</tbody>");
            }
            s.push_str("</table>");
            rest = &rest[close + 8..];
        }
    }
    s.push_str(rest);
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The generator: content plus what every page needs (style, icon symbols, design reports, back-links).
struct Wiki<'a> {
    c: &'a Content,
    style: Style,
    /// Symbol id → `<symbol>` for the site's sprite: every icon and every machine's ship drawing.
    symbols: BTreeMap<String, String>,
    /// The sim's design report of every ship (the drydock's numbers), in ship order.
    reports: Vec<Option<DesignReport>>,
    /// Entry → the entries whose texts or prose link to it, in content order.
    mentions: BTreeMap<(Kind, String), Vec<(Kind, String)>>,
    warnings: RefCell<Vec<String>>,
}

impl<'a> Wiki<'a> {
    fn new(c: &'a Content) -> Wiki<'a> {
        let mut warnings = Vec::new();
        let icons = c.icons.values().map(|i| (i, draw::icon_id(&i.key)));
        let ship_art = c.ship_art.values().map(|i| (i, draw::ship_art_id(&i.key)));
        let symbols = icons
            .chain(ship_art)
            .filter_map(|(i, id)| match draw::symbol(&id, &i.svg) {
                Ok(s) => Some((id, s)),
                Err(e) => {
                    warnings.push(format!("icon `{id}` ({}): {e}", i.source));
                    None
                }
            })
            .collect();
        let reports = c
            .ships
            .iter()
            .map(
                |s| match sim::design_report(c, s, &sim::SimConfig::default()) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        warnings.push(format!("ship `{}`: no design report: {e}", s.id));
                        None
                    }
                },
            )
            .collect();
        let mut mentions: BTreeMap<(Kind, String), Vec<(Kind, String)>> = BTreeMap::new();
        let mut add = |from: Kind, from_id: &str, text: &str| {
            for l in data::parse_links(text).into_iter().flatten() {
                let Some(kind) = Kind::from_key(l.kind) else {
                    continue;
                };
                let src = (from, from_id.to_string());
                let list = mentions.entry((kind, l.id.to_string())).or_default();
                if (kind, l.id) != (from, from_id) && !list.contains(&src) {
                    list.push(src);
                }
            }
        };
        c.for_each_entry_text(|kind, id, _, text| add(kind, id, text));
        for n in &c.behavior_nodes {
            for (_, text) in n.param_texts() {
                add(Kind::BehaviorNode, &n.id, text);
            }
        }
        for p in c.wiki.values() {
            add(p.kind, &p.id, &p.markdown);
        }
        Wiki {
            c,
            style: Style::embedded(),
            symbols,
            reports,
            mentions,
            warnings: RefCell::new(warnings),
        }
    }

    fn warn(&self, message: String) {
        self.warnings.borrow_mut().push(message);
    }

    /// Every icon and ship drawing as a `<symbol>`, plus the shared filter: the site puts this once into every page.
    fn sprite(&self) -> String {
        let symbols: Vec<String> = self.symbols.values().cloned().collect();
        draw::sprite(&self.style, &symbols)
    }

    /// The style tokens the site uses, as CSS variables, and the game's font.
    fn tokens_css(&self) -> String {
        let t = |p: &str| self.style.get(p);
        let mut s = String::from(
            "/* Generated by content-tool wiki-data from art/tokens.json: the game's colours for the wiki's theme. */\n",
        );
        for weight in [("Regular", 400), ("Medium", 500)] {
            writeln!(
                s,
                "@font-face {{ font-family: \"{}\"; src: url(\"./fonts/Barlow-{}.ttf\") format(\"truetype\"); font-weight: {}; font-display: swap; }}",
                t("ui.font"),
                weight.0,
                weight.1
            )
            .unwrap();
        }
        s.push_str(":root {\n");
        let vars = [
            ("bg", "ui.background"),
            ("teal", "ui.teal"),
            ("gold", "ui.gold"),
            ("coral", "ui.coral"),
            ("violet", "ui.violet"),
            ("label", "ui.label"),
            ("dim", "ui.dim"),
            ("number", "ui.number"),
            ("ring", "ui.dotted_ring"),
            ("particle", "ui.particle"),
            ("star", "ui.star"),
            ("blue", "ship.shield_bubble.color"),
            ("tile", "ship.machine_tile"),
            ("seam", "ship.seam"),
            ("glow", "ship.hull_glow.color"),
            ("glow-enemy", "ship.hull_glow.enemy_color"),
            ("darkest", "station.dark"),
            ("panel", "station.panel"),
        ];
        for (name, path) in vars {
            writeln!(s, "  --sg-{name}: {};", t(path)).unwrap();
        }
        for tone in data::content::art_keys::ARMOR_TONES {
            writeln!(s, "  --sg-armor-{tone}: {};", t(&format!("ship.armor.{tone}"))).unwrap();
        }
        for key in data::content::art_keys::SYSTEM_COLORS {
            writeln!(
                s,
                "  --sg-system-{key}: {};",
                t(&format!("ship.system_colors.{key}"))
            )
            .unwrap();
        }
        writeln!(s, "  --sg-font: \"{}\";\n}}", t("ui.font")).unwrap();
        s
    }

    // -----------------------------------------------------------------------------------------------------
    // Links and text

    /// An anchor to an entry with its name (or `label`); plain text and a warning if it does not exist.
    fn link(&self, root: &str, kind: &str, id: &str, label: Option<&str>) -> String {
        match Kind::from_key(kind).filter(|k| self.c.exists(*k, id)) {
            Some(k) => {
                let name = label.or(self.c.name_of(k, id)).unwrap_or(id);
                format!("<a href=\"{root}{}\">{}</a>", item_path(k, id), esc(name))
            }
            None => {
                self.warn(format!("link to a missing entry `[[{kind}:{id}]]`"));
                esc(label.unwrap_or(id))
            }
        }
    }

    fn a(&self, root: &str, kind: Kind, id: &str) -> String {
        self.link(root, kind.key(), id, None)
    }

    /// An anchor plus the "Locked" badge for parts, materials and frames a new game does not have (GDR-0054,
    /// GDR-0063).
    fn a_badged(&self, root: &str, kind: Kind, id: &str) -> String {
        format!(
            "{}{}",
            self.a(root, kind, id),
            if self.locked(kind, id) { LOCKED } else { "" }
        )
    }

    fn locked(&self, kind: Kind, id: &str) -> bool {
        match kind {
            Kind::Part => self.c.parts.get(id).is_some_and(|p| p.locked),
            Kind::Material => self.c.materials.get(id).is_some_and(|m| m.locked),
            Kind::Frame => self.c.frames.get(id).is_some_and(|f| f.locked),
            Kind::Facility => self.c.facilities.get(id).is_some_and(|f| f.locked),
            _ => false,
        }
    }

    /// A content text with its links and inline marks.
    fn text(&self, root: &str, text: &str) -> String {
        inline(text, &|k: &str, i: &str, l: Option<&str>| {
            self.link(root, k, i, l)
        })
    }

    /// Wiki prose as HTML blocks.
    fn prose(&self, root: &str, markdown_text: &str, source: &data::Source) -> String {
        let mut w = Vec::new();
        let html = markdown(
            markdown_text,
            &|k: &str, i: &str, l: Option<&str>| self.link(root, k, i, l),
            &mut w,
        );
        for m in w {
            self.warn(format!("{source}: {m}"));
        }
        html
    }

    /// A comma-separated list of anchors, or `None` when empty.
    fn links(&self, root: &str, items: impl IntoIterator<Item = (Kind, String)>) -> Option<String> {
        let v: Vec<String> = items
            .into_iter()
            .map(|(k, id)| self.a_badged(root, k, &id))
            .collect();
        (!v.is_empty()).then(|| v.join(", "))
    }

    // -----------------------------------------------------------------------------------------------------
    // Drawings

    fn tone(&self, m: Option<&Material>) -> String {
        match m.and_then(|m| m.armor.as_ref()) {
            Some(a) => self.style.get(&format!("ship.armor.{}", a.tone)),
            None => self.style.get("ship.machine_tile"),
        }
    }

    /// The material a hull part is shown in when none is given: the first unlocked armour material.
    fn default_armor(&self) -> Option<&Material> {
        let armor = || self.c.materials.iter().filter(|m| m.armor.is_some());
        armor().find(|m| !m.locked).or_else(|| armor().next())
    }

    /// One part's tile, unrotated: the drydock palette's look.
    fn part_tile(&self, p: &Part, material: Option<&Material>) -> Tile {
        let fill = if p.hull {
            self.tone(material.or(self.default_armor()))
        } else {
            self.tone(None)
        };
        Tile {
            shape: PlacedShape {
                footprint: p.footprint(),
                x: 0,
                y: 0,
                rot: 0,
                flip: false,
            },
            fill,
            art: p.icon.as_ref().filter(|_| !p.hull).map(|k| Art { symbol: draw::icon_id(k), overhang: 0.0 }),
        }
    }

    /// One part on its tile, unrotated: the drydock palette's look.
    fn part_svg(&self, p: &Part, material: Option<&Material>, px: f32, class: &str) -> String {
        draw::layout(
            &self.style,
            &[self.part_tile(p, material)],
            None,
            px,
            MARGIN_PART,
            class,
            &p.name,
        )
    }

    /// Every part, in category order (the drydock palette's order), then load order.
    fn parts_by_category(&self) -> Vec<&Part> {
        let mut v: Vec<&Part> = self.c.parts.iter().collect();
        v.sort_by_key(|p| {
            self.c
                .categories
                .position(&p.category)
                .unwrap_or(usize::MAX)
        });
        v
    }

    /// A part's tile scaled to fit a table row.
    fn chip(&self, p: &Part, material: Option<&Material>) -> String {
        self.part_svg(
            p,
            material,
            CHIP_BOX / p.size.0.max(p.size.1).max(1) as f32,
            "chip",
        )
    }

    /// A ship scaled to fit a table row: its longer side, glow margin included, is twice a part chip.
    fn ship_chip(&self, s: &Blueprint) -> String {
        let side = self
            .c
            .ship_grid(s)
            .ok()
            .and_then(|g| g.bounds())
            .map_or(1, |b| b.width().max(b.height()).max(1));
        self.ship_svg(
            s,
            2.0 * CHIP_BOX / (side as f32 + 2.0 * MARGIN_SHIP),
            "chip",
        )
    }

    /// A ship in style 2a, as the flight view draws it: armour tiles, machines as their housings (ship drawings) on a
    /// cell in the hull's tone (GDR-0069, `data::design::tone`), and the hull glow of its main piece in its faction's
    /// colour. A machine without a ship drawing shows its icon, as in the game.
    fn ship_svg(&self, s: &Blueprint, px: f32, class: &str) -> String {
        let Ok(grid) = self.c.ship_grid(s) else {
            return String::new();
        };
        let tones = HullTones::new(self.c, s, &grid);
        let armor = |tone: &str| self.style.get(&format!("ship.armor.{tone}"));
        let tiles: Vec<Tile> = s
            .parts
            .iter()
            .enumerate()
            .filter_map(|(i, pl)| {
                let part = self.c.parts.get(&pl.part)?;
                let shape = *grid.shape(i);
                if part.hull {
                    return Some(Tile { shape, fill: armor(armor_tone(self.c, pl)), art: None });
                }
                let art = part.icon.as_ref().map(|k| match self.c.ship_svg(k) {
                    Some(svg) => Art {
                        symbol: draw::ship_art_id(k),
                        overhang: data::content::svg::view_box(svg)
                            .and_then(|vb| data::content::svg::ship_overhang(vb, part.size).ok())
                            .unwrap_or(0.0),
                    },
                    None => Art { symbol: draw::icon_id(k), overhang: 0.0 },
                });
                Some(Tile { shape, fill: armor(tones.under(&grid, &shape.cells())), art })
            })
            .collect();
        let pieces = grid.pieces();
        let is_core = |i: usize| {
            self.c
                .parts
                .get(&s.parts[i].part)
                .is_some_and(|p| p.logic.is_some())
        };
        let glow = grid.main_piece(&pieces, is_core).map(|m| Glow {
            loops: hull_loops(&grid.piece_hull_edges(&pieces, m)),
            color: self.ship_glow(s),
        });
        draw::layout(
            &self.style,
            &tiles,
            glow.as_ref(),
            px,
            MARGIN_SHIP,
            class,
            &s.name,
        )
    }

    /// The hull glow colour of a ship's faction (teal for the player, coral for enemies).
    fn ship_glow(&self, s: &Blueprint) -> String {
        let key = self
            .c
            .factions
            .get(&s.faction)
            .map_or("player", |f| f.glow.as_str());
        self.style.glow(key)
    }

    fn color_dot(&self, key: &str) -> String {
        format!(
            "<span class=\"dot\" style=\"--c: {}\"></span>",
            esc(&self.style.get(&format!("ship.system_colors.{key}")))
        )
    }

    // -----------------------------------------------------------------------------------------------------
    // Stats

    /// A stat value as text (`10 MW`), formatted like the drydock; plain number if `stats.ron` lacks the stat.
    fn value(&self, stat: &str, v: f32) -> String {
        self.c
            .stat_line(stat, v)
            .map_or_else(|| format_number(v, 2), |l| l.text)
    }

    /// `used / of unit`, as the drydock shows power and control.
    fn pair(&self, stat: &str, used: f32, of: f32) -> String {
        let (decimals, unit) = self
            .c
            .stats
            .get(stat)
            .map_or((2, ""), |d| (d.decimals, d.unit.as_str()));
        format!(
            "{} / {}",
            format_number(used, decimals),
            with_unit(&format_number(of, decimals), unit)
        )
    }

    /// The ship-level numbers of the drydock's stats panel, from the sim's design report (full power, full use;
    /// GDR-0052), in `stats.ron` order. Yaw and device load are left out: the drydock shows them as words.
    fn ship_stats(&self, r: &DesignReport) -> Vec<(&'static str, String)> {
        let f = &r.flight;
        let moves = f.thrust.iter().any(|&t| t > 0.0);
        let mut v = vec![
            ("build_points", self.value("build_points", r.build_points)),
            ("mass", self.value("mass", r.mass)),
            ("hp", self.value("hp", r.hp)),
            (
                "power_use",
                self.pair("power_use", r.power.demand, r.power.supply),
            ),
            ("peak_heat", self.value("peak_heat", r.heat.peak * 100.0)),
            ("acceleration", self.value("acceleration", f.accel[0])),
            (
                "top_speed",
                if moves {
                    self.value("top_speed", f.top_speed[f.best])
                } else {
                    "—".into()
                },
            ),
            (
                "turn_time",
                f.turn_90
                    .map_or_else(|| "—".into(), |t| self.value("turn_time", t)),
            ),
            ("firepower", self.value("firepower", r.firepower)),
            (
                "control_load",
                self.pair(
                    "control_load",
                    r.control.load as f32,
                    r.control.capacity as f32,
                ),
            ),
            ("rule_slots", self.value("rule_slots", r.rule_slots as f32)),
        ];
        if r.cargo > 0.0 {
            v.push(("cargo_capacity", self.value("cargo_capacity", r.cargo)));
        }
        v.sort_by_key(|(s, _)| self.c.stats.position(s).unwrap_or(usize::MAX));
        v
    }

    /// A two-column stat table: label (linked to the stat's page, with its summary as tooltip) and value.
    fn stat_table(&self, root: &str, rows: &[(String, String)]) -> String {
        let mut s = String::from("<table class=\"stats\">\n");
        for (stat, value) in rows {
            writeln!(
                s,
                "<tr><th>{}</th><td>{}</td></tr>",
                self.stat_label(root, stat),
                esc(value)
            )
            .unwrap();
        }
        s + "</table>\n"
    }

    fn stat_label(&self, root: &str, stat: &str) -> String {
        match self.c.stats.get(stat) {
            Some(d) => format!(
                "<a href=\"{root}{}\" title=\"{}\">{}</a>",
                item_path(Kind::Stat, stat),
                esc(&self.c.render_plain(&d.summary)),
                esc(&d.name)
            ),
            None => esc(stat),
        }
    }

    fn plain_lines(lines: &[StatLine]) -> Lines {
        lines
            .iter()
            .filter(|l| l.bonus.is_none())
            .map(|l| (l.stat.clone(), l.text.clone()))
            .collect()
    }

    /// "+15% Fire rate for linked parts tagged laser (Pulse laser, Beam laser), at most 3 count, while it vents".
    fn bonus_text(&self, root: &str, l: &StatLine) -> String {
        let Some(b) = &l.bonus else {
            return String::new();
        };
        let target = match &b.target {
            Selector::Part(id) => format!("linked {}", self.a(root, Kind::Part, id)),
            Selector::Category(id) => format!("linked {} parts", self.a(root, Kind::Category, id)),
            Selector::Tag(tag) => {
                let parts = self
                    .c
                    .parts
                    .iter()
                    .filter(|p| p.has_tag(tag))
                    .map(|p| (Kind::Part, p.id.clone()));
                let list = self
                    .links(root, parts)
                    .map_or(String::new(), |l| format!(" ({l})"));
                format!("linked parts tagged <code>{}</code>{list}", esc(tag))
            }
        };
        let mut s = format!(
            "<strong>{}</strong> {} for {target}",
            esc(&l.text),
            self.stat_label(root, &l.stat)
        );
        if let Some(n) = b.max_stacks {
            write!(s, "; each counts at most {n}").unwrap();
        }
        for w in &b.when {
            match w {
                data::Condition::SourceVenting => s.push_str("; only while this part vents"),
            }
        }
        s
    }

    // -----------------------------------------------------------------------------------------------------
    // The entry page frame

    /// An entry page: the title, the lead (summary, texts, prose; the infobox beside it), the sections, mentions and
    /// the data details, as HTML blocks.
    fn entry<T: Entry>(&self, e: &T, parts: EntryParts) -> String {
        let root = UP;
        let mut out = heading(1, &esc(e.name()), &slug(e.id()));
        let mut main = String::new();
        writeln!(
            main,
            "<p class=\"summary\">{}{}</p>",
            self.text(root, e.summary()),
            parts.badges
        )
        .unwrap();
        if let Some(d) = e.description() {
            writeln!(main, "<p>{}</p>", self.text(root, d)).unwrap();
        }
        if let Some(l) = e.lore() {
            writeln!(
                main,
                "<blockquote class=\"lore\"><p>{}</p></blockquote>",
                self.text(root, l)
            )
            .unwrap();
        }
        if !e.tags().is_empty() {
            let tags: Vec<String> = e
                .tags()
                .iter()
                .map(|t| format!("<span class=\"tag\">{}</span>", esc(t)))
                .collect();
            writeln!(main, "<p class=\"tags\">{}</p>", tags.join(" ")).unwrap();
        }
        if let Some(p) = self.c.wiki_page(T::KIND, e.id()) {
            writeln!(
                main,
                "<div class=\"prose\">\n{}</div>",
                self.prose(root, &p.markdown, &p.source)
            )
            .unwrap();
        }
        let mut aside = String::new();
        if let Some(f) = &parts.figure {
            writeln!(aside, "<figure>{f}</figure>").unwrap();
        }
        if !parts.facts.is_empty() {
            aside.push_str("<dl class=\"facts\">\n");
            for (k, v) in &parts.facts {
                writeln!(aside, "<dt>{}</dt><dd>{v}</dd>", esc(k)).unwrap();
            }
            aside.push_str("</dl>\n");
        }
        aside.push_str(&parts.stats);
        let hero = parts.hero.unwrap_or_default();
        out.push_str(&block(
            "sg-entry",
            &format!(
                "{hero}<div class=\"sg-lead\">\n<div class=\"sg-main\">\n{main}</div>\n<aside class=\"sg-infobox\">\n{aside}</aside>\n</div>\n"
            ),
        ));
        for (title, html) in &parts.sections {
            out.push_str(&heading(2, &esc(title), &slug(title)));
            out.push_str(&wide_block("sg-section", html));
        }
        out.push_str(&self.mentioned(T::KIND, e.id()));
        out.push_str(&block(
            "sg-data",
            &self.data_details(T::KIND, e.id(), e.source(), &e.text_fields()),
        ));
        out
    }

    /// "Mentioned on": the entries whose texts or prose link here.
    fn mentioned(&self, kind: Kind, id: &str) -> String {
        match self.mentions.get(&(kind, id.to_string())) {
            Some(v) if !v.is_empty() => {
                let list = self.links(UP, v.iter().cloned()).unwrap_or_default();
                heading(2, "Mentioned on", "mentioned-on")
                    + wide_block("sg-section", &format!("<p>{list}</p>\n")).as_str()
            }
            _ => String::new(),
        }
    }

    /// For modders and translators: id, defining file, text keys and the prose file.
    fn data_details(
        &self,
        kind: Kind,
        id: &str,
        source: &data::Source,
        fields: &[(&str, &str)],
    ) -> String {
        let mut s = format!(
            "<details class=\"data\"><summary>Data</summary>\n<dl>\n<dt>Id</dt><dd><code>{}</code> (<code>[[{}:{}]]</code>)</dd>\n<dt>Defined in</dt><dd><code>{}</code></dd>\n",
            esc(id),
            kind.key(),
            esc(id),
            esc(&source.to_string())
        );
        if let Some(p) = self.c.wiki_page(kind, id) {
            writeln!(
                s,
                "<dt>Prose</dt><dd><code>{}</code></dd>",
                esc(&p.source.to_string())
            )
            .unwrap();
        }
        if !fields.is_empty() {
            let keys: Vec<String> = fields
                .iter()
                .map(|(f, _)| format!("<code>{}</code>", esc(&data::text_key(kind, id, f))))
                .collect();
            writeln!(s, "<dt>Text keys</dt><dd>{}</dd>", keys.join(" ")).unwrap();
        }
        s + "</dl>\n</details>\n"
    }

    /// The site's sidebar for the generated kinds: one group per kind, its entries in the list page's order
    /// (parts by category, research by family, nodes by role), as VitePress reads it.
    fn sidebar(&self) -> Value {
        let c = self.c;
        let link = |kind: Kind, id: &str| format!("/{}/{id}", kind_dir(kind));
        let item = |kind: Kind, id: &str, name: &str| json!({ "text": name, "link": link(kind, id) });
        let group = |text: &str, items: Vec<Value>| json!({ "text": text, "collapsed": true, "items": items });
        let flat = |kind: Kind, items: Vec<(&str, &str)>| -> Vec<Value> {
            items.into_iter().map(|(id, n)| item(kind, id, n)).collect()
        };
        let mut out = Vec::new();
        for (kind, title) in KINDS {
            let items: Vec<Value> = match kind {
                Kind::Part => c
                    .categories
                    .iter()
                    .filter_map(|cat| {
                        let parts: Vec<Value> = c
                            .parts
                            .iter()
                            .filter(|p| p.category == cat.id)
                            .map(|p| item(Kind::Part, &p.id, &p.name))
                            .collect();
                        (!parts.is_empty()).then(|| group(&cat.name, parts))
                    })
                    .collect(),
                Kind::Research => c
                    .families
                    .iter()
                    .filter_map(|f| {
                        let nodes: Vec<Value> = crate::family_research(c, &f.id)
                            .into_iter()
                            .map(|n| item(Kind::Research, &n.id, &n.name))
                            .collect();
                        (!nodes.is_empty()).then(|| group(&f.name, nodes))
                    })
                    .collect(),
                Kind::BehaviorNode => pages::ROLES
                    .iter()
                    .map(|&role| {
                        let nodes: Vec<Value> = c
                            .behavior_nodes
                            .iter()
                            .filter(|n| n.role == role)
                            .map(|n| item(Kind::BehaviorNode, &n.id, &n.name))
                            .collect();
                        group(pages::role_title(role), nodes)
                    })
                    .collect(),
                Kind::Control => data::input::Section::ALL
                    .iter()
                    .filter_map(|&section| {
                        let controls: Vec<Value> = c
                            .controls
                            .iter()
                            .filter(|e| e.section == section)
                            .map(|e| item(Kind::Control, &e.id, &e.name))
                            .collect();
                        (!controls.is_empty()).then(|| group(section.title(), controls))
                    })
                    .collect(),
                Kind::Stat => flat(kind, c.stats.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect()),
                Kind::Category => {
                    flat(kind, c.categories.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect())
                }
                Kind::Faction => {
                    flat(kind, c.factions.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect())
                }
                Kind::Material => {
                    flat(kind, c.materials.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect())
                }
                Kind::Frame => flat(kind, c.frames.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect()),
                Kind::Ship => flat(kind, c.ships.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect()),
                Kind::Behavior => {
                    flat(kind, c.behaviors.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect())
                }
                Kind::Family => {
                    flat(kind, c.families.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect())
                }
                Kind::Facility => {
                    flat(kind, c.facilities.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect())
                }
                Kind::Start => flat(kind, c.starts.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect()),
                Kind::Voyage => {
                    flat(kind, c.voyages.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect())
                }
                Kind::Encounter => {
                    flat(kind, c.encounters.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect())
                }
                Kind::Page => c
                    .wiki
                    .values()
                    .filter(|p| p.kind == Kind::Page)
                    .map(|p| item(Kind::Page, &p.id, c.name_of(Kind::Page, &p.id).unwrap_or(&p.id)))
                    .collect(),
            };
            out.push(json!({
                "text": title,
                "link": format!("/{}/", kind_dir(kind)),
                "collapsed": true,
                "items": items,
            }));
        }
        out.push(json!({ "text": "Evolution lab", "link": format!("/{}", pages::LAB_PAGE) }));
        Value::Array(out)
    }
}

/// What an entry page shows besides the common header fields.
#[derive(Default)]
struct EntryParts {
    badges: String,
    /// A wide figure above the lead (ships).
    hero: Option<String>,
    /// A drawing at the top of the infobox.
    figure: Option<String>,
    /// Infobox facts: label → HTML.
    facts: Vec<(String, String)>,
    /// Infobox stats table (HTML).
    stats: String,
    /// Sections after the lead: title → HTML.
    sections: Vec<(String, String)>,
}

const LOCKED: &str = " <span class=\"badge locked\" title=\"Not in a new game: research unlocks it\">Locked</span>";
