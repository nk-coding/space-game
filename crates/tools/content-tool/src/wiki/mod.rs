//! `content-tool wiki`: a static website generated from the loaded content (ADR-0032, `docs/wiki.md`).
//!
//! [`generate`] is pure: content in, files out (`path → bytes`, sorted), so two runs give identical bytes and the
//! tests need no disk; [`write`] puts them into a directory. Layout of the site:
//!
//! * `index.html`, `style.css` (the one stylesheet), `fonts/` (Barlow, the game's UI font);
//! * one list page per kind at the top level, named like the kind's pack directory (`parts.html`,
//!   `behavior_nodes.html`, `pages.html` for the free articles);
//! * one page per entry under the kind's link key, exactly like `[[part:fission_reactor]]`:
//!   `part/fission_reactor.html`.
//!
//! Every name, summary, stat label and unit comes from the content (the keyed English text, ADR-0018); colours
//! come from `art/tokens.json`; icons are the pack's SVGs, inlined; ships are drawn from the blueprint geometry.
//! The headings and words around them ("Used by", "Locked") are the tool's own, like the game's UI strings.

mod draw;
mod markup;
mod pages;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use data::design::geometry::hull_loops;
use data::design::stats::{SHIP_STATS, format_number, with_unit};
use data::{
    Behavior, BehaviorNode, Better, Blueprint, Content, Delivery, Encounter, Entry, Family, Kind,
    Material, NodeRole, ParamType, Part, PlacedShape, ResearchKind, ResearchNode, Selector, Start,
    StatLine, Unlock, Voyage,
};
use sim::DesignReport;

use draw::{Glow, Style, Tile};
use markup::{esc, inline, markdown};

/// The generated site: file path (relative, `/`-separated) → contents, sorted by path.
pub struct Site {
    pub files: BTreeMap<String, Vec<u8>>,
    /// Problems found while generating (an unclosed code fence in a prose file, a broken icon). The base pack
    /// has none.
    pub warnings: Vec<String>,
}

/// The kinds in navigation order, with the titles of their list pages.
const KINDS: [(Kind, &str); 16] = [
    (Kind::Part, "Parts"),
    (Kind::Material, "Materials"),
    (Kind::Category, "Categories"),
    (Kind::Ship, "Ships"),
    (Kind::Frame, "Frames"),
    (Kind::Faction, "Factions"),
    (Kind::Research, "Research"),
    (Kind::Family, "Families"),
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

/// Pages in a kind directory link to the top level through this.
const UP: &str = "../";
/// Pixels per cell of the drawings: ship pages, ship cards, part pages, the index's part palette and the small
/// chips in tables.
const PX_SHIP: f32 = 34.0;
const PX_CARD: f32 = 14.0;
const PX_PART: f32 = 72.0;
const PX_PALETTE: f32 = 36.0;
/// Table chips fit a square of this many pixels, whatever the part's size.
const CHIP_BOX: f32 = 34.0;
/// Margin around a drawing, in cells: room for the hull glow around ships, just the seams around parts.
const MARGIN_SHIP: f32 = 0.5;
const MARGIN_PART: f32 = 0.06;

/// Where the file written by [`write`] lists the files it made, so the next run can remove stale ones.
pub const MANIFEST: &str = ".wiki-files";

fn item_path(kind: Kind, id: &str) -> String {
    format!("{}/{id}.html", kind.key())
}

fn list_path(kind: Kind) -> String {
    format!("{}.html", kind.dir().unwrap_or("pages"))
}

fn kind_title(kind: Kind) -> &'static str {
    KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or("", |(_, t)| t)
}

/// Generates the whole site from validated content.
pub fn generate(c: &Content) -> Site {
    let w = Wiki::new(c);
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut page = |path: String, title: &str, crumbs: &[(String, &str)], body: String| {
        let html = w.document(&path, title, crumbs, &body);
        files.insert(path, html.into_bytes());
    };

    page("index.html".into(), &w.site_name(), &[], w.index());
    for (kind, title) in KINDS {
        page(list_path(kind), title, &[], w.list(kind));
    }
    // One page per entry, below the list of its kind.
    let mut entry = |kind: Kind, id: &str, name: &str, body: String| {
        let up = [(format!("{UP}{}", list_path(kind)), kind_title(kind))];
        page(item_path(kind, id), name, &up, body);
    };
    for e in &c.stats {
        entry(Kind::Stat, &e.id, &e.name, w.stat_page(e));
    }
    for e in &c.categories {
        entry(Kind::Category, &e.id, &e.name, w.category_page(e));
    }
    for e in &c.factions {
        entry(Kind::Faction, &e.id, &e.name, w.faction_page(e));
    }
    for e in &c.materials {
        entry(Kind::Material, &e.id, &e.name, w.material_page(e));
    }
    for e in &c.parts {
        entry(Kind::Part, &e.id, &e.name, w.part_page(e));
    }
    for e in &c.frames {
        entry(Kind::Frame, &e.id, &e.name, w.frame_page(e));
    }
    for (k, e) in c.ships.iter().enumerate() {
        entry(Kind::Ship, &e.id, &e.name, w.ship_page(k, e));
    }
    for e in &c.behavior_nodes {
        entry(Kind::BehaviorNode, &e.id, &e.name, w.node_page(e));
    }
    for e in &c.behaviors {
        entry(Kind::Behavior, &e.id, &e.name, w.behavior_page(e));
    }
    for e in &c.families {
        entry(Kind::Family, &e.id, &e.name, w.family_page(e));
    }
    for e in &c.research {
        entry(Kind::Research, &e.id, &e.name, w.research_page(e));
    }
    for e in &c.starts {
        entry(Kind::Start, &e.id, &e.name, w.start_page(e));
    }
    for e in &c.voyages {
        entry(Kind::Voyage, &e.id, &e.name, w.voyage_page(e));
    }
    for e in &c.encounters {
        entry(Kind::Encounter, &e.id, &e.name, w.encounter_page(e));
    }
    for e in &c.controls {
        entry(Kind::Control, &e.id, &e.name, w.control_page(e));
    }
    for p in c.wiki.values().filter(|p| p.kind == Kind::Page) {
        let title = c.name_of(Kind::Page, &p.id).unwrap_or(&p.id);
        entry(Kind::Page, &p.id, title, w.article(&p.id));
    }
    files.insert("style.css".into(), w.css().into_bytes());
    for (name, bytes) in FONTS {
        files.insert(format!("fonts/{name}"), bytes.to_vec());
    }
    Site {
        files,
        warnings: w.warnings.into_inner(),
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

/// Writes the site into `dir` (created if needed). A directory that is not empty must hold the file list of an
/// earlier run ([`MANIFEST`]); files of that run that are gone now are removed, nothing else is touched.
pub fn write(site: &Site, dir: &Path) -> Result<(), String> {
    let manifest = dir.join(MANIFEST);
    let old: Vec<String> = match std::fs::read_to_string(&manifest) {
        Ok(text) => text.lines().map(str::to_string).collect(),
        Err(_) => {
            let empty = std::fs::read_dir(dir).map_or(true, |mut d| d.next().is_none());
            if !empty {
                return Err(format!(
                    "{} is not empty and holds no wiki made by content-tool ({MANIFEST}); pick an empty or new directory",
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
        std::fs::write(&file, bytes)
            .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    }
    let list: Vec<&str> = site.files.keys().map(String::as_str).collect();
    std::fs::write(&manifest, list.join("\n") + "\n")
        .map_err(|e| format!("cannot write {}: {e}", manifest.display()))
}

/// The generator: content plus what every page needs (style, icon symbols, design reports, back-links).
struct Wiki<'a> {
    c: &'a Content,
    style: Style,
    /// Icon key → `<symbol>` for the page sprites.
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
        let symbols = c
            .icons
            .values()
            .filter_map(|i| match draw::icon_symbol(&i.key, &i.svg) {
                Ok(s) => Some((i.key.clone(), s)),
                Err(e) => {
                    warnings.push(format!("icon `{}` ({}): {e}", i.key, i.source));
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

    fn site_name(&self) -> String {
        let name = self.c.packs.first().map_or("Content", |p| p.name.as_str());
        format!("{name} wiki")
    }

    // -----------------------------------------------------------------------------------------------------
    // Page frame

    /// The full HTML document of a page: head, the icon sprite, navigation, crumbs, body, footer.
    fn document(&self, path: &str, title: &str, crumbs: &[(String, &str)], body: &str) -> String {
        let root = if path.contains('/') { UP } else { "" };
        let mut used = BTreeSet::new();
        for (k, _) in body.match_indices("href=\"#icon-") {
            let key = &body[k + 12..];
            used.insert(&key[..key.find('"').unwrap_or(0)]);
        }
        let symbols: Vec<String> = used
            .into_iter()
            .filter_map(|k| self.symbols.get(k).cloned())
            .collect();
        let mut s = String::with_capacity(body.len() + 4096);
        let site = esc(&self.site_name());
        let full_title = if title == self.site_name() {
            site.clone()
        } else {
            format!("{} · {site}", esc(title))
        };
        write!(
            s,
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
             <title>{full_title}</title>\n<link rel=\"stylesheet\" href=\"{root}style.css\">\n</head>\n<body>\n"
        )
        .unwrap();
        s.push_str(&draw::sprite(&self.style, &symbols));
        write!(
            s,
            "<header class=\"top\"><a class=\"brand\" href=\"{root}index.html\">{site}</a><nav>"
        )
        .unwrap();
        for (kind, t) in KINDS {
            let target = list_path(kind);
            let here = path == target || path.starts_with(&format!("{}/", kind.key()));
            let class = if here { " class=\"here\"" } else { "" };
            write!(s, "<a{class} href=\"{root}{target}\">{t}</a>").unwrap();
        }
        s.push_str("</nav></header>\n<main>\n");
        if !crumbs.is_empty() {
            write!(
                s,
                "<nav class=\"crumbs\"><a href=\"{root}index.html\">{site}</a>"
            )
            .unwrap();
            for (href, label) in crumbs {
                write!(s, " / <a href=\"{href}\">{}</a>", esc(label)).unwrap();
            }
            s.push_str("</nav>\n");
        }
        s.push_str(body);
        s.push_str("</main>\n<footer>Generated by <code>content-tool wiki</code> from ");
        let packs: Vec<String> = self
            .c
            .packs
            .iter()
            .map(|p| {
                format!(
                    "{} <code>{}</code> {}",
                    esc(&p.name),
                    esc(&p.id),
                    esc(&p.version)
                )
            })
            .collect();
        s.push_str(&packs.join(", "));
        s.push_str(".</footer>\n</body>\n</html>\n");
        s
    }

    fn css(&self) -> String {
        let t = |p: &str| self.style.get(p);
        let mut s =
            String::from("/* Generated by content-tool wiki; colours from art/tokens.json. */\n");
        for weight in [("Regular", 400), ("Medium", 500)] {
            writeln!(
                s,
                "@font-face {{ font-family: \"{}\"; src: url(\"fonts/Barlow-{}.ttf\") format(\"truetype\"); font-weight: {}; }}",
                t("ui.font"),
                weight.0,
                weight.1
            )
            .unwrap();
        }
        writeln!(
            s,
            ":root {{\n  --bg: {};\n  --teal: {};\n  --gold: {};\n  --coral: {};\n  --violet: {};\n  --label: {};\n  --dim: {};\n  --number: {};\n  --ring: {};\n  --particle: {};\n  --tile: {};\n  --seam: {};\n  --font: \"{}\";\n}}",
            t("ui.background"),
            t("ui.teal"),
            t("ui.gold"),
            t("ui.coral"),
            t("ui.violet"),
            t("ui.label"),
            t("ui.dim"),
            t("ui.number"),
            t("ui.dotted_ring"),
            t("ui.particle"),
            t("ship.machine_tile"),
            t("ship.seam"),
            t("ui.font"),
        )
        .unwrap();
        s + include_str!("style.css")
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

    /// One part on its tile, unrotated: the drydock palette's look.
    fn part_svg(&self, p: &Part, material: Option<&Material>, px: f32, class: &str) -> String {
        let shape = PlacedShape {
            footprint: p.footprint(),
            x: 0,
            y: 0,
            rot: 0,
            flip: false,
        };
        let fill = if p.hull {
            self.tone(material.or(self.default_armor()))
        } else {
            self.tone(None)
        };
        let icon = if p.hull { None } else { p.icon.clone() };
        draw::layout(
            &self.style,
            &[Tile { shape, fill, icon }],
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

    /// A ship in style 2a: tiles, icons and the hull glow of its main piece in its faction's colour.
    fn ship_svg(&self, s: &Blueprint, px: f32, class: &str) -> String {
        let Ok(grid) = self.c.ship_grid(s) else {
            return String::new();
        };
        let tiles: Vec<Tile> = s
            .parts
            .iter()
            .enumerate()
            .filter_map(|(i, pl)| {
                let part = self.c.parts.get(&pl.part)?;
                Some(Tile {
                    shape: *grid.shape(i),
                    fill: if part.hull {
                        self.tone(self.c.placement_material(pl))
                    } else {
                        self.tone(None)
                    },
                    icon: if part.hull { None } else { part.icon.clone() },
                })
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

    /// An entry page: the main column (title, texts, prose, sections, mentions, data) and the infobox.
    fn entry<T: Entry>(&self, e: &T, parts: EntryParts) -> String {
        let root = UP;
        let mut main = String::new();
        writeln!(main, "<h1>{}{}</h1>", esc(e.name()), parts.badges).unwrap();
        writeln!(
            main,
            "<p class=\"summary\">{}</p>",
            self.text(root, e.summary())
        )
        .unwrap();
        if let Some(hero) = &parts.hero {
            main.push_str(hero);
        }
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
        for (title, html) in &parts.sections {
            writeln!(main, "<section>\n<h2>{}</h2>\n{html}</section>", esc(title)).unwrap();
        }
        main.push_str(&self.mentioned(T::KIND, e.id()));
        main.push_str(&self.data_details(T::KIND, e.id(), e.source(), &e.text_fields()));

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
        format!(
            "<article class=\"entry\">\n<div class=\"main\">\n{main}</div>\n<aside class=\"infobox\">\n{aside}</aside>\n</article>\n"
        )
    }

    /// "Mentioned on": the entries whose texts or prose link here.
    fn mentioned(&self, kind: Kind, id: &str) -> String {
        match self.mentions.get(&(kind, id.to_string())) {
            Some(v) if !v.is_empty() => {
                let list = self.links(UP, v.iter().cloned()).unwrap_or_default();
                format!("<section>\n<h2>Mentioned on</h2>\n<p>{list}</p>\n</section>\n")
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
}

/// What an entry page shows besides the common header fields.
#[derive(Default)]
struct EntryParts {
    badges: String,
    /// A wide figure under the summary (ships).
    hero: Option<String>,
    /// A drawing at the top of the infobox.
    figure: Option<String>,
    /// Infobox facts: label → HTML.
    facts: Vec<(String, String)>,
    /// Infobox stats table (HTML).
    stats: String,
    /// Sections after the texts and prose: title → HTML.
    sections: Vec<(String, String)>,
}

const LOCKED: &str = " <span class=\"badge locked\" title=\"Not in a new game: research unlocks it\">Locked</span>";
