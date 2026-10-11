//! Rough cost of the sim core on large synthetic ships, with the `FreeFlight` backend. Native only;
//! wall-clock is used here, in the tool, never in the sim.
//!
//! `cargo run --release -p sim --example sim_perf -- [--ships 20] [--w 50] [--h 40] [--ticks 300]`

use std::sync::Arc;
use std::time::Instant;

use data::{Blueprint, Placement};
use sim::alloc::{Actuation, AllocTable, allocate};
use sim::{
    Body, DamageTarget, PilotInput, ShipCommand, ShipControls, ShipDesign, Sim, Target, TickInputs,
    Vec2,
};

fn place(part: &str, x: i32, y: i32, material: Option<&str>) -> Placement {
    Placement {
        part: part.into(),
        x,
        y,
        rot: 0,
        flip: false,
        material: material.map(Into::into),
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

fn time_steps(world: &mut Sim, inputs: &TickInputs, ticks: usize) -> ((f64, f64), (f64, f64)) {
    let mut step = Vec::new();
    let mut hash = Vec::new();
    for t in 0..ticks {
        let first = if t == 0 {
            inputs.clone()
        } else {
            TickInputs::default()
        };
        let t0 = Instant::now();
        world.step(&first);
        step.push(t0.elapsed().as_secs_f64() * 1e3);
        let t0 = Instant::now();
        std::hint::black_box(world.state_hash());
        hash.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    (stats(step), stats(hash))
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
                        shields: true,
                    },
                )
            })
            .collect(),
        ..Default::default()
    };
    let ((m, p95), (hm, hp95)) = time_steps(&mut world, &raw, ticks);
    println!("step, raw throttles ({ships} ships): mean {m:.3} ms, p95 {p95:.3} ms");
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
                    }),
                )
            })
            .collect(),
        ..Default::default()
    };
    let ((m, p95), _) = time_steps(&mut world, &pilot, ticks);
    println!("step, pilot + lock ({ships} ships): mean {m:.3} ms, p95 {p95:.3} ms");

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
                            shields: true,
                        },
                    )
                })
                .collect(),
            ..Default::default()
        };
        let p = &world.ship(ids[0]).unwrap().pieces()[0];
        let guns = p.guns.len();
        let mut step = Vec::new();
        let (mut live_max, mut live_sum, mut shots, mut hits) = (0usize, 0usize, 0usize, 0usize);
        for t in 0..ticks.min(150) {
            let input = if t == 0 {
                fire.clone()
            } else {
                TickInputs::default()
            };
            let t0 = Instant::now();
            world.step(&input);
            step.push(t0.elapsed().as_secs_f64() * 1e3);
            let live = world.projectiles().len();
            live_max = live_max.max(live);
            live_sum += live;
            for e in world.events() {
                match e {
                    sim::SimEvent::ShotFired { .. } => shots += 1,
                    sim::SimEvent::Hit { .. } => hits += 1,
                    _ => {}
                }
            }
        }
        let n = step.len();
        let (m, p95) = stats(step);
        println!(
            "combat ({ships} gunboats, {guns} guns each, shields): step mean {m:.3} ms, p95 {p95:.3} ms; \
             live projectiles avg {} max {live_max}; {shots} shots, {hits} hits in {n} ticks",
            live_sum / n
        );
    }

    // Breakage: one part in the middle (rebuild one big piece), then a full row (split).
    let ship = ids[0];
    let mut one = Vec::new();
    for k in 0..20 {
        world.apply_damage(ship, DamageTarget::Cell((5 + k, h / 2)), 1e9);
        let t = Instant::now();
        world.resolve_breakage();
        one.push(t.elapsed().as_secs_f64() * 1e3);
    }
    let (m, p95) = stats(one);
    println!(
        "lose one part (no split), rebuild piece + update body: mean {m:.3} ms, p95 {p95:.3} ms"
    );
    for x in 0..w {
        world.apply_damage(ship, DamageTarget::Cell((x, h / 3)), 1e9);
    }
    let t = Instant::now();
    world.resolve_breakage();
    let split_ms = t.elapsed().as_secs_f64() * 1e3;
    println!(
        "cut a full row ({w} parts) and split: {split_ms:.3} ms, pieces now {}",
        world.ship(ship).unwrap().pieces().len()
    );
}
