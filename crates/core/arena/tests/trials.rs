//! Trials and goals (GDR-0092 step 5): every kind builds deterministic jobs that replay from their records and fight
//! codes, a gauntlet's waves come in and count, weights and goals combine as specified, RE-SCORE and VALIDATE give
//! the same results however they are computed, and the lab's presets become trials.

use arena::evolve::{Goals, Job, JobEnd, JobResult, Sources, Trial, TrialSpec};
use arena::{Entry, FightCode, FightSpec, Outcome, Run, RunConfig, RunSave, Side, run_fight};
use data::Content;

/// Two starts and every trial kind, small and short: a voyage of two minutes, fights of 12 s.
const MIXED: &str = r#"(
    seed: 3,
    evolve: Both,
    generations: 2,
    population: 4,
    trials: [
        (kind: EnemyShips(opponents: [(design: Ship("automaton_raider"))], hall_of_fame: 1), seeds: 1, time_limit: 12),
        (kind: Encounters(voyages: ["yard_approach"]), weight: 2, seeds: 1, time_limit: 12),
        (kind: Waves(target: 2), seeds: 1, time_limit: 25),
        (kind: Gauntlet(waves: [[(design: Ship("automaton_raider"))], [(design: Ship("automaton_raider"), count: 2)]]), seeds: 1, time_limit: 25),
        (kind: BeatYourStart, weight: 0.5, seeds: 1, time_limit: 12),
        (kind: Fleet(designs: [(design: Ship("sample_early"))]), seeds: 1, time_limit: 12),
        (kind: HallOfFame(count: 2), seeds: 1, time_limit: 12),
        (kind: Champions(designs: [(design: Ship("sample_mid"))]), seeds: 1, time_limit: 12),
        (kind: Voyages(ids: ["yard_approach"], minutes: 2), seeds: 1),
    ],
    goals: (win: 1, unhurt: 1, quick: 0.5, cheap: 0.25),
    starts: [Design(design: Ship("rim_skiff")), Design(design: Ship("halvex_runner"))],
)"#;

fn config(text: &str) -> RunConfig {
    RunConfig::from_ron(text).unwrap()
}

fn stepped(c: &Content, cfg: RunConfig, budget: u64) -> Run {
    let mut run = Run::new(c, cfg).unwrap();
    while !run.step(budget).finished {}
    run
}

fn threaded(c: &Content, mut run: Run) -> Run {
    while !run.finished() {
        let jobs = run.jobs().to_vec();
        let results: Vec<JobResult> = jobs.iter().rev().map(|j| j.run(c)).collect::<Vec<_>>().into_iter().rev().collect();
        run.submit(results);
    }
    run
}

#[test]
fn every_trial_kind_makes_jobs_that_replay() {
    let c = data::base_content();
    let run = stepped(&c, config(MIXED), 3000);
    let again = threaded(&c, Run::new(&c, config(MIXED)).unwrap());
    assert_eq!(run.snapshot().to_ron(), again.snapshot().to_ron(), "slices and jobs on threads make the same run");
    let trials = run.trials();
    assert_eq!(trials.len(), 9);
    let mut kinds = [0usize; 9];
    for g in 0..run.generation() {
        for (child, rec) in run.state().records[g as usize].children.iter().enumerate() {
            let tagged = run.tagged_jobs(g, child).unwrap();
            assert_eq!(tagged.len(), rec.jobs.len());
            for (j, ((t, job), r)) in tagged.iter().zip(&rec.jobs).enumerate() {
                kinds[*t as usize] += 1;
                let goals = r.goals.expect("a run with trials keeps every job's goals");
                assert!(goals.all().iter().all(|g| (0.0..=1.0).contains(g)), "{goals:?}");
                assert_eq!(goals.win, r.score);
                match job {
                    Job::Fight { spec, .. } => {
                        // Its code replays to the record, and the record rebuilds the result.
                        let code = FightCode::decode(&run.fight_code(g, child, j).unwrap().encode()).unwrap();
                        assert_eq!(run_fight(&c, &code.spec).unwrap(), code.record, "gen {g} child {child} job {j}");
                        assert_eq!(code.spec, *spec);
                        if matches!(trials[*t as usize].kind, Trial::Waves { .. } | Trial::Gauntlet { .. }) {
                            assert!(!spec.waves.is_empty(), "a gauntlet has its waves");
                            let JobEnd::Fight { waves, .. } = r.end else { unreachable!() };
                            assert!((1..=spec.waves.len() as u32 + 1).contains(&waves) || waves == 0);
                            assert_eq!(goals.win, waves as f32 / (spec.waves.len() + 1) as f32);
                        }
                    }
                    Job::Voyage { .. } => {
                        assert_eq!(arena::evolve::JobRecord::of(&job.run(&c), true).end, r.end, "a voyage job replays");
                    }
                }
            }
        }
    }
    // Every kind made jobs; the hall of fame only after generation 0 (none in generation 0's archive).
    assert!(kinds.iter().all(|&n| n > 0), "{kinds:?}");
    assert!(run.state().records[0].children.iter().all(|c| c.jobs.len() == run.state().records[0].children[0].jobs.len()));
}

/// GDR-0092 "GOALS": per trial the goals' means weighed by the goal weights, then the trials by theirs; weights of 0
/// and trials without jobs leave the sums. One trial with WIN alone is the mean score of its jobs (today's score).
#[test]
fn weights_and_goals_combine_as_specified() {
    let c = data::base_content();
    let run = stepped(&c, config(MIXED), u64::MAX);
    let gw = [1.0f64, 1.0, 0.5, 0.25];
    for g in 0..run.generation() {
        for (child, rec) in run.state().records[g as usize].children.iter().enumerate() {
            let b = run.breakdown(g, child).unwrap();
            let (mut total, mut wsum) = (0.0f64, 0.0f64);
            for t in &b.trials {
                let spec = run.trials()[t.trial];
                if t.jobs == 0 {
                    assert_eq!(t.score, None, "no jobs, no score");
                    continue;
                }
                let s = t.goals.all().iter().zip(gw).map(|(m, w)| *m as f64 * w).sum::<f64>() / gw.iter().sum::<f64>();
                assert!((t.score.unwrap() as f64 - s).abs() < 1e-5);
                if spec.weight > 0.0 {
                    total += spec.weight as f64 * s;
                    wsum += spec.weight as f64;
                }
            }
            assert!((rec.score as f64 - total / wsum).abs() < 1e-5, "gen {g} child {child}: {} vs {}", rec.score, total / wsum);
            // CHEAP is the design's: the same on every job.
            let cheap = rec.jobs[0].goals.unwrap().cheap;
            assert!(rec.jobs.iter().all(|j| j.goals.unwrap().cheap == cheap));
        }
    }
    // The hall of fame has no jobs in generation 0 and leaves the sums.
    let b = run.breakdown(0, 0).unwrap();
    assert_eq!(b.trials[6].jobs, 0);
    assert!(b.trials[0].jobs > 0 && b.trials[1].jobs > 0);
    // One trial and WIN alone: the mean of its jobs' scores, exactly.
    let one = r#"(seed: 2, evolve: Behavior, generations: 1, population: 3,
        trials: [(kind: EnemyShips(opponents: [(design: Ship("automaton_raider"))]), seeds: 3, time_limit: 8)],
        starts: [Design(design: Ship("rim_skiff"))])"#;
    let run = stepped(&c, config(one), u64::MAX);
    for rec in &run.state().records[0].children {
        let mean = (rec.jobs.iter().map(|j| j.score as f64).sum::<f64>() / rec.jobs.len() as f64) as f32;
        assert_eq!(rec.score, mean);
    }
    // A weight of 0 changes nothing; goals all 0 or a trial weight out of range are refused.
    let mut cfg = config(one);
    cfg.trials.push(TrialSpec { weight: 0.0, ..TrialSpec::new(Trial::BeatYourStart) });
    let zero = stepped(&c, cfg, u64::MAX);
    let scores = |r: &Run| r.state().records[0].children.iter().map(|c| c.score).collect::<Vec<_>>();
    assert_eq!(scores(&zero), scores(&run));
    assert_eq!(zero.state().records[0].children[0].jobs.len(), 3, "a trial of weight 0 is not fought");
    let mut cfg = config(one);
    cfg.goals = Goals { win: 0.0, ..Goals::default() };
    assert!(Run::new(&c, cfg).is_err());
    let mut cfg = config(one);
    cfg.trials[0].weight = -1.0;
    assert!(Run::new(&c, cfg).is_err());
}

/// A gauntlet: side 1's next wave comes in once the one before has no running vessel, around side 0; the waves
/// cleared count; side 1 is not out while one is still to come; its code round-trips and replays.
#[test]
fn a_gauntlets_waves_come_in_one_after_the_other() {
    let c = data::base_content();
    let mut spec = FightSpec::new(4, vec![Entry::ship("halvex_runner")], vec![Entry::ship("automaton_raider")]);
    spec.time_limit = 120.0;
    spec.waves = vec![Side { entries: vec![Entry::ship("automaton_raider")] }, Side { entries: vec![Entry { count: 2, ..Entry::ship("automaton_raider") }] }];
    let r = run_fight(&c, &spec).unwrap();
    let mut setup = arena::FightSetup::build(&c, &spec).unwrap();
    assert_eq!(setup.designs[1].len(), 3, "the waves' designs come with side 1's, for drawing");
    let idle = sim::TickInputs::default();
    let mut most = 0;
    loop {
        setup.sim.step(&idle);
        let ended = setup.referee.observe(&mut setup.sim);
        most = most.max(setup.sim.ships().iter().filter(|s| s.owner() == arena::OWNERS[1]).count());
        if let Some(end) = ended {
            assert_eq!(end, r, "stepped by hand: the same fight");
            break;
        }
    }
    assert!(r.waves >= 1, "{r:?}");
    if r.outcome == Outcome::Won(0) {
        assert_eq!((r.waves, most), (3, 4), "all three waves came and went");
    }
    assert_eq!(run_fight(&c, &spec).unwrap(), r, "deterministic");
    let code = FightCode::new(&c, arena::rules_fingerprint(&c), spec.clone(), r);
    let back = FightCode::decode(&code.encode()).unwrap();
    assert_eq!((back.spec, back.record), (spec.clone(), r));
    // A plain fight's spec and result read and write as before: no `waves`.
    let plain = FightSpec::new(1, vec![Entry::ship("rim_skiff")], vec![Entry::ship("automaton_raider")]);
    assert!(!plain.to_ron().contains("waves"));
    // An empty wave is refused, named as a wave.
    spec.waves.push(Side { entries: Vec::new() });
    let e = run_fight(&c, &spec).unwrap_err().to_string();
    assert!(e.contains("reinforcement wave 3"), "{e}");
}

/// RE-SCORE: a stale run as a new run under today's rules: the archive's elites fought again, the archive rebuilt; the
/// same result stepped or on threads, and from a run file written in the middle of it; it then goes on to its end, and
/// its old generations' codes keep the rules they were fought under.
#[test]
fn rescore_is_deterministic_and_goes_on() {
    let c = data::base_content();
    let text = r#"(seed: 5, evolve: Both, generations: 3, population: 4,
        trials: [(kind: EnemyShips(opponents: [(design: Ship("automaton_raider"))]), seeds: 2, time_limit: 10),
                 (kind: HallOfFame(count: 1), seeds: 1, time_limit: 10)],
        goals: (win: 1, unhurt: 1),
        starts: [Design(design: Ship("rim_skiff"))])"#;
    let mut run = Run::new(&c, config(text)).unwrap();
    while run.generation() < 2 {
        run.step(u64::MAX);
    }
    let save = run.snapshot();
    let mut changed = c.clone();
    let thruster = changed.parts.get_mut("thruster").unwrap();
    thruster.mass = Some(thruster.mass.unwrap_or(1.0) * 3.0);
    assert!(Run::rescored(&c, &save).is_err(), "nothing to re-score under the same rules");
    let a = Run::rescored(&changed, &save).unwrap();
    assert!(!a.stale() && a.rules() == arena::rules_fingerprint(&changed));
    assert_eq!(a.progress().generations, 4, "the re-score takes none of the run's generations");
    let a = {
        let mut a = a;
        while a.generation() < 3 {
            a.step(997);
        }
        a
    };
    let mut b = Run::rescored(&changed, &save).unwrap();
    let jobs = b.jobs().to_vec();
    b.submit(jobs.iter().map(|j| j.run(&changed)).collect());
    assert_eq!(a.snapshot().to_ron(), b.snapshot().to_ron(), "stepped or on threads: the same re-score");
    let h = &a.state().history[2];
    assert!(h.rescore && h.evaluated == run.archive().cells().len() as u32 - h.rejected);
    assert!(a.archive().cells().values().all(|e| e.generation <= 1), "the archive holds the old elites, re-scored");
    // Interrupted in the middle of the re-score: a run file, a restore, the same.
    let mut mid = Run::rescored(&changed, &save).unwrap();
    mid.step(500);
    let mut resumed = Run::restore(&changed, &RunSave::from_ron(&mid.snapshot().to_ron()).unwrap()).unwrap();
    while resumed.generation() < 3 {
        resumed.step(u64::MAX);
    }
    assert_eq!(resumed.snapshot().to_ron(), a.snapshot().to_ron());
    // In parts (the lab's) the same: the head keeps the re-score that is due.
    let mut from_parts = Run::restore_parts(&changed, &mid.parts()).unwrap();
    assert_eq!(from_parts.snapshot(), mid.snapshot());
    while from_parts.generation() < 3 {
        from_parts.step(u64::MAX);
    }
    assert_eq!(from_parts.snapshot().to_ron(), a.snapshot().to_ron());
    // The old generations' codes carry their rules; the re-score's today's, and they replay as recorded.
    assert_eq!(a.fight_code(0, 0, 0).unwrap().rules, save.rules);
    let code = a.fight_code(2, 0, 0).unwrap();
    assert_eq!(code.rules, arena::rules_fingerprint(&changed));
    assert_eq!(run_fight(&changed, &code.spec).unwrap(), code.record);
    let elite = a.archive().best().unwrap();
    assert_eq!(a.scored(elite.id).unwrap().0, 2, "an elite was last scored by the re-score");
    // It goes on to its end.
    let done = stepped_from(&changed, a);
    assert_eq!((done.generation(), done.state().history.iter().filter(|h| h.rescore).count()), (4, 1));
}

fn stepped_from(c: &Content, mut run: Run) -> Run {
    let _ = c;
    while !run.step(u64::MAX).finished {}
    run
}

/// VALIDATE: the best designs on new seeds and their recorded fights again; deterministic, kept in the run file, and
/// under the same rules every recorded fight ends as recorded.
#[test]
fn validate_on_new_seeds_and_the_records() {
    let c = data::base_content();
    let run = stepped(&c, config(MIXED), u64::MAX);
    let best: Vec<u32> = run.results().elites.iter().take(2).map(|e| e.id).collect();
    let mut v = run.validation(&best, 2).unwrap();
    assert_eq!(v.genomes(), best);
    // The new jobs fight seeds after each trial's own (1): 2 and 3.
    let seeds: Vec<u64> = v.batch.jobs().iter().filter_map(|j| match j { Job::Fight { spec, .. } => Some(spec.seed), _ => None }).collect();
    assert!(seeds.contains(&2) && seeds.contains(&3));
    while !v.batch.step(4000) {}
    let mut on_threads = run.validation(&best, 2).unwrap();
    let results = on_threads.batch.jobs().iter().map(|j| j.run(&c)).collect();
    on_threads.batch.submit(results);
    let (mut a, mut b) = (run, stepped(&c, config(MIXED), u64::MAX));
    a.validated_by(v);
    b.validated_by(on_threads);
    for id in &best {
        let x = a.validated(*id).unwrap();
        assert_eq!(Some(x), b.validated(*id));
        assert_eq!(x.as_recorded.0, x.as_recorded.1, "the same rules: every fight as recorded");
        assert!((0.0..=1.0).contains(&x.score) && x.trials.len() == 9);
    }
    let file = a.snapshot().to_ron();
    let back = Run::restore(&c, &RunSave::from_ron(&file).unwrap()).unwrap();
    assert_eq!(back.validated(best[0]), a.validated(best[0]), "kept in the run file");
    assert_eq!(back.snapshot().to_ron(), file, "IMPORT RUN: a run file reads and writes back the same");
    let parts = arena::evolve::RunParts::of(&back.snapshot()).unwrap();
    assert_eq!(Run::restore_parts(&c, &parts).unwrap().snapshot().to_ron(), file, "VALIDATE's results in the head");
}

/// The lab's presets become trials with the career's open voyages, the player's designs and earlier runs' best; a
/// preset with nothing to fight is refused in words.
#[test]
fn the_labs_presets_become_trials() {
    let c = data::base_content();
    let src = Sources {
        open_voyages: vec!["yard_approach".into()],
        fleet: vec![Entry::ship("sample_early")],
        champions: vec![Entry::ship("sample_mid"), Entry::ship("halvex_runner")],
        escorts: vec![Entry::ship("rim_skiff")],
    };
    let mut trials = Vec::new();
    for p in &c.lab.trials {
        let t = TrialSpec::from_preset(p, p.weight, p.seeds, p.id == "beat_your_start", &src).unwrap();
        assert_eq!(t.preset.as_deref(), Some(p.id.as_str()));
        assert_eq!(t.kind.kind_id(), p.kind.id(), "the arena's kind is the preset's, by lab.ron's id");
        trials.push(t);
    }
    let by = |id: &str| trials.iter().find(|t| t.preset.as_deref() == Some(id)).unwrap();
    assert_eq!(by("enemies_you_meet").kind, Trial::Encounters { voyages: vec!["yard_approach".into()] });
    assert_eq!(by("enemies_you_meet").weight, 2.0);
    assert_eq!(by("beat_your_start").escorts, src.escorts, "WITH YOUR ESCORTS");
    assert!(matches!(&by("champions").kind, Trial::Champions { designs } if designs.len() == 2));
    assert!(matches!(&by("voyages").kind, Trial::Voyages { minutes, .. } if *minutes == 10.0));
    // The encounters of the first voyage: a gunner pair, the derelict field's raider (its wrecks aside) and the
    // raiding party at its goal.
    let enc = arena::evolve::encounters(&c, &["yard_approach".into()]).unwrap();
    assert_eq!(
        enc.iter().map(|e| e.0.as_str()).collect::<Vec<_>>(),
        ["gunner_pair", "derelict_field", "raiding_party"]
    );
    let none = Sources::default();
    let p = c.lab.trial("your_fleet").unwrap();
    assert!(TrialSpec::from_preset(p, 1.0, 2, false, &none).unwrap_err().contains("no saved design"));
    // Every preset's trial builds a run (the ARMS RACE with its enemy).
    let race = c.lab.trial("arms_race").unwrap();
    let cfg = RunConfig {
        trials: trials.into_iter().map(|t| TrialSpec { seeds: 1, ..t }).collect(),
        enemy: Some(arena::evolve::Population::from_preset(&c, race, &arena::evolve::enemy_starts(&c), 2).unwrap()),
        ..config(r#"(seed: 1, evolve: Both, generations: 1, population: 2, starts: [Design(design: Ship("rim_skiff"))])"#)
    };
    Run::new(&c, cfg).unwrap();
    // Every kind and measure the arena has is named in lab.ron's ids, and the lab's design spread is the arena's own
    // default (the CLI's runs spread as the game's).
    assert_eq!(arena::evolve::Measure::ALL.map(|m| m.id()), data::MEASURES);
    let spread = c.lab.spread("design").unwrap();
    let axes: Vec<(&str, f32, f32, u32)> = arena::evolve::ArchiveShape::default().axes.iter().map(|a| (a.measure.id(), a.min, a.max, a.cells)).collect();
    assert_eq!(axes, spread.axes.iter().map(|a| (a.measure.as_str(), a.min, a.max, a.cells)).collect::<Vec<_>>());
}
