//! Debug overlay (fps, frame time, entity count, toggles) and the keyboard toggles behind it.

use std::fmt::Write as _;

use bevy::diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use crate::camera::{CameraRig, MainCamera};
use crate::style::{ui_color, Style};
use crate::RenderToggles;

#[derive(Component)]
struct HudText;

#[derive(Resource)]
struct HudTimer(Timer);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((FrameTimeDiagnosticsPlugin::default(), EntityCountDiagnosticsPlugin::default()))
            .insert_resource(HudTimer(Timer::from_seconds(0.25, TimerMode::Repeating)))
            .add_systems(Startup, spawn_hud)
            .add_systems(Update, (keyboard_toggles, apply_bloom, update_hud));
    }
}

fn spawn_hud(mut commands: Commands, style: Res<Style>) {
    commands.spawn((
        HudText,
        Text::new("…"),
        TextFont { font_size: FontSize::Px(13.0), ..default() },
        TextColor(ui_color(style.ui_label, 1.0)),
        Node { position_type: PositionType::Absolute, top: px(12), left: px(14), ..default() },
    ));
}

fn keyboard_toggles(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggles: ResMut<RenderToggles>,
    mut rig: ResMut<CameraRig>,
    mut time: ResMut<Time<Virtual>>,
) {
    if keys.just_pressed(KeyCode::KeyD) {
        toggles.debug_shapes = !toggles.debug_shapes;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        toggles.bloom = !toggles.bloom;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        toggles.glow_sprites = !toggles.glow_sprites;
    }
    if keys.just_pressed(KeyCode::KeyS) {
        toggles.shields = !toggles.shields;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        toggles.hud = !toggles.hud;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        rig.follow = if rig.follow.is_some() { None } else { toggles.ring_target };
        rig.center = Vec2::ZERO;
    }
    if keys.just_pressed(KeyCode::KeyP) || keys.just_pressed(KeyCode::Space) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
}

fn apply_bloom(mut commands: Commands, toggles: Res<RenderToggles>, camera: Single<(Entity, Has<Bloom>), With<MainCamera>>) {
    let (entity, has) = *camera;
    if toggles.bloom && !has {
        commands.entity(entity).insert(Bloom { intensity: 0.3, ..Bloom::NATURAL });
    } else if !toggles.bloom && has {
        // The camera keeps `Hdr` (needed for sRGB compositing on WebGL2, see `spawn_camera`).
        commands.entity(entity).remove::<Bloom>();
    }
}

fn update_hud(
    time: Res<Time<Real>>,
    vtime: Res<Time<Virtual>>,
    mut timer: ResMut<HudTimer>,
    diagnostics: Res<DiagnosticsStore>,
    toggles: Res<RenderToggles>,
    rig: Res<CameraRig>,
    hud: Single<(&mut Text, &mut Visibility), With<HudText>>,
) {
    let (mut text, mut vis) = hud.into_inner();
    let want = if toggles.hud { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
    if !timer.0.tick(time.delta()).just_finished() && !toggles.is_changed() {
        return;
    }
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let ms = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let ents = diagnostics.get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT).and_then(|d| d.value()).unwrap_or(0.0);
    let on = |b: bool| if b { "on " } else { "off" };
    let s = &mut text.0;
    s.clear();
    let _ = write!(
        s,
        "FPS {fps:5.1}  {ms:5.2} ms  |  {ents:.0} entities  |  zoom {:.0} px/m{}\n\
         [D] debug shapes {}   [G] glow sprites {}   [B] bloom {}   [S] shields {}\n\
         [F] follow {}   [P] pause   [H] hide   wheel/pinch zoom | drag pan",
        rig.px_per_m,
        if vtime.is_paused() { "  |  PAUSED" } else { "" },
        on(toggles.debug_shapes),
        on(toggles.glow_sprites),
        on(toggles.bloom),
        on(toggles.shields),
        on(rig.follow.is_some()),
    );
}
