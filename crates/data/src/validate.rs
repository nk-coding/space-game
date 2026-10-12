//! Validation of merged content. Every problem names pack, file, kind, id and field where possible.

use std::collections::{BTreeMap, BTreeSet};

use crate::art_keys::{ARMOR_TONES, GLOWS, SYSTEM_COLORS, is_icon_key};
use crate::kind::Kind;
use crate::load::{Content, SCHEMA_VERSION};
use crate::model::*;
use crate::problem::Problem;
use crate::stats::BUILTIN_STATS;
use crate::text::parse_links;

/// Validates merged content. Returns errors and warnings (warnings: ships in several pieces, blocked guns
/// or engines, unused icons).
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
    v.entries(c.ships.as_slice(), |v, e| v.ship(e));
    v.entries(c.behavior_nodes.as_slice(), |v, e| v.behavior_node(e));
    v.entries(c.behaviors.as_slice(), |v, e| v.behavior(e));
    v.icons();
    v.wiki();
    v.out
}

struct Validator<'a> {
    c: &'a Content,
    out: Vec<Problem>,
}

/// Largest allowed engine gimbal, in degrees.
pub const MAX_GIMBAL: f32 = 45.0;

pub(crate) fn is_snake_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl<'a> Validator<'a> {
    fn err<T: Entry>(&mut self, e: &T, field: impl Into<String>, msg: impl Into<String>) {
        self.out.push(
            Problem::error(msg)
                .at(e.source())
                .entry(T::KIND, e.id())
                .field(field),
        );
    }

    fn warn<T: Entry>(&mut self, e: &T, field: impl Into<String>, msg: impl Into<String>) {
        self.out.push(
            Problem::warning(msg)
                .at(e.source())
                .entry(T::KIND, e.id())
                .field(field),
        );
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
            check(self, e);
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
        for id in BUILTIN_STATS {
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
            if part.engine.is_some() && !grid.exhaust_clear(k) {
                self.warn(
                    s,
                    format!("parts[{k}]"),
                    format!(
                        "`{}` at ({}, {}) has its exhaust blocked",
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
