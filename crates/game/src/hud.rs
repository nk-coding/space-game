//! Debug overlay (`F3`; always on in the load test): fps, frame and sim step times (mean / median / p95 over
//! a rolling window), entity and sim counts, the atlas tier against what the screen needs (window size and
//! scale factor), render toggles, the focus ship's heading, turn rate and speed (for tuning turning, GDR-0055), the
//! locked ship's active behavior rule and, in TEST IN RANGE, every rule of the tested behavior with why
//! (`test_range::BehaviorWatch`). Keys: `F3` the overlay (everywhere); in flight only: `1` debug shapes, `2` glow
//! sprites, `3` bloom, `H` hide the HUD, `P` pause, `Shift+R` reset the scene (`scene.rs`).
//!
//! In the load test the overlay also logs a one-line summary to the browser console after 60 s
//! (`LOADTEST ...`). `?slowmo=N` runs the game's clock N times slower (1 to 50; for looking at animations such as
//! the voyage's warp in headless screenshots; the sim steps the same ticks, only later).

use std::fmt::Write as _;

use bevy::diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::egui_wants_any_keyboard_input;

use crate::params::RenderToggles;
use crate::atlas::{Atlas, AtlasStatus};
use crate::camera::{CameraRig, MainCamera};
use crate::controls::PlayerInput;
use crate::mode::AppMode;
use crate::scene::{Scene, SceneKind};
use crate::sim_bridge::PilotFocus;
use crate::sim_bridge::{SimWorld, Timings, console_out};
use crate::style::{Style, ui_color};
use crate::test_range::BehaviorWatch;

#[derive(Component)]
struct DebugText;

#[derive(Resource)]
struct DebugState {
    timer: Timer,
    frame_ms: Timings,
    started: Option<f64>,
    summary_done: bool,
    scratch: String,
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((FrameTimeDiagnosticsPlugin::default(), EntityCountDiagnosticsPlugin::default()))
            .insert_resource(DebugState {
                timer: Timer::from_seconds(0.25, TimerMode::Repeating),
                frame_ms: Timings::new(600),
                started: None,
                summary_done: false,
                scratch: String::with_capacity(512),
            })
            .add_systems(Startup, (spawn_debug, slow_motion))
            .add_systems(
                Update,
                (
                    keyboard_toggles.run_if(in_state(AppMode::Flight)).run_if(not(egui_wants_any_keyboard_input)),
                    debug_toggle.run_if(not(egui_wants_any_keyboard_input)),
                    apply_bloom,
                    update_debug,
                ),
            );
    }
}

/// `?slowmo=N`: the virtual clock (the sim's steps, flight's effects) runs N times slower.
fn slow_motion(params: Res<crate::params::Params>, mut time: ResMut<Time<Virtual>>) {
    if let Some(n) = params.f32("slowmo").filter(|n| *n > 1.0) {
        time.set_relative_speed(1.0 / n.min(50.0));
    }
}

fn spawn_debug(mut commands: Commands, style: Res<Style>) {
    commands.spawn((
        DebugText,
        Text::new(""),
        TextFont { font_size: FontSize::Px(12.0), ..default() },
        TextColor(ui_color(style.ui_label, 1.0)),
        Node { position_type: PositionType::Absolute, bottom: px(64), left: px(14), ..default() },
    ));
}

fn keyboard_toggles(keys: Res<ButtonInput<KeyCode>>, mut toggles: ResMut<RenderToggles>, mut time: ResMut<Time<Virtual>>) {
    if keys.just_pressed(KeyCode::Digit1) {
        toggles.debug_shapes = !toggles.debug_shapes;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        toggles.glow_sprites = !toggles.glow_sprites;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        toggles.bloom = !toggles.bloom;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        toggles.hud = !toggles.hud;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
}

/// F3: the debug overlay, in every mode.
fn debug_toggle(keys: Res<ButtonInput<KeyCode>>, mut toggles: ResMut<RenderToggles>) {
    if keys.just_pressed(KeyCode::F3) {
        toggles.debug_info = !toggles.debug_info;
    }
}

fn apply_bloom(
mut commands: Commands, toggles: Res<RenderToggles>, camera: Single<(Entity, Has<Bloom>), With<MainCamera>>) {
    let (entity, has) = *camera;
    if toggles.bloom && !has {
        commands.entity(entity).insert(Bloom { intensity: 0.3, ..Bloom::NATURAL });
    } else if !toggles.bloom && has {
        // The camera keeps `Hdr` (needed for sRGB compositing on WebGL2, see `spawn_camera`).
        commands.entity(entity).remove::<Bloom>();
    }
}

#[allow(clippy::too_many_arguments)]
fn update_debug(
    time: Res<Time<Real>>,
    vtime: Res<Time<Virtual>>,
    mut state: ResMut<DebugState>,
    diagnostics: Res<DiagnosticsStore>,
    toggles: Res<RenderToggles>,
    rig: Res<CameraRig>,
    scene: Res<Scene>,
    mode: Res<State<AppMode>>,
    input: Res<PlayerInput>,
    focus: Res<PilotFocus>,
    watch: Res<BehaviorWatch>,
    world: Res<SimWorld>,
    atlas: Res<Atlas>,
    atlas_status: Res<AtlasStatus>,
    window: Single<&Window, With<PrimaryWindow>>,
    text: Single<(&mut Text, &mut Visibility), With<DebugText>>,
) {
    let state = &mut *state;
    let now = time.elapsed_secs_f64();
    state.frame_ms.push(time.delta_secs() * 1000.0);
    // The load test shows the overlay while it runs (not in the drydock).
    let load = matches!(scene.kind, SceneKind::Load(_)) && *mode.get() == AppMode::Flight;
    let started = *state.started.get_or_insert(now);

    // Load test summary after 60 s of wall time.
    if load && !state.summary_done && now - started > 60.0 {
        state.summary_done = true;
        let (fm, fmed, fp95, fmax) = state.frame_ms.stats();
        let (sm, smed, sp95, smax) = world.step_ms.stats();
        let n = if let SceneKind::Load(n) = scene.kind { n } else { 0 };
        console_out(&format!(
            "LOADTEST ships={n} seconds={:.0} ticks={} frame_ms mean={fm:.2} median={fmed:.2} p95={fp95:.2} max={fmax:.2} sim_ms mean={sm:.3} median={smed:.3} p95={sp95:.3} max={smax:.3} style={} glow={} bloom={} window={}x{}",
            now - started,
            world.sim.tick(),
            if toggles.debug_shapes { "debug" } else { "2a" },
            toggles.glow_sprites,
            toggles.bloom,
            rig.viewport.x,
            rig.viewport.y,
        ));
    }

    let (mut text, mut vis) = text.into_inner();
    let show = (toggles.debug_info || load) && toggles.hud;
    let want = if show { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
    if !show || !state.timer.tick(time.delta()).just_finished() {
        return;
    }
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let ents = diagnostics.get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT).and_then(|d| d.value()).unwrap_or(0.0);
    let (fm, fmed, fp95, _) = state.frame_ms.stats();
    let (sm, smed, sp95, _) = world.step_ms.stats();
    let pieces: usize = world.sim.ships().iter().map(|s| s.pieces().len()).sum();
    let on = |b: bool| if b { "on" } else { "off" };
    let s = &mut state.scratch;
    s.clear();
    let _ = write!(
        s,
        "FPS {fps:5.1}   frame ms mean {fm:5.2}  med {fmed:5.2}  p95 {fp95:5.2}   sim ms mean {sm:.3}  med {smed:.3}  p95 {sp95:.3}\n\
         tick {}  ships {}  pieces {}  shots {}  entities {:.0}  zoom {:.1} px/m{}\n\
         [1] debug shapes {}  [2] glow sprites {}  [3] bloom {}  [P] pause  [Shift+R] reset  [F3] stats",
        world.sim.tick(),
        world.sim.ships().len(),
        pieces,
        world.sim.projectiles().len(),
        ents,
        rig.zoom,
        if vtime.is_paused() { "  PAUSED" } else { "" },
        on(toggles.debug_shapes),
        on(toggles.glow_sprites),
        on(toggles.bloom),
    );
    // The atlas against what the screen needs (blurry art at high zoom: is the tier there?).
    let a = &*atlas_status;
    let _ = write!(
        s,
        "\natlas {} px/cell  need {:.0} (zoom x scale {:.2})  max {}  window {}x{} px",
        atlas.tier,
        a.need,
        window.scale_factor(),
        a.max_tier,
        window.physical_width(),
        window.physical_height(),
    );
    if let Some(t) = a.building {
        let _ = write!(s, "  building {t}");
    }
    if let Some((t, ms, frames, longest)) = a.last {
        let _ = write!(s, "  last {t}: {ms:.0} ms / {frames} frames, longest step {longest:.0} ms");
    }
    // The focus: heading, turn rate and speed, and whether the last resort (differential thrust) helps turning
    // (for tuning turning in the browser, GDR-0055).
    if let Some(ship) = focus.0.and_then(|id| world.sim.ship(id))
        && let Some(p) = ship.control_piece().and_then(|c| ship.piece(c))
    {
        let _ = write!(
            s,
            "\nfocus: heading {:+.1} deg  turn {:+.2} rad/s  speed {:.1} m/s  last resort {}",
            sim::flight::physics::wrap_angle(p.body.angle).to_degrees(),
            p.body.angvel,
            p.body.linvel.length(),
            on(ship.telemetry().assist),
        );
    }
    // The locked ship's behavior: which rule decides (for tuning behaviors in the browser).
    if let Some(id) = input.locked_ship()
        && let Some(ship) = world.sim.ship(id)
        && let Some(b) = ship.behavior()
    {
        match b.active_rule() {
            Some(i) => {
                let _ = write!(s, "\ntarget behavior: rule {}  ({})", i + 1, b.describe(i));
            }
            None => s.push_str("\ntarget behavior: no rule decides (brake)"),
        }
    }
    // TEST IN RANGE: the tested ship's behavior, every rule and why (`>` marks the one that decides).
    if watch.ship.is_some() {
        let state = if !watch.running {
            "not running (piloted or lost)".to_string()
        } else if watch.active.is_none() {
            "no rule decides (brake)".to_string()
        } else {
            format!("{} rule slots", watch.slots)
        };
        let _ = write!(s, "\ntested behavior: {}, {state}", watch.name);
        for (i, why) in watch.why.iter().enumerate() {
            let mark = if watch.active == Some(i) { '>' } else { ' ' };
            let _ = write!(s, "\n {mark} {}  {why}", i + 1);
        }
    }
    if text.0 != *s {
        text.0.clear();
        text.0.push_str(s);
    }
}
