//! The sim's asteroid rocks (GDR-0104), drawn as they spawn. The sim's outline is a coarse convex hull (the collider);
//! the view follows it closely but finer (17 Oct 2026, Niklas: "too low poly"): the hull's corners rounded a little
//! (two corner cuts, staying inside it), resampled to 32–96 points by size and roughened along the normal by a few
//! smooth harmonics (at most about 5 % of the radius), so hits land on what is drawn. Over the fill in the kind's tone
//! (`world.rock.*`) a lighter body shifted towards the light leaves a shadowed rim on the far side, craters (a dark
//! bowl, its inner wall lit in the facet tone), and the edge line, brighter on the sides facing the light. Each rock's look rolls
//! from the voyage's seed and its index (the same rock looks the same every time).
//! A rock never moves (an anchored body): the rocks that spawn together (a field, a piece of a belt) are one mesh in
//! world coordinates, drawn in one call.

use std::f32::consts::TAU;

use bevy::prelude::*;
use kit::atlas::Atlas;
use kit::meshbuf::MeshBuf;
use kit::rng::Rng;
use kit::style::vcol;

use super::{Scenery, SceneryPart, salted};
use crate::sim_bridge::{SimWorld, v2};

/// Rocks lie under the trails and every ship (z 5 and up).
const Z_ROCKS: f32 = 3.0;
/// The edge line's half width (m).
const EDGE: f32 = 0.2;
/// Where the light comes from (world, unit): the lit edge faces it.
const LIGHT: Vec2 = Vec2::new(-0.6, 0.8);
/// Outline points a metre of radius, and their least and most.
const POINTS_PER_M: f32 = 4.0;
const POINTS: (usize, usize) = (32, 96);
/// The corner cuts (share of an edge cut off at each end, one per pass).
const CUTS: [f32; 2] = [0.1, 0.12];
/// The edge's harmonics along the normal: how many waves round the rock and their size (share of the radius).
const WAVES: [(f32, f32); 3] = [(3.0, 0.028), (7.0, 0.016), (13.0, 0.008)];
/// The lit body: its size (share of the outline) and shift towards the light (share of the radius).
const BODY: (f32, f32) = (0.84, 0.09);
/// Craters: one per this many metres of radius (at least one, at most [`MAX_CRATERS`]), their radius (share of the
/// rock's) and how far from the centre they lie (share of the radius).
const CRATER_EVERY: f32 = 4.5;
const MAX_CRATERS: usize = 5;
const CRATER_SIZE: (f32, f32) = (0.07, 0.17);
const CRATER_REACH: f32 = 0.55;

/// Draws the rocks spawned since the last frame.
pub(super) fn draw_rocks(
    mut commands: Commands,
    mut state: ResMut<Scenery>,
    world: Res<SimWorld>,
    atlas: Res<Atlas>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(v) = world.sim.voyage().filter(|_| state.key.is_some()) else { return };
    let rocks = v.rocks();
    if state.rocks_drawn >= rocks.len() {
        return;
    }
    let seed = state.spec.as_ref().map_or(1, |s| salted(s.seed, 6));
    let white = atlas.region("fx/white");
    let uv = white.uv_center();
    let edge = atlas.token_color("world.rock_edge");
    let facet = atlas.token_color("world.rock_facet");
    let lit_edge = edge.mix(&Color::WHITE, 0.25);
    let mut m = MeshBuf::default();
    for (i, r) in rocks.iter().enumerate().skip(state.rocks_drawn) {
        let tone = world.content.rocks.get(&r.kind).map_or("iron", |k| k.tone.as_str());
        let fill = atlas.token_color(&format!("world.rock.{tone}"));
        let mut rng = Rng::new(seed ^ (i as u32).wrapping_mul(0x9e37_79b9) | 1);
        let (pos, turn) = (v2(r.pos), Vec2::from_angle(r.angle));
        let hull: Vec<Vec2> = r.points.iter().map(|&p| turn.rotate(v2(p))).collect();
        let pts: Vec<Vec2> = outline(&hull, r.radius, &mut rng).into_iter().map(|p| pos + p).collect();
        m.fan(pos, &pts, uv, vcol(fill, 1.0));
        // The lit body, leaving a shadowed rim away from the light.
        let shift = LIGHT * r.radius * BODY.1;
        let body: Vec<Vec2> = pts.iter().map(|&p| pos + (p - pos) * BODY.0 + shift).collect();
        m.fan(pos + shift, &body, uv, vcol(fill.mix(&Color::WHITE, 0.1), 1.0));
        // Craters: a dark bowl, its inner wall lit on the side away from the light.
        let n = ((r.radius / CRATER_EVERY) as usize).clamp(1, MAX_CRATERS);
        for _ in 0..n {
            let c = pos + Vec2::from_angle(rng.range(0.0, TAU)) * r.radius * CRATER_REACH * rng.next_f32().sqrt();
            let cr = r.radius * rng.range(CRATER_SIZE.0, CRATER_SIZE.1);
            let rim: Vec<Vec2> = (0..16).map(|k| c + Vec2::from_angle(k as f32 / 16.0 * TAU) * cr).collect();
            m.fan(c, &rim, uv, vcol(fill.mix(&Color::BLACK, 0.4), 0.9));
            let away = (-LIGHT).to_angle();
            let wall: Vec<Vec2> = (0..=8).map(|k| c + Vec2::from_angle(away + (k as f32 / 8.0 - 0.5) * 2.2) * cr).collect();
            m.strip(&wall, false, (cr * 0.06).max(0.06), &white, vcol(facet, 0.9));
        }
        m.strip(&pts, true, EDGE, &white, vcol(edge, 0.55));
        // The lit edge: the sides facing the light, brighter.
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            if (a - b).perp().normalize_or_zero().dot(LIGHT) > 0.3 {
                m.strip(&[a, b], false, EDGE, &white, vcol(lit_edge, 0.8));
            }
        }
    }
    commands.spawn((SceneryPart, Mesh2d(meshes.add(m.into_mesh())), MeshMaterial2d(atlas.material.clone()), Transform::from_xyz(0.0, 0.0, Z_ROCKS)));
    state.rocks_drawn = rocks.len();
}

/// A rock's drawn outline round its centre from its hull (counter-clockwise): corners cut, resampled evenly, then
/// roughened along the normal (see the module doc).
fn outline(hull: &[Vec2], radius: f32, rng: &mut Rng) -> Vec<Vec2> {
    let mut p = hull.to_vec();
    for cut in CUTS {
        p = p.iter().zip(p.iter().cycle().skip(1)).flat_map(|(&a, &b)| [a.lerp(b, cut), a.lerp(b, 1.0 - cut)]).collect();
    }
    let n = ((radius * POINTS_PER_M) as usize).clamp(POINTS.0, POINTS.1);
    let p = resample(&p, n);
    let waves: Vec<(f32, f32, f32)> = WAVES.iter().map(|&(f, a)| (f, a * radius, rng.range(0.0, TAU))).collect();
    p.iter()
        .enumerate()
        .map(|(k, &q)| {
            let t = k as f32 / n as f32 * TAU;
            let d: f32 = waves.iter().map(|&(f, a, ph)| a * (f * t + ph).sin()).sum();
            q + q.normalize_or_zero() * d
        })
        .collect()
}

/// `n` points evenly along a closed polyline.
fn resample(p: &[Vec2], n: usize) -> Vec<Vec2> {
    let len: f32 = p.iter().zip(p.iter().cycle().skip(1)).map(|(a, b)| a.distance(*b)).sum();
    let step = len / n as f32;
    let mut out = Vec::with_capacity(n);
    let (mut k, mut walked) = (0, 0.0);
    for j in 0..n {
        let want = j as f32 * step;
        loop {
            let (a, b) = (p[k % p.len()], p[(k + 1) % p.len()]);
            let l = a.distance(b);
            if walked + l >= want || k + 1 >= p.len() {
                out.push(a.lerp(b, if l > 0.0 { ((want - walked) / l).clamp(0.0, 1.0) } else { 0.0 }));
                break;
            }
            walked += l;
            k += 1;
        }
    }
    out
}
