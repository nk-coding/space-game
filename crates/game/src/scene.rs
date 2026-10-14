//! Scenes: what is in the world and who flies it. Chosen by URL parameters, or by the screen that starts a flight:
//!
//! * **sandbox** (home's TEST RANGE, `?scene=sandbox`): the player flies their design (`drydock::PlayerDesign`; at the first start a copy of
//!   the Halvex Runner, then the autosaved working design) as the focus; waves of raiders (the first content ship
//!   tagged `enemy`: the automaton raider) under their content behavior (`raider_basic`), a new wave a few seconds
//!   after the last raider goes dark (C turns the waves off and on), each wave as big as the flown design calls for
//!   ([`wave_size`], GDR-0074) unless the tuning panel sets it by hand; the player's other vessels (`?fleet=N` more of the design, their leader the player's ship, and pieces that split off with a
//!   logic core) fly the behavior the design names (default: its faction's, `skirmisher`; the player's saved
//!   behaviors are in the flight's library, `drydock::behaviors`); wrecks and salvage drift away and are cleaned up
//!   far from the player, loose items after 60 s ([`SANDBOX_LOOSE_TICKS`]); when the player has no vessel left, a
//!   new one re-forms.
//! * **test flight**: the sandbox launched from the drydock (T). Losing every vessel returns to the drydock
//!   after the same 3 s instead of re-forming; T returns at any time. The design itself is never touched.
//! * **TEST IN RANGE** (`test_range.rs`; `?scene=watch|alongside&behavior=<id>`): a test flight that flies a
//!   behavior: **watch** (the design unpiloted on it, the camera follows it, focus switches are dropped; losing it
//!   ends the test) or **alongside** (the player flies the design, a second copy is a wingman on it). T and a lost
//!   test return to where the test started (`mode::FlightExit`).
//! * **showcase** (`?scene=showcase`): the design holds position, three raiders orbit it without firing; with
//!   `&fire=1` they attack it under their content behavior instead (for looking at shield hits).
//! * **load test** (`?load=1|5|20`): `sim::scenario::load_test(n, seed)`, no player, the camera frames the fight.
//! * **voyage** (`voyage.rs` launches it: home's V, `?scene=voyage&voyage=<id>`): the sim gets the career's mind
//!   capacity and is seeded with the voyage's seed; the plan is rolled from the content the enemies fly with
//!   (`sim::VoyagePlan::roll`) and the sim directs it (`Sim::start_voyage`); the flagship (the drydock's working
//!   design) and the FLEET escorts (following the flagship's lock, like `?fleet=` wingmen, on their design's
//!   behavior) enter at the plan's entry, the flagship the focus. No waves, no re-form, no wreck clean-up; Shift+R,
//!   T does nothing (RECALL goes home; M asks in a panel whether to abandon the voyage, `voyage.rs`).
//!
//! The scene writes the player's pilot input into the tick inputs (`FixedUpdate`, before the step). Leaving
//! flight (`OnExit(AppMode::Flight)`) tears the scene down to an empty world; entering it from another mode
//! (`OnEnter`) builds what that mode asked for ([`Scene::launch`]): the test flight or the behavior test, or the
//! sandbox from home.
//!
//! Owners, vessels and the focus live in the sim (`sim::vessel`): the player is [`PLAYER`], the raiders
//! [`ENEMY`]; [`PilotFocus`] mirrors `Sim::focus(PLAYER)`, and focus switches are `ShipCommand::Focus`.

use std::collections::BTreeMap;
use std::sync::Arc;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::input::egui_wants_any_keyboard_input;
use sim::flight::{PilotAxes, nudge_step, turn_ramp_step};
use sim::{Body, BehaviorLibrary, Maneuver, Owner, PilotInput, ShipCommand, ShipDesign, ShipId, Target};

use crate::backdrop::Rng;
use crate::camera::{CamMode, CameraRig};
use crate::controls::PlayerInput;
use crate::drydock::PlayerDesign;
use crate::drydock::behaviors::{self, PlayerBehaviors};
use crate::fx::Effects;
use crate::mode::{AppMode, FlightExit};
use crate::params::Params;
use crate::shield::ShieldBubbles;
use crate::sim_bridge::{GameContent, HashLog, SimSet, SimWorld, console_out, despawn_all_views, sv, v2};
use crate::test_range::{self, BehaviorTest, TestMode};
use crate::tuning::Tuning;
use crate::ui_kit::Tone;
use crate::voyage::VoyageRun;

/// The player's owner in the sim, and the raiders'.
pub const PLAYER: Owner = Owner::PLAYER;
pub const ENEMY: Owner = Owner(1);

/// The vessel the player's input drives: a copy of `Sim::focus(PLAYER)`, refreshed every tick. Camera, HUD and
/// controls all read this. Every other vessel of the player runs its behaviour.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PilotFocus(pub Option<ShipId>);

/// Content ships with this tag are raiders: the sandbox's waves and the showcase's attackers fly the first one
/// (content order: the base pack's `automaton_raider`).
const RAIDER_TAG: &str = "enemy";

/// How long a loose item (a core fragment, spilled cargo) drifts in the sandbox and the showcase before it is gone
/// (ticks; 1800 = 60 s, the sim's default until GDR-0079). The sim keeps loose items until it ends (a voyage does),
/// but the sandbox's waves never end, so without a lifetime they would pile up as wrecks would without their
/// clean-up.
const SANDBOX_LOOSE_TICKS: u32 = 1800;

/// The design the raiders fly, compiled for [`ENEMY`].
fn raider_design(world: &mut SimWorld) -> Option<Arc<ShipDesign>> {
    let Some(id) = world.content.ships.iter().find(|s| s.tags.iter().any(|t| t == RAIDER_TAG)).map(|s| s.id.clone()) else {
        warn_once!("no content ship is tagged `{RAIDER_TAG}`: no raiders");
        return None;
    };
    world.design(&id, ENEMY)
}

/// Raiders per sandbox wave when the tuning does not set it by hand (GDR-0074, PLACEHOLDER rule): the flown
/// design's build points over a raider's, times [`WAVE_SHARE`], rounded, at least 1 and at most [`WAVE_CAP`].
/// With the content of 14 Oct 2026 (a raider is 15.8 build points) the starter `rim_skiff` (59) meets 2 raiders
/// and the Halvex Runner (132.5) 4: waves a pilot wins about nine times in ten
/// (`docs/results/slice-balance-2026-10-13.md`, "GDR-0074").
pub fn wave_size(design_bp: f32, raider_bp: f32) -> u32 {
    if !(design_bp > 0.0 && raider_bp > 0.0) {
        return 1;
    }
    (design_bp / raider_bp * WAVE_SHARE).round().clamp(1.0, WAVE_CAP as f32) as u32
}

/// One raider per this share of a raider's build points in the flown design ([`wave_size`]).
pub const WAVE_SHARE: f32 = 0.5;
/// The biggest wave [`wave_size`] makes.
pub const WAVE_CAP: u32 = 8;

/// A compiled design's build points (as placed: hull parts with their material's factor).
fn build_points(design: &ShipDesign) -> f32 {
    design.parts.iter().map(|p| p.build_points).sum()
}

/// Is this ship a vessel of the player?
pub fn is_friendly(world: &SimWorld, ship: ShipId) -> bool {
    world.sim.ship(ship).is_some_and(|s| s.team() == Some(PLAYER))
}

/// The player's vessels (ascending ship id), for focus cycling and the HUD.
pub fn player_vessels(world: &SimWorld) -> Vec<ShipId> {
    world.sim.vessels(PLAYER).map(|v| v.ship).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneKind {
    Sandbox,
    Showcase,
    Load(u32),
    /// A voyage (content id), as `voyage::VoyageRun` launched it.
    Voyage(String),
}

/// A flight another mode asked for; `OnEnter(AppMode::Flight)` builds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Launch {
    /// The drydock's test flight (T, TEST-FLY) or TEST IN RANGE: losing every vessel goes back.
    TestFlight,
    /// Home's TEST RANGE: the sandbox as at a plain start (a lost ship re-forms).
    Range,
    /// A voyage `voyage::VoyageRun` holds (the scene's kind says which).
    Voyage,
}

#[derive(Resource)]
pub struct Scene {
    pub kind: SceneKind,
    pub seed: u64,
    /// Launched from the drydock: losing every vessel returns there instead of re-forming.
    pub test_flight: bool,
    /// Raider waves come (sandbox; C toggles). Off: the raiders are gone and no wave starts.
    pub waves: bool,
    /// Another mode asked for a flight; `OnEnter(AppMode::Flight)` builds it.
    pub launch: Option<Launch>,
    /// The player's design as compiled for this scene (the player, the wingmen and re-forms fly it).
    player_design: Option<Arc<ShipDesign>>,
    /// The last vessel the player flew (where a new one re-forms; its wreck is removed then).
    last_focus: Option<ShipId>,
    /// Extra copies of the design flying with the player (`?fleet=N`).
    wingmen: u32,
    pub wave: u32,
    pub wave_ships: Vec<ShipId>,
    next_wave_tick: Option<u64>,
    respawn_tick: Option<u64>,
    dark_since: BTreeMap<ShipId, u64>,
    rng: Rng,
    /// A short banner for the HUD and the sim time (s) until which it shows.
    pub banner: Option<(String, f64)>,
    /// The line under the banner and the banner's colour (voyages' site banners; [`Scene::say`]).
    pub banner_sub: String,
    pub banner_tone: Tone,
    pub kills: u32,
    /// TEST IN RANGE: the behavior and how it flies (`test_range.rs`); kept by restarts, ended by leaving flight.
    test: Option<BehaviorTest>,
    /// The ship running the tested behavior (watch: the design; alongside: the wingman).
    test_ship: Option<ShipId>,
}

impl Scene {
    pub(crate) fn new(kind: SceneKind, seed: u64) -> Self {
        Self {
            kind,
            seed,
            test_flight: false,
            waves: true,
            launch: None,
            player_design: None,
            last_focus: None,
            wingmen: 0,
            wave: 0,
            wave_ships: Vec::new(),
            next_wave_tick: None,
            respawn_tick: None,
            dark_since: BTreeMap::new(),
            rng: Rng::new(seed as u32 ^ 0x9E37_79B9),
            banner: None,
            banner_sub: String::new(),
            banner_tone: Tone::Teal,
            kills: 0,
            test: None,
            test_ship: None,
        }
    }

    /// A fresh scene of the same kind and seed that keeps the settings that outlive a scene.
    fn restart(&self) -> Self {
        let mut s = Scene::new(self.kind.clone(), self.seed);
        s.wingmen = self.wingmen;
        s.test_flight = self.test_flight;
        s.waves = self.waves;
        s.test = self.test.clone();
        s
    }

    /// The next flight is TEST IN RANGE: a test flight in the sandbox with raider waves, flying `test`.
    pub fn begin_test(&mut self, test: BehaviorTest) {
        self.kind = SceneKind::Sandbox;
        self.test_flight = true;
        self.waves = true;
        self.test = Some(test);
    }

    /// How the running test flies, if this is TEST IN RANGE.
    pub fn test_mode(&self) -> Option<TestMode> {
        self.test.as_ref().map(|t| t.mode)
    }

    /// The ship running the tested behavior.
    pub fn test_ship(&self) -> Option<ShipId> {
        self.test_ship
    }

    /// The ship the camera follows and the HUD shows while nobody is piloted (watch).
    pub fn watched(&self) -> Option<ShipId> {
        self.test_ship.filter(|_| self.test_mode() == Some(TestMode::Watch))
    }

    /// A lost test flight going back (to the drydock, or the screen a behavior test started from): the tick it
    /// leaves at (`warp_fx.rs` dips the view to dark before it).
    pub fn leaving_at(&self) -> Option<u64> {
        self.respawn_tick.filter(|_| self.test_flight)
    }

    /// The voyage this scene flies, if it is one.
    pub fn voyage(&self) -> Option<&str> {
        match &self.kind {
            SceneKind::Voyage(id) => Some(id),
            _ => None,
        }
    }

    /// A banner with a line under it, in a tone, until the sim time `until` (s).
    pub fn say(&mut self, title: String, sub: String, tone: Tone, until: f64) {
        self.banner = Some((title, until));
        self.banner_sub = sub;
        self.banner_tone = tone;
    }
}

/// Set to rebuild the scene from scratch (tuning changes that need fresh designs).
#[derive(Resource, Default)]
pub struct ResetRequest(pub bool);

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        let params = app.world().resource::<Params>().clone();
        let seed = params.u64("seed").unwrap_or(42);
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
        if let Some(test) = BehaviorTest::from_params(&params) {
            scene.begin_test(test);
        }
        // A world exists from the start (the initial state's enter systems run before Startup builds the scene).
        app.insert_resource(empty_world(&scene))
            .insert_resource(scene)
            .init_resource::<PilotFocus>()
            .init_resource::<ResetRequest>()
            .add_systems(Startup, setup)
            .add_systems(FixedUpdate, scene_tick.in_set(SimSet::Inputs))
            .add_systems(
                Update,
                (reset, toggle_waves.run_if(not(egui_wants_any_keyboard_input))).run_if(in_state(AppMode::Flight)),
            )
            .add_systems(OnExit(AppMode::Flight), leave_flight)
            .add_systems(OnEnter(AppMode::Flight), launch);
    }
}

/// What a new scene is built from.
#[derive(SystemParam)]
pub(crate) struct SceneInputs<'w> {
    tuning: Res<'w, Tuning>,
    params: Res<'w, Params>,
    content: Res<'w, GameContent>,
    design: Res<'w, PlayerDesign>,
    behaviors: Res<'w, PlayerBehaviors>,
    voyage: Res<'w, VoyageRun>,
}

/// The scene at start (in the drydock the world stays empty).
pub(crate) fn setup(
    mut world: ResMut<SimWorld>,
    mut scene: ResMut<Scene>,
    mut focus: ResMut<PilotFocus>,
    inputs: SceneInputs,
    mode: Res<State<AppMode>>,
    mut rig: ResMut<CameraRig>,
) {
    if *mode.get() == AppMode::Flight {
        *world = build_world(&mut scene, &mut focus, &inputs, &mut rig);
    }
}

/// The world while the drydock is open: no ships (flight systems that read the world find nothing).
fn empty_world(scene: &Scene) -> SimWorld {
    SimWorld::new(sim::Sim::new(scene.seed), data::Content::default())
}

/// Creates the sim world for the current scene kind.
fn build_world(scene: &mut Scene, focus: &mut PilotFocus, inputs: &SceneInputs, rig: &mut CameraRig) -> SimWorld {
    let (tuning, params, base) = (&*inputs.tuning, &*inputs.params, &inputs.content.0);
    *scene = scene.restart();
    focus.0 = None;
    match scene.kind.clone() {
        SceneKind::Voyage(id) => build_voyage(scene, focus, inputs, rig, &id),
        SceneKind::Load(n) => {
            let sim = sim::scenario::load_test(n as usize, scene.seed);
            let mut world = SimWorld::new(sim, base.clone());
            if params.flag("hashlog").unwrap_or(false) {
                let ticks = params.u64("hashticks").unwrap_or(600).max(1);
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
            // The tuning's damage scale is the player's only: other sides compile from their own content. The
            // player's saved behaviors join the player's content (after the content's, in id order), so the
            // library has them and a design naming one compiles with it.
            let mut content = tuning.scaled_content(base, true);
            inputs.behaviors.add_to(&mut content);
            let others = tuning.scaled_content(base, false);
            let mut config = tuning.config();
            config.salvage.fragment_ticks = Some(SANDBOX_LOOSE_TICKS);
            let mut sim = sim::Sim::with_config(scene.seed, config);
            match BehaviorLibrary::new(&content) {
                Ok(lib) => sim.set_behaviors(lib),
                Err(problems) => error!("behavior library: {} problems, first: {:?}", problems.len(), problems.first()),
            }
            let mut world = SimWorld::new(sim, content).with_others(others);
            // A behavior the design names that can't fly (errors, deleted): its faction's, with a banner.
            let fallback = behaviors::fallback(inputs.design.blueprint(), &world.content, &inputs.behaviors);
            let bp = fallback.as_ref().map_or(inputs.design.blueprint(), |(bp, _)| bp);
            let Some(runner) = world.design_from(bp, PLAYER) else { return world };
            if let Some((_, why)) = &fallback {
                warn!("{why}");
                scene.banner = Some((why.to_uppercase(), 4.0));
            }
            scene.player_design = Some(runner.clone());
            // `Body` places the design's main piece, wherever the blueprint sits in its drydock frame.
            let me = world.sim.spawn_ship(runner.clone(), PLAYER, Body::at(sim::Vec2::ZERO, 0.0));
            if scene.kind == SceneKind::Sandbox {
                // Every vessel flies the behavior its design names; the focus parks it while piloted. Wingmen
                // follow the player's lock (`target locked`).
                let behavior = world.sim.behaviors().default_for(&runner);
                world.sim.command(me, ShipCommand::Behavior(behavior));
                let wingmen = if scene.test.is_some() { 0 } else { scene.wingmen };
                for k in 0..wingmen {
                    let id = world.sim.spawn_ship(runner.clone(), PLAYER, Body::at(wing_slot(k), 0.0));
                    world.sim.command(id, ShipCommand::Behavior(behavior));
                    world.sim.command(id, ShipCommand::Leader(Some(me)));
                }
                if scene.test_mode() != Some(TestMode::Watch) {
                    world.sim.command(me, ShipCommand::Focus);
                    focus.0 = world.sim.focus(PLAYER);
                }
                if let Some(test) = scene.test.clone() {
                    start_test(scene, &mut world, &runner, me, &test);
                }
            }
            rig.mode = CamMode::Follow;
            rig.auto_mode = CamMode::Follow;
            rig.pos = Vec2::ZERO;
            if scene.kind == SceneKind::Showcase {
                world.sim.command(me, ShipCommand::Maneuver { maneuver: Maneuver::Hold { at: sim::Vec2::ZERO }, fire: false });
                // Attacking raiders (as many as the tuning's raiders per wave) start where their behavior fights
                // from (about 45-50 m), so hits come at once.
                let attack = params.flag("fire").unwrap_or(false);
                let (n, dist) = if attack { (tuning.raiders_per_wave.unwrap_or(3).max(1), 46.0) } else { (3, 16.0) };
                if let Some(raider) = raider_design(&mut world) {
                    for k in 0..n {
                        let a = k as f32 * std::f32::consts::TAU / n as f32 + 0.4;
                        let pos = Vec2::new(a.cos(), a.sin()) * dist;
                        // Attackers face the runner (like wave spawns); orbiters keep the old pose.
                        let facing = if attack { a.cos().atan2(-a.sin()) } else { a };
                        let id = world.sim.spawn_ship(raider.clone(), ENEMY, Body::at(sv(pos), facing));
                        let command = if attack {
                            ShipCommand::Behavior(world.sim.behaviors().default_for(&raider))
                        } else {
                            let orbit = Maneuver::Orbit { target: Target::Ship(me), radius: 16.0, ccw: true };
                            ShipCommand::Maneuver { maneuver: orbit, fire: false }
                        };
                        world.sim.command(id, command);
                    }
                }
                // The showcase is for looking: nobody flies the runner (no focus).
                focus.0 = None;
                rig.mode = CamMode::Free;
                rig.auto_mode = CamMode::Free;
            } else if scene.waves {
                scene.next_wave_tick = Some(45);
            }
            world
        }
    }
}

/// A voyage's world (see the module docs): the sim with the career's mind capacity and the voyage's seed, the
/// plan rolled from the content the enemies fly with and started, the fleet at the entry. Without a launch (or
/// content that can't roll it) the world stays empty and says why.
fn build_voyage(scene: &mut Scene, focus: &mut PilotFocus, inputs: &SceneInputs, rig: &mut CameraRig, id: &str) -> SimWorld {
    let (tuning, base) = (&*inputs.tuning, &inputs.content.0);
    scene.waves = false;
    rig.mode = CamMode::Follow;
    rig.auto_mode = CamMode::Follow;
    let Some(launched) = inputs.voyage.launched.as_ref().filter(|l| l.voyage == id) else {
        error!("voyage {id}: not launched");
        return empty_world(scene);
    };
    let mut content = tuning.scaled_content(base, true);
    inputs.behaviors.add_to(&mut content);
    let others = tuning.scaled_content(base, false);
    let mut config = tuning.config();
    config.mind_capacity[PLAYER.0 as usize] = launched.mind;
    let mut sim = sim::Sim::with_config(launched.seed, config);
    match BehaviorLibrary::new(&content) {
        Ok(lib) => sim.set_behaviors(lib),
        Err(problems) => error!("behavior library: {} problems, first: {:?}", problems.len(), problems.first()),
    }
    // The sites' ships compile from the content every other owner flies with (`SimWorld::design` for ENEMY).
    let plan = others.voyages.get(id).ok_or_else(|| format!("no voyage `{id}`")).and_then(|v| sim::VoyagePlan::roll(&others, v, launched.seed));
    let mut world = SimWorld::new(sim, content).with_others(others);
    let plan = match plan {
        Ok(plan) => plan,
        Err(e) => {
            error!("voyage {id}: {e}");
            scene.banner = Some((format!("THE VOYAGE DOESN'T ROLL: {e}").to_uppercase(), 6.0));
            return world;
        }
    };
    let entry = plan.entry;
    world.sim.start_voyage(plan, PLAYER, ENEMY);
    rig.pos = v2(entry.pos);
    // The flagship, then the escorts in their slots behind it (turned with the entry), each flying the behavior
    // its design names (a gone or broken one: its faction's, with a banner) and following the flagship's lock.
    let ships = std::iter::once(inputs.design.blueprint()).chain(&launched.escorts);
    let mut flagship = None;
    let turn = Vec2::from_angle(entry.angle);
    for (k, bp) in ships.enumerate() {
        let fallback = behaviors::fallback(bp, &world.content, &inputs.behaviors);
        if let Some((_, why)) = &fallback {
            warn!("{why}");
            scene.banner = Some((why.to_uppercase(), 4.0));
        }
        let bp = fallback.as_ref().map_or(bp, |(bp, _)| bp);
        let Some(design) = world.design_from(bp, PLAYER) else { continue };
        let at = match k {
            0 => entry.pos,
            k => entry.pos + sv(turn.rotate(v2(wing_slot(k as u32 - 1)))),
        };
        let id = world.sim.spawn_ship(design.clone(), PLAYER, Body::at(at, entry.angle));
        let behavior = world.sim.behaviors().default_for(&design);
        world.sim.command(id, ShipCommand::Behavior(behavior));
        match flagship {
            None => {
                flagship = Some(id);
                scene.player_design = Some(design);
                world.sim.command(id, ShipCommand::Focus);
            }
            Some(lead) => world.sim.command(id, ShipCommand::Leader(Some(lead))),
        }
    }
    focus.0 = flagship;
    scene.last_focus = flagship;
    info!("voyage {id}: seed {}, mind {}, {} escorts", launched.seed, launched.mind, launched.escorts.len());
    world
}

/// "BACK TO THE DRYDOCK", or to the screen a behavior test was started from.
fn exit_banner(to: AppMode) -> &'static str {
    match to {
        AppMode::Drydock => "BACK TO THE DRYDOCK",
        _ => "BACK TO THE EDITOR",
    }
}

/// Where wingman `k` starts: alternately left and right of the player, a step further back every two.
fn wing_slot(k: u32) -> sim::Vec2 {
    let x = (k as f32 / 2.0).floor() + 1.0;
    let side = if k.is_multiple_of(2) { -1.0 } else { 1.0 };
    sim::Vec2::new(side * 22.0 * x, -12.0 * x)
}

/// TEST IN RANGE: `me` (the player's design, flying its own behavior) becomes the tested ship (watch: unpiloted)
/// or gets a wingman that is (alongside). A behavior that can't fly leaves the design's own, with a banner.
fn start_test(scene: &mut Scene, world: &mut SimWorld, runner: &Arc<ShipDesign>, me: ShipId, test: &BehaviorTest) {
    let tested = match test_range::resolve(world, &test.program, runner) {
        Ok(id) => Some(id),
        Err(why) => {
            warn!("test in range: {why}");
            scene.banner = Some((format!("{why}; flies its own").to_uppercase(), 4.0));
            world.sim.behaviors().default_for(runner)
        }
    };
    let ship = match test.mode {
        TestMode::Watch => me,
        TestMode::Alongside => {
            let wing = world.sim.spawn_ship(runner.clone(), PLAYER, Body::at(wing_slot(0), 0.0));
            world.sim.command(wing, ShipCommand::Leader(Some(me)));
            wing
        }
    };
    world.sim.command(ship, ShipCommand::Behavior(tested));
    scene.test_ship = Some(ship);
    let name = tested.and_then(|id| world.sim.behaviors().get(id)).map_or("no behavior", |b| b.name.as_str());
    info!("test in range: {:?}, {name}, ship {ship:?}", test.mode);
}

/// A flight another mode asked for: the sandbox with the player's design, raider waves on; a test flight (T or
/// TEST-FLY in the drydock, TEST IN RANGE) or home's TEST RANGE.
fn launch(
    mut scene: ResMut<Scene>,
    mut focus: ResMut<PilotFocus>,
    inputs: SceneInputs,
    mut rig: ResMut<CameraRig>,
    mut world: ResMut<SimWorld>,
) {
    let Some(launch) = scene.launch.take() else { return };
    if launch != Launch::Voyage {
        scene.kind = SceneKind::Sandbox;
    }
    scene.test_flight = launch == Launch::TestFlight;
    scene.waves = launch != Launch::Voyage;
    *world = build_world(&mut scene, &mut focus, &inputs, &mut rig);
    info!("{launch:?}: {} parts, revision {}", inputs.design.blueprint().parts.len(), inputs.design.revision());
}

/// Leaving flight (T, or a lost test flight): the world empties, views and effects go, the clock runs again.
#[allow(clippy::too_many_arguments)]
fn leave_flight(
    mut commands: Commands,
    mut scene: ResMut<Scene>,
    mut world: ResMut<SimWorld>,
    mut focus: ResMut<PilotFocus>,
    mut fx: ResMut<Effects>,
    mut shields: ResMut<ShieldBubbles>,
    mut input: ResMut<PlayerInput>,
    mut time: ResMut<Time<Virtual>>,
) {
    despawn_all_views(&mut commands, &mut world);
    fx.clear();
    shields.clear();
    input.lock = None;
    focus.0 = None;
    time.unpause();
    *scene = scene.restart();
    scene.test_flight = false;
    scene.test = None;
    if scene.voyage().is_some() {
        scene.kind = SceneKind::Sandbox;
    }
    *world = empty_world(&scene);
}

/// C in the sandbox: raider waves off (the raiders leave at once, no new wave) or on again (a wave soon).
fn toggle_waves(keys: Res<ButtonInput<KeyCode>>, mut scene: ResMut<Scene>, mut world: ResMut<SimWorld>) {
    if scene.kind != SceneKind::Sandbox || !keys.just_pressed(KeyCode::KeyC) {
        return;
    }
    scene.waves = !scene.waves;
    let now = world.sim.time_seconds();
    if scene.waves {
        scene.next_wave_tick = Some(world.sim.tick() + 60);
        scene.banner = Some(("RAIDER WAVES ON".into(), now + 2.5));
    } else {
        // Every raider vessel goes (their wrecks stay salvage); the views follow the sim's `ShipRemoved` events.
        let raiders: Vec<ShipId> = world.sim.vessels(ENEMY).map(|v| v.ship).collect();
        for id in raiders {
            world.sim.remove_ship(id);
        }
        scene.wave_ships.clear();
        scene.next_wave_tick = None;
        scene.banner = Some(("RAIDER WAVES OFF".into(), now + 2.5));
    }
}

fn scene_tick(
    mut scene: ResMut<Scene>,
    mut world: ResMut<SimWorld>,
    tuning: Res<Tuning>,
    mut player_input: ResMut<PlayerInput>,
    mut focus: ResMut<PilotFocus>,
    exit: Res<FlightExit>,
    mut next_mode: ResMut<NextState<AppMode>>,
) {
    // The keys and fire buttons for this tick (held now or since the last tick); the latch starts over every
    // tick, whoever flies.
    let (axes, fire) = player_input.for_tick();
    if !matches!(scene.kind, SceneKind::Sandbox | SceneKind::Voyage(_)) {
        return;
    }
    let world = &mut *world;
    let scene = &mut *scene;
    let tick = world.sim.tick();
    let now = world.sim.time_seconds();

    // Watch: nobody pilots. Focus switches (V, clicks, the fleet bar) are dropped; losing the tested ship ends
    // the test like losing every vessel ends a test flight.
    if let Some(watched) = scene.watched() {
        world.inputs.commands.retain(|(_, c)| !matches!(c, ShipCommand::Focus));
        if scene.respawn_tick.is_none() && !world.sim.ship(watched).is_some_and(|s| s.is_vessel()) {
            scene.respawn_tick = Some(tick + 90);
            scene.banner = Some((format!("SHIP LOST  ·  {}", exit_banner(exit.0)), now + 3.0));
        }
    }

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
                let turn = if lock.is_some() && !input.stick_mode { axes.x } else { 0.0 };
                input.nudge = nudge_step(input.nudge, turn);
                // Without a lock A/D rotate with a ramp (GDR-0055): a tap nudges the nose, a hold turns faster up
                // to the key turn rate (one step per tick).
                let key = if lock.is_none() && !input.stick_mode { axes.x } else { 0.0 };
                input.turn_ramp = turn_ramp_step(input.turn_ramp, key);
                let keys = (!input.stick_mode).then_some(PilotAxes { turn: input.turn_ramp, thrust: axes.y, strafe: axes.z });
                let pilot = PilotInput {
                    stick: sv(input.stick),
                    lock,
                    drift: input.drift,
                    fire,
                    axes: keys,
                    aim_offset: input.nudge,
                    ..default()
                };
                world.inputs.commands.push((me, ShipCommand::Pilot(pilot)));
            }
            _ => {}
        }
    }
    // A voyage has no re-form, waves or clean-up; losing every vessel ends it (`voyage.rs`).
    if scene.voyage().is_some() {
        return;
    }
    if focus.0.is_none() && scene.respawn_tick.is_none() && scene.last_focus.is_some() {
        // No vessel left: re-form in a moment, or back to the drydock (or the behavior test's screen) after a
        // test flight.
        scene.respawn_tick = Some(tick + 90);
        let banner = if scene.test_flight { format!("SHIP LOST  ·  {}", exit_banner(exit.0)) } else { "SHIP LOST".into() };
        scene.banner = Some((banner, now + 3.0));
    }
    if let Some(t) = scene.respawn_tick
        && tick >= t
    {
        scene.respawn_tick = None;
        if scene.test_flight {
            next_mode.set(exit.0);
            return;
        }
        let wreck = scene.last_focus.take().filter(|id| world.sim.ship(*id).is_some_and(|s| !s.is_vessel()));
        let at = wreck.and_then(|me| world.sim.ship(me)).and_then(|s| s.main_piece().and_then(|p| s.piece(p))).map(|p| p.com_world()).unwrap_or(sim::Vec2::ZERO);
        if let Some(me) = wreck {
            world.sim.remove_ship(me);
        }
        if let Some(runner) = scene.player_design.clone() {
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
    if scene.waves && alive == 0 && scene.next_wave_tick.is_none() && !scene.wave_ships.is_empty() {
        scene.next_wave_tick = Some(tick + 120);
        scene.banner = Some((format!("WAVE {} CLEARED", scene.wave), now + 3.0));
    }
    if let Some(t) = scene.next_wave_tick
        && tick >= t
        && scene.waves
    {
        scene.next_wave_tick = None;
        scene.wave += 1;
        scene.wave_ships.clear();
        let centre = focus
            .0
            .or(scene.watched())
            .and_then(|me| world.sim.ship(me))
            .and_then(|s| s.control_piece().and_then(|p| s.piece(p)))
            .map(|p| v2(p.com_world()))
            .unwrap_or(Vec2::ZERO);
        let raider = raider_design(world);
        let flown = scene.player_design.as_deref().map_or(0.0, build_points);
        let n = tuning.raiders_per_wave.unwrap_or_else(|| wave_size(flown, raider.as_deref().map_or(0.0, build_points))).max(1);
        if let Some(raider) = raider {
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
            .or(scene.watched())
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

/// Rebuilds the whole scene (tuning changes that need fresh designs, or the reset key). A test flight stays a
/// test flight; the waves keep their setting.
#[allow(clippy::too_many_arguments)]
fn reset(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut request: ResMut<ResetRequest>,
    mut scene: ResMut<Scene>,
    mut world: ResMut<SimWorld>,
    inputs: SceneInputs,
    mut rig: ResMut<CameraRig>,
    mut fx: ResMut<Effects>,
    mut shields: ResMut<ShieldBubbles>,
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
    if scene.voyage().is_some() {
        // A voyage can't start over (its launch is on record): RECALL or lose it.
        let now = world.sim.time_seconds();
        scene.say("NO RESET IN A VOYAGE".into(), "RECALL HOME INSTEAD".into(), Tone::Coral, now + 2.0);
        return;
    }
    despawn_all_views(&mut commands, &mut world);
    fx.clear();
    shields.clear();
    input.lock = None;
    *world = build_world(&mut scene, &mut focus, &inputs, &mut rig);
    info!("scene reset ({:?})", scene.kind);
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;

    use super::*;
    use crate::test_range::{StartTest, TestProgram, TestRangePlugin};

    /// GDR-0074: a wave follows the flown design's build points against a raider's, at least 1, at most the cap.
    #[test]
    fn waves_follow_the_flown_design() {
        assert_eq!(wave_size(59.0, 15.8), 2);
        assert_eq!(wave_size(132.5, 15.8), 4);
        assert_eq!(wave_size(1.0, 15.8), 1, "at least one raider");
        assert_eq!(wave_size(0.0, 15.8), 1);
        assert_eq!(wave_size(50.0, 0.0), 1, "no raider design to measure");
        assert_eq!(wave_size(10_000.0, 15.8), WAVE_CAP);
        // The content's designs, compiled: the starter meets 2 raiders, the Halvex Runner 4.
        let c = data::base_content();
        let bp = |id: &str| build_points(&ShipDesign::compile(&c, c.ships.get(id).unwrap()).unwrap());
        let raider = bp("automaton_raider");
        assert_eq!((wave_size(bp("rim_skiff"), raider), wave_size(bp("halvex_runner"), raider)), (2, 4));
    }

    /// The behavior screen's request: from another mode it launches a test flight; in flight it rebuilds the
    /// scene; the test outlives restarts and ends with the flight.
    #[test]
    fn a_test_request_launches_or_rebuilds_the_flight() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_state(AppMode::Drydock)
            .insert_resource(Scene::new(SceneKind::Sandbox, 1))
            .insert_resource(SimWorld::new(sim::Sim::new(1), data::Content::default()))
            .init_resource::<FlightExit>()
            .init_resource::<ResetRequest>()
            .add_plugins(TestRangePlugin);
        let test = BehaviorTest { mode: TestMode::Watch, program: TestProgram::Id("careful".into()) };
        app.world_mut().write_message(StartTest { test: test.clone(), back: AppMode::Drydock });
        app.update();
        let scene = app.world().resource::<Scene>();
        assert_eq!((scene.test_mode(), scene.launch, scene.test_flight, scene.waves), (Some(TestMode::Watch), Some(Launch::TestFlight), true, true));
        app.update();
        assert_eq!(*app.world().resource::<State<AppMode>>().get(), AppMode::Flight);
        // In flight, a new request rebuilds the scene instead.
        let alongside = BehaviorTest { mode: TestMode::Alongside, ..test };
        app.world_mut().write_message(StartTest { test: alongside, back: AppMode::Drydock });
        app.update();
        assert!(app.world().resource::<ResetRequest>().0);
        let scene = app.world().resource::<Scene>();
        assert_eq!(scene.test_mode(), Some(TestMode::Alongside));
        assert_eq!(scene.restart().test_mode(), Some(TestMode::Alongside));
    }
}
