//! The BEHAVIOR screen under a finger (docs/plans/android.md 4.4, GDR-0119). The kit already makes every button a
//! 48 dp target, shows hover while a finger is down and scrolls the graph, the inspector and the palette up and down
//! under a finger. Here:
//!
//! * [`pan_graph`]: on a phone a finger dragged sideways on the graph pans it (a long rule is not cut off there).
//! * Reordering takes a hold first (`reorder.rs`), a slider a sideways drag or a tap (`controls.rs`).
//! * [`SwallowClick`]: a finger lifted after a pan or a rule's drag clicks nothing.
//!
//! Thresholds as the drydock's (`drydock::touch`): 10 dp before a finger's drag counts.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use kit::ui_kit::{LayoutClass, UiPointer};

use super::list::BehaviorList;
use super::reorder::RowDrag;
use super::screen::GraphViewport;
use super::{BehaviorUi, BehaviorView};

/// How far (dp) a finger moves before its drag pans, slides or stops being a hold.
pub const SLOP: f32 = crate::drydock::touch::TAP_SLOP;

/// The finger lifted this frame ended a pan or a rule's drag: its release clicks nothing (`actions::clicks`).
#[derive(Resource, Default, Debug)]
pub struct SwallowClick(pub bool);

/// A finger on the graph that may pan it sideways: the finger, where it was (physical px) and the graph's sideways
/// scroll then (UI px), and whether it pans yet.
#[derive(Clone, Copy, Debug)]
pub(super) struct Pan {
    id: u64,
    from: Vec2,
    start: f32,
    engaged: bool,
}

/// On a phone a finger dragged sideways on the graph (more across than up and down, past [`SLOP`]) pans it 1:1;
/// up and down is the kit's scroll. Not while a finger holds a rule.
#[allow(clippy::too_many_arguments)]
pub(super) fn pan_graph(
    window: Single<&Window, With<PrimaryWindow>>,
    touches: Res<Touches>,
    pointer: Res<UiPointer>,
    class: Res<LayoutClass>,
    ui: Res<BehaviorUi>,
    list: Res<BehaviorList>,
    rows: Res<RowDrag>,
    mut swallow: ResMut<SwallowClick>,
    graph: Single<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition), With<GraphViewport>>,
    mut pan: Local<Option<Pan>>,
) {
    let Some(id) = pointer.finger().filter(|_| class.is_compact() && ui.view == BehaviorView::Graph && !list.is_open()) else {
        *pan = None;
        return;
    };
    let scale = window.scale_factor();
    let (node, tf, mut pos) = graph.into_inner();
    let at = touches.get_pressed(id).map(|t| t.position() * scale);
    if touches.just_pressed(id) {
        *pan = at.filter(|a| node.contains_point(*tf, *a)).map(|from| Pan { id, from, start: pos.x, engaged: false });
    }
    let Some(p) = pan.as_mut().filter(|p| p.id == id) else { return };
    let Some(at) = at.filter(|_| !rows.holding()) else {
        // Lifted (or a rule taken): a pan's release clicks nothing.
        swallow.0 |= p.engaged && at.is_none();
        *pan = None;
        return;
    };
    let moved = at - p.from;
    if !p.engaged {
        if moved.x.abs() < SLOP * scale || moved.x.abs() <= moved.y.abs() {
            return;
        }
        // From the slop on (no jump by it, however far the finger got in one frame).
        p.engaged = true;
        p.from.x += SLOP * scale * moved.x.signum();
        p.start = pos.x;
    }
    let moved = at - p.from;
    let k = node.inverse_scale_factor;
    let max = ((node.content_size.x - node.size.x) * k).max(0.0);
    let x = (p.start - moved.x * k).clamp(0.0, max);
    if pos.x != x {
        pos.x = x;
    }
}
