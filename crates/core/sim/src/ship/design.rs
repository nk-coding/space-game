//! A blueprint compiled for the simulation: every content lookup, string compare and geometry query happens
//! once here, so the per-tick code only touches flat numbers. One [`ShipDesign`] is shared (`Arc`) by every
//! ship built from the same blueprint.
//!
//! Everything in here is **static** for the intact blueprint. Two facts make that enough for breakage:
//! whether two present parts are linked depends only on those two parts' shapes, and destroyed parts only
//! remove links. So "the static neighbour list filtered by alive parts" is always the current connectivity.

use std::collections::BTreeMap;

use data::{
    Blueprint, Content, Delivery, Dir, Effect, PlacedShape, Selector, ShipGrid, Upgrades, WeaponFamily,
};

/// Number of weapon families.
pub const FAMILIES: usize = 6;

/// Stable index of a weapon family (for per-family tables).
pub fn family_index(f: WeaponFamily) -> usize {
    match f {
        WeaponFamily::Energy => 0,
        WeaponFamily::Kinetic => 1,
        WeaponFamily::Missile => 2,
        WeaponFamily::Disruption => 3,
        WeaponFamily::Drone => 4,
        WeaponFamily::Utility => 5,
    }
}

use crate::ship::mass::polygon_mass;

/// Index of a part in its blueprint (`Blueprint::parts`), also the index used by `data::ShipGrid`.
pub type PartIdx = u32;

/// Marker for "no slot".
pub const NO_SLOT: u32 = u32::MAX;

/// What drives a power consumer's demand each tick (who gets power first is its tier and weight, see
/// [`crate::ship::power`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConsumerKind {
    /// Logic cores: always on while the piece is active.
    Logic,
    /// Shield emitters: the shields switch.
    Shield,
    /// Everything that moves the ship (engines, RCS pods, gyros): how hard the actuators are driven.
    Engine,
    /// Guns: the trigger.
    Weapon,
    /// Scanners: while a scan charges on it (`Ship::start_scan`).
    Scanner,
    /// Blink drives: while a blink charges on it (`flight/blink.rs`, GDR-0127).
    Blink,
    /// Anything else that draws power: always on.
    Other,
}

/// Stats the simulation reads and adjacency bonuses may change. Ids match `stats.ron`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stat {
    PowerOutput,
    PowerDraw,
    HeatOutput,
    HeatLimit,
    HeatVent,
    Damage,
    FireRate,
    Thrust,
    ShieldRadius,
    /// Pure reaction torque (gyros, logic cores), kN·m.
    Torque,
    /// Thrust per RCS nozzle, kN.
    RcsThrust,
    /// Engine gimbal, degrees to either side.
    Gimbal,
    /// Weapon reach, m.
    Range,
    /// Shield bubble capacity.
    ShieldCapacity,
    /// Shield capacity regained per second.
    ShieldRegen,
    /// Engine rated speed (m/s): its thrust fades out towards it along its push direction.
    RatedSpeed,
    /// RCS nozzle rated speed (m/s).
    RcsRatedSpeed,
    /// Engine plume reach behind the engine (m).
    PlumeLength,
    /// Engine plume damage per second at full thrust.
    PlumeDamage,
    /// Engine plume heat per second at full thrust.
    PlumeHeat,
}

impl Stat {
    pub const COUNT: usize = 20;

    pub fn from_id(id: &str) -> Option<Stat> {
        Some(match id {
            "power_output" => Stat::PowerOutput,
            "power_draw" => Stat::PowerDraw,
            "heat_output" => Stat::HeatOutput,
            "heat_limit" => Stat::HeatLimit,
            "heat_vent" => Stat::HeatVent,
            "damage" => Stat::Damage,
            "fire_rate" => Stat::FireRate,
            "thrust" => Stat::Thrust,
            "shield_radius" => Stat::ShieldRadius,
            "torque" => Stat::Torque,
            "rcs_thrust" => Stat::RcsThrust,
            "gimbal" => Stat::Gimbal,
            "range" => Stat::Range,
            "shield_capacity" => Stat::ShieldCapacity,
            "shield_regen" => Stat::ShieldRegen,
            "rated_speed" => Stat::RatedSpeed,
            "rcs_rated_speed" => Stat::RcsRatedSpeed,
            "plume_length" => Stat::PlumeLength,
            "plume_damage" => Stat::PlumeDamage,
            "plume_heat" => Stat::PlumeHeat,
            _ => return None,
        })
    }
}

/// Effective values of the stats in [`Stat`] for one placed part (base values, or after bonuses).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartStats {
    pub v: [f32; Stat::COUNT],
}

impl PartStats {
    #[inline]
    pub fn get(&self, s: Stat) -> f32 {
        self.v[s as usize]
    }
    #[inline]
    pub fn set(&mut self, s: Stat, x: f32) {
        self.v[s as usize] = x;
    }
}

/// Everything the sim needs about one placed part, resolved from content.
#[derive(Clone, Debug)]
pub struct PartSpec {
    /// Content part id (for events, debugging and the client).
    pub id: String,
    pub shape: PlacedShape,
    pub hull: bool,
    pub slope: bool,
    /// Area in cells (= m²); slopes count half their footprint.
    pub area: f32,
    pub mass: f32,
    pub hp: f32,
    /// Centre of mass of the part in the blueprint frame.
    pub centroid: [f32; 2],
    /// Moment of inertia about its own centroid (uniform density over its polygon).
    pub inertia: f32,
    /// Base stats before bonuses. `HeatLimit` is `f32::INFINITY` for parts that never shut down;
    /// `FireRate` is a multiplier (1 = base).
    pub base: PartStats,
    pub weapon: Option<WeaponFamily>,
    /// How the gun's shots travel (guns only).
    pub delivery: Delivery,
    /// Ore it cuts from asteroid rock per second at full power (content `weapon.cut`, GDR-0114; 0: none).
    pub cut: f32,
    /// Point defence (GDR-0115): the gun aims itself and fires on its own (guns only).
    pub defence: Option<data::DefenceSpec>,
    /// Disruption (GDR-0125, `combat/disruption.rs`): static carried per point of damage that reaches a part
    /// (content `weapon.disrupt / weapon.damage`; 0: none).
    pub disrupt: f32,
    /// A lance (GDR-0127, `combat/lance.rs`): the shot strikes every part on its line through the struck piece.
    pub lance: Option<data::LanceSpec>,
    /// Shield bubbles: share of each family's damage absorbed (indexed by [`family_index`]).
    pub absorb: [f32; FAMILIES],
    /// A prism (GDR-0126, content `prism`): the shares of a relayed gun's reach and damage it adds (`[0, 0]`: none).
    pub relay: [f32; 2],
    /// A hull part of refractive armour (GDR-0126, its material's `refract`): the share of an energy hit-scan
    /// shot's damage it turns away along the mirrored line (0: none).
    pub refract: f32,
    pub is_engine: bool,
    /// Reaction-control pod: one nozzle per straight side open to space.
    pub is_rcs: bool,
    pub is_radiator: bool,
    pub is_shield: bool,
    pub logic: bool,
    /// Behavior rules this logic core can run (`logic.rule_slots`; 0 for other parts).
    pub rule_slots: u32,
    /// Control capacity of this logic core (`logic.control_capacity`, GDR-0038 "N"; 0 for other parts).
    pub control_capacity: u32,
    /// What the part costs against its vessel's control capacity (content `control_cost`, default by kind).
    pub control_cost: u32,
    /// Cargo capacity of a cargo part (content `cargo.capacity`, in cargo units; 0 for other parts).
    pub cargo: f32,
    /// How far the vessel sees while the part works (content `sensor.range`, m; 0 for parts without sensors).
    pub sensor: f32,
    /// A scanner's numbers (content `scanner`).
    pub scanner: Option<data::ScannerSpec>,
    /// Its draw comes in bursts, and when in words (content `power.intermittent`): the design report's full use
    /// leaves it out.
    pub intermittent: Option<String>,
    /// Armour material of a hull part (the placement's; `None` for other parts). Salvaged hull parts carry it.
    pub material: Option<String>,
    /// Build points as placed (hull parts with their material's factor): what a salvaged part is worth by
    /// default (`salvage.rs`).
    pub build_points: f32,
    /// Cargo units the part takes in a hold when salvaged: its footprint cells (`size.0 × size.1`, a code rule;
    /// a slope counts its whole footprint).
    pub cargo_units: u32,
    /// Collectors take it (content: the part has a `salvage` block; a hull part, its material has one).
    pub salvageable: bool,
    /// Destroyed by damage, it leaves a fragment to scoop (content: its `salvage` block names a non-empty
    /// `fragment`).
    pub fragment: bool,
    /// `Some` for parts that draw power.
    pub consumer: Option<ConsumerKind>,
    /// Consumers: tier and weight at spawn (the part's content default, or the placement's override).
    pub power_setting: crate::ship::power::PowerSetting,
    /// Consumers: below this share of its demand the part switches off (content `power.min_level`).
    pub power_min_level: f32,
    /// Feeds the power pool.
    pub reactor: bool,
    /// Its chain reaction when destroyed (content `explosion`, GDR-0084): damage and radius. Whom it hits is
    /// [`ShipDesign::blast`].
    pub explosion: Option<data::ExplosionSpec>,
    /// Share of an explosion's damage the part takes: a hull part its armour's `blast_factor`, others 1.
    pub blast_factor: f32,
    /// Heat capacity (heat units per degree); proportional to area.
    pub heat_capacity: f32,
    /// Living tissue (GDR-0099 A4): hit points regained per second on a vessel's controlling piece (a hull part
    /// its armour's `heal × area`, other parts their `heal`; 0: never heals).
    pub heal: f32,
    /// A heart (content `heart`, GDR-0099 A1): its regrowth's interval and the share of hit points a re-formed part
    /// starts with.
    pub heart: Option<data::HeartSpec>,
    /// As the touching part of a ram (content `ram`, GDR-0099 A2): the factor on the ram damage the other side
    /// takes, and on its own side's (1 and 1 for parts without `ram`).
    pub ram: [f32; 2],
}

/// Two linked parts and the number of cell edges they share (heat conducts through each).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    pub a: PartIdx,
    pub b: PartIdx,
    pub edges: u16,
}

/// One possible adjacency bonus: `source` grants `effect` on `stat` to linked `target`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BonusLink {
    pub target: PartIdx,
    pub source: PartIdx,
    /// Bonuses with the same group (same granting part type and bonus entry) share `max_stacks`.
    pub group: u32,
    pub stat: Stat,
    pub effect: Effect,
    pub max_stacks: u32,
    /// `Condition::SourceVenting`: only while the source has a side open to space.
    pub needs_venting: bool,
}

/// One part a blast reaches on the exploding part's own piece ([`ShipDesign::blast`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlastTarget {
    pub part: PartIdx,
    /// The explosion's damage times the target's `blast_factor`, before cover.
    pub damage: f32,
    /// Index of its cover list in [`ShipDesign::blast_cover`]: each of those parts still alive on the piece
    /// lets only its `blast_factor` of the damage through (GDR-0085).
    pub cover: u32,
}

/// Compressed adjacency lists: `items[start[i]..start[i + 1]]` belong to entry `i`.
#[derive(Clone, Debug, Default)]
pub struct Csr<T> {
    start: Vec<u32>,
    items: Vec<T>,
}

impl<T> Csr<T> {
    fn from_lists(lists: Vec<Vec<T>>) -> Csr<T> {
        let mut start = Vec::with_capacity(lists.len() + 1);
        let mut items = Vec::new();
        start.push(0);
        for l in lists {
            items.extend(l);
            start.push(items.len() as u32);
        }
        Csr { start, items }
    }

    #[inline]
    pub fn get(&self, i: usize) -> &[T] {
        &self.items[self.start[i] as usize..self.start[i + 1] as usize]
    }

    pub fn len(&self) -> usize {
        self.start.len().saturating_sub(1)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A gun's static mount: where it fires from, which way, and which parts block its line of fire.
#[derive(Clone, Debug)]
pub struct GunSpec {
    pub part: PartIdx,
    /// Centre of the facing side, blueprint frame.
    pub muzzle: [f32; 2],
    pub facing: Dir,
    /// Its weapon group, 1 to `data::MAX_WEAPON_GROUP` (GDR-0088): which of the pilot's triggers fires it.
    pub group: u8,
    /// The prisms of the design that take its beam (GDR-0126, their `target` picks it), ascending: its line of fire
    /// is a beam path through them ([`crate::Ship::beam_path`]). Empty: the plain line of fire.
    pub prisms: Vec<PartIdx>,
}

/// A hangar's static mount (GDR-0117): where its drones leave, which way, and the drone they are.
#[derive(Clone, Debug)]
pub struct HangarMount {
    pub part: PartIdx,
    /// Centre of the side the drones leave by (the part's forward side), blueprint frame.
    pub mouth: [f32; 2],
    pub facing: Dir,
    /// The cells straight in front of that side: a drone leaves only while none of them holds a part.
    pub front: Vec<(i32, i32)>,
    /// Drones it holds, seconds to rebuild one, metres they may range (content `hangar`).
    pub capacity: u32,
    pub rebuild: f32,
    pub leash: f32,
    /// The drone's design (compiled with the carrier's upgrades).
    pub drone: std::sync::Arc<ShipDesign>,
}

/// A flare launcher's static mount (GDR-0115, `combat/flare.rs`): where its decoys leave and which way.
#[derive(Clone, Copy, Debug)]
pub struct FlareMount {
    pub part: PartIdx,
    /// Centre of the part, blueprint frame.
    pub centre: [f32; 2],
    pub facing: Dir,
    /// Its numbers (content `countermeasure`).
    pub spec: data::CountermeasureSpec,
}

/// A blink drive's static mount (GDR-0127, `flight/blink.rs`).
#[derive(Clone, Copy, Debug)]
pub struct BlinkMount {
    pub part: PartIdx,
    /// Its numbers (content `blink`).
    pub spec: data::BlinkSpec,
}

/// A tractor beam's static mount (GDR-0131, `combat/tractor.rs`).
#[derive(Clone, Copy, Debug)]
pub struct TractorMount {
    pub part: PartIdx,
    /// Its numbers (content `tractor`).
    pub spec: data::TractorSpec,
}

/// An engine's static mount.
#[derive(Clone, Debug)]
pub struct EngineSpec {
    pub part: PartIdx,
    /// Centre of the part, blueprint frame. The thrust line runs through it along `facing`.
    pub point: [f32; 2],
    /// The side the engine pushes towards (the exhaust leaves through the opposite side).
    pub facing: Dir,
    /// Its plume lanes: `ShipDesign::plume_lanes[lanes[0]..lanes[1]]`, one per cell of its back side.
    pub lanes: [u32; 2],
}

/// One lane of an engine's exhaust plume: a straight line from the back face of one of the engine's cells,
/// along the exhaust (away from `EngineSpec::facing`). The plume's damage and heat are shared evenly over the
/// engine's lanes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlumeLane {
    /// Centre of the cell's back face, blueprint frame: where the plume leaves the engine.
    pub origin: [f32; 2],
}

/// A blueprint compiled for the simulation. Immutable; shared by all ships of this design.
#[derive(Clone, Debug)]
pub struct ShipDesign {
    pub id: String,
    pub name: String,
    pub faction: String,
    pub parts: Vec<PartSpec>,
    /// The intact occupancy grid; each ship clones it and removes destroyed parts.
    pub grid: ShipGrid,
    /// Linked neighbours of each part (ascending).
    pub neighbors: Csr<PartIdx>,
    /// Every linked pair once (`a < b`), in ascending order.
    pub links: Vec<Link>,
    /// Bonuses each part may receive, sorted by source.
    pub bonuses_in: Csr<BonusLink>,
    /// Engine slot → engine. Slots index `ShipControls::throttle`.
    pub engines: Vec<EngineSpec>,
    /// Gun slot → gun.
    pub guns: Vec<GunSpec>,
    /// Engine or gun slot of each part (`NO_SLOT` otherwise).
    pub slot_of_part: Vec<u32>,
    /// Parts with a sensor range, ascending (`ship/scan.rs`).
    pub sensors: Vec<PartIdx>,
    /// Scanners, ascending.
    pub scanners: Vec<PartIdx>,
    /// Hearts, ascending (`ship/living.rs`).
    pub hearts: Vec<PartIdx>,
    /// Hangars, ascending part (`ship/drones.rs`, GDR-0117).
    pub hangars: Vec<HangarMount>,
    /// Flare launchers, ascending part (`combat/flare.rs`, GDR-0115).
    pub flares: Vec<FlareMount>,
    /// Blink drives, ascending part (`flight/blink.rs`, GDR-0127).
    pub blinks: Vec<BlinkMount>,
    /// Tractor beams, ascending part (`combat/tractor.rs`, GDR-0131).
    pub tractors: Vec<TractorMount>,
    /// Parts that heal (`PartSpec::heal` above 0), ascending.
    pub healing: Vec<PartIdx>,
    /// Parts that block each gun's line of fire (cells straight ahead, to the grid edge), per gun slot.
    pub gun_blockers: Csr<PartIdx>,
    /// Every engine's plume lanes (see `EngineSpec::lanes`).
    pub plume_lanes: Vec<PlumeLane>,
    /// Per plume lane, the parts in its path to the grid edge in order: `(distance, part)`, the distance (m)
    /// from the lane's origin to where the plume enters the part's first cell on the lane. A plume burns the
    /// first of them that is alive and in the engine's piece, if it is within its length (`plume.rs`).
    pub plume_path: Csr<(f32, PartIdx)>,
    /// Per part, whom its explosion hits on its own piece and how hard (ascending part; empty for parts without
    /// one): every other part with a cell within the radius of its centre, the damage scaled by the target's
    /// `blast_factor`, and the armour between ([`BlastTarget::cover`]). The sim skips targets and cover that are
    /// dead or on another piece by then; other pieces and ships are found when the blast goes off (`chain.rs`).
    pub blast: Csr<BlastTarget>,
    /// Per blast target ([`BlastTarget::cover`]), the parts with a `blast_factor` below 1 (armour) whose cell the
    /// line from the blast's centre to the target's nearest cell centre passes through, ascending (GDR-0085).
    pub blast_cover: Csr<PartIdx>,
    /// Content problems that do not stop the sim (e.g. bonuses on stats the sim does not model yet).
    pub warnings: Vec<String>,
    /// Hit points of the intact ship (sum over all parts): the 100% of the `hull` behavior sensors.
    pub hp_total: f32,
    /// The behavior (content id) this ship flies under when nobody pilots it: the blueprint's own, else its
    /// faction's default. Look it up with `BehaviorLibrary::default_for`.
    pub behavior: Option<String>,
    /// A station (the blueprint's `anchored`, GDR-0110): its pieces' bodies turn but never move.
    pub anchored: bool,
}

/// Heat capacity per cell of area. PLACEHOLDER.
pub const HEAT_CAPACITY_PER_CELL: f32 = 1.0;

impl ShipDesign {
    /// Compiles a blueprint. Fails on unknown parts or materials (impossible for validated content).
    pub fn compile(content: &Content, ship: &Blueprint) -> Result<ShipDesign, String> {
        Self::compile_upgraded(content, ship, &Upgrades::NONE)
    }

    /// Compiles the player's blueprint with their researched part-stat steps (GDR-0024's refinement): each step
    /// changes its parts' base stats before the adjacency bonuses. The steps come in with the call (the career's,
    /// a lab run's, a spec's), so every place that builds the design gets the same numbers.
    pub fn compile_upgraded(content: &Content, ship: &Blueprint, upgrades: &Upgrades) -> Result<ShipDesign, String> {
        let grid = content.ship_grid(ship)?;
        let n = ship.parts.len();
        if n >= u32::MAX as usize / 2 {
            return Err(format!("ship `{}` has too many parts", ship.id));
        }
        let defs: Vec<&data::Part> = ship
            .parts
            .iter()
            .map(|p| {
                content
                    .parts
                    .get(&p.part)
                    .ok_or_else(|| format!("ship `{}`: unknown part `{}`", ship.id, p.part))
            })
            .collect::<Result<_, _>>()?;
        let mut warnings = Vec::new();

        // Part specs.
        let mut parts = Vec::with_capacity(n);
        for (i, (pl, def)) in ship.parts.iter().zip(&defs).enumerate() {
            let shape = *grid.shape(i);
            let mass = content.placement_mass(pl).ok_or_else(|| {
                format!(
                    "ship `{}` part {i} (`{}`): no mass (missing armour material?)",
                    ship.id, pl.part
                )
            })?;
            let hp = content.placement_hp(pl).ok_or_else(|| {
                format!("ship `{}` part {i} (`{}`): no hit points", ship.id, pl.part)
            })?;
            let (centroid, inertia) = polygon_mass(&shape.polygon(), mass);
            let power = def.power.clone().unwrap_or_default();
            let heat = def.heat.clone().unwrap_or_default();
            let mut base = PartStats {
                v: [0.0; Stat::COUNT],
            };
            base.set(Stat::PowerOutput, power.output);
            base.set(Stat::PowerDraw, power.draw);
            base.set(Stat::HeatOutput, heat.output);
            base.set(Stat::HeatLimit, heat.limit.unwrap_or(f32::INFINITY));
            base.set(
                Stat::HeatVent,
                def.radiator.as_ref().map_or(0.0, |r| r.vent),
            );
            base.set(Stat::Damage, def.weapon.as_ref().map_or(0.0, |w| w.damage));
            base.set(Stat::FireRate, 1.0);
            base.set(Stat::Thrust, def.engine.as_ref().map_or(0.0, |e| e.thrust));
            base.set(Stat::Gimbal, def.engine.as_ref().map_or(0.0, |e| e.gimbal));
            base.set(
                Stat::RatedSpeed,
                def.engine.as_ref().map_or(0.0, |e| e.rated_speed),
            );
            let plume = def.engine.as_ref().map(|e| &e.plume);
            base.set(Stat::PlumeLength, plume.map_or(0.0, |p| p.length));
            base.set(Stat::PlumeDamage, plume.map_or(0.0, |p| p.damage));
            base.set(Stat::PlumeHeat, plume.map_or(0.0, |p| p.heat));
            let shield = def.shield.as_ref();
            base.set(Stat::ShieldRadius, shield.map_or(0.0, |s| s.radius));
            base.set(Stat::ShieldCapacity, shield.map_or(0.0, |s| s.capacity));
            base.set(Stat::ShieldRegen, shield.map_or(0.0, |s| s.regen));
            base.set(Stat::Range, def.weapon.as_ref().map_or(0.0, |w| w.range));
            let mut absorb = [0.0f32; FAMILIES];
            if let Some(sh) = shield {
                for (&f, &a) in &sh.absorb {
                    absorb[family_index(f)] = a;
                }
            }
            base.set(Stat::Torque, def.gyro.as_ref().map_or(0.0, |g| g.torque));
            base.set(Stat::RcsThrust, def.rcs.as_ref().map_or(0.0, |r| r.thrust));
            base.set(
                Stat::RcsRatedSpeed,
                def.rcs.as_ref().map_or(0.0, |r| r.rated_speed),
            );
            if !upgrades.is_empty() {
                for id in data::design::stats::BONUS_STATS {
                    if let Some(stat) = Stat::from_id(id) {
                        base.set(stat, upgrades.apply(def, id, base.get(stat)));
                    }
                }
            }
            let consumer = (power.draw > 0.0).then(|| {
                if def.logic.is_some() {
                    ConsumerKind::Logic
                } else if def.shield.is_some() {
                    ConsumerKind::Shield
                } else if def.engine.is_some() || def.gyro.is_some() || def.rcs.is_some() {
                    ConsumerKind::Engine
                } else if def.weapon.as_ref().is_some_and(|w| w.defence.is_none()) {
                    // Point defence fires on its own: always on, as `Other`.
                    ConsumerKind::Weapon
                } else if def.scanner.is_some() {
                    ConsumerKind::Scanner
                } else if def.blink.is_some() {
                    ConsumerKind::Blink
                } else {
                    ConsumerKind::Other
                }
            });
            let over = pl.power.unwrap_or_default();
            let power_setting = crate::ship::power::PowerSetting {
                tier: over.tier.unwrap_or(power.tier),
                weight: over.weight.unwrap_or(power.weight),
            };
            let w = power_setting.weight;
            if !(w > 0.0 && w <= data::MAX_POWER_WEIGHT) || !(0.0..=1.0).contains(&power.min_level)
            {
                return Err(format!(
                    "ship `{}` part {i} (`{}`): power weight {w} or min_level {} out of range",
                    ship.id, pl.part, power.min_level
                ));
            }
            let area = def.area();
            parts.push(PartSpec {
                id: def.id.clone(),
                shape,
                hull: def.hull,
                slope: def.shape == data::Shape::Slope,
                area,
                mass,
                hp,
                centroid,
                inertia,
                base,
                weapon: def.weapon.as_ref().map(|w| w.family),
                delivery: def.weapon.as_ref().map_or(Delivery::Beam, |w| w.delivery),
                cut: def.weapon.as_ref().map_or(0.0, |w| w.cut),
                defence: def.weapon.as_ref().and_then(|w| w.defence),
                disrupt: def
                    .weapon
                    .as_ref()
                    .filter(|w| w.disrupt > 0.0 && w.damage > 0.0)
                    .map_or(0.0, |w| w.disrupt / w.damage),
                lance: def.weapon.as_ref().and_then(|w| w.lance),
                absorb,
                relay: def.prism.as_ref().map_or([0.0; 2], |p| [p.range, p.damage]),
                refract: if def.hull { content.placement_armor(pl).map_or(0.0, |a| a.refract) } else { 0.0 },
                is_engine: def.engine.is_some(),
                is_rcs: def.rcs.is_some(),
                is_radiator: def.radiator.is_some(),
                is_shield: def.shield.is_some(),
                logic: def.logic.is_some(),
                rule_slots: def.logic.as_ref().map_or(0, |l| l.rule_slots),
                control_capacity: def.logic.as_ref().map_or(0, |l| l.control_capacity),
                control_cost: def.control_cost(),
                cargo: def.cargo.as_ref().map_or(0.0, |c| c.capacity),
                sensor: def.sensor.as_ref().map_or(0.0, |x| x.range),
                scanner: def.scanner.clone(),
                intermittent: def.power.as_ref().and_then(|p| p.intermittent.clone()),
                material: if def.hull { pl.material.clone() } else { None },
                build_points: def.build_points_in(content.placement_armor(pl)),
                cargo_units: def.size.0.saturating_mul(def.size.1).max(1),
                salvageable: if def.hull {
                    content
                        .placement_material(pl)
                        .is_some_and(|m| m.salvage.is_some())
                } else {
                    def.salvage.is_some()
                },
                fragment: !def.hull
                    && def.salvage.as_ref().is_some_and(|s| !s.fragment.is_empty()),
                consumer,
                power_setting,
                power_min_level: power.min_level,
                reactor: power.output > 0.0,
                explosion: def.explosion.clone(),
                blast_factor: if def.hull {
                    content
                        .placement_armor(pl)
                        .map_or(1.0, |a| a.blast_factor)
                } else {
                    1.0
                },
                heat_capacity: HEAT_CAPACITY_PER_CELL * area,
                heal: def.heal_in(content.placement_armor(pl)).unwrap_or(0.0),
                heart: def.heart.clone(),
                ram: def.ram.as_ref().map_or([1.0, 1.0], |r| [r.dealt, r.taken]),
            });
        }

        // Links with shared edge counts (each shared edge seen once, from the lower part index).
        let mut edge_count: BTreeMap<(u32, u32), u16> = BTreeMap::new();
        for i in 0..n {
            for c in grid.shape(i).cells() {
                for d in Dir::ALL {
                    let (dx, dy) = d.delta();
                    if let Some(j) = grid.occupant((c.0 + dx, c.1 + dy))
                        && j > i
                        && grid.occupant(c) == Some(i)
                        && grid.linked(c, d)
                    {
                        *edge_count.entry((i as u32, j as u32)).or_insert(0) += 1;
                    }
                }
            }
        }
        let links: Vec<Link> = edge_count
            .iter()
            .map(|(&(a, b), &edges)| Link { a, b, edges })
            .collect();
        let mut nbr: Vec<Vec<PartIdx>> = vec![Vec::new(); n];
        for l in &links {
            nbr[l.a as usize].push(l.b);
            nbr[l.b as usize].push(l.a);
        }
        for l in &mut nbr {
            l.sort_unstable();
        }

        // Adjacency bonuses, declared on the source, applied to linked targets that match.
        let mut groups: BTreeMap<(&str, usize), u32> = BTreeMap::new();
        let mut bonus_lists: Vec<Vec<BonusLink>> = vec![Vec::new(); n];
        for (s, def) in defs.iter().enumerate() {
            for (k, b) in def.bonuses.iter().enumerate() {
                let Some(stat) = Stat::from_id(&b.stat) else {
                    warnings.push(format!(
                        "part `{}` bonus {k}: stat `{}` is not simulated yet; ignored",
                        def.id, b.stat
                    ));
                    continue;
                };
                let next = groups.len() as u32;
                let group = *groups.entry((def.id.as_str(), k)).or_insert(next);
                let needs_venting = b.when.contains(&data::Condition::SourceVenting);
                for &t in &nbr[s] {
                    if defs[t as usize].matches(&b.target) {
                        bonus_lists[t as usize].push(BonusLink {
                            target: t,
                            source: s as u32,
                            group,
                            stat,
                            effect: b.effect,
                            max_stacks: b.max_stacks.unwrap_or(u32::MAX),
                            needs_venting,
                        });
                    }
                }
                if let Selector::Part(id) = &b.target
                    && !content.parts.contains(id)
                {
                    warnings.push(format!(
                        "part `{}` bonus {k}: unknown target part `{id}`",
                        def.id
                    ));
                }
            }
        }
        for l in &mut bonus_lists {
            l.sort_by_key(|b| (b.source, b.group));
        }

        // Guns and engines.
        let bounds = grid.bounds();
        let mut slot_of_part = vec![NO_SLOT; n];
        let mut engines = Vec::new();
        let mut guns = Vec::new();
        let mut gun_blockers = Vec::new();
        let mut hangars = Vec::new();
        let mut flares = Vec::new();
        let mut blinks = Vec::new();
        let mut tractors = Vec::new();
        let mut plume_lanes = Vec::new();
        let mut plume_path = Vec::new();
        for (i, spec) in parts.iter().enumerate() {
            let shape = spec.shape;
            let (w, h) = shape.dims();
            let (x, y, w, h) = (shape.x as f32, shape.y as f32, w as f32, h as f32);
            let facing = shape.facing();
            if spec.is_engine {
                slot_of_part[i] = engines.len() as u32;
                let first = plume_lanes.len() as u32;
                lanes_of(&grid, i, facing.opposite(), bounds, &mut plume_lanes, &mut plume_path);
                engines.push(EngineSpec {
                    part: i as u32,
                    point: shape.center(),
                    facing,
                    lanes: [first, plume_lanes.len() as u32],
                });
            } else if spec.weapon.is_some() {
                slot_of_part[i] = guns.len() as u32;
                let muzzle = match facing {
                    Dir::Up => [x + w / 2.0, y],
                    Dir::Down => [x + w / 2.0, y + h],
                    Dir::Left => [x, y + h / 2.0],
                    Dir::Right => [x + w, y + h / 2.0],
                };
                guns.push(GunSpec {
                    part: i as u32,
                    muzzle,
                    facing,
                    // Out of range only in unvalidated blueprints (tests, tools): the nearest group.
                    group: ship.parts[i].group.unwrap_or(1).clamp(1, data::MAX_WEAPON_GROUP),
                    prisms: (0..n)
                        .filter(|&o| defs[o].prism.as_ref().is_some_and(|p| defs[i].matches(&p.target)))
                        .map(|o| o as PartIdx)
                        .collect(),
                });
                gun_blockers.push(line_of_fire_blockers(&grid, i, facing, bounds));
            }
            if let Some(c) = defs[i].countermeasure {
                flares.push(FlareMount { part: i as u32, centre: shape.center(), facing, spec: c });
            }
            if let Some(b) = defs[i].blink {
                blinks.push(BlinkMount { part: i as u32, spec: b });
            }
            if let Some(t) = defs[i].tractor {
                tractors.push(TractorMount { part: i as u32, spec: t });
            }
            if let Some(hs) = &defs[i].hangar {
                let bp = content
                    .ships
                    .get(&hs.drone)
                    .ok_or_else(|| format!("ship `{}` part {i}: unknown drone `{}`", ship.id, hs.drone))?;
                let drone = ShipDesign::compile_upgraded(content, bp, upgrades)?;
                if !drone.hangars.is_empty() {
                    return Err(format!("ship `{}` part {i}: the drone `{}` carries a hangar", ship.id, hs.drone));
                }
                hangars.push(HangarMount {
                    part: i as u32,
                    mouth: match facing {
                        Dir::Up => [x + w / 2.0, y],
                        Dir::Down => [x + w / 2.0, y + h],
                        Dir::Left => [x, y + h / 2.0],
                        Dir::Right => [x + w, y + h / 2.0],
                    },
                    facing,
                    front: {
                        let (sx, sy) = (shape.x, shape.y);
                        let (sw, sh) = shape.dims();
                        let (sw, sh) = (sw as i32, sh as i32);
                        match facing {
                            Dir::Up => (0..sw).map(|k| (sx + k, sy - 1)).collect(),
                            Dir::Down => (0..sw).map(|k| (sx + k, sy + sh)).collect(),
                            Dir::Left => (0..sh).map(|k| (sx - 1, sy + k)).collect(),
                            Dir::Right => (0..sh).map(|k| (sx + sw, sy + k)).collect(),
                        }
                    },
                    capacity: hs.capacity,
                    rebuild: hs.rebuild,
                    leash: hs.leash,
                    drone: std::sync::Arc::new(drone),
                });
            }
        }

        let (blast, blast_cover) = blast_targets(&grid, &parts);

        let hp_total = parts.iter().map(|p: &PartSpec| p.hp).sum::<f32>();
        let with = |f: fn(&PartSpec) -> bool| (0..n as PartIdx).filter(|&i| f(&parts[i as usize])).collect::<Vec<_>>();
        let (sensors, scanners) = (with(|p| p.sensor > 0.0), with(|p| p.scanner.is_some()));
        let (hearts, healing) = (with(|p| p.heart.is_some()), with(|p| p.heal > 0.0));
        Ok(ShipDesign {
            id: ship.id.clone(),
            name: ship.name.clone(),
            faction: ship.faction.clone(),
            parts,
            grid,
            neighbors: Csr::from_lists(nbr),
            links,
            bonuses_in: Csr::from_lists(bonus_lists),
            engines,
            guns,
            slot_of_part,
            sensors,
            scanners,
            hearts,
            hangars,
            flares,
            blinks,
            tractors,
            healing,
            gun_blockers: Csr::from_lists(gun_blockers),
            plume_lanes,
            plume_path: Csr::from_lists(plume_path),
            blast,
            blast_cover,
            warnings,
            hp_total,
            behavior: content.ship_behavior(ship).map(|b| b.id.clone()),
            anchored: ship.anchored,
        })
    }

}

/// Whom each part's explosion hits on its own piece ([`ShipDesign::blast`]): every other part with a cell whose
/// nearest point lies within the radius of the part's centre, ascending, with the damage it takes
/// (`damage × blast_factor`) and its cover ([`ShipDesign::blast_cover`]): the armour that the line from the centre
/// to the target's nearest cell centre passes through. Empty for parts without an explosion.
fn blast_targets(grid: &ShipGrid, parts: &[PartSpec]) -> (Csr<BlastTarget>, Csr<PartIdx>) {
    let mut lists = Vec::with_capacity(parts.len());
    let mut covers = Vec::new();
    let mut reach = Vec::new();
    let mut cover = Vec::new();
    for (i, spec) in parts.iter().enumerate() {
        let mut list = Vec::new();
        if let Some(e) = &spec.explosion {
            let c = spec.shape.center();
            crate::combat::chain::in_reach(grid, c, e.radius, |o| o != i, &mut reach);
            for &(t, cell) in &reach {
                let t = t as usize;
                crate::combat::chain::cover(
                    grid,
                    c,
                    cell,
                    |o| o != i && o != t && parts[o].blast_factor < 1.0,
                    &mut cover,
                );
                list.push(BlastTarget {
                    part: t as PartIdx,
                    damage: e.damage * parts[t].blast_factor,
                    cover: covers.len() as u32,
                });
                covers.push(cover.clone());
            }
        }
        lists.push(list);
    }
    (Csr::from_lists(lists), Csr::from_lists(covers))
}

/// Parts in the cells straight ahead of gun `i`'s `dir` side, to the edge of the grid. Same rule as
/// `ShipGrid::line_of_fire_clear`, but returning the blockers so breakage can re-check cheaply.
fn line_of_fire_blockers(
    grid: &ShipGrid,
    i: usize,
    dir: Dir,
    bounds: Option<data::Bounds>,
) -> Vec<PartIdx> {
    let mut out = Vec::new();
    for c in border_cells(grid, i, dir) {
        for (_, p) in cells_beyond(c, dir, bounds) {
            if let Some(o) = grid.occupant(p)
                && o != i
            {
                out.push(o as PartIdx);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The plume lanes of engine `i` exhausting towards `dir`: one per cell on its back side, each with the parts in
/// its path to the grid edge, in order, with the distance from the lane's origin to their first cell on it.
fn lanes_of(
    grid: &ShipGrid,
    i: usize,
    dir: Dir,
    bounds: Option<data::Bounds>,
    lanes: &mut Vec<PlumeLane>,
    paths: &mut Vec<Vec<(f32, PartIdx)>>,
) {
    for c in border_cells(grid, i, dir) {
        lanes.push(PlumeLane {
            origin: dir.side_center(c),
        });
        let mut path: Vec<(f32, PartIdx)> = Vec::new();
        for (k, p) in cells_beyond(c, dir, bounds) {
            if let Some(o) = grid.occupant(p)
                && o != i
                && path.last().is_none_or(|&(_, q)| q != o as PartIdx)
            {
                path.push((k as f32, o as PartIdx));
            }
        }
        paths.push(path);
    }
}

/// Cells of part `i` on its `dir` side whose neighbour in `dir` is not part of `i`.
fn border_cells(grid: &ShipGrid, i: usize, dir: Dir) -> impl Iterator<Item = data::Cell> + '_ {
    let (dx, dy) = dir.delta();
    grid.shape(i)
        .cells()
        .into_iter()
        .filter(move |c| grid.occupant((c.0 + dx, c.1 + dy)) != Some(i))
}

/// The cells beyond cell `c` in direction `dir` up to the edge of `bounds`, with the number of cells passed
/// before each (0 for the neighbour of `c`).
fn cells_beyond(
    c: data::Cell,
    dir: Dir,
    bounds: Option<data::Bounds>,
) -> impl Iterator<Item = (usize, data::Cell)> {
    let (dx, dy) = dir.delta();
    let inside = move |(x, y): data::Cell| {
        bounds.is_some_and(|b| x >= b.min_x && y >= b.min_y && x < b.max_x && y < b.max_y)
    };
    (1..)
        .map(move |k: i32| (k as usize - 1, (c.0 + dx * k, c.1 + dy * k)))
        .take_while(move |&(_, p)| inside(p))
}
