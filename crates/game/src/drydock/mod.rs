//! The drydock: the ship editor of the client (Drydock v1, decided by Niklas on 12 Oct 2026; the look follows
//! `docs/concept/boards/ui-mockups/desktop-drydock.png`): the mode, the view of the player's design, the screen
//! with its widgets, editing, the live stats, saving and sharing, and the test flight, in the modules below.
//!
//! * [`design`]: [`PlayerDesign`], the blueprint every flight of the player flies.
//! * [`state`]: what the widgets show (`DrydockUi`, `DrydockStatus`, `DrydockStats`).
//! * [`actions`]: `DrydockAction`, the one message buttons and keys send; the UI-state handler.
//! * [`view`]: the design drawn with the flight tiles, the frame overlay, the camera, `DrydockCursor`.
//! * [`panels`]: the screen (built from `ui_kit` widgets and `ui_icons`); locked parts dimmed, NEW tags.
//! * [`report`]: [`report::DrydockReport`], the design's `sim::report` per revision.
//! * [`stats`]: the stats grid and the status line (whether the design can fly) from the report; the
//!   voyage-ready line from the career.
//!
//! What the player may build with is the career's (`career::Granted`): the palette, the ARMOR pills and the frames
//! offer the unlocked parts, materials and frames and show the rest locked (everything in free mode,
//! `?unlock=all`). A design that uses locked parts still opens, shows and test-flies; it is only not voyage-ready.
//! * [`overlays`]: the HEAT, MASS, BONUS, SHIELDS and ARCS overlays and the issue marks over the design.
//! * [`tools`]: editing on the grid (paint, fill, erase, select), the selection, undo / redo, mirror, bevel.
//! * [`ghost`]: the ghost of the edit under the cursor, the fill rectangle, the selection brackets.
//! * [`inspector`]: the SELECTED PART block (a placed part with the report's numbers, or the palette part).
//! * [`library`], [`share`], [`blueprints`]: saving and the autosave, the BLUEPRINTS and EXPORT screens, share
//!   codes and files, the editable name.
//! * [`behaviors`]: the player's behaviors on the storage layer ([`behaviors::PlayerBehaviors`]), the behavior a
//!   design names, checks with the player's behaviors known.
//!
//! **For handlers** (the ring menu, later ones): read `MessageReader<DrydockAction>` in a system in
//! [`DockSet::Apply`]; change [`PlayerDesign`] through `PlayerDesign::edit` (the revision goes up: the view, the
//! report, the status and the palette follow); read [`DrydockUi`] (tool, material, rotation, picked part) and
//! [`DrydockCursor`] (the cell under the pointer).

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
use crate::career::{Granted, PlayerCareer};
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
        // The career (CareerPlugin comes first) names the ship a new game docks and what the palette offers.
        let content = &app.world().resource::<GameContent>().0;
        let design = PlayerDesign::starter(content, app.world().resource::<PlayerCareer>().start_ship(content));
        app.insert_resource(design)
            .insert_resource(DockCamera::new(0.0))
            .init_resource::<DrydockStatus>()
            .init_resource::<DrydockStats>()
            .init_resource::<report::DrydockReport>()
            .init_resource::<DrydockCursor>()
            .init_resource::<panels::Palette>()
            .add_message::<DrydockAction>()
            // `BlueprintsPlugin` puts the last session's working design in (in its build, at once).
            .add_plugins((behaviors::BehaviorsPlugin, blueprints::BlueprintsPlugin));
        // The UI state starts from the design the drydock opens with (its armour), so after the restore.
        let world = app.world();
        let ui = DrydockUi::new(&world.resource::<GameContent>().0, world.resource::<PlayerDesign>(), &world.resource::<Granted>().unlocks);
        app.insert_resource(ui)
            .configure_sets(
                Update,
                (DockSet::Input, DockSet::Apply, DockSet::Show)
                    .chain()
                    .after(UiInputSet)
                    .before(UiStyleSet)
                    .run_if(in_state(AppMode::Drydock)),
            )
            .add_systems(Startup, (view::setup, overlays::setup.after(view::setup), panels::spawn.after(crate::flight_hud::spawn_hud)))
            .add_systems(PostStartup, view::remember_start_zoom.run_if(in_state(AppMode::Drydock)))
            .add_systems(OnEnter(AppMode::Drydock), view::enter)
            .add_systems(OnExit(AppMode::Drydock), (view::exit, tools::leave))
            .add_systems(OnExit(AppMode::Behavior), view::unpark)
            // The screen's transition (`ui_kit::ScreenFx`) and its world elements' fade, in every mode (it closes
            // after the drydock is left).
            .add_systems(Update, (view::open_close, view::fade_world).chain())
            .add_systems(
                Update,
                (
                    (actions::clicks, actions::shortcuts, panels::sync_search).in_set(DockSet::Input),
                    (view::ui_scale, view::camera, actions::apply, report::update, stats::update, stats::voyage).chain().in_set(DockSet::Apply),
                    panels::see_fresh.after(inspector::update).in_set(DockSet::Apply),
                    (view::rebuild_ship, overlays::rebuild, panels::sync_palette, panels::scroll_selected, panels::sync_blocks, panels::sync_pills, panels::sync_texts, panels::sync_live)
                        .chain()
                        .in_set(DockSet::Show),
                ),
            )
            .add_systems(PostUpdate, view::draw_frame.run_if(view::shown).after(CameraSet).before(FxFlush))
            // After the camera: ARCS is cut at the edge of the view of this frame.
            .add_systems(PostUpdate, overlays::rebuild.run_if(view::shown).after(CameraSet));
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
        .add_systems(PostUpdate, ghost::draw.run_if(view::shown).after(view::draw_frame).before(FxFlush));
}
