//! Flight computer, allocation, maneuvers, the content raider, physics backend and interpolation.

mod common;

use core::f32::consts::{FRAC_PI_2, PI};

use common::*;
use sim::alloc::{Actuation, AllocTable, allocate, assist, wrench_of};
use sim::flight::Kin;
use sim::physics::wrap_angle;
use sim::{
    Body, DamageTarget, FlightGoal, HeadingGoal, Maneuver, PilotInput, ShipCommand, SimEvent,
    Target, TickInputs, Vec2,
};

const SHIPS: [&str; 2] = ["halvex_runner", "automaton_raider"];

fn piece(w: &sim::Sim, s: sim::ShipId) -> &sim::Piece {
    let ship = w.ship(s).unwrap();
    ship.piece(ship.control_piece().unwrap()).unwrap()
}

fn idle(w: &mut sim::Sim, ticks: u32) {
    for _ in 0..ticks {
        w.step(&TickInputs::default());
    }
}

#[test]
fn turns_to_a_heading_without_overshoot() {
    let c = content();
    for id in SHIPS {
        let (mut w, s) = world_with(design(&c, id));
        let goal = FlightGoal {
            heading: HeadingGoal::Angle(FRAC_PI_2),
            velocity: Some(Vec2::ZERO),
        };
        w.command(s, ShipCommand::Goal { goal, fire: false });
        let (mut reached, mut overshoot) = (None, 0.0f32);
        for t in 0..600 {
            w.step(&TickInputs::default());
            let e = wrap_angle(FRAC_PI_2 - piece(&w, s).body.angle);
            if reached.is_none() && e.abs() < 1f32.to_radians() {
                reached = Some(t);
            }
            if reached.is_some() {
                overshoot = overshoot.max(-e);
            }
        }
        let t = reached.unwrap_or_else(|| panic!("{id} never reached the heading"));
        assert!(t < 300, "{id}: 90° took {} s", t as f32 / 30.0);
        assert!(
            overshoot < 2f32.to_radians(),
            "{id}: overshoot {}°",
            overshoot.to_degrees()
        );
        assert!(piece(&w, s).body.angvel.abs() < 1e-3);
    }
}

/// 90° turn to `FRAC_PI_2` holding position: (seconds to within 1°, overshoot in degrees).
fn quarter_turn(w: &mut sim::Sim, s: sim::ShipId) -> (f32, f32) {
    let goal = FlightGoal {
        heading: HeadingGoal::Angle(FRAC_PI_2),
        velocity: Some(Vec2::ZERO),
    };
    w.command(s, ShipCommand::Goal { goal, fire: false });
    let (mut reached, mut overshoot) = (None, 0.0f32);
    for t in 1..=450 {
        w.step(&TickInputs::default());
        let e = wrap_angle(FRAC_PI_2 - piece(w, s).body.angle);
        if reached.is_none() && e.abs() < 1f32.to_radians() {
            reached = Some(t as f32 / 30.0);
        }
        if reached.is_some() {
            overshoot = overshoot.max(-e);
        }
    }
    (reached.unwrap_or(f32::INFINITY), overshoot.to_degrees())
}

/// A Halvex Runner that lost a logic core, with one gyro set to the lowest power priority: it goes idle with a
/// pulse laser (control load 18 against 16). The flight computer plans with what can still push and turn
/// (GDR-0044: idle parts do nothing), so the 90° turn takes longer but does not overshoot, and a stop from speed
/// (flip and burn: engines only at the back) neither overshoots the flip nor flies backwards.
#[test]
fn a_runner_with_idle_actuators_turns_and_brakes_cleanly() {
    let c = content();
    let d = design(&c, "halvex_runner");
    let (mut w, s) = world_with(d.clone());
    let gyro = find(&d, "gyro", 9, 3);
    let low = sim::PowerSetting {
        tier: sim::PowerTier::Low,
        weight: 0.5,
    };
    w.command(s, ShipCommand::Power { part: gyro, setting: low });
    w.apply_damage(s, DamageTarget::Part(find(&d, "logic_core", 3, 4)), 10.0);
    idle(&mut w, 2);
    let ship = w.ship(s).unwrap();
    let idle_parts: Vec<&str> = (0..d.parts.len())
        .filter(|&i| ship.part(i as sim::PartIdx).idle)
        .map(|i| d.parts[i].id.as_str())
        .collect();
    assert_eq!(idle_parts, ["pulse_laser", "gyro"]);
    // The planning authority leaves the idle gyro out: one gyro and one logic core turn the ship.
    let torque = |id: &str| c.parts.get(id).unwrap().gyro.as_ref().unwrap().torque;
    let p = piece(&w, s);
    assert!(close(p.authority.torque_pure, torque("gyro") + torque("logic_core"), 1e-2), "{:?}", p.authority);

    let (turn, overshoot) = quarter_turn(&mut w, s);
    println!("runner, one core and one gyro idle: 90° in {turn:.2} s, overshoot {overshoot:.2}°");
    assert!(turn < 2.5 && overshoot < 2.0, "90° in {turn} s, overshoot {overshoot}°");

    // Up to speed along the nose (world −x after the turn), then release the stick: flip and burn.
    let nose = Vec2::new(-1.0, 0.0);
    w.command(s, ShipCommand::Pilot(PilotInput { stick: nose, ..Default::default() }));
    idle(&mut w, 75);
    let v0 = piece(&w, s).body.linvel;
    assert!(v0.dot(nose) > 10.0, "{v0:?}");
    w.command(s, ShipCommand::Pilot(PilotInput::default()));
    let (mut backwards, mut stopped, mut spin_after) = (0.0f32, None, 0.0f32);
    for t in 1..=300 {
        w.step(&TickInputs::default());
        let b = piece(&w, s).body;
        backwards = backwards.max(-b.linvel.dot(nose));
        if stopped.is_none() && b.linvel.length() < 0.05 {
            stopped = Some(t as f32 / 30.0);
        }
        if stopped.is_some() {
            spin_after = spin_after.max(b.angvel.abs());
        }
    }
    let stopped = stopped.unwrap_or(f32::INFINITY);
    println!(
        "  stop from {:.1} m/s: {stopped:.2} s, flew backwards at up to {backwards:.2} m/s, spin after {spin_after:.3} rad/s",
        v0.length()
    );
    assert!(stopped < 6.0, "stopped after {stopped} s");
    assert!(backwards < 0.2, "flew backwards at {backwards} m/s");
    assert!(spin_after < 0.05, "still spinning at {spin_after} rad/s");
}

#[test]
fn stick_sets_course_and_speed_and_release_brakes() {
    let c = content();
    for id in SHIPS {
        let (mut w, s) = world_with(design(&c, id));
        let east = Vec2::new(1.0, 0.0);
        w.command(
            s,
            ShipCommand::Pilot(PilotInput {
                stick: east,
                ..Default::default()
            }),
        );
        let mut vmax = 0.0f32;
        idle(&mut w, 1500);
        for _ in 0..300 {
            w.step(&TickInputs::default());
            vmax = vmax.max(piece(&w, s).body.linvel.length());
        }
        let top = w.ship(s).unwrap().telemetry().top_speed;
        let p = piece(&w, s);
        assert!(top > 1.0, "{id}: top speed {top}");
        assert!(
            (p.body.linvel - east * top).length() < 0.05 * top,
            "{id}: {:?} vs top {top}",
            p.body.linvel
        );
        assert!(
            vmax < 1.05 * top,
            "{id}: top speed exceeded: {vmax} vs {top}"
        );
        // The nose points along the stick (east = body angle −90°).
        assert!(
            wrap_angle(p.body.angle + FRAC_PI_2).abs() < 5f32.to_radians(),
            "{id}: nose {}",
            p.body.angle.to_degrees()
        );

        // Half stick: half speed.
        w.command(
            s,
            ShipCommand::Pilot(PilotInput {
                stick: east * 0.5,
                ..Default::default()
            }),
        );
        idle(&mut w, 1800);
        let v = piece(&w, s).body.linvel.length();
        assert!(
            (v - 0.5 * top).abs() < 0.1 * top,
            "{id}: half stick gave {v} of {top}"
        );

        // Drift: let go and coast.
        w.command(
            s,
            ShipCommand::Pilot(PilotInput {
                drift: true,
                ..Default::default()
            }),
        );
        let v0 = piece(&w, s).body.linvel;
        idle(&mut w, 300);
        assert!(
            (piece(&w, s).body.linvel - v0).length() < 1e-3,
            "{id}: drift must coast"
        );

        // Release without drift: brake to a full stop, not spinning.
        w.command(s, ShipCommand::Pilot(PilotInput::default()));
        idle(&mut w, 2400);
        let p = piece(&w, s);
        assert!(
            p.body.linvel.length() < 0.06,
            "{id}: still moving at {:?}",
            p.body.linvel
        );
        assert!(
            p.body.angvel.abs() < 0.01,
            "{id}: still spinning at {}",
            p.body.angvel
        );
    }
}

#[test]
fn lock_keeps_the_nose_on_a_moving_target() {
    let c = content();
    let mut w = sim::Sim::new(1);
    let hunter = w.spawn_ship(
        design(&c, "halvex_runner"),
        sim::Owner::PLAYER,
        Body::default(),
    );
    let prey = w.spawn_ship(
        design(&c, "automaton_raider"),
        sim::Owner(1),
        Body {
            pos: Vec2::new(40.0, 30.0),
            linvel: Vec2::new(-1.5, 1.0),
            ..Body::default()
        },
    );
    w.command(
        hunter,
        ShipCommand::Pilot(PilotInput {
            lock: Some(Target::Ship(prey)),
            ..Default::default()
        }),
    );
    idle(&mut w, 450);
    let mut worst = 0.0f32;
    for _ in 0..300 {
        w.step(&TickInputs::default());
        let to = piece(&w, prey).com_world() - piece(&w, hunter).com_world();
        let err = wrap_angle(Kin::nose_heading(to) - piece(&w, hunter).body.angle);
        worst = worst.max(err.abs());
    }
    assert!(
        worst < 2f32.to_radians(),
        "lock error {}°",
        worst.to_degrees()
    );
    // (Braking under lock needs thrust away from the target; the runner only has engines pushing forward, so
    // it cannot brake while locked. Ships with RCS can: see `rcs_nozzles_follow_open_sides_and_strafe`.)
}

/// A raider at `(-100, 33)` drifting past the origin at 20 m/s, 33 m off (no behavior, no thrust).
fn crossing_raider(w: &mut sim::Sim) -> sim::ShipId {
    let c = content();
    w.spawn_ship(
        design(&c, "automaton_raider"),
        sim::Owner(1),
        Body { pos: Vec2::new(-100.0, 33.0), linvel: Vec2::new(20.0, 0.0), ..Body::default() },
    )
}

/// The worst angle (rad) between the nose of `ship` and the direction to `target` while the target passes within
/// 45° of its closest point (|x| < 33 m), stepping until it is past.
fn worst_nose_error(w: &mut sim::Sim, ship: sim::ShipId, target: sim::ShipId) -> f32 {
    let mut worst = 0.0f32;
    for _ in 0..240 {
        w.step(&TickInputs::default());
        let to = piece(w, target).com_world() - piece(w, ship).com_world();
        if to.x.abs() < 33.0 {
            let err = wrap_angle(Kin::nose_heading(to) - piece(w, ship).body.angle);
            worst = worst.max(err.abs());
        }
    }
    worst
}

/// GDR-0064: the lock feeds the target's bearing rate forward, so the nose leads a crossing target instead of
/// trailing it. A raider crossing 33 m in front of a Halvex Runner at 20 m/s turns the bearing at up to 0.6 rad/s;
/// without the feed-forward the Runner's nose trailed it by about 6.7° (ω² / (2 · margin · α) with its first-line
/// α of 2.21 rad/s²), a clean miss at that distance.
#[test]
fn the_lock_leads_a_crossing_target() {
    let c = content();
    let mut w = sim::Sim::new(1);
    let hunter = w.spawn_ship(design(&c, "halvex_runner"), sim::Owner::PLAYER, Body::default());
    let prey = crossing_raider(&mut w);
    w.command(hunter, ShipCommand::Pilot(PilotInput { lock: Some(Target::Ship(prey)), ..Default::default() }));
    let worst = worst_nose_error(&mut w, hunter, prey);
    println!("lock on a crossing raider: worst nose error {:.2}°", worst.to_degrees());
    assert!(worst < 1.5f32.to_radians(), "lock error {}°", worst.to_degrees());
}

/// GDR-0064: `face` feeds the bearing rate forward too. A small turret (no engines, a gyro) facing the crossing
/// raider: before, the nose trailed it by `ω / turn_gain` (about 2.9° at 0.6 rad/s).
#[test]
fn face_leads_a_crossing_target() {
    let c = content();
    let turret = custom(
        &c,
        &[
            ("pulse_laser", 1, 0, 0, false, None),
            ("fission_reactor", 0, 1, 0, false, None),
            ("logic_core", 1, 1, 0, false, None),
            ("gyro", 2, 1, 0, false, None),
        ],
    );
    let mut w = sim::Sim::new(1);
    let me = w.spawn_ship(turret, sim::Owner::PLAYER, Body::default());
    let prey = crossing_raider(&mut w);
    let face = Maneuver::FaceTarget { target: Target::Ship(prey), aim: sim::flight::AimMode::Target };
    w.command(me, ShipCommand::Maneuver { maneuver: face, fire: false });
    let worst = worst_nose_error(&mut w, me, prey);
    println!("face on a crossing raider: worst nose error {:.2}°", worst.to_degrees());
    assert!(worst < 1.0f32.to_radians(), "face error {}°", worst.to_degrees());
}

/// GDR-0064: no maneuver asks for more than the ship's top speed. Goals beyond it (a target faster than the
/// ship, an orbit around a moving target) left AI ships stuck at their top speed: the push they asked for was a
/// hair "with" the motion, the whole-speed fade cut it off, and the heading logic never turned.
#[test]
fn maneuver_goals_never_ask_for_more_than_the_top_speed() {
    let c = content();
    let (mut w, s) = world_with(design(&c, "automaton_raider"));
    w.step(&TickInputs::default());
    let kin = Kin::of(piece(&w, s), w.ship(s).unwrap().parts());
    let tuning = sim::SimConfig::default().flight;
    assert!(kin.top_speed > 0.0);
    let fast = |pos: Vec2| sim::flight::TargetState { pos, vel: Vec2::new(30.0, -10.0) };
    let t = Target::Point(Vec2::ZERO);
    let aim = sim::flight::AimMode::Auto;
    for (m, at) in [
        (Maneuver::Approach { target: t, range: 20.0, aim }, Vec2::new(0.0, 300.0)),
        (Maneuver::Approach { target: t, range: 20.0, aim }, Vec2::new(0.0, 10.0)),
        (Maneuver::KeepRange { target: t, range: 50.0, aim }, Vec2::new(0.0, 10.0)),
        (Maneuver::KeepRange { target: t, range: 50.0, aim }, Vec2::new(0.0, 300.0)),
        (Maneuver::Orbit { target: t, radius: 40.0, ccw: true }, Vec2::new(0.0, 40.0)),
        (Maneuver::Orbit { target: t, radius: 40.0, ccw: false }, Vec2::new(0.0, 200.0)),
        (Maneuver::FaceTarget { target: t, aim }, Vec2::new(0.0, 30.0)),
        (Maneuver::Flee { from: t }, Vec2::new(0.0, 30.0)),
    ] {
        let goal = m.goal_for(&kin, Some(fast(at)), &tuning);
        let v = goal.velocity.unwrap().length();
        assert!(v <= kin.top_speed * (1.0 + 1e-5), "{m:?} at {at:?}: {v} > {}", kin.top_speed);
    }
}

#[test]
fn allocator_gives_zero_torque_for_symmetric_translation() {
    let c = content();
    let (w, s) = world_with(design(&c, "halvex_runner"));
    let p = piece(&w, s);
    let fmax: Vec<f32> = p.thrusters.iter().map(|t| t.max_force).collect();
    let tmax: Vec<f32> = p.torque_sources.iter().map(|t| t.max).collect();
    let mut act = Actuation::default();
    act.resize(fmax.len(), tmax.len());
    let table = AllocTable::build(&p.thrusters);
    let w0 = alloc_all(
        &p.thrusters,
        &table,
        &fmax,
        &tmax,
        Vec2::new(0.0, 30.0),
        0.0,
        true,
        &mut act,
    );
    assert!(
        (w0.force.y - 30.0).abs() < 1e-3 && w0.force.x.abs() < 1e-4,
        "{:?}",
        w0
    );
    assert!(w0.torque.abs() < 1e-3, "torque {}", w0.torque);
    // Mirrored engines get the same throttle.
    let (l, r): (Vec<usize>, Vec<usize>) = (0..p.thrusters.len())
        .filter(|&i| p.thrusters[i].torque_per_force != 0.0)
        .partition(|&i| p.thrusters[i].point.x < 0.0);
    for (&a, &b) in l.iter().zip(r.iter().rev()) {
        assert!(
            (act.throttle[a] - act.throttle[b]).abs() < 1e-5,
            "{:?}",
            act.throttle
        );
    }
}

fn row(x: f32, y: f32, dir: Vec2, force: f32, gimbal: f32) -> sim::Thruster {
    let point = Vec2::new(x, y);
    let lat = dir.perp();
    sim::Thruster {
        slot: 0,
        part: 0,
        dir,
        point,
        arm: point,
        torque_per_force: point.cross(dir),
        max_force: force,
        rated: 20.0,
        plume_length: 0.0,
        plume_damage: 0.0,
        plume_heat: 0.0,
        gimbal,
        lat,
        torque_per_lateral: point.cross(lat),
        rcs: false,
    }
}

/// The first line, then the last resort if `last_resort` (as the flight computer does once it is on), with no
/// rated-speed falloff (`full` = `fmax`); returns the wrench.
#[allow(clippy::too_many_arguments)]
fn alloc_all(
    th: &[sim::Thruster],
    table: &AllocTable,
    fmax: &[f32],
    tmax: &[f32],
    force: Vec2,
    torque: f32,
    last_resort: bool,
    act: &mut Actuation,
) -> sim::Wrench {
    let missing = allocate(th, table, fmax, fmax, tmax, force, torque, act);
    if last_resort {
        assist(th, table, fmax, fmax, missing, true, act);
    }
    wrench_of(th, fmax, fmax, tmax, act)
}

#[test]
fn allocator_prefers_pure_torque_then_gimbal_then_differential_thrust() {
    let fwd = Vec2::new(0.0, 1.0);
    // Two engines at the back, 2 m either side of the centre; the right one gimbals 5°.
    let th = vec![
        row(-2.0, -3.0, fwd, 10.0, 0.0),
        row(2.0, -3.0, fwd, 10.0, 0.0872),
    ];
    let table = AllocTable::build(&th);
    let fmax = [10.0, 10.0];
    let mut act = Actuation::default();

    // Pure torque covers a small request: no thrust at all.
    act.resize(2, 1);
    let w = alloc_all(&th, &table, &fmax, &[5.0], Vec2::ZERO, 3.0, true, &mut act);
    assert_eq!(act.throttle, vec![0.0, 0.0]);
    assert!((w.torque - 3.0).abs() < 1e-5 && (act.torque[0] - 0.6).abs() < 1e-6);

    // No pure torque, thrusting forward: the blend of the two fills makes the torque for free.
    act.resize(2, 0);
    let w = alloc_all(
        &th,
        &table,
        &fmax,
        &[],
        Vec2::new(0.0, 10.0),
        8.0,
        false,
        &mut act,
    );
    assert!(
        (w.torque - 8.0).abs() < 1e-4 && (w.force.y - 10.0).abs() < 1e-4,
        "{w:?}"
    );

    // At full thrust the blend has no room left: the first line reports the torque it is missing, and the
    // last resort's gimbal adds it (and a little side force).
    act.resize(2, 0);
    let missing = allocate(&th, &table, &fmax, &fmax, &[], Vec2::new(0.0, 20.0), 1.0, &mut act);
    assert!((missing - 1.0).abs() < 1e-4, "missing {missing}");
    assert_eq!(act.deflect, vec![0.0, 0.0]);
    assist(&th, &table, &fmax, &fmax, missing, false, &mut act);
    let w = wrench_of(&th, &fmax, &fmax, &[], &act);
    assert!(
        act.deflect[1] != 0.0 && (w.torque - 1.0).abs() < 1e-4,
        "{w:?} {act:?}"
    );

    // Nothing requested but torque, no pure torque: differential thrust turns the ship (adds force) ...
    act.resize(2, 0);
    let w = alloc_all(&th, &table, &fmax, &[], Vec2::ZERO, -15.0, true, &mut act);
    assert!((w.torque + 15.0).abs() < 1e-4 && w.force.y > 0.0, "{w:?}");
    // ... but only as the last resort, and only for a real turn (not to hold a heading).
    act.resize(2, 0);
    let w = alloc_all(&th, &table, &fmax, &[], Vec2::ZERO, -15.0, false, &mut act);
    assert_eq!(w.torque, 0.0);
    act.resize(2, 0);
    let missing = allocate(&th, &table, &fmax, &fmax, &[], Vec2::ZERO, -15.0, &mut act);
    assist(&th, &table, &fmax, &fmax, missing, false, &mut act);
    assert_eq!(wrench_of(&th, &fmax, &fmax, &[], &act).torque, 0.0);
}

#[test]
fn rcs_couples_turn_without_pushing_and_ignore_the_rated_speed() {
    let (fwd, back) = (Vec2::new(0.0, 1.0), Vec2::new(0.0, -1.0));
    let rcs = |x: f32, dir: Vec2| sim::Thruster {
        slot: sim::design::NO_SLOT,
        rcs: true,
        gimbal: 0.0,
        ..row(x, 0.0, dir, 10.0, 0.0)
    };
    // Two pods 3 m either side of the centre, each with a forward and a backward nozzle, and an engine.
    let th = vec![
        rcs(3.0, fwd),
        rcs(3.0, back),
        rcs(-3.0, fwd),
        rcs(-3.0, back),
        row(2.0, -3.0, fwd, 10.0, 0.0),
    ];
    let table = AllocTable::build(&th);
    let fmax = [10.0; 5];
    let mut act = Actuation::default();
    act.resize(5, 1);
    // The gyro gives 5, a couple of the right pod's forward and the left pod's backward nozzle the rest; the
    // forces cancel, the engine stays off.
    let w = alloc_all(&th, &table, &fmax, &[5.0], Vec2::ZERO, 50.0, false, &mut act);
    assert!((w.torque - 50.0).abs() < 1e-4 && w.force.length() < 1e-4, "{w:?}");
    assert!(act.couple[0] > 0.0 && act.couple[3] == act.couple[0] && act.throttle[4] == 0.0, "{act:?}");
    // Beyond the gyro and both couples (5 + 2 × 3 × 10 = 65) the first line reports what is missing.
    act.resize(5, 1);
    let missing = allocate(&th, &table, &fmax, &fmax, &[5.0], Vec2::ZERO, 80.0, &mut act);
    assert!((missing - 15.0).abs() < 1e-4, "missing {missing}");
    // At its rated speed the forward nozzle cannot push, but a couple adds no net force, so it still fires at
    // full force.
    let faded = [0.0, 10.0, 0.0, 10.0, 0.0];
    act.resize(5, 1);
    let missing = allocate(&th, &table, &faded, &fmax, &[5.0], Vec2::ZERO, 50.0, &mut act);
    let w = wrench_of(&th, &faded, &fmax, &[5.0], &act);
    assert!(missing == 0.0 && (w.torque - 50.0).abs() < 1e-4 && w.force.length() < 1e-4, "{w:?}");
}

/// A 3×3 box: core in the middle, reactor beside it, RCS pods at the four corners, an engine at the back.
fn rcs_box() -> Vec<P<'static>> {
    vec![
        ("rcs_pod", 0, 0, 0, false, None),
        ("armor_block", 1, 0, 0, false, Some("steel")),
        ("rcs_pod", 2, 0, 0, false, None),
        ("fission_reactor", 0, 1, 0, false, None),
        ("logic_core", 1, 1, 0, false, None),
        ("armor_block", 2, 1, 0, false, Some("steel")),
        ("rcs_pod", 0, 2, 0, false, None),
        ("thruster", 1, 2, 0, false, None),
        ("rcs_pod", 2, 2, 0, false, None),
    ]
}

#[test]
fn rcs_nozzles_follow_open_sides_and_strafe() {
    let c = content();
    let d = custom(&c, &rcs_box());
    let (mut w, s) = world_with(d.clone());
    let p = piece(&w, s);
    // Each corner pod has two open sides.
    assert_eq!(p.thrusters.iter().filter(|t| t.rcs).count(), 8);
    assert!(p.authority.left > 0.0 && p.authority.right > 0.0 && p.authority.backward > 0.0);
    // Locked on a point straight ahead, stick to the right: the ship slides right, nose stays.
    let ahead = Target::Point(Vec2::new(0.0, 1000.0));
    w.command(
        s,
        ShipCommand::Pilot(PilotInput {
            stick: Vec2::new(1.0, 0.0),
            lock: Some(ahead),
            ..Default::default()
        }),
    );
    idle(&mut w, 300);
    let p = piece(&w, s);
    assert!(p.body.linvel.x > 0.5, "{:?}", p.body.linvel);
    // The nose stays on the point (which drifts a few degrees to the left as the ship slides).
    let bearing = Kin::nose_heading(Vec2::new(0.0, 1000.0) - p.com_world());
    assert!(
        wrap_angle(p.body.angle - bearing).abs() < 2f32.to_radians(),
        "nose {} vs bearing {}",
        p.body.angle.to_degrees(),
        bearing.to_degrees()
    );
    let out: f32 = p
        .thrusters
        .iter()
        .zip(&p.output)
        .filter(|(t, _)| t.rcs)
        .map(|(_, o)| o)
        .sum();
    assert!(out > 0.0, "RCS nozzles fire");
    // Shoot away the armour beside the top-left pod: it gains a nozzle.
    let before = p.thrusters.iter().filter(|t| t.rcs).count();
    w.apply_damage(s, DamageTarget::Cell((1, 0)), 100.0);
    w.step(&TickInputs::default());
    assert_eq!(
        piece(&w, s).thrusters.iter().filter(|t| t.rcs).count(),
        before + 2
    );
}

#[test]
fn maneuvers_approach_orbit_and_hold() {
    let c = content();
    let mut w = sim::Sim::new(3);
    let ship = w.spawn_ship(
        design(&c, "automaton_raider"),
        sim::Owner(1),
        Body::default(),
    );
    let beacon = Target::Point(Vec2::new(0.0, 80.0));
    w.command(
        ship,
        ShipCommand::Maneuver {
            maneuver: Maneuver::Approach {
                target: beacon,
                range: 20.0,
                aim: sim::flight::AimMode::Auto,
            },
            fire: false,
        },
    );
    idle(&mut w, 1800);
    let p = piece(&w, ship);
    let d = (p.com_world() - Vec2::new(0.0, 80.0)).length();
    assert!(
        (d - 20.0).abs() < 2.0 && p.body.linvel.length() < 0.2,
        "approach: {d} m, {:?}",
        p.body.linvel
    );

    w.command(
        ship,
        ShipCommand::Maneuver {
            maneuver: Maneuver::Orbit {
                target: beacon,
                radius: 25.0,
                ccw: true,
            },
            fire: false,
        },
    );
    idle(&mut w, 1800);
    let mut worst = 0.0f32;
    let mut swept = 0.0f32;
    let mut last = piece(&w, ship).com_world() - Vec2::new(0.0, 80.0);
    for _ in 0..600 {
        w.step(&TickInputs::default());
        let r = piece(&w, ship).com_world() - Vec2::new(0.0, 80.0);
        worst = worst.max((r.length() - 25.0).abs());
        swept += last.cross(r).signum()
            * wrap_angle(sim::math::atan2(r.y, r.x) - sim::math::atan2(last.y, last.x)).abs();
        last = r;
    }
    assert!(worst < 5.0, "orbit radius error {worst}");
    assert!(swept > 0.5, "orbits counter-clockwise: swept {swept} rad");

    let home = Vec2::new(-30.0, 10.0);
    w.command(
        ship,
        ShipCommand::Maneuver {
            maneuver: Maneuver::Hold { at: home },
            fire: false,
        },
    );
    idle(&mut w, 2400);
    let p = piece(&w, ship);
    assert!(
        (p.com_world() - home).length() < 1.0 && p.body.linvel.length() < 0.2,
        "hold: {:?}",
        p.com_world()
    );
}

#[test]
fn content_raider_hunts_and_fires() {
    let c = content();
    let mut w = sim::Sim::new(5);
    let runner = w.spawn_ship(
        design(&c, "halvex_runner"),
        sim::Owner::PLAYER,
        Body::default(),
    );
    let raider = w.spawn_ship(
        design(&c, "automaton_raider"),
        sim::Owner(1),
        Body::at(Vec2::new(120.0, 40.0), PI),
    );
    fight(&mut w, &c, raider);
    let mut fired = 0;
    let mut closest = f32::MAX;
    for _ in 0..2400 {
        w.step(&TickInputs::default());
        let d = (piece(&w, raider).com_world() - piece(&w, runner).com_world()).length();
        closest = closest.min(d);
        if piece(&w, raider).guns.iter().any(|g| g.output > 0.0) {
            fired += 1;
        }
    }
    // The cannon reaches 75 m; `raider_basic` closes to 85% of that, then strafes around within 120%.
    assert!(closest < 1.2 * 75.0, "closest approach {closest}");
    assert!(fired > 30, "fired on {fired} ticks");
    assert_eq!(
        w.ship(raider).unwrap().behavior().unwrap().target(),
        Some(runner)
    );
}

#[test]
fn dark_ships_do_nothing() {
    let c = content();
    let d = design(&c, "automaton_raider");
    let (mut w, s) = world_with(d.clone());
    w.command(
        s,
        ShipCommand::Pilot(PilotInput {
            stick: Vec2::new(0.0, 1.0),
            fire: true,
            ..Default::default()
        }),
    );
    idle(&mut w, 10);
    assert!(piece(&w, s).active && piece(&w, s).wrench.force.length() > 0.0);
    // The automaton core is also its reactor: overheat it artificially by destroying it -> control lost.
    let core = (0..d.parts.len()).find(|&i| d.parts[i].logic).unwrap() as u32;
    w.apply_damage(s, DamageTarget::Part(core), 100.0);
    w.step(&TickInputs::default());
    assert!(
        w.events()
            .iter()
            .any(|e| matches!(e, SimEvent::ControlLost { .. }))
    );
    let ship = w.ship(s).unwrap();
    assert!(!ship.is_active() && ship.control_piece().is_none());
    assert!(
        ship.pieces()
            .iter()
            .all(|p| p.wrench == sim::Wrench::default())
    );
}

#[test]
fn unpowered_piece_goes_dark_and_wakes_up() {
    let c = content();
    // Core + one reactor + engine; kill the reactor: no power, dark; the core is still there. An armour block
    // keeps the reactor's blast off the core and the engine.
    let d = custom(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("armor_block", 1, 0, 0, false, Some("steel")),
            ("fission_reactor", 2, 0, 0, false, None),
            ("thruster", 0, 1, 0, false, None),
        ],
    );
    let (mut w, s) = world_with(d.clone());
    w.command(
        s,
        ShipCommand::Pilot(PilotInput {
            stick: Vec2::new(0.0, 1.0),
            ..Default::default()
        }),
    );
    idle(&mut w, 5);
    assert!(w.ship(s).unwrap().is_active());
    w.apply_damage(s, DamageTarget::Cell((2, 0)), 100.0);
    idle(&mut w, 5);
    let ship = w.ship(s).unwrap();
    assert!(
        !ship.is_active() && ship.control_piece().is_some(),
        "dark, but still controllable when powered"
    );
    assert_eq!(ship.pieces()[0].wrench, sim::Wrench::default());
}

#[test]
fn split_pieces_ghost_for_a_few_ticks() {
    let c = content();
    let d = design(&c, "automaton_raider");
    let (mut w, s) = world_with(d);
    for cell in [(1, 0), (1, 1), (1, 2)] {
        w.apply_damage(s, DamageTarget::Cell(cell), 100.0);
    }
    w.step(&TickInputs::default());
    let keys: Vec<sim::BodyKey> = w
        .ship(s)
        .unwrap()
        .pieces()
        .iter()
        .map(|p| sim::BodyKey {
            ship: s,
            piece: p.id,
        })
        .collect();
    assert_eq!(keys.len(), 2);
    assert!(keys.iter().all(|&k| w.physics().is_ghost(k) == Some(true)));
    let ghost = w.config().split_ghost_ticks;
    idle(&mut w, ghost + 1);
    assert!(keys.iter().all(|&k| w.physics().is_ghost(k) == Some(false)));
}

#[test]
fn render_pose_interpolates_between_ticks() {
    let c = content();
    let mut w = sim::Sim::new(1);
    let s = w.spawn_ship(
        design(&c, "automaton_raider"),
        sim::Owner(1),
        Body {
            angle: PI - 0.01,
            linvel: Vec2::new(3.0, 0.0),
            angvel: 0.9,
            ..Body::default()
        },
    );
    w.step(&TickInputs::default());
    let p = piece(&w, s);
    assert_eq!(p.render_pose(0.0), (p.prev_pos, p.prev_angle));
    let (pos1, a1) = p.render_pose(1.0);
    assert!((pos1 - p.body.pos).length() < 1e-5 && wrap_angle(a1 - p.body.angle).abs() < 1e-5);
    // Crossing ±π takes the short way round.
    let (_, half) = p.render_pose(0.5);
    assert!(
        wrap_angle(half - p.prev_angle).abs() < 0.1,
        "{half} from {}",
        p.prev_angle
    );
}

#[test]
fn raycast_finds_the_part_under_the_hit() {
    let c = content();
    let d = design(&c, "automaton_raider");
    let mut w = sim::Sim::new(1);
    let s = w.spawn_ship(
        d.clone(),
        sim::Owner::PLAYER,
        Body::at(Vec2::new(20.0, 0.0), 0.0),
    );
    let hit = w
        .raycast(Vec2::ZERO, Vec2::new(1.0, 0.0), 100.0, None)
        .expect("hit");
    assert_eq!(hit.ship, s);
    // The raider is 3 m wide and centred near x = 20: the ray enters its left column.
    assert!(
        hit.hit.distance > 17.5 && hit.hit.distance < 19.0,
        "{}",
        hit.hit.distance
    );
    let part = hit.part;
    assert_eq!(d.parts[part as usize].shape.x, 0);
    assert!(
        w.raycast(Vec2::ZERO, Vec2::new(1.0, 0.0), 100.0, Some(s))
            .is_none()
    );
}

#[test]
fn lead_point_intercepts_a_moving_target() {
    use sim::flight::{intercept_time, lead_point};
    let (shooter, shooter_vel) = (Vec2::new(3.0, -2.0), Vec2::new(1.0, 0.5));
    let (target, target_vel) = (Vec2::new(50.0, 20.0), Vec2::new(-4.0, 9.0));
    let speed = 90.0;
    let aim = lead_point(shooter, shooter_vel, target, target_vel, speed);
    let r = aim - shooter;
    let dir = r * (1.0 / r.length());
    let t = intercept_time(target - shooter, target_vel - shooter_vel, speed).unwrap();
    // A shot along `dir` (plus the shooter's velocity, as the sim fires them) meets the target at time t.
    let shot = shooter + (shooter_vel + dir * speed) * t;
    let there = target + target_vel * t;
    assert!((shot - there).length() < 1e-3, "{shot:?} vs {there:?}");
    // A target faster than the shot, running away, cannot be caught: aim at it directly.
    assert_eq!(
        lead_point(
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::new(10.0, 0.0),
            Vec2::new(200.0, 0.0),
            90.0
        ),
        Vec2::new(10.0, 0.0)
    );
}

/// A raider on the spot (lock, drift: no flying) shooting at an enemy raider (another owner: no friendly fire)
/// crossing at 8 m/s, 50 m away.
fn crossing_hits(aim: sim::flight::AimMode) -> usize {
    let c = content();
    let mut w = sim::Sim::new(9);
    let shooter = w.spawn_ship(
        design(&c, "automaton_raider"),
        sim::Owner(1),
        Body::default(),
    );
    let target = w.spawn_ship(
        design(&c, "automaton_raider"),
        sim::Owner(2),
        Body {
            pos: Vec2::new(-20.0, 50.0),
            linvel: Vec2::new(8.0, 0.0),
            ..Body::default()
        },
    );
    let input = PilotInput {
        lock: Some(Target::Ship(target)),
        fire: true,
        drift: true,
        aim,
        ..Default::default()
    };
    w.command(shooter, ShipCommand::Pilot(input));
    let mut hits = 0;
    for _ in 0..150 {
        w.step(&TickInputs::default());
        hits += w
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::Hit { ship, by, .. } if *ship == target && *by == shooter))
            .count();
    }
    hits
}

#[test]
fn leading_hits_a_crossing_target() {
    let lead = crossing_hits(sim::flight::AimMode::Lead);
    let auto = crossing_hits(sim::flight::AimMode::Auto);
    let direct = crossing_hits(sim::flight::AimMode::Target);
    assert!(lead >= 3, "lead: {lead} hits");
    assert_eq!(auto, lead, "a cannon ship leads by default");
    assert!(
        direct < lead,
        "aiming at the target: {direct} hits, leading: {lead}"
    );
}

/// A lock turn while thrusting (finding of the 13 Oct review): the Runner with two RCS pods, locked on a point
/// 90° to the left with W held. Full forward thrust takes the pods' nozzles that make its couples, so the turn
/// must be planned with what translation leaves free (gyros and cores, then the last resort), or the nose
/// overshoots the lock.
#[test]
fn a_lock_turn_while_thrusting_plans_with_what_the_thrust_leaves_free() {
    let c = content();
    let d = std::sync::Arc::new(sim::ShipDesign::compile(&c, &runner_with_pods(&c, 2)).unwrap());
    let (mut w, s) = world_with(d);
    let input = PilotInput {
        lock: Some(Target::Point(Vec2::new(-5000.0, 0.0))),
        axes: Some(sim::flight::PilotAxes { turn: 0.0, thrust: 1.0, strafe: 0.0 }),
        ..Default::default()
    };
    let (mut reached, mut overshoot) = (None, 0.0f32);
    for t in 1..=240 {
        w.step(&TickInputs { commands: vec![(s, ShipCommand::Pilot(input))], ..Default::default() });
        let tel = *w.ship(s).unwrap().telemetry();
        let e = wrap_angle(tel.heading_target.unwrap() - piece(&w, s).body.angle);
        if reached.is_none() && e.abs() < 1f32.to_radians() {
            reached = Some(t as f32 / 30.0);
        }
        if reached.is_some() {
            overshoot = overshoot.max(-e);
        }
    }
    let reached = reached.expect("never reached the lock heading");
    println!("runner + 2 pods, lock 90° left with W: {reached:.2} s, overshoot {:.2}°", overshoot.to_degrees());
    assert!(overshoot < 2f32.to_radians(), "overshoot {}°", overshoot.to_degrees());
}

/// NaN and infinity from outside (a malformed packet, a tool) never reach the state (13 Oct 2026 review: one NaN
/// raw throttle made the body NaN for good and panicked the debug state hash). Commands with such numbers are
/// ignored, raw throttles, bodies and projectiles count them as 0, damage ignores them, a config takes the
/// defaults for them. Every step below hashes the state (debug builds assert there is no NaN in it).
#[test]
fn non_finite_inputs_never_reach_the_state() {
    let c = content();
    let (nan, inf) = (f32::NAN, f32::INFINITY);
    let d = sim::SimConfig::default();
    let cfg = sim::SimConfig {
        thrust_scale: nan,
        flight: sim::FlightTuning { turn_margin: nan, ..d.flight },
        physics: sim::PhysicsTuning { friction: inf, ..d.physics },
        ..d
    };
    let mut w = sim::Sim::with_config(1, cfg);
    assert_eq!(*w.config(), sim::SimConfig::default());
    let d = design(&c, "halvex_runner");
    let raw = w.spawn_ship(d.clone(), sim::Owner::PLAYER, Body { pos: Vec2::new(nan, 0.0), angle: inf, ..Default::default() });
    let flown = w.spawn_ship(d.clone(), sim::Owner::PLAYER, Body::at(Vec2::new(40.0, 0.0), 0.0));
    w.command(flown, ShipCommand::Maneuver { maneuver: Maneuver::Hold { at: Vec2::new(40.0, 10.0) }, fire: false });
    let bad = [
        ShipCommand::Pilot(PilotInput { stick: Vec2::new(nan, 1.0), ..Default::default() }),
        ShipCommand::Pilot(PilotInput { lock: Some(Target::Point(Vec2::new(0.0, inf))), ..Default::default() }),
        ShipCommand::Goal { goal: FlightGoal { heading: HeadingGoal::Angle(nan), velocity: None }, fire: false },
        ShipCommand::Goal { goal: FlightGoal { heading: HeadingGoal::Keep, velocity: Some(Vec2::new(inf, 0.0)) }, fire: false },
        ShipCommand::Maneuver { maneuver: Maneuver::Orbit { target: Target::Ship(raw), radius: nan, ccw: true }, fire: false },
    ];
    for t in 0..60 {
        w.step(&TickInputs {
            controls: vec![(raw, controls(d.engines.len(), nan, false, true))],
            commands: vec![(flown, bad[t % bad.len()])],
            damage: vec![sim::DamageCommand { ship: raw, target: DamageTarget::Part(0), amount: inf }],
        });
    }
    assert!(matches!(w.ship(flown).unwrap().helm(), sim::HelmMode::Maneuver { maneuver: Maneuver::Hold { .. }, .. }));
    assert!(w.ship(raw).unwrap().part(0).alive, "infinite damage is ignored");
    let key = piece(&w, raw).id;
    w.teleport(raw, key, Body { pos: Vec2::new(inf, 3.0), angle: 0.5, linvel: Vec2::new(nan, 0.0), angvel: nan });
    w.spawn_projectile(raw, data::WeaponFamily::Kinetic, Vec2::new(nan, 0.0), Vec2::new(0.0, inf), nan, 1, inf);
    w.set_config(cfg);
    idle(&mut w, 30);
    for s in [raw, flown] {
        let b = piece(&w, s).body;
        assert!(b.pos.is_finite() && b.angle.is_finite() && b.linvel.is_finite() && b.angvel.is_finite(), "{b:?}");
    }
    assert_eq!(*w.config(), sim::SimConfig::default());
}
