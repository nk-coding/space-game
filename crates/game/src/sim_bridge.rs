//! The seam between the deterministic `sim` crate and the Bevy client.
//!
//! * [`SimWorld`] owns the `Sim` (FreeFlight today; the bridge only uses the backend-independent API, so a
//!   physics engine slots in by changing the type alias [`SimT`]).
//! * `FixedUpdate` at [`sim::TICK_HZ`]: the scene fills [`SimWorld::inputs`] (`scene.rs`), [`step_sim`] steps once
//!   (timed; optional per-tick hash log), then [`apply_events`] turns the tick's [`sim::SimEvent`]s into view
//!   changes (spawn / rebuild / despawn piece views, keyed by `(ShipId, PieceId)`) and effects.
//! * Every frame, views read `piece.render_pose(alpha)` with `alpha = Time<Fixed>::overstep_fraction()`
//!   (`ship_view::sync_piece_views`). Nothing writes back into the sim except the inputs.

use std::collections::BTreeMap;
use std::sync::Arc;

use bevy::platform::time::Instant;
use bevy::prelude::*;
use data::Content;
use sim::{PieceId, ShipDesign, ShipId, SimEvent, Stat};

use crate::RenderToggles;
use crate::atlas::Atlas;
use crate::fx::Effects;
use crate::shield::ShieldBubbles;
use crate::ship_model::{DesignVisual, design_visual};
use crate::ship_view::{PieceView, Z_FOCUS, spawn_piece_view, world_of};
use crate::style::Style;

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
    scratch: Vec<f32>,
}

impl Timings {
    pub fn new(window: usize) -> Self {
        Self { samples: Vec::with_capacity(window), next: 0, scratch: Vec::with_capacity(window) }
    }
    pub fn push(&mut self, ms: f32) {
        if self.samples.len() < self.samples.capacity() {
            self.samples.push(ms);
        } else {
            self.samples[self.next] = ms;
            self.next = (self.next + 1) % self.samples.len();
        }
    }
    pub fn clear(&mut self) {
        self.samples.clear();
        self.next = 0;
    }
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    /// `(mean, median, p95, max)`.
    pub fn stats(&mut self) -> (f32, f32, f32, f32) {
        if self.samples.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        self.scratch.clear();
        self.scratch.extend_from_slice(&self.samples);
        self.scratch.sort_by(|a, b| a.total_cmp(b));
        let n = self.scratch.len();
        let mean = self.scratch.iter().sum::<f32>() / n as f32;
        let p = |q: f32| self.scratch[((n as f32 - 1.0) * q).round() as usize];
        (mean, p(0.5), p(0.95), self.scratch[n - 1])
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
    pub fn new(label: String, ticks: u64) -> Self {
        Self { label, remaining: ticks, total: ticks, acc: sim::StateHasher::new(), buf: String::with_capacity(4096) }
    }
}

#[derive(Resource)]
pub struct SimWorld {
    pub sim: SimT,
    /// Content the designs are compiled from (weapon scales from the tuning panel applied).
    pub content: Content,
    designs: BTreeMap<String, Arc<ShipDesign>>,
    visuals: BTreeMap<String, Arc<DesignVisual>>,
    /// Inputs for the next tick (filled by the scene, cleared after each step).
    pub inputs: sim::TickInputs,
    /// Live piece views.
    pub views: BTreeMap<(ShipId, PieceId), Entity>,
    next_z: u32,
    pub step_ms: Timings,
    pub hashlog: Option<HashLog>,
    /// Freeze the sim (used by the hash log when it is done, and by tools).
    pub frozen: bool,
}

impl SimWorld {
    pub fn new(sim: SimT, content: Content) -> Self {
        Self {
            sim,
            content,
            designs: BTreeMap::new(),
            visuals: BTreeMap::new(),
            inputs: sim::TickInputs::default(),
            views: BTreeMap::new(),
            next_z: 0,
            step_ms: Timings::new(900),
            hashlog: None,
            frozen: false,
        }
    }

    /// The compiled design of a blueprint (cached).
    pub fn design(&mut self, id: &str) -> Option<Arc<ShipDesign>> {
        if let Some(d) = self.designs.get(id) {
            return Some(d.clone());
        }
        let bp = self.content.ships.get(id)?;
        match ShipDesign::compile(&self.content, bp) {
            Ok(d) => {
                let d = Arc::new(d);
                self.designs.insert(id.to_string(), d.clone());
                Some(d)
            }
            Err(e) => {
                error!("cannot compile ship `{id}`: {e}");
                None
            }
        }
    }

    /// Drops compiled designs (after the content changed). Visuals stay (they don't depend on stats).
    pub fn forget_designs(&mut self) {
        self.designs.clear();
    }

    fn visual(&mut self, id: &str, style: &Style) -> Option<Arc<DesignVisual>> {
        if let Some(v) = self.visuals.get(id) {
            return Some(v.clone());
        }
        let bp = self.content.ships.get(id)?;
        match design_visual(&self.content, bp, style) {
            Ok(v) => {
                let v = Arc::new(v);
                self.visuals.insert(id.to_string(), v.clone());
                Some(v)
            }
            Err(e) => {
                error!("no visual for `{id}`: {e}");
                None
            }
        }
    }

    /// Draw order: every piece gets its own layer (z 10..89, round robin); the focused vessel draws above them
    /// all at `ship_view::Z_FOCUS`, decided every frame in `sync_piece_views` (so a focus switch reorders at once).
    fn next_layer(&mut self) -> f32 {
        self.next_z = (self.next_z + 1) % 80;
        10.0 + self.next_z as f32
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
            .configure_sets(FixedUpdate, (SimSet::Inputs, SimSet::Step, SimSet::Events).chain())
            .add_systems(FixedUpdate, (step_sim.in_set(SimSet::Step), apply_events.in_set(SimSet::Events)))
            .add_systems(
                PostUpdate,
                crate::ship_view::sync_piece_views.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

fn step_sim(mut world: ResMut<SimWorld>) {
    if world.frozen {
        return;
    }
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
    focus: Res<crate::scene::PilotFocus>,
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
            SimEvent::ShotFired { ship, kind, from, to, .. } => Act::Shot(*ship, *kind, v2(*from), v2(*to)),
            SimEvent::Hit { point, normal, family, damage, .. } => Act::Hit(v2(*point), v2(*normal), *family, *damage),
            SimEvent::Ricochet { point, vel, .. } => Act::Ricochet(v2(*point), v2(*vel)),
            SimEvent::ShieldHit { ship, piece, part, point, absorbed, depleted, .. } => {
                Act::Shield(*ship, *piece, *part, v2(*point), *absorbed, *depleted)
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
                    if let Ok((v, tf)) = views.get(e) {
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
            Act::PartGone(ship, piece, part, volatile) => {
                // Where the part was: its blueprint centroid through the last view transform.
                if let (Some(&e), Some(s)) = (world.views.get(&(ship, piece)), world.sim.ship(ship))
                    && let Ok((v, tf)) = views.get(e)
                {
                    let [bx, by] = s.design().parts[part as usize].centroid;
                    let local = v.visual.to_local(Vec2::new(bx, by));
                    let at = tf.transform_point(local.extend(0.0)).truncate();
                    let (c, size) = if volatile { (Color::srgb(1.0, 0.85, 0.55), 1.2) } else { (v.glow, 0.7) };
                    fx.burst(at, c, size);
                }
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
            Act::Shield(ship, piece, part, point, absorbed, depleted) => {
                // The ripple starts where the shot entered: a direction from the emitter in the piece frame (the
                // bubble turns with the ship), from the tick's pose, so it works while the view is being rebuilt;
                // its strength is the share of the capacity the shot took.
                if let Some(s) = world.sim.ship(ship)
                    && let Some(pc) = s.piece(piece)
                {
                    let [cx, cy] = s.design().parts[part as usize].shape.center();
                    let centre = Vec2::new(cx - pc.origin[0], pc.origin[1] - cy);
                    let local = Mat2::from_angle(-pc.body.angle) * (point - v2(pc.body.pos));
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
    PartGone(ShipId, PieceId, sim::PartIdx, bool),
    Shot(ShipId, sim::ShotKind, Vec2, Vec2),
    Hit(Vec2, Vec2, data::WeaponFamily, f32),
    Ricochet(Vec2, Vec2),
    Shield(ShipId, PieceId, sim::PartIdx, Vec2, f32, bool),
}

fn weapon_color(world: &mut SimWorld, ship: ShipId, style: &Style) -> Color {
    let id = world.sim.ship(ship).map(|s| s.design().id.clone());
    id.and_then(|id| world.visual(&id, style)).map(|v| v.weapon_color).unwrap_or(Color::WHITE)
}

/// (Re)creates the view of one piece from the current sim state.
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
    let old_z = world.views.get(&(ship, piece)).copied();
    if let Some(e) = old_z {
        commands.entity(e).despawn();
        world.views.remove(&(ship, piece));
    }
    let Some(design_id) = world.sim.ship(ship).map(|s| s.design().id.clone()) else { return };
    let Some(dv) = world.visual(&design_id, style) else { return };
    let layer = world.next_layer();
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
