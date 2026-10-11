//! Space game client (Bevy). Web first: `scripts/web-serve.sh` / `scripts/web-build.sh`.
//!
//! Module map:
//! - `atlas`, `style`: the baked texture atlas + style tokens (from `crates/atlas-tool`).
//! - `ship_model`: blueprint + content (`data` crate) → render-ready `ShipVisual`.
//! - `ship_view`: `ShipVisual` → entity hierarchy (body mesh, aura, plumes, shields) + per-frame updates.
//! - `sim_bridge`: `SimPose` / `EngineThrottle` written at 30 Hz, interpolated into transforms each frame.
//! - `trails`, `backdrop`, `camera`, `hud`: effects, space backdrop, zoom/pan, debug overlay + toggles.
//! - `showcase`: the current demo scene (scripted motion standing in for the sim).

mod atlas;
mod backdrop;
mod camera;
mod hud;
mod meshbuf;
mod params;
mod ship_model;
mod ship_view;
mod showcase;
mod sim_bridge;
mod style;
mod trails;

use bevy::camera::{CompositingSpace, Hdr};
use bevy::prelude::*;
use bevy::window::WindowResolution;

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
    pub shields: bool,
    pub hud: bool,
    /// The ship the range rings (and `F` follow) belong to.
    pub ring_target: Option<Entity>,
}

impl RenderToggles {
    fn from_params(p: &Params) -> Self {
        Self {
            debug_shapes: p.flag("debug").unwrap_or(false),
            bloom: p.flag("bloom").unwrap_or(false),
            glow_sprites: p.flag("glow").unwrap_or(true),
            shields: p.flag("shields").unwrap_or(true),
            hud: p.flag("hud").unwrap_or(true),
            ring_target: None,
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
    app.insert_resource(ClearColor(Color::srgb_u8(0x01, 0x04, 0x09)))
        .insert_resource(RenderToggles::from_params(&params))
        .add_plugins((
            atlas::AtlasPlugin,
            sim_bridge::SimBridgePlugin,
            camera::CameraPlugin,
            ship_view::ShipViewPlugin,
            trails::TrailsPlugin,
            backdrop::BackdropPlugin,
            hud::HudPlugin,
            showcase::ShowcasePlugin,
        ))
        .insert_resource(params)
        .add_systems(Startup, spawn_camera);
    app.run();
}

pub fn spawn_camera(mut commands: Commands, params: Res<Params>, mut rig: ResMut<camera::CameraRig>) {
    if let Some(z) = params.f32("zoom") {
        rig.px_per_m = z.clamp(camera::MIN_PX_PER_M, camera::MAX_PX_PER_M);
    }
    rig.center = Vec2::new(params.f32("x").unwrap_or(0.0), params.f32("y").unwrap_or(0.0));
    commands.spawn((
        camera::camera_bundle(),
        // Blend in sRGB space like the browser and the concept art (linear blending makes every faint glow
        // about twice as bright as the reference).
        CompositingSpace::Srgb,
        // Bevy 0.20 gives an 8-bit sRGB-composited main texture an `Rgba8UnormSrgb` view format, which
        // WebGL2 can't create (DownlevelFlags::VIEW_FORMATS). An HDR (Rgba16Float) main texture has no view
        // formats, so sRGB compositing works on WebGL2 with HDR on. Bloom then only adds the `Bloom` component.
        Hdr,
    ));
}
