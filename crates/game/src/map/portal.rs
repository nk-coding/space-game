//! The portal between the shipyard and the voyage map (Niklas, 14 Oct 2026: "more or less like going through the
//! portal / expanding the portal to see the map"). Home's gate (V, a click, the debrief's VOYAGE MAP) opens the map
//! at once and the portal takes the camera:
//!
//! * **In** ([`ENTER_SECONDS`]): the camera dives at the gate (`shipyard::fly::Fly`, zooming around its anchor)
//!   until it fills most of the window. Inside the ring the star map shows through an aperture that opens like an
//!   iris, then widens past the ring over the rest of the yard until the window is all map; the map comes up from
//!   depth (smaller, centred on the aperture, settling onto its place); the aperture's edge is soft, nothing is
//!   drawn along it (Niklas, 14 Oct 2026: a ring of light there looked weird). The map's screen (the panel, the
//!   words) opens once the window is all map
//!   ([`SCREEN_AT`]).
//! * **Out** ([`EXIT_SECONDS`]): Esc or the ring button closes the screen and the same path runs backwards (the
//!   screen first, then the aperture shrinks into the gate while the camera pulls out to the yard's view as home left
//!   it); home takes over resting on the yard.
//! * **How**: the aperture cuts the atlas materials (`atlas::set_aperture`): the map's drawing shows inside it, the
//!   shipyard's groups outside ([`Portal::hole`], `shipyard::show`), so space (the backdrop) shows through the gate
//!   behind the map. The map's drawing hangs under a pivot that carries the depth (the map places it as always,
//!   `place_mesh`). Everything follows the time along the transition, so it looks the same at any frame rate and turns back from
//!   wherever it is.
//!
//! Started on the map (`?scene=map`) the portal is open at once; leaving to flight (SET COURSE) shuts it.
//!
//! Timing (defaults for review): in 1.0 s, out 0.85 s; the camera dives over the first 60 %, the iris opens over the
//! first 30 %, the aperture widens from 35 % to 70 %, the map has settled at 75 %, the screen opens at 70 % (its own
//! transition, `ui_kit::ScreenFx`, runs on from there). The dive stops at a zoom like the old flight to the gate's
//! (the atlas keeps its tier; `atlas.rs`).

use bevy::prelude::*;
use bevy::state::state::StateTransitionEvent;
use bevy::window::PrimaryWindow;

use super::{MapMesh, Sheet};
use crate::atlas::{APERTURE_EDGE, Aperture, set_aperture};
use crate::camera::CameraRig;
use crate::mode::AppMode;
use crate::shipyard::Yard;
use crate::shipyard::fly::{Fly, View, smootherstep};
use crate::shipyard::layout::{GATE_RADIUS, fit};

/// How long the way in and the way out take (seconds).
pub const ENTER_SECONDS: f32 = 1.0;
pub const EXIT_SECONDS: f32 = 0.85;
/// Where along the way (0..1) the map's screen opens: the window is all map from here.
pub const SCREEN_AT: f32 = 0.7;
/// The camera's dive takes this share of the way; it ends with the gate's opening this share of the window's
/// shorter side from its centre.
const DIVE: f32 = 0.6;
const RING_SHARE: f32 = 0.4;
/// The iris opens over this share.
const IRIS: f32 = 0.3;
/// The aperture widens from the ring to past the window's corners between these points of the way.
const WIDEN: (f32, f32) = (0.35, SCREEN_AT);
/// The map comes up from this scale and has settled at this point of the way.
const DEPTH: f32 = 0.55;
const SETTLE: f32 = 0.75;
/// The map's drawing fades in over this share (the iris starts as a point).
const MAP_FADE: f32 = 0.15;
/// The gate's opening: the inside of its ring (m).
const OPENING: f32 = GATE_RADIUS * 0.92;

/// The portal's state (a resource).
#[derive(Resource, Debug, Default)]
pub struct Portal {
    open: bool,
    /// Along the whole way: 0 the yard, 1 all map.
    t: f32,
    /// The yard's view as home rested on it last: where the dive starts and the pull-out ends.
    yard: Option<View>,
    /// The flight's zoom (home gives it back when it leaves; the portal does when the map closes).
    flight_zoom: f32,
    /// The aperture now (logical px, y down) while the portal is on its way.
    aperture: Option<(Vec2, f32)>,
    /// Physical px per logical px.
    scale: f32,
}

impl Portal {
    /// Asked to be open (it may still be on its way).
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Back to the yard (Esc, the ring button).
    pub fn leave(&mut self) {
        self.open = false;
    }

    /// Whether the map's screen shows: open and far enough in.
    pub fn screen_open(&self) -> bool {
        self.open && self.t >= SCREEN_AT
    }

    /// Whether the map's drawing shows.
    pub fn shows(&self) -> bool {
        self.t > 0.0
    }

    /// The map drawing's fade.
    pub fn map_alpha(&self) -> f32 {
        smootherstep(self.t / MAP_FADE)
    }

    /// Where the yard is cut away (framebuffer px): outside the aperture on the way, all of it while open.
    pub fn hole(&self) -> Option<Aperture> {
        if self.t >= 1.0 {
            return Some(Aperture { centre: Vec2::ZERO, radius: 1e6, inside: false });
        }
        self.aperture.map(|(c, r)| Aperture { centre: c * self.scale, radius: r * self.scale, inside: false })
    }
}

/// One frame of the way: `t` moves to 1 in [`ENTER_SECONDS`] while open, to 0 in [`EXIT_SECONDS`] while closed.
pub fn step(t: f32, open: bool, dt: f32) -> f32 {
    if open { (t + dt / ENTER_SECONDS).min(1.0) } else { (t - dt / EXIT_SECONDS).max(0.0) }
}

/// How far the camera has dived at `t` (eased, 0..1).
fn dive(t: f32) -> f32 {
    smootherstep(t / DIVE)
}

/// The aperture's radius at `t`: an iris from nothing to the gate's opening (`ring` px now), then wider until it
/// reaches `cover` (px: past the window's farthest corner).
fn radius(t: f32, ring: f32, cover: f32) -> f32 {
    let iris = 1.0 - (1.0 - (t / IRIS).clamp(0.0, 1.0)).powi(3);
    let (from, to) = WIDEN;
    iris * (ring + (cover - ring).max(0.0) * smootherstep((t - from) / (to - from)))
}

/// How far the aperture centred at `c` has to reach to cover a `window` (logical px), its soft edge outside.
fn cover(c: Vec2, window: Vec2) -> f32 {
    c.x.max(window.x - c.x).hypot(c.y.max(window.y - c.y)) + APERTURE_EDGE
}

/// How far the map has come up from depth at `t` (eased out, 0..1).
fn settle(t: f32) -> f32 {
    1.0 - (1.0 - (t / SETTLE).clamp(0.0, 1.0)).powi(3)
}

/// The view the dive ends on: the gate at the window's centre, filling most of it.
fn dive_view(gate: Vec2, window: Vec2) -> View {
    View { centre: gate, zoom: RING_SHARE * window.min_element() / OPENING }
}

/// The camera at `t` between the yard's view and the dive's.
fn camera_at(t: f32, yard: View, gate: Vec2, window: Vec2) -> View {
    Fly::new(yard, dive_view(gate, window)).at(dive(t))
}

/// Runs the portal in `Update` (the map's drawing and the shipyard read it after).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortalSet;

/// The pivot the map's drawing hangs under (it carries the depth).
#[derive(Component)]
struct MapPivot;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Portal>()
        .add_systems(Startup, spawn.after(super::spawn))
        .add_systems(OnEnter(AppMode::Map), enter)
        .add_systems(OnExit(AppMode::Map), exit)
        .add_systems(Update, animate.in_set(PortalSet).after(super::input).run_if(in_state(AppMode::Map)))
        .add_systems(PostUpdate, remember.after(crate::camera::CameraSet).run_if(in_state(AppMode::Home)));
}

/// The map's drawings go under its pivot (not the chrome: the legend and the panel's backing open with the screen,
/// fixed).
fn spawn(mut commands: Commands, sheets: Query<(Entity, &MapMesh)>) {
    let pivot = commands.spawn((MapPivot, Transform::default(), Visibility::default())).id();
    for (e, _) in sheets.iter().filter(|(_, m)| m.sheet != Sheet::Chrome) {
        commands.entity(e).insert(ChildOf(pivot));
    }
}

/// Home's view of the yard (the dive starts from it and the pull-out ends on it).
fn remember(rig: Res<CameraRig>, mut portal: ResMut<Portal>) {
    let view = View { centre: rig.pos, zoom: rig.zoom };
    if portal.yard != Some(view) {
        portal.yard = Some(view);
    }
}

/// The map opens: through the gate from home, at once otherwise (`?scene=map`).
fn enter(mut transitions: MessageReader<StateTransitionEvent<AppMode>>, rig: Res<CameraRig>, mut portal: ResMut<Portal>) {
    let from_home = transitions.read().last().is_some_and(|t| t.exited == Some(AppMode::Home));
    portal.flight_zoom = rig.zoom_target;
    portal.open = true;
    portal.t = if from_home && portal.yard.is_some() { 0.0 } else { 1.0 };
    portal.aperture = None;
}

/// The map closes: shut, and the flight's zoom back (home keeps it, flight uses it).
fn exit(mut rig: ResMut<CameraRig>, mut portal: ResMut<Portal>) {
    rig.zoom = portal.flight_zoom;
    rig.zoom_target = portal.flight_zoom;
    portal.open = false;
    portal.t = 0.0;
    portal.aperture = None;
}

/// The way in or out, one frame: the camera, the aperture, the map's depth; home once it is back out.
#[allow(clippy::too_many_arguments)]
fn animate(
    time: Res<Time<Real>>,
    window: Single<&Window, With<PrimaryWindow>>,
    yard: Res<Yard>,
    sheets: Query<&MapMesh>,
    mut portal: ResMut<Portal>,
    mut rig: ResMut<CameraRig>,
    mut next: ResMut<NextState<AppMode>>,
    mut materials: ResMut<Assets<crate::atlas::AtlasMaterial>>,
    mut pivot: Single<&mut Transform, With<MapPivot>>,
) {
    let target = if portal.open { 1.0 } else { 0.0 };
    if portal.t == target && portal.aperture.is_none() {
        return;
    }
    let t = step(portal.t, portal.open, time.delta_secs().min(0.1));
    portal.t = t;
    portal.scale = window.scale_factor();
    if t == 0.0 {
        next.set(AppMode::Home);
    }
    let size = window.size();
    let gate = yard.layout.gate;
    let on_way = t > 0.0 && t < 1.0;
    // The camera (left alone while open: the map may move it).
    let from = portal.yard.unwrap_or_else(|| {
        let (centre, zoom) = fit(yard.layout.overview, size);
        View { centre, zoom }
    });
    let view = camera_at(t, from, gate, size);
    rig.end_zoom_anchor();
    rig.pos = view.centre;
    rig.zoom = view.zoom;
    rig.zoom_target = view.zoom;
    let aperture = on_way.then(|| {
        let c = rig.world_to_screen(gate);
        (c, radius(t, OPENING * rig.zoom, cover(c, size)))
    });
    portal.aperture = aperture;
    let cut = aperture.map(|(c, r)| Aperture { centre: c * portal.scale, radius: r * portal.scale, inside: true });
    for m in sheets.iter().filter(|m| m.sheet != Sheet::Chrome) {
        set_aperture(&mut materials, &m.material, cut);
    }
    // The map's depth: scaled around the window's centre, which follows the aperture's until it settles.
    let s = settle(t);
    let want = match aperture {
        Some((c, _)) if s < 1.0 => {
            let k = DEPTH + (1.0 - DEPTH) * s;
            let at = rig.screen_to_world(c.lerp(size / 2.0, s));
            Transform { translation: (at - rig.screen_to_world(size / 2.0) * k).extend(0.0), scale: Vec3::new(k, k, 1.0), ..default() }
        }
        _ => Transform::default(),
    };
    if **pivot != want {
        **pivot = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The way in takes ENTER_SECONDS at any frame rate, the way out EXIT_SECONDS.
    #[test]
    fn the_way_runs_its_time() {
        for fps in [30.0, 60.0, 144.0] {
            let (mut t, mut frames) = (0.0, 0);
            while t < 1.0 {
                t = step(t, true, 1.0 / fps);
                frames += 1;
            }
            assert!((frames as f32 / fps - ENTER_SECONDS).abs() <= 1.0 / fps + 1e-4, "{fps}");
            let mut frames = 0;
            while t > 0.0 {
                t = step(t, false, 1.0 / fps);
                frames += 1;
            }
            assert!((frames as f32 / fps - EXIT_SECONDS).abs() <= 1.0 / fps + 1e-4, "{fps}");
        }
    }

    /// The aperture starts as a point in the gate, opens to the ring and widens until it covers the window; the map
    /// comes up from depth and has settled when its screen opens.
    #[test]
    fn the_curves_run_from_the_gate_to_the_map() {
        let (ring, far) = (300.0, 800.0);
        assert_eq!((radius(0.0, ring, far), dive(0.0), settle(0.0)), (0.0, 0.0, 0.0));
        assert!((radius(IRIS, ring, far) - ring).abs() < 1.0);
        assert!(radius(SCREEN_AT, ring, far) == far && dive(1.0) == 1.0 && settle(1.0) == 1.0);
        let (mut o, mut d) = (0.0, 0.0);
        for k in 1..=100 {
            let t = k as f32 / 100.0;
            assert!(radius(t, ring, far) >= o && dive(t) >= d, "{t}");
            (o, d) = (radius(t, ring, far), dive(t));
        }
        assert!(settle(SCREEN_AT) > 0.99);
        assert_eq!(cover(Vec2::new(640.0, 360.0), Vec2::new(1280.0, 720.0)), Vec2::new(640.0, 360.0).length() + APERTURE_EDGE);
    }

    /// From the yard's view (the gate right of centre, small) the camera dives onto the gate, and by the time the
    /// screen opens the aperture covers the whole window, at any aspect; the gate's ring is inside the window then.
    #[test]
    fn the_window_is_all_map_when_the_screen_opens() {
        let gate = Vec2::new(54.0, -12.0);
        for window in [Vec2::new(1280.0, 720.0), Vec2::new(1920.0, 1080.0), Vec2::new(800.0, 1280.0), Vec2::new(3440.0, 1440.0)] {
            let yard = View { centre: Vec2::new(26.0, -12.0), zoom: window.x / 82.0 };
            let rig = |t: f32| {
                let v = camera_at(t, yard, gate, window);
                let mut rig = CameraRig::default();
                (rig.pos, rig.zoom, rig.viewport) = (v.centre, v.zoom, window);
                rig
            };
            // At the start the gate sits where the yard's view has it, the aperture closed.
            let start = rig(0.0);
            assert!(start.world_to_screen(gate).distance(window / 2.0 + Vec2::new(28.0, 0.0) * yard.zoom) < 1e-3);
            for t in [SCREEN_AT, 0.85, 1.0] {
                let r = rig(t);
                let c = r.world_to_screen(gate);
                let radius = radius(t, OPENING * r.zoom, cover(c, window));
                let corners = [Vec2::ZERO, Vec2::new(window.x, 0.0), Vec2::new(0.0, window.y), window];
                assert!(corners.iter().all(|k| k.distance(c) < radius), "{window} at {t}: {c} r {radius}");
            }
            // The dive ends on the gate, its ring inside the window.
            let end = rig(1.0);
            assert!(end.world_to_screen(gate).distance(window / 2.0) < 1e-2);
            assert!(GATE_RADIUS * end.zoom < window.min_element() / 2.0);
        }
    }
}
