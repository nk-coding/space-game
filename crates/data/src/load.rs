//! Pure pack loader: content comes in as `(path, contents)` pairs (from the embedded base pack, a picked
//! zip on the web, a directory for native tools, ...). No filesystem access here.

use std::collections::BTreeMap;

use ron::extensions::Extensions;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::kind::Kind;
use crate::model::*;
use crate::problem::Problem;
use crate::table::Table;

/// Content schema version this crate reads. Packs declare theirs in `pack.ron`.
pub const SCHEMA_VERSION: u32 = 1;

/// RON options used for every content file: `Some(..)` may be omitted (`power: (output: 10)`); nesting deeper
/// than [`crate::limits::RON_RECURSION_LIMIT`] is an error.
pub fn ron_options() -> ron::Options {
    ron::Options::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .with_recursion_limit(crate::limits::RON_RECURSION_LIMIT)
}

/// The text without a leading byte order mark (Windows editors write one; RON reads it as an error).
pub(crate) fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// One parsed pack: entries per kind in sorted file order, sources filled in. Not merged, not validated.
#[derive(Clone, Debug)]
pub struct Pack {
    pub manifest: PackManifest,
    pub stats: Vec<StatDef>,
    pub categories: Vec<Category>,
    pub factions: Vec<Faction>,
    pub materials: Vec<Material>,
    pub parts: Vec<Part>,
    pub frames: Vec<Frame>,
    pub ships: Vec<Blueprint>,
    pub behavior_nodes: Vec<BehaviorNode>,
    pub behaviors: Vec<Behavior>,
    pub families: Vec<Family>,
    pub research: Vec<ResearchNode>,
    pub starts: Vec<Start>,
    pub voyages: Vec<Voyage>,
    pub encounters: Vec<Encounter>,
    /// `icons/<key>.svg`, sorted by key.
    pub icons: Vec<Icon>,
    /// `icons/<key>.ship.svg`, sorted by key: machines as drawn on a ship (GDR-0069).
    pub ship_art: Vec<Icon>,
    pub wiki: Vec<WikiPage>,
}

impl Pack {
    /// Parses a pack from `(path, contents)` pairs. Paths are relative to the pack root and use `/`
    /// (backslashes are accepted). Layout:
    ///
    /// * `pack.ron`: the manifest (required).
    /// * `<kind-dir>.ron` or `<kind-dir>/**/*.ron` with kind-dir one of `stats`, `categories`, `factions`,
    ///   `materials`, `parts`, `frames`, `ships`, `behavior_nodes`, `behaviors`, `families`, `research`,
    ///   `starts`, `voyages`, `encounters` ([`Kind::dir`]). A file holds one entry `( ... )` or a list `[ ( ... ), ... ]`.
    /// * `icons/<key>.svg`: part, material and category icons as SVG text (`key` kebab-case).
    /// * `icons/<key>.ship.svg`: the ship drawing of the machines whose icon is `key` (GDR-0069).
    /// * `wiki/<kind>/<id>.md`: wiki prose (kind = `part`, `ship`, ..., or `page` for free articles).
    /// * Anything else that is not `.ron` or `.svg` is ignored (READMEs, `.gitkeep`); unknown `.ron` files and
    ///   `.svg` files outside `icons/` are errors.
    ///
    /// Files are processed in sorted path order. Errors: parse errors, unknown `.ron` locations, malformed
    /// wiki and icon paths, and ids defined twice within this pack (ids repeated across packs are overrides).
    pub fn from_files<P: AsRef<str>, C: AsRef<str>>(
        files: impl IntoIterator<Item = (P, C)>,
    ) -> Result<Pack, Vec<Problem>> {
        let mut files: Vec<(String, C)> = files
            .into_iter()
            .map(|(p, c)| (normalize_path(p.as_ref()), c))
            .collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        let mut problems = Vec::new();

        let manifest = match files.iter().find(|(p, _)| p == "pack.ron") {
            Some((_, c)) => match ron_options().from_str::<PackManifest>(strip_bom(c.as_ref())) {
                Ok(m) => Some(m),
                Err(e) => {
                    problems.push(parse_problem(
                        &e,
                        &Source {
                            pack: "?".into(),
                            file: "pack.ron".into(),
                        },
                    ));
                    None
                }
            },
            None => {
                problems.push(Problem::error("missing pack.ron (pack manifest)"));
                None
            }
        };
        let pack_id = manifest
            .as_ref()
            .map(|m| m.id.clone())
            .unwrap_or_else(|| "?".into());
        let mut pack = Pack {
            manifest: manifest.unwrap_or_else(|| PackManifest {
                id: pack_id.clone(),
                name: String::new(),
                version: String::new(),
                schema_version: 0,
                description: None,
                authors: Vec::new(),
                depends: Vec::new(),
            }),
            stats: Vec::new(),
            categories: Vec::new(),
            factions: Vec::new(),
            materials: Vec::new(),
            parts: Vec::new(),
            frames: Vec::new(),
            ships: Vec::new(),
            behavior_nodes: Vec::new(),
            behaviors: Vec::new(),
            families: Vec::new(),
            research: Vec::new(),
            starts: Vec::new(),
            voyages: Vec::new(),
            encounters: Vec::new(),
            icons: Vec::new(),
            ship_art: Vec::new(),
            wiki: Vec::new(),
        };

        for (path, contents) in &files {
            let src = Source {
                pack: pack_id.clone(),
                file: path.clone(),
            };
            let text = contents.as_ref();
            if path == "pack.ron" {
                continue;
            }
            if path.ends_with(".ron") {
                let first = path.split('/').next().unwrap();
                let dir = if path.contains('/') {
                    first
                } else {
                    first.trim_end_matches(".ron")
                };
                match Kind::from_dir(dir) {
                    Some(Kind::Stat) => parse_into(text, &src, &mut pack.stats, &mut problems),
                    Some(Kind::Category) => {
                        parse_into(text, &src, &mut pack.categories, &mut problems)
                    }
                    Some(Kind::Faction) => {
                        parse_into(text, &src, &mut pack.factions, &mut problems)
                    }
                    Some(Kind::Material) => {
                        parse_into(text, &src, &mut pack.materials, &mut problems)
                    }
                    Some(Kind::Part) => parse_into(text, &src, &mut pack.parts, &mut problems),
                    Some(Kind::Frame) => parse_into(text, &src, &mut pack.frames, &mut problems),
                    Some(Kind::Ship) => parse_into(text, &src, &mut pack.ships, &mut problems),
                    Some(Kind::BehaviorNode) => {
                        parse_into(text, &src, &mut pack.behavior_nodes, &mut problems)
                    }
                    Some(Kind::Behavior) => {
                        parse_into(text, &src, &mut pack.behaviors, &mut problems)
                    }
                    Some(Kind::Family) => parse_into(text, &src, &mut pack.families, &mut problems),
                    Some(Kind::Research) => {
                        parse_into(text, &src, &mut pack.research, &mut problems)
                    }
                    Some(Kind::Start) => parse_into(text, &src, &mut pack.starts, &mut problems),
                    Some(Kind::Voyage) => parse_into(text, &src, &mut pack.voyages, &mut problems),
                    Some(Kind::Encounter) => {
                        parse_into(text, &src, &mut pack.encounters, &mut problems)
                    }
                    Some(Kind::Page) | None => problems.push(
                        Problem::error(format!(
                            "unknown content file location; RON files belong in {}",
                            or_list(Kind::ALL.iter().filter_map(|k| k.dir()))
                        ))
                        .at(&src),
                    ),
                }
            } else if path.ends_with(".svg") {
                let stem = path
                    .strip_prefix("icons/")
                    .and_then(|f| f.strip_suffix(".svg"));
                let (stem, ship) = match stem.and_then(|s| s.strip_suffix(".ship")) {
                    Some(key) => (Some(key), true),
                    None => (stem, false),
                };
                match stem.filter(|k| crate::art_keys::is_icon_key(k)) {
                    Some(key) => {
                        let icon = Icon {
                            key: key.to_string(),
                            svg: text.to_string(),
                            source: src,
                        };
                        if ship {
                            pack.ship_art.push(icon);
                        } else {
                            pack.icons.push(icon);
                        }
                    }
                    None => problems.push(
                        Problem::error(
                            "SVG files must be icons/<key>.svg or icons/<key>.ship.svg with a kebab-case key (lowercase letters, digits and -)",
                        )
                        .at(&src),
                    ),
                }
            } else if path.starts_with("wiki/") {
                let segs: Vec<&str> = path.split('/').collect();
                let parsed = match segs.as_slice() {
                    ["wiki", kind, file] if file.ends_with(".md") => {
                        Kind::from_key(kind).map(|k| (k, file.trim_end_matches(".md")))
                    }
                    _ => None,
                };
                match parsed {
                    Some((kind, id)) => pack.wiki.push(WikiPage {
                        kind,
                        id: id.to_string(),
                        markdown: text.to_string(),
                        source: src,
                    }),
                    None if path.ends_with(".md") => problems.push(
                        Problem::error(format!(
                            "wiki files must be wiki/<kind>/<id>.md with kind {}",
                            or_list(Kind::ALL.iter().map(|k| k.key()))
                        ))
                        .at(&src),
                    ),
                    None => {}
                }
            }
        }

        check_duplicates(&pack.stats, &mut problems);
        check_duplicates(&pack.categories, &mut problems);
        check_duplicates(&pack.factions, &mut problems);
        check_duplicates(&pack.materials, &mut problems);
        check_duplicates(&pack.parts, &mut problems);
        check_duplicates(&pack.frames, &mut problems);
        check_duplicates(&pack.ships, &mut problems);
        check_duplicates(&pack.behavior_nodes, &mut problems);
        check_duplicates(&pack.behaviors, &mut problems);
        check_duplicates(&pack.families, &mut problems);
        check_duplicates(&pack.research, &mut problems);
        check_duplicates(&pack.starts, &mut problems);
        check_duplicates(&pack.voyages, &mut problems);
        check_duplicates(&pack.encounters, &mut problems);
        let mut seen: BTreeMap<(Kind, &str), &Source> = BTreeMap::new();
        for page in &pack.wiki {
            if let Some(first) = seen.insert((page.kind, &page.id), &page.source) {
                problems.push(
                    Problem::error(format!("wiki page defined twice (first in {first})"))
                        .at(&page.source)
                        .entry(page.kind, &page.id),
                );
            }
        }

        if problems.is_empty() {
            Ok(pack)
        } else {
            Err(problems)
        }
    }
}

/// `a, b or c`.
fn or_list<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let items: Vec<&str> = items.collect();
    match items.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        _ => items.join(""),
    }
}

fn normalize_path(p: &str) -> String {
    let p = p.replace('\\', "/");
    let p = p.trim_start_matches("./").trim_start_matches('/');
    p.to_string()
}

fn parse_problem(e: &ron::error::SpannedError, src: &Source) -> Problem {
    parse_error(e).at(src)
}

/// A RON parse error with its line and column, without a file.
pub(crate) fn parse_error(e: &ron::error::SpannedError) -> Problem {
    let mut p = Problem::error(e.code.to_string());
    p.line = Some((e.span.start.line, e.span.start.col));
    p
}

/// Is the first token (after whitespace, comments and `#![...]` attributes) a `[`?
fn is_list(src: &str) -> bool {
    let mut s = src;
    loop {
        s = s.trim_start();
        if let Some(rest) = s.strip_prefix("//") {
            s = rest.split_once('\n').map_or("", |(_, r)| r);
        } else if let Some(rest) = s.strip_prefix("/*") {
            s = rest.split_once("*/").map_or("", |(_, r)| r);
        } else if let Some(rest) = s.strip_prefix("#!") {
            s = rest.split_once(']').map_or("", |(_, r)| r);
        } else {
            return s.starts_with('[');
        }
    }
}

fn parse_into<T: DeserializeOwned + Entry>(
    text: &str,
    src: &Source,
    out: &mut Vec<T>,
    problems: &mut Vec<Problem>,
) {
    let opts = ron_options();
    let text = strip_bom(text);
    let parsed = if is_list(text) {
        opts.from_str::<Vec<T>>(text)
    } else {
        opts.from_str::<T>(text).map(|e| vec![e])
    };
    match parsed {
        Ok(entries) => {
            for mut e in entries {
                e.set_source(src.clone());
                out.push(e);
            }
        }
        Err(e) => {
            let mut p = parse_problem(&e, src);
            p.kind = Some(T::KIND);
            problems.push(p);
        }
    }
}

fn check_duplicates<T: Entry>(entries: &[T], problems: &mut Vec<Problem>) {
    let mut seen: BTreeMap<&str, &Source> = BTreeMap::new();
    for e in entries {
        if let Some(first) = seen.insert(e.id(), e.source()) {
            problems.push(
                Problem::error(format!("duplicate id (first defined in {first})"))
                    .at(e.source())
                    .entry(T::KIND, e.id()),
            );
        }
    }
}

/// All loaded content: packs merged in load order. Later packs replace entries with the same id.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Content {
    pub packs: Vec<PackManifest>,
    pub stats: Table<StatDef>,
    pub categories: Table<Category>,
    pub factions: Table<Faction>,
    pub materials: Table<Material>,
    pub parts: Table<Part>,
    /// Drydock frames in load order, which is also the drydock's display order.
    pub frames: Table<Frame>,
    pub ships: Table<Blueprint>,
    pub behavior_nodes: Table<BehaviorNode>,
    pub behaviors: Table<Behavior>,
    /// Research families in load order, which is also their display order.
    pub families: Table<Family>,
    pub research: Table<ResearchNode>,
    pub starts: Table<Start>,
    /// Voyages in load order, which is also the voyage map's order.
    pub voyages: Table<Voyage>,
    pub encounters: Table<Encounter>,
    /// Icons by key (`icons/<key>.svg`); a later pack's icon replaces an earlier one with the same key.
    pub icons: BTreeMap<String, Icon>,
    /// Ship drawings of machines by icon key (`icons/<key>.ship.svg`, GDR-0069); later packs replace them like icons.
    pub ship_art: BTreeMap<String, Icon>,
    /// Wiki prose keyed `"<kind>:<id>"` (e.g. `"part:fission_reactor"`). Empty in the game build.
    pub wiki: BTreeMap<String, WikiPage>,
}

impl Content {
    /// Merges packs in load order without validating. Use [`Content::load`] unless you validate yourself.
    pub fn merge(packs: &[Pack]) -> Content {
        let mut c = Content::default();
        for p in packs {
            c.packs.push(p.manifest.clone());
            p.stats
                .iter()
                .cloned()
                .for_each(|e| drop(c.stats.upsert(e)));
            p.categories
                .iter()
                .cloned()
                .for_each(|e| drop(c.categories.upsert(e)));
            p.factions
                .iter()
                .cloned()
                .for_each(|e| drop(c.factions.upsert(e)));
            p.materials
                .iter()
                .cloned()
                .for_each(|e| drop(c.materials.upsert(e)));
            p.parts
                .iter()
                .cloned()
                .for_each(|e| drop(c.parts.upsert(e)));
            p.frames
                .iter()
                .cloned()
                .for_each(|e| drop(c.frames.upsert(e)));
            p.ships
                .iter()
                .cloned()
                .for_each(|e| drop(c.ships.upsert(e)));
            p.behavior_nodes
                .iter()
                .cloned()
                .for_each(|e| drop(c.behavior_nodes.upsert(e)));
            p.behaviors
                .iter()
                .cloned()
                .for_each(|e| drop(c.behaviors.upsert(e)));
            p.families
                .iter()
                .cloned()
                .for_each(|e| drop(c.families.upsert(e)));
            p.research
                .iter()
                .cloned()
                .for_each(|e| drop(c.research.upsert(e)));
            p.starts
                .iter()
                .cloned()
                .for_each(|e| drop(c.starts.upsert(e)));
            p.voyages
                .iter()
                .cloned()
                .for_each(|e| drop(c.voyages.upsert(e)));
            p.encounters
                .iter()
                .cloned()
                .for_each(|e| drop(c.encounters.upsert(e)));
            for i in &p.icons {
                c.icons.insert(i.key.clone(), i.clone());
            }
            for i in &p.ship_art {
                c.ship_art.insert(i.key.clone(), i.clone());
            }
            for w in &p.wiki {
                c.wiki
                    .insert(format!("{}:{}", w.kind.key(), w.id), w.clone());
            }
        }
        c
    }

    /// Merges and validates. `Err` holds every problem (errors and warnings) if there is at least one error.
    pub fn load(packs: &[Pack]) -> Result<Content, Vec<Problem>> {
        let c = Content::merge(packs);
        let problems = crate::validate::validate(&c);
        if problems.iter().any(Problem::is_error) {
            Err(problems)
        } else {
            Ok(c)
        }
    }

    /// Does an entry (or wiki page for `Kind::Page`) exist?
    pub fn exists(&self, kind: Kind, id: &str) -> bool {
        match kind {
            Kind::Stat => self.stats.contains(id),
            Kind::Category => self.categories.contains(id),
            Kind::Faction => self.factions.contains(id),
            Kind::Material => self.materials.contains(id),
            Kind::Part => self.parts.contains(id),
            Kind::Frame => self.frames.contains(id),
            Kind::Ship => self.ships.contains(id),
            Kind::BehaviorNode => self.behavior_nodes.contains(id),
            Kind::Behavior => self.behaviors.contains(id),
            Kind::Family => self.families.contains(id),
            Kind::Research => self.research.contains(id),
            Kind::Start => self.starts.contains(id),
            Kind::Voyage => self.voyages.contains(id),
            Kind::Encounter => self.encounters.contains(id),
            Kind::Page => self.wiki.contains_key(&format!("page:{id}")),
        }
    }

    /// Display name of an entry. For wiki pages: the first `# ` heading, else the id.
    pub fn name_of(&self, kind: Kind, id: &str) -> Option<&str> {
        match kind {
            Kind::Stat => self.stats.get(id).map(|e| e.name.as_str()),
            Kind::Category => self.categories.get(id).map(|e| e.name.as_str()),
            Kind::Faction => self.factions.get(id).map(|e| e.name.as_str()),
            Kind::Material => self.materials.get(id).map(|e| e.name.as_str()),
            Kind::Part => self.parts.get(id).map(|e| e.name.as_str()),
            Kind::Frame => self.frames.get(id).map(|e| e.name.as_str()),
            Kind::Ship => self.ships.get(id).map(|e| e.name.as_str()),
            Kind::BehaviorNode => self.behavior_nodes.get(id).map(|e| e.name.as_str()),
            Kind::Behavior => self.behaviors.get(id).map(|e| e.name.as_str()),
            Kind::Family => self.families.get(id).map(|e| e.name.as_str()),
            Kind::Research => self.research.get(id).map(|e| e.name.as_str()),
            Kind::Start => self.starts.get(id).map(|e| e.name.as_str()),
            Kind::Voyage => self.voyages.get(id).map(|e| e.name.as_str()),
            Kind::Encounter => self.encounters.get(id).map(|e| e.name.as_str()),
            Kind::Page => self.wiki.get(&format!("page:{id}")).map(|p| {
                p.markdown
                    .lines()
                    .find_map(|l| l.strip_prefix("# "))
                    .map(str::trim)
                    .unwrap_or(p.id.as_str())
            }),
        }
    }

    /// Wiki prose for an entry, if a pack provides it (tools only; the game build embeds no wiki files).
    pub fn wiki_page(&self, kind: Kind, id: &str) -> Option<&WikiPage> {
        self.wiki.get(&format!("{}:{}", kind.key(), id))
    }

    /// The SVG source of an icon (`key` as in `Part::icon`), from the last loaded pack that has
    /// `icons/<key>.svg`. The client rasterises it at the resolution it needs; the wiki shows it as is.
    pub fn icon_svg(&self, key: &str) -> Option<&str> {
        self.icons.get(key).map(|i| i.svg.as_str())
    }

    /// The ship drawing of the machines whose icon is `key` (GDR-0069): `icons/<key>.ship.svg` from the last
    /// loaded pack that has it. The client draws it on the ship instead of the icon.
    pub fn ship_svg(&self, key: &str) -> Option<&str> {
        self.ship_art.get(key).map(|i| i.svg.as_str())
    }

    /// The system colour key for a part's icon: the part's own `color`, else its category's.
    pub fn part_color<'a>(&'a self, part: &'a Part) -> Option<&'a str> {
        part.color.as_deref().or_else(|| {
            self.categories
                .get(&part.category)
                .and_then(|c| c.color.as_deref())
        })
    }

    /// The first hull part with a 1×1 footprint of this shape, in content order: the drydock paints and fills
    /// with the `Rect` one (the armour block) and bevels with the `Slope` one (`data::edit`).
    pub fn unit_hull_part(&self, shape: Shape) -> Option<&Part> {
        self.parts
            .iter()
            .find(|p| p.hull && p.size == (1, 1) && p.shape == shape)
    }

    /// The geometry of a placement, `None` if the part id is unknown.
    pub fn placed_shape(&self, p: &Placement) -> Option<crate::geometry::PlacedShape> {
        let part = self.parts.get(&p.part)?;
        Some(crate::geometry::PlacedShape {
            footprint: part.footprint(),
            x: p.x,
            y: p.y,
            rot: p.rot,
            flip: p.flip,
        })
    }

    /// The occupancy grid of a blueprint; part indices = indices into `ship.parts`.
    /// `Err` names the first unknown part id, or says that the parts lie too far apart (see
    /// [`crate::geometry::ShipGrid::new`]); both are impossible for validated content.
    pub fn ship_grid(&self, ship: &Blueprint) -> Result<crate::geometry::ShipGrid, String> {
        let shapes = ship
            .parts
            .iter()
            .map(|p| {
                self.placed_shape(p)
                    .ok_or_else(|| format!("ship `{}`: unknown part `{}`", ship.id, p.part))
            })
            .collect::<Result<Vec<_>, _>>()?;
        crate::geometry::ShipGrid::new(shapes).ok_or_else(|| {
            format!(
                "ship `{}`: the parts span more than {} cells or lie too far out",
                ship.id,
                crate::validate::MAX_FRAME_SIZE
            )
        })
    }

    /// The armour material of a placement (hull parts only).
    pub fn placement_material(&self, p: &Placement) -> Option<&Material> {
        p.material.as_deref().and_then(|m| self.materials.get(m))
    }

    /// The armour spec of a placement's material (hull parts only).
    pub fn placement_armor(&self, p: &Placement) -> Option<&ArmorSpec> {
        self.placement_material(p)?.armor.as_ref()
    }

    /// Mass of a placed part: the part's `mass`, or for hull parts the material's `mass_per_cell × area`.
    /// `None` if the part or a hull part's armour material is unknown.
    pub fn placement_mass(&self, p: &Placement) -> Option<f32> {
        self.parts.get(&p.part)?.mass_in(self.placement_armor(p))
    }

    /// The behavior a ship flies under when nobody pilots it: its own `behavior`, else its faction's.
    pub fn ship_behavior(&self, ship: &Blueprint) -> Option<&Behavior> {
        let id = ship.behavior.as_deref().or_else(|| {
            self.factions
                .get(&ship.faction)
                .and_then(|f| f.behavior.as_deref())
        })?;
        self.behaviors.get(id)
    }

    /// The parts of an intact ship's main piece (the sim's controlling piece at spawn: `geometry::main_of`), in
    /// placement order; none for a ship whose grid does not build.
    fn main_piece_parts<'a>(&'a self, ship: &'a Blueprint) -> Vec<&'a Part> {
        let Ok(grid) = self.ship_grid(ship) else {
            return Vec::new();
        };
        let part = |i: usize| self.parts.get(&ship.parts[i].part);
        let pieces = grid.pieces();
        let Some(main) = grid.main_piece(&pieces, |i| part(i).is_some_and(|p| p.logic.is_some()))
        else {
            return Vec::new();
        };
        pieces.parts[main].iter().filter_map(|&i| part(i)).collect()
    }

    /// Rule slots of an intact ship: the `logic.rule_slots` of every logic core on its main piece.
    pub fn ship_rule_slots(&self, ship: &Blueprint) -> u32 {
        self.main_piece_parts(ship)
            .iter()
            .filter_map(|p| p.logic.as_ref())
            .fold(0, |sum: u32, l| sum.saturating_add(l.rule_slots))
    }

    /// The hold of an intact ship in cargo units: the summed `cargo.capacity` of its main piece, rounded down
    /// (the sim's `Ship::cargo_capacity`; cargo parts that fall off at spawn hold nothing).
    pub fn ship_hold(&self, ship: &Blueprint) -> u32 {
        self.main_piece_parts(ship)
            .iter()
            .filter_map(|p| p.cargo.as_ref())
            .map(|c| c.capacity)
            .sum::<f32>() as u32
    }

    /// Control of an intact ship's main piece (GDR-0044): `(load, capacity)` = the summed `control_cost` of
    /// its parts and the summed `logic.control_capacity` of its logic cores. Load above capacity means the
    /// lowest-priority parts start idle. `(0, 0)` for a ship whose grid does not build.
    pub fn ship_control(&self, ship: &Blueprint) -> (u32, u32) {
        self.main_piece_parts(ship)
            .iter()
            .fold((0u32, 0u32), |(load, cap), p| {
                (
                    load.saturating_add(p.control_cost()),
                    cap.saturating_add(p.logic.as_ref().map_or(0, |l| l.control_capacity)),
                )
            })
    }

    /// Is an intact ship armed (GDR-0068, the sim's `Ship::is_armed` at spawn)? Its main piece has a part with a
    /// `weapon` and a part that makes power. A voyage site clears once none of its ships is armed, so a goal
    /// encounter must always roll an armed ship. False for a ship whose grid does not build.
    pub fn ship_armed(&self, ship: &Blueprint) -> bool {
        let main = self.main_piece_parts(ship);
        main.iter().any(|p| p.weapon.is_some())
            && main
                .iter()
                .any(|p| p.power.as_ref().is_some_and(|x| x.output > 0.0))
    }

    /// Hit points of a placed part: the part's `hp`, or for hull parts `hp_per_cell × area`.
    pub fn placement_hp(&self, p: &Placement) -> Option<f32> {
        self.parts.get(&p.part)?.hp_in(self.placement_armor(p))
    }
}
