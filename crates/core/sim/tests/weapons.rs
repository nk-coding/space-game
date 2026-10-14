//! Weapons v1: lasers, projectiles (penetration, ricochet, recoil), shields, events.

mod common;

use std::sync::Arc;

use common::*;
use data::{Content, Pack, WeaponFamily};
use sim::{Body, ShipId, SimEvent, TickInputs, Vec2};

/// Base content plus a test mod with a railgun (penetration 3, big recoil).
fn content_with_railgun() -> Content {
    let files = [
        (
            "pack.ron",
            r#"(id: "test", name: "Test", version: "0.1.0", schema_version: 1, depends: ["base"])"#,
        ),
        (
            "parts/test.ron",
            r#"(id: "test_railgun", name: "Test railgun", summary: "A test gun.", category: "weapons",
                icon: "pulse-laser", size: (1, 1), build_points: 5, mass: 3, hp: 2,
                power: (draw: 2), heat: (output: 0.5, limit: 10),
                weapon: (family: Kinetic, damage: 30, range: 200,
                         delivery: Projectile(rate: 1, speed: 300, penetration: 3, recoil: 5)))"#,
        ),
    ];
    let test = Pack::from_files(files).unwrap();
    Content::load(&[data::base_pack().unwrap(), test]).unwrap()
}

/// A gun on top of a reactor and a core, one column: the muzzle sits on the centre line.
fn gunboat(gun: &'static str) -> Vec<P<'static>> {
    let h = if gun == "beam_laser" { 2 } else { 1 };
    vec![
        (gun, 0, 0, 0, false, None),
        ("fission_reactor", 0, h, 0, false, None),
        ("logic_core", 0, h + 1, 0, false, None),
    ]
}

/// A 5×4 wall of steel blocks (1 HP each).
fn wall() -> Vec<P<'static>> {
    let mut v = Vec::new();
    for y in 0..4 {
        for x in 0..5 {
            v.push(("armor_block", x, y, 0, false, Some("steel")));
        }
    }
    v
}

fn fire(s: ShipId) -> TickInputs {
    inputs(s, controls(0, 0.0, true, true))
}

/// HP of the wall block at (x, y).
fn hp(w: &sim::Sim, s: ShipId, d: &sim::ShipDesign, x: i32, y: i32) -> f32 {
    let i = find(d, "armor_block", x, y);
    let p = w.ship(s).unwrap().part(i);
    if p.alive { p.hp } else { 0.0 }
}

/// Shooter at the origin facing +y; wall 20 m ahead, offset so the shot runs down column 2.
fn range(c: &Content, gun: &'static str) -> (sim::Sim, ShipId, ShipId, Arc<sim::ShipDesign>) {
    let mut w = sim::Sim::new(1);
    let shooter = w.spawn_ship(
        custom(c, &gunboat(gun)),
        sim::Owner::PLAYER,
        Body::default(),
    );
    let d = custom(c, &wall());
    let target = w.spawn_ship(
        d.clone(),
        sim::Owner::PLAYER,
        Body::at(Vec2::new(0.4, 20.0), 0.0),
    );
    (w, shooter, target, d)
}

#[test]
fn beam_burns_the_first_block_only() {
    let c = content();
    let (mut w, s, t, d) = range(&c, "beam_laser");
    w.step(&fire(s));
    // The shot runs up column 2 and meets the wall's near row (blueprint row 3) first.
    assert!(hp(&w, t, &d, 2, 3) < 1.0);
    assert_eq!(hp(&w, t, &d, 2, 2), 1.0);
    assert_eq!(hp(&w, t, &d, 1, 3), 1.0);
    let beams = w.beams();
    assert_eq!(beams.len(), 1);
    assert!(
        beams[0].hit && (beams[0].to.y - 18.0).abs() < 1e-3,
        "{:?}",
        beams[0]
    );
    let hits = w
        .events()
        .iter()
        .filter(|e| matches!(e, SimEvent::Hit { .. }))
        .count();
    assert_eq!(hits, 1);
    // Damage per tick = dps × DT.
    let dps = w.ship(s).unwrap().pieces()[0].guns[0].dps;
    assert!((1.0 - hp(&w, t, &d, 2, 3) - dps * sim::DT).abs() < 1e-5);
    // Keep firing: the beam burns through block by block.
    for _ in 0..40 {
        w.step(&fire(s));
    }
    assert_eq!(hp(&w, t, &d, 2, 3), 0.0);
    assert!(hp(&w, t, &d, 2, 2) < 1.0);
}

#[test]
fn pulse_laser_fires_at_its_rate() {
    let c = content();
    let (mut w, s, t, d) = range(&c, "pulse_laser");
    let mut shots = 0;
    for _ in 0..30 {
        w.step(&fire(s));
        shots += w
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::ShotFired { .. }))
            .count();
    }
    // 4 pulses per second at full power.
    assert!((3..=5).contains(&shots), "{shots} pulses in 1 s");
    let per_pulse = w.ship(s).unwrap().pieces()[0].guns[0].shot_damage;
    assert!((1.0 - hp(&w, t, &d, 2, 3) - shots as f32 * per_pulse).abs() < 1e-4);
    assert!(w.beams().is_empty(), "pulses are not beams");
}

#[test]
fn railgun_pierces_three_blocks_and_recoils() {
    let c = content_with_railgun();
    let (mut w, s, t, d) = range(&c, "test_railgun");
    // The first shot comes within one period (gun cooldowns are staggered).
    let mut ticks = 0;
    while w.projectiles().is_empty() {
        w.step(&fire(s));
        ticks += 1;
        assert!(ticks <= 31, "no shot within a second");
    }
    assert_eq!(w.projectiles().len(), 1);
    assert!(
        w.events()
            .iter()
            .any(|e| matches!(e, SimEvent::ProjectileSpawned { .. }))
    );
    // Recoil: 5 kN·s on an 8 t ship pushes it back (−y) by 0.625 m/s.
    let v = w.ship(s).unwrap().pieces()[0].body.linvel;
    assert!((v.y + 5.0 / 8.0).abs() < 1e-3 && v.x.abs() < 1e-4, "{v:?}");
    let mut gone = None;
    for _ in 0..10 {
        w.step(&TickInputs::default());
        for e in w.events() {
            if let SimEvent::ProjectileGone { reason, .. } = e {
                gone = Some(*reason);
            }
        }
    }
    assert_eq!(gone, Some(sim::GoneReason::Hit));
    // 30 damage through 1 HP blocks: three blocks of column 2 go, the fourth stays.
    assert_eq!(hp(&w, t, &d, 2, 3), 0.0);
    assert_eq!(hp(&w, t, &d, 2, 2), 0.0);
    assert_eq!(hp(&w, t, &d, 2, 1), 0.0);
    assert_eq!(hp(&w, t, &d, 2, 0), 1.0);
    assert_eq!(hp(&w, t, &d, 1, 3), 1.0);
}

#[test]
fn cannon_stops_in_the_first_block() {
    let c = content();
    let mut w = sim::Sim::new(1);
    let raider = w.spawn_ship(
        design(&c, "automaton_raider"),
        sim::Owner(1),
        Body::default(),
    );
    let d = custom(&c, &wall());
    // The raider's cannon sits on its centre line (x = 0); aim it down wall column 2.
    let t = w.spawn_ship(
        d.clone(),
        sim::Owner::PLAYER,
        Body::at(Vec2::new(0.4, 20.0), 0.0),
    );
    // One shot, then cease fire and let it land.
    let mut ticks = 0;
    while !w
        .events()
        .iter()
        .any(|e| matches!(e, SimEvent::ShotFired { .. }))
    {
        w.step(&fire(raider));
        ticks += 1;
        assert!(ticks <= 31, "no shot within a second");
    }
    for _ in 0..12 {
        w.step(&inputs(raider, controls(1, 0.0, false, true)));
    }
    let shot = w.ship(raider).unwrap().pieces()[0].guns[0].shot_damage;
    assert!(
        (1.0 - hp(&w, t, &d, 2, 3) - shot).abs() < 1e-4,
        "one hit of {shot}"
    );
    assert_eq!(hp(&w, t, &d, 2, 2), 1.0);
}

/// A lone 1×3 slope (steel) at the origin: its face runs from (−1, 0.67) to (2, −0.33) in world metres.
fn slope_target(w: &mut sim::Sim, c: &Content) -> (ShipId, u32) {
    let d = custom(c, &[("slope_1x3", 0, 0, 0, false, Some("steel"))]);
    (w.spawn_ship(d, sim::Owner::PLAYER, Body::default()), 0)
}

#[test]
fn shallow_hits_ricochet_off_slopes() {
    let c = content();
    let mut w = sim::Sim::new(1);
    let (t, part) = slope_target(&mut w, &c);
    // Coming in from the right along y = 0: 72° from the face normal.
    let id = w.spawn_projectile(
        ShipId(999),
        WeaponFamily::Kinetic,
        Vec2::new(10.0, 0.0),
        Vec2::new(-100.0, 0.0),
        1.0,
        1,
        1.0,
    );
    let mut bounce = None;
    for _ in 0..6 {
        w.step(&TickInputs::default());
        for e in w.events() {
            if let SimEvent::Ricochet { id: rid, vel, .. } = e {
                assert_eq!(*rid, id);
                bounce = Some(*vel);
            }
        }
    }
    let vel = bounce.expect("ricochet");
    assert!(vel.y > 0.0 && vel.x < 0.0, "{vel:?}");
    assert!(
        (vel.length() - 60.0).abs() < 1e-2,
        "speed after the bounce: {}",
        vel.length()
    );
    let ship = w.ship(t).unwrap();
    assert_eq!(
        ship.part(part).hp,
        ship.design().parts[part as usize].hp,
        "a glancing shot does no damage"
    );

    // Straight down onto the face (18° from the normal): it hits.
    let mut w = sim::Sim::new(1);
    let (t, part) = slope_target(&mut w, &c);
    w.spawn_projectile(
        ShipId(999),
        WeaponFamily::Kinetic,
        Vec2::new(0.5, 10.0),
        Vec2::new(0.0, -100.0),
        0.25,
        1,
        1.0,
    );
    let mut ricochets = 0;
    for _ in 0..6 {
        w.step(&TickInputs::default());
        ricochets += w
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::Ricochet { .. }))
            .count();
    }
    assert_eq!(ricochets, 0);
    let ship = w.ship(t).unwrap();
    assert!(ship.part(part).hp < ship.design().parts[part as usize].hp);
}

/// Core, reactor and a shield emitter at the rear (towards a shooter behind it).
fn shielded() -> Vec<P<'static>> {
    vec![
        ("logic_core", 0, 0, 0, false, None),
        ("fission_reactor", 0, 1, 0, false, None),
        ("shield_emitter", 0, 2, 0, false, None),
    ]
}

#[test]
fn shields_absorb_energy_then_collapse() {
    let c = content();
    let mut w = sim::Sim::new(1);
    let s = w.spawn_ship(
        custom(&c, &gunboat("beam_laser")),
        sim::Owner::PLAYER,
        Body::default(),
    );
    let td = custom(&c, &shielded());
    // An enemy vessel (no friendly fire: a player vessel would let the beam pass).
    let t = w.spawn_ship(td.clone(), sim::Owner(1), Body::at(Vec2::new(0.0, 20.0), 0.0));
    let emitter = find(&td, "shield_emitter", 0, 2);
    let cap = w.ship(t).unwrap().part(emitter).shield;
    assert!(cap > 0.0);
    let mut hull_hits = 0;
    let mut shield_hits = 0;
    for _ in 0..30 {
        w.step(&fire(s));
        hull_hits += w
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::Hit { ship, .. } if *ship == t))
            .count();
        shield_hits += w
            .events()
            .iter()
            .filter(|e| matches!(e, SimEvent::ShieldHit { .. }))
            .count();
    }
    assert_eq!(
        hull_hits, 0,
        "energy is absorbed completely while the bubble holds"
    );
    assert_eq!(shield_hits, 30);
    assert!(w.bubbles().iter().any(|b| b.ship == t));
    assert!(w.ship(t).unwrap().part(emitter).shield < cap);
    // Keep burning: the bubble collapses, then the hull takes the beam.
    let mut collapsed = false;
    for _ in 0..600 {
        w.step(&fire(s));
        collapsed |= w
            .events()
            .iter()
            .any(|e| matches!(e, SimEvent::ShieldHit { depleted: true, .. }));
        if collapsed {
            hull_hits += w
                .events()
                .iter()
                .filter(|e| matches!(e, SimEvent::Hit { ship, .. } if *ship == t))
                .count();
        }
        if hull_hits > 5 {
            break;
        }
    }
    assert!(
        collapsed && hull_hits > 5,
        "collapsed {collapsed}, hull hits {hull_hits}"
    );
    assert!(w.ship(t).unwrap().part(emitter).shield_down);
}

#[test]
fn shields_let_half_of_a_kinetic_shot_through() {
    let c = content();
    let mut w = sim::Sim::new(1);
    let td = custom(&c, &shielded());
    let t = w.spawn_ship(td, sim::Owner::PLAYER, Body::at(Vec2::new(0.0, 20.0), 0.0));
    w.step(&TickInputs::default()); // the emitter powers up and the bubble appears
    w.spawn_projectile(
        ShipId(999),
        WeaponFamily::Kinetic,
        Vec2::new(0.0, 5.0),
        Vec2::new(0.0, 100.0),
        0.8,
        1,
        1.0,
    );
    let (mut absorbed, mut hull) = (0.0, 0.0);
    for _ in 0..10 {
        w.step(&TickInputs::default());
        for e in w.events() {
            match e {
                SimEvent::ShieldHit { absorbed: a, .. } => absorbed += a,
                SimEvent::Hit { ship, damage, .. } if *ship == t => hull += damage,
                _ => {}
            }
        }
    }
    assert!(
        (absorbed - 0.4f32).abs() < 1e-5 && (hull - 0.4f32).abs() < 1e-5,
        "absorbed {absorbed}, hull {hull}"
    );
}

#[test]
fn raiders_shoot_back() {
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
        Body::at(Vec2::new(60.0, 30.0), 2.0),
    );
    fight(&mut w, &c, raider);
    let mut hits_on_runner = 0;
    let mut shots = 0;
    for _ in 0..1800 {
        w.step(&TickInputs::default());
        for e in w.events() {
            match e {
                SimEvent::ShotFired { ship, .. } if *ship == raider => shots += 1,
                SimEvent::Hit { ship, by, .. } if *ship == runner && *by == raider => {
                    hits_on_runner += 1
                }
                _ => {}
            }
        }
    }
    assert!(shots > 10, "raider fired {shots} shots");
    assert!(hits_on_runner > 3, "{hits_on_runner} hits on the runner");
}

// ---------- no friendly fire (GDR-0048) ----------

/// A 5×2 steel screen with a logic core and a reactor in its back row, and a shield emitter on the line of fire
/// if `shield` (its bubble covers the shot's path): a vessel, not salvage.
fn screen(shield: bool) -> Vec<P<'static>> {
    let mut v: Vec<P<'static>> = (0..5)
        .map(|x| ("armor_block", x, 0, 0, false, Some("steel")))
        .collect();
    v.push(("armor_block", 0, 1, 0, false, Some("steel")));
    v.push(("fission_reactor", 1, 1, 0, false, None));
    v.push(if shield {
        ("shield_emitter", 2, 1, 0, false, None)
    } else {
        ("armor_block", 2, 1, 0, false, Some("steel"))
    });
    v.push(("logic_core", 3, 1, 0, false, None));
    v.push(("armor_block", 4, 1, 0, false, Some("steel")));
    v
}

/// A player shooter at the origin facing +y, a shielded player vessel 10 m ahead on its line of fire, and an
/// enemy vessel (no shield) 20 m ahead behind it.
fn screened(c: &Content, gun: &'static str) -> (sim::Sim, ShipId, ShipId, ShipId) {
    let mut w = sim::Sim::new(1);
    let s = w.spawn_ship(custom(c, &gunboat(gun)), sim::Owner::PLAYER, Body::default());
    let friend = w.spawn_ship(
        custom(c, &screen(true)),
        sim::Owner::PLAYER,
        Body::at(Vec2::new(0.4, 10.0), 0.0),
    );
    let enemy = w.spawn_ship(
        custom(c, &screen(false)),
        sim::Owner(1),
        Body::at(Vec2::new(0.4, 20.0), 0.0),
    );
    (w, s, friend, enemy)
}

/// Hull hits and shield hits on `ship` in the last step.
fn hits_on(w: &sim::Sim, ship: ShipId) -> (usize, usize) {
    let hull = w
        .events()
        .iter()
        .filter(|e| matches!(e, SimEvent::Hit { ship: s, .. } if *s == ship))
        .count();
    let shield = w
        .events()
        .iter()
        .filter(|e| matches!(e, SimEvent::ShieldHit { ship: s, .. } if *s == ship))
        .count();
    (hull, shield)
}

#[test]
fn shots_pass_through_friendly_vessels_and_hit_the_enemy_behind() {
    let c = content();
    for gun in ["beam_laser", "pulse_laser", "automaton_cannon"] {
        let (mut w, s, friend, enemy) = screened(&c, gun);
        let state = |w: &sim::Sim| -> Vec<(f32, f32)> {
            w.ship(friend)
                .unwrap()
                .parts()
                .iter()
                .map(|p| (p.hp, p.shield))
                .collect()
        };
        let intact = state(&w);
        let mut enemy_hits = 0;
        for _ in 0..60 {
            w.step(&fire(s));
            assert_eq!(hits_on(&w, friend), (0, 0), "{gun}: the friendly vessel is hit");
            enemy_hits += hits_on(&w, enemy).0;
        }
        assert!(w.bubbles().iter().any(|b| b.ship == friend), "{gun}: its bubble is up");
        assert_eq!(state(&w), intact, "{gun}: no damage, no shield drain");
        assert!(enemy_hits > 0, "{gun}: the enemy behind is hit");
    }
}

#[test]
fn salvage_of_the_own_owner_still_stops_shots() {
    let c = content();
    let (mut w, s, friend, enemy) = screened(&c, "pulse_laser");
    // The screen loses its core: a wreck, friendly to nobody.
    let d = w.ship(friend).unwrap().design().clone();
    w.apply_damage(friend, sim::DamageTarget::Part(find(&d, "logic_core", 3, 1)), 10.0);
    let mut wreck_hits = 0;
    for _ in 0..30 {
        w.step(&fire(s));
        wreck_hits += hits_on(&w, friend).0;
        assert_eq!(hits_on(&w, enemy).0, 0, "the wreck is in the way");
    }
    assert!(!w.ship(friend).unwrap().is_vessel() && wreck_hits > 0);
}

#[test]
fn a_projectile_keeps_the_owner_it_was_fired_with() {
    let c = content();
    let (mut w, s, friend, enemy) = screened(&c, "pulse_laser");
    w.step(&TickInputs::default()); // bubbles up
    w.spawn_projectile(
        s,
        WeaponFamily::Kinetic,
        Vec2::new(0.0, 2.0),
        Vec2::new(0.0, 60.0),
        0.8,
        1,
        1.0,
    );
    // The shooter becomes salvage while its shot is in flight: the shot still spares the player's vessels.
    let d = w.ship(s).unwrap().design().clone();
    w.apply_damage(s, sim::DamageTarget::Part(find(&d, "logic_core", 0, 2)), 10.0);
    assert_eq!(w.projectiles()[0].team, Some(sim::Owner::PLAYER));
    let mut enemy_hits = 0;
    for _ in 0..20 {
        w.step(&TickInputs::default());
        assert!(!w.ship(s).unwrap().is_vessel());
        assert_eq!(hits_on(&w, friend), (0, 0));
        enemy_hits += hits_on(&w, enemy).0;
    }
    assert_eq!(enemy_hits, 1);
}

/// A gun with an absurd rate (13 Oct 2026 review: a period below half an ulp of the cooldown never ended the
/// firing loop) fires at most `MAX_SHOTS_PER_TICK` shots per tick.
#[test]
fn a_gun_fires_at_most_a_few_shots_per_tick() {
    let c = content();
    let mut d = (*custom(&c, &gunboat("pulse_laser"))).clone();
    let gun = find(&d, "pulse_laser", 0, 0);
    d.parts[gun as usize].delivery = data::Delivery::Pulse { rate: 1e9 };
    let mut w = sim::Sim::new(1);
    let s = w.spawn_ship(Arc::new(d), sim::Owner::PLAYER, Body::default());
    for _ in 0..3 {
        w.step(&fire(s));
        let shots = w.events().iter().filter(|e| matches!(e, SimEvent::ShotFired { .. })).count();
        assert_eq!(shots as u32, sim::combat::weapons::tuning::MAX_SHOTS_PER_TICK);
    }
}

/// Two hits on one block in the same tick (13 Oct 2026 review): the first destroys it, but its collider stays
/// until breakage is resolved after the weapons phase. The second shot must go on to the part behind it, not
/// end on the stale collider without damage.
#[test]
fn a_second_hit_in_the_same_tick_reaches_the_part_behind() {
    let c = content();
    // Two projectiles in one tick (both fly before the guns).
    let (mut w, s, t, d) = range(&c, "pulse_laser");
    for _ in 0..2 {
        w.spawn_projectile(s, WeaponFamily::Kinetic, Vec2::new(0.0, 12.0), Vec2::new(0.0, 300.0), 1.5, 1, 1.0);
    }
    w.step(&TickInputs::default());
    assert_eq!(hp(&w, t, &d, 2, 3), 0.0, "the first projectile destroys the near block");
    assert_eq!(hp(&w, t, &d, 2, 2), 0.0, "the second one destroys the block behind it");
    assert_eq!(hp(&w, t, &d, 2, 1), 1.0, "and stops there (penetration 1)");
    // A projectile, then a pulse of a gun in the same tick.
    let (mut w, s, t, d) = range(&c, "pulse_laser");
    for _ in 0..10 {
        w.step(&inputs(s, controls(0, 0.0, false, true))); // the gun reloads to ready
    }
    w.spawn_projectile(s, WeaponFamily::Kinetic, Vec2::new(0.0, 12.0), Vec2::new(0.0, 300.0), 1.5, 1, 1.0);
    w.step(&fire(s));
    let per_pulse = w.ship(s).unwrap().pieces()[0].guns[0].shot_damage;
    assert_eq!(hp(&w, t, &d, 2, 3), 0.0);
    assert!((hp(&w, t, &d, 2, 2) - (1.0 - per_pulse)).abs() < 1e-5, "the pulse hits the block behind");
    let hits = w.events().iter().filter(|e| matches!(e, SimEvent::Hit { .. })).count();
    assert_eq!(hits, 2);
}

/// Fire within a tick is mutual (ADR-0037): two gunboats whose pulse lasers each die to one pulse both fire on
/// the same tick, and both lose their gun, whichever has the lower id. (Before, the lower id's pulse destroyed
/// the other gun before it fired.)
#[test]
fn guns_firing_in_one_tick_all_fire() {
    let c = content();
    let d = custom(&c, &gunboat("pulse_laser"));
    let gun = find(&d, "pulse_laser", 0, 0);
    let mut w = sim::Sim::new(1);
    let a = w.spawn_ship(d.clone(), sim::Owner(1), Body::default());
    let b = w.spawn_ship(d, sim::Owner(2), Body::at(Vec2::new(0.0, 20.0), core::f32::consts::PI));
    let both = |fire: bool| TickInputs {
        controls: [a, b].iter().map(|&s| (s, controls(0, 0.0, fire, true))).collect(),
        ..Default::default()
    };
    for _ in 0..10 {
        w.step(&both(false)); // the guns reload to ready
    }
    for s in [a, b] {
        w.apply_damage(s, sim::DamageTarget::Part(gun), 0.9);
    }
    w.step(&both(true));
    let fired = w.events().iter().filter(|e| matches!(e, SimEvent::ShotFired { .. })).count();
    assert_eq!(fired, 2, "both guns fire");
    for s in [a, b] {
        assert!(!w.ship(s).unwrap().part(gun).alive, "and both are shot off");
    }
}

/// Two pulses on one block in the same fire pass: the first destroys it, the second goes on to the block behind.
#[test]
fn a_second_pulse_of_the_pass_reaches_the_part_behind() {
    let c = content();
    let (mut w, s, t, d) = range(&c, "pulse_laser");
    // A second gunboat right behind the first (shots pass friendly vessels): both pulses run down column 2.
    let s2 = w.spawn_ship(custom(&c, &gunboat("pulse_laser")), sim::Owner::PLAYER, Body::at(Vec2::new(0.0, -5.0), 0.0));
    let both = |fire: bool| TickInputs {
        controls: [s, s2].iter().map(|&s| (s, controls(0, 0.0, fire, true))).collect(),
        ..Default::default()
    };
    for _ in 0..10 {
        w.step(&both(false));
    }
    let near = find(&d, "armor_block", 2, 3);
    w.apply_damage(t, sim::DamageTarget::Part(near), 0.9);
    w.step(&both(true));
    let per_pulse = w.ship(s).unwrap().pieces()[0].guns[0].shot_damage;
    assert_eq!(hp(&w, t, &d, 2, 3), 0.0);
    assert!((hp(&w, t, &d, 2, 2) - (1.0 - per_pulse)).abs() < 1e-5, "the second pulse hits the block behind");
}

/// A gun blocked by its own ship's parts never fires, though the shot would pass through its own hull (GDR-0004:
/// guns need a clear line of fire; GDR-0048: shots fly through the own vessel); it fires once the blocker is shot
/// off (GDR-0070: "the blocker can be shot off"). The layout of the evolved designs that put reactors and slopes in
/// front of their cannons (`docs/arena.md` "Evolution"): two automaton cores power the cannons, a reactor shutters
/// one and a slope the other. Since chain reactions (GDR-0084) only the slope is a shutter: the reactor shot off
/// takes its cannon with it, while the slope's cannon (out of the blast's reach) fires once the slope is gone.
#[test]
fn a_gun_behind_its_own_parts_fires_only_once_they_are_shot_off() {
    let c = content();
    let d = custom(
        &c,
        &[
            ("fission_reactor", 0, 0, 0, false, None),
            ("slope_1x1", 3, 0, 0, false, Some("steel")),
            ("automaton_cannon", 0, 1, 0, false, None),
            ("automaton_cannon", 3, 1, 0, false, None),
            ("automaton_core", 0, 2, 0, false, None),
            ("armor_block", 1, 2, 0, false, Some("steel")),
            ("armor_block", 2, 2, 0, false, Some("steel")),
            ("automaton_core", 3, 2, 0, false, None),
        ],
    );
    let (mut w, s) = world_with(d.clone());
    let behind_reactor = find(&d, "automaton_cannon", 0, 1);
    let behind_slope = find(&d, "automaton_cannon", 3, 1);
    let shots = |w: &sim::Sim| -> Vec<u32> {
        w.events()
            .iter()
            .filter_map(|e| match e {
                SimEvent::ShotFired { gun, .. } => Some(*gun),
                _ => None,
            })
            .collect()
    };
    // Two seconds with the trigger held: powered (the cores), never a shot, no draw.
    for _ in 0..60 {
        w.step(&fire(s));
        assert!(shots(&w).is_empty());
    }
    let ship = w.ship(s).unwrap();
    let piece = &ship.pieces()[0];
    assert!(piece.active && piece.power.supply > 0.0);
    assert!(piece.guns.iter().all(|g| !g.clear && g.output == 0.0));
    assert_eq!(piece.power.demand, 0.0, "a blocked gun asks for nothing");
    // The reactor is shot off: its blast destroys the cannon behind it; the one behind the slope still does not
    // fire.
    w.apply_damage(s, sim::DamageTarget::Cell((0, 0)), 10.0);
    let mut fired = Vec::new();
    for _ in 0..30 {
        w.step(&fire(s));
        fired.extend(shots(&w));
    }
    assert!(!w.ship(s).unwrap().part(behind_reactor).alive);
    assert!(fired.is_empty(), "{fired:?}");
    // Then the slope: its cannon fires.
    w.apply_damage(s, sim::DamageTarget::Cell((3, 0)), 10.0);
    let mut fired = Vec::new();
    for _ in 0..30 {
        w.step(&fire(s));
        fired.extend(shots(&w));
    }
    assert!(!fired.is_empty() && fired.iter().all(|&g| g == behind_slope), "{fired:?}");
}
