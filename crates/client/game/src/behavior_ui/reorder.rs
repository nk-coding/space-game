//! Dragging a rule to another place in the priority list. With the mouse: press on a rule's row (a chip, `always`
//! or the row's empty space) and move the pointer up or down by more than a few pixels; a teal line shows where the
//! rule will go, and letting go moves it there (one edit, one undo step). A press that stays put is a click as before.
//! With a finger, whose plain drag scrolls the graph (docs/plans/android.md 4.4): hold the row still for [`HOLD`]
//! (the line shows), then drag; the graph keeps its place meanwhile and scrolls on its own near its top and bottom
//! edges. A finger that moves before the hold is up scrolls (or pans) as usual.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::PrimaryWindow;
use data::behavior::edit::{NodeRef, RuleId};
use kit::style::Style;
use kit::ui_kit::{Hot, LayoutClass, UiPointer};
use world::camera::CameraRig;
use world::fx::{FxQueue, FxRegions};

use super::actions::{BehaviorAction, BehaviorButton};
use super::draft::BehaviorDraft;
use super::edit::Edit;
use super::screen::{GraphViewport, RowBand, graph_dims};
use super::touch::{SLOP, SwallowClick};

/// How far (logical px) the pointer moves up or down before a press on a row becomes a drag.
const START: f32 = 8.0;
/// How long (seconds) a finger holds a row still (within `touch::SLOP`) before it drags the rule (*placeholder*).
pub const HOLD: f32 = 0.35;
/// Within this far (logical px) of the graph's top or bottom edge a held rule scrolls the graph, this fast (logical
/// px a second; *placeholders*).
const EDGE: f32 = 36.0;
const EDGE_SPEED: f32 = 360.0;
/// Above the graph's dots, under the UI.
const Z_LINE: f32 = 5.0;

/// The rule being dragged.
#[derive(Resource, Default, Debug)]
pub struct RowDrag {
    /// The pressed rule and where the press started (physical px), until the press ends (the mouse).
    press: Option<(RuleId, f32)>,
    /// A finger on a row (touch).
    finger: Option<FingerHold>,
    /// Moving: the place the rule would go and the y of the line there (physical px).
    pub target: Option<(usize, f32)>,
}

/// A finger on a rule's row: the finger, the rule, where and when it went down (physical px, seconds), and once it
/// holds the rule, the graph's scroll it keeps (UI px).
#[derive(Clone, Copy, Debug)]
struct FingerHold {
    id: u64,
    rule: RuleId,
    from: Vec2,
    since: f32,
    scroll: Option<f32>,
}

impl RowDrag {
    /// A finger holds a rule: the graph neither scrolls nor pans under it.
    pub fn holding(&self) -> bool {
        self.finger.is_some_and(|f| f.scroll.is_some())
    }
}

/// Where a dragged rule lands: after every other rule whose row centre is above the pointer. `centres` are the
/// other rules' row centres, top to bottom.
pub fn drop_index(centres: &[f32], y: f32) -> usize {
    centres.iter().filter(|c| **c < y).count()
}

/// The rule a pressed widget belongs to.
fn rule_of(button: &BehaviorAction, draft: &BehaviorDraft) -> Option<RuleId> {
    let ed = draft.editor()?;
    match button {
        BehaviorAction::SelectRule(r) | BehaviorAction::Select(NodeRef::Action(r)) => Some(*r),
        BehaviorAction::Select(NodeRef::Cond(c)) => ed.rule_of(*c),
        _ => None,
    }
}

/// Where `rule` would go with the pointer at `y` (physical px), and the y of the line there.
fn target_at(rule: RuleId, y: f32, draft: &BehaviorDraft, rows: &Query<(&RowBand, &ComputedNode, &UiGlobalTransform)>) -> Option<(usize, f32)> {
    // The other rows' centres and edges, top to bottom (one band cell per rule is enough).
    let ed = draft.editor()?;
    let mut spans: Vec<(usize, f32, f32)> = Vec::new();
    for (band, node, tf) in rows {
        let Some(view) = ed.rule(band.rule()) else { continue };
        if band.rule() == rule || spans.iter().any(|s| s.0 == view.index) {
            continue;
        }
        spans.push((view.index, tf.translation.y, node.size.y / 2.0));
    }
    spans.sort_by_key(|s| s.0);
    let to = drop_index(&spans.iter().map(|s| s.1).collect::<Vec<_>>(), y);
    let line = match (to.checked_sub(1).and_then(|k| spans.get(k)), spans.get(to)) {
        (Some(above), _) => above.1 + above.2,
        (None, Some(below)) => below.1 - below.2,
        (None, None) => y,
    };
    Some((to, line))
}

/// Follows a press on a row; moves the rule when a drag ends.
#[allow(clippy::too_many_arguments)]
pub(super) fn drag_rows(
    window: Single<&Window, With<PrimaryWindow>>,
    touches: Res<Touches>,
    time: Res<Time<Real>>,
    pointer: Res<UiPointer>,
    ui_scale: Res<UiScale>,
    draft: Res<BehaviorDraft>,
    mut drag: ResMut<RowDrag>,
    mut swallow: ResMut<SwallowClick>,
    pressed: Query<(&Hot, &BehaviorButton)>,
    rows: Query<(&RowBand, &ComputedNode, &UiGlobalTransform)>,
    graph: Single<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition), With<GraphViewport>>,
    mut out: MessageWriter<BehaviorAction>,
) {
    let scale = window.scale_factor();
    if let Some(id) = pointer.finger() {
        drag.press = None;
        let held = pressed.iter().find(|(hot, _)| hot.pressed).and_then(|(_, b)| rule_of(&b.0, &draft));
        let step = FingerStep { id, scale, ui: ui_scale.0, now: time.elapsed_secs(), dt: time.delta_secs(), held };
        hold_rows(step, &touches, &draft, &mut drag, &mut swallow, &rows, graph.into_inner(), &mut out);
        return;
    }
    drag.finger = None;
    let pointer = window.physical_cursor_position().or_else(|| touches.first_pressed_position().map(|p| p * scale));
    let held = pressed.iter().find(|(hot, _)| hot.pressed).and_then(|(_, b)| rule_of(&b.0, &draft));
    let Some(rule) = held else {
        // Let go: a drag moves the rule.
        if let (Some((rule, _)), Some((to, _))) = (drag.press.take(), drag.target.take()) {
            out.write(BehaviorAction::Edit(Edit::MoveRule { rule, to }));
        }
        return;
    };
    let Some(at) = pointer else { return };
    match drag.press {
        Some((r, _)) if r == rule => {}
        _ => {
            *drag = RowDrag { press: Some((rule, at.y)), ..default() };
            return;
        }
    }
    let Some((_, y0)) = drag.press else { return };
    if drag.target.is_none() && (at.y - y0).abs() < START * ui_scale.0 * scale {
        return;
    }
    if let Some(t) = target_at(rule, at.y, &draft, &rows) {
        drag.target = Some(t);
    }
}

/// A frame of a finger: which, the window's and the UI's scale, the time and the frame's length (seconds), and the
/// rule whose row it pressed (while the press lasts).
struct FingerStep {
    id: u64,
    scale: f32,
    ui: f32,
    now: f32,
    dt: f32,
    held: Option<RuleId>,
}

/// A finger on a row: held still for [`HOLD`] it takes the rule; then the graph keeps its scroll (scrolling near its
/// edges) and lifting the finger moves the rule (the click of that release is swallowed).
#[allow(clippy::too_many_arguments)]
fn hold_rows(
    f: FingerStep,
    touches: &Touches,
    draft: &BehaviorDraft,
    drag: &mut RowDrag,
    swallow: &mut SwallowClick,
    rows: &Query<(&RowBand, &ComputedNode, &UiGlobalTransform)>,
    (node, tf, mut pos): (&ComputedNode, &UiGlobalTransform, Mut<ScrollPosition>),
    out: &mut MessageWriter<BehaviorAction>,
) {
    let down = touches.get_pressed(f.id).map(|t| t.position() * f.scale);
    if touches.just_pressed(f.id) {
        drag.target = None;
        drag.finger = f.held.zip(down).map(|(rule, from)| FingerHold { id: f.id, rule, from, since: f.now, scroll: None });
    }
    let Some(mut hold) = drag.finger.filter(|h| h.id == f.id) else {
        drag.finger = None;
        drag.target = None;
        return;
    };
    let Some(at) = down else {
        // Lifted: a held rule goes where the line is.
        if hold.scroll.is_some() {
            if let Some((to, _)) = drag.target {
                out.write(BehaviorAction::Edit(Edit::MoveRule { rule: hold.rule, to }));
            }
            swallow.0 = true;
        }
        drag.finger = None;
        drag.target = None;
        return;
    };
    match hold.scroll {
        None => {
            // Moved first: a scroll or a pan, not a hold.
            if (at - hold.from).length() > SLOP * f.scale {
                drag.finger = None;
                return;
            }
            if f.now - hold.since < HOLD {
                return;
            }
            hold.scroll = Some(pos.y);
        }
        Some(y) => {
            // The graph stays where it was (the kit scrolls a list under a moving finger), but near its top or bottom
            // edge it scrolls on.
            let area = Rect::from_center_size(tf.translation, node.size);
            let edge = EDGE * f.ui * f.scale;
            let dir = if at.y < area.min.y + edge {
                -1.0
            } else if at.y > area.max.y - edge {
                1.0
            } else {
                0.0
            };
            let max = ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0);
            let y = (y + dir * EDGE_SPEED * f.dt).clamp(0.0, max);
            hold.scroll = Some(y);
            if pos.y != y {
                pos.y = y;
            }
        }
    }
    drag.finger = Some(hold);
    if let Some(t) = target_at(hold.rule, at.y, draft, rows) {
        drag.target = Some(t);
    }
}

/// The line where the dragged rule would go, across the graph area from the trunk.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_drop_line(
    drag: Res<RowDrag>,
    rig: Res<CameraRig>,
    style: Res<Style>,
    ui_scale: Res<UiScale>,
    class: Res<LayoutClass>,
    window: Single<&Window, With<PrimaryWindow>>,
    graph: Single<(&ComputedNode, &UiGlobalTransform, &ScrollPosition), With<GraphViewport>>,
    regions: Option<Res<FxRegions>>,
    mut queue: ResMut<FxQueue>,
) {
    let (Some((_, y)), Some(r)) = (drag.target, regions) else { return };
    let (node, tf, scroll) = *graph;
    let scale = window.scale_factor();
    let area = Rect::from_center_size(tf.translation / scale, node.size / scale);
    let y = (y / scale).clamp(area.min.y, area.max.y);
    // From the trunk, after the root column (which a phone's graph may have panned to the left).
    let x0 = (area.min.x + (graph_dims(*class).root_w - scroll.x) * ui_scale.0).max(area.min.x);
    let (a, b) = (rig.screen_to_world(Vec2::new(x0, y)), rig.screen_to_world(Vec2::new(area.max.x - 20.0 * ui_scale.0, y)));
    queue.line(&r.white, a, b, 2.0 * ui_scale.0 / rig.zoom, style.ui.teal.with_alpha(0.9), Z_LINE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dragged_rule_lands_after_the_rows_above_the_pointer() {
        let centres = [100.0, 150.0, 200.0];
        assert_eq!(drop_index(&centres, 20.0), 0);
        assert_eq!(drop_index(&centres, 120.0), 1);
        assert_eq!(drop_index(&centres, 175.0), 2);
        assert_eq!(drop_index(&centres, 900.0), 3);
        assert_eq!(drop_index(&[], 50.0), 0);
    }
}
