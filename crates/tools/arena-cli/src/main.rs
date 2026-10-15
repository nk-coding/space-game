//! The arena's native CLI (`docs/arena.md`, ADR-0040): runs fights on every core and prints Markdown tables. A thin
//! binary over the `arena` library (`crates/core/arena`): arguments, the thread pool, tables and files.
//!
//! ```text
//! arena fight <spec.ron> [--seeds N] [--threads N]
//! arena matrix <design>... [--seeds N] [--seed S] [--distance M] [--time S] [--threads N]
//! arena evolve <run.ron> [--from <design>]... [--lock <lock>]... [--save FILE] [--until GEN] [--out DIR] [--best N] [--threads N]
//! arena evolve --resume FILE [--save FILE] [--until GEN] [--out DIR] [--best N] [--threads N]
//! arena replay <run file> <generation> <child> <job>
//! arena lineage <run file> [<genome>]
//! ```
//!
//! `fight` runs a fight spec once per seed (the spec's seed and the N − 1 after it) and prints a row per seed, a
//! summary and each fight's code (GDR-0092: `?scene=fight&fight=<code>` re-watches it in the game). `matrix` runs
//! every pair of designs (a content ship id, or a blueprint file `*.ron`) one against one under their default
//! behaviors, N seeds in each starting position, and prints the win rate and mean score of each row's design against
//! each column's. Both print fights per second. `evolve` runs an evolution (`docs/arena.md` "Evolution"), a
//! generation's fights or voyage runs on every core, prints a row per generation, the best elites with their
//! lineage, writes them (blueprint and behavior text) into `--out` and prints a fight code per generation (its best
//! child in its hardest fight). `--from` starts it from designs (replacing the run file's starts), `--lock` keeps
//! parts, cells, categories and rules of every start, `--save` writes the run file after every generation and
//! `--resume` goes on from one (GDR-0092 steps 2 and 3). `replay` prints the fight code of any fight of a saved run,
//! `lineage` a genome's line from its start. The results do not depend on the thread count, nor on where a run was
//! interrupted. The fights run on `available_parallelism` threads (`--threads` to change); only the wall clock line
//! depends on the machine.

use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use arena::evolve::{Job, Locks, Start};
use arena::{
    BehaviorRef, DesignRef, Entry, FightCode, FightResult, FightSpec, Formation, Outcome, Run, RunSave,
    Summary,
};

const USAGE: &str = "usage:
  arena fight <spec.ron> [--seeds N] [--threads N]
  arena matrix <design>... [--seeds N] [--seed S] [--distance M] [--time S] [--threads N]
  arena evolve <run.ron> [--from <design>]... [--lock <lock>]... [--save FILE] [--until GEN] [--out DIR] [--best N] [--threads N]
  arena evolve --resume FILE [--save FILE] [--until GEN] [--out DIR] [--best N] [--threads N]
  arena replay <run file> <generation> <child> <job>
  arena lineage <run file> [<genome>]
designs: a content ship id, or a blueprint file (*.ron; a behavior file of the same name with .txt beside it is its
behavior, as --out writes them)
locks (of every start): part:N (its parts[N]), category:ID (every part of it, e.g. category:weapons), cell:X,Y (keep
empty, in the start's coordinates), rule:N (its behavior's rule N, 0 = the top), target, order (the locked rules
keep their order)";

struct Opts {
    /// Positional arguments after the command.
    args: Vec<String>,
    seeds: u64,
    seed: Option<u64>,
    distance: Option<f32>,
    time: Option<f32>,
    threads: usize,
    /// `evolve`: where the best elites are written, and how many.
    out: Option<String>,
    best: usize,
    /// `evolve`: its starts and their locks, the run file to write after every generation or to go on from, the
    /// generation to stop after.
    from: Vec<String>,
    locks: Vec<String>,
    save: Option<String>,
    resume: Option<String>,
    until: Option<u32>,
}

/// The value of option `a`, as a number.
fn num<T: std::str::FromStr>(a: &str, v: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    v.parse().map_err(|e| format!("{a}: {e}: {v}"))
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        args: Vec::new(),
        seeds: 1,
        seed: None,
        distance: None,
        time: None,
        threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
        out: None,
        best: 8,
        from: Vec::new(),
        locks: Vec::new(),
        save: None,
        resume: None,
        until: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if !a.starts_with("--") {
            o.args.push(a.clone());
            continue;
        }
        let v = it.next().ok_or_else(|| format!("{a} needs a value"))?;
        match a.as_str() {
            "--seeds" => o.seeds = num(a, v)?,
            "--seed" => o.seed = Some(num(a, v)?),
            "--distance" => o.distance = Some(num(a, v)?),
            "--time" => o.time = Some(num(a, v)?),
            "--threads" => o.threads = num::<usize>(a, v)?.max(1),
            "--out" => o.out = Some(v.clone()),
            "--best" => o.best = num(a, v)?,
            "--from" => o.from.push(v.clone()),
            "--lock" => o.locks.push(v.clone()),
            "--save" => o.save = Some(v.clone()),
            "--resume" => o.resume = Some(v.clone()),
            "--until" => o.until = Some(num(a, v)?),
            _ => return Err(format!("unknown option {a}")),
        }
    }
    if o.seeds == 0 {
        return Err("--seeds must be at least 1".into());
    }
    Ok(o)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.split_first() {
        Some((cmd, rest)) if cmd == "fight" => parse(rest).and_then(|o| fight(&o)),
        Some((cmd, rest)) if cmd == "matrix" => parse(rest).and_then(|o| matrix(&o)),
        Some((cmd, rest)) if cmd == "evolve" => parse(rest).and_then(|o| evolve(&o)),
        Some((cmd, rest)) if cmd == "replay" => parse(rest).and_then(|o| replay(&o)),
        Some((cmd, rest)) if cmd == "lineage" => parse(rest).and_then(|o| lineage(&o)),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Runs `f` over `items` on `threads` threads (each takes the next item until none are left); the results come
/// back in the order of `items`.
fn par_map<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads.min(items.len()).max(1))
            .map(|_| {
                s.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(k) else { break out };
                        out.push((k, f(item)));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().expect("a fight thread panicked"))
            .collect()
    });
    done.sort_by_key(|(k, _)| *k);
    done.into_iter().map(|(_, r)| r).collect()
}

/// Runs every spec on the threads, failing on the first bad one; prints the fights-per-second line.
fn run_all(
    content: &data::Content,
    specs: &[FightSpec],
    threads: usize,
) -> Result<Vec<FightResult>, String> {
    let start = Instant::now();
    let results = par_map(specs, threads, |s| arena::run_fight(content, s));
    let secs = start.elapsed().as_secs_f64();
    let results = results
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let ticks: u64 = results.iter().map(|r| r.ticks).sum();
    println!(
        "# {} fights in {secs:.2} s on {} threads: {:.0} fights/s, {:.2} M ticks/s",
        results.len(),
        threads.min(specs.len()),
        results.len() as f64 / secs,
        ticks as f64 / secs / 1e6
    );
    Ok(results)
}

fn seconds(ticks: u64) -> String {
    format!("{:.1} s", ticks as f64 / sim::TICK_HZ as f64)
}

fn outcome(o: Outcome) -> String {
    match o {
        Outcome::Won(side) => format!("side {side} won"),
        Outcome::BothOut => "both out".into(),
        Outcome::TimeLimit => "time limit".into(),
    }
}

fn fight(o: &Opts) -> Result<(), String> {
    let [path] = o.args.as_slice() else {
        return Err(USAGE.into());
    };
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let spec = FightSpec::from_ron(&text).map_err(|e| format!("{path}: {e}"))?;
    let content = data::base_content();
    let specs: Vec<FightSpec> = (0..o.seeds)
        .map(|k| FightSpec {
            seed: spec.seed.wrapping_add(k),
            ..spec.clone()
        })
        .collect();
    println!(
        "# {path}: seeds {}..={}, time limit {} s",
        specs[0].seed,
        specs[specs.len() - 1].seed,
        spec.time_limit
    );
    let results = run_all(&content, &specs, o.threads)?;
    println!(
        "| seed | outcome | time | build points lost (0 / 1) | damage dealt (0 / 1) | vessels left (0 / 1) | score (0 / 1) | hash |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    for (s, r) in specs.iter().zip(&results) {
        let [a, b] = r.sides;
        println!(
            "| {} | {} | {} | {:.1} of {:.1} / {:.1} of {:.1} | {:.1} / {:.1} | {} / {} | {:.3} / {:.3} | {:016x} |",
            s.seed,
            outcome(r.outcome),
            seconds(r.ticks),
            a.build_points_lost,
            a.build_points,
            b.build_points_lost,
            b.build_points,
            a.damage_dealt,
            b.damage_dealt,
            a.vessels_left,
            b.vessels_left,
            a.score,
            b.score,
            r.state_hash
        );
    }
    let sum = Summary::of(&results);
    println!(
        "# summary: {} fights: side 0 won {}, side 1 won {}, undecided {}; mean score {:.3} / {:.3}; mean time {}",
        sum.fights,
        sum.wins[0],
        sum.wins[1],
        sum.undecided,
        sum.mean_score[0],
        sum.mean_score[1],
        seconds(sum.mean_ticks as u64)
    );
    let rules = arena::rules_fingerprint(&content);
    println!();
    println!("Fight codes (re-watch one in the game: `?scene=fight&fight=<code>`):");
    println!();
    for (s, r) in specs.into_iter().zip(results) {
        let seed = s.seed;
        println!("- seed {seed}: {}", FightCode::new(&content, rules, s, r).encode());
    }
    Ok(())
}

/// A design argument: a blueprint file (`*.ron`) or a content ship id.
fn design_ref(arg: &str) -> Result<DesignRef, String> {
    if arg.ends_with(".ron") {
        let text = std::fs::read_to_string(arg).map_err(|e| format!("{arg}: {e}"))?;
        Ok(DesignRef::Blueprint(text))
    } else {
        Ok(DesignRef::Ship(arg.to_string()))
    }
}

fn matrix(o: &Opts) -> Result<(), String> {
    if o.args.is_empty() {
        return Err(USAGE.into());
    }
    let content = data::base_content();
    let designs = o
        .args
        .iter()
        .map(|a| design_ref(a))
        .collect::<Result<Vec<_>, _>>()?;
    let base = o.seed.unwrap_or(1);
    let spec = |a: usize, b: usize, seed: u64| {
        let entry = |k: usize| Entry {
            design: designs[k].clone(),
            behavior: None,
            count: 1,
            formation: Formation::Line,
        };
        let mut s = FightSpec::new(seed, vec![entry(a)], vec![entry(b)]);
        s.start_distance = o.distance.unwrap_or(s.start_distance);
        s.time_limit = o.time.unwrap_or(s.time_limit);
        s
    };
    // Every pair in both starting positions (a design against itself once): `(row, column, spec)`.
    let n = designs.len();
    let mut jobs = Vec::new();
    for a in 0..n {
        for b in a..n {
            let orders: &[(usize, usize)] = if a == b { &[(a, a)] } else { &[(a, b), (b, a)] };
            for &(x, y) in orders {
                for k in 0..o.seeds {
                    jobs.push((x, y, spec(x, y, base.wrapping_add(k))));
                }
            }
        }
    }
    println!(
        "# matrix of {n} designs, seeds {base}..={} in each starting position",
        base.wrapping_add(o.seeds - 1)
    );
    let specs: Vec<FightSpec> = jobs.iter().map(|j| j.2.clone()).collect();
    let results = run_all(&content, &specs, o.threads)?;
    // Per (row, column): the row design's wins, fights and score sum.
    let mut cells = vec![(0u32, 0u32, 0.0f64); n * n];
    for ((x, y, _), r) in jobs.iter().zip(&results) {
        let mut add = |row: usize, col: usize, side: usize| {
            let c = &mut cells[row * n + col];
            c.0 += (r.winner() == Some(side)) as u32;
            c.1 += 1;
            c.2 += r.sides[side].score as f64;
        };
        add(*x, *y, 0);
        if x != y {
            add(*y, *x, 1);
        }
    }
    println!("Win rate (mean score) of the row's design against the column's; on the diagonal, side 0's in the mirror match.");
    println!();
    println!("| vs | {} |", o.args.join(" | "));
    println!("|---|{}", "---|".repeat(n));
    for (row, name) in o.args.iter().enumerate() {
        let line: Vec<String> = (0..n)
            .map(|col| {
                let (w, f, s) = cells[row * n + col];
                format!(
                    "{:.0}% ({:.2})",
                    100.0 * w as f64 / f as f64,
                    s / f as f64
                )
            })
            .collect();
        println!("| {name} | {} |", line.join(" | "));
    }
    Ok(())
}


/// The cell of an elite as text: one bin per axis.
fn cell_name(cell: &[u32]) -> String {
    cell.iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join("×")
}

/// A `--from` start: a content ship, or a blueprint file and the behavior file of the same name beside it.
fn start_of(arg: &str, locks: &Locks) -> Result<Start, String> {
    let behavior = match arg.strip_suffix(".ron") {
        Some(stem) => match std::fs::read_to_string(format!("{stem}.txt")) {
            Ok(text) => Some(BehaviorRef::Text(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("{stem}.txt: {e}")),
        },
        None => None,
    };
    Ok(Start::Design {
        design: design_ref(arg)?,
        behavior,
        locks: locks.clone(),
    })
}

/// The `--lock` arguments as locks.
fn locks_of(args: &[String]) -> Result<Locks, String> {
    let mut l = Locks::default();
    for a in args {
        let (kind, v) = a.split_once(':').unwrap_or((a, ""));
        let bad = || format!("--lock {a}: one of part:N, category:ID, cell:X,Y, rule:N, target, order");
        match kind {
            "part" => l.parts.push(v.parse().map_err(|_| bad())?),
            "category" => l.categories.push(v.to_string()),
            "cell" => {
                let (x, y) = v.split_once(',').ok_or_else(bad)?;
                l.cells.push((x.trim().parse().map_err(|_| bad())?, y.trim().parse().map_err(|_| bad())?));
            }
            "rule" => l.rules.push(v.parse().map_err(|_| bad())?),
            "target" => l.target = true,
            "order" => l.order = true,
            _ => return Err(bad()),
        }
    }
    Ok(l)
}

fn read_save(path: &str) -> Result<RunSave, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    RunSave::from_ron(&text).map_err(|e| format!("{path}: {e}"))
}

/// Writes the run file whole or not at all (a run killed mid-write keeps its last generation).
fn write_save(path: &str, run: &Run) -> Result<(), String> {
    let tmp = format!("{path}.part");
    std::fs::write(&tmp, run.snapshot().to_ron()).map_err(|e| format!("{tmp}: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{path}: {e}"))
}

/// A run file's run, read-only: refused if today's rules differ from the ones it was fought under.
fn restored(content: &data::Content, path: &str) -> Result<Run, String> {
    let run = Run::restore(content, &read_save(path)?).map_err(|e| format!("{path}: {e}"))?;
    if run.stale() {
        return Err(format!(
            "{path}: the rules have changed since this run (fingerprint {:016x}, today {:016x}): its fights no longer replay as recorded",
            run.rules(),
            arena::rules_fingerprint(content)
        ));
    }
    Ok(run)
}

fn row(g: &arena::evolve::GenerationStats, breed: Option<f64>, secs: Option<f64>) {
    let ms = |v: Option<f64>, d: usize| v.map_or("–".into(), |v| format!("{v:.d$}"));
    println!(
        "| {} | {:?} | {} | {} | {:.3} | {:.3} | {:.3} | {} | {} | {:.2} | {} | {} |",
        g.generation,
        g.genes,
        g.evaluated,
        g.rejected,
        g.best,
        g.mean,
        g.archive_best,
        g.filled,
        g.improved,
        g.ticks as f64 / 1e6,
        ms(breed, 1),
        ms(secs, 2)
    );
}

fn evolve(o: &Opts) -> Result<(), String> {
    let content = data::base_content();
    let (label, mut run) = match (&o.resume, o.args.as_slice()) {
        (Some(path), []) => {
            if !o.from.is_empty() || !o.locks.is_empty() {
                return Err("--from and --lock set up a new run; a resumed run keeps its own".into());
            }
            (path.clone(), restored(&content, path)?)
        }
        (None, [path]) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            let mut config = arena::RunConfig::from_ron(&text).map_err(|e| format!("{path}: {e}"))?;
            let locks = locks_of(&o.locks)?;
            if !o.from.is_empty() {
                config.start = None;
                config.starts = o.from.iter().map(|a| start_of(a, &locks)).collect::<Result<_, _>>()?;
            } else if !o.locks.is_empty() {
                for s in config.start.iter_mut().chain(&mut config.starts) {
                    if let Start::Design { locks: l, .. } = s {
                        *l = locks.clone();
                    }
                }
            }
            (path.clone(), Run::new(&content, config).map_err(|e| format!("{path}: {e}"))?)
        }
        _ => return Err(USAGE.into()),
    };
    let save = o.save.as_ref().or(o.resume.as_ref());
    let state = run.state();
    let until = o.until.unwrap_or(state.generations).min(state.generations);
    println!(
        "# {label}: {} generations of {} from {} start(s) on {} threads{}",
        state.generations,
        run.config().population,
        run.starts().len(),
        o.threads,
        match state.generation {
            0 => String::new(),
            g => format!(", resumed after generation {}", g - 1),
        }
    );
    println!(
        "| gen | evolves | children | refused | best | mean | archive best | cells | took a cell | M ticks | breed ms | s |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
    for g in state.history {
        row(g, None, None);
    }
    let start = Instant::now();
    let first = run.generation();
    while run.generation() < until {
        let t0 = Instant::now();
        // Breeding (mutation, crossover, the pre-filter) runs on this thread; the jobs on all of them.
        let jobs = run.jobs().to_vec();
        let breed = t0.elapsed().as_secs_f64() * 1e3;
        let results = par_map(&jobs, o.threads, |j| j.run(&content));
        run.submit(results);
        if let Some(path) = save {
            write_save(path, &run)?;
        }
        let g = run.state().history.last().cloned().expect("a generation is done");
        row(&g, Some(breed), Some(t0.elapsed().as_secs_f64()));
    }
    let secs = start.elapsed().as_secs_f64();
    let done = run.generation() - first;
    println!(
        "# {done} generations in {secs:.1} s on {} threads: {:.1} generations per minute",
        o.threads,
        done as f64 * 60.0 / secs
    );
    if let Some(path) = save {
        println!("# the run is in {path} (`arena evolve --resume {path}` goes on with it)");
    }
    let results = run.results();
    let axes: Vec<String> = run
        .archive()
        .shape()
        .axes
        .iter()
        .map(|a| format!("{:?}", a.measure))
        .collect();
    println!();
    println!("| rank | genome | cell | score | {} | generation | parts |", axes.join(" | "));
    println!("|---|---|---|---|{}---|---|", "---|".repeat(axes.len()));
    for (k, e) in results.elites.iter().take(o.best).enumerate() {
        let measures: Vec<String> = e.measures.iter().map(|m| format!("{:.1}", m + 0.0)).collect();
        println!(
            "| {} | {} | {} | {:.3} | {} | {} | {} |",
            k + 1,
            e.id,
            cell_name(&run.archive().cell_of(&e.measures)),
            e.score,
            measures.join(" | "),
            e.generation,
            e.genome.blueprint.parts.len()
        );
    }
    println!();
    println!("Lineage of the best (genome numbers from the start; `arena lineage <run file> <genome>` for each step):");
    println!();
    for (k, e) in results.elites.iter().take(o.best.min(3)).enumerate() {
        let line: Vec<String> = run.lineage(e.id).iter().map(|g| g.to_string()).collect();
        println!("- rank {}: {} (start {})", k + 1, line.join(" → "), e.genome.root + 1);
    }
    if let Some(out) = &o.out {
        std::fs::create_dir_all(out).map_err(|e| format!("{out}: {e}"))?;
        for (k, e) in results.elites.iter().take(o.best).enumerate() {
            let base = format!("{out}/elite_{:02}", k + 1);
            let mut bp = e.genome.blueprint.clone();
            bp.id = format!("evolved_{:02}", k + 1);
            let header = format!(
                "// Evolved by `arena evolve {label}`: rank {}, score {:.3}, generation {}, genome {}. Its behavior: elite_{:02}.txt\n",
                k + 1,
                e.score,
                e.generation,
                e.id,
                k + 1
            );
            std::fs::write(format!("{base}.ron"), header + data::blueprint_to_ron(&bp).as_str())
                .map_err(|e| format!("{base}.ron: {e}"))?;
            std::fs::write(format!("{base}.txt"), e.genome.behavior_text())
                .map_err(|e| format!("{base}.txt: {e}"))?;
        }
        println!(
            "# wrote the best {} elites to {out} (elite_NN.ron: the blueprint, elite_NN.txt: the behavior)",
            o.best.min(results.elites.len())
        );
    }
    // Every fight is kept, so each code comes from the run's records (GDR-0092): no fight runs again.
    let codes: Vec<String> = (0..run.generation())
        .filter_map(|g| {
            let child = run.best_child(g)?;
            let record = &run.state().records[g as usize].children[child];
            let (job, low) = record
                .jobs
                .iter()
                .enumerate()
                .filter(|(k, _)| matches!(run.job(g, child, *k), Some((Job::Fight { .. }, _))))
                .fold(None, |low: Option<(usize, f32)>, (k, r)| match low {
                    Some((_, s)) if s <= r.score => low,
                    _ => Some((k, r.score)),
                })?;
            let code = run.fight_code(g, child, job)?;
            Some(format!(
                "- gen {g}, score {low:.3}, seed {} (`arena replay` {g} {child} {job}): {}",
                code.spec.seed,
                code.encode()
            ))
        })
        .collect();
    if !codes.is_empty() {
        println!();
        println!("Fight codes: each generation's best child in its hardest fight (re-watch one in the game: `?scene=fight&fight=<code>`):");
        println!();
        for c in codes {
            println!("{c}");
        }
    }
    Ok(())
}

fn replay(o: &Opts) -> Result<(), String> {
    let [path, g, child, job] = o.args.as_slice() else {
        return Err(USAGE.into());
    };
    let content = data::base_content();
    let run = restored(&content, path)?;
    let (g, child, job) = (num("generation", g)?, num("child", child)?, num("job", job)?);
    let (j, record) = run
        .job(g, child, job)
        .ok_or_else(|| format!("{path}: no job {job} of child {child} of generation {g}"))?;
    let code = run
        .fight_code(g, child, job)
        .ok_or_else(|| format!("{path}: job {job} of child {child} of generation {g} is no fight"))?;
    let genome = run.state().records[g as usize].children[child].genome;
    println!(
        "# {path}: generation {g}, child {child} (genome {genome}), job {job}: seed {}, {}, score {:.3}, hash {:016x}",
        code.spec.seed,
        outcome(code.record.outcome),
        record.score,
        code.record.state_hash
    );
    if let Job::Fight { spec, .. } = &j {
        println!("# {} against {}", side_text(&spec.sides[0]), side_text(&spec.sides[1]));
    }
    println!("{}", code.encode());
    Ok(())
}

/// A side of a fight in a few words.
fn side_text(side: &arena::Side) -> String {
    let names: Vec<String> = side
        .entries
        .iter()
        .map(|e| {
            let d = match &e.design {
                DesignRef::Ship(id) => id.clone(),
                DesignRef::Blueprint(_) => "a blueprint".into(),
            };
            format!("{} × {d}", e.count)
        })
        .collect();
    names.join(", ")
}

fn lineage(o: &Opts) -> Result<(), String> {
    let (path, id) = match o.args.as_slice() {
        [path] => (path, None),
        [path, id] => (path, Some(num::<u32>("genome", id)?)),
        _ => return Err(USAGE.into()),
    };
    let content = data::base_content();
    let run = restored(&content, path)?;
    let id = match id {
        Some(id) => id,
        None => run.archive().best().map(|e| e.id).ok_or("the run has no elite yet")?,
    };
    if run.genome(id).is_none() {
        return Err(format!("{path}: no genome {id}"));
    }
    println!("# {path}: genome {id} from its start");
    println!("| genome | generation | start | parent | crossed with | parts lost | parts gained | behavior | score |");
    println!("|---|---|---|---|---|---|---|---|---|");
    for g in run.lineage(id) {
        let b = &run.table()[g as usize];
        let score = run
            .scored(g)
            .map_or("–".into(), |(at, k)| format!("{:.3}", run.state().records[at as usize].children[k].score));
        let opt = |v: Option<u32>| v.map_or("–".into(), |v| v.to_string());
        println!(
            "| {g} | {} | {} | {} | {} | {} | {} | {} | {score} |",
            b.generation,
            b.root + 1,
            opt(b.parent),
            opt(b.partner),
            b.removed.len(),
            b.added.len(),
            if b.parent.is_none() { "its own" } else if b.behavior.is_some() { "changed" } else { "–" },
        );
    }
    Ok(())
}
