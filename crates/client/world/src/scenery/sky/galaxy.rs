//! The far galaxy (GDR-0105, GDR-0107), ported from the backdrop study (Niklas, 18 Oct 2026; version 8, then his
//! notes on it, the study's round 9): a spiral is variant H (M74's structure), an elliptical, edge-on or irregular
//! galaxy variant D (a soft field with splats). Built once per voyage in the design frame (px, y down), origin at the
//! galaxy's centre, as three static meshes drawn in this order:
//!
//! * **the field** ([`Meshes::field`]), premultiplied: one grid (polar, 31 × 128; an edge-on disc's 110 × 25 across
//!   it) whose every vertex samples the galaxy's light and dust; the dust dims what lies below, the light adds. A
//!   spiral's field carries most of its light: a broad disc whose arms are only modestly brighter than the gaps;
//!   its patchiness is the sky's isotropic [`Noise`] (the study's noise of crossed waves drew a grid through it);
//! * **the dust** ([`Meshes::dust`]), alpha-blended: a spiral's wide soft dark strands just inside its inner arm
//!   segments, the other kinds' patchy dark splats;
//! * **the glow** ([`Meshes::glow`]), additive: a spiral's arms as soft strands following its 38 short wound arm
//!   segments (a third on two main arms), built large to small: two broad strands a segment carry the form, three
//!   finer ones lie within it, a few soft knots on top, and a soft core; the other kinds' clumps of splats with pink
//!   cores and a few sparkles.
//!
//! Nothing is a thin hard strip, a dotted chain or a field of small round splats (Niklas, 18 Oct 2026: hard lines,
//! "too dotted", "rather filaments"): every strand is five vertices across, fading to nothing at its sides and its
//! ends. The colours are rolled per galaxy ([`Palette`]), nudged towards the sky's nebula.
//!
//! About 18 000 vertices for a spiral. Distances in the galaxy's own frame are shares of its radius, before it is
//! flattened by `k` and turned.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use super::paint::Geo;
use super::roll::{Noise, Rgb, Stream, WHITE, mix, smooth, vc};
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
const DUST: Rgb = rgb(28, 15, 11);
/// The arms' hues a galaxy's palette picks from, by weight ([`Palette`]): blue-white, lavender, violet, teal and a
/// warm gold-white.
const ARM_HUES: [(Rgb, u32); 5] =
    [(rgb(200, 216, 255), 3), (rgb(176, 150, 255), 3), (rgb(150, 128, 248), 2), (rgb(140, 214, 228), 3), (rgb(255, 232, 196), 2)];
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

/// A galaxy's colours, rolled per galaxy (Niklas, 18 Oct 2026: the reference's violet was only an example): a warm
/// core (cream to gold or pink), two arm hues from [`ARM_HUES`] nudged towards the sky's nebula and evened out in
/// brightness, knots pink-magenta to orange-red, dust brown to deep red.
struct Palette {
    core: Rgb,
    core2: Rgb,
    inner: Rgb,
    arm: Rgb,
    arm2: Rgb,
    hot: Rgb,
    haze: Rgb,
    knot: Rgb,
    dust: Rgb,
}

/// `c` scaled to the luminance `l` (no channel over 1).
fn even(c: Rgb, l: f32) -> Rgb {
    let c = c * (l / c.dot(Vec3::new(0.2126, 0.7152, 0.0722)).max(0.05));
    c / c.max_element().max(1.0)
}

impl Palette {
    fn roll(r: &mut Stream, tint: Rgb) -> Palette {
        let near = mix(tint, WHITE, 0.45);
        let weights = ARM_HUES.map(|h| h.1);
        let mut arm = || even(mix(ARM_HUES[r.weighted(&weights)].0, near, 0.18), 0.6);
        let (arm, arm2) = (arm(), arm());
        let warm = if r.chance(0.5) { rgb(255, 222, 170) } else { rgb(255, 206, 206) };
        let core = mix(rgb(255, 238, 222), warm, r.next());
        let knot = mix(rgb(255, 84, 196), rgb(255, 110, 80), r.next());
        let dust = mix(rgb(44, 18, 14), rgb(58, 10, 14), r.next());
        Palette {
            core,
            core2: mix(core, knot, 0.22),
            inner: mix(core, rgb(255, 214, 200), 0.5),
            arm,
            arm2,
            hot: mix(arm, WHITE, 0.5),
            haze: even(arm, 0.5),
            knot,
            dust,
        }
    }
}

/// The far galaxy: where (design frame), its radius, kind and shape, and the seed of its own streams.
pub(super) struct Galaxy {
    pub at: Vec2,
    pub rg: f32,
    pub kind: Kind,
    dir: f32,
    ph0: f32,
    k: f32,
    rot: f32,
    seed: u64,
    /// The field's noise, coarse and fine.
    n1: Noise,
    n2: Noise,
    clumps: Vec<Clump>,
    pal: Palette,
    strength: f32,
}

/// The three meshes of [`Galaxy`] (see the module's docs).
pub(super) struct Meshes {
    pub field: Geo,
    pub dust: Geo,
    pub glow: Geo,
}

/// Rolls the galaxy clear of the view's middle (the planets pass over it), its colours nudged towards `tint` (the
/// sky's nebula).
pub(super) fn galaxy(r: &mut Stream, strength: f32, tint: Rgb) -> Galaxy {
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
    let dir = if r.chance(0.5) { 1.0 } else { -1.0 };
    let ph0 = r.range(0.0, TAU);
    let k = match kind {
        Kind::EdgeOn => r.range(0.11, 0.15),
        Kind::Elliptical => r.range(0.55, 0.9),
        _ => r.range(0.45, 0.85),
    };
    let rot = r.range(0.0, PI);
    let seed = (r.next() as f64 * 4_294_967_296.0) as u64;
    // an irregular's patches are coarser
    let (f1, f2) = if kind == Kind::Irregular { (6.0, 10.0) } else { (14.0, 27.0) };
    let mut nr = Stream::new(seed, "noise");
    let (n1, n2) = (Noise::new(&mut nr, 3, f1, 0.5), Noise::new(&mut nr, 3, f2, 0.5));
    let clumps = if kind == Kind::Irregular {
        let mut c = Stream::new(seed, "irr");
        (0..6).map(|_| Clump { at: Vec2::new(c.gauss() * 0.3, c.gauss() * 0.22), s: c.range(0.12, 0.3), w: c.range(0.4, 1.0), blue: c.chance(0.75) }).collect()
    } else {
        Vec::new()
    };
    let pal = Palette::roll(&mut Stream::new(seed, "hpal"), tint);
    Galaxy { at: best.0, rg: rg * 1.15, kind, dir, ph0, k, rot, seed, n1, n2, clumps, pal, strength }
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

    /// A soft strand along `pts`: five vertices across (half width `half`, alpha falling 1, 0.55, 0 to the edges),
    /// each point's alpha `a`.
    fn strand(&mut self, pts: &[Vec2], half: &[f32], col: Rgb, a: &[f32]) {
        const ACROSS: [(f32, f32); 5] = [(-1.0, 0.0), (-0.5, 0.55), (0.0, 1.0), (0.5, 0.55), (1.0, 0.0)];
        let n = pts.len();
        let first = self.g.len() as u32;
        for (o, k) in ACROSS {
            for i in 0..n {
                let d = (pts[(i + 1).min(n - 1)] - pts[i.saturating_sub(1)]).normalize_or(Vec2::X);
                self.v(pts[i] + Vec2::new(-d.y, d.x) * half[i] * o, col, a[i] * k);
            }
        }
        for j in 0..4 {
            let (p, q) = (first + j * n as u32, first + (j + 1) * n as u32);
            for i in 0..n as u32 - 1 {
                self.g.quad(p + i, q + i, q + i + 1, p + i + 1);
            }
        }
    }
}

/// A strand's point: where, its radius and how far along the strand (0..1).
type StrandPt = (Vec2, f32, f32);

impl Galaxy {
    /// The galaxy's three meshes at the backdrop's `strength`.
    pub fn meshes(&self, white: Vec2, strength: f32) -> Meshes {
        let gain = self.strength * strength;
        let mut m = Meshes { field: Geo::new(white), dust: Geo::new(white), glow: Geo::new(white) };
        self.field(&mut m.field, gain);
        let mut glow = Stream::new(self.seed, if self.kind == Kind::Spiral { "galH" } else { "galD" });
        if self.kind == Kind::Spiral {
            let segs = self.segments();
            self.dust_strands(&mut self.pen(&mut m.dust, gain), &segs);
            self.arms(&mut self.pen(&mut m.glow, gain), &mut glow, &segs);
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
        let (n1, n2) = (self.n1.at(p.extend(0.0)).clamp(-1.0, 1.0), self.n2.at(p.extend(0.0)).clamp(-1.0, 1.0));
        let mut e = Vec3::ZERO;
        let a = match self.kind {
            Kind::Spiral => {
                let ph = y.atan2(x);
                let lr = (r.max(0.03) / 0.08).ln() / H_WIND;
                let s = (2.0 * (ph - self.ph0 - self.dir * lr)).cos();
                let edge = 1.0 - smooth(0.72, 1.08, r + 0.1 * n1);
                e += self.pal.core * 0.9 * (-r / 0.015).exp();
                e += self.pal.core2 * (0.7 * (-r / 0.06).exp() + 0.38 * (-r / 0.15).exp());
                e += self.pal.haze * (0.85 * (-r / 0.32).exp() + 0.07) * edge * (1.0 + 0.7 * ((1.0 + s) / 2.0 - 0.5)) * (0.8 + 0.3 * n1);
                0.14 * ((1.0 + (2.0 * (ph - self.ph0 - self.dir * (lr + 0.4))).cos()) / 2.0).powi(3)
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

    /// `n` points of a strand along segment `s` from `t0` to `t1`, at `rs0` times its radius wobbling by `amp`.
    fn strand_pts(&self, s: &Segment, r: &mut Stream, (t0, t1): (f32, f32), rs0: f32, amp: f32, n: usize) -> Vec<StrandPt> {
        let (f, ph) = (r.range(2.0, 5.0), r.range(0.0, TAU));
        (0..n)
            .map(|i| {
                let u = i as f32 / (n - 1) as f32;
                let (p, rr) = self.seg_at(s, t0 + (t1 - t0) * u, rs0 + amp * (u * f + ph).sin());
                (p, rr, u)
            })
            .collect()
    }

    /// A spiral's dust: wide soft dark strands just inside every other inner segment, faded at both ends (thin strips
    /// there read as hard lines; Niklas, 18 Oct 2026).
    fn dust_strands(&self, pen: &mut Pen, segs: &[Segment]) {
        let mut r = Stream::new(self.seed, "hdust");
        for s in segs.iter().step_by(2).filter(|s| s.r0 <= 0.42) {
            let q = self.strand_pts(s, &mut r, (0.0, 0.8), 0.94, 0.015, 16);
            let w = r.range(0.03, 0.045);
            let half: Vec<f32> = q.iter().map(|q| w * (0.6 + 0.6 * q.1) * (0.5 + 0.5 * (PI * q.2).sin())).collect();
            let a: Vec<f32> = q.iter().map(|q| 0.22 * (PI * q.2).sin().max(0.0).powf(1.5) * (1.0 - smooth(0.35, 0.7, q.1))).collect();
            pen.strand(&q.iter().map(|q| q.0).collect::<Vec<_>>(), &half, self.pal.dust, &a);
        }
    }

    /// A spiral's glow as strands following the spiral flow (Niklas, 18 Oct 2026: round splats read as smeared
    /// dots): along each segment two broad soft strands carry the arm's form, three finer ones lie within it, and a
    /// few soft knots on top; then the core.
    fn arms(&self, pen: &mut Pen, r: &mut Stream, segs: &[Segment]) {
        let env = |u: f32| (PI * u).sin().max(0.0).powf(0.8);
        let edge = |rr: f32| 1.0 - smooth(0.75, 1.05, rr);
        let mut knots = Vec::new();
        for s in segs {
            for k in 0..2 {
                let rs0 = 1.0 + if k > 0 { r.gauss() * 0.05 } else { 0.0 };
                let amp = r.range(0.01, 0.03);
                let q = self.strand_pts(s, r, (0.0, 1.0), rs0, amp, 16);
                let w = r.range(0.09, 0.14);
                let half: Vec<f32> = q.iter().map(|q| w * (0.6 + 0.6 * q.1) * (0.5 + 0.5 * (PI * q.2).sin())).collect();
                let a: Vec<f32> = q.iter().map(|q| 0.06 * s.w * env(q.2) * edge(q.1)).collect();
                pen.strand(&q.iter().map(|q| q.0).collect::<Vec<_>>(), &half, mix(self.pal.arm, self.pal.haze, 0.5), &a);
            }
            for _ in 0..3 {
                let t0 = r.range(0.0, 0.55);
                let t1 = t0 + r.range(0.3, 0.45);
                let rs0 = 1.0 + r.gauss() * 0.05;
                let amp = r.range(0.01, 0.025);
                let q = self.strand_pts(s, r, (t0, t1), rs0, amp, 12);
                let (w, aa) = (r.range(0.03, 0.045), r.range(0.06, 0.09));
                let warm = 1.0 - smooth(0.06, 0.22, q[6].1);
                let base = if r.chance(0.4) {
                    self.pal.hot
                } else if r.chance(0.5) {
                    self.pal.arm2
                } else {
                    self.pal.arm
                };
                let half: Vec<f32> = q.iter().map(|q| w * (0.6 + 0.6 * q.1) * (0.4 + 0.6 * (PI * q.2).sin())).collect();
                let a: Vec<f32> = q.iter().map(|q| aa * s.w * env(q.2) * edge(q.1)).collect();
                pen.strand(&q.iter().map(|q| q.0).collect::<Vec<_>>(), &half, mix(base, self.pal.inner, warm), &a);
                if r.chance(0.15) && q[6].1 > 0.12 && q[6].1 < 0.95 {
                    knots.push(q[6].0);
                }
            }
        }
        for p in knots {
            let size = r.range(0.018, 0.028);
            pen.soft(p, size, self.pal.knot, 0.2, 10);
        }
        // the core, less flattened than the disc
        let k = pen.k;
        pen.k = k.max(0.7);
        pen.soft(Vec2::ZERO, 0.05, mix(self.pal.core, WHITE, 0.3), 0.5, 16);
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
                let c = mix(self.pal.arm, self.pal.hot, r.next());
                glow.soft(at, size, c, 0.07, 12);
            }
            if r.chance(0.45) {
                let size = r.range(0.008, 0.016);
                glow.soft(p, size, mix(self.pal.knot, WHITE, 0.2), 0.4, 8);
            }
        }
    }

    /// `n` sparkles scattered round the middle (an elliptical, edge-on or irregular galaxy; a spiral has none).
    fn sparkle(&self, pen: &mut Pen, r: &mut Stream, n: u32) {
        for _ in 0..n {
            let x = r.gauss() * 0.4;
            let p = Vec2::new(x, r.gauss() * if self.kind == Kind::EdgeOn { 0.04 / self.k } else { 0.3 });
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
            let g = galaxy(&mut Stream::new(seed, "galaxy"), 0.85, Vec3::new(0.12, 0.5, 0.55));
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
