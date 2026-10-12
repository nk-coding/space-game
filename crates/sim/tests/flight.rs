//! Flight computer, allocation, maneuvers, the content raider, physics backend and interpolation.

mod common;

use core::f32::consts::{FRAC_PI_2, PI};

use common::*;
use sim::alloc::{Actuation, AllocTable, allocate};
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
    let hunter = w.spawn_ship(design(&c, "halvex_runner"), Body::default());
    let prey = w.spawn_ship(
        design(&c, "automaton_raider"),
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
    let w0 = allocate(
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
        clear: true,
        gimbal,
        lat,
        torque_per_lateral: point.cross(lat),
        rcs: false,
    }
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
    let w = allocate(&th, &table, &fmax, &[5.0], Vec2::ZERO, 3.0, true, &mut act);
    assert_eq!(act.throttle, vec![0.0, 0.0]);
    assert!((w.torque - 3.0).abs() < 1e-5 && (act.torque[0] - 0.6).abs() < 1e-6);

    // No pure torque, thrusting forward: the blend of the two fills makes the torque for free.
    act.resize(2, 0);
    let w = allocate(
        &th,
        &table,
        &fmax,
        &[],
        Vec2::new(0.0, 10.0),
        8.0,
        true,
        &mut act,
    );
    assert!(
        (w.torque - 8.0).abs() < 1e-4 && (w.force.y - 10.0).abs() < 1e-4,
        "{w:?}"
    );

    // At full thrust the blend has no room left: the gimbal adds torque (and a little side force).
    act.resize(2, 0);
    let w = allocate(
        &th,
        &table,
        &fmax,
        &[],
        Vec2::new(0.0, 20.0),
        1.0,
        true,
        &mut act,
    );
    assert!(
        act.deflect[1] != 0.0 && (w.torque - 1.0).abs() < 1e-4,
        "{w:?} {act:?}"
    );

    // Nothing requested but torque, no pure torque: differential thrust turns the ship (adds force) ...
    act.resize(2, 0);
    let w = allocate(&th, &table, &fmax, &[], Vec2::ZERO, -15.0, true, &mut act);
    assert!((w.torque + 15.0).abs() < 1e-4 && w.force.y > 0.0, "{w:?}");
    // ... unless differential thrust is not allowed (holding a heading).
    act.resize(2, 0);
    let w = allocate(&th, &table, &fmax, &[], Vec2::ZERO, -15.0, false, &mut act);
    assert_eq!(w.torque, 0.0);
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
    let ship = w.spawn_ship(design(&c, "automaton_raider"), Body::default());
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
    let runner = w.spawn_ship(design(&c, "halvex_runner"), Body::default());
    let raider = w.spawn_ship(
        design(&c, "automaton_raider"),
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
    // Core + one reactor + engine; kill the reactor: no power, dark; the core is still there.
    let d = custom(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("fission_reactor", 1, 0, 0, false, None),
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
    w.apply_damage(s, DamageTarget::Cell((1, 0)), 100.0);
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
    let s = w.spawn_ship(d.clone(), Body::at(Vec2::new(20.0, 0.0), 0.0));
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
    let part = hit.part.expect("part");
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

/// A raider on the spot (lock, drift: no flying) shooting at a raider crossing at 8 m/s, 50 m away.
fn crossing_hits(aim: sim::flight::AimMode) -> usize {
    let c = content();
    let mut w = sim::Sim::new(9);
    let shooter = w.spawn_ship(design(&c, "automaton_raider"), Body::default());
    let target = w.spawn_ship(
        design(&c, "automaton_raider"),
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
