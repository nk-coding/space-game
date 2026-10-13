//! The vessel model (GDR-0013, GDR-0038, GDR-0044, GDR-0045; `docs/sim.md` "Vessels").
//!
//! * **Owners.** Every ship has an [`Owner`] (a player or an AI side; spawning takes it). It counts for game
//!   rules only while the ship is a vessel ([`Ship::team`]): pieces without a working logic core are
//!   salvage, friendly to nobody ([`friendly`]).
//! * **Vessels.** A ship holds at most one vessel: its controlling piece, which has the ship's logic cores.
//!   When a split leaves another piece with a logic core, that piece becomes a ship of its own (a new
//!   [`ShipId`], the same [`PieceId`], the same owner) that inherits the parent's behavior; its rule slots
//!   come from its own cores (GDR-0035). Every other piece of a split is salvage of the parent ship. A vessel
//!   whose cores are unpowered or overheated stays a vessel, dark until they work again.
//! * **Focus.** Per owner at most one vessel is piloted ([`Sim::focus`], set by `ShipCommand::Focus`):
//!   its behavior is parked while it is the focus, every other vessel runs its behavior. When the focus is
//!   lost (no logic core left, or removed), the focus moves to the owner's best other vessel.
//! * **Mind capacity.** Per owner `SimConfig::mind_capacity` logic cores run at once. Vessels claim it in
//!   order: the focus first, then by claim key (fresh claims from `ShipCommand::Wake` or a focus that just
//!   left come first, most recent first; then the oldest vessels); a vessel whose cores do not fit any more is
//!   **dormant**: dark like salvage (no behavior, no thrust, no power, no shields), still owned.
//!
//! Bookkeeping cost: everything here runs only when something changed (a vessel formed or was lost, a core
//! was destroyed, a focus or wake command, a config change): one pass over the ships and a sort of the
//! vessels in a reused buffer.

use std::sync::Arc;

use crate::behavior::BehaviorState;
use crate::control::ControlState;
use crate::design::ShipDesign;
use crate::events::SimEvent;
use crate::flight::{FlightTelemetry, HelmMode, PilotInput};
use crate::hash::StateHasher;
use crate::ids::{PieceId, ShipId};
use crate::maneuver::Maneuver;
use crate::physics::BodyKey;
use crate::ship::{Scratch, Ship};
use crate::world::Sim;

/// Owners the config holds settings for (`SimConfig::mind_capacity`); owners beyond have no mind limit.
pub const MAX_OWNERS: usize = 8;

/// Who a ship belongs to: a player or an AI side. Vessels of one owner are friendly: they never target each
/// other and their rams deal no damage (GDR-0045). For now an owner is also its team; co-op players on one
/// team would get their own owners and a team table behind [`friendly`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Owner(pub u8);

impl Owner {
    /// The (first) player, by convention: `SimConfig::default` gives it the player's mind capacity.
    pub const PLAYER: Owner = Owner(0);
}

/// Are two sides friendly? Salvage (`None`) is friendly to nobody (GDR-0046 lead default).
pub fn friendly(a: Option<Owner>, b: Option<Owner>) -> bool {
    a.is_some() && a == b
}

/// What a vessel is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VesselState {
    /// Its systems run: not dormant, a working logic core, power.
    Running,
    /// Not dormant, but its logic cores are unpowered or overheated: dark until they work again.
    Dark,
    /// Over its owner's mind capacity: dark like salvage, still owned.
    Dormant,
}

/// One vessel for a fleet bar ([`Sim::vessels`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VesselInfo {
    pub ship: ShipId,
    /// Its piece (the ship's controlling piece).
    pub piece: PieceId,
    pub owner: Owner,
    pub state: VesselState,
    /// The owner's focus (piloted).
    pub focus: bool,
    /// Alive logic cores: what the vessel takes of its owner's mind capacity.
    pub cores: u32,
    /// Control load and capacity (capacity 0 while dormant).
    pub control: ControlState,
    /// Tick it formed (spawned or split off).
    pub formed: u64,
}

/// One vessel in the mind pass (reused buffer).
#[derive(Clone, Copy, Debug)]
pub(crate) struct MindRow {
    owner: Owner,
    /// Not the focus (the focus sorts first).
    later: bool,
    claim: i64,
    k: usize,
    cores: u32,
}

impl Ship {
    /// Who the ship belongs to (also for salvage; game rules use [`Ship::team`]).
    pub fn owner(&self) -> Owner {
        self.owner
    }

    /// The owner while the ship is a vessel, else `None` (salvage has no owner for game rules).
    pub fn team(&self) -> Option<Owner> {
        self.control.map(|_| self.owner)
    }

    /// The owner of one of the ship's pieces for game rules: only the vessel's piece has one.
    pub fn piece_team(&self, piece: PieceId) -> Option<Owner> {
        (self.control == Some(piece)).then_some(self.owner)
    }

    /// Does the ship hold a vessel (a controlling piece with a logic core)?
    pub fn is_vessel(&self) -> bool {
        self.control.is_some()
    }

    /// Over its owner's mind capacity: dark, still owned.
    pub fn is_dormant(&self) -> bool {
        self.dormant
    }

    /// Running, dark or dormant; `None` for salvage.
    pub fn vessel_state(&self) -> Option<VesselState> {
        let p = self.piece(self.control?)?;
        Some(if self.dormant {
            VesselState::Dormant
        } else if p.active {
            VesselState::Running
        } else {
            VesselState::Dark
        })
    }

    /// Alive logic cores of the vessel (0 for salvage): what it takes of the owner's mind capacity.
    pub fn core_count(&self) -> u32 {
        self.control.and_then(|c| self.piece(c)).map_or(0, |p| {
            p.cores()
                .iter()
                .filter(|&&c| self.parts[c as usize].alive)
                .count() as u32
        })
    }

    /// The vessel's control load and capacity (`None` for salvage).
    pub fn control(&self) -> Option<ControlState> {
        self.piece(self.control?).map(|p| p.control)
    }

    /// Tick the vessel formed (spawned, or split off another vessel).
    pub fn formed(&self) -> u64 {
        self.formed
    }

    /// The behavior a focused vessel runs again when the focus leaves it.
    pub fn parked_behavior(&self) -> Option<&BehaviorState> {
        self.parked.as_ref()
    }

    /// Dormant or awake. Waking marks the vessel active until the next systems update decides.
    fn set_dormant(&mut self, dormant: bool) {
        self.dormant = dormant;
        if let Some(c) = self.control
            && let Some(p) = self.piece_mut(c)
        {
            p.active = !dormant && p.has_core;
        }
    }

    /// Moves piece `j` (not the controlling piece) out of this ship into a new ship `id` that controls it:
    /// the vessel model's split (GDR-0013). Part states, settings, controls, helm and leader are copied (the
    /// cargo stays with the ship that keeps the controlling piece);
    /// each ship keeps only its own parts alive and in its grid (pieces never link to each other, so no
    /// exposure, bonus or clearance changes). Behavior, claim and age are the caller's.
    pub(crate) fn detach(&mut self, j: usize, id: ShipId) -> Ship {
        let mut piece = self.pieces.remove(j);
        let pid = piece.id;
        let mut t = Ship {
            id,
            design: Arc::clone(&self.design),
            grid: self.grid.clone(),
            parts: self.parts.clone(),
            stats: self.stats.clone(),
            heat_k: self.heat_k.clone(),
            pieces: Vec::new(),
            controls: self.controls.clone(),
            power_settings: self.power_settings.clone(),
            scratch: Scratch::default(),
            pending_destroyed: Vec::new(),
            control: Some(pid),
            helm: self.helm,
            behavior: None,
            leader: self.leader,
            telemetry: FlightTelemetry::default(),
            thrust_scale: self.thrust_scale,
            torque_scale: self.torque_scale,
            speed_scale: self.speed_scale,
            gun_cooldown: self.gun_cooldown.clone(),
            owner: self.owner,
            dormant: false,
            claim: 0,
            formed: 0,
            parked: None,
            cargo: crate::salvage::Cargo::default(),
            burned_out: false,
        };
        for i in 0..self.parts.len() {
            if !self.parts[i].alive {
                continue;
            }
            if self.parts[i].piece == pid {
                self.parts[i].alive = false;
                self.grid.remove(i);
            } else {
                t.parts[i].alive = false;
                t.grid.remove(i);
            }
        }
        // The piece's caches (mass, colliders, power lists, thrusters, guns, control load) were built for
        // this piece already and stay valid; only control changes.
        piece.controlled = true;
        piece.active = piece.has_core;
        piece.control_dirty = true;
        t.pieces.push(piece);
        t
    }
}

impl Sim {
    /// The vessel an owner pilots, if any.
    pub fn focus(&self, owner: Owner) -> Option<ShipId> {
        self.focus
            .iter()
            .find(|(o, _)| *o == owner)
            .map(|&(_, s)| s)
    }

    /// The owner's vessels, ascending ship id (no allocation).
    pub fn vessels(&self, owner: Owner) -> impl Iterator<Item = VesselInfo> + '_ {
        let focus = self.focus(owner);
        self.ships
            .iter()
            .filter(move |s| s.owner == owner)
            .filter_map(move |s| self.info(s, focus))
    }

    /// One vessel's state (`None` for unknown ships and salvage).
    pub fn vessel(&self, ship: ShipId) -> Option<VesselInfo> {
        let s = self.ship(ship)?;
        self.info(s, self.focus(s.owner))
    }

    fn info(&self, s: &Ship, focus: Option<ShipId>) -> Option<VesselInfo> {
        let piece = s.piece(s.control?)?;
        Some(VesselInfo {
            ship: s.id,
            piece: piece.id,
            owner: s.owner,
            state: s.vessel_state()?,
            focus: focus == Some(s.id),
            cores: s.core_count(),
            control: piece.control,
            formed: s.formed,
        })
    }

    /// `(cores running, mind capacity)` of an owner: the logic cores of its vessels that are not dormant.
    pub fn mind_load(&self, owner: Owner) -> (u32, u32) {
        let running = self
            .ships
            .iter()
            .filter(|s| s.owner == owner && s.is_vessel() && !s.dormant)
            .map(|s| s.core_count())
            .sum();
        (running, self.config().mind_capacity(owner))
    }

    /// The owner of a body for game rules: the vessel's piece has its ship's owner, salvage none.
    pub fn piece_team(&self, key: BodyKey) -> Option<Owner> {
        self.ship(key.ship)?.piece_team(key.piece)
    }

    fn next_claim(&mut self) -> i64 {
        self.mind_seq += 1;
        self.mind_seq
    }

    /// A fresh content default behavior for a design (the unpiloted ship-mind default is `skirmisher`).
    fn default_behavior(&self, design: &ShipDesign) -> Option<BehaviorState> {
        let id = self.behaviors().default_for(design)?;
        let program = self.behaviors().get(id)?;
        Some(BehaviorState::new(id, Arc::clone(program)))
    }

    /// Ship `k` just became a vessel: newest claim, age, event.
    pub(crate) fn vessel_formed(&mut self, k: usize, from: Option<(ShipId, PieceId)>) {
        let claim = self.next_claim();
        let tick = self.tick();
        let s = &mut self.ships[k];
        s.claim = claim;
        s.formed = tick;
        let (ship, owner) = (s.id, s.owner);
        self.events
            .push(SimEvent::VesselFormed { ship, owner, from });
        self.mind_dirty = true;
    }

    /// Every piece of ship `k` other than its controlling one that has a logic core becomes a vessel of
    /// its own (after a split, or a blueprint built in several pieces). It inherits the parent's behavior (a
    /// copy of the running one, or of the one the focus parked; a split-off piece of an unpiloted parent
    /// without a behavior keeps the parent's helm). A piece split off the focus gets the content default
    /// behavior if the parent had none parked, and never the pilot's input. A wreck spawned as a wreck
    /// (`Ship::burned_out`) forms no vessels: its cores stay salvage.
    pub(crate) fn form_vessels(&mut self, k: usize) {
        if self.ships[k].burned_out {
            return;
        }
        let mut j = 0;
        while j < self.ships[k].pieces.len() {
            let p = &self.ships[k].pieces[j];
            if p.controlled || !p.has_core {
                j += 1;
                continue;
            }
            let pid = p.id;
            let parent = self.ships[k].id;
            let id = self.ids.ship();
            let mut t = self.ships[k].detach(j, id);
            let s = &self.ships[k];
            let piloted = self.focus(s.owner) == Some(parent);
            t.behavior = s
                .behavior
                .as_ref()
                .or(s.parked.as_ref())
                .map(|b| b.inherit());
            if piloted {
                if t.behavior.is_none() {
                    t.behavior = self.default_behavior(&t.design);
                }
                t.helm = HelmMode::Maneuver {
                    maneuver: Maneuver::Brake,
                    fire: false,
                };
            }
            self.physics.rekey(
                BodyKey {
                    ship: parent,
                    piece: pid,
                },
                BodyKey {
                    ship: id,
                    piece: pid,
                },
            );
            self.ships.push(t);
            let nk = self.ships.len() - 1;
            self.vessel_formed(nk, Some((parent, pid)));
        }
    }

    /// Makes `ship` its owner's focus (`ShipCommand::Focus`; ignored for salvage). The vessel the focus
    /// leaves gets its behavior back (its content default if it had none) and a fresh claim, so it stays
    /// awake; the new focus parks its behavior, is piloted from now on and never goes dormant.
    pub(crate) fn set_focus(&mut self, ship: ShipId) {
        let Some(k) = self.ship_index(ship) else {
            return;
        };
        let Some(owner) = self.ships[k].team() else {
            return;
        };
        let old = self.focus(owner);
        if old == Some(ship) {
            return;
        }
        if let Some(ko) = old.and_then(|o| self.ship_index(o)) {
            self.release_focus(ko);
        }
        self.take_focus(k);
        match self.focus.iter_mut().find(|(o, _)| *o == owner) {
            Some(e) => e.1 = ship,
            None => {
                self.focus.push((owner, ship));
                self.focus.sort_unstable_by_key(|e| e.0);
            }
        }
        self.events.push(SimEvent::FocusChanged {
            owner,
            from: old,
            to: Some(ship),
        });
        self.mind_dirty = true;
        self.update_minds();
    }

    /// `ShipCommand::Wake`: the vessel claims mind capacity before every vessel that has not been woken
    /// since (the newest other vessels go dormant to make room). Ignored for salvage.
    pub(crate) fn wake(&mut self, ship: ShipId) {
        let Some(k) = self.ship_index(ship) else {
            return;
        };
        if !self.ships[k].is_vessel() {
            return;
        }
        let claim = -self.next_claim();
        self.ships[k].claim = claim;
        self.mind_dirty = true;
        self.update_minds();
    }

    fn take_focus(&mut self, k: usize) {
        let s = &mut self.ships[k];
        if s.parked.is_none() {
            s.parked = s.behavior.take();
        }
        s.behavior = None;
        s.helm = HelmMode::Pilot(PilotInput::default());
    }

    fn release_focus(&mut self, k: usize) {
        let claim = -self.next_claim();
        let fallback = match self.ships[k].parked.is_none() && self.ships[k].behavior.is_none() {
            true => self.default_behavior(&self.ships[k].design),
            false => None,
        };
        let s = &mut self.ships[k];
        s.claim = claim;
        if s.behavior.is_none() {
            s.behavior = s.parked.take().or(fallback);
        }
        s.parked = None;
        if s.behavior.is_none() {
            s.helm = HelmMode::Maneuver {
                maneuver: Maneuver::Brake,
                fire: false,
            };
        }
    }

    /// The owner's best vessel to take over a lost focus: running before dark before dormant, then the
    /// heaviest vessel piece, then the lowest ship id.
    fn fallback_focus(&self, owner: Owner) -> Option<usize> {
        let rank = |s: &Ship| match s.vessel_state() {
            Some(VesselState::Running) => 0u8,
            Some(VesselState::Dark) => 1,
            _ => 2,
        };
        let mass = |s: &Ship| {
            s.control
                .and_then(|c| s.piece(c))
                .map_or(0.0, |p| p.mass.mass)
        };
        let mut best: Option<usize> = None;
        for (k, s) in self.ships.iter().enumerate() {
            if s.team() != Some(owner) {
                continue;
            }
            best = match best {
                Some(b) => {
                    let o = &self.ships[b];
                    let better = rank(s) < rank(o) || (rank(s) == rank(o) && mass(s) > mass(o));
                    Some(if better { k } else { b })
                }
                None => Some(k),
            };
        }
        best
    }

    /// The mind pass, when something changed: a lost focus moves on, then every owner's vessels claim its
    /// mind capacity in order (focus first, then by claim); vessels that do not fit go dormant, vessels
    /// that fit again wake. Emits `FocusChanged`, `VesselDormant` and `VesselWoken`.
    pub(crate) fn update_minds(&mut self) {
        if !self.mind_dirty {
            return;
        }
        self.mind_dirty = false;

        // A focus that is no longer a vessel of its owner moves to the owner's best other vessel.
        let mut n = 0;
        while n < self.focus.len() {
            let (owner, f) = self.focus[n];
            if self.ship(f).is_some_and(|s| s.team() == Some(owner)) {
                n += 1;
                continue;
            }
            let next = self.fallback_focus(owner);
            let to = next.map(|k| self.ships[k].id);
            match next {
                Some(k) => {
                    self.take_focus(k);
                    self.focus[n].1 = self.ships[k].id;
                    n += 1;
                }
                None => {
                    self.focus.remove(n);
                }
            }
            self.events.push(SimEvent::FocusChanged {
                owner,
                from: Some(f),
                to,
            });
        }

        let mut rows = std::mem::take(&mut self.mind_scratch);
        rows.clear();
        for (k, s) in self.ships.iter().enumerate() {
            if !s.is_vessel() {
                continue;
            }
            rows.push(MindRow {
                owner: s.owner,
                later: self.focus(s.owner) != Some(s.id),
                claim: s.claim,
                k,
                cores: s.core_count(),
            });
        }
        rows.sort_unstable_by_key(|r| (r.owner, r.later, r.claim, r.k));
        let mut owner = None;
        let mut left = 0u32;
        for r in &rows {
            if owner != Some(r.owner) {
                owner = Some(r.owner);
                left = self.config().mind_capacity(r.owner);
            }
            let run = !r.later || r.cores <= left;
            if run {
                left = left.saturating_sub(r.cores);
            }
            let s = &mut self.ships[r.k];
            if s.dormant == run {
                s.set_dormant(!run);
                let (ship, owner) = (s.id, s.owner);
                self.events.push(if run {
                    SimEvent::VesselWoken { ship, owner }
                } else {
                    SimEvent::VesselDormant { ship, owner }
                });
            }
        }
        self.mind_scratch = rows;
    }

    /// Hashes the mind state (focus per owner, claim counter).
    pub(crate) fn hash_minds(&self, h: &mut StateHasher) {
        h.u32(self.focus.len() as u32);
        for &(o, s) in &self.focus {
            h.u8(o.0);
            h.u32(s.0);
        }
        h.u64(self.mind_seq as u64);
    }
}
