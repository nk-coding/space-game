//! Parts and materials: armour, salvage, power, heat, weapons, engines, plumes, explosions, gyros, RCS, shields,
//! cargo, logic, bonuses.

use super::*;

/// Armour properties of a material. Hull parts placed in this material take their mass, hit points and
/// build-point factor from here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmorSpec {
    /// Tile tone key (see [`crate::content::art_keys::ARMOR_TONES`]).
    pub tone: String,
    pub mass_per_cell: f32,
    pub hp_per_cell: f32,
    pub build_point_factor: f32,
    /// Share of an explosion's damage a hull part of this armour takes (GDR-0084; default 1: no protection).
    #[serde(default = "one")]
    pub blast_factor: f32,
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
    /// What one cell of hull area in this material gives when salvaged (armour materials only).
    #[serde(default)]
    pub salvage: Option<HullSalvageSpec>,
    /// Not in a new game: research unlocks it per player (GDR-0063). The free drydock hides it; the career's
    /// drydock shows it dimmed until it is researched (GDR-0065). Content ships may still use it.
    #[serde(default)]
    pub locked: bool,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Material, Kind::Material);

/// A gun's barrel as drawn (style 2a: a short metal barrel through the nose, a muzzle glow at its tip while the
/// line of fire is clear), in cells.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BarrelArt {
    pub width: f32,
    /// How far it reaches beyond the part's front edge.
    pub length: f32,
    /// The radius of the muzzle glow at its tip.
    pub muzzle: f32,
}

/// What a salvaged part gives when the haul is dismantled at home (GDR-0063). The career turns a haul into
/// research data and materials with it. The sim reads only two facts: a part without it is never collected, and
/// only a part whose `fragment` is not empty leaves a fragment when destroyed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalvageSpec {
    /// The research family (id) the data goes to.
    pub family: String,
    /// Research data one intact part gives.
    pub data: f32,
    /// Materials (id → amount) one intact part gives besides the data.
    #[serde(default)]
    pub materials: BTreeMap<String, u32>,
    /// Materials (id → amount) a destroyed part of this kind leaves as a fragment to scoop (GDR-0063: destroyed
    /// cores leave programmable matter). Empty: it leaves nothing.
    #[serde(default)]
    pub fragment: BTreeMap<String, u32>,
}

/// What one cell of hull area in an armour material gives when salvaged (slopes count half a cell): research data
/// for a family. Hull parts have no salvage block of their own, as they have no mass or hit points of their own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HullSalvageSpec {
    /// The research family (id) the data goes to.
    pub family: String,
    /// Research data per cell of hull area.
    pub data: f32,
}

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

pub(super) fn one() -> f32 {
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

/// A chain reaction (GDR-0019, GDR-0084): when the part is destroyed, every other part of its piece with a cell
/// within `radius` metres of the part's centre takes `damage` at once (its own ship's shields do not help).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplosionSpec {
    /// Damage dealt to each part in reach (hit points, once).
    pub damage: f32,
    /// Reach from the part's centre to the nearest point of a cell, m (1 = the eight cells around a 1×1 part).
    pub radius: f32,
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
    /// How a gun's barrel looks (guns whose art has one). Only the client reads it.
    #[serde(default)]
    pub barrel: Option<BarrelArt>,
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
    /// Destroyed, it explodes into its neighbours (GDR-0019, GDR-0084).
    #[serde(default)]
    pub explosion: Option<ExplosionSpec>,
    #[serde(default)]
    pub bonuses: Vec<Bonus>,
    /// What the part gives when salvaged (non-hull parts; hull parts take it from their material per cell).
    #[serde(default)]
    pub salvage: Option<SalvageSpec>,
    /// Not in a new game: research unlocks it per player (GDR-0063). The free drydock hides it; the career's
    /// drydock shows it dimmed until it is researched (GDR-0065). Content ships may still use it.
    #[serde(default)]
    pub locked: bool,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Part, Kind::Part);

impl Part {
    pub fn footprint(&self) -> crate::design::geometry::Footprint {
        crate::design::geometry::Footprint {
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
