//! The space backdrop: the radial gradient from the style guide stretched over the viewport, two parallax
//! layers of faint dust particles, a far layer of stars twinkling in three groups, and dotted range rings around
//! the followed ship; and the veil the screens over the station (BEHAVIOR, RESEARCH, FLEET) lay over all of it, so
//! they share one dark ground.
//!
//! Outside flight the dust drifts slowly past (the station floats; Niklas, 15 Oct 2026: "very static"); in flight
//! it stays put, the ships' measure of their own motion. The rings keep their dots a few pixels across and apart at
//! any zoom (rebuilt when the zoom moves by a step), so they stay visible zoomed out.

use bevy::prelude::*;
use kit::atlas::{Atlas, AtlasMaterial, set_alpha};
use kit::meshbuf::MeshBuf;
use kit::params::RenderToggles;
use kit::rng::Rng;
use kit::style::{RadialGradient, Style, vcol};
use kit::ui_kit::ScreenFx;

use crate::camera::{CameraRig, CameraSet, MainCamera};
use crate::mode::AppMode;

const Z_BACKDROP: f32 = -900.0;
const Z_DUST: f32 = -800.0;
const Z_RINGS: f32 = -700.0;
/// The veil: over the gradient, the dust and the rings, under the station and everything else in the world.
const Z_VEIL: f32 = -650.0;
/// How much the veil hides of the backdrop (in its outer, darkest colour).
const VEIL: f32 = 0.92;

/// Dust tiles repeat every this many metres (in layer space).
const TILE: f32 = 240.0;
/// Copies per layer (3 x 3 covers any view up to 2 tiles wide).
const COPIES: i32 = 3;

#[derive(Component)]
struct Backdrop;

#[derive(Component)]
struct DustCopy {
    layer: usize,
    slot: i32,
}

/// A star group's material (the group twinkles as one).
#[derive(Component)]
struct Twinkle(usize);

/// The dotted range rings: the zoom step they were built for.
#[derive(Component)]
pub struct RangeRings(Option<i32>);

/// A screen that veils the backdrop while it shows (fading with its `ScreenFx`): BEHAVIOR, RESEARCH and FLEET sit
/// on the same dark ground, the gradient's outer colour, and the station behind them shows only as much as
/// `ui_kit::BEHIND` lets it (Niklas, 14 Oct 2026: the screens had different backgrounds).
#[derive(Component)]
pub struct VeilsBackdrop;

/// The veil: a quad in the gradient's outer colour over the backdrop (a child of it, so it fits the view too), with
/// its own material for the fade.
#[derive(Component)]
struct Veil(Handle<AtlasMaterial>);

struct DustLayer {
    /// How much of the camera motion the layer follows on screen (1 = like the ships).
    parallax: f32,
}

/// Two layers of dust, then three groups of stars (far away, each group twinkling on its own).
const LAYERS: [DustLayer; 5] =
    [DustLayer { parallax: 0.35 }, DustLayer { parallax: 0.7 }, DustLayer { parallax: 0.12 }, DustLayer { parallax: 0.12 }, DustLayer { parallax: 0.12 }];
const STAR_LAYER: usize = 2;
/// Stars per group and tile.
const STARS: usize = 140;
/// The dust's drift outside flight (m/s, in the far layer's space; nearer layers pass faster; Niklas, 15 Oct 2026:
/// 0.55 m/s moved too fast).
const DRIFT: Vec2 = Vec2::new(-0.16, 0.035);
/// The drift wraps after this many metres: a whole number of tiles in every layer (it moves 1, 35/12 and 70/12
/// times as far in the stars', the far dust's and the near dust's space), so nothing jumps.
const DRIFT_WRAP: f32 = TILE * 12.0;
/// The range rings: radius (m), and their dots' least size and spacing on screen (logical px).
const RINGS: [f32; 2] = [30.0, 60.0];
const RING_DOT_PX: f32 = 2.6;
const RING_GAP_PX: f32 = 11.0;
/// Zoom steps the rings are rebuilt at (per doubling).
const RING_STEPS: f32 = 6.0;

pub struct BackdropPlugin;

impl Plugin for BackdropPlugin {
    fn build(&self, app: &mut App) {
        // Outside the gradient mesh (and in screens that veil the world with it): the gradient's outer colour.
        // The style exists from the atlas plugin on.
        let clear = app.world().resource::<Style>().background.outer();
        app.insert_resource(ClearColor(clear))
            .add_systems(Startup, spawn_backdrop.after(crate::camera::spawn_camera))
            .init_resource::<Drift>()
            .add_systems(Update, (veil, twinkle, drift))
            // After the camera moved, before the transforms are propagated (else they show last frame's place).
            .add_systems(
                PostUpdate,
                (fit_backdrop, place_dust, follow_rings, build_rings).after(CameraSet).before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

fn spawn_backdrop(
    mut commands: Commands,
    atlas: Res<Atlas>,
    style: Res<Style>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AtlasMaterial>>,
    camera: Single<Entity, With<MainCamera>>,
) {
    // The style-guide gradient (`ui.background`, a CSS radial gradient) as a mesh with vertex colours
    // (resolution independent) in a unit square that is scaled to the viewport. Colours are interpolated in sRGB
    // at every ring (like the browser); between rings the GPU interpolates, which is indistinguishable with this
    // many rings.
    let backdrop = commands
        .spawn((
            Backdrop,
            Mesh2d(meshes.add(backdrop_mesh(&atlas, &style.background))),
            MeshMaterial2d(atlas.material.clone()),
            Transform::from_xyz(0.0, 0.0, Z_BACKDROP),
        ))
        .id();
    commands.entity(*camera).add_child(backdrop);
    let mut quad = MeshBuf::default();
    // The unit square the backdrop is scaled from, a little larger, so no edge shows.
    let (h, white) = (0.55, atlas.region("fx/white").uv_center());
    quad.quad_uv([Vec2::new(-h, -h), Vec2::new(h, -h), Vec2::new(h, h), Vec2::new(-h, h)], [white; 4], vcol(style.background.outer(), 1.0));
    let material = atlas.group_material(&mut materials);
    set_alpha(&mut materials, &material, 0.0);
    let veil = commands
        .spawn((Veil(material.clone()), Mesh2d(meshes.add(quad.into_mesh())), MeshMaterial2d(material), Transform::from_xyz(0.0, 0.0, Z_VEIL - Z_BACKDROP), Visibility::Hidden))
        .id();
    commands.entity(backdrop).add_child(veil);

    // Dust: far layer = many tiny faint dots, near layer = fewer, slightly larger ones.
    let soft = atlas.region("fx/dot_soft");
    let hard = atlas.region("fx/dot_hard");
    let mut rng = Rng::new(0x5eed_d057);
    for (li, _) in LAYERS.iter().enumerate().take(STAR_LAYER) {
        let mut m = MeshBuf::default();
        let (count, size, alpha) = if li == 0 { (3600, (0.05, 0.11), (0.12, 0.35)) } else { (1200, (0.08, 0.16), (0.2, 0.5)) };
        for _ in 0..count {
            let at = Vec2::new(rng.range(0.0, TILE), rng.range(0.0, TILE));
            let s = rng.range(size.0, size.1);
            let a = rng.range(alpha.0, alpha.1);
            m.region_at(&hard, at, Vec2::splat(s), 0.0, vcol(style.particle, a));
            if rng.next_f32() < 0.08 {
                // an occasional soft mote
                m.region_at(&soft, at, Vec2::splat(s * 6.0), 0.0, vcol(style.particle, a * 0.35));
            }
        }
        let mesh = meshes.add(m.into_mesh());
        for slot in 0..COPIES * COPIES {
            commands.spawn((
                DustCopy { layer: li, slot },
                Mesh2d(mesh.clone()),
                MeshMaterial2d(atlas.material.clone()),
                Transform::from_xyz(0.0, 0.0, Z_DUST + li as f32),
            ));
        }
    }

    // Stars: a hard point and a faint halo each, in three groups that twinkle out of step.
    for group in 0..LAYERS.len() - STAR_LAYER {
        let mut m = MeshBuf::default();
        for _ in 0..STARS {
            let at = Vec2::new(rng.range(0.0, TILE), rng.range(0.0, TILE));
            let s = rng.range(0.1, 0.24);
            let a = rng.range(0.45, 0.9);
            m.region_at(&hard, at, Vec2::splat(s), 0.0, vcol(style.star, a));
            m.region_at(&soft, at, Vec2::splat(s * 7.0), 0.0, vcol(style.star, a * 0.22));
        }
        let mesh = meshes.add(m.into_mesh());
        let material = atlas.group_material(&mut materials);
        for slot in 0..COPIES * COPIES {
            commands.spawn((
                DustCopy { layer: STAR_LAYER + group, slot },
                Twinkle(group),
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_xyz(0.0, 0.0, Z_DUST - 1.0),
            ));
        }
    }

    // Dotted range rings (style guide: dotted rings in #414c52), built for the zoom (`build_rings`).
    commands.spawn((RangeRings(None), Mesh2d(meshes.add(MeshBuf::placeholder())), MeshMaterial2d(atlas.material.clone()), Transform::from_xyz(0.0, 0.0, Z_RINGS)));
}

/// How far the dust has drifted (m, in the far layer's space).
#[derive(Resource, Default)]
struct Drift(Vec2);

/// The dust drifts outside flight.
fn drift(time: Res<Time<Real>>, mode: Res<State<AppMode>>, mut drift: ResMut<Drift>) {
    if *mode.get() == AppMode::Flight {
        return;
    }
    let d = drift.0 + DRIFT * time.delta_secs();
    drift.0 = Vec2::new(d.x.rem_euclid(DRIFT_WRAP), d.y.rem_euclid(DRIFT_WRAP));
}

/// How bright a star group is at `t` (s): two slow waves out of step, so no group repeats soon.
fn twinkle_alpha(group: usize, t: f32) -> f32 {
    let g = group as f32;
    let w = (t * (0.9 + 0.37 * g) + g * 2.1).sin() * 0.6 + (t * (2.3 + 0.53 * g) + g).sin() * 0.4;
    0.6 + 0.4 * w
}

/// The star groups twinkle.
fn twinkle(time: Res<Time<Real>>, stars: Query<(&Twinkle, &MeshMaterial2d<AtlasMaterial>)>, mut materials: ResMut<Assets<AtlasMaterial>>, mut done: Local<[bool; 3]>) {
    let t = time.elapsed_secs_wrapped();
    *done = [false; 3];
    for (tw, material) in &stars {
        if !done[tw.0] {
            done[tw.0] = true;
            set_alpha(&mut materials, &material.0, twinkle_alpha(tw.0, t));
        }
    }
}

/// The range rings for a zoom (px per m): each dot at least [`RING_DOT_PX`] across and [`RING_GAP_PX`] apart on
/// screen, never smaller or closer than the style's world sizes.
fn ring_mesh(hard: &kit::atlas::Region, color: Color, zoom: f32) -> Mesh {
    let mut m = MeshBuf::default();
    for (radius, (spacing, dot)) in RINGS.into_iter().zip([(1.1f32, 0.22f32), (1.6, 0.26)]) {
        let (spacing, dot) = (spacing.max(RING_GAP_PX / zoom), dot.max(RING_DOT_PX / zoom));
        let n = (std::f32::consts::TAU * radius / spacing).round().max(8.0) as usize;
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            m.region_at(hard, Vec2::new(a.cos(), a.sin()) * radius, Vec2::splat(dot / 0.92), 0.0, vcol(color, 0.9));
        }
    }
    m.into_mesh()
}

/// The rings again when the zoom has moved by a step.
fn build_rings(rig: Res<CameraRig>, atlas: Res<Atlas>, style: Res<Style>, mut meshes: ResMut<Assets<Mesh>>, rings: Single<(&mut RangeRings, &Mesh2d)>) {
    let step = (rig.zoom.max(1e-3).log2() * RING_STEPS).round() as i32;
    let (mut rings, mesh) = rings.into_inner();
    if rings.0 == Some(step) {
        return;
    }
    rings.0 = Some(step);
    let zoom = 2f32.powf(step as f32 / RING_STEPS);
    let _ = meshes.insert(&mesh.0, ring_mesh(&atlas.region("fx/dot_hard"), style.dotted_ring, zoom));
}

fn fit_backdrop(
    toggles: Res<RenderToggles>,
    camera: Single<(&Camera, &Projection), With<MainCamera>>,
    mut q: Single<(&mut Transform, &mut Visibility), With<Backdrop>>,
) {
    let (cam, proj) = camera.into_inner();
    let Some(size) = cam.logical_viewport_size() else { return };
    let scale = match proj {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };
    let want = (size * scale).extend(1.0);
    let (tf, vis) = &mut *q;
    if tf.scale != want {
        tf.scale = want;
    }
    let v = if toggles.debug_shapes { Visibility::Hidden } else { Visibility::Inherited };
    if **vis != v {
        **vis = v;
    }
}

/// The veil follows the screens that veil the backdrop: as strong as the most open of them.
fn veil(screens: Query<&ScreenFx, With<VeilsBackdrop>>, veil: Single<(&Veil, &mut Visibility)>, mut materials: ResMut<Assets<AtlasMaterial>>) {
    let open = screens.iter().map(ScreenFx::alpha).fold(0.0, f32::max);
    let (v, mut vis) = veil.into_inner();
    set_alpha(&mut materials, &v.0, VEIL * open);
    let want = if open > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
}

/// Unit-square backdrop gradient (x, y in -0.5..0.5, y up).
fn backdrop_mesh(atlas: &Atlas, gradient: &RadialGradient) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let colour = |t: f32| {
        let l = gradient.at(t).to_linear();
        [l.red, l.green, l.blue, 1.0]
    };
    let white = atlas.region("fx/white").uv_center();
    let (rings, segs) = (28usize, 72usize);
    let (centre, radii) = (Vec2::new(gradient.centre.x - 0.5, 0.5 - gradient.centre.y), gradient.radii);
    let mut pos = vec![[centre.x, centre.y, 0.0]];
    let mut col = vec![colour(0.0)];
    for r in 1..=rings {
        // a little denser in the middle, reaching t = 1.1 (outside the unit square everywhere)
        let t = 1.1 * (r as f32 / rings as f32).powf(1.15);
        for s in 0..segs {
            let a = s as f32 / segs as f32 * std::f32::consts::TAU;
            pos.push([centre.x + radii.x * t * a.cos(), centre.y + radii.y * t * a.sin(), 0.0]);
            col.push(colour(t));
        }
    }
    let mut idx: Vec<u32> = Vec::new();
    for s in 0..segs as u32 {
        idx.extend_from_slice(&[0, 1 + s, 1 + (s + 1) % segs as u32]);
    }
    for r in 0..rings as u32 - 1 {
        let a = 1 + r * segs as u32;
        let b = a + segs as u32;
        for s in 0..segs as u32 {
            let s1 = (s + 1) % segs as u32;
            idx.extend_from_slice(&[a + s, b + s, b + s1, a + s, b + s1, a + s1]);
        }
    }
    let n = pos.len();
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[white.x, white.y]; n]);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
    m
}

#[allow(clippy::type_complexity)]
fn place_dust(
    toggles: Res<RenderToggles>,
    drift: Res<Drift>,
    camera: Single<(&Camera, &Projection, &Transform), (With<MainCamera>, Without<DustCopy>)>,
    mut copies: Query<(&DustCopy, &mut Transform, &mut Visibility)>,
) {
    let (cam, proj, cam_tf) = camera.into_inner();
    let scale = match proj {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };
    let view = cam.logical_viewport_size().unwrap_or(Vec2::splat(1000.0)) * scale;
    let cam_pos = cam_tf.translation.truncate();
    for (copy, mut tf, mut vis) in &mut copies {
        let f = LAYERS[copy.layer].parallax;
        // Camera position in layer space (plus the drift, nearer layers passing faster); the layer is shifted so it
        // moves `f` times as fast as the world.
        let q = cam_pos * f - drift.0 * (f / LAYERS[STAR_LAYER].parallax);
        let first = ((q - view / 2.0) / TILE).floor();
        let (ix, iy) = (copy.slot % COPIES, copy.slot / COPIES);
        let tile = first + Vec2::new(ix as f32, iy as f32);
        let needed = (tile * TILE - q).cmple(view / 2.0);
        let origin = tile * TILE - q + cam_pos;
        if tf.translation.truncate() != origin {
            tf.translation.x = origin.x;
            tf.translation.y = origin.y;
        }
        let on = needed.all() && !toggles.debug_shapes;
        let v = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != v {
            *vis = v;
        }
    }
}

fn follow_rings(
    rig: Res<CameraRig>,
    toggles: Res<RenderToggles>,
    mut rings: Single<(&mut Transform, &mut Visibility), With<RangeRings>>,
) {
    let (tf, vis) = &mut *rings;
    match rig.focus {
        Some(p) if !toggles.debug_shapes => {
            tf.translation.x = p.x;
            tf.translation.y = p.y;
            if **vis != Visibility::Inherited {
                **vis = Visibility::Inherited;
            }
        }
        _ => {
            if **vis != Visibility::Hidden {
                **vis = Visibility::Hidden;
            }
        }
    }
}
