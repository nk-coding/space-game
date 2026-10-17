//! The filler between the sites (GDR-0104), drawn only:
//!
//! * **Glints and motes**: one tile of fine dust and a few twinkling glints on a near layer (parallax
//!   [`GLINT_PARALLAX`], between the backdrop's dust and the play), repeated 3 × 3 round the view like the
//!   backdrop's; as thick as the scenery's `dust`.
//! * **Debris**: bits of hull everywhere on a tiled layer just under the play (parallax [`DEBRIS_PARALLAX`], so ships
//!   pass over them; [`DEBRIS_BITS`] a tile at `debris` 1, so a few are always in sight), and thicker round each site
//!   in [`SITE_GROUPS`] drifting groups.
//! * **Derelicts**: the scenery's `derelicts` dead hull shells, broken in two or three, turning slowly as they drift
//!   along the way (between the entry and the sites), a little further below (parallax [`DERELICT_PARALLAX`]).

use bevy::prelude::*;
use kit::atlas::set_alpha;
use kit::meshbuf::MeshBuf;
use kit::rng::Rng;
use kit::style::vcol;

use super::{Build, Layered, SceneryPart, Spec, clock, salted, view};
use crate::camera::MainCamera;
use crate::sim_bridge::SimWorld;

const Z_GLINTS: f32 = -660.0;
const Z_DERELICTS: f32 = 1.0;
const Z_DEBRIS: f32 = 2.0;
/// The glints' layer: its share of the camera's motion, its tile (m) and copies a side.
const GLINT_PARALLAX: f32 = 0.85;
const GLINT_TILE: f32 = 260.0;
const COPIES: i32 = 3;
/// Motes and glints in a tile at `dust` 1.
const MOTES: f32 = 220.0;
const GLINTS: f32 = 18.0;
const DEBRIS_PARALLAX: f32 = 0.97;
const DERELICT_PARALLAX: f32 = 0.9;
/// Debris groups round each site at `debris` 1, their bits, and how far round the site they lie (m).
const SITE_GROUPS: f32 = 3.0;
const SITE_BITS: (f32, f32) = (10.0, 18.0);
const SITE_SPREAD: (f32, f32) = (40.0, 160.0);
/// The debris layer's tile (m) and its bits at `debris` 1.
const DEBRIS_TILE: f32 = 400.0;
const DEBRIS_BITS: f32 = 320.0;

/// A tile of a repeated layer (`slot` of the 3 × 3): the glints' or the debris'.
#[derive(Component)]
pub(super) struct Tile {
    slot: i32,
    size: f32,
    parallax: f32,
}

/// The glints' material, faded in a slow twinkle.
#[derive(Component)]
pub(super) struct Twinkle(Handle<kit::atlas::AtlasMaterial>);

/// What was spawned (for the log line).
pub(super) struct Counts {
    pub tiles: u32,
    pub motes: u32,
    pub groups: u32,
    pub bits: u32,
    pub derelicts: u32,
}

/// Spawns the glints, the debris and the derelicts of `spec`.
pub(super) fn spawn(cx: &mut Build, spec: &Spec) -> Counts {
    let s = &spec.scenery;
    let white = cx.atlas.region("fx/white");
    let hard = cx.atlas.region("fx/dot_hard");
    let soft = cx.atlas.region("fx/dot_soft");
    let mut counts = Counts { tiles: 0, motes: 0, groups: 0, bits: 0, derelicts: 0 };

    if s.dust > 0.0 {
        let mut rng = Rng::new(salted(spec.seed, 1));
        let (mut motes, mut glints) = (MeshBuf::default(), MeshBuf::default());
        let n = (MOTES * s.dust) as u32;
        for _ in 0..n {
            let at = Vec2::new(rng.range(0.0, GLINT_TILE), rng.range(0.0, GLINT_TILE));
            motes.region_at(&hard, at, Vec2::splat(rng.range(0.06, 0.14)), 0.0, vcol(cx.style.particle, rng.range(0.25, 0.6)));
        }
        for _ in 0..(GLINTS * s.dust) as u32 {
            let at = Vec2::new(rng.range(0.0, GLINT_TILE), rng.range(0.0, GLINT_TILE));
            let size = rng.range(0.1, 0.2);
            glints.region_at(&hard, at, Vec2::splat(size), 0.0, vcol(cx.style.star, 0.9));
            glints.region_at(&soft, at, Vec2::splat(size * 9.0), 0.0, vcol(cx.style.star, 0.18));
        }
        let (motes, glints) = (cx.meshes.add(motes.into_mesh()), cx.meshes.add(glints.into_mesh()));
        let twinkle = cx.atlas.group_material(cx.materials);
        for slot in 0..COPIES * COPIES {
            cx.commands
                .spawn((SceneryPart, Tile { slot, size: GLINT_TILE, parallax: GLINT_PARALLAX }, Mesh2d(motes.clone()), MeshMaterial2d(cx.atlas.material.clone()), Transform::from_xyz(0.0, 0.0, Z_GLINTS)))
                .with_child((Mesh2d(glints.clone()), MeshMaterial2d(twinkle.clone()), Transform::from_xyz(0.0, 0.0, 0.5)));
        }
        cx.commands.spawn((SceneryPart, Twinkle(twinkle)));
        counts.tiles = (COPIES * COPIES) as u32;
        counts.motes = n;
    }

    if s.debris > 0.0 {
        let mut rng = Rng::new(salted(spec.seed, 2));
        let tone = cx.atlas.token_color("world.debris");
        // The layer everywhere.
        let mut m = MeshBuf::default();
        let n = (DEBRIS_BITS * s.debris) as u32;
        for _ in 0..n {
            let c = Vec2::new(rng.range(0.0, DEBRIS_TILE), rng.range(0.0, DEBRIS_TILE));
            bit(&mut m, &mut rng, c, &white, tone);
        }
        let mesh = cx.meshes.add(m.into_mesh());
        for slot in 0..COPIES * COPIES {
            cx.commands.spawn((
                SceneryPart,
                Tile { slot, size: DEBRIS_TILE, parallax: DEBRIS_PARALLAX },
                Mesh2d(mesh.clone()),
                MeshMaterial2d(cx.atlas.material.clone()),
                Transform::from_xyz(0.0, 0.0, Z_DEBRIS),
            ));
        }
        counts.bits += n;
        // Thicker round the sites, drifting.
        let mut groups: Vec<(Vec2, f32, u32)> = Vec::new();
        for &site in &spec.sites {
            for _ in 0..(SITE_GROUPS * s.debris).round() as u32 {
                let at = site + Vec2::from_angle(rng.range(-3.15, 3.15)) * rng.range(SITE_SPREAD.0, SITE_SPREAD.1);
                groups.push((at, rng.range(18.0, 40.0), rng.range(SITE_BITS.0, SITE_BITS.1) as u32));
            }
        }
        for (at, spread, bits) in groups {
            let mut m = MeshBuf::default();
            for _ in 0..bits {
                let c = Vec2::from_angle(rng.range(-3.15, 3.15)) * spread * rng.next_f32();
                bit(&mut m, &mut rng, c, &white, tone);
            }
            let drift = Vec2::from_angle(rng.range(-3.15, 3.15)) * rng.range(0.1, 0.6);
            cx.commands.spawn((
                SceneryPart,
                Layered { anchor: at, drift, angle: 0.0, spin: 0.0, parallax: DEBRIS_PARALLAX },
                Mesh2d(cx.meshes.add(m.into_mesh())),
                MeshMaterial2d(cx.atlas.material.clone()),
                Transform::from_xyz(at.x, at.y, Z_DEBRIS),
            ));
            counts.groups += 1;
            counts.bits += bits;
        }
    }

    let (lo, hi) = s.derelicts;
    if hi > 0 {
        let mut rng = Rng::new(salted(spec.seed, 3));
        let n = lo + ((hi - lo + 1) as f32 * rng.next_f32()) as u32;
        let fill = cx.atlas.token_color("world.derelict");
        let edge = cx.atlas.token_color("world.derelict_edge");
        let stops: Vec<Vec2> = std::iter::once(spec.entry).chain(spec.sites.iter().copied()).collect();
        for _ in 0..n {
            // Along the way between two stops (the entry and the sites), off to one side.
            let a = stops[(rng.next_f32() * stops.len() as f32) as usize % stops.len()];
            let b = stops[(rng.next_f32() * stops.len() as f32) as usize % stops.len()];
            let along = a.lerp(b, rng.range(0.2, 0.8));
            let side = (b - a).perp().normalize_or(Vec2::X) * rng.range(20.0, 120.0) * if rng.next_f32() < 0.5 { -1.0 } else { 1.0 };
            let at = along + side;
            let mesh = derelict(&mut rng, &white, fill, edge);
            cx.commands.spawn((
                SceneryPart,
                Layered {
                    anchor: at,
                    drift: Vec2::from_angle(rng.range(-3.15, 3.15)) * rng.range(0.05, 0.3),
                    angle: rng.range(-3.15, 3.15),
                    spin: rng.range(-0.03, 0.03),
                    parallax: DERELICT_PARALLAX,
                },
                Mesh2d(cx.meshes.add(mesh)),
                MeshMaterial2d(cx.atlas.material.clone()),
                Transform::from_xyz(at.x, at.y, Z_DERELICTS),
            ));
            counts.derelicts += 1;
        }
    }
    counts
}

/// A bit of hull 0.2–0.9 m across round `c`: three or four corners, a little see-through.
fn bit(m: &mut MeshBuf, rng: &mut Rng, c: Vec2, white: &kit::atlas::Region, tone: Color) {
    let size = rng.range(0.2, 0.9);
    let turn = rng.range(-3.15, 3.15);
    let corners = 3 + (rng.next_f32() * 2.0) as usize;
    let pts: Vec<Vec2> = (0..corners)
        .map(|k| c + Vec2::from_angle(turn + k as f32 * std::f32::consts::TAU / corners as f32 + rng.range(-0.3, 0.3)) * size * rng.range(0.6, 1.0))
        .collect();
    m.polygon(&pts, white.uv_center(), vcol(tone, rng.range(0.4, 0.75)));
}

/// A dead hull shell, 16–36 m long, broken into two or three blocks along its spine with gaps between: each block a
/// plate (the nose block tapered) with an edge, ribs across it and a dark hole or two.
fn derelict(rng: &mut Rng, white: &kit::atlas::Region, fill: Color, edge: Color) -> Mesh {
    let length = rng.range(16.0, 36.0);
    let width = length * rng.range(0.22, 0.34);
    let blocks = 2 + (rng.next_f32() * 2.0) as usize;
    let mut m = MeshBuf::default();
    let mut x = -length / 2.0;
    let share = length / blocks as f32;
    for k in 0..blocks {
        let len = share * rng.range(0.7, 0.92);
        let (x0, x1) = (x, x + len);
        let w = width * rng.range(0.8, 1.0) / 2.0;
        let lean = rng.range(-0.08, 0.08) * len;
        let mut pts = vec![Vec2::new(x0, -w), Vec2::new(x1, -w + lean), Vec2::new(x1, w + lean), Vec2::new(x0, w)];
        if k + 1 == blocks {
            // The nose: tapered.
            pts[1] = Vec2::new(x1 - len * 0.3, -w * 0.6 + lean);
            pts[2] = Vec2::new(x1, lean);
            pts.insert(3, Vec2::new(x1 - len * 0.3, w * 0.6 + lean));
        }
        m.polygon(&pts, white.uv_center(), vcol(fill, 0.95));
        m.strip(&pts, true, 0.18, white, vcol(edge, 0.7));
        for r in 1..(len / 3.5) as usize {
            let rx = x0 + r as f32 * 3.5;
            m.strip(&[Vec2::new(rx, -w * 0.8), Vec2::new(rx, w * 0.8)], false, 0.08, white, vcol(edge, 0.35));
        }
        if rng.next_f32() < 0.6 {
            let c = Vec2::new(rng.range(x0 + 1.0, x1 - 1.0), rng.range(-w * 0.4, w * 0.4));
            let r = w * rng.range(0.25, 0.45);
            let hole: Vec<Vec2> = (0..5).map(|j| c + Vec2::from_angle(j as f32 * 1.2566 + rng.range(-0.3, 0.3)) * r).collect();
            m.polygon(&hole, white.uv_center(), vcol(Color::BLACK, 0.7));
        }
        x = x1 + share - len;
    }
    m.into_mesh()
}

/// The repeated layers' tiles round the view (each layer moving at its parallax), and the glints' twinkle.
#[allow(clippy::type_complexity)]
pub(super) fn place_tiles(
    world: Res<SimWorld>,
    fixed: Res<Time<Fixed>>,
    camera: Single<(&Camera, &Projection, &Transform), (With<MainCamera>, Without<Tile>)>,
    mut tiles: Query<(&Tile, &mut Transform)>,
    twinkle: Query<&Twinkle>,
    mut materials: ResMut<Assets<kit::atlas::AtlasMaterial>>,
) {
    let (cam, proj, tf) = camera.into_inner();
    let (cam_pos, size) = view(cam, proj, tf);
    for (tile, mut t) in &mut tiles {
        let q = cam_pos * tile.parallax;
        let first = ((q - size / 2.0) / tile.size).floor();
        let slot = first + Vec2::new((tile.slot % COPIES) as f32, (tile.slot / COPIES) as f32);
        let at = slot * tile.size - q + cam_pos;
        t.translation.x = at.x;
        t.translation.y = at.y;
    }
    if let Some(Twinkle(m)) = twinkle.iter().next() {
        let s = clock(&world, &fixed);
        set_alpha(&mut materials, m, 0.55 + 0.45 * (s * 0.9).sin() * (s * 0.37 + 1.0).cos());
    }
}
