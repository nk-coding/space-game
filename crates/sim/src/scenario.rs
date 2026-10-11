//! Deterministic scenarios for the determinism check (native vs wasm, later the browser) and for tests. Both
//! spawn two copies of every ship blueprint in the content (load order), so they need no hard-coded ids and
//! grow with the content, and both roll their inputs and hits on the `Scenario` RNG stream.
//!
//! * [`ScenarioKind::Core`]: raw engine throttles and fire flags (every second ship flat out), random hits and
//!   a blast every 75 ticks: power, heat, damage, splits; bodies move under the `FreeFlight` backend. Plus one
//!   heat-stress ship (three beams on two reactors, always firing) so overheating stays covered whatever the
//!   base ships do.
//! * [`ScenarioKind::Flight`]: player ships flown by random pilot input (stick, lock on an enemy, drift,
//!   fire), automaton ships by the raider brain, fewer hits: flight computer, allocation, maneuvers, brains.
//! * [`ScenarioKind::Behavior`]: every ship under a content behavior (raiders: `raider_basic`; ship-mind
//!   ships: `skirmisher` and three player programs with target priorities, among them an escort following
//!   the piloted leader's lock), Rhai conditions included, plus hits and blasts that knock out logic cores
//!   (rule slots drop).
//!
//! When the physics engine lands, the same scenarios run on it and the same check compares the hashes.

use std::sync::Arc;

use data::Content;

use crate::breakage::DamageTarget;
use crate::design::{PartIdx, ShipDesign};
use crate::events::SimEvent;
use crate::flight::{PilotInput, Target};
use crate::maneuver::RaiderBrain;
use crate::math::{Vec2, cos, sin};
use crate::rng::Stream;
use crate::ship::{Body, Ship, ShipControls};
use crate::world::{DamageCommand, ShipCommand, Sim, TickInputs};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioKind {
    Core,
    Flight,
    /// The shared load test with this many ships (see [`load_test`]).
    Load(u32),
    /// Behaviors (content and player programs, Rhai conditions, target priorities, slots).
    Behavior,
}

impl ScenarioKind {
    pub fn from_name(name: &str) -> Option<ScenarioKind> {
        match name {
            "core" => Some(ScenarioKind::Core),
            "flight" => Some(ScenarioKind::Flight),
            "load" => Some(ScenarioKind::Load(20)),
            "behavior" => Some(ScenarioKind::Behavior),
            _ => None,
        }
    }

    /// Chance per tick of a random hit on a random alive part.
    fn hit_chance(self) -> f32 {
        match self {
            ScenarioKind::Core => 0.25,
            ScenarioKind::Flight => 0.08,
            ScenarioKind::Behavior => 0.06,
            ScenarioKind::Load(_) => 0.0,
        }
    }

    /// Every this many ticks a heavy blast hits a part and all its neighbours (provokes splits).
    fn blast_every(self) -> u64 {
        match self {
            ScenarioKind::Core => 75,
            ScenarioKind::Flight | ScenarioKind::Behavior => 150,
            ScenarioKind::Load(_) => u64::MAX,
        }
    }

    /// Inputs of each ship are re-rolled every this many ticks (staggered per ship).
    fn reroll_every(self) -> u64 {
        match self {
            ScenarioKind::Core => 20,
            ScenarioKind::Flight | ScenarioKind::Behavior => 45,
            ScenarioKind::Load(_) => u64::MAX,
        }
    }
}

/// Counters over the run, for the summary line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScenarioStats {
    pub parts_destroyed: u32,
    pub splits: u32,
    pub pieces_spawned: u32,
    pub pieces_destroyed: u32,
    /// Part-ticks spent overheated (summed over all parts and ticks).
    pub overheated_part_ticks: u64,
    /// Ship-ticks with at least one gun firing.
    pub firing_ticks: u64,
    /// Pulses and projectiles fired.
    pub shots: u32,
    /// Damage events on hulls, shield hits and ricochets.
    pub hits: u32,
    pub shield_hits: u32,
    pub ricochets: u32,
    /// Rhai condition evaluations.
    pub script_evals: u64,
}

pub struct Scenario {
    pub sim: Sim,
    pub stats: ScenarioStats,
    pub kind: ScenarioKind,
    inputs: TickInputs,
}

impl Scenario {
    /// The core scenario.
    pub fn new(content: &Content, seed: u64) -> Result<Scenario, String> {
        Scenario::with_kind(content, seed, ScenarioKind::Core)
    }

    /// Two ships of every blueprint in `content`, placed on a circle, spinning a little.
    pub fn with_kind(content: &Content, seed: u64, kind: ScenarioKind) -> Result<Scenario, String> {
        if let ScenarioKind::Load(n) = kind {
            return Ok(Scenario {
                sim: load_test_with(content, n as usize, seed)?,
                stats: ScenarioStats::default(),
                kind,
                inputs: TickInputs::default(),
            });
        }
        let mut sim = Sim::new(seed);
        let designs: Vec<Arc<ShipDesign>> = content
            .ships
            .iter()
            .map(|b| ShipDesign::compile(content, b).map(Arc::new))
            .collect::<Result<_, _>>()?;
        let n = designs.len() * 2;
        let radius = match kind {
            ScenarioKind::Core => 60.0,
            _ => 90.0,
        };
        for k in 0..n {
            let design = Arc::clone(&designs[k % designs.len()]);
            let a = k as f32 * (core::f32::consts::TAU / n as f32);
            let body = Body {
                pos: Vec2::new(cos(a) * radius, sin(a) * radius),
                angle: a,
                linvel: Vec2::new(-sin(a) * 2.0, cos(a) * 2.0),
                angvel: ((k % 5) as f32 - 2.0) * 0.15,
            };
            let id = sim.spawn_ship(Arc::clone(&design), body);
            if kind == ScenarioKind::Flight && design.faction != "ship_mind" {
                sim.command(id, ShipCommand::Brain(Some(RaiderBrain::default())));
            }
        }
        if kind == ScenarioKind::Behavior {
            assign_behaviors(content, &mut sim)?;
        }
        if kind == ScenarioKind::Core
            && let Ok(d) = ShipDesign::compile(content, &heat_stress_blueprint())
        {
            sim.spawn_ship(Arc::new(d), Body::at(Vec2::new(0.0, -15.0), 0.0));
        }
        Ok(Scenario {
            sim,
            stats: ScenarioStats::default(),
            kind,
            inputs: TickInputs::default(),
        })
    }

    /// One tick: roll inputs, step the sim, count events. Returns the state hash.
    pub fn step(&mut self) -> u64 {
        self.roll_inputs();
        self.sim.step(&self.inputs);
        for e in self.sim.events() {
            match e {
                SimEvent::PartDestroyed { .. } => self.stats.parts_destroyed += 1,
                SimEvent::Split(s) => {
                    self.stats.splits += 1;
                    self.stats.pieces_spawned += s.spawned.len() as u32;
                }
                SimEvent::PieceDestroyed { .. } => self.stats.pieces_destroyed += 1,
                SimEvent::ShotFired { .. } => self.stats.shots += 1,
                SimEvent::Hit { .. } => self.stats.hits += 1,
                SimEvent::ShieldHit { .. } => self.stats.shield_hits += 1,
                SimEvent::Ricochet { .. } => self.stats.ricochets += 1,
                _ => {}
            }
        }
        for ship in self.sim.ships() {
            self.stats.overheated_part_ticks += ship
                .parts()
                .iter()
                .filter(|p| p.alive && p.overheated)
                .count() as u64;
            if ship
                .pieces()
                .iter()
                .any(|p| p.guns.iter().any(|g| g.output > 0.0))
            {
                self.stats.firing_ticks += 1;
            }
        }
        self.stats.script_evals = self.sim.script_evaluations();
        self.sim.state_hash()
    }

    fn roll_inputs(&mut self) {
        let tick = self.sim.tick();
        let kind = self.kind;
        self.inputs.controls.clear();
        self.inputs.commands.clear();
        self.inputs.damage.clear();
        if let ScenarioKind::Load(_) = kind {
            return; // brains fly and fight on their own
        }
        let ships: Vec<(crate::ShipId, usize, bool, bool)> = self
            .sim
            .ships()
            .iter()
            .map(|s| {
                (
                    s.id(),
                    s.design().engines.len(),
                    s.brain().is_some() || s.behavior().is_some(),
                    s.design().id == HEAT_STRESS,
                )
            })
            .collect();
        let enemies: Vec<crate::ShipId> = self
            .sim
            .ships()
            .iter()
            .filter(|s| s.design().faction != "ship_mind")
            .map(|s| s.id())
            .collect();
        for (k, &(id, engines, brain, heat_stress)) in ships.iter().enumerate() {
            if brain || !(tick + k as u64 * 3).is_multiple_of(kind.reroll_every()) {
                continue;
            }
            let rng = self.sim.rng(Stream::Scenario);
            match kind {
                ScenarioKind::Core => {
                    // Every second ship (and the heat-stress ship) runs flat out, so heat builds up.
                    let stressed = k % 2 == 1 || heat_stress;
                    let throttle = (0..engines)
                        .map(|_| {
                            if stressed {
                                1.0
                            } else if rng.chance(0.3) {
                                0.0
                            } else {
                                rng.next_f32()
                            }
                        })
                        .collect();
                    let fire = stressed || rng.chance(0.6);
                    let shields = rng.chance(0.85);
                    self.inputs.controls.push((
                        id,
                        ShipControls {
                            throttle,
                            fire,
                            shields,
                        },
                    ));
                }
                ScenarioKind::Flight | ScenarioKind::Behavior | ScenarioKind::Load(_) => {
                    let a = rng.range_f32(-core::f32::consts::PI, core::f32::consts::PI);
                    let len = if rng.chance(0.2) {
                        0.0
                    } else {
                        rng.range_f32(0.2, 1.0)
                    };
                    let lock = if !enemies.is_empty() && rng.chance(0.5) {
                        Some(Target::Ship(
                            enemies[rng.below(enemies.len() as u32) as usize],
                        ))
                    } else {
                        None
                    };
                    let input = PilotInput {
                        stick: Vec2::new(cos(a), sin(a)) * len,
                        lock,
                        drift: rng.chance(0.2),
                        fire: rng.chance(0.7),
                        aim: crate::flight::AimMode::Auto,
                    };
                    self.inputs.commands.push((id, ShipCommand::Pilot(input)));
                }
            }
        }

        let hit = self.sim.rng(Stream::Scenario).chance(kind.hit_chance());
        let blast = tick > 0 && tick.is_multiple_of(kind.blast_every());
        if !(hit || blast) {
            return;
        }
        // A random ship that still has parts (not the heat-stress ship), then a random alive part of it.
        let targets: Vec<(crate::ShipId, u32)> = self
            .sim
            .ships()
            .iter()
            .filter(|s| s.design().id != HEAT_STRESS)
            .map(|s| (s.id(), s.parts().iter().filter(|p| p.alive).count() as u32))
            .filter(|&(_, n)| n > 0)
            .collect();
        if targets.is_empty() {
            return;
        }
        let rng = self.sim.rng(Stream::Scenario);
        let (ship_id, alive) = targets[rng.below(targets.len() as u32) as usize];
        let pick = rng.below(alive);
        let amount = rng.range_f32(0.4, 2.2);
        let ship = self.sim.ship(ship_id).unwrap();
        let part = nth_alive(ship, pick);
        self.inputs.damage.push(DamageCommand {
            ship: ship_id,
            target: DamageTarget::Part(part),
            amount,
        });
        if blast {
            for &nb in ship.design().neighbors.get(part as usize) {
                self.inputs.damage.push(DamageCommand {
                    ship: ship_id,
                    target: DamageTarget::Part(nb),
                    amount: 3.0,
                });
            }
        }
    }
}

/// Player programs of the behavior scenario (text form, as typed in the editor).
const HUNTER: &str = "target weakest(engines)
when hull < 30% -> flee
when target_in_range(110%) and cycle(8s) >= 5s -> orbit(70%, cw)
when closing_in(3s) -> face
always -> approach(60%)";
const GUARD: &str = "target threat(weapons)
when under_attack and shield_down -> kite(120%)
when enemies_near(80m) >= 2 -> flee
when target_time > 20s and target_hull > 80% -> orbit(50%)
always -> approach(80%)";
const ESCORT: &str = "target locked(power)
when target_in_range and not heat > 90% -> face
always -> approach(70%)";

/// Behavior scenario: raiders fly their faction default; the first ship-mind ship is piloted (random input,
/// the leader); the others cycle through the skirmisher, the hunter, the guard and an escort of the leader.
fn assign_behaviors(content: &Content, sim: &mut Sim) -> Result<(), String> {
    let show = |e: Vec<String>| e.join("; ");
    let mut lib = crate::behavior::BehaviorLibrary::new(content)
        .map_err(|p| show(p.iter().map(|p| p.to_string()).collect()))?;
    let mut add = |id: &str, text: &str| {
        lib.add_text(id, text)
            .map_err(|e| show(e.iter().map(|e| e.to_string()).collect()))
    };
    let programs = [
        None,
        Some(add("hunter", HUNTER)?),
        Some(add("guard", GUARD)?),
        Some(add("escort", ESCORT)?),
    ];
    sim.set_behaviors(lib);
    let ships: Vec<(crate::ShipId, bool, Option<crate::BehaviorId>)> = sim
        .ships()
        .iter()
        .map(|s| {
            (
                s.id(),
                s.design().faction == "ship_mind",
                sim.behaviors().default_for(s.design()),
            )
        })
        .collect();
    let mut leader = None;
    let mut k = 0;
    for (id, mind, default) in ships {
        if !mind {
            sim.command(id, ShipCommand::Behavior(default));
            continue;
        }
        let Some(l) = leader else {
            leader = Some(id); // piloted by the scenario's random input
            continue;
        };
        let program = programs[k % programs.len()].or(default);
        sim.command(id, ShipCommand::Behavior(program));
        sim.command(id, ShipCommand::Leader(Some(l)));
        k += 1;
    }
    Ok(())
}

const HEAT_STRESS: &str = "heat_stress";

/// Three beam lasers side by side on a fusion and a fission reactor, no radiators: overheats when it fires.
/// A test fixture built from base part ids (skipped if a content pack lacks them).
fn heat_stress_blueprint() -> data::Blueprint {
    let p = |part: &str, x: i32, y: i32| data::Placement {
        part: part.into(),
        x,
        y,
        rot: 0,
        flip: false,
        material: None,
    };
    data::Blueprint {
        id: HEAT_STRESS.into(),
        name: "Heat stress".into(),
        summary: "Determinism-scenario fixture that overheats.".into(),
        description: None,
        lore: None,
        tags: vec![],
        faction: "ship_mind".into(),
        size: (3, 4),
        parts: vec![
            p("beam_laser", 0, 0),
            p("beam_laser", 1, 0),
            p("beam_laser", 2, 0),
            p("fusion_reactor", 0, 2),
            p("fission_reactor", 2, 2),
            p("logic_core", 2, 3),
        ],
        behavior: None,
        source: Default::default(),
    }
}

fn nth_alive(ship: &Ship, n: u32) -> PartIdx {
    let mut k = 0;
    for (i, p) in ship.parts().iter().enumerate() {
        if p.alive {
            if k == n {
                return i as PartIdx;
            }
            k += 1;
        }
    }
    0
}

/// The shared load test (native perf runs and the browser client use the same setup): `n` ships on the
/// embedded base content, half Halvex Runners (rounded up) facing half automaton raiders 80 m away, every ship
/// under the combat brain (approach, attack, orbit, fire with lead), so they engage at once and produce
/// projectiles, hits and debris. With `n = 1` the lone runner orbits a point and fires into space.
pub fn load_test(n: usize, seed: u64) -> Sim {
    load_test_with(&data::base_content(), n, seed)
        .expect("base content has the runner and the raider")
}

/// [`load_test`] on any content that has `halvex_runner` and `automaton_raider`.
pub fn load_test_with(content: &Content, n: usize, seed: u64) -> Result<Sim, String> {
    let get = |id: &str| -> Result<Arc<ShipDesign>, String> {
        let bp = content
            .ships
            .get(id)
            .ok_or_else(|| format!("load test needs the ship `{id}`"))?;
        ShipDesign::compile(content, bp).map(Arc::new)
    };
    let (runner, raider) = (get("halvex_runner")?, get("automaton_raider")?);
    let mut sim = Sim::new(seed);
    let runners = n.div_ceil(2);
    let raiders = n / 2;
    for k in 0..runners {
        let x = (k as f32 - (runners as f32 - 1.0) / 2.0) * 25.0;
        let id = sim.spawn_ship(Arc::clone(&runner), Body::at(Vec2::new(x, 0.0), 0.0));
        if raiders == 0 {
            let maneuver = crate::maneuver::Maneuver::Orbit {
                target: crate::flight::Target::Point(Vec2::new(0.0, 50.0)),
                radius: 30.0,
                ccw: true,
            };
            sim.command(
                id,
                ShipCommand::Maneuver {
                    maneuver,
                    fire: true,
                },
            );
        } else {
            sim.command(id, ShipCommand::Brain(Some(RaiderBrain::default())));
        }
    }
    for k in 0..raiders {
        let x = (k as f32 - (raiders as f32 - 1.0) / 2.0) * 20.0;
        let id = sim.spawn_ship(
            Arc::clone(&raider),
            Body::at(Vec2::new(x, 80.0), core::f32::consts::PI),
        );
        sim.command(id, ShipCommand::Brain(Some(RaiderBrain::default())));
    }
    Ok(sim)
}

/// Runs a scenario on the embedded base content and returns the state hash after every tick.
pub fn run_kind(kind: ScenarioKind, seed: u64, ticks: u64) -> (Vec<u64>, ScenarioStats) {
    let content = data::base_content();
    let mut sc = Scenario::with_kind(&content, seed, kind).expect("base content compiles");
    let hashes = (0..ticks).map(|_| sc.step()).collect();
    (hashes, sc.stats)
}

/// The core scenario (see [`run_kind`]).
pub fn run(seed: u64, ticks: u64) -> (Vec<u64>, ScenarioStats) {
    run_kind(ScenarioKind::Core, seed, ticks)
}
