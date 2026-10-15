//! Same seed + same inputs → identical hash sequence; different seeds → different runs.

mod common;

use sim::voyage::scenario::{self, Scenario, ScenarioKind};
use sim::{DamageTarget, TickInputs};

#[test]
fn same_seed_gives_identical_hash_sequence() {
    let (a, sa) = scenario::run(42, 600);
    let (b, sb) = scenario::run(42, 600);
    assert_eq!(a.len(), 600);
    if let Some(t) = a.iter().zip(&b).position(|(x, y)| x != y) {
        panic!("diverged at tick {}", t + 1);
    }
    assert_eq!(sa, sb);
    // The scenario exercises what it should.
    assert!(sa.parts_destroyed > 50, "{sa:?}");
    assert!(sa.splits > 5, "{sa:?}");
    assert!(
        sa.overheated_part_ticks > 0,
        "the heat-stress ship overheats: {sa:?}"
    );
    // Hashes change every tick (time, heat and motion all move).
    assert!(a.windows(2).all(|w| w[0] != w[1]));
}

#[test]
fn different_seeds_give_different_runs() {
    let (a, sa) = scenario::run(1, 300);
    let (b, sb) = scenario::run(2, 300);
    assert!(a.iter().zip(&b).all(|(x, y)| x != y));
    assert_ne!(sa, sb, "the RNG drives damage and controls");
}

#[test]
fn cloned_world_continues_identically() {
    let content = common::content();
    let mut sc = Scenario::new(&content, 7).unwrap();
    for _ in 0..200 {
        sc.step();
    }
    let mut a = sc.sim.clone();
    let mut b = sc.sim.clone();
    assert_eq!(a.state_hash(), b.state_hash());
    let ship = a.ships()[0].id();
    let inputs = TickInputs {
        damage: vec![sim::DamageCommand {
            ship,
            target: DamageTarget::Part(0),
            amount: 0.5,
        }],
        ..Default::default()
    };
    for _ in 0..100 {
        a.step(&inputs);
        b.step(&inputs);
        assert_eq!(a.state_hash(), b.state_hash());
    }
}

#[test]
fn hash_sees_the_smallest_change() {
    let content = common::content();
    let sc = Scenario::new(&content, 3).unwrap();
    let mut a = sc.sim.clone();
    let h0 = a.state_hash();
    // A tiny hit changes one part's hit points by a few ulps.
    let ship = a.ships()[0].id();
    let part = a.ships()[0].parts().iter().position(|p| p.alive).unwrap() as u32;
    a.apply_damage(ship, DamageTarget::Part(part), 1e-6);
    assert_ne!(a.state_hash(), h0);
}

#[cfg(debug_assertions)]
#[test]
fn debug_builds_hash_every_tick() {
    let content = common::content();
    let mut sc = Scenario::new(&content, 5).unwrap();
    assert_eq!(sc.sim.last_tick_hash(), None);
    sc.sim.step(&TickInputs::default());
    assert_eq!(sc.sim.last_tick_hash(), Some(sc.sim.state_hash()));
}

#[test]
fn incremental_caches_match_a_full_recompute() {
    let content = common::content();
    let mut sc = Scenario::new(&content, 11).unwrap();
    for t in 0..900 {
        sc.step();
        if t % 30 == 0 {
            for ship in sc.sim.ships() {
                ship.verify_caches()
                    .unwrap_or_else(|e| panic!("tick {t}, {:?}: {e}", ship.id()));
            }
        }
    }
    assert!(sc.stats.splits > 0);
}

#[test]
fn flight_scenario_is_deterministic_and_busy() {
    let (a, sa) = scenario::run_kind(ScenarioKind::Flight, 42, 900);
    let (b, sb) = scenario::run_kind(ScenarioKind::Flight, 42, 900);
    if let Some(t) = a.iter().zip(&b).position(|(x, y)| x != y) {
        panic!("diverged at tick {}", t + 1);
    }
    assert_eq!(sa, sb);
    assert!(sa.firing_ticks > 100, "{sa:?}");
    assert!(
        sa.shots > 50 && sa.hits > 20,
        "weapons fire and hit: {sa:?}"
    );
    assert!(sa.splits > 0 && sa.parts_destroyed > 20, "{sa:?}");
    let (c, _) = scenario::run_kind(ScenarioKind::Flight, 43, 900);
    assert!(a.iter().zip(&c).all(|(x, y)| x != y));
}

#[test]
fn assists_scenario_is_deterministic_and_the_assists_act() {
    let (a, sa) = scenario::run_kind(ScenarioKind::Assists, 42, 900);
    let (b, sb) = scenario::run_kind(ScenarioKind::Assists, 42, 900);
    assert_eq!(a, b);
    assert_eq!(sa, sb);
    // Ship-ticks with a station held, something dodged, a rule deciding each rule's channel.
    let mut sc = Scenario::with_kind(&data::base_content(), 42, ScenarioKind::Assists).unwrap();
    let (mut station, mut avoiding, mut decides) = (0, 0, [0; 6]);
    for _ in 0..900 {
        sc.step();
        for st in sc.sim.ships().iter().filter_map(|s| s.assists()) {
            station += st.station().is_some() as u32;
            avoiding += st.avoiding().is_some() as u32;
            for (i, d) in decides.iter_mut().enumerate() {
                *d += st.decides(i) as u32;
            }
        }
    }
    assert!(station > 100 && avoiding > 0, "station {station}, avoiding {avoiding}");
    assert!(decides.iter().all(|&d| d > 0), "every assist decides at times: {decides:?}");
    let (f, _) = scenario::run_kind(ScenarioKind::Flight, 42, 900);
    assert_ne!(a, f);
}

#[test]
fn load_test_is_deterministic_and_fights() {
    let (a, sa) = scenario::run_kind(ScenarioKind::Load(6), 42, 900);
    let (b, sb) = scenario::run_kind(ScenarioKind::Load(6), 42, 900);
    assert_eq!(a, b);
    assert_eq!(sa, sb);
    assert!(sa.shots > 20 && sa.hits > 10, "{sa:?}");
    let sim = scenario::load_test(1, 3);
    assert_eq!(sim.ships().len(), 1);
    let sim = scenario::load_test(5, 3);
    let runners = sim
        .ships()
        .iter()
        .filter(|s| s.design().id == "halvex_runner")
        .count();
    assert_eq!((runners, sim.ships().len()), (3, 5));
}
