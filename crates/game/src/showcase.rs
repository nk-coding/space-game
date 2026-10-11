//! The showcase scene: the Halvex Runner drifting and turning gently with its engines lit, three automaton
//! raiders circling it. Motion is scripted in `FixedUpdate` at the sim rate and written into `SimPose` /
//! `EngineThrottle`, exactly where the real simulation bridge will write later.

use bevy::prelude::*;

use crate::atlas::Atlas;
use crate::params::Params;
use crate::ship_model::ship_visual;
use crate::ship_view::spawn_ship_view;
use crate::sim_bridge::{EngineThrottle, Pose2, SimPose, SIM_HZ};
use crate::style::Style;
use crate::trails::TrailEmitter;
use crate::RenderToggles;

#[derive(Component, Clone, Copy)]
enum Script {
    /// Slow drift around the origin with a gentle turn.
    Drift,
    /// Circle the origin on an ellipse (`squash` = height / width, so the raiders stay on a landscape
    /// screen); negative speed = clockwise.
    Orbit { radius: f32, squash: f32, speed: f32, phase: f32 },
}

#[derive(Resource)]
struct ShowcaseClock {
    ticks: u64,
    /// Freeze motion (screenshots): ships keep their start pose, engines stay at full throttle.
    still: bool,
}

pub struct ShowcasePlugin;

impl Plugin for ShowcasePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup.after(crate::spawn_camera)).add_systems(FixedUpdate, tick);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    params: Res<Params>,
    mut toggles: ResMut<RenderToggles>,
) {
    let content = data::base_content();
    // `?ship=<blueprint id>` shows another blueprint in the player's place.
    let player_id = params.text("ship").unwrap_or("halvex_runner");
    let runner = content.ships.get(player_id).or_else(|| {
        warn!("unknown ship `{player_id}`, showing halvex_runner");
        content.ships.get("halvex_runner")
    });
    let runner = runner.expect("base content has halvex_runner");
    let raider = content.ships.get("automaton_raider").expect("base content has automaton_raider");
    let runner_v = ship_visual(&content, runner, &style).expect("halvex_runner visual");
    let raider_v = ship_visual(&content, raider, &style).expect("automaton_raider visual");
    info!(
        "showcase: {} {} parts, pivot {:?}; automaton_raider {} parts, pivot {:?}",
        runner.id,
        runner_v.parts.len(),
        runner_v.pivot,
        raider_v.parts.len(),
        raider_v.pivot
    );

    let still = params.flag("still").unwrap_or(false);
    let start = params.f32("t").unwrap_or(4.0) as f64;
    let start_ticks = (start * SIM_HZ) as u64;

    let pose0 = pose_of(Script::Drift, start_ticks, still);
    let player = spawn_ship_view(&mut commands, &mut meshes, &atlas, &style, &runner_v, pose0, 20.0, &toggles);
    commands.entity(player).insert((Script::Drift, TrailEmitter::default()));
    toggles.ring_target = Some(player);

    // One shared ellipse, evenly spaced, so the raiders never cross each other.
    let orbits = [0.5f32, 2.594, 4.689].map(|phase| Script::Orbit { radius: 12.0, squash: 0.62, speed: 0.13, phase });
    for (k, s) in orbits.into_iter().enumerate() {
        let pose = pose_of(s, start_ticks, still);
        let e = spawn_ship_view(&mut commands, &mut meshes, &atlas, &style, &raider_v, pose, 10.0 + k as f32, &toggles);
        commands.entity(e).insert((s, TrailEmitter::default()));
    }
    commands.insert_resource(ShowcaseClock { ticks: start_ticks, still });
}

fn pose_of(script: Script, ticks: u64, still: bool) -> Pose2 {
    let t = ticks as f32 / SIM_HZ as f32;
    match script {
        Script::Drift if still => Pose2::new(Vec2::ZERO, 0.0),
        Script::Drift => Pose2::new(
            Vec2::new(2.2 * (0.11 * t).sin(), 1.3 * (0.07 * t + 1.0).sin()),
            0.22 * (0.09 * t).sin() + 0.08 * (0.23 * t).sin(),
        ),
        Script::Orbit { radius, squash, speed, phase } => {
            let a = phase + if still { 0.0 } else { speed * t };
            let r = radius * (1.0 + if still { 0.0 } else { 0.04 * (0.5 * t + 2.0 * phase).sin() });
            let pos = Vec2::new(a.cos(), squash * a.sin()) * r;
            // Nose (local +y) along the direction of travel.
            let vel = Vec2::new(-a.sin(), squash * a.cos()) * speed.signum();
            Pose2::new(pos, (-vel.x).atan2(vel.y))
        }
    }
}

fn throttle_of(script: Script, engine: usize, ticks: u64, still: bool) -> f32 {
    if still {
        return 1.0;
    }
    let t = ticks as f32 / SIM_HZ as f32;
    match script {
        Script::Drift => (0.82 + 0.14 * (0.5 * t + engine as f32 * 0.9).sin()).clamp(0.0, 1.0),
        Script::Orbit { .. } => (0.7 + 0.2 * (0.8 * t + engine as f32).sin()).clamp(0.0, 1.0),
    }
}

fn tick(mut clock: ResMut<ShowcaseClock>, mut ships: Query<(&Script, &mut SimPose, &mut EngineThrottle)>) {
    clock.ticks += 1;
    for (script, mut pose, mut throttle) in &mut ships {
        pose.push(pose_of(*script, clock.ticks, clock.still));
        for (i, th) in throttle.0.iter_mut().enumerate() {
            *th = throttle_of(*script, i, clock.ticks, clock.still);
        }
    }
}
