//! Far things (GDR-0104), drawn only:
//!
//! * **Landmarks**: each of the scenery's landmarks that its chance gives this cluster (rolled from the voyage's
//!   seed), anchored somewhere in the cluster on a layer of the landmark's parallax, as a dim silhouette between the
//!   backdrop's stars and its dust: a derelict station (hub, spokes, a torn ring, two faint lights), a beacon (a mast
//!   whose light turns slowly), a ring of junk round a dead hub.
//! * **The corona** (a landmark of form `Corona`, only beside a hot star): arcs of the corona's light and a few loops
//!   off them at the edge of the view towards the star, over the backdrop's star glow, flickering softly; thin
//!   strips, so one more large translucent layer at most.
//! * **Distant fights**: now and then (the scenery's `fights` a minute on average) a skirmish far off on a layer of
//!   parallax [`FIGHT_PARALLAX`]: two knots of tiny ships, tracers between them, flashes, and a burst at the end.

use bevy::prelude::*;
use data::LandmarkForm;
use kit::atlas::{AtlasMaterial, Region, set_alpha};
use kit::meshbuf::MeshBuf;
use kit::rng::Rng;
use kit::style::vcol;

use super::{Build, Layered, Scenery, SceneryPart, Spec, clock, salted, view};
use crate::camera::MainCamera;
use crate::sim_bridge::SimWorld;

/// Landmarks: over the backdrop's stars (z −801), under its dust (−800).
const Z_LANDMARKS: f32 = -800.5;
/// Over the backdrop's star glow (−880), under its dust.
const Z_CORONA: f32 = -870.0;
const Z_FIGHTS: f32 = -800.3;
/// Landmarks lie within this share of the cluster's radius.
const LANDMARK_REACH: f32 = 0.7;
/// Distant fights hang this far off (their layer's share of the camera's motion) ...
const FIGHT_PARALLAX: f32 = 0.06;
/// ... somewhere within this share of the view round its centre, and last this long (s).
const FIGHT_SPREAD: f32 = 0.35;
const FIGHT_SECONDS: (f32, f32) = (4.0, 7.0);
/// A fight's sizes (m) are for a view this wide; it is drawn as large on screen at any zoom.
const FIGHT_VIEW: f32 = 70.0;

/// What [`spawn`] made (for the log line).
pub(super) struct FarCounts {
    pub landmarks: u32,
    pub corona: bool,
}

/// The corona's view and material.
#[derive(Component)]
pub(super) struct Corona(Handle<AtlasMaterial>);

/// A beacon's light: its material blinks.
#[derive(Component)]
pub(super) struct Blink(Handle<AtlasMaterial>);

/// Spawns the landmarks this cluster rolls (and the corona, beside a hot star).
pub(super) fn spawn(cx: &mut Build, spec: &Spec, content: &data::Content) -> FarCounts {
    let mut rng = Rng::new(salted(spec.seed, 4));
    let mut out = FarCounts { landmarks: 0, corona: false };
    let white = cx.atlas.region("fx/white");
    let soft = cx.atlas.region("fx/dot_soft");
    let hard = cx.atlas.region("fx/dot_hard");
    let fill = cx.atlas.token_color("world.landmark");
    let edge = cx.atlas.token_color("world.landmark_edge");
    let light = cx.atlas.token_color("world.landmark_light");
    for place in &spec.scenery.landmarks {
        let roll = rng.next_f32();
        let Some(l) = content.landmarks.get(&place.landmark) else { continue };
        if roll >= place.chance {
            continue;
        }
        let size = rng.range(l.size.0, l.size.1);
        if l.form == LandmarkForm::Corona {
            if spec.star.is_some() && !out.corona {
                let material = cx.atlas.group_material(cx.materials);
                let mesh = corona_mesh(&mut rng, &white, cx.atlas.token_color("world.corona"));
                cx.commands.spawn((SceneryPart, Corona(material.clone()), Mesh2d(cx.meshes.add(mesh)), MeshMaterial2d(material), Transform::from_xyz(0.0, 0.0, Z_CORONA), Visibility::Hidden));
                out.corona = true;
            }
            continue;
        }
        let anchor = Vec2::from_angle(rng.range(-3.15, 3.15)) * spec.radius * LANDMARK_REACH * rng.next_f32().sqrt();
        let mut m = MeshBuf::default();
        let mut lamp = None;
        match l.form {
            LandmarkForm::Station => station(&mut m, &mut rng, size, &white, &hard, fill, edge, light),
            LandmarkForm::Beacon => lamp = Some(beacon(&mut m, size, &white, fill, edge)),
            LandmarkForm::Ring => ring(&mut m, &mut rng, size, &white, fill, edge),
            LandmarkForm::Corona => unreachable!("handled above"),
        }
        let layered = Layered { anchor, drift: Vec2::ZERO, angle: rng.range(-0.4, 0.4), spin: 0.0, parallax: l.parallax };
        let mut e = cx.commands.spawn((
            SceneryPart,
            layered,
            Mesh2d(cx.meshes.add(m.into_mesh())),
            MeshMaterial2d(cx.atlas.material.clone()),
            Transform::from_xyz(anchor.x, anchor.y, Z_LANDMARKS),
        ));
        if let Some(top) = lamp {
            let material = cx.atlas.group_material(cx.materials);
            let mut glow = MeshBuf::default();
            glow.region_at(&soft, top, Vec2::splat(size * 0.5), 0.0, vcol(light, 0.5));
            glow.region_at(&hard, top, Vec2::splat(size * 0.05), 0.0, vcol(light, 0.95));
            e.with_child((Blink(material.clone()), Mesh2d(cx.meshes.add(glow.into_mesh())), MeshMaterial2d(material), Transform::from_xyz(0.0, 0.0, 0.1)));
        }
        out.landmarks += 1;
    }
    out
}

/// A dead station `size` across: a hub, spokes, a ring torn in places with modules on it, two faint lights.
#[allow(clippy::too_many_arguments)]
fn station(m: &mut MeshBuf, rng: &mut Rng, size: f32, white: &Region, hard: &Region, fill: Color, edge: Color, light: Color) {
    let r = size * 0.42;
    let hub: Vec<Vec2> = (0..16).map(|k| Vec2::from_angle(k as f32 * std::f32::consts::FRAC_PI_8) * size * 0.11).collect();
    let spokes = 4 + (rng.next_f32() * 3.0) as usize;
    for k in 0..spokes {
        let a = k as f32 * std::f32::consts::TAU / spokes as f32 + rng.range(-0.1, 0.1);
        let reach = if rng.next_f32() < 0.25 { rng.range(0.4, 0.8) } else { 1.0 };
        m.strip(&[Vec2::from_angle(a) * size * 0.1, Vec2::from_angle(a) * r * reach], false, size * 0.018, white, vcol(edge, 0.9));
    }
    // The ring in arcs, with gaps where it tore.
    let arcs = 48;
    let mut arc: Vec<Vec2> = Vec::new();
    for k in 0..=arcs {
        let a = k as f32 * std::f32::consts::TAU / arcs as f32;
        let torn = rng.next_f32() < 0.12;
        if torn || k == arcs {
            if arc.len() >= 2 {
                m.strip(&arc, false, size * 0.035, white, vcol(fill, 0.95));
                m.strip(&arc, false, size * 0.006, white, vcol(edge, 0.8));
            }
            arc.clear();
        }
        if !torn {
            arc.push(Vec2::from_angle(a) * r);
        }
        if !torn && k % 6 == 3 {
            let c = Vec2::from_angle(a) * r;
            let t = Vec2::from_angle(a + std::f32::consts::FRAC_PI_2) * size * 0.05;
            let n = Vec2::from_angle(a) * size * 0.03;
            let module = [c - t - n, c + t - n, c + t + n, c - t + n];
            m.polygon(&module, white.uv_center(), vcol(fill, 1.0));
            m.strip(&module, true, size * 0.004, white, vcol(edge, 0.7));
        }
    }
    m.polygon(&hub, white.uv_center(), vcol(fill, 1.0));
    m.strip(&hub, true, size * 0.006, white, vcol(edge, 0.9));
    for _ in 0..2 {
        let at = Vec2::from_angle(rng.range(-3.15, 3.15)) * r;
        m.region_at(hard, at, Vec2::splat(size * 0.008), 0.0, vcol(light, 0.35));
    }
}

/// A beacon mast `size` tall with crossbars and a base; its lamp's spot (local) is returned.
fn beacon(m: &mut MeshBuf, size: f32, white: &Region, fill: Color, edge: Color) -> Vec2 {
    let (base, top) = (Vec2::new(0.0, -size * 0.5), Vec2::new(0.0, size * 0.5));
    let foot = [base + Vec2::new(-size * 0.14, 0.0), base + Vec2::new(size * 0.14, 0.0), base + Vec2::new(size * 0.06, size * 0.1), base + Vec2::new(-size * 0.06, size * 0.1)];
    m.polygon(&foot, white.uv_center(), vcol(fill, 1.0));
    m.strip(&foot, true, size * 0.008, white, vcol(edge, 0.8));
    m.strip(&[base, top], false, size * 0.02, white, vcol(edge, 0.9));
    for k in 1..4 {
        let y = base.y + size * 0.25 * k as f32;
        let w = size * (0.12 - 0.025 * k as f32);
        m.strip(&[Vec2::new(-w, y), Vec2::new(w, y)], false, size * 0.012, white, vcol(edge, 0.75));
    }
    top
}

/// A ring of junk `size` across round a dead hub, seen at a slant.
fn ring(m: &mut MeshBuf, rng: &mut Rng, size: f32, white: &Region, fill: Color, edge: Color) {
    let (rx, ry) = (size * 0.5, size * 0.17);
    for _ in 0..160 {
        let a = rng.range(-3.15, 3.15);
        let c = Vec2::new(a.cos() * rx, a.sin() * ry) * rng.range(0.9, 1.08);
        let s = size * rng.range(0.004, 0.012);
        let turn = rng.range(-3.15, 3.15);
        let bit: Vec<Vec2> = (0..4).map(|k| c + Vec2::from_angle(turn + k as f32 * std::f32::consts::FRAC_PI_2) * s * rng.range(0.6, 1.0)).collect();
        m.polygon(&bit, white.uv_center(), vcol(edge, rng.range(0.4, 0.9)));
    }
    let hub: Vec<Vec2> = (0..12).map(|k| Vec2::new((k as f32 * std::f32::consts::FRAC_PI_6).cos(), (k as f32 * std::f32::consts::FRAC_PI_6).sin()) * size * 0.06).collect();
    m.polygon(&hub, white.uv_center(), vcol(fill, 1.0));
    m.strip(&hub, true, size * 0.005, white, vcol(edge, 0.8));
}

/// The corona in unit space (placed and scaled by [`corona`]: 1 is 1.2 times the view's larger side): three arcs
/// round the star's side of its centre and six loops rising off the inner one, all facing −x.
fn corona_mesh(rng: &mut Rng, white: &Region, color: Color) -> Mesh {
    let mut m = MeshBuf::default();
    let arc = |radius: f32, span: f32| -> Vec<Vec2> { (0..=40).map(|k| Vec2::from_angle(std::f32::consts::PI + span * (k as f32 / 20.0 - 1.0)) * radius).collect() };
    for (radius, width, alpha) in [(0.14, 0.003, 0.45), (0.165, 0.007, 0.22), (0.2, 0.014, 0.1)] {
        m.strip(&arc(radius, 0.7), false, width, white, vcol(color, alpha));
    }
    for _ in 0..6 {
        let a = std::f32::consts::PI + rng.range(-0.6, 0.6);
        let (w, h) = (rng.range(0.02, 0.05), rng.range(0.01, 0.025));
        let loop_pts: Vec<Vec2> = (0..=12)
            .map(|k| {
                let t = k as f32 / 12.0 * std::f32::consts::PI;
                let out = 0.14 - h * t.sin();
                Vec2::from_angle(a + w * (t.cos())) * out
            })
            .collect();
        m.strip(&loop_pts, false, 0.003, white, vcol(color, 0.3));
    }
    m.into_mesh()
}

/// The corona at the edge of the view towards the star (as the backdrop's star glow: centred past the edge),
/// flickering softly.
#[allow(clippy::type_complexity)]
pub(super) fn corona(
    world: Res<SimWorld>,
    fixed: Res<Time<Fixed>>,
    state: Res<Scenery>,
    camera: Single<(&Camera, &Projection, &Transform), (With<MainCamera>, Without<Corona>)>,
    mut corona: Query<(&Corona, &mut Transform, &mut Visibility)>,
    mut materials: ResMut<Assets<AtlasMaterial>>,
) {
    let Some(star) = state.spec.as_ref().and_then(|s| s.star) else { return };
    let (cam, proj, tf) = camera.into_inner();
    let (at, size) = view(cam, proj, tf);
    for (c, mut t, mut vis) in &mut corona {
        let dir = (star - at).try_normalize().unwrap_or(Vec2::X);
        let edge = (size / 2.0 / dir.abs().max(Vec2::splat(1e-3))).min_element();
        let scale = size.max_element() * 1.2;
        let centre = at + dir * (edge + scale * 0.06);
        t.translation.x = centre.x;
        t.translation.y = centre.y;
        t.scale = Vec3::new(scale, scale, 1.0);
        // The mesh's arcs face −x of its centre: turn them towards the view.
        t.rotation = Quat::from_rotation_z(dir.to_angle());
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        let s = clock(&world, &fixed);
        set_alpha(&mut materials, &c.0, 0.75 + 0.25 * (s * 1.3).sin() * (s * 0.7).cos());
    }
}

/// The distant fights under way, the next one's start and the mesh they are drawn into.
#[derive(Resource, Default)]
pub(super) struct Fights {
    rng: Option<Rng>,
    next: Option<f32>,
    active: Vec<Fight>,
    view: Option<(Entity, Handle<Mesh>)>,
}

/// One distant fight: its anchor (on its layer), when it started and how long it lasts, its ships (relative to the
/// anchor, side 0 then side 1), and its tracers' and flashes' phases.
struct Fight {
    anchor: Vec2,
    start: f32,
    seconds: f32,
    ships: Vec<(Vec2, usize)>,
    phase: Vec<f32>,
}

/// Starts distant fights at the scenery's rate and draws those under way (one mesh, rebuilt while any is on).
#[allow(clippy::too_many_arguments)]
pub(super) fn fights(
    mut commands: Commands,
    world: Res<SimWorld>,
    fixed: Res<Time<Fixed>>,
    state: Res<Scenery>,
    mut fights: ResMut<Fights>,
    atlas: Res<kit::atlas::Atlas>,
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Single<(&Camera, &Projection, &Transform), With<MainCamera>>,
) {
    let Some(spec) = state.spec.as_ref().filter(|s| s.scenery.fights > 0.0) else { return };
    let now = clock(&world, &fixed);
    let (cam, proj, tf) = camera.into_inner();
    let (at, size) = view(cam, proj, tf);
    let f = &mut *fights;
    let rng = f.rng.get_or_insert_with(|| Rng::new(salted(spec.seed, 5)));
    let mean = 60.0 / spec.scenery.fights;
    let next = *f.next.get_or_insert(now + mean * 0.3);
    if now >= next {
        // Somewhere in the view's middle now, anchored so that it stays put on its layer.
        let offset = Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) * size * FIGHT_SPREAD;
        let anchor = at + offset / FIGHT_PARALLAX;
        let spread = rng.range(3.0, 6.0);
        let mut ships = Vec::new();
        for side in 0..2 {
            let centre = Vec2::new(if side == 0 { -spread } else { spread }, rng.range(-1.0, 1.0));
            for _ in 0..2 + (rng.next_f32() * 2.0) as usize {
                ships.push((centre + Vec2::new(rng.range(-1.2, 1.2), rng.range(-1.2, 1.2)), side));
            }
        }
        let phase = (0..12).map(|_| rng.next_f32()).collect();
        f.active.push(Fight { anchor, start: now, seconds: rng.range(FIGHT_SECONDS.0, FIGHT_SECONDS.1), ships, phase });
        f.next = Some(now + mean * (-(rng.range(0.05, 1.0)).ln()));
    }
    f.active.retain(|x| now - x.start < x.seconds + 1.0);
    let hard = atlas.region("fx/dot_hard");
    let soft = atlas.region("fx/dot_soft");
    let white = atlas.region("fx/white");
    let tracer = [atlas.token_color("world.distant_tracer"), atlas.token_color("world.distant_fire")];
    let fire = atlas.token_color("world.distant_fire");
    // Drawn at the same size on screen at any zoom (so far off, the zoom hardly changes it).
    let k = size.x / FIGHT_VIEW;
    let mut m = MeshBuf::default();
    for x in &f.active {
        let centre = at + (x.anchor - at) * FIGHT_PARALLAX;
        let age = now - x.start;
        let fade = (1.0 - (age - x.seconds).max(0.0)).clamp(0.0, 1.0) * (age / 0.6).min(1.0);
        let n = x.ships.len();
        for (i, &(p, side)) in x.ships.iter().enumerate() {
            let (q, other) = x.ships[(i + n / 2) % n];
            let (p, q) = (centre + p * k, centre + q * k);
            m.region_at(&hard, p, Vec2::splat(0.14 * k), 0.0, vcol(tracer[side], 0.7 * fade));
            let ph = x.phase[i % x.phase.len()];
            if age < x.seconds && other != side {
                // A tracer towards a ship of the other side: a streak running along the line.
                let t = (age * (1.6 + ph) + ph).fract();
                m.strip(&[p.lerp(q, t), p.lerp(q, (t + 0.15).min(1.0))], false, 0.035 * k, &white, vcol(tracer[side], 0.85 * fade));
                // A hit's flash on the target now and then.
                let flash = (age * 0.9 + ph * 7.0).fract();
                if flash < 0.15 {
                    m.region_at(&soft, q, Vec2::splat((1.0 + flash * 9.0) * k), 0.0, vcol(fire, (0.8 - flash * 5.0).max(0.0) * fade));
                }
            }
        }
        // The end: one side's ship bursts.
        let end = age - x.seconds;
        if (0.0..1.0).contains(&end) {
            let p = centre + x.ships[0].0 * k;
            m.region_at(&soft, p, Vec2::splat((1.5 + end * 7.0) * k), 0.0, vcol(fire, 0.85 * (1.0 - end)));
            m.region_at(&hard, p, Vec2::splat((0.3 + end * 0.8) * k), 0.0, vcol(Color::WHITE, 0.9 * (1.0 - end)));
        }
    }
    let mesh = if m.is_empty() { MeshBuf::placeholder() } else { m.into_mesh() };
    match &f.view {
        Some((_, h)) => {
            if let Some(mut slot) = meshes.get_mut(h) {
                *slot = mesh;
            }
        }
        None => {
            let h = meshes.add(mesh);
            let e = commands.spawn((SceneryPart, Mesh2d(h.clone()), MeshMaterial2d(atlas.material.clone()), Transform::from_xyz(0.0, 0.0, Z_FIGHTS))).id();
            f.view = Some((e, h));
        }
    }
}

/// The beacons' lights turn slowly: a bright pass every few seconds.
pub(super) fn blink(world: Res<SimWorld>, fixed: Res<Time<Fixed>>, lights: Query<&Blink>, mut materials: ResMut<Assets<AtlasMaterial>>) {
    let s = clock(&world, &fixed);
    for b in &lights {
        let phase = (s / 3.2).fract();
        set_alpha(&mut materials, &b.0, if phase < 0.18 { 1.0 - phase / 0.18 * 0.8 } else { 0.2 });
    }
}
