//! Time per region at one tier (default 256): `cargo run --release -p art --example profile -- 256`.
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let tier: u32 = std::env::args().nth(1).map(|t| t.parse().unwrap()).unwrap_or(256);
    let layout = Arc::new(art::Layout::new(&data::base_content()));
    let mut r = art::Rasterizer::new(layout.clone(), tier);
    let mut times = Vec::new();
    for reg in &layout.regions {
        let t = Instant::now();
        r.step().unwrap();
        times.push((t.elapsed().as_secs_f64() * 1e3, reg.name.clone()));
    }
    let t = Instant::now();
    let _raster = r.finish();
    let mips = t.elapsed().as_secs_f64() * 1e3;
    times.sort_by(|a, b| b.0.total_cmp(&a.0));
    let total: f64 = times.iter().map(|t| t.0).sum();
    println!("tier {tier}: regions {total:.1} ms, mips {mips:.1} ms");
    for (ms, name) in times.iter().take(12) {
        println!("  {ms:7.2} ms  {name}");
    }
}
