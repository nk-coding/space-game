//! Same seed + same inputs → identical hash sequence; different seeds → different runs.

mod common;

use sim::scenario::{self, Scenario};
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
    assert!(sa.overheated_part_ticks > 0, "{sa:?}");
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
        controls: vec![],
        damage: vec![sim::DamageCommand {
            ship,
            target: DamageTarget::Part(0),
            amount: 0.5,
        }],
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
                ship.verify_caches().unwrap_or_else(|e| panic!("tick {t}, {:?}: {e}", ship.id()));
            }
        }
    }
    assert!(sc.stats.splits > 0);
}
