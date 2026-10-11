//! The simulation world: tick counter, config, RNG streams, ships, the physics backend, events and the state
//! hash.
//!
//! # One tick (`Sim::step`)
//!
//! 1. Events of the previous step are dropped (events from calls made since then are kept).
//! 2. Inputs, in list order: raw `controls`, `commands` (pilot input, maneuvers, brains), scripted `damage`.
//! 3. Breakage is resolved (connectivity, splits); bodies are created, updated or removed in the backend.
//! 4. Flight: every ship is planned from the same snapshot of the world (brains, maneuvers, pilot input),
//!    then the flight computer allocates the actuators of each controlling piece.
//! 5. Ship systems: power → heat → wrench (engines, RCS, gimbals, gyros, cores) and gun output.
//! 6. Physics: split ghosts expire, wrenches are applied, the backend steps `DT`, bodies are read back
//!    (previous pose kept for interpolation), new contacts become `SimEvent::Impact`.
//! 7. `tick += 1`; debug builds store the state hash.

use std::sync::Arc;

use crate::DT;
use crate::breakage::{DamageOutcome, DamageTarget};
use crate::config::SimConfig;
use crate::design::ShipDesign;
use crate::events::SimEvent;
use crate::flight::{self, HelmMode, PilotInput};
use crate::hash::{StateHash, StateHasher};
use crate::ids::{IdAlloc, PieceId, ShipId};
use crate::maneuver::{Maneuver, RaiderBrain};
use crate::math::{Rot, Vec2};
use crate::physics::{BodyDesc, BodyKey, FreeFlight, PhysicsBackend, RayHit};
use crate::rng::{Pcg32, RngStreams, Stream};
use crate::ship::{Body, Ship, ShipControls};

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
    /// Hand the ship to a brain (`Some`) or take it back (`None`). A brain overrides the helm.
    Brain(Option<RaiderBrain>),
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
    ids: IdAlloc,
    /// Ascending id.
    ships: Vec<Ship>,
    physics: P,
    events: Vec<SimEvent>,
    /// Events before this index belong to the previous step.
    events_mark: usize,
    tick_hash: Option<u64>,
    plans: Vec<flight::Plan>,
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
        }
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
                ShipCommand::Brain(b) => s.brain = b,
            }
        }
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
        let (ns, np) = self.ids.raw();
        h.u32(ns);
        h.u32(np);
        h.u32(self.ships.len() as u32);
        for s in &self.ships {
            s.hash_state(h);
        }
        self.physics.hash_state(h);
    }
}
