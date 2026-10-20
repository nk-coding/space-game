//! Disruption (GDR-0018, GDR-0125): ion pulses carry static into a vessel's cores past armour, not past shields;
//! enough static blinds its mind (no target, no enemies seen, sensors dark), then it is immune for a while; more
//! cores hold more static; determinism.

mod common;

use common::*;
use sim::combat::disruption::tuning;
use sim::{Body, Owner, ShipId, SimEvent, TickInputs, Vec2};

/// An ion cannon on a reactor and a core, facing +y.
fn ion_ship() -> Vec<P<'static>> {
    vec![
        ("ion_cannon", 0, 0, 0, false, None),
        ("fission_reactor", 0, 1, 0, false, None),
        ("logic_core", 0, 2, 0, false, None),
    ]
}

/// A target in steel, 3 wide and 3 + `front` deep: a core in its far row, a reactor behind it, `near` in the middle
/// of its near row (the one the ion cannon hits), `cores` more logic cores on the far row's sides.
fn target(near: Option<(&'static str, u8)>, cores: usize, front: i32) -> Vec<P<'static>> {
    let mut v = Vec::new();
    for y in 0..3 + front {
        for x in 0..3 {
            let part = match (x, y, near) {
                (1, 0, _) => ("logic_core", 0),
                (0, 0, _) if cores >= 1 => ("logic_core", 0),
                (2, 0, _) if cores >= 2 => ("logic_core", 0),
                (1, 1, _) => ("fission_reactor", 0),
                (1, y, Some(e)) if y == 2 + front => e,
                _ => ("armor_block", 0),
            };
            let material = (part.0 == "armor_block").then_some("steel");
            v.push((part.0, x, y, part.1, false, material));
        }
    }
    v
}

/// The ion ship at the origin and an enemy vessel (owner 1) 30 m ahead, flying `behavior` if given.
fn duel(near: Option<(&'static str, u8)>, cores: usize, behavior: Option<&str>) -> (sim::Sim, ShipId, ShipId) {
    duel_front(near, cores, 0, behavior)
}

/// [`duel`] with `front` more rows of armour on the target's near side.
fn duel_front(
    near: Option<(&'static str, u8)>,
    cores: usize,
    front: i32,
    behavior: Option<&str>,
) -> (sim::Sim, ShipId, ShipId) {
    let c = content();
    let mut w = sim::Sim::new(7);
    let s = w.spawn_ship(custom(&c, &ion_ship()), Owner::PLAYER, Body::default());
    let t = w.spawn_ship(custom(&c, &target(near, cores, front)), Owner(1), Body::at(Vec2::new(0.0, 30.0), 0.0));
    if let Some(text) = behavior {
        w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
        let id = w.behaviors_mut().add_text("test", text).unwrap();
        w.command(t, sim::ShipCommand::Behavior(Some(id)));
    }
    (w, s, t)
}

/// One tick with the ion cannon firing (the target's guns held, its shields on).
fn step(w: &mut sim::Sim, s: ShipId, t: ShipId) {
    let mut controls = vec![(s, controls(0, 0.0, true, true))];
    if w.ship(t).unwrap().behavior().is_none() {
        controls.push((t, common::controls(0, 0.0, false, true)));
    }
    w.step(&TickInputs { controls, ..Default::default() });
}

/// Ticks at which `t` was blinded over `ticks` ticks of fire.
fn blindings(w: &mut sim::Sim, s: ShipId, t: ShipId, ticks: usize) -> Vec<u64> {
    let mut at = Vec::new();
    for _ in 0..ticks {
        step(w, s, t);
        for e in w.events() {
            if let SimEvent::Blinded { by, ship, .. } = e
                && *ship == t
            {
                assert_eq!(*by, s);
                at.push(w.tick());
            }
        }
    }
    at
}

#[test]
fn ion_pulses_blind_a_vessel_through_its_armour() {
    let (mut w, s, t) = duel(None, 0, None);
    let at = blindings(&mut w, s, t, 120);
    assert!(!at.is_empty(), "blinded within 4 s");
    // Static rides on the shot's small damage: at most the near armour block is gone.
    let ship = w.ship(t).unwrap();
    assert!(ship.parts().iter().filter(|p| !p.alive).count() <= 1);
    assert!(ship.blinded() || ship.jam().immune > 0.0);
}

#[test]
fn a_blinded_behavior_loses_its_target_and_sees_no_enemy() {
    let (mut w, s, t) = duel(None, 0, Some("target nearest\nwhen enemies_near(100m) >= 1 -> face\nalways -> hold"));
    let mut seen = false;
    let mut blinded_at = None;
    for _ in 0..240 {
        step(&mut w, s, t);
        let ship = w.ship(t).unwrap();
        let b = ship.behavior().unwrap();
        if blinded_at.is_none() {
            seen |= b.target() == Some(s);
            if ship.blinded() {
                blinded_at = Some(w.tick());
            }
            continue;
        }
        // A think later (they run every 3 ticks), and while it lasts: no target, the `face` rule fails.
        if ship.blinded() && w.tick() >= blinded_at.unwrap() + 3 {
            assert_eq!(b.target(), None);
            assert_eq!(ship.sensor_range(), 0.0);
            assert_eq!(b.active_rule(), Some(1), "only `always -> hold` holds");
        }
    }
    assert!(seen, "it had its target before");
    let blinded_at = blinded_at.expect("blinded");
    // The blinding ran out: it sees again and picks its target back up.
    let end = blinded_at + (tuning::BLIND * sim::TICK_HZ as f32) as u64 + 3;
    assert!(w.tick() > end);
    assert_eq!(w.ship(t).unwrap().behavior().unwrap().target(), Some(s));
}

#[test]
fn a_shield_bubble_stops_the_static() {
    // The emitter's bubble covers the near row; it outlasts these 10 s (the ion cannon's damage is below 2 / 10 s
    // plus the regeneration).
    let (mut w, s, t) = duel(Some(("shield_emitter", 0)), 0, None);
    let at = blindings(&mut w, s, t, 300);
    assert!(at.is_empty(), "{at:?}");
    assert!(w.ship(t).unwrap().jam().is_clear());
}

#[test]
fn after_a_blinding_static_does_not_build_up_for_a_while() {
    // Armour enough for the 20 s.
    let (mut w, s, t) = duel_front(None, 0, 6, None);
    let at = blindings(&mut w, s, t, 600);
    assert!(at.len() >= 2, "{at:?}");
    let gap = ((tuning::BLIND + tuning::IMMUNE) * sim::TICK_HZ as f32) as u64;
    assert!(at.windows(2).all(|p| p[1] - p[0] >= gap), "{at:?}");
}

#[test]
fn more_cores_hold_more_static() {
    let first = |cores| {
        let (mut w, s, t) = duel(None, cores, None);
        assert_eq!(w.ship(t).unwrap().static_limit(), tuning::STATIC_PER_CORE * (cores + 1) as f32);
        blindings(&mut w, s, t, 300).first().copied()
    };
    let (one, three) = (first(0).unwrap(), first(2).unwrap());
    assert!(three > one, "{one} vs {three}");
}

#[test]
fn disruption_fights_are_deterministic() {
    let hashes = || {
        let (mut w, s, t) = duel(None, 1, Some("target nearest\nalways -> face"));
        (0..300)
            .map(|_| {
                step(&mut w, s, t);
                w.state_hash()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(hashes(), hashes());
}

#[test]
fn the_blinded_flag_holds_while_static_blinds_the_mind() {
    // GDR-0129: `blinded` lets a rule fly something on purpose while the mind is blind.
    let (mut w, s, t) = duel(None, 0, Some("target nearest\nwhen blinded -> hold\nwhen enemies_near(100m) >= 1 -> face\nalways -> hold"));
    let (mut clear, mut blind) = (false, false);
    for _ in 0..240 {
        step(&mut w, s, t);
        let ship = w.ship(t).unwrap();
        let rule = ship.behavior().unwrap().active_rule();
        // A think runs every 3 ticks: judge only once the state has held for one.
        if ship.blinded() && ship.jam().blind <= tuning::BLIND - 0.2 {
            assert_eq!(rule, Some(0));
            blind = true;
        } else if !ship.blinded() && rule == Some(1) {
            clear = true;
        }
    }
    assert!(clear && blind, "it faced the ion ship before, and held while blinded");
}
