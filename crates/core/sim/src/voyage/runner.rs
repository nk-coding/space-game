//! The headless voyage runner (track 7K's balance tool, `docs/sim.md` "The headless voyage runner"): flies a
//! content voyage with one ship (and escorts) as a simple player would, a tick at a time, and reports whether the
//! goal was cleared, the time, the hull lost and the haul. The `voyage` example prints it over many seeds; the
//! arena scores designs with it (`arena::voyage`, the evolution lab's voyage fitness).
//!
//! The pilot (a script, not a player), every [`DECIDE`] ticks, in this order:
//! - **Fight** an armed awake enemy (`Sim::armed_enemy`, GDR-0068) within 400 m, or the nearest such ship of the
//!   awake site it is heading for. It keeps its lock on one enemy while that one is armed and near (else the
//!   nearest), as the balance study's *closing* stand-in (`docs/results/balance-2026-10-13.md`): locked on it,
//!   firing, the stick towards it while it is farther than 75 % of its shortest clear gun's reach, eased off over
//!   the last 40 m and never more than 60 % of the top speed (a ship without forward thrust cannot brake with its
//!   nose on the target). With a fight behavior ([`Flagship::behavior`]) the ship flies that behavior while there
//!   is a fight.
//! - **Finish off** a disarmed awake enemy within 200 m (it rams, and its wreck is salvage) while the hold has room,
//!   giving up on one after 20 s.
//! - **Strip** salvage within 250 m that fits its hold and drifts slower than 6 m/s, the best value per cargo unit
//!   first (as the strip order counts it), a point of value worth 50 m of the way; it gives up on a piece after 20 s
//!   without a part.
//! - **Fly** to the nearest site still to do (a site with ships until it is cleared, a wreck site until it has been
//!   there), the goal last. A site whose ships sleep: it stops 40 m outside the wake radius, then creeps in at a
//!   quarter of its top speed, so the pack wakes in front of a slow ship.
//!
//! It goes home as soon as the sim allows (`Sim::can_recall`, running from the nearest enemy until then) once the
//! goal is cleared and every site is done with nothing left in reach to strip (or the hold is full), or when it is
//! disarmed (`Ship::is_armed`: no gun or no power source left; a gun that overheats or idles still counts), or after
//! 3 minutes without progress (no kill, site, salvage or new site). A run ends recalled, lost (the flagship is no
//! vessel any more) or out of time.
//!
//! Escorts start behind the flagship and fly the content's `escort` behavior with the flagship as their leader
//! (they fight its lock); their holds count for the haul. Deterministic like the sim: the same setup and seed give
//! the same run on every platform.

use std::collections::BTreeSet;
use std::sync::Arc;

use data::career::{Haul, HaulItem};

use crate::behavior::{BehaviorId, BehaviorLibrary};
use crate::config::SimConfig;
use crate::events::SimEvent;
use crate::flight::{AimMode, FIRE_ALL, PilotInput, Target};
use crate::ids::{PieceId, ShipId};
use crate::flight::maneuver::Maneuver;
use crate::math::Vec2;
use crate::report::DesignReport;
use crate::combat::salvage::Item;
use crate::ship::vessel::Owner;
use crate::voyage::{Site, SiteState, VoyagePlan};
use crate::world::{ShipCommand, Sim, TickInputs};
use crate::ship::design::ShipDesign;

/// The owner of the voyage's enemies.
pub const ENEMY: Owner = Owner(1);
/// The pilot decides every this many ticks.
pub const DECIDE: u64 = 10;
/// It fights awake enemies within this many metres.
const FIGHT_RANGE: f32 = 400.0;
/// It strips salvage within this many metres ...
const SALVAGE_RANGE: f32 = 250.0;
/// ... and gives up on a piece after this many ticks without a part from it.
const GIVE_UP: u64 = 600;
/// A point of value per cargo unit is worth this many metres of the way to the salvage.
const WORTH_PER_M: f32 = 50.0;
/// Salvage drifting faster than this (m/s) is not worth the chase.
const CATCHABLE: f32 = 6.0;
/// A wreck site counts as done once it came this close.
const VISITED: f32 = 60.0;
/// It closes to this share of its shortest clear gun's reach (so every gun can fire).
pub const CLOSE_TO: f32 = 0.75;
/// Escorts start this many metres behind and beside the flagship (per row).
const ESCORT_GAP: f32 = 25.0;
/// It gives up after this many ticks without progress (3 min).
const STALEMATE: u64 = 5400;
/// In a fight the stick eases off over this many metres before the stand-off ...
const EASE: f32 = 40.0;
/// ... and is never longer than this (a share of the top speed).
const CLOSE_STICK: f32 = 0.6;
/// It finishes off disarmed awake enemies within this many metres (when its hold has room).
const MOP_RANGE: f32 = 200.0;
/// It stops this many metres outside a sleeping site's wake radius ...
const STAGE: f32 = 40.0;
/// ... then creeps in with this share of the top speed, braking first if it is faster than this (m/s).
const CREEP: f32 = 0.25;
const CREEP_BRAKE: f32 = 7.5;

/// A salvage piece: its ship and piece.
type PieceKey = (ShipId, PieceId);

/// The ship that flies the voyage.
#[derive(Clone)]
pub struct Flagship {
    pub design: Arc<ShipDesign>,
    /// The closing pilot stands off at [`CLOSE_TO`] of this (m): its shortest clear gun's reach
    /// ([`closing_reach`]).
    pub reach: f32,
    /// Flies this behavior (of the library the runner gets) in fights; `None`: the closing pilot.
    pub behavior: Option<BehaviorId>,
}

/// The shortest reach of a design's clear guns (m), the closing pilot's yardstick; infinite without one.
pub fn closing_reach(report: &DesignReport) -> f32 {
    report
        .guns
        .iter()
        .filter(|g| g.clear)
        .map(|g| g.range)
        .fold(f32::INFINITY, f32::min)
}

/// How a run ended.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Outcome {
    /// It warped home (the goal cleared or not: see [`VoyageRun::goal`]).
    Recalled,
    /// The flagship is no vessel any more.
    Lost,
    OutOfTime,
}

/// What a run did.
#[derive(Clone, Debug)]
pub struct VoyageRun {
    pub outcome: Outcome,
    /// The voyage's goal site was cleared (whether or not the ship came home).
    pub goal: bool,
    pub ticks: u64,
    /// Hit points the flagship lost (parts destroyed or broken off its controlling piece), of `hp_total`.
    pub hp_lost: f32,
    pub hp_total: f32,
    pub sites_cleared: usize,
    pub sites: usize,
    /// Sites that rolled ships (the ones that can be cleared).
    pub sites_with_ships: usize,
    pub kills: u32,
    /// Awake enemies at the end (`Sim::awake_enemy`), and how many of them are disarmed (they no longer block
    /// clearing or the recall, GDR-0068).
    pub left: (usize, usize),
    /// What the fleet carries at the end (a lost flagship brings nothing home).
    pub haul: Haul,
    pub items: u32,
}

impl VoyageRun {
    /// The goal cleared and the ship home.
    pub fn cleared_and_home(&self) -> bool {
        self.goal && self.outcome == Outcome::Recalled
    }

    /// Share of the flagship's hit points lost, 0..1 (all of them when it was lost).
    pub fn hull_lost_share(&self) -> f32 {
        if self.outcome == Outcome::Lost {
            1.0
        } else if self.hp_total > 0.0 {
            (self.hp_lost / self.hp_total).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// The pilot's memory: the salvage piece it works on, since when, and the ones it gave up on; the sites it has
/// been to; the last tick something happened (a kill, a site woken or cleared, salvage, a new site reached).
#[derive(Default)]
struct Pilot {
    salvage: Option<(PieceKey, u64)>,
    given_up: BTreeSet<PieceKey>,
    visited: BTreeSet<usize>,
    flying_behavior: bool,
    progress: u64,
    /// The armed enemy it has locked.
    lock: Option<ShipId>,
    /// The disarmed enemy it hunts, since when, and the ones it gave up on.
    mopping: Option<(ShipId, u64)>,
    spared: BTreeSet<ShipId>,
}

/// A voyage being flown. Build it with [`VoyageRunner::new`], then call [`VoyageRunner::step`] until it returns
/// the run: all at once ([`VoyageRunner::run`]) or a slice at a time.
pub struct VoyageRunner {
    sim: Sim,
    me: ShipId,
    wing: Vec<ShipId>,
    flagship: Flagship,
    max_ticks: u64,
    pilot: Pilot,
    kills: u32,
    idle: TickInputs,
    result: Option<VoyageRun>,
}

fn at(sim: &Sim, ship: ShipId) -> Option<Vec2> {
    let s = sim.ship(ship)?;
    Some(s.piece(s.control_piece()?)?.com_world())
}

/// Armed awake enemies (`Sim::armed_enemy`): `(distance, ship, position)`.
fn armed_enemies(sim: &Sim, from: Vec2) -> Vec<(f32, ShipId, Vec2)> {
    sim.ships()
        .iter()
        .filter(|s| sim.armed_enemy(Owner::PLAYER, s))
        .filter_map(|s| {
            let p = at(sim, s.id())?;
            Some(((p - from).length(), s.id(), p))
        })
        .collect()
}

/// The nearest salvage within reach of the run that fits `free` units and was not given up: a loose fragment
/// (a point) or a salvage piece.
fn salvage_near(
    sim: &Sim,
    from: Vec2,
    free: u32,
    pilot: &Pilot,
) -> Option<(Option<PieceKey>, Vec2)> {
    // Worth: the best value per cargo unit it would get there (as the strip order counts it), less a point per
    // `WORTH_PER_M` metres of the way: a player goes for the machines on a wreck before its armour plates.
    let mut best: Option<(f32, Option<PieceKey>, Vec2)> = None;
    let mut consider = |key, pos: Vec2, value: f32| {
        let d = (pos - from).length();
        let score = value - d / WORTH_PER_M;
        if d <= SALVAGE_RANGE && best.is_none_or(|(b, _, _)| score > b) {
            best = Some((score, key, pos));
        }
    };
    if free > 0 {
        for l in sim.loose().iter().filter(|l| l.vel.length() <= CATCHABLE) {
            consider(None, l.pos, l.value);
        }
    }
    for s in sim.ships() {
        for p in s
            .pieces()
            .iter()
            .filter(|p| p.salvage() && p.body.linvel.length() <= CATCHABLE)
        {
            let key = (s.id(), p.id);
            let value = p
                .parts
                .iter()
                .filter_map(|&i| {
                    let spec = &s.design().parts[i as usize];
                    (s.parts()[i as usize].alive && spec.salvageable && spec.cargo_units <= free)
                        .then(|| spec.build_points / spec.cargo_units.max(1) as f32)
                })
                .fold(None, |b: Option<f32>, v| Some(b.map_or(v, |b| b.max(v))));
            if let Some(value) = value.filter(|_| !pilot.given_up.contains(&key)) {
                consider(Some(key), p.com_world(), value);
            }
        }
    }
    best.map(|(_, k, p)| (k, p))
}

impl VoyageRunner {
    /// Rolls the voyage from `seed` (the world seed too), starts it for the player against [`ENEMY`] and puts the
    /// flagship at the entry, the escorts behind it. `behaviors` is the library the world flies with: the
    /// content's, plus the flagship's fight behavior if it is not one of them; it needs the content's `escort`
    /// behavior when there are escorts. Stops after `max_ticks`.
    pub fn new(
        content: &data::Content,
        voyage: &data::Voyage,
        flagship: Flagship,
        behaviors: BehaviorLibrary,
        escorts: &[Arc<ShipDesign>],
        seed: u64,
        max_ticks: u64,
    ) -> Result<VoyageRunner, String> {
        let escort_behavior = match escorts {
            [] => None,
            _ => Some(
                behaviors
                    .id("escort")
                    .ok_or("the behaviors have no `escort` for the escorts")?,
            ),
        };
        let mut sim = Sim::with_config(seed, SimConfig::default());
        sim.set_behaviors(behaviors);
        let plan = VoyagePlan::roll(content, voyage, seed)?;
        let entry = plan.entry;
        sim.start_voyage(plan, Owner::PLAYER, ENEMY);
        let me = sim.spawn_ship(Arc::clone(&flagship.design), Owner::PLAYER, entry);
        if flagship.behavior.is_none() {
            sim.command(me, ShipCommand::Focus);
        }
        // Escorts (a fleet, GDR-0065): behind the flagship, left and right in turn, under the `escort` template,
        // following the flagship's lock.
        let mut wing = Vec::new();
        for (k, d) in escorts.iter().enumerate() {
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let row = (k / 2 + 1) as f32;
            let mut body = entry;
            body.pos = entry.pos + Vec2::new(side * ESCORT_GAP * row, -ESCORT_GAP * row);
            let id = sim.spawn_ship(Arc::clone(d), Owner::PLAYER, body);
            sim.command(id, ShipCommand::Leader(Some(me)));
            sim.command(id, ShipCommand::Behavior(escort_behavior));
            wing.push(id);
        }
        Ok(VoyageRunner {
            sim,
            me,
            wing,
            flagship,
            max_ticks,
            pilot: Pilot::default(),
            kills: 0,
            idle: TickInputs::default(),
            result: None,
        })
    }

    /// Runs up to `ticks` more ticks; stops early when the run ends. `Some` once it has ended (later calls step
    /// nothing and return the same run).
    pub fn step(&mut self, ticks: u64) -> Option<&VoyageRun> {
        for _ in 0..ticks {
            if self.result.is_some() {
                break;
            }
            if let Some(outcome) = self.tick() {
                self.result = Some(self.finish(outcome));
            }
        }
        self.result.as_ref()
    }

    /// Runs the voyage to its end.
    pub fn run(mut self) -> VoyageRun {
        self.step(u64::MAX);
        self.result.expect("a run ends by its time limit")
    }

    /// The world, for drawing or inspecting the run.
    pub fn sim(&self) -> &Sim {
        &self.sim
    }

    /// One tick: the end checks, the pilot's decision (every [`DECIDE`] ticks), the sim step and its events.
    /// `Some` when the run ends before the step.
    fn tick(&mut self) -> Option<Outcome> {
        if self.sim.tick() >= self.max_ticks {
            return Some(Outcome::OutOfTime);
        }
        let me = self.me;
        let here = at(&self.sim, me).filter(|_| self.sim.ship(me).is_some_and(|s| s.is_vessel()));
        let Some(here) = here else {
            return Some(Outcome::Lost);
        };
        if self.sim.tick().is_multiple_of(DECIDE) && self.decide(here) {
            return Some(Outcome::Recalled);
        }
        self.sim.step(&self.idle);
        let pilot = &mut self.pilot;
        for e in self.sim.events() {
            match e {
                SimEvent::VesselLost { owner, .. } if *owner == ENEMY => {
                    self.kills += 1;
                    pilot.progress = self.sim.tick();
                }
                SimEvent::SiteWoke { .. } | SimEvent::SiteCleared { .. } => {
                    pilot.progress = self.sim.tick()
                }
                SimEvent::Collected { ship, .. } if *ship == me => {
                    // A part came: the piece it works on is still worth it.
                    pilot.progress = self.sim.tick();
                    if let Some((_, since)) = &mut pilot.salvage {
                        *since = self.sim.tick();
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The pilot's decision; true when it recalls.
    fn decide(&mut self, here: Vec2) -> bool {
        let me = self.me;
        let sim = &mut self.sim;
        let pilot = &mut self.pilot;
        let ship = sim.ship(me).unwrap();
        let free = ship.cargo_capacity().saturating_sub(ship.cargo().used());
        // Disarmed, or nothing happened for a while (an enemy it cannot catch): give up.
        let fighting = sim.tick() - pilot.progress < STALEMATE && ship.is_armed();
        let v = sim.voyage().unwrap();
        for (k, s) in v.sites().iter().enumerate() {
            if (s.pos() - here).length() <= VISITED && pilot.visited.insert(k) {
                pilot.progress = sim.tick();
            }
        }
        let done = |k: usize, s: &Site| match s.state() {
            SiteState::Cleared => true,
            SiteState::Quiet => pilot.visited.contains(&k),
            _ => false,
        };
        let todo: Vec<(usize, Vec2, bool, SiteState)> = v
            .sites()
            .iter()
            .enumerate()
            .filter(|&(k, s)| !done(k, s))
            .map(|(k, s)| (k, s.pos(), s.goal(), s.state()))
            .collect();
        // The goal last, unless it is all that is left.
        let next = todo
            .iter()
            .filter(|t| !t.2 || todo.len() == 1)
            .min_by(|a, b| (a.1 - here).length().total_cmp(&(b.1 - here).length()))
            .copied();
        let goal = v.goal_cleared();
        let enemies = armed_enemies(sim, here);
        // It keeps its lock on one enemy while that one is armed and near (a player does not switch the lock to
        // whichever raider is nearest this second: every switch costs a turn), else the nearest.
        let foe = enemies
            .iter()
            .find(|e| Some(e.1) == pilot.lock && e.0 <= FIGHT_RANGE)
            .or_else(|| {
                enemies
                    .iter()
                    .filter(|e| e.0 <= FIGHT_RANGE)
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            })
            .or_else(|| {
                // The site it heads for is awake: hunt down its last ships wherever they are.
                let (k, ..) = next.filter(|n| n.3 == SiteState::Awake)?;
                let site = &v.sites()[k];
                enemies
                    .iter()
                    .filter(|e| site.ships().contains(&e.1))
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            })
            .copied();
        pilot.lock = foe.map(|f| f.1);
        // Done (or full), or given up: home as soon as the sim allows.
        // (Done: the goal cleared, every site done and nothing left in reach to strip.)
        let done = todo.is_empty() && salvage_near(sim, here, free, pilot).is_none();
        if ((goal && (done || free == 0)) || !fighting) && sim.can_recall(Owner::PLAYER) {
            return true;
        }
        // A disarmed awake enemy near by (it rams, and its wreck is salvage): finish it off while there is room
        // in the hold.
        let mop = (free > 0 && fighting)
            .then(|| {
                sim.ships()
                    .iter()
                    .filter(|s| sim.awake_enemy(Owner::PLAYER, s) && !s.is_armed())
                    .filter(|s| !pilot.spared.contains(&s.id()))
                    .filter_map(|s| {
                        let p = at(sim, s.id())?;
                        Some(((p - here).length(), s.id(), p))
                    })
                    .filter(|e| e.0 <= MOP_RANGE)
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            })
            .flatten();
        if let Some((_, foe, _)) = foe.filter(|_| !fighting) {
            // Nothing left to shoot with: run, and recall once clear.
            let maneuver = Maneuver::Flee {
                from: Target::Ship(foe),
            };
            fly(sim, me, maneuver, pilot);
        } else if let Some((_, foe, foe_at)) = foe.or_else(|| {
            // Hunting a disarmed one: give up on it after a time without the kill.
            let (_, id, _) = mop?;
            let tick = sim.tick();
            match pilot.mopping {
                Some((cur, since)) if cur == id && tick - since > GIVE_UP => {
                    pilot.spared.insert(id);
                    pilot.mopping = None;
                    return None;
                }
                Some((cur, _)) if cur == id => {}
                _ => pilot.mopping = Some((id, tick)),
            }
            mop
        }) {
            match self.flagship.behavior {
                Some(b) => {
                    if !pilot.flying_behavior {
                        sim.command(me, ShipCommand::Behavior(Some(b)));
                        pilot.flying_behavior = true;
                    }
                }
                None => {
                    // The closing stand-in of the balance study (lock, fire, the stick towards the target when it
                    // is out of reach), with the stick eased off as the gap closes, so it does not fly through
                    // the pack at top speed (a ship without forward thrust cannot brake with its nose on the
                    // target).
                    let way = foe_at - here;
                    let gap = way.length() - self.flagship.reach * CLOSE_TO;
                    let stick = if gap > 0.0 {
                        way * ((gap / EASE).min(CLOSE_STICK) / way.length())
                    } else {
                        Vec2::ZERO
                    };
                    let input = PilotInput {
                        stick,
                        lock: Some(Target::Ship(foe)),
                        drift: false,
                        fire: FIRE_ALL,
                        aim: AimMode::Auto,
                        ..Default::default()
                    };
                    pilot_input(sim, me, input, pilot);
                }
            }
        } else if let Some((key, pos)) = salvage_near(sim, here, free, pilot) {
            let tick = sim.tick();
            match (key, pilot.salvage) {
                (Some(k), Some((cur, since))) if k == cur => {
                    if tick - since > GIVE_UP {
                        pilot.given_up.insert(k);
                        pilot.salvage = None;
                    }
                }
                (Some(k), _) => pilot.salvage = Some((k, tick)),
                (None, _) => pilot.salvage = None,
            }
            let maneuver = match key {
                Some((s, p)) => Maneuver::Approach {
                    target: Target::Piece(s, p),
                    range: 0.0,
                    aim: AimMode::Auto,
                },
                None => Maneuver::Hold { at: pos },
            };
            fly(sim, me, maneuver, pilot);
        } else if let Some((k, pos, ..)) = next {
            // A site whose ships still sleep: stop short of its wake radius, then creep in, so the pack wakes in
            // front of a slow ship rather than one passing through at top speed.
            let site = &sim.voyage().unwrap().sites()[k];
            let sleeping = matches!(site.state(), SiteState::Unspawned | SiteState::Asleep)
                && !site.plan().ships.is_empty();
            let way = pos - here;
            let dist = way.length();
            let stage = site.wake_radius() + STAGE;
            let speed = sim
                .ship(me)
                .and_then(|s| s.piece(s.control_piece()?))
                .map_or(0.0, |p| p.body.linvel.length());
            if sleeping && dist > stage + VISITED {
                let at = pos - way * (stage / dist);
                fly(sim, me, Maneuver::Hold { at }, pilot);
            } else if sleeping && speed > CREEP_BRAKE && dist > stage - VISITED {
                fly(sim, me, Maneuver::Brake, pilot);
            } else if sleeping {
                let input = PilotInput {
                    stick: way * (CREEP / dist),
                    drift: false,
                    ..Default::default()
                };
                pilot_input(sim, me, input, pilot);
            } else {
                fly(sim, me, Maneuver::Hold { at: pos }, pilot);
            }
        } else {
            fly(sim, me, Maneuver::Brake, pilot);
        }
        false
    }

    /// The run's numbers at its end.
    fn finish(&self, outcome: Outcome) -> VoyageRun {
        let sim = &self.sim;
        let v = sim.voyage().unwrap();
        let left: Vec<bool> = sim
            .ships()
            .iter()
            .filter(|s| sim.awake_enemy(Owner::PLAYER, s))
            .map(|s| s.is_armed())
            .collect();
        let left = (left.len(), left.iter().filter(|&&armed| !armed).count());
        let ship = sim.ship(self.me).unwrap();
        let alive_hp: f32 = ship
            .control_piece()
            .and_then(|c| ship.piece(c))
            .map_or(0.0, |p| {
                p.parts.iter().map(|&i| ship.parts()[i as usize].hp).sum()
            });
        // The haul: the cargo of every player vessel (the flagship and its escorts; GDR-0072); a lost flagship
        // ends the run with nothing.
        let mut haul = Haul::default();
        if outcome != Outcome::Lost {
            for s in sim
                .ships()
                .iter()
                .filter(|s| s.id() == self.me || (self.wing.contains(&s.id()) && s.is_vessel()))
            {
                for (item, stack) in s.cargo().items() {
                    let item = match item {
                        Item::Part { part, material } => HaulItem::Part {
                            part: part.clone(),
                            material: material.clone(),
                        },
                        Item::Fragment { part } => HaulItem::Fragment { part: part.clone() },
                    };
                    haul.add(item, stack.count);
                }
            }
        }
        let items = haul.items.values().sum();
        let design = &self.flagship.design;
        VoyageRun {
            outcome,
            goal: v.goal_cleared(),
            ticks: sim.tick(),
            hp_lost: design.hp_total - alive_hp,
            hp_total: design.hp_total,
            sites_cleared: v
                .sites()
                .iter()
                .filter(|s| s.state() == SiteState::Cleared)
                .count(),
            sites: v.sites().len(),
            sites_with_ships: v
                .sites()
                .iter()
                .filter(|s| !s.plan().ships.is_empty())
                .count(),
            kills: self.kills,
            left,
            haul,
            items,
        }
    }
}

/// Flies a maneuver (leaving the fight behavior if it flew one).
fn fly(sim: &mut Sim, ship: ShipId, maneuver: Maneuver, pilot: &mut Pilot) {
    if pilot.flying_behavior {
        sim.command(ship, ShipCommand::Behavior(None));
        pilot.flying_behavior = false;
    }
    sim.command(
        ship,
        ShipCommand::Maneuver {
            maneuver,
            fire: false,
        },
    );
}

/// Flies pilot input (leaving the fight behavior if it flew one).
fn pilot_input(sim: &mut Sim, ship: ShipId, input: PilotInput, pilot: &mut Pilot) {
    if pilot.flying_behavior {
        sim.command(ship, ShipCommand::Behavior(None));
        pilot.flying_behavior = false;
    }
    sim.command(ship, ShipCommand::Pilot(input));
}
