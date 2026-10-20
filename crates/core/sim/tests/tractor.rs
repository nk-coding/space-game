//! Tractor beams (GDR-0131): they pull salvage (loose items, wreck pieces) in, never vessels; heavy pieces come
//! slower; nothing without hold room; the `salvage` action stands off with one; the same every run.

mod common;

use std::sync::Arc;

use common::{P, custom, design};
use sim::{Body, DamageTarget, Owner, Pulled, Salvaged, ShipDesign, ShipId, Sim, SimConfig, SimEvent, TickInputs, Vec2};

/// A 2×2 puller: logic core, fission reactor, a cargo bay (4 units) and a tractor beam; `tractor` false puts an
/// armour block in its place.
fn puller(tractor: bool) -> Arc<ShipDesign> {
    let c = common::content();
    let fourth: P = if tractor { ("tractor_beam", 1, 1, 0, false, None) } else { ("armor_block", 1, 1, 0, false, Some("steel")) };
    custom(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("fission_reactor", 1, 0, 0, false, None),
            ("cargo_bay", 0, 1, 0, false, None),
            fourth,
        ],
    )
}

fn world() -> Sim {
    let mut config = SimConfig::default();
    config.salvage.intact_chance = 1.0;
    Sim::with_config(1, config)
}

/// Steps `ticks`, returning what was collected: `(collector, from)`.
fn run(w: &mut Sim, ticks: u32) -> Vec<(ShipId, Salvaged)> {
    let mut got = Vec::new();
    for _ in 0..ticks {
        w.step(&TickInputs::default());
        for e in w.events() {
            if let SimEvent::Collected { ship, from, .. } = e {
                got.push((*ship, *from));
            }
        }
    }
    got
}

/// A raider 50 m off whose core is shot out: its core's fragment drifts loose and its hull is a salvage piece.
fn wrecked_raider(w: &mut Sim, at: Vec2) -> ShipId {
    let c = common::content();
    let raider = w.spawn_ship(design(&c, "automaton_raider"), Owner(1), Body::at(at, 0.0));
    let core = common::find(w.ship(raider).unwrap().design(), "automaton_core", 1, 1);
    w.apply_damage(raider, DamageTarget::Part(core), 1000.0);
    w.resolve_breakage();
    assert_eq!(w.loose().len(), 1, "the core's fragment");
    raider
}

#[test]
fn a_tractor_pulls_a_fragment_and_a_wreck_in_and_the_hull_collects_them() {
    let mut w = world();
    let me = w.spawn_ship(puller(true), Owner::PLAYER, Body::default());
    let raider = wrecked_raider(&mut w, Vec2::new(0.0, 50.0));
    let got = run(&mut w, 30 * 30);
    assert!(got.iter().any(|g| *g == (me, Salvaged::Loose(sim::LooseId(0)))), "the fragment came in: {got:?}");
    assert!(
        got.iter().any(|g| matches!(g.1, Salvaged::Part { ship, .. } if ship == raider)),
        "and the wreck's parts: {got:?}"
    );
    let s = w.ship(me).unwrap();
    let free = s.cargo_capacity() - s.cargo().used();
    assert!(free < s.cargo_capacity(), "it took some");
    assert!(w.tractor_beams().is_empty(), "nothing left that fits the hold: the beams let go");

    // Without the beam the same ship, standing still, takes nothing 50 m off.
    let mut w = world();
    w.spawn_ship(puller(false), Owner::PLAYER, Body::default());
    wrecked_raider(&mut w, Vec2::new(0.0, 50.0));
    assert!(run(&mut w, 30 * 30).is_empty());
}

#[test]
fn a_tractor_never_pulls_a_vessel_and_reaches_only_so_far() {
    let c = common::content();
    let mut w = world();
    let me = w.spawn_ship(puller(true), Owner::PLAYER, Body::default());
    let enemy = w.spawn_ship(design(&c, "automaton_raider"), Owner(1), Body::at(Vec2::new(30.0, 0.0), 0.0));
    let friend = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::new(-30.0, 0.0), 0.0));
    let far = w.spawn_wreck(design(&c, "automaton_raider"), Owner(1), Body::at(Vec2::new(0.0, 150.0), 0.0), 0.0, 0);
    let near = w.spawn_wreck(design(&c, "automaton_raider"), Owner(1), Body::at(Vec2::new(0.0, -40.0), 0.0), 0.0, 0);
    for _ in 0..30 {
        w.step(&TickInputs::default());
        for b in w.tractor_beams() {
            assert_eq!(b.ship, me);
            assert!(matches!(b.target, Pulled::Piece(s, _) if s == near), "only the near wreck: {b:?}");
        }
    }
    assert!(!w.tractor_beams().is_empty(), "it holds the near wreck");
    let speed = |id| w.ship(id).unwrap().pieces()[0].body.linvel.length();
    assert!(speed(enemy) < 1e-3 && speed(friend) < 1e-3, "vessels stay put");
    assert!(speed(far) < 1e-3, "out of reach: {}", speed(far));
    assert!(speed(near) > 0.5, "the near wreck comes: {}", speed(near));
}

#[test]
fn heavy_pieces_come_slower() {
    let c = common::content();
    let mut w = world();
    w.spawn_ship(puller(true), Owner::PLAYER, Body::default());
    let light = w.spawn_wreck(design(&c, "automaton_raider"), Owner(1), Body::at(Vec2::new(0.0, 60.0), 0.0), 0.0, 0);
    let heavy = w.spawn_wreck(design(&c, "halvex_runner"), Owner(1), Body::at(Vec2::new(0.0, -60.0), 0.0), 0.0, 0);
    let mass = |w: &Sim, id| w.ship(id).unwrap().pieces().iter().map(|p| p.mass.mass).sum::<f32>();
    assert!(mass(&w, heavy) > 2.0 * mass(&w, light), "{} against {}", mass(&w, heavy), mass(&w, light));
    for _ in 0..30 {
        w.step(&TickInputs::default());
    }
    let speed = |id| w.ship(id).unwrap().pieces()[0].body.linvel.length();
    assert!(speed(light) > 1.5 * speed(heavy), "light {} heavy {}", speed(light), speed(heavy));
}

#[test]
fn the_salvage_action_stands_off_with_a_tractor_and_lets_the_beam_pull_the_same_every_run() {
    let go = || {
        let c = common::content();
        let mut w = world();
        w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
        let me = w.spawn_ship(design(&c, "sample_salvager"), Owner::PLAYER, Body::default());
        let wreck_at = Vec2::new(0.0, 160.0);
        w.spawn_wreck(design(&c, "automaton_raider"), Owner(1), Body::at(wreck_at, 0.0), 0.0, 0);
        let b = w.behaviors().id("salvager").unwrap();
        w.command(me, sim::ShipCommand::Behavior(Some(b)));
        let mut closest = f32::INFINITY;
        let mut got = 0;
        for _ in 0..60 * 30 {
            got += run(&mut w, 1).len();
            let s = w.ship(me).unwrap();
            let at = s.pieces()[0].com_world();
            closest = closest.min((at - wreck_at).length());
        }
        let s = w.ship(me).unwrap();
        (got, s.cargo().used(), closest, w.state_hash())
    };
    let (a, b) = (go(), go());
    assert!(a.0 > 0, "it stripped the wreck");
    let reach = common::content().parts.get("tractor_beam").unwrap().tractor.unwrap().range;
    assert!(a.2 > reach * 0.3, "it stopped short of where the wreck was ({} m) and let the beam pull", a.2);
    assert_eq!(a, b, "deterministic");
}
