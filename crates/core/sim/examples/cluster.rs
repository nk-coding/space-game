//! Bigger clusters (galaxy plan track E0, `docs/plans/galaxy-voyages.md`): what a cluster of 2–3 km with a fight
//! and a few hundred sleeping ships and wrecks costs, and how `f32` holds far from the origin. Three sections,
//! each a markdown table:
//!
//! * **cost**: the shared load-test fight (`--awake N` ships, `sim::voyage::scenario::load_test`) moved to the
//!   entry of a voyage rolled on a cluster of `--radius` metres with the encounters of `--sites`. Its sites are
//!   either not in the world (`none`: no voyage at all; `records`: the director runs, nothing spawns: frozen sites
//!   kept as records) or all spawned at the start and asleep (`asleep`: the director's sleeping sites, the
//!   bodies at rest so Rapier puts them to sleep). Step times (whole, per phase, Rapier alone), the first tick
//!   (where `asleep` spawns the whole cluster), how many bodies sleep at the end, how many sites the fight woke.
//! * **hitch**: mid-fight (`--awake`'s first value), k raiders spawned 600 m off as a site spawns them (holding),
//!   woken 90 ticks later, removed (frozen back to a record) 90 ticks after that: the calls and the next steps
//!   against the fight's median step; and what compiling a design costs.
//! * **precision**: `f32` at 0, 3, 8 and 30 km from the origin: a holding ship's jitter, coasting at low
//!   speeds (distance moved against expected), a head-on ram, and the load-test fight (20 ships) from `--starts`
//!   spots 10 m apart (the fight ignores its seed, so this is its spread).
//!
//! Every row ends in a state hash, so a native run and a wasm32-wasip1 run under Node (V8, the browser's engine;
//! `scripts/cluster-bench.sh --wasm`) can be compared. `--repeat N` runs every timed case N times: times read
//! "median (min–max)" over the runs. Wall-clock is used here, in the tool, never in the sim.
//!
//! `cargo run --release -p sim --example cluster -- [--section cost,hitch,precision] [--awake 20,40]
//! [--radius 2500] [--sites raider_den:10,...] [--ticks 1800] [--seed 42] [--starts 5] [--repeat 1]`
//! (or `scripts/cluster-bench.sh`).

use std::sync::Arc;
use std::time::Instant;

use data::VoyageSite;
use sim::{
    Body, BodyKey, Maneuver, Owner, ShipCommand, ShipDesign, ShipId, Sim, SimEvent, StepPhase, TickInputs, Vec2,
    VoyagePlan,
};

/// The default cluster: the slice's encounters, about 200 sleeping ships and wrecks over 60 sites.
const SITES: &str = "raider_den:10,raider_pack:12,hunter_pack:8,hunter_den:4,derelict_field:10,core_graveyard:10,precursor_wrecks:6";

/// Far-from-origin offsets of the precision section (m), along (0.6, 0.8).
const OFFSETS: [f32; 4] = [0.0, 3000.0, 8000.0, 30000.0];

/// Mean, median, p95 and max of per-tick times.
fn stats(mut v: Vec<f64>) -> [f64; 4] {
    let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
    v.sort_by(|a, b| a.total_cmp(b));
    let at = |q: f64| v[((v.len() as f64 * q) as usize).min(v.len() - 1)];
    [mean, at(0.5), at(0.95), v[v.len() - 1]]
}

/// One value over the repeats: "median (min–max)", or the value alone for a single run.
fn spread(mut v: Vec<f64>) -> String {
    v.sort_by(|a, b| a.total_cmp(b));
    let median = v[v.len() / 2];
    if v.len() == 1 {
        format!("{median:.3}")
    } else {
        format!("{median:.3} ({:.3}–{:.3})", v[0], v[v.len() - 1])
    }
}

/// Milliseconds since `t`.
fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// Moves every piece of every ship by `d` (the load-test fight is built at the origin).
fn shift(sim: &mut Sim, d: Vec2) {
    let moves: Vec<(ShipId, sim::PieceId, Body)> = sim
        .ships()
        .iter()
        .flat_map(|s| s.pieces().iter().map(move |p| (s.id(), p.id, p.body)))
        .collect();
    for (ship, piece, mut body) in moves {
        body.pos += d;
        sim.teleport(ship, piece, body);
    }
}

/// The load-test fight with every core of the player running (its mind capacity raised past 20 runners).
fn fight(n: usize, seed: u64) -> Sim {
    let mut sim = sim::voyage::scenario::load_test(n, seed);
    let mut config = *sim.config();
    config.mind_capacity[Owner::PLAYER.0 as usize] = 1000;
    sim.set_config(config);
    sim
}

fn design(content: &data::Content, id: &str) -> Arc<ShipDesign> {
    let bp = content.ships.get(id).unwrap_or_else(|| panic!("no ship `{id}`"));
    Arc::new(ShipDesign::compile(content, bp).expect("base ships compile"))
}

/// Bodies in the world and how many of them sleep.
fn sleeping(sim: &Sim) -> (usize, usize) {
    let keys = sim
        .ships()
        .iter()
        .flat_map(|s| s.pieces().iter().map(move |p| BodyKey { ship: s.id(), piece: p.id }));
    keys.fold((0, 0), |(n, z), k| {
        (n + 1, z + sim.physics().is_sleeping(k).unwrap_or(false) as usize)
    })
}

struct Args {
    sections: Vec<String>,
    awake: Vec<usize>,
    radius: f32,
    sites: String,
    ticks: usize,
    seed: u64,
    starts: usize,
    repeat: usize,
}

fn main() {
    let mut a = Args {
        sections: vec!["cost".into(), "hitch".into(), "precision".into()],
        awake: vec![20, 40],
        radius: 2500.0,
        sites: SITES.into(),
        ticks: 1800,
        seed: 42,
        starts: 5,
        repeat: 1,
    };
    let mut args = std::env::args().skip(1);
    while let Some(k) = args.next() {
        let v = args.next().unwrap_or_else(|| panic!("{k} needs a value"));
        let list = |v: &str| v.split(',').map(|x| x.trim().to_string()).collect::<Vec<_>>();
        match k.as_str() {
            "--section" => a.sections = list(&v),
            "--awake" => {
                a.awake = list(&v)
                    .iter()
                    .map(|x| x.parse().expect("--awake: list of integers"))
                    .collect()
            }
            "--radius" => a.radius = v.parse().expect("--radius: metres"),
            "--sites" => a.sites = v,
            "--ticks" => a.ticks = v.parse().expect("--ticks: integer"),
            "--seed" => a.seed = v.parse().expect("--seed: integer"),
            "--starts" => a.starts = v.parse::<usize>().expect("--starts: integer").max(1),
            "--repeat" => a.repeat = v.parse::<usize>().expect("--repeat: integer").max(1),
            _ => panic!("unknown argument {k}"),
        }
    }
    let content = data::base_content();
    for s in &a.sections {
        match s.as_str() {
            "cost" => cost(&a, &content),
            "hitch" => hitch(&a, &content),
            "precision" => precision(&a, &content),
            _ => panic!("unknown section {s}"),
        }
        println!();
    }
}

/// The cluster's plan: a voyage of the base content with `--radius` and the sites of `--sites`.
fn plan(a: &Args, content: &data::Content) -> VoyagePlan {
    let mut voyage = content.voyages.get("yard_approach").expect("base voyage").clone();
    voyage.radius = a.radius;
    voyage.spacing = (300.0, 800.0);
    voyage.sites = a
        .sites
        .split(',')
        .map(|e| {
            let (id, n) = e.split_once(':').unwrap_or((e, "1"));
            VoyageSite {
                encounter: id.trim().into(),
                count: n.trim().parse().expect("--sites: encounter:count"),
                goal: false,
            }
        })
        .collect();
    VoyagePlan::roll(content, &voyage, a.seed).expect("the cluster rolls")
}

fn cost(a: &Args, content: &data::Content) {
    let plan = plan(a, content);
    let ships: usize = plan.sites.iter().map(|s| s.ships.len()).sum();
    let wrecks: usize = plan.sites.iter().map(|s| s.wrecks.len()).sum();
    println!(
        "# cost: a cluster of {} m, {} sites ({ships} ships, {wrecks} wrecks); the fight at the entry ({} m from the origin); {} ticks, seed {}, {} run(s); ms",
        a.radius,
        plan.sites.len(),
        a.radius,
        a.ticks,
        a.seed,
        a.repeat
    );
    println!(
        "| awake | sites | in world (ships) | first tick | step mean | median | p95 | max | flight | systems | weapons | Rapier | impacts + director | sleeping at end | sites woken | hash |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for &n in &a.awake {
        for mode in ["none", "records", "asleep"] {
            let mut cols: Vec<Vec<f64>> = vec![Vec::new(); 10];
            let (mut world, mut tail) = (String::new(), String::new());
            for _ in 0..a.repeat {
                let mut sim = fight(n, a.seed);
                shift(&mut sim, plan.entry.pos);
                if mode != "none" {
                    let mut config = *sim.config();
                    config.voyage.spawn_radius = if mode == "asleep" { 4.0 * a.radius } else { 0.0 };
                    sim.set_config(config);
                    sim.start_voyage(plan.clone(), Owner::PLAYER, Owner(1));
                }
                let idle = TickInputs::default();
                let (mut times, mut phases) = (Vec::with_capacity(a.ticks), [0.0f64; 5]);
                let mut in_world = 0usize;
                for _ in 0..a.ticks {
                    let t0 = Instant::now();
                    let mut at = [t0; 6];
                    sim.step_profiled(&idle, |p| match p {
                        StepPhase::Flight => at[0] = Instant::now(),
                        StepPhase::Systems => at[1] = Instant::now(),
                        StepPhase::Weapons => at[2] = Instant::now(),
                        StepPhase::Physics => at[3] = Instant::now(),
                        StepPhase::Impacts => at[4] = Instant::now(),
                        StepPhase::Done => at[5] = Instant::now(),
                        _ => {}
                    });
                    times.push((at[5] - t0).as_secs_f64() * 1e3);
                    for k in 0..5 {
                        phases[k] += (at[k + 1] - at[k]).as_secs_f64() * 1e3;
                    }
                    in_world = in_world.max(sim.ships().len());
                }
                let first = times.remove(0);
                let [mean, median, p95, max] = stats(times);
                for (k, v) in [first, mean, median, p95, max].into_iter().enumerate() {
                    cols[k].push(v);
                }
                for (k, p) in phases.iter().enumerate() {
                    cols[5 + k].push(p / a.ticks as f64);
                }
                let (bodies, asleep) = sleeping(&sim);
                let running = sim.ships().iter().filter(|s| s.is_active()).count();
                let woken = sim.voyage().map_or(0, |v| {
                    v.sites()
                        .iter()
                        .filter(|s| matches!(s.state(), sim::SiteState::Awake | sim::SiteState::Cleared))
                        .count()
                });
                world = format!("{in_world} ({running} running at end)");
                tail = format!("{asleep} of {bodies} | {woken} | {:016x}", sim.state_hash());
            }
            let times: Vec<String> = cols.into_iter().map(spread).collect();
            println!("| {n} | {mode} | {world} | {} | {tail} |", times.join(" | "));
        }
    }
}

fn hitch(a: &Args, content: &data::Content) {
    let n = a.awake[0];
    let raider = design(content, "automaton_raider");
    println!(
        "# hitch: k raiders spawned mid-fight ({n} ships fighting, tick 300) 600 m off, holding; woken at 390, removed at 480; seed {}, {} run(s); ms, median (min–max) over the runs",
        a.seed, a.repeat
    );
    println!(
        "| k | fight median step | spawn calls | next step | wake: max of next 3 steps | remove calls | next step | hash |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    for k in [1usize, 3, 6, 12, 24] {
        let mut cols: Vec<Vec<f64>> = vec![Vec::new(); 6];
        let mut hash = 0u64;
        for _ in 0..a.repeat {
            let mut sim = fight(n, a.seed);
            let idle = TickInputs::default();
            let step = |sim: &mut Sim| {
                let t = Instant::now();
                sim.step(&idle);
                ms(t)
            };
            let base: Vec<f64> = (0..300).map(|_| step(&mut sim)).collect();
            cols[0].push(stats(base)[1]);
            let t = Instant::now();
            let ids: Vec<ShipId> = (0..k)
                .map(|j| {
                    let at = Vec2::new(600.0 + 12.0 * j as f32, 0.0);
                    let id = sim.spawn_ship(Arc::clone(&raider), Owner(1), Body::at(at, 0.0));
                    sim.command(
                        id,
                        ShipCommand::Maneuver {
                            maneuver: Maneuver::Hold { at },
                            fire: false,
                        },
                    );
                    id
                })
                .collect();
            cols[1].push(ms(t));
            cols[2].push(step(&mut sim));
            (0..89).for_each(|_| {
                step(&mut sim);
            });
            let fly = sim.behaviors().id("raider_basic").expect("load test behavior");
            for &id in &ids {
                sim.command(id, ShipCommand::Behavior(Some(fly)));
            }
            cols[3].push((0..3).map(|_| step(&mut sim)).fold(0.0, f64::max));
            (0..87).for_each(|_| {
                step(&mut sim);
            });
            let t = Instant::now();
            for &id in &ids {
                sim.remove_ship(id);
            }
            cols[4].push(ms(t));
            cols[5].push(step(&mut sim));
            hash = sim.state_hash();
        }
        let times: Vec<String> = cols.into_iter().map(spread).collect();
        println!("| {k} | {} | {hash:016x} |", times.join(" | "));
    }
    let mut times = Vec::new();
    for id in ["automaton_raider", "automaton_hunter", "automaton_warden", "sample_mid"] {
        let t = Instant::now();
        for _ in 0..20 {
            design(content, id);
        }
        times.push(format!("{id} {:.3}", ms(t) / 20.0));
    }
    println!("\ncompiling a design (ShipDesign::compile, ms, mean of 20): {}", times.join(", "));
}

fn precision(a: &Args, content: &data::Content) {
    let raider = design(content, "automaton_raider");
    let dir = Vec2::new(0.6, 0.8);
    println!(
        "# precision: f32 at a distance from the origin (along (0.6, 0.8)); the fight is the load test with 20 ships, 900 ticks, from {} spots 10 m apart: mean (min–max)",
        a.starts
    );
    println!(
        "| offset (m) | f32 step there (mm) | hold: jitter (mm) / max speed (mm/s) | coast 0.012 / 0.05 / 0.2 / 1 m/s: moved ÷ expected | ram: impulse / closing (m/s) / speeds after (m/s) | fight: shots / hits / impacts / splits / running (mean) | hash |"
    );
    println!("|---|---|---|---|---|---|---|");
    for off in OFFSETS {
        let o = dir * off;
        let mut h = sim::StateHasher::new();
        // A ship holding where it spawned, pushed off at 2 m/s: the spread of its position over the last 10 s.
        let mut sim = Sim::new(a.seed);
        let id = sim.spawn_ship(
            Arc::clone(&raider),
            Owner(1),
            Body {
                linvel: Vec2::new(2.0, 1.0),
                ..Body::at(o, 0.0)
            },
        );
        sim.command(
            id,
            ShipCommand::Maneuver {
                maneuver: Maneuver::Hold { at: o },
                fire: false,
            },
        );
        let idle = TickInputs::default();
        let mut track = Vec::new();
        for t in 0..900 {
            sim.step(&idle);
            if t >= 600 {
                let p = &sim.ships()[0].pieces()[0].body;
                track.push((p.pos - o, p.linvel.length()));
            }
        }
        let n = track.len() as f32;
        let mean = track.iter().fold(Vec2::ZERO, |m, (p, _)| m + *p * (1.0 / n));
        let jitter = track.iter().map(|(p, _)| (*p - mean).length()).fold(0.0, f32::max);
        let speed = track.iter().map(|&(_, v)| v).fold(0.0, f32::max);
        h.u64(sim.state_hash());
        // Coasting, nothing commanded: the distance covered in 10 s against speed × 10 s.
        let coast: Vec<String> = [0.012f32, 0.05, 0.2, 1.0]
            .iter()
            .map(|&v| {
                let mut sim = Sim::new(a.seed);
                sim.spawn_ship(
                    Arc::clone(&raider),
                    Owner(1),
                    Body {
                        linvel: dir * v,
                        ..Body::at(o, 0.0)
                    },
                );
                let start = sim.ships()[0].pieces()[0].body.pos;
                (0..300).for_each(|_| sim.step(&idle));
                h.u64(sim.state_hash());
                let moved = (sim.ships()[0].pieces()[0].body.pos - start).length();
                format!("{:.3}", moved / (v * 10.0))
            })
            .collect();
        // Head-on: one raider at 20 m/s into another at rest, 30 m apart; the first impact and the speeds 2 s on.
        let mut sim = Sim::new(a.seed);
        let x = Vec2::new(1.0, 0.0);
        let ram_a = sim.spawn_ship(
            Arc::clone(&raider),
            Owner(1),
            Body {
                linvel: x * 20.0,
                ..Body::at(o - x * 15.0, 0.0)
            },
        );
        let ram_b = sim.spawn_ship(Arc::clone(&raider), Owner::PLAYER, Body::at(o + x * 15.0, 0.0));
        let mut first = None;
        for _ in 0..60 {
            sim.step(&idle);
            if first.is_none() {
                first = sim.events().iter().find_map(|e| match e {
                    SimEvent::Impact(c) => Some((c.impulse, c.relative_speed)),
                    _ => None,
                });
            }
        }
        let speed_of = |id: ShipId| sim.ship(id).map_or(0.0, |s| s.pieces()[0].body.linvel.length());
        let (impulse, closing) = first.unwrap_or((0.0, 0.0));
        let ram = format!(
            "{impulse:.3} / {closing:.4} / {:.4}, {:.4}",
            speed_of(ram_a),
            speed_of(ram_b)
        );
        h.u64(sim.state_hash());
        // The load-test fight moved there.
        let mut counts = vec![[0u32; 5]; a.starts];
        for (k, c) in counts.iter_mut().enumerate() {
            let mut sim = fight(20, a.seed);
            shift(&mut sim, o + Vec2::new(10.0 * k as f32, 0.0));
            for _ in 0..900 {
                sim.step(&idle);
                for e in sim.events() {
                    match e {
                        SimEvent::ShotFired { .. } => c[0] += 1,
                        SimEvent::Hit { .. } => c[1] += 1,
                        SimEvent::Impact(_) => c[2] += 1,
                        SimEvent::Split(_) => c[3] += 1,
                        _ => {}
                    }
                }
            }
            c[4] = sim.ships().iter().filter(|s| s.is_active()).count() as u32;
            h.u64(sim.state_hash());
        }
        let m = |k: usize| {
            let v = counts.iter().map(|c| c[k]);
            let mean = v.clone().sum::<u32>() as f64 / a.starts as f64;
            format!("{mean:.1} ({}–{})", v.clone().min().unwrap_or(0), v.max().unwrap_or(0))
        };
        let f = off.max(1.0);
        let ulp = (f32::from_bits(f.to_bits() + 1) - f) * 1e3;
        println!(
            "| {off} | {ulp:.3} | {:.3} / {:.3} | {} | {ram} | {} / {} / {} / {} / {} | {:016x} |",
            jitter * 1e3,
            speed * 1e3,
            coast.join(" / "),
            m(0),
            m(1),
            m(2),
            m(3),
            m(4),
            h.finish()
        );
    }
}
