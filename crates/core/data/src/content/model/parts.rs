//! Parts and materials: armour, salvage, power, heat, weapons, engines, plumes, explosions, gyros, RCS, shields,
//! cargo, logic, sensors, scanners, bonuses.

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
    /// Living armour (GDR-0099 A4, GDR-0102): hit points a hull part of this armour regains per second per cell
    /// while it is on a vessel's controlling piece, up to its full hit points. Default 0: it never heals.
    #[serde(default)]
    pub heal: f32,
    /// Refractive armour (GDR-0126, the Lattice's crystal): the share of an energy hit-scan shot's damage (beam,
    /// pulse) that a hull part of this armour turns away along the mirrored line, as a shot of nobody's that hits
    /// whatever it meets; the part takes the rest. Default 0: it turns nothing.
    #[serde(default)]
    pub refract: f32,
}

/// `materials/*.ron`: materials: armour materials (with an `armor` block) and fabrication resources (without one,
/// GDR-0031, GDR-0108): what parts' recipes take and dismantling gives back.
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

/// What a salvaged part gives when the haul is dismantled at home (GDR-0063): research data. Its materials come
/// from its [`Recipe`] (a share of them, GDR-0108). The sim reads only two facts: a part without it is never
/// collected, and only a part whose `fragment` is not empty leaves a fragment when destroyed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalvageSpec {
    /// The research family (id) the data goes to.
    pub family: String,
    /// Research data one dismantled part gives (intact or broken).
    pub data: f32,
    /// Materials (id → amount) a destroyed part of this kind leaves as a fragment to scoop (GDR-0063: destroyed
    /// cores leave programmable matter). Empty: it leaves nothing.
    #[serde(default)]
    pub fragment: BTreeMap<String, u32>,
}

/// What the workshop fabricates a part from (GDR-0031, GDR-0108): materials, taken once, and the workshop
/// upgrade (a facility `within` the workshop) it needs, if any. Dismantling gives a share of the materials back.
/// Hull parts have none: their plating is not counted yet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// Materials (id → amount) one part takes.
    pub materials: BTreeMap<String, u32>,
    /// The facility (id, an upgrade of the workshop) that must be open to fabricate it.
    #[serde(default)]
    pub needs: Option<String>,
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
    /// The draw comes in short bursts, not in steady use (a scanner draws only while a scan charges): when, in a
    /// few words ("while scanning"). The design report's full use leaves it out, and the drydock shows it beside
    /// the power figure ("+3 WHILE SCANNING"). Only scanners and blink drives (GDR-0127): the sim draws their power
    /// only while they charge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intermittent: Option<String>,
}

impl Default for PowerSpec {
    fn default() -> PowerSpec {
        PowerSpec {
            output: 0.0,
            draw: 0.0,
            tier: PowerTier::default(),
            weight: 1.0,
            min_level: 0.0,
            intermittent: None,
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
    /// Ore cut from asteroid rock per second at full power (cargo units; GDR-0114): a hit on a rock with ore left
    /// adds to it, and every whole unit goes into the shooter's hold as ore of the rock's material. 0 (default):
    /// the shot only stops on rock.
    #[serde(default)]
    pub cut: f32,
    /// Point defence (GDR-0115): the gun aims itself within `arc` of its facing and fires on its own whenever its
    /// piece is active, at the nearest hostile missile in reach, else the nearest hostile vessel of at most
    /// `ship_mass`. Pulse guns only.
    #[serde(default)]
    pub defence: Option<DefenceSpec>,
    /// Static per second at full power (disruption, GDR-0018, GDR-0125): what of the shot's damage passes the
    /// shields carries static in this proportion into the struck vessel's logic cores (twice on a hit on a core
    /// itself); enough static blinds its mind for a while. Hit-scan guns only (beam, pulse). 0 (default): none.
    #[serde(default)]
    pub disrupt: f32,
    /// A lance (GDR-0127, the Phase walkers): the shot goes on through the struck piece along its line, and every
    /// part on it takes what is left of the shot's strength, which drops with each cell it crosses. Pulse guns
    /// only.
    #[serde(default)]
    pub lance: Option<LanceSpec>,
}

/// How a lance's strength drops through a hull (GDR-0127): after each cell of a machine part it keeps `keep` of
/// what it had, after each cell of a hull part (armour) `armor`; so thick armour still helps a little.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanceSpec {
    pub keep: f32,
    pub armor: f32,
}

/// A point-defence gun's targeting (GDR-0115).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefenceSpec {
    /// Degrees to either side of the gun's facing it can aim (up to 180: all round).
    pub arc: f32,
    /// With no missile in reach it shoots at hostile vessels of at most this mass (t); 0: missiles only.
    #[serde(default)]
    pub ship_mass: f32,
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
    /// A homing missile (GDR-0115): `rate` launches per second, each `salvo` missiles fanned out a little. A missile
    /// leaves with the shooter's velocity, accelerates at `accel` (m/s²) to `speed` (m/s) and turns its course at
    /// up to `turn` (degrees per second) towards the nearest hostile vessel ahead of it; after `life` seconds it is
    /// gone. It has `hp` hit points (point defence shoots it down) and its warhead blasts `damage / rate / salvo`
    /// within `radius` metres of where it strikes, by the chain-reaction rule (GDR-0085).
    Missile {
        rate: f32,
        #[serde(default = "one_u32")]
        salvo: u32,
        speed: f32,
        accel: f32,
        turn: f32,
        life: f32,
        hp: f32,
        radius: f32,
    },
}

fn one_u32() -> u32 {
    1
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

/// A heart (GDR-0099 A1, GDR-0102): while it works (alive, not overheated, not idle) on a vessel's controlling
/// piece, the ship's lost parts slowly re-form from their living neighbours, one every `interval` seconds, nearest
/// to the heart first. Hearts themselves never re-form: killing the heart stops it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartSpec {
    /// Seconds between two re-formed parts.
    pub interval: f32,
    /// Share of its full hit points a re-formed part starts with (above 0, at most 1).
    pub hp: f32,
}

/// A prow built for ramming (GDR-0099 A2, GDR-0102): when the part is the touching part of a ram (GDR-0047), the
/// other side takes `dealt` times the ram damage and its own side `taken` times it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RamSpec {
    #[serde(default = "one")]
    pub dealt: f32,
    #[serde(default = "one")]
    pub taken: f32,
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

/// What a vessel sees on its own (GDR-0017; galaxy plan E3): its behaviors pick targets only within the longest
/// sensor range of its working cores, and a voyage's sites show once one of the player's vessels has one within
/// it (a trail's place only within `VoyageTuning::signal_reveal`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorSpec {
    /// Metres.
    pub range: f32,
}

/// A flare launcher (GDR-0115; GDR-0018's other counter to missiles): asked (the FLARE key, the `flare` behavior
/// action, the `auto_flare` assist), it releases `decoys` hot decoys that leave at `speed` m/s along its facing and
/// burn for `life` seconds; enemy homing missiles within `lure` metres of one turn onto it and are spent on it. Then
/// it cools down for `cooldown` seconds; it has `charges` releases per fight.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountermeasureSpec {
    pub charges: u32,
    pub cooldown: f32,
    pub decoys: u32,
    pub speed: f32,
    pub life: f32,
    pub lure: f32,
}

/// A blink drive (GDR-0127, the Phase walkers): asked (the BLINK key, the `blink` behavior action), it charges for
/// `charge` seconds at full power (drawing its power only then), then jumps its vessel's controlling piece up to
/// `range` metres, keeping its speed and heading; then it cools down for `cooldown` seconds. A drive carries
/// `mass` tonnes: a blink takes as many ready drives as the piece's mass needs, and all of them cool down.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlinkSpec {
    pub range: f32,
    pub charge: f32,
    pub cooldown: f32,
    pub mass: f32,
}

/// A tractor beam (GDR-0131; GDR-0018's utility family, GDR-0025's loose debris): while its vessel runs and its hold
/// has room, it pulls up to `beams` pieces of salvage within `range` metres (loose items, wreck pieces, mined ore;
/// never a vessel) towards the ship, nearest first, with at most `force` kN each (a heavy wreck piece comes slower),
/// closing at most `speed` m/s relative to the ship and easing off as it arrives, so the hull collects it (GDR-0063).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TractorSpec {
    /// Metres from the ship's centre of mass to the salvage (a piece's bounding circle).
    pub range: f32,
    /// kN on each pulled piece (loose items weigh `sim::combat::tractor::tuning::LOOSE_MASS` t a cargo unit).
    pub force: f32,
    /// m/s: the fastest it closes a pulled piece on the ship.
    pub speed: f32,
    /// How many pieces it holds at once.
    pub beams: u32,
}

/// A scanner (GDR-0080, GDR-0093): a scan (SCAN in flight, the `scan` behavior action) charges for `charge` seconds,
/// drawing the part's power only then, and sweeps: on a voyage every site within `range` shows, every trail place
/// beyond it gets a bearing, and sleeping sites within `wake` wake up. Then it cools down for `cooldown` seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScannerSpec {
    /// Metres from the ship.
    pub range: f32,
    /// Seconds at full power.
    pub charge: f32,
    /// Seconds after a sweep before the next scan may start.
    pub cooldown: f32,
    /// The sweep wakes sleeping sites within this many metres of the ship.
    pub wake: f32,
}

/// A hangar (GDR-0018 "drones", GDR-0117): it holds `capacity` drones, small vessels of the ship `drone`, and launches
/// them one by one while its vessel has a target; each drone it has lost is rebuilt in `rebuild` seconds at full
/// power. Its drones fight what the carrier aims at and turn back when they are more than `leash` metres from it;
/// when the hangar is destroyed or leaves its vessel's controlling piece, they die with it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HangarSpec {
    /// The drone's ship (id); a drone carries no hangar of its own.
    pub drone: String,
    /// Drones it holds, out and in the bay together.
    pub capacity: u32,
    /// Seconds to rebuild one drone at full power.
    pub rebuild: f32,
    /// Metres from the carrier its drones may range.
    pub leash: f32,
}

/// Most drones a hangar may hold (validation).
pub const MAX_HANGAR: u32 = 16;

/// A prism (GDR-0126, the Lattice's relay): it takes the beam of a gun the `target` picks when it is the first part
/// in that gun's line of fire (or a prism's before it) and the beam enters any side but its front, and sends it on
/// from its front side. Each prism in the chain raises the gun's reach by `range` and its damage by `damage`
/// (shares of the gun's own, summed over the chain); at most [`crate::design::geometry::MAX_PRISMS`] count.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrismSpec {
    /// Which guns' beams it takes (in the base game the parts tagged `laser`).
    pub target: Selector,
    pub range: f32,
    pub damage: f32,
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
    pub sensor: Option<SensorSpec>,
    #[serde(default)]
    pub scanner: Option<ScannerSpec>,
    /// Releases decoys that lure missiles (flares, GDR-0115).
    #[serde(default)]
    pub countermeasure: Option<CountermeasureSpec>,
    /// Jumps its vessel a short way (the Phase walkers' blink, GDR-0127).
    #[serde(default)]
    pub blink: Option<BlinkSpec>,
    /// Pulls salvage towards its vessel (GDR-0131).
    #[serde(default)]
    pub tractor: Option<TractorSpec>,
    #[serde(default)]
    pub gyro: Option<GyroSpec>,
    #[serde(default)]
    pub rcs: Option<RcsSpec>,
    /// Launches drones (GDR-0117).
    #[serde(default)]
    pub hangar: Option<HangarSpec>,
    /// Relays a laser's beam (GDR-0126).
    #[serde(default)]
    pub prism: Option<PrismSpec>,
    /// Destroyed, it explodes into its neighbours (GDR-0019, GDR-0084).
    #[serde(default)]
    pub explosion: Option<ExplosionSpec>,
    /// Re-forms the ship's lost parts while it works (GDR-0099 A1).
    #[serde(default)]
    pub heart: Option<HeartSpec>,
    /// Living tissue (non-hull parts; hull parts take it from their armour's `heal`): hit points regained per
    /// second while the part is on a vessel's controlling piece, up to its full hit points (GDR-0099 A4).
    #[serde(default)]
    pub heal: Option<f32>,
    /// Built for ramming (GDR-0099 A2).
    #[serde(default)]
    pub ram: Option<RamSpec>,
    #[serde(default)]
    pub bonuses: Vec<Bonus>,
    /// What the part gives when salvaged (non-hull parts; hull parts take it from their material per cell).
    #[serde(default)]
    pub salvage: Option<SalvageSpec>,
    /// What the workshop fabricates it from (non-hull parts, GDR-0108). None: it cannot be fabricated.
    #[serde(default)]
    pub recipe: Option<Recipe>,
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
    /// Hit points regained per second when placed in `armor` (GDR-0099 A4): hull parts `heal × area` of a healing
    /// armour, other parts their `heal`; `None` for a part that never heals.
    pub fn heal_in(&self, armor: Option<&ArmorSpec>) -> Option<f32> {
        if self.hull {
            armor.filter(|a| a.heal > 0.0).map(|a| a.heal * self.area())
        } else {
            self.heal
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
