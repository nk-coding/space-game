//! A blueprint compiled for the simulation: every content lookup, string compare and geometry query happens
//! once here, so the per-tick code only touches flat numbers. One [`ShipDesign`] is shared (`Arc`) by every
//! ship built from the same blueprint.
//!
//! Everything in here is **static** for the intact blueprint. Two facts make that enough for breakage:
//! whether two present parts are linked depends only on those two parts' shapes, and destroyed parts only
//! remove links. So "the static neighbour list filtered by alive parts" is always the current connectivity.

use std::collections::BTreeMap;

use data::{
    Blueprint, Content, Delivery, Dir, Effect, PlacedShape, Selector, ShipGrid, WeaponFamily,
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

use crate::mass::polygon_mass;

/// Index of a part in its blueprint (`Blueprint::parts`), also the index used by `data::ShipGrid`.
pub type PartIdx = u32;

/// Marker for "no slot".
pub const NO_SLOT: u32 = u32::MAX;

/// What drives a power consumer's demand each tick (who gets power first is its tier and weight, see
/// [`crate::power`]).
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
}

impl Stat {
    pub const COUNT: usize = 15;

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
    /// Shield bubbles: share of each family's damage absorbed (indexed by [`family_index`]).
    pub absorb: [f32; FAMILIES],
    pub is_engine: bool,
    /// Reaction-control pod: one nozzle per straight side open to space.
    pub is_rcs: bool,
    pub is_radiator: bool,
    pub is_shield: bool,
    pub logic: bool,
    /// Behavior rules this logic core can run (`logic.rule_slots`; 0 for other parts).
    pub rule_slots: u32,
    pub cargo: f32,
    /// `Some` for parts that draw power.
    pub consumer: Option<ConsumerKind>,
    /// Consumers: tier and weight at spawn (the part's content default, or the placement's override).
    pub power_setting: crate::power::PowerSetting,
    /// Consumers: below this share of its demand the part switches off (content `power.min_level`).
    pub power_min_level: f32,
    /// Feeds the power pool.
    pub reactor: bool,
    /// Candidate for chain reactions when destroyed (hook only; rule should move to data, see docs).
    pub volatile: bool,
    /// Heat capacity (heat units per degree); proportional to area.
    pub heat_capacity: f32,
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
}

/// An engine's static mount.
#[derive(Clone, Debug)]
pub struct EngineSpec {
    pub part: PartIdx,
    /// Centre of the part, blueprint frame. The thrust line runs through it along `facing`.
    pub point: [f32; 2],
    /// The side the engine pushes towards (the exhaust leaves through the opposite side).
    pub facing: Dir,
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
    /// Parts that block each gun's line of fire (cells straight ahead, to the grid edge), per gun slot.
    pub gun_blockers: Csr<PartIdx>,
    /// Parts that block each engine's exhaust (cells directly behind), per engine slot.
    pub exhaust_blockers: Csr<PartIdx>,
    /// Content problems that do not stop the sim (e.g. bonuses on stats the sim does not model yet).
    pub warnings: Vec<String>,
    /// Hit points of the intact ship (sum over all parts): the 100% of the `hull` behavior sensors.
    pub hp_total: f32,
    /// The behavior (content id) this ship flies under when nobody pilots it: the blueprint's own, else its
    /// faction's default. Look it up with `BehaviorLibrary::default_for`.
    pub behavior: Option<String>,
}

/// Heat capacity per cell of area. PLACEHOLDER.
pub const HEAT_CAPACITY_PER_CELL: f32 = 1.0;

impl ShipDesign {
    /// Compiles a blueprint. Fails on unknown parts or materials (impossible for validated content).
    pub fn compile(content: &Content, ship: &Blueprint) -> Result<ShipDesign, String> {
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
            let consumer = (power.draw > 0.0).then(|| {
                if def.logic.is_some() {
                    ConsumerKind::Logic
                } else if def.shield.is_some() {
                    ConsumerKind::Shield
                } else if def.engine.is_some() || def.gyro.is_some() || def.rcs.is_some() {
                    ConsumerKind::Engine
                } else if def.weapon.is_some() {
                    ConsumerKind::Weapon
                } else {
                    ConsumerKind::Other
                }
            });
            let over = pl.power.unwrap_or_default();
            let power_setting = crate::power::PowerSetting {
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
                absorb,
                is_engine: def.engine.is_some(),
                is_rcs: def.rcs.is_some(),
                is_radiator: def.radiator.is_some(),
                is_shield: def.shield.is_some(),
                logic: def.logic.is_some(),
                rule_slots: def.logic.as_ref().map_or(0, |l| l.rule_slots),
                cargo: def.cargo.as_ref().map_or(0.0, |c| c.capacity),
                consumer,
                power_setting,
                power_min_level: power.min_level,
                reactor: power.output > 0.0,
                volatile: power.output > 0.0,
                heat_capacity: HEAT_CAPACITY_PER_CELL * area,
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
        let mut exhaust_blockers = Vec::new();
        for (i, spec) in parts.iter().enumerate() {
            let shape = spec.shape;
            let (w, h) = shape.dims();
            let (x, y, w, h) = (shape.x as f32, shape.y as f32, w as f32, h as f32);
            let facing = shape.facing();
            if spec.is_engine {
                slot_of_part[i] = engines.len() as u32;
                engines.push(EngineSpec {
                    part: i as u32,
                    point: shape.center(),
                    facing,
                });
                exhaust_blockers.push(border_scan(&grid, i, facing.opposite(), false, bounds));
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
                });
                gun_blockers.push(border_scan(&grid, i, facing, true, bounds));
            }
        }

        let hp_total = parts.iter().map(|p: &PartSpec| p.hp).sum::<f32>();
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
            gun_blockers: Csr::from_lists(gun_blockers),
            exhaust_blockers: Csr::from_lists(exhaust_blockers),
            warnings,
            hp_total,
            behavior: content.ship_behavior(ship).map(|b| b.id.clone()),
        })
    }

    pub fn part_count(&self) -> usize {
        self.parts.len()
    }
}

/// Parts in the cells beyond part `i`'s `dir` side: only the adjacent row (`to_edge == false`, exhaust) or
/// every cell to the edge of the grid (`to_edge == true`, line of fire). Same rule as
/// `ShipGrid::exhaust_clear` / `line_of_fire_clear`, but returning the blockers so breakage can re-check cheaply.
fn border_scan(
    grid: &ShipGrid,
    i: usize,
    dir: Dir,
    to_edge: bool,
    bounds: Option<data::Bounds>,
) -> Vec<PartIdx> {
    let (dx, dy) = dir.delta();
    let Some(b) = bounds else {
        return Vec::new();
    };
    let inside = |(x, y): (i32, i32)| x >= b.min_x && y >= b.min_y && x < b.max_x && y < b.max_y;
    let mut out = Vec::new();
    for c in grid.shape(i).cells() {
        if grid.occupant((c.0 + dx, c.1 + dy)) == Some(i) {
            continue;
        }
        let mut p = (c.0 + dx, c.1 + dy);
        while inside(p) {
            if let Some(o) = grid.occupant(p)
                && o != i
            {
                out.push(o as u32);
            }
            if !to_edge {
                break;
            }
            p = (p.0 + dx, p.1 + dy);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}
