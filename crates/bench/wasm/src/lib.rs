//! WebAssembly entry points. One wasm binary per engine (cargo feature), so binary sizes can be compared.
//! Used by the Node runner (`node/run-node.mjs`) and the browser page (`web/`). Time comes from
//! `performance.now()`, which exists in Node, browser main thread and workers.

use bench_core::{Clock, EngineConfig, PhysicsEngine, RunOptions, RunParams, SCENARIOS, scenario};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn perf_now() -> f64;
}

struct JsClock;
impl Clock for JsClock {
    fn now_ms(&self) -> f64 {
        perf_now()
    }
}

fn make(name: &str, cfg: EngineConfig) -> Option<Box<dyn PhysicsEngine>> {
    let _ = cfg;
    match name {
        #[cfg(feature = "rapier")]
        "rapier" => Some(Box::new(bench_rapier::RapierEngine::new(cfg, false))),
        #[cfg(feature = "rapier")]
        "rapier-compound" => Some(Box::new(bench_rapier::RapierEngine::new(cfg, true))),
        #[cfg(feature = "box2d")]
        "box2d-rust" => Some(Box::new(bench_box2d::Box2dEngine::new(cfg))),
        #[cfg(feature = "avian")]
        "avian" => Some(Box::new(bench_avian::AvianEngine::new(cfg, false))),
        #[cfg(feature = "avian")]
        "avian-compound" => Some(Box::new(bench_avian::AvianEngine::new(cfg, true))),
        _ => None,
    }
}

/// Comma-separated engine ids compiled into this binary.
#[wasm_bindgen]
pub fn engines() -> String {
    let mut v: Vec<&str> = Vec::new();
    if cfg!(feature = "rapier") {
        v.extend(["rapier", "rapier-compound"]);
    }
    if cfg!(feature = "box2d") {
        v.push("box2d-rust");
    }
    if cfg!(feature = "avian") {
        v.extend(["avian", "avian-compound"]);
    }
    v.join(",")
}

/// Comma-separated scenario names.
#[wasm_bindgen]
pub fn scenarios() -> String {
    SCENARIOS.iter().map(|s| s.name).collect::<Vec<_>>().join(",")
}

/// Run one scenario on one engine (single thread). Returns the result as a JSON object.
#[wasm_bindgen]
pub fn run_scenario(engine: &str, scenario_name: &str, warmup: u32, ticks: u32, tick_hashes: bool) -> String {
    let spec = match scenario::scenario(scenario_name) {
        Some(s) => s,
        None => return format!("{{\"error\":\"unknown scenario {scenario_name}\"}}"),
    };
    let cfg = EngineConfig { dt: scenario::DT, substeps: scenario::SUBSTEPS, threads: 1 };
    let Some(mut e) = make(engine, cfg) else {
        return format!("{{\"error\":\"engine {engine} not in this build\"}}");
    };
    let params = RunParams { spec, seed: bench_core::harness::DEFAULT_SEED, warmup, ticks };
    let opts = RunOptions { threads: 1, keep_tick_hashes: tick_hashes, svg: false };
    let res = bench_core::run(e.as_mut(), params, &JsClock, opts);
    res.to_json("\"platform\":\"wasm\",\"build\":\"serial\"")
}
