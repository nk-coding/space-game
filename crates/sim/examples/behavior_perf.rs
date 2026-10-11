//! Cost of behaviors: 20 ships (10 Halvex Runners facing 10 Sample Early, 90 m apart), every ship under a
//! 10-rule behavior. Native only; wall-clock is used here, in the tool, never in the sim.
//!
//! `cargo run --release -p sim --example behavior_perf -- [--ticks 900] [--runs 5]`
//!
//! Reports `Sim::step` with behaviors (built-in nodes; plus one Rhai condition evaluated at every think)
//! against the same fight under the old raider brain, and the think pass alone (all 20 ships at once; a
//! normal tick thinks for about a third of them).

use std::sync::Arc;
use std::time::Instant;

use sim::{Body, RaiderBrain, ShipCommand, ShipDesign, Sim, TickInputs, Vec2};

const TEN_RULES: &str = "target nearest(engines)
when hull < 20% -> flee
when heat > 95% -> kite(150%)
when power < 50% -> hold
when shield_down and target_on_me -> orbit(90%, ccw)
when under_attack and enemies_near(60m) >= 3 -> flee
when target_hull < 15% -> approach(30%)
when target_in_range(130%) and cycle(10s) >= 6s -> orbit(60%)
when target_time > 60s -> orbit(80%, cw)
when not target_in_range(200%) -> approach(67%)
always -> approach(67%)";

#[derive(Clone, Copy)]
enum Mode {
    Brain,
    Behavior,
    Scripted,
}

fn fleet(mode: Mode) -> Sim {
    let c = data::base_content();
    let d = |id: &str| Arc::new(ShipDesign::compile(&c, c.ships.get(id).unwrap()).unwrap());
    let (runner, early) = (d("halvex_runner"), d("sample_early"));
    let mut w = Sim::new(3);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let text = match mode {
        Mode::Scripted => TEN_RULES.replace(
            "when hull < 20% -> flee",
            "when closing_in(1s) and hull < 20% -> flee",
        ),
        _ => TEN_RULES.to_string(),
    };
    let id = w.behaviors_mut().add_text("ten", &text).unwrap();
    for k in 0..10 {
        let x = (k as f32 - 4.5) * 25.0;
        let a = w.spawn_ship(runner.clone(), Body::at(Vec2::new(x, 0.0), 0.0));
        let b = w.spawn_ship(
            early.clone(),
            Body::at(Vec2::new(x, 90.0), core::f32::consts::PI),
        );
        for s in [a, b] {
            let cmd = match mode {
                Mode::Brain => ShipCommand::Brain(Some(RaiderBrain::default())),
                _ => ShipCommand::Behavior(Some(id)),
            };
            w.command(s, cmd);
        }
    }
    w
}

fn stats(mut v: Vec<f64>) -> (f64, f64, f64, f64) {
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    v.sort_by(|a, b| a.total_cmp(b));
    let at = |q: f64| v[((v.len() as f64 * q) as usize).min(v.len() - 1)];
    (mean, at(0.5), at(0.95), v[0])
}

fn main() {
    let (mut ticks, mut runs) = (900usize, 5usize);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let v = args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--ticks" => ticks = v.parse().expect("--ticks: integer"),
            "--runs" => runs = v.parse().expect("--runs: integer"),
            _ => panic!("unknown argument {a}"),
        }
    }
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    println!(
        "# behavior perf: 20 ships, 10 rules each, {ticks} ticks, {runs} runs (the run with the median mean); load avg {}",
        load.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("| What | Mean | Median | p95 | Min |");
    println!("|---|---|---|---|---|");
    let idle = TickInputs::default();
    let report = |label: &str, mut per_run: Vec<(f64, f64, f64, f64)>| {
        per_run.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (m, med, p95, min) = per_run[per_run.len() / 2];
        println!("| {label} | {m:.1} µs | {med:.1} µs | {p95:.1} µs | {min:.1} µs |");
    };
    for (label, mode) in [
        ("`Sim::step`, raider brain (reference)", Mode::Brain),
        ("`Sim::step`, 10-rule behavior", Mode::Behavior),
        (
            "`Sim::step`, 10-rule behavior + a Rhai condition at every think",
            Mode::Scripted,
        ),
    ] {
        let mut per_run = Vec::new();
        for _ in 0..runs {
            let mut w = fleet(mode);
            let mut t = Vec::with_capacity(ticks);
            for _ in 0..ticks {
                let t0 = Instant::now();
                w.step(&idle);
                t.push(t0.elapsed().as_secs_f64() * 1e6);
            }
            per_run.push(stats(t));
        }
        report(label, per_run);
    }
    for (label, mode) in [
        (
            "think pass, all 20 ships at once (built-in nodes)",
            Mode::Behavior,
        ),
        (
            "think pass, all 20 ships at once (+ Rhai condition)",
            Mode::Scripted,
        ),
    ] {
        let mut per_run = Vec::new();
        for _ in 0..runs {
            let mut w = fleet(mode);
            let mut t = Vec::with_capacity(ticks);
            for _ in 0..ticks {
                w.step(&idle);
                // 10 passes back to back (same world state), averaged: less timer noise.
                let t0 = Instant::now();
                for _ in 0..10 {
                    w.think_all_behaviors_now();
                }
                t.push(t0.elapsed().as_secs_f64() * 1e5);
            }
            per_run.push(stats(t));
        }
        report(label, per_run);
    }
}
