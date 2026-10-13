//! The drydock: the ship editor of the client (Drydock v1, decided by Niklas on 12 Oct 2026; the look follows
//! `docs/concept/boards/ui-mockups/desktop-drydock.png`). This is the foundation: the mode, the static view of
//! the player's design, the layout with its widgets, and the test flight; editing, the live stats, saving and
//! sharing (wave 2, 13 Oct 2026) are the modules below.
//!
//! * [`design`]: [`PlayerDesign`], the blueprint every flight of the player flies.
//! * [`state`]: what the widgets show (`DrydockUi`, `DrydockStatus`, `DrydockStats`).
//! * [`actions`]: `DrydockAction`, the one message buttons and keys send; the UI-state handler.
//! * [`view`]: the design drawn with the flight tiles, the frame overlay, the camera, `DrydockCursor`.
//! * [`panels`]: the screen (built from `ui_kit` widgets and `ui_icons`).
//! * [`report`]: [`report::DrydockReport`], the design's `sim::report` per revision.
//! * [`stats`]: the stats grid and the status line (whether the design can fly) from the report.
//! * [`overlays`]: the HEAT, MASS, BONUS, SHIELDS and ARCS overlays and the issue marks over the design.
//! * [`tools`]: editing on the grid (paint, fill, erase, select), the selection, undo / redo, mirror, bevel.
//! * [`ghost`]: the ghost of the edit under the cursor, the fill rectangle, the selection brackets.
//! * [`inspector`]: the SELECTED PART block (a placed part with the report's numbers, or the palette part).
//! * [`library`], [`share`], [`blueprints`]: saving and the autosave, the BLUEPRINTS and EXPORT screens, share
//!   codes and files, the editable name.
//! * [`behaviors`]: the player's behaviors on the storage layer ([`behaviors::PlayerBehaviors`]), the behavior a
//!   design names, checks with the player's behaviors known.
//!
//! **For handlers** (editing, `sim::report`, saving): read `MessageReader<DrydockAction>` in a system in
//! [`DockSet::Apply`]; change [`PlayerDesign`] through `PlayerDesign::edit` (the revision goes up: the view, the status and the palette follow),
//! write [`DrydockStats`] and [`DrydockStatus`]; read [`DrydockUi`] (tool, material, rotation, picked
//! part) and [`DrydockCursor`] (the cell under the pointer). Nothing here needs to change for that.

pub mod actions;
pub mod behaviors;
pub mod blueprints;
pub mod design;
pub mod ghost;
pub mod inspector;
pub mod library;
pub mod overlays;
pub mod panels;
pub mod report;
pub mod share;
pub mod state;
pub mod stats;
pub mod tools;
pub mod view;

use bevy::prelude::*;

use crate::camera::CameraSet;
use crate::fx::FxFlush;
use crate::mode::AppMode;
use crate::sim_bridge::GameContent;
use crate::ui_kit::{UiInputSet, UiStyleSet};

pub use design::PlayerDesign;

use actions::DrydockAction;
use state::{DrydockStats, DrydockStatus, DrydockUi};
use view::{DockCamera, DrydockCursor};

/// The drydock's `Update` systems, in order, only in the drydock: input (clicks and keys become
/// [`DrydockAction`]s) → apply (handlers, the camera, the report, stats and status) → show (the screen follows the state).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockSet {
    Input,
    Apply,
    Show,
}

pub struct DrydockPlugin;

impl Plugin for DrydockPlugin {
    fn build(&self, app: &mut App) {
        let content = &app.world().resource::<GameContent>().0;
        let design = PlayerDesign::starter(content);
        let ui = DrydockUi::new(content, &design);
        app.insert_resource(design)
            .insert_resource(ui)
            .insert_resource(DockCamera::new(0.0))
            .init_resource::<DrydockStatus>()
            .init_resource::<DrydockStats>()
            .init_resource::<report::DrydockReport>()
            .init_resource::<DrydockCursor>()
            .init_resource::<panels::Palette>()
            .add_message::<DrydockAction>()
            .add_plugins((behaviors::BehaviorsPlugin, blueprints::BlueprintsPlugin))
            .configure_sets(
                Update,
                (DockSet::Input, DockSet::Apply, DockSet::Show)
                    .chain()
                    .after(UiInputSet)
                    .before(UiStyleSet)
                    .run_if(in_state(AppMode::Drydock)),
            )
            .add_systems(Startup, (view::setup, overlays::setup.after(view::setup), panels::spawn.after(crate::flight_hud::spawn_hud)))
            .add_systems(PostStartup, (show, view::remember_start_zoom.run_if(in_state(AppMode::Drydock))))
            .add_systems(OnEnter(AppMode::Drydock), view::enter)
            .add_systems(OnExit(AppMode::Drydock), view::exit)
            .add_systems(Update, show.run_if(state_changed::<AppMode>))
            .add_systems(
                Update,
                (
                    (actions::clicks, actions::shortcuts, panels::sync_search).in_set(DockSet::Input),
                    (view::ui_scale, view::camera, actions::apply, report::update, stats::update).chain().in_set(DockSet::Apply),
                    (view::rebuild_ship, overlays::rebuild, panels::sync_palette, panels::sync_blocks, panels::sync_pills, panels::sync_texts, panels::sync_live)
                        .chain()
                        .in_set(DockSet::Show),
                ),
            )
            .add_systems(PostUpdate, view::draw_frame.run_if(in_state(AppMode::Drydock)).after(CameraSet).before(FxFlush));
        editing(app);
    }
}

/// Editing on the grid, the ghost and the inspector (`tools`, `ghost`, `inspector`).
fn editing(app: &mut App) {
    app.init_resource::<tools::Selection>()
        .init_resource::<tools::DrydockGesture>()
        .init_resource::<tools::EditNote>()
        .init_resource::<ghost::GhostView>()
        .init_resource::<inspector::Inspector>()
        .add_systems(Startup, ghost::setup.after(view::setup))
        .add_systems(
            Update,
            (
                (tools::apply, tools::pointer).chain().after(actions::apply).before(report::update).in_set(DockSet::Apply),
                inspector::update.after(report::update).in_set(DockSet::Apply),
                (ghost::update, ghost::rebuild, ghost::sync_hint).chain().before(panels::sync_live).in_set(DockSet::Show),
                (inspector::sync_controls, tools::sync_rings).in_set(DockSet::Show),
            ),
        )
        .add_systems(PostUpdate, ghost::draw.run_if(in_state(AppMode::Drydock)).after(view::draw_frame).before(FxFlush));
}

/// The drydock's screen and design are shown only in the drydock.
fn show(
    mode: Res<State<AppMode>>,
    mut root: Query<&mut Visibility, (With<panels::DrydockRoot>, Without<view::DockShip>)>,
    mut ship: Query<&mut Visibility, (With<view::DockShip>, Without<panels::DrydockRoot>)>,
) {
    let want = if *mode.get() == AppMode::Drydock { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in root.iter_mut().chain(ship.iter_mut()) {
        if *v != want {
            *v = want;
        }
    }
}
