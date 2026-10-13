//! Validation of merged content ([`validate`]) and of a single blueprint against it ([`validate_ship`], for
//! drydock designs). Every problem names pack, file, kind, id and field where possible.

use std::collections::{BTreeMap, BTreeSet};

use crate::art_keys::{ARMOR_TONES, GLOWS, SYSTEM_COLORS, is_icon_key};
use crate::kind::Kind;
use crate::load::{Content, SCHEMA_VERSION};
use crate::model::*;
use crate::problem::Problem;
use crate::stats::{BUILTIN_STATS, SHIP_STATS};
use crate::text::parse_links;

/// Validates merged content. Returns errors and warnings (warnings: ships in several pieces, blocked guns,
/// unused icons, research that unlocks what is already open).
pub fn validate(c: &Content) -> Vec<Problem> {
    let mut v = Validator { c, out: Vec::new() };
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
    v.research_cycles();
    v.entries(c.starts.as_slice(), |v, e| v.start(e));
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
    let mut v = Validator { c, out: Vec::new() };
    v.header(bp);
    v.ship(bp);
    v.out
}

struct Validator<'a> {
    c: &'a Content,
    out: Vec<Problem>,
}

/// Largest allowed engine gimbal, in degrees.
pub const MAX_GIMBAL: f32 = 45.0;
/// Longest allowed engine plume, in metres (keeps the plume casts short).
pub const MAX_PLUME_LENGTH: f32 = 50.0;
/// Longest allowed side of a drydock frame, in cells (placeholder: twice the capital frame).
pub const MAX_FRAME_SIZE: u32 = 128;
/// Largest research data amount in a salvage block or a research cost (a sanity bound, placeholder).
pub const MAX_RESEARCH_DATA: f32 = 10_000.0;
/// Largest material amount in a salvage block or a research cost (a sanity bound, placeholder).
pub const MAX_MATERIAL_AMOUNT: u32 = 10_000;
/// Largest mind capacity of a start or a research step (a sanity bound, placeholder).
pub const MAX_MIND_CAPACITY: u32 = 100;

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
            if let Err(msg) = crate::svg::check_svg(&icon.svg) {
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

    fn number<T: Entry>(&mut self, e: &T, field: &str, x: f32, min: f32, strict: bool) {
        let ok = x.is_finite() && if strict { x > min } else { x >= min };
        if !ok {
            let rel = if strict { ">" } else { ">=" };
            self.err(
                e,
                field,
                format!("must be a finite number {rel} {min}, got {x}"),
            );
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

    /// A research data amount: finite, not negative (above 0 if `strict`), at most [`MAX_RESEARCH_DATA`].
    fn research_data<T: Entry>(&mut self, e: &T, field: &str, x: f32, strict: bool) {
        self.number(e, field, x, 0.0, strict);
        if x > MAX_RESEARCH_DATA {
            self.err(e, field, format!("at most {MAX_RESEARCH_DATA}, got {x}"));
        }
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

    fn part(&mut self, p: &'a Part) {
        if !self.c.categories.contains(&p.category) {
            self.err(p, "category", format!("unknown category `{}`", p.category));
        }
        if let Some(icon) = &p.icon {
            self.icon(p, "icon", icon);
        }
        if let Some(col) = &p.color {
            self.key_in(p, "color", col, SYSTEM_COLORS, "system colour");
        }
        let (w, h) = p.size;
        if w == 0 || h == 0 {
            self.err(p, "size", "footprint must be at least 1×1");
        }
        if p.shape == Shape::Slope {
            if h != 1 {
                self.err(p, "size", format!("slopes must be one cell high (w × 1); got {w}×{h}. Steep slopes come from rotating"));
            }
            if !p.hull {
                self.err(p, "shape", "slopes must be hull parts (hull: true)");
            }
        }
        self.number(p, "build_points", p.build_points, 0.0, false);
        if p.hull {
            if p.mass.is_some() {
                self.err(
                    p,
                    "mass",
                    "hull parts take their mass from the material; remove `mass`",
                );
            }
            if p.hp.is_some() {
                self.err(
                    p,
                    "hp",
                    "hull parts take their hit points from the material; remove `hp`",
                );
            }
        } else {
            match p.mass {
                Some(m) => self.number(p, "mass", m, 0.0, true),
                None => self.err(p, "mass", "non-hull parts need `mass`"),
            }
            match p.hp {
                Some(x) => self.number(p, "hp", x, 0.0, true),
                None => self.err(p, "hp", "non-hull parts need `hp`"),
            }
        }
        if let Some(s) = &p.salvage {
            if p.hull {
                self.err(
                    p,
                    "salvage",
                    "hull parts are salvaged per cell by their material's `salvage`; remove `salvage`",
                );
            }
            self.family_ref(p, "salvage.family", &s.family);
            self.research_data(p, "salvage.data", s.data, false);
            self.material_amounts(p, "salvage.materials", &s.materials);
            self.material_amounts(p, "salvage.fragment", &s.fragment);
        }
        if let Some(x) = &p.power {
            self.number(p, "power.output", x.output, 0.0, false);
            self.number(p, "power.draw", x.draw, 0.0, false);
            self.power_weight(p, "power.weight", x.weight);
            if !(x.min_level.is_finite() && (0.0..=1.0).contains(&x.min_level)) {
                self.err(
                    p,
                    "power.min_level",
                    format!("a share of the demand, 0 to 1, got {}", x.min_level),
                );
            }
            let allocation_set =
                x.tier != PowerTier::default() || x.weight != 1.0 || x.min_level != 0.0;
            if x.draw <= 0.0 && allocation_set {
                self.err(
                    p,
                    "power",
                    "`tier`, `weight` and `min_level` are for parts that draw power",
                );
            }
        }
        if let Some(x) = &p.heat {
            self.number(p, "heat.output", x.output, 0.0, false);
            if let Some(l) = x.limit {
                self.number(p, "heat.limit", l, 0.0, true);
            }
        }
        if let Some(x) = &p.radiator {
            self.number(p, "radiator.vent", x.vent, 0.0, true);
        }
        if let Some(x) = &p.weapon {
            self.number(p, "weapon.damage", x.damage, 0.0, false);
            self.number(p, "weapon.range", x.range, 0.0, true);
            match x.delivery {
                crate::model::Delivery::Beam => {}
                crate::model::Delivery::Pulse { rate } => {
                    self.number(p, "weapon.delivery.rate", rate, 0.0, true)
                }
                crate::model::Delivery::Projectile {
                    rate,
                    speed,
                    penetration,
                    recoil,
                } => {
                    self.number(p, "weapon.delivery.rate", rate, 0.0, true);
                    self.number(p, "weapon.delivery.speed", speed, 0.0, true);
                    self.number(p, "weapon.delivery.recoil", recoil, 0.0, false);
                    if penetration == 0 {
                        self.err(p, "weapon.delivery.penetration", "must be at least 1");
                    }
                }
            }
        }
        if let Some(x) = &p.engine {
            self.number(p, "engine.thrust", x.thrust, 0.0, true);
            self.number(p, "engine.rated_speed", x.rated_speed, 0.0, true);
            self.number(p, "engine.gimbal", x.gimbal, 0.0, false);
            if x.gimbal > MAX_GIMBAL {
                self.err(
                    p,
                    "engine.gimbal",
                    format!("at most {MAX_GIMBAL} degrees, got {}", x.gimbal),
                );
            }
            self.number(p, "engine.plume.length", x.plume.length, 0.0, false);
            self.number(p, "engine.plume.damage", x.plume.damage, 0.0, false);
            self.number(p, "engine.plume.heat", x.plume.heat, 0.0, false);
            if x.plume.length > MAX_PLUME_LENGTH {
                self.err(
                    p,
                    "engine.plume.length",
                    format!("at most {MAX_PLUME_LENGTH} m, got {}", x.plume.length),
                );
            }
        }
        if let Some(x) = &p.gyro {
            self.number(p, "gyro.torque", x.torque, 0.0, true);
        }
        if let Some(x) = &p.rcs {
            self.number(p, "rcs.thrust", x.thrust, 0.0, true);
            self.number(p, "rcs.rated_speed", x.rated_speed, 0.0, true);
            if p.hull {
                self.err(p, "rcs", "hull parts cannot carry RCS nozzles");
            }
        }
        if let Some(x) = &p.shield {
            self.number(p, "shield.radius", x.radius, 0.0, true);
            self.number(p, "shield.capacity", x.capacity, 0.0, false);
            self.number(p, "shield.regen", x.regen, 0.0, false);
            for (f, &a) in &x.absorb {
                self.number(p, &format!("shield.absorb.{f:?}"), a, 0.0, false);
                if a > 1.0 {
                    self.err(p, format!("shield.absorb.{f:?}"), "a share, at most 1");
                }
            }
        }
        if let Some(x) = &p.cargo {
            self.number(p, "cargo.capacity", x.capacity, 0.0, true);
        }
        if let Some(c) = p.control_cost {
            if c > MAX_CONTROL_COST {
                self.err(
                    p,
                    "control_cost",
                    format!("at most {MAX_CONTROL_COST}, got {c}"),
                );
            } else if c > 0 && !p.has_power_role() {
                self.warn(
                    p,
                    "control_cost",
                    "the part draws and makes no power, so it can never go idle; it only uses up control capacity",
                );
            }
        }
        if let Some(l) = &p.logic
            && l.control_capacity == 0
        {
            self.warn(
                p,
                "logic.control_capacity",
                "a core with no control capacity runs no parts",
            );
        }
        for (k, b) in p.bonuses.iter().enumerate() {
            let f = format!("bonuses[{k}]");
            match &b.target {
                Selector::Part(id) if !self.c.parts.contains(id) => {
                    self.err(p, format!("{f}.target"), format!("unknown part `{id}`"))
                }
                Selector::Category(id) if !self.c.categories.contains(id) => {
                    self.err(p, format!("{f}.target"), format!("unknown category `{id}`"))
                }
                Selector::Tag(t) if !self.c.parts.iter().any(|q| q.has_tag(t)) => self.err(
                    p,
                    format!("{f}.target"),
                    format!("no part has the tag `{t}`"),
                ),
                _ => {}
            }
            if !self.c.stats.contains(&b.stat) {
                self.err(p, format!("{f}.stat"), format!("unknown stat `{}`", b.stat));
            }
            let amount = match b.effect {
                Effect::Add(x) | Effect::Percent(x) => x,
            };
            if !amount.is_finite() {
                self.err(p, format!("{f}.effect"), "must be a finite number");
            }
            if b.max_stacks == Some(0) {
                self.err(
                    p,
                    format!("{f}.max_stacks"),
                    "must be at least 1 (or omitted for no limit)",
                );
            }
        }
    }

    /// Locked parts and materials are hidden in the drydock until unlocked, so the parts the drydock paints,
    /// fills and bevels with must stay available, and so must at least one armour material.
    fn locked(&mut self) {
        let c = self.c;
        for (shape, what) in [
            (Shape::Rect, "paints and fills with"),
            (Shape::Slope, "bevels with"),
        ] {
            if let Some(p) = c.unit_hull_part(shape).filter(|p| p.locked) {
                self.err(
                    p,
                    "locked",
                    format!("the drydock {what} the first 1×1 hull part of this shape, so it can't be locked"),
                );
            }
        }
        let mut armor = c.materials.iter().filter(|m| m.armor.is_some());
        if let Some(first) = armor.clone().next()
            && armor.all(|m| m.locked)
        {
            self.err(
                first,
                "locked",
                "every armour material is locked: the drydock has nothing to build hull parts in",
            );
        }
        if let Some(first) = c.frames.iter().next()
            && c.frames.iter().all(|f| f.locked)
        {
            self.err(
                first,
                "locked",
                "every frame is locked: a new game has no frame to build in",
            );
        }
    }

    fn research(&mut self, n: &'a ResearchNode) {
        let c = self.c;
        self.family_ref(n, "family", &n.family);
        for (id, &x) in &n.cost.data {
            let f = format!("cost.data.{id}");
            if !c.families.contains(id) {
                self.err(n, &f, format!("unknown research family `{id}`"));
            }
            self.research_data(n, &f, x, true);
        }
        self.material_amounts(n, "cost.materials", &n.cost.materials);
        for (k, r) in n.requires.iter().enumerate() {
            let f = format!("requires[{k}]");
            if !c.research.contains(r) {
                self.err(n, &f, format!("unknown research node `{r}`"));
            } else if n.requires[..k].contains(r) {
                self.err(n, &f, format!("`{r}` is listed twice"));
            }
        }
        if n.unlocks.is_empty() {
            self.warn(n, "unlocks", "the node unlocks nothing");
        }
        for (k, u) in n.unlocks.iter().enumerate() {
            let f = format!("unlocks[{k}]");
            let (kind, id, locked) = match u {
                Unlock::MindCapacity(x) => {
                    self.mind_capacity(n, &f, *x);
                    continue;
                }
                Unlock::Part(id) => (Kind::Part, id, c.parts.get(id).map(|p| p.locked)),
                Unlock::Material(id) => (
                    Kind::Material,
                    id,
                    c.materials.get(id).map(|m| m.locked),
                ),
                Unlock::Frame(id) => (Kind::Frame, id, c.frames.get(id).map(|x| x.locked)),
            };
            match locked {
                None => self.err(n, &f, format!("unknown {kind} `{id}`")),
                Some(false) => self.warn(
                    n,
                    &f,
                    format!(
                        "{kind} `{id}` is not `locked`: a new game has it already, so unlocking it changes nothing"
                    ),
                ),
                Some(true) => {}
            }
        }
    }

    /// `requires` must not loop: each cycle is reported once, at the requirement that closes it.
    fn research_cycles(&mut self) {
        let c = self.c;
        let nodes = c.research.as_slice();
        // 0: not visited, 1: on the current path, 2: done.
        let mut state = vec![0u8; nodes.len()];
        for root in 0..nodes.len() {
            if state[root] != 0 {
                continue;
            }
            state[root] = 1;
            // Depth first without recursion (mods may chain many nodes): (node, next requirement to follow).
            let mut path: Vec<(usize, usize)> = vec![(root, 0)];
            while let Some(top) = path.last_mut() {
                let (node, k) = *top;
                let Some(r) = nodes[node].requires.get(k) else {
                    state[node] = 2;
                    path.pop();
                    continue;
                };
                top.1 += 1;
                let Some(j) = c.research.position(r) else {
                    continue; // an unknown node: reported by `research`
                };
                match state[j] {
                    0 => {
                        state[j] = 1;
                        path.push((j, 0));
                    }
                    1 => {
                        let from = path.iter().position(|&(x, _)| x == j).unwrap();
                        let cycle: Vec<&str> = path[from..]
                            .iter()
                            .map(|&(x, _)| nodes[x].id.as_str())
                            .chain([nodes[j].id.as_str()])
                            .collect();
                        self.err(
                            &nodes[node],
                            format!("requires[{k}]"),
                            format!(
                                "`requires` forms a cycle ({}, each requires the next), so none of them can be researched",
                                cycle.join(" → ")
                            ),
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    /// A start's ship must be one a new game can take on a voyage: built from what is not locked, fitting a frame
    /// that is not locked, with no more logic cores than the start's mind capacity.
    fn start(&mut self, s: &'a Start) {
        let c = self.c;
        self.mind_capacity(s, "mind_capacity", s.mind_capacity);
        for (k, v) in s.voyages.iter().enumerate() {
            let f = format!("voyages[{k}]");
            if !is_snake_id(v) {
                self.err(s, &f, format!("voyage id `{v}` must be snake_case"));
            } else if s.voyages[..k].contains(v) {
                self.err(s, &f, format!("`{v}` is listed twice"));
            }
        }
        let Some(ship) = c.ships.get(&s.ship) else {
            self.err(s, "ship", format!("unknown ship `{}`", s.ship));
            return;
        };
        let mut locked = BTreeSet::new();
        for pl in &ship.parts {
            if c.parts.get(&pl.part).is_some_and(|p| p.locked) {
                locked.insert(format!("part `{}`", pl.part));
            }
            if let Some(m) = c.placement_material(pl).filter(|m| m.locked) {
                locked.insert(format!("material `{}`", m.id));
            }
        }
        if !locked.is_empty() {
            let list: Vec<String> = locked.into_iter().collect();
            self.err(
                s,
                "ship",
                format!(
                    "ship `{}` uses what a new game doesn't have: {}",
                    s.ship,
                    list.join(", ")
                ),
            );
        }
        let (w, h) = ship.size;
        if !c
            .frames
            .iter()
            .any(|f| !f.locked && f.size.0 >= w && f.size.1 >= h)
        {
            self.err(
                s,
                "ship",
                format!(
                    "ship `{}` ({w}×{h}) fits no frame that is not locked",
                    s.ship
                ),
            );
        }
        let cores = c.ship_totals(ship).logic_cores;
        if cores > s.mind_capacity {
            self.err(
                s,
                "mind_capacity",
                format!(
                    "ship `{}` has {cores} logic cores, more than the mind capacity {}",
                    s.ship, s.mind_capacity
                ),
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

    fn behavior_ref<T: Entry>(&mut self, e: &T, id: &str) {
        if !self.c.behaviors.contains(id) {
            let mut msg = format!("unknown behavior `{id}`");
            if let Some(s) =
                crate::behavior::suggest(id, self.c.behaviors.iter().map(|b| b.id.as_str()))
            {
                msg += &format!("; did you mean `{s}`?");
            }
            self.err(e, "behavior", msg);
        }
    }

    fn behavior_node(&mut self, n: &'a BehaviorNode) {
        use crate::behavior::{KEYWORDS, MAX_ARGS, check_param_value, parse_value};
        if KEYWORDS.contains(&n.id.as_str()) {
            self.err(
                n,
                "id",
                format!("`{}` is a keyword of the behavior language", n.id),
            );
        }
        if n.value.is_some() && n.role != NodeRole::Condition {
            self.err(n, "value", "only conditions yield a value");
        }
        if let Some(script) = &n.script {
            if n.role != NodeRole::Condition {
                self.err(n, "script", "only conditions can be scripted (for now)");
            } else if n.value.is_some() {
                self.err(
                    n,
                    "script",
                    "a scripted condition must be a flag (no `value`) for now",
                );
            }
            if script.trim().is_empty() {
                self.err(n, "script", "empty script");
            }
        }
        if n.params.len() > MAX_ARGS {
            self.err(n, "params", format!("at most {MAX_ARGS} parameters"));
        }
        let mut seen = BTreeSet::new();
        let mut optional = false;
        for (k, p) in n.params.iter().enumerate() {
            let f = format!("params[{k}]");
            if !is_snake_id(&p.id) || KEYWORDS.contains(&p.id.as_str()) {
                self.err(
                    n,
                    format!("{f}.id"),
                    format!(
                        "parameter id `{}` must be snake_case and not a keyword",
                        p.id
                    ),
                );
            }
            if !seen.insert(p.id.as_str()) {
                self.err(
                    n,
                    format!("{f}.id"),
                    format!("parameter `{}` is declared twice", p.id),
                );
            }
            if p.name.trim().is_empty() {
                self.err(n, format!("{f}.name"), "name must not be empty");
            }
            for (field, text) in [
                ("name", Some(p.name.as_str())),
                ("summary", p.summary.as_deref()),
            ] {
                for problem in text.map(|t| self.link_problems(t)).unwrap_or_default() {
                    self.err(n, format!("{f}.{field}"), problem);
                }
            }
            if let ParamType::Choice(opts) = &p.ty {
                if opts.is_empty() {
                    self.err(n, format!("{f}.type"), "a choice needs at least one option");
                }
                let mut ids = BTreeSet::new();
                for (j, o) in opts.iter().enumerate() {
                    if !is_snake_id(&o.id) || KEYWORDS.contains(&o.id.as_str()) {
                        self.err(
                            n,
                            format!("{f}.type[{j}].id"),
                            format!("option id `{}` must be snake_case and not a keyword", o.id),
                        );
                    }
                    if !ids.insert(o.id.as_str()) {
                        self.err(
                            n,
                            format!("{f}.type[{j}].id"),
                            format!("option `{}` is listed twice", o.id),
                        );
                    }
                    if o.name.trim().is_empty() {
                        self.err(n, format!("{f}.type[{j}].name"), "name must not be empty");
                    }
                }
                if p.min.is_some() || p.max.is_some() {
                    self.err(n, &f, "`min` and `max` are for numeric parameters");
                }
            }
            for (field, x) in [("min", p.min), ("max", p.max)] {
                if let Some(x) = x
                    && !x.is_finite()
                {
                    self.err(n, format!("{f}.{field}"), "must be a finite number");
                }
            }
            if let (Some(lo), Some(hi)) = (p.min, p.max)
                && lo > hi
            {
                self.err(n, &f, format!("min {lo} is above max {hi}"));
            }
            match &p.default {
                Some(d) => {
                    optional = true;
                    match parse_value(d).and_then(|v| check_param_value(&v, p)) {
                        Ok(_) => {}
                        Err(m) => self.err(n, format!("{f}.default"), m),
                    }
                }
                None if optional => self.err(
                    n,
                    format!("{f}.default"),
                    "parameters without a default must come before those with one",
                ),
                None => {}
            }
        }
    }

    fn behavior(&mut self, b: &'a Behavior) {
        let program = match b.program() {
            Ok(p) => p,
            Err(errors) => {
                for e in errors {
                    self.err(
                        b,
                        e.line.field(),
                        format!("column {}: {}", e.col, e.message),
                    );
                }
                return;
            }
        };
        for e in crate::behavior::check_program(&program, &self.c.behavior_nodes) {
            let msg = match e.line {
                crate::behavior::LineRef::Whole => e.message,
                _ => format!("column {}: {}", e.col, e.message),
            };
            self.err(b, e.line.field(), msg);
        }
    }

    fn ship(&mut self, s: &'a Blueprint) {
        if !self.c.factions.contains(&s.faction) {
            self.err(s, "faction", format!("unknown faction `{}`", s.faction));
        }
        if let Some(b) = &s.behavior {
            self.behavior_ref(s, b);
        }
        let (sw, sh) = (s.size.0 as i32, s.size.1 as i32);
        if sw == 0 || sh == 0 {
            self.err(s, "size", "size must be at least 1×1");
        }
        if s.parts.is_empty() {
            self.err(s, "parts", "a ship needs at least one part");
            return;
        }
        let errors_before = self.out.iter().filter(|p| p.is_error()).count();
        let mut occupied: BTreeMap<(i32, i32), usize> = BTreeMap::new();
        for (k, pl) in s.parts.iter().enumerate() {
            let f = format!("parts[{k}]");
            if pl.rot > 3 {
                self.err(
                    s,
                    format!("{f}.rot"),
                    format!("rot is a number of quarter turns, 0 to 3; got {}", pl.rot),
                );
            }
            let Some(part) = self.c.parts.get(&pl.part) else {
                self.err(
                    s,
                    format!("{f}.part"),
                    format!("unknown part `{}`", pl.part),
                );
                continue;
            };
            match (&pl.material, part.hull) {
                (None, true) => self.err(
                    s,
                    format!("{f}.material"),
                    format!("hull part `{}` needs a material", pl.part),
                ),
                (Some(m), true) => match self.c.materials.get(m) {
                    None => self.err(
                        s,
                        format!("{f}.material"),
                        format!("unknown material `{m}`"),
                    ),
                    Some(mat) if mat.armor.is_none() => self.err(
                        s,
                        format!("{f}.material"),
                        format!("material `{m}` is not an armour material"),
                    ),
                    _ => {}
                },
                (Some(m), false) => self.err(
                    s,
                    format!("{f}.material"),
                    format!("`{}` is not a hull part; remove material `{m}`", pl.part),
                ),
                (None, false) => {}
            }
            if let Some(o) = &pl.power {
                if part.power.as_ref().is_none_or(|x| x.draw <= 0.0) {
                    self.err(
                        s,
                        format!("{f}.power"),
                        format!("`{}` draws no power; remove `power`", pl.part),
                    );
                }
                if let Some(w) = o.weight {
                    self.power_weight(s, &format!("{f}.power.weight"), w);
                }
                if o.tier.is_none() && o.weight.is_none() {
                    self.warn(
                        s,
                        format!("{f}.power"),
                        "an empty power override changes nothing",
                    );
                }
            }
            let shape = self.c.placed_shape(pl).unwrap();
            let (w, h) = shape.dims();
            if pl.x < 0 || pl.y < 0 || pl.x + w as i32 > sw || pl.y + h as i32 > sh {
                self.err(
                    s,
                    &f,
                    format!(
                        "`{}` at ({}, {}) with footprint {w}×{h} does not fit the {sw}×{sh} frame",
                        pl.part, pl.x, pl.y
                    ),
                );
            }
            for cell in shape.cells() {
                if let Some(other) = occupied.insert(cell, k) {
                    self.err(
                        s,
                        &f,
                        format!(
                            "`{}` overlaps parts[{other}] (`{}`) at cell {:?}",
                            pl.part, s.parts[other].part, cell
                        ),
                    );
                    break;
                }
            }
        }
        if self.out.iter().filter(|p| p.is_error()).count() > errors_before {
            return;
        }
        // The behavior it flies under must fit its rule slots (a ship without cores cannot think at all).
        if let Some(b) = self.c.ship_behavior(s) {
            let slots = self.c.ship_rule_slots(s);
            if slots > 0 && b.rules.len() > slots as usize {
                let (field, whose) = match &s.behavior {
                    Some(_) => ("behavior", "its behavior".to_string()),
                    None => ("faction", "its faction's default behavior".to_string()),
                };
                self.err(
                    s,
                    field,
                    format!(
                        "{whose} `{}` has {} rules, but the ship's logic cores give only {slots} rule slots",
                        b.id,
                        b.rules.len()
                    ),
                );
            }
        }
        // Design warnings (valid content, but probably not intended).
        let (load, capacity) = self.c.ship_control(s);
        if capacity > 0 && load > capacity {
            self.warn(
                s,
                "parts",
                format!(
                    "control load {load} exceeds the logic cores' control capacity {capacity}: the lowest-priority parts start idle"
                ),
            );
        }
        let grid = self.c.ship_grid(s).unwrap();
        let pieces = grid.pieces();
        if pieces.len() > 1 {
            let sizes: Vec<usize> = pieces.parts.iter().map(Vec::len).collect();
            self.warn(
                s,
                "parts",
                format!(
                    "the ship falls apart into {} pieces (parts per piece: {sizes:?})",
                    pieces.len()
                ),
            );
        }
        for (k, pl) in s.parts.iter().enumerate() {
            let part = self.c.parts.get(&pl.part).unwrap();
            if part.weapon.is_some() && !grid.line_of_fire_clear(k) {
                self.warn(
                    s,
                    format!("parts[{k}]"),
                    format!(
                        "`{}` at ({}, {}) has no clear line of fire",
                        pl.part, pl.x, pl.y
                    ),
                );
            }
            if part.rcs.is_some() && grid.open_sides(k).is_empty() {
                self.warn(
                    s,
                    format!("parts[{k}]"),
                    format!(
                        "`{}` at ({}, {}) has no side open to space, so no RCS nozzle",
                        pl.part, pl.x, pl.y
                    ),
                );
            }
            if part.radiator.is_some() && !grid.is_exposed(k) {
                self.warn(
                    s,
                    format!("parts[{k}]"),
                    format!(
                        "`{}` at ({}, {}) is boxed in and cannot vent",
                        pl.part, pl.x, pl.y
                    ),
                );
            }
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
