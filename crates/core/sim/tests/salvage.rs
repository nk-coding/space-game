//! Salvage (GDR-0063): holds, collecting order, full holds, a lost ship's cargo, fragments, wrecks spawned as
//! wrecks, and the salvage determinism scenario.

mod common;

use std::sync::Arc;

use common::{P, custom, design};
use sim::voyage::scenario::{self, Scenario, ScenarioKind};
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

/// A sim in which every stripped part comes away intact (`intact_chance` 1, no draw): the collecting rules alone.
fn whole(seed: u64) -> Sim {
    let mut config = SimConfig::default();
    config.salvage.intact_chance = 1.0;
    Sim::with_config(seed, config)
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
    let mut w = whole(1);
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
        0,
    );
    assert_eq!(w.ship(wreck).unwrap().cargo_capacity(), 0);
}

#[test]
fn collecting_takes_the_best_part_first_with_ties_by_part_index() {
    let c = common::content();
    let mut w = whole(1);
    let me = w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
    // gyro (4 per unit), steel block (1), fission reactor (4), gyro (4): taken 0, 2, 3, 1. Stripping is no
    // destruction, so the reactor leaves without its blast (GDR-0084) and the gyro beside it is still there to take.
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
        0,
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
    let mut w = whole(1);
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
        0,
    );
    let got = run(&mut w, 120);
    let items: Vec<(String, u32)> = got.iter().map(|g| (g.2.part().unwrap().to_string(), g.3)).collect();
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
        config.salvage.intact_chance = 1.0;
        config.salvage.collects[1] = collects;
        let mut w = Sim::with_config(1, config);
        w.spawn_ship(collector(2), Owner(1), Body::default());
        w.spawn_wreck(design(&c, "automaton_raider"), Owner(1), wreck_at, 0.0, 0);
        assert_eq!(!run(&mut w, 30).is_empty(), collects);
    }
    // A dormant vessel (no mind capacity left) takes nothing.
    let mut config = SimConfig::default();
    config.mind_capacity[0] = 0;
    let mut w = Sim::with_config(1, config);
    let me = w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
    w.spawn_wreck(design(&c, "automaton_raider"), Owner(1), wreck_at, 0.0, 0);
    assert!(w.ship(me).unwrap().is_dormant());
    assert!(run(&mut w, 30).is_empty());
    // Out of reach (5 m from the hull) takes nothing either.
    let mut w = whole(1);
    w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
    w.spawn_wreck(
        design(&c, "automaton_raider"),
        Owner(1),
        Body::at(Vec2::new(9.0, 0.0), 0.0),
        0.0,
        0,
    );
    assert!(run(&mut w, 30).is_empty());
}

#[test]
fn a_lost_ship_keeps_its_cargo_and_its_wreck_gives_fragments_and_cargo_first() {
    let c = common::content();
    let mut w = whole(1);
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
        0,
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
            (g.1, g.2.part().unwrap().to_string())
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
    config.salvage.intact_chance = 1.0;
    config.salvage.fragment_ticks = Some(30);
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
        p.body.to_world(sim::ship::mass::to_piece(
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

/// By default (GDR-0079) a loose item lasts until the sim ends: still there long after the old 60 s, drifting on.
#[test]
fn loose_items_last_until_the_sim_ends_by_default() {
    let c = common::content();
    let mut w = whole(1);
    let raider = w.spawn_ship(
        design(&c, "automaton_raider"),
        Owner(1),
        Body {
            linvel: Vec2::new(1.0, 0.0),
            ..Body::at(Vec2::new(50.0, 0.0), 0.0)
        },
    );
    let core = common::find(w.ship(raider).unwrap().design(), "automaton_core", 1, 1);
    w.apply_damage(raider, DamageTarget::Part(core), 1000.0);
    w.step(&TickInputs::default());
    assert_eq!(w.loose().len(), 1);
    let f = w.loose()[0].clone();
    assert_eq!(f.expires, None);
    for _ in 0..2400 {
        w.step(&TickInputs::default());
    }
    assert_eq!(w.loose().len(), 1, "80 s later");
    assert!(w.loose()[0].pos.x > f.pos.x + 70.0);
}

#[test]
fn spawned_wrecks_are_burned_out_and_pre_broken() {
    let c = common::content();
    let mid = design(&c, "sample_mid");
    let n = mid.parts.len();
    // The wreck's own seed decides, not the world's: the world seed is the same everywhere below.
    let spawn = |seed: u64, damage: f32| {
        let mut w = whole(1);
        let id = w.spawn_wreck(Arc::clone(&mid), Owner(1), Body::default(), damage, seed);
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
    // The same seed breaks the same parts, also after other wrecks spawned first (no shared stream), and the
    // sim's encounter stream is left alone.
    assert_eq!(alive(&spawn(3, 0.4).0), alive(&w));
    assert_ne!(alive(&spawn(4, 0.4).0), alive(&w));
    let mut later = whole(1);
    for s in [7, 8] {
        later.spawn_wreck(Arc::clone(&mid), Owner(1), Body::default(), 0.4, s);
    }
    let again = later.spawn_wreck(Arc::clone(&mid), Owner(1), Body::default(), 0.4, 3);
    let alive_again: Vec<bool> = later.ship(again).unwrap().parts().iter().map(|p| p.alive).collect();
    assert_eq!(alive_again, alive(&w));
    assert_eq!(
        later.rng(sim::Stream::Encounter).clone(),
        whole(1).rng(sim::Stream::Encounter).clone()
    );
    // A NaN or infinite pose counts as 0 (13 Oct 2026 review), and pending damage on another ship stays pending.
    let mut w2 = whole(1);
    let other = w2.spawn_ship(design(&c, "automaton_raider"), Owner(1), Body::at(Vec2::new(60.0, 0.0), 0.0));
    w2.apply_damage(other, DamageTarget::Part(0), 1000.0);
    let nan = Body {
        pos: Vec2::new(f32::NAN, f32::INFINITY),
        angle: f32::NAN,
        linvel: Vec2::new(f32::INFINITY, 0.0),
        angvel: f32::NAN,
    };
    let bad = w2.spawn_wreck(Arc::clone(&mid), Owner(1), nan, 0.4, 3);
    let b = w2.ship(bad).unwrap().pieces()[0].body;
    assert!(b.pos.is_finite() && b.angle.is_finite() && b.linvel.is_finite() && b.angvel.is_finite());
    assert!(
        !w2.events().iter().any(|e| matches!(e, SimEvent::PartDestroyed { ship, .. } if *ship == other)),
        "the raider's damage resolves in the next step, not in the spawn"
    );
    w2.step(&TickInputs::default());
    w2.state_hash();
    assert!(w2.events().iter().any(|e| matches!(e, SimEvent::PartDestroyed { ship, .. } if *ship == other)));
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

/// A test pack for the content's salvage blocks: a gyro and an armour material without one, a thruster whose block
/// names a fragment and a logic core whose block names none.
const SALVAGE_PACK: [(&str, &str); 3] = [
    (
        "pack.ron",
        r#"(id: "test_salvage", name: "Test salvage", version: "1.0.0", schema_version: 1, depends: ["base"])"#,
    ),
    (
        "parts/test.ron",
        r#"[
    (id: "bare_gyro", name: "Bare gyro", summary: "No salvage block.", category: "propulsion", icon: "gyro",
     size: (1, 1), build_points: 9, mass: 2, hp: 1, power: (draw: 3), gyro: (torque: 1250)),
    (id: "rich_thruster", name: "Rich thruster", summary: "Leaves a fragment.", category: "propulsion",
     icon: "thruster", size: (1, 1), build_points: 3, mass: 2, hp: 1, power: (draw: 2),
     engine: (thrust: 200, rated_speed: 20, plume: (length: 3, damage: 1, heat: 1.5)),
     salvage: (family: "propulsion", data: 3, fragment: {"programmable_matter": 1})),
    (id: "quiet_core", name: "Quiet core", summary: "Leaves no fragment.", category: "logic", icon: "logic-core",
     size: (1, 1), build_points: 3, mass: 1, hp: 1, power: (draw: 1, tier: Critical),
     logic: (rule_slots: 6, control_capacity: 16), salvage: (family: "logic", data: 3)),
]"#,
    ),
    (
        "materials/test.ron",
        r#"[(id: "bare_steel", name: "Bare steel", summary: "No salvage block.", icon: "armor-steel",
     armor: (tone: "steel", mass_per_cell: 2, hp_per_cell: 1, build_point_factor: 9))]"#,
    ),
];

/// The content's salvage blocks decide (`docs/content-model.md` "Salvage"): a part without one (a hull part: its
/// material without one) is never collected, and only a part whose block names a `fragment` leaves one, logic
/// block or not.
#[test]
fn salvage_blocks_decide_what_is_collected_and_what_leaves_fragments() {
    let m = data::Pack::from_files(SALVAGE_PACK).unwrap();
    let c = data::Content::load(&[data::base_pack().unwrap(), m]).unwrap();
    let mut w = whole(1);
    let me = w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
    // The bare gyro and the bare-steel block are worth the most by build points, yet only the gyro is taken.
    let wreck_design = custom(
        &c,
        &[
            ("bare_gyro", 0, 0, 0, false, None),
            ("armor_block", 1, 0, 0, false, Some("bare_steel")),
            ("gyro", 0, 1, 0, false, None),
        ],
    );
    let spec = |i: usize| (wreck_design.parts[i].salvageable, wreck_design.parts[i].fragment);
    assert_eq!([spec(0), spec(1), spec(2)], [(false, false), (false, false), (true, false)]);
    let wreck = w.spawn_wreck(wreck_design, Owner(1), Body::at(Vec2::new(4.0, 0.0), 0.0), 0.0, 0);
    let got = run(&mut w, 60);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].2, part("gyro"));
    assert_eq!(w.ship(me).unwrap().cargo().used(), 1);
    let left = w.ship(wreck).unwrap();
    assert_eq!(left.parts().iter().filter(|p| p.alive).count(), 2);

    // Destroyed: the thruster's block names a fragment, the core's does not, the gyro has no block.
    let d = custom(
        &c,
        &[
            ("quiet_core", 0, 0, 0, false, None),
            ("rich_thruster", 1, 0, 0, false, None),
            ("bare_gyro", 0, 1, 0, false, None),
        ],
    );
    let mut w = whole(1);
    let s = w.spawn_wreck(d, Owner(1), Body::at(Vec2::new(40.0, 0.0), 0.0), 0.0, 0);
    for p in 0..3 {
        w.apply_damage(s, DamageTarget::Part(p), 1000.0);
    }
    w.resolve_breakage();
    let items: Vec<&Item> = w.loose().iter().map(|l| &l.item).collect();
    assert_eq!(
        items,
        [&Item::Fragment {
            part: "rich_thruster".into()
        }]
    );
}

/// GDR-0063: fly slowly over a wreck. Salvage moving faster than `SalvageTuning::max_speed` relative to the
/// collector (centre of mass to centre of mass) is passed by; at the same speed, or slower, it is taken. The limit
/// is in the hash, and a NaN one counts as its default.
#[test]
fn collecting_needs_a_slow_pass() {
    let c = common::content();
    let wreck = || {
        custom(
            &c,
            &[
                ("gyro", 0, 0, 0, false, None),
                ("gyro", 1, 0, 0, false, None),
            ],
        )
    };
    assert_eq!(SimConfig::default().salvage.max_speed, 8.0);
    let cases = [
        (0.0, 10.0, false),
        (10.0, 10.0, true),
        (0.0, 7.0, true),
        (-4.5, 4.5, false),
    ];
    for (mine, theirs, taken) in cases {
        let mut w = whole(1);
        let body = |x: f32, vy: f32| Body {
            linvel: Vec2::new(0.0, vy),
            ..Body::at(Vec2::new(x, 0.0), 0.0)
        };
        w.spawn_ship(collector(2), Owner::PLAYER, body(0.0, mine));
        w.spawn_wreck(wreck(), Owner(1), body(3.5, theirs), 0.0, 0);
        let got = run(&mut w, 1);
        assert_eq!(!got.is_empty(), taken, "{mine} vs {theirs} m/s: {got:?}");
    }
    let mut config = SimConfig::default();
    config.salvage.max_speed = 5.5;
    let (a, b) = (Sim::new(1), Sim::with_config(1, config));
    assert_ne!(a.state_hash(), b.state_hash());
    config.salvage.max_speed = f32::NAN;
    assert_eq!(*Sim::with_config(1, config).config(), SimConfig::default());
}

/// Two collectors at one piece in one pass (13 Oct 2026 review): the first strips a part, and the piece's
/// colliders are stale until the pass ends, so the second leaves it for the next pass.
#[test]
fn a_piece_stripped_in_a_pass_waits_for_the_next() {
    let c = common::content();
    let mut w = whole(1);
    let a = w.spawn_ship(collector(2), Owner::PLAYER, Body::at(Vec2::new(-4.0, 0.0), 0.0));
    let b = w.spawn_ship(collector(2), Owner::PLAYER, Body::at(Vec2::new(4.5, 0.0), 0.0));
    let wreck = custom(
        &c,
        &[
            ("gyro", 0, 0, 0, false, None),
            ("gyro", 1, 0, 0, false, None),
            ("gyro", 0, 1, 0, false, None),
            ("gyro", 1, 1, 0, false, None),
        ],
    );
    w.spawn_wreck(wreck, Owner(1), Body::default(), 0.0, 0);
    let first = run(&mut w, 1);
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0].0, a);
    // In ship order the first collector goes first every pass; the second takes what is left once it is done.
    assert_eq!(run(&mut w, 10).iter().map(|g| g.0).collect::<Vec<_>>(), [a]);
    w.remove_ship(a);
    assert_eq!(run(&mut w, 10).iter().map(|g| g.0).collect::<Vec<_>>(), [b]);
}

/// A full hold that loses a cargo bay drops its lowest-value items and says so (`SimEvent::CargoLost`).
#[test]
fn a_shrinking_full_hold_reports_what_it_drops() {
    let c = common::content();
    let mut w = whole(1);
    let me = w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
    let wreck = custom(
        &c,
        &[
            ("gyro", 0, 0, 0, false, None),
            ("gyro", 1, 0, 0, false, None),
            ("radiator", 2, 0, 0, false, None),
            ("radiator", 0, 1, 0, false, None),
            ("armor_block", 1, 1, 0, false, STEEL),
            ("armor_block", 2, 1, 0, false, STEEL),
            ("armor_block", 0, 2, 0, false, STEEL),
            ("armor_block", 1, 2, 0, false, STEEL),
            ("armor_block", 2, 2, 0, false, STEEL),
        ],
    );
    w.spawn_wreck(wreck, Owner(1), Body::at(Vec2::new(4.0, 0.0), 0.0), 0.0, 0);
    run(&mut w, 100);
    assert_eq!(w.ship(me).unwrap().cargo().used(), 8, "full");
    let bay = common::find(w.ship(me).unwrap().design(), "cargo_bay", 1, 1);
    w.apply_damage(me, DamageTarget::Part(bay), 1000.0);
    w.step(&TickInputs::default());
    let lost: Vec<(String, u32)> = w
        .events()
        .iter()
        .filter_map(|e| match e {
            SimEvent::CargoLost { ship, item, count } if *ship == me => {
                Some((item.part()?.to_string(), *count))
            }
            _ => None,
        })
        .collect();
    let s = w.ship(me).unwrap();
    assert_eq!((s.cargo_capacity(), s.cargo().used()), (4, 4));
    // The best went in first (two gyros and two radiators, then four steel blocks); the blocks go now.
    assert_eq!(lost, [("armor_block".to_string(), 4)]);
    assert_eq!(s.cargo().count(&part("gyro")), 2);
    assert_eq!(s.cargo().count(&part("radiator")), 2);
}

/// What a shrinking hold drops spills as loose items at the lost bay (GDR-0076): one per item, the same item with
/// its units and value, pushed apart at half to all of `spill_speed`; another collector scoops them up again (they
/// leave the hold once and come back once); the same run gives the same spill.
#[test]
fn a_shrinking_hold_spills_what_it_drops_and_it_can_be_collected_again() {
    let spill = || {
        let c = common::content();
        let mut w = whole(3);
        let me = w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
        let wreck = custom(
            &c,
            &[
                ("gyro", 0, 0, 0, false, None),
                ("gyro", 1, 0, 0, false, None),
                ("radiator", 2, 0, 0, false, None),
                ("radiator", 0, 1, 0, false, None),
                ("armor_block", 1, 1, 0, false, STEEL),
                ("armor_block", 2, 1, 0, false, STEEL),
                ("armor_block", 0, 2, 0, false, STEEL),
                ("armor_block", 1, 2, 0, false, STEEL),
                ("armor_block", 2, 2, 0, false, STEEL),
            ],
        );
        w.spawn_wreck(wreck, Owner(1), Body::at(Vec2::new(4.0, 0.0), 0.0), 0.0, 0);
        run(&mut w, 100);
        assert_eq!(w.ship(me).unwrap().cargo().used(), 8, "full");
        assert!(w.loose().is_empty());
        let bay = common::find(w.ship(me).unwrap().design(), "cargo_bay", 1, 1);
        let (bay_at, bay_vel) = w.ship(me).unwrap().part_world(bay).unwrap();
        w.apply_damage(me, DamageTarget::Part(bay), 1000.0);
        w.step(&TickInputs::default());
        let lost: Vec<(Item, u32)> = w
            .events()
            .iter()
            .filter_map(|e| match e {
                SimEvent::CargoLost { item, count, .. } => Some((item.clone(), *count)),
                _ => None,
            })
            .collect();
        (w, me, lost, bay_at, bay_vel)
    };
    let (mut w, me, lost, bay_at, bay_vel) = spill();
    let block = Item::Part {
        part: "armor_block".into(),
        material: STEEL.map(Into::into),
    };
    assert_eq!(lost, [(block.clone(), 4)]);
    assert_eq!(w.ship(me).unwrap().cargo().count(&block), 0);

    // Four loose blocks from the lost bay, each pushed its own way.
    let loose = w.loose().to_vec();
    assert_eq!(loose.len(), 4, "{loose:?}");
    let speed = SimConfig::default().salvage.spill_speed;
    for (n, l) in loose.iter().enumerate() {
        assert_eq!((&l.item, l.units), (&block, 1));
        assert!((l.prev_pos - bay_at).length() < 0.05, "{l:?} from {bay_at:?}");
        let push = (l.vel - bay_vel).length();
        assert!(push >= 0.5 * speed - 1e-4 && push < speed + 1e-4, "{push}");
        assert!(loose[..n].iter().all(|o| (o.vel - l.vel).length() > 1e-3), "spread");
    }
    let (again, _, _, _, _) = spill();
    assert_eq!(again.loose(), &loose[..], "the same run spills the same way");
    assert_eq!(again.state_hash(), w.state_hash());

    // A second collector in the first one's place scoops them before the wreck: each block once.
    w.remove_ship(me);
    let b = w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
    let got = run(&mut w, 40);
    let mut scooped: Vec<_> = got
        .iter()
        .take(4)
        .map(|g| {
            assert_eq!((g.0, &g.2), (b, &block));
            match g.1 {
                Salvaged::Loose(id) => id,
                ref other => panic!("{other:?}"),
            }
        })
        .collect();
    scooped.sort();
    let ids: Vec<_> = loose.iter().map(|l| l.id).collect();
    assert_eq!(scooped, ids);
    assert!(w.loose().is_empty());
    assert_eq!(w.ship(b).unwrap().cargo().count(&block), 4);
}

/// Most stripped parts break (GDR-0108): at an intact chance of 0 every part that is not hull comes away broken,
/// hull never does; at 1 everything stays intact; the default rolls on the debris stream, the same for the same
/// seed.
#[test]
fn stripped_parts_break_by_the_intact_chance() {
    let c = common::content();
    let wreck = || {
        custom(
            &c,
            &[
                ("gyro", 0, 0, 0, false, None),
                ("radiator", 1, 0, 0, false, None),
                ("armor_block", 0, 1, 0, false, STEEL),
                ("gyro", 1, 1, 0, false, None),
                ("radiator", 2, 1, 0, false, None),
                ("armor_block", 2, 0, 0, false, STEEL),
            ],
        )
    };
    let haul = |chance: f32, seed: u64| {
        let mut config = SimConfig::default();
        config.salvage.intact_chance = chance;
        let mut w = Sim::with_config(seed, config);
        w.spawn_ship(collector(2), Owner::PLAYER, Body::default());
        w.spawn_wreck(wreck(), Owner(1), Body::at(Vec2::new(4.0, 0.0), 0.0), 0.0, 0);
        run(&mut w, 90).into_iter().map(|g| g.2).collect::<Vec<Item>>()
    };
    let broken = |items: &[Item]| items.iter().filter(|i| matches!(i, Item::Broken { .. })).count();
    let none = haul(0.0, 1);
    assert_eq!(none.len(), 6);
    assert_eq!(broken(&none), 4, "{none:?}");
    assert!(none.iter().all(|i| !matches!(i, Item::Broken { part, .. } if part == "armor_block")));
    assert_eq!(broken(&haul(1.0, 1)), 0);
    let rolled = haul(SimConfig::default().salvage.intact_chance, 7);
    assert_eq!(rolled, haul(SimConfig::default().salvage.intact_chance, 7));
    assert_eq!(Item::Broken { part: "gyro".into(), material: None }.haul(), data::career::HaulItem::Broken { part: "gyro".into(), material: None });
}

/// A sim with the content's behaviors, a rim skiff (a hold of 4) at the origin and an automaton raider's wreck
/// at `wreck_at`.
fn salvage_world(wreck_at: Vec2) -> (Sim, ShipId) {
    let c = common::content();
    let mut w = whole(1);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::default());
    w.spawn_wreck(design(&c, "automaton_raider"), Owner(1), Body::at(wreck_at, 0.0), 0.0, 0);
    (w, me)
}

#[test]
fn wreck_in_range_measures_to_salvage_and_cargo_full_reads_the_hold() {
    let c = common::content();
    let (mut w, me) = salvage_world(Vec2::new(0.0, 100.0));
    let text = "when cargo_full -> hold\nwhen wreck_in_range(50m) -> hold\nwhen wreck_in_range(150m) -> hold\nalways -> hold";
    let b = w.behaviors_mut().add_text("probe", text).unwrap();
    // A ship without a hold: always full.
    let bare = w.spawn_ship(
        custom(&c, &[("logic_core", 0, 0, 0, false, None), ("fission_reactor", 1, 0, 0, false, None)]),
        Owner::PLAYER,
        Body::at(Vec2::new(-200.0, 0.0), 0.0),
    );
    w.command(me, sim::ShipCommand::Behavior(Some(b)));
    w.command(bare, sim::ShipCommand::Behavior(Some(b)));
    w.step(&TickInputs::default());
    let s = w.ship(me).unwrap().behavior().unwrap();
    assert_eq!(s.active_rule(), Some(2), "the wreck is about 100 m off: {:?}", s.rules());
    assert_eq!(w.ship(bare).unwrap().behavior().unwrap().active_rule(), Some(0), "no hold: full");
}

#[test]
fn the_salvager_strips_the_nearest_wreck_until_its_hold_is_full_the_same_every_run() {
    let go = || {
        let (mut w, me) = salvage_world(Vec2::new(60.0, 120.0));
        let b = w.behaviors().id("salvager").unwrap();
        w.command(me, sim::ShipCommand::Behavior(Some(b)));
        let got = run(&mut w, 60 * 30);
        let s = w.ship(me).unwrap();
        (got.len(), s.cargo().used(), s.cargo_capacity(), s.behavior().unwrap().active_rule(), w.state_hash())
    };
    let (a, b) = (go(), go());
    assert!(a.0 > 0, "flew there and stripped it");
    assert_eq!(a.1, a.2, "until the hold is full");
    assert_eq!(a.3, Some(3), "then the salvage rule steps aside for `escort`");
    assert_eq!(a, b, "deterministic");
}

#[test]
fn cargo_reads_the_holds_share_and_leader_near_the_leaders_distance() {
    // GDR-0129: `cargo` (no hold reads full) and `leader_near` (true without a leader).
    let c = common::content();
    let (mut w, me) = salvage_world(Vec2::new(0.0, 400.0));
    let text = "when cargo >= 50% -> hold\nwhen leader_near(50m) -> hold\nwhen leader_near(150m) -> hold\nalways -> hold";
    let b = w.behaviors_mut().add_text("probe", text).unwrap();
    let bare = w.spawn_ship(
        custom(&c, &[("logic_core", 0, 0, 0, false, None), ("fission_reactor", 1, 0, 0, false, None)]),
        Owner::PLAYER,
        Body::at(Vec2::new(-200.0, 0.0), 0.0),
    );
    let wing = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::new(100.0, 0.0), 0.0));
    let alone = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::new(0.0, -300.0), 0.0));
    w.command(wing, sim::ShipCommand::Leader(Some(me)));
    for s in [me, bare, wing, alone] {
        w.command(s, sim::ShipCommand::Behavior(Some(b)));
    }
    w.command(me, sim::ShipCommand::Leader(Some(alone)));
    w.step(&TickInputs::default());
    let rule = |s| w.ship(s).unwrap().behavior().unwrap().active_rule();
    assert_eq!(rule(bare), Some(0), "no hold: reads 100%");
    assert_eq!(rule(wing), Some(2), "an empty hold, the leader about 100 m off");
    assert_eq!(rule(alone), Some(1), "no leader: always near");
    assert_eq!(rule(me), Some(3), "its leader 300 m off");
}
