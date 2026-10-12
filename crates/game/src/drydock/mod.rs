//! The drydock: the ship editor of the client (Drydock v1, decided by Niklas on 12 Oct 2026; the look follows
//! `docs/concept/boards/ui-mockups/desktop-drydock.png`). This is the foundation: the mode, the static view of
//! the player's design, the layout with its widgets, and the test flight. Editing, the live stats and saving
//! plug in through the resources and the action message below.
//!
//! * [`design`]: [`PlayerDesign`], the blueprint every flight of the player flies, and the flight check.
//! * [`state`]: what the widgets show (`DrydockUi`, `DrydockStatus`, `DrydockStats`).
//! * [`actions`]: `DrydockAction`, the one message buttons and keys send; the UI-state handler.
//! * [`view`]: the design drawn with the flight tiles, the frame overlay, the camera, `DrydockCursor`.
//! * [`panels`]: the screen (built from `ui_kit` widgets and `ui_icons`).
//!
//! **For handlers** (editing, `sim::report`, saving): read `MessageReader<DrydockAction>` in a system in
//! [`DockSet::Apply`]; change [`PlayerDesign`] (bump `revision`: the view, the status and the palette follow),
//! write [`DrydockStats`] and [`DrydockStatus`]; read [`DrydockUi`] (tool, material, mirror, rotation, picked
//! part) and [`DrydockCursor`] (the cell under the pointer). Nothing here needs to change for that.

pub mod actions;
pub mod design;
pub mod panels;
pub mod state;
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
/// [`DrydockAction`]s) → apply (handlers, the camera, the design check) → show (the screen follows the state).
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
            .init_resource::<DrydockCursor>()
            .init_resource::<panels::Palette>()
            .add_message::<DrydockAction>()
            .configure_sets(
                Update,
                (DockSet::Input, DockSet::Apply, DockSet::Show)
                    .chain()
                    .after(UiInputSet)
                    .before(UiStyleSet)
                    .run_if(in_state(AppMode::Drydock)),
            )
            .add_systems(Startup, (view::setup, panels::spawn.after(crate::flight_hud::spawn_hud)))
            .add_systems(PostStartup, (show, view::remember_start_zoom.run_if(in_state(AppMode::Drydock))))
            .add_systems(OnEnter(AppMode::Drydock), view::enter)
            .add_systems(OnExit(AppMode::Drydock), view::exit)
            .add_systems(Update, show.run_if(state_changed::<AppMode>))
            .add_systems(
                Update,
                (
                    (actions::clicks, actions::shortcuts, panels::sync_search).in_set(DockSet::Input),
                    (view::ui_scale, view::camera, actions::apply, design::check_design).chain().in_set(DockSet::Apply),
                    (view::rebuild_ship, panels::sync_palette, panels::sync_blocks, panels::sync_pills, panels::sync_texts, panels::sync_live)
                        .chain()
                        .in_set(DockSet::Show),
                ),
            )
            .add_systems(PostUpdate, view::draw_frame.run_if(in_state(AppMode::Drydock)).after(CameraSet).before(FxFlush));
    }
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
