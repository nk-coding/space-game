//! The headless voyage runner (track 7K's balance tool, `docs/sim.md` "Voyages"): flies a content voyage with one
//! ship (and escorts) over many seeds, as a simple player would, and reports per seed whether the goal was cleared,
//! the time, the hull lost, and the haul (items, and research data per family as the career unloads it), then a
//! summary.
//!
//! `cargo run --release -p sim --example voyage -- --voyage yard_approach --design rim_skiff --seeds 1..20`
//! `[--escort rim_skiff[,...]] [--behavior skirmisher] [--minutes 30]`
//!
//! `--design` and `--escort` take a content ship id or a blueprint file (`*.ron`, a saved or exported drydock
//! design: a ship a player would build that is not content). Escorts start behind the flagship and fly the
//! `escort` template with the flagship as their leader (they fight its lock); their holds count for the haul.
//!
//! The pilot (a script, not a player), every 10 ticks, in this order:
//! - **Fight** an armed awake enemy (`Sim::armed_enemy`, GDR-0068) within 400 m, or the nearest such ship of the
//!   awake site it is heading for. It keeps its lock on one enemy while that one is armed and near (else the
//!   nearest), as the balance study's *closing* stand-in (`docs/results/balance-2026-10-13.md`): locked on it,
//!   firing, the stick towards it while it is farther than 75 % of its shortest clear gun's reach, eased off over
//!   the last 40 m and never more than 60 % of the top speed (a ship without forward thrust cannot brake with its
//!   nose on the target). With `--behavior` the ship flies that content behavior while there is a fight.
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
//! 3 minutes without progress (no kill, site, salvage or new site). A seed ends recalled, lost (the flagship is no
//! vessel any more) or out of time. Native only; the wall clock times the steps (median and p95), never the sim.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use data::career::{Haul, HaulItem, unload};
use sim::flight::AimMode;
use sim::{
    BehaviorLibrary, Item, Maneuver, Owner, PieceId, PilotInput, ShipCommand, ShipDesign, ShipId,
    Sim, SimConfig, SimEvent, SiteState, Target, TickInputs, Vec2, VoyagePlan,
};

const ENEMY: Owner = Owner(1);
/// The pilot decides every this many ticks.
const DECIDE: u64 = 10;
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
const CLOSE_TO: f32 = 0.75;
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

struct Args {
    voyage: String,
    design: String,
    seeds: Vec<u64>,
    behavior: Option<String>,
    escorts: Vec<String>,
    minutes: u64,
}

fn args() -> Args {
    let mut a = Args {
        voyage: "yard_approach".into(),
        design: "rim_skiff".into(),
        seeds: (1..=20).collect(),
        behavior: None,
        escorts: Vec::new(),
        minutes: 30,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let v = it.next().unwrap_or_else(|| panic!("{k} needs a value"));
        match k.as_str() {
            "--voyage" => a.voyage = v,
            "--design" => a.design = v,
            "--behavior" => a.behavior = Some(v),
            "--escort" => a.escorts.extend(v.split(',').map(str::to_string)),
            "--minutes" => a.minutes = v.parse().expect("--minutes: whole minutes"),
            "--seeds" => {
                a.seeds = match v.split_once("..") {
                    Some((lo, hi)) => {
                        let lo: u64 = lo.parse().expect("--seeds: a..b");
                        let hi: u64 = hi.parse().expect("--seeds: a..b");
                        (lo..=hi).collect()
                    }
                    None => v
                        .split(',')
                        .map(|s| s.trim().parse().expect("--seeds: a list or a..b"))
                        .collect(),
                }
            }
            _ => panic!("unknown argument {k}"),
        }
    }
    a
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Outcome {
    Recalled,
    Lost,
    OutOfTime,
}

struct Run {
    outcome: Outcome,
    goal: bool,
    ticks: u64,
    hp_lost: f32,
    hp_total: f32,
    sites_cleared: usize,
    sites: usize,
    kills: u32,
    /// Awake enemies at the end (`Sim::awake_enemy`), and how many of them are disarmed (they no longer block
    /// clearing or the recall, GDR-0068).
    left: (usize, usize),
    /// What the ship carries at the end (a lost ship brings nothing home).
    haul: Haul,
    items: u32,
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

fn fly(sim: &mut Sim, ship: ShipId, maneuver: Maneuver, fire: bool, pilot: &mut Pilot) {
    if pilot.flying_behavior {
        sim.command(ship, ShipCommand::Behavior(None));
        pilot.flying_behavior = false;
    }
    sim.command(ship, ShipCommand::Maneuver { maneuver, fire });
}

#[allow(clippy::too_many_arguments)]
fn run(
    content: &data::Content,
    voyage: &data::Voyage,
    design: &Arc<ShipDesign>,
    reach: f32,
    behavior: Option<&str>,
    escorts: &[Arc<ShipDesign>],
    seed: u64,
    max_ticks: u64,
    step_ms: &mut Vec<f64>,
) -> Run {
    let mut sim = Sim::with_config(seed, SimConfig::default());
    sim.set_behaviors(BehaviorLibrary::new(content).expect("content behaviors compile"));
    let fight_behavior = behavior.map(|id| {
        sim.behaviors()
            .id(id)
            .unwrap_or_else(|| panic!("no behavior `{id}`"))
    });
    let plan = VoyagePlan::roll(content, voyage, seed).expect("the voyage rolls");
    let entry = plan.entry;
    sim.start_voyage(plan, Owner::PLAYER, ENEMY);
    let me = sim.spawn_ship(Arc::clone(design), Owner::PLAYER, entry);
    if fight_behavior.is_none() {
        sim.command(me, ShipCommand::Focus);
    }
    // Escorts (a fleet, GDR-0065): behind the flagship, left and right in turn, under the `escort` template,
    // following the flagship's lock.
    let escort_behavior = sim.behaviors().id("escort").expect("the escort behavior");
    let mut wing = Vec::new();
    for (k, d) in escorts.iter().enumerate() {
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let row = (k / 2 + 1) as f32;
        let mut body = entry;
        body.pos = entry.pos + Vec2::new(side * ESCORT_GAP * row, -ESCORT_GAP * row);
        let id = sim.spawn_ship(Arc::clone(d), Owner::PLAYER, body);
        sim.command(id, ShipCommand::Leader(Some(me)));
        sim.command(id, ShipCommand::Behavior(Some(escort_behavior)));
        wing.push(id);
    }
    let mut pilot = Pilot::default();
    let mut kills = 0;
    let idle = TickInputs::default();
    let outcome = loop {
        if sim.tick() >= max_ticks {
            break Outcome::OutOfTime;
        }
        let Some(here) = at(&sim, me).filter(|_| sim.ship(me).is_some_and(|s| s.is_vessel()))
        else {
            break Outcome::Lost;
        };
        if sim.tick().is_multiple_of(DECIDE) {
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
            let done = |k: usize, s: &sim::Site| match s.state() {
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
            let enemies = armed_enemies(&sim, here);
            // It keeps its lock on one enemy while that one is armed and near (a player does not switch the
            // lock to whichever raider is nearest this second: every switch costs a turn), else the nearest.
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
            let done = todo.is_empty() && salvage_near(&sim, here, free, &pilot).is_none();
            if ((goal && (done || free == 0)) || !fighting) && sim.can_recall(Owner::PLAYER) {
                break Outcome::Recalled;
            }
            // A disarmed awake enemy near by (it rams, and its wreck is salvage): finish it off while there is
            // room in the hold.
            let mop = (free > 0 && fighting)
                .then(|| {
                    sim.ships()
                        .iter()
                        .filter(|s| sim.awake_enemy(Owner::PLAYER, s) && !s.is_armed())
                        .filter(|s| !pilot.spared.contains(&s.id()))
                        .filter_map(|s| {
                            let p = at(&sim, s.id())?;
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
                fly(&mut sim, me, maneuver, false, &mut pilot);
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
                match fight_behavior {
                    Some(b) => {
                        if !pilot.flying_behavior {
                            sim.command(me, ShipCommand::Behavior(Some(b)));
                            pilot.flying_behavior = true;
                        }
                    }
                    None => {
                        // The closing stand-in of the balance study (lock, fire, the stick towards the target
                        // when it is out of reach), with the stick eased off as the gap closes, so it does
                        // not fly through the pack at top speed (a ship without forward thrust cannot brake
                        // with its nose on the target).
                        let way = foe_at - here;
                        let gap = way.length() - reach * CLOSE_TO;
                        let stick = if gap > 0.0 {
                            way * ((gap / EASE).min(CLOSE_STICK) / way.length())
                        } else {
                            Vec2::ZERO
                        };
                        let input = PilotInput {
                            stick,
                            lock: Some(Target::Ship(foe)),
                            drift: false,
                            fire: true,
                            aim: AimMode::Auto,
                            ..Default::default()
                        };
                        if pilot.flying_behavior {
                            sim.command(me, ShipCommand::Behavior(None));
                            pilot.flying_behavior = false;
                        }
                        sim.command(me, ShipCommand::Pilot(input));
                    }
                }
            } else if let Some((key, pos)) = salvage_near(&sim, here, free, &pilot) {
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
                fly(&mut sim, me, maneuver, false, &mut pilot);
            } else if let Some((k, pos, ..)) = next {
                // A site whose ships still sleep: stop short of its wake radius, then creep in, so the pack
                // wakes in front of a slow ship rather than one passing through at top speed.
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
                    fly(&mut sim, me, Maneuver::Hold { at }, false, &mut pilot);
                } else if sleeping && speed > CREEP_BRAKE && dist > stage - VISITED {
                    fly(&mut sim, me, Maneuver::Brake, false, &mut pilot);
                } else if sleeping {
                    let input = PilotInput {
                        stick: way * (CREEP / dist),
                        drift: false,
                        ..Default::default()
                    };
                    if pilot.flying_behavior {
                        sim.command(me, ShipCommand::Behavior(None));
                        pilot.flying_behavior = false;
                    }
                    sim.command(me, ShipCommand::Pilot(input));
                } else {
                    fly(&mut sim, me, Maneuver::Hold { at: pos }, false, &mut pilot);
                }
            } else {
                fly(&mut sim, me, Maneuver::Brake, false, &mut pilot);
            }
        }
        let t0 = Instant::now();
        sim.step(&idle);
        step_ms.push(t0.elapsed().as_secs_f64() * 1e3);
        for e in sim.events() {
            match e {
                SimEvent::VesselLost { owner, .. } if *owner == ENEMY => {
                    kills += 1;
                    pilot.progress = sim.tick();
                }
                SimEvent::SiteWoke { .. } | SimEvent::SiteCleared { .. } => {
                    pilot.progress = sim.tick()
                }
                SimEvent::Collected { ship, .. } if *ship == me => {
                    // A part came: the piece it works on is still worth it.
                    pilot.progress = sim.tick();
                    if let Some((_, since)) = &mut pilot.salvage {
                        *since = sim.tick();
                    }
                }
                _ => {}
            }
        }
    };
    let v = sim.voyage().unwrap();
    let left: Vec<bool> = sim
        .ships()
        .iter()
        .filter(|s| sim.awake_enemy(Owner::PLAYER, s))
        .map(|s| s.is_armed())
        .collect();
    let left = (left.len(), left.iter().filter(|&&armed| !armed).count());
    let ship = sim.ship(me).unwrap();
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
            .filter(|s| s.id() == me || (wing.contains(&s.id()) && s.is_vessel()))
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
    Run {
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
        kills,
        left,
        haul,
        items,
    }
}

fn mmss(ticks: u64) -> String {
    let s = ticks / sim::TICK_HZ as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

fn p95(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)]
}

fn data_line(data: &BTreeMap<String, f32>) -> String {
    data.iter()
        .map(|(f, x)| format!("{f} {x:.0}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A content ship by id, or a blueprint file (a saved or exported drydock design, `*.ron`): a design a player
/// would build that is not content.
fn blueprint(content: &data::Content, design: &str) -> data::Blueprint {
    if design.ends_with(".ron") {
        let text = std::fs::read_to_string(design).unwrap_or_else(|e| panic!("{design}: {e}"));
        data::parse_blueprint(&text).unwrap_or_else(|e| panic!("{design}: {e:?}"))
    } else {
        content
            .ships
            .get(design)
            .unwrap_or_else(|| panic!("no ship `{design}`"))
            .clone()
    }
}

fn main() {
    let a = args();
    let content = data::base_content();
    let voyage = content
        .voyages
        .get(&a.voyage)
        .unwrap_or_else(|| panic!("no voyage `{}`", a.voyage));
    let bp = blueprint(&content, &a.design);
    let design = Arc::new(ShipDesign::compile(&content, &bp).expect("the design compiles"));
    let report = sim::design_report(&content, &bp, &SimConfig::default()).expect("a report");
    let reach = report
        .guns
        .iter()
        .filter(|g| g.clear)
        .map(|g| g.range)
        .fold(f32::INFINITY, f32::min);
    let escorts: Vec<Arc<ShipDesign>> = a
        .escorts
        .iter()
        .map(|id| {
            let bp = blueprint(&content, id);
            Arc::new(ShipDesign::compile(&content, &bp).expect("the escort compiles"))
        })
        .collect();
    let pilot = match &a.behavior {
        Some(b) => format!("behavior `{b}` in fights"),
        None => format!("closing to {:.0} m", reach * CLOSE_TO),
    };
    println!(
        "# voyage {} with {}{} ({pilot}), seeds {}..{} ({}), at most {} min",
        a.voyage,
        a.design,
        if a.escorts.is_empty() {
            String::new()
        } else {
            format!(" and escorts {} (`escort`)", a.escorts.join(", "))
        },
        a.seeds.first().unwrap_or(&0),
        a.seeds.last().unwrap_or(&0),
        a.seeds.len(),
        a.minutes
    );
    println!(
        "| seed | outcome | goal | sites | time | hull lost | kills | left (disarmed) | haul (items) | data |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    let max_ticks = a.minutes * 60 * sim::TICK_HZ as u64;
    let mut runs = Vec::new();
    let mut step_ms = Vec::new();
    for &seed in &a.seeds {
        let r = run(
            &content,
            voyage,
            &design,
            reach,
            a.behavior.as_deref(),
            &escorts,
            seed,
            max_ticks,
            &mut step_ms,
        );
        let report = unload(&content, &r.haul);
        println!(
            "| {seed} | {:?} | {} | {}/{} | {} | {:.1}/{:.0} | {} | {} ({}) | {} | {} |",
            r.outcome,
            if r.goal { "yes" } else { "no" },
            r.sites_cleared,
            r.sites,
            mmss(r.ticks),
            r.hp_lost,
            r.hp_total,
            r.kills,
            r.left.0,
            r.left.1,
            r.items,
            data_line(&report.data)
        );
        runs.push((r, report));
    }
    let home: Vec<&(Run, data::career::HaulReport)> = runs
        .iter()
        .filter(|(r, _)| r.outcome == Outcome::Recalled)
        .collect();
    let count = |o: Outcome| runs.iter().filter(|(r, _)| r.outcome == o).count();
    let cleared_home = home.iter().filter(|(r, _)| r.goal).count();
    let mut data: BTreeMap<String, f32> = BTreeMap::new();
    let mut pm = 0;
    for (_, rep) in &home {
        for (f, x) in &rep.data {
            *data.entry(f.clone()).or_default() += x / home.len() as f32;
        }
        pm += rep
            .materials
            .get("programmable_matter")
            .copied()
            .unwrap_or(0);
    }
    println!(
        "# summary: {} seeds: goal cleared and home {}, home without the goal {}, lost {}, out of time {}",
        runs.len(),
        cleared_home,
        home.len() - cleared_home,
        count(Outcome::Lost),
        count(Outcome::OutOfTime)
    );
    let f = |v: Vec<f64>| format!("median {:.1}, p95 {:.1}", median(v.clone()), p95(v));
    if !home.is_empty() {
        println!(
            "# home: time (min) {}; hull lost {}; haul items {}; programmable matter {:.1} per run",
            f(home.iter().map(|(r, _)| r.ticks as f64 / 1800.0).collect()),
            f(home.iter().map(|(r, _)| r.hp_lost as f64).collect()),
            f(home.iter().map(|(r, _)| r.items as f64).collect()),
            pm as f64 / home.len() as f64
        );
        println!("# home: data per run (mean): {}", data_line(&data));
    }
    println!(
        "# step time (ms, wall clock, all seeds): median {:.4}, p95 {:.4}, max {:.3}",
        median(step_ms.clone()),
        p95(step_ms.clone()),
        step_ms.iter().copied().fold(0.0, f64::max)
    );
}
