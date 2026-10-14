//! Prints the per-tick state hashes of the deterministic scenario, one `tick hash` line per tick.
//! Built natively and for `wasm32-wasip1`; `scripts/determinism-check.sh` runs both and compares.
//!
//! Usage: `determinism [--seed N] [--ticks N] [--scenario core|flight|load|behavior|salvage|voyage] [--ships N]`
//! (`--ships` sets the load test's ship count, default 20).

fn main() {
    let mut seed: u64 = 42;
    let mut ticks: u64 = 900;
    let mut scenario = String::from("core");
    let mut ships: u32 = 20;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--seed" => seed = val().parse().expect("--seed: integer"),
            "--ticks" => ticks = val().parse().expect("--ticks: integer"),
            "--scenario" => scenario = val(),
            "--ships" => ships = val().parse().expect("--ships: integer"),
            other => panic!("unknown argument {other}"),
        }
    }
    let mut kind = sim::voyage::scenario::ScenarioKind::from_name(&scenario).unwrap_or_else(|| {
        panic!("unknown scenario {scenario} (known: core, flight, load, behavior, salvage, voyage)")
    });
    if let sim::voyage::scenario::ScenarioKind::Load(_) = kind {
        kind = sim::voyage::scenario::ScenarioKind::Load(ships);
    }
    let (hashes, stats) = sim::voyage::scenario::run_kind(kind, seed, ticks);
    println!("# sim determinism scenario={scenario} seed={seed} ticks={ticks}");
    for (t, h) in hashes.iter().enumerate() {
        println!("{} {:016x}", t + 1, h);
    }
    let mut all = sim::StateHasher::new();
    for &h in &hashes {
        all.u64(h);
    }
    println!(
        "# final={:016x} parts_destroyed={} splits={} pieces_spawned={} pieces_destroyed={} overheated_part_ticks={} firing_ticks={} shots={} hits={} shield_hits={} ricochets={} script_evals={} impacts={} rams={} burns={} explosions={} split_vessels={} vessels_lost={} dormant={} woken={} focus_changes={} collected={} collected_units={} collected_cargo={} collected_fragments={} fragments={} sites_woken={} sites_cleared={} recall_ticks={}",
        all.finish(),
        stats.parts_destroyed,
        stats.splits,
        stats.pieces_spawned,
        stats.pieces_destroyed,
        stats.overheated_part_ticks,
        stats.firing_ticks,
        stats.shots,
        stats.hits,
        stats.shield_hits,
        stats.ricochets,
        stats.script_evals,
        stats.impacts,
        stats.rams,
        stats.burns,
        stats.explosions,
        stats.split_vessels,
        stats.vessels_lost,
        stats.dormant,
        stats.woken,
        stats.focus_changes,
        stats.collected,
        stats.collected_units,
        stats.collected_cargo,
        stats.collected_fragments,
        stats.fragments,
        stats.sites_woken,
        stats.sites_cleared,
        stats.recall_ticks
    );
}
