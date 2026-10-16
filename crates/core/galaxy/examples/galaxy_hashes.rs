//! Prints the generation hash of the base content's galaxy for `--count` seeds from `--seed` on, one `seed hash` line
//! each (and, as a comment, what the galaxy holds). Built natively and for `wasm32-wasip1`;
//! `scripts/determinism-check.sh --galaxy N` runs both and compares (ADR-0042).
//!
//! Usage: `galaxy_hashes [--seed N] [--count N] [--galaxy ID]`.

fn main() {
    let (mut seed, mut count, mut id) = (1u64, 20u64, String::from("galaxy"));
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--seed" => seed = val().parse().expect("--seed: integer"),
            "--count" => count = val().parse().expect("--count: integer"),
            "--galaxy" => id = val(),
            other => panic!("unknown argument {other}"),
        }
    }
    let content = data::base_content();
    println!("# galaxy={id} seeds={seed}..{} version={}", seed + count, galaxy::GENERATOR_VERSION);
    for s in seed..seed + count {
        let g = galaxy::generate(&content, &id, s).unwrap_or_else(|e| panic!("seed {s}: {e}"));
        // The interiors are part of what a seed means: their hash follows the skeleton's on the line.
        let interiors: Vec<galaxy::Interior> =
            (0..g.clusters.len() as u32).map(|k| galaxy::interior(&content, &g, k).expect("an interior")).collect();
        let sites: usize = interiors.iter().map(|i| i.sites.len()).sum();
        println!("# seed {s}: {} clusters, {} trails, {sites} sites", g.clusters.len(), g.trails.len());
        let text = ron::to_string(&interiors).expect("interiors write as RON");
        println!("{s} {:016x}{:016x}", g.hash(), data::hash::fnv1a64(text.as_bytes()));
    }
}
