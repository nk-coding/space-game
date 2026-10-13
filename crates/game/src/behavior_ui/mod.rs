//! The BEHAVIOR screen (GDR-0057): how the design on the drydock's grid thinks, in the look of
//! `docs/concept/boards/ui-mockups/phone-behavior-editor.png` (laid out for desktop, 1280 × 720 and up). Opened
//! with the drydock's BEHAVIOR button ([`AppMode::Behavior`]); Esc or the DRYDOCK pill go back, the design
//! untouched. `?scene=behavior` starts here; `?behavior=<id>` opens a content or saved behavior instead of the
//! design's.
//!
//! The mode, the screen and its widgets, the graph and text views of the behavior, the inspector and the
//! palette; editing in the graph view goes through the editing model (`data::behavior_edit`); behaviors as saved
//! documents (SAVE, the BEHAVIORS list, the design's behavior) and TEST IN RANGE with its live panel in the flight
//! HUD.
//!
//! * [`draft`]: [`BehaviorDraft`], the behavior being edited: the editing model, the text, the errors.
//! * [`graph`]: the graph view as data (the program → chips and rows with the model's stable ids, the slot line,
//!   labels).
//! * [`inspect`]: the inspector (the selected node's or rule's controls) and the ADD NODE palette as data.
//! * [`values`]: the inspector's numbers: kinds, ranges, steps, slider scales, metres ↔ % of the weapon reach.
//! * [`edit`]: editing: [`edit::Edit`], what palette clicks, `+ RULE` and Delete do, the handler, undo / redo.
//! * [`actions`]: [`BehaviorAction`], the one message the screen's buttons and keys send; the UI-state handler.
//! * [`screen`]: the screen's nodes (`ui_kit` widgets), rebuilt from the data above when it changes.
//! * [`controls`]: the inspector's controls as nodes; slider drags and note fields.
//! * [`reorder`]: dragging a rule's row to another place in the list.
//! * [`text_view`]: the TEXT view (an egui text editor in the look) with live errors.
//! * [`dashed`]: the dashed pill outline (Rhai nodes, `+ RULE`), a UI material.
//! * [`saving`]: the name and saved state in the header, SAVE, the working draft (unsaved edits survive leaving
//!   and reloading), linking a behavior to the design, TEST IN RANGE.
//! * [`list`]: the BEHAVIORS list (the player's behaviors and the templates: open, use for this design, duplicate,
//!   rename, delete, new).
//! * [`live`]: the live rule panel in the flight HUD while a test runs.
//!
//! **For handlers** (the range scenes): read `MessageReader<BehaviorAction>` in a system in
//! [`BehaviorSet::Apply`]; change the program through [`BehaviorDraft::edit`] (the graph and the text
//! follow its `revision`), read [`BehaviorUi`] (the view, the selected node, the test mode) and
//! [`EditorContext`] (the design's name and rule slots). Nothing in `screen.rs` needs to change for that.

pub mod actions;
pub mod controls;
pub mod dashed;
pub mod draft;
pub mod edit;
pub mod graph;
pub mod inspect;
pub mod list;
pub mod live;
pub mod reorder;
pub mod saving;
pub mod screen;
pub mod text_view;
pub mod values;

use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiPrimaryContextPass;

use crate::camera::{CameraRig, CameraSet};
use crate::fx::FxFlush;
use crate::mode::AppMode;
use crate::ui_kit::{UiInputSet, UiStyleSet};

pub use actions::BehaviorAction;
pub use draft::BehaviorDraft;

use inspect::Selection;

/// The behavior screen's `Update` systems, in order, only on the screen: input (clicks and keys become
/// [`BehaviorAction`]s) → apply (handlers) → show (the nodes follow the state).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BehaviorSet {
    Input,
    Apply,
    Show,
}

/// GRAPH or TEXT.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BehaviorView {
    #[default]
    Graph,
    Text,
}

/// The two modes of TEST IN RANGE (GDR-0057): WATCH and FLY ALONGSIDE.
pub use crate::test_range::TestMode;

/// The screen's UI state.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BehaviorUi {
    pub view: BehaviorView,
    /// What the inspector shows (`None`: the behavior itself).
    pub selected: Option<Selection>,
    pub test_mode: TestMode,
    /// The palette search.
    pub search: String,
}

/// What the screen edits for: the design's name and its rule slots (from its logic cores), set when the
/// screen opens.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct EditorContext {
    pub design_name: String,
    pub slots: u32,
}

/// The layout's size: 1280 × 720 logical px; larger windows scale the screen (as the drydock does).
const MAX_UI_SCALE: f32 = 2.0;

pub struct BehaviorPlugin;

impl Plugin for BehaviorPlugin {
    fn build(&self, app: &mut App) {
        dashed::plugin(app);
        app.init_resource::<BehaviorDraft>()
            .init_resource::<BehaviorUi>()
            .init_resource::<EditorContext>()
            .init_resource::<screen::GraphBuilt>()
            .init_resource::<edit::EditState>()
            .init_resource::<controls::SliderDrag>()
            .init_resource::<controls::DesignReach>()
            .init_resource::<reorder::RowDrag>()
            .init_resource::<saving::DraftSlot>()
            .init_resource::<saving::ScreenNote>()
            .init_resource::<list::BehaviorList>()
            .add_message::<BehaviorAction>()
            .configure_sets(
                Update,
                (BehaviorSet::Input, BehaviorSet::Apply, BehaviorSet::Show)
                    .chain()
                    .after(UiInputSet)
                    .before(UiStyleSet)
                    .run_if(in_state(AppMode::Behavior)),
            )
            .add_systems(Startup, (screen::spawn, list::spawn, live::spawn).after(crate::flight_hud::spawn_hud))
            .add_systems(PostStartup, show)
            .add_systems(OnEnter(AppMode::Behavior), (saving::open, controls::measure_reach))
            .add_systems(OnExit(AppMode::Behavior), (controls::drop_drags, list::leave, saving::leave, text_view::leave).chain())
            .add_systems(OnTransition { exited: AppMode::Behavior, entered: AppMode::Flight }, |mut s: ResMut<UiScale>| s.0 = 1.0)
            .add_systems(Update, show.run_if(state_changed::<AppMode>))
            .add_systems(
                Update,
                (
                    (actions::clicks, actions::shortcuts, screen::sync_search, controls::commit_notes, reorder::drag_rows).in_set(BehaviorSet::Input),
                    // One order: the UI state, then what replaces or saves the draft, then edits of the draft on the
                    // screen (they see a replaced draft's new ids), then the autosave (it sees every edit of the frame,
                    // so leaving the screen in the same frame keeps it).
                    (
                        fit_window,
                        actions::apply,
                        (saving::title, saving::apply, list::apply, list::scroll).chain(),
                        edit::apply,
                        controls::drag_sliders,
                        screen::scroll,
                        saving::autosave,
                    )
                        .chain()
                        .in_set(BehaviorSet::Apply),
                    (
                        screen::show_view,
                        screen::rebuild_graph,
                        screen::rebuild_inspector,
                        screen::rebuild_palette,
                        screen::sync_texts,
                        screen::sync_note,
                        screen::sync_pills,
                        edit::sync_rings,
                        screen::style_chips,
                        screen::style_rows,
                    )
                        .chain()
                        .in_set(BehaviorSet::Show),
                ),
            )
            .add_systems(Update, (saving::sync_title, list::sync).in_set(BehaviorSet::Show))
            // The live rule panel in the flight HUD (TEST IN RANGE).
            .add_systems(Update, live::update.run_if(in_state(AppMode::Flight)))
            .add_systems(EguiPrimaryContextPass, text_view::editor.run_if(in_state(AppMode::Behavior)))
            .add_systems(PostUpdate, (screen::draw_dots, reorder::draw_drop_line).run_if(in_state(AppMode::Behavior)).after(CameraSet).before(FxFlush));
        // Opening the screen from the drydock.
        app.add_systems(Update, actions::open_from_drydock.in_set(crate::drydock::DockSet::Apply));
    }
}

/// The screen is shown only in its mode.
fn show(mode: Res<State<AppMode>>, mut root: Query<&mut Visibility, With<screen::BehaviorRoot>>) {
    let want = if *mode.get() == AppMode::Behavior { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut root {
        if *v != want {
            *v = want;
        }
    }
}

/// The window: the UI scale (min(w / 1280, h / 720), 1 to 2) and the camera rig's viewport, which places the
/// icons and the dot grid (world sprites under the UI) on screen.
fn fit_window(window: Single<&Window, With<PrimaryWindow>>, mut ui_scale: ResMut<UiScale>, mut rig: ResMut<CameraRig>) {
    let s = (window.width() / 1280.0).min(window.height() / 720.0).clamp(1.0, MAX_UI_SCALE);
    if (ui_scale.0 - s).abs() > 1e-3 {
        ui_scale.0 = s;
    }
    if rig.viewport != window.size() {
        rig.viewport = window.size();
    }
}
