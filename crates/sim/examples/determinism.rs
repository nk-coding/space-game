//! Prints the per-tick state hashes of the deterministic scenario, one `tick hash` line per tick.
//! Built natively and for `wasm32-wasip1`; `scripts/determinism-check.sh` runs both and compares.
//!
//! Usage: `determinism [--seed N] [--ticks N] [--scenario core]`

fn main() {
    let mut seed: u64 = 42;
    let mut ticks: u64 = 900;
    let mut scenario = String::from("core");
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--seed" => seed = val().parse().expect("--seed: integer"),
            "--ticks" => ticks = val().parse().expect("--ticks: integer"),
            "--scenario" => scenario = val(),
            other => panic!("unknown argument {other}"),
        }
    }
    let (hashes, stats) = match scenario.as_str() {
        "core" => sim::scenario::run(seed, ticks),
        other => panic!("unknown scenario {other} (known: core)"),
    };
    println!("# sim determinism scenario={scenario} seed={seed} ticks={ticks}");
    for (t, h) in hashes.iter().enumerate() {
        println!("{} {:016x}", t + 1, h);
    }
    let mut all = sim::StateHasher::new();
    for &h in &hashes {
        all.u64(h);
    }
    println!(
        "# final={:016x} parts_destroyed={} splits={} pieces_spawned={} pieces_destroyed={} overheated_part_ticks={}",
        all.finish(),
        stats.parts_destroyed,
        stats.splits,
        stats.pieces_spawned,
        stats.pieces_destroyed,
        stats.overheated_part_ticks
    );
}
