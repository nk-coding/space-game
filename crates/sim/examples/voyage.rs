//! The headless voyage runner (track 7K's balance tool, `docs/sim.md` "Voyages"): flies a content voyage with one
//! ship (and escorts) over many seeds, as a simple player would, and reports per seed whether the goal was cleared,
//! the time, the hull lost, and the haul (items, and research data per family as the career unloads it), then a
//! summary.
//!
//! `cargo run --release -p sim --example voyage -- --voyage yard_approach --design rim_skiff --seeds 1..20`
//! `[--escort rim_skiff[,...]] [--behavior skirmisher] [--minutes 30]`
//!
//! `--design` and `--escort` take a content ship id or a blueprint file (`*.ron`, a saved or exported drydock
//! design: a ship a player would build that is not content). Escorts start behind the flagship and fly the
//! `escort` template with the flagship as their leader (they fight its lock); their holds count for the haul.
//! With `--behavior` the ship flies that content behavior in fights, else the closing pilot.
//!
//! The pilot and the run live in `sim::voyage_runner` (its module docs say how the pilot flies); this example
//! parses the arguments, runs the seeds and prints. Native only; the wall clock times each tick of a run (the
//! pilot's decision and the sim step; median and p95), never the sim.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use data::career::unload;
use sim::voyage_runner::{CLOSE_TO, Outcome};
use sim::{BehaviorLibrary, Flagship, ShipDesign, SimConfig, VoyageRun, VoyageRunner};

struct Args {
    voyage: String,
    design: String,
    seeds: Vec<u64>,
    behavior: Option<String>,
    escorts: Vec<String>,
    minutes: u64,
}

fn args() -> Args {
    let mut a = Args {
        voyage: "yard_approach".into(),
        design: "rim_skiff".into(),
        seeds: (1..=20).collect(),
        behavior: None,
        escorts: Vec::new(),
        minutes: 30,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let v = it.next().unwrap_or_else(|| panic!("{k} needs a value"));
        match k.as_str() {
            "--voyage" => a.voyage = v,
            "--design" => a.design = v,
            "--behavior" => a.behavior = Some(v),
            "--escort" => a.escorts.extend(v.split(',').map(str::to_string)),
            "--minutes" => a.minutes = v.parse().expect("--minutes: whole minutes"),
            "--seeds" => {
                a.seeds = match v.split_once("..") {
                    Some((lo, hi)) => {
                        let lo: u64 = lo.parse().expect("--seeds: a..b");
                        let hi: u64 = hi.parse().expect("--seeds: a..b");
                        (lo..=hi).collect()
                    }
                    None => v
                        .split(',')
                        .map(|s| s.trim().parse().expect("--seeds: a list or a..b"))
                        .collect(),
                }
            }
            _ => panic!("unknown argument {k}"),
        }
    }
    a
}

fn mmss(ticks: u64) -> String {
    let s = ticks / sim::TICK_HZ as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

fn p95(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)]
}

fn data_line(data: &BTreeMap<String, f32>) -> String {
    data.iter()
        .map(|(f, x)| format!("{f} {x:.0}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A content ship by id, or a blueprint file (a saved or exported drydock design, `*.ron`): a design a player
/// would build that is not content.
fn blueprint(content: &data::Content, design: &str) -> data::Blueprint {
    if design.ends_with(".ron") {
        let text = std::fs::read_to_string(design).unwrap_or_else(|e| panic!("{design}: {e}"));
        data::parse_blueprint(&text).unwrap_or_else(|e| panic!("{design}: {e:?}"))
    } else {
        content
            .ships
            .get(design)
            .unwrap_or_else(|| panic!("no ship `{design}`"))
            .clone()
    }
}

fn main() {
    let a = args();
    let content = data::base_content();
    let voyage = content
        .voyages
        .get(&a.voyage)
        .unwrap_or_else(|| panic!("no voyage `{}`", a.voyage));
    let bp = blueprint(&content, &a.design);
    let design = Arc::new(ShipDesign::compile(&content, &bp).expect("the design compiles"));
    let report = sim::design_report(&content, &bp, &SimConfig::default()).expect("a report");
    let reach = sim::closing_reach(&report);
    let behaviors = BehaviorLibrary::new(&content).expect("content behaviors compile");
    let fight_behavior = a.behavior.as_deref().map(|id| {
        behaviors
            .id(id)
            .unwrap_or_else(|| panic!("no behavior `{id}`"))
    });
    let escorts: Vec<Arc<ShipDesign>> = a
        .escorts
        .iter()
        .map(|id| {
            let bp = blueprint(&content, id);
            Arc::new(ShipDesign::compile(&content, &bp).expect("the escort compiles"))
        })
        .collect();
    let pilot = match &a.behavior {
        Some(b) => format!("behavior `{b}` in fights"),
        None => format!("closing to {:.0} m", reach * CLOSE_TO),
    };
    println!(
        "# voyage {} with {}{} ({pilot}), seeds {}..{} ({}), at most {} min",
        a.voyage,
        a.design,
        if a.escorts.is_empty() {
            String::new()
        } else {
            format!(" and escorts {} (`escort`)", a.escorts.join(", "))
        },
        a.seeds.first().unwrap_or(&0),
        a.seeds.last().unwrap_or(&0),
        a.seeds.len(),
        a.minutes
    );
    println!(
        "| seed | outcome | goal | sites | time | hull lost | kills | left (disarmed) | haul (items) | data |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    let max_ticks = a.minutes * 60 * sim::TICK_HZ as u64;
    let mut runs = Vec::new();
    let mut step_ms = Vec::new();
    for &seed in &a.seeds {
        let flagship = Flagship {
            design: Arc::clone(&design),
            reach,
            behavior: fight_behavior,
        };
        let mut runner = VoyageRunner::new(
            &content,
            voyage,
            flagship,
            behaviors.clone(),
            &escorts,
            seed,
            max_ticks,
        )
        .expect("the voyage starts");
        let r: VoyageRun = loop {
            let t0 = Instant::now();
            let done = runner.step(1).cloned();
            step_ms.push(t0.elapsed().as_secs_f64() * 1e3);
            if let Some(r) = done {
                break r;
            }
        };
        let report = unload(&content, &r.haul);
        println!(
            "| {seed} | {:?} | {} | {}/{} | {} | {:.1}/{:.0} | {} | {} ({}) | {} | {} |",
            r.outcome,
            if r.goal { "yes" } else { "no" },
            r.sites_cleared,
            r.sites,
            mmss(r.ticks),
            r.hp_lost,
            r.hp_total,
            r.kills,
            r.left.0,
            r.left.1,
            r.items,
            data_line(&report.data)
        );
        runs.push((r, report));
    }
    let home: Vec<&(VoyageRun, data::career::HaulReport)> = runs
        .iter()
        .filter(|(r, _)| r.outcome == Outcome::Recalled)
        .collect();
    let count = |o: Outcome| runs.iter().filter(|(r, _)| r.outcome == o).count();
    let cleared_home = home.iter().filter(|(r, _)| r.goal).count();
    let mut data: BTreeMap<String, f32> = BTreeMap::new();
    let mut pm = 0;
    for (_, rep) in &home {
        for (f, x) in &rep.data {
            *data.entry(f.clone()).or_default() += x / home.len() as f32;
        }
        pm += rep
            .materials
            .get("programmable_matter")
            .copied()
            .unwrap_or(0);
    }
    println!(
        "# summary: {} seeds: goal cleared and home {}, home without the goal {}, lost {}, out of time {}",
        runs.len(),
        cleared_home,
        home.len() - cleared_home,
        count(Outcome::Lost),
        count(Outcome::OutOfTime)
    );
    let f = |v: Vec<f64>| format!("median {:.1}, p95 {:.1}", median(v.clone()), p95(v));
    if !home.is_empty() {
        println!(
            "# home: time (min) {}; hull lost {}; haul items {}; programmable matter {:.1} per run",
            f(home.iter().map(|(r, _)| r.ticks as f64 / 1800.0).collect()),
            f(home.iter().map(|(r, _)| r.hp_lost as f64).collect()),
            f(home.iter().map(|(r, _)| r.items as f64).collect()),
            pm as f64 / home.len() as f64
        );
        println!("# home: data per run (mean): {}", data_line(&data));
    }
    println!(
        "# step time (ms, wall clock, all seeds): median {:.4}, p95 {:.4}, max {:.3}",
        median(step_ms.clone()),
        p95(step_ms.clone()),
        step_ms.iter().copied().fold(0.0, f64::max)
    );
}
