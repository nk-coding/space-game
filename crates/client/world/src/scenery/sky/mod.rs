//! The sky behind a cluster (GDR-0105): two or three planets (each whole, one at most large), their moons, a nebula
//! and a far galaxy, rolled from the cluster's seed on named streams (one per layer) from its type's or its band's
//! `sky` and the planet kinds (`planets.ron`), built once per voyage as vertex-coloured meshes and drawn far behind
//! the play; at home the home band's nebula and galaxy alone, fainter. Client-only: the sim never sees it.
//!
//! Composed in a design frame of 1600 × 900 px (the study's), scaled to the view's height: the galaxy kept at its
//! distance from the view's left or right edge, the nebula stretched to the view's width, each planet and its moons
//! round the view's middle. Every piece also hangs at an anchor in the cluster on a layer of its own parallax like
//! the rest of the scenery (`camera + (anchor − camera) · p · depth_scale(p)`, GDR-0107; true parallax since
//! Niklas's look of 18 Oct 2026, no cap, so a planet leaves the view): the galaxy 0, the nebula 0.003, the planets
//! 0.020–0.028 (the bigger the nearer and faster) and their moons a third more, above the stars (0.005–0.017) and
//! below the junk (0.3 and up, `backdrop.rs`, `filler.rs`). A planet's anchor is where the camera sees it in the
//! view's middle: the first between the entry and the centre, the others round the centre, so some planet is in
//! view from most of the cluster. Everything is drawn at [`STRENGTH`] (veiled towards the sky's dark), so it stays
//! quiet behind a fight.

mod kinds;
mod nebula;
mod paint;
mod roll;

use std::f32::consts::TAU;

use bevy::prelude::*;
use data::{BodyClass, PlanetKind, Sky};
use kit::params::RenderToggles;

use super::{Build, SceneryPart};
use crate::camera::{MainCamera, REF_ZOOM, depth_scale};
use paint::{Body, Paint};
use roll::{Stream, mix};

/// The design frame (px).
const W: f32 = 1600.0;
const H: f32 = 900.0;
/// How strong the sky shows in flight (the rest is veiled in the sky's dark).
pub(super) const STRENGTH: f32 = 0.85;
/// Each layer's share of the camera's motion: the nebula's, and the planets' round which each planet's lies
/// (from [`PLANET_SPREAD`] below to as far above, by its size).
const PARALLAX_NEBULA: f32 = 0.003;
const PARALLAX_PLANET: f32 = 0.024;
const PLANET_SPREAD: f32 = 0.15;
/// A moon's parallax over its planet's.
const MOON_DEPTH: f32 = 4.0 / 3.0;
/// The planets: two or three by weight.
const PLANET_COUNTS: [(usize, u32); 2] = [(2, 3), (3, 7)];
/// The share of a planet's place towards the cluster's centre from the entry (the first planet) and from the centre
/// (the others, round it).
const FIRST_AT: f32 = 0.3;
const OTHERS_AT: (f32, f32) = (0.4, 0.6);
/// Draw order: the galaxy over the gradient, the far stars (−893, `backdrop.rs`), the nebula, the rings' back
/// halves, the planets, the rings' front halves, the moons. The hot star's glow (−889), its corona and the landmarks
/// (−888.5) lie between the nebula and the planets: nothing farther off draws over a planet (GDR-0107).
const Z_GALAXY: f32 = -897.0;
const Z_NEBULA: f32 = -890.0;
const Z_PLANET: f32 = -886.0;
const Z_MOONS: f32 = -883.0;

/// Where a piece sits across the view: at its distance from the left or the right edge, round the middle, or
/// stretched to the view's width (the nebula).
#[derive(Clone, Copy)]
enum Edge {
    Left,
    Right,
    Mid,
    Stretch,
}

/// A piece of the sky: where it sits in the design frame (y down) and across the view, and its anchor in the
/// cluster (m) on its layer of parallax `parallax`.
#[derive(Component)]
pub(super) struct SkyPart {
    at: Vec2,
    edge: Edge,
    anchor: Vec2,
    parallax: f32,
}

/// What [`spawn`] made (for the log line).
pub(super) struct SkyCounts {
    pub planets: Vec<String>,
    pub moons: Vec<String>,
    pub vertices: [usize; 4],
    pub nebula_peak: u8,
}

/// The cluster a sky hangs over: its radius and where the fleet enters (m, round its centre).
#[derive(Clone, Copy)]
pub(super) struct Over {
    pub radius: f32,
    pub entry: Vec2,
}

/// The sky's inputs: the sky itself, the seed, the hot star's direction on screen from the entry (y up), a planet
/// kind to show as the first planet (`?planet=<kind>`, for looking at kinds), the cluster (none at home: no
/// planets) and how strong it shows.
pub(super) struct SkySpec<'a> {
    pub sky: &'a Sky,
    pub seed: u64,
    pub star: Option<Vec2>,
    pub planet: Option<&'a str>,
    pub over: Option<Over>,
    pub strength: f32,
}

/// A rolled planet: its body (in the design frame's middle), its kind, its anchor and its parallax.
struct Planet {
    body: Body,
    kind: String,
    anchor: Vec2,
    parallax: f32,
}

/// Spawns the sky's meshes; `None` in a cluster without planet kinds.
pub(super) fn spawn(cx: &mut Build, s: &SkySpec, content: &data::Content) -> Option<SkyCounts> {
    let white = cx.atlas.region("fx/white").uv_center();
    let soft = cx.atlas.region("fx/dot_soft");
    let dot = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)].map(|f| soft.uv_at(f));
    let bg = cx.style.background.outer().to_srgba();
    let veil = Vec3::new(bg.red, bg.green, bg.blue);
    let night = mix(veil, Vec3::new(6.0, 10.0, 16.0) / 255.0, 0.5);
    let paint = Paint { night, veil, veil_share: 1.0 - s.strength };

    // The hot star's side, where the light comes from (design frame, y down).
    let hot = s.star.map(|d| {
        let d = Vec2::new(d.x, -d.y).normalize_or(Vec2::X);
        let t = (W / 2.0 / d.x.abs().max(1e-3)).min(H / 2.0 / d.y.abs().max(1e-3)) * 1.1;
        Vec2::new(W, H) / 2.0 + d * t
    });
    let planets = match s.over {
        Some(over) => {
            let all: Vec<(&PlanetKind, u32)> = content.planets.iter().filter(|k| k.body == BodyClass::Planet).map(|k| (k, 1)).collect();
            let pool: Vec<(&PlanetKind, u32)> =
                if s.sky.planets.is_empty() { all.clone() } else { s.sky.planets.iter().filter_map(|w| Some((content.planets.get(&w.kind)?, w.weight))).collect() };
            if pool.is_empty() {
                return None;
            }
            roll_planets(&mut Stream::new(s.seed, "planet"), s, &pool, &all, hot, over)
        }
        None => Vec::new(),
    };
    let moons = roll_moons(&mut Stream::new(s.seed, "moons"), s, content, &planets, hot);
    let nebula = nebula::nebula(&mut Stream::new(s.seed, "nebula"), s.sky, night, hot.is_some());
    let galaxy = nebula::galaxy(&mut Stream::new(s.seed, "galaxy"), s.sky.galaxy);

    let mut counts = SkyCounts { planets: Vec::new(), moons: Vec::new(), vertices: [0; 4], nebula_peak: nebula.peak };
    let put = |cx: &mut Build, g: paint::Geo, part: SkyPart, z: f32| -> usize {
        let n = g.len();
        cx.commands.spawn((SceneryPart, part, Mesh2d(cx.meshes.add(g.into_mesh())), MeshMaterial2d(cx.atlas.material.clone()), Transform::from_xyz(0.0, 0.0, z)));
        n
    };
    for (i, p) in planets.iter().enumerate() {
        let painted = paint::paint(&p.body, &paint, white);
        let part = || SkyPart { at: p.body.at, edge: Edge::Mid, anchor: p.anchor, parallax: p.parallax };
        // nearer planets over farther ones
        let z = Z_PLANET + p.parallax / PARALLAX_PLANET * 0.1 + i as f32 * 0.01;
        counts.vertices[0] += put(cx, painted.body, part(), z);
        if let Some(g) = painted.ring_back {
            counts.vertices[0] += put(cx, g, part(), z - 1.0);
        }
        if let Some(g) = painted.ring_front {
            counts.vertices[0] += put(cx, g, part(), z + 1.0);
        }
        counts.planets.push(format!("{} (r {:.0} px, p {:.3}{})", p.kind, p.body.r, p.parallax, if p.body.look.ring.is_some() { ", ringed" } else { "" }));
    }
    for (k, (m, of, name)) in moons.iter().enumerate() {
        let painted = paint::paint(m, &paint, white);
        let p = &planets[*of];
        let part = SkyPart { at: m.at, edge: Edge::Mid, anchor: p.anchor, parallax: p.parallax * MOON_DEPTH };
        counts.vertices[1] += put(cx, painted.body, part, Z_MOONS + k as f32 * 0.1);
        counts.moons.push(name.clone());
    }
    let neb = nebula::nebula_mesh(&nebula, white, dot, s.strength);
    let anchor = s.over.map_or(Vec2::ZERO, |o| o.entry);
    counts.vertices[2] = put(cx, neb, SkyPart { at: Vec2::new(W, H) / 2.0, edge: Edge::Stretch, anchor, parallax: PARALLAX_NEBULA }, Z_NEBULA);
    let gal = nebula::galaxy_mesh(&galaxy, white, dot, s.strength);
    let edge = if galaxy.at.x > W / 2.0 { Edge::Right } else { Edge::Left };
    counts.vertices[3] = put(cx, gal, SkyPart { at: galaxy.at, edge, anchor: Vec2::ZERO, parallax: 0.0 }, Z_GALAXY);
    Some(counts)
}

/// Two or three planets of different kinds by weight (the first `?planet=`'s if given), each in the design frame's
/// middle, small (0.07–0.13 of the view's height) or at most one large (25 %: 0.22–0.34; Niklas, 18 Oct 2026,
/// planets read too near), its parallax by its size; lit from the hot star, from behind (a backlit sky) or from a
/// side. The first is anchored between the entry and the cluster's centre, so it greets the fleet, the others round
/// the centre a turn's share apart from it.
fn roll_planets(r: &mut Stream, s: &SkySpec, pool: &[(&PlanetKind, u32)], all: &[(&PlanetKind, u32)], hot: Option<Vec2>, over: Over) -> Vec<Planet> {
    let n = PLANET_COUNTS[r.weighted(&PLANET_COUNTS.map(|c| c.1))].0;
    let mid = Vec2::new(W, H) / 2.0;
    let mut pool = pool.to_vec();
    let mut big_left = true;
    let first = Vec2::from_angle(over.entry.to_angle() + r.range(-0.4, 0.4)) * over.radius * (1.0 - FIRST_AT);
    let mut out = Vec::new();
    for i in 0..n {
        let weights: Vec<u32> = pool.iter().map(|p| p.1).collect();
        let rolled = pool[r.weighted(&weights)].0;
        let kind = s.planet.filter(|_| i == 0).and_then(|id| all.iter().find(|k| k.0.id == id)).map_or(rolled, |k| k.0);
        pool.retain(|p| p.0.id != kind.id);
        let big = big_left && r.chance(0.25);
        big_left &= !big;
        let mut rad = if big { r.range(0.22, 0.34) } else { r.range(0.07, 0.13) } * H;
        let (light, phase) = match hot {
            Some(h) => ((h.y - mid.y).atan2(h.x - mid.x), r.range(0.35, 0.95)),
            None if s.sky.backlit => (r.range(0.0, TAU), r.range(2.3, 2.65)),
            None => (r.range(0.0, TAU), r.range(0.55, 1.6)),
        };
        let tilt = r.range(-0.4, 0.4);
        let stream = if i == 0 { format!("planet:{}", kind.id) } else { format!("planet{i}:{}", kind.id) };
        let look = kinds::look(&mut Stream::new(s.seed, &stream), kind, tilt);
        if look.ring.is_some() && big {
            // a large ringed planet is smaller, so its rings fit the view
            rad *= 0.75;
        }
        let size = ((rad / H - 0.07) / (0.34 - 0.07)).clamp(0.0, 1.0);
        let parallax = PARALLAX_PLANET * (1.0 - PLANET_SPREAD + 2.0 * PLANET_SPREAD * size);
        let anchor = if i == 0 {
            first
        } else {
            let turn = first.to_angle() + TAU * i as f32 / n as f32 + r.range(-0.3, 0.3);
            Vec2::from_angle(turn) * over.radius * r.range(OTHERS_AT.0, OTHERS_AT.1)
        };
        let mut body = Body { r: rad, at: mid, light, phase, tilt, moon: false, look };
        aim(&mut body);
        out.push(Planet { body, kind: kind.id.clone(), anchor, parallax });
        if pool.is_empty() {
            break;
        }
    }
    out
}

/// An eyeball world's sea faces its light: its rings centred on the point under it.
fn aim(b: &mut Body) {
    let sun = Vec3::new(b.light.cos() * b.phase.sin(), b.light.sin() * b.phase.sin(), b.phase.cos());
    let (sn, cs) = (-b.tilt).sin_cos();
    kinds::aim_eye(&mut b.look, Vec3::new(sun.x * cs - sun.y * sn, sun.x * sn + sun.y * cs, sun.z));
}

/// Up to three moons round each planet, inside the view and clear of their planet and of each other, each of a kind
/// not yet in the sky; with the index of their planet.
fn roll_moons(r: &mut Stream, s: &SkySpec, content: &data::Content, planets: &[Planet], hot: Option<Vec2>) -> Vec<(Body, usize, String)> {
    let mut kinds: Vec<(&PlanetKind, u32)> = s.sky.moons.iter().filter_map(|w| Some((content.planets.get(&w.kind)?, w.weight))).collect();
    let mut out: Vec<(Body, usize, String)> = Vec::new();
    for (of, planet) in planets.iter().enumerate() {
        let p = &planet.body;
        let n = [0, 1, 2, 3][r.weighted(&[1, 3, 3, 2])];
        let first = out.len();
        for _ in 0..n {
            if kinds.is_empty() {
                break;
            }
            for _ in 0..50 {
                let mr = (p.r * r.range(0.05, 0.14)).clamp(7.0, 40.0);
                let a = r.range(0.0, TAU);
                let d = p.r * if p.look.ring.is_some() { r.range(2.2, 2.9) } else { r.range(1.35, 2.4) } + mr;
                let at = p.at + Vec2::from_angle(a) * d;
                if at.x < mr + 24.0 || at.x > W - mr - 24.0 || at.y < mr + 24.0 || at.y > H - mr - 24.0 {
                    continue;
                }
                if out[first..].iter().any(|(m, _, _)| m.at.distance(at) < m.r + mr + 30.0) {
                    continue;
                }
                let weights: Vec<u32> = kinds.iter().map(|k| k.1).collect();
                let kind = kinds.remove(r.weighted(&weights)).0;
                let light = hot.map_or(p.light, |h| (h.y - at.y).atan2(h.x - at.x));
                let tilt = r.range(-0.4, 0.4);
                let look = kinds::look(&mut Stream::new(s.seed, &format!("moon{}:{}", out.len(), kind.id)), kind, tilt);
                let mut m = Body { r: mr, at, light, phase: p.phase, tilt, moon: true, look };
                aim(&mut m);
                out.push((m, of, kind.id.clone()));
                break;
            }
        }
    }
    out
}

/// Places the sky's pieces for the view: each at its place across the view, scaled to the view's height (and to its
/// depth's scale, the sky hardly growing with the zoom), and moved by its parallax about its anchor.
#[allow(clippy::type_complexity)]
pub(super) fn place(
    toggles: Res<RenderToggles>,
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
    let s = view.y / H;
    let zoom = 1.0 / (REF_ZOOM * k.max(1e-6));
    let shown = if toggles.debug_shapes { Visibility::Hidden } else { Visibility::Inherited };
    for (p, mut t, mut vis) in &mut parts {
        let x = match p.edge {
            Edge::Left => -view.x / 2.0 + p.at.x * s,
            Edge::Right => view.x / 2.0 - (W - p.at.x) * s,
            Edge::Mid | Edge::Stretch => (p.at.x - W / 2.0) * s,
        };
        let y = (H / 2.0 - p.at.y) * s;
        let off = (p.anchor - at) * p.parallax * depth_scale(k, p.parallax);
        t.translation.x = at.x + x * k + off.x;
        t.translation.y = at.y + y * k + off.y;
        // the size on screen goes with the zoom to the power of the parallax (GDR-0107: hardly, the sky being far)
        let g = zoom.powf(p.parallax);
        let sx = if matches!(p.edge, Edge::Stretch) { view.x / W } else { s * g };
        t.scale = Vec3::new(sx * k, s * g * k, 1.0);
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

    /// The planets and moons of `seed` over a cluster of `radius`.
    fn roll(c: &data::Content, sky: &Sky, seed: u64, radius: f32) -> (Vec<Planet>, Vec<(Body, usize, String)>) {
        let over = Over { radius, entry: Vec2::new(0.0, -radius) };
        let s = SkySpec { sky, seed, star: seed.is_multiple_of(3).then_some(Vec2::new(1.0, 0.4)), planet: None, over: Some(over), strength: STRENGTH };
        let all: Vec<(&PlanetKind, u32)> = c.planets.iter().filter(|k| k.body == BodyClass::Planet).map(|k| (k, 1)).collect();
        let pool: Vec<(&PlanetKind, u32)> = if sky.planets.is_empty() { all.clone() } else { sky.planets.iter().map(|w| (c.planets.get(&w.kind).unwrap(), w.weight)).collect() };
        let hot = s.star.map(|_| Vec2::new(W + 100.0, H / 2.0));
        let planets = roll_planets(&mut Stream::new(seed, "planet"), &s, &pool, &all, hot, over);
        let moons = roll_moons(&mut Stream::new(seed, "moons"), &s, c, &planets, hot);
        (planets, moons)
    }

    /// Every sky rolls two or three planets of different kinds of its pool, at most one large, their parallax between
    /// the stars' and the moons', and moons of different kinds inside the design frame.
    #[test]
    fn planets_and_moons_never_repeat_a_kind() {
        let c = data::base_content();
        for sky in skies(&c) {
            for seed in 0..200u64 {
                let (planets, moons) = roll(&c, &sky, seed, 2500.0);
                assert!((2..=3).contains(&planets.len()), "seed {seed}: {} planets", planets.len());
                let mut kinds: Vec<&String> = planets.iter().map(|p| &p.kind).collect();
                kinds.sort();
                kinds.dedup();
                assert_eq!(kinds.len(), planets.len(), "seed {seed}: a planet kind twice");
                assert!(planets.iter().filter(|p| p.body.r > 0.15 * H).count() <= 1);
                for p in &planets {
                    assert!(p.parallax > 0.017 && p.parallax * MOON_DEPTH < 0.3 && p.anchor.length() < 2500.0);
                }
                let mut ids: Vec<&String> = moons.iter().map(|m| &m.2).collect();
                ids.sort();
                ids.dedup();
                assert_eq!(ids.len(), moons.len(), "seed {seed}: a moon kind twice");
                for (m, _, _) in &moons {
                    assert!(m.at.x > 0.0 && m.at.x < W && m.at.y > 0.0 && m.at.y < H);
                }
            }
        }
    }

    /// Some planet is in a 1280 × 720 view at the starting zoom from most of the cluster: camera places sampled
    /// evenly over clusters of 2–3 km (the cluster types' radii; 4 km at most) and over many seeds.
    #[test]
    fn a_planet_is_in_view_from_most_of_the_cluster() {
        let c = data::base_content();
        let (view, s) = (Vec2::new(1280.0, 720.0), 720.0 / H);
        for radius in [2000.0, 2500.0, 3000.0, 4000.0] {
            let (mut seen, mut all) = (0usize, 0usize);
            for sky in skies(&c) {
                for seed in 0..100u64 {
                    let (planets, _) = roll(&c, &sky, seed, radius);
                    let mut r = Stream::new(seed, "cameras");
                    for _ in 0..200 {
                        let cam = Vec2::from_angle(r.range(0.0, TAU)) * radius * r.next().sqrt();
                        all += 1;
                        // at the starting zoom a layer's offset is its share of the camera's offset times 18 px a metre
                        seen += usize::from(planets.iter().any(|p| {
                            let off = ((p.anchor - cam) * p.parallax * REF_ZOOM).abs();
                            let reach = p.body.r * s * if p.body.look.ring.is_some() { 1.6 } else { 1.0 };
                            off.x < view.x / 2.0 + reach && off.y < view.y / 2.0 + reach
                        }));
                    }
                }
            }
            let share = seen as f32 / all as f32;
            println!("radius {radius} m: a planet in view from {:.0} % of the cluster", share * 100.0);
            if radius <= 3000.0 {
                assert!(share > 0.6, "radius {radius}: {share}");
            }
        }
    }

    /// The vertices of the planets and moons together, at most and on average, over skies of three planets.
    #[test]
    fn three_planets_stay_within_budget() {
        let c = data::base_content();
        let paint = Paint { night: Vec3::splat(0.02), veil: Vec3::splat(0.03), veil_share: 1.0 - STRENGTH };
        let (mut most, mut sum, mut n) = (0usize, 0usize, 0usize);
        for sky in skies(&c) {
            for seed in 0..100u64 {
                let (planets, moons) = roll(&c, &sky, seed, 2500.0);
                if planets.len() < 3 {
                    continue;
                }
                let count = |b: &Body| {
                    let p = paint::paint(b, &paint, Vec2::ZERO);
                    p.body.len() + p.ring_back.as_ref().map_or(0, |g| g.len()) + p.ring_front.as_ref().map_or(0, |g| g.len())
                };
                let v: usize = planets.iter().map(|p| count(&p.body)).sum::<usize>() + moons.iter().map(|m| count(&m.0)).sum::<usize>();
                most = most.max(v);
                sum += v;
                n += 1;
            }
        }
        println!("three planets and their moons: {} vertices on average, {most} at most", sum / n.max(1));
        assert!(most < 120_000);
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
