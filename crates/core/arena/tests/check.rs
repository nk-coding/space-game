//! The balance check (`arena::check`, `docs/arena.md` "The balance check"): the committed set and baseline read and
//! match, a row sums its fights, and the tolerances flag what leaves them. The set itself runs in release
//! (`scripts/arena-check.sh`), not here.

use arena::check::{Baseline, CheckRow, CheckSet, Tolerance};
use arena::{Entry, FightSpec, run_fight};

const SET: &str = include_str!("../check/set.ron");
const BASELINE: &str = include_str!("../check/baseline.ron");

fn stem(path: &str) -> &str {
    path.rsplit('/').next().unwrap().trim_end_matches(".ron")
}

#[test]
fn the_committed_set_reads_and_has_a_baseline_row_per_fight() {
    let set = CheckSet::from_ron(SET).unwrap();
    let base = Baseline::from_ron(BASELINE).unwrap();
    assert!(!set.fights.is_empty());
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("check");
    for f in &set.fights {
        let text = std::fs::read_to_string(dir.join(&f.fight)).unwrap();
        FightSpec::from_ron(&text).unwrap();
        let row = base
            .row(stem(&f.fight))
            .unwrap_or_else(|| panic!("no baseline row for {}", f.fight));
        assert_eq!(
            row.seeds, f.seeds,
            "{}: run scripts/arena-check.sh --update",
            f.fight
        );
    }
    assert_eq!(base.fights.len(), set.fights.len());
}

#[test]
fn a_baseline_round_trips() {
    let base = Baseline::from_ron(BASELINE).unwrap();
    assert_eq!(Baseline::from_ron(&base.to_ron()).unwrap(), base);
}

#[test]
fn a_row_sums_its_fights_and_repeats_bit_for_bit() {
    let content = data::base_content();
    let rows: Vec<CheckRow> = (0..2)
        .map(|_| {
            let results: Vec<_> = (1..=3)
                .map(|seed| {
                    let mut spec = FightSpec::new(
                        seed,
                        vec![Entry::ship("rim_skiff")],
                        vec![Entry::ship("automaton_raider")],
                    );
                    spec.time_limit = 5.0;
                    run_fight(&content, &spec).unwrap()
                })
                .collect();
            CheckRow::of("short", &results)
        })
        .collect();
    assert_eq!(rows[0], rows[1]);
    let r = &rows[0];
    assert_eq!((r.fight.as_str(), r.seeds), ("short", 3));
    assert!(r.won[0] + r.won[1] <= 3 && (0.0..=1.0).contains(&r.score) && r.time <= 5.0);
    assert_eq!(r.hashes.len(), 16);
}

#[test]
fn the_tolerances_flag_what_leaves_them() {
    let tol = Tolerance {
        wins: 0.15,
        score: 0.12,
        time: 0.15,
    };
    let base = CheckRow {
        fight: "f".into(),
        seeds: 100,
        won: [50, 30],
        score: 0.6,
        time: 60.0,
        hashes: "0".into(),
    };
    let moved = |f: &dyn Fn(&mut CheckRow)| {
        let mut r = base.clone();
        f(&mut r);
        r.out_of(Some(&base), &tol)
    };
    assert!(moved(&|_| {}).is_empty());
    assert!(
        moved(&|r| {
            r.won = [64, 20];
            r.score = 0.71;
            r.time = 68.0
        })
        .is_empty()
    );
    assert_eq!(moved(&|r| r.won[0] = 66), ["side 0 wins +16 %"]);
    assert_eq!(moved(&|r| r.won[1] = 14), ["side 1 wins -16 %"]);
    assert_eq!(moved(&|r| r.score = 0.45), ["score -0.150"]);
    assert_eq!(moved(&|r| r.time = 70.0), ["time +17 %"]);
    assert_eq!(moved(&|r| r.seeds = 50), ["50 seeds, baseline 100"]);
    assert_eq!(base.out_of(None, &tol), ["no baseline"]);
}
