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
    /// Default behavior (id) of this faction's ships, unless a ship names its own.
    #[serde(default)]
    pub behavior: Option<String>,
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
    /// Not available to the player until unlocked (salvage, research): the drydock hides it. Content ships may
    /// still use it.
    #[serde(default)]
    pub locked: bool,
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

/// Power: `output` feeds the piece's shared pool, `draw` is taken from it at full load. Parts that draw also
/// carry their allocation defaults: `tier`, `weight` and `min_level` (see `docs/sim.md` "Power").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerSpec {
    #[serde(default)]
    pub output: f32,
    #[serde(default)]
    pub draw: f32,
    /// Default allocation tier of a consumer; a blueprint placement may override it.
    #[serde(default)]
    pub tier: PowerTier,
    /// Default weight within its tier (share of a tier that cannot be fully served); a placement may override
    /// it. Between 0 (exclusive) and [`MAX_POWER_WEIGHT`].
    #[serde(default = "one")]
    pub weight: f32,
    /// Below this share of its demand (0..1) the part switches off instead of running partially.
    #[serde(default)]
    pub min_level: f32,
}

impl Default for PowerSpec {
    fn default() -> PowerSpec {
        PowerSpec {
            output: 0.0,
            draw: 0.0,
            tier: PowerTier::default(),
            weight: 1.0,
            min_level: 0.0,
        }
    }
}

fn one() -> f32 {
    1.0
}

/// Largest allowed power weight (content defaults, placement overrides, runtime settings).
pub const MAX_POWER_WEIGHT: f32 = 100.0;

/// Power allocation tier of a consumer. When a piece's supply falls short, tiers are served in this order;
/// inside the first tier that cannot be fully served the rest is shared by weight, and the tiers below get
/// nothing. `Off` never draws.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum PowerTier {
    Critical,
    High,
    #[default]
    Normal,
    Low,
    Off,
}

impl PowerTier {
    /// Every tier in serving order.
    pub const ALL: [PowerTier; 5] = [
        PowerTier::Critical,
        PowerTier::High,
        PowerTier::Normal,
        PowerTier::Low,
        PowerTier::Off,
    ];
}

/// A placement's override of its part's power allocation defaults (the player's drydock setting).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerOverride {
    #[serde(default)]
    pub tier: Option<PowerTier>,
    #[serde(default)]
    pub weight: Option<f32>,
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
#[serde(deny_unknown_fields)]
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

/// Pushes towards the part's forward side. It may sit anywhere (GDR-0039): its exhaust plume leaves through the
/// back side and damages and heats the first part in its path, the ship's own included.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSpec {
    pub thrust: f32,
    /// Rated speed (m/s): while the engine pushes with the ship's motion, its thrust fades out as the ship's whole
    /// speed approaches it (full thrust up to 80%, none at 100%, GDR-0041, GDR-0055); braking or sideways it keeps
    /// full thrust. So the fastest engines set the ship's top speed.
    pub rated_speed: f32,
    /// Maximum thrust deflection in degrees to either side of the engine's axis (0 = fixed nozzle).
    /// The deflected thrust acts at the same point, so it adds torque without moving the engine.
    #[serde(default)]
    pub gimbal: f32,
    /// The exhaust plume (required).
    pub plume: PlumeSpec,
}

/// An engine's exhaust plume (GDR-0039, GDR-0049): it reaches `length` metres straight back from the engine's
/// back side and burns the first part in its path (own ship or not, except other vessels of the same owner),
/// with damage and heat scaled by the engine's thrust output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlumeSpec {
    /// Reach behind the engine's back side, m (0 = no plume).
    pub length: f32,
    /// Damage per second at full thrust, shared over the plume's width (one lane per exhaust cell).
    pub damage: f32,
    /// Heat per second at full thrust (same units as `heat.output`), shared the same way.
    pub heat: f32,
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
    /// Rated speed (m/s) of every nozzle: a nozzle pushing with the ship's motion fades out as the ship's whole
    /// speed approaches it, as an engine does (GDR-0055). Two nozzles of opposite sides fire as a turning couple
    /// that keeps full thrust at any speed.
    pub rated_speed: f32,
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
    /// How much control cost of parts the core can run (GDR-0038, "N"): a vessel's control capacity is the
    /// sum over its running cores. Parts beyond it go idle by power priority (GDR-0044).
    pub control_capacity: u32,
}

/// Highest control cost a part may have (validation).
pub const MAX_CONTROL_COST: u32 = 100;

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
    /// Icon key: `icons/<key>.svg` in a loaded pack (e.g. "fission-reactor"). Hull parts use their material's
    /// icon.
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
    /// What the part costs against its vessel's control capacity (GDR-0044). Default from the part's kind
    /// ([`Part::control_cost`]): 0 for hull parts, logic cores and passive parts, 1 for parts that draw or
    /// make power.
    #[serde(default)]
    pub control_cost: Option<u32>,
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
    /// Not available to the player until unlocked (salvage, research): the drydock hides it. Content ships may
    /// still use it.
    #[serde(default)]
    pub locked: bool,
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
    /// Build points when placed in `armor`: hull parts scale by its `build_point_factor`, other parts ignore it.
    pub fn build_points_in(&self, armor: Option<&ArmorSpec>) -> f32 {
        match armor {
            Some(a) if self.hull => self.build_points * a.build_point_factor,
            _ => self.build_points,
        }
    }
    /// Mass when placed in `armor`: hull parts `mass_per_cell × area` (`None` without armour), other parts their
    /// `mass`.
    pub fn mass_in(&self, armor: Option<&ArmorSpec>) -> Option<f32> {
        if self.hull {
            armor.map(|a| a.mass_per_cell * self.area())
        } else {
            self.mass
        }
    }
    /// Hit points when placed in `armor`: hull parts `hp_per_cell × area` (`None` without armour), other parts
    /// their `hp`.
    pub fn hp_in(&self, armor: Option<&ArmorSpec>) -> Option<f32> {
        if self.hull {
            armor.map(|a| a.hp_per_cell * self.area())
        } else {
            self.hp
        }
    }
    /// Does the part draw power (the parts that take a power tier, weight and placement override)?
    pub fn draws_power(&self) -> bool {
        self.power.as_ref().is_some_and(|p| p.draw > 0.0)
    }
    /// Does the part draw or make power (the parts that can go idle)?
    pub fn has_power_role(&self) -> bool {
        self.power
            .as_ref()
            .is_some_and(|p| p.draw > 0.0 || p.output > 0.0)
    }
    /// Control cost against the vessel's control capacity: the `control_cost` field, else the default by
    /// kind. Hull parts and logic cores cost 0 (a core runs parts, it is not run), parts that draw or make
    /// power cost 1, passive parts (radiators, capacitors, cargo) 0: idling has nothing to switch off on them.
    pub fn control_cost(&self) -> u32 {
        match self.control_cost {
            Some(c) => c,
            None if self.hull || self.logic.is_some() => 0,
            None => self.has_power_role() as u32,
        }
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
    /// Left column of the rotated footprint (reading refuses values beyond ±[`crate::limits::MAX_COORD`]).
    #[serde(deserialize_with = "crate::limits::coord")]
    pub x: i32,
    /// Top row of the rotated footprint (likewise within ±[`crate::limits::MAX_COORD`]).
    #[serde(deserialize_with = "crate::limits::coord")]
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
    /// Power tier and weight of this placement, overriding the part's defaults (parts that draw power only).
    #[serde(default)]
    pub power: Option<PowerOverride>,
}

/// `frames.ron` / `frames/*.ron`: a drydock frame (GDR-0006), the grid a design is built on. Bigger frames come
/// with drydock expansions. Order in the files = order in the drydock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Grid size `(w, h)` in cells, each side 1 to [`crate::validate::MAX_FRAME_SIZE`].
    pub size: (u32, u32),
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Frame, Kind::Frame);

/// An icon of a pack: `icons/<key>.svg`, kept as SVG text (rasterised by the client at runtime, shown as is
/// by the wiki). Later packs replace icons with the same key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Icon {
    pub key: String,
    pub svg: String,
    pub source: Source,
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
    /// Behavior (id) this ship flies under when it is not piloted; default: its faction's behavior.
    #[serde(default)]
    pub behavior: Option<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Blueprint, Kind::Ship);

/// What a behavior node is in a rule (the editor colours them: conditions violet, actions teal).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NodeRole {
    /// Before the arrow: a flag (`shield_down`) or a value that is compared (`hull < 30%`).
    Condition,
    /// After the arrow: the maneuver the ship flies while the rule is active.
    Action,
    /// On the `target` line: picks the ship the rules are about.
    Target,
}

/// The kind of number a value condition yields or a parameter takes. Fixes the units it may be written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Quantity {
    /// Metres (`45m` or `45`), or a share of the ship's weapon reach (`60%`).
    Distance,
    /// A percentage, always written with `%` (`30%`).
    Percent,
    /// Seconds (`5s` or `5`).
    Seconds,
    /// A whole number without unit.
    Count,
}

impl Quantity {
    pub fn describe(self) -> &'static str {
        match self {
            Quantity::Distance => "a distance",
            Quantity::Percent => "a percentage",
            Quantity::Seconds => "a time",
            Quantity::Count => "a count",
        }
    }

    /// A sample value for messages.
    pub fn example(self) -> &'static str {
        match self {
            Quantity::Distance => "60m",
            Quantity::Percent => "30%",
            Quantity::Seconds => "5s",
            Quantity::Count => "2",
        }
    }

    pub fn unit_hint(self) -> &'static str {
        match self {
            Quantity::Distance => "write metres (45m) or a share of the weapon reach (60%)",
            Quantity::Percent => "write a percentage (30%)",
            Quantity::Seconds => "write seconds (5s)",
            Quantity::Count => "write a whole number without unit",
        }
    }

    /// Unit suffix of the number as written for ranges (`m`, `%`, `s`, none).
    pub fn unit_suffix(self) -> &'static str {
        match self {
            Quantity::Distance => "m",
            Quantity::Percent => "%",
            Quantity::Seconds => "s",
            Quantity::Count => "",
        }
    }
}

/// One option of a choice parameter (written as its id: `orbit(45m, cw)`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOption {
    /// snake_case; what the player types.
    pub id: String,
    /// Label in the editor and wiki.
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
}

/// The type of a node parameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParamType {
    Distance,
    Percent,
    Seconds,
    Count,
    /// One of the listed words.
    Choice(Vec<ChoiceOption>),
}

impl ParamType {
    /// The quantity of a numeric parameter (`None` for choices).
    pub fn quantity(&self) -> Option<Quantity> {
        match self {
            ParamType::Distance => Some(Quantity::Distance),
            ParamType::Percent => Some(Quantity::Percent),
            ParamType::Seconds => Some(Quantity::Seconds),
            ParamType::Count => Some(Quantity::Count),
            ParamType::Choice(_) => None,
        }
    }

    pub fn unit_suffix(&self) -> &'static str {
        self.quantity().map_or("", Quantity::unit_suffix)
    }
}

/// A positional parameter of a behavior node (`orbit(radius, dir)`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeParam {
    /// snake_case, unique within the node; also the variable name in a Rhai script.
    pub id: String,
    /// Label in the editor and wiki (translatable: `behavior_node.<node>.params.<id>.name`).
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(rename = "type")]
    pub ty: ParamType,
    /// Value used when a rule leaves it out, written as in a rule (`67%`, `45m`, `cw`). Parameters without a
    /// default must come first and are required.
    #[serde(default)]
    pub default: Option<String>,
    /// Range of the number as written in the parameter's unit (metres, percent, seconds, count).
    /// Reach-relative distances (`%`) are checked against 0–1000% instead.
    #[serde(default)]
    pub min: Option<f32>,
    #[serde(default)]
    pub max: Option<f32>,
}

/// `behavior_nodes/*.ron`: one word of the behavior language (a condition, an action or a target
/// selector). The editor's palette and the wiki list exactly these; the sim implements every built-in id,
/// and a node with a `script` is a custom condition written in Rhai.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorNode {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub role: NodeRole,
    /// Conditions only: the quantity a value condition yields (`hull` → `Percent`); it must be compared in a
    /// rule. `None` = a flag, used bare (`shield_down`).
    #[serde(default)]
    pub value: Option<Quantity>,
    #[serde(default)]
    pub params: Vec<NodeParam>,
    /// Rhai source of a custom condition (tagged "Rhai" in the editor): an expression or statements that
    /// end in `true` or `false`. `None` = built into the sim.
    #[serde(default)]
    pub script: Option<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(BehaviorNode, Kind::BehaviorNode);

impl BehaviorNode {
    /// Text keys and texts of the parameters and their options (`params.<id>.name`, `.summary`,
    /// `params.<id>.options.<option>.name`, `.summary`), relative to `behavior_node.<node>.`.
    pub fn param_texts(&self) -> Vec<(String, &str)> {
        let mut v = Vec::new();
        for p in &self.params {
            v.push((format!("params.{}.name", p.id), p.name.as_str()));
            if let Some(s) = &p.summary {
                v.push((format!("params.{}.summary", p.id), s.as_str()));
            }
            if let ParamType::Choice(opts) = &p.ty {
                for o in opts {
                    v.push((
                        format!("params.{}.options.{}.name", p.id, o.id),
                        o.name.as_str(),
                    ));
                    if let Some(s) = &o.summary {
                        v.push((
                            format!("params.{}.options.{}.summary", p.id, o.id),
                            s.as_str(),
                        ));
                    }
                }
            }
        }
        v
    }

    /// Mutable access to the same texts, for locale overrides.
    pub fn param_texts_mut(&mut self) -> Vec<(String, &mut String)> {
        let mut v = Vec::new();
        for p in &mut self.params {
            v.push((format!("params.{}.name", p.id), &mut p.name));
            if let Some(s) = &mut p.summary {
                v.push((format!("params.{}.summary", p.id), s));
            }
            if let ParamType::Choice(opts) = &mut p.ty {
                for o in opts {
                    v.push((
                        format!("params.{}.options.{}.name", p.id, o.id),
                        &mut o.name,
                    ));
                    if let Some(s) = &mut o.summary {
                        v.push((format!("params.{}.options.{}.summary", p.id, o.id), s));
                    }
                }
            }
        }
        v
    }
}

/// `behaviors/*.ron`: a behavior program. `target` and `rules` are lines of the text format
/// (`docs/behavior-format.md`); the text view of the editor is exactly `target <target>` followed by the
/// rules, one per line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Behavior {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The target selector (`"nearest"`, `"weakest(engines)"`); default `nearest`.
    #[serde(default)]
    pub target: Option<String>,
    /// The rules in priority order, one line each (`"when hull < 30% -> flee"`).
    pub rules: Vec<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Behavior, Kind::Behavior);

impl Behavior {
    /// Parses `target` and `rules` (syntax only; see [`crate::behavior::check_program`]).
    pub fn program(&self) -> Result<crate::behavior::Program, Vec<crate::behavior::LineError>> {
        crate::behavior::Program::from_parts(self.target.as_deref(), &self.rules)
    }
}

/// Long-form wiki prose from `wiki/<kind>/<id>.md`. Never needed by the game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WikiPage {
    pub kind: Kind,
    pub id: String,
    pub markdown: String,
    pub source: Source,
}
