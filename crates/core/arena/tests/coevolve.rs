//! The ARMS RACE (GDR-0092 step 7, "Co-evolution"): an enemy population evolves against yours in the same run, each
//! scored against the other's frozen best and past champions; one genome table, one run file, every fight kept, the
//! PAST CHAMPIONS check, saving and exporting evolved enemies.

use arena::evolve::{Camp, Job, JobResult, Population, RunParts, Trial, TrialSpec};
use arena::{FightCode, Run, RunConfig, RunError, RunSave, run_fight};
use data::Content;

/// The starter against the raider and the hunter evolving beside it: every kind of record, a check every second
/// generation, short fights.
const RACE: &str = r#"(
    seed: 3,
    evolve: MayChange(()),
    generations: 5,
    population: 4,
    trials: [
        (kind: EnemyShips(opponents: [(design: Ship("automaton_raider"))]), seeds: 1, time_limit: 8),
        (kind: CoEvolution, seeds: 2, time_limit: 8),
    ],
    starts: [Design(design: Ship("rim_skiff"))],
    enemy: (
        starts: [Design(design: Ship("automaton_raider")), Design(design: Ship("automaton_hunter"))],
        population: 3,
        palette: ["shield_emitter"],
        mirror: true,
        elites: 2,
        hall: 1,
        check_every: 2,
        champions_every: 1,
    ),
)"#;

fn config() -> RunConfig {
    RunConfig::from_ron(RACE).unwrap()
}

fn text(run: &Run) -> String {
    run.snapshot().to_ron()
}

fn stepped(c: &Content, budget: u64) -> Run {
    let mut run = Run::new(c, config()).unwrap();
    while !run.step(budget).finished {}
    run
}

/// Both populations breed in one table, each on its own stream, and the run is the same however it is computed and
/// wherever it is interrupted: slices, the jobs handed out, a run file in the middle of a generation, its parts.
#[test]
fn an_arms_race_is_one_run_however_it_is_computed() {
    let c = data::base_content();
    let straight = stepped(&c, u64::MAX);
    let whole = text(&straight);
    assert_eq!(text(&stepped(&c, 97)), whole, "slices of 97 ticks");
    let mut run = Run::new(&c, config()).unwrap();
    while !run.finished() {
        let jobs = run.jobs().to_vec();
        let results: Vec<JobResult> = jobs.iter().rev().map(|j| j.run(&c)).collect();
        run.submit(results.into_iter().rev().collect());
    }
    assert_eq!(text(&run), whole, "the jobs handed out");
    let mut mid = Run::new(&c, config()).unwrap();
    while mid.generation() < 2 {
        mid.step(u64::MAX);
    }
    // Into the enemy's breeding: yours bred, part of its children too.
    mid.step(arena::evolve::BREED_COST * 6);
    let save = RunSave::from_ron(&mid.snapshot().to_ron()).unwrap();
    let parts = RunParts::of(&save).unwrap();
    for mut resumed in [Run::restore(&c, &save).unwrap(), Run::restore_parts(&c, &parts).unwrap()] {
        while !resumed.step(777).finished {}
        assert_eq!(text(&resumed), whole, "resumed byte for byte");
    }
    // Yours breed as they would: the enemy's draws are on its own stream.
    let n = straight.starts().len() as u32;
    assert_eq!(straight.enemy_starts().len(), 2);
    assert!(straight.enemy_starts().iter().enumerate().all(|(k, g)| g.root == n + k as u32));
    let table = straight.table();
    assert_eq!((table[1].root, table[2].root), (1, 2), "the enemy's starts follow yours");
    for (id, b) in table.iter().enumerate() {
        let camp = straight.camp(id as u32).unwrap();
        assert_eq!(camp == Camp::Enemy, b.root >= n, "genome {id}");
        let g = straight.genome(id as u32).unwrap();
        assert_eq!(g.blueprint.tags.iter().any(|t| t == "enemy"), camp == Camp::Enemy);
        if camp == Camp::Enemy {
            assert_eq!(g.blueprint.faction, "automatons");
            let start = straight.start(b.root).unwrap();
            assert_eq!(g.blueprint.assists, start.blueprint.assists, "enemies are never piloted: their assists stay");
            let parts = ["armor_block", "automaton_cannon", "automaton_core", "slope_1x1", "thruster", "shield_emitter"];
            assert!(g.blueprint.parts.iter().all(|p| parts.contains(&p.part.as_str())), "its palette: {:?}", g.blueprint.parts);
        }
    }
    let h = &straight.state().history;
    assert!(h.iter().all(|g| g.enemy.as_ref().is_some_and(|e| e.evaluated > 0)));
    assert!(straight.enemy_archive().is_some_and(|a| !a.cells().is_empty()));
}

/// Generation 0 fights the other side's starts; later ones the frozen sets (best, past champions, your starts as the
/// anchor), every fight with your side as side 0 and an enemy in a pack sized to it; each replays as recorded.
#[test]
fn every_fight_of_both_sides_replays_and_your_side_is_side_0() {
    let c = data::base_content();
    let run = stepped(&c, u64::MAX);
    let records = &run.state().records;
    let n = run.starts().len() as u32;
    let r0 = records[0].enemy.as_ref().unwrap();
    assert_eq!((r0.opponents.clone(), r0.rivals.clone()), (vec![1, 2], vec![0]), "generation 0: the starts");
    let r = records[3].enemy.as_ref().unwrap();
    assert!(r.opponents.len() == 3 && r.opponents.iter().all(|&g| run.camp(g) == Some(Camp::Enemy)));
    assert!(r.rivals.contains(&0), "your start, the anchor");
    assert!(r.rivals.iter().all(|&g| run.camp(g) == Some(Camp::Yours)));
    let mut fights = 0;
    for (g, record) in records.iter().enumerate() {
        let first = record.enemy.as_ref().unwrap().first as usize;
        assert_eq!(run.children_of(g as u32, Camp::Yours), 0..first);
        for (child, rec) in record.children.iter().enumerate() {
            let enemy = child >= first;
            let jobs = run.tagged_jobs(g as u32, child).unwrap();
            assert_eq!(jobs.len(), rec.jobs.len());
            for (j, ((trial, job), jr)) in jobs.iter().zip(&rec.jobs).enumerate() {
                let Job::Fight { spec, side, .. } = job else { unreachable!() };
                assert_eq!(*side == 1, enemy);
                let coevolution = matches!(run.trials()[*trial as usize].kind, Trial::CoEvolution);
                assert!(coevolution || !enemy, "an enemy fights only the co-evolution trial");
                if coevolution {
                    // Side 0 is always yours, side 1 the enemy's, in a pack of at least one.
                    let theirs = data::parse_blueprint(match &spec.sides[1].entries[0].design {
                        arena::DesignRef::Blueprint(t) => t,
                        arena::DesignRef::Ship(_) => unreachable!("genomes are inlined"),
                    })
                    .unwrap();
                    assert!(theirs.tags.iter().any(|t| t == "enemy") && spec.sides[1].entries[0].count >= 1);
                }
                if fights % 7 == 0 {
                    let code = FightCode::decode(&run.fight_code(g as u32, child, j).unwrap().encode()).unwrap();
                    assert_eq!(run_fight(&c, &code.spec).unwrap(), code.record, "generation {g}, child {child}, job {j}");
                    assert_eq!(code.record.sides[*side as usize].score, jr.score, "scored for its side");
                }
                fights += 1;
            }
        }
        assert!(record.children[first..].iter().all(|c| c.genome >= n || g == 0 && c.genome < n + 2));
    }
    assert!(fights > 150, "{fights}");
    // Its own best of each side.
    for g in 0..run.generation() {
        let best = run.best_of(g, Camp::Enemy).unwrap();
        assert!(run.children_of(g, Camp::Enemy).contains(&best));
        assert!(run.children_of(g, Camp::Yours).contains(&run.best_child(g).unwrap()));
    }
    // An enemy's score is its side's WIN, its breakdown its one trial.
    let (g, k) = run.scored(run.enemy_archive().unwrap().best().unwrap().id).unwrap();
    let b = run.breakdown(g, k).unwrap();
    assert_eq!(b.trials.len(), 1);
    assert_eq!(b.trials[0].score, Some(records[g as usize].children[k].score));
}

/// PAST CHAMPIONS: every second generation both bests fight the other side's champions of every generation so far;
/// the line says how many each beat, and the check's fights replay.
#[test]
fn past_champions_are_checked_and_replay() {
    let c = data::base_content();
    let run = stepped(&c, u64::MAX);
    let beaten = run.beaten();
    assert_eq!(beaten.iter().map(|b| b.generation).collect::<Vec<_>>(), [2, 4]);
    for b in &beaten {
        assert_eq!((b.yours.1, b.theirs.1), (b.generation, b.generation), "a champion of every generation before");
        assert!(b.yours.0 <= b.yours.1 && b.theirs.0 <= b.theirs.1);
        let check = run.check(b.generation).unwrap();
        assert_eq!(run.camp(check.yours), Some(Camp::Yours));
        assert_eq!(run.camp(check.theirs), Some(Camp::Enemy));
        let jobs = run.check_jobs(b.generation).unwrap();
        assert_eq!(jobs.len(), check.jobs.len());
        assert_eq!(jobs.len(), 2 * 2 * b.generation as usize, "two seeds, each side");
        for k in [0, jobs.len() - 1] {
            let code = run.check_fight_code(b.generation, k).unwrap();
            assert_eq!(run_fight(&c, &code.spec).unwrap(), code.record);
        }
    }
    assert!(run.check(1).is_none() && run.check(3).is_none());
}

/// SAVE AS DESIGN keeps an enemy's faction and tags; EXPORT TO CONTENT writes the content ship format with its origin
/// and its behavior as a comment block, which the content reads as a ship and a behavior.
#[test]
fn an_evolved_enemy_exports_to_content() {
    let c = data::base_content();
    let run = stepped(&c, u64::MAX);
    let best = run.enemy_archive().unwrap().best().unwrap().id;
    assert!(run.export_enemy(run.archive().best().unwrap().id, "lab-1", 1).is_none(), "yours don't");
    let text = run.export_enemy(best, "lab-1", 3).unwrap();
    let ship = data::parse_blueprint(&text).unwrap();
    assert_eq!(ship.id, "evolved_automatons_3");
    assert_eq!(ship.behavior.as_deref(), Some("evolved_automatons_3"));
    assert!(ship.tags.contains(&"enemy".to_string()) && ship.tags.contains(&"evolved".to_string()));
    let origin = ship.origin.clone().unwrap();
    assert_eq!((origin.run.as_str(), origin.genome), ("lab-1", best));
    assert!(origin.from[0].starts_with("automaton_"));
    let block: Vec<&str> = text.lines().skip_while(|l| !l.starts_with(data::BEHAVIOR_MARK)).skip(1).map(|l| l.trim_start_matches("// ")).collect();
    let behavior: data::Behavior = data::ron_options().from_str(&block.join("\n")).unwrap();
    assert_eq!(behavior.id, "evolved_automatons_3");
    let program = behavior.program().unwrap();
    assert!(data::behavior::check_program(&program, &c.behavior_nodes).is_empty());
    // Its behavior is the file in the block: checked as the content will have it.
    let problems = data::validate_ship_flying(&c, &data::Blueprint { behavior: None, ..ship.clone() }, &program);
    assert!(problems.iter().all(|p| !p.is_error()), "{problems:?}");
}

/// The setup's and the CLI's ARMS RACE: `lab.ron`'s preset with the faction's ships (its boss only when picked) and
/// palette; a run needs its enemy and its trial together, and enemies never change their assists.
#[test]
fn the_arms_race_preset_and_its_refusals() {
    let c = data::base_content();
    assert_eq!(
        arena::evolve::enemy_starts(&c),
        ["automaton_gunner", "automaton_hunter", "automaton_lancer", "automaton_raider"],
        "the warden is a boss"
    );
    let preset = c.lab.trial("arms_race").unwrap();
    let ships = arena::evolve::enemy_starts(&c);
    let pop = Population::from_preset(&c, preset, &ships, 32).unwrap();
    assert_eq!((pop.population, pop.elites, pop.hall, pop.palette.clone()), (16, 3, 2, vec!["shield_emitter".to_string()]));
    assert!(Population::from_preset(&c, preset, &[], 32).is_err());
    assert!(Population::from_preset(&c, preset, &["automaton_raider".into(), "rim_skiff".into()], 32).is_err(), "two factions");
    let spec = TrialSpec::from_preset(preset, 1.0, 2, false, &Default::default()).unwrap();
    assert_eq!(spec.kind, Trial::CoEvolution);
    let mut no_enemy = config();
    no_enemy.enemy = None;
    assert!(matches!(Run::new(&c, no_enemy), Err(RunError::Config(_))));
    let mut no_trial = config();
    no_trial.trials.pop();
    assert!(matches!(Run::new(&c, no_trial), Err(RunError::Config(_))));
    let mut piloted = config();
    piloted.enemy.as_mut().unwrap().evolve = arena::evolve::MayChange::default();
    assert!(matches!(Run::new(&c, piloted), Err(RunError::Config(_))));
    // The run file's text keeps the enemy; one without reads and writes as before.
    let cfg = config();
    assert_eq!(RunConfig::from_ron(&cfg.to_ron()).unwrap(), cfg);
    let plain = RunConfig { enemy: None, trials: cfg.trials[..1].to_vec(), ..cfg };
    assert!(!plain.to_ron().contains("enemy:"));
    // A validation is of your designs.
    let run = stepped(&c, u64::MAX);
    let theirs = run.enemy_archive().unwrap().best().unwrap().id;
    assert!(run.validation(&[theirs], 1).is_none());
    assert!(run.validation(&[run.archive().best().unwrap().id], 1).is_some());
}
