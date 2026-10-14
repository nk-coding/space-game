//! The arena's native CLI (`docs/arena.md`): runs fights on every core and prints Markdown tables.
//!
//! ```text
//! arena fight <spec.ron> [--seeds N] [--threads N]
//! arena matrix <design>... [--seeds N] [--seed S] [--distance M] [--time S] [--threads N]
//! ```
//!
//! `fight` runs a fight spec once per seed (the spec's seed and the N − 1 after it) and prints a row per seed and
//! a summary. `matrix` runs every pair of designs (a content ship id, or a blueprint file `*.ron`) one against
//! one under their default behaviors, N seeds in each starting position, and prints the win rate and mean score
//! of each row's design against each column's. Both print fights per second. The fights run on
//! `available_parallelism` threads (`--threads` to change); only the wall clock line depends on the machine.

use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use arena::{DesignRef, Entry, FightResult, FightSpec, Formation, Outcome, Summary};

const USAGE: &str = "usage:
  arena fight <spec.ron> [--seeds N] [--threads N]
  arena matrix <design>... [--seeds N] [--seed S] [--distance M] [--time S] [--threads N]
designs: a content ship id, or a blueprint file (*.ron)";

struct Opts {
    /// Positional arguments after the command.
    args: Vec<String>,
    seeds: u64,
    seed: Option<u64>,
    distance: Option<f32>,
    time: Option<f32>,
    threads: usize,
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
