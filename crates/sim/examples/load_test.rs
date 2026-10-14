//! The shared load test (`sim::voyage::scenario::load_test`): n ships, half Halvex Runners and half automaton
//! raiders, all under the content behavior `raider_basic`, engaging at once. Reports step times (the whole
//! step, and the Rapier step alone via `Sim::step_profiled`) and what happened. Native only (wall-clock is used
//! here, in the tool, never in the sim); the browser client runs the same setup.
//!
//! `cargo run --release -p sim --example load_test -- [--ships 1,5,20] [--ticks 1800] [--seed 42]`
//! (or `scripts/load-test-native.sh`).

use std::time::Instant;

use sim::{SimEvent, StepPhase, TickInputs};

/// Mean, median, p95 and max of per-tick times.
fn stats(mut v: Vec<f64>) -> [f64; 4] {
    let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
    v.sort_by(|a, b| a.total_cmp(b));
    let at = |q: f64| v[((v.len() as f64 * q) as usize).min(v.len() - 1)];
    [mean, at(0.5), at(0.95), v[v.len() - 1]]
}

fn main() {
    let (mut ships, mut ticks, mut seed) = (vec![1usize, 5, 20], 1800usize, 42u64);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let v = args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--ships" => {
                ships = v
                    .split(',')
                    .map(|x| x.trim().parse().expect("--ships: list of integers"))
                    .collect()
            }
            "--ticks" => ticks = v.parse().expect("--ticks: integer"),
            "--seed" => seed = v.parse().expect("--seed: integer"),
            _ => panic!("unknown argument {a}"),
        }
    }
    println!(
        "# load test: {ticks} ticks, seed {seed}; step times in ms (release, native): whole step mean / median / p95 / max, and the Rapier step alone mean / median / p95"
    );
    println!(
        "| ships | step mean | median | p95 | max | Rapier mean | median | p95 | pieces max | projectiles avg / max | shots | hits | impacts / rams | splits | active at end | final hash |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for &n in &ships {
        let mut world = sim::voyage::scenario::load_test(n, seed);
        let (mut times, mut rapier) = (Vec::with_capacity(ticks), Vec::with_capacity(ticks));
        let (mut pieces_max, mut proj_sum, mut proj_max) = (0usize, 0usize, 0usize);
        let (mut shots, mut hits, mut splits, mut impacts, mut rams) =
            (0u32, 0u32, 0u32, 0u32, 0u32);
        let idle = TickInputs::default();
        for _ in 0..ticks {
            let t0 = Instant::now();
            let (mut p0, mut p1) = (t0, t0);
            world.step_profiled(&idle, |phase| match phase {
                StepPhase::Physics => p0 = Instant::now(),
                StepPhase::Impacts => p1 = Instant::now(),
                _ => {}
            });
            times.push(t0.elapsed().as_secs_f64() * 1e3);
            rapier.push((p1 - p0).as_secs_f64() * 1e3);
            let pieces: usize = world.ships().iter().map(|s| s.pieces().len()).sum();
            pieces_max = pieces_max.max(pieces);
            proj_sum += world.projectiles().len();
            proj_max = proj_max.max(world.projectiles().len());
            for e in world.events() {
                match e {
                    SimEvent::ShotFired { .. } => shots += 1,
                    SimEvent::Hit { .. } => hits += 1,
                    SimEvent::Split(_) => splits += 1,
                    SimEvent::Impact(c) => {
                        impacts += 1;
                        rams += (c.damage > 0.0) as u32;
                    }
                    _ => {}
                }
            }
        }
        let [mean, median, p95, max] = stats(times);
        let [r_mean, r_median, r_p95, _] = stats(rapier);
        let active = world.ships().iter().filter(|s| s.is_active()).count();
        println!(
            "| {n} | {mean:.3} | {median:.3} | {p95:.3} | {max:.3} | {r_mean:.3} | {r_median:.3} | {r_p95:.3} | {pieces_max} | {:.1} / {proj_max} | {shots} | {hits} | {impacts} / {rams} | {splits} | {active} | {:016x} |",
            proj_sum as f64 / ticks.max(1) as f64,
            world.state_hash()
        );
    }
}
