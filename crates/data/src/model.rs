//! Serde types for every content kind. See `docs/content-model.md` for the meaning of each field.
//!
//! Every entry starts with the shared header (`id`, `name`, `summary`, optional `description` and `lore`, `tags`).
//! Structs deny unknown fields so typos fail loudly. Numbers are `f32`, containers are ordered.

use serde::{Deserialize, Serialize};

use crate::kind::Kind;

/// Where an entry came from: pack id and the file path inside the pack (`/`-separated).
/// Filled in by the loader, never read from RON.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Source {
    pub pack: String,
    pub file: String,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.pack, self.file)
    }
}

/// Behaviour shared by every content entry (header fields, source, text fields).
pub trait Entry {
    const KIND: Kind;
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn summary(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn lore(&self) -> Option<&str>;
    fn tags(&self) -> &[String];
    fn source(&self) -> &Source;
    fn set_source(&mut self, source: Source);
    /// Every translatable text field as `(field, text)`; the text key is `<kind>.<id>.<field>`.
    fn text_fields(&self) -> Vec<(&'static str, &str)>;
    /// Mutable access to the same fields, for locale overrides.
    fn text_fields_mut(&mut self) -> Vec<(&'static str, &mut String)>;
}

macro_rules! entry {
    ($ty:ty, $kind:expr $(, $extra:ident)*) => {
        impl Entry for $ty {
            const KIND: Kind = $kind;
            fn id(&self) -> &str { &self.id }
            fn name(&self) -> &str { &self.name }
            fn summary(&self) -> &str { &self.summary }
            fn description(&self) -> Option<&str> { self.description.as_deref() }
            fn lore(&self) -> Option<&str> { self.lore.as_deref() }
            fn tags(&self) -> &[String] { &self.tags }
            fn source(&self) -> &Source { &self.source }
            fn set_source(&mut self, source: Source) { self.source = source; }
            fn text_fields(&self) -> Vec<(&'static str, &str)> {
                let mut v = vec![("name", self.name.as_str()), ("summary", self.summary.as_str())];
                $( v.push((stringify!($extra), self.$extra.as_str())); )*
                if let Some(d) = &self.description { v.push(("description", d.as_str())); }
                if let Some(l) = &self.lore { v.push(("lore", l.as_str())); }
                v
            }
            fn text_fields_mut(&mut self) -> Vec<(&'static str, &mut String)> {
                let mut v: Vec<(&'static str, &mut String)> = vec![("name", &mut self.name), ("summary", &mut self.summary)];
                $( v.push((stringify!($extra), &mut self.$extra)); )*
                if let Some(d) = &mut self.description { v.push(("description", d)); }
                if let Some(l) = &mut self.lore { v.push(("lore", l)); }
                v
            }
        }
    };
}

/// `pack.ron`: the manifest of a content pack. The base game is the pack `base`; mods are packs too.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    /// snake_case, unique across loaded packs.
    pub id: String,
    pub name: String,
    /// Free-form content version of the pack (e.g. "0.1.0").
    pub version: String,
    /// Version of the content schema the files are written against. See [`crate::SCHEMA_VERSION`].
    pub schema_version: u32,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    /// Packs that must be loaded before this one (by id).
    #[serde(default)]
    pub depends: Vec<String>,
}

/// Whether a bigger or smaller number of a stat is better (drives green/red diffs in the drydock).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Better {
    Higher,
    Lower,
    #[default]
    Neutral,
}

/// `stats.ron`: display information for one stat. Order in the file = display order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatDef {
    pub id: String,
    /// The label shown next to the value.
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Unit suffix, e.g. "t", "kN", "/s". Empty for unitless values. Translatable (`stat.<id>.unit`).
    #[serde(default)]
    pub unit: String,
    /// Maximum decimals shown; trailing zeros are trimmed.
    #[serde(default)]
    pub decimals: u8,
    #[serde(default)]
    pub better: Better,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(StatDef, Kind::Stat, unit);

/// `categories.ron`: part categories (palette tabs, wiki sections). Order in the file = display order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// System colour key (see [`crate::art_keys::SYSTEM_COLORS`]) for the icons of this category's parts.
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Category, Kind::Category);

/// `factions/*.ron`: who builds and flies ships.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Faction {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Hull glow key (see [`crate::art_keys::GLOWS`]): "player" (teal) or "enemy" (coral).
    pub glow: String,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Faction, Kind::Faction);

/// Armour properties of a material. Hull parts placed in this material take their mass, hit points and
/// build-point factor from here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmorSpec {
    /// Tile tone key (see [`crate::art_keys::ARMOR_TONES`]).
    pub tone: String,
    pub mass_per_cell: f32,
    pub hp_per_cell: f32,
    pub build_point_factor: f32,
}

/// `materials/*.ron`: materials. For now only armour materials (with an `armor` block); fabrication
/// resources will be materials without one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub armor: Option<ArmorSpec>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Material, Kind::Material);

/// Footprint shape. `Slope` is a right triangle over a `w × 1` footprint (see `geometry`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Shape {
    #[default]
    Rect,
    Slope,
}

/// Power: `output` feeds the piece's shared pool, `draw` is taken from it at full load.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerSpec {
    #[serde(default)]
    pub output: f32,
    #[serde(default)]
    pub draw: f32,
}

/// Heat added at full load and the level where the part shuts down (none = never).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeatSpec {
    #[serde(default)]
    pub output: f32,
    #[serde(default)]
    pub limit: Option<f32>,
}

/// Dumps heat through its straight sides that are open to space.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadiatorSpec {
    pub vent: f32,
}

/// Weapon families (settled 10 Oct 2026: all families).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum WeaponFamily {
    Energy,
    Kinetic,
    Missile,
    Disruption,
    Drone,
    Utility,
}

/// A fixed gun. Fires along the part's forward side (local −y, turned with the placement) and needs a
/// clear line of fire.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponSpec {
    pub family: WeaponFamily,
    /// Damage per second at full power, before bonuses (pulses and projectiles split it over their shots).
    pub damage: f32,
    /// Reach in metres (hit-scan length; projectiles fly `range / speed` seconds).
    pub range: f32,
    /// How the shot travels. Default `Beam`.
    #[serde(default)]
    pub delivery: Delivery,
}

/// How a weapon's shots travel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Delivery {
    /// A continuous hit-scan beam: damage per second while firing, burns the first block it touches.
    #[default]
    Beam,
    /// Discrete hit-scan pulses, `rate` per second; each deals `damage / rate`, burns the first block.
    Pulse { rate: f32 },
    /// A physical projectile, `rate` per second, flying at `speed` (m/s, plus the shooter's velocity).
    /// Each deals `damage / rate`, goes through up to `penetration` blocks (1 = stops at the first), and
    /// pushes the shooter back with `recoil` (kN·s per shot).
    Projectile {
        rate: f32,
        speed: f32,
        penetration: u32,
        #[serde(default)]
        recoil: f32,
    },
}

/// Pushes towards the part's forward side; needs open cells directly behind its back side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSpec {
    pub thrust: f32,
    /// Maximum thrust deflection in degrees to either side of the engine's axis (0 = fixed nozzle).
    /// The deflected thrust acts at the same point, so it adds torque without moving the engine.
    #[serde(default)]
    pub gimbal: f32,
}

/// Pure reaction torque about the centre of mass, in either direction, without thrust: a gyro, or the small
/// built-in turning help of a logic core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GyroSpec {
    /// Maximum torque (kN·m) at full power.
    pub torque: f32,
}

/// Reaction-control thrusters: one nozzle on every straight side of the part that is open to space (the
/// neighbouring cell is empty), so the part needs no orientation. Each nozzle pushes the ship away from its
/// side (the exhaust leaves through the side) at the side's centre. Nozzles appear and vanish as neighbours
/// are built or shot away; see `ShipGrid::open_sides`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RcsSpec {
    /// Thrust per nozzle (kN) at full power.
    pub thrust: f32,
}

/// Projects a shield bubble over the hull around it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShieldSpec {
    /// Bubble radius in metres from the part's centre.
    pub radius: f32,
    /// Damage the bubble can take before it collapses.
    pub capacity: f32,
    /// Capacity regained per second while the emitter is powered.
    pub regen: f32,
    /// Share of a shot's damage the bubble takes, per weapon family (missing = 0: passes straight through).
    #[serde(default)]
    pub absorb: std::collections::BTreeMap<WeaponFamily, f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoSpec {
    pub capacity: f32,
}

/// Hosts a copy of the ship-mind (or an automaton mind) and runs behaviour rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogicSpec {
    pub rule_slots: u32,
}

/// Which parts a bonus applies to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Selector {
    Part(String),
    Tag(String),
    Category(String),
}

/// How a bonus changes the target's stat, per qualifying neighbour.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    /// Adds this amount (in the stat's unit).
    Add(f32),
    /// Raises the stat by this many percent.
    Percent(f32),
}

/// Extra conditions for a bonus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Condition {
    /// The part granting the bonus has a straight side open to space (a radiator that vents).
    SourceVenting,
}

/// Adjacency bonus granted by the part that declares it to each linked neighbour matching `target`
/// (linked = touching through straight sides, the same rule as piece connectivity).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bonus {
    pub target: Selector,
    /// Stat id (from `stats.ron`) changed on the target.
    pub stat: String,
    pub effect: Effect,
    /// At most this many sources of this bonus count per target.
    #[serde(default)]
    pub max_stacks: Option<u32>,
    #[serde(default)]
    pub when: Vec<Condition>,
}

/// `parts/*.ron`: a ship part. Optional component blocks describe what it does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub category: String,
    /// Icon key = file stem in `art/icons` (e.g. "fission-reactor"). Hull parts use their material's icon.
    #[serde(default)]
    pub icon: Option<String>,
    /// System colour key overriding the category's colour (e.g. "enemy" for automaton parts).
    #[serde(default)]
    pub color: Option<String>,
    /// Footprint `(w, h)` in cells before rotation.
    pub size: (u32, u32),
    #[serde(default)]
    pub shape: Shape,
    pub build_points: f32,
    /// Takes an armour material per placement; mass, hit points and build-point factor come from it.
    #[serde(default)]
    pub hull: bool,
    /// Mass of a non-hull part.
    #[serde(default)]
    pub mass: Option<f32>,
    /// Hit points of a non-hull part.
    #[serde(default)]
    pub hp: Option<f32>,
    #[serde(default)]
    pub power: Option<PowerSpec>,
    #[serde(default)]
    pub heat: Option<HeatSpec>,
    #[serde(default)]
    pub radiator: Option<RadiatorSpec>,
    #[serde(default)]
    pub weapon: Option<WeaponSpec>,
    #[serde(default)]
    pub engine: Option<EngineSpec>,
    #[serde(default)]
    pub shield: Option<ShieldSpec>,
    #[serde(default)]
    pub cargo: Option<CargoSpec>,
    #[serde(default)]
    pub logic: Option<LogicSpec>,
    #[serde(default)]
    pub gyro: Option<GyroSpec>,
    #[serde(default)]
    pub rcs: Option<RcsSpec>,
    #[serde(default)]
    pub bonuses: Vec<Bonus>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Part, Kind::Part);

impl Part {
    pub fn footprint(&self) -> crate::geometry::Footprint {
        crate::geometry::Footprint {
            w: self.size.0,
            h: self.size.1,
            shape: self.shape,
        }
    }
    /// Area in cells (slopes count half their footprint).
    pub fn area(&self) -> f32 {
        self.footprint().area()
    }
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
    pub fn matches(&self, selector: &Selector) -> bool {
        match selector {
            Selector::Part(id) => &self.id == id,
            Selector::Tag(tag) => self.has_tag(tag),
            Selector::Category(c) => &self.category == c,
        }
    }
}

/// One part in a blueprint. Coordinates in cells; see `geometry` for the conventions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub part: String,
    /// Left column of the rotated footprint.
    pub x: i32,
    /// Top row of the rotated footprint.
    pub y: i32,
    /// Quarter turns clockwise (0..=3).
    #[serde(default)]
    pub rot: u8,
    /// Mirror left-right before rotating.
    #[serde(default)]
    pub flip: bool,
    /// Armour material; required for hull parts, not allowed on other parts.
    #[serde(default)]
    pub material: Option<String>,
}

/// `ships/*.ron`: a ship blueprint (one per file by convention).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blueprint {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub faction: String,
    /// Drydock frame `(w, h)` in cells; every placement must fit inside.
    pub size: (u32, u32),
    pub parts: Vec<Placement>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Blueprint, Kind::Ship);

/// Long-form wiki prose from `wiki/<kind>/<id>.md`. Never needed by the game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WikiPage {
    pub kind: Kind,
    pub id: String,
    pub markdown: String,
    pub source: Source,
}
