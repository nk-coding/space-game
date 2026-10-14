//! Rapier physics as the sim sees it: splits, collisions and ramming, ray casts, ghosts, mass properties,
//! sleeping.

mod common;

use core::f32::consts::PI;

use common::*;
use sim::mass::to_piece;
use sim::{
    Body, BodyKey, Contact, DamageTarget, Owner, PartIdx, ShipId, SimEvent, TickInputs, Vec2,
    inherited_velocity,
};

fn key(w: &sim::Sim, ship: ShipId, k: usize) -> BodyKey {
    BodyKey {
        ship,
        piece: w.ship(ship).unwrap().pieces()[k].id,
    }
}

fn impacts(w: &sim::Sim) -> Vec<Contact> {
    w.events()
        .iter()
        .filter_map(|e| match e {
            SimEvent::Impact(c) => Some(*c),
            _ => None,
        })
        .collect()
}

fn idle(w: &mut sim::Sim, ticks: u32) -> Vec<Contact> {
    let mut all = Vec::new();
    for _ in 0..ticks {
        w.step(&TickInputs::default());
        all.extend(impacts(w));
    }
    all
}

/// A raider of `owner` at `pos` facing `angle`, coasting at `vel`.
fn raider(w: &mut sim::Sim, owner: Owner, pos: Vec2, angle: f32, vel: Vec2) -> ShipId {
    let c = content();
    w.spawn_ship(
        design(&c, "automaton_raider"),
        owner,
        Body {
            pos,
            angle,
            linvel: vel,
            angvel: 0.0,
        },
    )
}

#[test]
fn split_pieces_carry_their_inherited_velocity_in_rapier() {
    // Without chain reactions: the fusion reactor's blast (GDR-0085) would take a ring of parts around the cut,
    // and this test is about the split.
    let c = content_without_explosions();
    let d = design(&c, "halvex_runner");
    let mut w = sim::Sim::new(1);
    let body = Body {
        pos: Vec2::new(10.0, 5.0),
        angle: 0.4,
        linvel: Vec2::new(3.0, -1.0),
        angvel: 0.7,
    };
    let s = w.spawn_ship(d.clone(), Owner::PLAYER, body);
    let parent = w.ship(s).unwrap().pieces()[0].clone();
    let com_old = parent.com_world();
    // The centre column: the runner splits down the middle. Each half keeps a logic core, so the second
    // half is a vessel of its own: a new ship with one piece (its body moved to the new key).
    for cell in [(5, 0), (6, 0), (5, 2), (5, 4), (6, 4), (5, 5)] {
        w.apply_damage(s, DamageTarget::Cell(cell), 100.0);
    }
    w.resolve_breakage();
    assert_eq!(w.ships().len(), 2);
    let ships = [s, w.ships()[1].id()];
    // Rapier holds the velocities the sim handed over (centre of mass moved, new body created).
    for &id in &ships {
        let ship = w.ship(id).unwrap();
        assert_eq!(ship.pieces().len(), 1);
        let p = &ship.pieces()[0];
        let want = inherited_velocity(body.linvel, body.angvel, com_old, p.com_world());
        let rb = w.physics().body(key(&w, id, 0)).unwrap();
        assert!((rb.linvel - want).length() < 1e-4, "{rb:?} vs {want:?}");
        assert_eq!(rb.angvel, body.angvel);
        assert!((rb.pos - p.body.pos).length() < 1e-5 && (rb.angle - p.body.angle).abs() < 1e-6);
    }
    // One free step later both halves still move with the old body (to second order in DT), and the
    // velocities are unchanged: no forces, and the two halves are ghosts.
    let before: Vec<Option<Vec2>> = parts_world(&w, &ships);
    let rigid = |p: Vec2| {
        let dt = sim::DT;
        let r = sim::Rot::from_angle(body.angvel * dt).apply(p - com_old);
        com_old + body.linvel * dt + r
    };
    w.step(&TickInputs::default());
    assert!(impacts(&w).is_empty());
    let after = parts_world(&w, &ships);
    for (p0, p1) in before.iter().zip(&after) {
        if let (Some(p0), Some(p1)) = (p0, p1) {
            assert!((rigid(*p0) - *p1).length() < 2e-3, "{p0:?} → {p1:?}");
        }
    }
    let pieces: Vec<&sim::Piece> = ships
        .iter()
        .flat_map(|&id| w.ship(id).unwrap().pieces())
        .collect();
    let (m, mom) = pieces.iter().fold((0.0, Vec2::ZERO), |(m, mom), p| {
        (m + p.mass.mass, mom + p.body.linvel * p.mass.mass)
    });
    let c_rem = pieces
        .iter()
        .fold(Vec2::ZERO, |c, p| c + p.com_world() * p.mass.mass)
        * (1.0 / m);
    let want = inherited_velocity(
        body.linvel,
        body.angvel,
        com_old,
        c_rem - body.linvel * sim::DT,
    );
    assert!((mom * (1.0 / m) - want).length() < 1e-3, "{mom:?}");
}

/// World centroid of every alive part of one blueprint, wherever its ship (a split-off vessel holds its own
/// parts).
fn parts_world(w: &sim::Sim, ships: &[ShipId]) -> Vec<Option<Vec2>> {
    let d = w.ship(ships[0]).unwrap().design().clone();
    (0..d.parts.len())
        .map(|i| {
            ships.iter().find_map(|&s| {
                let ship = w.ship(s).unwrap();
                let ps = ship.part(i as PartIdx);
                ps.alive.then(|| {
                    let p = ship.piece(ps.piece).unwrap();
                    p.body.to_world(to_piece(d.parts[i].centroid, p.origin))
                })
            })
        })
        .collect()
}

#[test]
fn head_on_ram_reports_an_impact_and_damages_both_noses() {
    let mut w = sim::Sim::new(1);
    // Nose to nose, 8 m/s each, of two owners (one owner's vessels deal no ram damage, GDR-0045).
    let a = raider(&mut w, Owner(1), Vec2::ZERO, 0.0, Vec2::new(0.0, 8.0));
    let b = raider(
        &mut w,
        Owner(2),
        Vec2::new(0.0, 12.0),
        PI,
        Vec2::new(0.0, -8.0),
    );
    let hp0: Vec<Vec<f32>> = [a, b]
        .iter()
        .map(|&s| w.ship(s).unwrap().parts().iter().map(|p| p.hp).collect())
        .collect();
    let m = w.ship(a).unwrap().pieces()[0].mass.mass;
    let all = idle(&mut w, 30);
    let hit = all.first().copied().expect("they collide");
    let (ka, kb) = (key(&w, a, 0), key(&w, b, 0));
    assert_eq!((hit.a, hit.b), (ka, kb));
    // The normal points from a to b (+y); they closed at 16 m/s.
    assert!(hit.normal.y > 0.99, "{hit:?}");
    assert!((hit.relative_speed - 16.0).abs() < 0.5, "{hit:?}");
    assert!(
        (hit.point.y - 6.0).abs() < 0.5 && hit.point.x.abs() < 1.0,
        "{hit:?}"
    );
    // Impulse ≈ (1 + e) · μ · v with the reduced mass μ = m / 2.
    let want = 1.2 * (m / 2.0) * 16.0;
    assert!(
        hit.impulse > 0.6 * want && hit.impulse < 1.3 * want,
        "{} vs {want}",
        hit.impulse
    );
    // Damage = (impulse − threshold) × per-impulse damage, to the noses of both.
    let t = w.config().physics;
    assert!((hit.damage - (hit.impulse - t.ram_threshold) * t.ram_damage).abs() < 1e-5);
    assert!(hit.damage > 1.0, "{hit:?}");
    let cannon = |s: ShipId| {
        let d = w.ship(s).unwrap().design().clone();
        d.parts
            .iter()
            .position(|p| p.id == "automaton_cannon")
            .unwrap() as PartIdx
    };
    assert_eq!((hit.part_a, hit.part_b), (Some(cannon(a)), Some(cannon(b))));
    for (n, s) in [a, b].into_iter().enumerate() {
        let ship = w.ship(s).unwrap();
        let p = cannon(s);
        let st = ship.part(p);
        assert!(
            !st.alive || (hp0[n][p as usize] - st.hp - hit.damage).abs() < 1e-4,
            "{st:?}"
        );
    }
    // They bounced: both now move apart (restitution).
    let va = w.ship(a).unwrap().pieces()[0].body.linvel;
    let vb = w.ship(b).unwrap().pieces()[0].body.linvel;
    assert!(va.y < 0.0 && vb.y > 0.0, "{va:?} {vb:?}");
}

#[test]
fn a_gentle_bump_is_an_impact_without_damage() {
    let mut w = sim::Sim::new(1);
    let a = raider(&mut w, Owner(1), Vec2::ZERO, 0.0, Vec2::new(0.0, 0.5));
    let b = raider(
        &mut w,
        Owner(2),
        Vec2::new(0.0, 3.5),
        PI,
        Vec2::new(0.0, -0.5),
    );
    let all = idle(&mut w, 60);
    assert!(!all.is_empty(), "they touch");
    assert!(
        all.iter().all(|c| c.damage == 0.0 && c.impulse < 20.0),
        "{all:?}"
    );
    for s in [a, b] {
        let ship = w.ship(s).unwrap();
        assert!(
            ship.parts()
                .iter()
                .zip(&ship.design().parts)
                .all(|(p, d)| p.hp == d.hp)
        );
    }
}

#[test]
fn raycasts_skip_every_piece_of_the_own_ship() {
    let c = content();
    let mut w = sim::Sim::new(1);
    let me = w.spawn_ship(design(&c, "automaton_raider"), Owner(1), Body::default());
    let other = w.spawn_ship(
        design(&c, "automaton_raider"),
        Owner(2),
        Body::at(Vec2::new(0.0, 30.0), 0.0),
    );
    // From inside the own hull straight up: without exclusion the own hull is hit at once.
    let own = w.raycast(Vec2::ZERO, Vec2::Y, 100.0, None).unwrap();
    assert_eq!((own.ship, own.hit.distance), (me, 0.0));
    let hit = w.raycast(Vec2::ZERO, Vec2::Y, 100.0, Some(me)).unwrap();
    assert_eq!(hit.ship, other);
    assert!(
        hit.hit.distance > 27.0 && hit.hit.distance < 29.0,
        "{hit:?}"
    );
    assert!(w.ship(other).unwrap().part(hit.part).alive);
    // Split the own ship (the middle column goes): both pieces are skipped.
    for cell in [(1, 0), (1, 1), (1, 2)] {
        w.apply_damage(me, DamageTarget::Cell(cell), 100.0);
    }
    w.resolve_breakage();
    assert_eq!(w.ship(me).unwrap().pieces().len(), 2);
    let left = w.ship(me).unwrap().pieces()[0].com_world();
    let from = left - Vec2::new(0.0, 5.0);
    assert_eq!(w.raycast(from, Vec2::Y, 100.0, None).unwrap().ship, me);
    let hit = w.raycast(from, Vec2::Y, 100.0, Some(me));
    assert!(hit.is_none_or(|h| h.ship != me), "{hit:?}");
    // Projectiles never hit their own ship either: fired from inside the other hull down at the left
    // piece, they leave their own ship and hit mine.
    w.spawn_projectile(
        other,
        data::WeaponFamily::Kinetic,
        Vec2::new(left.x, 30.0),
        Vec2::new(0.0, -100.0),
        0.5,
        1,
        1.0,
    );
    let mut hit_ships = Vec::new();
    for _ in 0..15 {
        w.step(&TickInputs::default());
        for e in w.events() {
            if let SimEvent::Hit { ship, .. } = e {
                hit_ships.push(*ship);
            }
        }
    }
    assert!(
        !hit_ships.is_empty() && hit_ships.iter().all(|&s| s == me),
        "{hit_ships:?}"
    );
}

#[test]
fn split_pieces_ghost_each_other_then_collide() {
    let mut w = sim::Sim::new(1);
    let s = raider(&mut w, Owner(1), Vec2::ZERO, 0.0, Vec2::ZERO);
    // Cut the middle column: a left and a right piece, 1 m apart.
    for cell in [(1, 0), (1, 1), (1, 2)] {
        w.apply_damage(s, DamageTarget::Cell(cell), 100.0);
    }
    w.step(&TickInputs::default());
    let (l, r) = (key(&w, s, 0), key(&w, s, 1));
    assert!(w.physics().is_ghost(l) == Some(true) && w.physics().is_ghost(r) == Some(true));
    let launch = |w: &mut sim::Sim, k: BodyKey, vx: f32| {
        let mut b = w.ship(s).unwrap().piece(k.piece).unwrap().body;
        b.linvel = Vec2::new(vx, 0.0);
        b.angvel = 0.0;
        w.teleport(s, k.piece, b);
    };
    // While they are ghosts, the left piece flies through the right one.
    let start_l = w.ship(s).unwrap().piece(l.piece).unwrap().body;
    let start_r = w.ship(s).unwrap().piece(r.piece).unwrap().body;
    launch(&mut w, l, 9.0);
    let ghost = w.config().split_ghost_ticks;
    let during = idle(&mut w, ghost - 2);
    assert!(during.is_empty(), "{during:?}");
    assert_eq!(
        w.ship(s).unwrap().piece(l.piece).unwrap().body.linvel,
        Vec2::new(9.0, 0.0)
    );
    // After the grace period the same approach is a collision.
    idle(&mut w, 3);
    assert!(w.physics().is_ghost(l) == Some(false) && w.physics().is_ghost(r) == Some(false));
    w.teleport(s, l.piece, start_l);
    w.teleport(s, r.piece, start_r);
    launch(&mut w, l, 9.0);
    let after = idle(&mut w, 10);
    assert!(
        after.iter().any(|c| (c.a, c.b) == (l, r)),
        "pieces of one ship collide once the ghost time is over: {after:?}"
    );
    assert!(w.ship(s).unwrap().piece(l.piece).unwrap().body.linvel.x < 8.0);
}

#[test]
fn rapier_mass_properties_are_the_pieces() {
    let content = content();
    let mut sc = sim::scenario::Scenario::new(&content, 9).unwrap();
    let mut checked = 0;
    for t in 0..400 {
        sc.step();
        if t % 50 != 0 && t != 399 {
            continue;
        }
        for ship in sc.sim.ships() {
            for p in ship.pieces() {
                let k = BodyKey {
                    ship: ship.id(),
                    piece: p.id,
                };
                let m = sc.sim.physics().mass_props(k).unwrap();
                assert_eq!(m.com, p.mass.com, "{k:?} at tick {t}");
                assert!(
                    (m.mass - p.mass.mass).abs() <= 1e-6 * p.mass.mass,
                    "{m:?} {:?}",
                    p.mass
                );
                assert!(
                    (m.inertia - p.mass.inertia).abs() <= 1e-5 * p.mass.inertia,
                    "{m:?} {:?}",
                    p.mass
                );
                checked += 1;
            }
        }
    }
    assert!(
        sc.stats.splits > 0 && sc.stats.parts_destroyed > 0,
        "{:?}",
        sc.stats
    );
    assert!(checked > 100);
    assert_eq!(sc.sim.physics().len(), {
        sc.sim
            .ships()
            .iter()
            .map(|s| s.pieces().len())
            .sum::<usize>()
    });
}

#[test]
fn sleeping_bodies_wake_on_thrust_and_on_hits() {
    let c = content();
    let d = design(&c, "automaton_raider");
    let mut w = sim::Sim::new(1);
    let parked = w.spawn_ship(d.clone(), Owner(1), Body::default());
    let other = w.spawn_ship(d.clone(), Owner(1), Body::at(Vec2::new(30.0, 0.0), 0.0));
    let sleep = (w.config().physics.sleep_time / sim::DT) as u32 + 15;
    idle(&mut w, sleep);
    let (kp, ko) = (key(&w, parked, 0), key(&w, other, 0));
    assert_eq!(w.physics().is_sleeping(kp), Some(true));
    assert_eq!(w.physics().is_sleeping(ko), Some(true));
    // Thrust wakes it.
    let engines = d.engines.len();
    w.step(&inputs(parked, controls(engines, 1.0, false, true)));
    w.step(&inputs(parked, controls(engines, 0.0, false, true)));
    assert_eq!(w.physics().is_sleeping(kp), Some(false));
    assert!(w.ship(parked).unwrap().pieces()[0].body.linvel.y > 0.0);
    // A ram wakes the sleeping one and pushes it.
    w.teleport(
        parked,
        kp.piece,
        Body {
            linvel: Vec2::new(6.0, 0.0),
            ..Body::at(Vec2::new(20.0, 0.0), 0.0)
        },
    );
    let all = idle(&mut w, 60);
    assert!(all.iter().any(|c| (c.a, c.b) == (kp, ko)), "{all:?}");
    assert!(w.ship(other).unwrap().pieces()[0].body.linvel.x > 1.0);
}
