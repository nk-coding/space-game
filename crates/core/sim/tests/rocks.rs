//! Asteroid rock (`sim::voyage::rocks`, GDR-0104): the roll keeps rocks clear of sites and the entry and inside the
//! cluster, outlines are convex, fields spawn lazily as anchored bodies, shots and rams stop on them, and behaviors'
//! maneuvers steer round them.

mod common;

use common::*;
use sim::voyage::rocks::{FieldPlan, Mover, Rock, outline, steer};
use sim::{Body, Maneuver, Owner, ShipCommand, Sim, SimEvent, TickInputs, Vec2, VoyagePlan};

/// A voyage with no sites and one rock of `radius` at `at` (spawned when a player vessel comes near).
fn rock_plan(at: Vec2, radius: f32) -> VoyagePlan {
    let points = (0..8)
        .map(|k| {
            let a = k as f32 * core::f32::consts::TAU / 8.0;
            Vec2::new(libm::cosf(a), libm::sinf(a)) * radius
        })
        .collect();
    let rock = Rock { kind: "iron_rock".into(), ram: 0.5, pos: at, angle: 0.0, points, radius };
    VoyagePlan {
        voyage: "rocks".into(),
        seed: 1,
        radius: 1000.0,
        entry: Body::at(Vec2::new(0.0, -1000.0), 0.0),
        sites: Vec::new(),
        star: None,
        rocks: vec![FieldPlan { centre: at, reach: radius, rocks: vec![rock] }],
    }
}

fn world(at: Vec2, radius: f32) -> Sim {
    let c = content();
    let mut w = Sim::new(1);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    w.start_voyage(rock_plan(at, radius), Owner::PLAYER, Owner(1));
    w
}

#[test]
fn the_roll_keeps_rocks_inside_and_clear_of_sites_and_the_entry() {
    let c = content();
    for (id, seeds) in [("yard_approach", 1..20), ("corran_reach", 1..20)] {
        let v = c.voyages.get(id).unwrap();
        for seed in seeds {
            let a = VoyagePlan::roll(&c, v, seed).unwrap();
            let b = VoyagePlan::roll(&c, v, seed).unwrap();
            assert_eq!(a.rocks, b.rocks, "{id} seed {seed}: the same rocks every roll");
            let rocks: Vec<&Rock> = a.rocks.iter().flat_map(|f| &f.rocks).collect();
            assert!(!rocks.is_empty(), "{id} seed {seed}: rocks placed");
            for (k, r) in rocks.iter().enumerate() {
                assert!(r.pos.length() + r.radius <= a.radius + 1e-3);
                assert!((r.pos - a.entry.pos).length() >= r.radius + sim::voyage::rocks::ENTRY_CLEAR - 1e-3);
                for s in &a.sites {
                    assert!((r.pos - s.pos).length() >= r.radius + sim::voyage::rocks::SITE_CLEAR - 1e-3);
                }
                for o in &rocks[..k] {
                    assert!((r.pos - o.pos).length() >= r.radius + o.radius + sim::voyage::rocks::ROCK_GAP - 1e-3);
                }
            }
            for f in &a.rocks {
                for r in &f.rocks {
                    assert!((r.pos - f.centre).length() + r.radius <= f.reach + 1e-3, "a field's reach holds its rocks");
                }
            }
        }
    }
    // The rocks come after the sites: the sites lie where a roll without scenery puts them.
    let mut bare = c.voyages.get("corran_reach").unwrap().clone();
    bare.scenery = Default::default();
    let with = VoyagePlan::roll(&c, c.voyages.get("corran_reach").unwrap(), 3).unwrap();
    let without = VoyagePlan::roll(&c, &bare, 3).unwrap();
    assert!(without.rocks.is_empty());
    assert_eq!(with.sites.iter().map(|s| s.pos).collect::<Vec<_>>(), without.sites.iter().map(|s| s.pos).collect::<Vec<_>>());
}

#[test]
fn outlines_are_convex_and_within_the_kinds_size() {
    let c = content();
    for kind in c.rocks.iter() {
        let mut rng = sim::Pcg32::new(7, 1);
        for _ in 0..50 {
            let pts = outline(kind, &mut rng);
            assert!(pts.len() >= 3);
            let reach = pts.iter().fold(0.0f32, |m, p| m.max(p.length()));
            assert!(reach <= kind.size.1 * kind.stretch.1.sqrt() + 1e-3);
            for k in 0..pts.len() {
                let (a, b, n) = (pts[k], pts[(k + 1) % pts.len()], pts[(k + 2) % pts.len()]);
                assert!((b - a).cross(n - b) > 0.0, "{}: counter-clockwise and convex", kind.id);
            }
        }
    }
}

#[test]
fn a_field_spawns_when_the_fleet_comes_near() {
    let c = content();
    let mut w = world(Vec2::new(0.0, 900.0), 12.0);
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::new(0.0, -900.0), 0.0));
    w.step(&TickInputs::default());
    assert!(w.voyage().unwrap().rocks().is_empty(), "far off: still a record");
    let piece = w.ship(me).unwrap().control_piece().unwrap();
    w.teleport(me, piece, Body::at(Vec2::new(0.0, 400.0), 0.0));
    w.step(&TickInputs::default());
    assert_eq!(w.voyage().unwrap().rocks().len(), 1, "within the spawn radius: spawned");
    let key = sim::BodyKey::rock(0);
    assert_eq!(w.physics().body(key).map(|b| b.pos), Some(Vec2::new(0.0, 900.0)));
}

#[test]
fn shots_stop_on_a_rock_and_rams_hurt_only_the_ship() {
    let c = content();
    let mut w = world(Vec2::new(0.0, 40.0), 10.0);
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
    w.command(me, ShipCommand::Maneuver { maneuver: Maneuver::Brake, fire: true });
    let (mut rock_hits, mut hits) = (0, 0);
    for _ in 0..60 {
        w.step(&TickInputs::default());
        for e in w.events() {
            match e {
                SimEvent::RockHit { rock: 0, .. } => rock_hits += 1,
                SimEvent::Hit { .. } => hits += 1,
                _ => {}
            }
        }
    }
    assert!(rock_hits > 0 && hits == 0, "{rock_hits} shots on the rock, {hits} on hulls");

    // Rammed at 30 m/s: the ship takes the ram, the rock stays put.
    let mut w = world(Vec2::new(0.0, 40.0), 10.0);
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
    w.step(&TickInputs::default());
    let piece = w.ship(me).unwrap().control_piece().unwrap();
    w.teleport(me, piece, Body { linvel: Vec2::new(0.0, 30.0), ..Body::at(Vec2::ZERO, 0.0) });
    let mut rammed = 0.0;
    for _ in 0..90 {
        w.step(&TickInputs::default());
        for e in w.events() {
            if let SimEvent::Impact(i) = e
                && i.b == sim::BodyKey::rock(0)
            {
                assert_eq!(i.a.ship, me);
                assert!(i.part_b.is_none());
                rammed += i.damage;
            }
        }
    }
    assert!(rammed > 0.0, "the ram dealt damage");
    assert_eq!(w.physics().body(sim::BodyKey::rock(0)).map(|b| b.pos), Some(Vec2::new(0.0, 40.0)));
}

#[test]
fn maneuvers_steer_round_a_rock_in_the_way() {
    let c = content();
    let mut w = world(Vec2::new(0.0, 150.0), 20.0);
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
    w.command(me, ShipCommand::Maneuver { maneuver: Maneuver::Hold { at: Vec2::new(0.0, 300.0) }, fire: false });
    let mut touched = false;
    let mut end = Vec2::ZERO;
    for _ in 0..(30 * 60) {
        w.step(&TickInputs::default());
        touched |= w.events().iter().any(|e| matches!(e, SimEvent::Impact(i) if i.b == sim::BodyKey::rock(0)));
        let s = w.ship(me).unwrap();
        end = s.piece(s.control_piece().unwrap()).unwrap().body.pos;
    }
    assert!(!touched, "it never touched the rock");
    assert!((end - Vec2::new(0.0, 300.0)).length() < 10.0, "it got there: {end:?}");
}

#[test]
fn steer_leaves_a_clear_way_alone_and_turns_aside_for_a_rock() {
    let rock = Rock { kind: "iron_rock".into(), ram: 0.5, pos: Vec2::new(0.0, 50.0), angle: 0.0, points: Vec::new(), radius: 10.0 };
    let v = Vec2::new(0.0, 20.0);
    let at = |x: f32| Mover { pos: Vec2::new(x, 0.0), radius: 5.0, vel: Vec2::ZERO, accel: 10.0 };
    assert_eq!(steer(&at(0.0), v, &[]), v);
    assert_eq!(steer(&at(100.0), v, std::slice::from_ref(&rock)), v, "the rock is off to the side");
    let bent = steer(&at(1.0), v, std::slice::from_ref(&rock));
    assert!((bent.length() - 20.0).abs() < 1e-3, "same speed");
    assert!(bent.x > 0.0, "it passes on the side it was already on: {bent:?}");
}

#[test]
fn steer_slows_for_a_gap_too_narrow_and_bends_a_ship_carried_into_a_rock() {
    let rock = |x: f32| Rock { kind: "iron_rock".into(), ram: 0.5, pos: Vec2::new(x, 60.0), angle: 0.0, points: Vec::new(), radius: 10.0 };
    // Two rocks 4 m apart edge to edge: no way through for a 5 m ship.
    let pair = [rock(-12.0), rock(12.0)];
    let m = Mover { pos: Vec2::ZERO, radius: 5.0, vel: Vec2::ZERO, accel: 10.0 };
    // At 40 m/s it could not stop before the gaps (2·10·d < 40²): it slows to what it can stop from.
    let out = steer(&m, Vec2::new(0.0, 40.0), &pair);
    assert!(out.length() < 40.0, "it slows: {out:?}");
    // Coasting at 20 m/s straight at a rock with the goal to stop: the motion is bent aside first.
    let m = Mover { vel: Vec2::new(0.0, 20.0), ..m };
    let one = [rock(0.0)];
    assert!(m.heading_into(&one).is_some());
    let out = steer(&m, Vec2::ZERO, &one);
    assert!(out.x.abs() > 1.0, "across the rock's way: {out:?}");
}
