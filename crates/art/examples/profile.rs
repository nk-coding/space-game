//! Time per region and per mip step at one tier (default 256):
//! `cargo run --release -p art --example profile -- 256`.
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
    // The mip chain, a band of rows per step.
    let (mut mips, mut longest, mut steps) = (0.0f64, 0.0f64, 0);
    loop {
        let t = Instant::now();
        let done = r.step().unwrap();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        mips += ms;
        longest = longest.max(ms);
        steps += 1;
        if done {
            break;
        }
    }
    let raster = r.finish().unwrap();
    times.sort_by(|a, b| b.0.total_cmp(&a.0));
    let total: f64 = times.iter().map(|t| t.0).sum();
    println!(
        "tier {tier}: regions {total:.1} ms, mips {mips:.1} ms in {steps} steps (longest {longest:.2} ms), {} levels",
        raster.levels
    );
    for (ms, name) in times.iter().take(12) {
        println!("  {ms:7.2} ms  {name}");
    }
}
