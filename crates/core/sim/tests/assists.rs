//! Pilot assists (GDR-0088 step 3): face target, brake and auto-fire on their channels, the pilot winning each
//! channel while using it, the pilot's conditions, the toggle slots and ALL ASSISTS, and that only the piloted ship
//! runs them.

mod common;

use common::*;
use data::AssistList;
use sim::flight::{PilotAxes, turn_ramp_step};
use sim::{AssistSwitch, Body, PilotInput, ShipCommand, ShipId, Sim, SimEvent, Target, TickInputs, Vec2};

fn lib() -> sim::BehaviorLibrary {
    sim::BehaviorLibrary::new(&content()).expect("base behaviors compile")
}

/// Gives `ship` an assist list (rules in the text form, target line `nearest`).
fn assists(w: &mut Sim, ship: ShipId, rules: &[&str]) {
    let list = AssistList { target: None, rules: rules.iter().map(|r| r.to_string()).collect() };
    let id = w.behaviors_mut().add_assists(&list).unwrap_or_else(|e| panic!("{rules:?}: {e:?}"));
    w.command(ship, ShipCommand::Assists(Some(id)));
}

fn fly(w: &mut Sim, s: ShipId, input: PilotInput, ticks: usize) {
    for _ in 0..ticks {
        w.step(&TickInputs { commands: vec![(s, ShipCommand::Pilot(input))], ..Default::default() });
    }
}

/// No key held (the keyboard's axes, all released).
fn keys(turn: f32, thrust: f32) -> PilotInput {
    PilotInput { axes: Some(PilotAxes { turn, thrust, strafe: 0.0 }), ..Default::default() }
}

fn body(w: &Sim, s: ShipId) -> Body {
    let ship = w.ship(s).unwrap();
    ship.piece(ship.control_piece().unwrap()).unwrap().body
}

fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    let a = a.rem_euclid(t);
    if a > std::f32::consts::PI { a - t } else { a }
}

/// A piloted ship of `design` at the origin, nose up, with the content's behavior library, and idle raiders (no
/// behavior: they neither move nor fire) at the given points.
fn piloted(mine: std::sync::Arc<sim::ShipDesign>, raiders: &[(f32, f32)]) -> (Sim, ShipId, Vec<ShipId>) {
    let c = content();
    let mut w = Sim::new(3);
    w.set_behaviors(lib());
    let me = w.spawn_ship(mine, sim::Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
    w.command(me, ShipCommand::Focus);
    let rs = raiders
        .iter()
        .map(|&(x, y)| w.spawn_ship(design(&c, "automaton_raider"), sim::Owner(1), Body::at(Vec2::new(x, y), 0.0)))
        .collect();
    (w, me, rs)
}

/// RCS pods on three sides and a thruster at the back: it can push every way, so it can stop straight.
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

/// Sets the ship drifting at `v`.
fn drift(w: &mut Sim, s: ShipId, v: Vec2) {
    let ship = w.ship(s).unwrap();
    let piece = ship.control_piece().unwrap();
    let b = body(w, s);
    w.teleport(s, piece, Body { linvel: v, ..b });
}

fn speed(w: &Sim, s: ShipId) -> f32 {
    body(w, s).linvel.length()
}

#[test]
fn face_turns_to_the_target_and_steering_overrides_it() {
    let c = content();
    // A raider to the right: the nose (and the Runner's front guns) must turn from up to +x.
    let (mut w, me, _) = piloted(design(&c, "halvex_runner"), &[(45.0, 0.0)]);
    // Smart aim puts one of the two front guns (5.5 m either side of the centre line) on the target, so the nose
    // ends up to 0.15 rad off the centre bearing.
    let want = -std::f32::consts::FRAC_PI_2;
    // Without the assist the ship keeps its heading.
    fly(&mut w, me, keys(0.0, 0.0), 30);
    assert!(wrap(body(&w, me).angle).abs() < 0.01, "no assist: {}", body(&w, me).angle);
    assists(&mut w, me, &["always -> face_target"]);
    fly(&mut w, me, keys(0.0, 0.0), 120);
    let a = wrap(body(&w, me).angle - want);
    assert!(a.abs() < 0.3, "faces the target: off by {a}");
    assert!(w.ship(me).unwrap().assists().unwrap().decides(0));
    // The pilot steers (A held, ramped as the client does): the turn is the pilot's, away from the target.
    let before = body(&w, me).angle;
    let mut level = 0.0;
    for _ in 0..20 {
        level = turn_ramp_step(level, 1.0);
        fly(&mut w, me, keys(level, 0.0), 1);
    }
    let turned = wrap(body(&w, me).angle - before);
    assert!(turned > 0.25, "A turned it left, against the assist: {turned}");
    // Letting go hands the channel back: it faces the target again.
    fly(&mut w, me, keys(0.0, 0.0), 120);
    assert!(wrap(body(&w, me).angle - want).abs() < 0.3);
}

#[test]
fn brake_kills_drift_and_thrust_overrides_it() {
    let c = content();
    let (mut w, me, _) = piloted(rcs_ship(&c), &[]);
    // Coasting (GDR-0086) keeps the drift.
    drift(&mut w, me, Vec2::new(3.0, 2.0));
    fly(&mut w, me, keys(0.0, 0.0), 60);
    assert!(speed(&w, me) > 3.4, "coasts: {}", speed(&w, me));
    // Brake: the drift dies, without turning the ship.
    assists(&mut w, me, &["always -> brake"]);
    fly(&mut w, me, keys(0.0, 0.0), 120);
    assert!(speed(&w, me) < 0.05, "braked: {}", speed(&w, me));
    assert!(wrap(body(&w, me).angle).abs() < 0.02, "brake never turns: {}", body(&w, me).angle);
    // W held: thrust is the pilot's, the ship speeds up.
    fly(&mut w, me, keys(0.0, 1.0), 45);
    let pushed = speed(&w, me);
    assert!(pushed > 1.0, "W wins over brake: {pushed}");
    // Released: brake has thrust again.
    fly(&mut w, me, keys(0.0, 0.0), 150);
    assert!(speed(&w, me) < 0.05, "braked again: {}", speed(&w, me));
}

/// A pulse laser ahead in group 1, one left and one right in group 2 (the fire mask test's ship).
fn three_guns(c: &data::Content) -> std::sync::Arc<sim::ShipDesign> {
    let mut bp = blueprint(&[
        ("pulse_laser", 1, 0, 0, false, None),
        ("fission_reactor", 1, 1, 0, false, None),
        ("pulse_laser", 0, 1, 3, false, None),
        ("pulse_laser", 2, 1, 1, false, None),
        ("thruster", 0, 2, 0, false, None),
        ("logic_core", 1, 2, 0, false, None),
    ]);
    bp.parts[2].group = Some(2);
    bp.parts[3].group = Some(2);
    std::sync::Arc::new(sim::ShipDesign::compile(c, &bp).unwrap())
}

/// The guns of `ship` that fired in `ticks` ticks of `input`.
fn shots(w: &mut Sim, s: ShipId, input: PilotInput, ticks: usize) -> Vec<u32> {
    let mut fired = std::collections::BTreeSet::new();
    for _ in 0..ticks {
        fly(w, s, input, 1);
        for e in w.events() {
            if let SimEvent::ShotFired { ship, gun, .. } = e
                && *ship == s
            {
                fired.insert(*gun);
            }
        }
    }
    fired.into_iter().collect()
}

#[test]
fn auto_fire_fires_its_group_only_when_its_guns_bear() {
    let c = content();
    let d = three_guns(&c);
    // The target ahead: group 1's gun bears, group 2's (left and right) don't.
    for (rules, want) in [(["always -> auto_fire(1)"], vec![0]), (["always -> auto_fire(2)"], vec![])] {
        let (mut w, me, _) = piloted(d.clone(), &[(0.0, 30.0)]);
        assists(&mut w, me, &rules);
        assert_eq!(shots(&mut w, me, PilotInput::default(), 30), want, "{rules:?}, target ahead");
    }
    // The target to the right: only the right gun of group 2 fires; group 1 holds.
    for (rules, want) in [(["always -> auto_fire(2)"], vec![3]), (["always -> auto_fire(1)"], vec![])] {
        let (mut w, me, _) = piloted(d.clone(), &[(30.0, 0.0)]);
        assists(&mut w, me, &rules);
        assert_eq!(shots(&mut w, me, PilotInput::default(), 30), want, "{rules:?}, target right");
    }
    // Out of reach: nothing.
    let (mut w, me, _) = piloted(d.clone(), &[(0.0, 120.0)]);
    assists(&mut w, me, &["always -> auto_fire(1)"]);
    assert!(shots(&mut w, me, PilotInput::default(), 30).is_empty());
    // The pilot's trigger of group 2 wins: both group 2 guns fire, bearing or not; auto-fire keeps group 1.
    let (mut w, me, _) = piloted(d, &[(0.0, 30.0)]);
    assists(&mut w, me, &["always -> auto_fire(1)", "always -> auto_fire(2)"]);
    assert_eq!(shots(&mut w, me, PilotInput { fire: 0b010, ..Default::default() }, 30), vec![0, 2, 3]);
}

#[test]
fn conditions_gate_the_assists() {
    let c = content();
    // `lock_held`: brakes only while the pilot holds a lock.
    let (mut w, me, rs) = piloted(rcs_ship(&c), &[(0.0, 80.0)]);
    assists(&mut w, me, &["when lock_held -> brake"]);
    drift(&mut w, me, Vec2::new(2.0, 0.0));
    fly(&mut w, me, keys(0.0, 0.0), 60);
    assert!(speed(&w, me) > 1.9, "no lock, no brake: {}", speed(&w, me));
    let locked = PilotInput { lock: Some(Target::Ship(rs[0])), ..keys(0.0, 0.0) };
    fly(&mut w, me, locked, 120);
    assert!(speed(&w, me) < 0.05, "locked: braked: {}", speed(&w, me));

    // `idle(thrust)`: brakes once thrust has been let go for a second.
    let (mut w, me, _) = piloted(rcs_ship(&c), &[]);
    assists(&mut w, me, &["when idle(thrust) >= 1s -> brake"]);
    fly(&mut w, me, keys(0.0, 1.0), 30);
    let v0 = speed(&w, me);
    fly(&mut w, me, keys(0.0, 0.0), 21);
    assert!(speed(&w, me) > v0 - 0.05, "still coasting within the second: {} of {v0}", speed(&w, me));
    fly(&mut w, me, keys(0.0, 0.0), 120);
    assert!(speed(&w, me) < 0.05, "then braked: {}", speed(&w, me));

    // `speed` and `can_stop_straight`: a ship with its only engine at the back can't stop a forward drift
    // straight, so the rule doesn't decide; drifting backwards it can.
    let back_only = custom(
        &c,
        &[("fission_reactor", 0, 0, 0, false, None), ("logic_core", 0, 1, 0, false, None), ("thruster", 0, 2, 0, false, None)],
    );
    let (mut w, me, _) = piloted(back_only, &[]);
    assists(&mut w, me, &["when speed > 1% and can_stop_straight -> brake"]);
    drift(&mut w, me, Vec2::new(0.0, 2.0));
    fly(&mut w, me, keys(0.0, 0.0), 30);
    assert!(!w.ship(me).unwrap().assists().unwrap().decides(0), "forward: it would have to turn");
    assert!(speed(&w, me) > 1.9, "so it drifts on: {}", speed(&w, me));
    drift(&mut w, me, Vec2::new(0.0, -2.0));
    fly(&mut w, me, keys(0.0, 0.0), 30);
    assert!(speed(&w, me) < 0.05, "backwards: the engine pushes against it: {}", speed(&w, me));
    assert!(!w.ship(me).unwrap().assists().unwrap().decides(0), "standing still: speed is not above 1%");
}

#[test]
fn toggles_and_all_assists_switch_rules_for_the_mission() {
    let c = content();
    let (mut w, me, _) = piloted(rcs_ship(&c), &[]);
    assists(&mut w, me, &["always -> brake(toggle2, off)", "always -> face_target(toggle2)"]);
    let st = |w: &Sim| w.ship(me).unwrap().assists().unwrap().clone();
    assert_eq!((st(&w).is_on(0), st(&w).is_on(1)), (false, true), "each rule starts as its `start` says");
    assert_eq!((st(&w).toggle(0), st(&w).toggle(1)), (Some(2), Some(2)));
    drift(&mut w, me, Vec2::new(2.0, 0.0));
    fly(&mut w, me, keys(0.0, 0.0), 30);
    assert!(speed(&w, me) > 1.9, "brake starts off");
    // Toggle 2 flips both of its rules: brake on (face off, no target anyway).
    w.command(me, ShipCommand::SwitchAssists(AssistSwitch::Toggle(2)));
    assert_eq!((st(&w).is_on(0), st(&w).is_on(1)), (true, false));
    fly(&mut w, me, keys(0.0, 0.0), 120);
    assert!(speed(&w, me) < 0.05, "toggled on: braked");
    // Another slot changes nothing here; ALL ASSISTS turns every assist off and back as the toggles left them.
    w.command(me, ShipCommand::SwitchAssists(AssistSwitch::Toggle(1)));
    assert!(st(&w).is_on(0));
    w.command(me, ShipCommand::SwitchAssists(AssistSwitch::All));
    assert!(!st(&w).all_on());
    drift(&mut w, me, Vec2::new(2.0, 0.0));
    fly(&mut w, me, keys(0.0, 0.0), 30);
    assert!(speed(&w, me) > 1.9, "all off: coasts");
    assert!(!st(&w).decides(0));
    w.command(me, ShipCommand::SwitchAssists(AssistSwitch::All));
    fly(&mut w, me, keys(0.0, 0.0), 120);
    assert!(speed(&w, me) < 0.05, "all on again: brake is back");
}

#[test]
fn only_the_piloted_ship_runs_its_assists() {
    let c = content();
    let (mut w, me, _) = piloted(rcs_ship(&c), &[]);
    let other = w.spawn_ship(rcs_ship(&c), sim::Owner::PLAYER, Body::at(Vec2::new(30.0, 0.0), 0.0));
    assists(&mut w, other, &["always -> brake"]);
    // Not the focus, it flies its helm (a raw drift here): the assists neither think nor act.
    w.command(other, ShipCommand::Goal { goal: sim::FlightGoal { heading: sim::HeadingGoal::Keep, velocity: None }, fire: false });
    drift(&mut w, other, Vec2::new(2.0, 0.0));
    fly(&mut w, me, keys(0.0, 0.0), 60);
    assert!(speed(&w, other) > 1.9, "unpiloted: no brake");
    assert!(w.ship(other).unwrap().assists().unwrap().target().is_none());
    // An unknown list is ignored; `None` takes the list away.
    w.command(me, ShipCommand::Assists(Some(sim::AssistsId(99))));
    assert!(w.ship(me).unwrap().assists().is_none());
    assists(&mut w, me, &["always -> brake"]);
    w.command(me, ShipCommand::Assists(None));
    assert!(w.ship(me).unwrap().assists().is_none());
}
