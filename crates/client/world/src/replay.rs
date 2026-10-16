//! The fight viewer (GDR-0092 step 1): `?scene=fight&fight=<code>` re-simulates a fight from its code
//! (`arena::FightCode`, printed by `arena fight` and `arena evolve`) in the flight view, as a spectator.
//!
//! * **The world** is built by the arena (`arena::FightSetup`), never by the scene's own spawning, from the packs'
//!   content only (no tuning scales, no player behaviors): the same world the CLI fought in. The scene steps it
//!   with no inputs (nobody pilots: whatever the controls and the fleet bar queue is dropped) and asks the fight's
//!   `arena::Referee` after every tick, as `arena::Fight` does, so the replay ends where the record does.
//! * **Who is who**: side 0 is drawn in the player's teal and side 1 in coral, whatever their designs' faction
//!   (`SimWorld::looks`): an automaton against an automaton shows which is which.
//! * **The camera** frames both sides; a click on a ship follows it (the HUD then shows its hull, speed and what its
//!   behavior targets), a click on empty space frames again; F frees the camera, drag pans, the wheel zooms.
//! * **Playback** (the `replay` actions, `controls.ron`): pause and play (Space; at the end it plays again from the
//!   start), slower and faster (`,` `.`: [`SPEEDS`], through `Time<Virtual>`'s speed), one second back and forward
//!   (← →), from the start (R). The first pass keeps a copy of the world and the referee every [`KEYFRAME_TICKS`]
//!   (both are `Clone`); going back restores the keyframe before the target and runs on to it, going forward runs
//!   ahead, in one frame either way (a second is 30 ticks).
//! * **Checked**: when the code's rules fingerprint differs from today's (`arena::rules_fingerprint`) the banner says
//!   so at once; the fight still plays under today's rules. At the end the result is compared with the record:
//!   `AS RECORDED`, or `THE RULES HAVE CHANGED SINCE THIS FIGHT` with how it ends now and how it ended then.
//! * **The HUD**: the title names both sides and the seed, the subtitle the time, the speed, each side's build
//!   points left and who flew side 0's first ship when a stand-in pilot did (`FLOWN BY STAND-IN · CASUAL`, GDR-0092
//!   step 6: the referee flies it, so the replay is the fight); a timeline under the view shows how far the fight is, how far it has been simulated, and a mark
//!   for every vessel lost (teal: side 0's, coral: side 1's).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use arena::{FightCode, FightResult, FightSetup, OWNERS, Outcome, Referee};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::egui_wants_any_pointer_input;
use data::input::Action;
use kit::params::{Params, RenderToggles};
use kit::style::{Style, ui_color};
use kit::ui_kit::Tone;
use sim::{Sim, SimEvent, Target, TickInputs};

use crate::camera::{CamMode, CameraRig};
use crate::controls::pick;
use crate::flight_hud::{FlightMenu, HudZone};
use crate::fx::Effects;
use crate::mode::{AppMode, FlightUi};
use crate::scene::{Scene, SceneKind};
use crate::settings::Act;
use crate::shield::ShieldBubbles;
use crate::ship_model::Look;
use crate::sim_bridge::{SimSet, SimWorld};
use crate::voyage::clock;

/// The playback speeds (× real time); the replay starts at 1×.
pub const SPEEDS: [f32; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
const NORMAL_SPEED: usize = 2;
/// A keyframe every 10 s of the fight.
pub const KEYFRAME_TICKS: u64 = 10 * sim::TICK_HZ as u64;
/// One step back or forward: a second.
const SEEK_TICKS: u64 = sim::TICK_HZ as u64;
/// The changed-rules banner shows for the first seconds of the fight (and the end check from the end on).
const NOTICE_TICKS: u64 = 6 * sim::TICK_HZ as u64;
/// Marks on the timeline at most (a fight of many small ships loses more vessels than the bar has room for).
const MARKS: usize = 24;
/// Following a clicked ship zooms in to at least this (logical px per metre; flight's default).
const FOLLOW_ZOOM: f32 = 18.0;
/// A click moves the pointer less than this (logical px); more is a drag that pans.
const CLICK_SLOP: f32 = 6.0;

/// The fight being watched: the code from the URL, and once read the fight or why it doesn't play.
#[derive(Resource, Default)]
pub struct Replay {
    /// `?fight=<code>`.
    code: Option<String>,
    pub watch: Option<Result<Watched, String>>,
}

/// A fight that plays.
pub struct Watched {
    pub code: FightCode,
    /// The code's rules fingerprint differs from today's.
    pub stale: bool,
    /// Each side as the banner names it (`3 × AUTOMATON RAIDER`).
    pub names: [String; 2],
    /// The stand-in pilot that flies side 0's first ship, by its `lab.ron` name.
    pub pilot: Option<String>,
    referee: Referee,
    /// A copy of the world and the referee every [`KEYFRAME_TICKS`] seen so far, by tick (tick 0: the start).
    keyframes: BTreeMap<u64, (Sim, Referee)>,
    /// The furthest tick simulated.
    pub seen: u64,
    /// Vessels lost so far (the tick and the side), for the timeline.
    pub marks: Vec<(u64, usize)>,
    /// The playback speed, an index into [`SPEEDS`].
    pub speed: usize,
}

impl Watched {
    /// The result now: `Some` once the fight has ended.
    pub fn result(&self) -> Option<FightResult> {
        self.referee.result()
    }

    /// The fight's time limit in ticks.
    pub fn limit(&self) -> u64 {
        self.referee.limit()
    }

    /// The HUD's title: `REPLAY · HALVEX RUNNER VS 3 × AUTOMATON RAIDER · SEED 3` (a gauntlet's: `… · 3 WAVES · …`).
    pub fn title(&self) -> String {
        let waves = match self.code.spec.waves.len() {
            0 => String::new(),
            n => format!("  ·  {} WAVES", n + 1),
        };
        format!("REPLAY  ·  {}  VS  {}{waves}  ·  SEED {}", self.names[0], self.names[1], self.code.spec.seed)
    }

    /// The HUD's subtitle: the time, the speed and each side's build points left of its start.
    pub fn subtitle(&self, sim: &Sim, out: &mut String) {
        let bp = self.referee.build_points();
        let left = [0, 1].map(|side| arena::standing(sim, side).0);
        let speed = SPEEDS[self.speed];
        let _ = write!(out, "{} / {}  ·  {speed}×", clock(sim.tick()), clock(self.limit()));
        let _ = write!(out, "  ·  BUILD POINTS {:.0} / {:.0}  VS  {:.0} / {:.0}", left[0], bp[0], left[1], bp[1]);
        if !self.code.spec.waves.is_empty() {
            let (wave, of) = self.referee.wave();
            let _ = write!(out, "  ·  WAVE {} / {of}", wave.min(of));
        }
        if let Some(p) = &self.pilot {
            let _ = write!(out, "  ·  FLOWN BY STAND-IN · {}", p.to_uppercase());
        }
    }

    /// The banner the viewer shows at `tick`, if any: the end check from the end on, the changed rules at the start.
    pub fn notice(&self, tick: u64) -> Option<(String, String, Tone)> {
        if let Some(now) = self.result() {
            let then = self.code.record;
            return Some(if now == then {
                ("AS RECORDED".into(), format!("{}  ·  HASH {:016x}", ending(&now), now.state_hash), Tone::Teal)
            } else {
                (
                    "THE RULES HAVE CHANGED SINCE THIS FIGHT".into(),
                    format!("NOW {}  (RECORDED: {})", ending(&now), ending(&then)),
                    Tone::Coral,
                )
            });
        }
        (self.stale && tick < NOTICE_TICKS).then(|| {
            let sub = "IT PLAYS UNDER TODAY'S RULES  ·  THE END SAYS WHETHER IT STILL ENDS AS RECORDED";
            ("THE RULES HAVE CHANGED SINCE THIS FIGHT".into(), sub.into(), Tone::Coral)
        })
    }

    /// After a tick of the world: the referee's look, a keyframe every [`KEYFRAME_TICKS`] and the vessels lost the
    /// first time the tick is seen. `true` once the fight has ended.
    fn after_tick(&mut self, sim: &mut Sim) -> bool {
        let ended = self.referee.observe(sim).is_some();
        let tick = sim.tick();
        if tick > self.seen {
            self.seen = tick;
            if tick.is_multiple_of(KEYFRAME_TICKS) {
                self.keyframes.insert(tick, (sim.clone(), self.referee.clone()));
            }
            for e in sim.events() {
                if let SimEvent::VesselLost { owner, .. } = e
                    && let Some(side) = OWNERS.iter().position(|o| o == owner)
                    && self.marks.len() < MARKS
                {
                    self.marks.push((tick, side));
                }
            }
        }
        ended
    }

    /// Puts the world at `target` (at most the fight's end): from the keyframe before it when that is behind the
    /// world, else on from where it is. `true` once the fight has ended.
    fn seek(&mut self, world: &mut SimWorld, target: u64) -> bool {
        if target < world.sim.tick()
            && let Some((_, (sim, referee))) = self.keyframes.range(..=target).next_back()
        {
            world.sim = sim.clone();
            self.referee = referee.clone();
        }
        let idle = TickInputs::default();
        let mut ended = self.result().is_some();
        while world.sim.tick() < target && !ended {
            world.sim.step(&idle);
            ended = self.after_tick(&mut world.sim);
        }
        world.hold = ended;
        world.rebuild_views = true;
        ended
    }
}

/// How a fight ends, for side 0: `WON AT 0:23`, `LOST AT 1:02`, `BOTH OUT AT 0:40`, `DRAW AT 1:30 · 0.62`.
fn ending(r: &FightResult) -> String {
    let at = clock(r.ticks);
    match r.outcome {
        Outcome::Won(0) => format!("WON AT {at}"),
        Outcome::Won(_) => format!("LOST AT {at}"),
        Outcome::BothOut => format!("BOTH OUT AT {at}"),
        Outcome::TimeLimit => format!("DRAW AT {at}  ·  {:.2}", r.sides[0].score),
    }
}

/// A side as the banner names it: its entries' names, `3 × ` before a count, joined by `+`.
fn side_name(designs: &[(data::Blueprint, std::sync::Arc<sim::ShipDesign>)], spec: &arena::Side) -> String {
    let names: Vec<String> = designs
        .iter()
        .zip(&spec.entries)
        .map(|((bp, _), e)| match e.count {
            1 => bp.name.to_uppercase(),
            n => format!("{n} × {}", bp.name.to_uppercase()),
        })
        .collect();
    names.join(" + ")
}

impl Replay {
    /// Plays `code` when a fight scene starts next (the evolution lab's WATCH, `scene::Launch::Fight`): read afresh,
    /// at the normal speed.
    pub fn queue(&mut self, code: String) {
        self.code = Some(code);
        self.watch = None;
    }

    /// The fight's world (the scene builds it, `scene::build_world`): read the code the first time, then build the
    /// fight afresh. The camera frames it. A code that doesn't read or build leaves an empty world and says why.
    pub fn start(&mut self, content: &data::Content, rig: &mut CameraRig) -> SimWorld {
        rig.mode = CamMode::Frame;
        rig.auto_mode = CamMode::Frame;
        rig.user_zoom = false;
        rig.pos = Vec2::new(0.0, 50.0);
        let empty = || SimWorld::new(Sim::new(0), data::Content::default());
        let code = match &self.watch {
            Some(Ok(w)) => Ok(w.code.clone()),
            Some(Err(why)) => Err(why.clone()),
            None => match &self.code {
                Some(text) => FightCode::decode(text).map_err(|e| e.to_string()),
                None => Err("no fight code: open ?scene=fight&fight=<code>".into()),
            },
        };
        let built = code.and_then(|code| {
            let setup = FightSetup::build(content, &code.spec).map_err(|e| format!("the fight doesn't build here: {e}"))?;
            Ok((code, setup))
        });
        let (code, setup) = match built {
            Ok(b) => b,
            Err(why) => {
                warn!("fight viewer: {why}");
                self.watch = Some(Err(why));
                return empty();
            }
        };
        // Today's rules are computed once per viewer (about 5 ms natively).
        let stale = match &self.watch {
            Some(Ok(w)) => w.stale,
            _ => code.rules != arena::rules_fingerprint(content),
        };
        let names = [0, 1].map(|side| side_name(&setup.designs[side], &code.spec.sides[side]));
        let mut world = SimWorld::new(setup.sim, content.clone());
        world.looks.insert(OWNERS[0], Look::Player);
        world.looks.insert(OWNERS[1], Look::Enemy);
        for (bp, design) in setup.designs.into_iter().flatten() {
            world.adopt(&design, bp);
        }
        let keyframes = BTreeMap::from([(0, (world.sim.clone(), setup.referee.clone()))]);
        info!("fight viewer: seed {}, {} vs {}, rules {}", code.spec.seed, names[0], names[1], if stale { "changed" } else { "as recorded" });
        let speed = match &self.watch {
            Some(Ok(w)) => w.speed,
            _ => NORMAL_SPEED,
        };
        let pilot = code.spec.sides[0].entries.iter().find_map(|e| e.pilot.as_ref()).map(|p| content.lab.pilot(&p.profile).map_or_else(|| p.profile.clone(), |l| l.name.clone()));
        self.watch = Some(Ok(Watched { code, stale, names, pilot, referee: setup.referee, keyframes, seen: 0, marks: Vec::new(), speed }));
        world
    }

    /// The fight that plays, if one does.
    pub fn watched(&self) -> Option<&Watched> {
        self.watch.as_ref().and_then(|w| w.as_ref().ok())
    }

    /// Why the code doesn't play, if it doesn't.
    pub fn problem(&self) -> Option<&str> {
        self.watch.as_ref().and_then(|w| w.as_ref().err()).map(String::as_str)
    }
}

/// The timeline's bar, its fills and its marks.
#[derive(Component)]
struct Timeline;
/// How far the fight has been simulated, and how far it is.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Fill {
    Seen,
    Now,
}
#[derive(Component)]
struct Mark(usize);

pub struct ReplayPlugin;

impl Plugin for ReplayPlugin {
    fn build(&self, app: &mut App) {
        let params = app.world().resource::<Params>();
        let code = (params.text("scene") == Some("fight")).then(|| params.text("fight").unwrap_or_default().to_string());
        app.insert_resource(Replay { code, watch: None })
            .add_systems(Startup, spawn_timeline)
            .add_systems(FixedUpdate, (drop_inputs.in_set(SimSet::Inputs), observe.in_set(SimSet::Events)))
            .add_systems(
                Update,
                (playback, follow_click.run_if(not(egui_wants_any_pointer_input)), update_timeline)
                    .chain()
                    .run_if(in_state(AppMode::Flight)),
            )
            .add_systems(OnExit(AppMode::Flight), |mut time: ResMut<Time<Virtual>>| time.set_relative_speed(1.0));
    }
}

/// Is this flight a replayed fight?
fn fight_scene(scene: &Scene) -> bool {
    scene.kind == SceneKind::Fight
}

/// Nobody pilots a replay: whatever the controls and the fleet bar queued for the tick goes.
fn drop_inputs(scene: Res<Scene>, mut world: ResMut<SimWorld>) {
    if fight_scene(&scene) {
        world.inputs = TickInputs::default();
    }
}

/// After every tick: the referee looks, keyframes and marks are kept; at the end the world holds.
fn observe(scene: Res<Scene>, mut replay: ResMut<Replay>, mut world: ResMut<SimWorld>) {
    if !fight_scene(&scene) || world.hold {
        return;
    }
    let Some(Ok(w)) = replay.watch.as_mut() else { return };
    if w.after_tick(&mut world.sim) {
        world.hold = true;
    }
}

/// The replay keys.
#[allow(clippy::too_many_arguments)]
fn playback(
    act: Act,
    scene: Res<Scene>,
    mut replay: ResMut<Replay>,
    mut world: ResMut<SimWorld>,
    mut time: ResMut<Time<Virtual>>,
    mut fx: ResMut<Effects>,
    mut shields: ResMut<ShieldBubbles>,
    menu: Res<FlightMenu>,
) {
    if !fight_scene(&scene) {
        return;
    }
    let Some(Ok(w)) = replay.watch.as_mut() else { return };
    let world = &mut *world;
    if act.just_pressed(Action::ReplayFaster) {
        w.speed = (w.speed + 1).min(SPEEDS.len() - 1);
    }
    if act.just_pressed(Action::ReplaySlower) {
        w.speed = w.speed.saturating_sub(1);
    }
    if time.relative_speed() != SPEEDS[w.speed] {
        time.set_relative_speed(SPEEDS[w.speed]);
    }
    let now = world.sim.tick();
    let seek = if act.just_pressed(Action::ReplayRestart) || (act.just_pressed(Action::ReplayPause) && w.result().is_some()) {
        time.unpause();
        Some(0)
    } else if act.just_pressed(Action::ReplayBack) {
        Some(now.saturating_sub(SEEK_TICKS))
    } else if act.just_pressed(Action::ReplayForward) {
        Some(now + SEEK_TICKS)
    } else {
        if act.just_pressed(Action::ReplayPause) && !menu.open {
            if time.is_paused() { time.unpause() } else { time.pause() }
        }
        None
    };
    if let Some(target) = seek {
        w.seek(world, target);
        fx.clear();
        shields.clear();
    }
    // At its end the replay stands still (paused, so the drawn poses stop between the last two ticks too).
    if world.hold && !time.is_paused() {
        time.pause();
    }
}

/// A click (not a drag, which pans) on a ship follows it; on empty space frames the fight again.
fn follow_click(
    mut scene: ResMut<Scene>,
    world: Res<SimWorld>,
    mut rig: ResMut<CameraRig>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    fixed: Res<Time<Fixed>>,
    mut press: Local<Option<Vec2>>,
) {
    if !fight_scene(&scene) {
        return;
    }
    let cursor = window.cursor_position();
    if buttons.just_pressed(MouseButton::Left) {
        *press = cursor;
    }
    if buttons.just_released(MouseButton::Left)
        && let (Some(at), Some(c)) = (press.take(), cursor)
        && at.distance(c) < CLICK_SLOP
    {
        let alpha = fixed.overstep_fraction();
        match pick(&world, None, rig.screen_to_world(c), 1.0 / rig.zoom, alpha) {
            Some(Target::Ship(id) | Target::Piece(id, _)) => {
                scene.spectate = Some(id);
                rig.mode = CamMode::Follow;
                // Close enough to see the ship (framing a whole fight zooms far out).
                rig.zoom_target = rig.zoom_target.max(FOLLOW_ZOOM);
            }
            _ => {
                scene.spectate = None;
                rig.mode = CamMode::Frame;
                rig.user_zoom = false;
            }
        }
    }
}

/// The timeline over the help line: a thin bar, how far the fight has been simulated (dim) and how far it is
/// (teal), and the vessels lost.
fn spawn_timeline(mut commands: Commands, style: Res<Style>) {
    let ui = &style.ui;
    commands
        .spawn((
            Timeline,
            FlightUi,
            HudZone,
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(52),
                left: percent(25),
                width: percent(50),
                height: px(4),
                ..default()
            },
            BackgroundColor(ui_color(ui.dim, 0.35)),
        ))
        .with_children(|bar| {
            let fill = |w: f32| Node { position_type: PositionType::Absolute, left: px(0), top: px(0), height: percent(100), width: percent(w), ..default() };
            bar.spawn((Fill::Seen, fill(0.0), BackgroundColor(ui_color(ui.label, 0.45))));
            bar.spawn((Fill::Now, fill(0.0), BackgroundColor(ui_color(ui.teal, 0.95))));
            for k in 0..MARKS {
                bar.spawn((
                    Mark(k),
                    Visibility::Hidden,
                    Node { position_type: PositionType::Absolute, top: px(-4), width: px(2), height: px(12), ..default() },
                    BackgroundColor(Color::NONE),
                ));
            }
        });
}

/// Places the timeline's fills and marks (hidden outside a replay and with the HUD off).
#[allow(clippy::too_many_arguments)]
fn update_timeline(
    scene: Res<Scene>,
    replay: Res<Replay>,
    world: Res<SimWorld>,
    toggles: Res<RenderToggles>,
    style: Res<Style>,
    mut bar: Query<&mut Visibility, (With<Timeline>, Without<Mark>)>,
    mut fills: Query<(&Fill, &mut Node), Without<Mark>>,
    mut marks: Query<(&Mark, &mut Node, &mut BackgroundColor, &mut Visibility), Without<Timeline>>,
) {
    let shown = fight_scene(&scene) && toggles.hud && replay.watched().is_some();
    for mut v in &mut bar {
        v.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }
    let Some(w) = replay.watched().filter(|_| shown) else { return };
    let share = |tick: u64| (tick as f32 / w.limit().max(1) as f32 * 100.0).min(100.0);
    let set = |node: &mut Node, pct: f32| {
        if node.width != percent(pct) {
            node.width = percent(pct);
        }
    };
    for (fill, mut n) in &mut fills {
        set(&mut n, share(if *fill == Fill::Seen { w.seen } else { world.sim.tick() }));
    }
    let side_color = [style.ui.teal, style.ui.coral];
    for (m, mut node, mut color, mut vis) in &mut marks {
        match w.marks.get(m.0) {
            Some(&(tick, side)) => {
                if node.left != percent(share(tick)) {
                    node.left = percent(share(tick));
                }
                color.0 = ui_color(side_color[side], 0.9);
                vis.set_if_neq(Visibility::Inherited);
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use arena::{Entry, FightSpec, run_fight, rules_fingerprint};

    use super::*;

    fn code(content: &data::Content) -> FightCode {
        let spec = FightSpec::new(3, vec![Entry::ship("halvex_runner")], vec![Entry { count: 3, ..Entry::ship("automaton_raider") }]);
        let record = run_fight(content, &spec).unwrap();
        FightCode::new(content, rules_fingerprint(content), spec, record)
    }

    fn replay(code: String) -> Replay {
        Replay { code: Some(code), watch: None }
    }

    /// GDR-0092: the viewer's world, stepped as the scene steps it and seeked back and forth through its keyframes,
    /// ends exactly as the CLI recorded the fight; side 0 is drawn teal and side 1 coral.
    #[test]
    fn a_replay_ends_as_recorded_and_seeks_exactly() {
        let content = data::base_content();
        let code = code(&content);
        let mut r = replay(code.encode());
        let mut world = r.start(&content, &mut CameraRig::default());
        assert_eq!((world.looks.get(&OWNERS[0]), world.looks.get(&OWNERS[1])), (Some(&Look::Player), Some(&Look::Enemy)));
        let w = r.watch.as_mut().unwrap().as_mut().unwrap();
        assert!(!w.stale);
        assert_eq!(w.names, ["HALVEX RUNNER".to_string(), "3 × AUTOMATON RAIDER".to_string()]);
        assert_eq!(w.notice(0), None);
        let idle = TickInputs::default();
        for _ in 0..400 {
            world.sim.step(&idle);
            w.after_tick(&mut world.sim);
        }
        let at = world.sim.state_hash();
        assert_eq!(w.keyframes.keys().copied().collect::<Vec<_>>(), [0, KEYFRAME_TICKS]);
        w.seek(&mut world, 370);
        assert_eq!(world.sim.tick(), 370);
        assert!(world.rebuild_views);
        w.seek(&mut world, 400);
        assert_eq!(world.sim.state_hash(), at, "back through a keyframe and on again: the same world");
        assert!(w.seek(&mut world, u64::MAX));
        assert!(world.hold);
        assert_eq!(w.result(), Some(code.record));
        assert_eq!(world.sim.tick(), code.record.ticks);
        assert_eq!(w.notice(world.sim.tick()).unwrap().0, "AS RECORDED");
        assert!(w.marks.iter().all(|&(t, side)| t <= code.record.ticks && side < 2) && !w.marks.is_empty());
        // From the start again, and to the end the same way.
        w.seek(&mut world, 0);
        assert_eq!((world.sim.tick(), w.result(), world.hold), (0, None, false));
        assert!(w.seek(&mut world, u64::MAX));
        assert_eq!(w.result(), Some(code.record));
        // A restarted scene builds the fight afresh and keeps the speed.
        w.speed = 4;
        let again = r.start(&content, &mut CameraRig::default());
        assert_eq!(again.sim.tick(), 0);
        assert_eq!(r.watched().unwrap().speed, 4);
    }

    /// A code fought under other rules says so at once; a record today's rules don't reproduce says so at the end;
    /// a code that doesn't read says why and leaves the world empty.
    #[test]
    fn changed_rules_and_bad_codes_are_told() {
        let content = data::base_content();
        let mut old = code(&content);
        old.rules ^= 1;
        old.record.ticks += 1;
        let mut r = replay(old.encode());
        let mut world = r.start(&content, &mut CameraRig::default());
        let w = r.watch.as_mut().unwrap().as_mut().unwrap();
        assert!(w.stale);
        assert_eq!(w.notice(0).unwrap().0, "THE RULES HAVE CHANGED SINCE THIS FIGHT");
        assert_eq!(w.notice(NOTICE_TICKS), None, "only at the start while it plays");
        w.seek(&mut world, u64::MAX);
        let (title, sub, tone) = w.notice(world.sim.tick()).unwrap();
        assert_eq!((title.as_str(), tone), ("THE RULES HAVE CHANGED SINCE THIS FIGHT", Tone::Coral));
        assert!(sub.starts_with("NOW WON AT ") && sub.contains("(RECORDED: WON AT "), "{sub}");
        let mut bad = replay("SGF1:nonsense".into());
        let world = bad.start(&content, &mut CameraRig::default());
        assert!(world.sim.ships().is_empty() && bad.watched().is_none());
        assert_eq!(bad.problem(), Some("the code is damaged or incomplete"));
    }
}
