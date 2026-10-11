//! Rough cost of the sim core on large synthetic ships (no physics). Native only; wall-clock is used here,
//! in the tool, never in the sim.
//!
//! `cargo run --release -p sim --example sim_perf -- [--ships 20] [--w 50] [--h 40] [--ticks 300]`

use std::sync::Arc;
use std::time::Instant;

use data::{Blueprint, Placement};
use sim::{Body, DamageTarget, ShipControls, ShipDesign, Sim, TickInputs, Vec2};

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

/// A `w × h` slab: guns along the nose row, thrusters along the tail row, a mix of machines inside.
fn big_ship(w: i32, h: i32) -> Blueprint {
    let mut parts = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let p = if y == 0 {
                if x % 2 == 0 { place("pulse_laser", x, y, None) } else { place("armor_block", x, y, Some("composite")) }
            } else if y == h - 1 {
                if x % 2 == 0 { place("thruster", x, y, None) } else { place("armor_block", x, y, Some("steel")) }
            } else {
                match (x * 7 + y * 13) % 25 {
                    0 | 1 => place("fission_reactor", x, y, None),
                    2 | 3 => place("radiator", x, y, None),
                    4 => place("capacitor", x, y, None),
                    5 => place("shield_emitter", x, y, None),
                    6 if (x + y) % 3 == 0 => place("logic_core", x, y, None),
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

fn stats(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let p95 = v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)];
    (mean, p95)
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
    let mut world = Sim::new(1);
    let t = Instant::now();
    let ids: Vec<_> = (0..ships)
        .map(|k| world.spawn_ship(Arc::clone(&design), Body::at(Vec2::new(k as f32 * 80.0, 0.0), 0.0)))
        .collect();
    let spawn_ms = t.elapsed().as_secs_f64() * 1e3 / ships as f64;
    let p = &world.ship(ids[0]).unwrap().pieces()[0];
    println!(
        "{ships} ships of {w}x{h} = {} parts each ({} colliders, {} engines, {} guns); compile {compile_ms:.2} ms, spawn {spawn_ms:.2} ms/ship",
        design.parts.len(),
        p.colliders.len(),
        design.engines.len(),
        design.guns.len()
    );
    let inputs = TickInputs {
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
        damage: vec![],
    };
    let mut step = Vec::new();
    let mut hash = Vec::new();
    for _ in 0..ticks {
        let t = Instant::now();
        world.step(&inputs);
        step.push(t.elapsed().as_secs_f64() * 1e3);
        let t = Instant::now();
        std::hint::black_box(world.state_hash());
        hash.push(t.elapsed().as_secs_f64() * 1e3);
    }
    let (m, p95) = stats(step);
    println!("step (systems, {ships} ships): mean {m:.3} ms, p95 {p95:.3} ms");
    let (m, p95) = stats(hash);
    println!("state_hash: mean {m:.3} ms, p95 {p95:.3} ms");

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
    println!("lose one part (no split), rebuild piece: mean {m:.3} ms, p95 {p95:.3} ms");
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
