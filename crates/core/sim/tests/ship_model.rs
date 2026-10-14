//! Ship model: mass properties, colliders, power, heat, bonuses, engines and guns.

mod common;

use common::*;
use sim::{DamageTarget, Stat, Vec2};

// ---------- mass properties ----------

#[test]
fn runner_mass_properties() {
    let c = content();
    let bp = c.ships.get("halvex_runner").unwrap();
    let d = design(&c, "halvex_runner");
    let (w, s) = world_with(d.clone());
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.pieces().len(), 1);
    let p = &ship.pieces()[0];

    let total: f32 = bp
        .parts
        .iter()
        .map(|pl| c.placement_mass(pl).unwrap())
        .sum();
    assert!(
        close(p.mass.mass, total, 1e-3),
        "{} vs {total}",
        p.mass.mass
    );
    // Frame origin = centre of mass, which sits on the mirror axis between columns 5 and 6.
    assert_eq!(p.mass.com, Vec2::ZERO);
    assert!(close(p.origin[0], 6.0, 1e-5), "CoM x {}", p.origin[0]);
    assert!(
        p.origin[1] > 2.0 && p.origin[1] < 5.0,
        "CoM y {}",
        p.origin[1]
    );

    // Inertia against brute-force sampling of every part's polygon.
    let k = 24;
    let (mut m, mut mx, mut my) = (0.0f64, 0.0f64, 0.0f64);
    let mut samples: Vec<(f64, f64, f64)> = Vec::new();
    for (i, spec) in d.parts.iter().enumerate() {
        let poly = spec.shape.polygon();
        let (pw, ph) = spec.shape.dims();
        let mut pts = Vec::new();
        for sy in 0..ph as i32 * k {
            for sx in 0..pw as i32 * k {
                let x = spec.shape.x as f64 + (sx as f64 + 0.5) / k as f64;
                let y = spec.shape.y as f64 + (sy as f64 + 0.5) / k as f64;
                if inside(&poly, x, y) {
                    pts.push((x, y));
                }
            }
        }
        let pm = c.placement_mass(&bp.parts[i]).unwrap() as f64 / pts.len() as f64;
        for (x, y) in pts {
            samples.push((pm, x, y));
            m += pm;
            mx += pm * x;
            my += pm * y;
        }
    }
    let (cx, cy) = (mx / m, my / m);
    assert!((cx - p.origin[0] as f64).abs() < 1e-3 && (cy - p.origin[1] as f64).abs() < 1e-2);
    let inertia: f64 = samples
        .iter()
        .map(|&(pm, x, y)| pm * ((x - cx) * (x - cx) + (y - cy) * (y - cy)))
        .sum();
    let rel = (p.mass.inertia as f64 - inertia).abs() / inertia;
    assert!(
        rel < 0.005,
        "inertia {} vs sampled {inertia}",
        p.mass.inertia
    );
    // Sensible: between a point mass and the bounding box's uniform plate (12 × 7 m).
    let plate = p.mass.mass * (12.0 * 12.0 + 7.0 * 7.0) / 12.0;
    assert!(
        p.mass.inertia > 0.3 * plate && p.mass.inertia < plate,
        "{} vs plate {plate}",
        p.mass.inertia
    );
}

fn inside(poly: &[data::GridPoint], x: f64, y: f64) -> bool {
    let n = poly.len();
    let mut sign = 0.0;
    for k in 0..n {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        let c = (b.x - a.x) as f64 * (y - a.y as f64) - (b.y - a.y) as f64 * (x - a.x as f64);
        if c != 0.0 {
            if sign != 0.0 && c.signum() != sign {
                return false;
            }
            sign = c.signum();
        }
    }
    true
}

#[test]
fn symmetric_ships_have_com_on_axis() {
    let c = content();
    for (id, axis) in [
        ("automaton_raider", 1.5),
        ("sample_early", 8.0),
        ("sample_mid", 8.0),
    ] {
        let (w, s) = world_with(design(&c, id));
        let p = &w.ship(s).unwrap().pieces()[0];
        assert!(close(p.origin[0], axis, 1e-4), "{id}: {}", p.origin[0]);
        assert!(p.mass.inertia > 0.0);
    }
}

// ---------- colliders ----------

#[test]
fn colliders_cover_each_piece_exactly() {
    let c = content();
    for bp in c.ships.iter() {
        let d = design(&c, &bp.id);
        let (w, s) = world_with(d.clone());
        let ship = w.ship(s).unwrap();
        for p in ship.pieces() {
            let area: f32 = p.parts.iter().map(|&i| d.parts[i as usize].area).sum();
            let col_area: f32 = p.colliders.iter().map(|k| k.area()).sum();
            assert!(
                close(area, col_area, 1e-3),
                "{}: {area} vs {col_area}",
                bp.id
            );
            let cells: usize = p
                .parts
                .iter()
                .map(|&i| d.parts[i as usize].shape.cells().len())
                .sum();
            assert!(
                p.colliders.len() < cells,
                "{}: merging should reduce shapes",
                bp.id
            );
            for k in &p.colliders {
                assert!(k.area() > 0.0, "{}: collider not counter-clockwise", bp.id);
                let pts = k.points();
                for i in 0..pts.len() {
                    let (a, b, cc) = (pts[i], pts[(i + 1) % pts.len()], pts[(i + 2) % pts.len()]);
                    assert!(
                        (b - a).cross(cc - b) >= 0.0,
                        "{}: collider not convex",
                        bp.id
                    );
                }
                // Every collider lies on parts of this piece.
                let centre = pts.iter().fold(Vec2::ZERO, |s, &q| s + q) * (1.0 / pts.len() as f32);
                assert!(
                    ship.part_at_local(p.id, centre).is_some(),
                    "{}: collider off the ship",
                    bp.id
                );
            }
        }
    }
    // The runner: every slope stays a triangle, the square-ish parts merge into far fewer rectangles.
    let d = design(&c, "halvex_runner");
    let (w, s) = world_with(d.clone());
    let p = &w.ship(s).unwrap().pieces()[0];
    let tris = p.colliders.iter().filter(|k| k.points().len() == 3).count();
    assert_eq!(tris, d.parts.iter().filter(|p| p.slope).count());
    assert!(
        p.colliders.len() - tris <= 16,
        "{} rects",
        p.colliders.len() - tris
    );
}

// ---------- power ----------

/// Two beams on top, core | fission | shield in the middle, a thruster below the fission reactor. (The power
/// allocation itself is tested in `tests/power.rs`.)
fn brownout_ship() -> Vec<P<'static>> {
    vec![
        ("beam_laser", 0, 0, 0, false, None),
        ("beam_laser", 1, 0, 0, false, None),
        ("logic_core", 0, 2, 0, false, None),
        ("fission_reactor", 1, 2, 0, false, None),
        ("shield_emitter", 2, 2, 0, false, None),
        ("thruster", 1, 3, 0, false, None),
    ]
}

#[test]
fn idle_consumers_draw_nothing() {
    let c = content();
    let d = custom(&c, &brownout_ship());
    let (mut w, s) = world_with(d);
    w.step(&inputs(s, controls(1, 0.0, false, false)));
    let p = &w.ship(s).unwrap().pieces()[0];
    assert_eq!(p.power.demand, 1.0, "only the logic core");
    assert_eq!(p.wrench, sim::Wrench::default());
}

#[test]
fn runner_power_balance() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let (mut w, s) = world_with(d.clone());
    w.step(&inputs(s, controls(3, 1.0, true, true)));
    let ship = w.ship(s).unwrap();
    let p = &ship.pieces()[0];
    // Supply = every reactor's output with its radiator bonuses; the layout's venting radiators add some.
    let base: f32 = d.parts.iter().map(|x| x.base.get(Stat::PowerOutput)).sum();
    let bonus: f32 = (0..d.parts.len() as u32)
        .map(|i| ship.stats(i).get(Stat::PowerOutput))
        .sum();
    assert_eq!(p.power.supply, bonus);
    assert!(bonus > base, "venting radiators beside reactors add power");
    // Demand at full throttle with guns firing (raw controls: gyros idle): guns with a clear line of fire,
    // shields, cores, engines.
    let want: f32 = p
        .consumers()
        .iter()
        .filter(|c| {
            ship.part(c.part).clear
                && !(c.kind == sim::ConsumerKind::Engine && !d.parts[c.part as usize].is_engine)
        })
        .map(|c| c.draw)
        .sum();
    assert_eq!(p.power.demand, want);
    assert_eq!(p.power.served, want.min(p.power.supply));
}

// ---------- heat ----------

#[test]
fn heat_builds_while_working_and_vents() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let (mut busy, s1) = world_with(d.clone());
    let (mut idle, s2) = world_with(d.clone());
    for _ in 0..300 {
        busy.step(&inputs(s1, controls(3, 1.0, true, true)));
        idle.step(&inputs(s2, controls(3, 0.0, false, false)));
    }
    let beam = find(&d, "beam_laser", 5, 0);
    let hb = busy.ship(s1).unwrap().part(beam).heat;
    let hi = idle.ship(s2).unwrap().part(beam).heat;
    assert!(hb > 1.0 && hb > 3.0 * hi, "busy {hb} idle {hi}");
    // Heat conducts: the armour beside the fusion reactor warms up although it makes no heat itself.
    let armor = find(&d, "armor_block", 5, 4);
    assert!(busy.ship(s1).unwrap().part(armor).heat > 0.5);
    // Turn everything off: the ship cools down again.
    for _ in 0..1500 {
        busy.step(&inputs(s1, controls(3, 0.0, false, false)));
    }
    let cooled = busy.ship(s1).unwrap().part(beam).heat;
    assert!(cooled < 0.5 * hb, "{cooled} vs {hb}");
}

#[test]
fn radiators_vent_only_through_open_sides() {
    let c = content();
    // A fission reactor with a radiator to its right, open to space; and the same with the radiator
    // boxed in by armour (left of the reactor: core).
    let open = custom(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("fission_reactor", 1, 0, 0, false, None),
            ("radiator", 2, 0, 0, false, None),
        ],
    );
    let mut boxed_parts = vec![
        ("logic_core", 0, 1, 0, false, None),
        ("fission_reactor", 1, 1, 0, false, None),
        ("radiator", 2, 1, 0, false, None),
    ];
    for (x, y) in [(2, 0), (3, 1), (2, 2)] {
        boxed_parts.push(("armor_block", x, y, 0, false, Some("steel")));
    }
    let boxed = custom(&c, &boxed_parts);
    let run = |d: std::sync::Arc<sim::ShipDesign>| {
        let (mut w, s) = world_with(d.clone());
        for _ in 0..900 {
            w.step(&inputs(s, controls(0, 0.0, false, true)));
        }
        let ship = w.ship(s).unwrap();
        let r = d.parts.iter().position(|p| p.id == "radiator").unwrap() as u32;
        let f = d
            .parts
            .iter()
            .position(|p| p.id == "fission_reactor")
            .unwrap() as u32;
        (
            ship.part(r).open_sides,
            ship.part(f).heat,
            ship.pieces()[0].power.supply,
        )
    };
    let (open_sides, heat_open, supply_open) = run(open);
    let (boxed_sides, heat_boxed, supply_boxed) = run(boxed);
    assert_eq!(open_sides, 3);
    assert_eq!(boxed_sides, 0);
    assert_eq!(supply_open, 12.0, "venting radiator: +2");
    assert_eq!(supply_boxed, 10.0, "boxed-in radiator: no bonus");
    assert!(heat_open < heat_boxed, "{heat_open} vs {heat_boxed}");
}

#[test]
fn overheated_parts_shut_down_until_they_cool() {
    let c = content();
    // Three beams side by side on top of a fusion and a fission reactor: runs hot when firing.
    let d = custom(
        &c,
        &[
            ("beam_laser", 0, 0, 0, false, None),
            ("beam_laser", 1, 0, 0, false, None),
            ("beam_laser", 2, 0, 0, false, None),
            ("fusion_reactor", 0, 2, 0, false, None),
            ("fission_reactor", 2, 2, 0, false, None),
            ("logic_core", 2, 3, 0, false, None),
        ],
    );
    let (mut w, s) = world_with(d.clone());
    let n = d.parts.len();
    let mut prev = vec![false; n];
    let mut went_hot = 0;
    let mut recovered = 0;
    let check = |w: &sim::Sim, prev: &mut [bool], went_hot: &mut u32, recovered: &mut u32| {
        let ship = w.ship(s).unwrap();
        for (i, was_hot) in prev.iter_mut().enumerate() {
            let ps = ship.part(i as u32);
            let limit = ship.stats(i as u32).get(Stat::HeatLimit);
            if *was_hot {
                // Overheated when the tick started: shut down for the whole tick (no work, no draw).
                assert_eq!(ps.work, 0.0, "{}", d.parts[i].id);
            }
            if ps.overheated && !*was_hot {
                assert!(ps.heat > limit);
                *went_hot += 1;
            }
            if !ps.overheated && *was_hot {
                assert!(
                    ps.heat < limit * 0.75,
                    "hysteresis: restarts below 75% of the limit"
                );
                *recovered += 1;
            }
            *was_hot = ps.overheated;
        }
    };
    for _ in 0..3000 {
        w.step(&inputs(s, controls(0, 1.0, true, true)));
        check(&w, &mut prev, &mut went_hot, &mut recovered);
    }
    assert!(went_hot > 0, "firing for 100 s, something must overheat");
    for _ in 0..3000 {
        w.step(&inputs(s, controls(0, 0.0, false, false)));
        check(&w, &mut prev, &mut went_hot, &mut recovered);
    }
    assert!(recovered > 0);
    assert!(w.ship(s).unwrap().parts().iter().all(|p| !p.overheated));
}

// ---------- bonuses ----------

#[test]
fn capacitors_speed_up_lasers_up_to_max_stacks() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let (w, s) = world_with(d.clone());
    let ship = w.ship(s).unwrap();
    for (x, y) in [(5, 0), (6, 0)] {
        assert!(close(
            ship.stats(find(&d, "beam_laser", x, y)).get(Stat::FireRate),
            1.15,
            1e-6
        ));
    }
    // A pulse laser with four capacitors around it counts three. (Without chain reactions: two capacitor blasts
    // would take the 1-hit-point laser too.)
    let c = content_without_explosions();
    let d4 = custom(
        &c,
        &[
            ("pulse_laser", 1, 1, 0, false, None),
            ("capacitor", 0, 1, 0, false, None),
            ("capacitor", 2, 1, 0, false, None),
            ("capacitor", 1, 0, 0, false, None),
            ("capacitor", 1, 2, 0, false, None),
            ("logic_core", 1, 3, 0, false, None),
        ],
    );
    let (mut w4, s4) = world_with(d4.clone());
    let laser = find(&d4, "pulse_laser", 1, 1);
    assert!(close(
        w4.ship(s4).unwrap().stats(laser).get(Stat::FireRate),
        1.45,
        1e-6
    ));
    // Lose two capacitors: two remain.
    w4.apply_damage(s4, DamageTarget::Cell((0, 1)), 5.0);
    w4.apply_damage(s4, DamageTarget::Cell((2, 1)), 5.0);
    w4.step(&sim::TickInputs::default());
    assert!(close(
        w4.ship(s4).unwrap().stats(laser).get(Stat::FireRate),
        1.30,
        1e-6
    ));
    let g = w4.ship(s4).unwrap().pieces()[0].guns[0];
    assert!(close(g.dps, 1.0 * 1.30, 1e-6));
}

#[test]
fn radiator_starts_venting_when_its_neighbour_is_shot_away() {
    let c = content();
    // core | fission | radiator, the radiator boxed in by armour.
    let d = custom(
        &c,
        &[
            ("logic_core", 0, 1, 0, false, None),
            ("fission_reactor", 1, 1, 0, false, None),
            ("radiator", 2, 1, 0, false, None),
            ("armor_block", 2, 0, 0, false, Some("steel")),
            ("armor_block", 3, 1, 0, false, Some("steel")),
            ("armor_block", 2, 2, 0, false, Some("steel")),
        ],
    );
    let (mut w, s) = world_with(d.clone());
    w.step(&inputs(s, controls(0, 0.0, false, true)));
    assert_eq!(w.ship(s).unwrap().pieces()[0].power.supply, 10.0);
    // The armour block right of the radiator goes: the radiator vents and feeds the reactor.
    w.apply_damage(s, DamageTarget::Cell((3, 1)), 10.0);
    w.step(&inputs(s, controls(0, 0.0, false, true)));
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.part(find(&d, "radiator", 2, 1)).open_sides, 1);
    assert_eq!(ship.pieces()[0].power.supply, 12.0);
    assert_eq!(
        ship.stats(find(&d, "fission_reactor", 1, 1))
            .get(Stat::PowerOutput),
        12.0
    );
}

// ---------- engines ----------

/// Thrust and rated speed of an engine part in the content.
fn engine(c: &data::Content, id: &str) -> (f32, f32) {
    let e = c.parts.get(id).unwrap().engine.as_ref().unwrap();
    (e.thrust, e.rated_speed)
}

#[test]
fn runner_thrusts_straight_at_full_throttle() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let (thruster, thruster_speed) = engine(&c, "thruster");
    let (torch, torch_speed) = engine(&c, "torch_drive");
    let total = torch + 2.0 * thruster;
    let (mut w, s) = world_with(d.clone());
    let p = &w.ship(s).unwrap().pieces()[0];
    assert_eq!(p.thrusters.len(), 3);
    assert!(
        p.thrusters
            .iter()
            .all(|t| t.dir == Vec2::new(0.0, 1.0) && t.plume_length > 0.0)
    );
    let full = engine_wrench(&p.thrusters, |_| 1.0);
    assert!(close(full.force.x, 0.0, 1e-6) && close(full.force.y, total, 1e-3));
    assert!(full.torque.abs() < 1e-2, "torque {}", full.torque);
    assert!(close(p.authority.forward, total, 1e-3));
    assert!(close(p.authority.torque_ccw, p.authority.torque_cw, 1e-2));
    // Top speed per group: the fastest engine pushing that way (forward only).
    assert_eq!(
        p.authority.speed,
        [torch_speed.max(thruster_speed), 0.0, 0.0, 0.0]
    );
    // Only the left thruster: pushing forward on the left turns the ship clockwise (negative torque).
    let left = d.engines.iter().position(|e| e.point[0] < 6.0).unwrap();
    let mut th = [0.0; 3];
    th[left] = 1.0;
    let wl = engine_wrench(&p.thrusters, |slot| th[slot as usize]);
    assert!(wl.torque < -10.0, "{}", wl.torque);
    // The runtime wrench equals the geometric one when power is plentiful (at rest: no speed falloff).
    w.step(&inputs(s, controls(3, 1.0, false, true)));
    let p = &w.ship(s).unwrap().pieces()[0];
    assert!(close(p.wrench.force.y, total, 1e-3) && p.wrench.torque.abs() < 1e-2);
}

// ---------- guns ----------

#[test]
fn gun_mounts_and_line_of_fire() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let (mut w, s) = world_with(d.clone());
    let p = &w.ship(s).unwrap().pieces()[0];
    let oy = p.origin[1];
    let beams: Vec<_> = p
        .guns
        .iter()
        .filter(|g| d.parts[g.part as usize].id == "beam_laser")
        .collect();
    assert_eq!(beams.len(), 2);
    for g in &beams {
        assert!(g.clear);
        assert_eq!(g.dir, Vec2::new(0.0, 1.0));
        assert!(
            close(g.muzzle.x.abs(), 0.5, 1e-6) && close(g.muzzle.y, oy, 1e-6),
            "{:?}",
            g.muzzle
        );
    }
    // Every gun of the runner has a clear line of fire and fires.
    w.step(&inputs(s, controls(3, 0.0, true, true)));
    assert!(
        w.ship(s).unwrap().pieces()[0]
            .guns
            .iter()
            .all(|g| g.clear && g.output > 0.0)
    );

    // A pulse laser with armour in front: blocked until the armour is shot away.
    let d = custom(
        &c,
        &[
            ("armor_block", 0, 0, 0, false, Some("steel")),
            ("pulse_laser", 0, 1, 0, false, None),
            ("fission_reactor", 1, 1, 0, false, None),
            ("logic_core", 0, 2, 0, false, None),
        ],
    );
    let (mut w, s) = world_with(d.clone());
    let pulse = find(&d, "pulse_laser", 0, 1);
    assert!(!w.ship(s).unwrap().part(pulse).clear);
    w.apply_damage(s, DamageTarget::Cell((0, 0)), 10.0);
    w.step(&inputs(s, controls(0, 0.0, true, true)));
    let ship = w.ship(s).unwrap();
    assert!(ship.part(pulse).clear);
    let g = ship.pieces()[0]
        .guns
        .iter()
        .find(|g| g.part == pulse)
        .unwrap();
    assert!(g.output > 0.0);
}
