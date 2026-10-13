//! The app mode: flying ([`AppMode::Flight`]: the sandbox, the showcase, the load test), building
//! ([`AppMode::Drydock`]: the ship editor, `drydock/`) or writing how a design thinks ([`AppMode::Behavior`]: the
//! BEHAVIOR screen, `behavior_ui/`, opened from the drydock). `?scene=drydock` starts in the drydock,
//! `?scene=behavior` on the BEHAVIOR screen; everything else starts in flight.
//!
//! **T** switches everywhere: in flight it opens the drydock (the design is untouched), or the screen a behavior
//! test was started from ([`FlightExit`], `test_range.rs`); in the drydock it launches a test flight
//! (`drydock::actions`, the TEST-FLY button does the same). Losing every vessel in a test flight returns there
//! too (`scene.rs`).
//!
//! Systems of one mode run only in it (`run_if(in_state(..))`), so no input of one mode reaches another. The
//! switch itself happens in the state's enter / exit schedules (`scene.rs`: the flight world; `drydock`: its
//! camera and UI scale; `behavior_ui`: the draft and the window fit), so the first frame of a mode is
//! already consistent. A [`SimWorld`](crate::sim_bridge::SimWorld) always exists: the drydock keeps an empty
//! one, because many flight systems read it.

use bevy::prelude::*;
use bevy_egui::input::egui_wants_any_keyboard_input;

use crate::params::Params;

#[derive(States, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppMode {
    Flight,
    Drydock,
    Behavior,
}

impl AppMode {
    /// The mode the app starts in: the drydock for `?scene=drydock`, the BEHAVIOR screen for `?scene=behavior`,
    /// else flight.
    pub fn at_start(params: &Params) -> Self {
        match params.text("scene") {
            Some("drydock") => AppMode::Drydock,
            Some("behavior") => AppMode::Behavior,
            _ => AppMode::Flight,
        }
    }
}

/// Marks the root of a flight-only UI block (HUD, fleet bar, touch widgets). Hidden when another mode opens;
/// its own flight system shows it again (they set their visibility every frame while they run).
#[derive(Component)]
pub struct FlightUi;

/// Where T (and a lost test flight) leaves flight to: the drydock, or the screen a behavior test was started from
/// (`test_range::StartTest::back`). Back to the drydock whenever flight ends.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlightExit(pub AppMode);

impl Default for FlightExit {
    fn default() -> Self {
        Self(AppMode::Drydock)
    }
}

pub struct ModePlugin;

impl Plugin for ModePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlightExit>()
            .add_systems(
                Update,
                open_drydock.run_if(in_state(AppMode::Flight)).run_if(not(egui_wants_any_keyboard_input)),
            )
            .add_systems(OnExit(AppMode::Flight), (|mut exit: ResMut<FlightExit>| *exit = FlightExit::default(), hide_flight_ui))
            // The initial OnEnter runs before Startup spawns the flight UI.
            .add_systems(PostStartup, hide_flight_ui.run_if(not(in_state(AppMode::Flight))));
    }
}

/// T in flight: back to the drydock, or to where a behavior test started (the scene is torn down in
/// `OnExit(Flight)`).
fn open_drydock(keys: Res<ButtonInput<KeyCode>>, exit: Res<FlightExit>, mut next: ResMut<NextState<AppMode>>) {
    if keys.just_pressed(KeyCode::KeyT) {
        next.set(exit.0);
    }
}

fn hide_flight_ui(mut roots: Query<&mut Visibility, With<FlightUi>>) {
    for mut v in &mut roots {
        *v = Visibility::Hidden;
    }
}
