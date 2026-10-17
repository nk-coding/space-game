//! The sky's dice and small tools: named random streams (one per layer, so a change to one layer leaves the others
//! as they were), wobbly outlines, a smooth noise on the sphere, and colours in sRGB.

use std::f32::consts::TAU;

use bevy::prelude::*;

/// A random stream: mulberry32 seeded from the sky's seed and a name (FNV-1a), the design study's generator.
pub(super) struct Stream(u32);

impl Stream {
    pub fn new(seed: u64, name: &str) -> Self {
        let mut h: u32 = 2_166_136_261;
        for b in name.bytes() {
            h = (h ^ u32::from(b)).wrapping_mul(16_777_619);
        }
        let mut s = Stream((seed ^ (seed >> 32)) as u32 ^ h);
        s.next();
        s.next();
        s
    }

    /// In `0..1`.
    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }

    /// An integer in `a..=b`.
    pub fn int(&mut self, a: i32, b: i32) -> i32 {
        (a + ((b - a + 1) as f32 * self.next()) as i32).min(b)
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.next() < p
    }

    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[((self.next() * xs.len() as f32) as usize).min(xs.len() - 1)]
    }

    /// A standard normal number.
    pub fn gauss(&mut self) -> f32 {
        let u = self.next().max(1e-6);
        (-2.0 * u.ln()).sqrt() * (TAU * self.next()).cos()
    }

    /// An index into `weights` by weight.
    pub fn weighted(&mut self, weights: &[u32]) -> usize {
        let total: u32 = weights.iter().sum();
        let mut x = self.next() * total as f32;
        for (i, w) in weights.iter().enumerate() {
            x -= *w as f32;
            if x <= 0.0 {
                return i;
            }
        }
        0
    }

    /// A random direction on the side of the sphere facing the view, within `lat` (sine) and `lon` (radians).
    pub fn facing(&mut self, lat: f32, lon: f32) -> Vec3 {
        let la = self.range(-lat, lat).asin();
        let lo = self.range(-lon, lon);
        Vec3::new(la.cos() * lo.sin(), la.sin(), la.cos() * lo.cos())
    }
}

/// A closed wobbly outline: a radius factor as a sum of sine harmonics (periodic).
#[derive(Clone)]
pub(super) struct Wob {
    h: Vec<(f32, f32, f32)>,
    pub max: f32,
}

impl Wob {
    pub fn new(r: &mut Stream, amp: f32, n: u32, exp: f32) -> Self {
        let mut h = Vec::new();
        let mut max = 1.0;
        for k in 2..2 + n {
            let a = amp * r.range(0.35, 1.0) / ((k - 1) as f32).powf(exp);
            h.push((k as f32, r.range(0.0, TAU), a));
            max += a;
        }
        Wob { h, max }
    }

    pub fn at(&self, t: f32) -> f32 {
        1.0 + self.h.iter().map(|(k, p, a)| a * (k * t + p).sin()).sum::<f32>()
    }
}

/// A smooth noise on the sphere: plane waves of rising frequency in random directions, `-1..1` roughly.
pub(super) struct Noise(Vec<(Vec3, f32, f32)>);

impl Noise {
    /// `octaves` waves from frequency `f0`, each 1.7 times the last and its amplitude `fall` times.
    pub fn new(r: &mut Stream, octaves: u32, f0: f32, fall: f32) -> Self {
        let (mut f, mut a, mut sum) = (f0, 1.0, 0.0);
        let mut w = Vec::new();
        for _ in 0..octaves {
            for _ in 0..2 {
                let d = Vec3::new(r.gauss(), r.gauss(), r.gauss()).normalize_or(Vec3::X);
                w.push((d * f * r.range(0.8, 1.2), r.range(0.0, TAU), a));
                sum += a;
            }
            f *= 1.7;
            a *= fall;
        }
        Noise(w.into_iter().map(|(d, p, a)| (d, p, a / sum * 1.6)).collect())
    }

    pub fn at(&self, p: Vec3) -> f32 {
        self.0.iter().map(|(k, ph, a)| a * (k.dot(p) + ph).sin()).sum()
    }
}

/// An sRGB colour, components `0..1`.
pub(super) type Rgb = Vec3;

pub(super) fn hex(s: &str) -> Rgb {
    let [r, g, b] = data::hex_rgb(s).unwrap_or([255, 0, 255]);
    Vec3::new(r as f32, g as f32, b as f32) / 255.0
}

pub(super) const WHITE: Rgb = Vec3::ONE;

pub(super) fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    a + (b - a) * t
}

/// A colour shifted a little in hue, saturation and lightness (degrees, shares), so no two bodies match.
pub(super) fn jit(r: &mut Stream, c: Rgb, dh: f32, ds: f32, dl: f32) -> Rgb {
    let h = Hsla::from(Srgba::new(c.x, c.y, c.z, 1.0));
    let h = Hsla::new(
        (h.hue + r.range(-dh, dh)).rem_euclid(360.0),
        (h.saturation + r.range(-ds, ds)).clamp(0.0, 1.0),
        (h.lightness + r.range(-dl, dl)).clamp(0.0, 1.0),
        1.0,
    );
    let s = Srgba::from(h);
    Vec3::new(s.red, s.green, s.blue)
}

/// A vertex colour (linear, as the atlas material wants) from an sRGB colour and an alpha.
pub(super) fn vc(c: Rgb, a: f32) -> [f32; 4] {
    let l = Color::srgb(c.x.clamp(0.0, 1.0), c.y.clamp(0.0, 1.0), c.z.clamp(0.0, 1.0)).to_linear();
    [l.red, l.green, l.blue, a.clamp(0.0, 1.0)]
}

pub(super) fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Two unit vectors at right angles to `c` (a basis of its tangent plane).
pub(super) fn basis(c: Vec3) -> (Vec3, Vec3) {
    let a = if c.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let u = c.cross(a).normalize();
    (u, c.cross(u))
}
