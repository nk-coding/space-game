//! Debug preview and timing of the runtime rasteriser. Never writes into the repo.
//!
//! `cargo run --release -p art --example preview` prints the rasterisation time per tier;
//! `... -- <tier> <out.png>` also writes the atlas at that tier as a PNG (use a scratch path).

use std::sync::Arc;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let t0 = Instant::now();
    let layout = Arc::new(art::Layout::new(&data::base_content()));
    println!("layout: {} regions, {:?} units ({:.1} ms)", layout.regions.len(), layout.size, t0.elapsed().as_secs_f64() * 1e3);
    let tiers: Vec<u32> = match args.first() {
        Some(t) => vec![t.parse().expect("tier")],
        None => vec![32, 64, 128, 256, 512],
    };
    for &tier in &tiers {
        // best of 3
        let mut best = f64::MAX;
        let mut raster = None;
        for _ in 0..3 {
            let t = Instant::now();
            let r = art::rasterise(&layout, tier).expect("rasterise");
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
            raster = Some(r);
        }
        let r = raster.unwrap();
        println!("tier {tier:4}: {}x{} px, {} mip levels, {:.1} ms (best of 3)", r.width, r.height, r.levels, best);
        if let Some(out) = args.get(1) {
            // premultiply level 0 for tiny-skia and save
            let n = (r.width * r.height * 4) as usize;
            let mut px = r.data[..n].to_vec();
            for p in px.as_chunks_mut::<4>().0 {
                let a = p[3] as u32;
                for c in &mut p[..3] {
                    *c = ((*c as u32 * a + 127) / 255) as u8;
                }
            }
            let pm = resvg::tiny_skia::Pixmap::from_vec(px, resvg::tiny_skia::IntSize::from_wh(r.width, r.height).unwrap()).unwrap();
            pm.save_png(out).expect("write png");
            println!("wrote {out}");
        }
    }
}
