//! What a run keeps and how it goes on (GDR-0092 steps 2 and 3): resume after a snapshot, budgeted slices, the job
//! records as fight codes, the genome table, the locks, the extended pre-filter, several starts, stale runs.

use std::collections::BTreeSet;

use arena::evolve::{Bound, Job, JobResult, Locks, RunHead, RunPart, RunParts, Start};
use arena::{FightCode, Run, RunConfig, RunError, RunSave, run_fight};
use data::Content;

/// Design and behavior from two starts under the mirror lock, with a hall of fame: every kind of record.
const BOTH: &str = r#"(
    seed: 9,
    evolve: Both,
    generations: 6,
    population: 6,
    crossover: 0.4,
    trials: [(kind: EnemyShips(opponents: [(design: Ship("automaton_raider"))], hall_of_fame: 1), seeds: 2, time_limit: 8)],
    limits: (mirror: true),
    starts: [
        Design(design: Ship("rim_skiff"), locks: (categories: ["weapons"], parts: [0], rules: [0], order: true)),
        Design(design: Ship("halvex_runner")),
    ],
)"#;

fn config(text: &str) -> RunConfig {
    RunConfig::from_ron(text).unwrap()
}

/// Runs to the end with `step(budget)`.
fn stepped(c: &Content, cfg: RunConfig, budget: u64) -> Run {
    let mut run = Run::new(c, cfg).unwrap();
    while !run.step(budget).finished {}
    run
}

/// The run as a document's text (what a file holds).
fn text(run: &Run) -> String {
    run.snapshot().to_ron()
}

#[test]
fn a_resumed_run_is_the_uninterrupted_run() {
    let c = data::base_content();
    let straight = stepped(&c, config(BOTH), u64::MAX);
    // Three generations, a snapshot written and read back, a restore, three more.
    let mut first = Run::new(&c, config(BOTH)).unwrap();
    while first.generation() < 3 {
        first.step(u64::MAX);
    }
    let file = first.snapshot().to_ron();
    let mut resumed = Run::restore(&c, &RunSave::from_ron(&file).unwrap()).unwrap();
    assert!(!resumed.stale());
    assert_eq!(resumed.state().history, &straight.state().history[..3]);
    assert_eq!(text(&resumed), file, "a restored run snapshots to the same document");
    while !resumed.step(u64::MAX).finished {}
    assert_eq!(resumed.results(), straight.results());
    assert_eq!(text(&resumed), text(&straight), "byte for byte");
    // Interrupted in the middle of a generation (part bred, part fought): the snapshot is the state before it, and
    // the restored run breeds it again from the same random state.
    let mut mid = Run::new(&c, config(BOTH)).unwrap();
    while mid.generation() < 2 {
        mid.step(u64::MAX);
    }
    mid.step(arena::evolve::BREED_COST * 3);
    mid.step(500);
    let save = mid.snapshot();
    assert_eq!(save.history.len(), 2);
    let mut resumed = Run::restore(&c, &save).unwrap();
    while !resumed.step(777).finished {}
    assert_eq!(text(&resumed), text(&straight));
    // A damaged or newer file is refused with a reason.
    let mut bad = save.clone();
    bad.payload.truncate(bad.payload.len() / 2);
    assert!(matches!(Run::restore(&c, &bad), Err(RunError::Save(_))));
    assert!(matches!(RunSave::from_ron(&file.replace("format: 1", "format: 9")), Err(RunError::Save(_))));
    assert!(matches!(RunSave::from_ron("(format: 1"), Err(RunError::Save(_))));
}

/// A run's parts through their text, as a store keeps them.
fn reread(parts: &RunParts) -> RunParts {
    RunParts {
        head: RunHead::from_ron(&parts.head.to_ron()).unwrap(),
        config: RunConfig::from_ron(&parts.config.to_ron()).unwrap(),
        generations: parts.generations.iter().map(|p| RunPart::from_ron(&p.to_ron()).unwrap()).collect(),
    }
}

/// The run in parts (the game's lab, ADR-0041): written as it grows (the head again after every generation, each
/// generation's part once), read back through text, it restores to the run its file restores to, byte for byte, and
/// goes on to the same end; a run file splits into the same parts, at any point, also in the middle of a generation
/// and before the first one closed.
#[test]
fn a_run_in_parts_is_its_run_file() {
    let c = data::base_content();
    let straight = stepped(&c, config(BOTH), u64::MAX);
    let mut run = Run::new(&c, config(BOTH)).unwrap();
    // Before any generation: the starts are in the head.
    let fresh = reread(&run.parts());
    assert!(fresh.generations.is_empty());
    assert_eq!(text(&Run::restore_parts(&c, &fresh).unwrap()), text(&run));
    let mut kept: Vec<RunPart> = Vec::new();
    let mut head = run.head();
    while run.generation() < 4 {
        let g = run.generation();
        while run.generation() == g {
            run.step(1500);
        }
        kept.push(run.part(g).unwrap());
        head = run.head();
        assert!(run.part(g + 1).is_none(), "only closed generations have parts");
    }
    // The middle of the next generation: the parts stay those of the closed ones.
    run.step(arena::evolve::BREED_COST * 2);
    assert_eq!(run.head(), head);
    let parts = RunParts { head, config: config(BOTH), generations: kept };
    assert_eq!(parts, run.parts(), "written as it grew: the run's parts");
    assert_eq!(RunParts::of(&run.snapshot()).unwrap(), parts, "a run file splits into the same parts");
    let file = text(&run);
    let mut resumed = Run::restore_parts(&c, &reread(&parts)).unwrap();
    assert_eq!(text(&resumed), file, "restored from its parts: the run file's run");
    while !resumed.step(777).finished {}
    assert_eq!(text(&resumed), text(&straight), "and it goes on byte for byte");
    // Each part is about its generation's bytes; the head stays small however long the run is.
    let all = RunParts::of(&straight.snapshot()).unwrap();
    let head = all.head.to_ron().len();
    assert!(head < 1200 && head < all.generations[1].to_ron().len(), "a head of {head} bytes");
    // A missing or misplaced part is refused with a reason.
    let mut short = parts.clone();
    short.generations.pop();
    assert!(matches!(Run::restore_parts(&c, &short), Err(RunError::Save(_))));
    let mut swapped = parts.clone();
    swapped.generations.swap(1, 2);
    assert!(matches!(Run::restore_parts(&c, &swapped), Err(RunError::Save(_))));
    assert!(matches!(RunHead::from_ron(&parts.head.to_ron().replace("format: 1", "format: 9")), Err(RunError::Save(_))));
    // A genome rebuilds from the parts as from the file.
    let best = straight.archive().best().unwrap().id;
    assert_eq!(all.genome(&c, best).unwrap(), straight.snapshot().genome(&c, best).unwrap());
}

#[test]
fn any_slicing_gives_the_same_run() {
    let c = data::base_content();
    let whole = text(&stepped(&c, config(BOTH), u64::MAX));
    for budget in [1, arena::evolve::BREED_COST - 1, 97, 5000] {
        assert_eq!(text(&stepped(&c, config(BOTH), budget)), whole, "slices of {budget} ticks");
    }
    // The CLI's way: every generation's jobs at once, run anywhere, the results handed back in order.
    let mut run = Run::new(&c, config(BOTH)).unwrap();
    while !run.finished() {
        let jobs = run.jobs().to_vec();
        let results: Vec<JobResult> = jobs.iter().rev().map(|j| j.run(&c)).collect();
        run.submit(results.into_iter().rev().collect());
    }
    assert_eq!(text(&run), whole);
    // The threads' way in a client: each generation bred in slices of its frame budget, then its jobs handed out.
    let mut run = Run::new(&c, config(BOTH)).unwrap();
    while !run.finished() {
        let mut slices = 0;
        while !run.breed(arena::evolve::BREED_COST * 2) {
            slices += 1;
        }
        assert!(slices >= 1, "a generation of six breeds in more than one slice of two");
        let jobs = run.jobs().to_vec();
        let results: Vec<JobResult> = jobs.iter().map(|j| j.run(&c)).collect();
        run.submit(results);
    }
    assert_eq!(text(&run), whole);
    // Breeding is budgeted: a small slice breeds a child or two and fights nothing yet.
    let mut run = Run::new(&c, config(BOTH)).unwrap();
    let p = run.step(arena::evolve::BREED_COST * 2);
    assert_eq!((p.generation, run.table().len()), (0, 4), "the two starts and two children");
    run.step(1);
    assert_eq!(run.table().len(), 5);
    assert!(run.state().records.is_empty());
}

/// Every job of every child is kept: rebuilt, it is the job that ran, and its code replays to the recorded end.
#[test]
fn every_recorded_fight_replays_as_recorded() {
    let c = data::base_content();
    let mut run = Run::new(&c, config(BOTH)).unwrap();
    let mut ran = Vec::new();
    while !run.finished() {
        let jobs = run.jobs().to_vec();
        let results: Vec<JobResult> = jobs.iter().map(|j| j.run(&c)).collect();
        ran.push((jobs, results.clone()));
        run.submit(results);
    }
    let mut fights = 0;
    for (g, (jobs, results)) in ran.iter().enumerate() {
        let record = &run.state().records[g];
        let mut k = 0;
        for (child, r) in record.children.iter().enumerate() {
            for j in 0..r.jobs.len() {
                let (job, rec) = run.job(g as u32, child, j).unwrap();
                assert_eq!(job, jobs[k], "generation {g}, child {child}, job {j}");
                assert_eq!((rec.score, rec.ticks, rec.end), (results[k].score, results[k].ticks, results[k].end));
                let code = run.fight_code(g as u32, child, j).unwrap();
                let back = FightCode::decode(&code.encode()).unwrap();
                assert_eq!(back.rules, run.rules());
                let Job::Fight { spec, .. } = &job else { unreachable!() };
                assert_eq!(&back.spec, spec);
                assert_eq!(run_fight(&c, &back.spec).unwrap(), back.record, "replays as recorded");
                assert_eq!(back.record.ticks, rec.ticks);
                assert_eq!(back.record.sides[0].score, rec.score);
                k += 1;
                fights += 1;
            }
        }
        assert_eq!(k, jobs.len());
    }
    assert!(fights > 100);
    // Each fight records the rules that decided in it.
    let decided = run.state().records.iter().flat_map(|g| &g.children).flat_map(|c| &c.jobs);
    assert!(decided.filter(|j| matches!(j.end, arena::evolve::JobEnd::Fight { decided, .. } if decided != 0)).count() > 100);
}

/// The genome table rebuilds every genome bred: the design and behavior each child fought with.
#[test]
fn the_lineage_rebuilds_every_genome() {
    let c = data::base_content();
    let mut run = Run::new(&c, config(BOTH)).unwrap();
    let mut seen = 0;
    while !run.finished() {
        let jobs = run.jobs().to_vec();
        let results: Vec<JobResult> = jobs.iter().map(|j| j.run(&c)).collect();
        run.submit(results);
        let g = run.generation() - 1;
        for (k, child) in run.state().records[g as usize].children.iter().enumerate() {
            let genome = run.genome(child.genome).unwrap();
            let Job::Fight { spec, .. } = &jobs.iter().find(|j| j.candidate() == k as u32).unwrap() else {
                unreachable!()
            };
            let me = &spec.sides[0].entries[0];
            assert_eq!(me.design, arena::DesignRef::Blueprint(genome.blueprint_text()), "genome {}", child.genome);
            assert_eq!(me.behavior, Some(arena::BehaviorRef::Text(genome.behavior.clone())));
            let line = run.lineage(child.genome);
            assert_eq!(line.last(), Some(&child.genome));
            assert!(line[0] < 2 && run.table()[line[0] as usize].parent.is_none(), "it goes back to a start");
            assert_eq!(run.table()[line[0] as usize].root, genome.root);
            seen += 1;
        }
    }
    assert_eq!(seen, run.table().len(), "every genome bred was a child");
    for e in run.archive().cells().values() {
        assert_eq!(run.genome(e.id).as_ref(), Some(&e.genome), "elite {}", e.id);
        assert_eq!(run.scored(e.id).map(|(g, _)| g), Some(e.generation));
    }
    // Generation 0: both starts, then mutants of each in turn.
    let roots: Vec<u32> = run.state().records[0]
        .children
        .iter()
        .map(|c| run.table()[c.genome as usize].root)
        .collect();
    assert_eq!(roots, [0, 1, 0, 1, 0, 1]);
}

/// Locked parts keep their frame cells through every generation (genomes are opened placed, never re-centred), KEEP
/// EMPTY cells stay empty, locked rules stay in order, and limits taken from the start hold.
#[test]
fn locks_and_limits_hold_across_generations() {
    let c = data::base_content();
    let text = r#"(
        seed: 4,
        evolve: Both,
        generations: 8,
        population: 8,
        crossover: 0.5,
        trials: [(kind: EnemyShips(opponents: [(design: Ship("automaton_raider"))]), seeds: 1, time_limit: 6)],
        limits: (frame: "frigate"),
        starts: [Design(design: Ship("rim_skiff"), behavior: Content("skirmisher"),
            locks: (parts: [0, 1], categories: ["weapons"], cells: [(-1, 0), (-1, 1)], rules: [0, 3], target: true, order: true))],
    )"#;
    let run = stepped(&c, config(text), u64::MAX);
    let start = &run.starts()[0];
    let skiff = c.ships.get("rim_skiff").unwrap();
    let frame_start = start.placed(run.frame(0).unwrap());
    let weapons: Vec<usize> = (0..skiff.parts.len())
        .filter(|&i| c.parts.get(&skiff.parts[i].part).unwrap().category == "weapons")
        .collect();
    let locked: Vec<&data::Placement> = [0, 1].into_iter().chain(weapons).map(|i| &frame_start.parts[i]).collect();
    // The KEEP EMPTY cells in frame coordinates: the start's cells moved as it was centred.
    let shift = (frame_start.parts[0].x - skiff.parts[0].x, frame_start.parts[0].y - skiff.parts[0].y);
    let empty: BTreeSet<(i32, i32)> = [(-1, 0), (-1, 1)].map(|(x, y)| (x + shift.0, y + shift.1)).into();
    let start_rules = data::behavior::Program::parse(&start.behavior).unwrap();
    let (cost, hold) = (report(&c, &start.blueprint).build_points, c.ship_hold(&start.blueprint));
    let mut changed = 0;
    for id in 0..run.table().len() as u32 {
        let g = run.genome(id).unwrap();
        let placed = g.placed(run.frame(g.root).unwrap());
        for p in &locked {
            assert!(placed.parts.contains(p), "genome {id} lost or moved {p:?}");
        }
        for p in &placed.parts {
            for cell in c.placed_shape(p).unwrap().cells() {
                assert!(!empty.contains(&cell), "genome {id} covers a KEEP EMPTY cell");
            }
        }
        let rules = data::behavior::Program::parse(&g.behavior).unwrap();
        let k0 = rules.rules.iter().position(|r| *r == start_rules.rules[0]).expect("locked rule 0");
        let k3 = rules.rules.iter().position(|r| *r == start_rules.rules[3]).expect("locked rule 3");
        assert!(k0 < k3, "LOCK ORDER");
        assert_eq!(rules.target, start_rules.target, "the locked target line");
        assert!(rules.rules.len() <= c.ship_rule_slots(&g.blueprint) as usize, "its own rule slots");
        // The defaults: the start's build points and hold.
        assert!(report(&c, &g.blueprint).build_points <= cost);
        assert!(c.ship_hold(&g.blueprint) >= hold);
        changed += (g.blueprint.parts != start.blueprint.parts) as u32;
    }
    assert!(changed > 10, "the unlocked rest evolves ({changed} genomes changed)");
}

fn report(c: &Content, bp: &data::Blueprint) -> sim::DesignReport {
    sim::design_report(c, bp, &sim::SimConfig::default()).unwrap()
}

/// The pre-filter checks the genome's own behavior against its own rule slots (not its faction's), the mind
/// capacity and the hold.
#[test]
fn the_prefilter_knows_cores_slots_and_cargo() {
    let c = data::base_content();
    let skiff = c.ships.get("rim_skiff").unwrap();
    let cores = c.ship_totals(skiff).logic_cores;
    let slots = c.ship_rule_slots(skiff);
    let hold = c.ship_hold(skiff);
    assert!(cores >= 1 && slots >= 1 && hold >= 1);
    let base = |limits: &str, behavior: &str| {
        format!(
            r#"(seed: 1, evolve: Both, generations: 3, population: 6,
                trials: [(kind: EnemyShips(opponents: [(design: Ship("automaton_raider"))]), seeds: 1, time_limit: 5)],
                limits: {limits},
                starts: [Design(design: Ship("rim_skiff"){behavior})])"#
        )
    };
    let err = |text: String| Run::new(&c, config(&text)).err();
    // Over the mind capacity: the start itself is refused, and in a run no child gets a core more.
    assert!(matches!(err(base(&format!("(max_cores: {})", cores - 1), "")), Some(RunError::Start(_))));
    let run = stepped(&c, config(&base(&format!("(max_cores: {cores}, build_points: Off)"), "")), u64::MAX);
    for id in 0..run.table().len() as u32 {
        assert!(c.ship_totals(&run.genome(id).unwrap().blueprint).logic_cores <= cores);
    }
    // More rules than its slots: refused, though its faction's default fits.
    let long: Vec<String> = (0..=slots).map(|k| format!("when hull < {}% -> flee", 10 + k)).collect();
    let behavior = format!(r#", behavior: Text("{}")"#, long.join("\\n"));
    assert!(matches!(err(base("()", &behavior)), Some(RunError::Start(_))));
    // A hold below the limit: refused; with the hold switched off a run may trade the cargo away.
    assert!(matches!(err(base(&format!("(min_hold: At({}))", hold + 1), "")), Some(RunError::Start(_))));
    let cfg = config(&base("(min_hold: Off, build_points: Off)", ""));
    assert_eq!(cfg.limits.min_hold, Bound::Off);
    assert!(Run::new(&c, cfg).is_ok());
    // Part counts: the start holds more than allowed.
    let part = &skiff.parts[0].part;
    let n = skiff.parts.iter().filter(|p| p.part == *part).count();
    assert!(matches!(
        err(base(&format!(r#"(counts: {{"{part}": {}}})"#, n - 1), "")),
        Some(RunError::Start(_))
    ));
    // A number reads as `At` and writes back the same.
    let at = config(&base("(build_points: At(60))", ""));
    assert_eq!(at.limits.build_points, Bound::At(60.0));
    assert_eq!(RunConfig::from_ron(&at.to_ron()).unwrap(), at);
    // Starts: one to four, each lock in range.
    let five = BOTH.replace(
        r#"Design(design: Ship("halvex_runner")),"#,
        &r#"Design(design: Ship("halvex_runner")),"#.repeat(4),
    );
    assert!(matches!(Run::new(&c, config(&five)), Err(RunError::Config(_))));
    let mut cfg = config(BOTH);
    if let Start::Design { locks, .. } = &mut cfg.starts[0] {
        *locks = Locks { rules: vec![99], ..Locks::default() };
    }
    assert!(matches!(Run::new(&c, cfg), Err(RunError::Start(_))));
}

#[test]
fn a_changed_rule_makes_a_run_stale() {
    let c = data::base_content();
    let mut run = Run::new(&c, config(BOTH)).unwrap();
    run.step(u64::MAX);
    let save = run.snapshot();
    assert_eq!(save.rules, arena::rules_fingerprint(&c));
    assert!(!Run::restore(&c, &save).unwrap().stale());
    let mut changed = c.clone();
    let thruster = changed.parts.get_mut("thruster").unwrap();
    thruster.mass = Some(thruster.mass.unwrap_or(1.0) * 3.0);
    let mut stale = Run::restore(&changed, &save).unwrap();
    assert!(stale.stale());
    assert_eq!(stale.rules(), save.rules, "it keeps the rules it was fought under");
    // Read-only: it doesn't step, and its codes say which rules they were fought under.
    let p = stale.step(u64::MAX);
    assert!(p.stale && p.generation == 1);
    assert!(stale.jobs().is_empty());
    let code = stale.fight_code(0, 0, 0).unwrap();
    assert_ne!(code.rules, arena::rules_fingerprint(&changed));
}

/// The head's name, creation time and status are the game's (GDR-0092 step 4): a snapshot leaves them out, so a run
/// file reads as before; set, they round-trip and the run restores the same. `worked` counts the step budget used,
/// and the open generation's share done grows to the close.
#[test]
fn the_head_keeps_name_time_and_status() {
    let c = data::base_content();
    let mut run = Run::new(&c, config(BOTH)).unwrap();
    assert_eq!((run.worked(), run.generation_done()), (0, 0.0));
    let slice = arena::evolve::BREED_COST * 4 + 1200;
    run.step(slice);
    assert_eq!(run.worked(), slice, "a slice of a generation uses all of its budget");
    let mid = run.generation_done();
    assert!(mid > 0.0 && mid < 1.0, "{mid}");
    while run.generation() < 1 {
        run.step(400);
    }
    assert_eq!(run.generation_done(), 0.0, "closed");
    let plain = run.snapshot();
    assert_eq!((plain.name.as_deref(), plain.created, plain.status), (None, None, None));
    let file = plain.to_ron();
    assert!(!file.contains("name:") && !file.contains("created:") && !file.contains("status:"), "{}", &file[..200]);
    let named = RunSave { name: Some("Evolving the skiff".into()), created: Some(1_760_000_000), status: Some(arena::evolve::RunStatus::Paused), ..plain.clone() };
    let back = RunSave::from_ron(&named.to_ron()).unwrap();
    assert_eq!(back, named);
    assert!(named.to_ron().starts_with("(\n    format: 1,\n    name: \"Evolving the skiff\",\n    created: 1760000000,\n    status: Paused,\n"));
    assert_eq!(text(&Run::restore(&c, &back).unwrap()), file);
}

/// GDR-0092 step 6, `runs/everything.ron` from a start with pilot assists (one locked): the design, the behavior and
/// the assists all evolve, the locked assist stays, the genome table rebuilds the assists every child fought with, the
/// stand-in flies the even seeds, and the run resumes as it was.
#[test]
fn everything_mutates_and_the_stand_in_flies_the_even_seeds() {
    let c = data::base_content();
    let mut cfg = config(include_str!("../runs/everything.ron"));
    let mut bp = c.ships.get("rim_skiff").unwrap().clone();
    let brake = "when idle(thrust) >= 1s -> brake";
    bp.assists = Some(data::AssistList { rules: vec!["always -> auto_lock(nearest)".into(), brake.into()] });
    let design = arena::DesignRef::Blueprint(data::blueprint_to_ron(&bp));
    cfg.starts = vec![Start::Design { design, behavior: None, locks: Locks { assists: vec![1], ..Locks::default() } }];
    let mut run = Run::new(&c, cfg.clone()).unwrap();
    while !run.finished() {
        let jobs = run.jobs().to_vec();
        for j in &jobs {
            let Job::Fight { spec, .. } = j else { unreachable!() };
            assert_eq!(spec.sides[0].entries[0].pilot.is_some(), spec.seed % 2 == 0, "FLOWN BY BOTH");
        }
        let results: Vec<JobResult> = jobs.iter().map(|j| j.run(&c)).collect();
        run.submit(results);
        let g = run.generation() - 1;
        for (k, child) in run.state().records[g as usize].children.iter().enumerate() {
            let genome = run.genome(child.genome).unwrap();
            let Job::Fight { spec, .. } = jobs.iter().find(|j| j.candidate() == k as u32).unwrap() else { unreachable!() };
            assert_eq!(spec.sides[0].entries[0].design, arena::DesignRef::Blueprint(genome.blueprint_text()), "genome {}", child.genome);
        }
    }
    let start = &run.starts()[0];
    let rules = |g: &arena::Genome| data::behavior::Program::parse(&g.assists_text()).unwrap().rules;
    let locked = &rules(start)[1];
    let mut changed = [0; 3];
    for id in 0..run.table().len() as u32 {
        let g = run.genome(id).unwrap();
        assert!(rules(&g).contains(locked), "genome {id} lost its locked assist: {}", g.assists_text());
        changed[0] += (g.blueprint.parts != start.blueprint.parts) as u32;
        changed[1] += (g.behavior != start.behavior) as u32;
        changed[2] += (g.blueprint.assists != start.blueprint.assists) as u32;
    }
    assert!(changed.iter().all(|&n| n > 3), "design, behavior and assists all evolve: {changed:?}");
    // Resumed after two generations: the same run.
    let mut first = Run::new(&c, cfg.clone()).unwrap();
    while first.generation() < 2 {
        first.step(u64::MAX);
    }
    let mut resumed = Run::restore(&c, &RunSave::from_ron(&first.snapshot().to_ron()).unwrap()).unwrap();
    while !resumed.step(5_000).finished {}
    assert_eq!(text(&resumed), text(&run), "byte for byte");
}

/// Every run file of `runs/` reads and builds against the base content.
#[test]
fn every_run_file_reads_and_builds() {
    let c = data::base_content();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/runs");
    let mut n = 0;
    for f in std::fs::read_dir(dir).unwrap() {
        let path = f.unwrap().path();
        let cfg = RunConfig::from_ron(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        Run::new(&c, cfg).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        n += 1;
    }
    assert!(n >= 7);
}
