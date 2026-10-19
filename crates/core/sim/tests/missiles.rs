//! Missiles and point defence (GDR-0115): homing, the warhead's blast, shields absorbing missiles whole, point
//! defence shooting them down, determinism.

mod common;

use common::*;
use sim::{Body, GoneReason, Owner, ShipId, SimEvent, TickInputs, Vec2};

/// A rocket pod on a reactor and a core, facing +y.
fn launcher() -> Vec<P<'static>> {
    vec![
        ("rocket_pod", 0, 0, 0, false, None),
        ("fission_reactor", 0, 1, 0, false, None),
        ("logic_core", 0, 2, 0, false, None),
    ]
}

/// A 3×3 target: a core and a reactor in steel, with `extra` in the middle of its near (bottom) row.
fn target(extra: Option<(&'static str, u8)>) -> Vec<P<'static>> {
    let mut v = Vec::new();
    for y in 0..3 {
        for x in 0..3 {
            let part = match (x, y, extra) {
                (1, 0, _) => ("logic_core", 0),
                (1, 1, _) => ("fission_reactor", 0),
                (1, 2, Some(e)) => e,
                _ => ("armor_block", 0),
            };
            let material = (part.0 == "armor_block").then_some("steel");
            v.push((part.0, x, y, part.1, false, material));
        }
    }
    v
}

/// The launcher at the origin and an enemy vessel (owner 1) ahead and to the side, at `at`.
fn duel(extra: Option<(&'static str, u8)>, at: Vec2) -> (sim::Sim, ShipId, ShipId) {
    let c = content();
    let mut w = sim::Sim::new(7);
    let s = w.spawn_ship(custom(&c, &launcher()), Owner::PLAYER, Body::default());
    let t = w.spawn_ship(custom(&c, &target(extra)), Owner(1), Body::at(at, 0.0));
    (w, s, t)
}

fn fire(s: ShipId, t: ShipId) -> TickInputs {
    TickInputs {
        controls: vec![(s, controls(0, 0.0, true, true)), (t, controls(0, 0.0, false, true))],
        ..Default::default()
    }
}

#[derive(Default, Debug)]
struct Tally {
    launched: usize,
    warheads: Vec<Vec2>,
    hits: usize,
    absorbed: usize,
    shot_down: usize,
    /// Hits on the target before its first bubble collapsed.
    hits_shielded: usize,
    collapsed: bool,
}

fn run(w: &mut sim::Sim, s: ShipId, t: ShipId, ticks: usize) -> Tally {
    let mut n = Tally::default();
    for _ in 0..ticks {
        w.step(&fire(s, t));
        for e in w.events() {
            match e {
                SimEvent::ProjectileSpawned { owner, .. } if *owner == s => n.launched += 1,
                SimEvent::Warhead { by, at, .. } if *by == s => n.warheads.push(*at),
                SimEvent::Hit { by, ship, .. } if *by == s && *ship == t => {
                    n.hits += 1;
                    n.hits_shielded += !n.collapsed as usize;
                }
                SimEvent::ShieldHit { ship, depleted: true, .. } if *ship == t => n.collapsed = true,
                SimEvent::ProjectileGone { reason: GoneReason::Absorbed, .. } => n.absorbed += 1,
                SimEvent::ProjectileGone { reason: GoneReason::ShotDown, .. } => n.shot_down += 1,
                _ => {}
            }
        }
    }
    n
}

#[test]
fn rockets_turn_onto_an_enemy_off_their_line_and_blast_it() {
    // 14 m to the side, 40 m ahead: a rocket flying straight along the pod's facing would pass it by.
    let (mut w, s, t) = duel(None, Vec2::new(14.0, 40.0));
    let n = run(&mut w, s, t, 150);
    assert!(n.launched >= 3, "{n:?}");
    assert!(!n.warheads.is_empty(), "{n:?}");
    assert!(n.warheads.iter().all(|at| (at.x - 14.0).abs() < 3.0), "struck the target: {n:?}");
    // One warhead hits every part in its reach: more hits than warheads.
    assert!(n.hits > n.warheads.len(), "{n:?}");
}

#[test]
fn a_shield_absorbs_rockets_whole() {
    // The target's bubble (radius 2.3 m around its emitter in the near row) covers its hull.
    let (mut w, s, t) = duel(Some(("shield_emitter", 0)), Vec2::new(0.0, 35.0));
    let n = run(&mut w, s, t, 150);
    assert!(n.absorbed > 0, "{n:?}");
    assert_eq!(n.hits_shielded, 0, "nothing reached the hull while the bubble held: {n:?}");
}

#[test]
fn point_defence_shoots_rockets_down() {
    // The turret in the target's near row faces the launcher (turned half round: its forward side is down).
    let (mut w, s, t) = duel(Some(("point_defence", 2)), Vec2::new(0.0, 45.0));
    let with = run(&mut w, s, t, 150);
    let (mut w, s, t) = duel(None, Vec2::new(0.0, 45.0));
    let without = run(&mut w, s, t, 150);
    assert!(with.shot_down > 0, "{with:?}");
    assert!(with.warheads.len() < without.warheads.len(), "{with:?} vs {without:?}");
}

#[test]
fn missile_fights_are_deterministic() {
    let hashes = || {
        let (mut w, s, t) = duel(Some(("point_defence", 2)), Vec2::new(6.0, 40.0));
        (0..120)
            .map(|_| {
                w.step(&fire(s, t));
                w.state_hash()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(hashes(), hashes());
}
