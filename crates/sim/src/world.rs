//! The simulation world: tick counter, config, RNG streams, ships, the physics backend, events and the state
//! hash.
//!
//! # One tick (`Sim::step`)
//!
//! 1. Events of the previous step are dropped (events from calls made since then are kept).
//! 2. Inputs, in list order: raw `controls`, `commands` (pilot input, maneuvers, behaviors), scripted `damage`.
//! 3. Breakage is resolved (connectivity, splits); bodies are created, updated or removed in the backend.
//! 4. Flight: every ship is planned from the same snapshot of the world (behaviors, maneuvers, pilot input),
//!    then the flight computer allocates the actuators of each controlling piece.
//! 5. Ship systems: power → heat → wrench (engines, RCS, gimbals, gyros, cores) and gun output.
//! 6. Physics: split ghosts expire, wrenches are applied, the backend steps `DT`, bodies are read back
//!    (previous pose kept for interpolation), new contacts become `SimEvent::Impact`.
//! 7. `tick += 1`; debug builds store the state hash.

use std::sync::Arc;

use crate::DT;
use crate::behavior::{self, BehaviorId, BehaviorLibrary, BehaviorState};
use crate::breakage::{DamageOutcome, DamageTarget};
use crate::config::SimConfig;
use crate::design::ShipDesign;
use crate::events::SimEvent;
use crate::flight::{self, HelmMode, PilotInput};
use crate::hash::{StateHash, StateHasher};
use crate::ids::{IdAlloc, PieceId, ShipId};
use crate::maneuver::Maneuver;
use crate::math::{Rot, Vec2};
use crate::physics::{BodyDesc, BodyKey, FreeFlight, PhysicsBackend, RayHit};
use crate::rng::{Pcg32, RngStreams, Stream};
use crate::ship::{Body, Ship, ShipControls};
use crate::weapons::{BeamTrace, Bubble, Projectile};

/// Scripted damage (tests, scenarios, replays).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageCommand {
    pub ship: ShipId,
    pub target: DamageTarget,
    pub amount: f32,
}

/// Who flies a ship from now on (persists until the next command).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShipCommand {
    /// Pilot stick, lock, drift and fire (send it every tick the input changes, or every tick).
    Pilot(PilotInput),
    /// Fly a built-in maneuver.
    Maneuver { maneuver: Maneuver, fire: bool },
    /// Fly a goal directly (tools, tests).
    Goal {
        goal: flight::FlightGoal,
        fire: bool,
    },
    /// Hand the ship to a behavior of the sim's [`BehaviorLibrary`] (`Some`) or take it back (`None`). A
    /// behavior overrides the helm. Unknown ids are ignored.
    Behavior(Option<BehaviorId>),
    /// Set the ship whose lock the `locked` target selector follows (an escort's leader), or none.
    Leader(Option<ShipId>),
    /// Change a consumer part's power tier and weight (`Ship::set_power_setting`; ignored for parts that draw
    /// no power and for invalid weights).
    Power {
        part: crate::design::PartIdx,
        setting: crate::power::PowerSetting,
    },
}

/// Everything from outside that affects one tick. Applied in list order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickInputs {
    /// Raw throttles per engine slot (switches the ship to `HelmMode::Raw`: no flight computer).
    pub controls: Vec<(ShipId, ShipControls)>,
    pub commands: Vec<(ShipId, ShipCommand)>,
    pub damage: Vec<DamageCommand>,
}

/// A ray hit resolved to a part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartHit {
    pub ship: ShipId,
    pub piece: PieceId,
    /// The part under the hit point (`None` only if the backend and the grid disagree).
    pub part: Option<crate::design::PartIdx>,
    pub hit: RayHit,
}

/// The deterministic world. Same seed + same config + same inputs = same state hash every tick, on every
/// platform. `P` is the physics backend (`FreeFlight` until the engine is chosen).
#[derive(Clone, Debug)]
pub struct Sim<P: PhysicsBackend = FreeFlight> {
    tick: u64,
    seed: u64,
    config: SimConfig,
    rng: RngStreams,
    pub(crate) ids: IdAlloc,
    /// Ascending id.
    pub(crate) ships: Vec<Ship>,
    pub(crate) physics: P,
    pub(crate) events: Vec<SimEvent>,
    /// Events before this index belong to the previous step.
    events_mark: usize,
    tick_hash: Option<u64>,
    plans: Vec<flight::Plan>,
    /// Live projectiles, ascending id.
    pub(crate) projectiles: Vec<Projectile>,
    /// Beams that fired this tick.
    pub(crate) beams: Vec<BeamTrace>,
    /// Shield bubbles this tick.
    pub(crate) bubbles: Vec<Bubble>,
    pub(crate) shield_scratch: Vec<(f32, u32)>,
    /// Behavior programs (shared; ships refer to them by `BehaviorId`).
    behaviors: BehaviorLibrary,
    behavior_scratch: behavior::Scratch,
}

impl Sim<FreeFlight> {
    /// A world with the default config and the engine-free `FreeFlight` backend.
    pub fn new(seed: u64) -> Sim<FreeFlight> {
        Sim::with_backend(seed, SimConfig::default(), FreeFlight::new())
    }

    pub fn with_config(seed: u64, config: SimConfig) -> Sim<FreeFlight> {
        Sim::with_backend(seed, config, FreeFlight::new())
    }
}

impl<P: PhysicsBackend> Sim<P> {
    pub fn with_backend(seed: u64, config: SimConfig, physics: P) -> Sim<P> {
        Sim {
            tick: 0,
            seed,
            config,
            rng: RngStreams::new(seed),
            ids: IdAlloc::default(),
            ships: Vec::new(),
            physics,
            events: Vec::new(),
            events_mark: 0,
            tick_hash: None,
            plans: Vec::new(),
            projectiles: Vec::new(),
            beams: Vec::new(),
            bubbles: Vec::new(),
            shield_scratch: Vec::new(),
            behaviors: BehaviorLibrary::default(),
            behavior_scratch: behavior::Scratch::default(),
        }
    }

    /// Replaces the behavior library (`BehaviorLibrary::new(&content)`). Every ship's behavior is dropped,
    /// since handles refer to the old library; set the library before commanding behaviors.
    pub fn set_behaviors(&mut self, lib: BehaviorLibrary) {
        self.behavior_scratch = behavior::Scratch::default();
        self.behavior_scratch.scopes = lib.script_scopes();
        self.behaviors = lib;
        for s in &mut self.ships {
            s.behavior = None;
        }
    }

    pub fn behaviors(&self) -> &BehaviorLibrary {
        &self.behaviors
    }

    /// Add player programs here (`add_text`); existing handles stay valid.
    pub fn behaviors_mut(&mut self) -> &mut BehaviorLibrary {
        &mut self.behaviors
    }

    /// Rhai condition evaluations so far (diagnostics; not part of the state).
    pub fn script_evaluations(&self) -> u64 {
        self.behavior_scratch.script_evals
    }

    /// Runs the behavior think pass for every ship with a behavior now, whatever the cadence (benchmarks and
    /// tools; `step` thinks on its own).
    #[doc(hidden)]
    pub fn think_all_behaviors_now(&mut self) {
        behavior::think_all(
            &mut self.ships,
            &self.behaviors,
            &mut self.behavior_scratch,
            self.tick,
            &self.config,
            true,
        );
    }

    /// Number of completed ticks.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn config(&self) -> &SimConfig {
        &self.config
    }

    /// Changes the config. Thrust and torque scales apply to every ship at once.
    pub fn set_config(&mut self, config: SimConfig) {
        self.config = config;
        for s in &mut self.ships {
            s.thrust_scale = config.thrust_scale;
            s.torque_scale = config.torque_scale;
            for k in 0..s.pieces.len() {
                s.rebuild_piece(k);
            }
        }
    }

    /// Simulated time in seconds (`tick × DT`, computed in f64 for display; never fed back into the sim).
    pub fn time_seconds(&self) -> f64 {
        self.tick as f64 / crate::TICK_HZ as f64
    }

    /// The random stream of one system.
    pub fn rng(&mut self, s: Stream) -> &mut Pcg32 {
        self.rng.get(s)
    }

    pub fn physics(&self) -> &P {
        &self.physics
    }

    /// Spawns a ship. `body` is the pose and velocity of the blueprint's main piece. Emits `ShipSpawned`.
    pub fn spawn_ship(&mut self, design: Arc<ShipDesign>, body: Body) -> ShipId {
        let id = self.ids.ship();
        let ship = Ship::new(id, design, body, &mut self.ids, &self.config);
        for p in ship.pieces() {
            self.physics.create_body(
                BodyKey {
                    ship: id,
                    piece: p.id,
                },
                &BodyDesc {
                    body: p.body,
                    mass: p.mass,
                    colliders: &p.colliders,
                },
            );
        }
        self.ships.push(ship);
        self.events.push(SimEvent::ShipSpawned { ship: id });
        id
    }

    /// Removes a ship and all its bodies. Emits `ShipRemoved`.
    pub fn remove_ship(&mut self, id: ShipId) -> Option<Ship> {
        let k = self.ship_index(id)?;
        let ship = self.ships.remove(k);
        for p in ship.pieces() {
            self.physics.remove_body(BodyKey {
                ship: id,
                piece: p.id,
            });
        }
        self.events.push(SimEvent::ShipRemoved { ship: id });
        Some(ship)
    }

    pub fn ships(&self) -> &[Ship] {
        &self.ships
    }

    pub fn ship(&self, id: ShipId) -> Option<&Ship> {
        self.ship_index(id).map(|k| &self.ships[k])
    }

    /// Mutable ship access (controls, power priority). Do not move pieces through it; use `teleport`.
    pub fn ship_mut(&mut self, id: ShipId) -> Option<&mut Ship> {
        self.ship_index(id).map(move |k| &mut self.ships[k])
    }

    fn ship_index(&self, id: ShipId) -> Option<usize> {
        self.ships.binary_search_by_key(&id, |s| s.id()).ok()
    }

    /// Sets a piece's pose and velocities in the sim and the backend (tools, tests, respawn).
    pub fn teleport(&mut self, ship: ShipId, piece: PieceId, body: Body) {
        if let Some(p) = self.ship_mut(ship).and_then(|s| s.piece_mut(piece)) {
            p.body = body;
            p.prev_pos = body.pos;
            p.prev_angle = body.angle;
            self.physics.set_body(BodyKey { ship, piece }, body);
        }
    }

    /// Hands a command to a ship now (same as putting it into the next tick's inputs).
    pub fn command(&mut self, ship: ShipId, cmd: ShipCommand) {
        if let Some(s) = self.ship_mut(ship) {
            match cmd {
                ShipCommand::Pilot(p) => s.helm = HelmMode::Pilot(p),
                ShipCommand::Maneuver { maneuver, fire } => {
                    s.helm = HelmMode::Maneuver { maneuver, fire }
                }
                ShipCommand::Goal { goal, fire } => s.helm = HelmMode::Goal { goal, fire },
                ShipCommand::Behavior(None) => s.behavior = None,
                ShipCommand::Behavior(Some(id)) => {
                    let program = self.behaviors.get(id).cloned();
                    let s = match self.ships.binary_search_by_key(&ship, |s| s.id()) {
                        Ok(k) => &mut self.ships[k],
                        Err(_) => return,
                    };
                    if let Some(p) = program {
                        s.behavior = Some(BehaviorState::new(id, p));
                    }
                }
                ShipCommand::Leader(l) => s.leader = l,
                ShipCommand::Power { part, setting } => {
                    s.set_power_setting(part, setting);
                }
            }
        }
    }

    /// Spawns a projectile directly (tools, tests). It flies from the next step on and never hits `owner`.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_projectile(
        &mut self,
        owner: ShipId,
        family: data::WeaponFamily,
        pos: Vec2,
        vel: Vec2,
        damage: f32,
        pierce: u32,
        life: f32,
    ) -> crate::weapons::ProjectileId {
        let id = self.ids.projectile();
        self.projectiles.push(Projectile {
            id,
            owner,
            family,
            pos,
            prev_pos: pos,
            vel,
            damage,
            pierce: pierce.max(1),
            life,
            ricochets: 0,
        });
        self.events.push(SimEvent::ProjectileSpawned {
            id,
            owner,
            pos,
            vel,
        });
        id
    }

    /// Live projectiles (ascending id). Render each at `prev_pos + (pos − prev_pos) × alpha`.
    pub fn projectiles(&self) -> &[Projectile] {
        &self.projectiles
    }

    /// Beams that fired in the last step (draw from `from` to `to`).
    pub fn beams(&self) -> &[BeamTrace] {
        &self.beams
    }

    /// Shield bubbles that were up in the last step (centre, radius, capacity left).
    pub fn bubbles(&self) -> &[Bubble] {
        &self.bubbles
    }

    /// Events of the last step plus those of calls made since (spawns, direct damage resolution).
    pub fn events(&self) -> &[SimEvent] {
        &self.events
    }

    /// Applies damage now. Splits are resolved in the next step or by [`Sim::resolve_breakage`].
    pub fn apply_damage(
        &mut self,
        ship: ShipId,
        target: DamageTarget,
        amount: f32,
    ) -> DamageOutcome {
        match self.ship_mut(ship) {
            Some(s) => s.apply_damage(target, amount),
            None => DamageOutcome::Ignored,
        }
    }

    /// Resolves breakage of all ships now (ascending ship id), appends the events and updates the bodies in
    /// the backend.
    pub fn resolve_breakage(&mut self) {
        for k in 0..self.ships.len() {
            let from = self.events.len();
            self.ships[k].resolve_breakage(&mut self.ids, &mut self.events);
            if self.events.len() > from {
                self.sync_bodies(k, from);
            }
        }
    }

    /// Mirrors new breakage events of ship `k` into the backend and starts split ghosting.
    fn sync_bodies(&mut self, k: usize, from: usize) {
        let ghost_until = self.tick + self.config.split_ghost_ticks as u64;
        for e in from..self.events.len() {
            let ship = &mut self.ships[k];
            let sid = ship.id();
            match &self.events[e] {
                SimEvent::PieceChanged { piece, .. } => {
                    if let Some(p) = ship.piece(piece.id) {
                        self.physics.update_body(
                            BodyKey {
                                ship: sid,
                                piece: p.id,
                            },
                            &BodyDesc {
                                body: p.body,
                                mass: p.mass,
                                colliders: &p.colliders,
                            },
                        );
                    }
                }
                SimEvent::Split(split) => {
                    let mut ids = vec![split.kept.id];
                    ids.extend(split.spawned.iter().map(|s| s.id));
                    for (n, &pid) in ids.iter().enumerate() {
                        let Some(p) = ship.piece_mut(pid) else {
                            continue;
                        };
                        if self.config.split_ghost_ticks > 0 {
                            p.ghost_until = ghost_until;
                        }
                        let key = BodyKey {
                            ship: sid,
                            piece: pid,
                        };
                        let desc = BodyDesc {
                            body: p.body,
                            mass: p.mass,
                            colliders: &p.colliders,
                        };
                        if n == 0 {
                            self.physics.update_body(key, &desc);
                        } else {
                            self.physics.create_body(key, &desc);
                        }
                        if self.config.split_ghost_ticks > 0 {
                            self.physics.set_ghost(key, true);
                        }
                    }
                }
                SimEvent::PieceDestroyed { piece, .. } => {
                    self.physics.remove_body(BodyKey {
                        ship: sid,
                        piece: *piece,
                    });
                }
                _ => {}
            }
        }
    }

    /// First part hit by a ray (unit `dir`), ignoring ship `exclude` (e.g. the shooter).
    pub fn raycast(
        &self,
        origin: Vec2,
        dir: Vec2,
        max_distance: f32,
        exclude: Option<ShipId>,
    ) -> Option<PartHit> {
        let hit = self.physics.raycast(origin, dir, max_distance, exclude)?;
        let ship = self.ship(hit.key.ship)?;
        // Nudge the point into the solid so it lands inside the hit cell.
        let piece = ship.piece(hit.key.piece)?;
        let n_local = Rot::from_angle(piece.body.angle).apply_inverse(hit.normal);
        let part = ship.part_at_local(hit.key.piece, hit.local_point - n_local * 1e-3);
        Some(PartHit {
            ship: hit.key.ship,
            piece: hit.key.piece,
            part,
            hit,
        })
    }

    /// Advances the world by one fixed step of [`crate::DT`].
    pub fn step(&mut self, inputs: &TickInputs) {
        self.events.drain(..self.events_mark);

        for (id, c) in &inputs.controls {
            if let Some(s) = self.ship_mut(*id) {
                s.set_controls(c);
            }
        }
        for (id, cmd) in &inputs.commands {
            self.command(*id, *cmd);
        }
        for d in &inputs.damage {
            self.apply_damage(d.ship, d.target, d.amount);
        }
        self.resolve_breakage();

        // Behaviors decide (each ship every few ticks, from one snapshot), then flight flies the decisions.
        behavior::think_all(
            &mut self.ships,
            &self.behaviors,
            &mut self.behavior_scratch,
            self.tick,
            &self.config,
            false,
        );

        // Flight: plan every ship from the same world, then act.
        let mut plans = std::mem::take(&mut self.plans);
        plans.clear();
        plans.extend(
            self.ships
                .iter()
                .map(|s| flight::plan(s, &self.ships, &self.config)),
        );
        for (s, plan) in self.ships.iter_mut().zip(&plans) {
            flight::execute(s, plan, &self.config);
        }
        self.plans = plans;

        for s in &mut self.ships {
            s.update_systems();
        }

        // Weapons (shields, projectiles, guns), then whatever they broke.
        self.weapons_step();
        self.resolve_breakage();

        // Physics.
        for s in &mut self.ships {
            let sid = s.id();
            for p in &mut s.pieces {
                let key = BodyKey {
                    ship: sid,
                    piece: p.id,
                };
                if p.ghost_until != 0 && self.tick >= p.ghost_until {
                    p.ghost_until = 0;
                    self.physics.set_ghost(key, false);
                }
                if p.wrench.force != Vec2::ZERO || p.wrench.torque != 0.0 {
                    let f = Rot::from_angle(p.body.angle).apply(p.wrench.force);
                    self.physics.apply_wrench(key, f, p.wrench.torque);
                }
            }
        }
        self.physics.step(DT);
        for s in &mut self.ships {
            let sid = s.id();
            for p in &mut s.pieces {
                p.prev_pos = p.body.pos;
                p.prev_angle = p.body.angle;
                if let Some(b) = self.physics.body(BodyKey {
                    ship: sid,
                    piece: p.id,
                }) {
                    p.body = b;
                }
            }
        }
        for c in self.physics.contacts() {
            self.events.push(SimEvent::Impact(*c));
        }

        self.tick += 1;
        self.events_mark = self.events.len();
        if cfg!(debug_assertions) {
            self.tick_hash = Some(self.state_hash());
        }
    }

    /// The state hash stored at the end of the last step (debug builds only; `None` in release).
    pub fn last_tick_hash(&self) -> Option<u64> {
        self.tick_hash
    }

    /// FNV-1a 64 over the whole simulation state (exact float bits). Computed on demand.
    pub fn state_hash(&self) -> u64 {
        let mut h = StateHasher::new();
        self.hash_state(&mut h);
        h.finish()
    }
}

impl<P: PhysicsBackend> StateHash for Sim<P> {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.tick);
        h.u64(self.seed);
        self.config.hash_state(h);
        for r in self.rng.iter() {
            let (s, i) = r.raw();
            h.u64(s);
            h.u64(i);
        }
        let (ns, np, nq) = self.ids.raw();
        h.u32(ns);
        h.u32(np);
        h.u32(nq);
        h.u32(self.ships.len() as u32);
        for s in &self.ships {
            s.hash_state(h);
        }
        h.u32(self.projectiles.len() as u32);
        for p in &self.projectiles {
            p.hash_state(h);
        }
        self.physics.hash_state(h);
    }
}
