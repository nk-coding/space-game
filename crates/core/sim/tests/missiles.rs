//! Missiles and point defence (GDR-0115): homing, the warhead's blast, shields absorbing missiles whole, point
//! defence shooting them down (only what it sees), flares luring them off (charges, cooldown, the `flare` action and
//! the `auto_flare` assist), determinism.

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

#[test]
fn point_defence_does_not_shoot_through_a_hull() {
    // A wreck's wall of steel three blocks thick (no core: salvage) between the launcher and the turret: the
    // rockets strike the wall, and the turret behind it never sees them.
    let wall: Vec<P<'static>> =
        (0..39).map(|i| ("armor_block", i % 13, i / 13, 0, false, Some("steel"))).collect();
    let (mut w, s, t) = duel(Some(("point_defence", 2)), Vec2::new(0.0, 45.0));
    let wreck = w.spawn_ship(custom(&content(), &wall), Owner(1), Body::at(Vec2::new(0.0, 40.0), 0.0));
    let c = w.ship(wreck).unwrap().pieces()[0].com_world();
    assert!((c.x).abs() < 1.0 && (c.y - 40.0).abs() < 1.0, "the wall lies across the line: {c:?}");
    let n = run(&mut w, s, t, 150);
    assert!(!n.warheads.is_empty(), "{n:?}");
    assert_eq!(n.shot_down, 0, "{n:?}");
}

/// The duel with a flare launcher in the target's near row (facing the launcher): the launcher fires if `shoot`;
/// the target asks for flares on every tick while a missile homing on it is within `near` m of it.
fn flare_duel(shoot: bool, near: f32, ticks: usize) -> (Tally, usize, Vec<u64>) {
    let (mut w, s, t) = duel(Some(("flare_launcher", 2)), Vec2::new(0.0, 45.0));
    let (mut n, mut released, mut at) = (Tally::default(), 0, Vec::new());
    for _ in 0..ticks {
        let mut inputs = TickInputs {
            controls: vec![(s, controls(0, 0.0, shoot, true)), (t, controls(0, 0.0, false, true))],
            ..Default::default()
        };
        let homing = w.projectiles().iter().any(|p| {
            p.missile.is_some_and(|m| m.target == Some(t)) && (p.pos - Vec2::new(0.0, 45.0)).length() < near
        });
        if homing || !shoot {
            inputs.commands.push((t, sim::ShipCommand::Flare));
        }
        w.step(&inputs);
        for e in w.events() {
            match e {
                SimEvent::FlaresReleased { ship, decoys, .. } if *ship == t => {
                    released += *decoys as usize;
                    at.push(w.tick());
                }
                SimEvent::Warhead { by, at, .. } if *by == s => n.warheads.push(*at),
                SimEvent::ProjectileGone { reason: GoneReason::Decoyed, .. } => n.absorbed += 1,
                _ => {}
            }
        }
    }
    (n, released, at)
}

#[test]
fn flares_pull_rockets_off_their_target() {
    let (with, released, _) = flare_duel(true, 25.0, 300);
    let (mut w, s, t) = duel(None, Vec2::new(0.0, 45.0));
    let without = run(&mut w, s, t, 300);
    assert!(released > 0, "{with:?}");
    assert!(with.absorbed > 0, "rockets spent on decoys: {with:?}");
    assert!(with.warheads.len() < without.warheads.len(), "{with:?} vs {without:?}");
}

#[test]
fn a_flare_launcher_has_its_charges_and_cooldown() {
    let c = content();
    let spec = c.parts.get("flare_launcher").unwrap().countermeasure.unwrap();
    // Asked on every tick for 40 s, nothing shooting: one release per cooldown, until the charges are spent.
    let (_, released, at) = flare_duel(false, 0.0, 1200);
    assert_eq!(at.len(), spec.charges as usize, "{at:?}");
    assert_eq!(released, (spec.charges * spec.decoys) as usize);
    let gap = (spec.cooldown * sim::TICK_HZ as f32) as u64;
    assert!(at.windows(2).all(|p| p[1] - p[0] >= gap), "{at:?}");
}

#[test]
fn missile_and_flare_fights_are_deterministic() {
    let hashes = || {
        let (mut w, s, t) = duel(Some(("flare_launcher", 2)), Vec2::new(6.0, 40.0));
        (0..150)
            .map(|i| {
                let mut inputs = fire(s, t);
                if i % 20 == 0 {
                    inputs.commands.push((t, sim::ShipCommand::Flare));
                }
                w.step(&inputs);
                w.state_hash()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(hashes(), hashes());
}

/// Flares released (ticks) by `t` while the rocket pod `s` fires at it, from a world set up by the caller.
fn flare_ticks(w: &mut sim::Sim, s: ShipId, t: ShipId, piloted: bool, ticks: usize) -> (Vec<u64>, Option<u64>) {
    let (mut at, mut first_homing) = (Vec::new(), None);
    for _ in 0..ticks {
        let mut inputs = TickInputs { controls: vec![(s, controls(0, 0.0, true, true))], ..Default::default() };
        if piloted {
            inputs.commands.push((t, sim::ShipCommand::Pilot(sim::PilotInput::default())));
        }
        w.step(&inputs);
        if first_homing.is_none() && w.projectiles().iter().any(|p| p.missile.is_some_and(|m| m.target == Some(t))) {
            first_homing = Some(w.tick());
        }
        for e in w.events() {
            if let SimEvent::FlaresReleased { ship, .. } = e
                && *ship == t
            {
                at.push(w.tick());
            }
        }
    }
    (at, first_homing)
}

#[test]
fn a_behavior_and_the_auto_flare_assist_release_flares_when_missiles_come() {
    let c = content();
    // A behavior: the target runs `flare` under `missiles_incoming`.
    let (mut w, s, t) = duel(Some(("flare_launcher", 2)), Vec2::new(0.0, 45.0));
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let id = w
        .behaviors_mut()
        .add_text("test", "target nearest\nwhen missiles_incoming >= 1 -> flare\nalways -> hold")
        .unwrap();
    w.command(t, sim::ShipCommand::Behavior(Some(id)));
    let (at, homing) = flare_ticks(&mut w, s, t, false, 150);
    assert!(!at.is_empty() && homing.is_some_and(|h| at[0] >= h), "{at:?} {homing:?}");
    // An assist: the player's flown ship, under fire from an enemy pod.
    let mut w = sim::Sim::new(7);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let s = w.spawn_ship(custom(&c, &launcher()), Owner(1), Body::default());
    let t = w.spawn_ship(custom(&c, &target(Some(("flare_launcher", 2)))), Owner::PLAYER, Body::at(Vec2::new(0.0, 45.0), 0.0));
    w.command(t, sim::ShipCommand::Focus);
    let list = data::AssistList { rules: vec!["when missiles_incoming >= 1 -> auto_flare".into()] };
    let id = w.behaviors_mut().add_assists(&list).unwrap();
    w.command(t, sim::ShipCommand::Assists(Some(id)));
    let (at, homing) = flare_ticks(&mut w, s, t, true, 150);
    assert!(!at.is_empty() && homing.is_some_and(|h| at[0] >= h), "{at:?} {homing:?}");
}
