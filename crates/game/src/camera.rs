//! The world camera: 2D orthographic, 1 world unit = 1 m = 1 cell. Zoom with the wheel or a pinch, pan by
//! dragging (mouse or one finger). Optionally follows an entity.

use bevy::camera::ScalingMode;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::sim_bridge::RenderPose;

pub const MIN_PX_PER_M: f32 = 6.0;
pub const MAX_PX_PER_M: f32 = 220.0;

#[derive(Component)]
pub struct MainCamera;

/// Runs in `PostUpdate` after pose interpolation; place camera-relative things after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraSet;

#[derive(Resource, Debug)]
pub struct CameraRig {
    /// World point at the centre of the screen (offset from the follow target if there is one).
    pub center: Vec2,
    /// Logical pixels per metre.
    pub px_per_m: f32,
    pub follow: Option<Entity>,
    drag_last: Option<Vec2>,
    pinch_last: Option<(f32, Vec2)>,
    touch_last: Option<(u64, Vec2)>,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self { center: Vec2::ZERO, px_per_m: 40.0, follow: None, drag_last: None, pinch_last: None, touch_last: None }
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraRig>()
            .add_systems(Update, (zoom_and_pan, touch_controls))
            .add_systems(PostUpdate, apply_rig.in_set(CameraSet).after(crate::sim_bridge::interpolate_poses));
    }
}

/// The camera bundle (spawned by `main`).
pub fn camera_bundle() -> impl Bundle {
    (
        MainCamera,
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::WindowSize,
            scale: 1.0 / 40.0,
            ..OrthographicProjection::default_2d()
        }),
    )
}

fn zoom_at(rig: &mut CameraRig, factor: f32, screen: Vec2, viewport: Vec2) {
    let before = screen_to_world_offset(rig, screen, viewport);
    rig.px_per_m = (rig.px_per_m * factor).clamp(MIN_PX_PER_M, MAX_PX_PER_M);
    let after = screen_to_world_offset(rig, screen, viewport);
    rig.center += before - after;
}

/// World offset from the screen centre for a logical screen position (y down).
fn screen_to_world_offset(rig: &CameraRig, screen: Vec2, viewport: Vec2) -> Vec2 {
    let d = screen - viewport / 2.0;
    Vec2::new(d.x, -d.y) / rig.px_per_m
}

fn zoom_and_pan(
    mut rig: ResMut<CameraRig>,
    scroll: Res<AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let viewport = window.size();
    let cursor = window.cursor_position();
    if scroll.delta.y != 0.0 {
        let steps = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 100.0,
        };
        let factor = 1.15f32.powf(steps.clamp(-5.0, 5.0));
        zoom_at(&mut rig, factor, cursor.unwrap_or(viewport / 2.0), viewport);
    }
    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        zoom_at(&mut rig, 1.25, viewport / 2.0, viewport);
    }
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        zoom_at(&mut rig, 0.8, viewport / 2.0, viewport);
    }
    if keys.just_pressed(KeyCode::Home) {
        rig.center = Vec2::ZERO;
    }

    let dragging = buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Middle);
    match (dragging, cursor) {
        (true, Some(c)) => {
            if let Some(last) = rig.drag_last {
                let d = c - last;
                let px = rig.px_per_m;
                rig.center -= Vec2::new(d.x, -d.y) / px;
            }
            rig.drag_last = Some(c);
        }
        _ => rig.drag_last = None,
    }
}

fn touch_controls(mut rig: ResMut<CameraRig>, touches: Res<Touches>, window: Single<&Window, With<PrimaryWindow>>) {
    let viewport = window.size();
    let mut active = touches.iter();
    let first = active.next();
    let second = active.next();
    match (first, second) {
        (Some(a), Some(b)) => {
            rig.touch_last = None;
            let (pa, pb) = (a.position(), b.position());
            let dist = pa.distance(pb).max(1.0);
            let mid = (pa + pb) / 2.0;
            if let Some((last_dist, last_mid)) = rig.pinch_last {
                zoom_at(&mut rig, dist / last_dist, mid, viewport);
                let d = mid - last_mid;
                let px = rig.px_per_m;
                rig.center -= Vec2::new(d.x, -d.y) / px;
            }
            rig.pinch_last = Some((dist, mid));
        }
        (Some(a), None) => {
            rig.pinch_last = None;
            let p = a.position();
            if let Some((id, last)) = rig.touch_last
                && id == a.id()
            {
                let d = p - last;
                let px = rig.px_per_m;
                rig.center -= Vec2::new(d.x, -d.y) / px;
            }
            rig.touch_last = Some((a.id(), p));
        }
        _ => {
            rig.pinch_last = None;
            rig.touch_last = None;
        }
    }
}

fn apply_rig(
    rig: Res<CameraRig>,
    targets: Query<&RenderPose>,
    camera: Single<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let (mut tf, mut proj) = camera.into_inner();
    let base = rig.follow.and_then(|e| targets.get(e).ok()).map(|p| p.0.pos).unwrap_or(Vec2::ZERO);
    let c = base + rig.center;
    tf.translation.x = c.x;
    tf.translation.y = c.y;
    if let Projection::Orthographic(o) = proj.as_mut() {
        let s = 1.0 / rig.px_per_m;
        if (o.scale - s).abs() > f32::EPSILON {
            o.scale = s;
        }
    }
}
