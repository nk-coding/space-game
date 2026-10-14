//! The simulation world: tick counter, config, RNG streams, ships, the physics world, events and the state
//! hash.
//!
//! # One tick (`Sim::step`)
//!
//! 1. Events of the previous step are dropped (events from calls made since then are kept).
//! 2. Inputs, in list order: raw `controls`, `commands` (pilot input, maneuvers, behaviors), scripted `damage`.
//! 3. Breakage is resolved (connectivity, splits); bodies are created, updated or removed in the physics;
//!    split-off pieces with a logic core become vessels of their own, and the mind pass (focus, dormant
//!    vessels; `vessel.rs`) runs if anything changed.
//! 4. Behaviors think, then flight: every ship is planned from the same snapshot of the world (behaviors,
//!    maneuvers, pilot input), then the flight computer allocates the actuators of each controlling piece.
//! 5. Ship systems: per piece the control pass (idle parts), power, shield regeneration, the wrench (engines,
//!    RCS, gimbals, gyros, cores) and gun output; then heat (ship-wide); then each piece's authority.
//! 6. Weapons (shields, projectiles, guns) and engine plumes, then breakage again; every
//!    `SalvageTuning::interval` ticks collecting vessels take salvage (`salvage.rs`), and what they stripped
//!    breaks up.
//! 7. Physics: split ghosts expire, wrenches are set, Rapier steps `DT` and loose fragments drift, contacts
//!    become `SimEvent::Impact` (closing speed from the velocities before the step), bodies are read back
//!    (previous pose kept for interpolation); impacts above the ramming threshold crush inward from the
//!    touching parts (none between vessels of one owner; shield bubbles covering the point absorb first), and
//!    breakage is resolved once more.
//! 8. The voyage director, if a voyage is underway (`voyage.rs`): sites spawn, wake and clear.
//! 9. `tick += 1`; debug builds store the state hash.

use std::sync::Arc;

use crate::behavior::{self, BehaviorId, BehaviorLibrary, BehaviorState};
use crate::breakage::{DamageOutcome, DamageTarget};
use crate::config::SimConfig;
use crate::design::{PartIdx, ShipDesign};
use crate::events::SimEvent;
use crate::flight::{self, HelmMode, PilotInput};
use crate::hash::{StateHash, StateHasher};
use crate::ids::{IdAlloc, PieceId, ShipId};
use crate::maneuver::Maneuver;
use crate::math::{Rot, Vec2};
use crate::physics::{BodyDesc, BodyKey, Contact, Physics, RayHit};
use crate::rng::{Pcg32, RngStreams, Stream};
use crate::salvage::SalvageState;
use crate::ship::{Body, Piece, Ship, ShipControls};
use crate::vessel::{self, MindRow, Owner};
use crate::voyage::Voyage;
use crate::weapons::{BeamTrace, Bubble, Projectile};

/// The phases of one [`Sim::step`], in order, for profiling tools ([`Sim::step_profiled`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepPhase {
    /// Inputs and breakage resolution.
    Inputs,
    /// Behaviors and the flight computer.
    Flight,
    /// Power, heat, wrenches, guns.
    Systems,
    /// Shields, projectiles, firing, breakage, salvage.
    Weapons,
    /// The Rapier step (with setting wrenches and ghosts).
    Physics,
    /// Contacts → impacts, read-back, ramming damage and its breakage.
    Impacts,
    /// The debug-build state hash.
    Hash,
    /// The step is over.
    Done,
}

/// A contact point is moved this far into the hull (against the outward normal) to find the part under it.
const CONTACT_NUDGE: f32 = 0.05;

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
    /// behavior overrides the helm. Unknown ids are ignored. For its owner's focus (piloted) it sets the
    /// behavior the vessel runs once the focus leaves it.
    Behavior(Option<BehaviorId>),
    /// Set the ship whose lock the `locked` target selector follows (an escort's leader), or none.
    Leader(Option<ShipId>),
    /// Change a consumer part's power tier and weight (`Ship::set_power_setting`; ignored for parts that draw
    /// no power and for invalid weights).
    Power {
        part: crate::design::PartIdx,
        setting: crate::power::PowerSetting,
    },
    /// Make this vessel its owner's focus (instant; GDR-0013): it is piloted from now on (its behavior is
    /// parked) and never goes dormant; the vessel the focus left runs its behavior again. Ignored for salvage.
    Focus,
    /// Wake this vessel (GDR-0044): it claims its owner's mind capacity before every vessel not woken since,
    /// so the newest other vessels go dormant to make room. Ignored for salvage.
    Wake,
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
    /// The part the ray enters.
    pub part: PartIdx,
    pub hit: RayHit,
}

/// The deterministic world. Same seed + same config + same inputs = same state hash every tick, on every
/// platform.
#[derive(Clone, Debug)]
pub struct Sim {
    tick: u64,
    seed: u64,
    config: SimConfig,
    rng: RngStreams,
    pub(crate) ids: IdAlloc,
    /// Ascending id.
    pub(crate) ships: Vec<Ship>,
    pub(crate) physics: Physics,
    /// This tick's impacts (scratch, reused).
    impacts: Vec<Contact>,
    pub(crate) events: Vec<SimEvent>,
    /// Events before this index belong to the previous step.
    events_mark: usize,
    tick_hash: Option<u64>,
    /// This tick's physics step has run (the impacts phase of `step`); false between steps.
    stepped: bool,
    plans: Vec<flight::Plan>,
    /// Live projectiles, ascending id.
    pub(crate) projectiles: Vec<Projectile>,
    /// Beams that fired this tick.
    pub(crate) beams: Vec<BeamTrace>,
    /// Shield bubbles this tick.
    pub(crate) bubbles: Vec<Bubble>,
    pub(crate) shield_scratch: Vec<(f32, u32)>,
    /// The fire pass's hits, applied once every gun has fired (scratch, reused; `weapons.rs`).
    pub(crate) gun_hits: Vec<crate::weapons::GunHit>,
    pub(crate) plume_scratch: crate::plume::PlumeScratch,
    /// The chain pass's queue and buffers (scratch, reused; `chain.rs`).
    pub(crate) chain_scratch: crate::chain::ChainScratch,
    /// Behavior programs (shared; ships refer to them by `BehaviorId`).
    behaviors: BehaviorLibrary,
    behavior_scratch: behavior::Scratch,
    /// The focus of each owner that has one, ascending owner (`vessel.rs`).
    pub(crate) focus: Vec<(Owner, ShipId)>,
    /// Claim counter of the mind pass (formation and wake order).
    pub(crate) mind_seq: i64,
    /// Something changed that the mind pass must look at (vessels, cores, focus, config).
    pub(crate) mind_dirty: bool,
    pub(crate) mind_scratch: Vec<MindRow>,
    /// Loose fragments and the collect pass's buffer (`salvage.rs`).
    pub(crate) salvage: SalvageState,
    /// The voyage underway, if any (`voyage.rs`).
    pub(crate) voyage: Option<Box<Voyage>>,
}

impl Sim {
    /// A world with the default config.
    pub fn new(seed: u64) -> Sim {
        Sim::with_config(seed, SimConfig::default())
    }

    /// A world with `config` (NaN or infinite numbers in it count as their defaults,
    /// `SimConfig::finite_or_default`).
    pub fn with_config(seed: u64, config: SimConfig) -> Sim {
        let config = config.finite_or_default();
        Sim {
            tick: 0,
            seed,
            config,
            rng: RngStreams::new(seed),
            ids: IdAlloc::default(),
            ships: Vec::new(),
            physics: Physics::new(config.physics),
            impacts: Vec::new(),
            events: Vec::new(),
            events_mark: 0,
            tick_hash: None,
            stepped: false,
            plans: Vec::new(),
            projectiles: Vec::new(),
            beams: Vec::new(),
            bubbles: Vec::new(),
            shield_scratch: Vec::new(),
            gun_hits: Vec::new(),
            plume_scratch: Default::default(),
            chain_scratch: Default::default(),
            behaviors: BehaviorLibrary::default(),
            behavior_scratch: behavior::Scratch::default(),
            focus: Vec::new(),
            mind_seq: 0,
            mind_dirty: false,
            mind_scratch: Vec::new(),
            salvage: SalvageState::default(),
            voyage: None,
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
            s.parked = None;
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

    /// Changes the config. Thrust, torque and speed scales, the physics tuning and the mind capacities apply to
    /// every ship at once. NaN or infinite numbers count as their defaults (`SimConfig::finite_or_default`).
    pub fn set_config(&mut self, config: SimConfig) {
        let config = config.finite_or_default();
        self.config = config;
        self.physics.configure(config.physics);
        for s in &mut self.ships {
            s.thrust_scale = config.thrust_scale;
            s.torque_scale = config.torque_scale;
            s.speed_scale = config.speed_scale;
            for k in 0..s.pieces.len() {
                s.rebuild_piece(k);
            }
        }
        self.mind_dirty = true;
        self.update_minds();
    }

    /// Simulated time in seconds (`tick × DT`, computed in f64 for display; never fed back into the sim).
    pub fn time_seconds(&self) -> f64 {
        self.tick as f64 / crate::TICK_HZ as f64
    }

    /// The random stream of one system.
    pub fn rng(&mut self, s: Stream) -> &mut Pcg32 {
        self.rng.get(s)
    }

    /// The physics world (read-only: bodies, ghosts, sleeping, ray casts).
    pub fn physics(&self) -> &Physics {
        &self.physics
    }

    /// Spawns a ship for `owner`. `body` is the pose and velocity of the blueprint's main piece. Emits
    /// `ShipSpawned`, and `VesselFormed` if it has a logic core (a blueprint in several pieces with cores in
    /// more than one: each further cored piece becomes a vessel of its own). It may start dormant if its
    /// owner's mind capacity is used up (`VesselDormant`). NaN or infinite numbers in `body` count as 0.
    pub fn spawn_ship(&mut self, design: Arc<ShipDesign>, owner: Owner, body: Body) -> ShipId {
        let k = self.new_ship(design, owner, body);
        let id = self.ships[k].id();
        if self.ships[k].is_vessel() {
            self.vessel_formed(k, None);
        }
        self.form_vessels(k);
        self.update_minds();
        id
    }

    /// A ship from outside (`spawn_ship`, `spawn_wreck`): a new id, built with NaN or infinite numbers in `body`
    /// counting as 0, added (`ShipSpawned`). Returns its index.
    pub(crate) fn new_ship(&mut self, design: Arc<ShipDesign>, owner: Owner, body: Body) -> usize {
        let id = self.ids.ship();
        let body = body.finite_or_zero();
        let ship = Ship::new(id, design, owner, body, &mut self.ids, &self.config);
        self.add_ship(ship)
    }

    /// Adds a new ship (its id after every existing one): a body per piece, `ShipSpawned`. Returns its index.
    pub(crate) fn add_ship(&mut self, ship: Ship) -> usize {
        let id = ship.id();
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
        self.ships.len() - 1
    }

    /// Removes a ship and all its bodies. Emits `VesselLost` (if it was a vessel) and `ShipRemoved`; a
    /// removed focus moves on.
    pub fn remove_ship(&mut self, id: ShipId) -> Option<Ship> {
        let k = self.ship_index(id)?;
        let ship = self.ships.remove(k);
        for p in ship.pieces() {
            self.physics.remove_body(BodyKey {
                ship: id,
                piece: p.id,
            });
        }
        if ship.is_vessel() {
            self.events.push(SimEvent::VesselLost {
                ship: id,
                owner: ship.owner(),
            });
            self.mind_dirty = true;
        }
        self.events.push(SimEvent::ShipRemoved { ship: id });
        self.update_minds();
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

    pub(crate) fn ship_index(&self, id: ShipId) -> Option<usize> {
        self.ships.binary_search_by_key(&id, |s| s.id()).ok()
    }

    /// Sets a piece's pose and velocities in the sim and the physics (tools, tests, respawn); NaN or infinite
    /// numbers count as 0.
    pub fn teleport(&mut self, ship: ShipId, piece: PieceId, body: Body) {
        let body = body.finite_or_zero();
        if let Some(p) = self.ship_mut(ship).and_then(|s| s.piece_mut(piece)) {
            p.body = body;
            p.prev_pos = body.pos;
            p.prev_angle = body.angle;
            self.physics.set_body(BodyKey { ship, piece }, body);
        }
    }

    /// Hands a command to a ship now (same as putting it into the next tick's inputs). A pilot input, maneuver
    /// or goal with a NaN or infinite number in it is ignored (it would poison the flight computer and the
    /// physics body).
    pub fn command(&mut self, ship: ShipId, cmd: ShipCommand) {
        let Some(k) = self.ship_index(ship) else {
            return;
        };
        let finite = match &cmd {
            ShipCommand::Pilot(p) => p.is_finite(),
            ShipCommand::Maneuver { maneuver, .. } => maneuver.is_finite(),
            ShipCommand::Goal { goal, .. } => goal.is_finite(),
            _ => true,
        };
        if !finite {
            return;
        }
        let piloted = self.focus(self.ships[k].owner()) == Some(ship);
        match cmd {
            ShipCommand::Focus => return self.set_focus(ship),
            ShipCommand::Wake => return self.wake(ship),
            _ => {}
        }
        let program = match cmd {
            ShipCommand::Behavior(Some(id)) => match self.behaviors.get(id) {
                Some(p) => Some(BehaviorState::new(id, Arc::clone(p))),
                None => return,
            },
            _ => None,
        };
        let s = &mut self.ships[k];
        match cmd {
            ShipCommand::Pilot(p) => s.helm = HelmMode::Pilot(p),
            ShipCommand::Maneuver { maneuver, fire } => {
                s.helm = HelmMode::Maneuver { maneuver, fire }
            }
            ShipCommand::Goal { goal, fire } => s.helm = HelmMode::Goal { goal, fire },
            ShipCommand::Behavior(_) if piloted => s.parked = program,
            ShipCommand::Behavior(_) => s.behavior = program,
            ShipCommand::Leader(l) => s.leader = l,
            ShipCommand::Power { part, setting } => {
                s.set_power_setting(part, setting);
            }
            ShipCommand::Focus | ShipCommand::Wake => {}
        }
    }

    /// Spawns a projectile directly (tools, tests). It flies from the next step on and never hits `owner`, nor
    /// any vessel of `owner`'s owner while `owner` is a vessel now (GDR-0048). NaN or infinite numbers count as 0.
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
        use crate::math::finite_or_zero;
        let (pos, vel) = (pos.finite_or_zero(), vel.finite_or_zero());
        let (damage, life) = (finite_or_zero(damage), finite_or_zero(life));
        let id = self.ids.projectile();
        let team = self.ship(owner).and_then(|s| s.team());
        self.projectiles.push(Projectile {
            id,
            owner,
            team,
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
    /// the physics. First every chain reaction goes off (`chain.rs`: a blast may reach any ship); then destroyed
    /// parts whose salvage block names a fragment leave one; split-off pieces with a logic core
    /// become vessels of their own (new ships, after the existing ones); lost vessels and cores update the
    /// minds (focus, dormant vessels); a vessel whose hold shrank spills cargo as loose items.
    pub fn resolve_breakage(&mut self) {
        self.chain_reactions();
        for k in 0..self.ships.len() {
            self.resolve_ship(k);
        }
        self.update_minds();
    }

    /// Resolves the breakage of ship `k` alone (fragments, splits, bodies, vessels formed, its hold); the caller
    /// runs the mind pass (`update_minds`).
    pub(crate) fn resolve_ship(&mut self, k: usize) {
        if !self.ships[k].pending_destroyed.is_empty() {
            // Every chain first, so every part it destroys leaves its fragment too (a no-op after
            // `resolve_breakage` set them off; parts a blast destroys on other ships wait for their resolution).
            self.chain_reactions();
            self.spawn_fragments(k);
        }
        let from = self.events.len();
        self.ships[k].resolve_breakage(&mut self.ids, &mut self.events);
        if self.events.len() > from {
            self.sync_bodies(k, from);
            self.after_breakage(k, from);
        }
    }

    /// Vessel bookkeeping after ship `k`'s breakage events `from..`: a lost core or a lost vessel wakes the
    /// mind pass, cored pieces of a split become vessels.
    fn after_breakage(&mut self, k: usize, from: usize) {
        let mut lost = false;
        for e in &self.events[from..] {
            match *e {
                SimEvent::PartDestroyed { part, .. }
                    if self.ships[k].design.parts[part as usize].logic =>
                {
                    self.mind_dirty = true;
                }
                SimEvent::ControlLost { .. } => lost = true,
                _ => {}
            }
        }
        if lost {
            // Salvage is never dormant (the mind pass looks at vessels only).
            self.ships[k].dormant = false;
            let s = &self.ships[k];
            self.events.push(SimEvent::VesselLost {
                ship: s.id(),
                owner: s.owner(),
            });
            self.mind_dirty = true;
        }
        self.form_vessels(k);
        self.shed_cargo(k, from);
    }

    /// Mirrors new breakage events of ship `k` into the physics and starts split ghosting: the pieces ghost each
    /// other for `split_ghost_ticks` physics steps from the next one (this tick's, or the next tick's when this
    /// tick has stepped already: a split from the impacts phase).
    fn sync_bodies(&mut self, k: usize, from: usize) {
        let next_step = self.tick + self.stepped as u64;
        let ghost_until = next_step + self.config.split_ghost_ticks as u64;
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
                            self.physics.set_ghost(key, Some(sid));
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

    /// First alive part hit by a ray (unit `dir`), ignoring ship `exclude` (e.g. the shooter); the hit rule of
    /// shots (`Sim::cast_part`: parts destroyed since the last breakage resolution are passed through).
    pub fn raycast(
        &self,
        origin: Vec2,
        dir: Vec2,
        max_distance: f32,
        exclude: Option<ShipId>,
    ) -> Option<PartHit> {
        let h = self.cast_part(origin, dir, max_distance, |key| {
            exclude.is_some_and(|s| key.ship == s)
        })?;
        Some(PartHit {
            ship: h.hit.key.ship,
            piece: h.hit.key.piece,
            part: h.part,
            hit: h.hit,
        })
    }

    /// Advances the world by one fixed step of [`crate::DT`].
    pub fn step(&mut self, inputs: &TickInputs) {
        self.step_profiled(inputs, |_| {});
    }

    /// [`Sim::step`] that calls `mark` at the start of every [`StepPhase`] (and with `Done` at the end), so
    /// a tool can time the phases with its own clock. `mark` sees nothing of the state and changes nothing.
    pub fn step_profiled(&mut self, inputs: &TickInputs, mut mark: impl FnMut(StepPhase)) {
        mark(StepPhase::Inputs);
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
        mark(StepPhase::Flight);
        behavior::think_all(
            &mut self.ships,
            &self.behaviors,
            &mut self.behavior_scratch,
            self.tick,
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

        mark(StepPhase::Systems);
        for s in &mut self.ships {
            s.update_systems();
        }

        // Weapons (shields, projectiles, guns) and engine plumes, then whatever they broke; then salvage.
        mark(StepPhase::Weapons);
        self.weapons_step();
        self.resolve_breakage();
        self.collect();

        // Physics: ghosts expire, this tick's wrenches replace the last ones (world force at the centre of
        // mass), one Rapier step.
        mark(StepPhase::Physics);
        for s in &mut self.ships {
            let sid = s.id();
            for p in &mut s.pieces {
                let key = BodyKey {
                    ship: sid,
                    piece: p.id,
                };
                if p.ghost_until != 0 && self.tick >= p.ghost_until {
                    p.ghost_until = 0;
                    self.physics.set_ghost(key, None);
                }
                let f = if p.wrench.force != Vec2::ZERO {
                    Rot::from_angle(p.body.angle).apply(p.wrench.force)
                } else {
                    Vec2::ZERO
                };
                self.physics.set_wrench(key, f, p.wrench.torque);
            }
        }
        self.physics.step();
        self.drift_loose();
        self.stepped = true;

        // Contacts → impacts (needs the velocities from before the step), read-back, ramming.
        mark(StepPhase::Impacts);
        self.collect_impacts();
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
        self.ram();
        // The voyage director: sites spawn, wake and clear around the vessels as they are now.
        self.direct_voyage();

        self.stepped = false;
        self.tick += 1;
        self.events_mark = self.events.len();
        if cfg!(debug_assertions) {
            mark(StepPhase::Hash);
            self.tick_hash = Some(self.state_hash());
        }
        mark(StepPhase::Done);
    }

    /// Ship index and piece of a body.
    fn piece_of(&self, key: BodyKey) -> Option<(usize, &Piece)> {
        let k = self.ship_index(key.ship)?;
        Some((k, self.ships[k].piece(key.piece)?))
    }

    /// Turns the physics contacts of the step into [`Contact`]s: closing speed at the contact point from the
    /// velocities before the step (`Piece::body` still holds them) about the centres of mass after it, the
    /// part under the point on each side, and the ramming damage. Call before the bodies are read back.
    fn collect_impacts(&mut self) {
        let t = self.config.physics;
        let mut impacts = std::mem::take(&mut self.impacts);
        impacts.clear();
        for rc in &self.physics.contacts {
            let (Some((ka, pa)), Some((kb, pb))) = (self.piece_of(rc.a), self.piece_of(rc.b))
            else {
                continue;
            };
            let (Some(ba), Some(bb)) = (self.physics.body(rc.a), self.physics.body(rc.b)) else {
                continue;
            };
            let velocity = |p: &Piece, after: &Body| {
                let r = rc.point - after.to_world(p.mass.com);
                p.body.linvel + Vec2::cross_scalar(p.body.angvel, r)
            };
            let closing = (velocity(pa, &ba) - velocity(pb, &bb)).dot(rc.normal);
            // The normal points out of `a` and into `b`.
            let out_a = Rot::from_angle(ba.angle).apply_inverse(rc.normal);
            let out_b = Rot::from_angle(bb.angle).apply_inverse(rc.normal * -1.0);
            // Vessels of one owner bump without ram damage (GDR-0045); salvage is nobody's friend.
            let friendly = vessel::friendly(
                self.ships[ka].piece_team(rc.a.piece),
                self.ships[kb].piece_team(rc.b.piece),
            );
            let damage = if rc.impulse > t.ram_threshold && !friendly {
                (rc.impulse - t.ram_threshold) * t.ram_damage
            } else {
                0.0
            };
            impacts.push(Contact {
                a: rc.a,
                b: rc.b,
                point: rc.point,
                normal: rc.normal,
                impulse: rc.impulse,
                relative_speed: closing,
                part_a: contact_part(
                    &self.ships[ka],
                    pa,
                    [rc.local_a, rc.strongest_a],
                    out_a,
                    rc.sub_a,
                ),
                part_b: contact_part(
                    &self.ships[kb],
                    pb,
                    [rc.local_b, rc.strongest_b],
                    out_b,
                    rc.sub_b,
                ),
                damage,
            });
        }
        self.impacts = impacts;
    }

    /// Reports this tick's impacts and deals their ramming damage to each side (equal on both: the same
    /// impulse acts on both hulls, so armour and hit points decide who breaks): the shield bubbles covering
    /// the contact point take their kinetic share first (GDR-0045), the rest crushes inward from the touching
    /// part (GDR-0047). Then resolves what broke.
    fn ram(&mut self) {
        let mut rammed = false;
        for n in 0..self.impacts.len() {
            let c = self.impacts[n];
            self.events.push(SimEvent::Impact(c));
            if c.damage <= 0.0 {
                continue;
            }
            // The normal points out of `a` into `b`: inward is against it for `a`, along it for `b`.
            for (key, part, by, inward) in [
                (c.a, c.part_a, c.b, c.normal * -1.0),
                (c.b, c.part_b, c.a, c.normal),
            ] {
                if let (Some(part), Some(k)) = (part, self.ship_index(key.ship)) {
                    let hull = self.ram_shields(by, c.point, c.damage);
                    rammed |= self.ram_crush(k, key, part, c.point, inward, hull);
                }
            }
        }
        if rammed {
            self.resolve_breakage();
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

/// The part of `piece` under a contact (piece frame), looked up with `Ship::part_at_local` at the contact's
/// centre `centre`, then at the centre of its strongest manifold `strongest`, each moved [`CONTACT_NUDGE`]
/// into the hull (against `outward`, the contact normal in the piece frame). If both miss (corners), the
/// strongest manifold's sub-shape decides: a slope triangle names its part, a merged rectangle is looked up
/// at the point clamped into it.
fn contact_part(
    ship: &Ship,
    piece: &Piece,
    [centre, strongest]: [Vec2; 2],
    outward: Vec2,
    sub: u32,
) -> Option<PartIdx> {
    for point in [centre, strongest] {
        if let Some(p) = ship.part_at_local(piece.id, point - outward * CONTACT_NUDGE) {
            return Some(p);
        }
    }
    let poly = piece.colliders.get(sub as usize)?;
    if poly.part.is_some() {
        return poly.part;
    }
    let (min, max) = (poly.points()[0], poly.points()[2]);
    let e = 1e-3;
    let q = Vec2::new(
        strongest.x.clamp(min.x + e, max.x - e),
        strongest.y.clamp(min.y + e, max.y - e),
    );
    ship.part_at_local(piece.id, q)
}

impl StateHash for Sim {
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
        self.hash_minds(h);
        h.u32(self.ships.len() as u32);
        for s in &self.ships {
            s.hash_state(h);
        }
        h.u32(self.projectiles.len() as u32);
        for p in &self.projectiles {
            p.hash_state(h);
        }
        self.salvage.hash_state(h);
        if let Some(v) = &self.voyage {
            v.hash_state(h);
        }
        self.physics.hash_state(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Split pieces ghost each other for `split_ghost_ticks` physics steps from the next one, also when the split
    /// is resolved after this tick's step (a ram; 13 Oct 2026 review: those ghosted one step less).
    #[test]
    fn split_ghosts_last_the_configured_steps_from_the_next_physics_step() {
        let content = data::base_content();
        let raider = content.ships.get("automaton_raider").unwrap();
        let d = Arc::new(ShipDesign::compile(&content, raider).unwrap());
        for stepped in [false, true] {
            let mut sim = Sim::new(1);
            let s = sim.spawn_ship(d.clone(), Owner(1), Body::default());
            sim.step(&TickInputs::default());
            sim.stepped = stepped;
            for cell in [(1, 0), (1, 1), (1, 2)] {
                sim.apply_damage(s, DamageTarget::Cell(cell), 100.0);
            }
            sim.resolve_breakage();
            let next = sim.tick + stepped as u64;
            let pieces = sim.ship(s).unwrap().pieces();
            assert_eq!(pieces.len(), 2);
            for p in pieces {
                assert_eq!(p.ghost_until, next + sim.config.split_ghost_ticks as u64);
            }
        }
    }
}
