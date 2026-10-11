//! The space backdrop: the radial gradient from the style guide stretched over the viewport, two parallax
//! layers of faint dust particles and dotted range rings around the followed ship.

use bevy::prelude::*;

use crate::atlas::Atlas;
use crate::camera::{CameraRig, CameraSet, MainCamera};
use crate::meshbuf::MeshBuf;
use crate::sim_bridge::RenderPose;
use crate::style::{vcol, Style};
use crate::RenderToggles;

const Z_BACKDROP: f32 = -900.0;
const Z_DUST: f32 = -800.0;
const Z_RINGS: f32 = -700.0;

/// Dust tiles repeat every this many metres (in layer space).
const TILE: f32 = 120.0;
/// Copies per layer (3 x 3 covers any view up to 2 tiles wide).
const COPIES: i32 = 3;

#[derive(Component)]
struct Backdrop;

#[derive(Component)]
struct DustCopy {
    layer: usize,
    slot: i32,
}

#[derive(Component)]
pub struct RangeRings;

struct DustLayer {
    /// How much of the camera motion the layer follows on screen (1 = like the ships).
    parallax: f32,
}

const LAYERS: [DustLayer; 2] = [DustLayer { parallax: 0.35 }, DustLayer { parallax: 0.7 }];

pub struct BackdropPlugin;

impl Plugin for BackdropPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_backdrop.after(crate::spawn_camera))
            .add_systems(PostUpdate, (fit_backdrop, place_dust, follow_rings).after(CameraSet));
    }
}

/// Tiny deterministic generator for decoration (client-only; the sim has its own seeded streams).
pub struct Rng(u32);
impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }
    pub fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next_f32()
    }
}

fn spawn_backdrop(
    mut commands: Commands,
    atlas: Res<Atlas>,
    style: Res<Style>,
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Single<Entity, With<MainCamera>>,
) {
    let bg = atlas.region("fx/backdrop");
    // Sample half a texel inside the region so the gutter never shows at the screen edge.
    let inset = Vec2::splat(1.0);
    let rect = Rect::from_corners(bg.px.min + inset, bg.px.max - inset);
    let backdrop = commands
        .spawn((
            Backdrop,
            Sprite { image: atlas.image.clone(), rect: Some(rect), custom_size: Some(Vec2::ONE), ..default() },
            Transform::from_xyz(0.0, 0.0, Z_BACKDROP),
        ))
        .id();
    commands.entity(*camera).add_child(backdrop);

    // Dust: far layer = many tiny faint dots, near layer = fewer, slightly larger ones.
    let soft = atlas.region("fx/dot_soft");
    let hard = atlas.region("fx/dot_hard");
    let mut rng = Rng::new(0x5eed_d057);
    for (li, _) in LAYERS.iter().enumerate() {
        let mut m = MeshBuf::default();
        let (count, size, alpha) = if li == 0 { (1100, (0.05, 0.11), (0.12, 0.35)) } else { (360, (0.08, 0.16), (0.2, 0.5)) };
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

    // Dotted range rings (style guide: dotted rings in #414c52).
    let mut m = MeshBuf::default();
    for (radius, spacing, dot) in [(11.0f32, 0.55f32, 0.09f32), (17.5, 0.7, 0.08)] {
        let n = (std::f32::consts::TAU * radius / spacing).round() as usize;
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            m.region_at(&hard, Vec2::new(a.cos(), a.sin()) * radius, Vec2::splat(dot / 0.92), 0.0, vcol(style.dotted_ring, 0.9));
        }
    }
    commands.spawn((
        RangeRings,
        Mesh2d(meshes.add(m.into_mesh())),
        MeshMaterial2d(atlas.material.clone()),
        Transform::from_xyz(0.0, 0.0, Z_RINGS),
    ));
}

fn fit_backdrop(
    toggles: Res<RenderToggles>,
    camera: Single<(&Camera, &Projection), With<MainCamera>>,
    mut q: Single<(&mut Sprite, &mut Visibility), With<Backdrop>>,
) {
    let (cam, proj) = camera.into_inner();
    let Some(size) = cam.logical_viewport_size() else { return };
    let scale = match proj {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };
    let want = size * scale;
    let (sprite, vis) = &mut *q;
    if sprite.custom_size != Some(want) {
        sprite.custom_size = Some(want);
    }
    let v = if toggles.debug_shapes { Visibility::Hidden } else { Visibility::Inherited };
    if **vis != v {
        **vis = v;
    }
}

fn place_dust(
    toggles: Res<RenderToggles>,
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
        // Camera position in layer space; the layer is shifted so it moves `f` times as fast as the world.
        let q = cam_pos * f;
        let first = ((q - view / 2.0) / TILE).floor();
        let (ix, iy) = (copy.slot % COPIES, copy.slot / COPIES);
        let tile = first + Vec2::new(ix as f32, iy as f32);
        let needed = (tile * TILE - q).cmple(view / 2.0);
        let origin = tile * TILE + cam_pos * (1.0 - f);
        tf.translation.x = origin.x;
        tf.translation.y = origin.y;
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
    targets: Query<&RenderPose>,
    mut rings: Single<(&mut Transform, &mut Visibility), With<RangeRings>>,
) {
    let target = rig.follow.or(toggles.ring_target).and_then(|e| targets.get(e).ok());
    let (tf, vis) = &mut *rings;
    match target {
        Some(p) if !toggles.debug_shapes => {
            tf.translation.x = p.0.pos.x;
            tf.translation.y = p.0.pos.y;
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
