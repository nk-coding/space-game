//! Space game client (Bevy): the screens and the app (ADR-0038). Web first (the development platform):
//! `scripts/web-serve.sh`. See `docs/client.md` for controls, URL parameters and the structure. The running game
//! (the sim bridge, scenes, flight, voyages, what the player owns) is the `world` crate, the toolkit (atlas, style,
//! widgets, icons) the `kit` crate; this crate holds the screens, which build on both.
//!
//! Module map:
//! - `home`: the start screen over the shipyard (places, labels, the camera's flights to the drydock, behavior, test
//!   range and fleet); `shipyard`: the station itself, a world scene built around the drydock frame; `fleet`: the
//!   FLEET layer over home (escorts picked from saved designs, against the mind capacity); `research`: the research
//!   screen over the lab (the tree of research nodes, UNLOCK); `voyage_log`: the LOG layer over home (the last
//!   voyages, newest first); `evolution`: the EVOLUTION layer over home (the evolution lab's runs, their setup and
//!   progress, re-watching their fights, saving results); `settings`: the SETTINGS layer over home and flight.
//! - `map`: the voyage map (a star map of the voyages, their states from the career, SET COURSE).
//! - `debrief`: the panel over home after a voyage.
//! - `drydock`: the ship editor (the view of the player's design, the screen, editing, saving and sharing, the
//!   test flight; the design itself is `world::player`).
//! - `behavior_ui`: the BEHAVIOR screen (how the design thinks: graph and text views of its rules), opened from
//!   the drydock.

mod behavior_ui;
mod debrief;
mod drydock;
mod evolution;
mod fleet;
mod home;
mod map;
mod research;
mod settings;
mod shipyard;
mod voyage_log;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;
use kit::params::{Params, RenderToggles};

fn main() {
    // The web's storage is IndexedDB, which only answers asynchronously: every document is read into memory before
    // the app is built, so the game's storage calls never wait (ADR-0041).
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async {
        world::storage::preload().await;
        run();
    });
    #[cfg(not(target_arch = "wasm32"))]
    run();
}

fn run() {
    let params = Params::load();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Space Game".into(),
                    // Web: render into the page's canvas and track its size (CSS makes it fill the window).
                    canvas: Some("#game".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    resolution: WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            })
            .set(bevy::log::LogPlugin { filter: "wgpu=error,naga=warn,bevy_render=warn".into(), ..default() }),
    );
    app.add_plugins(EguiPlugin::default());
    app.insert_state(world::mode::AppMode::at_start(&params));
    app.insert_resource(RenderToggles::from_params(&params))
        .insert_resource(params)
        .add_plugins((
            kit::atlas::AtlasPlugin,
            world::tuning::TuningPlugin,
            world::mode::ModePlugin,
            (world::scene::ScenePlugin, world::replay::ReplayPlugin, world::test_range::TestRangePlugin, world::voyage::VoyagePlugin),
            world::sim_bridge::SimBridgePlugin,
            (world::storage::StoragePlugin, kit::clipboard::ClipboardPlugin, world::career::CareerPlugin, world::settings::SettingsPlugin, world::evolution::EvolutionPlugin),
            // After the bridge (it reads the content the bridge loads), the storage and the career.
            (
                drydock::DrydockPlugin,
                shipyard::ShipyardPlugin,
                behavior_ui::BehaviorPlugin,
                home::HomePlugin,
                fleet::FleetPlugin,
                voyage_log::VoyageLogPlugin,
                evolution::EvolutionPlugin,
                settings::SettingsPlugin,
                map::MapPlugin,
                debrief::DebriefPlugin,
                research::ResearchPlugin,
                kit::ui_kit::UiKitPlugin,
                kit::ui_icons::UiIconsPlugin::<world::sim_bridge::GameContent>::default(),
            ),
            world::camera::CameraPlugin,
            world::controls::ControlsPlugin,
            world::fx::FxPlugin,
            world::shield::ShieldPlugin,
            (world::backdrop::BackdropPlugin, world::scenery::SceneryPlugin),
            (world::flight_hud::FlightHudPlugin, world::warp_fx::WarpFxPlugin),
            world::fleet_bar::FleetBarPlugin,
            world::hud::HudPlugin,
        ))
        .add_systems(Startup, world::camera::spawn_camera);
    app.run();
}
