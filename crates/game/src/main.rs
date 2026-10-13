//! Space game client (Bevy). Web first (the development platform): `scripts/web-serve.sh`. See
//! `docs/client.md` for controls, URL parameters and the structure.
//!
//! Module map:
//! - `mode`: the app mode (home, flight, drydock or the BEHAVIOR screen) and what runs in which.
//! - `home`: the start screen, the shipyard's ring menu around the docked ship (drydock, behavior, test range).
//! - `atlas`, `style`, `meshbuf`: the texture atlas rasterised at runtime + style tokens, mesh building.
//! - `sim_bridge`: owns the `sim::Sim`, steps it at 30 Hz, turns events into view changes and effects.
//! - `scene`: sandbox (player + raider waves; home's TEST RANGE), test flight, showcase, load test; writes the
//!   player's pilot input.
//! - `test_range`: TEST IN RANGE for behaviors (watch, fly alongside), its request and the live behavior view.
//! - `drydock`: the ship editor (the player's design, its view, the screen, the test flight).
//! - `behavior_ui`: the BEHAVIOR screen (how the design thinks: graph and text views of its rules), opened from
//!   the drydock.
//! - `ui_kit`, `ui_icons`: bevy_ui widgets with hand-made hit testing; vector icons in UI nodes.
//! - `storage`, `clipboard`: the player's documents (designs and behaviors now; saves, settings later) and copy /
//!   paste of UI text, per platform.
//! - `ship_model` / `ship_view`: content + sim piece → style-2a meshes, posed from `render_pose` every frame.
//! - `fx`: immediate-mode sprites (plumes, trails, beams, tracers, sparks, flashes).
//! - `shield`: shield bubbles (one batched mesh, procedural shader `shield.wesl`).
//! - `controls`, `camera`: keyboard / mouse / touch input, follow / frame / free camera.
//! - `flight_hud`, `fleet_bar`, `hud`, `tuning`: flight HUD, fleet bar (the player's vessels), debug overlay and
//!   toggles, live tuning panel (egui).
//! - `backdrop`: gradient, parallax dust, range rings.

mod atlas;
mod backdrop;
mod behavior_ui;
mod camera;
mod clipboard;
mod controls;
mod drydock;
mod fleet_bar;
mod flight_hud;
mod fx;
mod home;
mod hud;
mod meshbuf;
mod mode;
mod params;
mod scene;
mod shield;
mod ship_model;
mod ship_view;
mod sim_bridge;
mod storage;
mod style;
mod test_range;
mod tuning;
mod ui_icons;
mod ui_kit;

use bevy::camera::{CompositingSpace, Hdr};
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;

use crate::params::Params;

/// Render switches. The spike measures style 2a vs. debug shapes and pre-blurred glow sprites vs. bloom.
#[derive(Resource, Debug, Clone)]
pub struct RenderToggles {
    /// Plain coloured cells only: no seams, icons, glow, plumes, trails or backdrop.
    pub debug_shapes: bool,
    /// Full-screen bloom (HDR camera).
    pub bloom: bool,
    /// Pre-blurred glow layers (icon glows, hull halo, aura, soft muzzle glows).
    pub glow_sprites: bool,
    pub hud: bool,
    /// Fps / timings overlay.
    pub debug_info: bool,
}

impl RenderToggles {
    fn from_params(p: &Params) -> Self {
        Self {
            debug_shapes: p.flag("debug").unwrap_or(false),
            bloom: p.flag("bloom").unwrap_or(false),
            glow_sprites: p.flag("glow").unwrap_or(true),
            hud: p.flag("hud").unwrap_or(true),
            debug_info: p.flag("stats").unwrap_or(false),
        }
    }
}

fn main() {
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
    app.insert_state(mode::AppMode::at_start(&params));
    app.insert_resource(ClearColor(Color::srgb_u8(0x01, 0x04, 0x09)))
        .insert_resource(RenderToggles::from_params(&params))
        .insert_resource(params)
        .add_plugins((
            atlas::AtlasPlugin,
            tuning::TuningPlugin,
            mode::ModePlugin,
            (scene::ScenePlugin, test_range::TestRangePlugin),
            sim_bridge::SimBridgePlugin,
            (storage::StoragePlugin, clipboard::ClipboardPlugin),
            // After the bridge (it reads the content the bridge loads) and the storage.
            (drydock::DrydockPlugin, behavior_ui::BehaviorPlugin, home::HomePlugin, ui_kit::UiKitPlugin, ui_icons::UiIconsPlugin),
            camera::CameraPlugin,
            controls::ControlsPlugin,
            fx::FxPlugin,
            shield::ShieldPlugin,
            backdrop::BackdropPlugin,
            flight_hud::FlightHudPlugin,
            fleet_bar::FleetBarPlugin,
            hud::HudPlugin,
        ))
        .add_systems(Startup, spawn_camera);
    app.run();
}

pub fn spawn_camera(mut commands: Commands, params: Res<Params>, mut rig: ResMut<camera::CameraRig>) {
    if let Some(z) = params.f32("zoom") {
        rig.zoom = z.clamp(camera::MIN_PX_PER_M, camera::MAX_PX_PER_M);
        rig.zoom_target = rig.zoom;
        rig.user_zoom = true;
    }
    commands.spawn((
        camera::camera_bundle(rig.zoom),
        // Blend in sRGB space like the browser and the concept art (linear blending makes every faint glow
        // about twice as bright as the reference).
        CompositingSpace::Srgb,
        // Bevy 0.20 gives an 8-bit sRGB-composited main texture an `Rgba8UnormSrgb` view format, which
        // WebGL2 can't create (DownlevelFlags::VIEW_FORMATS). An HDR (Rgba16Float) main texture has no view
        // formats, so sRGB compositing works on WebGL2 with HDR on. Bloom then only adds the `Bloom` component.
        Hdr,
    ));
}
