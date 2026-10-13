//! Deterministic scenarios for the determinism check (native vs wasm, later the browser) and for tests. Both
//! spawn two copies of every ship blueprint in the content (load order), so they need no hard-coded ids and
//! grow with the content, and both roll their inputs and hits on the `Scenario` RNG stream.
//!
//! * [`ScenarioKind::Core`]: raw engine throttles and fire flags (every second ship flat out), random hits and
//!   a blast every 75 ticks: power, heat, damage, splits, collisions and ramming. Plus one
//!   heat-stress ship (three beams on two reactors, always firing) so overheating stays covered whatever the
//!   base ships do, a ram run every 600 ticks (a ship-mind and an automaton vessel launched head-on, away
//!   from the others: shields and the inward crush of GDR-0047), a burner (two buried thrusters facing each
//!   other, burning the blocks between them: the own-hull plume of GDR-0049) and a plume run every 600 ticks (an
//!   automaton or a wreck right behind a ship-mind vessel's engine at full throttle).
//! * [`ScenarioKind::Flight`]: player ships flown by random pilot input (stick, lock on an enemy, drift,
//!   fire), automaton ships by their faction's content behavior (`raider_basic`), fewer hits: flight
//!   computer, allocation, maneuvers, fire at will.
//! * [`ScenarioKind::Behavior`]: every ship under a content behavior (raiders: `raider_basic`; ship-mind
//!   ships: `skirmisher` and three player programs with target priorities, among them an escort following
//!   the piloted leader's lock), Rhai conditions included, plus hits and blasts that knock out logic cores
//!   (rule slots drop). The leader is the player's focus, and the player's mind capacity is
//!   [`BEHAVIOR_MIND_CAPACITY`] cores, so the newest ship-mind vessels start dormant and wake as others fall.
//! * [`ScenarioKind::Salvage`]: salvage (`salvage.rs`): a wreck of every blueprint spawned as a wreck
//!   (`Sim::spawn_wreck`, part of it already gone), the player's ship-mind ships with a hold flying to the
//!   nearest salvage (pieces and loose fragments) and collecting it, two raiders under their content behavior
//!   whose cores are shot out (fragments), a cargo part of one collector destroyed (its hold sheds) and one
//!   collector lost with its cargo (its wreck keeps it; the others strip it), plus random hits.
//! * [`ScenarioKind::Voyage`]: the voyage director (`voyage.rs`): the content's first voyage rolled from the seed
//!   and started, one player ship per ship-mind blueprint with a hold entering at its edge. Each flies random
//!   pilot input locked on the nearest armed awake enemy (`Sim::armed_enemy`) within reach of a fight, else to the
//!   nearest salvage, else to a site not yet cleared, so sites spawn, sleep, wake and clear; the cores of an armed
//!   awake enemy are shot out every few seconds (fragments, wrecks to strip), plus random hits.
//!
//! Owners: ship-mind ships belong to [`Owner::PLAYER`], every other faction's to `Owner(1)`
//! ([`scenario_owner`]). Splits leave vessels of their own (GDR-0013), so ship counts grow during a run.
//!
//! The state hash includes the physics (Rapier) state, so the native-vs-wasm check covers the engine too.

use std::sync::Arc;

use data::Content;

use crate::behavior::BehaviorLibrary;
use crate::breakage::DamageTarget;
use crate::design::{PartIdx, ShipDesign};
use crate::events::SimEvent;
use crate::flight::{PilotInput, Target};
use crate::math::{Vec2, cos, sin};
use crate::rng::Stream;
use crate::salvage::{Item, Salvaged};
use crate::ship::{Body, Ship, ShipControls};
use crate::vessel::Owner;
use crate::world::{DamageCommand, ShipCommand, Sim, TickInputs};

/// The owner a scenario gives a design's ships: ship-mind ships are the player's, everyone else is one
/// enemy side.
pub fn scenario_owner(design: &ShipDesign) -> Owner {
    if design.faction == "ship_mind" {
        Owner::PLAYER
    } else {
        Owner(1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioKind {
    Core,
    Flight,
    /// The shared load test with this many ships (see [`load_test`]).
    Load(u32),
    /// Behaviors (content and player programs, Rhai conditions, target priorities, slots).
    Behavior,
    /// Salvage: wrecks spawned as wrecks, collecting, fragments, cargo kept by a lost ship.
    Salvage,
    /// A voyage: sites spawning, sleeping, waking and clearing, salvage, the recall check.
    Voyage,
}

impl ScenarioKind {
    pub fn from_name(name: &str) -> Option<ScenarioKind> {
        match name {
            "core" => Some(ScenarioKind::Core),
            "flight" => Some(ScenarioKind::Flight),
            "load" => Some(ScenarioKind::Load(20)),
            "behavior" => Some(ScenarioKind::Behavior),
            "salvage" => Some(ScenarioKind::Salvage),
            "voyage" => Some(ScenarioKind::Voyage),
            _ => None,
        }
    }

    /// Chance per tick of a random hit on a random alive part.
    fn hit_chance(self) -> f32 {
        match self {
            ScenarioKind::Core => 0.25,
            ScenarioKind::Flight => 0.08,
            ScenarioKind::Behavior => 0.06,
            ScenarioKind::Salvage => 0.04,
            ScenarioKind::Voyage => 0.03,
            ScenarioKind::Load(_) => 0.0,
        }
    }

    /// Every this many ticks a heavy blast hits a part and all its neighbours (provokes splits).
    fn blast_every(self) -> u64 {
        match self {
            ScenarioKind::Core => 75,
            ScenarioKind::Flight | ScenarioKind::Behavior => 150,
            ScenarioKind::Salvage | ScenarioKind::Voyage => 300,
            ScenarioKind::Load(_) => u64::MAX,
        }
    }

    /// Inputs of each ship are re-rolled every this many ticks (staggered per ship).
    fn reroll_every(self) -> u64 {
        match self {
            ScenarioKind::Core => 20,
            ScenarioKind::Flight | ScenarioKind::Behavior => 45,
            ScenarioKind::Salvage | ScenarioKind::Voyage => SALVAGE_RETARGET,
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
    /// Collisions (`SimEvent::Impact`) and those that dealt ramming damage.
    pub impacts: u32,
    pub rams: u32,
    /// Engine plume hits (`SimEvent::PlumeHit`, one per lane and tick).
    pub burns: u32,
    /// Vessels formed by splits (`VesselFormed` with a parent) and vessels lost (`VesselLost`).
    pub split_vessels: u32,
    pub vessels_lost: u32,
    /// Mind events: vessels gone dormant, woken, focus changes.
    pub dormant: u32,
    pub woken: u32,
    pub focus_changes: u32,
    /// Salvage: items collected (`SimEvent::Collected`), the cargo units they took, how many came from a
    /// wreck's cargo and how many were fragments; fragments spawned (loose ids handed out).
    pub collected: u32,
    pub collected_units: u32,
    pub collected_cargo: u32,
    pub collected_fragments: u32,
    pub fragments: u32,
    /// Voyage: sites woken and cleared (`SiteWoke`, `SiteCleared`), and ticks the player could recall.
    pub sites_woken: u32,
    pub sites_cleared: u32,
    pub recall_ticks: u32,
}

pub struct Scenario {
    pub sim: Sim,
    pub stats: ScenarioStats,
    pub kind: ScenarioKind,
    inputs: TickInputs,
    /// Salvage and voyage: the player's ships the script flies, in spawn order.
    collectors: Vec<crate::ShipId>,
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
                collectors: Vec::new(),
            });
        }
        if kind == ScenarioKind::Salvage {
            return salvage_scenario(content, seed);
        }
        if kind == ScenarioKind::Voyage {
            return voyage_scenario(content, seed);
        }
        let mut config = crate::SimConfig::default();
        if kind == ScenarioKind::Behavior {
            config.mind_capacity[Owner::PLAYER.0 as usize] = BEHAVIOR_MIND_CAPACITY;
        }
        let mut sim = Sim::with_config(seed, config);
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
            let owner = scenario_owner(&design);
            sim.spawn_ship(design, owner, body);
        }
        if kind == ScenarioKind::Flight {
            sim.set_behaviors(library(content)?);
            let enemies: Vec<(crate::ShipId, Option<crate::BehaviorId>)> = sim
                .ships()
                .iter()
                .filter(|s| s.owner() != Owner::PLAYER)
                .map(|s| (s.id(), sim.behaviors().default_for(s.design())))
                .collect();
            for (id, b) in enemies {
                sim.command(id, ShipCommand::Behavior(b));
            }
        }
        if kind == ScenarioKind::Behavior {
            assign_behaviors(content, &mut sim)?;
        }
        if kind == ScenarioKind::Core
            && let Ok(d) = ShipDesign::compile(content, &heat_stress_blueprint())
        {
            sim.spawn_ship(
                Arc::new(d),
                Owner::PLAYER,
                Body::at(Vec2::new(0.0, -15.0), 0.0),
            );
        }
        if kind == ScenarioKind::Core
            && let Ok(d) = ShipDesign::compile(content, &burner_blueprint())
        {
            sim.spawn_ship(Arc::new(d), Owner::PLAYER, Body::at(Vec2::new(-300.0, 0.0), 0.0));
        }
        Ok(Scenario {
            sim,
            stats: ScenarioStats::default(),
            kind,
            inputs: TickInputs::default(),
            collectors: Vec::new(),
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
                SimEvent::Impact(c) => {
                    self.stats.impacts += 1;
                    self.stats.rams += (c.damage > 0.0) as u32;
                }
                SimEvent::PlumeHit { .. } => self.stats.burns += 1,
                SimEvent::VesselFormed { from: Some(_), .. } => self.stats.split_vessels += 1,
                SimEvent::VesselLost { .. } => self.stats.vessels_lost += 1,
                SimEvent::VesselDormant { .. } => self.stats.dormant += 1,
                SimEvent::VesselWoken { .. } => self.stats.woken += 1,
                SimEvent::FocusChanged { .. } => self.stats.focus_changes += 1,
                SimEvent::Collected {
                    from, item, units, ..
                } => {
                    self.stats.collected += 1;
                    self.stats.collected_units += units;
                    self.stats.collected_cargo += matches!(from, Salvaged::Cargo { .. }) as u32;
                    self.stats.collected_fragments += matches!(item, Item::Fragment { .. }) as u32;
                }
                SimEvent::SiteWoke { .. } => self.stats.sites_woken += 1,
                SimEvent::SiteCleared { .. } => self.stats.sites_cleared += 1,
                _ => {}
            }
        }
        if let Some(l) = self.sim.loose().last() {
            self.stats.fragments = self.stats.fragments.max(l.id.0 + 1);
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
        if self.kind == ScenarioKind::Voyage && self.sim.can_recall(Owner::PLAYER) {
            self.stats.recall_ticks += 1;
        }
        self.sim.state_hash()
    }

    fn roll_inputs(&mut self) {
        let tick = self.sim.tick();
        let kind = self.kind;
        self.inputs.controls.clear();
        self.inputs.commands.clear();
        self.inputs.damage.clear();
        if let ScenarioKind::Load(_) = kind {
            return; // behaviors fly and fight on their own
        }
        if kind == ScenarioKind::Core && tick % RAM_RUN_EVERY == RAM_RUN_EVERY / 2 {
            self.ram_run(tick / RAM_RUN_EVERY);
        }
        if kind == ScenarioKind::Core && tick % RAM_RUN_EVERY == RAM_RUN_EVERY * 3 / 4 {
            self.plume_run(tick / RAM_RUN_EVERY);
        }
        if kind == ScenarioKind::Salvage {
            self.salvage_run(tick);
        }
        if kind == ScenarioKind::Voyage {
            self.voyage_run(tick);
        }
        let ships: Vec<(crate::ShipId, usize, bool, bool)> = self
            .sim
            .ships()
            .iter()
            .map(|s| {
                (
                    s.id(),
                    s.design().engines.len(),
                    s.behavior().is_some(),
                    s.design().id == HEAT_STRESS || s.design().id == BURNER,
                )
            })
            .collect();
        let enemies: Vec<crate::ShipId> = self
            .sim
            .ships()
            .iter()
            .filter(|s| s.owner() != Owner::PLAYER)
            .map(|s| s.id())
            .collect();
        for (k, &(id, engines, autonomous, heat_stress)) in ships.iter().enumerate() {
            if autonomous || !(tick + k as u64 * 3).is_multiple_of(kind.reroll_every()) {
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
                            guns: Vec::new(),
                            shields,
                        },
                    ));
                }
                // Flight and behavior (the load test returned above: its behaviors fly on their own).
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
                        ..Default::default()
                    };
                    self.inputs.commands.push((id, ShipCommand::Pilot(input)));
                }
                // The collectors fly to salvage (`salvage_run`) or through the voyage (`voyage_run`), enemies
                // their behavior.
                ScenarioKind::Salvage | ScenarioKind::Voyage => {}
            }
        }

        let hit = self.sim.rng(Stream::Scenario).chance(kind.hit_chance());
        let blast = tick > 0 && tick.is_multiple_of(kind.blast_every());
        if !(hit || blast) {
            return;
        }
        // A random ship that still has parts (not the heat-stress ship or the burner), then a random alive part.
        let targets: Vec<(crate::ShipId, u32)> = self
            .sim
            .ships()
            .iter()
            .filter(|s| s.design().id != HEAT_STRESS && s.design().id != BURNER)
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

/// The core scenario's ram run comes every this many ticks (half-way through each period).
const RAM_RUN_EVERY: u64 = 600;

impl Scenario {
    /// Launches the first ship-mind vessel (its controlling piece) and the main piece of the first ship that is
    /// no player vessel (an automaton, or a wreck: salvage is nobody's friend) head-on at 20 m/s each, far from
    /// the rest, so a hard ram is part of the determinism check.
    fn ram_run(&mut self, n: u64) {
        let ships = self.sim.ships();
        let a = ships
            .iter()
            .filter(|s| s.team() == Some(Owner::PLAYER))
            .find_map(|s| Some((s.id(), s.control_piece()?)));
        let b = ships
            .iter()
            .filter(|s| s.team() != Some(Owner::PLAYER))
            .find_map(|s| Some((s.id(), s.main_piece()?)));
        let (Some(a), Some(b)) = (a, b) else {
            return;
        };
        let x = 400.0 + n as f32 * 100.0;
        let launch = |pos: Vec2, angle: f32, vy: f32| Body {
            pos,
            angle,
            linvel: Vec2::new(0.0, vy),
            angvel: 0.0,
        };
        self.sim
            .teleport(a.0, a.1, launch(Vec2::new(x, 0.0), 0.0, 20.0));
        self.sim.teleport(
            b.0,
            b.1,
            launch(Vec2::new(x, 16.0), core::f32::consts::PI, -20.0),
        );
    }
}

impl Scenario {
    /// Puts the first ship-mind vessel with an engine plume far from the rest, at rest, nose up, and the main
    /// piece of the first ship that is no player vessel (an automaton, or a wreck) right behind that engine's
    /// first plume lane (its bounding circle half a metre off the lane's origin), then opens that vessel's
    /// throttles: the plume burns an enemy (or salvage) for a few ticks until the vessel pulls away.
    fn plume_run(&mut self, n: u64) {
        let ships = self.sim.ships();
        let a = ships
            .iter()
            .filter(|s| s.team() == Some(Owner::PLAYER) && s.design().id != BURNER)
            .find_map(|s| {
                let piece = s.piece(s.control_piece()?)?;
                let row = piece
                    .thrusters
                    .iter()
                    .find(|t| t.slot != crate::design::NO_SLOT && t.plume_length > 0.0)?;
                let lane = s.design().engines[row.slot as usize].lanes[0];
                let origin = s.design().plume_lanes.get(lane as usize)?.origin;
                let local = crate::mass::to_piece(origin, piece.origin);
                Some((s.id(), piece.id, local, -row.dir, s.design().engines.len()))
            });
        let b = ships
            .iter()
            .filter(|s| s.team() != Some(Owner::PLAYER))
            .find_map(|s| {
                let p = s.piece(s.main_piece()?)?;
                Some((s.id(), p.id, p.radius))
            });
        let (Some((sa, pa, lane, exhaust, engines)), Some((sb, pb, radius))) = (a, b) else {
            return;
        };
        let at = Vec2::new(-400.0 - n as f32 * 100.0, 0.0);
        self.sim.teleport(sa, pa, Body::at(at, 0.0));
        self.sim
            .teleport(sb, pb, Body::at(at + lane + exhaust * (radius + 0.5), 0.0));
        self.inputs.controls.push((
            sa,
            ShipControls {
                throttle: vec![1.0; engines],
                fire: false,
                guns: Vec::new(),
                shields: true,
            },
        ));
    }
}

/// The salvage scenario's collectors pick a new target every this many ticks (staggered per collector).
const SALVAGE_RETARGET: u64 = 30;
/// Salvage scenario script: at each of these ticks the cores of the first raider still a vessel are shot out.
const SALVAGE_RAIDER_KILLS: [u64; 2] = [120, 240];
/// Salvage scenario script: a cargo part of the second collector is destroyed (its hold sheds).
const SALVAGE_HOLD_HIT: u64 = 450;
/// Salvage scenario script: the first collector stops (90 ticks before) and loses its logic cores (its wreck
/// keeps its cargo); the last collector waits until then with an empty hold and then strips that wreck.
const SALVAGE_LOST: u64 = 540;

/// The salvage scenario: a wreck of every blueprint in a ring (radius 30 m, drifting a little, 20–50 % of
/// their parts gone), one collector per ship-mind blueprint with a hold in a ring outside it (45 m), two raiders
/// 100 m out under their content behavior.
fn salvage_scenario(content: &Content, seed: u64) -> Result<Scenario, String> {
    let mut sim = Sim::new(seed);
    sim.set_behaviors(library(content)?);
    let designs: Vec<Arc<ShipDesign>> = content
        .ships
        .iter()
        .map(|b| ShipDesign::compile(content, b).map(Arc::new))
        .collect::<Result<_, _>>()?;
    let ring = |k: usize, n: usize, r: f32| {
        let a = k as f32 * (core::f32::consts::TAU / n.max(1) as f32);
        (Vec2::new(cos(a) * r, sin(a) * r), a)
    };
    for (k, d) in designs.iter().enumerate() {
        let (pos, a) = ring(k, designs.len(), 30.0);
        let damage = sim.rng(Stream::Scenario).range_f32(0.2, 0.5);
        let body = Body {
            pos,
            angle: a,
            linvel: Vec2::new(-sin(a) * 0.5, cos(a) * 0.5),
            angvel: 0.05,
        };
        sim.spawn_wreck(Arc::clone(d), scenario_owner(d), body, damage);
    }
    let holds: Vec<&Arc<ShipDesign>> = designs
        .iter()
        .filter(|d| scenario_owner(d) == Owner::PLAYER && d.parts.iter().any(|p| p.cargo > 0.0))
        .collect();
    let mut collectors = Vec::new();
    for (k, d) in holds.iter().enumerate() {
        let (pos, a) = ring(k, holds.len(), 45.0);
        collectors.push(sim.spawn_ship(Arc::clone(d), Owner::PLAYER, Body::at(pos, a)));
    }
    if let Some(raider) = designs.iter().find(|d| scenario_owner(d) != Owner::PLAYER) {
        for k in 0..2 {
            let (pos, a) = ring(k, 2, 100.0);
            let body = Body::at(pos, a + core::f32::consts::PI);
            let id = sim.spawn_ship(Arc::clone(raider), Owner(1), body);
            let behavior = sim.behaviors().default_for(raider);
            sim.command(id, ShipCommand::Behavior(behavior));
        }
    }
    Ok(Scenario {
        sim,
        stats: ScenarioStats::default(),
        kind: ScenarioKind::Salvage,
        inputs: TickInputs::default(),
        collectors,
    })
}

/// Damage that destroys every alive part of `ship` that `which` picks.
fn destroy_parts(
    ship: &Ship,
    which: impl Fn(&crate::design::PartSpec) -> bool,
) -> Vec<DamageCommand> {
    ship.parts()
        .iter()
        .enumerate()
        .filter(|&(i, p)| p.alive && which(&ship.design().parts[i]))
        .map(|(i, _)| DamageCommand {
            ship: ship.id(),
            target: DamageTarget::Part(i as PartIdx),
            amount: 1000.0,
        })
        .collect()
}

impl Scenario {
    /// The salvage scenario's script for this tick: collectors fly to the nearest salvage, and the scripted
    /// losses (the `SALVAGE_*` ticks).
    fn salvage_run(&mut self, tick: u64) {
        let last = self.collectors.len().saturating_sub(1);
        for (n, &c) in self.collectors.iter().enumerate() {
            let brake = Some(crate::maneuver::Maneuver::Brake);
            let target = if n == last && n > 0 {
                match tick < SALVAGE_LOST {
                    true => brake,
                    false => salvage_target(&self.sim, c, self.collectors.first().copied()),
                }
            } else if n == 0 && tick + 90 >= SALVAGE_LOST {
                brake
            } else {
                salvage_target(&self.sim, c, None)
            };
            if (tick + n as u64 * 7).is_multiple_of(SALVAGE_RETARGET)
                && let Some(maneuver) = target
            {
                let fly = ShipCommand::Maneuver {
                    maneuver,
                    fire: false,
                };
                self.inputs.commands.push((c, fly));
            }
        }
        if SALVAGE_RAIDER_KILLS.contains(&tick)
            && let Some(raider) = self.sim.ships().iter().find(|s| s.team() == Some(Owner(1)))
        {
            self.inputs
                .damage
                .extend(destroy_parts(raider, |p| p.logic));
        }
        let collector = |n: usize| self.collectors.get(n).and_then(|&id| self.sim.ship(id));
        if tick == SALVAGE_HOLD_HIT
            && let Some(s) = collector(1)
        {
            let hold = destroy_parts(s, |p| p.cargo > 0.0);
            self.inputs.damage.extend(hold.into_iter().take(1));
        }
        if tick == SALVAGE_LOST
            && let Some(s) = collector(0)
        {
            self.inputs.damage.extend(destroy_parts(s, |p| p.logic));
        }
    }
}

/// Where collector `c` flies: to the nearest salvage piece of ship `prefer` within 150 m, else the nearest loose
/// fragment (to it), else the nearest other salvage piece (close in on it); brake when there is none. `None`
/// when `c` is no vessel.
fn salvage_target(
    sim: &Sim,
    c: crate::ShipId,
    prefer: Option<crate::ShipId>,
) -> Option<crate::maneuver::Maneuver> {
    use crate::maneuver::Maneuver;
    let ship = sim.ship(c)?;
    let at = ship.piece(ship.control_piece()?)?.com_world();
    // Rank: pieces of `prefer`, then loose fragments, then other pieces; nearest first in each.
    let mut best: Option<(u8, f32, Maneuver)> = None;
    let mut consider = |rank: u8, pos: Vec2, m: Maneuver| {
        let d = (pos - at).length_squared();
        if d <= 150.0 * 150.0 && best.is_none_or(|(r, b, _)| (rank, d) < (r, b)) {
            best = Some((rank, d, m));
        }
    };
    for l in sim.loose() {
        consider(1, l.pos, Maneuver::Hold { at: l.pos });
    }
    for s in sim.ships() {
        let rank = if Some(s.id()) == prefer { 0 } else { 2 };
        for p in s.pieces().iter().filter(|p| p.salvage()) {
            let target = Target::Piece(s.id(), p.id);
            let aim = crate::flight::AimMode::Auto;
            consider(
                rank,
                p.com_world(),
                Maneuver::Approach {
                    target,
                    range: 0.0,
                    aim,
                },
            );
        }
    }
    Some(best.map_or(Maneuver::Brake, |(_, _, m)| m))
}

/// Voyage scenario script: every this many ticks the cores of the first armed awake enemy vessel are shot out.
const VOYAGE_KILL_EVERY: u64 = 150;
/// Voyage scenario script: a player ship fights the nearest armed awake enemy within this many metres.
const VOYAGE_FIGHT_RANGE: f32 = 300.0;

/// What a player ship of the voyage scenario does next.
enum Order {
    /// Fight this enemy, which lies this way (world).
    Fight(crate::ShipId, Vec2),
    /// Fly a maneuver (to salvage or to a site).
    Fly(crate::maneuver::Maneuver),
}

/// The voyage scenario: the content's first voyage (load order) rolled from the seed and started for the player
/// against `Owner(1)`, one ship per ship-mind blueprint with a hold in a row at the entry, the first the focus.
fn voyage_scenario(content: &Content, seed: u64) -> Result<Scenario, String> {
    let voyage = content
        .voyages
        .iter()
        .next()
        .ok_or("the voyage scenario needs a voyage")?;
    let plan = crate::voyage::VoyagePlan::roll(content, voyage, seed)?;
    let entry = plan.entry;
    let mut sim = Sim::new(seed);
    sim.set_behaviors(library(content)?);
    sim.start_voyage(plan, Owner::PLAYER, Owner(1));
    let holds: Vec<Arc<ShipDesign>> = content
        .ships
        .iter()
        .map(|b| ShipDesign::compile(content, b).map(Arc::new))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|d| scenario_owner(d) == Owner::PLAYER && d.parts.iter().any(|p| p.cargo > 0.0))
        .collect();
    let mut collectors = Vec::new();
    for (k, d) in holds.iter().enumerate() {
        let x = (k as f32 - (holds.len() as f32 - 1.0) / 2.0) * 25.0;
        let body = Body::at(entry.pos + Vec2::new(x, 0.0), entry.angle);
        collectors.push(sim.spawn_ship(Arc::clone(d), Owner::PLAYER, body));
    }
    if let Some(&first) = collectors.first() {
        sim.command(first, ShipCommand::Focus);
    }
    Ok(Scenario {
        sim,
        stats: ScenarioStats::default(),
        kind: ScenarioKind::Voyage,
        inputs: TickInputs::default(),
        collectors,
    })
}

impl Scenario {
    /// The voyage scenario's script for this tick: the scripted kills, then each player ship fights, salvages or
    /// flies on (re-decided every `SALVAGE_RETARGET` ticks, staggered).
    fn voyage_run(&mut self, tick: u64) {
        let Some(v) = self.sim.voyage() else {
            return;
        };
        // The enemies that matter (GDR-0068): armed and awake.
        let awake = |s: &Ship| self.sim.armed_enemy(Owner::PLAYER, s);
        if tick > 0
            && tick.is_multiple_of(VOYAGE_KILL_EVERY)
            && let Some(enemy) = self.sim.ships().iter().find(|s| awake(s))
        {
            self.inputs.damage.extend(destroy_parts(enemy, |p| p.logic));
        }
        // Decide from the world as it is (fights: the foe and the way to it), then roll the pilot input.
        let mut decided: Vec<(crate::ShipId, Order)> = Vec::new();
        for (n, &c) in self.collectors.iter().enumerate() {
            if !(tick + n as u64 * 7).is_multiple_of(SALVAGE_RETARGET) {
                continue;
            }
            let Some(ship) = self.sim.ship(c) else {
                continue;
            };
            let Some(at) = ship
                .control_piece()
                .and_then(|p| ship.piece(p))
                .map(|p| p.com_world())
            else {
                continue;
            };
            let foe = self
                .sim
                .ships()
                .iter()
                .filter(|s| awake(s))
                .filter_map(|s| {
                    let p = s.piece(s.control_piece()?)?.com_world();
                    Some(((p - at).length(), s.id(), p - at))
                })
                .filter(|&(d, _, _)| d <= VOYAGE_FIGHT_RANGE)
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let room = ship.cargo_capacity() > ship.cargo().used();
            let salvage = salvage_target(&self.sim, c, None)
                .filter(|&m| room && m != crate::maneuver::Maneuver::Brake);
            // Else each ship to a site of its own among those not cleared yet.
            let open: Vec<&crate::Site> = v
                .sites()
                .iter()
                .filter(|s| s.state() != crate::SiteState::Cleared)
                .collect();
            let fly = match open.get(n % open.len().max(1)) {
                Some(s) => crate::maneuver::Maneuver::Hold { at: s.pos() },
                None => crate::maneuver::Maneuver::Brake,
            };
            let plan = match (foe, salvage) {
                (Some((_, foe, way)), _) => Order::Fight(foe, way),
                (None, Some(m)) => Order::Fly(m),
                (None, None) => Order::Fly(fly),
            };
            decided.push((c, plan));
        }
        for (c, plan) in decided {
            let cmd = match plan {
                Order::Fight(foe, way) => {
                    let rng = self.sim.rng(Stream::Scenario);
                    let a = crate::math::atan2(way.y, way.x) + rng.range_f32(-0.8, 0.8);
                    ShipCommand::Pilot(PilotInput {
                        stick: Vec2::new(cos(a), sin(a)) * rng.range_f32(0.3, 1.0),
                        lock: Some(Target::Ship(foe)),
                        drift: rng.chance(0.2),
                        fire: true,
                        aim: crate::flight::AimMode::Auto,
                        ..Default::default()
                    })
                }
                Order::Fly(maneuver) => ShipCommand::Maneuver {
                    maneuver,
                    fire: false,
                },
            };
            self.inputs.commands.push((c, cmd));
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
/// The content's behavior library, with problems as one message.
fn library(content: &Content) -> Result<BehaviorLibrary, String> {
    BehaviorLibrary::new(content).map_err(|p| {
        p.iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

fn assign_behaviors(content: &Content, sim: &mut Sim) -> Result<(), String> {
    let show = |e: Vec<String>| e.join("; ");
    let mut lib = library(content)?;
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
                s.owner() == Owner::PLAYER,
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
            leader = Some(id); // piloted by the scenario's random input: the player's focus
            sim.command(id, ShipCommand::Focus);
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

/// The player's mind capacity in the behavior scenario (logic cores): fewer than its ship-mind ships have, so
/// dormant vessels, waking and the focus are part of the determinism check.
pub const BEHAVIOR_MIND_CAPACITY: u32 = 8;

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
        power: None,
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

const BURNER: &str = "burner";

/// A burner: two thrusters in one column facing each other (their thrust cancels, so it stays put) with reactive
/// blocks between them, framed by steel. Their plumes burn the blocks from both ends, then each other (GDR-0049).
/// A test fixture built from base part ids (skipped if a content pack lacks them).
fn burner_blueprint() -> data::Blueprint {
    let p = |part: &str, x: i32, y: i32, rot: u8, material: Option<&str>| data::Placement {
        part: part.into(),
        x,
        y,
        rot,
        flip: false,
        material: material.map(Into::into),
        power: None,
    };
    let mut parts = vec![
        p("logic_core", 0, 0, 0, None),
        p("fission_reactor", 1, 0, 0, None),
        p("thruster", 1, 1, 0, None),
        p("thruster", 1, 5, 2, None),
    ];
    for y in 2..5 {
        parts.push(p("armor_block", 1, y, 0, Some("reactive")));
    }
    for y in 1..6 {
        parts.push(p("armor_block", 0, y, 0, Some("steel")));
        parts.push(p("armor_block", 2, y, 0, Some("steel")));
    }
    data::Blueprint {
        id: BURNER.into(),
        name: "Burner".into(),
        summary: "Determinism-scenario fixture that burns its own hull.".into(),
        description: None,
        lore: None,
        tags: vec![],
        faction: "ship_mind".into(),
        size: (3, 6),
        parts,
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
/// under the content behavior `raider_basic` (approach, then strafe around the target; fire at will with
/// lead), so they engage at once and produce projectiles, hits and debris. With `n = 1` the lone runner orbits
/// a point and fires into space.
pub fn load_test(n: usize, seed: u64) -> Sim {
    load_test_with(&data::base_content(), n, seed)
        .expect("base content has the runner and the raider")
}

/// [`load_test`] on any content that has `halvex_runner`, `automaton_raider` and the behavior `raider_basic`.
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
    sim.set_behaviors(library(content)?);
    let fight = sim
        .behaviors()
        .id("raider_basic")
        .ok_or("load test needs the behavior `raider_basic`")?;
    let runners = n.div_ceil(2);
    let raiders = n / 2;
    for k in 0..runners {
        let x = (k as f32 - (runners as f32 - 1.0) / 2.0) * 25.0;
        let id = sim.spawn_ship(
            Arc::clone(&runner),
            Owner::PLAYER,
            Body::at(Vec2::new(x, 0.0), 0.0),
        );
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
            sim.command(id, ShipCommand::Behavior(Some(fight)));
        }
    }
    for k in 0..raiders {
        let x = (k as f32 - (raiders as f32 - 1.0) / 2.0) * 20.0;
        let id = sim.spawn_ship(
            Arc::clone(&raider),
            Owner(1),
            Body::at(Vec2::new(x, 80.0), core::f32::consts::PI),
        );
        sim.command(id, ShipCommand::Behavior(Some(fight)));
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
