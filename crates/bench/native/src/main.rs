//! Native benchmark runner. Runs scenario x engine x repeat, prints a line per run and appends one JSON
//! object per run to `--out` (JSON lines). Repeats are interleaved (repeat -> scenario -> engine) so background
//! load hits every engine alike.
//!
//! Usage: bench-native [--engines a,b] [--scenarios a,b] [--repeats 3] [--warmup 30] [--ticks 900]
//!                     [--threads 1] [--out results/native.jsonl] [--tick-hashes] [--label st]
//!                     [--rep-base 0] [--svg dir]   (--svg writes the final state of each run as an SVG)
//!                     [--no-sleep]   (disable body sleeping in every engine; default: each engine's default)
//!                     [--lockstep]   (all engines of this binary in one process, ticking in turn; see README)

use bench_core::harness::{DEFAULT_SEED, DEFAULT_TICKS, DEFAULT_WARMUP};
use bench_core::{Clock, EngineConfig, PhysicsEngine, RunOptions, RunParams, RunResult, Runner, SCENARIOS, scenario};
use std::io::Write;
use std::time::Instant;

struct NativeClock(Instant);
impl Clock for NativeClock {
    fn now_ms(&self) -> f64 {
        self.0.elapsed().as_secs_f64() * 1000.0
    }
}


/// Engines compiled into this binary.
fn compiled() -> Vec<&'static str> {
    let mut v = Vec::new();
    if cfg!(feature = "rapier") {
        v.extend(["rapier", "rapier-compound"]);
    }
    if cfg!(feature = "box2d") {
        v.push("box2d-rust");
    }
    if cfg!(feature = "avian") {
        v.extend(["avian", "avian-compound"]);
    }
    if cfg!(feature = "boxdd") {
        v.push("box2d-c");
    }
    v
}

fn supports_threads(name: &str) -> bool {
    match name {
        #[cfg(feature = "rapier")]
        "rapier" | "rapier-compound" => bench_rapier::RapierEngine::supports_threads(),
        #[cfg(feature = "box2d")]
        "box2d-rust" => bench_box2d::Box2dEngine::supports_threads(),
        #[cfg(feature = "avian")]
        "avian" | "avian-compound" => bench_avian::AvianEngine::supports_threads(),
        #[cfg(feature = "boxdd")]
        "box2d-c" => bench_boxdd::BoxddEngine::supports_threads(),
        _ => false,
    }
}

fn make(name: &str, cfg: EngineConfig) -> Box<dyn PhysicsEngine> {
    let _ = cfg;
    match name {
        #[cfg(feature = "rapier")]
        "rapier" => Box::new(bench_rapier::RapierEngine::new(cfg, false)),
        #[cfg(feature = "rapier")]
        "rapier-compound" => Box::new(bench_rapier::RapierEngine::new(cfg, true)),
        #[cfg(feature = "box2d")]
        "box2d-rust" => Box::new(bench_box2d::Box2dEngine::new(cfg)),
        #[cfg(feature = "avian")]
        "avian" => Box::new(bench_avian::AvianEngine::new(cfg, false)),
        #[cfg(feature = "avian")]
        "avian-compound" => Box::new(bench_avian::AvianEngine::new(cfg, true)),
        #[cfg(feature = "boxdd")]
        "box2d-c" => Box::new(bench_boxdd::BoxddEngine::new(cfg)),
        _ => panic!("engine {name} is not compiled into this binary"),
    }
}

fn loadavg() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let list = |flag: &str, all: Vec<String>| -> Vec<String> {
        get(flag).map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or(all)
    };
    let have = compiled();
    let engines: Vec<String> = list("--engines", have.iter().map(|s| s.to_string()).collect())
        .into_iter()
        .filter(|e| have.contains(&e.as_str()))
        .collect();
    let scenarios = list("--scenarios", SCENARIOS.iter().map(|s| s.name.to_string()).collect());
    let repeats: u32 = get("--repeats").map(|s| s.parse().unwrap()).unwrap_or(3);
    let warmup: u32 = get("--warmup").map(|s| s.parse().unwrap()).unwrap_or(DEFAULT_WARMUP);
    let ticks: u32 = get("--ticks").map(|s| s.parse().unwrap()).unwrap_or(DEFAULT_TICKS);
    let threads: u32 = get("--threads").map(|s| s.parse().unwrap()).unwrap_or(1);
    let label = get("--label").unwrap_or_else(|| if threads > 1 { "mt".into() } else { "st".into() });
    let keep_hashes = args.iter().any(|a| a == "--tick-hashes");
    let rep_base: u32 = get("--rep-base").map(|s| s.parse().unwrap()).unwrap_or(0);
    let out_path = get("--out");
    let svg_dir = get("--svg");
    let no_sleep = args.iter().any(|a| a == "--no-sleep");
    let mut out = out_path.as_ref().map(|p| {
        if let Some(dir) = std::path::Path::new(p).parent() {
            std::fs::create_dir_all(dir).ok();
        }
        std::fs::OpenOptions::new().create(true).append(true).open(p).expect("open --out")
    });

    for s in &scenarios {
        assert!(scenario::scenario(s).is_some(), "unknown scenario {s}");
    }
    eprintln!(
        "bench-native: engines {:?} scenarios {:?} repeats {} warmup {} ticks {} threads {} build {}",
        engines,
        scenarios,
        repeats,
        warmup,
        ticks,
        threads,
        if cfg!(feature = "parallel") { "parallel" } else { "serial" }
    );

    let clock = NativeClock(Instant::now());
    let lockstep = args.iter().any(|a| a == "--lockstep");
    let mut emit = |res: &RunResult, rep: u32, load_before: &str, load_after: &str| {
        println!(
            "rep {} {:16} {:16} step med {:7.3} p95 {:7.3} max {:7.3} | total med {:7.3} p95 {:7.3}{}{} | bodies {}->{} | hash {:016x} | load {}",
            rep,
            res.scenario,
            res.engine,
            res.step.median,
            res.step.p95,
            res.step.max,
            res.total.median,
            res.total.p95,
            res.rays.map(|r| format!(" | rays med {:6.3} hits {}", r.median, res.ray_hits)).unwrap_or_default(),
            res.split.map(|s| format!(" | split mean {:6.3} max {:6.3}", s.mean, s.max)).unwrap_or_default(),
            res.bodies_initial,
            res.bodies_final,
            res.state_hash,
            load_after
        );
        if let (Some(dir), Some(svg)) = (svg_dir.as_ref(), res.svg.as_ref()) {
            std::fs::create_dir_all(dir).ok();
            std::fs::write(format!("{dir}/{}-{}.svg", res.engine, res.scenario), svg).unwrap();
        }
        if let Some(f) = out.as_mut() {
            let extra = format!(
                "\"platform\":\"native\",\"build\":\"{}\",\"label\":\"{}\",\"rep\":{},\"sleep\":{},\"lockstep\":{},\"load_before\":\"{}\",\"load_after\":\"{}\"",
                if cfg!(feature = "parallel") { "parallel" } else { "serial" },
                label,
                rep,
                !no_sleep,
                lockstep,
                load_before,
                load_after
            );
            writeln!(f, "{}", res.to_json(&extra)).unwrap();
            f.flush().ok();
        }
    };

    for rep in rep_base..rep_base + repeats {
        for sname in &scenarios {
            let spec = scenario::scenario(sname).unwrap();
            let params = RunParams { spec, seed: DEFAULT_SEED, warmup, ticks };
            let opts = RunOptions { threads, keep_tick_hashes: keep_hashes, svg: svg_dir.is_some() };
            let cfg = EngineConfig { dt: scenario::DT, substeps: scenario::SUBSTEPS, threads, sleep: !no_sleep };
            let selected: Vec<&String> = engines
                .iter()
                .filter(|e| {
                    let ok = threads == 1 || supports_threads(e);
                    if !ok && rep == rep_base && sname == &scenarios[0] {
                        eprintln!("  skip {e}: no multithreading support");
                    }
                    ok
                })
                .collect();
            if lockstep {
                // All engines in one process, one tick each in turn (order rotated every tick), so machine noise
                // (other processes, host core scheduling, clock changes) hits every engine alike.
                let load_before = loadavg();
                let mut es: Vec<Box<dyn PhysicsEngine>> = selected.iter().map(|e| make(e, cfg)).collect();
                let mut rs: Vec<Runner> = es.iter_mut().map(|e| Runner::new(e.as_mut(), params, &clock, opts)).collect();
                let n = es.len();
                let mut t = 0usize;
                while !rs[0].done() {
                    for k in 0..n {
                        let i = (t + k) % n;
                        rs[i].tick(es[i].as_mut(), &clock);
                    }
                    t += 1;
                }
                let results: Vec<RunResult> = rs.into_iter().zip(es.iter_mut()).map(|(r, e)| r.finish(e.as_mut(), &clock)).collect();
                drop(es);
                let load_after = loadavg();
                for res in &results {
                    emit(res, rep, &load_before, &load_after);
                }
            } else {
                for ename in selected {
                    let load_before = loadavg();
                    let mut engine = make(ename, cfg);
                    let res = bench_core::run(engine.as_mut(), params, &clock, opts);
                    drop(engine);
                    let load_after = loadavg();
                    emit(&res, rep, &load_before, &load_after);
                }
            }
        }
    }
}
