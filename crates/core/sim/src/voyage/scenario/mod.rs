//! Deterministic scenarios for the determinism check (native vs wasm, later the browser) and for tests. Both
//! spawn two copies of every ship blueprint in the content (load order) but those past the rim ([`DEEP_TAGS`]), so
//! they need no hard-coded ids and grow with the content, and both roll their inputs and hits on the `Scenario` RNG
//! stream.
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
//!   computer, allocation, maneuvers, fire at will. Plus one ion ship (two ion cannons, GDR-0125) in the ring's
//!   centre, so static and blinded behaviors are covered.
//! * [`ScenarioKind::Assists`]: the flight scenario with pilot assists (GDR-0088): every player ship flies its random
//!   input with [`fixtures::ASSISTS`] (avoid collisions, brake, hold station beside an ally, auto-lock, auto-fire),
//!   lets go of the controls more often so the assists have channels, and flips a toggle or ALL ASSISTS now and then.
//!   Its pilots keep their lock as a player does: now and then they drop it (Esc: auto-lock pauses) or press LOCK
//!   TARGET ([`crate::flight::next_lock`]), and while they hold none they take the one auto-lock asks for, as the
//!   client does.
//! * [`ScenarioKind::Behavior`]: every ship under a content behavior (raiders: `raider_basic`; ship-mind
//!   ships: `skirmisher` and three player programs with target priorities, among them an escort following
//!   the piloted leader's lock), Rhai conditions included, plus hits and blasts that knock out logic cores
//!   (rule slots drop). The leader is the player's focus, and the player's mind capacity is
//!   [`BEHAVIOR_MIND_CAPACITY`] cores, so the newest ship-mind vessels start dormant and wake as others fall.
//! * [`ScenarioKind::Salvage`]: salvage (`salvage.rs`): a wreck of every blueprint spawned as a wreck
//!   (`Sim::spawn_wreck`, part of it already gone), the player's ship-mind ships with a hold flying to the
//!   nearest salvage (pieces and loose fragments) and collecting it, one more under the `salvager` behavior
//!   (GDR-0123), two raiders under their content behavior
//!   whose cores are shot out (fragments), a cargo part of one collector destroyed (its hold sheds) and one
//!   collector lost with its cargo (its wreck keeps it; the others strip it), plus random hits.
//! * [`ScenarioKind::Grown`]: the Grown's mechanics (`grown.rs`, GDR-0102): a wary grove of thornbacks (hearts, ram
//!   prows, thorn guns, tissue armour) and their wreck beside a hot star, the player's ships with a hold and a mender
//!   (a heart of its own) on the voyage script without its kills; random hits and blasts, a ram run every
//!   [`grown::GROWN_RAM_EVERY`] ticks: regrowth, healing, prow rams, thorns, star heat and the wary wake rules.
//! * [`ScenarioKind::Voyage`]: the voyage director (`voyage/mod.rs`): the content's first voyage rolled from the seed
//!   and started, one player ship per ship-mind blueprint with a hold entering at its edge. Each flies random
//!   pilot input locked on the nearest armed awake enemy (`Sim::armed_enemy`) within reach of a fight, else to the
//!   nearest salvage, else to a site not yet cleared, so sites spawn, sleep, wake and clear; the cores of an armed
//!   awake enemy are shot out every few seconds (fragments, wrecks to strip), plus random hits.
//!
//! Owners: ship-mind ships belong to [`Owner::PLAYER`], every other faction's to `Owner(1)`
//! ([`scenario_owner`]). Splits leave vessels of their own (GDR-0013), so ship counts grow during a run.
//!
//! The state hash includes the physics (Rapier) state, so the native-vs-wasm check covers the engine too.

mod fixtures;
mod grown;
mod salvage;
mod voyage;

use std::sync::Arc;

use data::Content;

use crate::behavior::{AssistSwitch, BehaviorLibrary};
use crate::ship::breakage::DamageTarget;
use crate::ship::design::{PartIdx, ShipDesign};
use crate::events::SimEvent;
use crate::flight::{PilotInput, Target};
use crate::math::{Vec2, cos, sin};
use crate::rng::Stream;
use crate::combat::salvage::{Item, Salvaged};
use crate::ship::{Body, Ship, ShipControls};
use crate::ship::vessel::Owner;
use crate::world::{DamageCommand, ShipCommand, Sim, TickInputs};

/// Content ships with one of these tags (past the rim: the outer reach's Grown and bastion, the middle band's
/// keeper, the inner band's spire) are left out of the scenarios that spawn every blueprint: the `grown` scenario
/// flies the Grown's mechanics, and two of each would outnumber the player's ships two to one (GDR-0103), so those
/// scenarios run as before them.
pub const DEEP_TAGS: [&str; 3] = ["outer", "middle", "inner"];

/// The blueprints the scenarios spawn "every blueprint" of: the content's in load order, but [`DEEP_TAGS`]'.
pub(super) fn scenario_blueprints(content: &Content) -> impl Iterator<Item = &data::Blueprint> {
    content.ships.iter().filter(|b| !b.tags.iter().any(|t| DEEP_TAGS.contains(&t.as_str())))
}

pub use fixtures::BEHAVIOR_MIND_CAPACITY;
use fixtures::{
    BURNER, HEAT_STRESS, assign_assists, assign_behaviors, burner_blueprint, heat_stress_blueprint, ion_blueprint, library,
    nth_alive,
};
use salvage::{SALVAGE_RETARGET, destroy_parts, salvage_scenario, salvage_target};
use grown::{GROWN_RAM_EVERY, grown_scenario};
use voyage::voyage_scenario;

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
    /// The flight scenario with pilot assists on the player's ships.
    Assists,
    /// The shared load test with this many ships (see [`load_test`]).
    Load(u32),
    /// Behaviors (content and player programs, Rhai conditions, target priorities, slots).
    Behavior,
    /// Salvage: wrecks spawned as wrecks, collecting, fragments, cargo kept by a lost ship.
    Salvage,
    /// A voyage: sites spawning, sleeping, waking and clearing, salvage, the recall check.
    Voyage,
    /// The Grown's mechanics: regrowth, healing, ram prows, thorn guns, a hot star, a wary grove.
    Grown,
}

impl ScenarioKind {
    pub fn from_name(name: &str) -> Option<ScenarioKind> {
        match name {
            "core" => Some(ScenarioKind::Core),
            "flight" => Some(ScenarioKind::Flight),
            "assists" => Some(ScenarioKind::Assists),
            "load" => Some(ScenarioKind::Load(20)),
            "behavior" => Some(ScenarioKind::Behavior),
            "salvage" => Some(ScenarioKind::Salvage),
            "voyage" => Some(ScenarioKind::Voyage),
            "grown" => Some(ScenarioKind::Grown),
            _ => None,
        }
    }

    /// Chance per tick of a random hit on a random alive part.
    fn hit_chance(self) -> f32 {
        match self {
            ScenarioKind::Core => 0.25,
            ScenarioKind::Flight | ScenarioKind::Assists => 0.08,
            ScenarioKind::Behavior => 0.06,
            ScenarioKind::Salvage => 0.04,
            ScenarioKind::Voyage => 0.03,
            ScenarioKind::Grown => 0.05,
            ScenarioKind::Load(_) => 0.0,
        }
    }

    /// Every this many ticks a heavy blast hits a part and all its neighbours (provokes splits).
    fn blast_every(self) -> u64 {
        match self {
            ScenarioKind::Core => 75,
            ScenarioKind::Flight | ScenarioKind::Assists | ScenarioKind::Behavior => 150,
            ScenarioKind::Salvage | ScenarioKind::Voyage | ScenarioKind::Grown => 300,
            ScenarioKind::Load(_) => u64::MAX,
        }
    }

    /// Inputs of each ship are re-rolled every this many ticks (staggered per ship).
    fn reroll_every(self) -> u64 {
        match self {
            ScenarioKind::Core => 20,
            ScenarioKind::Flight | ScenarioKind::Assists | ScenarioKind::Behavior => 45,
            ScenarioKind::Salvage | ScenarioKind::Voyage | ScenarioKind::Grown => SALVAGE_RETARGET,
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
    /// Chain reactions (`SimEvent::Explosion`, GDR-0084).
    pub explosions: u32,
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
    /// Parts re-grown by hearts (`PartRegrown`, GDR-0102).
    pub regrown: u32,
    /// Asteroid rocks spawned so far and shots that struck one (`RockHit`, GDR-0104).
    pub rocks: u32,
    pub rock_hits: u32,
    /// Vessels blinded by static (`Blinded`, disruption, GDR-0125).
    pub blinded: u32,
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

    /// Two ships of every blueprint in `content` ([`scenario_blueprints`]), placed on a circle, spinning a little.
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
        if kind == ScenarioKind::Grown {
            return grown_scenario(content, seed);
        }
        let mut config = crate::SimConfig::default();
        if kind == ScenarioKind::Behavior {
            config.mind_capacity[Owner::PLAYER.0 as usize] = BEHAVIOR_MIND_CAPACITY;
        }
        let mut sim = Sim::with_config(seed, config);
        let designs: Vec<Arc<ShipDesign>> = scenario_blueprints(content)
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
        let mut ion = None;
        if (kind == ScenarioKind::Flight || kind == ScenarioKind::Assists)
            && let Ok(d) = ShipDesign::compile(content, &ion_blueprint())
        {
            // A player ship with ion cannons (GDR-0125) just outside the ring by the first raider (else the first
            // enemy), its nose on it, under the ship-mind's default behavior (it closes in and fires at will): its
            // static blinds the automatons' behaviors now and then.
            let enemies = || sim.ships().iter().filter(|s| s.owner() != Owner::PLAYER);
            let near = enemies()
                .find(|s| s.design().id == "automaton_raider")
                .or_else(|| enemies().next())
                .and_then(|s| s.pieces().first());
            let at = near.map_or(Vec2::ZERO, |p| p.body.pos);
            let u = at * (1.0 / at.length().max(1e-3));
            let body = Body::at(at + u * 22.0, crate::math::atan2(u.x, -u.y));
            ion = Some(sim.spawn_ship(Arc::new(d), Owner::PLAYER, body));
        }
        if kind == ScenarioKind::Flight || kind == ScenarioKind::Assists {
            sim.set_behaviors(library(content)?);
            if let Some(id) = ion {
                let b = sim.ship(id).and_then(|s| sim.behaviors().default_for(s.design()));
                sim.command(id, ShipCommand::Behavior(b));
            }
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
        if kind == ScenarioKind::Assists {
            assign_assists(&mut sim)?;
        }
        if kind == ScenarioKind::Core
            && let Ok(d) = ShipDesign::compile(content, &heat_stress_blueprint())
        {
            // Away from the ring (like the burner), so the base ships' fire (the lancers' homing rockets, GDR-0115)
            // cannot take it apart before it overheats.
            sim.spawn_ship(
                Arc::new(d),
                Owner::PLAYER,
                Body::at(Vec2::new(300.0, 0.0), 0.0),
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
                SimEvent::RockHit { .. } => self.stats.rock_hits += 1,
                SimEvent::Impact(c) => {
                    self.stats.impacts += 1;
                    self.stats.rams += (c.damage > 0.0) as u32;
                }
                SimEvent::PlumeHit { .. } => self.stats.burns += 1,
                SimEvent::Explosion { .. } => self.stats.explosions += 1,
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
                SimEvent::PartRegrown { .. } => self.stats.regrown += 1,
                SimEvent::Blinded { .. } => self.stats.blinded += 1,
                _ => {}
            }
        }
        if let Some(l) = self.sim.loose().last() {
            self.stats.fragments = self.stats.fragments.max(l.id.0 + 1);
        }
        self.stats.rocks = self.sim.voyage().map_or(0, |v| v.rocks().len() as u32);
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
        if matches!(self.kind, ScenarioKind::Voyage | ScenarioKind::Grown) && self.sim.can_recall(Owner::PLAYER) {
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
        if kind == ScenarioKind::Voyage || kind == ScenarioKind::Grown {
            self.voyage_run(tick);
        }
        if kind == ScenarioKind::Grown && tick % GROWN_RAM_EVERY == GROWN_RAM_EVERY / 2 {
            self.ram_run(tick / GROWN_RAM_EVERY);
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
            let locks = (kind == ScenarioKind::Assists).then(|| self.pilot_locks(id));
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
                ScenarioKind::Flight | ScenarioKind::Assists | ScenarioKind::Behavior | ScenarioKind::Load(_) => {
                    // With assists the pilot lets go more often and fires less, so the assists get channels.
                    let assisted = kind == ScenarioKind::Assists;
                    let a = rng.range_f32(-core::f32::consts::PI, core::f32::consts::PI);
                    let len = if rng.chance(if assisted { 0.5 } else { 0.2 }) {
                        0.0
                    } else {
                        rng.range_f32(0.2, 1.0)
                    };
                    let lock = match locks {
                        // A pilot with assists: Esc, LOCK TARGET, or the lock held (else the one auto-lock asks for).
                        Some((held, auto, tab)) => match rng.below(8) {
                            0 => None,
                            1 => tab,
                            _ => held.or(auto),
                        },
                        None if !enemies.is_empty() && rng.chance(0.5) => Some(Target::Ship(
                            enemies[rng.below(enemies.len() as u32) as usize],
                        )),
                        None => None,
                    };
                    let input = PilotInput {
                        stick: Vec2::new(cos(a), sin(a)) * len,
                        lock,
                        drift: rng.chance(0.2),
                        fire: if rng.chance(if assisted { 0.3 } else { 0.7 }) { crate::flight::FIRE_ALL } else { 0 },
                        aim: crate::flight::AimMode::Auto,
                        ..Default::default()
                    };
                    self.inputs.commands.push((id, ShipCommand::Pilot(input)));
                    if assisted && rng.chance(0.15) {
                        let switch = match rng.below(5) {
                            0 => AssistSwitch::All,
                            k => AssistSwitch::Toggle(k as u8),
                        };
                        self.inputs.commands.push((id, ShipCommand::SwitchAssists(switch)));
                    }
                }
                // The collectors fly to salvage (`salvage_run`) or through the voyage (`voyage_run`), enemies
                // their behavior.
                ScenarioKind::Salvage | ScenarioKind::Voyage | ScenarioKind::Grown => {}
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

impl Scenario {
    /// The locks an assists-scenario pilot chooses from, as the client sees them: the lock it holds (dropped once its
    /// ship is no longer active, as the client drops it), the one its auto-lock asks for, and LOCK TARGET's.
    fn pilot_locks(&self, id: crate::ShipId) -> (Option<Target>, Option<Target>, Option<Target>) {
        let ships = self.sim.ships();
        let Some(ship) = self.sim.ship(id) else {
            return (None, None, None);
        };
        let held = match ship.helm {
            crate::flight::HelmMode::Pilot(p) => p.lock,
            _ => None,
        }
        .filter(|t| matches!(t, Target::Ship(s) if self.sim.ship(*s).is_some_and(Ship::is_active)));
        let held_ship = match held {
            Some(Target::Ship(s)) => Some(s),
            _ => None,
        };
        let auto = ship.assists().and_then(|a| a.auto_lock()).map(Target::Ship);
        (held, auto, crate::flight::next_lock(ships, id, held_ship))
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
                    .find(|t| t.slot != crate::ship::design::NO_SLOT && t.plume_length > 0.0)?;
                let lane = s.design().engines[row.slot as usize].lanes[0];
                let origin = s.design().plume_lanes.get(lane as usize)?.origin;
                let local = crate::ship::mass::to_piece(origin, piece.origin);
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
            let maneuver = crate::flight::maneuver::Maneuver::Orbit {
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
