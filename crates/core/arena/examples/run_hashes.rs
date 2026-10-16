//! Prints an evolution run's hashes, one `generation hash` line per generation (FNV-1a 64 of the run file after it,
//! `Run::snapshot`), for a run straight through in budgeted slices, and checks that the same run interrupted after
//! `--resume-at` generations, written as a run file, read back, restored and run on in other slices gives the same
//! lines, and that its parts (the game's lab's) restore to the same run (it panics if not). Built natively and for `wasm32-wasip1`; `scripts/determinism-check.sh --run NAME` runs
//! both and compares, so a run resumes alike on every platform (GDR-0092).
//!
//! Usage: `run_hashes [--scenario two_starts_locked|mixed_trials|everything] [--seed N] [--resume-at K]`: the run files of
//! `crates/core/arena/runs/` that are small enough (compiled in: the WASI run has no filesystem), with the seed
//! replaced.

const RUNS: [(&str, &str); 3] = [
    ("two_starts_locked", include_str!("../runs/two_starts_locked.ron")),
    // Every kind of job: fights, gauntlets and a voyage, with goals (GDR-0092 step 5).
    ("mixed_trials", include_str!("../runs/mixed_trials.ron")),
    // Every gene, and fights flown by a stand-in pilot (GDR-0092 step 6).
    ("everything", include_str!("../runs/everything.ron")),
];

/// The two slicings: as a client at 4 ms a frame steps, and much coarser.
const BUDGETS: [u64; 2] = [700, 50_000];

fn main() {
    let mut seed: u64 = 1;
    let mut at: u32 = 3;
    let mut name = String::from("two_starts_locked");
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--seed" => seed = val().parse().expect("--seed: integer"),
            "--resume-at" => at = val().parse().expect("--resume-at: integer"),
            "--scenario" => name = val(),
            other => panic!("unknown argument {other}"),
        }
    }
    let text = RUNS
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("unknown run {name} (known: two_starts_locked, mixed_trials)"))
        .1;
    let mut config = arena::RunConfig::from_ron(text).expect("the run reads");
    config.seed = seed;
    let content = data::base_content();
    let hash = |run: &arena::Run| data::hash::fnv1a64(run.snapshot().to_ron().as_bytes());
    let mut straight = arena::Run::new(&content, config.clone()).expect("the run builds");
    let generations = straight.progress().generations;
    println!("# arena run={name} seed={seed} generations={generations} resume-at={at}");
    let mut lines = Vec::new();
    while !straight.finished() {
        let g = straight.generation();
        while straight.generation() == g {
            straight.step(BUDGETS[0]);
        }
        lines.push(hash(&straight));
        println!("{g} {:016x}", hash(&straight));
    }
    let mut first = arena::Run::new(&content, config).expect("the run builds");
    while first.generation() < at.min(generations) {
        first.step(BUDGETS[1]);
    }
    let file = first.snapshot().to_ron();
    let save = arena::RunSave::from_ron(&file).expect("the run file reads");
    // The game's lab keeps it in parts (ADR-0041): read back through their text, the same run as the file's.
    let parts = first.parts();
    let parts = arena::evolve::RunParts {
        head: arena::evolve::RunHead::from_ron(&parts.head.to_ron()).expect("the head reads"),
        config: arena::RunConfig::from_ron(&parts.config.to_ron()).expect("the configuration reads"),
        generations: parts.generations.iter().map(|p| arena::evolve::RunPart::from_ron(&p.to_ron()).expect("a part reads")).collect(),
    };
    let from_parts = arena::Run::restore_parts(&content, &parts).expect("the parts restore");
    assert_eq!(from_parts.snapshot().to_ron(), file, "restored from its parts, the run DIFFERS from its file's");
    let mut resumed = arena::Run::restore(&content, &save).expect("the run restores");
    assert!(!resumed.stale(), "the same rules");
    while !resumed.finished() {
        let g = resumed.generation();
        while resumed.generation() == g {
            resumed.step(BUDGETS[1]);
        }
        assert_eq!(hash(&resumed), lines[g as usize], "generation {g} DIFFERS after the resume");
    }
    let best = straight.archive().best().expect("an elite");
    println!(
        "# resumed after generation {}: identical; {} genomes, best {} score={:.4}, run file {} bytes",
        at.min(generations) as i64 - 1,
        straight.table().len(),
        best.id,
        best.score,
        straight.snapshot().to_ron().len()
    );
}
