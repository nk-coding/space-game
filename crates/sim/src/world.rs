//! The simulation world: tick counter, RNG streams, ships, events and the state hash.
//!
//! # One tick (`Sim::step`)
//!
//! 1. Apply inputs: controls replace each named ship's controls; scripted damage is applied.
//! 2. Resolve breakage (connectivity, splits) → events.
//! 3. Ship systems: power → heat → engine wrench and gun output per piece.
//! 4. *(physics layer, later: apply each piece's wrench, step the engine, write bodies back, turn contacts
//!    and ray hits into damage, resolve breakage again.)*
//! 5. `tick += 1`; in debug builds the state hash of the new state is stored.

use std::sync::Arc;

use crate::breakage::{DamageOutcome, DamageTarget};
use crate::design::ShipDesign;
use crate::events::SimEvent;
use crate::hash::{StateHash, StateHasher};
use crate::ids::{IdAlloc, ShipId};
use crate::rng::{Pcg32, RngStreams, Stream};
use crate::ship::{Body, Ship, ShipControls};

/// Scripted damage (tests, scenarios, replays).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageCommand {
    pub ship: ShipId,
    pub target: DamageTarget,
    pub amount: f32,
}

/// Everything from outside that affects one tick. Applied in list order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickInputs {
    /// New controls for these ships (others keep theirs).
    pub controls: Vec<(ShipId, ShipControls)>,
    pub damage: Vec<DamageCommand>,
}

/// The deterministic world. Same seed + same inputs = same state hash every tick, on every platform.
#[derive(Clone, Debug)]
pub struct Sim {
    tick: u64,
    seed: u64,
    rng: RngStreams,
    ids: IdAlloc,
    /// Ascending id.
    ships: Vec<Ship>,
    events: Vec<SimEvent>,
    tick_hash: Option<u64>,
}

impl Sim {
    pub fn new(seed: u64) -> Sim {
        Sim {
            tick: 0,
            seed,
            rng: RngStreams::new(seed),
            ids: IdAlloc::default(),
            ships: Vec::new(),
            events: Vec::new(),
            tick_hash: None,
        }
    }

    /// Number of completed ticks.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Simulated time in seconds (`tick × DT`, computed in f64 for display; never fed back into the sim).
    pub fn time_seconds(&self) -> f64 {
        self.tick as f64 / crate::TICK_HZ as f64
    }

    /// The random stream of one system.
    pub fn rng(&mut self, s: Stream) -> &mut Pcg32 {
        self.rng.get(s)
    }

    /// Spawns a ship. `body` is the pose and velocity of the blueprint's main piece.
    pub fn spawn_ship(&mut self, design: Arc<ShipDesign>, body: Body) -> ShipId {
        let id = self.ids.ship();
        let ship = Ship::new(id, design, body, &mut self.ids);
        self.ships.push(ship);
        id
    }

    /// Removes a ship and all its pieces.
    pub fn remove_ship(&mut self, id: ShipId) -> Option<Ship> {
        let k = self.ship_index(id)?;
        Some(self.ships.remove(k))
    }

    pub fn ships(&self) -> &[Ship] {
        &self.ships
    }

    /// Mutable ships, e.g. for the physics layer's body sync-back.
    pub fn ships_mut(&mut self) -> &mut [Ship] {
        &mut self.ships
    }

    pub fn ship(&self, id: ShipId) -> Option<&Ship> {
        self.ship_index(id).map(|k| &self.ships[k])
    }

    pub fn ship_mut(&mut self, id: ShipId) -> Option<&mut Ship> {
        self.ship_index(id).map(move |k| &mut self.ships[k])
    }

    fn ship_index(&self, id: ShipId) -> Option<usize> {
        self.ships.binary_search_by_key(&id, |s| s.id()).ok()
    }

    /// Events of the last step (plus those of `resolve_breakage` calls since). Cleared when a step starts.
    pub fn events(&self) -> &[SimEvent] {
        &self.events
    }

    /// Applies damage now. Splits are resolved in the next step or by [`Sim::resolve_breakage`].
    pub fn apply_damage(&mut self, ship: ShipId, target: DamageTarget, amount: f32) -> DamageOutcome {
        match self.ship_mut(ship) {
            Some(s) => s.apply_damage(target, amount),
            None => DamageOutcome::Ignored,
        }
    }

    /// Resolves breakage of all ships now (ascending ship id) and appends the events.
    pub fn resolve_breakage(&mut self) {
        for s in &mut self.ships {
            s.resolve_breakage(&mut self.ids, &mut self.events);
        }
    }

    /// Advances the world by one fixed step of [`crate::DT`].
    pub fn step(&mut self, inputs: &TickInputs) {
        self.events.clear();
        for (id, c) in &inputs.controls {
            if let Some(s) = self.ship_mut(*id) {
                s.set_controls(c);
            }
        }
        for d in &inputs.damage {
            self.apply_damage(d.ship, d.target, d.amount);
        }
        self.resolve_breakage();
        for s in &mut self.ships {
            s.update_systems();
        }
        self.tick += 1;
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

impl StateHash for Sim {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.tick);
        h.u64(self.seed);
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
    }
}
