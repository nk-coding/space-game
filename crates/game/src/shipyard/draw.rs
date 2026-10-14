//! The station as meshes: flat vector shapes from the layout, coloured with the style tokens (`station.*` and
//! the UI accents), with a few soft glows from the atlas's glow regions (nav lights, lit cores, the inner edge's
//! light strip, the gate), as the ships draw theirs. Built once per frame size and yard state (`super::rebuild`).
//!
//! Static layers, bottom to top: the yard (struts, buildings, the gate and its approach path, the truss bridges and
//! the moorings), the building area's floor (fill, cell dots, work lights, corner brackets) and the U's walls (plate
//! runs, trusses, conduits, window rows, radiator fins, the name plate, nav lights, antennas, the lit inner edge).
//! The rigging (cranes, carriages, hoist lines, clamps, drones) are small meshes of their own, moved by transforms.
//! Sizes follow the sketch at 20 px per cell (so 1 px = 0.05 m).

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use super::layout::{BASE_STRIP, BRIDGE, CLAMP_REACH, GATE_RADIUS, Layout, MOORING_DEPTH, MOORING_HALF, MOORING_WALL, TIP, WALL};
use crate::atlas::{Atlas, Region};
use crate::rng::Rng;
use crate::meshbuf::MeshBuf;
use crate::style::{Style, vcol};

/// The station's colours: `station.*` in `art/tokens.json`, the accents from `ui.*`.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub hull: Color,
    pub hull_hi: Color,
    pub hull_lo: Color,
    pub panel: Color,
    pub dark: Color,
    pub structure: Color,
    pub structure_hi: Color,
    pub floor: Color,
    pub port: Color,
    pub starboard: Color,
    pub lamp: Color,
    pub dots: Color,
    pub teal: Color,
    pub gold: Color,
    pub coral: Color,
    pub violet: Color,
    pub dim: Color,
}

impl Palette {
    pub fn new(atlas: &Atlas, style: &Style) -> Self {
        let c = |k: &str| atlas.token_color(&format!("station.{k}"));
        Self {
            hull: c("hull"),
            hull_hi: c("hull_hi"),
            hull_lo: c("hull_lo"),
            panel: c("panel"),
            dark: c("dark"),
            structure: c("structure"),
            structure_hi: c("structure_hi"),
            floor: c("floor"),
            port: c("nav_port"),
            starboard: c("nav_starboard"),
            lamp: c("lamp"),
            dots: style.dotted_ring,
            teal: style.ui.teal,
            gold: style.ui.gold,
            coral: style.ui.coral,
            violet: style.ui.violet,
            dim: style.ui.dim,
        }
    }
}

/// What lights the yard (from the career, `super::YardState`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Lights {
    pub research: bool,
    pub gate: bool,
    /// Moorings with an escort in them.
    pub moored: usize,
}

/// A mesh being drawn: flat shapes sample the atlas's white texel, glows its soft dot and halo strip.
pub struct Pen {
    pub m: MeshBuf,
    white: Region,
    soft: Region,
    halo: Region,
    glow: bool,
}

/// Points around a circle (closed: the first point is not repeated).
fn ring(c: Vec2, r: f32, n: usize) -> Vec<Vec2> {
    (0..n).map(|k| c + Vec2::from_angle(k as f32 / n as f32 * TAU) * r).collect()
}

fn ring_n(r: f32) -> usize {
    ((r * 10.0) as usize).clamp(16, 96)
}

impl Pen {
    pub fn new(atlas: &Atlas, glow: bool) -> Self {
        Self { m: MeshBuf::default(), white: atlas.region("fx/white"), soft: atlas.region("fx/dot_soft"), halo: atlas.region("fx/halo"), glow }
    }

    pub fn into_mesh(self) -> Mesh {
        if self.m.is_empty() { MeshBuf::placeholder() } else { self.m.into_mesh() }
    }

    fn poly(&mut self, pts: &[Vec2], c: [f32; 4]) {
        self.m.polygon(pts, self.white.uv_center(), c);
    }

    fn rect(&mut self, r: Rect, c: [f32; 4]) {
        self.poly(&[r.min, Vec2::new(r.max.x, r.min.y), r.max, Vec2::new(r.min.x, r.max.y)], c);
    }

    fn path(&mut self, pts: &[Vec2], closed: bool, width: f32, c: [f32; 4]) {
        let white = self.white;
        self.m.strip(pts, closed, width / 2.0, &white, c);
    }

    fn line(&mut self, a: Vec2, b: Vec2, width: f32, c: [f32; 4]) {
        self.path(&[a, b], false, width, c);
    }

    fn outline(&mut self, r: Rect, width: f32, c: [f32; 4]) {
        self.path(&[r.min, Vec2::new(r.max.x, r.min.y), r.max, Vec2::new(r.min.x, r.max.y)], true, width, c);
    }

    /// A filled rectangle with an outline (centred on its edge, like an SVG stroke).
    fn boxed(&mut self, r: Rect, fill: [f32; 4], stroke: [f32; 4], width: f32) {
        self.rect(r, fill);
        self.outline(r, width, stroke);
    }

    /// Dashes along `a → b`.
    fn dashes(&mut self, a: Vec2, b: Vec2, width: f32, dash: f32, gap: f32, c: [f32; 4]) {
        let len = a.distance(b);
        let d = (b - a) / len.max(1e-6);
        let mut s = 0.0;
        while s < len {
            let e = (s + dash).min(len);
            self.line(a + d * s, a + d * e, width, c);
            s = e + gap;
        }
    }

    fn circle(&mut self, c: Vec2, r: f32, width: f32, col: [f32; 4]) {
        self.path(&ring(c, r, ring_n(r)), true, width, col);
    }

    fn disc(&mut self, c: Vec2, r: f32, col: [f32; 4]) {
        self.poly(&ring(c, r, ring_n(r).min(24)), col);
    }

    /// A soft glow `d` m across (the atlas's blurred dot).
    fn glow(&mut self, at: Vec2, d: f32, c: [f32; 4]) {
        if self.glow {
            let soft = self.soft;
            self.m.region_at(&soft, at, Vec2::splat(d), 0.0, c);
        }
    }

    /// An elliptical pool of light.
    fn pool(&mut self, at: Vec2, size: Vec2, c: [f32; 4]) {
        if self.glow {
            let soft = self.soft;
            self.m.region_at(&soft, at, size, 0.0, c);
        }
    }

    /// A blurred band along a path (the atlas's halo strip, stretched to `half` m on each side).
    fn halo(&mut self, pts: &[Vec2], closed: bool, half: f32, c: [f32; 4]) {
        if self.glow {
            let halo = self.halo;
            self.m.strip(pts, closed, half, &halo, c);
        }
    }

    /// A red or green navigation light: a glow and a white core.
    fn nav(&mut self, at: Vec2, col: Color, lamp: Color) {
        self.glow(at, 1.9, vcol(col, 0.6));
        self.disc(at, 0.12, vcol(lamp, 1.0));
    }
}

/// Runs of plates in slightly different tones along `a → b` (a wall `across` thick, on the left of the run), with
/// dark seams between them.
fn plates(pen: &mut Pen, pal: &Palette, rng: &mut Rng, from: Vec2, to: Vec2, across: Vec2) {
    let len = from.distance(to);
    let d = (to - from) / len;
    let tones = [pal.hull, pal.hull, pal.panel, pal.hull_lo];
    let mut s = 0.0;
    while s < len - 0.05 {
        let k = [1.0, 1.0, 1.5, 2.0][(rng.next_f32() * 4.0) as usize % 4];
        let e = (s + 2.3 * k).min(len);
        let a = from + d * s;
        let b = from + d * e;
        let tone = tones[(rng.next_f32() * 4.0) as usize % 4];
        pen.poly(&[a, b, b + across, a + across], vcol(tone, 1.0));
        pen.line(a, a + across, 0.075, vcol(pal.dark, 1.0));
        s = e;
    }
}

/// A lattice band `width` wide along `a → b` with `n` zigzag bays.
fn truss(pen: &mut Pen, pal: &Palette, a: Vec2, b: Vec2, width: f32, n: usize) {
    let d = (b - a).normalize_or_zero();
    let side = d.perp() * (width / 2.0);
    let band = [a - side, b - side, b + side, a + side];
    pen.poly(&band, vcol(pal.dark, 1.0));
    pen.path(&band, true, 0.075, vcol(pal.hull_hi, 1.0));
    let zig: Vec<Vec2> = (0..=n.max(1))
        .map(|k| a.lerp(b, k as f32 / n.max(1) as f32) + d.perp() * (width / 2.0 - 0.1) * if k % 2 == 1 { 1.0 } else { -1.0 })
        .collect();
    pen.path(&zig, false, 0.1, vcol(pal.hull_hi, 1.0));
}

/// The three conduits (power, data, the mind's violet) along `a → b`.
fn conduits(pen: &mut Pen, pal: &Palette, a: Vec2, b: Vec2) {
    let n = (b - a).normalize_or_zero().perp();
    for (k, c) in [pal.teal, pal.gold, pal.violet].into_iter().enumerate() {
        let o = n * (k as f32 - 1.0) * 0.2;
        pen.line(a + o, b + o, 0.06, vcol(c, 0.45));
    }
}

/// A row of `n` windows from `a` to `b`, a share `lit` of them glowing in `col` (the others dark).
#[allow(clippy::too_many_arguments)]
fn windows(pen: &mut Pen, pal: &Palette, rng: &mut Rng, a: Vec2, b: Vec2, n: usize, lit: f32, col: Color, upright: bool) {
    let size = if upright { Vec2::new(0.15, 0.25) } else { Vec2::new(0.25, 0.15) };
    for k in 0..n {
        let at = a.lerp(b, (k as f32 + 0.5) / n as f32);
        let on = rng.next_f32() < lit;
        let c = if on { vcol(col, 0.9) } else { vcol(pal.hull_lo, 1.0) };
        pen.rect(Rect::from_center_size(at, size), c);
    }
}

/// Radiator fins standing out from a wall at `y` (outwards: `dir` = +1 up, −1 down).
fn fins(pen: &mut Pen, pal: &Palette, x0: f32, x1: f32, y: f32, dir: f32, n: usize) {
    for k in 0..n {
        let x = x0 + (x1 - x0) * (k as f32 + 0.5) / n as f32;
        let r = Rect::from_corners(Vec2::new(x - 0.15, y), Vec2::new(x + 0.15, y + dir * 1.1));
        pen.boxed(r, vcol(pal.hull_hi, 1.0), vcol(pal.coral, 0.55), 0.05);
    }
}

/// An antenna from `at` by `d`, a glowing tip.
fn antenna(pen: &mut Pen, pal: &Palette, at: Vec2, d: Vec2, col: Color) {
    pen.line(at, at + d, 0.1, vcol(pal.hull_hi, 1.0));
    pen.glow(at + d, 1.4, vcol(col, 0.6));
    pen.disc(at + d, 0.08, vcol(pal.lamp, 1.0));
}

/// The U: plate runs, its outline, trusses outside, conduits inside, window rows, fins, the name plate, nav
/// lights and antennas, and the lit inner edge around the building area.
pub fn walls(pen: &mut Pen, l: &Layout, pal: &Palette) {
    let mut rng = Rng::new(42);
    let a = l.area;
    let (w, h) = (a.width(), a.height());
    let (base, tip) = (l.base_x, l.tip_x);
    // The arms and the base, as plate runs.
    plates(pen, pal, &mut rng, Vec2::new(base, a.max.y + WALL), Vec2::new(tip, a.max.y + WALL), Vec2::new(0.0, -WALL));
    plates(pen, pal, &mut rng, Vec2::new(base, a.min.y), Vec2::new(tip, a.min.y), Vec2::new(0.0, -WALL));
    plates(pen, pal, &mut rng, Vec2::new(base, a.max.y), Vec2::new(base, a.min.y), Vec2::new(WALL, 0.0));
    // Chamfered tips and the base's outer strip.
    let hi = vcol(pal.hull_hi, 1.0);
    let (t, b) = (a.max.y, a.min.y);
    pen.poly(&[Vec2::new(tip, t + WALL), Vec2::new(tip + TIP, t + 0.65 * WALL), Vec2::new(tip + TIP, t + 0.35 * WALL), Vec2::new(tip, t)], hi);
    pen.poly(&[Vec2::new(tip, b), Vec2::new(tip + TIP, b - 0.35 * WALL), Vec2::new(tip + TIP, b - 0.65 * WALL), Vec2::new(tip, b - WALL)], hi);
    let back = base - BASE_STRIP;
    pen.poly(&[Vec2::new(base, t + WALL), Vec2::new(back, t + WALL - BASE_STRIP), Vec2::new(back, b - WALL + BASE_STRIP), Vec2::new(base, b - WALL)], hi);
    let outline = [
        Vec2::new(tip + TIP, t + 0.65 * WALL),
        Vec2::new(tip, t + WALL),
        Vec2::new(base, t + WALL),
        Vec2::new(back, t + WALL - BASE_STRIP),
        Vec2::new(back, b - WALL + BASE_STRIP),
        Vec2::new(base, b - WALL),
        Vec2::new(tip, b - WALL),
        Vec2::new(tip + TIP, b - 0.65 * WALL),
    ];
    pen.path(&outline, false, 0.15, hi);
    // Trusses on the outer faces, conduits along the inner ones.
    let bays = |len: f32| (len / 1.1) as usize;
    truss(pen, pal, Vec2::new(base + 1.0, t + WALL - 0.45), Vec2::new(tip - 0.5, t + WALL - 0.45), 0.7, bays(tip - base));
    truss(pen, pal, Vec2::new(base + 1.0, b - WALL + 0.45), Vec2::new(tip - 0.5, b - WALL + 0.45), 0.7, bays(tip - base));
    truss(pen, pal, Vec2::new(base + 0.45, t - 0.5), Vec2::new(base + 0.45, b + 0.5), 0.7, bays(h));
    conduits(pen, pal, Vec2::new(0.3, t + 0.5), Vec2::new(tip, t + 0.5));
    conduits(pen, pal, Vec2::new(0.3, b - 0.5), Vec2::new(tip, b - 0.5));
    conduits(pen, pal, Vec2::new(-0.5, t - 0.3), Vec2::new(-0.5, b + 0.3));
    // Window rows: the yard runs, so about half of them are lit.
    let n = ((tip - base) / 0.9) as usize;
    windows(pen, pal, &mut rng, Vec2::new(base + 1.5, t + WALL - 1.3), Vec2::new(tip - 1.0, t + WALL - 1.3), n, 0.55, pal.gold, false);
    windows(pen, pal, &mut rng, Vec2::new(base + 1.5, b - WALL + 1.3), Vec2::new(tip - 1.0, b - WALL + 1.3), n, 0.55, pal.gold, false);
    windows(pen, pal, &mut rng, Vec2::new(base + 1.3, t - 1.0), Vec2::new(base + 1.3, b + 1.0), (h / 0.9) as usize, 0.5, pal.gold, true);
    // Radiator fins on the outer faces between the buildings.
    fins(pen, pal, 0.62 * w, 0.92 * w, t + WALL, 1.0, 7);
    fins(pen, pal, 0.72 * w, 0.95 * w, b - WALL, -1.0, 6);
    // The name plate (its name is a label, `home`).
    pen.boxed(l.name_plate(), vcol(pal.dark, 1.0), vcol(pal.gold, 0.6), 0.05);
    // Nav lights (port red on the top arm, starboard green on the bottom, facing out of the mouth) and antennas.
    pen.nav(Vec2::new(tip + 0.9, t + WALL - 0.4), pal.port, pal.lamp);
    pen.nav(Vec2::new(tip + 0.9, b - WALL + 0.4), pal.starboard, pal.lamp);
    pen.nav(Vec2::new(back + 0.4, t + WALL - 1.5), pal.port, pal.lamp);
    pen.nav(Vec2::new(back + 0.4, b - WALL + 1.5), pal.starboard, pal.lamp);
    antenna(pen, pal, Vec2::new(tip + 0.2, t + WALL), Vec2::new(1.2, 1.5), pal.teal);
    antenna(pen, pal, Vec2::new(tip + 0.2, b - WALL), Vec2::new(1.2, -1.5), pal.teal);
    // The inner edge: a light strip around the building area with lamps along the walls.
    let edge = [a.min, Vec2::new(a.max.x, a.min.y), a.max, Vec2::new(a.min.x, a.max.y)];
    pen.halo(&edge, true, 0.45, vcol(pal.teal, 0.28));
    pen.path(&edge, true, 0.065, vcol(pal.teal, 0.6));
    for (from, to) in [(Vec2::new(0.0, t - 0.15), Vec2::new(w, t - 0.15)), (Vec2::new(0.0, b + 0.15), Vec2::new(w, b + 0.15)), (Vec2::new(0.15, t), Vec2::new(0.15, b))] {
        pen.dashes(from, to, 0.1, 0.1, 0.7, vcol(pal.teal, 0.9));
    }
}

/// The building area's floor: the fill, a dot at every cell corner (where the drydock puts its own), work lights
/// pooling from the walls, gold corner brackets.
pub fn floor(pen: &mut Pen, l: &Layout, pal: &Palette) {
    let a = l.area;
    pen.rect(a, vcol(pal.floor, 1.0));
    for f in [0.25, 0.75] {
        for y in [a.max.y - 1.5, a.min.y + 1.5] {
            pen.pool(Vec2::new(a.min.x + a.width() * f, y), Vec2::new(0.32 * a.width(), 7.0), vcol(pal.gold, 0.1));
        }
    }
    let (w, h) = l.frame;
    for y in 0..=h {
        for x in 0..=w {
            let at = Vec2::new(x as f32, -(y as f32));
            pen.rect(Rect::from_center_size(at, Vec2::splat(0.1)), vcol(pal.dots, 0.6));
        }
    }
    let (k, o) = (1.1, 0.3);
    for (corner, s) in [(a.min, Vec2::new(1.0, 1.0)), (Vec2::new(a.max.x, a.min.y), Vec2::new(-1.0, 1.0)), (a.max, Vec2::new(-1.0, -1.0)), (Vec2::new(a.min.x, a.max.y), Vec2::new(1.0, -1.0))] {
        let p = corner + s * o;
        pen.path(&[p + Vec2::new(s.x * (k - o), 0.0), p, p + Vec2::new(0.0, s.y * (k - o))], false, 0.11, vcol(pal.gold, 1.0));
    }
}

/// A strut joining a building to the station.
fn strut(pen: &mut Pen, pal: &Palette, from: Vec2, to: Vec2) {
    pen.line(from, to, 0.7, vcol(pal.hull_hi, 1.0));
    pen.dashes(from, to, 0.1, 0.2, 0.3, vcol(pal.dark, 1.0));
}

/// A building's body: a chamfered block with window rows top and bottom (`lit` lights most of them in `col`).
fn block(pen: &mut Pen, pal: &Palette, rng: &mut Rng, r: Rect, lit: bool, col: Color) {
    let c = r.width().min(r.height()) * 0.18;
    let (lo, hi) = (r.min, r.max);
    let pts = [
        Vec2::new(lo.x + c, lo.y),
        Vec2::new(hi.x - c, lo.y),
        Vec2::new(hi.x, lo.y + c),
        Vec2::new(hi.x, hi.y - c),
        Vec2::new(hi.x - c, hi.y),
        Vec2::new(lo.x + c, hi.y),
        Vec2::new(lo.x, hi.y - c),
        Vec2::new(lo.x, lo.y + c),
    ];
    pen.poly(&pts, vcol(pal.hull, 1.0));
    pen.path(&pts, true, 0.15, vcol(pal.hull_hi, 1.0));
    let n = ((r.width() / 1.1) as usize).max(3);
    let share = if lit { 0.7 } else { 0.05 };
    windows(pen, pal, rng, Vec2::new(lo.x + 0.6, hi.y - 0.4), Vec2::new(hi.x - 0.6, hi.y - 0.4), n, share, col, false);
    windows(pen, pal, rng, Vec2::new(lo.x + 0.6, lo.y + 0.4), Vec2::new(hi.x - 0.6, lo.y + 0.4), n, share, col, false);
}

/// An ellipse's outline (`rx`, `ry` turned by `angle`).
fn ellipse(pen: &mut Pen, c: Vec2, rx: f32, ry: f32, angle: f32, width: f32, col: [f32; 4]) {
    let rot = Vec2::from_angle(angle);
    let pts: Vec<Vec2> = (0..48).map(|k| c + rot.rotate(Vec2::from_angle(k as f32 / 48.0 * TAU) * Vec2::new(rx, ry))).collect();
    pen.path(&pts, true, width, col);
}

/// A dish facing `dir` (radians): its bowl and the feed.
fn dish(pen: &mut Pen, pal: &Palette, c: Vec2, r: f32, dir: f32, col: Color) {
    let arc: Vec<Vec2> = (0..=16).map(|k| c + Vec2::from_angle(dir - 1.2 + 2.4 * k as f32 / 16.0) * r).collect();
    pen.poly(&arc, vcol(pal.hull_lo, 1.0));
    pen.path(&arc, false, 0.08, vcol(col, 1.0));
    pen.line(c, c + Vec2::from_angle(dir) * r * 0.8, 0.06, vcol(col, 1.0));
}

/// Three dark bays: a building waiting for its screen (the fabricator, the salvage bay).
fn bays(pen: &mut Pen, pal: &Palette, c: Vec2, s: f32) {
    for k in 0..3 {
        let x = c.x + (-45.0 + 32.0 * k as f32) * s;
        let r = Rect::new(x, c.y - 22.0 * s, x + 24.0 * s, c.y + 22.0 * s);
        pen.boxed(r, vcol(pal.dark, 1.0), vcol(pal.dim, 1.0), 0.06);
    }
}

/// The mind's emblem: a hexagon with a glowing core, six spokes and a diamond.
fn mind(pen: &mut Pen, pal: &Palette, c: Vec2) {
    let hex: Vec<Vec2> = (0..6).map(|k| c + Vec2::from_angle((60.0 * k as f32 + 30.0).to_radians()) * 2.64).collect();
    pen.poly(&hex, vcol(pal.structure, 1.0));
    pen.path(&hex, true, 0.15, vcol(pal.structure_hi, 1.0));
    pen.glow(c, 5.2, vcol(pal.violet, 0.45));
    for k in 0..6 {
        let end = c + Vec2::from_angle((60.0 * k as f32).to_radians()) * 1.87;
        pen.line(c, end, 0.07, vcol(pal.violet, 0.5));
        pen.disc(end, 0.17, vcol(pal.violet, 0.8));
    }
    let d = 0.96;
    let diamond = [c + Vec2::new(0.0, d), c + Vec2::new(-d, 0.0), c + Vec2::new(0.0, -d), c + Vec2::new(d, 0.0)];
    pen.poly(&diamond, vcol(pal.dark, 1.0));
    pen.path(&diamond, true, 0.1, vcol(pal.violet, 1.0));
}

/// A square turned by `angle` at `c`.
fn turned_square(pen: &mut Pen, c: Vec2, side: f32, angle: f32, fill: [f32; 4], stroke: [f32; 4], width: f32) {
    let r = Vec2::from_angle(angle);
    let h = side / 2.0;
    let pts = [Vec2::new(-h, -h), Vec2::new(h, -h), Vec2::new(h, h), Vec2::new(-h, h)].map(|p| c + r.rotate(p));
    pen.poly(&pts, fill);
    pen.path(&pts, true, width, stroke);
}

/// The gate: a segmented ring with emitters, glowing and filled while awake.
fn gate(pen: &mut Pen, pal: &Palette, c: Vec2, awake: bool) {
    let r = GATE_RADIUS;
    let col = if awake { pal.teal } else { pal.dim };
    if awake {
        pen.halo(&ring(c, r, 96), true, 1.2, vcol(pal.teal, 0.45));
        pen.pool(c, Vec2::splat(2.0 * r), vcol(pal.teal, 0.2));
    }
    pen.circle(c, r, 0.16 * r, vcol(pal.structure_hi, 1.0));
    // The sketch's dashes: 3 px on, 9 off around a 150 px ring.
    let n = 78;
    for k in 0..n {
        let a0 = k as f32 / n as f32 * TAU;
        let arc: Vec<Vec2> = (0..=2).map(|j| c + Vec2::from_angle(a0 + j as f32 * 0.25 * TAU / n as f32) * r).collect();
        pen.path(&arc, false, 0.1, vcol(col, 0.9));
    }
    for k in 0..8 {
        let a = k as f32 * PI / 4.0;
        turned_square(pen, c + Vec2::from_angle(a) * r, 0.7, a, vcol(pal.structure, 1.0), vcol(col, 0.6), 0.05);
    }
    for k in 0..4 {
        let a = PI / 4.0 + k as f32 * PI / 2.0;
        turned_square(pen, c + Vec2::from_angle(a) * (r + 0.9), 0.9, a, vcol(pal.hull, 1.0), vcol(col, 0.7), 0.075);
    }
}

/// The test range's target lane: a rounded band with a dashed centre line and three targets.
fn lane(pen: &mut Pen, pal: &Palette, a: Vec2, b: Vec2) {
    let band = vcol(pal.structure_hi, 1.0);
    pen.line(a, b, 2.2, band);
    pen.disc(a, 1.1, band);
    pen.disc(b, 1.1, band);
    pen.dashes(a, b, 0.075, 0.1, 0.5, vcol(pal.coral, 0.6));
    for k in 1..4 {
        let at = a.lerp(b, k as f32 / 4.0);
        pen.circle(at, 0.45, 0.08, vcol(pal.coral, 1.0));
        pen.disc(at, 0.15, vcol(pal.coral, 1.0));
    }
}

/// A mooring: a small U hanging off a bridge, its mouth away from it (`open` +1 up, −1 down), lit while an escort
/// lies in it. `c` is its interior's centre.
fn mooring(pen: &mut Pen, pal: &Palette, rng: &mut Rng, c: Vec2, open: f32, lit: bool) {
    let (hw, d, t) = (MOORING_HALF, MOORING_DEPTH / 2.0, MOORING_WALL);
    let ch = t * 0.45;
    // The berth's own frame: `u` from the base towards the mouth, `v` across it.
    let p = |u: f32, v: f32| c + Vec2::new(v, open * u);
    let (x0, x1) = (-d, d); // base inner face, tips
    let (back, y0, y1) = (x0 - t, -hw - t, hw + t);
    let quad = |a: (f32, f32), b: (f32, f32)| Rect::from_corners(p(a.0, a.1), p(b.0, b.1));
    pen.rect(quad((x0, -hw), (x1, hw)), vcol(pal.floor, 1.0));
    let fill = vcol(pal.hull, 1.0);
    let pts = |q: &[(f32, f32)]| q.iter().map(|&(u, v)| p(u, v)).collect::<Vec<_>>();
    pen.poly(&pts(&[(back, y0 + ch), (back + ch, y0), (x0, y0), (x0, y1), (back + ch, y1), (back, y1 - ch)]), fill);
    pen.poly(&pts(&[(x0, hw), (x1, hw), (x1, y1 - ch), (x1 - ch, y1), (x0, y1)]), fill);
    pen.poly(&pts(&[(x0, -hw), (x0, y0), (x1 - ch, y0), (x1, y0 + ch), (x1, -hw)]), fill);
    let outline = pts(&[
        (x1, hw),
        (x0, hw),
        (x0, -hw),
        (x1, -hw),
        (x1, y0 + ch),
        (x1 - ch, y0),
        (back + ch, y0),
        (back, y0 + ch),
        (back, y1 - ch),
        (back + ch, y1),
        (x1 - ch, y1),
        (x1, y1 - ch),
    ]);
    pen.path(&outline, true, 0.15, vcol(pal.hull_hi, 1.0));
    // Spines along the walls.
    let hi = vcol(pal.hull_hi, 1.0);
    pen.rect(quad((x0 - 0.48, hw + 0.46), (x1 - 1.0, hw + 0.74)), hi);
    pen.rect(quad((x0 - 0.48, -hw - 0.74), (x1 - 1.0, -hw - 0.46)), hi);
    pen.rect(quad((x0 - 0.74, -hw - 0.46), (x0 - 0.46, hw + 0.46)), hi);
    // Windows on the outer faces.
    let share = if lit { 0.55 } else { 0.08 };
    windows(pen, pal, rng, p(x1 - 1.5, y1 - 0.3), p(back + 1.0, y1 - 0.3), 10, share, pal.gold, true);
    windows(pen, pal, rng, p(x1 - 1.5, y0 + 0.3), p(back + 1.0, y0 + 0.3), 10, share, pal.gold, true);
    windows(pen, pal, rng, p(back + 0.3, -hw), p(back + 0.3, hw), 9, share, pal.gold, false);
    // The lit inner edge and small clamps.
    let inner = pts(&[(x1, hw), (x0, hw), (x0, -hw), (x1, -hw)]);
    if lit {
        pen.halo(&inner, false, 0.3, vcol(pal.teal, 0.3));
    }
    pen.path(&inner, false, 0.07, vcol(pal.teal, if lit { 0.6 } else { 0.25 }));
    for f in [0.3, 0.7] {
        let x = x1 - f * 2.0 * d;
        for s in [-1.0, 1.0] {
            pen.rect(quad((x - 0.2, s * hw), (x + 0.2, s * (hw - 0.6))), hi);
        }
    }
    pen.nav(p(x1 - 0.4, hw + t / 2.0), pal.port, pal.lamp);
    pen.nav(p(x1 - 0.4, -hw - t / 2.0), pal.starboard, pal.lamp);
    pen.nav(p(back + 0.4, y1 - 0.4), pal.port, pal.lamp);
    pen.nav(p(back + 0.4, y0 + 0.4), pal.starboard, pal.lamp);
}

/// Everything around the U: struts, buildings, the gate with its pylons and approach path, the bridges and the
/// moorings.
pub fn yard(pen: &mut Pen, l: &Layout, pal: &Palette, lights: Lights) {
    let mut rng = Rng::new(7);
    // The truss bridges to the moorings, and the cross bars at their far end and behind the mind.
    let (bx0, bx1) = l.bridge_x;
    for y in l.bridges {
        truss(pen, pal, Vec2::new(bx0, y), Vec2::new(bx1, y), BRIDGE, ((bx1 - bx0) / 1.1) as usize);
    }
    for x in [bx0, bx1] {
        pen.line(Vec2::new(x, l.bridges[0]), Vec2::new(x, l.bridges[1]), 0.5, vcol(pal.hull_hi, 1.0));
    }
    for (k, c) in l.moorings.iter().enumerate() {
        mooring(pen, pal, &mut rng, *c, if c.y > l.area.center().y { 1.0 } else { -1.0 }, k < lights.moored);
    }
    // Struts under the buildings; violet conduits from the mind into the base.
    for b in [&l.research, &l.fabricator, &l.mind, &l.range, &l.salvage] {
        strut(pen, pal, b.strut.0, b.strut.1);
    }
    for dy in [-2.0, 2.0] {
        let y = l.mind.centre.y + dy;
        pen.line(Vec2::new(l.mind.rect().max.x, y), Vec2::new(l.base_x - BASE_STRIP, y), 0.06, vcol(pal.violet, 0.45));
    }
    // Research: a lab with an orbit emblem and dishes on its roof, lit when research runs.
    let r = &l.research;
    let lab_col = if lights.research { pal.gold } else { pal.dim };
    block(pen, pal, &mut rng, r.rect(), lights.research, pal.gold);
    let (rx, ry) = (1.69, 0.62);
    ellipse(pen, r.centre, rx, ry, 20f32.to_radians(), 0.1, vcol(lab_col, 1.0));
    ellipse(pen, r.centre, rx, ry, -40f32.to_radians(), 0.06, vcol(lab_col, 0.5));
    if lights.research {
        pen.glow(r.centre, 3.0, vcol(pal.gold, 0.6));
    }
    pen.disc(r.centre, 0.29, vcol(lab_col, 1.0));
    dish(pen, pal, r.centre + Vec2::new(-3.5, 3.1), 0.9, PI / 2.0, lab_col);
    dish(pen, pal, r.centre + Vec2::new(3.3, 3.1), 0.7, PI / 3.0, lab_col);
    antenna(pen, pal, r.centre + Vec2::new(1.0, 2.75), Vec2::new(0.0, 1.3), pal.gold);
    // The fabricator and the salvage bay: dark until they have screens.
    block(pen, pal, &mut rng, l.fabricator.rect(), false, pal.dim);
    bays(pen, pal, l.fabricator.centre, 0.0287);
    block(pen, pal, &mut rng, l.salvage.rect(), false, pal.dim);
    bays(pen, pal, l.salvage.centre, 0.0233);
    // The mind, behind the base.
    block(pen, pal, &mut rng, l.mind.rect(), true, pal.violet);
    mind(pen, pal, l.mind.centre);
    // The test range: its control tower and the target lane.
    let t = l.range.rect();
    block(pen, pal, &mut rng, t, true, pal.teal);
    pen.boxed(Rect::from_center_size(t.center(), t.size() * Vec2::new(0.64, 0.44)), vcol(pal.dark, 1.0), vcol(pal.teal, 0.5), 0.06);
    lane(pen, pal, l.lane.0, l.lane.1);
    // The approach path, the pylons and the gate.
    let path_col = if lights.gate { pal.teal } else { pal.dim };
    for (k, c) in l.chevrons.iter().enumerate() {
        let s = 0.4;
        pen.path(&[*c + Vec2::new(0.0, s), *c + Vec2::new(s, 0.0), *c - Vec2::new(0.0, s)], false, 0.08, vcol(path_col, 0.25 + 0.1 * k as f32));
    }
    for (k, (a, b)) in l.pylons.iter().enumerate() {
        pen.line(*a, *b, 0.45, vcol(pal.hull_hi, 1.0));
        pen.nav(*b, if k == 1 { pal.port } else { pal.starboard }, pal.lamp);
    }
    gate(pen, pal, l.gate, lights.gate);
}

/// A gantry crane spanning the arms (local: its rail centred on x = 0, from the top arm to the bottom one).
pub fn crane(pen: &mut Pen, l: &Layout, pal: &Palette) {
    let (t, b) = (l.area.max.y + 0.2, l.area.min.y - 0.2);
    pen.rect(Rect::new(-0.35, b, 0.35, t), vcol(pal.hull_hi, 0.93));
    pen.dashes(Vec2::new(0.0, t - 0.2), Vec2::new(0.0, b + 0.2), 0.075, 0.25, 0.25, vcol(pal.dark, 1.0));
}

/// A crane's carriage with its work lamp (local: centred).
pub fn carriage(pen: &mut Pen, pal: &Palette) {
    pen.boxed(Rect::from_center_size(Vec2::ZERO, Vec2::new(1.6, 1.3)), vcol(pal.hull, 1.0), vcol(pal.gold, 0.6), 0.06);
    pen.glow(Vec2::ZERO, 1.4, vcol(pal.gold, 0.75));
    pen.disc(Vec2::ZERO, 0.1, vcol(pal.gold, 1.0));
}

/// A hoist line, one metre along +x (scaled to its length).
pub fn hoist(pen: &mut Pen, pal: &Palette) {
    for k in 0..8 {
        let x = k as f32 / 8.0;
        pen.line(Vec2::new(x, 0.0), Vec2::new(x + 0.07, 0.0), 0.05, vcol(pal.gold, 0.35));
    }
}

/// A docking clamp reaching down from y = 0 (the top arm's; the bottom ones are turned over).
pub fn clamp(pen: &mut Pen, pal: &Palette) {
    pen.rect(Rect::new(-0.25, -CLAMP_REACH, 0.25, 0.0), vcol(pal.hull_hi, 1.0));
    pen.boxed(Rect::new(-0.5, -CLAMP_REACH - 0.3, 0.5, -CLAMP_REACH + 0.0), vcol(pal.hull_hi, 1.0), vcol(pal.teal, 0.5), 0.05);
}

/// A drone heading +x with its short trail.
pub fn drone(pen: &mut Pen, pal: &Palette) {
    pen.line(Vec2::new(-0.7, 0.0), Vec2::ZERO, 0.06, vcol(pal.teal, 0.35));
    pen.glow(Vec2::ZERO, 0.6, vcol(pal.teal, 0.35));
    pen.disc(Vec2::ZERO, 0.11, vcol(pal.teal, 1.0));
}
