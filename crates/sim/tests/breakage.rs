//! Damage, breakage, split events and inherited velocities.

mod common;

use common::*;
use sim::mass::to_piece;
use sim::{
    Body, DamageCommand, DamageOutcome, DamageTarget, PartIdx, ShipId, SimEvent, TickInputs, Vec2,
    inherited_velocity,
};

fn kill(ship: ShipId, cells: &[(i32, i32)]) -> TickInputs {
    TickInputs {
        controls: vec![],
        damage: cells
            .iter()
            .map(|&c| DamageCommand {
                ship,
                target: DamageTarget::Cell(c),
                amount: 100.0,
            })
            .collect(),
    }
}

/// World position of every alive part's centroid.
fn part_positions(w: &sim::Sim, s: ShipId) -> Vec<Option<Vec2>> {
    let ship = w.ship(s).unwrap();
    let d = ship.design();
    (0..d.parts.len())
        .map(|i| {
            let ps = ship.part(i as PartIdx);
            if !ps.alive {
                return None;
            }
            let p = ship.piece(ps.piece).unwrap();
            Some(p.body.to_world(to_piece(d.parts[i].centroid, p.origin)))
        })
        .collect()
}

#[test]
fn inherited_velocity_is_rigid_body_motion() {
    // ω = 2 rad/s ccw, the new centre 1 m to the right of the old one: it moves up at 2 m/s extra.
    let v = inherited_velocity(Vec2::new(1.0, 2.0), 2.0, Vec2::ZERO, Vec2::new(1.0, 0.0));
    assert_eq!(v, Vec2::new(1.0, 4.0));
    assert_eq!(inherited_velocity(Vec2::new(1.0, 2.0), 0.0, Vec2::ZERO, Vec2::new(5.0, 5.0)), Vec2::new(1.0, 2.0));
}

#[test]
fn runner_splits_down_the_middle() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let mut w = sim::Sim::new(1);
    let body = Body {
        pos: Vec2::new(10.0, 5.0),
        angle: 0.4,
        linvel: Vec2::new(3.0, -1.0),
        angvel: 0.7,
    };
    let s = w.spawn_ship(d.clone(), body);
    let parent = w.ship(s).unwrap().pieces()[0].clone();
    let before = part_positions(&w, s);
    let com_old = parent.com_world();

    // The centre column: both beams, the fusion reactor, two armour blocks and the torch drive.
    let centre = [(5, 0), (6, 0), (5, 2), (5, 4), (6, 4), (5, 5)];
    w.step(&kill(s, &centre));

    let destroyed = w.events().iter().filter(|e| matches!(e, SimEvent::PartDestroyed { .. })).count();
    assert_eq!(destroyed, 6);
    let splits: Vec<_> = w
        .events()
        .iter()
        .filter_map(|e| match e {
            SimEvent::Split(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(splits.len(), 1);
    let split = splits[0];
    assert_eq!(split.parent, parent.id);
    assert_eq!(split.kept.id, parent.id);
    assert_eq!(split.spawned.len(), 1);
    let new = &split.spawned[0];
    assert!(new.id > parent.id);
    // Both halves keep a logic core: neither is salvage.
    assert!(!split.kept.salvage && !new.salvage);

    let ship = w.ship(s).unwrap();
    assert_eq!(ship.pieces().len(), 2);
    let (a, b) = (&ship.pieces()[0], &ship.pieces()[1]);
    assert_eq!((a.id, b.id), (split.kept.id, new.id));
    // Parts are partitioned; the kept piece is the left half (lowest part index on a tie).
    let mut all: Vec<PartIdx> = a.parts.iter().chain(&b.parts).copied().collect();
    all.sort();
    let alive: Vec<PartIdx> = (0..d.parts.len() as PartIdx).filter(|&i| ship.part(i).alive).collect();
    assert_eq!(all, alive);
    assert!(a.parts.contains(&find(&d, "logic_core", 4, 3)));
    assert!(b.parts.contains(&find(&d, "logic_core", 7, 3)));
    for &p in &b.parts {
        assert_eq!(ship.part(p).piece, b.id);
    }

    // Mass: conserved minus the destroyed parts; the halves mirror each other.
    let lost: f32 = centre
        .iter()
        .map(|&cell| d.parts[d.grid.occupant(cell).unwrap()].mass)
        .sum();
    assert!(close(a.mass.mass + b.mass.mass, parent.mass.mass - lost, 1e-3));
    assert!(close(a.mass.mass, b.mass.mass, 1e-4));
    assert!(close(a.mass.inertia, b.mass.inertia, 1e-2));
    let com_a_bp = [a.origin[0] + a.mass.com.x, a.origin[1] - a.mass.com.y];
    assert!(close(com_a_bp[0] + b.origin[0], 12.0, 1e-4), "{com_a_bp:?} vs {:?}", b.origin);
    assert!(close(com_a_bp[1], b.origin[1], 1e-4));
    // The new piece's frame sits on its centre of mass; colliders and snapshots match the live pieces.
    assert_eq!(b.mass.com, Vec2::ZERO);
    assert_eq!(new.colliders, b.colliders);
    assert_eq!(split.kept.colliders, a.colliders);

    // Nothing jumped: every surviving part is where it was.
    let after = part_positions(&w, s);
    for (i, (p0, p1)) in before.iter().zip(&after).enumerate() {
        if let (Some(p0), Some(p1)) = (p0, p1) {
            assert!((*p0 - *p1).length() < 1e-4, "part {i} moved from {p0:?} to {p1:?}");
        }
    }
    // Velocities: rigid-body motion of the old body at each new centre of mass; spin unchanged.
    for p in [a, b] {
        let want = inherited_velocity(body.linvel, body.angvel, com_old, p.com_world());
        assert!((p.body.linvel - want).length() < 1e-4, "{:?} vs {want:?}", p.body.linvel);
        assert_eq!(p.body.angvel, body.angvel);
        assert_eq!(p.body.angle, body.angle);
    }
    // Linear momentum of the remaining mass is that of the same mass moving with the old body.
    let m = a.mass.mass + b.mass.mass;
    let mom = a.body.linvel * a.mass.mass + b.body.linvel * b.mass.mass;
    let c_rem = (a.com_world() * a.mass.mass + b.com_world() * b.mass.mass) * (1.0 / m);
    let want = inherited_velocity(body.linvel, body.angvel, com_old, c_rem) * m;
    assert!((mom - want).length() < 1e-2, "{mom:?} vs {want:?}");
    // Each half has its own closed hull outline.
    for p in [a, b] {
        let loops = data::geometry::hull_loops(&ship.piece_hull_edges(p.id));
        assert!(!loops.is_empty() && loops.iter().all(|l| l.closed));
    }
    // The split pose in the event equals the live body.
    assert_eq!(new.body, b.body);
    assert!((new.body.pos - (body.pos + sim::Rot::from_angle(body.angle).apply(new.offset_in_parent))).length() < 1e-5);
}

#[test]
fn raider_breaks_into_salvage() {
    let c = content();
    let d = design(&c, "automaton_raider");
    let (mut w, s) = world_with(d.clone());
    let id0 = w.ship(s).unwrap().pieces()[0].id;
    // Losing only the core leaves one piece, but it is salvage now: the thruster stops.
    w.step(&kill(s, &[(1, 1)]));
    assert!(matches!(w.events().last(), Some(SimEvent::PieceChanged { piece, .. }) if piece.salvage));
    w.step(&inputs(s, controls(1, 1.0, true, true)));
    let p = &w.ship(s).unwrap().pieces()[0];
    assert!(p.salvage());
    assert_eq!(p.wrench, sim::Wrench::default());
    assert_eq!(p.power.demand, 0.0);
    // Cannon and thruster gone too: the left and right columns drift apart.
    w.step(&kill(s, &[(1, 0), (1, 2)]));
    let split = w
        .events()
        .iter()
        .find_map(|e| match e {
            SimEvent::Split(s) => Some(s.clone()),
            _ => None,
        })
        .expect("split");
    assert_eq!(split.kept.id, id0);
    assert_eq!(split.spawned.len(), 1);
    assert!(split.kept.salvage && split.spawned[0].salvage);
    let col = |x: i32| -> Vec<PartIdx> { (0..3).map(|y| d.grid.occupant((x, y)).unwrap() as PartIdx).collect() };
    let mut left = col(0);
    left.sort();
    let mut right = col(2);
    right.sort();
    assert_eq!(split.kept.parts, left);
    assert_eq!(split.spawned[0].parts, right);
    assert!(close(split.kept.mass.mass, split.spawned[0].mass.mass, 1e-5));
}

#[test]
fn last_part_destroys_the_piece() {
    let c = content();
    let d = design(&c, "automaton_raider");
    let (mut w, s) = world_with(d);
    let all: Vec<(i32, i32)> = (0..3).flat_map(|y| (0..3).map(move |x| (x, y))).collect();
    w.step(&kill(s, &all));
    assert!(w.ship(s).unwrap().pieces().is_empty());
    assert!(w.events().iter().any(|e| matches!(e, SimEvent::PieceDestroyed { .. })));
    // Further damage is ignored and nothing breaks.
    assert_eq!(w.apply_damage(s, DamageTarget::Cell((1, 1)), 1.0), DamageOutcome::Ignored);
    w.step(&TickInputs::default());
}

#[test]
fn damage_outcomes() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let (mut w, s) = world_with(d.clone());
    let armor = find(&d, "armor_block", 3, 1); // composite: 1.4 hp
    assert_eq!(w.apply_damage(s, DamageTarget::Part(armor), 0.0), DamageOutcome::Ignored);
    assert_eq!(w.apply_damage(s, DamageTarget::Cell((0, 0)), 1.0), DamageOutcome::Ignored, "empty cell");
    match w.apply_damage(s, DamageTarget::Part(armor), 1.0) {
        DamageOutcome::Damaged { hp_left } => assert!(close(hp_left, 0.4, 1e-5)),
        o => panic!("{o:?}"),
    }
    match w.apply_damage(s, DamageTarget::Cell((3, 1)), 1.0) {
        DamageOutcome::Destroyed { excess } => assert!(close(excess, 0.6, 1e-5)),
        o => panic!("{o:?}"),
    }
    assert_eq!(w.apply_damage(s, DamageTarget::Part(armor), 1.0), DamageOutcome::Ignored);
    assert!(w.ship(s).unwrap().grid().occupant((3, 1)).is_none());
}

#[test]
fn losing_a_part_shifts_the_centre_of_mass() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let mut w = sim::Sim::new(1);
    let body = Body {
        pos: Vec2::new(-4.0, 2.0),
        angle: -1.1,
        linvel: Vec2::new(0.5, 0.25),
        angvel: -0.6,
    };
    let s = w.spawn_ship(d.clone(), body);
    let com_old = w.ship(s).unwrap().pieces()[0].com_world();
    let before = part_positions(&w, s);
    // The left wing-tip pulse laser goes; the ship stays in one piece.
    w.step(&kill(s, &[(0, 3)]));
    let ev = w.events().iter().find_map(|e| match e {
        SimEvent::PieceChanged { piece, .. } => Some(piece.clone()),
        _ => None,
    });
    let snap = ev.expect("PieceChanged");
    let p = &w.ship(s).unwrap().pieces()[0];
    assert_eq!(snap.id, p.id);
    assert!(p.mass.com.x > 0.0, "lost mass on the left: CoM moves right");
    assert_eq!(p.body.pos, body.pos, "the frame does not move");
    let want = inherited_velocity(body.linvel, body.angvel, com_old, p.com_world());
    assert!((p.body.linvel - want).length() < 1e-5);
    assert_eq!(snap.body, p.body);
    // Thrusters' lever arms follow the new centre of mass.
    let t = p.thrusters[0];
    assert!(((t.point - p.mass.com) - t.arm).length() < 1e-6);
    let after = part_positions(&w, s);
    for (p0, p1) in before.iter().zip(&after) {
        if let (Some(p0), Some(p1)) = (p0, p1) {
            assert!((*p0 - *p1).length() < 1e-5);
        }
    }
}

#[test]
fn pieces_keep_splitting_with_fresh_ids() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let (mut w, s) = world_with(d.clone());
    w.step(&kill(s, &[(5, 0), (6, 0), (5, 2), (5, 4), (6, 4), (5, 5)]));
    assert_eq!(w.ship(s).unwrap().pieces().len(), 2);
    // Cut the right half again: the right wing (x ≥ 9) loses its link at x = 8.
    w.step(&kill(s, &[(8, 1), (8, 2), (8, 3), (8, 4), (8, 5)]));
    let ship = w.ship(s).unwrap();
    let ids: Vec<u32> = ship.pieces().iter().map(|p| p.id.0).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "pieces stay sorted by id: {ids:?}");
    assert!(ship.pieces().len() >= 3);
    // Exactly one piece per alive part, and each part knows its piece.
    for p in ship.pieces() {
        for &i in &p.parts {
            assert_eq!(ship.part(i).piece, p.id);
        }
    }
    let total: usize = ship.pieces().iter().map(|p| p.parts.len()).sum();
    assert_eq!(total, ship.parts().iter().filter(|p| p.alive).count());
}
