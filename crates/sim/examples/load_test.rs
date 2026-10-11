//! The shared load test (`sim::scenario::load_test`): n ships, half Halvex Runners and half automaton
//! raiders, all under the combat brain, engaging at once. Reports step times and what happened. Native only
//! (wall-clock is used here, in the tool, never in the sim); the browser client runs the same setup.
//!
//! `cargo run --release -p sim --example load_test -- [--ships 1,5,20] [--ticks 1800] [--seed 42]`
//! (or `scripts/load-test-native.sh`).

use std::time::Instant;

use sim::{SimEvent, TickInputs};

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
    println!("# load test: {ticks} ticks, seed {seed}; step times in ms (release, native)");
    println!(
        "| ships | mean | median | p95 | max | pieces max | projectiles avg / max | shots | hits | splits | active at end | final hash |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
    for &n in &ships {
        let mut world = sim::scenario::load_test(n, seed);
        let mut times = Vec::with_capacity(ticks);
        let (mut pieces_max, mut proj_sum, mut proj_max) = (0usize, 0usize, 0usize);
        let (mut shots, mut hits, mut splits) = (0u32, 0u32, 0u32);
        let idle = TickInputs::default();
        for _ in 0..ticks {
            let t0 = Instant::now();
            world.step(&idle);
            times.push(t0.elapsed().as_secs_f64() * 1e3);
            let pieces: usize = world.ships().iter().map(|s| s.pieces().len()).sum();
            pieces_max = pieces_max.max(pieces);
            proj_sum += world.projectiles().len();
            proj_max = proj_max.max(world.projectiles().len());
            for e in world.events() {
                match e {
                    SimEvent::ShotFired { .. } => shots += 1,
                    SimEvent::Hit { .. } => hits += 1,
                    SimEvent::Split(_) => splits += 1,
                    _ => {}
                }
            }
        }
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        times.sort_by(|a, b| a.total_cmp(b));
        let at = |q: f64| times[((times.len() as f64 * q) as usize).min(times.len() - 1)];
        let active = world.ships().iter().filter(|s| s.is_active()).count();
        println!(
            "| {n} | {mean:.3} | {:.3} | {:.3} | {:.3} | {pieces_max} | {:.1} / {proj_max} | {shots} | {hits} | {splits} | {active} | {:016x} |",
            at(0.5),
            at(0.95),
            times[times.len() - 1],
            proj_sum as f64 / ticks.max(1) as f64,
            world.state_hash()
        );
    }
}
