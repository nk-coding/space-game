//! The sky behind a cluster (GDR-0105): one to four planets (by the cluster's size) (each whole, one at most large), their moons, a nebula
//! and a far galaxy, rolled from the cluster's seed on named streams (one per layer) from its type's or its band's
//! `sky` and the planet kinds (`planets.ron`), built once per voyage as vertex-coloured meshes and drawn far behind
//! the play; at home the home band's nebula and galaxy alone, fainter. Client-only: the sim never sees it.
//!
//! Composed in a design frame of 1600 × 900 px (the study's), scaled to the view's height: the galaxy kept at its
//! distance from the view's left or right edge, the nebula stretched to the view's width, each planet and its moons
//! round the view's middle. Every piece also hangs at an anchor in the cluster on a layer of its own parallax like
//! the rest of the scenery (`camera + (anchor − camera) · p · depth_scale(p)`, GDR-0107; true parallax since
//! Niklas's look of 18 Oct 2026, no cap, so a planet leaves the view): the galaxy 0, the nebula 0.003, the planets
//! 0.018–0.046 (by the cluster's size, [`depth`]; within a sky the bigger the nearer and faster) and their moons a
//! tenth more, above the stars (0.005–0.017) and below the junk (0.3 and up, `backdrop.rs`, `filler.rs`). A planet's
//! anchor is where the camera sees it in the view's middle: the first so the entry's view shows it off its middle,
//! the others spread over the cluster about a view apart, never meeting on screen from anywhere the camera goes
//! ([`clear`]), so mostly exactly one planet is in view ([`anchor_planets`]). The planets and moons draw by their parallax, the nearer over the
//! farther. Everything is drawn at [`STRENGTH`] (veiled towards the sky's dark), so it stays quiet behind a fight.

mod kinds;
mod nebula;
mod paint;
mod roll;

use std::f32::consts::{PI, TAU};

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
/// Each layer's share of the camera's motion: the nebula's, and the planets', round which each planet's lies (from
/// [`PLANET_SPREAD`] below to as far above, by its size). The planets' is set per sky by the cluster's size
/// ([`depth`]) between the least (above the nearest stars, 0.017, at its spread's low end) and the most (a planet
/// grows by under an eighth zoomed all the way in).
const PARALLAX_NEBULA: f32 = 0.003;
const PARALLAX_PLANET: (f32, f32) = (0.021, 0.04);
const PLANET_SPREAD: f32 = 0.15;
/// The planets aim at a view of 1280 × 720 px at the starting zoom: the camera's reach on a planets' layer is a disc
/// of the cluster's radius × parallax × [`REF_ZOOM`] px, and the sky takes as many planets (at most [`MOST_PLANETS`])
/// as views of that size fill [`FILL`] of the disc at a parallax no more than the most; fewer in a small cluster.
const VIEW: Vec2 = Vec2::new(1280.0, 720.0);
const MOST_PLANETS: usize = 4;
const FILL: f32 = 0.9;
/// The first planet's place in the entry's view off its middle (shares of [`VIEW`], to a side, clear of the ship
/// and of the HUD's columns, and level or towards the cluster's centre); the least distance between two planets' places on screen (px); the candidate anchors tried for
/// each other planet and the camera places they are judged from.
const FIRST_OFF: [(f32, f32); 2] = [(0.16, 0.26), (0.0, 0.16)];
const APART: f32 = 900.0;
const CANDIDATES: usize = 64;
const CAMERAS: usize = 128;
/// A moon's parallax over its planet's: a tenth more, so it stays by its planet as the camera moves (a third more
/// swung it hundreds of px off; 18 Oct 2026).
const MOON_DEPTH: f32 = 1.1;
/// A planet's radius as a share of the view's height: small, or (one at most) large; distant (Niklas, 18 Oct 2026:
/// a large planet of 0.22–0.34 filled half the view at Corran Reach's entry).
const SMALL: (f32, f32) = (0.05, 0.09);
const LARGE: (f32, f32) = (0.10, 0.12);
/// The clearance between planets: how far past the cluster's radius the camera may go, the view heights checked
/// (as scales of the design frame: 720 to 1125 px) and the least gap between two bodies on screen (px).
const CAMERA_REACH: f32 = 1.15;
const SCALES: [f32; 3] = [0.8, 1.0, 1.25];
const GAP: f32 = 24.0;
/// Draw order: the galaxy over the gradient, the far stars (−893, `backdrop.rs`), the nebula, then the planets and
/// moons by their parallax (`Z_BODIES + parallax · DEPTH_Z`, −885 to −884), a planet's rings' back half just under
/// it and the front half just over it. The hot star's glow (−889), its corona and the landmarks (−888.5) lie between
/// the nebula and the planets: nothing farther off draws over a planet (GDR-0107).
const Z_GALAXY: f32 = -897.0;
const Z_NEBULA: f32 = -890.0;
const Z_BODIES: f32 = -887.0;
const DEPTH_Z: f32 = 100.0;

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
    /// Where the first planet's light comes from (angle, design frame, y down): what lights the rocks.
    pub light: Option<f32>,
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
    let (planets, moons) = match s.over {
        Some(over) => {
            let all: Vec<(&PlanetKind, u32)> = content.planets.iter().filter(|k| k.body == BodyClass::Planet).map(|k| (k, 1)).collect();
            let pool: Vec<(&PlanetKind, u32)> =
                if s.sky.planets.is_empty() { all.clone() } else { s.sky.planets.iter().filter_map(|w| Some((content.planets.get(&w.kind)?, w.weight))).collect() };
            if pool.is_empty() {
                return None;
            }
            let planets = roll_planets(&mut Stream::new(s.seed, "planet"), s, &pool, &all, hot, depth(over.radius));
            let moons = roll_moons(&mut Stream::new(s.seed, "moons"), s, content, &planets, hot);
            anchor_planets(&mut Stream::new(s.seed, "anchors"), planets, moons, over)
        }
        None => (Vec::new(), Vec::new()),
    };
    let nebula = nebula::nebula(&mut Stream::new(s.seed, "nebula"), s.sky, night, hot.is_some());
    let galaxy = nebula::galaxy(&mut Stream::new(s.seed, "galaxy"), s.sky.galaxy);

    let light = planets.first().map(|p| p.body.light);
    let mut counts = SkyCounts { planets: Vec::new(), moons: Vec::new(), vertices: [0; 4], nebula_peak: nebula.peak, light };
    let put = |cx: &mut Build, g: paint::Geo, part: SkyPart, z: f32| -> usize {
        let n = g.len();
        cx.commands.spawn((SceneryPart, part, Mesh2d(cx.meshes.add(g.into_mesh())), MeshMaterial2d(cx.atlas.material.clone()), Transform::from_xyz(0.0, 0.0, z)));
        n
    };
    for (i, p) in planets.iter().enumerate() {
        let painted = paint::paint(&p.body, &paint, white);
        let part = || SkyPart { at: p.body.at, edge: Edge::Mid, anchor: p.anchor, parallax: p.parallax };
        let z = depth_z(p.parallax, i);
        counts.vertices[0] += put(cx, painted.body, part(), z);
        if let Some(g) = painted.ring_back {
            counts.vertices[0] += put(cx, g, part(), z - 4e-4);
        }
        if let Some(g) = painted.ring_front {
            counts.vertices[0] += put(cx, g, part(), z + 4e-4);
        }
        counts.planets.push(format!("{} (r {:.0} px, p {:.3}{})", p.kind, p.body.r, p.parallax, if p.body.look.ring.is_some() { ", ringed" } else { "" }));
    }
    for (m, of, name) in &moons {
        let painted = paint::paint(m, &paint, white);
        let p = &planets[*of];
        let part = SkyPart { at: m.at, edge: Edge::Mid, anchor: p.anchor, parallax: p.parallax * MOON_DEPTH };
        counts.vertices[1] += put(cx, painted.body, part, depth_z(p.parallax * MOON_DEPTH, *of));
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

/// The z of a planet or moon of parallax `parallax` (the nearer over the farther), the `i`th planet's a hair over
/// an earlier one's of the same parallax.
fn depth_z(parallax: f32, i: usize) -> f32 {
    Z_BODIES + parallax * DEPTH_Z + i as f32 * 1e-3
}

/// How many planets a cluster of `radius` takes and the parallax round which theirs lie ([`VIEW`]).
fn depth(radius: f32) -> (usize, f32) {
    let (least, most) = PARALLAX_PLANET;
    (1..=MOST_PLANETS)
        .rev()
        .map(|n| (n, (n as f32 * VIEW.x * VIEW.y / PI).sqrt() * FILL / (radius.max(1.0) * REF_ZOOM)))
        .find(|&(n, p)| p <= most || n == 1)
        .map_or((1, most), |(n, p)| (n, p.clamp(least, most)))
}

/// Up to `n` planets of different kinds by weight (the first `?planet=`'s if given), each in the design frame's
/// middle, [`SMALL`] or at most one [`LARGE`] (25 %), its parallax round `parallax` by its size; lit from the hot
/// star, from behind (a backlit sky) or from a side. Their anchors come after their moons ([`anchor_planets`]).
fn roll_planets(r: &mut Stream, s: &SkySpec, pool: &[(&PlanetKind, u32)], all: &[(&PlanetKind, u32)], hot: Option<Vec2>, (n, parallax): (usize, f32)) -> Vec<Planet> {
    let mid = Vec2::new(W, H) / 2.0;
    let mut pool = pool.to_vec();
    let mut big_left = true;
    let mut out = Vec::new();
    for i in 0..n {
        let weights: Vec<u32> = pool.iter().map(|p| p.1).collect();
        let rolled = pool[r.weighted(&weights)].0;
        let kind = s.planet.filter(|_| i == 0).and_then(|id| all.iter().find(|k| k.0.id == id)).map_or(rolled, |k| k.0);
        pool.retain(|p| p.0.id != kind.id);
        let big = big_left && r.chance(0.25);
        big_left &= !big;
        let mut rad = if big { r.range(LARGE.0, LARGE.1) } else { r.range(SMALL.0, SMALL.1) } * H;
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
        let size = ((rad / H - SMALL.0) / (LARGE.1 - SMALL.0)).clamp(0.0, 1.0);
        let parallax = parallax * (1.0 - PLANET_SPREAD + 2.0 * PLANET_SPREAD * size);
        let mut body = Body { r: rad, at: mid, light, phase, tilt, moon: false, look };
        aim(&mut body);
        out.push(Planet { body, kind: kind.id.clone(), anchor: Vec2::ZERO, parallax });
        if pool.is_empty() {
            break;
        }
    }
    out
}

/// Anchors the planets so that about one is in a [`VIEW`] at the starting zoom from anywhere in the cluster: the
/// first in the entry's view off its middle, to a side ([`FIRST_OFF`]); each other at
/// the best of [`CANDIDATES`] anchors over the cluster at least [`APART`] on screen from every planet before it and
/// [`clear`] of them, judged by [`view_score`] from [`CAMERAS`] camera places, and only if it raises the score; the
/// planets left over are left out with their moons.
fn anchor_planets(r: &mut Stream, planets: Vec<Planet>, moons: Vec<(Body, usize, String)>, over: Over) -> (Vec<Planet>, Vec<(Body, usize, String)>) {
    let bodies: Vec<Vec<Disc>> = planets.iter().enumerate().map(|(i, p)| discs(p, &moons, i)).collect();
    let cams: Vec<Vec2> = (0..CAMERAS).map(|_| in_disc(r, over.radius)).collect();
    let inward = -over.entry.normalize_or(Vec2::NEG_Y);
    let mut side = |d: f32, (a, b): (f32, f32)| {
        let sign = if d.abs() > 0.3 { d.signum() } else if r.chance(0.5) { 1.0 } else { -1.0 };
        sign * r.range(a, b)
    };
    let off = Vec2::new(side(inward.x, FIRST_OFF[0]), side(inward.y, FIRST_OFF[1])) * VIEW;
    let mut kept: Vec<(Planet, usize)> = Vec::new();
    let mut best = i32::MIN;
    for (i, mut p) in planets.into_iter().enumerate() {
        let par = p.parallax;
        let looks = |kept: &[(Planet, usize)], anchor: Vec2| {
            let others = kept.iter().map(|(q, j)| (q.anchor, q.parallax, reach(&bodies[*j])));
            view_score(others.chain(std::iter::once((anchor, par, reach(&bodies[i])))), &cams)
        };
        if kept.is_empty() {
            p.anchor = over.entry + off / (par * REF_ZOOM);
            best = looks(&kept, p.anchor);
            kept.push((p, i));
            continue;
        }
        let pick = (0..CANDIDATES)
            .map(|_| in_disc(r, over.radius))
            .filter(|&a| {
                kept.iter().all(|(q, j)| {
                    (a * par - q.anchor * q.parallax).length() * REF_ZOOM >= APART && clear((a, &bodies[i]), (q.anchor, &bodies[*j]), over.radius)
                })
            })
            .map(|a| (looks(&kept, a), a))
            .max_by_key(|c| c.0);
        if let Some((score, anchor)) = pick.filter(|c| c.0 > best) {
            best = score;
            p.anchor = anchor;
            kept.push((p, i));
        }
    }
    let moons = moons.into_iter().filter_map(|(m, of, id)| Some((m, kept.iter().position(|(_, i)| *i == of)?, id))).collect();
    (kept.into_iter().map(|(p, _)| p).collect(), moons)
}

/// A place in the disc of `radius`, evenly over its area.
fn in_disc(r: &mut Stream, radius: f32) -> Vec2 {
    Vec2::from_angle(r.range(0.0, TAU)) * radius * r.next().sqrt()
}

/// How far a planet's discs reach from its middle on a 720 px high view (px).
fn reach(discs: &[Disc]) -> f32 {
    discs.iter().fold(0.0_f32, |m, &(o, r, _)| m.max(o.length() + r)) * VIEW.y / H
}

/// Is a planet at `anchor` of parallax `parallax` reaching `reach` px in a [`VIEW`] round `cam` at the starting zoom?
fn in_view(anchor: Vec2, parallax: f32, reach: f32, cam: Vec2) -> bool {
    let off = ((anchor - cam) * parallax * REF_ZOOM).abs();
    off.x < VIEW.x / 2.0 + reach && off.y < VIEW.y / 2.0 + reach
}

/// How well planets (anchor, parallax, reach) fill the sky: the camera places of `cams` that see exactly one, less
/// four for each that sees three or more.
fn view_score(planets: impl Iterator<Item = (Vec2, f32, f32)> + Clone, cams: &[Vec2]) -> i32 {
    cams.iter()
        .map(|&c| match planets.clone().filter(|&(a, p, rc)| in_view(a, p, rc, c)).count() {
            1 => 1,
            0 | 2 => 0,
            _ => -4,
        })
        .sum()
}

/// A planet or moon as the clearance sees it: its offset from its planet's anchor point (design px, y up), its
/// radius (a ringed planet's out to its rings) and its parallax.
type Disc = (Vec2, f32, f32);

/// The `i`th planet's discs: itself and its moons.
fn discs(p: &Planet, moons: &[(Body, usize, String)], i: usize) -> Vec<Disc> {
    let ring = p.body.look.ring.as_ref().map_or(1.0, |g| g.subs.iter().fold(1.0_f32, |a, s| a.max(s.1)));
    let own = moons.iter().filter(|m| m.1 == i).map(|(m, _, _)| (Vec2::new(m.at.x - p.body.at.x, p.body.at.y - m.at.y), m.r, p.parallax * MOON_DEPTH));
    std::iter::once((Vec2::ZERO, p.body.r * ring, p.parallax)).chain(own).collect()
}

/// Can two planets, each at its anchor with its discs, never meet on screen? A disc lies at `offset · s + (anchor −
/// camera) · parallax · REF_ZOOM` px from the view's middle at the starting zoom (other zooms scale both alike, by
/// `zoom^parallax`), so two discs come at least `|fixed part| − camera reach · |Δparallax| · REF_ZOOM` close for any
/// camera in the cluster; that must leave [`GAP`] between them at every scale of [`SCALES`].
fn clear((a, ours): (Vec2, &[Disc]), (b, theirs): (Vec2, &[Disc]), radius: f32) -> bool {
    ours.iter().all(|&(o, r, p)| {
        theirs.iter().all(|&(o2, r2, p2)| {
            SCALES.iter().all(|&s| {
                let fixed = (o - o2) * s + (a * p - b * p2) * REF_ZOOM;
                fixed.length() - radius * CAMERA_REACH * (p - p2).abs() * REF_ZOOM >= (r + r2) * s + GAP
            })
        })
    })
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
                let mr = (p.r * r.range(0.08, 0.2)).clamp(4.0, 24.0);
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
    use crate::camera::{MAX_PX_PER_M, MIN_PX_PER_M};

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
        let planets = roll_planets(&mut Stream::new(seed, "planet"), &s, &pool, &all, hot, depth(radius));
        let moons = roll_moons(&mut Stream::new(seed, "moons"), &s, c, &planets, hot);
        anchor_planets(&mut Stream::new(seed, "anchors"), planets, moons, over)
    }

    /// Every sky rolls one to four planets of different kinds of its pool, at most one large, their parallax between
    /// the stars' and the moons', and moons of different kinds inside the design frame.
    #[test]
    fn planets_and_moons_never_repeat_a_kind() {
        let c = data::base_content();
        for sky in skies(&c) {
            for seed in 0..200u64 {
                let (planets, moons) = roll(&c, &sky, seed, 2500.0);
                assert!((1..=MOST_PLANETS).contains(&planets.len()), "seed {seed}: {} planets", planets.len());
                let mut kinds: Vec<&String> = planets.iter().map(|p| &p.kind).collect();
                kinds.sort();
                kinds.dedup();
                assert_eq!(kinds.len(), planets.len(), "seed {seed}: a planet kind twice");
                assert!(planets.iter().filter(|p| p.body.r > SMALL.1 * H).count() <= 1);
                for p in &planets {
                    assert!(p.body.r <= LARGE.1 * H && p.parallax > 0.017 && p.parallax * MOON_DEPTH < 0.3);
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

    /// How many planets a 1280 × 720 view at the starting zoom shows, over camera places sampled evenly over clusters
    /// of 1–4 km (Corran Reach is 1 km, the cluster types 2–3 km) and many seeds: mostly exactly one, three or more
    /// hardly ever.
    #[test]
    fn mostly_one_planet_is_in_view() {
        let c = data::base_content();
        for radius in [1000.0, 2000.0, 2500.0, 3000.0, 4000.0] {
            let (mut seen, mut all) = ([0usize; MOST_PLANETS + 1], 0usize);
            for sky in skies(&c) {
                for seed in 0..100u64 {
                    let (planets, moons) = roll(&c, &sky, seed, radius);
                    let at: Vec<(Vec2, f32, f32)> = planets.iter().enumerate().map(|(i, p)| (p.anchor, p.parallax, reach(&discs(p, &moons, i)))).collect();
                    let mut r = Stream::new(seed, "cameras");
                    for _ in 0..200 {
                        let cam = in_disc(&mut r, radius);
                        all += 1;
                        seen[at.iter().filter(|&&(a, p, rc)| in_view(a, p, rc, cam)).count()] += 1;
                    }
                }
            }
            let share = |n: &[usize]| n.iter().sum::<usize>() as f32 / all as f32;
            let (one, any, three) = (share(&seen[1..2]), share(&seen[1..]), share(&seen[3..]));
            println!("radius {radius} m: exactly one planet in view from {:.0} %, one or more {:.0} %, three or more {:.1} %", one * 100.0, any * 100.0, three * 100.0);
            assert!(three < 0.005, "radius {radius}: three planets in view from {three}");
            if radius <= 3000.0 {
                assert!(one > 0.6, "radius {radius}: one planet in view from {one}");
            }
        }
    }

    /// The first planet is in the entry's view but off its middle, to a side; the planets lie at
    /// least [`APART`] apart on screen; a small cluster keeps fewer of them.
    #[test]
    fn planets_spread_a_view_apart() {
        let c = data::base_content();
        for (radius, most) in [(600.0, 1), (1000.0, 2), (2500.0, MOST_PLANETS)] {
            for sky in skies(&c) {
                for seed in 0..60u64 {
                    let (planets, _) = roll(&c, &sky, seed, radius);
                    assert!(planets.len() <= most, "radius {radius}, seed {seed}: {} planets", planets.len());
                    let first = (planets[0].anchor - Vec2::new(0.0, -radius)) * planets[0].parallax * REF_ZOOM;
                    assert!(first.x.abs() >= 0.16 * VIEW.x && first.x.abs() <= 0.26 * VIEW.x && first.y >= 0.0 && first.y <= 0.16 * VIEW.y, "seed {seed}: {first}");
                    for (i, p) in planets.iter().enumerate() {
                        for q in &planets[i + 1..] {
                            assert!((p.anchor * p.parallax - q.anchor * q.parallax).length() * REF_ZOOM >= APART);
                        }
                    }
                }
            }
        }
    }

    /// No two planets meet on screen, rings and moons included: cameras sampled over the cluster and a little past
    /// its edge, at the smallest and the largest zoom, at 720 and 1125 px high views, over many seeds and cluster
    /// radii from the first voyage's 600 m to 4 km (Corran Reach is 1 km); and how many planets each keeps.
    #[test]
    fn planets_never_meet_on_screen() {
        let c = data::base_content();
        for radius in [600.0, 1000.0, 2500.0, 4000.0] {
            let mut counts = [0usize; MOST_PLANETS + 1];
            for sky in skies(&c) {
                for seed in 0..60u64 {
                    let (planets, moons) = roll(&c, &sky, seed, radius);
                    counts[planets.len()] += 1;
                    let discs: Vec<(Vec2, Vec<Disc>)> = planets.iter().enumerate().map(|(i, p)| (p.anchor, discs(p, &moons, i))).collect();
                    let mut r = Stream::new(seed, "cameras");
                    for _ in 0..60 {
                        let cam = Vec2::from_angle(r.range(0.0, TAU)) * radius * CAMERA_REACH * r.next().sqrt();
                        for (zoom, s) in [(MIN_PX_PER_M, 0.8), (MAX_PX_PER_M, 1.25)] {
                            let zr = zoom / REF_ZOOM;
                            let at = |a: Vec2, &(o, rad, p): &Disc| (o * s + (a - cam) * p * REF_ZOOM, rad * s, zr.powf(p));
                            for (i, (a, ours)) in discs.iter().enumerate() {
                                for (b, theirs) in &discs[i + 1..] {
                                    for d in ours {
                                        for e in theirs {
                                            let ((x, rx, gx), (y, ry, gy)) = (at(*a, d), at(*b, e));
                                            assert!((x * gx).distance(y * gy) > rx * gx + ry * gy, "radius {radius}, seed {seed}: two planets meet");
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            println!("radius {radius} m: skies of 0 / 1 / 2 / 3 / 4 planets: {counts:?}");
            assert_eq!(counts[0], 0, "radius {radius}: a sky without a planet");
            if radius >= 2000.0 {
                assert_eq!(counts[1], 0, "radius {radius}: a sky of one planet");
            }
        }
    }

    /// Zoomed all the way in or out, a planet's size on screen changes by `zoom^parallax`: under an eighth.
    #[test]
    fn zoom_hardly_changes_a_planet() {
        let most = PARALLAX_PLANET.1 * (1.0 + PLANET_SPREAD) * MOON_DEPTH;
        let (out, inn) = ((MIN_PX_PER_M / REF_ZOOM).powf(most), (MAX_PX_PER_M / REF_ZOOM).powf(most));
        println!("a planet's size zoomed out {out:.3}, zoomed in {inn:.3}");
        assert!(out > 0.875 && inn < 1.125);
    }

    /// The vertices of the planets and moons together, at most and on average, over skies of three or four planets.
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
        println!("three or four planets and their moons: {} vertices on average, {most} at most", sum / n.max(1));
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
