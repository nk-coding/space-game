//! Prints the per-tick state hashes of an arena fight, one `tick hash` line per tick, until the fight ends or
//! `--ticks` have run. Built natively and for `wasm32-wasip1`; `scripts/determinism-check.sh --fight NAME` runs
//! both and compares.
//!
//! Usage: `fight_hashes [--scenario runner_vs_raiders|skiff_mirror|close_quarters|warden|SGF1:…] [--seed N]
//! [--ticks N]`: the fight specs of `crates/core/arena/fights/` and the rules probes of `probes/` (compiled in: the
//! WASI run has no filesystem), with the seed replaced, or a fight code (GDR-0092) with its own seed, checked
//! against its record at the end (`# record=as recorded`).

const FIGHTS: [(&str, &str); 4] = [
    ("runner_vs_raiders", include_str!("../fights/runner_vs_raiders.ron")),
    ("skiff_mirror", include_str!("../fights/skiff_mirror.ron")),
    // The rules probes (`arena::rules_fingerprint`), with their own seeds 1 and 2.
    ("close_quarters", include_str!("../probes/close_quarters.ron")),
    ("warden", include_str!("../probes/warden.ron")),
];

fn main() {
    let mut seed: u64 = 1;
    let mut ticks: u64 = 2700;
    let mut name = String::from("runner_vs_raiders");
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--seed" => seed = val().parse().expect("--seed: integer"),
            "--ticks" => ticks = val().parse().expect("--ticks: integer"),
            "--scenario" => name = val(),
            other => panic!("unknown argument {other}"),
        }
    }
    let code = name
        .starts_with(arena::CODE_TAG)
        .then(|| arena::FightCode::decode(&name).expect("the fight code reads"));
    let spec = match &code {
        Some(code) => code.spec.clone(),
        None => {
            let text = FIGHTS
                .iter()
                .find(|(n, _)| *n == name)
                .unwrap_or_else(|| {
                    panic!("unknown fight {name} (known: runner_vs_raiders, skiff_mirror, close_quarters, warden, or a fight code)")
                })
                .1;
            let mut spec = arena::FightSpec::from_ron(text).expect("the fight spec reads");
            spec.seed = seed;
            spec
        }
    };
    let content = data::base_content();
    let mut fight = arena::Fight::new(&content, &spec).expect("the fight builds");
    match &code {
        Some(code) => {
            let stale = code.rules != arena::rules_fingerprint(&content);
            let rules = if stale { "changed" } else { "same" };
            println!("# arena fight=code seed={} ticks={ticks} rules={rules}", spec.seed);
        }
        None => println!("# arena fight={name} seed={seed} ticks={ticks}"),
    }
    let mut result = None;
    for t in 1..=ticks {
        result = fight.step(1);
        println!("{t} {:016x}", fight.sim().state_hash());
        if result.is_some() {
            break;
        }
    }
    match result {
        Some(r) => println!(
            "# outcome={:?} ticks={} build_points_lost={:.3}/{:.3} damage_dealt={:.3}/{:.3} score={:.4}/{:.4} final={:016x}",
            r.outcome,
            r.ticks,
            r.sides[0].build_points_lost,
            r.sides[1].build_points_lost,
            r.sides[0].damage_dealt,
            r.sides[1].damage_dealt,
            r.sides[0].score,
            r.sides[1].score,
            r.state_hash
        ),
        None => println!("# still running after {ticks} ticks"),
    }
    if let Some(code) = &code {
        let same = result == Some(code.record);
        println!("# record={}", if same { "as recorded" } else { "DIFFERS" });
    }
}
