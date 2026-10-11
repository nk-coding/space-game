//! Rough cost of the sim core on large synthetic ships, with Rapier physics; step times are split into the
//! Rapier step and the rest (the sim's own systems) with `Sim::step_profiled`. Native only; wall-clock is
//! used here, in the tool, never in the sim.
//!
//! `cargo run --release -p sim --example sim_perf -- [--ships 20] [--w 50] [--h 40] [--ticks 300]`

use std::sync::Arc;
use std::time::Instant;

use data::{Blueprint, Placement};
use sim::alloc::{Actuation, AllocTable, allocate};
use sim::{
    Body, DamageTarget, PilotInput, ShipCommand, ShipControls, ShipDesign, Sim, StepPhase, Target,
    TickInputs, Vec2,
};

fn place(part: &str, x: i32, y: i32, material: Option<&str>) -> Placement {
    Placement {
        part: part.into(),
        x,
        y,
        rot: 0,
        flip: false,
        material: material.map(Into::into),
        power: None,
    }
}

/// A `w × h` slab: pulse lasers and RCS pods along the nose row, a full row of thrusters at the tail, RCS pods
/// down both sides, a mix of machines (reactors, radiators, gyros, cores, ...) inside.
fn big_ship(w: i32, h: i32) -> Blueprint {
    let mut parts = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let edge = x == 0 || x == w - 1;
            let p = if y == 0 {
                match x % 4 {
                    0 => place("pulse_laser", x, y, None),
                    2 => place("rcs_pod", x, y, None),
                    _ => place("armor_block", x, y, Some("composite")),
                }
            } else if y == h - 1 {
                place("thruster", x, y, None)
            } else if edge && y % 2 == 0 {
                place("rcs_pod", x, y, None)
            } else {
                match (x * 7 + y * 13) % 25 {
                    0 | 1 => place("fission_reactor", x, y, None),
                    2 | 3 => place("radiator", x, y, None),
                    4 => place("capacitor", x, y, None),
                    5 => place("shield_emitter", x, y, None),
                    6 if (x + y) % 3 == 0 => place("logic_core", x, y, None),
                    7 if (x + y) % 2 == 0 => place("gyro", x, y, None),
                    _ => place("armor_block", x, y, Some("steel")),
                }
            };
            parts.push(p);
        }
    }
    Blueprint {
        id: "perf_slab".into(),
        name: "Perf slab".into(),
        summary: "Synthetic load-test ship.".into(),
        description: None,
        lore: None,
        tags: vec![],
        faction: "ship_mind".into(),
        size: (w as u32, h as u32),
        parts,
        behavior: None,
        source: Default::default(),
    }
}

/// A `w × h` combat slab: pulse lasers and automaton cannons alternate along the nose row, shield emitters
/// and reactors inside, thrusters at the tail.
fn gunboat(w: i32, h: i32) -> Blueprint {
    let mut bp = big_ship(w, h);
    bp.id = "perf_gunboat".into();
    for p in &mut bp.parts {
        if p.y == 0 {
            *p = match p.x % 3 {
                0 => place("pulse_laser", p.x, 0, None),
                1 => place("automaton_cannon", p.x, 0, None),
                _ => place("armor_block", p.x, 0, Some("composite")),
            };
        } else if p.y == 2 && p.x % 6 == 3 {
            *p = place("shield_emitter", p.x, p.y, None);
        }
    }
    bp
}

fn stats(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let p95 = v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)];
    (mean, p95)
}

/// One step, timed: the whole step and the Rapier step alone (`StepPhase::Physics` → `Impacts`), in ms.
fn timed_step(world: &mut Sim, inputs: &TickInputs) -> (f64, f64) {
    let t0 = Instant::now();
    let (mut p0, mut p1) = (t0, t0);
    world.step_profiled(inputs, |phase| match phase {
        StepPhase::Physics => p0 = Instant::now(),
        StepPhase::Impacts => p1 = Instant::now(),
        _ => {}
    });
    (
        t0.elapsed().as_secs_f64() * 1e3,
        (p1 - p0).as_secs_f64() * 1e3,
    )
}

/// Per-tick step times: the whole step, the Rapier step, and the rest (the sim's own systems).
#[derive(Default)]
struct StepTimes {
    step: Vec<f64>,
    rapier: Vec<f64>,
}

impl StepTimes {
    fn push(&mut self, (step, rapier): (f64, f64)) {
        self.step.push(step);
        self.rapier.push(rapier);
    }

    /// "mean / median / p95" of the step, the Rapier step and the rest.
    fn line(&self) -> String {
        let three = |v: &[f64]| {
            let mut v = v.to_vec();
            v.sort_by(|a, b| a.total_cmp(b));
            let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
            let at = |q: f64| v[((v.len() as f64 * q) as usize).min(v.len() - 1)];
            format!("{mean:.3} / {:.3} / {:.3}", at(0.5), at(0.95))
        };
        let rest: Vec<f64> = self
            .step
            .iter()
            .zip(&self.rapier)
            .map(|(s, r)| s - r)
            .collect();
        format!(
            "step {} ms; Rapier step {}; sim systems {} (mean / median / p95)",
            three(&self.step),
            three(&self.rapier),
            three(&rest)
        )
    }
}

fn time_steps(world: &mut Sim, inputs: &TickInputs, ticks: usize) -> (StepTimes, (f64, f64)) {
    let mut times = StepTimes::default();
    let mut hash = Vec::new();
    for t in 0..ticks {
        let first = if t == 0 {
            inputs.clone()
        } else {
            TickInputs::default()
        };
        times.push(timed_step(world, &first));
        let t0 = Instant::now();
        std::hint::black_box(world.state_hash());
        hash.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    (times, stats(hash))
}

fn main() {
    let (mut ships, mut w, mut h, mut ticks) = (20usize, 50i32, 40i32, 300usize);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let v: i64 = args.next().expect("value").parse().expect("number");
        match a.as_str() {
            "--ships" => ships = v as usize,
            "--w" => w = v as i32,
            "--h" => h = v as i32,
            "--ticks" => ticks = v as usize,
            _ => panic!("unknown {a}"),
        }
    }
    let content = data::base_content();
    let bp = big_ship(w, h);
    let t = Instant::now();
    let design = Arc::new(ShipDesign::compile(&content, &bp).unwrap());
    let compile_ms = t.elapsed().as_secs_f64() * 1e3;
    let spawn = |world: &mut Sim| -> (Vec<sim::ShipId>, f64) {
        let t = Instant::now();
        let ids: Vec<_> = (0..ships)
            .map(|k| {
                world.spawn_ship(
                    Arc::clone(&design),
                    Body::at(Vec2::new(k as f32 * 80.0, 0.0), 0.0),
                )
            })
            .collect();
        (ids, t.elapsed().as_secs_f64() * 1e3 / ships as f64)
    };
    let mut world = Sim::new(1);
    let (ids, spawn_ms) = spawn(&mut world);
    let p = &world.ship(ids[0]).unwrap().pieces()[0];
    println!(
        "{ships} ships of {w}x{h} = {} parts each ({} colliders, {} thruster rows of which {} RCS, {} torque sources, {} guns); compile {compile_ms:.2} ms, spawn {spawn_ms:.2} ms/ship",
        design.parts.len(),
        p.colliders.len(),
        p.thrusters.len(),
        p.thrusters.iter().filter(|t| t.rcs).count(),
        p.torque_sources.len(),
        p.guns.len()
    );

    // Raw controls (no flight computer).
    let raw = TickInputs {
        controls: ids
            .iter()
            .map(|&id| {
                (
                    id,
                    ShipControls {
                        throttle: vec![0.8; design.engines.len()],
                        fire: true,
                        guns: Vec::new(),
                        shields: true,
                    },
                )
            })
            .collect(),
        ..Default::default()
    };
    let (times, (hm, hp95)) = time_steps(&mut world, &raw, ticks);
    println!("raw throttles ({ships} ships): {}", times.line());
    println!("state_hash: mean {hm:.3} ms, p95 {hp95:.3} ms");

    // Pilot input with a lock: flight computer + allocation every tick for every ship.
    let mut world = Sim::new(1);
    let (ids, _) = spawn(&mut world);
    let pilot = TickInputs {
        commands: ids
            .iter()
            .enumerate()
            .map(|(k, &id)| {
                let lock = Some(Target::Ship(ids[(k + 1) % ids.len()]));
                (
                    id,
                    ShipCommand::Pilot(PilotInput {
                        stick: Vec2::new(0.6, 0.8),
                        lock,
                        drift: false,
                        fire: true,
                        ..Default::default()
                    }),
                )
            })
            .collect(),
        ..Default::default()
    };
    let (times, _) = time_steps(&mut world, &pilot, ticks);
    println!("pilot + lock ({ships} ships): {}", times.line());

    // The allocator alone on one big piece.
    let p = &world.ship(ids[0]).unwrap().pieces()[0];
    let fmax: Vec<f32> = p.thrusters.iter().map(|t| t.max_force).collect();
    let tmax: Vec<f32> = p.torque_sources.iter().map(|s| s.max).collect();
    let table = AllocTable::build(&p.thrusters);
    let mut act = Actuation::default();
    let n = 20_000;
    let t = Instant::now();
    for k in 0..n {
        act.resize(fmax.len(), tmax.len());
        let f = Vec2::new((k % 7) as f32 * 30.0 - 90.0, (k % 5) as f32 * 200.0 - 100.0);
        std::hint::black_box(allocate(
            &p.thrusters,
            &table,
            &fmax,
            &tmax,
            f,
            (k % 3) as f32 * 5e4 - 5e4,
            true,
            &mut act,
        ));
    }
    println!(
        "allocate ({} rows, {} torque sources): {:.2} µs per call",
        fmax.len(),
        tmax.len(),
        t.elapsed().as_secs_f64() * 1e6 / n as f64
    );

    // Combat: two lines of gunboats facing each other 60 m apart, every gun firing (beams via the pulse
    // lasers' hit-scan, cannons with projectiles, shields up).
    {
        let gb = Arc::new(ShipDesign::compile(&content, &gunboat(w, h)).unwrap());
        let mut world = Sim::new(2);
        let ids: Vec<_> = (0..ships)
            .map(|k| {
                let line = k % 2;
                let x = (k / 2) as f32 * (w as f32 + 10.0);
                let (y, angle) = if line == 0 {
                    (0.0, 0.0)
                } else {
                    (60.0 + h as f32, core::f32::consts::PI)
                };
                world.spawn_ship(Arc::clone(&gb), Body::at(Vec2::new(x, y), angle))
            })
            .collect();
        let fire = TickInputs {
            controls: ids
                .iter()
                .map(|&id| {
                    (
                        id,
                        ShipControls {
                            throttle: vec![0.0; gb.engines.len()],
                            fire: true,
                            guns: Vec::new(),
                            shields: true,
                        },
                    )
                })
                .collect(),
            ..Default::default()
        };
        let p = &world.ship(ids[0]).unwrap().pieces()[0];
        let guns = p.guns.len();
        let mut times = StepTimes::default();
        let mut destroyed: Vec<usize> = Vec::new();
        let (mut live_max, mut live_sum, mut shots, mut hits) = (0usize, 0usize, 0usize, 0usize);
        for t in 0..ticks.min(150) {
            let input = if t == 0 {
                fire.clone()
            } else {
                TickInputs::default()
            };
            times.push(timed_step(&mut world, &input));
            let live = world.projectiles().len();
            live_max = live_max.max(live);
            live_sum += live;
            let mut d = 0;
            for e in world.events() {
                match e {
                    sim::SimEvent::ShotFired { .. } => shots += 1,
                    sim::SimEvent::Hit { .. } => hits += 1,
                    sim::SimEvent::PartDestroyed { .. } => d += 1,
                    _ => {}
                }
            }
            destroyed.push(d);
        }
        let n = times.step.len();
        if std::env::var("PERF_DETAIL").is_ok() {
            for (t, ms) in times.step.iter().enumerate() {
                if *ms > 3.0 {
                    println!(
                        "  slow tick {t}: {ms:.2} ms, destroyed parts {}",
                        destroyed[t]
                    );
                }
            }
            let quiet: Vec<f64> = times
                .step
                .iter()
                .zip(&destroyed)
                .filter(|(_, d)| **d == 0)
                .map(|(s, _)| *s)
                .collect();
            println!(
                "  ticks without destroyed parts: {} of {n}, mean {:.3} ms",
                quiet.len(),
                quiet.iter().sum::<f64>() / quiet.len().max(1) as f64
            );
        }
        println!(
            "combat ({ships} gunboats, {guns} guns each, shields): {}; \
             live projectiles avg {} max {live_max}; {shots} shots, {hits} hits in {n} ticks",
            times.line(),
            live_sum / n
        );
    }

    // Breakage, 20 samples each: one part in the middle of ship 0 (rebuild one big piece), then a full row of
    // every ship (split). Each is timed twice: the sim's own work on a clone of the ship (no physics), and
    // `Sim::resolve_breakage` (the same plus the Rapier bodies: compound rebuild, mass, new bodies).
    let median = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2]
    };
    let breakage = |world: &mut Sim, ship: sim::ShipId, cells: &[(i32, i32)]| {
        let mut alone = world.ship(ship).unwrap().clone();
        for &c in cells {
            alone.apply_damage(DamageTarget::Cell(c), 1e9);
            world.apply_damage(ship, DamageTarget::Cell(c), 1e9);
        }
        let (mut ids, mut events) = (sim::ids::IdAlloc::default(), Vec::new());
        let t = Instant::now();
        alone.resolve_breakage(&mut ids, &mut events);
        let sim_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        world.resolve_breakage();
        (sim_ms, t.elapsed().as_secs_f64() * 1e3)
    };
    let report = |what: &str, samples: Vec<(f64, f64)>| {
        let total = median(samples.iter().map(|s| s.1).collect());
        let alone = median(samples.iter().map(|s| s.0).collect());
        let rapier = median(samples.iter().map(|s| s.1 - s.0).collect());
        println!(
            "{what}: median {total:.3} ms with Rapier, {alone:.3} ms sim alone, Rapier part {rapier:.3} ms"
        );
    };
    let lose: Vec<(f64, f64)> = (0..20)
        .map(|k| breakage(&mut world, ids[0], &[(5 + k, h / 2)]))
        .collect();
    report("lose one part (no split), rebuild piece + body", lose);
    let row: Vec<(i32, i32)> = (0..w).map(|x| (x, h / 3)).collect();
    let split: Vec<(f64, f64)> = ids.iter().map(|&s| breakage(&mut world, s, &row)).collect();
    report(&format!("cut a full row ({w} parts) and split"), split);
    println!(
        "pieces of ship 0 now {}",
        world.ship(ids[0]).unwrap().pieces().len()
    );
}
