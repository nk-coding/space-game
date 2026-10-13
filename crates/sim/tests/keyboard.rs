//! PC keyboard flight (GDR-0040): rotate / thrust / strafe axes, smart lock aim, the A/D aim nudge.

mod common;

use common::*;
use sim::flight::{KEY_TURN_START, PilotAxes, nudge_step, turn_ramp_step};
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
fn the_turn_ramp_starts_small_and_grows_to_the_full_rate() {
    let mut level = turn_ramp_step(0.0, 1.0);
    assert_eq!(level, KEY_TURN_START, "a fresh press starts small");
    let mut ticks = 1;
    while level < 1.0 {
        level = turn_ramp_step(level, 1.0);
        ticks += 1;
    }
    assert!((22..=26).contains(&ticks), "full rate after {ticks} ticks");
    assert_eq!(turn_ramp_step(level, 1.0), 1.0, "never above the key");
    assert_eq!(turn_ramp_step(level, -1.0), -KEY_TURN_START, "the other key starts over");
    assert_eq!(turn_ramp_step(level, 0.0), 0.0, "released");
    assert_eq!(turn_ramp_step(0.5, 0.3), 0.3, "a half-pressed (analog) key caps it");
}

/// Holds A (`turn` = +1) or D (-1) for `ticks` ticks the way the client does (the ramp once per tick), then
/// releases it for `after` ticks. Returns the heading change and the fastest turn rate while held, and checks
/// that the last resort (differential thrust) never helps a keyboard turn of the Runner.
fn key_turn(w: &mut sim::Sim, s: sim::ShipId, turn: f32, ticks: usize, after: usize) -> (f32, f32) {
    let start = body(w, s).angle;
    let (mut level, mut fastest) = (0.0, 0.0f32);
    for t in 0..ticks + after {
        level = turn_ramp_step(level, if t < ticks { turn } else { 0.0 });
        steer(w, s, axes(level, 0.0, 0.0), 1);
        fastest = fastest.max(body(w, s).angvel.abs());
        assert!(!w.ship(s).unwrap().telemetry().assist, "tick {t}: differential thrust");
    }
    (wrap(body(w, s).angle - start), fastest)
}

#[test]
fn rotate_input_turns_the_ship() {
    let c = content();
    let cap = sim::SimConfig::default().flight.key_turn_rate;
    // A tap (0.1 s) nudges the nose a little.
    let (mut w, s) = world_with(design(&c, "halvex_runner"));
    let (tap, _) = key_turn(&mut w, s, 1.0, 3, 30);
    println!("runner, A tapped for 0.1 s: {:.2}°", tap.to_degrees());
    assert!(tap > 0.5f32.to_radians() && tap < 5f32.to_radians(), "a tap: {}°", tap.to_degrees());
    // Held for 1.5 s: it ramps up to the key turn rate and no faster.
    let (mut w, s) = world_with(design(&c, "halvex_runner"));
    let mut log = String::new();
    let (mut level, start) = (0.0, body(&w, s).angle);
    for t in 1..=45 {
        level = turn_ramp_step(level, 1.0);
        steer(&mut w, s, axes(level, 0.0, 0.0), 1);
        let b = body(&w, s);
        if t % 5 == 0 {
            log += &format!(" {:.2}s {:.1}° {:.2} rad/s |", t as f32 / 30.0, wrap(b.angle - start).to_degrees(), b.angvel);
        }
        assert!(b.angvel <= cap + 1e-3, "{t}: {} rad/s", b.angvel);
        assert!(!w.ship(s).unwrap().telemetry().assist, "{t}: differential thrust");
    }
    println!("runner, A held:{log}");
    assert!(body(&w, s).angvel > 0.98 * cap, "held: {}", body(&w, s).angvel);
    // Released: stops turning (and holds roughly where it is).
    steer(&mut w, s, axes(0.0, 0.0, 0.0), 150);
    let held = body(&w, s);
    assert!(held.angvel.abs() < 0.02, "stops turning: {}", held.angvel);
    // D turns clockwise; a 1 s hold turns about 45°.
    let (mut w, s) = world_with(design(&c, "halvex_runner"));
    let (right, fastest) = key_turn(&mut w, s, -1.0, 30, 0);
    println!("runner, D held for 1 s: {:.1}°, up to {fastest:.2} rad/s", right.to_degrees());
    assert!(right < -0.6 && right > -1.5, "D turns clockwise: {right}");
}

#[test]
fn w_thrusts_along_the_nose_and_cancels_drift() {
    let c = content();
    let mut w = sim::Sim::new(3);
    // Nose rotated 0.6 rad, drifting sideways at 3 m/s.
    let angle = 0.6f32;
    let nose = Vec2::new(-angle.sin(), angle.cos());
    let side = Vec2::new(angle.cos(), angle.sin());
    let s = w.spawn_ship(rcs_ship(&c), sim::Owner::PLAYER, Body { pos: Vec2::ZERO, angle, linvel: side * 3.0, angvel: 0.0 });
    steer(&mut w, s, axes(0.0, 1.0, 0.0), 600);
    let b = body(&w, s);
    let n = Vec2::new(-b.angle.sin(), b.angle.cos());
    let sd = Vec2::new(b.angle.cos(), b.angle.sin());
    assert!(b.linvel.dot(n) > 1.0, "moves along the nose: {:?}", b.linvel);
    assert!(b.linvel.dot(sd).abs() < 0.5, "sideways drift cancelled: {}", b.linvel.dot(sd));
    assert!((b.angle - angle).abs() < 0.05, "W does not turn the ship");
    // Drift on: W is pure thrust, the sideways component is kept.
    let mut w = sim::Sim::new(3);
    let s = w.spawn_ship(rcs_ship(&c), sim::Owner::PLAYER, Body { pos: Vec2::ZERO, angle, linvel: side * 3.0, angvel: 0.0 });
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
    let me = w.spawn_ship(d, sim::Owner::PLAYER, Body::default());
    let target = w.spawn_ship(design(&c, "automaton_raider"), sim::Owner(1), Body::at(Vec2::new(30.0, 20.0), 0.0));
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
    let me = w.spawn_ship(design(&c, "halvex_runner"), sim::Owner::PLAYER, Body::default());
    let target = w.spawn_ship(design(&c, "automaton_raider"), sim::Owner(1), Body::at(Vec2::new(0.0, 60.0), 0.0));
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
    // The nose follows the nudge (GDR-0055 turning is slower; the nudge must stay usable). While the nudge grows
    // (1.2 rad/s) the nose trails it by up to about 0.3 rad, then catches up.
    let mut lag = 0.0f32;
    for _ in 0..240 {
        tick(&mut w, 1.0, &mut nudge);
        lag = lag.max((nudge - wrap(body(&w, me).angle - aimed)).abs());
    }
    let nudged = body(&w, me).angle;
    println!("runner, lock nudge held 8 s: {:.3} rad (nudge {nudge:.3}), largest lag {lag:.3} rad", wrap(nudged - aimed));
    assert!(wrap(nudged - aimed) > 0.3, "A nudges the aim left: {}", wrap(nudged - aimed));
    assert!(lag < 0.35, "the nose lags the nudge by up to {lag} rad");
    assert!((nudge - wrap(nudged - aimed)).abs() < 0.05, "and catches up: {nudged} vs {nudge}");
    for _ in 0..450 {
        tick(&mut w, 0.0, &mut nudge);
    }
    // Back on the target. (A turn's differential thrust, when the last resort helps, can push the runner a few
    // metres, which it cannot brake under a lock with engines only at the back, so compare with the bearing now.)
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
