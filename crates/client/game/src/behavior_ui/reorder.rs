//! Dragging a rule to another place in the priority list: press on a rule's row (a chip, `always` or the row's
//! empty space) and move the pointer up or down by more than a few pixels; a teal line shows where the rule will
//! go, and letting go moves it there (one edit, one undo step). A press that stays put is a click as before.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::PrimaryWindow;
use data::behavior::edit::{NodeRef, RuleId};
use kit::style::Style;
use kit::ui_kit::Hot;
use world::camera::CameraRig;
use world::fx::{FxQueue, FxRegions};

use super::actions::{BehaviorAction, BehaviorButton};
use super::draft::BehaviorDraft;
use super::edit::Edit;
use super::screen::{GraphViewport, ROOT_W, RowBand};

/// How far (logical px) the pointer moves up or down before a press on a row becomes a drag.
const START: f32 = 8.0;
/// Above the graph's dots, under the UI.
const Z_LINE: f32 = 5.0;

/// The rule being dragged.
#[derive(Resource, Default, Debug)]
pub struct RowDrag {
    /// The pressed rule and where the press started (physical px), until the press ends.
    press: Option<(RuleId, f32)>,
    /// Moving: the place the rule would go and the y of the line there (physical px).
    pub target: Option<(usize, f32)>,
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

/// Follows a press on a row; moves the rule when a drag ends.
#[allow(clippy::too_many_arguments)]
pub(super) fn drag_rows(
    window: Single<&Window, With<PrimaryWindow>>,
    touches: Res<Touches>,
    ui_scale: Res<UiScale>,
    draft: Res<BehaviorDraft>,
    mut drag: ResMut<RowDrag>,
    pressed: Query<(&Hot, &BehaviorButton)>,
    rows: Query<(&RowBand, &ComputedNode, &UiGlobalTransform)>,
    mut out: MessageWriter<BehaviorAction>,
) {
    let scale = window.scale_factor();
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
            *drag = RowDrag { press: Some((rule, at.y)), target: None };
            return;
        }
    }
    let Some((_, y0)) = drag.press else { return };
    if drag.target.is_none() && (at.y - y0).abs() < START * ui_scale.0 * scale {
        return;
    }
    // The other rows' centres and edges, top to bottom (one band cell per rule is enough).
    let Some(ed) = draft.editor() else { return };
    let mut spans: Vec<(usize, f32, f32)> = Vec::new();
    for (band, node, tf) in &rows {
        let Some(view) = ed.rule(band.rule()) else { continue };
        if band.rule() == rule || spans.iter().any(|s| s.0 == view.index) {
            continue;
        }
        spans.push((view.index, tf.translation.y, node.size.y / 2.0));
    }
    spans.sort_by_key(|s| s.0);
    let to = drop_index(&spans.iter().map(|s| s.1).collect::<Vec<_>>(), at.y);
    let line = match (to.checked_sub(1).and_then(|k| spans.get(k)), spans.get(to)) {
        (Some(above), _) => above.1 + above.2,
        (None, Some(below)) => below.1 - below.2,
        (None, None) => at.y,
    };
    drag.target = Some((to, line));
}

/// The line where the dragged rule would go, across the graph area from the trunk.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_drop_line(
    drag: Res<RowDrag>,
    rig: Res<CameraRig>,
    style: Res<Style>,
    ui_scale: Res<UiScale>,
    window: Single<&Window, With<PrimaryWindow>>,
    graph: Single<(&ComputedNode, &UiGlobalTransform), With<GraphViewport>>,
    regions: Option<Res<FxRegions>>,
    mut queue: ResMut<FxQueue>,
) {
    let (Some((_, y)), Some(r)) = (drag.target, regions) else { return };
    let (node, tf) = *graph;
    let scale = window.scale_factor();
    let area = Rect::from_center_size(tf.translation / scale, node.size / scale);
    let y = (y / scale).clamp(area.min.y, area.max.y);
    // From the trunk, after the root column.
    let x0 = area.min.x + ROOT_W * ui_scale.0;
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
