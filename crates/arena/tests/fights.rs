//! Fights: determinism, ending, scoring, the spec's text form and errors for bad specs.

use arena::{
    ArenaError, BehaviorRef, DesignRef, Entry, Fight, FightSpec, Formation, Outcome, Summary,
    arena_score, run_fight, run_seeds,
};

fn runner_vs_raiders(seed: u64) -> FightSpec {
    FightSpec::new(
        seed,
        vec![Entry::ship("halvex_runner")],
        vec![Entry {
            count: 3,
            ..Entry::ship("automaton_raider")
        }],
    )
}

#[test]
fn a_fight_is_deterministic() {
    let content = data::base_content();
    let spec = runner_vs_raiders(3);
    let a = run_fight(&content, &spec).unwrap();
    let b = run_fight(&content, &spec).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.state_hash, b.state_hash);
    let other = run_fight(&content, &runner_vs_raiders(4)).unwrap();
    assert_ne!(a.state_hash, other.state_hash, "seeds move the spawns");
}

#[test]
fn stepping_in_slices_gives_the_same_result() {
    let content = data::base_content();
    let spec = runner_vs_raiders(5);
    let whole = run_fight(&content, &spec).unwrap();
    let mut fight = Fight::new(&content, &spec).unwrap();
    let mut sliced = None;
    let mut calls = 0;
    while sliced.is_none() {
        sliced = fight.step(7);
        calls += 1;
    }
    assert_eq!(sliced.unwrap(), whole);
    assert_eq!(calls, whole.ticks.div_ceil(7));
    assert_eq!(fight.step(100), Some(whole), "an ended fight stays ended");
    assert_eq!(fight.sim().tick(), whole.ticks);
}

#[test]
fn odd_seeds_spawn_side_one_first() {
    let content = data::base_content();
    for seed in [6, 7] {
        let fight = Fight::new(&content, &runner_vs_raiders(seed)).unwrap();
        let ships = fight.sim().ships();
        assert_eq!(ships.len(), 4);
        let first = (seed % 2) as usize;
        assert_eq!(ships[0].owner(), arena::OWNERS[first]);
        assert_eq!(ships[3].owner(), arena::OWNERS[1 - first]);
        // Side 0 faces +y from the origin, side 1 faces it from `start_distance` ahead.
        for s in ships {
            let y = s.pieces()[0].body.pos.y;
            if s.owner() == arena::OWNERS[0] {
                assert!(y.abs() <= arena::JITTER, "{y}");
            } else {
                assert!((y - 100.0).abs() <= arena::JITTER, "{y}");
            }
        }
    }
}

#[test]
fn the_starter_against_one_raider_ends() {
    let content = data::base_content();
    let spec = FightSpec::new(
        1,
        vec![Entry::ship("rim_skiff")],
        vec![Entry::ship("automaton_raider")],
    );
    let r = run_fight(&content, &spec).unwrap();
    assert!(r.ticks <= spec.limit_ticks());
    for (k, s) in r.sides.iter().enumerate() {
        assert!(s.build_points > 0.0, "side {k} starts with build points");
        assert!((0.0..=s.build_points).contains(&s.build_points_lost));
        assert!((0.0..=1.0).contains(&s.score));
    }
    assert!(
        r.sides[0].damage_dealt + r.sides[1].damage_dealt > 0.0,
        "they fight: {r:?}"
    );
    match r.outcome {
        Outcome::Won(w) => {
            assert_eq!(r.sides[w].score, 1.0);
            assert_eq!(r.sides[1 - w].score, 0.0);
            assert!(r.sides[w].vessels_left > 0);
        }
        Outcome::BothOut | Outcome::TimeLimit => {
            let sum = r.sides[0].score + r.sides[1].score;
            assert!((sum - 1.0).abs() < 1e-6, "undecided scores add up to 1: {r:?}");
        }
    }
}

#[test]
fn a_short_time_limit_ends_the_fight_undecided() {
    let content = data::base_content();
    let mut spec = runner_vs_raiders(1);
    spec.start_distance = 1000.0;
    spec.time_limit = 2.0;
    let r = run_fight(&content, &spec).unwrap();
    assert_eq!(r.outcome, Outcome::TimeLimit);
    assert_eq!(r.ticks, 60);
    assert_eq!(r.winner(), None);
    assert_eq!([r.sides[0].score, r.sides[1].score], [0.5, 0.5]);
    assert_eq!(r.sides[1].vessels_left, 3);
}

#[test]
fn scoring_edge_cases() {
    let even = [0.0, 0.0];
    assert_eq!(arena_score(Outcome::Won(0), 0, even), 1.0);
    assert_eq!(arena_score(Outcome::Won(0), 1, even), 0.0);
    // A win is a win whatever it cost.
    assert_eq!(arena_score(Outcome::Won(1), 1, [0.0, 0.9]), 1.0);
    assert_eq!(arena_score(Outcome::TimeLimit, 0, even), 0.5);
    assert_eq!(arena_score(Outcome::TimeLimit, 0, [0.0, 0.5]), 0.75);
    assert_eq!(arena_score(Outcome::TimeLimit, 1, [0.0, 0.5]), 0.25);
    assert_eq!(arena_score(Outcome::TimeLimit, 0, [0.0, 1.0]), 1.0);
    assert_eq!(arena_score(Outcome::TimeLimit, 1, [0.0, 1.0]), 0.0);
    assert_eq!(arena_score(Outcome::BothOut, 0, [1.0, 1.0]), 0.5);
    assert_eq!(arena_score(Outcome::BothOut, 0, [0.25, 0.75]), 0.75);
    // Shares out of range (they are clamped where they are computed) still give a score in 0..1.
    assert_eq!(arena_score(Outcome::TimeLimit, 0, [-1.0, 2.0]), 1.0);
    assert_eq!(arena_score(Outcome::TimeLimit, 0, [2.0, -1.0]), 0.0);
}

#[test]
fn seeds_average_into_a_summary() {
    let content = data::base_content();
    let mut spec = runner_vs_raiders(0);
    spec.time_limit = 20.0;
    let results = run_seeds(&content, &spec, &[1, 2, 3]).unwrap();
    assert_eq!(results.len(), 3);
    let one = {
        let mut s = spec.clone();
        s.seed = 2;
        run_fight(&content, &s).unwrap()
    };
    assert_eq!(results[1], one, "run_seeds replaces the seed");
    let sum = Summary::of(&results);
    assert_eq!(sum.fights, 3);
    assert_eq!(sum.wins[0] + sum.wins[1] + sum.undecided, 3);
    let mean0 = results.iter().map(|r| r.sides[0].score).sum::<f32>() / 3.0;
    assert!((sum.mean_score[0] - mean0).abs() < 1e-6);
    assert_eq!(Summary::of(&[]), Summary::default());
}

#[test]
fn the_spec_reads_back_from_its_text() {
    let content = data::base_content();
    let skiff = data::blueprint_to_ron(content.ships.get("rim_skiff").unwrap());
    let spec = FightSpec {
        seed: 99,
        sides: [
            arena::Side {
                entries: vec![
                    Entry {
                        design: DesignRef::Blueprint(skiff),
                        behavior: Some(BehaviorRef::Text(
                            "target nearest\nalways -> approach(60%)".into(),
                        )),
                        count: 2,
                        formation: Formation::Column,
                    },
                    Entry::ship("halvex_runner"),
                ],
            },
            arena::Side {
                entries: vec![Entry {
                    behavior: Some(BehaviorRef::Content("raider_basic".into())),
                    count: 4,
                    ..Entry::ship("automaton_raider")
                }],
            },
        ],
        start_distance: 120.5,
        time_limit: 45.0,
    };
    let text = spec.to_ron();
    assert_eq!(FightSpec::from_ron(&text).unwrap(), spec, "{text}");
    // The texts are written over several lines, as typed.
    assert!(text.contains("always -> approach(60%)\""), "{text}");
    // The written spec runs.
    let mut short = spec.clone();
    short.time_limit = 1.0;
    run_fight(&content, &short).unwrap();
    // Defaults may be left out.
    let min = FightSpec::from_ron(
        "(seed: 1, sides: [(entries: [(design: Ship(\"rim_skiff\"))]), (entries: [(design: Ship(\"automaton_raider\"))])])",
    )
    .unwrap();
    assert_eq!(
        min,
        FightSpec::new(
            1,
            vec![Entry::ship("rim_skiff")],
            vec![Entry::ship("automaton_raider")]
        )
    );
}

#[test]
fn bad_specs_give_errors() {
    let content = data::base_content();
    let build = |spec: &FightSpec| Fight::new(&content, spec).err();
    let pair = |a: Entry, b: Entry| FightSpec::new(1, vec![a], vec![b]);
    let raider = || Entry::ship("automaton_raider");

    assert!(matches!(FightSpec::from_ron("(seed: 1"), Err(ArenaError::Parse(_))));
    assert!(matches!(
        FightSpec::from_ron("(seed: 1, sides: [(entries: []), (entries: [])], speed: 3)"),
        Err(ArenaError::Parse(_))
    ));
    assert!(matches!(
        FightSpec::from_ron("(seed: 1, sides: [(entries: []), (entries: []), (entries: [])])"),
        Err(ArenaError::Parse(e)) if e.contains("two sides")
    ));

    assert_eq!(
        build(&pair(Entry::ship("no_such_ship"), raider())),
        Some(ArenaError::UnknownShip {
            side: 0,
            entry: 0,
            id: "no_such_ship".into()
        })
    );
    let unknown_behavior = Entry {
        behavior: Some(BehaviorRef::Content("dance".into())),
        ..raider()
    };
    assert_eq!(
        build(&pair(raider(), unknown_behavior)),
        Some(ArenaError::UnknownBehavior {
            side: 1,
            entry: 0,
            id: "dance".into()
        })
    );
    let bad_text = Entry {
        behavior: Some(BehaviorRef::Text("always -> moonwalk".into())),
        ..raider()
    };
    assert!(matches!(
        build(&pair(bad_text, raider())),
        Some(ArenaError::BehaviorText { side: 0, entry: 0, errors }) if !errors.is_empty()
    ));
    let garbage = Entry {
        design: DesignRef::Blueprint("(id: \"x\"".into()),
        ..raider()
    };
    assert!(matches!(
        build(&pair(raider(), garbage)),
        Some(ArenaError::Blueprint { side: 1, entry: 0, .. })
    ));
    // Reads, but fails the content's checks (an unknown part).
    let mut bp = content.ships.get("automaton_raider").unwrap().clone();
    bp.parts[0].part = "warp_core".into();
    bp.behavior = None;
    let invalid = Entry {
        design: DesignRef::Blueprint(data::blueprint_to_ron(&bp)),
        ..raider()
    };
    assert!(matches!(
        build(&pair(invalid, raider())),
        Some(ArenaError::Blueprint { side: 0, entry: 0, problems }) if !problems.is_empty()
    ));
    // No behavior of its own and a faction without a default (content of a mod, say).
    let mut modded = data::base_content();
    modded.factions.get_mut("ship_mind").unwrap().behavior = None;
    assert_eq!(
        Fight::new(&modded, &pair(raider(), Entry::ship("rim_skiff"))).err(),
        Some(ArenaError::NoBehavior {
            side: 1,
            entry: 0,
            design: "rim_skiff".into()
        })
    );

    let mut spec = pair(raider(), raider());
    spec.time_limit = f32::NAN;
    assert!(matches!(build(&spec), Some(ArenaError::Number { field: "time_limit", .. })));
    let mut spec = pair(raider(), raider());
    spec.start_distance = -5.0;
    assert!(matches!(build(&spec), Some(ArenaError::Number { field: "start_distance", .. })));
    let spec = FightSpec::new(1, vec![], vec![raider()]);
    assert_eq!(build(&spec), Some(ArenaError::EmptySide { side: 0 }));
    let spec = pair(raider(), Entry { count: 0, ..raider() });
    assert_eq!(build(&spec), Some(ArenaError::ZeroCount { side: 1, entry: 0 }));
    let spec = pair(Entry { count: 65, ..raider() }, raider());
    assert_eq!(build(&spec), Some(ArenaError::TooManyShips { side: 0, ships: 65 }));
    // Every error says what is wrong in words.
    assert!(build(&spec).unwrap().to_string().contains("65 ships"));
}
