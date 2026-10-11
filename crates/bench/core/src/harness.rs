//! Runs one scenario on one engine and collects timings + hashes. Platform-free (clock is injected).

use crate::engine::{Clock, PhysicsEngine, RayStats};
use crate::hash::StateHasher;
use crate::scenario::{self, Ray, ScenarioSpec};

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub spec: ScenarioSpec,
    pub seed: u64,
    pub warmup: u32,
    pub ticks: u32,
}

pub const DEFAULT_SEED: u64 = 0x5EED_2026_1011;
pub const DEFAULT_WARMUP: u32 = 30;
pub const DEFAULT_TICKS: u32 = 900;

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub n: usize,
    pub mean: f64,
    pub median: f64,
    pub p95: f64,
    pub max: f64,
    pub min: f64,
}

impl Stats {
    pub fn of(samples: &[f64]) -> Stats {
        if samples.is_empty() {
            return Stats::default();
        }
        let mut s = samples.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = s.len();
        let rank = |p: f64| -> f64 {
            // Nearest-rank percentile.
            let k = ((p * n as f64).ceil() as usize).clamp(1, n);
            s[k - 1]
        };
        let median = if n % 2 == 1 { s[n / 2] } else { 0.5 * (s[n / 2 - 1] + s[n / 2]) };
        Stats { n, mean: s.iter().sum::<f64>() / n as f64, median, p95: rank(0.95), max: s[n - 1], min: s[0] }
    }

    fn json(&self) -> String {
        format!(
            "{{\"n\":{},\"mean\":{:.4},\"median\":{:.4},\"p95\":{:.4},\"max\":{:.4},\"min\":{:.4}}}",
            self.n, self.mean, self.median, self.p95, self.max, self.min
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct RunResult {
    pub engine: String,
    pub scenario: String,
    pub threads: u32,
    pub warmup: u32,
    pub ticks: u32,
    pub seed: u64,
    pub bodies_initial: usize,
    pub colliders_initial: usize,
    pub bodies_final: usize,
    pub colliders_final: usize,
    pub setup_ms: f64,
    /// Physics step only, per measured tick.
    pub step: Stats,
    /// Raycast batch per measured tick (5000 rays), if the scenario has raycasts.
    pub rays: Option<Stats>,
    /// Per breakage event in the measured window (remove/rebuild colliders, create pieces, set velocities).
    pub split: Option<Stats>,
    /// split + step + rays per measured tick.
    pub total: Stats,
    pub ray_count: u64,
    pub ray_hits: u64,
    pub ray_hash: u64,
    pub splits: u32,
    pub pieces: u32,
    /// Running hash over the per-tick state hashes of all ticks (warm-up included).
    pub state_hash: u64,
    /// (tick count, running hash after that many ticks)
    pub checkpoints: Vec<(u32, u64)>,
    /// Per-tick state hashes (only kept when requested).
    pub tick_hashes: Vec<u64>,
    /// SVG of the final state (only when requested; debug aid).
    pub svg: Option<String>,
    pub wall_ms: f64,
}

impl RunResult {
    /// One JSON object (no external deps). `extra` is spliced in verbatim (e.g. platform info), may be empty.
    pub fn to_json(&self, extra: &str) -> String {
        let opt = |s: &Option<Stats>| s.map(|s| s.json()).unwrap_or_else(|| "null".into());
        let cps: Vec<String> = self.checkpoints.iter().map(|(t, h)| format!("[{},\"{:016x}\"]", t, h)).collect();
        let mut s = format!(
            "{{\"engine\":\"{}\",\"scenario\":\"{}\",\"threads\":{},\"warmup\":{},\"ticks\":{},\"seed\":{},\
             \"bodies_initial\":{},\"colliders_initial\":{},\"bodies_final\":{},\"colliders_final\":{},\
             \"setup_ms\":{:.3},\"step\":{},\"rays\":{},\"split\":{},\"total\":{},\
             \"ray_count\":{},\"ray_hits\":{},\"ray_hash\":\"{:016x}\",\"splits\":{},\"pieces\":{},\
             \"state_hash\":\"{:016x}\",\"checkpoints\":[{}],\"wall_ms\":{:.1}",
            self.engine,
            self.scenario,
            self.threads,
            self.warmup,
            self.ticks,
            self.seed,
            self.bodies_initial,
            self.colliders_initial,
            self.bodies_final,
            self.colliders_final,
            self.setup_ms,
            self.step.json(),
            opt(&self.rays),
            opt(&self.split),
            self.total.json(),
            self.ray_count,
            self.ray_hits,
            self.ray_hash,
            self.splits,
            self.pieces,
            self.state_hash,
            cps.join(","),
            self.wall_ms,
        );
        if !self.tick_hashes.is_empty() {
            let th: Vec<String> = self.tick_hashes.iter().map(|h| format!("\"{:016x}\"", h)).collect();
            s.push_str(&format!(",\"tick_hashes\":[{}]", th.join(",")));
        }
        if !extra.is_empty() {
            s.push(',');
            s.push_str(extra);
        }
        s.push('}');
        s
    }
}

const CHECKPOINTS: &[u32] = &[1, 10, 30, 100, 300, 600];

/// Extra outputs of a run.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunOptions {
    /// Recorded in the result (the engine was created with this thread count).
    pub threads: u32,
    pub keep_tick_hashes: bool,
    pub svg: bool,
}

/// Run one scenario. The engine must be freshly created and empty.
pub fn run(engine: &mut dyn PhysicsEngine, params: RunParams, clock: &dyn Clock, opts: RunOptions) -> RunResult {
    let threads = opts.threads;
    let keep_tick_hashes = opts.keep_tick_hashes;
    let wall0 = clock.now_ms();
    let total_ticks = params.warmup + params.ticks;
    let world = scenario::generate(params.spec, params.seed, total_ticks);

    let t0 = clock.now_ms();
    for b in &world.bodies {
        engine.add_body(b);
    }
    let setup_ms = clock.now_ms() - t0;

    let mut res = RunResult {
        engine: engine.name(),
        scenario: params.spec.name.to_string(),
        threads,
        warmup: params.warmup,
        ticks: params.ticks,
        seed: params.seed,
        bodies_initial: engine.body_count(),
        colliders_initial: engine.collider_count(),
        setup_ms,
        ..Default::default()
    };

    let n = params.ticks as usize;
    let mut step_t = Vec::with_capacity(n);
    let mut ray_t = Vec::with_capacity(n);
    let mut total_t = Vec::with_capacity(n);
    let mut split_t = Vec::new();
    let mut rays: Vec<Ray> = Vec::with_capacity(scenario::RAYS_PER_TICK);
    let mut ray_stats = RayStats::default();
    let mut running = StateHasher::default();
    let mut next_break = 0;

    for tick in 0..total_ticks {
        let measured = tick >= params.warmup;

        let mut split_ms = 0.0;
        while next_break < world.breaks.len() && world.breaks[next_break].tick == tick {
            let ev = &world.breaks[next_break];
            let a = clock.now_ms();
            engine.split(ev);
            let d = clock.now_ms() - a;
            split_ms += d;
            if measured {
                split_t.push(d);
                res.splits += 1;
                res.pieces += ev.pieces.len() as u32;
            }
            next_break += 1;
        }

        let a = clock.now_ms();
        engine.step();
        let step_ms = clock.now_ms() - a;

        let mut ray_ms = 0.0;
        if params.spec.raycasts {
            scenario::rays_for_tick(params.seed, tick, &mut rays);
            let a = clock.now_ms();
            engine.cast_rays(&rays, &mut ray_stats);
            ray_ms = clock.now_ms() - a;
            ray_stats.rays += rays.len() as u64;
        }

        let mut h = StateHasher::default();
        engine.hash_state(&mut h);
        let th = h.finish();
        running.u64(th);
        if keep_tick_hashes {
            res.tick_hashes.push(th);
        }
        if CHECKPOINTS.contains(&(tick + 1)) {
            res.checkpoints.push((tick + 1, running.finish()));
        }

        if measured {
            step_t.push(step_ms);
            if params.spec.raycasts {
                ray_t.push(ray_ms);
            }
            total_t.push(step_ms + ray_ms + split_ms);
        }
    }
    res.checkpoints.push((total_ticks, running.finish()));

    res.step = Stats::of(&step_t);
    res.total = Stats::of(&total_t);
    if params.spec.raycasts {
        res.rays = Some(Stats::of(&ray_t));
    }
    if params.spec.breakage {
        res.split = Some(Stats::of(&split_t));
    }
    res.ray_count = ray_stats.rays;
    res.ray_hits = ray_stats.hits;
    res.ray_hash = ray_stats.hasher.finish();
    res.state_hash = running.finish();
    res.bodies_final = engine.body_count();
    res.colliders_final = engine.collider_count();
    if opts.svg {
        res.svg = Some(render_svg(&world, engine));
    }
    res.wall_ms = clock.now_ms() - wall0;
    res
}

/// Final state as SVG (all break events applied). Ships teal, pieces magenta, debris orange, asteroids grey.
pub fn render_svg(world: &scenario::World, engine: &dyn PhysicsEngine) -> String {
    use crate::scenario::{BodyKind, BodyRole, Shape};
    let mut shapes: Vec<(BodyRole, BodyKind, Vec<Shape>)> = world.bodies.iter().map(|b| (b.role, b.kind, b.shapes.clone())).collect();
    for ev in &world.breaks {
        shapes[ev.parent as usize].2 = ev.parent_shapes.clone();
        for (id, sh) in &ev.pieces {
            debug_assert_eq!(*id as usize, shapes.len());
            shapes.push((BodyRole::Piece, BodyKind::Dynamic, sh.clone()));
        }
    }
    let mut s = String::from("<svg xmlns='http://www.w3.org/2000/svg' viewBox='-760 -760 1520 1520' width='1520' height='1520' style='background:#111'><g transform='scale(1,-1)'>");
    for (id, (role, kind, sh)) in shapes.iter().enumerate() {
        if id >= engine.body_count() {
            break;
        }
        let (p, r) = engine.pose(id as u32);
        let col = match (role, kind) {
            (BodyRole::Ship, _) => "#3fd6b4",
            (BodyRole::Piece, _) => "#e05ad8",
            (BodyRole::Debris, _) => "#f0a040",
            (BodyRole::Asteroid, BodyKind::Static) => "#777",
            (BodyRole::Asteroid, _) => "#aaa",
        };
        s += &format!("<g transform='matrix({} {} {} {} {} {})' fill='{}'>", r[0], r[1], -r[1], r[0], p[0], p[1], col);
        for x in sh {
            match x {
                Shape::Box { cx, cy, hx, hy } => s += &format!("<rect x='{}' y='{}' width='{}' height='{}'/>", cx - hx, cy - hy, 2.0 * hx, 2.0 * hy),
                Shape::Polygon { verts } => {
                    s += "<polygon points='";
                    for v in verts {
                        s += &format!("{},{} ", v[0], v[1]);
                    }
                    s += "'/>";
                }
            }
        }
        s += "</g>";
    }
    s += "</g></svg>";
    s
}
