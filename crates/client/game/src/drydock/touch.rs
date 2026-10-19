//! Touch in the drydock (docs/drydock.md "Touch", `docs/plans/android.md` 4.3). The mouse is untouched by it
//! (`tools::pointer`, `view::camera`).
//!
//! * **On the grid** ([`Pad`], device-free and tested): one finger runs the current tool once it has stayed down alone
//!   for [`HOLD_SECS`] (a second finger before that turns the touch into a pan and pinch with nothing edited; after
//!   it, the gesture ends as when leaving the drydock: a stroke closes, a fill or move is dropped). Paint strokes,
//!   erase strokes and the fill rectangle follow the finger; SELECT grabs the part under it and moves it on release.
//!   A machine or a slope is not put down where the finger lands: its ghost hangs [`LIFT`] dp above the finger and
//!   lands there when the finger lifts ("hold, drop to place"), so the thumb doesn't hide it; a tap puts it under the
//!   finger. One finger that starts off the frame, or in SELECT on an empty cell, pans (a tap there deselects); two
//!   fingers pan and pinch.
//! * **From the palette** ([`palette`]): a part's row dragged sideways onto the grid picks it and carries its ghost
//!   [`LIFT`] dp above the finger; lifting the finger over the grid drops it there. Dragged up or down the list
//!   scrolls (the kit's finger scroll), a tap picks the part (the row's click).

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use data::Cell;
use kit::ui_kit::{Pill, UiPointer};
use world::camera::CameraRig;
use world::player::PlayerDesign;
use world::sim_bridge::GameContent;

use super::actions::DrydockAction;
use super::panels::{PaletteList, PaletteRow};
use super::state::{DrydockUi, Tool};
use super::tools::{Brush, Driver, DrydockGesture, EditNote, Gesture, Op, Selection, run_ops};
use super::view::{DockCamera, DrydockCursor};

/// How long one finger stays down alone before it edits (s): a second finger within it makes a pan and pinch with
/// nothing edited (*placeholder*, plan 4.3: 120 ms).
pub const HOLD_SECS: f32 = 0.12;
/// A touch shorter than this (s) that moved less than [`TAP_SLOP`] (dp) is a tap (*placeholders*).
pub const TAP_SECS: f32 = 0.3;
pub const TAP_SLOP: f32 = 10.0;
/// How far above the finger a part being placed hangs and lands (dp, *placeholder*, plan 4.3).
pub const LIFT: f32 = 48.0;

/// What a finger that lands alone does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// Nothing (it landed on a panel, off the grid): the touch is ignored until every finger is up.
    Ignore,
    /// The current tool.
    Tool,
    /// It moves the view.
    Pan,
}

/// What the fingers on the grid ask for (positions in logical window px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadOut {
    /// The tool starts where the finger landed.
    Press(Vec2),
    /// The finger, still down, is here.
    Drag(Vec2),
    /// The finger lifted here; `tap`: it was short and stayed put.
    Release { at: Vec2, tap: bool },
    /// The tool's gesture ends without being finished (a second finger came after [`HOLD_SECS`]).
    Cancel,
    /// A tap on a panning start (an empty cell in SELECT, off the frame).
    Tap(Vec2),
    /// Pan and zoom: the world point under `from` goes under `to`, the zoom times `factor`.
    View { factor: f32, from: Vec2, to: Vec2 },
}

/// Where a finger's touch started and how far it has gone from there.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Track {
    id: u64,
    start: Vec2,
    at: f32,
    far: f32,
    last: Vec2,
}

impl Track {
    fn new(id: u64, p: Vec2, now: f32) -> Self {
        Self { id, start: p, at: now, far: 0.0, last: p }
    }

    fn moved(&mut self, p: Vec2) {
        self.far = self.far.max(p.distance(self.start));
        self.last = p;
    }

    fn tap(&self, now: f32) -> bool {
        now - self.at < TAP_SECS && self.far < TAP_SLOP
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum State {
    #[default]
    Idle,
    /// Ignored until every finger is up.
    Ignored,
    /// One finger is down for the tool, which has not started yet.
    Pending(Track),
    /// One finger runs the tool.
    Tool(Track),
    /// One finger moves the view.
    Pan(Track),
    /// Two fingers: their ids, their distance and midpoint last frame.
    Pinch { a: u64, b: u64, dist: f32, mid: Vec2 },
}

/// The fingers on the grid, from the fingers that are down each frame to [`PadOut`]s.
#[derive(Resource, Default, Debug)]
pub struct Pad {
    state: State,
}

impl Pad {
    /// One frame: `fingers` are the fingers down now (id, logical px; the first one first), `now` in seconds,
    /// `start` says what a finger that lands alone does.
    pub fn update(&mut self, fingers: &[(u64, Vec2)], now: f32, start: impl Fn(Vec2) -> Start) -> Vec<PadOut> {
        let find = |id: u64| fingers.iter().find(|f| f.0 == id).map(|f| f.1);
        let rest = |fingers: &[(u64, Vec2)]| if fingers.is_empty() { State::Idle } else { State::Ignored };
        let mut out = Vec::new();
        self.state = match self.state {
            State::Idle => match fingers {
                [] => State::Idle,
                [(id, p)] => match start(*p) {
                    Start::Ignore => State::Ignored,
                    Start::Tool => State::Pending(Track::new(*id, *p, now)),
                    Start::Pan => State::Pan(Track::new(*id, *p, now)),
                },
                [(a, pa), (b, pb), ..] => match start((*pa + *pb) / 2.0) {
                    Start::Ignore => State::Ignored,
                    _ => pinch(*a, *pa, *b, *pb),
                },
            },
            State::Ignored => rest(fingers),
            State::Pending(mut t) => match (fingers, find(t.id)) {
                ([(a, pa), (b, pb), ..], _) => pinch(*a, *pa, *b, *pb),
                (_, None) => {
                    // Lifted before the hold: the tool's whole gesture at once.
                    out.push(PadOut::Press(t.start));
                    if t.last != t.start {
                        out.push(PadOut::Drag(t.last));
                    }
                    out.push(PadOut::Release { at: t.last, tap: t.tap(now) });
                    rest(fingers)
                }
                (_, Some(p)) => {
                    t.moved(p);
                    if now - t.at >= HOLD_SECS {
                        out.push(PadOut::Press(t.start));
                        if p != t.start {
                            out.push(PadOut::Drag(p));
                        }
                        State::Tool(t)
                    } else {
                        State::Pending(t)
                    }
                }
            },
            State::Tool(mut t) => match (fingers, find(t.id)) {
                ([(a, pa), (b, pb), ..], _) => {
                    out.push(PadOut::Cancel);
                    pinch(*a, *pa, *b, *pb)
                }
                (_, None) => {
                    out.push(PadOut::Release { at: t.last, tap: t.tap(now) });
                    rest(fingers)
                }
                (_, Some(p)) => {
                    if p != t.last {
                        out.push(PadOut::Drag(p));
                    }
                    t.moved(p);
                    State::Tool(t)
                }
            },
            State::Pan(mut t) => match (fingers, find(t.id)) {
                ([(a, pa), (b, pb), ..], _) => pinch(*a, *pa, *b, *pb),
                (_, None) => {
                    if t.tap(now) {
                        out.push(PadOut::Tap(t.start));
                    }
                    rest(fingers)
                }
                (_, Some(p)) => {
                    if p != t.last {
                        out.push(PadOut::View { factor: 1.0, from: t.last, to: p });
                    }
                    t.moved(p);
                    State::Pan(t)
                }
            },
            State::Pinch { a, b, dist, mid } => match (find(a), find(b)) {
                (Some(pa), Some(pb)) => {
                    let (d, m) = (pa.distance(pb).max(1.0), (pa + pb) / 2.0);
                    if d != dist || m != mid {
                        out.push(PadOut::View { factor: d / dist, from: mid, to: m });
                    }
                    State::Pinch { a, b, dist: d, mid: m }
                }
                // One finger left: it pans on (never a tap, never the tool).
                (Some(p), None) => State::Pan(Track { far: f32::INFINITY, ..Track::new(a, p, now) }),
                (None, Some(p)) => State::Pan(Track { far: f32::INFINITY, ..Track::new(b, p, now) }),
                (None, None) => rest(fingers),
            },
        };
        out
    }
}

fn pinch(a: u64, pa: Vec2, b: u64, pb: Vec2) -> State {
    State::Pinch { a, b, dist: pa.distance(pb).max(1.0), mid: (pa + pb) / 2.0 }
}

/// A pan and pinch step on a view (`pos` at the view centre `centre`, logical px, `zoom` px per metre): the world point
/// under `from` ends up under `to`, the zoom times `factor` within `min..=max`. The new position and zoom.
pub fn pinch_view(pos: Vec2, zoom: f32, centre: Vec2, factor: f32, from: Vec2, to: Vec2, (min, max): (f32, f32)) -> (Vec2, f32) {
    let flip = |d: Vec2| Vec2::new(d.x, -d.y);
    let world = pos + flip(from - centre) / zoom;
    let z = (zoom * factor).clamp(min, max);
    (world - flip(to - centre) / z, z)
}

/// The fingers down now, the pointer's first (the kit's `UiPointer::finger`), the rest in id order.
fn fingers(touches: &Touches, first: Option<u64>) -> Vec<(u64, Vec2)> {
    let mut f: Vec<(u64, Vec2)> = touches.iter().map(|t| (t.id(), t.position())).collect();
    f.sort_by_key(|(id, _)| (Some(*id) != first, *id));
    f
}

/// The frame cell under a logical window position, if it is on the frame.
fn cell_at(rig: &CameraRig, design: &PlayerDesign, p: Vec2) -> Option<Cell> {
    let w = rig.screen_to_world(p);
    let c = (w.x.floor() as i32, (-w.y).floor() as i32);
    let (fw, fh) = (design.frame().0 as i32, design.frame().1 as i32);
    (c.0 >= 0 && c.1 >= 0 && c.0 < fw && c.1 < fh).then_some(c)
}

/// Whether the paint tool with this brush holds the part above the finger ([`LIFT`]): machines and slopes.
fn lifts(tool: Tool, brush: Option<Brush>) -> bool {
    tool == Tool::Paint && matches!(brush, Some(Brush::Place | Brush::Slope))
}

/// The fingers on the grid ([`Pad`]): the tools, pan and pinch zoom. Runs after `view::camera` (which leaves the
/// cursor cell to this while a finger is down) and before `tools::pointer`.
#[allow(clippy::too_many_arguments)]
pub(super) fn grid(
    touches: Res<Touches>,
    pointer: Res<UiPointer>,
    time: Res<Time<Real>>,
    content: Res<GameContent>,
    ui: Res<DrydockUi>,
    mut cam: ResMut<DockCamera>,
    mut rig: ResMut<CameraRig>,
    mut pad: ResMut<Pad>,
    mut edit: EditParams,
) {
    let now = time.elapsed_secs();
    let fingers = fingers(&touches, pointer.finger());
    let design = &edit.design;
    let start = |p: Vec2| {
        if pointer.blocked || !cam.viewport.contains(p) || edit.g.by.is_some_and(|d| d != Driver::Finger && d != Driver::Lift) {
            return Start::Ignore;
        }
        match cell_at(&rig, design, p) {
            None => Start::Pan,
            Some(c) if ui.tool == Tool::Select && design.dock().part_at(c).is_none() => Start::Pan,
            Some(_) => Start::Tool,
        }
    };
    // A finger down and up within one frame (a fast tap, a test's) is seen down, then up.
    let quick: Vec<(u64, Vec2)> = touches.iter_just_released().filter(|t| touches.just_pressed(t.id())).map(|t| (t.id(), t.position())).collect();
    let mut outs = Vec::new();
    if !quick.is_empty() {
        outs = pad.update(&[fingers.as_slice(), quick.as_slice()].concat(), now, start);
    }
    outs.extend(pad.update(&fingers, now, start));
    let content = &content.0;
    let brush = ui.picked.as_deref().and_then(|p| content.parts.get(p)).map(Brush::of);
    let lift = Vec2::new(0.0, LIFT);
    for o in outs {
        let mut ops = Vec::new();
        let cell = |p: Vec2| cell_at(&rig, &edit.design, p);
        match o {
            PadOut::Press(p) if lifts(ui.tool, brush) => {
                edit.g.by = Some(Driver::Lift);
                edit.set_cursor(cell(p - lift));
            }
            PadOut::Press(p) => {
                let Some(c) = cell(p) else { continue };
                let (gesture, first) = Gesture::press(ui.tool, brush, c, edit.design.dock().part_at(c).is_some());
                *edit.g = DrydockGesture { gesture, by: Some(Driver::Finger) };
                edit.set_cursor(Some(c));
                ops = first;
            }
            PadOut::Drag(p) => match edit.g.by {
                Some(Driver::Lift) => edit.set_cursor(cell(p - lift)),
                Some(Driver::Finger) => {
                    if let Some(c) = cell(p) {
                        ops = edit.g.gesture.drag(c);
                        edit.set_cursor(Some(c));
                    }
                }
                _ => {}
            },
            PadOut::Release { at, tap } => {
                match edit.g.by {
                    Some(Driver::Lift) => {
                        if let Some(c) = cell(if tap { at } else { at - lift }) {
                            let (mut g, first) = Gesture::press(Tool::Paint, brush, c, false);
                            ops = first;
                            ops.extend(g.release());
                        }
                    }
                    Some(Driver::Finger) => ops = edit.g.gesture.release(),
                    _ => {}
                }
                *edit.g = DrydockGesture::default();
                edit.set_cursor(None);
            }
            PadOut::Cancel => {
                if edit.g.by == Some(Driver::Finger) && matches!(edit.g.gesture, Gesture::Stroke { .. }) {
                    ops.push(Op::EndStroke);
                }
                *edit.g = DrydockGesture::default();
                edit.set_cursor(None);
            }
            PadOut::Tap(_) => {
                if ui.tool == Tool::Select {
                    ops.push(Op::Select(None));
                }
            }
            PadOut::View { factor, from, to } => {
                let (pos, zoom) = pinch_view(rig.pos, rig.zoom, rig.view_center(), factor, from, to, cam.zoom_limits());
                rig.end_zoom_anchor();
                rig.pos = pos;
                rig.zoom = zoom;
                rig.zoom_target = zoom;
                cam.touched = true;
            }
        }
        run_ops(ops, content, &ui, &mut edit.design, &mut edit.sel, &mut edit.note, now);
    }
}

/// What the touch systems edit.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct EditParams<'w> {
    design: ResMut<'w, PlayerDesign>,
    sel: ResMut<'w, Selection>,
    g: ResMut<'w, DrydockGesture>,
    note: ResMut<'w, EditNote>,
    cursor: ResMut<'w, DrydockCursor>,
}

impl EditParams<'_> {
    /// The cell the ghost and the info line show (written only when it changes: the ghost works its view out then).
    fn set_cursor(&mut self, cell: Option<Cell>) {
        if self.cursor.cell != cell {
            self.cursor.cell = cell;
        }
    }
}

/// A palette row's drag onto the grid: the finger, the part, where it landed, and whether it carries the part yet.
#[derive(Resource, Default, Debug)]
pub struct PaletteDrag {
    finger: Option<u64>,
    part: String,
    start: Vec2,
    carrying: bool,
}

/// A finger that lands on a palette row and moves sideways (more than [`TAP_SLOP`], more across than along the list)
/// picks the part (and the paint tool) and carries its ghost [`LIFT`] dp above it; lifted over the grid it drops the
/// part there.
#[allow(clippy::too_many_arguments)]
pub(super) fn palette(
    touches: Res<Touches>,
    window: Single<&Window, With<PrimaryWindow>>,
    time: Res<Time<Real>>,
    content: Res<GameContent>,
    ui: Res<DrydockUi>,
    cam: Res<DockCamera>,
    rig: Res<CameraRig>,
    rows: Query<(&PaletteRow, &Pill, &ComputedNode, &UiGlobalTransform, &InheritedVisibility)>,
    list: Single<(&ComputedNode, &UiGlobalTransform), With<PaletteList>>,
    mut drag: ResMut<PaletteDrag>,
    mut actions: MessageWriter<DrydockAction>,
    mut edit: EditParams,
) {
    let scale = window.scale_factor();
    let inside = |(node, tf): (&ComputedNode, &UiGlobalTransform), p: Vec2| {
        let d = (p * scale - tf.translation).abs();
        d.x <= node.size.x / 2.0 && d.y <= node.size.y / 2.0
    };
    let Some(id) = drag.finger else {
        // A finger landing on a row (inside the list's visible area) of a part that can be picked.
        for t in touches.iter_just_pressed() {
            let p = t.position();
            if !inside(*list, p) {
                continue;
            }
            if let Some((row, ..)) = rows.iter().find(|(_, pill, n, tf, v)| v.get() && !pill.disabled && inside((n, tf), p)) {
                *drag = PaletteDrag { finger: Some(t.id()), part: row.0.clone(), start: p, carrying: false };
                break;
            }
        }
        return;
    };
    let lifted = |p: Vec2| {
        let q = p - Vec2::new(0.0, LIFT);
        cam.viewport.contains(q).then(|| cell_at(&rig, &edit.design, q)).flatten()
    };
    match touches.get_pressed(id) {
        Some(t) => {
            let d = t.position() - drag.start;
            if !drag.carrying && d.x.abs() > TAP_SLOP && d.x.abs() > d.y.abs() {
                drag.carrying = true;
                actions.write(DrydockAction::PickPart(drag.part.clone()));
                actions.write(DrydockAction::Tool(Tool::Paint));
                edit.g.by = Some(Driver::Lift);
            }
            if drag.carrying {
                edit.set_cursor(lifted(t.position()));
            }
        }
        None => {
            if drag.carrying {
                let at = touches.iter_just_released().find(|t| t.id() == id).map(|t| t.position());
                let part = content.0.parts.get(&drag.part);
                if let (Some(c), Some(part)) = (at.and_then(lifted), part)
                    && ui.picked.as_deref() == Some(drag.part.as_str())
                {
                    let (mut g, mut ops) = Gesture::press(Tool::Paint, Some(Brush::of(part)), c, false);
                    ops.extend(g.release());
                    run_ops(ops, &content.0, &ui, &mut edit.design, &mut edit.sel, &mut edit.note, time.elapsed_secs());
                }
                *edit.g = DrydockGesture::default();
                edit.set_cursor(None);
            }
            *drag = PaletteDrag::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u64 = 1;
    const B: u64 = 2;

    fn run(pad: &mut Pad, frames: &[(f32, &[(u64, Vec2)])], start: Start) -> Vec<PadOut> {
        frames.iter().flat_map(|(t, f)| pad.update(f, *t, |_| start)).collect()
    }

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    /// One finger edits after the hold: the press where it landed, its moves, the release.
    #[test]
    fn one_finger_runs_the_tool_after_the_hold() {
        let mut pad = Pad::default();
        let outs = run(
            &mut pad,
            &[(0.0, &[(A, v(10.0, 10.0))]), (0.05, &[(A, v(14.0, 10.0))]), (0.13, &[(A, v(20.0, 10.0))]), (0.2, &[(A, v(30.0, 12.0))]), (0.5, &[])],
            Start::Tool,
        );
        assert_eq!(
            outs,
            [PadOut::Press(v(10.0, 10.0)), PadOut::Drag(v(20.0, 10.0)), PadOut::Drag(v(30.0, 12.0)), PadOut::Release { at: v(30.0, 12.0), tap: false }]
        );
        assert_eq!(pad.state, State::Idle);
    }

    /// A quick tap is the whole gesture at once, marked as a tap.
    #[test]
    fn a_quick_tap_presses_and_releases() {
        let mut pad = Pad::default();
        let outs = run(&mut pad, &[(0.0, &[(A, v(5.0, 5.0))]), (0.08, &[(A, v(6.0, 5.0))]), (0.1, &[])], Start::Tool);
        assert_eq!(outs, [PadOut::Press(v(5.0, 5.0)), PadOut::Drag(v(6.0, 5.0)), PadOut::Release { at: v(6.0, 5.0), tap: true }]);
        // Held still past the hold but short: still a tap.
        let outs = run(&mut pad, &[(1.0, &[(A, v(5.0, 5.0))]), (1.2, &[(A, v(5.0, 5.0))]), (1.25, &[])], Start::Tool);
        assert_eq!(outs, [PadOut::Press(v(5.0, 5.0)), PadOut::Release { at: v(5.0, 5.0), tap: true }]);
    }

    /// A second finger within the hold: nothing is edited, the two fingers pinch; a second finger after it cancels
    /// the gesture first.
    #[test]
    fn a_second_finger_pinches_and_cancels_the_tool() {
        let mut pad = Pad::default();
        let outs = run(
            &mut pad,
            &[(0.0, &[(A, v(100.0, 100.0))]), (0.05, &[(A, v(100.0, 100.0)), (B, v(200.0, 100.0))]), (0.1, &[(A, v(50.0, 100.0)), (B, v(250.0, 100.0))])],
            Start::Tool,
        );
        assert_eq!(outs, [PadOut::View { factor: 2.0, from: v(150.0, 100.0), to: v(150.0, 100.0) }]);
        // Lifting both ends it without an edit.
        assert_eq!(pad.update(&[], 0.2, |_| Start::Tool), []);
        let outs = run(
            &mut pad,
            &[(1.0, &[(A, v(100.0, 100.0))]), (1.2, &[(A, v(110.0, 100.0))]), (1.3, &[(A, v(110.0, 100.0)), (B, v(210.0, 100.0))])],
            Start::Tool,
        );
        assert_eq!(outs, [PadOut::Press(v(100.0, 100.0)), PadOut::Drag(v(110.0, 100.0)), PadOut::Cancel]);
        // The pinch moves the midpoint: a pan as well as the zoom.
        let outs = pad.update(&[(A, v(130.0, 120.0)), (B, v(230.0, 120.0))], 1.4, |_| Start::Tool);
        assert_eq!(outs, [PadOut::View { factor: 1.0, from: v(160.0, 100.0), to: v(180.0, 120.0) }]);
        // One finger lifts: the other pans on, and its lift is no tap.
        let outs = run(&mut pad, &[(1.5, &[(B, v(230.0, 120.0))]), (1.55, &[(B, v(240.0, 120.0))]), (1.6, &[])], Start::Tool);
        assert_eq!(outs, [PadOut::View { factor: 1.0, from: v(230.0, 120.0), to: v(240.0, 120.0) }]);
    }

    /// A panning start moves the view at once; a tap there is a tap; an ignored start stays ignored until every
    /// finger is up.
    #[test]
    fn pans_taps_and_ignored_touches() {
        let mut pad = Pad::default();
        let outs = run(&mut pad, &[(0.0, &[(A, v(0.0, 0.0))]), (0.02, &[(A, v(5.0, -3.0))]), (0.5, &[])], Start::Pan);
        assert_eq!(outs, [PadOut::View { factor: 1.0, from: v(0.0, 0.0), to: v(5.0, -3.0) }]);
        let outs = run(&mut pad, &[(1.0, &[(A, v(0.0, 0.0))]), (1.1, &[])], Start::Pan);
        assert_eq!(outs, [PadOut::Tap(v(0.0, 0.0))]);
        let outs = run(&mut pad, &[(2.0, &[(A, v(0.0, 0.0))]), (2.3, &[(A, v(9.0, 0.0)), (B, v(50.0, 0.0))]), (2.4, &[(B, v(60.0, 0.0))])], Start::Ignore);
        assert_eq!(outs, []);
        assert_eq!(pad.update(&[], 2.5, |_| Start::Tool), []);
        assert_eq!(pad.state, State::Idle);
    }

    /// The pinch keeps the world point under the fingers' midpoint under it, within the zoom limits.
    #[test]
    fn the_pinch_keeps_the_point_under_the_fingers() {
        let centre = v(400.0, 200.0);
        let (pos, zoom) = (v(12.0, -12.0), 10.0);
        let at = |pos: Vec2, zoom: f32, s: Vec2| pos + Vec2::new(s.x - centre.x, centre.y - s.y) / zoom;
        let from = v(500.0, 150.0);
        let world = at(pos, zoom, from);
        // Zoom in around a point.
        let (p2, z2) = pinch_view(pos, zoom, centre, 2.0, from, from, (1.0, 100.0));
        assert_eq!(z2, 20.0);
        assert!(at(p2, z2, from).distance(world) < 1e-4);
        // Zoom and move: the point follows the midpoint.
        let to = v(450.0, 220.0);
        let (p3, z3) = pinch_view(pos, zoom, centre, 0.5, from, to, (1.0, 100.0));
        assert_eq!(z3, 5.0);
        assert!(at(p3, z3, to).distance(world) < 1e-4);
        // Clamped zoom: the point still stays under the fingers.
        let (p4, z4) = pinch_view(pos, zoom, centre, 50.0, from, from, (1.0, 30.0));
        assert_eq!(z4, 30.0);
        assert!(at(p4, z4, from).distance(world) < 1e-4);
        // A pan only.
        let (p5, z5) = pinch_view(pos, zoom, centre, 1.0, v(0.0, 0.0), v(10.0, 20.0), (1.0, 100.0));
        assert_eq!((p5, z5), (pos + v(-1.0, 2.0), zoom));
    }
}
