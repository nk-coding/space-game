//! Scenes: what is in the world and who flies it. Chosen by URL parameters:
//!
//! * **sandbox** (default): the player flies the Halvex Runner (the focus); waves of automaton raiders under
//!   their content behavior (`raider_basic`), a new wave a few seconds after the last raider goes dark; the
//!   player's other vessels (`?fleet=N`, and pieces that split off with a logic core) fly their default
//!   behavior (`skirmisher`); wrecks and salvage drift away and are cleaned up far from the player; when the
//!   player has no vessel left, a new runner re-forms.
//!
//! Owners, vessels and the focus live in the sim (`sim::vessel`): the player is [`PLAYER`], the raiders
//! [`ENEMY`]; [`PilotFocus`] mirrors `Sim::focus(PLAYER)`, and focus switches are `ShipCommand::Focus`.
//! * **showcase** (`?scene=showcase`): the runner holds position, three raiders orbit it without firing.
//! * **load test** (`?load=1|5|20`): `sim::scenario::load_test(n, seed)`, no player, the camera frames the fight.
//!
//! The scene writes the player's pilot input into the tick inputs (`FixedUpdate`, before the step).

use std::collections::BTreeMap;

use bevy::prelude::*;
use sim::flight::{PilotAxes, nudge_step};
use sim::{Body, BehaviorLibrary, Maneuver, Owner, PilotInput, ShipCommand, ShipId, Target};

use crate::backdrop::Rng;
use crate::camera::{CamMode, CameraRig};
use crate::controls::PlayerInput;
use crate::fx::Effects;
use crate::params::Params;
use crate::sim_bridge::{HashLog, SimSet, SimWorld, console_out, despawn_all_views, sv, v2};
use crate::tuning::Tuning;

/// The player's owner in the sim, and the raiders'.
pub const PLAYER: Owner = Owner::PLAYER;
pub const ENEMY: Owner = Owner(1);

/// The vessel the player's input drives: a copy of `Sim::focus(PLAYER)`, refreshed every tick. Camera, HUD and
/// controls all read this. Every other vessel of the player runs its behaviour.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PilotFocus(pub Option<ShipId>);

/// Is this ship a vessel of the player?
pub fn is_friendly(world: &SimWorld, ship: ShipId) -> bool {
    world.sim.ship(ship).is_some_and(|s| s.team() == Some(PLAYER))
}

/// The player's vessels (ascending ship id), for focus cycling and the HUD.
pub fn player_vessels(world: &SimWorld) -> Vec<ShipId> {
    world.sim.vessels(PLAYER).map(|v| v.ship).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneKind {
    Sandbox,
    Showcase,
    Load(u32),
}

#[derive(Resource)]
pub struct Scene {
    pub kind: SceneKind,
    pub seed: u64,
    /// The last vessel the player flew (where a new runner re-forms; its wreck is removed then).
    last_focus: Option<ShipId>,
    /// Extra runners flying with the player (`?fleet=N`).
    wingmen: u32,
    pub wave: u32,
    pub wave_ships: Vec<ShipId>,
    next_wave_tick: Option<u64>,
    respawn_tick: Option<u64>,
    dark_since: BTreeMap<ShipId, u64>,
    rng: Rng,
    /// A short banner for the HUD and the sim time (s) until which it shows.
    pub banner: Option<(String, f64)>,
    pub kills: u32,
}

impl Scene {
    fn new(kind: SceneKind, seed: u64) -> Self {
        Self {
            kind,
            seed,
            last_focus: None,
            wingmen: 0,
            wave: 0,
            wave_ships: Vec::new(),
            next_wave_tick: None,
            respawn_tick: None,
            dark_since: BTreeMap::new(),
            rng: Rng::new(seed as u32 ^ 0x9E37_79B9),
            banner: None,
            kills: 0,
        }
    }
}

/// Set to rebuild the scene from scratch (tuning changes that need fresh designs).
#[derive(Resource, Default)]
pub struct ResetRequest(pub bool);

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        let params = app.world().resource::<Params>().clone();
        let seed = params.f32("seed").map(|s| s as u64).unwrap_or(42);
        let kind = if let Some(n) = params.f32("load") {
            SceneKind::Load(n.max(1.0) as u32)
        } else if params.flag("hashlog").unwrap_or(false) {
            SceneKind::Load(20)
        } else if params.text("scene") == Some("showcase") {
            SceneKind::Showcase
        } else {
            SceneKind::Sandbox
        };
        let mut scene = Scene::new(kind, seed);
        scene.wingmen = params.f32("fleet").map(|n| n.clamp(0.0, 8.0) as u32).unwrap_or(0);
        app.insert_resource(scene)
            .init_resource::<PilotFocus>()
            .init_resource::<ResetRequest>()
            .add_systems(Startup, setup)
            .add_systems(FixedUpdate, scene_tick.in_set(SimSet::Inputs))
            .add_systems(Update, reset);
    }
}

fn setup(
    mut commands: Commands,
    mut scene: ResMut<Scene>,
    mut focus: ResMut<PilotFocus>,
    tuning: Res<Tuning>,
    params: Res<Params>,
    mut rig: ResMut<CameraRig>,
) {
    let world = build_world(&mut scene, &mut focus, &tuning, &params, &mut rig);
    commands.insert_resource(world);
}

/// Creates the sim world for the current scene kind.
fn build_world(scene: &mut Scene, focus: &mut PilotFocus, tuning: &Tuning, params: &Params, rig: &mut CameraRig) -> SimWorld {
    let base = data::base_content();
    let wingmen = scene.wingmen;
    *scene = Scene::new(scene.kind, scene.seed);
    scene.wingmen = wingmen;
    focus.0 = None;
    match scene.kind {
        SceneKind::Load(n) => {
            let sim = sim::scenario::load_test(n as usize, scene.seed);
            let mut world = SimWorld::new(sim, base);
            if params.flag("hashlog").unwrap_or(false) {
                let ticks = params.f32("hashticks").map(|t| t as u64).unwrap_or(600);
                let label = format!("load ships={n} seed={} (browser)", scene.seed);
                console_out(&format!("# sim determinism scenario=load ships={n} seed={} ticks={ticks} (browser)", scene.seed));
                world.hashlog = Some(HashLog::new(label, ticks));
            }
            rig.mode = CamMode::Frame;
            rig.auto_mode = CamMode::Frame;
            rig.zoom = 6.0;
            rig.zoom_target = 6.0;
            rig.pos = Vec2::new(0.0, 40.0);
            info!("load test: {n} ships, seed {}", scene.seed);
            world
        }
        SceneKind::Sandbox | SceneKind::Showcase => {
            let content = tuning.scaled_content(&base);
            let mut sim = sim::Sim::with_config(scene.seed, tuning.config());
            match BehaviorLibrary::new(&content) {
                Ok(lib) => sim.set_behaviors(lib),
                Err(problems) => error!("behavior library: {} problems, first: {:?}", problems.len(), problems.first()),
            }
            let mut world = SimWorld::new(sim, content);
            let Some(runner) = world.design("halvex_runner") else { return world };
            let me = world.sim.spawn_ship(runner.clone(), PLAYER, Body::at(sim::Vec2::ZERO, 0.0));
            let mut fleet = vec![me];
            if scene.kind == SceneKind::Sandbox {
                for k in 0..scene.wingmen {
                    let x = (k as f32 / 2.0).floor() + 1.0;
                    let side = if k % 2 == 0 { -1.0 } else { 1.0 };
                    let id = world.sim.spawn_ship(runner.clone(), PLAYER, Body::at(sim::Vec2::new(side * 22.0 * x, -12.0 * x), 0.0));
                    fleet.push(id);
                }
                // Every vessel gets its default behaviour (`skirmisher`); the focus parks it while piloted.
                let behavior = world.sim.behaviors().default_for(&runner);
                for id in fleet {
                    world.sim.command(id, ShipCommand::Behavior(behavior));
                }
                world.sim.command(me, ShipCommand::Focus);
                focus.0 = world.sim.focus(PLAYER);
            }
            rig.mode = CamMode::Follow;
            rig.auto_mode = CamMode::Follow;
            rig.pos = Vec2::ZERO;
            if scene.kind == SceneKind::Showcase {
                world.sim.command(me, ShipCommand::Maneuver { maneuver: Maneuver::Hold { at: sim::Vec2::ZERO }, fire: false });
                if let Some(raider) = world.design("automaton_raider") {
                    for k in 0..3 {
                        let a = k as f32 * std::f32::consts::TAU / 3.0 + 0.4;
                        let pos = Vec2::new(a.cos(), a.sin()) * 16.0;
                        let id = world.sim.spawn_ship(raider.clone(), ENEMY, Body::at(sv(pos), a));
                        let orbit = Maneuver::Orbit { target: Target::Ship(me), radius: 16.0, ccw: true };
                        world.sim.command(id, ShipCommand::Maneuver { maneuver: orbit, fire: false });
                    }
                }
                // The showcase is for looking: nobody flies the runner (no focus).
                focus.0 = None;
                rig.mode = CamMode::Free;
                rig.auto_mode = CamMode::Free;
            } else {
                scene.next_wave_tick = Some(45);
            }
            world
        }
    }
}

fn scene_tick(
    mut scene: ResMut<Scene>,
    mut world: ResMut<SimWorld>,
    tuning: Res<Tuning>,
    mut player_input: ResMut<PlayerInput>,
    mut focus: ResMut<PilotFocus>,
) {
    if scene.kind != SceneKind::Sandbox {
        return;
    }
    let world = &mut *world;
    let scene = &mut *scene;
    let tick = world.sim.tick();
    let now = world.sim.time_seconds();

    // The sim owns the focus (it moves on by itself when the focus is lost); the client mirrors it.
    let now_focus = world.sim.focus(PLAYER);
    if now_focus != focus.0 {
        focus.0 = now_focus;
        player_input.lock = None;
    }
    if now_focus.is_some() {
        scene.last_focus = now_focus;
    }

    // The focus.
    if let Some(me) = focus.0 {
        match world.sim.ship(me) {
            Some(ship) if ship.is_vessel() => {
                // Drop a lock on a target that went dark or vanished.
                let input = &mut *player_input;
                let lock = match input.lock {
                    Some(Target::Ship(s)) if world.sim.ship(s).is_some_and(|s| s.is_active()) => input.lock,
                    Some(Target::Piece(s, p)) if world.sim.ship(s).and_then(|s| s.piece(p)).is_some() => input.lock,
                    Some(Target::Point(_)) => input.lock,
                    _ => None,
                };
                input.lock = lock;
                // The A/D nudge only acts under a lock; it springs back otherwise (one step per tick).
                let turn = if lock.is_some() && !input.stick_mode { input.axes.x } else { 0.0 };
                input.nudge = nudge_step(input.nudge, turn);
                let axes = (!input.stick_mode).then_some(PilotAxes { turn: input.axes.x, thrust: input.axes.y, strafe: input.axes.z });
                let pilot = PilotInput {
                    stick: sv(input.stick),
                    lock,
                    drift: input.drift,
                    fire: input.fire,
                    axes,
                    aim_offset: input.nudge,
                    ..default()
                };
                world.inputs.commands.push((me, ShipCommand::Pilot(pilot)));
            }
            _ => {}
        }
    } else if scene.respawn_tick.is_none() && scene.last_focus.is_some() {
        // No vessel left: re-form in a moment.
        scene.respawn_tick = Some(tick + 90);
        scene.banner = Some(("SHIP LOST".into(), now + 3.0));
    }
    if let Some(t) = scene.respawn_tick
        && tick >= t
    {
        scene.respawn_tick = None;
        let wreck = scene.last_focus.take().filter(|id| world.sim.ship(*id).is_some_and(|s| !s.is_vessel()));
        let at = wreck.and_then(|me| world.sim.ship(me)).and_then(|s| s.main_piece().and_then(|p| s.piece(p))).map(|p| p.com_world()).unwrap_or(sim::Vec2::ZERO);
        if let Some(me) = wreck {
            world.sim.remove_ship(me);
        }
        if let Some(runner) = world.design("halvex_runner") {
            let me = world.sim.spawn_ship(runner.clone(), PLAYER, Body::at(at, 0.0));
            let behavior = world.sim.behaviors().default_for(&runner);
            world.sim.command(me, ShipCommand::Behavior(behavior));
            world.sim.command(me, ShipCommand::Focus);
            focus.0 = world.sim.focus(PLAYER);
            scene.last_focus = focus.0;
            player_input.lock = None;
            scene.banner = Some(("RE-FORMED".into(), now + 2.5));
        }
    }

    // Waves (a raider shot in half can be two raiders: count the enemy's running vessels).
    let alive = world.sim.vessels(ENEMY).filter(|v| v.state == sim::VesselState::Running).count();
    if alive == 0 && scene.next_wave_tick.is_none() && !scene.wave_ships.is_empty() {
        scene.next_wave_tick = Some(tick + 120);
        scene.banner = Some((format!("WAVE {} CLEARED", scene.wave), now + 3.0));
    }
    if let Some(t) = scene.next_wave_tick
        && tick >= t
    {
        scene.next_wave_tick = None;
        scene.wave += 1;
        scene.wave_ships.clear();
        let centre = focus
            .0
            .and_then(|me| world.sim.ship(me))
            .and_then(|s| s.control_piece().and_then(|p| s.piece(p)))
            .map(|p| v2(p.com_world()))
            .unwrap_or(Vec2::ZERO);
        let n = tuning.raiders_per_wave.max(1);
        if let Some(raider) = world.design("automaton_raider") {
            let base = scene.rng.range(0.0, std::f32::consts::TAU);
            for k in 0..n {
                let a = base + (k as f32 - (n as f32 - 1.0) / 2.0) * 0.32;
                let r = scene.rng.range(105.0, 125.0);
                let pos = centre + Vec2::new(a.cos(), a.sin()) * r;
                let to = (centre - pos).normalize_or_zero();
                let body = Body { pos: sv(pos), angle: (-to.x).atan2(to.y), linvel: sv(to * 3.0), angvel: 0.0 };
                let behavior = world.sim.behaviors().default_for(&raider);
                let id = world.sim.spawn_ship(raider.clone(), ENEMY, body);
                world.sim.command(id, ShipCommand::Behavior(behavior));
                scene.wave_ships.push(id);
            }
        }
        scene.banner = Some((format!("WAVE {} · {} RAIDERS", scene.wave, n), now + 3.0));
    }

    // Clean up wrecks: dark ships far from the player, or dark for a long time.
    if tick.is_multiple_of(30) {
        let centre = focus
            .0
            .and_then(|me| world.sim.ship(me))
            .and_then(|s| s.main_piece().and_then(|p| s.piece(p)))
            .map(|p| v2(p.com_world()))
            .unwrap_or(Vec2::ZERO);
        let mut remove: Vec<ShipId> = Vec::new();
        for s in world.sim.ships() {
            // The player's vessels stay (dark and dormant ones too).
            if s.team() == Some(PLAYER) || s.is_active() {
                continue;
            }
            let since = *scene.dark_since.entry(s.id()).or_insert_with(|| {
                if scene.wave_ships.contains(&s.id()) {
                    scene.kills += 1;
                }
                tick
            });
            let far = s.pieces().iter().all(|p| v2(p.com_world()).distance(centre) > 320.0);
            if far || tick - since > 30 * 75 {
                remove.push(s.id());
            }
        }
        for id in remove {
            world.sim.remove_ship(id);
            scene.dark_since.remove(&id);
        }
    }
}

/// Rebuilds the whole scene (tuning changes that need fresh designs, or the reset key).
#[allow(clippy::too_many_arguments)]
fn reset(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut request: ResMut<ResetRequest>,
    mut scene: ResMut<Scene>,
    mut world: ResMut<SimWorld>,
    tuning: Res<Tuning>,
    params: Res<Params>,
    mut rig: ResMut<CameraRig>,
    mut fx: ResMut<Effects>,
    mut input: ResMut<PlayerInput>,
    mut focus: ResMut<PilotFocus>,
) {
    if keys.just_pressed(KeyCode::KeyR) && keys.pressed(KeyCode::ShiftLeft) {
        request.0 = true;
    }
    if !request.0 {
        return;
    }
    request.0 = false;
    despawn_all_views(&mut commands, &mut world);
    fx.clear();
    input.lock = None;
    *world = build_world(&mut scene, &mut focus, &tuning, &params, &mut rig);
    info!("scene reset ({:?})", scene.kind);
}
