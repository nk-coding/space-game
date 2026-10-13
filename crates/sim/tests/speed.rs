//! Rated speed and top speed (GDR-0041): the falloff, no drag, braking and turning at full thrust, the cruise
//! flame; and the agility target of the base ships (GDR-0042).

mod common;

use std::sync::Arc;

use common::*;
use sim::flight::Kin;
use sim::{Body, PilotInput, ShipCommand, ShipDesign, SimConfig, TickInputs, Vec2, rated_falloff};

fn control(w: &sim::Sim, s: sim::ShipId) -> &sim::Piece {
    let ship = w.ship(s).unwrap();
    ship.piece(ship.control_piece().unwrap()).unwrap()
}

fn idle(w: &mut sim::Sim, ticks: u32) {
    for _ in 0..ticks {
        w.step(&TickInputs::default());
    }
}

/// Thrust and rated speed of the content's thruster.
fn thruster(c: &data::Content) -> (f32, f32) {
    let e = c.parts.get("thruster").unwrap().engine.as_ref().unwrap();
    (e.thrust, e.rated_speed)
}

/// A small test ship in one column (so the thrust line runs through the centre of mass): reactor, logic core,
/// one thruster at the back.
fn pusher(c: &data::Content) -> Arc<ShipDesign> {
    custom(
        c,
        &[
            ("fission_reactor", 0, 0, 0, false, None),
            ("logic_core", 0, 1, 0, false, None),
            ("thruster", 0, 2, 0, false, None),
        ],
    )
}

#[test]
fn falloff_is_flat_then_linear_and_full_while_braking() {
    assert_eq!(rated_falloff(0.0, 20.0), 1.0);
    assert_eq!(rated_falloff(-50.0, 20.0), 1.0, "braking: full thrust");
    assert_eq!(rated_falloff(16.0, 20.0), 1.0, "flat to 80%");
    assert!(close(rated_falloff(18.0, 20.0), 0.5, 1e-6));
    assert_eq!(rated_falloff(20.0, 20.0), 0.0);
    assert_eq!(rated_falloff(35.0, 20.0), 0.0);
}

/// A thruster row pushing along +y with rated speed `rated` (for the falloff alone).
fn row(rated: f32) -> sim::Thruster {
    sim::Thruster {
        slot: 0,
        part: 0,
        dir: Vec2::new(0.0, 1.0),
        point: Vec2::ZERO,
        arm: Vec2::ZERO,
        torque_per_force: 0.0,
        max_force: 100.0,
        rated,
        plume_length: 0.0,
        plume_damage: 0.0,
        plume_heat: 0.0,
        gimbal: 0.0,
        lat: Vec2::new(-1.0, 0.0),
        torque_per_lateral: 0.0,
        rcs: false,
    }
}

/// GDR-0064: the whole-speed fade is continuous in the angle between the push and the motion. At the rating a
/// row pushing with the motion has nothing left (GDR-0055), one pushing a hair less than square to it keeps all
/// of it, and nothing jumps in between: no push across the motion is cut off by a sign test.
#[test]
fn the_fade_is_continuous_in_the_push_angle() {
    let t = row(20.0);
    // `deg`: angle between the push (+y) and the motion.
    let at = |deg: f32, speed: f32| {
        let a = deg.to_radians();
        let v = Vec2::new(sim::math::sin(a), sim::math::cos(a)) * speed;
        t.falloff(v, v.length())
    };
    assert_eq!(at(0.0, 20.0), 0.0, "straight with the motion at the rating");
    assert!(close(at(0.0, 18.0), 0.5, 1e-5));
    assert_eq!(at(45.0, 20.0), 0.0, "mostly with the motion: the whole speed counts");
    assert!(at(89.0, 20.0) > 0.999, "a hair less than square: {}", at(89.0, 20.0));
    assert!(at(89.0, 21.0) > 0.999, "also a little past the rating");
    assert_eq!(at(91.0, 40.0), 1.0, "against the motion");
    let mut last = at(0.0, 20.0);
    for k in 1..=1800 {
        let f = at(k as f32 * 0.1, 20.0);
        assert!((f - last).abs() < 0.05, "a jump at {}°: {last} → {f}", k as f32 * 0.1);
        assert!(f >= last - 1e-4, "never less thrust as the push turns away from the motion");
        last = f;
    }
}

/// GDR-0064: a ship at its top speed whose velocity is square to its locked nose still answers the stick: the
/// forward push across the motion is not cut off (with the sign test of GDR-0055 it got nothing, and the ship
/// flew on sideways). It turns the velocity towards the nose and stays within a few per cent of the top speed.
#[test]
fn a_locked_ship_at_top_speed_still_answers_the_stick_across_its_motion() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let mut w = sim::Sim::new(1);
    let s = w.spawn_ship(d, sim::Owner::PLAYER, Body { linvel: Vec2::new(22.0, 0.3), ..Default::default() });
    let top = Kin::of(control(&w, s), w.ship(s).unwrap().parts()).top_speed;
    w.command(
        s,
        ShipCommand::Pilot(PilotInput {
            stick: Vec2::new(0.0, 1.0),
            lock: Some(sim::Target::Point(Vec2::new(0.0, 5000.0))),
            ..Default::default()
        }),
    );
    let mut fastest = 0.0f32;
    for _ in 0..90 {
        w.step(&TickInputs::default());
        fastest = fastest.max(control(&w, s).body.linvel.length());
    }
    let v = control(&w, s).body.linvel;
    println!("locked at top speed across the motion: {v:?}, max |v| {fastest:.2} (top {top})");
    assert!(v.y > 4.0, "pushes along the nose: {v:?}");
    assert!(fastest < 1.04 * top, "{fastest} vs {top}");
}

#[test]
fn raw_thrust_tops_out_at_the_rated_speed_and_coasting_keeps_it() {
    let c = content();
    let (_, rated) = thruster(&c);
    let (mut w, s) = world_with(pusher(&c));
    let mut vmax = 0.0f32;
    for _ in 0..900 {
        w.step(&inputs(s, controls(1, 1.0, false, false)));
        vmax = vmax.max(control(&w, s).body.linvel.y);
    }
    let p = control(&w, s);
    let v = p.body.linvel.y;
    assert!(v > 0.995 * rated && vmax <= rated, "{v} (max {vmax}) vs rated {rated}");
    // At the rating the engine has no thrust left, and it draws no power and makes no heat for it.
    assert!(p.output[0] < 0.01, "output {}", p.output[0]);
    assert!(p.falloff[0] < 0.01, "falloff {}", p.falloff[0]);
    let engine = p.thrusters[0].part;
    assert!(w.ship(s).unwrap().part(engine).work < 0.01);
    // Throttle off: it coasts at that speed (no drag).
    for _ in 0..300 {
        w.step(&inputs(s, controls(1, 0.0, false, false)));
    }
    assert!((control(&w, s).body.linvel.y - v).abs() < 1e-4);
}

#[test]
fn braking_and_sideways_keep_full_thrust() {
    let c = content();
    let (thrust, rated) = thruster(&c);
    let d = pusher(&c);
    // (velocity, thrust left): moving backwards fast (braking), sideways (turning), forwards past the rating,
    // and diagonally forwards and backwards (GDR-0055: a thruster pushing with the motion fades by the whole
    // speed, here 0.94 of the rating, although the speed along it is only 0.6 of it; braking keeps it all).
    let diag = (0.72f32 * 0.72 + 0.6 * 0.6).sqrt();
    for (vel, want) in [
        (Vec2::new(0.0, -3.0 * rated), thrust),
        (Vec2::new(3.0 * rated, 0.0), thrust),
        (Vec2::new(0.0, 0.9 * rated), 0.5 * thrust),
        (Vec2::new(0.0, 1.5 * rated), 0.0),
        (Vec2::new(0.72 * rated, 0.6 * rated), rated_falloff(diag * rated, rated) * thrust),
        (Vec2::new(0.72 * rated, -0.6 * rated), thrust),
    ] {
        let mut w = sim::Sim::new(1);
        let s = w.spawn_ship(
            d.clone(),
            sim::Owner::PLAYER,
            Body {
                linvel: vel,
                ..Default::default()
            },
        );
        w.step(&inputs(s, controls(1, 1.0, false, false)));
        let f = control(&w, s).wrench.force.y;
        assert!(close(f, want, 1e-2 * thrust), "{vel:?}: {f} vs {want}");
    }
}

#[test]
fn cruise_flame_scales_with_the_stick_and_holds_at_top_speed() {
    let c = content();
    let (mut w, s) = world_with(design(&c, "halvex_runner"));
    let nose = Vec2::new(0.0, 1.0);
    let fly = |w: &mut sim::Sim, stick: Vec2, drift: bool, ticks: u32| {
        w.command(
            s,
            ShipCommand::Pilot(PilotInput {
                stick,
                drift,
                ..Default::default()
            }),
        );
        idle(w, ticks);
    };
    let engines = |w: &sim::Sim| -> Vec<(f32, f32)> {
        let p = control(w, s);
        (0..p.thrusters.len())
            .filter(|&i| !p.thrusters[i].rcs)
            .map(|i| (p.output[i], p.cruise[i]))
            .collect()
    };
    // Half stick from rest: half the top speed, then a cruise flame at half (no thrust: it has arrived).
    fly(&mut w, nose * 0.5, false, 240);
    let top = w.ship(s).unwrap().telemetry().top_speed;
    assert!(close(control(&w, s).body.linvel.y, 0.5 * top, 0.01 * top));
    for (out, cruise) in engines(&w) {
        assert!(out < 0.01 && close(cruise, 0.5, 0.01), "{out} {cruise}");
    }
    // Full stick: up to the top speed; the engines keep a full cruise flame and nothing jitters.
    fly(&mut w, nose, false, 300);
    for _ in 0..60 {
        w.step(&TickInputs::default());
        let p = control(&w, s);
        let v = p.body.linvel;
        assert!(v.y > 0.98 * top && v.y <= top + 1e-3 && v.x.abs() < 1e-2, "{v:?} vs {top}");
        assert!(p.wrench.force.length() < 0.02 * p.authority.forward, "{:?}", p.wrench);
        for (out, cruise) in engines(&w) {
            assert!(out < 0.05 && cruise > 0.999, "{out} {cruise}");
        }
    }
    // Drift with the stick released: coasting, engines off and dark.
    fly(&mut w, Vec2::ZERO, true, 30);
    for (out, cruise) in engines(&w) {
        assert!(out == 0.0 && cruise == 0.0, "{out} {cruise}");
    }
    assert!(control(&w, s).body.linvel.y > 0.98 * top, "no drag");
}

/// A 3×3 box: core in the middle, reactor beside it, RCS pods at the four corners, a thruster at the back.
fn rcs_box(c: &data::Content) -> Arc<ShipDesign> {
    custom(
        c,
        &[
            ("rcs_pod", 0, 0, 0, false, None),
            ("armor_block", 1, 0, 0, false, Some("steel")),
            ("rcs_pod", 2, 0, 0, false, None),
            ("fission_reactor", 0, 1, 0, false, None),
            ("logic_core", 1, 1, 0, false, None),
            ("armor_block", 2, 1, 0, false, Some("steel")),
            ("rcs_pod", 0, 2, 0, false, None),
            ("thruster", 1, 2, 0, false, None),
            ("rcs_pod", 2, 2, 0, false, None),
        ],
    )
}

#[test]
fn top_speed_per_direction_comes_from_the_thrusters_pushing_that_way() {
    let c = content();
    let (_, engine) = thruster(&c);
    let rcs = c.parts.get("rcs_pod").unwrap().rcs.as_ref().unwrap().rated_speed;
    let (mut w, s) = world_with(rcs_box(&c));
    let kin = Kin::of(control(&w, s), w.ship(s).unwrap().parts());
    // Forward: the thruster (and the rear pods' nozzles); the other ways only RCS.
    assert_eq!(kin.speeds, [engine.max(rcs), rcs, rcs, rcs]);
    assert_eq!(kin.top_along(Vec2::new(1.0, 0.0)), rcs);
    // Diagonally the slowest group pushing that way caps the speed (GDR-0055: thrusters pushing with the motion
    // fade by the whole speed), here the pods pushing sideways ...
    assert!(close(kin.top_along(Vec2::new(0.6, 0.8)), rcs, 1e-4));
    // ... but a small sideways share, done early, leaves the forward top speed.
    let nearly_ahead = Vec2::new(0.05, (1.0f32 - 0.05 * 0.05).sqrt());
    assert!(close(kin.top_along(nearly_ahead), engine, 1e-4));
    // Locked on a point ahead, stick right: it slides right up to the pods' rating, never beyond, and (the
    // whole speed counts) no faster in the world either, though the nose turns to follow the point.
    w.command(
        s,
        ShipCommand::Pilot(PilotInput {
            stick: Vec2::new(1.0, 0.0),
            lock: Some(sim::Target::Point(Vec2::new(0.0, 1000.0))),
            ..Default::default()
        }),
    );
    let (mut vmax, mut world_max) = (0.0f32, 0.0f32);
    for _ in 0..600 {
        w.step(&TickInputs::default());
        let p = control(&w, s);
        vmax = vmax.max(sim::Rot::from_angle(p.body.angle).apply_inverse(p.body.linvel).x);
        world_max = world_max.max(p.body.linvel.length());
    }
    let p = control(&w, s);
    let v = sim::Rot::from_angle(p.body.angle).apply_inverse(p.body.linvel);
    assert!(v.length() > 0.97 * rcs && vmax <= 1.001 * rcs, "{v:?} max {vmax} vs {rcs}");
    assert!(world_max <= 1.001 * rcs, "world speed {world_max} vs {rcs}");
    // A global speed scale scales every rating.
    w.set_config(SimConfig {
        speed_scale: 1.5,
        ..Default::default()
    });
    let kin = Kin::of(control(&w, s), w.ship(s).unwrap().parts());
    assert!(close(kin.speeds[2], 1.5 * rcs, 1e-4) && close(kin.top_speed, 1.5 * engine.max(rcs), 1e-4));
}

/// GDR-0055: a thruster pushing with the motion fades by the whole speed, so a ship flying diagonally is no
/// faster than the slowest thrusters pushing that way (with the per-axis fade of GDR-0041 this box flew the
/// diagonal at 23 m/s on 14 m/s pods and a 20 m/s thruster), and no ship is faster than its fastest thrusters.
#[test]
fn a_diagonal_is_no_faster_than_the_slowest_thrusters_pushing_that_way() {
    let c = content();
    let (_, engine) = thruster(&c);
    let rcs = c.parts.get("rcs_pod").unwrap().rcs.as_ref().unwrap().rated_speed;
    // Locked on a point far ahead, stick forward-right: it flies the diagonal at the pods' rating.
    let (mut w, s) = world_with(rcs_box(&c));
    let stick = Vec2::new(0.6, 0.8);
    w.command(
        s,
        ShipCommand::Pilot(PilotInput {
            stick,
            lock: Some(sim::Target::Point(Vec2::new(0.0, 5000.0))),
            ..Default::default()
        }),
    );
    let mut fastest = 0.0f32;
    for _ in 0..900 {
        w.step(&TickInputs::default());
        fastest = fastest.max(control(&w, s).body.linvel.length());
    }
    let v = control(&w, s).body.linvel;
    println!("rcs box, locked, stick (0.6, 0.8): {v:?}, |v| = {:.2} (max {fastest:.2}) m/s", v.length());
    assert!(fastest <= 1.001 * rcs && v.length() > 0.95 * rcs, "{v:?} max {fastest} vs {rcs}");
    assert!(v.dot(stick) > 0.99 * v.length(), "along the stick: {v:?}");
    // Drifting with W and E held (pure thrust forward and right): the pods stop pushing sideways once the
    // whole speed passes their rating, and the thruster at its own.
    let (mut w, s) = world_with(rcs_box(&c));
    w.command(
        s,
        ShipCommand::Pilot(PilotInput {
            axes: Some(sim::flight::PilotAxes { turn: 0.0, thrust: 1.0, strafe: 1.0 }),
            drift: true,
            ..Default::default()
        }),
    );
    let mut fastest = 0.0f32;
    for _ in 0..900 {
        w.step(&TickInputs::default());
        fastest = fastest.max(control(&w, s).body.linvel.length());
    }
    let p = control(&w, s);
    let v = sim::Rot::from_angle(p.body.angle).apply_inverse(p.body.linvel);
    println!("rcs box, drift, W + E: {v:?} (max {fastest:.2} m/s)");
    assert!(fastest <= 1.001 * engine && v.length() > 0.98 * engine, "{v:?} max {fastest}");
    assert!(v.x < rcs, "sideways {} stays below the pods' rating", v.x);
}

/// Agility of one design: (mass, 90° turn s, max α measured, acceleration of the best axis, top speed,
/// 0 → 95% of top speed s).
fn agility(id: &str, d: Arc<ShipDesign>) -> (f32, f32, f32, f32, f32, f32) {
    // 90° turn from rest to within 1°, holding position.
    let (turn, alpha, overshoot) = turn_90(d.clone());
    let turn = turn.unwrap_or_else(|| panic!("{id} never turned"));
    assert!(overshoot < 2f32.to_radians(), "{id}: overshoot {}°", overshoot.to_degrees());
    // 0 → 95% of the top speed with the stick along the nose.
    let (mut w, s) = world_with(d);
    let kin = Kin::of(control(&w, s), w.ship(s).unwrap().parts());
    w.command(
        s,
        ShipCommand::Pilot(PilotInput {
            stick: Vec2::new(0.0, 1.0),
            ..Default::default()
        }),
    );
    let mut reach = None;
    for t in 1..=600 {
        w.step(&TickInputs::default());
        if reach.is_none() && control(&w, s).body.linvel.length() >= 0.95 * kin.top_speed {
            reach = Some(t as f32 / 30.0);
        }
    }
    let reach = reach.unwrap_or_else(|| panic!("{id} never reached its top speed"));
    (kin.mass, turn, alpha, kin.accel, kin.top_speed, reach)
}

/// The agility targets: the Halvex Runner turns 90° in about 1.5 s (GDR-0055) and gets from 0 to about 22 m/s
/// in about 2.5 s (GDR-0042); the small raider stays clearly snappier. Prints the table in `docs/sim.md`
/// (`cargo test -p sim --test speed -- --nocapture`).
#[test]
fn base_ships_meet_the_agility_target() {
    let c = content();
    let mut rows = Vec::new();
    println!("| Ship | Mass | 90° turn | Max α | Accel | Top speed | 0 → 95% top |");
    for id in ["automaton_raider", "halvex_runner", "sample_early", "sample_mid"] {
        let (mass, turn, alpha, accel, top, reach) = agility(id, design(&c, id));
        println!(
            "| {id} | {mass:.1} t | {turn:.2} s | {alpha:.1} rad/s² | {accel:.1} m/s² | {top:.1} m/s | {reach:.2} s |"
        );
        rows.push((id, turn, top, reach));
    }
    let runner = rows.iter().find(|r| r.0 == "halvex_runner").unwrap();
    let raider = rows.iter().find(|r| r.0 == "automaton_raider").unwrap();
    assert!((1.3..=1.7).contains(&runner.1), "runner 90° in {} s", runner.1);
    assert!((20.0..=24.0).contains(&runner.2), "runner top speed {}", runner.2);
    assert!((2.0..=3.0).contains(&runner.3), "runner 0 → top in {} s", runner.3);
    assert!(raider.1 < 0.5 * runner.1, "the raider turns much faster: {} vs {}", raider.1, runner.1);
    assert!(raider.3 < runner.3, "the raider gets to speed faster: {} vs {}", raider.3, runner.3);
}

/// GDR-0055: RCS pods are the strong turners. The Halvex Runner with 2 and 4 pods on outriggers at its wing tips
/// (`common::runner_with_pods`, a test design) turns visibly faster than with its gyros alone. Prints the rows
/// for `docs/sim.md`.
#[test]
fn rcs_pods_make_the_runner_turn_faster() {
    let c = content();
    let mut turns = Vec::new();
    for pods in [0, 2, 4] {
        let bp = runner_with_pods(&c, pods);
        let d = Arc::new(ShipDesign::compile(&c, &bp).unwrap());
        let (mass, turn, alpha, accel, top, reach) = agility("runner with pods", d.clone());
        let (w, s) = world_with(d);
        let inertia = control(&w, s).mass.inertia;
        println!(
            "| Halvex Runner + {pods} RCS pods | {mass:.1} t | {inertia:.0} t·m² | {turn:.2} s | {alpha:.1} rad/s² | {accel:.1} m/s² | {top:.1} m/s | {reach:.2} s |"
        );
        turns.push(turn);
    }
    assert!(turns[1] < 0.85 * turns[0], "2 pods: {} vs {} s", turns[1], turns[0]);
    assert!(turns[2] < 0.9 * turns[1], "4 pods: {} vs {} s", turns[2], turns[1]);
}
