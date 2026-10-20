//! Validation of merged content ([`validate`]) and of a single blueprint against it ([`validate_ship`], for
//! drydock designs). Every problem names pack, file, kind, id and field where possible.
//!
//! The checks are split by what they check: parts (`parts.rs`), research and starts (`progression.rs`), encounters
//! and voyages (`voyages.rs`), the galaxy's kinds (`galaxy.rs`), rock kinds, landmarks and scenery (`scenery.rs`), planet kinds and skies (`sky.rs`), behaviors (`behaviors.rs`), ships (`ships.rs`), the lab (`lab.rs`); packs, headers,
//! links, icons and numbers here.

mod behaviors;
mod controls;
mod galaxy;
mod lab;
mod parts;
mod progression;
mod scenery;
mod ships;
mod sky;
mod voyages;

use std::collections::{BTreeMap, BTreeSet};

use crate::behavior::Program;
use crate::content::art_keys::{ARMOR_TONES, GLOWS, SYSTEM_COLORS, is_icon_key};
use crate::content::kind::Kind;
use crate::content::load::{Content, SCHEMA_VERSION};
use crate::content::model::*;
use crate::content::problem::Problem;
use crate::design::stats::{BONUS_STATS, BUILTIN_STATS, SHIP_STATS};
use crate::content::table::Table;
use crate::content::text::parse_links;

/// Validates merged content. Returns errors and warnings (warnings: a ship in several pieces, a gun without a clear
/// line of fire, an RCS part with no side open to space, a radiator that cannot vent, an empty placement power
/// override, a control load above the cores' capacity, a `control_cost` on a part without a power role, a core
/// with no control capacity, an icon or ship drawing nothing uses, research that unlocks what is already open, an encounter ship
/// without a behavior, a voyage nothing can open, a start ship without a hold).
pub fn validate(c: &Content) -> Vec<Problem> {
    let mut v = Validator { c, flies: None, out: Vec::new() };
    v.packs();
    v.stats();
    v.entries(c.categories.as_slice(), |v, e| {
        if let Some(col) = &e.color {
            v.key_in(e, "color", col, SYSTEM_COLORS, "system colour");
        }
        if let Some(icon) = &e.icon {
            v.icon(e, "icon", icon);
        }
    });
    v.entries(c.factions.as_slice(), |v, e| {
        v.key_in(e, "glow", &e.glow, GLOWS, "glow");
        if let Some(b) = &e.behavior {
            v.behavior_ref(e, b);
        }
        if let Some(t) = &e.territory {
            v.number(e, "territory.wake", t.wake, 0.0, true);
            v.number(e, "territory.warn", t.warn, 0.0, true);
            if t.warn < t.wake {
                v.err(e, "territory.warn", "it warns before it wakes: `warn` at least `wake`");
            }
            if !e.wary {
                v.err(e, "territory", "only a wary faction keeps a territory (a hostile one wakes at the encounter's wake radius)");
            }
        }
    });
    v.entries(c.materials.as_slice(), |v, e| v.material(e));
    v.entries(c.parts.as_slice(), |v, e| v.part(e));
    v.entries(c.frames.as_slice(), |v, e| v.frame(e));
    v.entries(c.ships.as_slice(), |v, e| v.ship(e));
    v.entries(c.behavior_nodes.as_slice(), |v, e| v.behavior_node(e));
    v.entries(c.behaviors.as_slice(), |v, e| v.behavior(e));
    v.entries(c.families.as_slice(), |v, e| {
        if let Some(col) = &e.color {
            v.key_in(e, "color", col, SYSTEM_COLORS, "system colour");
        }
    });
    v.entries(c.research.as_slice(), |v, e| v.research(e));
    v.entries(c.facilities.as_slice(), |v, e| v.facility(e));
    if c.facilities.iter().filter(|f| f.workshop.is_some()).count() > 1 {
        v.out.push(Problem::error("more than one facility has `workshop` rules: only the first is the workshop").field("facilities"));
    }
    v.cycles(
        &c.research,
        |n| &n.requires,
        "requires",
        "requires",
        "can be researched",
    );
    v.entries(c.starts.as_slice(), |v, e| v.start(e));
    if c.starts.is_empty() {
        v.out.push(
            Problem::error("the content has no start (`starts.ron`): a new game cannot begin")
                .field("starts"),
        );
    }
    v.entries(c.encounters.as_slice(), |v, e| v.encounter(e));
    v.entries(c.voyages.as_slice(), |v, e| v.voyage(e));
    v.cycles(
        &c.voyages,
        |x| &x.opens_after,
        "opens_after",
        "opens after",
        "can ever open",
    );
    v.entries(c.galaxies.as_slice(), |v, e| v.galaxy(e));
    v.entries(c.bands.as_slice(), |v, e| v.band(e));
    v.entries(c.cluster_types.as_slice(), |v, e| v.cluster_type(e));
    v.entries(c.rocks.as_slice(), |v, e| v.rock_kind(e));
    v.entries(c.landmarks.as_slice(), |v, e| v.landmark(e));
    v.entries(c.planets.as_slice(), |v, e| v.planet_kind(e));
    v.entries(c.trails.as_slice(), |v, e| v.trail(e));
    v.entries(c.signal_sources.as_slice(), |v, e| v.signal_source(e));
    v.entries(c.lore.as_slice(), |v, e| v.lore_fragment(e));
    v.entries(c.controls.as_slice(), |v, e| v.control(e));
    v.controls();
    v.lab();
    v.voyages_open();
    v.locked();
    v.icons();
    v.wiki();
    v.out
}

/// Validates one blueprint against the content with exactly the checks [`validate`] runs on every content ship:
/// the header (id, name, summary, tags, links in its texts) and the ship rules. The blueprint need not be part of
/// `c`: a design in the drydock or a file being imported is checked the same way.
///
/// Every problem has kind `ship`, the blueprint's id and a field path (`size`, `parts[3]`, `parts[3].material`,
/// `parts[3].power.weight`, ...), so an editor can map it to a placement. `source` is set only for a blueprint
/// loaded from a pack.
///
/// Placement errors (unknown part, missing or wrong material, `rot` out of range, bad power override, outside
/// the frame, overlaps) end the check: the ship-wide rules after them (rule slots, control load, pieces, lines of
/// fire, RCS and radiator exposure) need a well-formed grid, so they report nothing until the placements are
/// clean. A blueprint without parts stops at that error too.
pub fn validate_ship(c: &Content, bp: &Blueprint) -> Vec<Problem> {
    let mut v = Validator { c, flies: None, out: Vec::new() };
    v.header(bp);
    v.ship(bp);
    v.out
}

/// [`validate_ship`] for a ship that flies `program` whatever behavior its blueprint names (an arena entry with its
/// own behavior text, an evolved genome, GDR-0092): the rule-slot check counts that program's rules instead of the
/// blueprint's or its faction's behavior.
pub fn validate_ship_flying(c: &Content, bp: &Blueprint, program: &Program) -> Vec<Problem> {
    let mut v = Validator { c, flies: Some(program), out: Vec::new() };
    v.header(bp);
    v.ship(bp);
    v.out
}

struct Validator<'a> {
    c: &'a Content,
    /// The program the ship flies, when it is not the one its blueprint names ([`validate_ship_flying`]).
    flies: Option<&'a Program>,
    out: Vec<Problem>,
}

/// Largest allowed engine gimbal, in degrees.
pub const MAX_GIMBAL: f32 = 45.0;
/// Longest allowed engine plume, in metres (keeps the plume casts short).
pub const MAX_PLUME_LENGTH: f32 = 50.0;
/// Largest explosion radius, in metres (keeps the blast lists of a design short; placeholder).
pub const MAX_BLAST_RADIUS: f32 = 4.0;
/// Longest sensor or scanner reach, in metres: a cluster's radius at most (one `f32` space, ADR-0022, E0).
pub const MAX_SENSOR_RANGE: f32 = 4000.0;
/// Longest allowed side of a drydock frame and of a ship's `size`, in cells (placeholder: twice the capital
/// frame). The drydock editor uses the same limit.
pub const MAX_FRAME_SIZE: u32 = 128;
/// Largest research data amount in a salvage block or a research cost (a sanity bound, placeholder).
pub const MAX_RESEARCH_DATA: f32 = 10_000.0;
/// Largest material amount in a salvage block or a research cost (a sanity bound, placeholder).
pub const MAX_MATERIAL_AMOUNT: u32 = 10_000;
/// Largest mind capacity of a start or a research step (a sanity bound, placeholder).
pub const MAX_MIND_CAPACITY: u32 = 100;
/// Longest allowed side of a part's footprint, in cells (placeholder).
pub const MAX_PART_SIDE: u32 = 16;
/// Largest voyage radius, site spacing and encounter wake radius, in metres (a sanity bound, placeholder: `f32`
/// still resolves millimetres there, ADR-0022).
pub const MAX_VOYAGE_RADIUS: f32 = 20_000.0;
/// Most sites one voyage places, the sum of its sites' counts (a sanity bound, placeholder).
pub const MAX_VOYAGE_SITES: u32 = 64;
/// Most ships or wrecks one entry of an encounter rolls (a sanity bound, placeholder).
pub const MAX_ENCOUNTER_COUNT: u32 = 32;
/// Longest expected voyage, in minutes (a sanity bound, placeholder).
pub const MAX_VOYAGE_MINUTES: u32 = 600;
/// Largest cluster radius, in metres: one `f32` simulation space centred on the origin holds about 4 km (ADR-0022,
/// galaxy track E0, `docs/results/2026-10-16-bigger-clusters.md`).
pub const MAX_CLUSTER_RADIUS: f32 = 4000.0;
/// Farthest a cluster's hot star lies from its centre, in cluster radii (a sanity bound, placeholder).
pub const MAX_STAR_DISTANCE: f32 = 10.0;
/// Largest galaxy radius and galaxy-map distance, in S (a sanity bound on the generator's work, placeholder).
pub const MAX_GALAXY_RADIUS: f32 = 100.0;
/// Most gates of one band, home's required neighbours and trails of one entry of a band (sanity bounds, placeholder).
pub const MAX_GATES: u32 = 16;
pub const MAX_HOME_NEIGHBOURS: u32 = 16;
pub const MAX_BAND_TRAILS: u32 = 64;
/// Most signals in one trail (GDR-0078's plan: 1 to 4).
pub const MAX_TRAIL_LENGTH: u32 = 4;
/// Largest number in content (masses, hit points, thrust, damage, bonus amounts, ...): a sanity limit that keeps
/// sums over a ship far from overflowing to infinity (placeholder).
pub const MAX_NUMBER: f32 = 1.0e6;
/// Most shots per second of a pulse or projectile weapon, before bonuses (placeholder: two per sim tick).
pub const MAX_SHOT_RATE: f32 = 60.0;
/// Largest `Percent` bonus; the smallest is above −100 (a stat never drops to 0 or below).
pub const MAX_BONUS_PERCENT: f32 = 1000.0;
/// Largest count in content: a projectile's penetration, a core's control capacity, a bonus's `max_stacks`.
pub const MAX_COUNT: u32 = 10_000;

pub(crate) fn is_snake_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl<'a> Validator<'a> {
    fn push<T: Entry>(&mut self, p: Problem, e: &T, field: impl Into<String>) {
        let mut p = p.entry(T::KIND, e.id()).field(field);
        // An entry built in memory (a drydock design) comes from no pack.
        if *e.source() != Source::default() {
            p = p.at(e.source());
        }
        self.out.push(p);
    }

    fn err<T: Entry>(&mut self, e: &T, field: impl Into<String>, msg: impl Into<String>) {
        self.push(Problem::error(msg), e, field);
    }

    fn warn<T: Entry>(&mut self, e: &T, field: impl Into<String>, msg: impl Into<String>) {
        self.push(Problem::warning(msg), e, field);
    }

    fn packs(&mut self) {
        let mut seen = BTreeSet::new();
        for p in &self.c.packs {
            let src = Source {
                pack: p.id.clone(),
                file: "pack.ron".into(),
            };
            if !is_snake_id(&p.id) {
                self.out.push(
                    Problem::error(format!("pack id `{}` must be snake_case", p.id))
                        .at(&src)
                        .field("id"),
                );
            }
            if p.schema_version != SCHEMA_VERSION {
                self.out.push(
                    Problem::error(format!(
                        "schema_version {} is not supported (this build reads {SCHEMA_VERSION})",
                        p.schema_version
                    ))
                    .at(&src)
                    .field("schema_version"),
                );
            }
            for d in &p.depends {
                if !seen.contains(d) {
                    self.out.push(
                        Problem::error(format!(
                            "depends on pack `{d}`, which is not loaded before it"
                        ))
                        .at(&src)
                        .field("depends"),
                    );
                }
            }
            if !seen.insert(p.id.clone()) {
                self.out.push(
                    Problem::error(format!("pack `{}` is loaded twice", p.id))
                        .at(&src)
                        .field("id"),
                );
            }
        }
    }

    /// Header checks for every entry of a kind, then the kind-specific checks.
    fn entries<T: Entry>(&mut self, items: &'a [T], mut check: impl FnMut(&mut Self, &'a T)) {
        for e in items {
            self.header(e);
            check(self, e);
        }
    }

    /// The checks every entry gets: id and tag format, non-empty name and summary, links in its texts.
    fn header<T: Entry>(&mut self, e: &T) {
        if !is_snake_id(e.id()) {
            self.err(
                e,
                "id",
                "ids must be snake_case: a-z, 0-9 and _, starting with a letter",
            );
        }
        if e.name().trim().is_empty() {
            self.err(e, "name", "name must not be empty");
        }
        if e.summary().trim().is_empty() {
            self.err(e, "summary", "summary must not be empty");
        }
        for (k, t) in e.tags().iter().enumerate() {
            if !is_snake_id(t) {
                self.err(
                    e,
                    format!("tags[{k}]"),
                    format!("tag `{t}` must be snake_case"),
                );
            }
        }
        for (field, text) in e.text_fields() {
            for problem in self.link_problems(text) {
                self.err(e, field, problem);
            }
        }
    }

    fn link_problems(&self, text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for l in parse_links(text) {
            match l {
                Err((_, msg)) => out.push(msg),
                Ok(link) => match Kind::from_key(link.kind) {
                    None => out.push(format!(
                        "link `[[{}:{}]]`: unknown kind `{}`",
                        link.kind, link.id, link.kind
                    )),
                    Some(k) if !self.c.exists(k, link.id) => out.push(format!(
                        "link `[[{}:{}]]` points at nothing",
                        link.kind, link.id
                    )),
                    _ => {}
                },
            }
        }
        out
    }

    fn key_in<T: Entry>(&mut self, e: &T, field: &str, key: &str, allowed: &[&str], what: &str) {
        if !allowed.contains(&key) {
            self.err(
                e,
                field,
                format!("unknown {what} key `{key}` (known: {})", allowed.join(", ")),
            );
        }
    }

    fn icon<T: Entry>(&mut self, e: &T, field: &str, key: &str) {
        if !is_icon_key(key) {
            self.err(
                e,
                field,
                format!("icon key `{key}` must be kebab-case (the stem of icons/<key>.svg)"),
            );
        } else if !self.c.icons.contains_key(key) {
            self.err(
                e,
                field,
                format!("icon `{key}` not found: no loaded pack has icons/{key}.svg"),
            );
        }
    }

    /// A machine's ship drawing (GDR-0069): a machine with an icon has `icons/<icon>.ship.svg`, and its viewBox fits
    /// the footprint; only engines reach past the hull. (A machine without an icon is drawn as a bare cell.)
    fn ship_drawing(&mut self, p: &'a Part) {
        let Some(key) = &p.icon else { return };
        if !is_icon_key(key) {
            return; // reported by `icon`
        }
        let Some(svg) = self.c.ship_svg(key) else {
            self.err(
                p,
                "icon",
                format!("machine without a ship drawing: no loaded pack has icons/{key}.ship.svg (GDR-0069)"),
            );
            return;
        };
        let Some(vb) = crate::content::svg::view_box(svg) else { return }; // reported by `icons`
        match crate::content::svg::ship_overhang(vb, p.size) {
            Err(msg) => self.err(p, "icon", format!("icons/{key}.ship.svg: {msg}")),
            Ok(over) if over > 0.0 && p.engine.is_none() => self.err(
                p,
                "icon",
                format!(
                    "icons/{key}.ship.svg reaches {over} cells past the stern, but only engines reach past the hull (GDR-0069)"
                ),
            ),
            Ok(_) => {}
        }
    }

    /// Every icon is a usable SVG; icons nothing refers to are probably left over.
    fn icons(&mut self) {
        let c = self.c;
        let used: BTreeSet<&str> = c
            .parts
            .iter()
            .filter_map(|p| p.icon.as_deref())
            .chain(c.materials.iter().filter_map(|m| m.icon.as_deref()))
            .chain(c.categories.iter().filter_map(|k| k.icon.as_deref()))
            .collect();
        for icon in c.icons.values() {
            if let Err(msg) = crate::content::svg::check_svg(&icon.svg) {
                self.out
                    .push(Problem::error(format!("icon `{}`: {msg}", icon.key)).at(&icon.source));
            }
            if !used.contains(icon.key.as_str()) {
                self.out.push(
                    Problem::warning(format!(
                        "icon `{}` is not used by any part, material or category",
                        icon.key
                    ))
                    .at(&icon.source),
                );
            }
        }
        let machines: BTreeSet<&str> =
            c.parts.iter().filter(|p| !p.hull).filter_map(|p| p.icon.as_deref()).collect();
        for art in c.ship_art.values() {
            if let Err(msg) = crate::content::svg::check_svg(&art.svg) {
                self.out
                    .push(Problem::error(format!("ship drawing `{}`: {msg}", art.key)).at(&art.source));
            }
            if !machines.contains(art.key.as_str()) {
                self.out.push(
                    Problem::warning(format!(
                        "ship drawing `{}` is not used: no machine has the icon `{}`",
                        art.key, art.key
                    ))
                    .at(&art.source),
                );
            }
        }
    }

    /// A power weight: finite, above 0, at most [`MAX_POWER_WEIGHT`].
    fn power_weight<T: Entry>(&mut self, e: &T, field: &str, w: f32) {
        if !(w.is_finite() && w > 0.0 && w <= MAX_POWER_WEIGHT) {
            self.err(
                e,
                field,
                format!("a weight above 0 and at most {MAX_POWER_WEIGHT}, got {w}"),
            );
        }
    }

    /// A finite number above (`strict`) or at least `min`, and at most [`MAX_NUMBER`].
    fn number<T: Entry>(&mut self, e: &T, field: &str, x: f32, min: f32, strict: bool) {
        let ok = x.is_finite() && x <= MAX_NUMBER && if strict { x > min } else { x >= min };
        if !ok {
            let rel = if strict { ">" } else { ">=" };
            self.err(
                e,
                field,
                format!("must be a finite number {rel} {min} and at most {MAX_NUMBER}, got {x}"),
            );
        }
    }

    /// A count of at most `max`.
    fn count<T: Entry>(&mut self, e: &T, field: &str, n: u32, max: u32) {
        if n > max {
            self.err(e, field, format!("at most {max}, got {n}"));
        }
    }

    /// A shot rate: above 0 and at most [`MAX_SHOT_RATE`] per second.
    fn shot_rate<T: Entry>(&mut self, e: &T, rate: f32) {
        self.number(e, "weapon.delivery.rate", rate, 0.0, true);
        if rate > MAX_SHOT_RATE {
            self.err(
                e,
                "weapon.delivery.rate",
                format!("at most {MAX_SHOT_RATE} shots per second, got {rate}"),
            );
        }
    }

    /// A sensor's or scanner's reach: above 0, at most [`MAX_SENSOR_RANGE`].
    fn reach<T: Entry>(&mut self, e: &T, field: &str, x: f32) {
        self.number(e, field, x, 0.0, true);
        if x > MAX_SENSOR_RANGE {
            self.err(e, field, format!("at most {MAX_SENSOR_RANGE} m (a cluster's radius), got {x}"));
        }
    }

    fn stats(&mut self) {
        self.entries(self.c.stats.as_slice(), |v, e| {
            if e.decimals > 6 {
                v.err(e, "decimals", "at most 6 decimals");
            }
        });
        for id in BUILTIN_STATS.iter().chain(SHIP_STATS) {
            if !self.c.stats.contains(id) {
                self.out.push(
                    Problem::error(format!(
                        "stat `{id}` is used by the game but not defined in stats.ron"
                    ))
                    .entry(Kind::Stat, id),
                );
            }
        }
    }

    fn material(&mut self, m: &'a Material) {
        if let Some(icon) = &m.icon {
            self.icon(m, "icon", icon);
        }
        if let Some(a) = &m.armor {
            self.key_in(m, "armor.tone", &a.tone, ARMOR_TONES, "armour tone");
            self.number(m, "armor.mass_per_cell", a.mass_per_cell, 0.0, true);
            self.number(m, "armor.hp_per_cell", a.hp_per_cell, 0.0, true);
            self.number(m, "armor.blast_factor", a.blast_factor, 0.0, false);
            self.number(m, "armor.heal", a.heal, 0.0, false);
            self.number(m, "armor.refract", a.refract, 0.0, false);
            if a.refract > 1.0 {
                self.err(m, "armor.refract", "a share of the shot: at most 1");
            }
            self.number(
                m,
                "armor.build_point_factor",
                a.build_point_factor,
                0.0,
                true,
            );
        }
        if let Some(s) = &m.salvage {
            if m.armor.is_none() {
                self.err(
                    m,
                    "salvage",
                    "only armour materials are salvaged (per cell of hull); remove `salvage`",
                );
            }
            self.family_ref(m, "salvage.family", &s.family);
            self.research_data(m, "salvage.data", s.data, false);
        }
    }

    fn family_ref<T: Entry>(&mut self, e: &T, field: &str, id: &str) {
        if !self.c.families.contains(id) {
            self.err(e, field, format!("unknown research family `{id}`"));
        }
    }

    /// A research data amount: finite, not negative (above 0 if `strict`), at most [`MAX_RESEARCH_DATA`] (one
    /// error per field).
    fn research_data<T: Entry>(&mut self, e: &T, field: &str, x: f32, strict: bool) {
        let before = self.out.len();
        self.number(e, field, x, 0.0, strict);
        if self.out.len() == before && x > MAX_RESEARCH_DATA {
            self.err(e, field, format!("at most {MAX_RESEARCH_DATA}, got {x}"));
        }
    }

    /// What a trail's end or a voyage's first clear `gives`: known families and materials, amounts in range.
    fn gives<T: Entry>(&mut self, e: &T, gives: &ResearchCost) {
        for (id, &x) in &gives.data {
            let f = format!("gives.data.{id}");
            if !self.c.families.contains(id) {
                self.err(e, &f, format!("unknown research family `{id}`"));
            }
            self.research_data(e, &f, x, true);
        }
        self.material_amounts(e, "gives.materials", &gives.materials);
    }

    /// Material amounts: known materials, each 1 to [`MAX_MATERIAL_AMOUNT`].
    fn material_amounts<T: Entry>(&mut self, e: &T, field: &str, amounts: &BTreeMap<String, u32>) {
        for (id, &n) in amounts {
            let f = format!("{field}.{id}");
            if !self.c.materials.contains(id) {
                self.err(e, &f, format!("unknown material `{id}`"));
            }
            if !(1..=MAX_MATERIAL_AMOUNT).contains(&n) {
                self.err(
                    e,
                    &f,
                    format!("an amount of 1 to {MAX_MATERIAL_AMOUNT}, got {n}"),
                );
            }
        }
    }

    /// A mind capacity (of a start or a research step): 1 to [`MAX_MIND_CAPACITY`].
    fn mind_capacity<T: Entry>(&mut self, e: &T, field: &str, n: u32) {
        if !(1..=MAX_MIND_CAPACITY).contains(&n) {
            self.err(
                e,
                field,
                format!("a mind capacity of 1 to {MAX_MIND_CAPACITY} logic cores, got {n}"),
            );
        }
    }

    fn frame(&mut self, f: &'a Frame) {
        let (w, h) = f.size;
        if !(1..=MAX_FRAME_SIZE).contains(&w) || !(1..=MAX_FRAME_SIZE).contains(&h) {
            self.err(
                f,
                "size",
                format!("each side must be 1 to {MAX_FRAME_SIZE} cells; got {w}×{h}"),
            );
        }
    }

    fn wiki(&mut self) {
        for page in self.c.wiki.values() {
            let base = Problem::error("")
                .at(&page.source)
                .entry(page.kind, &page.id);
            if page.kind != Kind::Page && !self.c.exists(page.kind, &page.id) {
                self.out.push(Problem {
                    message: format!("wiki page for a {} that does not exist", page.kind),
                    ..base.clone()
                });
            }
            if page.kind == Kind::Page && !is_snake_id(&page.id) {
                self.out.push(Problem {
                    message: "wiki page ids must be snake_case".into(),
                    ..base.clone()
                });
            }
            for msg in self.link_problems(&page.markdown) {
                self.out.push(Problem {
                    message: msg,
                    ..base.clone()
                });
            }
        }
    }
}
