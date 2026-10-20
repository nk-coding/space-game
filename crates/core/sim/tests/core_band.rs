//! The core band (GDR-0130): radiation heats every part open to space everywhere in a core cluster, so a ship without
//! radiators overheats where a cooled one copes; the Archive's warden switches its tactics as its sections break off;
//! the `archive` scenario is deterministic.

mod common;

use std::sync::Arc;

use common::*;
use sim::{Body, DamageCommand, DamageTarget, Owner, ShipCommand, ShipControls, ShipDesign, Sim, TickInputs, Vec2, VoyagePlan};

/// A world whose voyage radiates `r` everywhere and holds nothing else.
fn radiant_world(r: f32) -> Sim {
    let mut w = Sim::new(1);
    w.start_voyage(
        VoyagePlan {
            voyage: "radiant".into(),
            seed: 1,
            radius: 3000.0,
            entry: Body::at(Vec2::new(0.0, -3000.0), 0.0),
            sites: Vec::new(),
            star: None,
            radiation: r,
            rocks: Vec::new(),
        },
        Owner::PLAYER,
        Owner(1),
    );
    w
}

/// Seconds until a part of `design` first overheats in a world radiating `r`, flown as in a running fight (its guns
/// firing three seconds of every five, its engines at half throttle, its shields up), within `seconds`.
fn overheats_after(design: &Arc<ShipDesign>, r: f32, seconds: u64) -> Option<u64> {
    let mut w = radiant_world(r);
    let s = w.spawn_ship(Arc::clone(design), Owner::PLAYER, Body::default());
    for t in 0..seconds * 30 {
        let fire = (t / 30) % 5 < 3;
        let c = ShipControls { throttle: vec![0.5; design.engines.len()], fire, guns: Vec::new(), shields: true };
        w.step(&TickInputs { controls: vec![(s, c)], ..Default::default() });
        let ship = w.ship(s).unwrap();
        if (0..design.parts.len() as u32).any(|i| ship.part(i).overheated) {
            return Some(t / 30);
        }
    }
    None
}

/// The Halvex Runner with its radiators swapped for steel blocks.
fn bare_runner(c: &data::Content) -> Arc<ShipDesign> {
    let mut bp = c.ships.get("halvex_runner").unwrap().clone();
    for p in bp.parts.iter_mut().filter(|p| p.part == "radiator") {
        p.part = "armor_block".into();
        p.material = Some("steel".into());
    }
    Arc::new(ShipDesign::compile(c, &bp).unwrap())
}

#[test]
fn radiation_overheats_a_ship_without_radiators_and_a_cooled_one_copes() {
    let c = content();
    let spec = c.cluster_types.get("archive_heart").unwrap().radiation.clone().unwrap();
    let (edge, centre) = (spec.edge, spec.centre);
    let (runner, bare) = (design(&c, "halvex_runner"), bare_runner(&c));
    // Without radiation nothing overheats in five minutes of fighting.
    assert_eq!(overheats_after(&bare, 0.0, 300), None);
    // At the galaxy's centre the bare Runner overheats within a minute; with its four radiators it copes, and so do
    // the core's own sentries.
    assert!(overheats_after(&bare, centre, 60).is_some(), "the bare Runner copes at {centre}");
    assert_eq!(overheats_after(&runner, centre, 300), None, "the Runner overheats at {centre}");
    assert_eq!(overheats_after(&design(&c, "precursor_sentry"), centre, 300), None);
    // On the core's outer edge the bare Runner lasts a few minutes.
    let t = overheats_after(&bare, edge, 300);
    assert!(t.is_some_and(|t| t >= 60), "the bare Runner at {edge}: {t:?}");
}

/// The warden's behavior (`archive_warden`) reads its hull share, which counts the sections shot off: whole it holds off
/// (rule 2 or 3), with its fore section shot off it closes in (rule 1), down to about half it charges (rule 0). The
/// section shot off is a vessel of its own, whole by its own count, and starts over at range.
#[test]
fn the_warden_switches_its_tactics_as_its_sections_break_off() {
    let c = content();
    let warden = design(&c, "archive_warden");
    let mut w = Sim::new(1);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let me = w.spawn_ship(warden.clone(), Owner(1), Body::default());
    let foe = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::new(0.0, 90.0), 0.0));
    w.command(foe, ShipCommand::Focus);
    let b = w.behaviors().default_for(&warden);
    assert!(b.is_some());
    w.command(me, ShipCommand::Behavior(b));
    let rule = |w: &Sim, s| w.ship(s).and_then(|s| s.behavior()).and_then(|b| b.active_rule());
    let run = |w: &mut Sim, ticks: u32, damage: Vec<DamageCommand>| {
        w.step(&TickInputs { damage, ..Default::default() });
        for _ in 1..ticks {
            w.step(&TickInputs::default());
        }
    };
    run(&mut w, 30, Vec::new());
    assert!(matches!(rule(&w, me), Some(2 | 3)), "whole: {:?}", rule(&w, me));
    // The fore section's bridge shot through: the fore section is a vessel of its own, the heart closes in.
    let ships = w.ships().len();
    run(&mut w, 30, vec![DamageCommand { ship: me, target: DamageTarget::Cell((7, 4)), amount: 50.0 }]);
    assert_eq!(w.ships().len(), ships + 1, "the fore section split off");
    let fore = w.ships().last().unwrap().id();
    assert_eq!(rule(&w, me), Some(1), "a section lost");
    assert!(matches!(rule(&w, fore), Some(2 | 3)), "the section on its own: {:?}", rule(&w, fore));
    // Both wings shot off and the heart's plating worn down: it charges.
    let mut hits = vec![
        DamageCommand { ship: me, target: DamageTarget::Cell((2, 8)), amount: 50.0 },
        DamageCommand { ship: me, target: DamageTarget::Cell((12, 8)), amount: 50.0 },
    ];
    for cell in [(6, 6), (8, 6), (4, 7), (5, 7), (9, 7), (10, 7), (8, 8), (9, 8), (10, 8), (5, 10), (8, 10), (9, 10)] {
        hits.push(DamageCommand { ship: me, target: DamageTarget::Cell(cell), amount: 2.2 });
    }
    run(&mut w, 30, hits);
    assert_eq!(rule(&w, me), Some(0), "down to about half");
}

#[test]
fn archive_scenario_is_deterministic_and_the_warden_switches() {
    use sim::ScenarioKind;
    use sim::voyage::scenario;
    let (a, sa) = scenario::run_kind(ScenarioKind::Archive, 42, 900);
    let (b, sb) = scenario::run_kind(ScenarioKind::Archive, 42, 900);
    if let Some(t) = a.iter().zip(&b).position(|(x, y)| x != y) {
        panic!("diverged at tick {}", t + 1);
    }
    assert_eq!(sa, sb);
    // The site woke, the warden broke apart and ran more than one phase's rules, shots flew.
    assert!(sa.sites_woken == 1 && sa.split_vessels > 0 && sa.shots > 0, "{sa:?}");
    assert!(sa.warden_rules & 0b0010 != 0 && sa.warden_rules & 0b1100 != 0, "{sa:?}");
}
