//! Mining (GDR-0114, `sim::combat::mining`): a mining beam on a rock cuts its ore into the hold a unit at a time until
//! the rock is spent, a full hold lets the ore drift loose, other guns cut nothing, the `mine` behavior action flies
//! to the nearest rock with ore and cuts it, and the same run hashes the same.

mod common;

use std::sync::Arc;

use common::*;
use sim::voyage::rocks::{FieldPlan, Rock};
use sim::{Body, BehaviorLibrary, Item, Maneuver, Owner, ShipCommand, ShipDesign, Sim, SimEvent, TickInputs, Vec2, VoyagePlan};

/// The rim skiff with its right pulse laser a mining laser and its scanner a second cargo bay (a hold of 8).
fn miner(c: &data::Content) -> Arc<ShipDesign> {
    let mut bp = c.ships.get("rim_skiff").unwrap().clone();
    for p in &mut bp.parts {
        match (p.part.as_str(), p.x, p.y) {
            ("pulse_laser", 4, 0) => p.part = "mining_laser".into(),
            ("scanner", 5, 2) => p.part = "cargo_bay".into(),
            _ => {}
        }
    }
    Arc::new(ShipDesign::compile(c, &bp).unwrap())
}

/// A voyage with no sites and one iron rock of `radius` holding `ore` units at `at`.
fn plan(at: Vec2, radius: f32, ore: u32) -> VoyagePlan {
    let points: Vec<Vec2> = (0..8)
        .map(|k| {
            let a = k as f32 * core::f32::consts::TAU / 8.0;
            Vec2::new(libm::cosf(a), libm::sinf(a)) * radius
        })
        .collect();
    let rock = Rock { kind: "iron_rock".into(), ram: 0.5, material: "nickel_iron".into(), ore, cut: 0.0, pos: at, angle: 0.0, parts: vec![points.clone()], points, radius };
    VoyagePlan {
        voyage: "mining".into(),
        seed: 1,
        radius: 1000.0,
        entry: Body::at(Vec2::new(0.0, -1000.0), 0.0),
        sites: Vec::new(),
        star: None,
        rocks: vec![FieldPlan { centre: at, reach: radius, rocks: vec![rock] }],
    }
}

fn world(plan: VoyagePlan) -> Sim {
    let c = content();
    let mut w = Sim::new(1);
    w.set_behaviors(BehaviorLibrary::new(&c).unwrap());
    w.start_voyage(plan, Owner::PLAYER, Owner(1));
    w
}

fn ore() -> Item {
    Item::Ore { material: "nickel_iron".into() }
}

/// Steps `secs` seconds; the `Mined` events (stowed or not) and the tick of the first.
fn run(w: &mut Sim, secs: u64) -> (u32, u32, Option<u64>) {
    let (mut stowed, mut loose, mut first) = (0, 0, None);
    for _ in 0..secs * 30 {
        w.step(&TickInputs::default());
        for e in w.events() {
            if let SimEvent::Mined { stowed: s, .. } = e {
                if *s { stowed += 1 } else { loose += 1 }
                first.get_or_insert(w.tick());
            }
        }
    }
    (stowed, loose, first)
}

#[test]
fn a_mining_beam_cuts_the_rock_into_the_hold_until_it_is_spent() {
    let c = content();
    let cut = c.parts.get("mining_laser").unwrap().weapon.as_ref().unwrap().cut;
    let mut w = world(plan(Vec2::new(0.0, 30.0), 8.0, 5));
    let me = w.spawn_ship(miner(&c), Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
    w.command(me, ShipCommand::Maneuver { maneuver: Maneuver::Brake, fire: true });
    let (stowed, loose, first) = run(&mut w, 20);
    assert_eq!((stowed, loose), (5, 0));
    // The first unit after a unit's worth of cutting at full power (the beam works from the first tick).
    let first = first.unwrap() as f32 / 30.0;
    assert!(first >= 1.0 / cut - 0.1 && first <= 1.0 / cut + 1.0, "first unit after {first} s");
    let ship = w.ship(me).unwrap();
    assert_eq!(ship.cargo().count(&ore()), 5);
    let rock = &w.voyage().unwrap().rocks()[0];
    assert_eq!((rock.ore, rock.cut), (0, 0.0), "spent");
    // A spent rock gives nothing more.
    assert_eq!(run(&mut w, 5).0, 0);
}

#[test]
fn a_full_hold_lets_the_ore_drift_loose_and_plain_guns_cut_nothing() {
    let c = content();
    let mut w = world(plan(Vec2::new(0.0, 30.0), 8.0, 11));
    let me = w.spawn_ship(miner(&c), Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
    w.command(me, ShipCommand::Maneuver { maneuver: Maneuver::Brake, fire: true });
    let (stowed, loose, _) = run(&mut w, 30);
    assert_eq!((stowed, loose), (8, 3), "a hold of 8");
    assert_eq!(w.loose().iter().filter(|l| l.item == ore()).count(), 3);
    // The plain skiff's lasers stop on rock and cut nothing.
    let mut w = world(plan(Vec2::new(0.0, 30.0), 8.0, 11));
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
    w.command(me, ShipCommand::Maneuver { maneuver: Maneuver::Brake, fire: true });
    assert_eq!(run(&mut w, 10), (0, 0, None));
    assert_eq!(w.voyage().unwrap().rocks()[0].ore, 11);
}

#[test]
fn the_mine_action_flies_to_the_nearest_rock_and_cuts_it_the_same_every_run() {
    let c = content();
    let go = || {
        let mut w = world(plan(Vec2::new(60.0, 160.0), 10.0, 4));
        let me = w.spawn_ship(miner(&c), Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
        let b = w.behaviors().id("miner").unwrap();
        w.command(me, ShipCommand::Behavior(Some(b)));
        let got = run(&mut w, 60);
        (got, w.ship(me).unwrap().cargo().count(&ore()), w.state_hash())
    };
    let (a, b) = (go(), go());
    assert_eq!(a.0.0, 4, "flew there and cut every unit");
    assert_eq!(a.1, 4);
    assert_eq!(a, b, "deterministic");
}
