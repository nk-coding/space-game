//! The seam between the deterministic `sim` crate and the Bevy client.
//!
//! * [`SimWorld`] owns the `Sim` (the type alias [`SimT`]; Rapier2D runs inside the sim, no physics type reaches
//!   the client).
//! * `FixedUpdate` at [`sim::TICK_HZ`]: the scene fills [`SimWorld::inputs`] (`scene.rs`), [`step_sim`] steps once
//!   (timed; optional per-tick hash log), then [`apply_events`] turns the tick's [`sim::SimEvent`]s into view
//!   changes (spawn / rebuild / despawn piece views, keyed by `(ShipId, PieceId)`) and effects.
//! * Every frame, views read `piece.render_pose(alpha)` with `alpha = Time<Fixed>::overstep_fraction()`
//!   (`ship_view::sync_piece_views`). Nothing writes back into the sim except the inputs.
//! * Designs come from content ids ([`SimWorld::design`], cached per id: raiders) or from an in-memory blueprint
//!   ([`SimWorld::design_from`]: the player's design from the drydock, compiled fresh every time). Design
//!   visuals are cached per compiled design (its `Arc`), never per id, so an edited blueprint flown again under
//!   the same id never meets a stale visual.
//! * The FixedUpdate chain runs only in flight (`AppMode::Flight`); the drydock keeps an empty world.

use std::collections::BTreeMap;
use std::sync::Arc;

use bevy::platform::time::Instant;
use bevy::prelude::*;
use data::{Blueprint, Content};
use sim::{Owner, PieceId, ShipDesign, ShipId, SimEvent, Stat};
use kit::atlas::Atlas;
use kit::params::RenderToggles;
use kit::style::Style;

use crate::fx::Effects;
use crate::mode::AppMode;
use crate::shield::ShieldBubbles;
use crate::ship_model::{DesignVisual, design_visual};
use crate::ship_view::{PieceView, Z_FOCUS, draw_layer, spawn_piece_view, world_of};

/// The simulation type the client runs. Change the backend here once the physics engine is chosen.
pub type SimT = sim::Sim;

pub fn v2(v: sim::Vec2) -> Vec2 {
    Vec2::new(v.x, v.y)
}

pub fn sv(v: Vec2) -> sim::Vec2 {
    sim::Vec2::new(v.x, v.y)
}

/// Rolling window of timings (ms) with mean / median / p95.
#[derive(Clone, Debug)]
pub struct Timings {
    samples: Vec<f32>,
    next: usize,
}

impl Timings {
    pub fn new(window: usize) -> Self {
        Self { samples: Vec::with_capacity(window), next: 0 }
    }
    pub fn push(&mut self, ms: f32) {
        if self.samples.len() < self.samples.capacity() {
            self.samples.push(ms);
        } else {
            self.samples[self.next] = ms;
            self.next = (self.next + 1) % self.samples.len();
        }
    }
    /// `(mean, median, p95, max)`. Read-only, so readers of the timings (the HUD a few times a second) take
    /// `SimWorld` shared, not exclusively.
    pub fn stats(&self) -> (f32, f32, f32, f32) {
        if self.samples.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let n = sorted.len();
        let mean = sorted.iter().sum::<f32>() / n as f32;
        let p = |q: f32| sorted[((n as f32 - 1.0) * q).round() as usize];
        (mean, p(0.5), p(0.95), sorted[n - 1])
    }
}

/// Per-tick state hash log for cross-platform comparison (`scripts/determinism-check.sh --compare`).
pub struct HashLog {
    pub label: String,
    pub remaining: u64,
    pub total: u64,
    acc: sim::StateHasher,
    buf: String,
}

impl HashLog {
    /// A log of `ticks` ticks (at least one).
    pub fn new(label: String, ticks: u64) -> Self {
        let ticks = ticks.max(1);
        Self { label, remaining: ticks, total: ticks, acc: sim::StateHasher::new(), buf: String::with_capacity(4096) }
    }
}

/// The loaded content packs (the base pack today), parsed once at start. The drydock, the UI icons and new
/// scenes read it; [`SimWorld::content`] is the copy a running scene compiles from (tuning scales applied).
#[derive(Resource)]
pub struct GameContent(pub Content);

/// The vessel the player's input drives: a copy of `Sim::focus(PLAYER)`, refreshed every tick (by the scene). Camera, HUD and
/// controls all read this. Every other vessel of the player runs its behaviour.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PilotFocus(pub Option<ShipId>);

#[derive(Resource)]
pub struct SimWorld {
    pub sim: SimT,
    /// Content the player's designs are compiled from (weapon scales from the tuning panel applied); the HUD
    /// and the visuals read it too.
    pub content: Content,
    /// Content every other owner's designs are compiled from, when it differs (the tuning's damage scale is the
    /// player's only).
    others: Option<Content>,
    /// Compiled content designs by id: the player's, every other owner's.
    designs: [BTreeMap<String, Arc<ShipDesign>>; 2],
    /// The blueprints of designs compiled from memory ([`SimWorld::design_from`]), by design identity.
    blueprints: BTreeMap<usize, (Arc<ShipDesign>, Blueprint)>,
    /// Visuals by design identity (`Arc::as_ptr`; the stored `Arc` keeps the address from being reused).
    visuals: BTreeMap<usize, (Arc<ShipDesign>, Arc<DesignVisual>)>,
    /// Inputs for the next tick (filled by the scene, cleared after each step).
    pub inputs: sim::TickInputs,
    /// Live piece views.
    pub views: BTreeMap<(ShipId, PieceId), Entity>,
    pub step_ms: Timings,
    pub hashlog: Option<HashLog>,
}

impl SimWorld {
    pub fn new(sim: SimT, content: Content) -> Self {
        Self {
            sim,
            content,
            others: None,
            designs: Default::default(),
            blueprints: BTreeMap::new(),
            visuals: BTreeMap::new(),
            inputs: sim::TickInputs::default(),
            views: BTreeMap::new(),
            step_ms: Timings::new(900),
            hashlog: None,
        }
    }

    /// Content for every owner but the player (see [`SimWorld::others`]).
    pub fn with_others(mut self, others: Content) -> Self {
        self.others = Some(others);
        self
    }

    /// The compiled design of a blueprint for ships of `owner` (cached).
    pub fn design(&mut self, id: &str, owner: Owner) -> Option<Arc<ShipDesign>> {
        let theirs = owner != Owner::PLAYER && self.others.is_some();
        if let Some(d) = self.designs[theirs as usize].get(id) {
            return Some(d.clone());
        }
        let content = if theirs { self.others.as_ref()? } else { &self.content };
        let bp = content.ships.get(id)?;
        match ShipDesign::compile(content, bp) {
            Ok(d) => {
                let d = Arc::new(d);
                self.designs[theirs as usize].insert(id.to_string(), d.clone());
                Some(d)
            }
            Err(e) => {
                error!("cannot compile ship `{id}`: {e}");
                None
            }
        }
    }

    /// Compiles an in-memory blueprint (the player's design) for ships of `owner`. Not cached by id: every
    /// call compiles afresh, so an edited blueprint under the same id is never confused with an older one.
    pub fn design_from(&mut self, bp: &Blueprint, owner: Owner) -> Option<Arc<ShipDesign>> {
        let content = if owner != Owner::PLAYER && self.others.is_some() { self.others.as_ref()? } else { &self.content };
        match ShipDesign::compile(content, bp) {
            Ok(d) => {
                let d = Arc::new(d);
                self.blueprints.insert(Arc::as_ptr(&d) as usize, (d.clone(), bp.clone()));
                Some(d)
            }
            Err(e) => {
                error!("cannot compile ship `{}`: {e}", bp.id);
                None
            }
        }
    }

    /// The look of a compiled design (cached per design): from its in-memory blueprint, else from the content
    /// blueprint of its id (content designs, and the load test's, which the sim compiles itself).
    fn visual(&mut self, design: &Arc<ShipDesign>, style: &Style) -> Option<Arc<DesignVisual>> {
        let key = Arc::as_ptr(design) as usize;
        if let Some((_, v)) = self.visuals.get(&key) {
            return Some(v.clone());
        }
        let bp = match self.blueprints.get(&key) {
            Some((_, bp)) => bp,
            None => self.content.ships.get(&design.id)?,
        };
        match design_visual(&self.content, bp, style) {
            Ok(v) => {
                let v = Arc::new(v);
                self.visuals.insert(key, (design.clone(), v.clone()));
                Some(v)
            }
            Err(e) => {
                error!("no visual for `{}`: {e}", design.id);
                None
            }
        }
    }
}

pub struct SimBridgePlugin;

/// `FixedUpdate` order: scene inputs → step → events.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimSet {
    Inputs,
    Step,
    Events,
}

impl Plugin for SimBridgePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(sim::TICK_HZ as f64))
            .insert_resource(GameContent(data::base_content()))
            .configure_sets(
                FixedUpdate,
                (SimSet::Inputs, SimSet::Step, SimSet::Events).chain().run_if(in_state(AppMode::Flight)),
            )
            .add_systems(FixedUpdate, (step_sim.in_set(SimSet::Step), apply_events.in_set(SimSet::Events)))
            .add_systems(
                PostUpdate,
                crate::ship_view::sync_piece_views.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

fn step_sim(mut world: ResMut<SimWorld>) {
    let world = &mut *world;
    let t0 = Instant::now();
    world.sim.step(&world.inputs);
    let ms = t0.elapsed().as_secs_f64() as f32 * 1000.0;
    world.step_ms.push(ms);
    world.inputs.commands.clear();
    world.inputs.controls.clear();
    world.inputs.damage.clear();

    if let Some(log) = world.hashlog.as_mut() {
        use std::fmt::Write as _;
        let h = world.sim.state_hash();
        log.acc.u64(h);
        let _ = writeln!(log.buf, "{} {:016x}", world.sim.tick(), h);
        log.remaining -= 1;
        // One console call per 30 lines keeps the log fast.
        if log.remaining == 0 || world.sim.tick().is_multiple_of(30) {
            console_out(log.buf.trim_end());
            log.buf.clear();
        }
        if log.remaining == 0 {
            let fin = std::mem::replace(&mut log.acc, sim::StateHasher::new()).finish();
            console_out(&format!("# final={fin:016x} label={} ticks={}", log.label, log.total));
            console_out("# hashlog done");
            world.hashlog = None;
        }
    }
}

/// Plain console output (no log formatting), for machine-readable lines.
pub fn console_out(s: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&s.into());
    #[cfg(not(target_arch = "wasm32"))]
    println!("{s}");
}

/// Turns the tick's events into view changes and effects.
#[allow(clippy::too_many_arguments)]
fn apply_events(
    mut commands: Commands,
    mut world: ResMut<SimWorld>,
    mut meshes: ResMut<Assets<Mesh>>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    focus: Res<PilotFocus>,
    mut fx: ResMut<Effects>,
    mut shields: ResMut<ShieldBubbles>,
    views: Query<(&PieceView, &Transform)>,
) {
    let world = &mut *world;
    let n = world.sim.events().len();
    if n == 0 {
        return;
    }
    let player = focus.0;
    // Pieces a vessel stripped parts from this tick: when one is emptied, it goes without a burst.
    let mut stripped: Vec<(ShipId, PieceId)> = Vec::new();
    // Shots and plumes meet the shields before the physics step, at the tick's start pose (`prev_pos`); rams
    // after the read-back, at the end pose, each led by its `Impact` (`Sim::step`).
    let mut after_physics = false;
    for k in 0..n {
        // Copy what is needed out of the event (no clones of snapshots), then act with `world` borrowed mutably.
        let act = match &world.sim.events()[k] {
            SimEvent::ShipSpawned { ship } => Act::Spawned(*ship),
            SimEvent::PieceChanged { ship, piece } => Act::Rebuild(*ship, piece.id),
            SimEvent::Split(split) => {
                let mut ids = [PieceId(0); 8];
                let mut count = 0;
                for s in std::iter::once(&split.kept).chain(&split.spawned) {
                    if count < ids.len() {
                        ids[count] = s.id;
                        count += 1;
                    } else {
                        // more than 8 fragments in one split: rebuild the rest from the sim's piece list
                        ids[ids.len() - 1] = PieceId(u32::MAX);
                    }
                }
                Act::Split(split.ship, ids, count)
            }
            SimEvent::PieceDestroyed { ship, piece } => Act::PieceGone(*ship, *piece),
            // A piece with a logic core split off and became a vessel (ship) of its own: same piece id, new ship.
            SimEvent::VesselFormed { ship, from: Some((old, piece)), .. } => Act::Moved(*ship, *old, *piece),
            SimEvent::ShipRemoved { ship } => Act::ShipGone(*ship),
            SimEvent::PartDestroyed { ship, piece, part, volatile } => Act::PartGone(*ship, *piece, *part, *volatile),
            SimEvent::Explosion { ship, piece, part, at, radius, damage } => {
                Act::Blast(*ship, *piece, *part, v2(*at), *radius, *damage)
            }
            SimEvent::ShotFired { ship, kind, from, to, .. } => Act::Shot(*ship, *kind, v2(*from), v2(*to)),
            SimEvent::Hit { point, normal, family, damage, .. } => Act::Hit(v2(*point), v2(*normal), *family, *damage),
            SimEvent::Ricochet { point, vel, .. } => Act::Ricochet(v2(*point), v2(*vel)),
            SimEvent::PlumeHit { by, engine, point, .. } => Act::Burn(*by, *engine, v2(*point)),
            SimEvent::ShieldHit { ship, piece, part, point, absorbed, depleted, .. } => {
                Act::Shield(*ship, *piece, *part, v2(*point), *absorbed, *depleted)
            }
            SimEvent::Impact(_) => {
                after_physics = true;
                Act::None
            }
            SimEvent::Collected { ship, from, at, .. } => {
                if let sim::Salvaged::Part { ship, piece, .. } = *from {
                    stripped.push((ship, piece));
                }
                Act::Collected(*ship, v2(*at))
            }
            _ => Act::None,
        };
        match act {
            Act::None => {}
            Act::Spawned(ship) => {
                let count = world.sim.ship(ship).map(|s| s.pieces().len()).unwrap_or(0);
                for j in 0..count {
                    let Some(pid) = world.sim.ship(ship).map(|s| s.pieces()[j].id) else { break };
                    respawn_view(&mut commands, world, &mut meshes, &atlas, &style, &toggles, player, ship, pid);
                }
            }
            Act::Rebuild(ship, piece) => {
                respawn_view(&mut commands, world, &mut meshes, &atlas, &style, &toggles, player, ship, piece);
            }
            Act::Moved(ship, old, piece) => {
                if let Some(e) = world.views.remove(&(old, piece)) {
                    commands.entity(e).despawn();
                }
                if let Some(p) = world.sim.ship(ship).and_then(|s| s.piece(piece)) {
                    shields.moved(old, ship, &p.parts);
                }
                respawn_view(&mut commands, world, &mut meshes, &atlas, &style, &toggles, player, ship, piece);
            }
            Act::Split(ship, ids, count) => {
                for &pid in &ids[..count] {
                    if pid == PieceId(u32::MAX) {
                        // Overflow: make sure every live piece of the ship has a view.
                        let pieces = world.sim.ship(ship).map(|s| s.pieces().len()).unwrap_or(0);
                        for j in 0..pieces {
                            let Some(p) = world.sim.ship(ship).map(|s| s.pieces()[j].id) else { break };
                            if !world.views.contains_key(&(ship, p)) {
                                respawn_view(&mut commands, world, &mut meshes, &atlas, &style, &toggles, player, ship, p);
                            }
                        }
                    } else {
                        respawn_view(&mut commands, world, &mut meshes, &atlas, &style, &toggles, player, ship, pid);
                    }
                }
            }
            Act::PieceGone(ship, piece) => {
                if let Some(e) = world.views.remove(&(ship, piece)) {
                    if let Ok((v, tf)) = views.get(e)
                        && !stripped.contains(&(ship, piece))
                    {
                        let c = tf.transform_point(v.center.extend(0.0)).truncate();
                        fx.burst(c, v.glow, 1.4);
                    }
                    commands.entity(e).despawn();
                }
            }
            Act::ShipGone(ship) => {
                while let Some((&key, &e)) = world.views.range((ship, PieceId(0))..=(ship, PieceId(u32::MAX))).next() {
                    world.views.remove(&key);
                    commands.entity(e).despawn();
                }
            }
            // A part that explodes bursts as its blast (`SimEvent::Explosion`, next).
            Act::PartGone(_, _, _, true) => {}
            Act::PartGone(ship, piece, part, false) => {
                if let Some((at, glow)) = drawn_part(world, &views, ship, piece, part) {
                    fx.burst(at, glow, 0.7);
                }
            }
            Act::Blast(ship, piece, part, at, radius, damage) => {
                // Where the part is drawn (the sim's point is a tick ahead of the render pose), carried with its
                // piece.
                let at = drawn_part(world, &views, ship, piece, part).map_or(at, |(p, _)| p);
                let vel = world.sim.ship(ship).and_then(|s| s.piece(piece)).map_or(Vec2::ZERO, |p| v2(p.body.linvel));
                fx.blast(at, vel, radius, damage, style.system_color("power"), style.ui.coral);
            }
            Act::Shot(ship, kind, from, to) => {
                let color = weapon_color(world, ship, &style);
                fx.muzzle(from, color);
                if kind == sim::ShotKind::Pulse {
                    fx.pulse(from, to, color);
                }
            }
            Act::Hit(point, normal, family, damage) => {
                let color = match family {
                    data::WeaponFamily::Kinetic => Color::srgb(1.0, 0.78, 0.5),
                    _ => Color::srgb(0.75, 0.97, 1.0),
                };
                fx.hit(point, normal, color, damage);
            }
            Act::Ricochet(point, vel) => fx.ricochet(point, vel),
            Act::Collected(ship, at) => {
                // Salvage scooped into a hold: a short teal streak from where it was into the ship.
                let to = world.sim.ship(ship).and_then(|s| s.piece(s.control_piece()?)).map(|p| v2(p.com_world()));
                fx.flash(at, style.glow_player, 0.9, 0.25);
                if let Some(to) = to {
                    fx.pulse(at, to, style.glow_player);
                }
            }
            Act::Burn(ship, engine, point) => fx.burn(ship, engine, point),
            Act::Shield(ship, piece, part, point, absorbed, depleted) => {
                // The ripple starts where the shot entered: a direction from the emitter in the piece frame (the
                // bubble turns with the ship), from the pose the hit was resolved at (above), so it works while
                // the view is being rebuilt; its strength is the share of the capacity the shot took.
                if let Some(s) = world.sim.ship(ship)
                    && let Some(pc) = s.piece(piece)
                {
                    let [cx, cy] = s.design().parts[part as usize].shape.center();
                    let centre = Vec2::new(cx - pc.origin[0], pc.origin[1] - cy);
                    let (pos, angle) = if after_physics { (pc.body.pos, pc.body.angle) } else { (pc.prev_pos, pc.prev_angle) };
                    let local = Mat2::from_angle(-angle) * (point - v2(pos));
                    let dir = (local - centre).normalize_or(Vec2::Y);
                    let share = absorbed / s.stats(part).get(Stat::ShieldCapacity).max(1e-3);
                    shields.hit(ship, part, dir, share, depleted);
                }
            }
        }
    }
}

/// What an event asks the client to do (copied out of the event).
enum Act {
    None,
    Spawned(ShipId),
    Rebuild(ShipId, PieceId),
    /// The view of `(old, piece)` now belongs to `(ship, piece)`.
    Moved(ShipId, ShipId, PieceId),
    Split(ShipId, [PieceId; 8], usize),
    PieceGone(ShipId, PieceId),
    ShipGone(ShipId),
    /// A part destroyed; `true`: it explodes (a `Blast` follows).
    PartGone(ShipId, PieceId, sim::PartIdx, bool),
    /// A destroyed part exploded at a world point: radius, damage.
    Blast(ShipId, PieceId, sim::PartIdx, Vec2, f32, f32),
    Shot(ShipId, sim::ShotKind, Vec2, Vec2),
    Hit(Vec2, Vec2, data::WeaponFamily, f32),
    Ricochet(Vec2, Vec2),
    /// A vessel collected salvage that was at this point.
    Collected(ShipId, Vec2),
    /// An engine's plume (ship, engine part) burns a part at a point.
    Burn(ShipId, sim::PartIdx, Vec2),
    Shield(ShipId, PieceId, sim::PartIdx, Vec2, f32, bool),
}

/// Where a part of a piece is drawn (its blueprint centroid through the last view transform) and its piece's glow.
fn drawn_part(
    world: &SimWorld,
    views: &Query<(&PieceView, &Transform)>,
    ship: ShipId,
    piece: PieceId,
    part: sim::PartIdx,
) -> Option<(Vec2, Color)> {
    let (v, tf) = views.get(*world.views.get(&(ship, piece))?).ok()?;
    let [bx, by] = world.sim.ship(ship)?.design().parts[part as usize].centroid;
    let local = v.visual.to_local(Vec2::new(bx, by));
    Some((tf.transform_point(local.extend(0.0)).truncate(), v.glow))
}

fn weapon_color(world: &mut SimWorld, ship: ShipId, style: &Style) -> Color {
    let design = world.sim.ship(ship).map(|s| s.design().clone());
    design.and_then(|d| world.visual(&d, style)).map(|v| v.weapon_color).unwrap_or(Color::WHITE)
}

/// (Re)creates the view of one piece from the current sim state, on the piece's own draw layer
/// (`ship_view::draw_layer`: a rebuilt view keeps its place among other ships); the focused vessel draws above
/// them all at `Z_FOCUS`, decided every frame in `sync_piece_views` (so a focus switch reorders at once).
#[allow(clippy::too_many_arguments)]
fn respawn_view(
    commands: &mut Commands,
    world: &mut SimWorld,
    meshes: &mut Assets<Mesh>,
    atlas: &Atlas,
    style: &Style,
    toggles: &RenderToggles,
    player: Option<ShipId>,
    ship: ShipId,
    piece: PieceId,
) {
    if let Some(e) = world.views.remove(&(ship, piece)) {
        commands.entity(e).despawn();
    }
    let Some(design) = world.sim.ship(ship).map(|s| s.design().clone()) else { return };
    let Some(dv) = world.visual(&design, style) else { return };
    let layer = draw_layer(ship, piece);
    let Some(s) = world.sim.ship(ship) else { return };
    let Some(p) = s.piece(piece) else { return };
    let z = if player == Some(ship) && p.controlled { Z_FOCUS } else { layer };
    let e = spawn_piece_view(commands, meshes, atlas, style, toggles, &dv, s, p, layer, z, 1.0);
    world.views.insert((ship, piece), e);
}

/// Removes every view (scene reset).
pub fn despawn_all_views(commands: &mut Commands, world: &mut SimWorld) {
    for (_, e) in std::mem::take(&mut world.views) {
        commands.entity(e).despawn();
    }
}

/// World position of a piece's centre of mass at the render pose.
pub fn piece_com(piece: &sim::Piece, alpha: f32) -> Vec2 {
    let (pos, angle) = piece.render_pose(alpha);
    world_of(pos, angle, v2(piece.mass.com))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `?hashticks=0` underflowed the countdown on the first tick (a panic in debug builds).
    #[test]
    fn a_hash_log_logs_at_least_one_tick() {
        let log = HashLog::new("load".into(), 0);
        assert_eq!((log.remaining, log.total), (1, 1));
    }
}
