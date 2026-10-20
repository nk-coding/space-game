//! The Lattice (GDR-0126): prisms relay a laser's beam further and harder, refractive crystal turns half of an energy
//! shot away along the mirrored line, and a wary site keeps its territory (it warns, then wakes).

mod common;

use common::*;
use data::Dir;
use sim::{Body, DamageTarget, Owner, ShipId, Sim, SimEvent, SiteState, TickInputs, Vec2, VoyagePlan};

fn fire(s: ShipId) -> TickInputs {
    inputs(s, controls(0, 0.0, true, true))
}

/// A gun at (0, 2) on a reactor and a core, with prisms in front of it (`prisms`: (x, y, rot)).
fn lance(gun: &'static str, gun_rot: u8, prisms: &[(i32, i32, u8)]) -> Vec<P<'static>> {
    let mut v = vec![(gun, 0, 2, gun_rot, false, None), ("fission_reactor", 0, 3, 0, false, None), ("logic_core", 0, 4, 0, false, None)];
    v.extend(prisms.iter().map(|&(x, y, r)| ("prism", x, y, r, false, None)));
    v
}

/// A 3×2 wall of `material` blocks (salvage: no core), its frame origin at `at`.
fn wall(material: &'static str) -> Vec<P<'static>> {
    let mut v = Vec::new();
    for y in 0..2 {
        for x in 0..3 {
            v.push(("armor_block", x, y, 0, false, Some(material)));
        }
    }
    v
}

#[test]
fn prisms_relay_a_lasers_beam_further_and_harder() {
    let c = content();
    let prism = c.parts.get("prism").unwrap().prism.clone().unwrap();
    let laser = c.parts.get("pulse_laser").unwrap().weapon.clone().unwrap();
    let mut w = Sim::new(1);
    // A spine of steel beside the prisms keeps the front one on the ship when the back one is shot out.
    let mut parts = lance("pulse_laser", 0, &[(0, 1, 0), (0, 0, 0)]);
    parts.extend((0..3).map(|y| ("armor_block", 1, y, 0, false, Some("steel"))));
    let d = custom(&c, &parts);
    let s = w.spawn_ship(d.clone(), Owner::PLAYER, Body::default());
    let g = w.ship(s).unwrap().pieces()[0].guns[0];
    assert_eq!(g.relay.prisms().len(), 2);
    assert!(close(g.range, laser.range * (1.0 + 2.0 * prism.range), 1e-4), "{}", g.range);
    assert!(close(g.dps, laser.damage * (1.0 + 2.0 * prism.damage), 1e-4), "{}", g.dps);
    assert!(g.clear);
    // The beam leaves from the front prism's top side (blueprint y 0), not the laser's (y 2).
    let top = w.ship(s).unwrap().pieces()[0].guns[0].muzzle;
    assert!(close(top.y - g.relay.from.y, 2.0, 1e-4), "{top:?} {:?}", g.relay.from);

    // A wall past the laser's own reach but within the relayed one: hit.
    let at = Vec2::new(-1.0, laser.range + 15.0);
    let t = w.spawn_ship(custom(&c, &wall("steel")), Owner::PLAYER, Body::at(at, 0.0));
    let mut hits = 0;
    for _ in 0..30 {
        w.step(&fire(s));
        hits += w.events().iter().filter(|e| matches!(e, SimEvent::Hit { ship, .. } if *ship == t)).count();
    }
    assert!(hits > 0, "the relayed beam reaches the wall");

    // The back prism shot out: the beam crosses the gap to the front one; one prism's worth.
    let back = find(&d, "prism", 0, 1);
    w.apply_damage(s, DamageTarget::Part(back), 100.0);
    w.step(&TickInputs::default());
    let g = w.ship(s).unwrap().pieces()[0].guns[0];
    assert_eq!(g.relay.prisms().len(), 1);
    assert!(close(g.range, laser.range * (1.0 + prism.range), 1e-4));
}

#[test]
fn a_prism_turns_a_beam_and_takes_only_lasers() {
    let c = content();
    // A laser facing right into a prism facing up: the beam leaves upwards from the prism.
    let d = custom(&c, &lance("pulse_laser", 1, &[(1, 2, 0)]));
    let mut w = Sim::new(1);
    let s = w.spawn_ship(d, Owner::PLAYER, Body::default());
    let g = w.ship(s).unwrap().pieces()[0].guns[0];
    assert_eq!((g.relay.prisms().len(), g.clear), (1, true));
    assert!(close(g.dir.y, 1.0, 1e-6), "{:?}", g.dir);
    // A prism facing back at the laser blocks it.
    let d = custom(&c, &lance("pulse_laser", 1, &[(1, 2, 3)]));
    let s = w.spawn_ship(d, Owner::PLAYER, Body::at(Vec2::new(40.0, 0.0), 0.0));
    let g = w.ship(s).unwrap().pieces()[0].guns[0];
    assert_eq!((g.relay.prisms().len(), g.clear), (0, false));
    // A cannon is no laser: a prism in its line of fire blocks it.
    let d = custom(&c, &lance("reverse_engineered_cannon", 0, &[(0, 1, 0)]));
    let s = w.spawn_ship(d, Owner::PLAYER, Body::at(Vec2::new(80.0, 0.0), 0.0));
    let g = w.ship(s).unwrap().pieces()[0].guns[0];
    assert_eq!((g.relay.prisms().len(), g.clear), (0, false));
    // The drydock's and validation's path agrees (every part in the way).
    let bp = blueprint(&lance("pulse_laser", 1, &[(1, 2, 0)]));
    let grid = c.ship_grid(&bp).unwrap();
    let path = c.beam_path(&bp.parts, &grid, 0);
    assert_eq!((path.prisms(), path.dir, path.clear()), (&[3u32][..], Dir::Up, true));
}

/// A pulse laser fired head on at a flat wall of refractive crystal: the wall takes half, the other half comes back
/// along the beam and hits the shooter's own gun. A cannon's shells do not glance off.
#[test]
fn refractive_crystal_throws_half_a_laser_back_at_the_gun() {
    let c = content();
    let refract = c.materials.get("refractive_crystal").unwrap().armor.clone().unwrap().refract;
    assert!(refract > 0.0);
    let mut w = Sim::new(1);
    let s = w.spawn_ship(custom(&c, &lance("pulse_laser", 0, &[])), Owner::PLAYER, Body::default());
    let t = w.spawn_ship(custom(&c, &wall("refractive_crystal")), Owner::PLAYER, Body::at(Vec2::new(0.0, 22.0), 0.0));
    let (mut on_wall, mut on_me, mut glances) = (0.0f32, 0.0f32, 0);
    for _ in 0..8 {
        w.step(&fire(s));
        for e in w.events() {
            match e {
                SimEvent::Hit { ship, damage, .. } if *ship == t => on_wall += damage,
                SimEvent::Hit { ship, damage, by, .. } if *ship == s => {
                    assert_eq!(*by, t, "the glanced shot is the wall's, not the shooter's");
                    on_me += damage;
                }
                SimEvent::Refracted { ship, hit, .. } => {
                    assert_eq!(*ship, t);
                    assert!(*hit);
                    glances += 1;
                }
                _ => {}
            }
        }
    }
    assert!(glances > 0);
    assert!(on_wall > 0.0 && close(on_me / on_wall, refract / (1.0 - refract), 1e-3), "{on_me} {on_wall}");

    // Shells do not glance off crystal.
    let mut w = Sim::new(1);
    let s = w.spawn_ship(custom(&c, &lance("reverse_engineered_cannon", 0, &[])), Owner::PLAYER, Body::default());
    w.spawn_ship(custom(&c, &wall("refractive_crystal")), Owner::PLAYER, Body::at(Vec2::new(0.0, 22.0), 0.0));
    for _ in 0..60 {
        w.step(&fire(s));
        assert!(!w.events().iter().any(|e| matches!(e, SimEvent::Refracted { .. })));
    }
}

/// A wary site with a territory (the Lattice's): asleep when the fleet is in sight, a warning at `warn`, awake at
/// `wake`.
#[test]
fn a_lattice_site_warns_then_wakes_inside_its_territory() {
    let c = content();
    let t = c.factions.get("lattice").unwrap().territory.unwrap();
    let enemy = Owner(1);
    let pos = Vec2::new(0.0, 0.0);
    let site = sim::SitePlan {
        encounter: Some("lance_watch".into()),
        goal: true,
        signal: false,
        core: None,
        pos,
        wake_radius: 1000.0,
        wary: true,
        territory: Some(t),
        ships: vec![sim::Spawn { design: design(&c, "lattice_needle"), body: Body::at(pos, 0.0), damage: 0.0, seed: 0 }],
        wrecks: Vec::new(),
        calls: None,
        called: false,
    };
    let me_at = Vec2::new(0.0, -(t.warn + 60.0));
    let plan = VoyagePlan {
        voyage: "test".into(),
        seed: 1,
        radius: 2500.0,
        entry: Body::at(me_at, 0.0),
        sites: vec![site],
        star: None,
        rocks: Vec::new(),
    };
    let mut w = Sim::new(3);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    w.start_voyage(plan, Owner::PLAYER, enemy);
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(me_at, 0.0));
    let put = |w: &mut Sim, y: f32| {
        let piece = w.ship(me).unwrap().control_piece().unwrap();
        w.teleport(me, piece, Body::at(Vec2::new(0.0, y), 0.0));
        w.step(&TickInputs::default());
        w.events().to_vec()
    };
    let ev = put(&mut w, -(t.warn + 40.0));
    assert!(!ev.iter().any(|e| matches!(e, SimEvent::SiteWarned { .. } | SimEvent::SiteWoke { .. })));
    assert_eq!(w.voyage().unwrap().sites()[0].state(), SiteState::Asleep, "within the wake radius, but wary");
    let ev = put(&mut w, -(t.warn - 20.0));
    assert!(ev.iter().any(|e| matches!(e, SimEvent::SiteWarned { site: 0 })));
    assert_eq!(w.voyage().unwrap().sites()[0].state(), SiteState::Asleep);
    let ev = put(&mut w, -(t.wake + 20.0));
    assert!(!ev.iter().any(|e| matches!(e, SimEvent::SiteWarned { .. })), "it warns once");
    assert_eq!(w.voyage().unwrap().sites()[0].state(), SiteState::Asleep);
    let ev = put(&mut w, -(t.wake - 20.0));
    assert!(ev.iter().any(|e| matches!(e, SimEvent::SiteWoke { site: 0 })));
}
