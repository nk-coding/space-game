//! The drydock's world view and camera.
//!
//! * **The design** is drawn with the flight tile pipeline: `ship_model::blueprint_visual` (every part, line of
//!   fire and hull edge from the ship grid) and `ship_view::build_body`, into one mesh that is rebuilt only when
//!   the design's revision changes (or the glow setting). Blueprint `(x, y)` sits at world `(x, −y)`: the frame
//!   spans `0..w` × `−h..0`.
//! * **The frame**: a dotted grid at every cell corner, corner brackets around it, the dashed mirror axis (the
//!   frame's centre line, while mirroring is on) and the cell under the pointer, all immediate-mode FX sprites
//!   sized in screen pixels (`fx::FxQueue`), drawn inside the grid viewport. Outside it a veil in the backdrop's
//!   darkest colour dims the world (a ship zoomed under the side panels), as in the mockup, where the grid
//!   area is lighter than the panels; the panels themselves stay unboxed.
//! * **The camera** drives the shared `CameraRig` (so the atlas tier follows the drydock zoom): centred on the
//!   frame in the grid viewport between the side panels (`CameraRig::view_offset`), wheel zoom around the
//!   cursor, pan with Space + drag or the middle button, zoom buttons and + / −. 100 % = the frame fits the
//!   viewport. The pointer over a panel reaches neither the grid nor the camera.

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::PrimaryWindow;

use crate::RenderToggles;
use crate::atlas::Atlas;
use crate::camera::{CamMode, CameraRig, MAX_PX_PER_M};
use crate::fx::{FxQueue, FxRegions, FxSprite};
use crate::ship_model::{blueprint_visual, design_visual};
use crate::ship_view::build_body;
use crate::sim_bridge::GameContent;
use crate::style::Style;
use crate::ui_kit::UiPointer;

use super::actions::DrydockAction;
use super::PlayerDesign;

/// The design's mesh sits where flight pieces do (above the frame's dots, below the hover cell).
const Z_SHIP: f32 = 10.0;
/// FX layers (`fx::layer_of`): under the ships, and the HUD layer above them.
const Z_UNDER: f32 = 4.0;
const Z_OVER: f32 = 410.0;
/// How much the veil outside the grid viewport darkens the world.
const VEIL: f32 = 0.62;
/// Empty cells kept around the frame at 100 %.
const FIT_MARGIN: f32 = 0.6;
/// Zoom limits, as shares of the fit.
const ZOOM_RANGE: (f32, f32) = (0.5, 8.0);
/// The UI scale follows the window: 1 at 1280 × 720 logical, up to this at larger windows.
const MAX_UI_SCALE: f32 = 2.0;
/// The frame's dots are at least this many screen pixels apart.
const MIN_DOT_GAP: f32 = 6.0;

/// The design's mesh.
#[derive(Component)]
pub(super) struct DockShip {
    /// The revision and glow setting the mesh shows.
    built: Option<(u64, bool)>,
}

/// The UI node between the toolbar and the info line: the camera centres the frame in it.
#[derive(Component)]
pub struct GridViewport;

/// The drydock camera between visits, and the flight zoom to return to.
#[derive(Resource, Debug)]
pub struct DockCamera {
    /// px per metre at 100 % (the frame fits the viewport); 0 until the viewport is laid out.
    pub fit: f32,
    /// The grid viewport on screen (logical px); the frame overlay is drawn inside it.
    pub viewport: Rect,
    /// The view to restore when the drydock opens again: the world point at the viewport centre and the zoom
    /// as a share of the fit. `None`: fit the frame.
    view: Option<(Vec2, f32)>,
    /// The flight camera's zoom, restored when a test flight starts.
    flight_zoom: f32,
    drag_last: Option<Vec2>,
    /// The frame the view was made for: another one (FRAME, its undo) fits the view again.
    frame: (u32, u32),
}

/// The cell under the pointer (frame cells, `None` off the frame or over a panel), for the info line and the
/// editing tools.
#[derive(Resource, Debug, Default)]
pub struct DrydockCursor {
    pub cell: Option<(i32, i32)>,
}

pub(super) fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, atlas: Res<Atlas>) {
    commands.spawn((
        DockShip { built: None },
        Mesh2d(meshes.add(crate::meshbuf::MeshBuf::placeholder())),
        MeshMaterial2d(atlas.material.clone()),
        Transform::from_xyz(0.0, 0.0, Z_SHIP),
        Visibility::Hidden,
    ));
}

impl DockCamera {
    pub(super) fn new(flight_zoom: f32) -> Self {
        Self { fit: 0.0, viewport: Rect::default(), view: None, flight_zoom, drag_last: None, frame: (0, 0) }
    }

    /// Fits the frame again (another design or frame was opened).
    pub(super) fn reset_view(&mut self) {
        self.fit = 0.0;
        self.view = None;
    }
}

/// Opening the drydock: remember the flight zoom; the camera stops following. The view is set once the
/// viewport is laid out ([`camera`]).
pub(super) fn enter(mut rig: ResMut<CameraRig>, mut cam: ResMut<DockCamera>) {
    cam.flight_zoom = rig.zoom_target;
    cam.fit = 0.0;
    rig.mode = CamMode::Free;
    rig.auto_mode = CamMode::Free;
}

/// At start in the drydock: the flight zoom of the URL (`?zoom=`, applied at Startup) is the one to return to.
pub(super) fn remember_start_zoom(rig: Res<CameraRig>, mut cam: ResMut<DockCamera>) {
    cam.flight_zoom = rig.zoom_target;
}

/// Leaving the drydock: keep the view for next time, give flight its zoom back, the UI its normal scale.
pub(super) fn exit(mut rig: ResMut<CameraRig>, mut cam: ResMut<DockCamera>, mut ui_scale: ResMut<UiScale>) {
    if cam.fit > 0.0 {
        cam.view = Some((rig.pos, rig.zoom_target / cam.fit));
    }
    rig.view_offset = Vec2::ZERO;
    rig.zoom = cam.flight_zoom;
    rig.zoom_target = cam.flight_zoom;
    cam.drag_last = None;
    ui_scale.0 = 1.0;
}

/// The UI scale: the layout is made for 1280 × 720 logical pixels and grows with larger windows.
pub(super) fn ui_scale(window: Single<&Window, With<PrimaryWindow>>, mut ui_scale: ResMut<UiScale>) {
    let s = (window.width() / 1280.0).min(window.height() / 720.0).clamp(1.0, MAX_UI_SCALE);
    if (ui_scale.0 - s).abs() > 1e-3 {
        ui_scale.0 = s;
    }
}

/// Fit, zoom and pan; the hover cell.
#[allow(clippy::too_many_arguments)]
pub(super) fn camera(
    window: Single<&Window, With<PrimaryWindow>>,
    viewport: Single<(&ComputedNode, &UiGlobalTransform), With<GridViewport>>,
    scroll: Res<AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<UiPointer>,
    design: Res<PlayerDesign>,
    mut actions: MessageReader<DrydockAction>,
    mut rig: ResMut<CameraRig>,
    mut cam: ResMut<DockCamera>,
    mut cursor_cell: ResMut<DrydockCursor>,
) {
    let (node, tf) = *viewport;
    if cam.frame != design.frame() {
        cam.frame = design.frame();
        cam.reset_view();
    }
    let scale = window.scale_factor();
    let size = node.size / scale;
    rig.viewport = window.size();
    if size.min_element() < 8.0 {
        return;
    }
    let centre = tf.translation / scale;
    rig.view_offset = centre - rig.viewport / 2.0;
    cam.viewport = Rect::from_center_size(centre, size);
    let frame = Vec2::new(design.frame().0 as f32, design.frame().1 as f32);
    let fit = (size.x / (frame.x + 2.0 * FIT_MARGIN)).min(size.y / (frame.y + 2.0 * FIT_MARGIN));
    let frame_centre = Vec2::new(frame.x / 2.0, -frame.y / 2.0);
    if cam.fit <= 0.0 {
        // First frame of a visit: the last view, else the whole frame.
        let (pos, share) = cam.view.unwrap_or((frame_centre, 1.0));
        rig.pos = pos;
        rig.zoom = fit * share;
        rig.zoom_target = rig.zoom;
    } else if (fit - cam.fit).abs() > 1e-3 {
        // The window changed: keep the zoom share.
        let k = fit / cam.fit;
        rig.zoom *= k;
        rig.zoom_target *= k;
    }
    cam.fit = fit;
    let (min, max) = (fit * ZOOM_RANGE.0, (fit * ZOOM_RANGE.1).min(MAX_PX_PER_M).max(fit));
    let cursor = window.cursor_position();

    for a in actions.read() {
        match a {
            DrydockAction::ZoomIn => rig.zoom_around(1.25, None, min, max),
            DrydockAction::ZoomOut => rig.zoom_around(0.8, None, min, max),
            _ => {}
        }
    }
    if scroll.delta.y != 0.0 && !pointer.blocked {
        let steps = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 100.0,
        };
        rig.zoom_around(1.15f32.powf(steps.clamp(-5.0, 5.0)), cursor, min, max);
    }
    // Pan: the middle button, or Space + the left button; a drag starts only on the grid.
    let space = keys.pressed(KeyCode::Space);
    let start = buttons.just_pressed(MouseButton::Middle) || (space && buttons.just_pressed(MouseButton::Left));
    let held = buttons.pressed(MouseButton::Middle) || (space && buttons.pressed(MouseButton::Left));
    if start && !pointer.blocked {
        cam.drag_last = cursor;
    }
    match (held, cam.drag_last, cursor) {
        (true, Some(last), Some(c)) => {
            let d = c - last;
            let z = rig.zoom;
            rig.pos -= Vec2::new(d.x, -d.y) / z;
            cam.drag_last = Some(c);
        }
        (true, Some(_), None) => {}
        _ => cam.drag_last = None,
    }
    // The frame centre never leaves the frame.
    rig.pos = rig.pos.clamp(Vec2::new(0.0, -frame.y), Vec2::new(frame.x, 0.0));

    let in_view = |c: &Vec2| !pointer.blocked && cam.viewport.contains(*c);
    let cell = cursor.filter(in_view).map(|c| rig.screen_to_world(c)).map(|w| (w.x.floor() as i32, (-w.y).floor() as i32));
    let cell = cell.filter(|c| c.0 >= 0 && c.1 >= 0 && c.0 < design.frame().0 as i32 && c.1 < design.frame().1 as i32);
    // Written only when it changes: the ghost works its view out again then.
    if cursor_cell.cell != cell {
        cursor_cell.cell = cell;
    }
}

/// Rebuilds the design's mesh when its revision (or the glow setting) changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild_ship(
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ship: Single<(&mut DockShip, &Mesh2d)>,
) {
    let key = (design.revision(), toggles.glow_sprites);
    let (dock, mesh) = &mut *ship;
    if dock.built == Some(key) {
        return;
    }
    dock.built = Some(key);
    let content = &content.0;
    let built = design_visual(content, design.blueprint(), &style)
        .and_then(|dv| content.ship_grid(design.blueprint()).map(|grid| blueprint_visual(&dv, &grid)));
    let new = match built {
        Ok(v) => build_body(&v, &atlas, &style, toggles.glow_sprites, true),
        Err(e) => {
            error!("drydock: cannot draw the design: {e}");
            crate::meshbuf::MeshBuf::placeholder()
        }
    };
    let _ = meshes.insert(&mesh.0, new);
}

/// Corners between two dots of the frame's grid at `zoom` px per cell: 1, or the smallest power of two that keeps
/// them [`MIN_DOT_GAP`] pixels apart.
fn dot_stride(zoom: f32) -> i32 {
    let mut stride = 1;
    while (stride as f32) * zoom < MIN_DOT_GAP && stride < 1 << 16 {
        stride *= 2;
    }
    stride
}

/// The frame's dots, brackets and mirror axis, and the hover cell (screen-pixel sizes, so redrawn each frame),
/// inside the grid viewport.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_frame(
    design: Res<PlayerDesign>,
    cursor: Res<DrydockCursor>,
    rig: Res<CameraRig>,
    cam: Res<DockCamera>,
    style: Res<Style>,
    clear: Res<ClearColor>,
    regions: Option<Res<FxRegions>>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(r) = regions else { return };
    let px = 1.0 / rig.zoom;
    let (w, h) = (design.frame().0 as i32, design.frame().1 as i32);
    let ui_c = &style.ui;
    // What of the world the grid viewport shows (frame rows count down: blueprint y = −world y).
    let a = rig.screen_to_world(cam.viewport.min);
    let b = rig.screen_to_world(cam.viewport.max);
    let view = Rect::from_corners(a, b);
    // The veil: four quads around the viewport, out to the window's edges.
    let window = Rect::from_corners(rig.screen_to_world(Vec2::ZERO), rig.screen_to_world(rig.viewport));
    let veil = clear.0.with_alpha(VEIL);
    for q in [
        Rect::new(window.min.x, view.max.y, window.max.x, window.max.y),
        Rect::new(window.min.x, window.min.y, window.max.x, view.min.y),
        Rect::new(window.min.x, view.min.y, view.min.x, view.max.y),
        Rect::new(view.max.x, view.min.y, window.max.x, view.max.y),
    ] {
        if q.width() > 0.0 && q.height() > 0.0 {
            queue.push(FxSprite { rect: r.white.uv, pos: q.center(), angle: 0.0, size: q.size(), anchor: Vec2::ZERO, color: veil, z: Z_OVER });
        }
    }
    // Dots at the cell corners in view, a little larger where the cells are large. Where cells are smaller than
    // `MIN_DOT_GAP` pixels (a large frame zoomed out) only every 2nd, 4th, ... corner gets one, so the dots stay a
    // grid, not a carpet, and their number stays below about (viewport / gap)² whatever the frame.
    let dot = (1.6 + rig.zoom * 0.04).min(3.0) * px;
    let dot_c = style.dotted_ring.with_alpha(0.85);
    let stride = dot_stride(rig.zoom);
    let first = |a: f32| ((a.ceil() as i32).max(0) + stride - 1) / stride * stride;
    let (x0, x1) = (first(view.min.x), (view.max.x.floor() as i32).min(w));
    let (y0, y1) = (first(-view.max.y), ((-view.min.y).floor() as i32).min(h));
    for y in (y0..=y1).step_by(stride as usize) {
        for x in (x0..=x1).step_by(stride as usize) {
            queue.dot(&r.hard, Vec2::new(x as f32, -y as f32), dot, dot_c, Z_UNDER);
        }
    }
    // Corner brackets just outside the frame.
    let (arm, off, lw) = (16.0 * px, 9.0 * px, 1.5 * px);
    let bracket_c = ui_c.label.with_alpha(0.6);
    for (cx, cy, sx, sy) in [(0.0, 0.0, -1.0, 1.0), (w as f32, 0.0, 1.0, 1.0), (0.0, -h as f32, -1.0, -1.0), (w as f32, -h as f32, 1.0, -1.0)] {
        let corner = Vec2::new(cx + sx * off, cy + sy * off);
        if view.contains(corner) {
            queue.line(&r.white, corner, corner - Vec2::new(sx * arm, 0.0), lw, bracket_c, Z_OVER);
            queue.line(&r.white, corner, corner - Vec2::new(0.0, sy * arm), lw, bracket_c, Z_OVER);
        }
    }
    // The mirror axis: dashes down the frame's centre line.
    let x = w as f32 / 2.0;
    if design.dock().mirror && x >= view.min.x && x <= view.max.x {
        let (dash, gap) = (6.0 * px, 5.0 * px);
        let axis_c = ui_c.label.with_alpha(0.45);
        let (top, bottom) = ((-view.max.y).max(0.0), (-view.min.y).min(h as f32));
        let mut y = top;
        while y < bottom {
            let end = (y + dash).min(bottom);
            queue.line(&r.white, Vec2::new(x, -y), Vec2::new(x, -end), 1.2 * px, axis_c, Z_UNDER);
            y = end + gap;
        }
    }
    // The cell under the pointer.
    if let Some((cx, cy)) = cursor.cell {
        let c = Vec2::new(cx as f32 + 0.5, -(cy as f32) - 0.5);
        queue.push(FxSprite { rect: r.white.uv, pos: c, angle: 0.0, size: Vec2::ONE, anchor: Vec2::ZERO, color: ui_c.teal.with_alpha(0.1), z: Z_OVER });
        let teal = ui_c.teal.with_alpha(0.8);
        let corners = [c + Vec2::new(-0.5, 0.5), c + Vec2::new(0.5, 0.5), c + Vec2::new(0.5, -0.5), c + Vec2::new(-0.5, -0.5)];
        for k in 0..4 {
            queue.line(&r.white, corners[k], corners[(k + 1) % 4], 1.5 * px, teal, Z_OVER);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame's dots stay apart: one per corner where cells are large, every 2nd, 4th, ... where they are small.
    #[test]
    fn dots_keep_their_distance() {
        assert_eq!(dot_stride(30.0), 1);
        assert_eq!(dot_stride(MIN_DOT_GAP), 1);
        assert_eq!(dot_stride(5.0), 2);
        assert_eq!(dot_stride(1.0), 8);
        // A 1024 × 1024 frame fitted into a 900 px viewport: under 1 px per cell, every 8th corner, 129² dots.
        let zoom = 900.0 / 1025.2;
        assert_eq!(dot_stride(zoom), 8);
        assert!(dot_stride(0.0) >= 1 << 16);
    }
}
