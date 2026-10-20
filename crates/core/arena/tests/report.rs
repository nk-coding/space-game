//! Ratings and part reports (`arena::report`, `docs/arena.md` "Ratings and part reports"): the round robin, the
//! Bradley–Terry fit, the part sums and flags, the report's text forms, and a tiny pool fought for real.

use std::collections::BTreeMap;

use arena::report::{
    Entrant, Flag, MEAN_RATING, Pool, Report, THRESHOLDS, Tally, bradley_terry, expected, report, round_robin,
};
use arena::{Entry, FightSpec, run_fight};

fn entrant(name: &str, parts: &[&str]) -> Entrant {
    let mut map = BTreeMap::new();
    for p in parts {
        *map.entry(p.to_string()).or_insert(0) += 1;
    }
    Entrant {
        name: name.into(),
        build_points: 10.0,
        parts: map,
    }
}

/// `a` against `b`: `wins` won by `a`, `losses` by `b`, `draws` at 0.5 each.
fn play(t: &mut Tally, a: usize, b: usize, wins: u32, losses: u32, draws: u32) {
    for _ in 0..wins {
        t.record(a, b, Some(0), [1.0, 0.0]);
    }
    for _ in 0..losses {
        t.record(a, b, Some(1), [0.0, 1.0]);
    }
    for _ in 0..draws {
        t.record(a, b, None, [0.5, 0.5]);
    }
}

#[test]
fn the_round_robin_plays_every_pair_both_ways_and_no_mirrors() {
    let jobs = round_robin(4, &[1, 2, 3]);
    assert_eq!(jobs.len(), 4 * 3 * 3);
    assert!(jobs.iter().all(|(a, b, _)| a != b));
    for a in 0..4 {
        for b in 0..4 {
            let n = jobs.iter().filter(|j| j.0 == a && j.1 == b).count();
            assert_eq!(n, if a == b { 0 } else { 3 });
        }
    }
    assert_eq!(jobs, round_robin(4, &[1, 2, 3]));
}

#[test]
fn a_tally_counts_both_sides_and_ignores_mirrors() {
    let mut t = Tally::new(2);
    t.record(0, 1, Some(0), [1.0, 0.0]);
    t.record(1, 0, None, [0.7, 0.3]);
    t.record(1, 1, Some(0), [1.0, 0.0]);
    let a = t.cell(0, 1);
    let b = t.cell(1, 0);
    assert_eq!((a.fights, a.wins, b.fights, b.wins), (2, 1, 2, 0));
    assert!((a.score - 1.3).abs() < 1e-6 && (b.score - 0.7).abs() < 1e-6);
    assert_eq!(t.cell(1, 1).fights, 0);
    assert_eq!(t.total(0).fights, 2);
}

#[test]
fn bradley_terry_matches_the_scores_and_centres_on_the_mean() {
    // Two designs, 3 : 1 by score: the fit's expected score is the observed one (with the prior's half-draw).
    let mut t = Tally::new(2);
    play(&mut t, 0, 1, 60, 20, 20);
    let r = bradley_terry(&t, 1.0);
    let want = (70.0 + 0.5) / 101.0;
    assert!((expected(r[0], r[1]) - want).abs() < 1e-9, "{r:?}");
    assert!(((r[0] + r[1]) / 2.0 - MEAN_RATING).abs() < 1e-9);

    // Three in a line: A beats B beats C, A beats C most.
    let mut t = Tally::new(3);
    play(&mut t, 0, 1, 7, 3, 0);
    play(&mut t, 1, 2, 7, 3, 0);
    play(&mut t, 0, 2, 9, 1, 0);
    let r = bradley_terry(&t, 1.0);
    assert!(r[0] > r[1] && r[1] > r[2], "{r:?}");
    assert_eq!(r, bradley_terry(&t, 1.0), "the same tally, the same bits");
}

#[test]
fn a_design_that_wins_everything_gets_a_finite_rating() {
    let mut t = Tally::new(3);
    play(&mut t, 0, 1, 10, 0, 0);
    play(&mut t, 0, 2, 10, 0, 0);
    play(&mut t, 1, 2, 5, 5, 0);
    let r = bradley_terry(&t, 1.0);
    assert!(r.iter().all(|x| x.is_finite()), "{r:?}");
    assert!(r[0] > r[1] + 400.0);
}

#[test]
fn the_report_ranks_designs_and_flags_parts() {
    let content = data::base_content();
    let ids: Vec<&str> = content.parts.iter().map(|p| p.id.as_str()).take(4).collect();
    let [core, gun, odd, unused] = ids[..] else { panic!() };
    // Four designs: the two with `gun` win; only the worst carries `odd`; none carries `unused`.
    let pool = [
        entrant("ace", &[core, gun, gun]),
        entrant("good", &[core, gun]),
        entrant("meh", &[core]),
        entrant("bad", &[core, odd]),
    ];
    let mut t = Tally::new(4);
    play(&mut t, 0, 1, 6, 4, 0);
    play(&mut t, 0, 2, 9, 1, 0);
    play(&mut t, 0, 3, 10, 0, 0);
    play(&mut t, 1, 2, 8, 2, 0);
    play(&mut t, 1, 3, 9, 1, 0);
    play(&mut t, 2, 3, 6, 4, 0);
    let r = report(&content, &pool, &t, &THRESHOLDS);
    let names: Vec<&str> = r.designs.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["ace", "good", "meh", "bad"]);
    assert_eq!(r.top, 2);
    assert_eq!(r.fights, 60);
    assert_eq!(r.designs.iter().filter(|d| d.top).count(), 2);
    let row = |id: &str| r.parts.iter().find(|p| p.part == id).unwrap();
    assert_eq!(row(gun).flag, Some(Flag::Dominant));
    assert_eq!((row(gun).designs, row(gun).pick_rate), (2, 1.0));
    assert_eq!(row(core).flag, None, "in every design: scores like the pool");
    assert!((row(core).score - 0.5).abs() < 1e-6);
    assert_eq!(row(odd).flag, Some(Flag::Underused));
    assert_eq!(row(unused).flag, Some(Flag::NeverPicked));
    assert_eq!(r.parts.len(), content.parts.len());
    assert_eq!(r.parts[0].part, gun, "carried parts first, by pick rate and score");
    assert!(r.parts.iter().skip_while(|p| p.designs > 0).all(|p| p.designs == 0));

    let md = r.to_markdown();
    assert!(md.contains("| 1 | ace |") && md.contains("DOMINANT") && md.contains("NEVER PICKED ("));
    assert_eq!(Report::from_ron(&r.to_ron()).unwrap(), r);
    assert_eq!(report(&content, &pool, &t, &THRESHOLDS), r, "deterministic");
}

#[test]
fn the_committed_pool_reads_and_names_content_ships() {
    let pool = Pool::from_ron(include_str!("../report/pool.ron")).unwrap();
    let content = data::base_content();
    assert!(pool.designs.len() >= 2 && pool.seeds >= 1);
    for d in &pool.designs {
        assert!(d.ends_with(".ron") || content.ships.get(d).is_some(), "unknown ship {d}");
    }
}

#[test]
fn a_tiny_pool_fought_for_real() {
    let content = data::base_content();
    let ships = ["rim_skiff", "automaton_raider", "automaton_lancer"];
    let pool: Vec<Entrant> = ships
        .iter()
        .map(|id| Entrant::new(&content, id, content.ships.get(id).unwrap()))
        .collect();
    assert!(pool.iter().all(|e| e.build_points > 0.0 && !e.parts.is_empty()));
    let mut t = Tally::new(ships.len());
    for (a, b, seed) in round_robin(ships.len(), &[1]) {
        let mut spec = FightSpec::new(seed, vec![Entry::ship(ships[a])], vec![Entry::ship(ships[b])]);
        spec.time_limit = 8.0;
        t.add(a, b, &run_fight(&content, &spec).unwrap());
    }
    let r = report(&content, &pool, &t, &THRESHOLDS);
    assert_eq!(r.fights, 6);
    assert_eq!(r.designs.len(), 3);
    let mean = r.designs.iter().map(|d| d.rating as f64).sum::<f64>() / 3.0;
    assert!((mean - MEAN_RATING).abs() < 1.0, "{mean}");
    assert!(r.parts.iter().any(|p| p.designs == 3), "the three share a part (a logic core)");
}
