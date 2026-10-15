//! Rough cost of the sim core on large synthetic ships, with Rapier physics; step times are split into the
//! Rapier step and the rest (the sim's own systems) with `Sim::step_profiled`. Native only; wall-clock is
//! used here, in the tool, never in the sim.
//!
//! `cargo run --release -p sim --example sim_perf -- [--ships 20] [--w 50] [--h 40] [--ticks 300]`
//!
//! `--report`: times `sim::design_report` (the drydock's live stats) for the base ships and the slab.
//! `--calibrate`: measures one ship's cost per tick for several designs next to the report's device-load estimate
//! (the data behind `sim::report::tuning`); `--ships` sets the copies per design (at most 10).

use std::sync::Arc;
use std::time::Instant;

use data::{Blueprint, Placement};
use sim::ship::alloc::{Actuation, AllocTable, allocate, assist};
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
        group: None,
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

/// Median and p95 of a sample.
fn median_p95(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.total_cmp(b));
    let at = |q: f64| v[((v.len() as f64 * q) as usize).min(v.len() - 1)];
    (at(0.5), at(0.95))
}

/// `--report`: how long `sim::design_report` takes for the base ships and a `w × h` slab (the drydock
/// recomputes it after every edit), with `ShipDesign::compile` alone for comparison.
fn report_timing(content: &data::Content, w: i32, h: i32) {
    let mut designs: Vec<(String, Blueprint)> = content
        .ships
        .iter()
        .map(|s| (s.id.clone(), s.clone()))
        .collect();
    designs.push((format!("slab {w}x{h}"), big_ship(w, h)));
    designs.push((format!("gunboat {w}x{h}"), gunboat(w, h)));
    let cfg = sim::SimConfig::default();
    println!("| Design | Parts | Report median / p95 (µs) | Compile median (µs) | Device load |");
    for (name, bp) in &designs {
        let r = sim::design_report(content, bp, &cfg).unwrap();
        let runs = if bp.parts.len() > 500 { 60 } else { 600 };
        let (mut report, mut compile) = (Vec::new(), Vec::new());
        for _ in 0..runs {
            let t = Instant::now();
            std::hint::black_box(sim::design_report(content, bp, &cfg).unwrap());
            report.push(t.elapsed().as_secs_f64() * 1e6);
            let t = Instant::now();
            std::hint::black_box(ShipDesign::compile(content, bp).unwrap());
            compile.push(t.elapsed().as_secs_f64() * 1e6);
        }
        let (rm, rp) = median_p95(report);
        let (cm, _) = median_p95(compile);
        println!(
            "| {name} | {} | {rm:.1} / {rp:.1} | {cm:.1} | {:.1} µs/tick ({:?}) |",
            bp.parts.len(),
            r.device.cost_us,
            r.device.class
        );
    }
}

/// `--calibrate`: the measured cost of one ship for one tick (native, `copies` copies flying apart under pilot
/// input with every gun firing, median step ÷ copies) next to the device-load estimate of `sim::report` and its
/// cost drivers, for the base ships and slabs of several sizes. Feeds the coefficients in `sim::report::tuning`.
fn calibrate(content: &data::Content, copies: usize, ticks: usize) {
    let mut designs: Vec<(String, Blueprint)> = content
        .ships
        .iter()
        .map(|s| (s.id.clone(), s.clone()))
        .collect();
    for (w, h) in [(12, 10), (16, 12), (30, 20), (50, 40)] {
        designs.push((format!("slab {w}x{h}"), big_ship(w, h)));
    }
    designs.push(("gunboat 50x40".into(), gunboat(50, 40)));
    let cfg = sim::SimConfig::default();
    println!(
        "| Design | Parts | Rows | Torque sources | Guns | Pieces | Colliders | Flight / systems / weapons / physics / other µs | Measured µs/ship | Estimate µs |"
    );
    for (name, bp) in &designs {
        let r = sim::design_report(content, bp, &cfg).unwrap();
        let d = Arc::new(ShipDesign::compile(content, bp).unwrap());
        let mut world = Sim::new(3);
        let ids: Vec<_> = (0..copies)
            .map(|k| {
                world.spawn_ship(
                    Arc::clone(&d),
                    sim::Owner(1),
                    Body::at(Vec2::new(k as f32 * 150.0, 0.0), 0.0),
                )
            })
            .collect();
        let pilot = TickInputs {
            commands: ids
                .iter()
                .map(|&id| {
                    (
                        id,
                        ShipCommand::Pilot(PilotInput {
                            stick: Vec2::new(0.0, 1.0),
                            fire: true,
                            ..Default::default()
                        }),
                    )
                })
                .collect(),
            ..Default::default()
        };
        world.step(&pilot);
        for _ in 0..30 {
            world.step(&TickInputs::default());
        }
        // Per tick: the whole step and its phases (flight, systems, weapons, physics, the rest), in µs.
        let mut phases: [Vec<f64>; 6] = Default::default();
        for _ in 0..ticks {
            let t0 = Instant::now();
            let mut at = [t0; 5];
            world.step_profiled(&TickInputs::default(), |phase| match phase {
                StepPhase::Flight => at[0] = Instant::now(),
                StepPhase::Systems => at[1] = Instant::now(),
                StepPhase::Weapons => at[2] = Instant::now(),
                StepPhase::Physics => at[3] = Instant::now(),
                StepPhase::Impacts => at[4] = Instant::now(),
                _ => {}
            });
            let total = t0.elapsed().as_secs_f64() * 1e6;
            let us = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1e6;
            let parts = [
                us(at[0], at[1]),
                us(at[1], at[2]),
                us(at[2], at[3]),
                us(at[3], at[4]),
            ];
            for (k, v) in parts.iter().enumerate() {
                phases[k].push(*v);
            }
            phases[4].push(total - parts.iter().sum::<f64>());
            phases[5].push(total);
        }
        let per = |v: &Vec<f64>| median_p95(v.clone()).0 / copies as f64;
        let c = r.device.drivers;
        println!(
            "| {name} | {} | {} | {} | {} | {} | {} | {:.2} / {:.2} / {:.2} / {:.2} / {:.2} | {:.2} | {:.2} |",
            c.parts,
            c.rows,
            c.torque_sources,
            c.guns,
            c.pieces,
            c.colliders,
            per(&phases[0]),
            per(&phases[1]),
            per(&phases[2]),
            per(&phases[3]),
            per(&phases[4]),
            per(&phases[5]),
            r.device.cost_us
        );
    }
}

fn main() {
    let (mut ships, mut w, mut h, mut ticks) = (20usize, 50i32, 40i32, 300usize);
    let (mut report, mut calib) = (false, false);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--report" => {
                report = true;
                continue;
            }
            "--calibrate" => {
                calib = true;
                continue;
            }
            _ => {}
        }
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
    if report || calib {
        if report {
            report_timing(&content, w, h);
        }
        if calib {
            calibrate(&content, ships.min(10), ticks);
        }
        return;
    }
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
                    sim::Owner(1),
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
        let missing = allocate(
            &p.thrusters,
            &table,
            &fmax,
            &fmax,
            &tmax,
            f,
            (k % 3) as f32 * 5e4 - 5e4,
            &mut act,
        );
        std::hint::black_box(assist(&p.thrusters, &table, &fmax, &fmax, missing, true, &mut act));
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
                world.spawn_ship(
                    Arc::clone(&gb),
                    sim::Owner(1 + line as u8),
                    Body::at(Vec2::new(x, y), angle),
                )
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
