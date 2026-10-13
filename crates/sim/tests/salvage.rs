//! Salvage (GDR-0063): holds, collecting order, full holds, a lost ship's cargo, fragments, wrecks spawned as
//! wrecks, and the salvage determinism scenario.

mod common;

use std::sync::Arc;

use common::{P, custom, design};
use sim::scenario::{self, Scenario, ScenarioKind};
use sim::{
    Body, DamageTarget, Item, Owner, Salvaged, ShipDesign, ShipId, Sim, SimConfig, SimEvent,
    TickInputs, Vec2,
};

const STEEL: Option<&str> = Some("steel");

/// A 2×2 collector: logic core, fission reactor and two cargo bays (8 units), or one bay and a block (4).
fn collector(bays: usize) -> Arc<ShipDesign> {
    let c = common::content();
    let second: P = if bays == 2 {
        ("cargo_bay", 1, 1, 0, false, None)
    } else {
        ("armor_block", 1, 1, 0, false, STEEL)
    };
    custom(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("fission_reactor", 1, 0, 0, false, None),
            ("cargo_bay", 0, 1, 0, false, None),
            second,
        ],
    )
}

/// What the collect passes of `ticks` steps took: `(collector, from, item, units)`, in order.
fn run(w: &mut Sim, ticks: u32) -> Vec<(ShipId, Salvaged, Item, u32)> {
    let mut got = Vec::new();
    for _ in 0..ticks {
        w.step(&TickInputs::default());
        for e in w.events() {
            assert!(
                !matches!(e, SimEvent::PartDestroyed { .. }),
                "nothing is destroyed here: {e:?}"
            );
            if let SimEvent::Collected {
                ship,
                from,
                item,
                units,
                ..
            } = e
            {
                got.push((*ship, *from, item.clone(), *units));
            }
        }
    }
    got
}

fn part(id: &str) -> Item {
    Item::Part {
        part: id.into(),
        material: None,
    }
}

#[test]
fn the_hold_is_the_alive_cargo_parts_of_the_controlling_piece() {
    let c = common::content();
    let mut w = Sim::new(1);
    let runner = w.spawn_ship(design(&c, "halvex_runner"), Owner::PLAYER, Body::default());
    let s = w.ship(runner).unwrap();
    assert_eq!(
        (s.cargo_capacity(), s.cargo().used()),
        (8, 0),
        "two cargo bays of 4"
    );
    let bay = common::find(s.design(), "cargo_bay", 4, 3);
    w.apply_damage(runner, DamageTarget::Part(bay), 1000.0);
    w.step(&TickInputs::default());
    assert_eq!(w.ship(runner).unwrap().cargo_capacity(), 4);
    // A raider has no hold; salvage has none.
    let raider = w.spawn_ship(
        design(&c, "automaton_raider"),
        Owner(1),
        Body::at(Vec2::new(80.0, 0.0), 0.0),
    );
    assert_eq!(w.ship(raider).unwrap().cargo_capacity(), 0);
    let wreck = w.spawn_wreck(
        design(&c, "sample_early"),
        Owner::PLAYER,
        Body::at(Vec2::new(-80.0, 0.0), 0.0),
        0.0,
    );
    assert_eq!(w.ship(wreck).unwrap().cargo_capacity(), 0);
}

#[test]
fn collecting_takes_the_best_part_first_with_ties_by_part_index() {
    let c = common::content();
    let mut w = Sim::new(1);
    let me = w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
    // gyro (4 per unit), steel block (1), fission reactor (4), gyro (4): taken 0, 2, 3, 1.
    let wreck_design = custom(
        &c,
        &[
            ("gyro", 0, 0, 0, false, None),
            ("armor_block", 1, 0, 0, false, STEEL),
            ("fission_reactor", 0, 1, 0, false, None),
            ("gyro", 1, 1, 0, false, None),
        ],
    );
    let wreck = w.spawn_wreck(
        wreck_design,
        Owner(1),
        Body::at(Vec2::new(4.0, 0.0), 0.0),
        0.0,
    );
    let got = run(&mut w, 60);
    let parts: Vec<u32> = got
        .iter()
        .map(|g| match g.1 {
            Salvaged::Part { ship, part, .. } => {
                assert_eq!((g.0, ship), (me, wreck));
                part
            }
            other => panic!("only parts here: {other:?}"),
        })
        .collect();
    assert_eq!(parts, [0, 2, 3, 1]);
    // One part per pass (every 10 ticks), the hold counts them, and the steel block carries its material.
    let s = w.ship(me).unwrap();
    assert_eq!(s.cargo().used(), 4);
    assert_eq!(s.cargo().count(&part("gyro")), 2);
    let steel = Item::Part {
        part: "armor_block".into(),
        material: Some("steel".into()),
    };
    assert_eq!(s.cargo().count(&steel), 1);
    // The wreck is gone part by part (its last piece went without a burst).
    assert!(w.ship(wreck).unwrap().pieces().is_empty());
}

#[test]
fn a_full_hold_takes_nothing_and_the_best_that_fits_goes_first() {
    let c = common::content();
    let mut w = Sim::new(1);
    let me = w.spawn_ship(collector(1), Owner::PLAYER, Body::default());
    // fusion reactor (2.5 per unit, 4 units), beam laser (3.5, 2 units), capacitor (2), steel block (1).
    let wreck_design = custom(
        &c,
        &[
            ("fusion_reactor", 0, 0, 0, false, None),
            ("beam_laser", 2, 0, 0, false, None),
            ("capacitor", 0, 2, 0, false, None),
            ("armor_block", 1, 2, 0, false, STEEL),
        ],
    );
    w.spawn_wreck(
        wreck_design,
        Owner(1),
        Body::at(Vec2::new(4.5, 0.0), 0.0),
        0.0,
    );
    let got = run(&mut w, 120);
    let items: Vec<(String, u32)> = got.iter().map(|g| (g.2.part().to_string(), g.3)).collect();
    assert_eq!(
        items,
        [
            ("beam_laser".to_string(), 2),
            ("capacitor".to_string(), 1),
            ("armor_block".to_string(), 1)
        ],
        "the fusion reactor never fits after the beam"
    );
    let s = w.ship(me).unwrap();
    assert_eq!((s.cargo().used(), s.cargo_capacity()), (4, 4));
}

#[test]
fn only_running_vessels_of_collecting_owners_collect() {
    let c = common::content();
    let wreck_at = Body::at(Vec2::new(4.0, 0.0), 0.0);
    // Another owner's vessel takes nothing, unless the config lets it collect.
    for collects in [false, true] {
        let mut config = SimConfig::default();
        config.salvage.collects[1] = collects;
        let mut w = Sim::with_config(1, config);
        w.spawn_ship(collector(2), Owner(1), Body::default());
        w.spawn_wreck(design(&c, "automaton_raider"), Owner(1), wreck_at, 0.0);
        assert_eq!(!run(&mut w, 30).is_empty(), collects);
    }
    // A dormant vessel (no mind capacity left) takes nothing.
    let mut config = SimConfig::default();
    config.mind_capacity[0] = 0;
    let mut w = Sim::with_config(1, config);
    let me = w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
    w.spawn_wreck(design(&c, "automaton_raider"), Owner(1), wreck_at, 0.0);
    assert!(w.ship(me).unwrap().is_dormant());
    assert!(run(&mut w, 30).is_empty());
    // Out of reach (5 m from the hull) takes nothing either.
    let mut w = Sim::new(1);
    w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
    w.spawn_wreck(
        design(&c, "automaton_raider"),
        Owner(1),
        Body::at(Vec2::new(9.0, 0.0), 0.0),
        0.0,
    );
    assert!(run(&mut w, 30).is_empty());
}

#[test]
fn a_lost_ship_keeps_its_cargo_and_its_wreck_gives_fragments_and_cargo_first() {
    let c = common::content();
    let mut w = Sim::new(1);
    let a = w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
    let wreck_design = custom(
        &c,
        &[
            ("gyro", 0, 0, 0, false, None),
            ("fission_reactor", 0, 1, 0, false, None),
        ],
    );
    w.spawn_wreck(
        wreck_design,
        Owner(1),
        Body::at(Vec2::new(3.5, 0.0), 0.0),
        0.0,
    );
    assert_eq!(run(&mut w, 20).len(), 2);
    let held = w.ship(a).unwrap().cargo().clone();
    assert_eq!(held.used(), 2);

    // A loses its logic core: no vessel any more, the hold stays with the wreck, the core leaves a fragment.
    let core = common::find(w.ship(a).unwrap().design(), "logic_core", 0, 0);
    w.apply_damage(a, DamageTarget::Part(core), 1000.0);
    w.step(&TickInputs::default());
    let s = w.ship(a).unwrap();
    assert!(!s.is_vessel());
    assert_eq!(s.cargo(), &held);
    assert_eq!(w.loose().len(), 1);
    assert_eq!(
        w.loose()[0].item,
        Item::Fragment {
            part: "logic_core".into()
        }
    );

    // B comes alongside: the fragment first, then A's cargo (best first, ties in item order), then A's parts.
    let b = w.spawn_ship(
        collector(2),
        Owner::PLAYER,
        Body::at(Vec2::new(-3.5, 0.0), 0.0),
    );
    let got = run(&mut w, 40);
    let kinds: Vec<(Salvaged, String)> = got
        .iter()
        .map(|g| {
            assert_eq!(g.0, b);
            (g.1, g.2.part().to_string())
        })
        .collect();
    assert!(
        matches!(kinds[0], (Salvaged::Loose(_), ref p) if p == "logic_core"),
        "{kinds:?}"
    );
    assert_eq!(
        kinds[1],
        (Salvaged::Cargo { ship: a }, "fission_reactor".into())
    );
    assert_eq!(kinds[2], (Salvaged::Cargo { ship: a }, "gyro".into()));
    assert!(
        matches!(kinds[3], (Salvaged::Part { ship, .. }, ref p) if ship == a && p == "fission_reactor"),
        "{kinds:?}"
    );
    assert!(w.ship(a).unwrap().cargo().is_empty());
    assert!(w.loose().is_empty());
}

#[test]
fn destroyed_cores_leave_fragments_that_drift_and_expire() {
    let c = common::content();
    let mut config = SimConfig::default();
    config.salvage.fragment_ticks = 30;
    let mut w = Sim::with_config(1, config);
    let raider = w.spawn_ship(
        design(&c, "automaton_raider"),
        Owner(1),
        Body {
            linvel: Vec2::new(3.0, 0.0),
            ..Body::at(Vec2::new(50.0, 0.0), 0.0)
        },
    );
    let core = common::find(w.ship(raider).unwrap().design(), "automaton_core", 1, 1);
    let core_at = {
        let s = w.ship(raider).unwrap();
        let p = &s.pieces()[0];
        p.body.to_world(sim::mass::to_piece(
            s.design().parts[core as usize].centroid,
            p.origin,
        ))
    };
    w.apply_damage(raider, DamageTarget::Part(core), 1000.0);
    w.resolve_breakage();
    assert_eq!(w.loose().len(), 1);
    let f = w.loose()[0].clone();
    assert_eq!(
        (f.item.clone(), f.units),
        (
            Item::Fragment {
                part: "automaton_core".into()
            },
            1
        )
    );
    assert!((f.pos - core_at).length() < 1e-4 && (f.vel - Vec2::new(3.0, 0.0)).length() < 1e-4);
    w.step(&TickInputs::default());
    let g = &w.loose()[0];
    assert_eq!(
        g.pos,
        f.pos + f.vel * sim::DT,
        "a fragment moves with its velocity"
    );
    for _ in 0..30 {
        w.step(&TickInputs::default());
    }
    assert!(w.loose().is_empty(), "uncollected fragments expire");
    // Other parts leave none.
    let thruster = common::find(w.ship(raider).unwrap().design(), "thruster", 1, 2);
    w.apply_damage(raider, DamageTarget::Part(thruster), 1000.0);
    w.step(&TickInputs::default());
    assert!(w.loose().is_empty());
}

#[test]
fn spawned_wrecks_are_burned_out_and_pre_broken() {
    let c = common::content();
    let mid = design(&c, "sample_mid");
    let n = mid.parts.len();
    let spawn = |seed: u64, damage: f32| {
        let mut w = Sim::new(seed);
        let id = w.spawn_wreck(Arc::clone(&mid), Owner(1), Body::default(), damage);
        (w, id)
    };
    let (w, id) = spawn(3, 0.4);
    let s = w.ship(id).unwrap();
    assert!(s.is_burned_out() && !s.is_vessel() && s.team().is_none());
    let alive = |w: &Sim| -> Vec<bool> {
        w.ship(id)
            .unwrap()
            .parts()
            .iter()
            .map(|p| p.alive)
            .collect()
    };
    assert_eq!(
        alive(&w).iter().filter(|&&a| a).count(),
        n - (0.4 * n as f32) as usize
    );
    assert_eq!(
        w.ships().len(),
        1,
        "no vessel forms from its cores, whatever pieces it broke into"
    );
    assert!(w.events().iter().all(|e| !matches!(
        e,
        SimEvent::PartDestroyed { .. } | SimEvent::VesselFormed { .. }
    )));
    assert!(
        w.loose().is_empty(),
        "parts removed at spawn leave no fragments"
    );
    assert!(s.pieces().iter().all(|p| p.salvage() && !p.active));
    // Rolled on the encounter stream: the same seed breaks the same parts.
    assert_eq!(alive(&spawn(3, 0.4).0), alive(&w));
    assert_ne!(alive(&spawn(4, 0.4).0), alive(&w));
    // Intact and fully broken wrecks.
    let (w, id) = spawn(3, 0.0);
    assert_eq!(w.ship(id).unwrap().pieces().len(), 1);
    let (w, id) = spawn(3, 1.0);
    assert!(w.ship(id).unwrap().pieces().is_empty());
    // A shot-out core of a wreck still leaves a fragment; it never becomes a vessel.
    let (mut w, id) = spawn(3, 0.0);
    let core = (0..n).find(|&i| mid.parts[i].logic).unwrap() as u32;
    w.apply_damage(id, DamageTarget::Part(core), 1000.0);
    w.step(&TickInputs::default());
    assert_eq!(w.loose().len(), 1);
    assert_eq!(w.ships().len(), 1);
}

#[test]
fn salvage_scenario_is_deterministic_and_collects() {
    let (a, sa) = scenario::run_kind(ScenarioKind::Salvage, 42, 900);
    let (b, sb) = scenario::run_kind(ScenarioKind::Salvage, 42, 900);
    if let Some(t) = a.iter().zip(&b).position(|(x, y)| x != y) {
        panic!("diverged at tick {}", t + 1);
    }
    assert_eq!(sa, sb);
    assert!(
        sa.collected > 10 && sa.collected_units >= sa.collected,
        "{sa:?}"
    );
    // Whether a lost collector's wreck gets stripped depends on how the fight goes; the rule itself is covered by
    // `a_lost_ship_keeps_its_cargo_and_its_wreck_gives_fragments_and_cargo_first`.
    assert!(sa.fragments > 0 && sa.collected_fragments > 0, "{sa:?}");
    let (c, _) = scenario::run_kind(ScenarioKind::Salvage, 43, 900);
    assert!(a.iter().zip(&c).all(|(x, y)| x != y));
    // Stripping keeps the incremental caches right.
    let content = common::content();
    let mut sc = Scenario::with_kind(&content, 7, ScenarioKind::Salvage).unwrap();
    for t in 0..600 {
        sc.step();
        if t % 30 == 0 {
            for ship in sc.sim.ships() {
                ship.verify_caches()
                    .unwrap_or_else(|e| panic!("tick {t}, {:?}: {e}", ship.id()));
            }
        }
    }
    assert!(sc.stats.collected > 0);
}
