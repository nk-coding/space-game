//! Engine-independent world generation. Every engine gets exactly the same bodies, shapes, velocities,
//! raycasts and breakage events. All coordinates in metres, 1 cell = 1 m, zero gravity.

use crate::rng::{Rng, sin_cos};

/// Collider shape in the body's local frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// Axis-aligned box (in body space): centre and half extents.
    Box { cx: f32, cy: f32, hx: f32, hy: f32 },
    /// Convex polygon, counter-clockwise, 3..=8 vertices (slope triangles, asteroids).
    Polygon { verts: Vec<[f32; 2]> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Dynamic,
    Static,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyRole {
    Ship,
    Debris,
    Asteroid,
    /// A piece split off a ship by a breakage event.
    Piece,
}

#[derive(Clone, Debug)]
pub struct BodyDef {
    /// Stable index; engines keep bodies in this order for hashing. Equals the body's position in the list.
    pub id: u32,
    pub kind: BodyKind,
    pub role: BodyRole,
    pub pos: [f32; 2],
    /// Rotation as (cos, sin), unit length.
    pub rot: [f32; 2],
    /// Linear velocity of the centre of mass.
    pub linvel: [f32; 2],
    pub angvel: f32,
    pub shapes: Vec<Shape>,
}

/// Material used for every shape in every engine.
pub const DENSITY: f32 = 1.0;
pub const FRICTION: f32 = 0.5;
pub const RESTITUTION: f32 = 0.0;
/// Fixed step (30 Hz).
pub const DT: f32 = 1.0 / 30.0;
/// Solver substeps / iterations used for every engine (see README for how each engine interprets it).
pub const SUBSTEPS: u32 = 4;

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: [f32; 2],
    /// Unit direction.
    pub dir: [f32; 2],
    pub max_dist: f32,
}

pub const RAYS_PER_TICK: usize = 5000;
pub const RAY_LENGTH: f32 = 200.0;

/// A ship splits: the parent body keeps the largest piece (its colliders are rebuilt), every other piece
/// becomes a new body. All shapes are in the parent's local frame; new bodies start at the parent's pose.
#[derive(Clone, Debug)]
pub struct BreakEvent {
    /// Tick index (counting from the first warm-up tick) before whose step the event is applied.
    pub tick: u32,
    pub parent: u32,
    pub parent_shapes: Vec<Shape>,
    /// (new body id, shapes in the parent frame)
    pub pieces: Vec<(u32, Vec<Shape>)>,
    pub removed_cells: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColliderMode {
    /// One 1x1 m box per cell (+ slope triangles).
    Cells,
    /// Greedy-merged rectangles per ship (+ slope triangles).
    Merged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScenarioSpec {
    pub name: &'static str,
    pub ships: bool,
    pub debris: bool,
    pub asteroids: bool,
    pub raycasts: bool,
    pub breakage: bool,
    pub mode: ColliderMode,
}

pub const SCENARIOS: &[ScenarioSpec] = &[
    ScenarioSpec { name: "ships", ships: true, debris: false, asteroids: false, raycasts: false, breakage: false, mode: ColliderMode::Cells },
    ScenarioSpec { name: "ships-merged", ships: true, debris: false, asteroids: false, raycasts: false, breakage: false, mode: ColliderMode::Merged },
    ScenarioSpec { name: "debris", ships: false, debris: true, asteroids: false, raycasts: false, breakage: false, mode: ColliderMode::Cells },
    ScenarioSpec { name: "raycasts", ships: true, debris: true, asteroids: false, raycasts: true, breakage: false, mode: ColliderMode::Cells },
    ScenarioSpec { name: "breakage", ships: true, debris: false, asteroids: false, raycasts: false, breakage: true, mode: ColliderMode::Cells },
    ScenarioSpec { name: "breakage-merged", ships: true, debris: false, asteroids: false, raycasts: false, breakage: true, mode: ColliderMode::Merged },
    ScenarioSpec { name: "asteroids", ships: false, debris: false, asteroids: true, raycasts: false, breakage: false, mode: ColliderMode::Cells },
    ScenarioSpec { name: "combined", ships: true, debris: true, asteroids: true, raycasts: true, breakage: true, mode: ColliderMode::Cells },
    ScenarioSpec { name: "combined-merged", ships: true, debris: true, asteroids: true, raycasts: true, breakage: true, mode: ColliderMode::Merged },
];

pub fn scenario(name: &str) -> Option<ScenarioSpec> {
    SCENARIOS.iter().copied().find(|s| s.name == name)
}

pub const SHIP_COUNT: usize = 20;
pub const SHIP_MIN_CELLS: usize = 200;
pub const SHIP_MAX_CELLS: usize = 2000;
pub const DEBRIS_CLOUDS: usize = 8;
pub const DEBRIS_PER_CLOUD: usize = 250;
pub const ASTEROID_COUNT: usize = 300;
pub const BREAK_INTERVAL_TICKS: u32 = 30;

// ---------------------------------------------------------------------------------------------------------
// Ship grids

pub const EMPTY: u8 = 0;
pub const BLOCK: u8 = 1;
/// Slopes: the named corner of the cell is cut away (that corner's two neighbours are empty).
pub const SLOPE_NE: u8 = 2;
pub const SLOPE_NW: u8 = 3;
pub const SLOPE_SE: u8 = 4;
pub const SLOPE_SW: u8 = 5;

/// A ship's cell grid. +y is forward (nose).
#[derive(Clone, Debug)]
pub struct Grid {
    pub w: i32,
    pub h: i32,
    pub cells: Vec<u8>,
    /// Body origin in grid coordinates (centroid of the original cells); shapes are relative to it.
    pub origin: [f64; 2],
}

impl Grid {
    fn new(w: i32, h: i32) -> Self {
        Grid { w, h, cells: vec![EMPTY; (w * h) as usize], origin: [0.0, 0.0] }
    }
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h { EMPTY } else { self.cells[(y * self.w + x) as usize] }
    }
    fn set(&mut self, x: i32, y: i32, v: u8) {
        self.cells[(y * self.w + x) as usize] = v;
    }
    pub fn count(&self) -> usize {
        self.cells.iter().filter(|&&c| c != EMPTY).count()
    }
    fn compute_origin(&mut self) {
        let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for y in 0..self.h {
            for x in 0..self.w {
                if self.get(x, y) != EMPTY {
                    sx += x as f64 + 0.5;
                    sy += y as f64 + 0.5;
                    n += 1.0;
                }
            }
        }
        self.origin = [sx / n, sy / n];
    }
    /// Bounding radius around the origin (cell corners).
    pub fn radius(&self) -> f64 {
        let mut r2: f64 = 0.0;
        for y in 0..self.h {
            for x in 0..self.w {
                if self.get(x, y) != EMPTY {
                    for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                        let px = x as f64 + dx - self.origin[0];
                        let py = y as f64 + dy - self.origin[1];
                        r2 = r2.max(px * px + py * py);
                    }
                }
            }
        }
        r2.sqrt()
    }

    /// Turn exposed convex corners into slopes (both neighbours of one corner empty, the other two filled).
    fn add_slopes(&mut self) {
        let mut out = self.cells.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                if self.get(x, y) != BLOCK {
                    continue;
                }
                let up = self.get(x, y + 1) != EMPTY;
                let down = self.get(x, y - 1) != EMPTY;
                let left = self.get(x - 1, y) != EMPTY;
                let right = self.get(x + 1, y) != EMPTY;
                let s = match (up, down, left, right) {
                    (false, true, true, false) => SLOPE_NE,
                    (false, true, false, true) => SLOPE_NW,
                    (true, false, true, false) => SLOPE_SE,
                    (true, false, false, true) => SLOPE_SW,
                    _ => BLOCK,
                };
                out[(y * self.w + x) as usize] = s;
            }
        }
        self.cells = out;
    }

    /// Shapes for a subset of cells (`mask[i]` true = include), relative to the grid origin.
    pub fn shapes(&self, mask: &[bool], mode: ColliderMode) -> Vec<Shape> {
        let ox = self.origin[0];
        let oy = self.origin[1];
        let mut shapes = Vec::new();
        let cell_center = |x: i32, y: i32| ((x as f64 + 0.5 - ox) as f32, (y as f64 + 0.5 - oy) as f32);
        // Slopes are always triangles.
        for y in 0..self.h {
            for x in 0..self.w {
                let i = (y * self.w + x) as usize;
                if !mask[i] {
                    continue;
                }
                let c = self.cells[i];
                if c >= SLOPE_NE {
                    let (cx, cy) = cell_center(x, y);
                    let local: [[f32; 2]; 3] = match c {
                        SLOPE_NE => [[-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5]],
                        SLOPE_NW => [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5]],
                        SLOPE_SE => [[-0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
                        _ => [[0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
                    };
                    shapes.push(Shape::Polygon { verts: local.iter().map(|v| [v[0] + cx, v[1] + cy]).collect() });
                }
            }
        }
        match mode {
            ColliderMode::Cells => {
                for y in 0..self.h {
                    for x in 0..self.w {
                        let i = (y * self.w + x) as usize;
                        if mask[i] && self.cells[i] == BLOCK {
                            let (cx, cy) = cell_center(x, y);
                            shapes.push(Shape::Box { cx, cy, hx: 0.5, hy: 0.5 });
                        }
                    }
                }
            }
            ColliderMode::Merged => {
                // Greedy merge: extend right, then down while the whole row segment is free.
                let mut used = vec![false; self.cells.len()];
                let free = |x: i32, y: i32, used: &Vec<bool>| {
                    if x < 0 || y < 0 || x >= self.w || y >= self.h {
                        return false;
                    }
                    let i = (y * self.w + x) as usize;
                    mask[i] && self.cells[i] == BLOCK && !used[i]
                };
                for y in 0..self.h {
                    for x in 0..self.w {
                        if !free(x, y, &used) {
                            continue;
                        }
                        let mut w = 1;
                        while free(x + w, y, &used) {
                            w += 1;
                        }
                        let mut h = 1;
                        'grow: loop {
                            for xx in x..x + w {
                                if !free(xx, y + h, &used) {
                                    break 'grow;
                                }
                            }
                            h += 1;
                        }
                        for yy in y..y + h {
                            for xx in x..x + w {
                                used[(yy * self.w + xx) as usize] = true;
                            }
                        }
                        let cx = (x as f64 + w as f64 * 0.5 - ox) as f32;
                        let cy = (y as f64 + h as f64 * 0.5 - oy) as f32;
                        shapes.push(Shape::Box { cx, cy, hx: w as f32 * 0.5, hy: h as f32 * 0.5 });
                    }
                }
            }
        }
        shapes
    }

    pub fn full_mask(&self) -> Vec<bool> {
        self.cells.iter().map(|&c| c != EMPTY).collect()
    }
}

/// Generate a symmetric, tapered ship hull with wings and edge notches, at least `target` cells.
pub fn gen_ship(rng: &mut Rng, target: usize) -> Grid {
    let aspect = rng.range(1.5, 2.4);
    let nose = rng.range(0.2, 0.4);
    let tail = rng.range(0.0, 0.12);
    let wing_pos = rng.range(0.15, 0.5);
    let wing_len = rng.range(0.1, 0.25);
    let wing_ext = rng.range(0.2, 0.6);
    let notch_count = rng.int(0, 4) as usize;
    let notches: Vec<(f64, f64, i32)> =
        (0..notch_count).map(|_| (rng.range(0.1, 0.8), rng.range(0.03, 0.08), rng.int(1, 3) as i32)).collect();

    let mut len = 10;
    loop {
        let wid = ((len as f64 / aspect) + 0.5) as i32;
        let max_half = ((wid as f64 * 0.5) * (1.0 + wing_ext)) as i32 + 2;
        let gw = max_half * 2 + 2;
        let mut g = Grid::new(gw, len);
        let cx = gw / 2;
        for r in 0..len {
            let t = (r as f64 + 0.5) / len as f64;
            let mut hw = wid as f64 * 0.5;
            if t > 1.0 - nose {
                hw *= 0.12 + 0.88 * (1.0 - t) / nose;
            }
            if t < tail {
                hw *= 0.7 + 0.3 * t / tail;
            }
            if t >= wing_pos && t < wing_pos + wing_len {
                hw += wing_ext * wid as f64 * 0.5;
            }
            let mut half = ((hw + 0.5) as i32).max(1);
            for &(np, nl, depth) in &notches {
                if t >= np && t < np + nl {
                    half = (half - depth).max(1);
                }
            }
            for x in cx - half..cx + half {
                g.set(x, r, BLOCK);
            }
        }
        if g.count() >= target {
            g.add_slopes();
            g.compute_origin();
            return g;
        }
        len += 1;
    }
}

/// Polyomino offsets for debris (1..4 cells).
const DEBRIS_SHAPES: &[&[(i32, i32)]] = &[
    &[(0, 0)],
    &[(0, 0)],
    &[(0, 0)],
    &[(0, 0), (1, 0)],
    &[(0, 0), (1, 0)],
    &[(0, 0), (1, 0), (2, 0)],
    &[(0, 0), (1, 0), (0, 1)],
    &[(0, 0), (1, 0), (2, 0), (3, 0)],
    &[(0, 0), (1, 0), (0, 1), (1, 1)],
    &[(0, 0), (1, 0), (2, 0), (1, 1)],
    &[(0, 0), (1, 0), (2, 0), (2, 1)],
    &[(0, 0), (1, 0), (1, 1), (2, 1)],
];

// ---------------------------------------------------------------------------------------------------------
// World

/// The full world (all elements). A scenario selects a subset; ids are re-assigned per scenario.
pub struct World {
    pub spec: ScenarioSpec,
    pub seed: u64,
    pub bodies: Vec<BodyDef>,
    /// Ship grids by body id of the ship (for breakage), index = ship number.
    pub ship_ids: Vec<u32>,
    pub breaks: Vec<BreakEvent>,
}

struct Placed {
    c: [f64; 2],
    r: f64,
}

fn overlaps(placed: &[Placed], c: [f64; 2], r: f64, margin: f64) -> bool {
    placed.iter().any(|p| {
        let dx = p.c[0] - c[0];
        let dy = p.c[1] - c[1];
        let rr = p.r + r + margin;
        dx * dx + dy * dy < rr * rr
    })
}

fn f2(v: [f64; 2]) -> [f32; 2] {
    [v[0] as f32, v[1] as f32]
}

/// Ship sizes: evenly spread 200..2000 cells, order shuffled so pairs mix sizes.
pub fn ship_targets(seed: u64) -> Vec<usize> {
    let mut sizes: Vec<usize> = (0..SHIP_COUNT)
        .map(|i| SHIP_MIN_CELLS + (SHIP_MAX_CELLS - SHIP_MIN_CELLS) * i / (SHIP_COUNT - 1))
        .collect();
    let mut rng = Rng::stream(seed, 1);
    for i in (1..sizes.len()).rev() {
        let j = rng.int(0, i as i64) as usize;
        sizes.swap(i, j);
    }
    sizes
}

pub fn generate(spec: ScenarioSpec, seed: u64, total_ticks: u32) -> World {
    let mut placed: Vec<Placed> = Vec::new();
    let mut bodies: Vec<BodyDef> = Vec::new();
    let mut ship_ids = Vec::new();
    let mut grids: Vec<Grid> = Vec::new();

    // Ships are always generated (and placed) so that debris/asteroid positions are the same in every scenario.
    let targets = ship_targets(seed);
    for (i, &t) in targets.iter().enumerate() {
        let mut rng = Rng::stream(seed, 100 + i as u64);
        grids.push(gen_ship(&mut rng, t));
    }
    let mut ship_place = Vec::new(); // (pos, rot, linvel, angvel) per ship
    {
        let mut rng = Rng::stream(seed, 2);
        for pair in 0..SHIP_COUNT / 2 {
            let (a, b) = (pair * 2, pair * 2 + 1);
            let (ra, rb) = (grids[a].radius(), grids[b].radius());
            let mut attempt = 0;
            loop {
                attempt += 1;
                let d = rng.unit();
                let closing = rng.range(3.0, 6.0);
                let meet = rng.range(3.0, 15.0);
                let gap = closing * meet;
                let lateral = rng.range(-0.5, 0.5) * ra.min(rb);
                let perp = [-d[1], d[0]];
                let half_span = (ra + rb + gap) * 0.5;
                let extent = 450.0 + (attempt / 200) as f64 * 50.0;
                let c = [rng.range(-extent, extent), rng.range(-extent, extent)];
                let pa = [c[0] - d[0] * (half_span - ra), c[1] - d[1] * (half_span - ra)];
                let pb = [
                    c[0] + d[0] * (half_span - rb) + perp[0] * lateral,
                    c[1] + d[1] * (half_span - rb) + perp[1] * lateral,
                ];
                if overlaps(&placed, pa, ra, 15.0) || overlaps(&placed, pb, rb, 15.0) {
                    continue;
                }
                let dist_ab = ((pa[0] - pb[0]).powi(2) + (pa[1] - pb[1]).powi(2)).sqrt();
                if dist_ab < ra + rb + 2.0 {
                    continue;
                }
                placed.push(Placed { c: pa, r: ra });
                placed.push(Placed { c: pb, r: rb });
                let drift = rng.unit();
                let drift_mag = rng.range(0.0, 2.0);
                let base = [drift[0] * drift_mag, drift[1] * drift_mag];
                let va = [base[0] + d[0] * closing * 0.5, base[1] + d[1] * closing * 0.5];
                let vb = [base[0] - d[0] * closing * 0.5, base[1] - d[1] * closing * 0.5];
                for (p, v, r) in [(pa, va, ra), (pb, vb, rb)] {
                    let rot = rng.unit();
                    // Big ships spin slower: tip speed up to ~3 m/s.
                    let w = rng.range(-1.0, 1.0) * (3.0 / r).min(0.3);
                    ship_place.push((p, rot, v, w));
                }
                break;
            }
        }
    }
    if spec.ships {
        for (i, g) in grids.iter().enumerate() {
            let (p, rot, v, w) = ship_place[i];
            let id = bodies.len() as u32;
            ship_ids.push(id);
            bodies.push(BodyDef {
                id,
                kind: BodyKind::Dynamic,
                role: BodyRole::Ship,
                pos: f2(p),
                rot: f2(rot),
                linvel: f2(v),
                angvel: w as f32,
                shapes: g.shapes(&g.full_mask(), spec.mode),
            });
        }
    }

    // Debris clouds.
    {
        let mut rng = Rng::stream(seed, 3);
        let cloud_r = 40.0;
        let mut clouds = Vec::new();
        for _ in 0..DEBRIS_CLOUDS {
            loop {
                let c = [rng.range(-450.0, 450.0), rng.range(-450.0, 450.0)];
                if !overlaps(&placed, c, cloud_r, 10.0) && !overlaps(&clouds, c, cloud_r, 10.0) {
                    clouds.push(Placed { c, r: cloud_r });
                    break;
                }
            }
        }
        let mut chunks: Vec<Placed> = Vec::new();
        for cl in &clouds {
            let drift = rng.unit();
            let drift_mag = rng.range(0.0, 2.0);
            for _ in 0..DEBRIS_PER_CLOUD {
                let shape = DEBRIS_SHAPES[rng.int(0, DEBRIS_SHAPES.len() as i64 - 1) as usize];
                let n = shape.len() as f64;
                let ox = shape.iter().map(|c| c.0 as f64 + 0.5).sum::<f64>() / n;
                let oy = shape.iter().map(|c| c.1 as f64 + 0.5).sum::<f64>() / n;
                let mut r: f64 = 0.0;
                for &(x, y) in shape {
                    for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                        let px = x as f64 + dx - ox;
                        let py = y as f64 + dy - oy;
                        r = r.max((px * px + py * py).sqrt());
                    }
                }
                let pos = loop {
                    let u = rng.unit();
                    let rad = cl.r * rng.f64().sqrt();
                    let p = [cl.c[0] + u[0] * rad, cl.c[1] + u[1] * rad];
                    if !overlaps(&chunks, p, r, 0.05) {
                        break p;
                    }
                };
                chunks.push(Placed { c: pos, r });
                let rv = rng.unit();
                let rv_mag = rng.range(0.0, 2.5);
                let v = [drift[0] * drift_mag + rv[0] * rv_mag, drift[1] * drift_mag + rv[1] * rv_mag];
                let rot = rng.unit();
                let w = rng.range(-1.5, 1.5);
                if spec.debris {
                    let id = bodies.len() as u32;
                    bodies.push(BodyDef {
                        id,
                        kind: BodyKind::Dynamic,
                        role: BodyRole::Debris,
                        pos: f2(pos),
                        rot: f2(rot),
                        linvel: f2(v),
                        angvel: w as f32,
                        shapes: shape
                            .iter()
                            .map(|&(x, y)| Shape::Box {
                                cx: (x as f64 + 0.5 - ox) as f32,
                                cy: (y as f64 + 0.5 - oy) as f32,
                                hx: 0.5,
                                hy: 0.5,
                            })
                            .collect(),
                    });
                }
            }
        }
        placed.extend(clouds);
    }

    // Asteroid field: a belt around the battle area, a third static.
    {
        let mut rng = Rng::stream(seed, 4);
        let mut rocks: Vec<Placed> = Vec::new();
        for i in 0..ASTEROID_COUNT {
            let a = rng.range(3.0, 14.0);
            let b = a * rng.range(0.6, 1.0);
            let n = rng.int(5, 8) as usize;
            let phi = rng.unit();
            let mut verts = Vec::with_capacity(n);
            for k in 0..n {
                let theta = (k as f64 + rng.range(-0.3, 0.3)) * core::f64::consts::TAU / n as f64;
                let (s, c) = sin_cos(theta);
                let (x, y) = (a * c, b * s);
                verts.push([(x * phi[0] - y * phi[1]) as f32, (x * phi[1] + y * phi[0]) as f32]);
            }
            let pos = loop {
                let u = rng.unit();
                let rad = rng.range(420.0, 700.0);
                let p = [u[0] * rad, u[1] * rad];
                if !overlaps(&placed, p, a, 5.0) && !overlaps(&rocks, p, a, 1.0) {
                    break p;
                }
            };
            rocks.push(Placed { c: pos, r: a });
            let is_static = i % 3 == 0;
            let v = rng.unit();
            let vm = if is_static { 0.0 } else { rng.range(0.0, 2.0) };
            let w = if is_static { 0.0 } else { rng.range(-0.3, 0.3) };
            if spec.asteroids {
                let id = bodies.len() as u32;
                bodies.push(BodyDef {
                    id,
                    kind: if is_static { BodyKind::Static } else { BodyKind::Dynamic },
                    role: BodyRole::Asteroid,
                    pos: f2(pos),
                    rot: [1.0, 0.0],
                    linvel: [(v[0] * vm) as f32, (v[1] * vm) as f32],
                    angvel: w as f32,
                    shapes: vec![Shape::Polygon { verts }],
                });
            }
        }
    }

    // Breakage schedule: once per simulated second, cut a ship across a random row (the removed cells split it).
    let mut breaks = Vec::new();
    if spec.breakage && spec.ships {
        let mut rng = Rng::stream(seed, 5);
        let mut masks: Vec<Vec<bool>> = grids.iter().map(|g| g.full_mask()).collect();
        let mut next_id = bodies.len() as u32;
        let mut k = 0u32;
        let mut tick = BREAK_INTERVAL_TICKS;
        while tick < total_ticks {
            let s = (k as usize) % SHIP_COUNT;
            k += 1;
            let g = &grids[s];
            let mask = &mut masks[s];
            // Rows that still have cells.
            let rows: Vec<i32> = (0..g.h).filter(|&y| (0..g.w).any(|x| mask[(y * g.w + x) as usize])).collect();
            if rows.len() < 5 {
                tick += BREAK_INTERVAL_TICKS;
                continue;
            }
            let lo = rows[rows.len() / 5];
            let hi = rows[rows.len() * 4 / 5];
            let cut = rng.int(lo as i64, hi as i64) as i32;
            let mut removed = 0;
            for x in 0..g.w {
                let i = (cut * g.w + x) as usize;
                if mask[i] {
                    mask[i] = false;
                    removed += 1;
                }
            }
            let comps = components(g, mask);
            // Largest component stays with the parent (ties: first found, i.e. lowest cell index).
            let mut best = 0;
            for (ci, c) in comps.iter().enumerate() {
                if c.len() > comps[best].len() {
                    best = ci;
                }
            }
            let mut parent_mask = vec![false; mask.len()];
            for &i in &comps[best] {
                parent_mask[i] = true;
            }
            let mut pieces = Vec::new();
            for (ci, c) in comps.iter().enumerate() {
                if ci == best {
                    continue;
                }
                let mut pm = vec![false; mask.len()];
                for &i in c {
                    pm[i] = true;
                }
                pieces.push((next_id, g.shapes(&pm, spec.mode)));
                next_id += 1;
            }
            *mask = parent_mask.clone();
            breaks.push(BreakEvent {
                tick,
                parent: ship_ids[s],
                parent_shapes: g.shapes(&parent_mask, spec.mode),
                pieces,
                removed_cells: removed,
            });
            tick += BREAK_INTERVAL_TICKS;
        }
    }

    World { spec, seed, bodies, ship_ids, breaks }
}

/// 4-connected components of the masked cells (in cell index order).
fn components(g: &Grid, mask: &[bool]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; mask.len()];
    let mut out = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        let mut comp = Vec::new();
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            comp.push(i);
            let x = (i as i32) % g.w;
            let y = (i as i32) / g.w;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= g.w || ny >= g.h {
                    continue;
                }
                let j = (ny * g.w + nx) as usize;
                if mask[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        comp.sort_unstable();
        out.push(comp);
    }
    out
}

/// Rays for one tick, from their own RNG stream (independent of the physics state).
pub fn rays_for_tick(seed: u64, tick: u32, out: &mut Vec<Ray>) {
    out.clear();
    let mut rng = Rng::stream(seed ^ 0x5241_5953, 1_000_000 + tick as u64);
    for _ in 0..RAYS_PER_TICK {
        let o = [rng.range(-450.0, 450.0), rng.range(-450.0, 450.0)];
        let d = rng.unit();
        out.push(Ray { origin: f2(o), dir: f2(d), max_dist: RAY_LENGTH });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_is_reproducible_and_sane() {
        let spec = scenario("combined").unwrap();
        let a = generate(spec, 42, 930);
        let b = generate(spec, 42, 930);
        assert_eq!(a.bodies.len(), b.bodies.len());
        for (x, y) in a.bodies.iter().zip(&b.bodies) {
            assert_eq!(x.pos, y.pos);
            assert_eq!(x.shapes, y.shapes);
        }
        let ships = a.bodies.iter().filter(|b| b.role == BodyRole::Ship).count();
        assert_eq!(ships, SHIP_COUNT);
        assert_eq!(a.bodies.iter().filter(|b| b.role == BodyRole::Debris).count(), DEBRIS_CLOUDS * DEBRIS_PER_CLOUD);
        assert_eq!(a.bodies.iter().filter(|b| b.role == BodyRole::Asteroid).count(), ASTEROID_COUNT);
        assert!(a.breaks.len() >= 29, "{}", a.breaks.len());
        for e in &a.breaks {
            assert!(!e.pieces.is_empty());
        }
    }

    #[test]
    fn merged_covers_same_area() {
        let mut rng = Rng::new(7);
        let g = gen_ship(&mut rng, 1500);
        let area = |s: &[Shape]| -> f64 {
            s.iter()
                .map(|s| match s {
                    Shape::Box { hx, hy, .. } => (4.0 * hx * hy) as f64,
                    Shape::Polygon { .. } => 0.5,
                })
                .sum()
        };
        let m = g.full_mask();
        let a = area(&g.shapes(&m, ColliderMode::Cells));
        let b = area(&g.shapes(&m, ColliderMode::Merged));
        assert_eq!(a, b);
        assert!(g.shapes(&m, ColliderMode::Merged).len() * 4 < g.shapes(&m, ColliderMode::Cells).len());
    }
}

/// Area-weighted centroid of shapes (uniform density), in the body frame. Used by adapters whose engine only
/// recomputes mass properties lazily.
pub fn shapes_centroid(shapes: &[Shape]) -> [f32; 2] {
    let (mut a, mut x, mut y) = (0.0f64, 0.0f64, 0.0f64);
    for s in shapes {
        match s {
            Shape::Box { cx, cy, hx, hy } => {
                let ar = 4.0 * *hx as f64 * *hy as f64;
                a += ar;
                x += ar * *cx as f64;
                y += ar * *cy as f64;
            }
            Shape::Polygon { verts } => {
                let n = verts.len();
                for i in 0..n {
                    let (p, q) = (verts[i], verts[(i + 1) % n]);
                    let cr = p[0] as f64 * q[1] as f64 - q[0] as f64 * p[1] as f64;
                    a += cr * 0.5;
                    x += (p[0] as f64 + q[0] as f64) * cr / 6.0;
                    y += (p[1] as f64 + q[1] as f64) * cr / 6.0;
                }
            }
        }
    }
    [(x / a) as f32, (y / a) as f32]
}
