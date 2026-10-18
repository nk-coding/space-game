//! The far galaxy (GDR-0105, GDR-0107), ported from the backdrop study's version 8 (Niklas, 18 Oct 2026): a spiral
//! is variant H (flocculent, like M74), an elliptical, edge-on or irregular galaxy variant D (a soft field with
//! splats). Built once per voyage in the design frame (px, y down), origin at the galaxy's centre, as three static
//! meshes drawn in this order:
//!
//! * **the field** ([`Galaxy::field`]), premultiplied: one grid (polar, 31 × 128; an edge-on disc's 110 × 25 across
//!   it) whose every vertex samples the galaxy's light and dust; the dust dims what lies below, the light adds;
//! * **the dust** ([`Galaxy::dust`]), alpha-blended: a spiral's thin filaments on the inner edges of its inner arm
//!   segments, the other kinds' patchy dark splats;
//! * **the glow** ([`Galaxy::glow`]), additive: a spiral's 38 short wound arm segments (a third on two main arms),
//!   each a chain of small blue-violet clumps, a violet haze of splats, magenta knots and a soft core; the other
//!   kinds' clumps of splats with pink cores; a few sparkles.
//!
//! About 17 000 vertices for a spiral. Distances in the galaxy's own frame are shares of its radius, before it is
//! flattened by `k` and turned.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use super::paint::Geo;
use super::roll::{Rgb, Stream, WHITE, mix, smooth, vc};
use super::{H, W};

/// An sRGB colour from bytes.
const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Vec3::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

const CORE: Rgb = rgb(255, 242, 222);
const WARM: Rgb = rgb(255, 206, 152);
const DISK: Rgb = rgb(196, 188, 210);
const BLUE: Rgb = rgb(132, 172, 255);
const BLUE2: Rgb = rgb(198, 220, 255);
const PINK: Rgb = rgb(255, 108, 168);
const DUST: Rgb = rgb(28, 15, 11);
/// Variant H's palette.
const H_HAZE: Rgb = rgb(146, 132, 236);
const H_BLUE: Rgb = rgb(118, 140, 255);
const H_LAV: Rgb = rgb(176, 140, 255);
const H_HOT: Rgb = rgb(205, 215, 255);
const H_PINK: Rgb = rgb(255, 84, 196);
const H_DUST: Rgb = rgb(44, 18, 14);
/// The arms' winding in the spiral's field and segments.
const H_WIND: f32 = 0.34;
/// Wound arm segments of a spiral.
const SEGMENTS: usize = 38;

/// What kind of galaxy a sky shows (spirals most common: the study's spiral and barred spiral, which variant H draws
/// alike).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Kind {
    Spiral,
    Elliptical,
    EdgeOn,
    Irregular,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Spiral => "spiral",
            Kind::Elliptical => "elliptical",
            Kind::EdgeOn => "edge-on",
            Kind::Irregular => "irregular",
        }
    }
}

/// An irregular galaxy's clump of light.
struct Clump {
    at: Vec2,
    s: f32,
    w: f32,
    blue: bool,
}

/// A spiral's wound arm segment: a log spiral from radius `r0` at angle `ph`, winding `b`, `len` radians long.
struct Segment {
    r0: f32,
    ph: f32,
    b: f32,
    len: f32,
    w: f32,
}

/// The far galaxy: where (design frame), its radius, kind and shape, and the seed of its own streams.
pub(super) struct Galaxy {
    pub at: Vec2,
    pub rg: f32,
    pub kind: Kind,
    arms: u32,
    /// The arms' winding, which way they wind and where they start (the sparkles follow these arms).
    b: f32,
    dir: f32,
    ph0: f32,
    k: f32,
    rot: f32,
    seed: u64,
    n1: f32,
    n2: f32,
    clumps: Vec<Clump>,
    strength: f32,
}

/// The three meshes of [`Galaxy`] (see the module's docs).
pub(super) struct Meshes {
    pub field: Geo,
    pub dust: Geo,
    pub glow: Geo,
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
    let kind = [Kind::Spiral, Kind::Elliptical, Kind::EdgeOn, Kind::Irregular][r.weighted(&[60, 12, 16, 12])];
    let arms = if r.chance(0.75) { 2 } else { 3 };
    let b = r.range(0.22, 0.3);
    let dir = if r.chance(0.5) { 1.0 } else { -1.0 };
    let ph0 = r.range(0.0, TAU);
    let k = match kind {
        Kind::EdgeOn => r.range(0.11, 0.15),
        Kind::Elliptical => r.range(0.55, 0.9),
        _ => r.range(0.45, 0.85),
    };
    let rot = r.range(0.0, PI);
    let seed = (r.next() as f64 * 4_294_967_296.0) as u64;
    let (n1, n2) = (r.range(0.0, TAU), r.range(0.0, TAU));
    let clumps = if kind == Kind::Irregular {
        let mut c = Stream::new(seed, "irr");
        (0..6).map(|_| Clump { at: Vec2::new(c.gauss() * 0.3, c.gauss() * 0.22), s: c.range(0.12, 0.3), w: c.range(0.4, 1.0), blue: c.chance(0.75) }).collect()
    } else {
        Vec::new()
    };
    Galaxy { at: best.0, rg: rg * 1.15, kind, arms, b, dir, ph0, k, rot, seed, n1, n2, clumps, strength }
}

/// The study's cheap smooth noise in about −1..1.
fn nz2(x: f32, y: f32, a: f32, b: f32) -> f32 {
    ((x * 2.3 + a + (y * 1.7 + b).sin() * 1.3).sin() * (y * 2.9 - b + (x * 2.1 + a).sin() * 1.1).sin()
        + 0.5 * (x * 5.3 + y * 4.1 + a * 2.0).sin() * (x * 4.4 - y * 6.2 + b).sin())
        / 1.5
}

/// Draws into a mesh in the galaxy's frame: placed (scaled by its radius, flattened by `k`, turned), every alpha
/// times the gain.
struct Pen<'a> {
    g: &'a mut Geo,
    rg: f32,
    turn: Vec2,
    k: f32,
    gain: f32,
}

impl Pen<'_> {
    fn at(&self, p: Vec2) -> Vec2 {
        self.turn.rotate(Vec2::new(p.x, p.y * self.k)) * self.rg
    }

    fn v(&mut self, p: Vec2, c: Rgb, a: f32) -> u32 {
        let at = self.at(p);
        self.g.v(at, vc(c, a * self.gain))
    }

    /// A disc round `c` of `n` segments through rings of (radius, colour, alpha) from the middle (radius 0) out.
    fn disc(&mut self, c: Vec2, n: u32, rings: &[(f32, Rgb, f32)]) {
        let (_, col, a) = rings[0];
        let mid = self.v(c, col, a);
        let first = mid + 1;
        for &(r, col, a) in &rings[1..] {
            for i in 0..n {
                self.v(c + Vec2::from_angle(i as f32 / n as f32 * TAU) * r, col, a);
            }
        }
        for i in 0..n {
            let j = (i + 1) % n;
            self.g.tri(mid, first + i, first + j);
            for ring in 0..rings.len() as u32 - 2 {
                let (o, p) = (first + ring * n, first + (ring + 1) * n);
                self.g.quad(o + i, p + i, p + j, o + j);
            }
        }
    }

    /// A soft splat: `c` at alpha `a` in the middle, fading to nothing at radius `r`.
    fn soft(&mut self, c: Vec2, r: f32, col: Rgb, a: f32, n: u32) {
        self.disc(c, n, &[(0.0, col, a), (r, col, 0.0)]);
    }

    /// A soft ribbon along `pts`: three vertices across (`half` either side), each point's colour and alpha along the
    /// middle, nothing at the edges.
    fn ribbon(&mut self, pts: &[Vec2], half: &[f32], col: Rgb, a: &[f32]) {
        let n = pts.len();
        let first = self.g.len() as u32;
        for i in 0..n {
            let d = (pts[(i + 1).min(n - 1)] - pts[i.saturating_sub(1)]).normalize_or(Vec2::X);
            let nrm = Vec2::new(-d.y, d.x) * half[i];
            self.v(pts[i] + nrm, col, 0.0);
            self.v(pts[i], col, a[i]);
            self.v(pts[i] - nrm, col, 0.0);
        }
        for i in 0..n as u32 - 1 {
            let (p, q) = (first + 3 * i, first + 3 * (i + 1));
            self.g.quad(p, q, q + 1, p + 1);
            self.g.quad(p + 1, q + 1, q + 2, p + 2);
        }
    }
}

impl Galaxy {
    /// The galaxy's three meshes at the backdrop's `strength`.
    pub fn meshes(&self, white: Vec2, strength: f32) -> Meshes {
        let gain = self.strength * strength;
        let mut m = Meshes { field: Geo::new(white), dust: Geo::new(white), glow: Geo::new(white) };
        self.field(&mut m.field, gain);
        let mut glow = Stream::new(self.seed, if self.kind == Kind::Spiral { "galH" } else { "galD" });
        if self.kind == Kind::Spiral {
            let segs = self.segments();
            self.filaments(&mut self.pen(&mut m.dust, gain), &segs);
            self.arms(&mut self.pen(&mut m.glow, gain), &mut glow, &segs);
            self.sparkle(&mut self.pen(&mut m.glow, gain), &mut glow, 2);
        } else {
            self.splats(&mut self.pen(&mut m.dust, gain), &mut self.pen(&mut m.glow, gain), &mut glow);
            self.sparkle(&mut self.pen(&mut m.glow, gain), &mut glow, 4);
        }
        m
    }

    fn pen<'a>(&self, g: &'a mut Geo, gain: f32) -> Pen<'a> {
        Pen { g, rg: self.rg, turn: Vec2::from_angle(self.rot), k: self.k, gain }
    }

    /// The light (premultiplied, sRGB) and the dust's alpha at `p`.
    fn sample(&self, p: Vec2) -> (Rgb, f32) {
        let (x, y) = (p.x, p.y);
        let r = p.length() + 1e-4;
        let (qx, qy) = (x * 0.8 + y * 0.6, y * 0.8 - x * 0.6);
        let (f1, f2) = if self.kind == Kind::Irregular { (2.5, 4.0) } else { (6.0, 11.0) };
        let n1 = nz2(qx * f1, qy * f1, self.n1, self.n2);
        let n2 = nz2(qx * f2 + 3.0, qy * f2 - 2.0, self.n2, self.n1);
        let mut e = Vec3::ZERO;
        let a = match self.kind {
            Kind::Spiral => {
                let ph = y.atan2(x);
                let lr = (r.max(0.03) / 0.08).ln() / H_WIND;
                let s = (2.0 * (ph - self.ph0 - self.dir * lr)).cos();
                let edge = 1.0 - smooth(0.72, 1.08, r + 0.1 * n1);
                e += rgb(255, 236, 222) * 0.9 * (-r / 0.015).exp();
                e += rgb(255, 200, 200) * (0.7 * (-r / 0.06).exp() + 0.38 * (-r / 0.15).exp());
                e += H_HAZE * (0.64 * (-r / 0.3).exp() + 0.05) * edge * (1.0 + 1.3 * (((1.0 + s) / 2.0).powi(2) - 0.35)) * (0.8 + 0.3 * n1);
                0.18 * ((1.0 + (2.0 * (ph - self.ph0 - self.dir * (lr + 0.4))).cos()) / 2.0).powi(6)
                    * smooth(0.04, 0.1, r)
                    * (1.0 - smooth(0.3, 0.6, r))
                    * (0.6 + 0.5 * n2)
            }
            Kind::Elliptical => {
                e += mix(CORE, rgb(236, 202, 170), smooth(0.0, 0.3, r))
                    * (1.5 * (-r / 0.03).exp() + 0.55 * (-r / 0.1).exp() + 0.14 * (-r / 0.3).exp())
                    * (1.0 - smooth(0.8, 1.1, r));
                0.0
            }
            Kind::EdgeOn => {
                let ax = x.abs();
                let rb = Vec2::new(x, y / 0.6).length();
                e += CORE * (1.2 * (-rb / 0.045).exp() + 0.35 * (-rb / 0.13).exp());
                e += mix(WARM, BLUE, smooth(0.15, 0.7, ax)) * 0.55 * (-ax / 0.3).exp() * (-y.abs() / 0.026).exp() * (1.0 - smooth(0.82, 1.04, ax)) * (0.8 + 0.3 * n1);
                0.88 * (-((y - 0.006) / 0.012).powi(2)).exp() * (1.0 - smooth(0.5, 0.95, ax)) * (0.7 + 0.3 * n2)
            }
            Kind::Irregular => {
                for c in &self.clumps {
                    e += if c.blue { BLUE } else { WARM } * 0.45 * c.w * (-(p - c.at).length_squared() / (c.s * c.s)).exp() * (0.7 + 0.5 * n1);
                }
                e += DISK * 0.06 * (-r / 0.35).exp();
                0.5 * (n2 - 0.25).max(0.0) * (-r / 0.3).exp()
            }
        };
        ((e * (1.0 - 0.85 * a)).min(Vec3::ONE), a)
    }

    /// The field: rows of points sampled, neighbouring rows stitched (closed round a polar grid).
    fn field(&self, g: &mut Geo, gain: f32) {
        let rows: Vec<Vec<Vec2>> = if self.kind == Kind::EdgeOn {
            let (nx, ny) = (110, 25);
            let warp = |v: f32, e: f32| v.signum() * v.abs().powf(e);
            (0..ny)
                .map(|j| {
                    let y = warp(j as f32 / (ny - 1) as f32 * 2.0 - 1.0, 1.8) * 0.32;
                    (0..nx).map(|i| Vec2::new(warp(i as f32 / (nx - 1) as f32 * 2.0 - 1.0, 1.15) * 1.08, y)).collect()
                })
                .collect()
        } else {
            let (nr, ns) = (30, 128);
            (0..=nr)
                .map(|i| {
                    let rho = 1.15 * (i as f32 / nr as f32).powf(1.7);
                    (0..ns).map(|j| Vec2::from_angle(j as f32 / ns as f32 * TAU) * rho).collect()
                })
                .collect()
        };
        // the edge-on grid is laid out across the flat disc already
        let k = if self.kind == Kind::EdgeOn { 1.0 } else { self.k };
        let turn = Vec2::from_angle(self.rot);
        let closed = self.kind != Kind::EdgeOn;
        let n = rows[0].len() as u32;
        for row in &rows {
            for &p in row {
                let (e, a) = self.sample(p);
                let c = vc(e * gain.min(1.0), 1.0);
                g.v(turn.rotate(Vec2::new(p.x, p.y * k)) * self.rg, [c[0], c[1], c[2], (a * gain).clamp(0.0, 1.0)]);
            }
        }
        for row in 0..rows.len() as u32 - 1 {
            let (o, p) = (row * n, (row + 1) * n);
            for i in 0..if closed { n } else { n - 1 } {
                let j = (i + 1) % n;
                g.quad(o + i, p + i, p + j, o + j);
            }
        }
    }

    /// A spiral's wound arm segments.
    fn segments(&self) -> Vec<Segment> {
        let mut r = Stream::new(self.seed, "hseg");
        (0..SEGMENTS)
            .map(|i| {
                let r0 = 0.1 + 0.75 * r.next().powf(1.1);
                let main = i % 3 == 0;
                let b = H_WIND * r.range(0.8, 1.2);
                let ph = if main {
                    self.ph0 + if i % 4 == 0 { 0.0 } else { PI } + self.dir * (r0 / 0.08).ln() / H_WIND + r.gauss() * 0.2
                } else {
                    r.range(0.0, TAU)
                };
                let len = r.range(0.8, 2.4) * if main { 1.3 } else { 1.0 } * (1.0 - 0.3 * r0);
                let w = r.range(0.6, 1.1) * if main { 1.15 } else { 0.8 };
                Segment { r0, ph, b, len, w }
            })
            .collect()
    }

    /// The point `t` (0..1) along a segment at `rs` times its radius, and that radius.
    fn seg_at(&self, s: &Segment, t: f32, rs: f32) -> (Vec2, f32) {
        let th = t * s.len;
        let rr = s.r0 * (s.b * th).exp() * rs;
        (Vec2::from_angle(s.ph + self.dir * th) * rr, rr)
    }

    /// A spiral's dust: a thin filament just inside every other inner segment.
    fn filaments(&self, pen: &mut Pen, segs: &[Segment]) {
        for s in segs.iter().step_by(2).filter(|s| s.r0 <= 0.42) {
            let pts: Vec<(Vec2, f32)> = (0..18).map(|i| self.seg_at(s, i as f32 / 17.0 * 0.8, 0.955)).filter(|p| p.1 < 1.0).collect();
            if pts.len() <= 3 {
                continue;
            }
            let n = pts.len();
            let at: Vec<Vec2> = pts.iter().map(|p| p.0).collect();
            let half: Vec<f32> = pts.iter().map(|p| 0.006 + 0.008 * p.1).collect();
            let a: Vec<f32> = pts.iter().enumerate().map(|(i, p)| 0.75 * (PI * i as f32 / (n - 1) as f32).sin() * (1.0 - smooth(0.35, 0.7, p.1))).collect();
            pen.ribbon(&at, &half, H_DUST, &a);
        }
    }

    /// A spiral's glow: clumps along the segments, the haze, the knots and the core.
    fn arms(&self, pen: &mut Pen, r: &mut Stream, segs: &[Segment]) {
        let mut knots = Vec::new();
        for s in segs {
            let n = (s.len * s.r0 * 70.0 + 9.0).round() as u32;
            for _ in 0..n {
                let t = r.next();
                let (p, rr) = self.seg_at(s, t, 1.0 + r.gauss() * 0.03);
                if rr > 1.02 || r.next() > 1.0 - smooth(0.75, 1.05, rr + r.gauss() * 0.08) {
                    continue;
                }
                let warm = 1.0 - smooth(0.06, 0.22, rr);
                let base = if r.chance(0.35) {
                    H_HOT
                } else if r.chance(0.5) {
                    H_LAV
                } else {
                    H_BLUE
                };
                let c = mix(mix(base, rgb(255, 214, 200), warm), WHITE, if r.chance(0.08) { 0.4 } else { 0.0 });
                let size = r.range(0.018, 0.042) * (0.7 + 0.5 * rr);
                let a = r.range(0.085, 0.16) * s.w * (PI * (0.08 + 0.84 * t)).sin();
                pen.soft(p, size, c, a, 6);
                if r.chance(0.03) && rr > 0.12 {
                    knots.push(p);
                }
            }
        }
        for _ in 0..90 {
            let rr = (-(1.0 - r.next() * 0.97).ln() * 0.3).min(1.0);
            let at = Vec2::from_angle(r.range(0.0, TAU)) * rr;
            let size = r.range(0.02, 0.045);
            pen.soft(at, size, H_LAV, 0.035 * (1.0 - smooth(0.7, 1.0, rr)), 8);
        }
        for p in knots {
            let size = r.range(0.007, 0.013);
            pen.soft(p, size, H_PINK, 0.55, 8);
            pen.soft(p, 0.004, rgb(255, 200, 240), 0.6, 6);
        }
        // the core, less flattened than the disc
        let k = pen.k;
        pen.k = k.max(0.7);
        pen.soft(Vec2::ZERO, 0.05, rgb(255, 226, 214), 0.5, 16);
        pen.k = k;
    }

    /// The other kinds' splats: dark dust and clumps of blue light with pink cores (an irregular galaxy's round its
    /// clumps, an edge-on disc's along its plane; none in an elliptical).
    fn splats(&self, dust: &mut Pen, glow: &mut Pen, r: &mut Stream) {
        let (mut clumps, mut dark) = (Vec::new(), Vec::new());
        match self.kind {
            Kind::Irregular => {
                for c in &self.clumps {
                    for _ in 0..6 {
                        clumps.push(c.at + Vec2::new(r.gauss() * c.s * 0.55, r.gauss() * c.s * 0.5));
                    }
                    for _ in 0..4 {
                        dark.push(c.at + Vec2::new(r.gauss(), r.gauss()) * c.s * 0.5);
                    }
                }
            }
            Kind::EdgeOn => {
                for _ in 0..16 {
                    clumps.push(Vec2::new(r.range(-0.8, 0.8), r.gauss() * 0.025 / self.k));
                }
            }
            _ => {}
        }
        for p in dark {
            let size = r.range(0.03, 0.06);
            dust.soft(p, size, DUST, 0.2, 12);
        }
        for p in clumps {
            for _ in 0..3 {
                let at = p + Vec2::new(r.gauss(), r.gauss()) * 0.015;
                let size = r.range(0.025, 0.05);
                let c = mix(BLUE, BLUE2, r.next());
                glow.soft(at, size, c, 0.07, 12);
            }
            if r.chance(0.45) {
                let size = r.range(0.008, 0.016);
                glow.soft(p, size, mix(PINK, WHITE, 0.2), 0.4, 8);
            }
        }
    }

    /// `n` sparkles: along a spiral's arms, else scattered round the middle.
    fn sparkle(&self, pen: &mut Pen, r: &mut Stream, n: u32) {
        for i in 0..n {
            let p = if self.kind == Kind::Spiral {
                let r0 = 0.09;
                let th = r.range(0.3, 1.0) * (1.0f32 / r0).ln() / self.b;
                let rr = r0 * (self.b * th).exp() * (1.0 + r.gauss() * 0.05);
                Vec2::from_angle(self.dir * th + self.ph0 + (i % self.arms) as f32 * TAU / self.arms as f32) * rr
            } else {
                let x = r.gauss() * 0.4;
                Vec2::new(x, r.gauss() * if self.kind == Kind::EdgeOn { 0.04 / self.k } else { 0.3 })
            };
            pen.disc(p, 6, &[(0.0, WHITE, 0.8), (0.007, BLUE2, 0.0)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind meshes to finite numbers; a spiral to about 17 000 vertices.
    #[test]
    fn meshes() {
        let mut seen = Vec::new();
        for seed in 0..60u64 {
            let g = galaxy(&mut Stream::new(seed, "galaxy"), 0.85);
            let m = g.meshes(Vec2::ZERO, 0.85);
            assert!(m.field.finite() && m.dust.finite() && m.glow.finite());
            let n = m.field.len() + m.dust.len() + m.glow.len();
            if g.kind == Kind::Spiral {
                assert!((12_000..24_000).contains(&n), "spiral of {n} vertices");
            }
            if !seen.contains(&g.kind) {
                seen.push(g.kind);
            }
        }
        assert_eq!(seen.len(), 4, "every kind rolled in 60 skies: {seen:?}");
    }
}
