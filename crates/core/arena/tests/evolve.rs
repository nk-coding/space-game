//! The voyage fitness (`arena::voyage`) and the evolution (`arena::evolve`): determinism, slices and threads,
//! limits, the configuration's text form and errors, and a tiny run that improves.

use arena::evolve::{Fitness, Genes, Job, JobResult, Measure, Results};
use arena::{DesignRef, Run, RunConfig, RunError, VoyageSpec};
use data::Content;
use sim::voyage::runner::Outcome;

/// A small, quick run: the skiff's behavior against one raider, 10 s fights.
const TINY: &str = r#"(
    seed: 5,
    evolve: Behavior,
    generations: 3,
    population: 6,
    fitness: ArenaPool(opponents: [(design: Ship("automaton_raider"))], seeds: 2, time_limit: 10),
    start: Design(design: Ship("rim_skiff")),
    archive: (axes: [(measure: EngageDistance, min: 0, max: 120, cells: 6), (measure: Rules, min: 0, max: 12, cells: 4)]),
)"#;

fn config(text: &str) -> RunConfig {
    RunConfig::from_ron(text).unwrap()
}

/// Runs to the end with `step(budget)`.
fn stepped(c: &Content, cfg: RunConfig, budget: u64) -> Results {
    let mut run = Run::new(c, cfg).unwrap();
    while !run.step(budget).finished {}
    run.results()
}

/// Runs to the end as the CLI does: each generation's jobs on `threads` threads, results handed back in order.
fn threaded(c: &Content, cfg: RunConfig, threads: usize) -> Results {
    let mut run = Run::new(c, cfg).unwrap();
    while !run.finished() {
        let jobs = run.jobs().to_vec();
        let mut results: Vec<Option<JobResult>> = vec![None; jobs.len()];
        std::thread::scope(|s| {
            let chunk = jobs.len().div_ceil(threads).max(1);
            let handles: Vec<_> = jobs
                .chunks(chunk)
                .enumerate()
                .map(|(k, js)| s.spawn(move || (k, js.iter().map(|j| j.run(c)).collect::<Vec<_>>())))
                .collect();
            for h in handles {
                let (k, rs) = h.join().unwrap();
                for (i, r) in rs.into_iter().enumerate() {
                    results[k * chunk + i] = Some(r);
                }
            }
        });
        run.submit(results.into_iter().map(Option::unwrap).collect());
    }
    run.results()
}

#[test]
fn the_same_run_gives_the_same_archive_in_slices_and_on_threads() {
    let c = data::base_content();
    let whole = stepped(&c, config(TINY), u64::MAX);
    assert_eq!(whole.history.len(), 3);
    assert!(!whole.elites.is_empty());
    assert_eq!(whole, stepped(&c, config(TINY), u64::MAX), "same config and seed");
    assert_eq!(whole, stepped(&c, config(TINY), 97), "small slices");
    assert_eq!(whole, threaded(&c, config(TINY), 1), "one thread");
    assert_eq!(whole, threaded(&c, config(TINY), 4), "four threads");
    let other = stepped(&c, config(&TINY.replace("seed: 5", "seed: 6")), u64::MAX);
    assert_ne!(whole, other, "another seed, another run");
}

#[test]
fn design_runs_keep_the_limits() {
    let c = data::base_content();
    let text = r#"(
        seed: 2,
        evolve: Stages([(evolve: Design, generations: 3), (evolve: Both, generations: 2)]),
        population: 8,
        crossover: 0.5,
        fitness: ArenaPool(opponents: [(design: Ship("automaton_raider"))], seeds: 2, time_limit: 8),
        limits: (frame: "frigate", build_points: 62, mirror: true,
            parts: ["armor_block", "slope_1x1", "pulse_laser", "radiator", "capacitor", "fission_reactor",
                "cargo_bay", "logic_core", "thruster", "shield_emitter", "gyro", "rcs_pod"],
            materials: ["steel"]),
        start: Design(design: Ship("rim_skiff")),
    )"#;
    let cfg = config(text);
    let allowed = cfg.limits.parts.clone().unwrap();
    let mut run = Run::new(&c, cfg).unwrap();
    while !run.step(u64::MAX).finished {}
    let r = run.results();
    let genes: Vec<Genes> = r.history.iter().map(|g| g.genes).collect();
    assert_eq!(genes, [Genes::Design, Genes::Design, Genes::Design, Genes::Both, Genes::Both]);
    assert!(r.elites.len() > 1, "the archive spreads");
    assert!(r.elites.iter().any(|e| e.genome.blueprint.parts != c.ships.get("rim_skiff").unwrap().parts));
    for e in &r.elites {
        let bp = &e.genome.blueprint;
        let report = sim::design_report(&c, bp, &sim::SimConfig::default()).unwrap();
        assert!(report.build_points <= 62.0, "{} build points", report.build_points);
        assert!(report.flight_ready() && !report.guns.is_empty());
        // The default archive: acceleration × latent firepower (blocked and idle guns count).
        assert_eq!(e.measures[1], report.latent_firepower());
        assert!(bp.size.0 <= 24 && bp.size.1 <= 24, "the frigate frame");
        for p in &bp.parts {
            assert!(allowed.contains(&p.part), "{} is not allowed", p.part);
            assert!(p.material.as_deref().is_none_or(|m| m == "steel"));
        }
        // Mirror lock: every cell's part has the same part on the mirror cell.
        let mut cells = std::collections::BTreeMap::new();
        for p in &bp.parts {
            for cell in c.placed_shape(p).unwrap().cells() {
                cells.insert(cell, (p.part.clone(), p.material.clone()));
            }
        }
        let w = bp.size.0 as i32;
        for ((x, y), part) in &cells {
            assert_eq!(cells.get(&(w - 1 - x, *y)), Some(part), "symmetric at ({x}, {y})");
        }
        // The text forms read back (SAVE AS DESIGN).
        let back = data::parse_blueprint(&e.genome.blueprint_text()).unwrap();
        assert_eq!(back.parts, bp.parts);
        assert!(data::BehaviorEditor::from_text(&c.behavior_nodes, e.genome.behavior_text()).is_ok());
    }
}

#[test]
fn a_tiny_run_improves_on_an_easy_target() {
    // The skiff starts by running away from one raider: a fight it cannot win. A few generations of behavior
    // evolution find one that fights.
    let c = data::base_content();
    let text = r#"(
        seed: 1,
        evolve: Behavior,
        generations: 5,
        population: 8,
        fitness: ArenaPool(opponents: [(design: Ship("automaton_raider"))], seeds: 2, time_limit: 20),
        start: Design(design: Ship("rim_skiff"), behavior: Text("always -> flee")),
        archive: (axes: [(measure: Rules, min: 0, max: 12, cells: 4)]),
    )"#;
    let r = stepped(&c, config(text), u64::MAX);
    let first = r.history[0].best;
    let last = r.history.last().unwrap().archive_best;
    assert!(last > first + 0.2, "{first} -> {last}");
    let best: Vec<f32> = r.history.iter().map(|g| g.archive_best).collect();
    assert!(best.windows(2).all(|w| w[1] >= w[0]), "the archive's best never drops: {best:?}");
}

#[test]
fn a_random_start_flies_and_evolves() {
    let c = data::base_content();
    let text = r#"(
        seed: 3,
        evolve: Both,
        generations: 2,
        population: 4,
        fitness: ArenaPool(opponents: [(design: Ship("automaton_raider"))], seeds: 2, time_limit: 5),
        limits: (frame: "frigate"),
        start: Random(faction: "ship_mind"),
    )"#;
    let r = stepped(&c, config(text), u64::MAX);
    assert_eq!(r.history.len(), 2);
    let report = sim::design_report(&c, &r.elites[0].genome.blueprint, &sim::SimConfig::default()).unwrap();
    assert!(report.flight_ready());
}

#[test]
fn the_run_text_reads_back_and_errors_are_values() {
    let c = data::base_content();
    for path in ["skiff_behavior.ron", "skiff_design_60bp.ron", "skiff_voyage_stages.ron"] {
        let text = std::fs::read_to_string(format!("{}/runs/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let cfg = RunConfig::from_ron(&text).unwrap();
        assert_eq!(RunConfig::from_ron(&cfg.to_ron()).unwrap(), cfg, "{path}");
        Run::new(&c, cfg).unwrap_or_else(|e| panic!("{path}: {e}"));
    }
    let err = |text: &str| Run::new(&c, RunConfig::from_ron(text)?).map(|_| ());
    assert!(matches!(RunConfig::from_ron("(seed: 1"), Err(RunError::Config(_))));
    assert!(matches!(err(&TINY.replace("generations: 3,", "")), Err(RunError::Config(_))));
    assert!(matches!(err(&TINY.replace("population: 6", "population: 0")), Err(RunError::Config(_))));
    assert!(matches!(
        err(&TINY.replace("evolve: Behavior", "evolve: Stages([(evolve: Design, generations: 2)])")),
        Err(RunError::Config(_))
    ));
    assert!(matches!(
        err(&TINY.replace(r#"start: Design(design: Ship("rim_skiff"))"#, r#"start: Design(design: Ship("nope"))"#)),
        Err(RunError::Start(_))
    ));
    assert!(matches!(
        err(&TINY.replace("automaton_raider", "nope")),
        Err(RunError::Fitness(_))
    ));
    // Over the budget, a locked-out part, no such frame.
    let limited = |limits: &str| TINY.replace("archive:", &format!("limits: {limits}, archive:"));
    assert!(matches!(err(&limited("(build_points: 30)")), Err(RunError::Start(_))));
    assert!(matches!(err(&limited(r#"(parts: ["armor_block"])"#)), Err(RunError::Start(_))));
    assert!(matches!(err(&limited(r#"(frame: "nope")"#)), Err(RunError::Config(_))));
    // A measure that needs fights with the voyage fitness.
    let voyages = TINY.replace(
        r#"ArenaPool(opponents: [(design: Ship("automaton_raider"))], seeds: 2, time_limit: 10)"#,
        r#"Voyages(ids: ["yard_approach"])"#,
    );
    assert!(matches!(err(&voyages), Err(RunError::Config(_))));
    let cfg = config(&voyages.replace("EngageDistance", "Firepower"));
    assert!(matches!(cfg.fitness, Fitness::Voyages { .. }));
    assert_eq!(cfg.archive.axes[0].measure, Measure::Firepower);
    assert!(Run::new(&c, cfg).is_ok());
}

#[test]
fn voyage_fitness_scores_a_run() {
    let c = data::base_content();
    let spec = VoyageSpec {
        voyage: "yard_approach".into(),
        seed: 2,
        design: DesignRef::Ship("rim_skiff".into()),
        behavior: None,
        minutes: 4.0,
    };
    let run = arena::run_voyage(&c, &spec).unwrap();
    assert!(run.ticks <= spec.limit_ticks());
    let score = arena::voyage_score(&run, spec.limit_ticks());
    assert!((0.0..=1.0).contains(&score));
    // In slices: the same run.
    let mut runner = arena::start_voyage(&c, &spec).unwrap();
    let sliced = loop {
        if let Some(r) = runner.step(500) {
            break r.clone();
        }
    };
    assert_eq!((sliced.outcome, sliced.ticks, sliced.goal), (run.outcome, run.ticks, run.goal));
    let fit = arena::voyage_fitness(&c, &spec, &["yard_approach".into()], &[2]).unwrap();
    assert_eq!(fit, score);
    // The formula's edges: a clear beats any non-clear, faster and less hurt is better.
    let mut r = run.clone();
    r.sites_with_ships = 4;
    r.sites_cleared = 4;
    r.goal = true;
    r.outcome = Outcome::Recalled;
    r.hp_lost = 0.0;
    r.ticks = 0;
    assert_eq!(arena::voyage_score(&r, 1000), 1.0);
    r.ticks = 1000;
    r.hp_lost = r.hp_total;
    assert!((arena::voyage_score(&r, 1000) - 0.6).abs() < 1e-6);
    r.outcome = Outcome::OutOfTime;
    assert!((arena::voyage_score(&r, 1000) - 0.5).abs() < 1e-6);
    r.outcome = Outcome::Lost;
    assert!((arena::voyage_score(&r, 1000) - 0.25).abs() < 1e-6);
    r.sites_cleared = 0;
    assert_eq!(arena::voyage_score(&r, 1000), 0.0);
    // Errors are values.
    let bad = |f: &dyn Fn(&mut VoyageSpec)| {
        let mut s = spec.clone();
        f(&mut s);
        arena::start_voyage(&c, &s).err().unwrap()
    };
    assert!(matches!(bad(&|s| s.voyage = "nope".into()), arena::ArenaError::UnknownVoyage(_)));
    assert!(matches!(bad(&|s| s.minutes = 0.0), arena::ArenaError::Number { .. }));
    assert!(matches!(
        bad(&|s| s.behavior = Some(arena::BehaviorRef::Text("always -> nope".into()))),
        arena::ArenaError::Voyage(_)
    ));
}

/// GDR-0092: after each generation the run hands out its best child's jobs and results (its fights to re-watch),
/// in the threaded and the stepped way alike; jobs and results are plain data with a RON form.
#[test]
fn the_best_childs_fights_are_kept_and_jobs_are_data() {
    let c = data::base_content();
    let mut run = Run::new(&c, config(TINY)).unwrap();
    assert!(run.champion().is_empty());
    let jobs = run.jobs().to_vec();
    run.submit(jobs.iter().map(|j| j.run(&c)).collect());
    let champion = run.champion().to_vec();
    assert_eq!(champion.len(), 2, "one opponent, two seeds");
    let mean = champion.iter().map(|(_, r)| r.score as f64).sum::<f64>() / champion.len() as f64;
    assert_eq!(mean as f32, run.state().history[0].best);
    let mut stepped = Run::new(&c, config(TINY)).unwrap();
    while stepped.state().history.is_empty() {
        stepped.step(1000);
    }
    assert_eq!(stepped.champion(), champion.as_slice());
    for (job, result) in &champion {
        round_trip(job);
        round_trip(result);
    }
    round_trip(&Job::Voyage {
        candidate: 3,
        spec: VoyageSpec { voyage: "yard_approach".into(), seed: 2, design: DesignRef::Ship("rim_skiff".into()), behavior: None, minutes: 5.0 },
    });
}

/// Writes a value as RON and reads it back to an equal one.
fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(v: &T) {
    let text = ron::to_string(v).unwrap();
    assert_eq!(&ron::from_str::<T>(&text).unwrap(), v, "{text}");
}
