//! The headless voyage runner (track 7K's balance tool, `docs/sim.md` "Voyages"): flies a content voyage with one
//! ship over many seeds, as a simple player would, and reports per seed whether the goal was cleared, the time, the
//! hull lost, and the haul (items, and research data per family as the career unloads it), then a summary.
//!
//! `cargo run --release -p sim --example voyage -- --voyage yard_approach --design rim_skiff --seeds 1..20`
//! `[--behavior skirmisher] [--minutes 30]`
//!
//! The pilot (a script, not a player): it fights an armed awake enemy (`Sim::armed_enemy`, GDR-0068) within 400 m
//! (the nearest), or the nearest such ship of the awake site it is heading for, as the balance study's *closing*
//! stand-in does (`docs/results/balance-2026-10-13.md`: locked on it, firing, the stick towards it while it is
//! farther than 75 % of its longest clear gun's reach) or, with `--behavior`, by handing the ship to that content
//! behavior while there is a fight; else it strips the nearest salvage within 250 m that fits its hold and drifts
//! slower than 6 m/s (giving up on a piece after 20 s without a part); else it flies to the nearest site still to do
//! (a site with ships until it is cleared, a wreck site until it has been there), the goal last. It goes home as
//! soon as the sim allows (`Sim::can_recall`, running from the nearest enemy until then) once the goal is cleared
//! and every site is done or the hold is full, or when it is disarmed (`Ship::is_armed`: no gun or no power source
//! left; a gun that overheats or idles still counts), or after 3 minutes without progress (no kill, site, salvage
//! or new site). A seed ends recalled, lost (the ship is no vessel any more) or out of time. Native only; the wall
//! clock times the steps (median and p95), never the sim.

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
/// Salvage drifting faster than this (m/s) is not worth the chase.
const CATCHABLE: f32 = 6.0;
/// A wreck site counts as done once it came this close.
const VISITED: f32 = 60.0;
/// It closes to this share of its longest clear gun's reach.
const CLOSE_TO: f32 = 0.75;
/// It gives up after this many ticks without progress (3 min).
const STALEMATE: u64 = 5400;

/// A salvage piece: its ship and piece.
type PieceKey = (ShipId, PieceId);

struct Args {
    voyage: String,
    design: String,
    seeds: Vec<u64>,
    behavior: Option<String>,
    minutes: u64,
}

fn args() -> Args {
    let mut a = Args {
        voyage: "yard_approach".into(),
        design: "rim_skiff".into(),
        seeds: (1..=20).collect(),
        behavior: None,
        minutes: 30,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let v = it.next().unwrap_or_else(|| panic!("{k} needs a value"));
        match k.as_str() {
            "--voyage" => a.voyage = v,
            "--design" => a.design = v,
            "--behavior" => a.behavior = Some(v),
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
    let mut best: Option<(f32, Option<PieceKey>, Vec2)> = None;
    let mut consider = |key, pos: Vec2| {
        let d = (pos - from).length();
        if d <= SALVAGE_RANGE && best.is_none_or(|(b, _, _)| d < b) {
            best = Some((d, key, pos));
        }
    };
    if free > 0 {
        for l in sim.loose().iter().filter(|l| l.vel.length() <= CATCHABLE) {
            consider(None, l.pos);
        }
    }
    for s in sim.ships() {
        for p in s
            .pieces()
            .iter()
            .filter(|p| p.salvage() && p.body.linvel.length() <= CATCHABLE)
        {
            let key = (s.id(), p.id);
            let fits = p.parts.iter().any(|&i| {
                s.parts()[i as usize].alive && s.design().parts[i as usize].cargo_units <= free
            });
            if fits && !pilot.given_up.contains(&key) {
                consider(Some(key), p.com_world());
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
            let foe = enemies
                .iter()
                .filter(|e| e.0 <= FIGHT_RANGE)
                .min_by(|a, b| a.0.total_cmp(&b.0))
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
            // Done (or full), or given up: home as soon as the sim allows.
            if ((goal && (todo.is_empty() || free == 0)) || !fighting)
                && sim.can_recall(Owner::PLAYER)
            {
                break Outcome::Recalled;
            }
            if let Some((_, foe, _)) = foe.filter(|_| !fighting) {
                // Nothing left to shoot with: run, and recall once clear.
                let maneuver = Maneuver::Flee {
                    from: Target::Ship(foe),
                };
                fly(&mut sim, me, maneuver, false, &mut pilot);
            } else if let Some((_, foe, foe_at)) = foe {
                match fight_behavior {
                    Some(b) => {
                        if !pilot.flying_behavior {
                            sim.command(me, ShipCommand::Behavior(Some(b)));
                            pilot.flying_behavior = true;
                        }
                    }
                    None => {
                        // The closing stand-in of the balance study: lock, fire, the stick towards the
                        // target while it is out of reach, drift on.
                        let way = foe_at - here;
                        let stick = if way.length() > reach * CLOSE_TO {
                            way * (1.0 / way.length())
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
            } else {
                let maneuver = match next {
                    Some((_, pos, ..)) => Maneuver::Hold { at: pos },
                    None => Maneuver::Brake,
                };
                fly(&mut sim, me, maneuver, false, &mut pilot);
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
    let haul: Haul = if outcome == Outcome::Lost {
        Haul::default()
    } else {
        ship.cargo()
            .items()
            .map(|(item, stack)| {
                let item = match item {
                    Item::Part { part, material } => HaulItem::Part {
                        part: part.clone(),
                        material: material.clone(),
                    },
                    Item::Fragment { part } => HaulItem::Fragment { part: part.clone() },
                };
                (item, stack.count)
            })
            .collect()
    };
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

fn main() {
    let a = args();
    let content = data::base_content();
    let voyage = content
        .voyages
        .get(&a.voyage)
        .unwrap_or_else(|| panic!("no voyage `{}`", a.voyage));
    let bp = content
        .ships
        .get(&a.design)
        .unwrap_or_else(|| panic!("no ship `{}`", a.design));
    let design = Arc::new(ShipDesign::compile(&content, bp).expect("the design compiles"));
    let report = sim::design_report(&content, bp, &SimConfig::default()).expect("a report");
    let reach = report
        .guns
        .iter()
        .filter(|g| g.clear)
        .map(|g| g.range)
        .fold(0.0, f32::max);
    let pilot = match &a.behavior {
        Some(b) => format!("behavior `{b}` in fights"),
        None => format!("closing to {:.0} m", reach * CLOSE_TO),
    };
    println!(
        "# voyage {} with {} ({pilot}), seeds {}..{} ({}), at most {} min",
        a.voyage,
        a.design,
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
