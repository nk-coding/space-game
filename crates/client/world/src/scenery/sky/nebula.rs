//! The nebula and the far galaxy (GDR-0105), rolled and meshed in the design frame (px of a 1600 × 900 view, y
//! down, origin at the view's top left).
//!
//! The nebula is soft blobs strung along a curve across the view: wide washes in its two colours, a few brighter
//! cores with stars in them, thin faint dust lanes; each blob a fan whose alpha falls from its middle to its wobbly
//! edge. No point of the view lies under more than [`STACK`] blobs (checked on a coarse grid; one less beside a hot
//! star), the overdraw cap. The
//! galaxy is a soft glowing spiral (Niklas, 18 Oct 2026: it read as a cloud of dots): a smooth warm core, a soft
//! glow along each arm, fine faint dots over the glow, a faint halo.

use std::f32::consts::TAU;

use bevy::prelude::*;

use super::paint::Geo;
use super::roll::{Rgb, Stream, WHITE, Wob, hex, mix, vc};
use super::{H, W};

/// The most blobs over any point (one less beside a hot star, whose glow is a large translucent layer too).
const STACK: u8 = 3;
/// Sprites along each of the galaxy's arms for its glow.
const GLOW_STEPS: usize = 36;
/// The coverage grid.
const GX: usize = 64;
const GY: usize = 36;

/// A soft blob: centre, radii, turn, outline, colour, alpha.
struct Blob {
    at: Vec2,
    r: Vec2,
    rot: f32,
    f: Wob,
    col: Rgb,
    a: f32,
}

impl Blob {
    fn point(&self, t: f32, s: f32) -> Vec2 {
        let f = self.f.at(t) * s;
        let l = Vec2::new(t.cos() * self.r.x * f, t.sin() * self.r.y * f);
        self.at + Vec2::from_angle(self.rot).rotate(l)
    }

    /// The grid cells it covers.
    fn cells(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let back = Vec2::from_angle(-self.rot);
        for j in 0..GY {
            for i in 0..GX {
                let p = back.rotate(Vec2::new((i as f32 + 0.5) / GX as f32 * W, (j as f32 + 0.5) / GY as f32 * H) - self.at);
                let u = Vec2::new(p.x / self.r.x, p.y / self.r.y);
                let rr = u.length();
                if rr <= self.f.max && rr < self.f.at(u.y.atan2(u.x)) {
                    out.push(j * GX + i);
                }
            }
        }
        out
    }
}

/// The nebula: washes and cores (light), lanes (dark), stars in the cores; the most blobs over a point.
pub(super) struct Nebula {
    light: Vec<Blob>,
    lanes: Vec<Blob>,
    stars: Vec<(Vec2, f32, Rgb)>,
    pub peak: u8,
}

/// Rolls the nebula in the sky's colours and strength, `dark` for its lanes; `hot`: beside a hot star.
pub(super) fn nebula(r: &mut Stream, sky: &data::Sky, dark: Rgb, hot: bool) -> Nebula {
    let stack = if hot { STACK - 1 } else { STACK };
    let (n0, n1, core, s) = (hex(&sky.nebula.0), hex(&sky.nebula.1), hex(&sky.nebula_core), sky.nebula_strength);
    let flip = r.chance(0.5);
    let mut p = [
        Vec2::new(r.range(-250.0, 350.0), r.range(-150.0, H + 150.0)),
        Vec2::new(r.range(250.0, 800.0), r.range(-100.0, H + 100.0)),
        Vec2::new(r.range(800.0, 1350.0), r.range(-100.0, H + 100.0)),
        Vec2::new(r.range(1250.0, 1850.0), r.range(-150.0, H + 150.0)),
    ];
    if flip {
        for q in &mut p {
            q.x = W - q.x;
        }
    }
    let bez = |t: f32| {
        let u = 1.0 - t;
        p[0] * u * u * u + p[1] * 3.0 * u * u * t + p[2] * 3.0 * u * t * t + p[3] * t * t * t
    };
    let tan = |t: f32| {
        let d = bez((t + 0.01).min(1.0)) - bez((t - 0.01).max(0.0));
        d.y.atan2(d.x)
    };
    let mut grid = [0u8; GX * GY];
    let mut add = |list: &mut Vec<Blob>, mut b: Blob| -> bool {
        for _ in 0..5 {
            let cells = b.cells();
            if cells.iter().all(|&i| grid[i] < stack) {
                cells.iter().for_each(|&i| grid[i] += 1);
                list.push(b);
                return true;
            }
            b.r *= 0.85;
        }
        false
    };
    let mut neb = Nebula { light: Vec::new(), lanes: Vec::new(), stars: Vec::new(), peak: 0 };
    let nw = r.int(5, 7);
    for i in 0..nw {
        let t = (i as f32 / (nw - 1) as f32 + r.range(-0.06, 0.06)).clamp(0.0, 1.0);
        let rx = r.range(200.0, 340.0);
        let b = Blob {
            at: bez(t) + Vec2::new(r.gauss(), r.gauss()) * 45.0,
            r: Vec2::new(rx, rx * r.range(0.42, 0.72)),
            rot: tan(t) + r.range(-0.35, 0.35),
            f: Wob::new(r, 0.24, 6, 1.1),
            col: mix(n0, n1, (t + r.range(-0.2, 0.2)).clamp(0.0, 1.0)),
            a: r.range(0.2, 0.3) * s,
        };
        add(&mut neb.light, b);
    }
    for _ in 0..r.int(2, 4) {
        let t = r.range(0.15, 0.85);
        let (th, off, rx) = (tan(t), r.range(-40.0, 40.0), r.range(90.0, 180.0));
        let b = Blob {
            at: bez(t) + Vec2::new(-th.sin(), th.cos()) * off,
            r: Vec2::new(rx, rx * r.range(0.5, 0.8)),
            rot: th + r.range(-0.5, 0.5),
            f: Wob::new(r, 0.26, 6, 1.1),
            col: mix(core, n1, r.range(0.0, 0.4)),
            a: r.range(0.22, 0.32) * s,
        };
        let (at, rr) = (b.at, b.r);
        if add(&mut neb.light, b) {
            for _ in 0..r.int(2, 4) {
                let p = at + Vec2::new(r.gauss() * rr.x, r.gauss() * rr.y) * 0.3;
                neb.stars.push((p, r.range(1.0, 2.0), mix(core, WHITE, 0.7)));
            }
        }
    }
    for _ in 0..r.int(1, 3) {
        let t = r.range(0.2, 0.8);
        let (th, off, rx) = (tan(t), r.range(-50.0, 50.0), r.range(140.0, 240.0));
        neb.lanes.push(Blob {
            at: bez(t) + Vec2::new(-th.sin(), th.cos()) * off,
            r: Vec2::new(rx, rx * r.range(0.07, 0.13)),
            rot: th + r.range(-0.15, 0.15),
            f: Wob::new(r, 0.3, 6, 1.1),
            col: dark,
            a: r.range(0.14, 0.22),
        });
    }
    neb.peak = grid.iter().copied().max().unwrap_or(0);
    neb
}

/// A blob as a fan: alpha `a` in the middle falling smoothly to nothing at the edge.
fn blob(g: &mut Geo, b: &Blob, strength: f32) {
    let (rings, segs) = (7u32, 40u32);
    let mid = g.v(b.at, vc(b.col, b.a * strength));
    let first = g.len() as u32;
    for k in 1..=rings {
        let s = k as f32 / rings as f32;
        let a = b.a * strength * (1.0 - s).powf(1.4);
        for i in 0..segs {
            g.v(b.point(i as f32 / segs as f32 * TAU, s), vc(b.col, a));
        }
    }
    for i in 0..segs {
        g.tri(mid, first + i, first + (i + 1) % segs);
    }
    for k in 0..rings - 1 {
        let (a, c) = (first + k * segs, first + (k + 1) * segs);
        for i in 0..segs {
            let j = (i + 1) % segs;
            g.quad(a + i, c + i, c + j, a + j);
        }
    }
}

/// The nebula's mesh (origin at the design frame's centre), at `strength` (the backdrop's).
pub(super) fn nebula_mesh(n: &Nebula, white: Vec2, dot: [Vec2; 4], strength: f32) -> Geo {
    let mut g = Geo::new(white);
    let centre = Vec2::new(W, H) / 2.0;
    let shift = |b: &Blob| Blob { at: b.at - centre, r: b.r, rot: b.rot, f: b.f.clone(), col: b.col, a: b.a };
    for b in n.light.iter().chain(&n.lanes) {
        blob(&mut g, &shift(b), strength);
    }
    for (p, s, c) in &n.stars {
        let p = *p - centre;
        g.sprite(p, Vec2::splat(s * 3.0), dot, vc(*c, 0.85 * strength));
        g.sprite(p, Vec2::new(10.0, 0.9), dot, vc(*c, 0.35 * strength));
        g.sprite(p, Vec2::new(0.9, 10.0), dot, vc(*c, 0.35 * strength));
    }
    g
}

/// The far galaxy: where (design frame), its radius, flattening and turn, and its dots (offset, size, colour,
/// alpha) in its own frame.
pub(super) struct Galaxy {
    pub at: Vec2,
    pub rg: f32,
    k: f32,
    rot: f32,
    dots: Vec<(Vec2, f32, Rgb, f32)>,
    strength: f32,
}

/// Rolls the galaxy clear of the view's middle (the planets pass over it).
pub(super) fn galaxy(r: &mut Stream, strength: f32) -> Galaxy {
    let rg = r.range(70.0, 140.0);
    let mut best = (Vec2::ZERO, f32::MIN);
    for _ in 0..40 {
        let at = Vec2::new(r.range(rg + 30.0, W - rg - 30.0), r.range(rg * 0.6 + 30.0, H - rg * 0.6 - 30.0));
        // the first place clear of the middle, else the clearest
        let score = (at.distance(Vec2::new(W, H) / 2.0) - 260.0 - rg).min(0.0);
        if score > best.1 {
            best = (at, score);
        }
    }
    let k = r.range(0.28, 0.68);
    let rot = r.range(0.0, std::f32::consts::PI);
    let arms = *r.pick(&[2, 2, 3, 4]);
    let b = r.range(0.22, 0.36);
    let turns = r.range(1.1, 1.7);
    let dir = if r.chance(0.5) { 1.0 } else { -1.0 };
    let warm = Vec3::new(1.0, 0.925, 0.784);
    let blue = Vec3::new(0.667, 0.769, 1.0);
    let pink = Vec3::new(1.0, 0.627, 0.784);
    let th_max = turns * TAU;
    let a0 = rg / (b * th_max).exp();
    let mut dots = Vec::new();
    // The arms' glow first (under the dots): wide soft sprites strung along each spiral, overlapping into a ribbon
    // that fades outwards. No draws: the spiral alone places them.
    for arm in 0..arms {
        for j in 0..GLOW_STEPS {
            let t = (j as f32 + 0.5) / GLOW_STEPS as f32;
            let th = t.powf(0.75) * th_max;
            let rr = a0 * (b * th).exp();
            let ang = dir * th + arm as f32 * TAU / arms as f32;
            let size = rg * (0.05 + 0.05 * t);
            dots.push((Vec2::from_angle(ang) * rr, size, mix(warm, blue, (t * 1.3).min(1.0)), 0.16 * (1.0 - 0.7 * t)));
        }
    }
    for _ in 0..500 {
        let rr = r.gauss().abs() * rg * 0.12;
        let th = r.range(0.0, TAU);
        dots.push((Vec2::from_angle(th) * rr, r.range(0.4, 0.8), warm, r.range(0.15, 0.4)));
    }
    for i in 0..2200 {
        let arm = (i % arms) as f32;
        let t = r.next().powf(0.75);
        let th = t * th_max;
        let rr = a0 * (b * th).exp() + r.gauss() * rg * (0.02 + 0.045 * t);
        let ang = dir * th + arm * TAU / arms as f32 + r.gauss() * 0.08;
        let knot = r.chance(0.03);
        let (size, col, a) = if knot { (1.0, pink, 0.45) } else { (r.range(0.35, 0.7), mix(warm, blue, (t * 1.4).min(1.0)), r.range(0.1, 0.32)) };
        dots.push((Vec2::from_angle(ang) * rr, size, col, a * (1.0 - 0.5 * t)));
    }
    Galaxy { at: best.0, rg, k, rot, dots, strength }
}

/// The galaxy's mesh (origin at its centre), at the backdrop's `strength`.
pub(super) fn galaxy_mesh(gx: &Galaxy, white: Vec2, dot: [Vec2; 4], strength: f32) -> Geo {
    let mut g = Geo::new(white);
    let a = gx.strength * strength;
    let place = |p: Vec2| Vec2::from_angle(gx.rot).rotate(Vec2::new(p.x, p.y * gx.k));
    // the halo: a faint flattened disc
    let halo = Vec3::new(0.78, 0.82, 1.0);
    let mid = g.v(Vec2::ZERO, vc(halo, 0.09 * a));
    let first = g.len() as u32;
    let segs = 40;
    for i in 0..segs {
        g.v(place(Vec2::from_angle(i as f32 / segs as f32 * TAU) * gx.rg * 1.1), vc(halo, 0.0));
    }
    for i in 0..segs {
        g.tri(mid, first + i, first + (i + 1) % segs);
    }
    for (p, s, c, al) in &gx.dots {
        // soft dots a little larger than the study's squares, as faint, so sub-pixel ones still show smoothly
        g.sprite(place(*p), Vec2::new(s * 2.2, s * 2.2 * gx.k.max(0.6)), dot, vc(*c, al * a * 0.6));
    }
    // The core: a bright middle falling off smoothly through rings to nothing.
    let warm = Vec3::new(1.0, 0.925, 0.784);
    let core = Vec3::new(1.0, 0.965, 0.88);
    let mid = g.v(Vec2::ZERO, vc(core, 0.6 * a));
    let first = g.len() as u32;
    let rings = [(0.03, 0.45), (0.07, 0.27), (0.13, 0.14), (0.21, 0.06), (0.32, 0.0)];
    for (s, al) in rings {
        for i in 0..segs {
            g.v(place(Vec2::from_angle(i as f32 / segs as f32 * TAU) * gx.rg * s), vc(warm, al * a));
        }
    }
    for i in 0..segs {
        let j = (i + 1) % segs;
        g.tri(mid, first + i, first + j);
        for k in 0..rings.len() as u32 - 1 {
            let (o, n) = (first + k * segs, first + (k + 1) * segs);
            g.quad(o + i, n + i, n + j, o + j);
        }
    }
    g
}
