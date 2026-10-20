//! Asteroid rock (`sim::voyage::rocks`, GDR-0104): the roll keeps rocks clear of sites and the entry and inside the
//! cluster, outlines are star-shaped and covered by their convex pieces, fields spawn lazily as anchored bodies, shots and rams stop on them, and behaviors'
//! maneuvers steer round them.

mod common;

use common::*;
use sim::voyage::rocks::{FieldPlan, Mover, OUTLINE_POINTS, Rock, curve, outline, pieces, steer};
use sim::{Body, Maneuver, Owner, ShipCommand, Sim, SimEvent, TickInputs, Vec2, VoyagePlan};

/// A voyage with no sites and one rock of `radius` at `at` (spawned when a player vessel comes near).
fn rock_plan(at: Vec2, radius: f32) -> VoyagePlan {
    let points: Vec<Vec2> = (0..8)
        .map(|k| {
            let a = k as f32 * core::f32::consts::TAU / 8.0;
            Vec2::new(libm::cosf(a), libm::sinf(a)) * radius
        })
        .collect();
    let rock = Rock { kind: "iron_rock".into(), ram: 0.5, material: "nickel_iron".into(), ore: 0, cut: 0.0, pos: at, angle: 0.0, parts: vec![points.clone()], points, radius };
    VoyagePlan {
        voyage: "rocks".into(),
        seed: 1,
        radius: 1000.0,
        entry: Body::at(Vec2::new(0.0, -1000.0), 0.0),
        sites: Vec::new(),
        star: None,
        radiation: 0.0,
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
fn outlines_are_star_shaped_within_the_kinds_size_and_covered_by_their_pieces() {
    let c = content();
    let mut split = [0; 3];
    for kind in c.rocks.iter() {
        let mut rng = sim::Pcg32::new(7, 1);
        for _ in 0..50 {
            let pts = outline(kind, &mut rng);
            assert!(pts.len() >= 6);
            let reach = pts.iter().fold(0.0f32, |m, p| m.max(p.length()));
            assert!(reach <= kind.size.1 * kind.stretch.1.sqrt() + 1e-3);
            let line = curve(&pts, OUTLINE_POINTS);
            for k in 0..line.len() {
                let (a, b) = (line[k], line[(k + 1) % line.len()]);
                assert!(a.cross(b) > 0.0, "{}: star-shaped round the centre, counter-clockwise", kind.id);
            }
            let r = line.iter().fold(0.0f32, |m, p| m.max(p.length()));
            let parts = pieces(&line, r);
            assert!((1..=3).contains(&parts.len()));
            split[parts.len() - 1] += 1;
            for p in &parts {
                for k in 0..p.len() {
                    let (a, b, n) = (p[k], p[(k + 1) % p.len()], p[(k + 2) % p.len()]);
                    assert!((b - a).cross(n - b) > -1e-4, "{}: each piece counter-clockwise and convex", kind.id);
                }
            }
            // Every point of the drawn outline lies on or inside a piece.
            let inside = |p: &Vec<Vec2>, q: Vec2, slack: f32| (0..p.len()).all(|k| (p[(k + 1) % p.len()] - p[k]).cross(q - p[k]) >= -slack * (p[(k + 1) % p.len()] - p[k]).length());
            for &q in &line {
                assert!(parts.iter().any(|p| inside(p, q, 1e-4 * r)), "{}: the outline within its pieces", kind.id);
            }
        }
    }
    println!("rocks in one, two and three pieces: {split:?}");
    assert!(split[1] + split[2] > 0, "deeply dented rocks are cut into pieces");
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
fn a_rock_in_pieces_leaves_its_dents_open() {
    let c = content();
    let square = |x0: f32, x1: f32| vec![Vec2::new(x0, -10.0), Vec2::new(x1, -10.0), Vec2::new(x1, 10.0), Vec2::new(x0, 10.0)];
    let mut plan = rock_plan(Vec2::new(0.0, 40.0), 15.0);
    plan.rocks[0].rocks[0].parts = vec![square(-10.0, -1.0), square(1.0, 10.0)];
    let mut w = Sim::new(1);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    w.start_voyage(plan, Owner::PLAYER, Owner(1));
    w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
    w.step(&TickInputs::default());
    assert_eq!(w.voyage().unwrap().rocks().len(), 1);
    assert!(!w.rock_between(Vec2::new(0.0, 20.0), Vec2::new(0.0, 60.0)), "through the gap between its pieces");
    assert!(w.rock_between(Vec2::new(5.0, 20.0), Vec2::new(5.0, 60.0)), "through a piece");
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
    let rock = Rock { kind: "iron_rock".into(), ram: 0.5, material: "nickel_iron".into(), ore: 0, cut: 0.0, pos: Vec2::new(0.0, 50.0), angle: 0.0, points: Vec::new(), parts: Vec::new(), radius: 10.0 };
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
    let rock = |x: f32| Rock { kind: "iron_rock".into(), ram: 0.5, material: "nickel_iron".into(), ore: 0, cut: 0.0, pos: Vec2::new(x, 60.0), angle: 0.0, points: Vec::new(), parts: Vec::new(), radius: 10.0 };
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

#[test]
fn steer_pushes_a_ship_off_a_rock_it_rests_on_and_out_of_the_nearest_gap() {
    let rock = |y: f32| Rock { kind: "iron_rock".into(), ram: 0.5, material: "nickel_iron".into(), ore: 0, cut: 0.0, pos: Vec2::new(0.0, y), angle: 0.0, points: Vec::new(), parts: Vec::new(), radius: 10.0 };
    // At rest against a rock (the circles overlap) with the goal to stop: pushed straight off it.
    let m = Mover { pos: Vec2::new(0.0, 14.0), radius: 5.0, vel: Vec2::ZERO, accel: 10.0 };
    let out = steer(&m, Vec2::ZERO, &[rock(0.0)]);
    assert!(out.y > 1.0 && out.x.abs() < 1e-3, "off the rock: {out:?}");
    // Clear of it, only inside its gap: left to stop.
    let clear = Mover { pos: Vec2::new(0.0, 18.0), ..m };
    assert_eq!(steer(&clear, Vec2::ZERO, &[rock(0.0)]), Vec2::ZERO);
    // Inside two rocks' gaps, the farther listed first: out of the nearer one's, not pushed onto it.
    let out = steer(&m, Vec2::new(0.0, -10.0), &[rock(35.0), rock(0.0)]);
    assert!(out.y > 0.0, "away from the nearer rock: {out:?}");
}

#[test]
fn a_turn_blocked_the_short_way_turns_the_long_way_round() {
    let mut a = sim::flight::TurnAssist { on: true, ..Default::default() };
    // Not turning with the last resort on and the target 2.5 rad clockwise: the short way for a while ...
    for _ in 0..9 {
        assert_eq!(a.error(-2.5, 0.0), -2.5);
    }
    // ... then the long way round, counter-clockwise, also once past the half turn (the short way flips sides).
    let tau = core::f32::consts::TAU;
    assert!((a.error(-2.5, 0.0) - (tau - 2.5)).abs() < 1e-5);
    assert!((a.error(-3.1, 0.4) - (tau - 3.1)).abs() < 1e-5);
    assert!((a.error(2.9, 0.4) - 2.9).abs() < 1e-5);
    // Near the target the short way again; a spin passing through the still rate for a tick is no block.
    assert_eq!(a.error(1.5, 0.4), 1.5);
    assert_eq!(a.error(-2.5, 0.0), -2.5);
    assert_eq!(a.error(-2.5, 0.3), -2.5);
    assert_eq!(a.long_way, 0);
}
