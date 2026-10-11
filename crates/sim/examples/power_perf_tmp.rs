//! TEMPORARY (power agent): cost of `Ship::update_systems` (power + heat + wrench) before/after the power
//! allocation change. Not committed.

use std::sync::Arc;
use std::time::Instant;

use data::{Blueprint, Placement};
use sim::{Body, ShipControls, ShipDesign, Sim, TickInputs, Vec2};

fn place(part: &str, x: i32, y: i32, material: Option<&str>) -> Placement {
    let mut p: Placement = data::ron_options().from_str(&format!("(part: {part:?}, x: {x}, y: {y})")).unwrap();
    p.material = material.map(Into::into);
    p
}

/// A `w × h` slab with lasers on the nose row, thrusters at the tail, RCS on the sides and a mix inside.
/// `reactor_mod`: one reactor per `reactor_mod` interior cells (fewer reactors = brown-out).
fn slab(w: i32, h: i32, reactor_mod: i32) -> Blueprint {
    let mut parts = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let edge = x == 0 || x == w - 1;
            let k = (x * 7 + y * 13) % 25;
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
            } else if (x * 31 + y * 17) % reactor_mod == 0 {
                place("fission_reactor", x, y, None)
            } else {
                match k {
                    2 | 3 => place("radiator", x, y, None),
                    5 | 9 => place("shield_emitter", x, y, None),
                    6 if (x + y) % 3 == 0 => place("logic_core", x, y, None),
                    7 if (x + y) % 2 == 0 => place("gyro", x, y, None),
                    11 => place("beam_laser_dummy", x, y, None),
                    _ => place("armor_block", x, y, Some("steel")),
                }
            };
            parts.push(p);
        }
    }
    parts.retain(|p| p.part != "beam_laser_dummy");
    let mut bp: Blueprint = data::ron_options().from_str(
        r#"(id: "perf", name: "Perf", summary: "Perf.", faction: "ship_mind", size: (1, 1), parts: [])"#,
    )
    .unwrap();
    bp.size = (w as u32, h as u32);
    bp.parts = parts;
    bp
}

fn stats(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    (mean, v[v.len() / 2], v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)])
}

fn main() {
    let content = data::base_content();
    for (label, rmod) in [("plenty", 9), ("brown-out", 60)] {
        let bp = slab(50, 40, rmod);
        let design = Arc::new(ShipDesign::compile(&content, &bp).unwrap());
        let mut world = Sim::new(1);
        let ids: Vec<_> = (0..20)
            .map(|k| world.spawn_ship(Arc::clone(&design), Body::at(Vec2::new(k as f32 * 80.0, 0.0), 0.0)))
            .collect();
        let raw = TickInputs {
            controls: ids
                .iter()
                .map(|&id| {
                    let mut c = ShipControls::idle(design.engines.len());
                    c.throttle.iter_mut().for_each(|t| *t = 0.8);
                    c.fire = true;
                    c.shields = true;
                    (id, c)
                })
                .collect(),
            ..Default::default()
        };
        world.step(&raw);
        let p = &world.ship(ids[0]).unwrap().pieces()[0];
        let consumers = p.consumers().len();
        let (sup, dem) = (p.power.supply, p.power.demand);
        let mut t_sys = Vec::new();
        for _ in 0..400 {
            let t0 = Instant::now();
            for &id in &ids {
                world.ship_mut(id).unwrap().update_systems();
            }
            t_sys.push(t0.elapsed().as_secs_f64() * 1e6);
        }
        let (m, med, p95) = stats(t_sys);
        println!(
            "{label}: {} parts, {consumers} consumers/ship, supply {sup:.0} demand {dem:.0}; update_systems x20 ships: mean {m:.1} µs, median {med:.1} µs, p95 {p95:.1} µs",
            design.parts.len()
        );
    }
}
