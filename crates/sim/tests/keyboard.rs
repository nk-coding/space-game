//! PC keyboard flight (GDR-0040): rotate / thrust / strafe axes, smart lock aim, the A/D aim nudge.

mod common;

use common::*;
use sim::flight::{PilotAxes, nudge_step};
use sim::{Body, DamageTarget, PilotInput, ShipCommand, Target, TickInputs, Vec2};

fn steer(w: &mut sim::Sim, s: sim::ShipId, input: PilotInput, ticks: usize) {
    for _ in 0..ticks {
        w.step(&TickInputs { commands: vec![(s, ShipCommand::Pilot(input))], ..Default::default() });
    }
}

fn body(w: &sim::Sim, s: sim::ShipId) -> sim::Body {
    let ship = w.ship(s).unwrap();
    ship.piece(ship.control_piece().unwrap()).unwrap().body
}

fn axes(turn: f32, thrust: f32, strafe: f32) -> PilotInput {
    PilotInput { axes: Some(PilotAxes { turn, thrust, strafe }), ..Default::default() }
}

fn wrap(a: f32) -> f32 {
    let mut a = a % std::f32::consts::TAU;
    if a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    if a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// A small test ship: core in the middle, RCS pods on three sides, a thruster at the back, two reactors.
fn rcs_ship(c: &data::Content) -> std::sync::Arc<sim::ShipDesign> {
    custom(
        c,
        &[
            ("armor_block", 0, 0, 0, false, Some("steel")),
            ("rcs_pod", 1, 0, 0, false, None),
            ("armor_block", 2, 0, 0, false, Some("steel")),
            ("rcs_pod", 0, 1, 0, false, None),
            ("logic_core", 1, 1, 0, false, None),
            ("rcs_pod", 2, 1, 0, false, None),
            ("fission_reactor", 0, 2, 0, false, None),
            ("thruster", 1, 2, 0, false, None),
            ("fission_reactor", 2, 2, 0, false, None),
        ],
    )
}

#[test]
fn rotate_input_turns_the_ship() {
    let c = content();
    let (mut w, s) = world_with(design(&c, "halvex_runner"));
    steer(&mut w, s, axes(1.0, 0.0, 0.0), 12);
    let left = body(&w, s).angle;
    assert!(left > 0.3 && left < 2.5, "A turns counter-clockwise: {left}");
    assert!(body(&w, s).angvel > 1.0, "and keeps turning");
    // Released: stops turning (and holds roughly where it is).
    steer(&mut w, s, axes(0.0, 0.0, 0.0), 150);
    let held = body(&w, s);
    assert!(held.angvel.abs() < 0.02, "stops turning: {}", held.angvel);
    let (mut w, s) = world_with(design(&c, "halvex_runner"));
    steer(&mut w, s, axes(-1.0, 0.0, 0.0), 12);
    let right = body(&w, s).angle;
    assert!(right < -0.3 && right > -2.5, "D turns clockwise: {right}");
}

#[test]
fn w_thrusts_along_the_nose_and_cancels_drift() {
    let c = content();
    let mut w = sim::Sim::new(3);
    // Nose rotated 0.6 rad, drifting sideways at 3 m/s.
    let angle = 0.6f32;
    let nose = Vec2::new(-angle.sin(), angle.cos());
    let side = Vec2::new(angle.cos(), angle.sin());
    let s = w.spawn_ship(rcs_ship(&c), Body { pos: Vec2::ZERO, angle, linvel: side * 3.0, angvel: 0.0 });
    steer(&mut w, s, axes(0.0, 1.0, 0.0), 600);
    let b = body(&w, s);
    let n = Vec2::new(-b.angle.sin(), b.angle.cos());
    let sd = Vec2::new(b.angle.cos(), b.angle.sin());
    assert!(b.linvel.dot(n) > 1.0, "moves along the nose: {:?}", b.linvel);
    assert!(b.linvel.dot(sd).abs() < 0.5, "sideways drift cancelled: {}", b.linvel.dot(sd));
    assert!((b.angle - angle).abs() < 0.05, "W does not turn the ship");
    // Drift on: W is pure thrust, the sideways component is kept.
    let mut w = sim::Sim::new(3);
    let s = w.spawn_ship(rcs_ship(&c), Body { pos: Vec2::ZERO, angle, linvel: side * 3.0, angvel: 0.0 });
    steer(&mut w, s, PilotInput { drift: true, ..axes(0.0, 1.0, 0.0) }, 300);
    let b = body(&w, s);
    assert!(b.linvel.dot(side) > 2.5, "drift keeps momentum: {:?}", b.linvel);
    assert!(b.linvel.dot(nose) > 0.3, "and thrusts along the nose");
}

#[test]
fn smart_aim_picks_a_side_gun_when_the_front_guns_are_gone() {
    let c = content();
    let d = custom(
        &c,
        &[
            ("pulse_laser", 1, 0, 0, false, None),
            ("fission_reactor", 0, 1, 0, false, None),
            ("logic_core", 1, 1, 0, false, None),
            ("pulse_laser", 2, 1, 1, false, None),
            ("fission_reactor", 1, 2, 0, false, None),
        ],
    );
    let front = find(&d, "pulse_laser", 1, 0);
    let mut w = sim::Sim::new(5);
    let me = w.spawn_ship(d, Body::default());
    let target = w.spawn_ship(design(&c, "automaton_raider"), Body::at(Vec2::new(30.0, 20.0), 0.0));
    let lock = PilotInput { lock: Some(Target::Ship(target)), ..axes(0.0, 0.0, 0.0) };
    steer(&mut w, me, lock, 300);
    let to = {
        let t = w.ship(target).unwrap();
        let tp = t.piece(t.control_piece().unwrap()).unwrap().com_world();
        let ship = w.ship(me).unwrap();
        tp - ship.piece(ship.control_piece().unwrap()).unwrap().com_world()
    };
    let bearing = to.y.atan2(to.x);
    // Both guns are equal; the front group wins the tie: the nose (+y) points at the target.
    let nose = body(&w, me).angle + std::f32::consts::FRAC_PI_2;
    assert!(wrap(nose - bearing).abs() < 0.1, "front gun bears first: {}", wrap(nose - bearing));
    // Destroy the front gun: now the right-facing gun (+x) must bear.
    w.apply_damage(me, DamageTarget::Part(front), 1e6);
    steer(&mut w, me, lock, 400);
    let right = body(&w, me).angle;
    let ship = w.ship(me).unwrap();
    let t = w.ship(target).unwrap();
    let to = t.piece(t.control_piece().unwrap()).unwrap().com_world() - ship.piece(ship.control_piece().unwrap()).unwrap().com_world();
    let bearing = to.y.atan2(to.x);
    assert!(wrap(right - bearing).abs() < 0.1, "side gun bears: {}", wrap(right - bearing));
}

#[test]
fn the_nudge_springs_back() {
    // The helper: held → grows to the cap; released → returns to zero.
    let mut n = 0.0;
    for _ in 0..30 {
        n = nudge_step(n, 1.0);
    }
    assert!(n > 0.5 && n <= sim::flight::NUDGE_MAX, "held: {n}");
    for _ in 0..60 {
        n = nudge_step(n, 0.0);
    }
    assert!(n.abs() < 0.01, "released: {n}");

    // In flight: a locked ship follows the nudge and comes back to the target when it springs back.
    let c = content();
    let mut w = sim::Sim::new(9);
    let me = w.spawn_ship(design(&c, "halvex_runner"), Body::default());
    let target = w.spawn_ship(design(&c, "automaton_raider"), Body::at(Vec2::new(0.0, 60.0), 0.0));
    let mut nudge = 0.0;
    let tick = |w: &mut sim::Sim, turn: f32, n: &mut f32| {
        *n = nudge_step(*n, turn);
        let input = PilotInput { lock: Some(Target::Ship(target)), aim_offset: *n, ..axes(turn, 0.0, 0.0) };
        w.step(&TickInputs { commands: vec![(me, ShipCommand::Pilot(input))], ..Default::default() });
    };
    for _ in 0..300 {
        tick(&mut w, 0.0, &mut nudge);
    }
    let aimed = body(&w, me).angle;
    for _ in 0..240 {
        tick(&mut w, 1.0, &mut nudge);
    }
    let nudged = body(&w, me).angle;
    assert!(wrap(nudged - aimed) > 0.3, "A nudges the aim left: {}", wrap(nudged - aimed));
    for _ in 0..450 {
        tick(&mut w, 0.0, &mut nudge);
    }
    // Back on the target. (The turns' differential thrust nudged the runner a few metres, which it cannot brake
    // under a lock with engines only at the back, so compare with the bearing now.)
    let back = body(&w, me).angle;
    let to = {
        let t = w.ship(target).unwrap();
        let ship = w.ship(me).unwrap();
        t.piece(t.control_piece().unwrap()).unwrap().com_world()
            - ship.piece(ship.control_piece().unwrap()).unwrap().com_world()
    };
    let bearing = to.y.atan2(to.x) - std::f32::consts::FRAC_PI_2;
    assert!(wrap(back - bearing).abs() < 0.05, "springs back to the target: {}", wrap(back - bearing));
}
