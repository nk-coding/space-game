//! Atlas rasterisation as the game spends it (`crates/client/kit/src/atlas.rs`): `Rasterizer::step` until the frame
//! budget of 6 ms is used, then the next frame, for a whole job at one tier (default 512), `runs` times (default
//! 9), each with a new layout (as the first job of a session: no soft layer rendered yet). Prints the job start
//! (`Rasterizer::new`), the longest step, the longest frame and the frames, as median and p95 over the runs:
//! `cargo run --release -p art --example profile -- 512 [runs]`. The VM is noisy: note `uptime` next to the
//! numbers.
use std::sync::Arc;
use std::time::Instant;

/// `atlas.rs` `FRAME_BUDGET_MS`.
const BUDGET_MS: f64 = 6.0;

fn main() {
    let mut args = std::env::args().skip(1);
    let tier: u32 = args.next().map(|t| t.parse().expect("tier")).unwrap_or(512);
    let runs: usize = args.next().map(|t| t.parse().expect("runs")).unwrap_or(9);
    let content = data::base_content();
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1e3;
    let (mut start, mut step, mut frame, mut frames, mut total) = (vec![], vec![], vec![], vec![], vec![]);
    for _ in 0..runs {
        let layout = Arc::new(art::Layout::new(&content));
        let t = Instant::now();
        let mut r = art::Rasterizer::new(layout.clone(), tier);
        start.push(ms(t));
        let (mut longest_step, mut longest_frame, mut n, mut work) = (0.0f64, 0.0f64, 0, 0.0);
        let mut done = false;
        while !done {
            let f = Instant::now();
            while !done && ms(f) < BUDGET_MS {
                let s = Instant::now();
                done = r.step().expect("step");
                longest_step = longest_step.max(ms(s));
            }
            let spent = ms(f);
            longest_frame = longest_frame.max(spent);
            work += spent;
            n += 1;
        }
        let t = Instant::now();
        let raster = r.finish().expect("finish");
        assert_eq!(raster.tier, tier);
        drop(raster);
        let end = ms(t);
        step.push(longest_step);
        frame.push(longest_frame.max(end));
        frames.push(n as f64);
        total.push(work + end);
    }
    println!("tier {tier}, {runs} runs, budget {BUDGET_MS} ms per frame: median / p95");
    for (name, v) in [
        ("job start (ms)", &mut start),
        ("longest step (ms)", &mut step),
        ("longest frame (ms)", &mut frame),
        ("frames", &mut frames),
        ("work (ms)", &mut total),
    ] {
        v.sort_by(f64::total_cmp);
        let p = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
        println!("  {name:20} {:8.2} {:8.2}", p(0.5), p(0.95));
    }
}
