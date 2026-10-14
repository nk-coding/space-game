//! Power allocation (GDR-0014): tiers, weights, minimum levels, blueprint overrides, runtime settings, and
//! partial output scaling what parts do. The allocator's arithmetic has unit tests in `src/ship/power.rs`.

mod common;

use common::*;
use data::{PowerOverride, PowerTier, WeaponFamily};
use sim::{
    DamageTarget, PartIdx, PowerSetting, ShipCommand, ShipDesign, ShipId, Sim, SimEvent,
    TickInputs, Vec2,
};
use std::sync::Arc;

use PowerTier::*;

/// Three pulse lasers on top; core | shield | fission reactor below them; a thruster under the shield.
/// Supply 10 MW. Demand at full use: core 1 (critical), shield 3 (high), thruster 2 (normal), lasers 3 × 4
/// (low). Stays below every heat limit for 20 s.
fn gunship() -> Vec<P<'static>> {
    vec![
        ("pulse_laser", 0, 0, 0, false, None),
        ("pulse_laser", 1, 0, 0, false, None),
        ("pulse_laser", 2, 0, 0, false, None),
        ("logic_core", 0, 1, 0, false, None),
        ("shield_emitter", 1, 1, 0, false, None),
        ("fission_reactor", 2, 1, 0, false, None),
        ("thruster", 1, 2, 0, false, None),
    ]
}
const LASERS: [PartIdx; 3] = [0, 1, 2];
const CORE: PartIdx = 3;
const SHIELD: PartIdx = 4;
const REACTOR: PartIdx = 5;
const THRUSTER: PartIdx = 6;

fn all_on(s: ShipId) -> TickInputs {
    inputs(s, controls(1, 1.0, true, true))
}

fn set(s: ShipId, part: PartIdx, tier: PowerTier, weight: f32) -> TickInputs {
    let mut t = all_on(s);
    t.commands.push((
        s,
        ShipCommand::Power {
            part,
            setting: PowerSetting { tier, weight },
        },
    ));
    t
}

fn level(w: &Sim, s: ShipId, part: PartIdx) -> f32 {
    w.ship(s).unwrap().part(part).power
}

#[test]
fn tiers_are_served_in_order_and_off_never_draws() {
    let c = content();
    let (mut w, s) = world_with(custom(&c, &gunship()));
    w.step(&all_on(s));
    // Critical 1, high 3 and normal 2 fit into 10; the low tier shares the last 4 MW: a third each.
    for p in [CORE, SHIELD, THRUSTER] {
        assert_eq!(level(&w, s, p), 1.0, "part {p}");
    }
    for p in LASERS {
        assert!(close(level(&w, s, p), 1.0 / 3.0, 1e-6), "laser {p}");
    }
    let power = w.ship(s).unwrap().pieces()[0].power;
    assert_eq!(
        (power.supply, power.demand, power.served),
        (10.0, 18.0, 10.0)
    );

    // One laser moved up to critical is fed first; the low tier now gets nothing.
    w.step(&set(s, LASERS[0], Critical, 1.0));
    assert_eq!(level(&w, s, LASERS[0]), 1.0);
    assert_eq!(level(&w, s, SHIELD), 1.0);
    assert_eq!(level(&w, s, THRUSTER), 1.0);
    assert_eq!(level(&w, s, LASERS[1]), 0.0);
    assert_eq!(level(&w, s, LASERS[2]), 0.0);
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.part(LASERS[1]).work, 0.0);
    let guns: Vec<f32> = ship.pieces()[0].guns.iter().map(|g| g.output).collect();
    assert!(
        guns[0] > 0.0 && guns[1] == 0.0 && guns[2] == 0.0,
        "{guns:?}"
    );

    // `Off` never draws: the shield goes dark (no bubble), its demand no longer counts, and the 3 MW go to
    // the low tier (critical 5, normal 2, low 8 → 3/8 each).
    assert!(!w.bubbles().is_empty());
    w.step(&set(s, SHIELD, Off, 1.0));
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.part(SHIELD).power, 0.0);
    assert_eq!(ship.part(SHIELD).work, 0.0);
    assert!(
        w.bubbles().is_empty(),
        "a switched-off emitter projects no bubble"
    );
    assert_eq!(ship.pieces()[0].power.demand, 15.0);
    for p in [LASERS[1], LASERS[2]] {
        assert!(close(level(&w, s, p), 3.0 / 8.0, 1e-6));
    }
}

#[test]
fn weights_share_the_partial_tier_and_scale_fire_rate() {
    let c = content();
    let (mut w, s) = world_with(custom(&c, &gunship()));
    // Weight 2 on the first laser: k = 4 / (2·4 + 4 + 4) → levels 0.5, 0.25, 0.25.
    w.step(&set(s, LASERS[0], Low, 2.0));
    let lv: Vec<f32> = LASERS.iter().map(|&p| level(&w, s, p)).collect();
    assert_eq!(lv, vec![0.5, 0.25, 0.25]);
    let guns: Vec<f32> = w.ship(s).unwrap().pieces()[0]
        .guns
        .iter()
        .map(|g| g.output)
        .collect();
    assert_eq!(
        guns[0],
        2.0 * guns[1],
        "damage per second follows the level"
    );

    // Fire rate follows the level: 4 shots/s at full power → 2 and 1 shot/s here, over 10 s.
    let mut shots = [0u32; 3];
    for _ in 0..300 {
        w.step(&all_on(s));
        for e in w.events() {
            if let SimEvent::ShotFired { gun, .. } = e {
                shots[*gun as usize] += 1;
            }
        }
    }
    assert!((19..=21).contains(&shots[0]), "{shots:?}");
    assert!(
        (9..=11).contains(&shots[1]) && (9..=11).contains(&shots[2]),
        "{shots:?}"
    );
    assert!(
        w.ship(s).unwrap().parts().iter().all(|p| !p.overheated),
        "the test ship must stay cool"
    );
}

#[test]
fn a_holding_gun_draws_nothing() {
    let c = content();
    let (mut w, s) = world_with(custom(&c, &gunship()));
    // Fire at will holds the middle gun (per-gun mask): the other two share the low tier's 4 MW.
    let mut t = all_on(s);
    t.controls[0].1.guns = vec![true, false, true];
    w.step(&t);
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.part(LASERS[1]).work, 0.0);
    assert_eq!(ship.pieces()[0].power.demand, 14.0);
    assert_eq!(level(&w, s, LASERS[0]), 0.5);
    assert_eq!(level(&w, s, LASERS[2]), 0.5);
    // Idle, it shows the level its tier is served at (for a dimmed icon), but draws and fires nothing.
    assert_eq!(level(&w, s, LASERS[1]), 0.5);
    assert_eq!(ship.pieces()[0].guns[1].output, 0.0);
}

#[test]
fn minimum_level_switches_a_part_off_and_shares_its_power() {
    let c = content();
    // Two beams (9 MW, minimum 50%) and a pulse laser (4 MW) share the 9 MW left after the core.
    let d = custom(
        &c,
        &[
            ("beam_laser", 0, 0, 0, false, None),
            ("beam_laser", 1, 0, 0, false, None),
            ("pulse_laser", 2, 1, 0, false, None),
            ("logic_core", 0, 2, 0, false, None),
            ("fission_reactor", 1, 2, 0, false, None),
            ("armor_block", 2, 2, 0, false, Some("steel")),
        ],
    );
    assert_eq!(d.parts[0].power_min_level, 0.5);
    let (mut w, s) = world_with(d);
    w.step(&inputs(s, controls(0, 0.0, true, true)));
    // Shared evenly every part would run at 9/22 = 41%, below the beams' 50%. The beam with the higher part
    // index switches off first; the other two then run at 9/13 = 69%, and that beam stays on.
    let l = 9.0 / 13.0;
    assert!(close(level(&w, s, 0), l, 1e-6), "{}", level(&w, s, 0));
    assert_eq!(level(&w, s, 1), 0.0);
    assert!(close(level(&w, s, 2), l, 1e-6));
    let p = w.ship(s).unwrap().pieces()[0].power;
    assert!(
        close(p.served, 10.0, 1e-5),
        "the dropped share is used: {p:?}"
    );
    let firing: Vec<PartIdx> = w.beams().iter().map(|b| b.gun).collect();
    assert_eq!(firing, vec![0], "only the powered beam fires");

    // A shield below its minimum (25%) cannot hold its bubble; the charge stays for later.
    let (mut w, s) = world_with(custom(&c, &gunship()));
    w.step(&all_on(s));
    let full = w.ship(s).unwrap().part(SHIELD).shield;
    assert!(!w.bubbles().is_empty());
    for p in LASERS {
        w.step(&set(s, p, Low, 100.0));
    }
    w.step(&set(s, SHIELD, Low, 1.0));
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.part(SHIELD).power, 0.0);
    assert!(w.bubbles().is_empty());
    assert_eq!(ship.part(SHIELD).shield, full);
    // Its 3 MW went to the lasers: 7 MW over 12 MW of lasers.
    assert!(close(level(&w, s, LASERS[0]), 7.0 / 12.0, 1e-6));
    w.step(&set(s, SHIELD, High, 1.0));
    assert_eq!(w.bubbles().len(), 1, "back up once it is fed again");
}

#[test]
fn blueprint_override_front_shields_high_rear_low() {
    let c = content();
    // Two shields in front, two behind, a core and one reactor: 9 MW for 12 MW of shields.
    let parts: Vec<P> = vec![
        ("shield_emitter", 0, 0, 0, false, None),
        ("shield_emitter", 1, 0, 0, false, None),
        ("logic_core", 0, 1, 0, false, None),
        ("fission_reactor", 1, 1, 0, false, None),
        ("shield_emitter", 0, 2, 0, false, None),
        ("shield_emitter", 1, 2, 0, false, None),
    ];
    let (front, rear) = ([0, 1], [4, 5]);
    let plain = Arc::new(ShipDesign::compile(&c, &blueprint(&parts)).unwrap());
    let mut bp = blueprint(&parts);
    for i in front {
        bp.parts[i].power = Some(PowerOverride {
            tier: Some(High),
            weight: None,
        });
    }
    for i in rear {
        bp.parts[i].power = Some(PowerOverride {
            tier: Some(Low),
            weight: None,
        });
    }
    let tuned = Arc::new(ShipDesign::compile(&c, &bp).unwrap());
    assert_eq!(tuned.parts[4].power_setting.tier, Low);

    // Without overrides all four share the high tier: 9/12 each.
    let (mut w, s) = world_with(plain);
    w.step(&inputs(s, controls(0, 0.0, false, true)));
    for i in front.into_iter().chain(rear) {
        assert!(close(level(&w, s, i as PartIdx), 0.75, 1e-6));
    }
    // Front high, rear low: the front is fed in full, the rear shares the last 3 MW.
    let (mut w, s) = world_with(tuned);
    w.step(&inputs(s, controls(0, 0.0, false, true)));
    for i in front {
        assert_eq!(level(&w, s, i as PartIdx), 1.0);
    }
    for i in rear {
        assert!(close(level(&w, s, i as PartIdx), 0.5, 1e-6));
    }
    let ship = w.ship(s).unwrap();
    assert_eq!(
        ship.power_setting(4),
        Some(PowerSetting {
            tier: Low,
            weight: 1.0
        })
    );
}

#[test]
fn partial_power_scales_thrust_and_shield_recharge() {
    let c = content();
    let d = custom(&c, &gunship());
    // A: defaults (thruster and shield fully fed). B: thruster and shield moved into the low tier, where they
    // share 9 MW with the lasers: 9/17 each.
    let (mut a, sa) = world_with(d.clone());
    let (mut b, sb) = world_with(d);
    a.step(&all_on(sa));
    b.step(&set(sb, THRUSTER, Low, 1.0));
    b.step(&set(sb, SHIELD, Low, 1.0));
    a.step(&all_on(sa));
    let l = 9.0 / 17.0;
    assert_eq!(level(&a, sa, THRUSTER), 1.0);
    assert!(close(level(&b, sb, THRUSTER), l, 1e-6));
    assert!(close(level(&b, sb, SHIELD), l, 1e-6));

    // Thrust: the content thrust at full power, 9/17 of it here.
    let full = c.parts.get("thruster").unwrap().engine.as_ref().unwrap().thrust;
    let fa = a.ship(sa).unwrap().pieces()[0].wrench.force;
    let fb = b.ship(sb).unwrap().pieces()[0].wrench.force;
    assert!(close(fa.y, full, 1e-4), "{fa:?}");
    assert!(close(fb.y, full * l, 1e-4), "{fb:?}");
    let out = b.ship(sb).unwrap().pieces()[0].output[0];
    assert!(close(out, l, 1e-6), "the plume shows the partial output");

    // Shield recharge: empty both bubbles with one shot of their whole capacity, then let them recharge for a
    // second.
    let spec = c.parts.get("shield_emitter").unwrap().shield.as_ref().unwrap();
    let (capacity, regen) = (spec.capacity, spec.regen);
    let mut recharge = Vec::new();
    for (w, s) in [(&mut a, sa), (&mut b, sb)] {
        let centre = w.bubbles()[0].centre;
        w.spawn_projectile(
            ShipId(999),
            WeaponFamily::Energy,
            centre + Vec2::new(0.0, 6.0),
            Vec2::new(0.0, -60.0),
            capacity,
            1,
            1.0,
        );
        let mut depleted = false;
        for _ in 0..4 {
            w.step(&all_on(s));
            depleted |= w
                .events()
                .iter()
                .any(|e| matches!(e, SimEvent::ShieldHit { depleted: true, .. }));
        }
        assert!(depleted);
        let before = w.ship(s).unwrap().part(SHIELD).shield;
        for _ in 0..30 {
            w.step(&all_on(s));
        }
        recharge.push(w.ship(s).unwrap().part(SHIELD).shield - before);
    }
    assert!(
        close(recharge[0], regen, 1e-4),
        "regen {regen}/s at full power: {recharge:?}"
    );
    assert!(close(recharge[1] / recharge[0], l, 1e-4), "{recharge:?}");
}

#[test]
fn settings_are_checked_survive_rebuilds_and_are_hashed() {
    let c = content();
    let (mut w, s) = world_with(custom(&c, &gunship()));
    w.step(&all_on(s));
    let ship = w.ship_mut(s).unwrap();
    assert_eq!(ship.power_setting(REACTOR), None, "reactors draw nothing");
    assert_eq!(
        ship.power_setting(LASERS[0]),
        Some(PowerSetting {
            tier: Low,
            weight: 1.0
        })
    );
    assert_eq!(
        ship.power_setting(CORE).map(|p| p.tier),
        Some(Critical),
        "content default"
    );
    for weight in [0.0, -1.0, f32::NAN, f32::INFINITY, 101.0] {
        assert!(!ship.set_power_setting(LASERS[0], PowerSetting { tier: High, weight }));
    }
    assert!(!ship.set_power_setting(
        REACTOR,
        PowerSetting {
            tier: High,
            weight: 1.0
        }
    ));
    assert!(!ship.set_power_setting(
        99,
        PowerSetting {
            tier: High,
            weight: 1.0
        }
    ));
    assert_eq!(ship.power_setting(LASERS[0]).unwrap().tier, Low);

    // The setting is part of the state hash even when it changes nothing this tick (critical is fed in full).
    let mut other = w.clone();
    w.step(&set(s, CORE, Critical, 3.0));
    other.step(&all_on(s));
    assert_eq!(level(&w, s, CORE), level(&other, s, CORE));
    assert_ne!(w.state_hash(), other.state_hash());

    // Runtime settings survive a piece rebuild (a destroyed laser).
    w.step(&set(s, LASERS[1], Critical, 2.0));
    w.apply_damage(s, DamageTarget::Part(LASERS[2]), 100.0);
    w.step(&all_on(s));
    let ship = w.ship(s).unwrap();
    assert!(!ship.part(LASERS[2]).alive);
    assert_eq!(
        ship.power_setting(LASERS[1]),
        Some(PowerSetting {
            tier: Critical,
            weight: 2.0
        })
    );
    let rows: Vec<(PartIdx, PowerTier)> = ship.pieces()[0]
        .consumers()
        .iter()
        .map(|r| (r.part, r.tier))
        .collect();
    assert_eq!(
        rows,
        vec![
            (CORE, Critical), // weight 3 from above
            (LASERS[1], Critical),
            (SHIELD, High),
            (THRUSTER, Normal),
            (LASERS[0], Low)
        ],
        "allocation order: tier, weight descending, part"
    );
    ship.verify_caches().unwrap();
}
