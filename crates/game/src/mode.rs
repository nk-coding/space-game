//! The app mode: at home ([`AppMode::Home`]: the shipyard's ring menu around the docked ship, `home.rs`, where the
//! game starts), flying ([`AppMode::Flight`]: the sandbox, which home calls the TEST RANGE, the showcase, the load
//! test), building ([`AppMode::Drydock`]: the ship editor, `drydock/`) or writing how a design thinks
//! ([`AppMode::Behavior`]: the BEHAVIOR screen, `behavior_ui/`). `?scene=` picks the start ([`AppMode::at_start`]).
//!
//! **T** switches between flight and the editors: in flight it opens the drydock (the design is untouched), or the
//! screen a behavior test was started from ([`FlightExit`], `test_range.rs`); in the drydock it launches a test
//! flight (`drydock::actions`, the TEST-FLY button does the same). Losing every vessel in a test flight returns
//! there too (`scene.rs`). **M** in flight goes home (so does the HUD's ring top left, `flight_hud.rs`); the ring
//! button top left of the drydock and the BEHAVIOR screen does the same there.
//!
//! Systems of one mode run only in it (`run_if(in_state(..))`), so no input of one mode reaches another. The
//! switch itself happens in the state's enter / exit schedules (`scene.rs`: the flight world; `drydock` and
//! `home`: their camera and UI scale; `behavior_ui`: the draft and the window fit), so the first frame of a mode
//! is already consistent. A [`SimWorld`](crate::sim_bridge::SimWorld) always exists: outside flight it is an empty
//! one, because many flight systems read it.

use bevy::prelude::*;
use bevy_egui::input::egui_wants_any_keyboard_input;

use crate::params::Params;

#[derive(States, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppMode {
    Home,
    Flight,
    Drydock,
    Behavior,
}

impl AppMode {
    /// The mode the app starts in: home (`?scene=home`, and the default), the drydock (`?scene=drydock`), the
    /// BEHAVIOR screen (`?scene=behavior`), or flight for the flight scenes (`?scene=sandbox|showcase|watch|
    /// alongside`) and for parameters that only mean something in flight ([`Params::wants_flight`]: `?load=`,
    /// `?fleet=`, `?hashlog=` ...). An unknown scene starts at home.
    pub fn at_start(params: &Params) -> Self {
        match params.text("scene") {
            Some("home") => AppMode::Home,
            Some("drydock") => AppMode::Drydock,
            Some("behavior") => AppMode::Behavior,
            Some("sandbox" | "showcase" | "watch" | "alongside") => AppMode::Flight,
            None if params.wants_flight() => AppMode::Flight,
            None => AppMode::Home,
            Some(other) => {
                warn!("unknown scene `{other}`: starting at home");
                AppMode::Home
            }
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
                leave_by_key.run_if(in_state(AppMode::Flight)).run_if(not(egui_wants_any_keyboard_input)),
            )
            .add_systems(OnExit(AppMode::Flight), (|mut exit: ResMut<FlightExit>| *exit = FlightExit::default(), hide_flight_ui))
            // The initial OnEnter runs before Startup spawns the flight UI.
            .add_systems(PostStartup, hide_flight_ui.run_if(not(in_state(AppMode::Flight))));
    }
}

/// T in flight: back to the drydock, or to where a behavior test started; M: home (the scene is torn down in
/// `OnExit(Flight)`).
fn leave_by_key(keys: Res<ButtonInput<KeyCode>>, exit: Res<FlightExit>, mut next: ResMut<NextState<AppMode>>) {
    if keys.just_pressed(KeyCode::KeyT) {
        next.set(exit.0);
    } else if keys.just_pressed(KeyCode::KeyM) {
        next.set(AppMode::Home);
    }
}

fn hide_flight_ui(mut roots: Query<&mut Visibility, With<FlightUi>>) {
    for mut v in &mut roots {
        *v = Visibility::Hidden;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(q: &str) -> AppMode {
        AppMode::at_start(&Params::parse(q))
    }

    /// Home is the start; every scene of before keeps its mode; flight-only parameters still fly.
    #[test]
    fn the_game_starts_at_home_unless_a_scene_or_a_flight_parameter_says_otherwise() {
        assert_eq!(start(""), AppMode::Home);
        assert_eq!(start("?scene=home&zoom=60"), AppMode::Home);
        assert_eq!(start("?debug=1&hud=0&stats=1"), AppMode::Home);
        assert_eq!(start("?scene=nowhere"), AppMode::Home);
        for q in ["?scene=sandbox", "?scene=showcase&fire=1", "?scene=watch&behavior=raider_basic", "?scene=alongside"] {
            assert_eq!(start(q), AppMode::Flight, "{q}");
        }
        for q in ["?load=20", "?hashlog=1&hashticks=600", "?fleet=3&mind=4", "?zoom=70", "?seed=7", "?tuning=1"] {
            assert_eq!(start(q), AppMode::Flight, "{q}");
        }
        assert_eq!(start("?scene=drydock&zoom=40"), AppMode::Drydock);
        assert_eq!(start("?scene=behavior&behavior=raider_basic"), AppMode::Behavior);
    }
}
