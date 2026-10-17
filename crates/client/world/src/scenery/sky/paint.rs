//! Painting a planet or moon into vertex-coloured meshes (GDR-0105).
//!
//! The disc is a polar mesh of evenly spaced vertices; each vertex's colour is worked out on the CPU like a shader
//! would: the surface at that point of the sphere (bands, storms, seas and land, patches, ice caps), the clouds
//! blended over it as a smooth density (soft blobs, wispy streaks, storm swirls), the air's tint at the limb, then
//! the light (a soft terminator, the limb darkened, the night side in the sky's dark). The GPU interpolates between
//! the vertices, so every edge is a gradient. What must stay crisp is drawn over the disc as thin geometry, lit the
//! same way: craters, cracks, the turbulent streaks between bands; glowing things (lava rifts, vents, a garden's
//! lights, plumes) are not darkened at night. The air's glow is a strip round the limb fading outwards, the rings
//! two halves of feathered ellipses, one behind the disc and one in front.
//!
//! Everything is in the design frame (px of a 1600 × 900 view, y down like the design study), centred on the body;
//! `mod.rs` flips and scales it.

use std::f32::consts::{PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::roll::{Noise, Rgb, WHITE, Wob, basis, mix, smooth, vc};

/// Vertex-coloured triangles with a texture point each (the atlas's white texel, or a soft dot's corners).
#[derive(Default)]
pub(super) struct Geo {
    pos: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
    /// The white texel (every flat colour samples it).
    pub white: Vec2,
}

impl Geo {
    pub fn new(white: Vec2) -> Self {
        Geo { white, ..default() }
    }

    pub fn len(&self) -> usize {
        self.pos.len()
    }

    /// Every position and colour a number.
    #[cfg(test)]
    pub fn finite(&self) -> bool {
        self.pos.iter().flatten().chain(self.col.iter().flatten()).all(|x| x.is_finite())
    }

    /// A vertex at `p` (design px, y down: stored y up).
    pub fn v(&mut self, p: Vec2, c: [f32; 4]) -> u32 {
        self.vuv(p, self.white, c)
    }

    pub fn vuv(&mut self, p: Vec2, uv: Vec2, c: [f32; 4]) -> u32 {
        let i = self.pos.len() as u32;
        self.pos.push([p.x, -p.y, 0.0]);
        self.uv.push([uv.x, uv.y]);
        self.col.push(c);
        i
    }

    pub fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.idx.extend_from_slice(&[a, b, c]);
    }

    pub fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.idx.extend_from_slice(&[a, b, c, a, c, d]);
    }

    /// A textured quad (a soft dot) centred at `at`, `size` across.
    pub fn sprite(&mut self, at: Vec2, size: Vec2, uv: [Vec2; 4], c: [f32; 4]) {
        let h = size / 2.0;
        let a = self.vuv(at + Vec2::new(-h.x, -h.y), uv[0], c);
        let b = self.vuv(at + Vec2::new(h.x, -h.y), uv[1], c);
        let cc = self.vuv(at + Vec2::new(h.x, h.y), uv[2], c);
        let d = self.vuv(at + Vec2::new(-h.x, h.y), uv[3], c);
        self.quad(a, b, cc, d);
    }

    /// A soft-edged strip along `pts`: full colour `c` (alpha `a`) across `half` either side of the line, fading to
    /// nothing over `feather` further out. `cols` gives each point its colour.
    pub fn soft_strip(&mut self, pts: &[Vec2], cols: &[Rgb], a: &[f32], half: f32, feather: f32) {
        let n = pts.len();
        if n < 2 {
            return;
        }
        let first = self.pos.len() as u32;
        for i in 0..n {
            let d0 = if i > 0 { (pts[i] - pts[i - 1]).normalize_or_zero() } else { Vec2::ZERO };
            let d1 = if i + 1 < n { (pts[i + 1] - pts[i]).normalize_or_zero() } else { Vec2::ZERO };
            let d = (d0 + d1).normalize_or(Vec2::X);
            let nrm = Vec2::new(-d.y, d.x);
            let k = if i > 0 && i + 1 < n { nrm.dot(Vec2::new(-d1.y, d1.x)).max(0.5) } else { 1.0 };
            for (off, alpha) in [(half + feather, 0.0), (half, 1.0), (-half, 1.0), (-half - feather, 0.0)] {
                self.v(pts[i] + nrm * off / k, vc(cols[i], a[i] * alpha));
            }
        }
        for s in 0..n as u32 - 1 {
            let (p, q) = (first + 4 * s, first + 4 * (s + 1));
            for r in 0..3 {
                self.quad(p + r, q + r, q + r + 1, p + r + 1);
            }
        }
    }

    /// A soft disc (a fan): colour `c` at alpha `a` in the middle, fading to nothing at radius `r`.
    pub fn glow(&mut self, at: Vec2, r: f32, c: Rgb, a: f32, segs: u32) {
        let mid = self.v(at, vc(c, a));
        let first = self.pos.len() as u32;
        for k in 0..segs {
            let t = k as f32 / segs as f32 * TAU;
            self.v(at + Vec2::from_angle(t) * r, vc(c, 0.0));
        }
        for k in 0..segs {
            self.tri(mid, first + k, first + (k + 1) % segs);
        }
    }

    pub fn into_mesh(self) -> Mesh {
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uv);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        m.insert_indices(Indices::U32(self.idx));
        m
    }
}

/// The air: how much (0 none, 1 thick), its colour and how much haze it lays over the surface.
pub(super) struct Air {
    pub lv: f32,
    pub col: Rgb,
    pub haze: f32,
}

/// Latitude bands with wavy soft boundaries (feature frame, y down), and fine ripples along them.
pub(super) struct Bands {
    pub bounds: Vec<Wave>,
    pub cols: Vec<Rgb>,
    /// Half the width a boundary blends over (share of the radius).
    pub soft: f32,
    /// Fine ripples: (frequency, phase, amplitude, wobble frequency, wobble amplitude, wobble phase).
    pub ripples: Vec<[f32; 6]>,
}

/// A wavy line across the disc: `y + amp · (sin(fr x + ph) + ½ sin(2.3 fr x + ph2))`.
#[derive(Clone, Copy)]
pub(super) struct Wave {
    pub y: f32,
    pub amp: f32,
    pub fr: f32,
    pub ph: f32,
    pub ph2: f32,
}

impl Wave {
    pub fn at(&self, x: f32) -> f32 {
        self.y + self.amp * ((self.fr * x + self.ph).sin() + 0.5 * (2.3 * self.fr * x + self.ph2).sin())
    }
}

/// A streak between bands (or a dune ridge): a thin soft strip from `wave` down by `h`.
pub(super) struct Streak {
    pub wave: Wave,
    pub h: f32,
    pub col: Rgb,
    pub a: f32,
}

/// An oval storm on the disc (feature frame): centre, radii (shares of the radius), colour, with a pale halo.
pub(super) struct Storm {
    pub at: Vec2,
    pub r: Vec2,
    pub col: Rgb,
    pub halo: bool,
}

/// A soft cap on the sphere round `c`: layers of colour out to `rho × scale × wobble` (radians), soft over `soft`,
/// the edge roughened by the body's noise.
pub(super) struct Cap {
    pub c: Vec3,
    pub rho: f32,
    pub f: Wob,
    pub layers: Vec<(f32, Rgb, f32)>,
    pub soft: f32,
    pub rough: f32,
}

/// Ice at the poles: how far down they reach (share of the radius from the centre), colour, wobble.
pub(super) struct Poles {
    pub n: f32,
    pub s: f32,
    pub col: Rgb,
    pub f: Wob,
}

/// The clouds: soft blobs (caps with their alpha), wispy streaks along latitudes and storm swirls.
pub(super) struct Clouds {
    pub col: Rgb,
    pub blobs: Vec<(Vec3, f32, Wob, f32)>,
    pub streaks: Vec<(Wave, f32, f32)>,
    pub swirls: Vec<Swirl>,
    pub wisp: Noise,
}

/// A storm spiral in the clouds: centre, angular size, turns of its two arms, which way, alpha.
pub(super) struct Swirl {
    pub c: Vec3,
    pub size: f32,
    pub turns: f32,
    pub dir: f32,
    pub a: f32,
}

/// A crater: where (feature frame) and how big (share of the radius).
pub(super) struct Crater {
    pub at: Vec3,
    pub s: f32,
}

/// A line over the surface (a crack, a rift): points on the sphere (feature frame).
pub(super) type Walk = Vec<Vec3>;

/// A plume off the limb: angle (design frame), length and width (shares of the radius).
pub(super) struct Plume {
    pub a: f32,
    pub len: f32,
    pub w: f32,
}

/// A faceted body: corners (angle, radius share), an off-centre core, a shade per facet, light and dark colours.
pub(super) struct Facets {
    pub pts: Vec<(f32, f32)>,
    pub core: Vec2,
    pub shades: Vec<f32>,
    pub cols: (Rgb, Rgb),
}

/// A ring system: its tilt on screen, how flat (minor/major), its bands (inner, outer radius as shares of the
/// body's, colour, alpha).
pub(super) struct Ring {
    pub rot: f32,
    pub k: f32,
    pub subs: Vec<(f32, f32, Rgb, f32)>,
}

/// How a body looks: everything its form rolled.
pub(super) struct Look {
    pub base: Rgb,
    pub bands: Option<Bands>,
    pub streaks: Vec<Streak>,
    pub storms: Vec<Storm>,
    pub caps: Vec<Cap>,
    pub poles: Option<Poles>,
    pub clouds: Option<Clouds>,
    pub craters: Vec<Crater>,
    pub cracks: Vec<(Walk, Rgb, f32)>,
    pub rifts: Vec<Walk>,
    pub rift_cols: (Rgb, Rgb),
    pub vents: Vec<(Vec3, f32)>,
    pub biolum: Option<(Rgb, Vec<Vec3>)>,
    pub night_glow: Option<Rgb>,
    pub plumes: Vec<Plume>,
    pub facets: Option<Facets>,
    pub shape: Option<Wob>,
    /// How much the ground's brightness is mottled by fine noise.
    pub mottle: f32,
    pub air: Air,
    pub ring: Option<Ring>,
    pub noise: Noise,
}

/// A body in the sky: radius and centre (design px, y down), where its light comes from (angle on screen, y down)
/// and how far round (phase: 0 full, π/2 half, beyond: backlit), its tilt, and its look.
pub(super) struct Body {
    pub r: f32,
    pub at: Vec2,
    pub light: f32,
    pub phase: f32,
    pub tilt: f32,
    pub moon: bool,
    pub look: Look,
}

/// What painting needs besides the body: the sky's night colour, the veil colour and how much of it lies over
/// everything (the backdrop's strength).
pub(super) struct Paint {
    pub night: Rgb,
    pub veil: Rgb,
    pub veil_share: f32,
}

impl Paint {
    /// A body colour behind the veil.
    fn c(&self, c: Rgb, a: f32) -> [f32; 4] {
        vc(mix(c, self.veil, self.veil_share), a)
    }
}

impl Body {
    /// The point of the sphere under design-frame offset `p` (y down) from the centre (unit radius); `None` off the
    /// disc.
    fn sphere(&self, p: Vec2) -> Vec3 {
        let q = p / self.r;
        Vec3::new(q.x, q.y, (1.0 - q.length_squared()).max(0.0).sqrt())
    }

    /// The sphere point in the surface's own frame (turned by the tilt).
    fn feature(&self, s: Vec3) -> Vec3 {
        let (sn, cs) = (-self.tilt).sin_cos();
        Vec3::new(s.x * cs - s.y * sn, s.x * sn + s.y * cs, s.z)
    }

    /// Back from the surface's frame to the design frame (unit sphere).
    fn unfeature(&self, f: Vec3) -> Vec3 {
        let (sn, cs) = self.tilt.sin_cos();
        Vec3::new(f.x * cs - f.y * sn, f.x * sn + f.y * cs, f.z)
    }

    /// Where the light comes from (unit, design frame).
    fn sun(&self) -> Vec3 {
        Vec3::new(self.light.cos() * self.phase.sin(), self.light.sin() * self.phase.sin(), self.phase.cos())
    }

    /// How lit a sphere point is: 1 day, 0 night, soft across the terminator (softer with more air).
    fn lit(&self, s: Vec3) -> f32 {
        let lv = self.look.air.lv;
        smooth(-0.1 - 0.22 * lv, 0.1 + 0.12 * lv, s.dot(self.sun()))
    }

    /// The light on a surface colour `c` at sphere point `s` (design frame).
    fn shade(&self, s: Vec3, c: Rgb, night: Rgb) -> Rgb {
        let lv = self.look.air.lv;
        let sun = self.sun();
        let d = s.dot(sun);
        let r = s.truncate().length();
        let mut day = c * (0.8 + 0.3 * d.max(0.0));
        // a soft highlight towards the light
        let hl = s.truncate() - sun.truncate().normalize_or_zero() * 0.35;
        day += WHITE * 0.07 * (1.0 - hl.length() / 1.1).max(0.0);
        day = mix(day, night, 0.45 * (1.0 - 0.4 * lv) * ((r - 0.5) / 0.5).clamp(0.0, 1.0));
        let mut out = mix(mix(night, c, 0.07), day, self.lit(s));
        if let Some(g) = self.look.night_glow {
            out += g * (1.0 - self.lit(s)) * (0.06 + 0.2 * r * r);
        }
        out
    }

    /// The surface colour at feature-frame point `f` before the light: ground, then clouds, then the air's tint.
    fn surface(&self, f: Vec3) -> Rgb {
        let l = &self.look;
        let mut c = l.base;
        if let Some(b) = &l.bands {
            c = b.cols[0];
            for (i, w) in b.bounds.iter().enumerate() {
                c = mix(c, b.cols[i + 1], smooth(-b.soft, b.soft, f.y - w.at(f.x)));
            }
            let rip: f32 = b.ripples.iter().map(|[fr, ph, a, wf, wa, wp]| a * (fr * (f.y + wa * (wf * f.x + wp).sin()) + ph).sin()).sum();
            c *= 1.0 + rip;
        }
        for s in &l.storms {
            let fz = (1.0 - s.at.length_squared()).max(0.05).sqrt() / (1.0 - s.at.y * s.at.y).max(0.05).sqrt();
            let d = Vec2::new((f.x - s.at.x) / (s.r.x * fz), (f.y - s.at.y) / s.r.y);
            let rho = d.length();
            if s.halo {
                c = mix(c, mix(s.col, WHITE, 0.55), 0.28 * (1.0 - smooth(1.0, 1.3, rho)));
            }
            let swirl = 1.0 + 0.12 * (3.0 * d.y.atan2(d.x) - 7.0 * rho).sin() * smooth(0.0, 0.3, rho);
            let core = mix(mix(s.col, WHITE, 0.25 * (1.0 - rho).max(0.0)), s.col, smooth(0.0, 0.6, rho));
            c = mix(c, core * swirl, 0.95 * (1.0 - smooth(0.5, 1.0, rho)));
        }
        let n = l.noise.at(f * 2.0);
        if l.mottle > 0.0 {
            c *= 1.0 + l.mottle * l.noise.at(f * 7.0 + Vec3::splat(3.0));
        }
        for cap in &l.caps {
            let cos = f.dot(cap.c);
            let reach = cap.rho * cap.f.max * cap.layers.iter().fold(0.0f32, |m, x| m.max(x.0)) + cap.soft + cap.rough;
            if cos < reach.min(PI).cos() {
                continue;
            }
            let d = cos.clamp(-1.0, 1.0).acos();
            let (u, v) = basis(cap.c);
            let t = f.dot(v).atan2(f.dot(u));
            let edge = cap.rho * cap.f.at(t) + cap.rough * n;
            for (scale, col, a) in &cap.layers {
                c = mix(c, *col, a * (1.0 - smooth(edge * scale - cap.soft, edge * scale + cap.soft, d)));
            }
        }
        if let Some(p) = &l.poles {
            let w = 0.03 * p.f.at(f.x * PI) - 0.03 + 0.02 * n;
            let north = -(p.n + w);
            let south = p.s + w;
            for (grow, a) in [(0.035, 0.35), (0.0, 0.85)] {
                let cov = smooth(north + grow + 0.012, north + grow - 0.012, f.y).max(smooth(south - grow - 0.012, south - grow + 0.012, f.y));
                c = mix(c, p.col, a * cov);
            }
        }
        if let Some(cl) = &l.clouds {
            c = mix(c, cl.col, cloud_density(cl, f));
        }
        let r = f.truncate().length();
        let air = &l.air;
        if air.haze > 0.0 {
            c = mix(c, air.col, air.haze);
        }
        if air.lv > 0.0 {
            c = mix(c, air.col, (0.1 + 0.35 * air.lv) * smooth(0.75 - 0.25 * air.lv, 1.0, r));
        }
        c
    }
}

/// How thick the clouds are at feature-frame point `f` (0 clear, up to 1).
fn cloud_density(cl: &Clouds, f: Vec3) -> f32 {
    let wisp = cl.wisp.at(f * 3.0);
    let mut clear = 1.0;
    for (c, rho, w, a) in &cl.blobs {
        let cos = f.dot(*c);
        if cos < (rho * w.max * 1.6).min(PI).cos() {
            continue;
        }
        let d = cos.clamp(-1.0, 1.0).acos();
        let (u, v) = basis(*c);
        let edge = rho * w.at(f.dot(v).atan2(f.dot(u))) * (1.0 + 0.35 * wisp);
        let cov = 1.0 - smooth(edge * 0.25, edge * 1.25, d);
        clear *= 1.0 - a * cov * (0.75 + 0.25 * wisp);
    }
    for (w, h, a) in &cl.streaks {
        let y = (f.y - w.at(f.x)) / h;
        // streaked along the latitude: the noise stretched across it, so threads come and go along it
        let threads = 0.5 + 0.5 * cl.wisp.at(Vec3::new(f.x * 1.2, f.y * 9.0, f.z * 1.2));
        clear *= 1.0 - a * (-y * y).exp() * threads;
    }
    for s in &cl.swirls {
        let cos = f.dot(s.c);
        if cos < (s.size * 1.6).cos() {
            continue;
        }
        let (u, v) = basis(s.c);
        let p = Vec2::new(f.dot(u), f.dot(v));
        let r = p.length() / s.size.sin();
        // two logarithmic arms wound round a dense core with a small clear eye
        let phase = p.y.atan2(p.x) + s.dir * s.turns * r.max(0.03).ln();
        let arms = (0.5 + 0.5 * (2.0 * phase + wisp).cos()).powf(1.5);
        let env = (-(r / 0.75).powi(2)).exp();
        let core = (-(r / 0.22).powi(2)).exp();
        let eye = smooth(0.03, 0.09, r);
        clear *= 1.0 - s.a * ((env * (0.2 + 0.8 * arms) + 0.7 * core) * eye).min(1.0);
    }
    (1.0 - clear).clamp(0.0, 1.0)
}

/// The body's meshes: the body itself (air glow, disc, what lies on it), and the back and front halves of its rings.
pub(super) struct Painted {
    pub body: Geo,
    pub ring_back: Option<Geo>,
    pub ring_front: Option<Geo>,
}

/// How finely the disc is meshed: rings of vertices from the centre to the limb.
fn rings_for(b: &Body) -> u32 {
    if b.moon { (b.r / 2.5).clamp(10.0, 26.0) as u32 } else { (b.r / 4.0).clamp(40.0, 84.0) as u32 }
}

pub(super) fn paint(b: &Body, p: &Paint, white: Vec2) -> Painted {
    let mut g = Geo::new(white);
    let night = p.night;
    if b.look.air.lv > 0.0 {
        air_glow(&mut g, b, p);
    }
    if let Some(f) = &b.look.facets {
        facets(&mut g, b, f, p);
    } else {
        disc(&mut g, b, p, night);
        overlays(&mut g, b, p, night);
    }
    plumes(&mut g, b, p);
    let ring = b.look.ring.as_ref().map(|r| (ring_half(b, r, p, white, true), ring_half(b, r, p, white, false)));
    let (ring_back, ring_front) = ring.unzip();
    Painted { body: g, ring_back, ring_front }
}

/// The disc: evenly spaced vertices in rings, each coloured by the surface and the light; an outer feather of a
/// pixel so the limb is smooth.
fn disc(g: &mut Geo, b: &Body, p: &Paint, night: Rgb) {
    let n = rings_for(b);
    let step = b.r / n as f32;
    let colour = |pt: Vec2| {
        let s = b.sphere(pt);
        p.c(b.shade(s, b.surface(b.feature(s)), night), 1.0)
    };
    let shape = |t: f32| b.look.shape.as_ref().map_or(1.0, |w| w.at(t) / w.max);
    let centre = g.v(Vec2::ZERO, colour(Vec2::ZERO));
    let mut prev: Vec<u32> = vec![centre];
    for k in 1..=n {
        let rad = k as f32 * step;
        let m = ((TAU * rad / step).round() as u32).max(6);
        let ring: Vec<u32> = (0..m)
            .map(|i| {
                let t = i as f32 / m as f32 * TAU;
                let dir = Vec2::from_angle(t);
                let at = dir * rad;
                let c = colour(at * if k == n { 0.999 } else { 1.0 });
                g.v(at * shape(t), c)
            })
            .collect();
        stitch(g, &prev, &ring);
        prev = ring;
    }
    // the feather: the limb's colours fading out over about a pixel
    let m = prev.len();
    let outer: Vec<u32> = (0..m)
        .map(|i| {
            let t = i as f32 / m as f32 * TAU;
            let at = Vec2::from_angle(t) * (b.r * shape(t) + 1.1);
            let c = g.col[prev[i] as usize];
            g.v(at, [c[0], c[1], c[2], 0.0])
        })
        .collect();
    stitch(g, &prev, &outer);
}

/// Triangles between two closed rings of vertices (angles evenly spread, both starting at angle 0).
fn stitch(g: &mut Geo, a: &[u32], b: &[u32]) {
    if a.len() == 1 {
        for i in 0..b.len() {
            g.tri(a[0], b[i], b[(i + 1) % b.len()]);
        }
        return;
    }
    let (na, nb) = (a.len(), b.len());
    let (mut i, mut j) = (0usize, 0usize);
    while i < na || j < nb {
        let fa = (i + 1) as f32 / na as f32;
        let fb = (j + 1) as f32 / nb as f32;
        if j >= nb || (i < na && fa < fb) {
            g.tri(a[i % na], a[(i + 1) % na], b[j % nb]);
            i += 1;
        } else {
            g.tri(a[i % na], b[(j + 1) % nb], b[j % nb]);
            j += 1;
        }
    }
}

/// A point of the surface (feature frame) on screen (design px from the centre), and whether it faces the view.
fn project(b: &Body, f: Vec3) -> (Vec2, Vec3) {
    let s = b.unfeature(f);
    (s.truncate() * b.r, s)
}

/// What lies on the disc: streaks, craters, cracks (lit), rifts, vents and a garden's lights (glowing).
fn overlays(g: &mut Geo, b: &Body, p: &Paint, night: Rgb) {
    let l = &b.look;
    // streaks between bands: soft thin strips across the disc, lit like the surface
    for s in &l.streaks {
        let n = 48;
        let mut pts = Vec::new();
        let (mut cols, mut alpha) = (Vec::new(), Vec::new());
        for i in 0..=n {
            let x = -1.0 + 2.0 * i as f32 / n as f32;
            let y = s.wave.at(x) + s.h / 2.0;
            let f = Vec3::new(x, y, 0.0);
            let rr = f.truncate().length_squared();
            let inside = rr < 0.985;
            let f = Vec3::new(x, y, (1.0 - rr).max(0.0).sqrt());
            let f = if inside { f } else { (f.truncate().normalize_or_zero() * 0.99).extend(0.14) };
            let (at, sp) = project(b, f);
            pts.push(at);
            cols.push(mix(b.shade(sp, s.col, night), p.veil, p.veil_share));
            alpha.push(if inside { s.a * smooth(0.0, 0.25, f.z) } else { 0.0 });
        }
        g.soft_strip(&pts, &cols, &alpha, (s.h / 2.0 * b.r).max(0.4), (0.6 * s.h * b.r).max(0.8));
    }
    // craters: a dark floor, a rim lit on the side facing the light, foreshortened towards the limb
    let sun2 = b.sun().truncate().normalize_or(Vec2::X);
    for c in &l.craters {
        let (at, sp) = project(b, c.at);
        if sp.z < 0.06 {
            continue;
        }
        let s = (c.s * b.r).max(0.9);
        let radial = at.normalize_or(Vec2::X);
        let tang = Vec2::new(-radial.y, radial.x);
        let segs = if s > 6.0 { 20 } else { 10 };
        let floor = mix(l.base, Vec3::ZERO, 0.3);
        let rim_lit = mix(l.base, WHITE, 0.28);
        let rim_dark = mix(l.base, Vec3::ZERO, 0.15);
        let fc = mix(b.shade(sp, floor, night), p.veil, p.veil_share);
        let mid = g.v(at + sun2 * s * 0.2 * sp.z, vc(fc, 0.7));
        let first = g.len() as u32;
        for k in 0..segs {
            let t = k as f32 / segs as f32 * TAU;
            let o = radial * t.cos() * s * sp.z + tang * t.sin() * s;
            let facing = -o.normalize_or_zero().dot(sun2);
            let rim = mix(rim_dark, rim_lit, 0.5 + 0.5 * facing);
            let rc = mix(b.shade(sp, rim, night), p.veil, p.veil_share);
            g.v(at + o * 0.75, vc(fc, 0.65));
            g.v(at + o, vc(rc, 0.6));
            g.v(at + o * 1.25, vc(rc, 0.0));
        }
        for k in 0..segs {
            let a = first + 3 * k;
            let bb = first + 3 * ((k + 1) % segs);
            g.tri(mid, a, bb);
            g.quad(a, bb, bb + 1, a + 1);
            g.quad(a + 1, bb + 1, bb + 2, a + 2);
        }
    }
    // cracks (lit) and rifts (glowing), broken where they pass round the limb
    let lines = |g: &mut Geo, w: &Walk, col: Rgb, a: f32, half: f32, feather: f32, glow: bool| {
        let mut run: Vec<(Vec2, Vec3)> = Vec::new();
        let flush = |g: &mut Geo, run: &mut Vec<(Vec2, Vec3)>| {
            if run.len() > 1 {
                let pts: Vec<Vec2> = run.iter().map(|x| x.0).collect();
                let cols: Vec<Rgb> = run.iter().map(|x| if glow { col } else { mix(b.shade(x.1, col, night), p.veil, p.veil_share) }).collect();
                let alpha: Vec<f32> = run.iter().map(|x| a * smooth(0.02, 0.2, x.1.z)).collect();
                g.soft_strip(&pts, &cols, &alpha, half, feather);
            }
            run.clear();
        };
        for f in w {
            let (at, sp) = project(b, *f);
            if sp.z < 0.02 {
                flush(g, &mut run);
            } else {
                run.push((at, sp));
            }
        }
        flush(g, &mut run);
    };
    for (w, col, width) in &l.cracks {
        lines(g, w, *col, 0.75, (width * b.r * 0.5).max(0.35), 0.7, false);
    }
    for w in &l.rifts {
        lines(g, w, l.rift_cols.0, 0.22, (b.r * 0.009).max(1.5), (b.r * 0.012).max(1.5), true);
        lines(g, w, l.rift_cols.1, 0.8, (b.r * 0.0025).max(0.45), 0.8, true);
    }
    for (c, rho) in &l.vents {
        let (at, sp) = project(b, *c);
        if sp.z < 0.05 {
            continue;
        }
        let s = rho * b.r * 2.2;
        g.glow(at, s, l.rift_cols.0, 0.45, 14);
        g.glow(at, s * 0.35, mix(l.rift_cols.1, WHITE, 0.3), 0.8, 10);
    }
    if let Some((col, dots)) = &l.biolum {
        for f in dots {
            let (at, sp) = project(b, *f);
            let dark = 1.0 - b.lit(sp);
            if sp.z < 0.05 || dark < 0.05 {
                continue;
            }
            g.glow(at, (b.r * 0.015).max(1.6) * 1.5, *col, 0.35 * dark, 8);
        }
    }
}

/// The air's glow round the limb: a strip outside the disc, strongest at the limb and on the lit side (all round
/// when backlit), fading outwards.
fn air_glow(g: &mut Geo, b: &Body, p: &Paint) {
    let air = &b.look.air;
    let back = b.phase > 2.1;
    let w = |th: f32| {
        let k = (th - b.light).cos();
        if back { (0.3 + 0.7 * k.max(0.0)) * 1.25 } else { (0.15 + 0.85 * k).clamp(0.0, 1.0).powf(1.2) }
    };
    let t = (b.r * (0.015 + 0.05 * air.lv)).clamp(2.0, 42.0);
    let col = mix(air.col, WHITE, 0.2);
    let segs = ((TAU * b.r / 6.0) as u32).clamp(48, 256);
    let rows = 6;
    let first = g.len() as u32;
    for i in 0..segs {
        let th = i as f32 / segs as f32 * TAU;
        let a0 = 0.7 * w(th) * (0.4 + 0.6 * air.lv) * (1.0 - p.veil_share);
        for r in 0..=rows {
            let f = r as f32 / rows as f32;
            let at = Vec2::from_angle(th) * (b.r - 0.5 + t * f);
            g.v(at, vc(col, a0 * (1.0 - f).powf(1.8)));
        }
    }
    let k = rows + 1;
    for i in 0..segs {
        let (a, c) = (first + i * k, first + ((i + 1) % segs) * k);
        for r in 0..rows {
            g.quad(a + r, c + r, c + r + 1, a + r + 1);
        }
    }
}

/// A crystal moon: a fan of facets round an off-centre core, each shaded by how it faces the light, with pale edges.
fn facets(g: &mut Geo, b: &Body, f: &Facets, p: &Paint) {
    let core = f.core * b.r;
    let n = f.pts.len();
    for i in 0..n {
        let (t0, k0) = f.pts[i];
        let (t1, k1) = f.pts[(i + 1) % n];
        let mid = (t0 + t1 + if i == n - 1 { TAU } else { 0.0 }) / 2.0;
        let br = (0.5 + 0.45 * (mid - b.light).cos() + f.shades[i]).clamp(0.0, 1.0);
        let c = p.c(mix(f.cols.1, f.cols.0, br), 1.0);
        let a = g.v(core, c);
        let bb = g.v(Vec2::from_angle(t0) * b.r * k0, c);
        let cc = g.v(Vec2::from_angle(t1) * b.r * k1, c);
        g.tri(a, bb, cc);
    }
    let edge = mix(f.cols.0, WHITE, 0.4);
    for i in 0..n {
        let (t, k) = f.pts[i];
        let corner = Vec2::from_angle(t) * b.r * k;
        let next = Vec2::from_angle(f.pts[(i + 1) % n].0) * b.r * f.pts[(i + 1) % n].1;
        for (from, to) in [(core, corner), (corner, next)] {
            let cols = [mix(edge, p.veil, p.veil_share); 2];
            g.soft_strip(&[from, to], &cols, &[0.45, 0.45], 0.35, 0.6);
        }
    }
}

/// A geyser moon's plumes: soft fans off the limb, bright at the vent and fading outwards.
fn plumes(g: &mut Geo, b: &Body, p: &Paint) {
    let bright = Vec3::new(0.86, 0.94, 1.0);
    let far = Vec3::new(0.63, 0.82, 1.0);
    for pl in &b.look.plumes {
        let dir = Vec2::from_angle(pl.a);
        let nrm = Vec2::new(-dir.y, dir.x);
        let base = dir * b.r * 0.92;
        let end = dir * b.r * (1.0 + pl.len);
        let (hw, ew) = (pl.w * b.r * 0.3, pl.w * b.r * 3.2);
        let a = 0.5 * (1.0 - p.veil_share);
        let v0 = g.v(base + nrm * hw * 2.0, vc(bright, 0.0));
        let v1 = g.v(base, vc(bright, a));
        let v2 = g.v(base - nrm * hw * 2.0, vc(bright, 0.0));
        let v3 = g.v(end + nrm * ew, vc(far, 0.0));
        let v4 = g.v(end, vc(far, 0.0));
        let v5 = g.v(end - nrm * ew, vc(far, 0.0));
        let mid = g.v((base + end) / 2.0, vc(mix(bright, far, 0.5), a * 0.4));
        let mn = g.v((base + end) / 2.0 + nrm * (hw + ew) / 2.0 * 1.2, vc(far, 0.0));
        let ms = g.v((base + end) / 2.0 - nrm * (hw + ew) / 2.0 * 1.2, vc(far, 0.0));
        g.quad(v0, v1, mid, mn);
        g.quad(v1, v2, ms, mid);
        g.quad(mn, mid, v4, v3);
        g.quad(mid, ms, v5, v4);
    }
}

/// One half of the rings (`back`: the half behind the body, drawn under it): each band an elliptical annulus with
/// feathered edges; the back half darkened where the body's shadow falls.
fn ring_half(b: &Body, r: &Ring, p: &Paint, white: Vec2, back: bool) -> Geo {
    let mut g = Geo::new(white);
    let segs = 72;
    let (rs, rc) = r.rot.sin_cos();
    let (ls, lc) = (-b.light).sin_cos();
    let feather = 0.8 / b.r;
    for (r0, r1, col, a) in &r.subs {
        let rows = [(r0 - feather, 0.0), (*r0, *a), (*r1, *a), (r1 + feather, 0.0)];
        let first = g.len() as u32;
        for i in 0..=segs {
            let t = if back { PI + PI * i as f32 / segs as f32 } else { PI * i as f32 / segs as f32 };
            for (rad, alpha) in rows {
                let local = Vec2::new(t.cos() * rad, t.sin() * rad * r.k) * b.r;
                let at = Vec2::new(local.x * rc - local.y * rs, local.x * rs + local.y * rc);
                // the body's shadow: behind it, away from the light
                let l = Vec2::new(at.x * lc - at.y * ls, at.x * ls + at.y * lc);
                let shadow = if back { smooth(0.0, -0.08 * b.r, l.x) * (1.0 - smooth(0.9 * b.r, 1.0 * b.r, l.y.abs())) } else { 0.0 };
                let c = mix(*col, p.night, 0.82 * shadow);
                g.v(at, vc(mix(c, p.veil, p.veil_share), alpha));
            }
        }
        for i in 0..segs {
            let (s0, s1) = (first + i * 4, first + (i + 1) * 4);
            for k in 0..3 {
                g.quad(s0 + k, s1 + k, s1 + k + 1, s0 + k + 1);
            }
        }
    }
    g
}
