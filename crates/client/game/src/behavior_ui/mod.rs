//! The BEHAVIOR screen (GDR-0057): how the design on the drydock's grid thinks, in the look of
//! the concept's phone behavior-editor mockup (laid out for desktop, 1280 × 720 and up). Opened
//! with the drydock's BEHAVIOR button or from home ([`AppMode::Behavior`]); Esc goes back the way it came, the
//! DRYDOCK pill to the drydock, the design untouched. `?scene=behavior` starts here; `?behavior=<id>` opens a content or saved behavior instead of the
//! design's.
//!
//! The mode, the screen and its widgets, the graph and text views of the behavior, the inspector and the
//! palette; editing in the graph view goes through the editing model (`data::behavior::edit`); behaviors as saved
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
//! * [`assists`]: the PILOT ASSISTS tab (GDR-0088): the design's pilot assists in the same graph and text views,
//!   inspector and palette, saved with the design.
//! * [`live`]: the live rule panel in the flight HUD while a test runs.
//!
//! **For handlers** (the ring menu, later ones): read `MessageReader<BehaviorAction>` in a system in
//! [`BehaviorSet::Apply`]; change the program through [`BehaviorDraft::edit`] (the graph and the text
//! follow its `revision`), read [`BehaviorUi`] (the tab, the view, the selected node, the test mode) and
//! [`EditorContext`] (the design's name and rule slots).

pub mod actions;
pub mod assists;
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
use bevy::state::state::StateTransitionEvent;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiPrimaryContextPass;
use kit::ui_kit::{ScreenFx, show_screen, snap_screen, UiInputSet, UiStyleSet};
use world::camera::{CameraRig, CameraSet};
use world::fx::FxFlush;
use world::mode::AppMode;
use world::player::PlayerDesign;
use world::sim_bridge::GameContent;

use crate::shipyard::Yard;
use crate::shipyard::layout::{Place, fit};
use crate::drydock::report::DrydockReport;

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

/// BEHAVIOR or PILOT ASSISTS: which of the design's two rule lists the screen edits (GDR-0088).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BehaviorTab {
    #[default]
    Behavior,
    Assists,
}

/// GRAPH or TEXT: how the open list is shown, one choice for both tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BehaviorView {
    #[default]
    Graph,
    Text,
}

/// The two modes of TEST IN RANGE (GDR-0057): WATCH and FLY ALONGSIDE.
pub use world::test_range::TestMode;

/// The screen's UI state.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BehaviorUi {
    pub tab: BehaviorTab,
    pub view: BehaviorView,
    /// What the inspector shows (`None`: the behavior itself).
    pub selected: Option<Selection>,
    pub test_mode: TestMode,
    /// The palette search.
    pub search: String,
    /// The screen was opened from home (the shipyard's behavior building), so Esc goes back there; from the
    /// drydock it goes back to the drydock. A test flight keeps the way in.
    pub from_home: bool,
}

/// What the screen edits for: the design's name and its rule slots (from its logic cores), set when the
/// screen opens.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct EditorContext {
    pub design_name: String,
    pub slots: u32,
}

/// The flight zoom to give back when the screen, opened from home, closes (the camera rests on the mind meanwhile).
#[derive(Resource, Debug, Default)]
struct BehaviorCamera {
    flight_zoom: f32,
}

pub struct BehaviorPlugin;

impl Plugin for BehaviorPlugin {
    fn build(&self, app: &mut App) {
        dashed::plugin(app);
        app.init_resource::<BehaviorDraft>()
            .init_resource::<BehaviorUi>()
            .init_resource::<EditorContext>()
            .init_resource::<BehaviorCamera>()
            .init_resource::<screen::GraphBuilt>()
            .init_resource::<edit::EditState>()
            .init_resource::<controls::SliderDrag>()
            .init_resource::<controls::DesignReach>()
            .init_resource::<reorder::RowDrag>()
            .init_resource::<saving::DraftSlot>()
            .init_resource::<saving::ScreenNote>()
            .init_resource::<list::BehaviorList>()
            .init_resource::<assists::AssistTab>()
            .add_message::<BehaviorAction>()
            .configure_sets(
                Update,
                (BehaviorSet::Input, BehaviorSet::Apply, BehaviorSet::Show)
                    .chain()
                    .after(UiInputSet)
                    .before(UiStyleSet)
                    .run_if(in_state(AppMode::Behavior)),
            )
            .add_systems(Startup, (screen::spawn, list::spawn, live::spawn).after(world::flight_hud::spawn_hud))

            // The design report (the drydock's, `drydock::report`) is brought up to date first: the reach and TEST IN
            // RANGE read it.
            .add_systems(OnEnter(AppMode::Behavior), (actions::way_in, remember_flight_zoom, design_report, (saving::open, controls::measure_reach)).chain())
            .add_systems(OnExit(AppMode::Behavior), (assists::leave, give_back_zoom, controls::drop_drags, list::leave, saving::leave, text_view::leave).chain())
            .add_systems(OnTransition { exited: AppMode::Behavior, entered: AppMode::Flight }, |mut s: ResMut<UiScale>, size: Res<kit::ui_kit::UiSize>| s.0 = size.0)
            .add_systems(Update, show.run_if(state_changed::<AppMode>))
            .add_systems(PostUpdate, screen::draw_dots.after(CameraSet).before(FxFlush))
            .add_systems(
                Update,
                (
                    (actions::clicks, actions::shortcuts, screen::sync_search, controls::commit_notes, reorder::drag_rows).in_set(BehaviorSet::Input),
                    // One order: the UI state, then what replaces or saves the draft, then edits of the draft on the
                    // screen (they see a replaced draft's new ids), then the autosave (it sees every edit of the frame,
                    // so leaving the screen in the same frame keeps it).
                    (
                        fit_window,
                        hold_camera,
                        actions::apply,
                        assists::switch,
                        design_report,
                        (saving::title, saving::apply, list::apply, list::scroll).chain(),
                        edit::apply,
                        controls::drag_sliders,
                        assists::write,
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
            .add_systems(PostUpdate, reorder::draw_drop_line.run_if(in_state(AppMode::Behavior)).after(CameraSet).before(FxFlush));
        // Opening the screen from the drydock.
        app.add_systems(Update, actions::open_from_drydock.in_set(crate::drydock::DockSet::Apply));
    }
}

/// Opened from home: the flight zoom to give back on leaving (home's exit has just given it back).
fn remember_flight_zoom(ui: Res<BehaviorUi>, rig: Res<CameraRig>, mut cam: ResMut<BehaviorCamera>) {
    if ui.from_home {
        cam.flight_zoom = rig.zoom_target;
    }
}

/// Opened from home, the camera rests on the mind (the shipyard's behavior building), exactly where home's flight
/// there ended, so it shows faintly behind the screen as the lab behind research; refitted when the window changes.
/// From the drydock the drydock's parked view stays.
fn hold_camera(ui: Res<BehaviorUi>, window: Single<&Window, With<PrimaryWindow>>, yard: Res<Yard>, mut rig: ResMut<CameraRig>) {
    if !ui.from_home {
        return;
    }
    let size = window.size();
    let (centre, zoom) = fit(yard.layout.focus(Place::Behavior), size);
    if rig.pos != centre || rig.zoom != zoom || rig.view_offset != Vec2::ZERO {
        rig.view_offset = Vec2::ZERO;
        rig.end_zoom_anchor();
        rig.pos = centre;
        rig.zoom = zoom;
        rig.zoom_target = zoom;
    }
}

/// Leaving a screen opened from home: flight (TEST IN RANGE), the drydock and home get the flight zoom back.
fn give_back_zoom(ui: Res<BehaviorUi>, cam: Res<BehaviorCamera>, mut rig: ResMut<CameraRig>) {
    if ui.from_home {
        rig.view_offset = Vec2::ZERO;
        rig.zoom = cam.flight_zoom;
        rig.zoom_target = cam.flight_zoom;
    }
}

/// The drydock's design report (`drydock::report`), brought up to date on this screen too: the weapon reach and
/// TEST IN RANGE read it. (A system of its own: the drydock's schedule orders its systems by `report::update`'s
/// name, which must stay unique there.)
fn design_report(design: Res<PlayerDesign>, content: Res<GameContent>, time: Res<Time<Real>>, out: ResMut<DrydockReport>) {
    crate::drydock::report::update(design, content, time, out);
}

/// The screen is shown only in its mode, opening and closing with its transition (`ui_kit::ScreenFx`); to and from
/// flight it snaps (the UI scale and the world change at once there).
fn show(mode: Res<State<AppMode>>, mut transitions: MessageReader<StateTransitionEvent<AppMode>>, mut root: Single<&mut ScreenFx, With<screen::BehaviorRoot>>) {
    let here = *mode.get() == AppMode::Behavior;
    let flight = transitions.read().last().is_some_and(|t| t.entered == Some(AppMode::Flight));
    if !here && flight {
        snap_screen(&mut root, false);
    } else {
        show_screen(&mut root, here);
    }
}

/// The window: the UI scale (min(w / 1280, h / 720), 1 to 2) and the camera rig's viewport, which places the
/// icons and the dot grid (world sprites under the UI) on screen.
fn fit_window(window: Single<&Window, With<PrimaryWindow>>, size: Res<kit::ui_kit::UiSize>, mut ui_scale: ResMut<UiScale>, mut rig: ResMut<CameraRig>) {
    let s = kit::ui_kit::ui_scale_for(&window, *size);
    if (ui_scale.0 - s).abs() > 1e-3 {
        ui_scale.0 = s;
    }
    if rig.viewport != window.size() {
        rig.viewport = window.size();
    }
}
