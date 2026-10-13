//! Engine exhaust plumes (GDR-0039, GDR-0046, GDR-0049): engines may sit anywhere; the plume burns the first part
//! in its path (own hull, enemies, salvage; never other vessels of the same owner), scaled by the thrust output;
//! a blocked plume keeps full thrust; enemy bubbles absorb it, the ship's own never.

mod common;

use std::sync::Arc;

use common::*;
use sim::mass::to_piece;
use sim::{Body, DamageTarget, Owner, ShipDesign, ShipId, Sim, SimEvent, Vec2};

/// Core and reactor on top, a thruster pushing up with two composite blocks right behind it (a buried
/// engine): its plume runs down column 1 into (1, 2), then (1, 3).
fn burner(c: &data::Content) -> Arc<ShipDesign> {
    custom(
        c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("fission_reactor", 1, 0, 0, false, None),
            ("thruster", 1, 1, 0, false, None),
            ("armor_block", 1, 2, 0, false, Some("composite")),
            ("armor_block", 1, 3, 0, false, Some("composite")),
        ],
    )
}

fn plume(c: &data::Content, id: &str) -> data::PlumeSpec {
    c.parts
        .get(id)
        .unwrap()
        .engine
        .as_ref()
        .unwrap()
        .plume
        .clone()
}

/// Plume hits of the last step: (by, ship, part, damage, heat, absorbed).
fn burns(w: &Sim) -> Vec<(ShipId, ShipId, u32, f32, f32, f32)> {
    w.events()
        .iter()
        .filter_map(|e| match *e {
            SimEvent::PlumeHit {
                by,
                ship,
                part,
                damage,
                heat,
                absorbed,
                ..
            } => Some((by, ship, part, damage, heat, absorbed)),
            _ => None,
        })
        .collect()
}

/// One step at full throttle on every engine of `s`; `hold` keeps the ship at rest (as if pushing against a
/// dock), so its engines never reach their rated speed.
fn burn_step(w: &mut Sim, s: ShipId, hold: bool) {
    let engines = w.ship(s).unwrap().design().engines.len();
    w.step(&inputs(s, controls(engines, 1.0, false, true)));
    if hold {
        let piece = w.ship(s).unwrap().control_piece().unwrap();
        w.teleport(s, piece, Body::default());
    }
}

#[test]
fn a_buried_engine_keeps_full_thrust_and_burns_through_its_own_hull() {
    let c = content();
    let d = burner(&c);
    let (thrust, p) = (c.parts.get("thruster").unwrap().engine.as_ref().unwrap().thrust, plume(&c, "thruster"));
    let (mut w, s) = world_with(d.clone());
    let (first, second) = (find(&d, "armor_block", 1, 2), find(&d, "armor_block", 1, 3));
    burn_step(&mut w, s, true);
    // Full thrust although the exhaust is blocked (GDR-0049: the cost is the burning).
    let piece = &w.ship(s).unwrap().pieces()[0];
    assert!(close(piece.wrench.force.y, thrust, 1e-3), "{:?}", piece.wrench);
    // The block right behind it takes the plume: damage and heat per tick at full output, no shields.
    let b = burns(&w);
    assert_eq!(b.len(), 1, "{b:?}");
    let (by, ship, part, damage, heat, absorbed) = b[0];
    assert_eq!((by, ship, part), (s, s, first));
    assert!(close(damage, p.damage / 30.0, 1e-6) && close(heat, p.heat / 30.0, 1e-6) && absorbed == 0.0);
    assert!(w.ship(s).unwrap().part(first).heat > 0.0);
    // It burns through the block (composite, 1.4 hp) in under two seconds, then starts on the next one.
    let mut gone = None;
    for t in 2..=120 {
        burn_step(&mut w, s, true);
        let ship = w.ship(s).unwrap();
        if gone.is_none() && !ship.part(first).alive {
            gone = Some(t);
        }
        if gone.is_some() {
            break;
        }
    }
    let gone = gone.expect("the plume burned through the block") as f32 / 30.0;
    println!("buried thruster: first block (1.4 hp) gone after {gone:.2} s");
    assert!((1.2..2.0).contains(&gone), "{gone} s");
    burn_step(&mut w, s, true);
    let b = burns(&w);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].2, second, "the plume reaches the next block once the first is gone");
    for _ in 0..60 {
        burn_step(&mut w, s, true);
    }
    assert!(!w.ship(s).unwrap().part(second).alive, "both blocks within about three seconds");
}

#[test]
fn plume_damage_scales_with_the_thrust_output() {
    let c = content();
    let p = plume(&c, "thruster");
    let (mut w, s) = world_with(burner(&c));
    w.step(&inputs(s, controls(1, 0.0, false, true)));
    assert!(burns(&w).is_empty(), "an idle engine burns nothing");
    w.step(&inputs(s, controls(1, 0.5, false, true)));
    let b = burns(&w);
    assert!(close(b[0].3, 0.5 * p.damage / 30.0, 1e-6), "{b:?}");
    // At its rated speed the engine gives no thrust any more, and its plume burns nothing.
    let piece = w.ship(s).unwrap().control_piece().unwrap();
    let rated = c.parts.get("thruster").unwrap().engine.as_ref().unwrap().rated_speed;
    w.teleport(
        s,
        piece,
        Body {
            linvel: Vec2::new(0.0, rated),
            ..Body::default()
        },
    );
    w.step(&inputs(s, controls(1, 1.0, false, true)));
    assert!(burns(&w).is_empty(), "{:?}", burns(&w));
}

/// A Halvex Runner at the origin, nose up, and `target` spawned for `owner` with its nose pointing up at the
/// Runner's tail, `gap` metres below it, offset 0.3 m to the right so the torch drive's two lanes run down two of
/// its columns.
fn tail_chase(c: &data::Content, target: Arc<ShipDesign>, owner: Owner, gap: f32) -> (Sim, ShipId, ShipId) {
    tail_chase_with(design(c, "halvex_runner"), target, owner, gap)
}

/// [`tail_chase`] with a Runner design of its own (stats changed in memory).
fn tail_chase_with(runner: Arc<ShipDesign>, target: Arc<ShipDesign>, owner: Owner, gap: f32) -> (Sim, ShipId, ShipId) {
    let mut w = Sim::new(1);
    let r = w.spawn_ship(runner.clone(), Owner::PLAYER, Body::default());
    let origin = w.ship(r).unwrap().pieces()[0].origin;
    let tail = to_piece([6.0, runner.grid.bounds().unwrap().max_y as f32], origin);
    let t = w.spawn_ship(target.clone(), owner, Body::default());
    let p = &w.ship(t).unwrap().pieces()[0];
    let (to, b) = (p.origin, target.grid.bounds().unwrap());
    let top_centre = to_piece([(b.min_x + b.max_x) as f32 / 2.0, b.min_y as f32], to);
    let pos = tail - top_centre + Vec2::new(0.3, -gap);
    let id = p.id;
    w.teleport(t, id, Body::at(pos, 0.0));
    (w, r, t)
}

#[test]
fn a_torch_drive_held_on_a_raider_wrecks_a_part_or_two_in_a_second() {
    let c = content();
    let raider = design(&c, "automaton_raider");
    let (mut w, r, t) = tail_chase(&c, raider, Owner(1), 0.5);
    let mut destroyed = 0;
    let mut damage = 0.0f32;
    for _ in 0..30 {
        burn_step(&mut w, r, false);
        for e in w.events() {
            match *e {
                SimEvent::PartDestroyed { ship, .. } if ship == t => destroyed += 1,
                SimEvent::PlumeHit { by, ship, damage: d, .. } if by == r && ship == t => damage += d,
                SimEvent::PlumeHit { ship, .. } => assert_ne!(ship, r, "the Runner's exhaust is clear"),
                _ => {}
            }
        }
    }
    let gap = {
        let a = w.ship(r).unwrap().pieces()[0].body.pos;
        let b = w.ship(t).unwrap().pieces()[0].body.pos;
        (a - b).length()
    };
    println!("torch drive on a raider for 1 s: {damage:.2} hp, {destroyed} parts destroyed (ships {gap:.1} m apart)");
    assert!((1..=3).contains(&destroyed), "{destroyed} parts, {damage} hp");
}

#[test]
fn a_plume_spares_friendly_vessels_and_burns_salvage() {
    let c = content();
    let raider = design(&c, "automaton_raider");
    // A raider of the Runner's own owner (a friendly vessel): untouched.
    let (mut w, r, t) = tail_chase(&c, raider.clone(), Owner::PLAYER, 0.5);
    for _ in 0..30 {
        burn_step(&mut w, r, true);
        assert!(burns(&w).is_empty(), "{:?}", burns(&w));
    }
    let ship = w.ship(t).unwrap();
    assert!(ship.parts().iter().zip(&raider.parts).all(|(p, s)| p.alive && p.hp == s.hp));
    // The same raider as a wreck of the same owner (core shot away): salvage is nobody's friend.
    let (mut w, r, t) = tail_chase(&c, raider.clone(), Owner::PLAYER, 0.5);
    let core = find(&raider, "automaton_core", 1, 1);
    w.apply_damage(t, DamageTarget::Part(core), 10.0);
    burn_step(&mut w, r, true);
    assert!(!w.ship(t).unwrap().is_vessel());
    burn_step(&mut w, r, true);
    let b = burns(&w);
    assert!(!b.is_empty() && b.iter().all(|x| x.0 == r && x.1 == t), "{b:?}");
}

/// A shielded enemy target: steel blocks in a row with a shield emitter in the middle of the second row, its
/// reactor and core beside it. The bubble (2.3 m) covers the top row.
fn shielded(c: &data::Content) -> Arc<ShipDesign> {
    custom(
        c,
        &[
            ("armor_block", 0, 0, 0, false, Some("steel")),
            ("armor_block", 1, 0, 0, false, Some("steel")),
            ("armor_block", 2, 0, 0, false, Some("steel")),
            ("fission_reactor", 0, 1, 0, false, None),
            ("shield_emitter", 1, 1, 0, false, None),
            ("logic_core", 2, 1, 0, false, None),
        ],
    )
}

#[test]
fn enemy_bubbles_absorb_a_plume_but_own_bubbles_never_stop_the_own_exhaust() {
    let c = content();
    let (mut w, r, t) = tail_chase(&c, shielded(&c), Owner(1), 0.5);
    burn_step(&mut w, r, true); // bubbles come up in the first tick's systems
    burn_step(&mut w, r, true);
    let shield_hits = w
        .events()
        .iter()
        .filter(|e| matches!(e, SimEvent::ShieldHit { by, ship, .. } if *by == r && *ship == t))
        .count();
    assert!(shield_hits > 0, "{:?}", w.events());
    // Energy is absorbed fully while the bubble holds: nothing reaches the blocks.
    assert!(burns(&w).iter().all(|b| b.3 == 0.0 && b.4 == 0.0), "{:?}", burns(&w));
    let ship = w.ship(t).unwrap();
    assert!(ship.parts().iter().all(|p| p.alive && p.heat < 0.5));

    // A buried engine beside its own shield emitter: the bubble covers the block, the block burns anyway.
    let d = custom(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("fission_reactor", 1, 0, 0, false, None),
            ("shield_emitter", 0, 1, 0, false, None),
            ("thruster", 1, 1, 0, false, None),
            ("armor_block", 1, 2, 0, false, Some("composite")),
        ],
    );
    let (mut w, s) = world_with(d.clone());
    burn_step(&mut w, s, true);
    burn_step(&mut w, s, true);
    assert!(!w.bubbles().is_empty());
    let b = burns(&w);
    assert!(b.len() == 1 && b[0].3 > 0.0 && b[0].5 == 0.0, "{b:?}");
    assert!(!w.events().iter().any(|e| matches!(e, SimEvent::ShieldHit { .. })));
}

/// A plume that does only heat (13 Oct 2026 review: it ignored shields): an enemy bubble covering the hit point
/// takes its energy share of the heat (all of it for the base emitter) without spending its charge.
#[test]
fn a_heat_only_plume_is_stopped_by_enemy_bubbles_too() {
    let c = content();
    let mut runner = (*design(&c, "halvex_runner")).clone();
    for p in runner.parts.iter_mut().filter(|p| p.is_engine) {
        p.base.v[sim::Stat::PlumeDamage as usize] = 0.0;
    }
    let target = shielded(&c);
    let emitter = find(&target, "shield_emitter", 1, 1);
    let (mut w, r, t) = tail_chase_with(Arc::new(runner), target, Owner(1), 0.5);
    burn_step(&mut w, r, true); // bubbles come up in the first tick's systems
    let full = w.ship(t).unwrap().part(emitter).shield;
    for _ in 0..30 {
        burn_step(&mut w, r, true);
        assert!(burns(&w).iter().all(|b| b.1 != t), "{:?}", burns(&w));
    }
    let ship = w.ship(t).unwrap();
    assert!(ship.part(emitter).shield >= full, "heat spends no charge");
    // Without the bubble the heat arrives.
    w.apply_damage(t, DamageTarget::Part(emitter), 100.0);
    burn_step(&mut w, r, true);
    burn_step(&mut w, r, true);
    let b = burns(&w);
    assert!(!b.is_empty() && b.iter().all(|x| x.1 == t && x.3 == 0.0 && x.4 > 0.0), "{b:?}");
}
