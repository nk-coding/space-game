//! The world camera: 2D orthographic, 1 world unit = 1 m = 1 cell, never rotates. Modes: follow the focused
//! vessel (sandbox; the watched ship of a behavior test while nobody pilots), frame the whole fight (load test),
//! or free (drag to pan). Zoom with the wheel or a pinch;
//! both position and zoom ease towards their targets (time-based, so framerate-independent).
//!
//! A focus switch glides over (GDR-0013): the follow point snaps to the new vessel and the camera eases from
//! where it was onto it over 0.3–0.5 s (longer for longer jumps, smootherstep). Only in follow mode: a free
//! camera stays put, dragging during a glide ends it, and the zoom is untouched.
//!
//! The camera always covers the whole window. [`CameraRig::view_offset`] moves the point the rig centres on
//! away from the window centre (the drydock centres its grid between the side panels); the screen ↔ world
//! helpers include it. Flight input (wheel, drag, pinch, F) runs only in flight; the drydock drives the rig
//! itself (`drydock::view`).

use bevy::camera::ScalingMode;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::egui_wants_any_pointer_input;

use crate::controls::PlayerInput;
use crate::mode::AppMode;
use crate::scene::{PilotFocus, Scene};
use crate::sim_bridge::{SimWorld, piece_com, v2};

pub const MIN_PX_PER_M: f32 = 3.0;
pub const MAX_PX_PER_M: f32 = 160.0;
/// Follow lag (time constant, s).
const FOLLOW_TAU: f32 = 0.12;
/// Focus-switch glide: duration for a short hop and for a jump of [`GLIDE_FAR`] metres or more.
const GLIDE_SECONDS: (f32, f32) = (0.3, 0.5);
const GLIDE_FAR: f32 = 150.0;

#[derive(Component)]
pub struct MainCamera;

/// Runs in `PostUpdate` after the piece views are posed; camera-relative things go after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CamMode {
    Follow,
    Frame,
    Free,
}

#[derive(Resource, Debug)]
pub struct CameraRig {
    pub pos: Vec2,
    /// Logical pixels per metre (current, eased).
    pub zoom: f32,
    pub zoom_target: f32,
    pub mode: CamMode,
    /// The mode `F` returns to.
    pub auto_mode: CamMode,
    /// The user zoomed by hand (framing keeps its zoom then).
    pub user_zoom: bool,
    /// Where the focused vessel is (for the range rings), if any.
    pub focus: Option<Vec2>,
    /// The vessel followed last frame; a change starts a glide.
    followed: Option<sim::ShipId>,
    /// Where plain following puts the camera; during a glide the camera eases from its old place onto it.
    follow_pos: Vec2,
    glide: Option<Glide>,
    /// Viewport size in logical pixels (last frame).
    pub viewport: Vec2,
    /// Where on screen `pos` sits, relative to the window centre (logical pixels, y down). Zero in flight.
    pub view_offset: Vec2,
    drag_last: Option<Vec2>,
    pinch_last: Option<(f32, Vec2)>,
    /// While a zoom around a screen point eases in (free camera): the point's offset from the view centre
    /// (logical pixels, y up) and the world point kept under it until the zoom arrives. Moving the camera any
    /// other way ends it ([`CameraRig::end_zoom_anchor`]).
    zoom_anchor: Option<(Vec2, Vec2)>,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            pos: Vec2::ZERO,
            zoom: 18.0,
            zoom_target: 18.0,
            mode: CamMode::Follow,
            auto_mode: CamMode::Follow,
            user_zoom: false,
            focus: None,
            followed: None,
            follow_pos: Vec2::ZERO,
            glide: None,
            viewport: Vec2::new(1280.0, 720.0),
            view_offset: Vec2::ZERO,
            drag_last: None,
            pinch_last: None,
            zoom_anchor: None,
        }
    }
}

/// A focus-switch glide in progress.
#[derive(Clone, Copy, Debug)]
struct Glide {
    /// Camera position minus the follow point at the switch; shrinks to zero.
    offset: Vec2,
    seconds: f32,
    duration: f32,
}

/// Zero speed and acceleration at both ends.
fn smootherstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

impl CameraRig {
    /// The focus moved to a vessel at `f`: the follow point snaps there, the camera eases from where it is.
    fn start_glide(&mut self, f: Vec2) {
        let offset = self.pos - f;
        let far = (offset.length() / GLIDE_FAR).min(1.0);
        let duration = GLIDE_SECONDS.0 + (GLIDE_SECONDS.1 - GLIDE_SECONDS.0) * far;
        self.follow_pos = f;
        self.glide = Some(Glide { offset, seconds: 0.0, duration });
    }

    /// Follow mode, one frame: trail the focus at `f` (exponential, time constant [`FOLLOW_TAU`]); during a glide
    /// the camera is the follow point plus the switch offset, shrinking to zero along a smootherstep.
    fn follow(&mut self, f: Vec2, dt: f32) {
        let k = 1.0 - (-dt / FOLLOW_TAU).exp();
        match self.glide.as_mut() {
            Some(g) => {
                g.seconds += dt;
                let offset = g.offset * (1.0 - smootherstep(g.seconds / g.duration));
                if g.seconds >= g.duration {
                    self.glide = None;
                }
                self.follow_pos = self.follow_pos.lerp(f, k);
                self.pos = self.follow_pos + offset;
            }
            None => {
                self.pos = self.pos.lerp(f, k);
                self.follow_pos = self.pos;
            }
        }
    }

    /// The logical screen point (y down) the rig centres `pos` on.
    pub fn view_center(&self) -> Vec2 {
        self.viewport / 2.0 + self.view_offset
    }
    /// World point under a logical screen position (y down).
    pub fn screen_to_world(&self, screen: Vec2) -> Vec2 {
        let d = screen - self.view_center();
        self.pos + Vec2::new(d.x, -d.y) / self.zoom
    }
    /// Logical screen position (y down) of a world point.
    pub fn world_to_screen(&self, world: Vec2) -> Vec2 {
        let d = (world - self.pos) * self.zoom;
        self.view_center() + Vec2::new(d.x, -d.y)
    }
    /// Zooms the target by `factor` within `min..max` px per metre. With a screen point, a free camera keeps
    /// the world point under it in place (wheel zoom around the cursor) all through the eased zoom, not only
    /// once it arrives: the position follows the zoom ([`CameraRig::ease_zoom`]).
    pub fn zoom_around(&mut self, factor: f32, around: Option<Vec2>, min: f32, max: f32) {
        self.zoom_target = (self.zoom_target * factor).clamp(min, max);
        self.zoom_anchor = match (self.mode, around) {
            (CamMode::Free, Some(screen)) => {
                let d = screen - self.view_center();
                let d = Vec2::new(d.x, -d.y);
                Some((d, self.pos + d / self.zoom))
            }
            _ => None,
        };
    }
    /// The camera was moved another way (pan, clamp, reset): a zoom still easing in does so around the view
    /// centre from here on.
    pub fn end_zoom_anchor(&mut self) {
        self.zoom_anchor = None;
    }
    /// Jumps to where an easing zoom is going (leaving a screen keeps the view it was zooming to).
    pub fn settle_zoom(&mut self) {
        self.zoom = self.zoom_target;
        if let (CamMode::Free, Some((d, w))) = (self.mode, self.zoom_anchor) {
            self.pos = w - d / self.zoom;
        }
        self.zoom_anchor = None;
    }
    /// One frame of the eased zoom (time constant 0.1 s); a zoom around a point moves the camera with it.
    fn ease_zoom(&mut self, dt: f32) {
        let k = 1.0 - (-dt / 0.1).exp();
        let z = self.zoom + (self.zoom_target - self.zoom) * k;
        // Within 0.1 % the zoom has arrived (an exponential never does).
        let arrived = (z - self.zoom_target).abs() <= 1e-3 * self.zoom_target;
        self.zoom = if arrived { self.zoom_target } else { z }.clamp(MIN_PX_PER_M, MAX_PX_PER_M);
        if let Some((d, w)) = self.zoom_anchor {
            if self.mode == CamMode::Free {
                self.pos = w - d / self.zoom;
            }
            if self.mode != CamMode::Free || arrived {
                self.zoom_anchor = None;
            }
        }
    }
    /// Half the visible world size.
    pub fn half_extent(&self) -> Vec2 {
        self.viewport / (2.0 * self.zoom)
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraRig>()
            .add_systems(
                Update,
                (wheel_and_drag.run_if(not(egui_wants_any_pointer_input)), pinch).run_if(in_state(AppMode::Flight)),
            )
            .add_systems(
                PostUpdate,
                update_camera
                    .in_set(CameraSet)
                    .after(crate::ship_view::sync_piece_views)
                    // The projection (zoom) must reach this frame's camera matrices like the transform does;
                    // otherwise the world draws one frame's zoom behind the UI icons placed for it.
                    .before(bevy::camera::CameraUpdateSystems)
                    .before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// The camera bundle (spawned by `main`).
pub fn camera_bundle(zoom: f32) -> impl Bundle {
    (
        MainCamera,
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::WindowSize,
            scale: 1.0 / zoom,
            ..OrthographicProjection::default_2d()
        }),
    )
}

fn zoom_by(rig: &mut CameraRig, factor: f32, around: Option<Vec2>) {
    rig.zoom_around(factor, around, MIN_PX_PER_M, MAX_PX_PER_M);
    rig.user_zoom = true;
}

fn wheel_and_drag(
    mut rig: ResMut<CameraRig>,
    scroll: Res<AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<PilotFocus>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    rig.viewport = window.size();
    let cursor = window.cursor_position();
    if scroll.delta.y != 0.0 {
        let steps = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 100.0,
        };
        zoom_by(&mut rig, 1.15f32.powf(steps.clamp(-5.0, 5.0)), cursor);
    }
    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        zoom_by(&mut rig, 1.25, None);
    }
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        zoom_by(&mut rig, 0.8, None);
    }
    if keys.just_pressed(KeyCode::KeyF) {
        rig.mode = if rig.mode == CamMode::Free { rig.auto_mode } else { CamMode::Free };
        rig.user_zoom = false;
    }
    // Drag to pan: middle button always; left button only without a ship to fly (load test, showcase).
    let drag_button = buttons.pressed(MouseButton::Middle)
        || (focus.0.is_none() && buttons.pressed(MouseButton::Left));
    match (drag_button, cursor) {
        (true, Some(c)) => {
            if let Some(last) = rig.drag_last
                && last != c
            {
                let d = c - last;
                let z = rig.zoom;
                rig.pos -= Vec2::new(d.x, -d.y) / z;
                rig.mode = CamMode::Free;
            }
            rig.drag_last = Some(c);
        }
        _ => rig.drag_last = None,
    }
}

/// Two fingers that are not on the stick or the fire button: pinch zoom (and pan in free mode).
fn pinch(mut rig: ResMut<CameraRig>, touches: Res<Touches>, input: Res<PlayerInput>) {
    let mut free = touches.iter().filter(|t| !input.captures(t.id()));
    let (Some(a), Some(b)) = (free.next(), free.next()) else {
        rig.pinch_last = None;
        return;
    };
    let (pa, pb) = (a.position(), b.position());
    let dist = pa.distance(pb).max(1.0);
    let mid = (pa + pb) / 2.0;
    if let Some((last_dist, last_mid)) = rig.pinch_last {
        let f = dist / last_dist;
        rig.zoom_target = (rig.zoom_target * f).clamp(MIN_PX_PER_M, MAX_PX_PER_M);
        rig.user_zoom = true;
        if rig.mode == CamMode::Free {
            let d = mid - last_mid;
            let z = rig.zoom;
            rig.pos -= Vec2::new(d.x, -d.y) / z;
        }
    }
    rig.pinch_last = Some((dist, mid));
}

fn update_camera(
    time: Res<Time<Real>>,
    fixed: Res<Time<Fixed>>,
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    scene: Res<Scene>,
    mut rig: ResMut<CameraRig>,
    camera: Single<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let dt = time.delta_secs().min(0.1);
    let alpha = fixed.overstep_fraction().clamp(0.0, 1.0);

    // The focused vessel, else the watched one of a behavior test (controlling piece, else the main piece).
    let followed = focus.0.or(scene.watched());
    rig.focus = followed.and_then(|id| world.sim.ship(id)).and_then(|s| {
        let pid = s.control_piece().or_else(|| s.main_piece())?;
        s.piece(pid).map(|p| piece_com(p, alpha))
    });
    // A new focus: glide over from where the camera is.
    if followed != rig.followed {
        rig.followed = followed;
        if let (CamMode::Follow, Some(f)) = (rig.mode, rig.focus) {
            rig.start_glide(f);
        }
    }
    if rig.mode != CamMode::Follow {
        rig.glide = None;
    }

    match rig.mode {
        CamMode::Follow => {
            if let Some(f) = rig.focus {
                rig.follow(f, dt);
            }
        }
        CamMode::Frame => {
            // Frame every active ship (or every piece if none is active).
            let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
            let mut any = false;
            for s in world.sim.ships() {
                if !s.is_active() {
                    continue;
                }
                for p in s.pieces() {
                    let c = v2(p.com_world());
                    min = min.min(c - Vec2::splat(6.0));
                    max = max.max(c + Vec2::splat(6.0));
                    any = true;
                }
            }
            if any {
                let k = 1.0 - (-dt / 0.6).exp();
                let centre = (min + max) / 2.0;
                rig.pos = rig.pos.lerp(centre, k);
                if !rig.user_zoom {
                    let size = (max - min) * 1.15;
                    let fit = (rig.viewport.x / size.x).min(rig.viewport.y / size.y);
                    rig.zoom_target = fit.clamp(MIN_PX_PER_M, 40.0);
                }
            }
        }
        CamMode::Free => {}
    }
    rig.ease_zoom(dt);

    let (mut tf, mut proj) = camera.into_inner();
    // The camera covers the whole window; `pos` sits at the view centre.
    let at = rig.pos - Vec2::new(rig.view_offset.x, -rig.view_offset.y) / rig.zoom;
    tf.translation.x = at.x;
    tf.translation.y = at.y;
    if let Projection::Orthographic(o) = proj.as_mut() {
        let s = 1.0 / rig.zoom;
        if (o.scale - s).abs() > f32::EPSILON * s {
            o.scale = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glides a camera from the origin to a vessel 100 m away that flies at 20 m/s, at `fps` (a multiple of 10);
    /// returns the camera position every 0.1 s of the first 0.7 s and the largest jump between two frames,
    /// scaled to a 60 Hz frame.
    fn glide_path(fps: u32) -> (Vec<Vec2>, f32) {
        let mut rig = CameraRig::default();
        let start = Vec2::new(100.0, 0.0);
        rig.start_glide(start);
        let dt = 1.0 / fps as f32;
        let (mut path, mut jump, mut last) = (Vec::new(), 0.0f32, rig.pos);
        for frame in 1..=(fps * 7 / 10) {
            let t = frame as f32 * dt;
            rig.follow(start + Vec2::new(0.0, 20.0 * t), dt);
            jump = jump.max(rig.pos.distance(last) * fps as f32 / 60.0);
            last = rig.pos;
            if frame % (fps / 10) == 0 {
                path.push(rig.pos);
            }
        }
        (path, jump)
    }

#[test]
    fn a_zoom_around_the_cursor_keeps_its_world_point_under_it_while_easing() {
        let mut rig =
            CameraRig { mode: CamMode::Free, pos: Vec2::new(3.0, -2.0), zoom: 20.0, zoom_target: 20.0, ..default() };
        let cursor = Vec2::new(900.0, 200.0);
        let w = rig.screen_to_world(cursor);
        rig.zoom_around(1.5, Some(cursor), 1.0, 200.0);
        for _ in 0..60 {
            rig.ease_zoom(1.0 / 60.0);
            assert!(rig.screen_to_world(cursor).distance(w) < 1e-3, "{} vs {w}", rig.screen_to_world(cursor));
        }
        assert_eq!(rig.zoom, 30.0);
        // Panning ends the anchor: the rest of the zoom eases around the view centre.
        rig.zoom_around(1.5, Some(cursor), 1.0, 200.0);
        rig.end_zoom_anchor();
        let pos = rig.pos;
        rig.ease_zoom(1.0 / 60.0);
        assert_eq!(rig.pos, pos);
    }

    #[test]
    fn focus_glide_is_quick_smooth_and_framerate_independent() {
        let (a, jump_a) = glide_path(30);
        let (b, jump_b) = glide_path(150);
        // Arrives within the glide (0.3-0.5 s): at 0.6 s the camera trails the vessel by its follow lag only.
        let end = *a.last().unwrap();
        assert!(end.distance(Vec2::new(100.0, 20.0 * 0.7)) < 20.0 * 0.7 * FOLLOW_TAU + 1.0, "{end}");
        // Smooth: no frame jumps far above the glide's peak speed (smootherstep: 1.875 x 100 m / 0.43 s, about
        // 7.3 m per 60 Hz frame).
        assert!(jump_a < 8.5 && jump_b < 8.5, "{jump_a} {jump_b}");
        // The same path at 30 and 150 fps.
        assert_eq!(a.len(), 7);
        for (p, q) in a.iter().zip(&b) {
            assert!(p.distance(*q) < 1.0, "{p} vs {q}");
        }
    }
}
