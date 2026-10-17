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
use kit::atlas::{Atlas, Region};
use kit::meshbuf::MeshBuf;
use kit::rng::Rng;
use kit::style::{Style, vcol};

use super::layout::{BASE_STRIP, BRIDGE, CLAMP_REACH, GATE_POD, GATE_RADIUS, Layout, MOORING_DEPTH, MOORING_HALF, MOORING_WALL, TIP, WALL, chamfer};

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

/// What lights the yard (from the career and the lab, `super::Yard`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Lights {
    pub research: bool,
    pub gate: bool,
    /// Moorings with an escort in them.
    pub moored: usize,
    /// A voyage's result waits in the LOG: its mast glows.
    pub log: bool,
    pub evolution: Evo,
    /// Hauled parts wait in the workshop to be sorted (GDR-0108): its bays glow gold, else teal.
    pub workshop: bool,
}

/// The EVOLUTION lab's state (GDR-0092 "Its states"): dark until its research node opens it, dim without a run, its
/// racks flickering while a run computes, gold once a run has finished until it is opened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Evo {
    #[default]
    Dark,
    Idle,
    Running,
    Paused,
    Done,
}

/// A light that blinks (`super::life`): where it is, its colour and how it blinks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beacon {
    pub at: Vec2,
    pub color: Color,
    /// The glow's size (m).
    pub size: f32,
    pub strobe: bool,
}

/// The windows of one building or wall: their panes, the colour they light in, and whether the building is lit (most
/// of them on) or dark (hardly any). Here they are drawn dark; `super::life` lights them, a few at a time, and fast
/// while the building is `busy` (the EVOLUTION lab's racks while its run computes).
#[derive(Clone, Debug, PartialEq)]
pub struct Windows {
    pub panes: Vec<Rect>,
    pub color: Color,
    pub lit: bool,
    pub busy: bool,
}

/// A mesh being drawn: flat shapes sample the atlas's white texel, glows its soft dot and halo strip. Blinking
/// lights and windows are collected, not drawn (their glows and lights are small meshes of their own, `super::life`).
pub struct Pen {
    pub m: MeshBuf,
    pub beacons: Vec<Beacon>,
    pub windows: Vec<Windows>,
    white: Region,
    soft: Region,
    halo: Region,
    glow: bool,
}

/// The approach path's chevrons' half height (m).
const CHEVRON: f32 = 0.4;

/// Points around a circle (closed: the first point is not repeated).
fn ring(c: Vec2, r: f32, n: usize) -> Vec<Vec2> {
    (0..n).map(|k| c + Vec2::from_angle(k as f32 / n as f32 * TAU) * r).collect()
}

fn ring_n(r: f32) -> usize {
    ((r * 10.0) as usize).clamp(16, 96)
}

impl Pen {
    pub fn new(atlas: &Atlas, glow: bool) -> Self {
        Self { m: MeshBuf::default(), beacons: Vec::new(), windows: Vec::new(), white: atlas.region("fx/white"), soft: atlas.region("fx/dot_soft"), halo: atlas.region("fx/halo"), glow }
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

    /// A soft glow `d` m across at the origin: a blinking light's own mesh.
    pub fn beacon_glow(&mut self, d: f32, c: Color) {
        self.glow(Vec2::ZERO, d, vcol(c, 0.6));
    }

    /// A light whose glow blinks: its white core here, the glow collected.
    fn beacon(&mut self, at: Vec2, col: Color, lamp: Color, size: f32, core: f32, strobe: bool) {
        if self.glow {
            self.beacons.push(Beacon { at, color: col, size, strobe });
        }
        self.disc(at, core, vcol(lamp, 1.0));
    }

    /// A red or green navigation light: a glow that breathes and a white core.
    fn nav(&mut self, at: Vec2, col: Color, lamp: Color) {
        self.beacon(at, col, lamp, 1.9, 0.12, false);
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

/// One building's or wall's windows, in rows of `n` from `a` to `b` (each pane long along its row), drawn dark and
/// collected to light in `col` (`lit`: most of them, else hardly any).
fn windows(pen: &mut Pen, pal: &Palette, rows: &[(Vec2, Vec2, usize)], col: Color, lit: bool) {
    let mut panes = Vec::new();
    for &(a, b, n) in rows {
        let d = b - a;
        let size = if d.y.abs() > d.x.abs() { Vec2::new(0.15, 0.25) } else { Vec2::new(0.25, 0.15) };
        for k in 0..n {
            let pane = Rect::from_center_size(a.lerp(b, (k as f32 + 0.5) / n as f32), size);
            pen.rect(pane, vcol(pal.hull_lo, 1.0));
            panes.push(pane);
        }
    }
    pen.windows.push(Windows { panes, color: col, lit, busy: false });
}

/// The windows drawn last flicker while `busy`.
fn busy(pen: &mut Pen, busy: bool) {
    if let Some(w) = pen.windows.last_mut() {
        w.busy = busy;
    }
}

/// Windows' lights over their dark panes, each as bright as its `glow` (0: off, 1: on).
pub fn window_lights(pen: &mut Pen, w: &Windows, glow: impl Iterator<Item = f32>) {
    for (pane, g) in w.panes.iter().zip(glow) {
        if g > 0.0 {
            pen.rect(*pane, vcol(w.color, 0.9 * g));
        }
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

/// An antenna from `at` by `d`, its tip a flashing beacon.
fn antenna(pen: &mut Pen, pal: &Palette, at: Vec2, d: Vec2, col: Color) {
    pen.line(at, at + d, 0.1, vcol(pal.hull_hi, 1.0));
    pen.beacon(at + d, col, pal.lamp, 1.4, 0.08, true);
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
    // Window rows on each arm and the base: the yard runs, so they are lit.
    let n = ((tip - base) / 0.9) as usize;
    windows(pen, pal, &[(Vec2::new(base + 1.5, t + WALL - 1.3), Vec2::new(tip - 1.0, t + WALL - 1.3), n)], pal.gold, true);
    windows(pen, pal, &[(Vec2::new(base + 1.5, b - WALL + 1.3), Vec2::new(tip - 1.0, b - WALL + 1.3), n)], pal.gold, true);
    windows(pen, pal, &[(Vec2::new(base + 1.3, t - 1.0), Vec2::new(base + 1.3, b + 1.0), (h / 0.9) as usize)], pal.gold, true);
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
fn block(pen: &mut Pen, pal: &Palette, r: Rect, lit: bool, col: Color) {
    let c = chamfer(r);
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
    let rows = [(Vec2::new(lo.x + 0.6, hi.y - 0.4), Vec2::new(hi.x - 0.6, hi.y - 0.4), n), (Vec2::new(lo.x + 0.6, lo.y + 0.4), Vec2::new(hi.x - 0.6, lo.y + 0.4), n)];
    windows(pen, pal, &rows, col, lit);
}

/// An ellipse's outline (`rx`, `ry` turned by `angle`).
fn ellipse(pen: &mut Pen, c: Vec2, rx: f32, ry: f32, angle: f32, width: f32, col: [f32; 4]) {
    let rot = Vec2::from_angle(angle);
    let pts: Vec<Vec2> = (0..48).map(|k| c + rot.rotate(Vec2::from_angle(k as f32 / 48.0 * TAU) * Vec2::new(rx, ry))).collect();
    pen.path(&pts, true, width, col);
}

/// A dish's bowl and feed, facing +x around the origin (`super::life` turns it).
pub fn dish(pen: &mut Pen, pal: &Palette, r: f32, col: Color) {
    let arc: Vec<Vec2> = (0..=16).map(|k| Vec2::from_angle(-1.2 + 2.4 * k as f32 / 16.0) * r).collect();
    pen.poly(&arc, vcol(pal.hull_lo, 1.0));
    pen.path(&arc, false, 0.08, vcol(col, 1.0));
    pen.line(Vec2::ZERO, Vec2::X * r * 0.8, 0.06, vcol(col, 1.0));
    pen.disc(Vec2::ZERO, 0.14, vcol(pal.hull_hi, 1.0));
}

/// Research's emblem around the origin: two orbits and a body riding the outer one (`super::life` turns it).
pub fn orbit(pen: &mut Pen, col: Color) {
    let (rx, ry) = (1.69, 0.62);
    ellipse(pen, Vec2::ZERO, rx, ry, 20f32.to_radians(), 0.1, vcol(col, 1.0));
    ellipse(pen, Vec2::ZERO, rx, ry, -40f32.to_radians(), 0.06, vcol(col, 0.5));
    let on = Vec2::from_angle(20f32.to_radians()).rotate(Vec2::new(rx, 0.0));
    pen.disc(on, 0.13, vcol(col, 1.0));
}

/// The LOG's records mast (GDR-0091): a small house on the top arm with its windows, a lattice mast with two
/// signal yards and a slow beacon at its top (the glow of a new result is `super::life`'s).
fn log_mast(pen: &mut Pen, l: &Layout, pal: &Palette) {
    let r = l.log.rect();
    block(pen, pal, r, true, pal.gold);
    let (foot, top) = (Vec2::new(r.center().x, r.max.y), l.log_beacon);
    truss(pen, pal, foot, top, 0.42, 5);
    for (f, half) in [(0.38, 0.85), (0.7, 0.55)] {
        let y = foot.lerp(top, f);
        pen.line(y - Vec2::X * half, y + Vec2::X * half, 0.1, vcol(pal.hull_hi, 1.0));
        for s in [-1.0, 1.0] {
            pen.disc(y + Vec2::X * s * half, 0.09, vcol(pal.teal, 0.85));
        }
    }
    pen.disc(top, 0.24, vcol(pal.hull_hi, 1.0));
    pen.beacon(top, pal.teal, pal.lamp, 1.6, 0.1, true);
}

/// The LOG's glow while a voyage's result waits, around the origin (`super::life` breathes it at the mast's top): a
/// soft gold glow and a thin ring (the ring alone without glows).
pub fn log_glow(pen: &mut Pen, pal: &Palette) {
    pen.glow(Vec2::ZERO, 4.2, vcol(pal.gold, 0.55));
    pen.circle(Vec2::ZERO, 0.62, 0.06, vcol(pal.gold, 0.85));
}

/// The EVOLUTION lab (GDR-0092): a chamfered block with window rows, the stack of compute rings seen from above (the
/// ring of segments on it that fills with the run's generations is `super::life`'s), a rack either side whose
/// windows flicker while a run computes, and the mind's violet conduit running down from it to the mind (the lab
/// is the mind's own simulation).
fn evolution_lab(pen: &mut Pen, l: &Layout, pal: &Palette, evo: Evo) {
    let (r, c) = (l.evolution.rect(), l.evolution.centre);
    let lit = evo != Evo::Dark;
    let col = if lit { pal.violet } else { pal.dim };
    let (a, z) = l.evolution_conduit;
    pen.line(a, z, 0.34, vcol(pal.hull_hi, 1.0));
    for dx in [-0.08, 0.08] {
        pen.line(a + Vec2::X * dx, z + Vec2::X * dx, 0.05, vcol(pal.violet, if lit { 0.6 } else { 0.3 }));
    }
    block(pen, pal, r, lit, pal.violet);
    busy(pen, evo == Evo::Running);
    for x in [c.x - 2.35, c.x + 2.35] {
        pen.boxed(Rect::from_center_size(Vec2::new(x, c.y), Vec2::new(0.7, 2.6)), vcol(pal.dark, 1.0), vcol(col, 0.4), 0.05);
        windows(pen, pal, &[(Vec2::new(x, c.y - 1.0), Vec2::new(x, c.y + 1.0), 4)], pal.violet, lit);
        busy(pen, evo == Evo::Running);
    }
    pen.disc(c, EVO_STACK, vcol(pal.structure, 1.0));
    pen.circle(c, EVO_STACK, 0.1, vcol(pal.structure_hi, 1.0));
    pen.circle(c, 1.3, 0.08, vcol(pal.hull_hi, 1.0));
    pen.circle(c, 0.9, 0.06, vcol(col, 0.6));
    pen.circle(c, 0.5, 0.06, vcol(col, 0.85));
    pen.disc(c, 0.16, vcol(col, 0.9));
}

/// The compute stack's radius and its ring of segments (between its edge and its outer ring): how many, how far
/// out and how wide.
const EVO_STACK: f32 = 1.78;
pub const EVO_SEGMENTS: usize = 16;
const EVO_RING: (f32, f32) = (1.55, 0.14);

/// The lab's ring of segments around the origin: the first `lit` of [`EVO_SEGMENTS`] lit teal (the run's generations
/// done, clockwise from the top), the rest dim; all dim while `on` is false (no run).
pub fn evolution_ring(pen: &mut Pen, pal: &Palette, lit: usize, on: bool) {
    let (r, w) = EVO_RING;
    let step = TAU / EVO_SEGMENTS as f32;
    for k in 0..EVO_SEGMENTS {
        let a0 = PI / 2.0 - k as f32 * step - 0.12 * step;
        let arc: Vec<Vec2> = (0..=3).map(|j| Vec2::from_angle(a0 - j as f32 * 0.25 * step) * r).collect();
        let (c, a) = if on && k < lit { (pal.teal, 0.95) } else { (pal.dim, if on { 0.55 } else { 0.3 }) };
        pen.path(&arc, false, w, vcol(c, a));
    }
}

/// The lab's glow once a run has finished, around the origin (`super::life` breathes it): soft gold and a ring.
pub fn evolution_glow(pen: &mut Pen, pal: &Palette) {
    pen.glow(Vec2::ZERO, 6.5, vcol(pal.gold, 0.5));
    pen.circle(Vec2::ZERO, 2.05, 0.07, vcol(pal.gold, 0.85));
}

/// The workshop (GDR-0108): three bays with a part on the bench in each, its windows lit (gold while hauled parts
/// wait to be sorted), and a crane arm off its outer face over the middle bay.
fn workshop(pen: &mut Pen, l: &Layout, pal: &Palette, waiting: bool) {
    let b = &l.workshop;
    let col = if waiting { pal.gold } else { pal.teal };
    block(pen, pal, b.rect(), true, col);
    let (c, s) = (b.centre, 0.0233);
    for k in 0..3 {
        let x = c.x + (-45.0 + 32.0 * k as f32) * s;
        let r = Rect::new(x, c.y - 22.0 * s, x + 24.0 * s, c.y + 22.0 * s);
        pen.boxed(r, vcol(pal.dark, 1.0), vcol(col, 0.55), 0.06);
        // The part on its bench.
        pen.boxed(Rect::from_center_size(Vec2::new(r.center().x, r.min.y + 0.35), Vec2::new(0.32, 0.32)), vcol(pal.hull_hi, 1.0), vcol(col, 0.8), 0.04);
    }
    // The crane arm over the middle bay, from the roof's left end.
    let roof = b.rect().min.y;
    pen.line(Vec2::new(c.x - 2.6, roof), Vec2::new(c.x - 2.6, roof - 0.9), 0.14, vcol(pal.hull_hi, 1.0));
    pen.line(Vec2::new(c.x - 2.6, roof - 0.9), Vec2::new(c.x + 0.4, roof - 0.9), 0.12, vcol(pal.hull_hi, 1.0));
    pen.line(Vec2::new(c.x + 0.2, roof - 0.9), Vec2::new(c.x + 0.2, roof - 0.35), 0.05, vcol(col, 0.7));
}

/// The mind's emblem plate: a hexagon (its spokes and core turn and pulse over it, `super::life`).
fn mind(pen: &mut Pen, pal: &Palette, c: Vec2) {
    let hex: Vec<Vec2> = (0..6).map(|k| c + Vec2::from_angle((60.0 * k as f32 + 30.0).to_radians()) * 2.64).collect();
    pen.poly(&hex, vcol(pal.structure, 1.0));
    pen.path(&hex, true, 0.15, vcol(pal.structure_hi, 1.0));
    // An inner ring the spokes run on.
    pen.circle(c, 1.87, 0.05, vcol(pal.violet, 0.25));
}

/// The mind's six spokes, around the origin.
pub fn mind_rotor(pen: &mut Pen, pal: &Palette) {
    for k in 0..6 {
        let end = Vec2::from_angle((60.0 * k as f32).to_radians()) * 1.87;
        pen.line(Vec2::ZERO, end, 0.07, vcol(pal.violet, 0.5));
        pen.disc(end, 0.17, vcol(pal.violet, 0.8));
    }
}

/// The mind's core around the origin: its glow and the diamond.
pub fn mind_core(pen: &mut Pen, pal: &Palette) {
    pen.glow(Vec2::ZERO, 5.2, vcol(pal.violet, 0.45));
    let d = 0.96;
    let diamond = [Vec2::new(0.0, d), Vec2::new(-d, 0.0), Vec2::new(0.0, -d), Vec2::new(d, 0.0)];
    pen.poly(&diamond, vcol(pal.dark, 1.0));
    pen.path(&diamond, true, 0.1, vcol(pal.violet, 1.0));
    pen.disc(Vec2::ZERO, 0.18, vcol(pal.violet, 0.9));
}

/// The mind's building: its block, cognition banks above and below the emblem, heat sinks on its top and bottom
/// faces, coolant runs down its sides and the collar into the base.
fn mind_house(pen: &mut Pen, l: &Layout, pal: &Palette) {
    let r = l.mind.rect();
    let c = l.mind.centre;
    block(pen, pal, r, true, pal.violet);
    for s in [1.0f32, -1.0] {
        // Cognition banks: two rows of cells, most of them thinking.
        let y = c.y + s * 3.55;
        pen.boxed(Rect::from_center_size(Vec2::new(c.x, y), Vec2::new(4.6, 0.95)), vcol(pal.dark, 1.0), vcol(pal.violet, 0.35), 0.05);
        windows(pen, pal, &[-0.2, 0.2].map(|dy| (Vec2::new(c.x - 2.1, y + dy), Vec2::new(c.x + 2.1, y + dy), 9)), pal.violet, true);
        // Heat sinks standing out of the top and bottom faces, right of the bridges' corners.
        let face = if s > 0.0 { r.max.y } else { r.min.y };
        for k in 0..4 {
            let x = r.min.x + 2.6 + 0.85 * k as f32;
            pen.boxed(Rect::from_corners(Vec2::new(x - 0.14, face), Vec2::new(x + 0.14, face + s * 0.8)), vcol(pal.hull_hi, 1.0), vcol(pal.violet, 0.5), 0.05);
        }
    }
    // Coolant runs down both sides of the emblem.
    for x in [r.min.x + 0.55, r.max.x - 0.55] {
        pen.line(Vec2::new(x, c.y - 2.9), Vec2::new(x, c.y + 2.9), 0.12, vcol(pal.hull_hi, 1.0));
        pen.line(Vec2::new(x, c.y - 2.9), Vec2::new(x, c.y + 2.9), 0.04, vcol(pal.violet, 0.45));
    }
    // The collar into the base, with the violet conduits on it.
    let back = l.base_x - BASE_STRIP;
    pen.boxed(Rect::new(r.max.x - 0.1, c.y - 2.4, back, c.y + 2.4), vcol(pal.hull_lo, 1.0), vcol(pal.hull_hi, 1.0), 0.08);
    for dy in [-1.6, -0.55, 0.55, 1.6] {
        pen.line(Vec2::new(r.max.x, c.y + dy), Vec2::new(back, c.y + dy), 0.08, vcol(pal.violet, 0.5));
    }
    mind(pen, pal, c);
}

/// Research's building: the lab block, a dome and the dishes' mounts on its roof, a rack of servers either side of
/// its emblem (lit while research runs), its antenna.
fn research_house(pen: &mut Pen, l: &Layout, pal: &Palette, lit: bool) {
    let r = &l.research;
    let col = if lit { pal.gold } else { pal.dim };
    block(pen, pal, r.rect(), lit, pal.gold);
    let roof = r.rect().max.y;
    // The dome: a half disc with its slit.
    let dome = Vec2::new(r.centre.x - 1.1, roof);
    let half: Vec<Vec2> = (0..=12).map(|k| dome + Vec2::from_angle(PI * k as f32 / 12.0) * 0.85).collect();
    pen.poly(&half, vcol(pal.hull_hi, 1.0));
    pen.path(&half, false, 0.06, vcol(col, 0.7));
    pen.line(dome, dome + Vec2::from_angle(1.2) * 0.8, 0.12, vcol(pal.dark, 1.0));
    // The dishes' mounts (the dishes turn on them).
    for (at, _) in research_dishes(l) {
        pen.line(Vec2::new(at.x, roof), at, 0.16, vcol(pal.hull_hi, 1.0));
    }
    // Server racks either side of the emblem.
    for x in [r.centre.x - 3.3, r.centre.x + 3.3] {
        pen.boxed(Rect::from_center_size(Vec2::new(x, r.centre.y), Vec2::new(1.3, 2.4)), vcol(pal.dark, 1.0), vcol(col, 0.4), 0.05);
        let rows = [0.0, 1.0, 2.0, 3.0].map(|k| {
            let y = r.centre.y - 0.8 + 0.53 * k;
            (Vec2::new(x - 0.45, y), Vec2::new(x + 0.45, y), 3)
        });
        windows(pen, pal, &rows, pal.gold, lit);
    }
    antenna(pen, pal, r.centre + Vec2::new(1.0, 2.75), Vec2::new(0.0, 1.3), pal.gold);
}

/// Research's dishes: where each turns, its size and the way it faces at rest.
pub fn research_dishes(l: &Layout) -> [(Vec2, (f32, f32)); 2] {
    let c = l.research.centre;
    [(c + Vec2::new(-3.5, 3.1), (0.9, PI / 2.0)), (c + Vec2::new(3.3, 3.1), (0.7, PI / 3.0))]
}

/// A square turned by `angle` at `c`.
fn turned_square(pen: &mut Pen, c: Vec2, side: f32, angle: f32, fill: [f32; 4], stroke: [f32; 4], width: f32) {
    let r = Vec2::from_angle(angle);
    let h = side / 2.0;
    let pts = [Vec2::new(-h, -h), Vec2::new(h, -h), Vec2::new(h, h), Vec2::new(-h, h)].map(|p| c + r.rotate(p));
    pen.poly(&pts, fill);
    pen.path(&pts, true, width, stroke);
}

/// The gate: its ring and the four big pods, glowing and filled while awake (the dashes and small emitters turn,
/// [`gate_ring`]).
fn gate(pen: &mut Pen, pal: &Palette, c: Vec2, awake: bool) {
    let r = GATE_RADIUS;
    let col = if awake { pal.teal } else { pal.dim };
    if awake {
        pen.halo(&ring(c, r, 96), true, 1.2, vcol(pal.teal, 0.45));
        pen.pool(c, Vec2::splat(2.0 * r), vcol(pal.teal, 0.2));
    }
    pen.circle(c, r, 0.16 * r, vcol(pal.structure_hi, 1.0));
    for k in 0..4 {
        let a = PI / 4.0 + k as f32 * PI / 2.0;
        turned_square(pen, c + Vec2::from_angle(a) * (r + GATE_POD), 0.9, a, vcol(pal.hull, 1.0), vcol(col, 0.7), 0.075);
    }
}

/// The gate's dashes and small emitters around the origin (the sketch's dashes: 3 px on, 9 off around a 150 px
/// ring), turning slowly while it is awake (`super::life`).
pub fn gate_ring(pen: &mut Pen, pal: &Palette, awake: bool) {
    let r = GATE_RADIUS;
    let col = if awake { pal.teal } else { pal.dim };
    let n = 78;
    for k in 0..n {
        let a0 = k as f32 / n as f32 * TAU;
        let arc: Vec<Vec2> = (0..=2).map(|j| Vec2::from_angle(a0 + j as f32 * 0.25 * TAU / n as f32) * r).collect();
        pen.path(&arc, false, 0.1, vcol(col, 0.9));
    }
    for k in 0..8 {
        let a = k as f32 * PI / 4.0;
        turned_square(pen, Vec2::from_angle(a) * r, 0.7, a, vcol(pal.structure, 1.0), vcol(col, 0.6), 0.05);
    }
}

/// The approach path's chevrons lit over the path's own, each as bright as its `glow` (`super::life` runs a wave up
/// them).
pub fn approach_lights(pen: &mut Pen, pal: &Palette, chevrons: &[Vec2], glow: impl Iterator<Item = f32>) {
    for (c, g) in chevrons.iter().zip(glow) {
        if g > 0.0 {
            let s = CHEVRON;
            pen.glow(*c + Vec2::X * s / 2.0, 1.6, vcol(pal.teal, 0.7 * g));
            pen.path(&[*c + Vec2::new(0.0, s), *c + Vec2::new(s, 0.0), *c - Vec2::new(0.0, s)], false, 0.1, vcol(pal.teal, g));
        }
    }
}

/// A shuttle heading +x: a small hull, its cockpit, an engine glow behind it.
pub fn shuttle(pen: &mut Pen, pal: &Palette) {
    pen.glow(Vec2::new(-0.55, 0.0), 0.9, vcol(pal.teal, 0.5));
    let hull = [Vec2::new(0.55, 0.0), Vec2::new(0.25, 0.26), Vec2::new(-0.45, 0.26), Vec2::new(-0.45, -0.26), Vec2::new(0.25, -0.26)];
    pen.poly(&hull, vcol(pal.hull_hi, 1.0));
    pen.path(&hull, true, 0.04, vcol(pal.structure_hi, 1.0));
    pen.rect(Rect::new(0.08, -0.08, 0.28, 0.08), vcol(pal.gold, 0.9));
    pen.rect(Rect::new(-0.5, -0.14, -0.42, 0.14), vcol(pal.teal, 0.9));
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
fn mooring(pen: &mut Pen, pal: &Palette, c: Vec2, open: f32, lit: bool) {
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
    let rows = [(p(x1 - 1.5, y1 - 0.3), p(back + 1.0, y1 - 0.3), 10), (p(x1 - 1.5, y0 + 0.3), p(back + 1.0, y0 + 0.3), 10), (p(back + 0.3, -hw), p(back + 0.3, hw), 9)];
    windows(pen, pal, &rows, pal.gold, lit);
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
    // The truss bridges to the moorings, bending into the mind's corners, and the cross bar at their far end.
    let (bx0, bx1) = l.bridge_x;
    for (y, (bend, end)) in l.bridges.into_iter().zip(l.mind_joints) {
        truss(pen, pal, Vec2::new(bx0, y), Vec2::new(bx1, y), BRIDGE, ((bx1 - bx0) / 1.1) as usize);
        truss(pen, pal, bend, end, BRIDGE, 2);
        pen.disc(bend, BRIDGE * 0.75, vcol(pal.hull_hi, 1.0));
        pen.circle(bend, BRIDGE * 0.75, 0.06, vcol(pal.dark, 1.0));
        pen.disc(bend, 0.12, vcol(pal.dark, 1.0));
    }
    pen.line(Vec2::new(bx0, l.bridges[0]), Vec2::new(bx0, l.bridges[1]), 0.5, vcol(pal.hull_hi, 1.0));
    for (k, c) in l.moorings.iter().enumerate() {
        mooring(pen, pal, *c, if c.y > l.area.center().y { 1.0 } else { -1.0 }, k < lights.moored);
    }
    // Struts under the buildings (the mind's collar is its own), and the one from the range's tower to its lane.
    for b in [&l.research, &l.range, &l.workshop, &l.log, &l.evolution] {
        strut(pen, pal, b.strut.0, b.strut.1);
    }
    let t = l.range.rect();
    strut(pen, pal, Vec2::new(t.max.x - 0.1, l.lane.0.y), l.lane.0 - Vec2::X * 1.0);
    research_house(pen, l, pal, lights.research);
    workshop(pen, l, pal, lights.workshop);
    mind_house(pen, l, pal);
    log_mast(pen, l, pal);
    evolution_lab(pen, l, pal, lights.evolution);
    // The test range: its control tower and the target lane.
    block(pen, pal, t, true, pal.teal);
    pen.boxed(Rect::from_center_size(t.center(), t.size() * Vec2::new(0.64, 0.44)), vcol(pal.dark, 1.0), vcol(pal.teal, 0.5), 0.06);
    lane(pen, pal, l.lane.0, l.lane.1);
    // The approach path, the pylons (trusses from the arm tips to the gate's pods) and the gate.
    let path_col = if lights.gate { pal.teal } else { pal.dim };
    for (k, c) in l.chevrons.iter().enumerate() {
        let s = CHEVRON;
        pen.path(&[*c + Vec2::new(0.0, s), *c + Vec2::new(s, 0.0), *c - Vec2::new(0.0, s)], false, 0.08, vcol(path_col, 0.25 + 0.1 * k as f32));
    }
    for (a, b) in l.pylons {
        truss(pen, pal, a, b, 0.6, (a.distance(b) / 1.1) as usize);
        let d = (b - a).normalize_or_zero();
        turned_square(pen, a + d * 0.2, 0.75, d.to_angle(), vcol(pal.hull_hi, 1.0), vcol(pal.dark, 1.0), 0.05);
    }
    gate(pen, pal, l.gate, lights.gate);
    for (k, (_, b)) in l.pylons.iter().enumerate() {
        pen.nav(*b, if k == 1 { pal.port } else { pal.starboard }, pal.lamp);
    }
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
