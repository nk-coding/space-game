//! The sky behind a cluster (GDR-0105): a planet (large and cut by a corner of the view, or small and whole), its
//! moons, a nebula and a far galaxy, rolled from the cluster's seed on named streams (one per layer) from its type's
//! or its band's `sky` and the planet kinds (`planets.ron`), built once per voyage as vertex-coloured meshes and
//! drawn far behind the play. Client-only: the sim never sees it.
//!
//! Composed in a design frame of 1600 × 900 px (the study's) and drawn the same size on screen at any zoom: scaled
//! to the view's height, each body kept at its distance from the view's left or right edge (so a planet cut by a
//! corner stays cut by it), the nebula stretched to the view's width. The layers move against the camera by a share
//! of its motion on screen (feel, not physics): the galaxy not at all, the nebula 0.005, the planet 0.05 and its moons
//! 0.075 (the far stars 0.02, `backdrop.rs`). A voyage carries the camera kilometres, so the shift is kept within
//! [`DRIFT_REACH`] of the view's height: moving further out it slows to nothing, moving back it is whole again.
//! Everything is drawn at [`STRENGTH`] (veiled towards the sky's dark), so it stays quiet behind a fight.

mod kinds;
mod nebula;
mod paint;
mod roll;

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use data::{BodyClass, PlanetKind, Sky};
use kit::params::RenderToggles;

use super::{Build, SceneryPart};
use crate::camera::MainCamera;
use paint::{Body, Paint};
use roll::{Stream, mix};

/// The design frame (px).
const W: f32 = 1600.0;
const H: f32 = 900.0;
/// How strong the sky shows (the rest is veiled in the sky's dark).
const STRENGTH: f32 = 0.85;
/// Each layer's share of the camera's motion on screen.
const PARALLAX_NEBULA: f32 = 0.005;
const PARALLAX_PLANET: f32 = 0.05;
const PARALLAX_MOONS: f32 = 0.075;
/// The planet layer's shift stays within this share of the view's height.
const DRIFT_REACH: f32 = 0.3;
/// Draw order: the galaxy over the gradient, the far stars (−893, `backdrop.rs`), the nebula, the rings' back
/// halves, the planet, the rings' front halves, the moons; the hot star's glow (−880) over them all.
const Z_GALAXY: f32 = -897.0;
const Z_NEBULA: f32 = -890.0;
const Z_PLANET: f32 = -886.0;
const Z_MOONS: f32 = -883.0;

/// A piece of the sky: where it sits in the design frame (y down), which edge of the view it keeps its distance to,
/// how much of the planet layer's shift it takes, and whether it stretches to the view's width (the nebula).
#[derive(Component)]
pub(super) struct SkyPart {
    at: Vec2,
    right: bool,
    depth: f32,
    stretch: bool,
}

/// The planet layer's shift on screen (logical px) and the camera's place last frame.
#[derive(Resource, Default)]
pub(super) struct SkyDrift {
    shift: Vec2,
    last: Option<Vec2>,
}

/// What [`spawn`] made (for the log line).
pub(super) struct SkyCounts {
    pub planet: String,
    pub moons: Vec<String>,
    pub vertices: [usize; 4],
    pub nebula_peak: u8,
}

/// The sky's inputs: the sky itself, the seed, the hot star's direction on screen from the entry (y up), and a
/// planet kind to show instead of the rolled one (`?planet=<kind>`, for looking at kinds).
pub(super) struct SkySpec<'a> {
    pub sky: &'a Sky,
    pub seed: u64,
    pub star: Option<Vec2>,
    pub planet: Option<&'a str>,
}

/// Spawns the sky's meshes; `None` without planet kinds.
pub(super) fn spawn(cx: &mut Build, s: &SkySpec, content: &data::Content, drift: &mut SkyDrift) -> Option<SkyCounts> {
    *drift = SkyDrift::default();
    let white = cx.atlas.region("fx/white").uv_center();
    let soft = cx.atlas.region("fx/dot_soft");
    let dot = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)].map(|f| soft.uv_at(f));
    let bg = cx.style.background.outer().to_srgba();
    let veil = Vec3::new(bg.red, bg.green, bg.blue);
    let night = mix(veil, Vec3::new(6.0, 10.0, 16.0) / 255.0, 0.5);
    let paint = Paint { night, veil, veil_share: 1.0 - STRENGTH };

    // The hot star's side, where the light comes from (design frame, y down).
    let hot = s.star.map(|d| {
        let d = Vec2::new(d.x, -d.y).normalize_or(Vec2::X);
        let t = (W / 2.0 / d.x.abs().max(1e-3)).min(H / 2.0 / d.y.abs().max(1e-3)) * 1.1;
        Vec2::new(W, H) / 2.0 + d * t
    });
    let planets: Vec<&PlanetKind> = content.planets.iter().filter(|k| k.body == BodyClass::Planet).collect();
    let pool: Vec<(&PlanetKind, u32)> = if s.sky.planets.is_empty() {
        planets.iter().map(|k| (*k, 1)).collect()
    } else {
        s.sky.planets.iter().filter_map(|w| Some((content.planets.get(&w.kind)?, w.weight))).collect()
    };
    if pool.is_empty() {
        return None;
    }
    let all: Vec<(&PlanetKind, u32)> = planets.iter().map(|k| (*k, 1)).collect();
    let (planet, kind) = roll_planet(&mut Stream::new(s.seed, "planet"), s, &pool, &all, hot);
    let moons = roll_moons(&mut Stream::new(s.seed, "moons"), s, content, &planet, hot);
    let nebula = nebula::nebula(&mut Stream::new(s.seed, "nebula"), s.sky, night, hot.is_some());
    let reach = planet.r * if planet.look.ring.is_some() { 2.2 } else { 1.1 };
    let galaxy = nebula::galaxy(&mut Stream::new(s.seed, "galaxy"), (planet.at, reach), s.sky.galaxy);

    let mut counts = SkyCounts { planet: String::new(), moons: Vec::new(), vertices: [0; 4], nebula_peak: nebula.peak };
    let right = planet.at.x > W / 2.0;
    let put = |cx: &mut Build, g: paint::Geo, part: SkyPart, z: f32| -> usize {
        let n = g.len();
        cx.commands.spawn((SceneryPart, part, Mesh2d(cx.meshes.add(g.into_mesh())), MeshMaterial2d(cx.atlas.material.clone()), Transform::from_xyz(0.0, 0.0, z)));
        n
    };
    let painted = paint::paint(&planet, &paint, white);
    let body = |depth: f32, b: &Body| SkyPart { at: b.at, right, depth, stretch: false };
    counts.vertices[0] += put(cx, painted.body, body(1.0, &planet), Z_PLANET);
    if let Some(g) = painted.ring_back {
        counts.vertices[0] += put(cx, g, body(1.0, &planet), Z_PLANET - 1.0);
    }
    if let Some(g) = painted.ring_front {
        counts.vertices[0] += put(cx, g, body(1.0, &planet), Z_PLANET + 1.0);
    }
    for (k, (m, name)) in moons.iter().enumerate() {
        let painted = paint::paint(m, &paint, white);
        counts.vertices[1] += put(cx, painted.body, body(PARALLAX_MOONS / PARALLAX_PLANET, m), Z_MOONS + k as f32 * 0.1);
        counts.moons.push(name.clone());
    }
    let neb = nebula::nebula_mesh(&nebula, white, dot, STRENGTH);
    counts.vertices[2] = put(cx, neb, SkyPart { at: Vec2::new(W, H) / 2.0, right: false, depth: PARALLAX_NEBULA / PARALLAX_PLANET, stretch: true }, Z_NEBULA);
    let gal = nebula::galaxy_mesh(&galaxy, white, dot, STRENGTH);
    counts.vertices[3] = put(cx, gal, SkyPart { at: galaxy.at, right: galaxy.at.x > W / 2.0, depth: 0.0, stretch: false }, Z_GALAXY);
    counts.planet = format!("{kind} (r {:.0} px{})", planet.r, if planet.look.ring.is_some() { ", ringed" } else { "" });
    Some(counts)
}

/// The planet: a kind by weight; large (60 %) and cut by a corner, or small and whole away from the view's middle;
/// lit from the hot star, from behind (a backlit sky) or from the view's middle side.
fn roll_planet(r: &mut Stream, s: &SkySpec, pool: &[(&PlanetKind, u32)], all: &[(&PlanetKind, u32)], hot: Option<Vec2>) -> (Body, String) {
    let weights: Vec<u32> = pool.iter().map(|p| p.1).collect();
    let rolled = pool[r.weighted(&weights)].0;
    let kind = s.planet.and_then(|id| all.iter().find(|k| k.0.id == id)).map_or(rolled, |k| k.0);
    let big = r.chance(0.6);
    let mut rad;
    let mut at = Vec2::ZERO;
    if big {
        rad = r.range(0.34, 0.56) * H;
        let (left, top) = (r.chance(0.5), r.chance(0.5));
        for _ in 0..12 {
            at.x = if left { rad * r.range(-0.25, 0.35) } else { W - rad * r.range(-0.25, 0.35) };
            at.y = if top { rad * r.range(-0.25, 0.35) } else { H - rad * r.range(-0.25, 0.35) };
            if at.distance(Vec2::new(W, H) / 2.0) > rad + 130.0 {
                break;
            }
            rad *= 0.92;
        }
    } else {
        rad = r.range(0.11, 0.19) * H;
        for _ in 0..40 {
            at = Vec2::new(r.range(0.12, 0.88) * W, r.range(0.17, 0.83) * H);
            if at.distance(Vec2::new(W, H) / 2.0) > rad * 2.4 + 230.0 {
                break;
            }
        }
    }
    let (light, phase) = match hot {
        Some(h) => ((h.y - at.y).atan2(h.x - at.x), r.range(0.35, 0.95)),
        None if s.sky.backlit => (r.range(0.0, TAU), r.range(2.3, 2.65)),
        None => ((H / 2.0 - at.y).atan2(W / 2.0 - at.x) + r.range(-1.2, 1.2), r.range(0.55, 1.6)),
    };
    let mut tilt = r.range(-0.4, 0.4);
    let mut look = kinds::look(&mut Stream::new(s.seed, &format!("planet:{}", kind.id)), kind, tilt);
    if let Some(ring) = &mut look.ring
        && big
    {
        // a large ringed planet is smaller, and its rings (and bands) lean towards the view's middle, so the rings
        // cross the view instead of running off it past the corner
        rad *= 0.75;
        let to_mid = (H / 2.0 - at.y).atan2(W / 2.0 - at.x);
        let lean = (to_mid + PI / 2.0).rem_euclid(PI) - PI / 2.0;
        let aim = lean.clamp(-0.7, 0.7);
        ring.rot += aim - tilt;
        tilt = aim;
    }
    let mut b = Body { r: rad, at, light, phase, tilt, moon: false, look };
    aim(&mut b);
    (b, kind.id.clone())
}

/// An eyeball world's sea faces its light: its rings centred on the point under it.
fn aim(b: &mut Body) {
    let sun = Vec3::new(b.light.cos() * b.phase.sin(), b.light.sin() * b.phase.sin(), b.phase.cos());
    let (sn, cs) = (-b.tilt).sin_cos();
    kinds::aim_eye(&mut b.look, Vec3::new(sun.x * cs - sun.y * sn, sun.x * sn + sun.y * cs, sun.z));
}

/// Up to three moons round the planet, inside the view and clear of its middle and of each other, each of a kind
/// not yet in the sky.
fn roll_moons(r: &mut Stream, s: &SkySpec, content: &data::Content, p: &Body, hot: Option<Vec2>) -> Vec<(Body, String)> {
    let n = [0, 1, 2, 3][r.weighted(&[1, 3, 3, 2])];
    let mut kinds: Vec<(&PlanetKind, u32)> = s.sky.moons.iter().filter_map(|w| Some((content.planets.get(&w.kind)?, w.weight))).collect();
    let mut out: Vec<(Body, String)> = Vec::new();
    for i in 0..n {
        if kinds.is_empty() {
            break;
        }
        for _ in 0..50 {
            let mr = (p.r * r.range(0.05, 0.14)).clamp(10.0, 60.0);
            let a = r.range(0.0, TAU);
            let d = p.r * if p.look.ring.is_some() { r.range(2.2, 2.9) } else { r.range(1.35, 2.4) } + mr;
            let at = p.at + Vec2::from_angle(a) * d;
            if at.x < mr + 24.0 || at.x > W - mr - 24.0 || at.y < mr + 24.0 || at.y > H - mr - 24.0 {
                continue;
            }
            if at.distance(Vec2::new(W, H) / 2.0) < mr + 180.0 || out.iter().any(|(m, _)| m.at.distance(at) < m.r + mr + 30.0) {
                continue;
            }
            let weights: Vec<u32> = kinds.iter().map(|k| k.1).collect();
            let kind = kinds.remove(r.weighted(&weights)).0;
            let light = hot.map_or(p.light, |h| (h.y - at.y).atan2(h.x - at.x));
            let tilt = r.range(-0.4, 0.4);
            let look = kinds::look(&mut Stream::new(s.seed, &format!("moon{i}:{}", kind.id)), kind, tilt);
            let mut m = Body { r: mr, at, light, phase: p.phase, tilt, moon: true, look };
            aim(&mut m);
            out.push((m, kind.id.clone()));
            break;
        }
    }
    out
}

/// Places the sky's pieces for the view: each at its distance from its edge of the view, scaled to the view's
/// height, shifted by its share of the planet layer's drift.
#[allow(clippy::type_complexity)]
pub(super) fn place(
    toggles: Res<RenderToggles>,
    mut drift: ResMut<SkyDrift>,
    camera: Single<(&Camera, &Projection, &Transform), (With<MainCamera>, Without<SkyPart>)>,
    mut parts: Query<(&SkyPart, &mut Transform, &mut Visibility)>,
) {
    if parts.is_empty() {
        return;
    }
    let (cam, proj, tf) = camera.into_inner();
    let k = match proj {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };
    let view = cam.logical_viewport_size().unwrap_or(Vec2::new(W, H));
    let at = tf.translation.truncate();
    if let Some(last) = drift.last {
        // the camera's motion on screen; the layer moves the other way by its share
        let mut d = -(at - last) / k.max(1e-6) * PARALLAX_PLANET;
        let reach = DRIFT_REACH * view.y;
        if d.dot(drift.shift) > 0.0 {
            d *= (1.0 - (drift.shift.length() / reach).powi(2)).max(0.0);
        }
        drift.shift += d;
    }
    drift.last = Some(at);
    let s = view.y / H;
    let shown = if toggles.debug_shapes { Visibility::Hidden } else { Visibility::Inherited };
    for (p, mut t, mut vis) in &mut parts {
        let x = if p.stretch {
            0.0
        } else if p.right {
            view.x / 2.0 - (W - p.at.x) * s
        } else {
            -view.x / 2.0 + p.at.x * s
        };
        let y = (H / 2.0 - p.at.y) * s;
        let off = drift.shift * p.depth;
        t.translation.x = at.x + (x + off.x) * k;
        t.translation.y = at.y + (y + off.y) * k;
        let sx = if p.stretch { view.x / W } else { s };
        t.scale = Vec3::new(sx * k, s * k, 1.0);
        if *vis != shown {
            *vis = shown;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skies(c: &data::Content) -> Vec<Sky> {
        c.cluster_types.iter().filter_map(|t| t.sky.clone()).chain(c.bands.iter().filter_map(|b| b.sky.clone())).collect()
    }

    /// Every sky rolls a planet of its pool and moons of different kinds, all inside the design frame.
    #[test]
    fn moons_never_repeat_a_kind() {
        let c = data::base_content();
        for sky in skies(&c) {
            for seed in 0..200u64 {
                let s = SkySpec { sky: &sky, seed, star: (seed % 3 == 0).then_some(Vec2::new(1.0, 0.4)), planet: None };
                let all: Vec<(&PlanetKind, u32)> = c.planets.iter().filter(|k| k.body == BodyClass::Planet).map(|k| (k, 1)).collect();
                let pool: Vec<(&PlanetKind, u32)> = if sky.planets.is_empty() { all.clone() } else { sky.planets.iter().map(|w| (c.planets.get(&w.kind).unwrap(), w.weight)).collect() };
                let hot = s.star.map(|_| Vec2::new(W + 100.0, H / 2.0));
                let (p, kind) = roll_planet(&mut Stream::new(seed, "planet"), &s, &pool, &all, hot);
                assert!(pool.iter().any(|k| k.0.id == kind));
                let moons = roll_moons(&mut Stream::new(seed, "moons"), &s, &c, &p, hot);
                let mut ids: Vec<&String> = moons.iter().map(|m| &m.1).collect();
                ids.sort();
                ids.dedup();
                assert_eq!(ids.len(), moons.len(), "seed {seed}: a moon kind twice");
                for (m, _) in &moons {
                    assert!(m.at.x > 0.0 && m.at.x < W && m.at.y > 0.0 && m.at.y < H);
                }
            }
        }
    }

    /// Every kind paints, large or as a moon, within the vertex budget and without a broken colour.
    #[test]
    fn every_kind_paints_within_budget() {
        let c = data::base_content();
        let paint = Paint { night: Vec3::splat(0.02), veil: Vec3::splat(0.03), veil_share: 1.0 - STRENGTH };
        for k in c.planets.iter() {
            for seed in 0..4u64 {
                let moon = k.body == BodyClass::Moon;
                let mut r = Stream::new(seed, &k.id);
                let look = kinds::look(&mut r, k, 0.2);
                let b = Body { r: if moon { 60.0 } else { 500.0 }, at: Vec2::new(300.0, 300.0), light: 0.5, phase: 0.9, tilt: 0.2, moon, look };
                let p = paint::paint(&b, &paint, Vec2::ZERO);
                let n = p.body.len() + p.ring_back.as_ref().map_or(0, |g| g.len()) + p.ring_front.as_ref().map_or(0, |g| g.len());
                assert!(n < if moon { 4_000 } else { 40_000 }, "{} paints {n} vertices", k.id);
                assert!(p.body.finite(), "{}: a colour or position is not finite", k.id);
            }
        }
    }
}
