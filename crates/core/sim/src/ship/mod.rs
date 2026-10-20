//! The runtime ship: per-part state, pieces (one rigid body each) and the per-tick systems (power, heat,
//! engines, guns). Damage and breakage live in `breakage.rs`.
//!
//! # Cost model
//!
//! Everything that depends only on topology (which parts are alive) is cached and updated only when a part
//! is destroyed: per part (exposure, bonus-adjusted stats, heat constants) only around the destroyed part;
//! per piece (mass properties, colliders, power lists, thruster table, gun mounts) once per damaged piece in
//! `resolve_breakage`. The per-tick code walks compact cached rows and allocates nothing.

mod caches;
pub mod drones;
mod living;
mod piece;
mod scan;
mod systems;

pub mod alloc;
pub mod breakage;
pub mod collider;
pub mod control;
pub mod design;
pub mod mass;
pub mod power;
pub mod vessel;

use std::sync::Arc;

use data::{Delivery, Dir, Effect, ShipGrid, WeaponFamily};

use crate::DT;
use crate::ship::alloc::{Actuation, AllocTable, TorqueSource, wrench_of};
use crate::ship::collider::{ColliderScratch, ConvexPolygon, build_colliders};
use crate::ship::design::{BonusLink, ConsumerKind, NO_SLOT, PartIdx, PartStats, ShipDesign, Stat};
use crate::flight::{FlightTelemetry, HelmMode};
use crate::hash::{StateHash, StateHasher};
use crate::ids::{IdAlloc, PieceId, ShipId};
use crate::ship::mass::{MassProps, combine, to_piece};
use crate::math::{Rot, Vec2, sin};
use crate::ship::power::{Consumer, PowerSetting, Reactor};

pub use piece::{Defence, GunMount, Piece, Relay, ThrustAuthority, Thruster, gun_works, producing};
pub use scan::{Scan, tuning as sensor_tuning};
pub(crate) use piece::{HeatK, refresh_authority};
pub(crate) use systems::{applied_bonuses, gun_blockers, power_pass, reactor_supply};

/// Heat model constants. **PLACEHOLDERS** for feel; the steady state matches the concept-board drydock: `loss · T = gen − Σ K (T − T_neighbour)`.
pub mod tuning {
    /// Heat flowing per second per shared cell edge per unit of heat difference (drydock: 1).
    pub const HEAT_CONDUCTANCE: f32 = 1.0;
    /// Loss to the void per cell of area per unit of heat (drydock: 0.02 per cell).
    pub const HEAT_LOSS_BASE: f32 = 0.02;
    /// Loss per open side of a non-radiator part (drydock: 0.3). Radiators use their `vent` instead.
    pub const HEAT_LOSS_OPEN_SIDE: f32 = 0.3;
    /// An overheated part restarts once its heat drops below this fraction of its limit (hysteresis).
    pub const OVERHEAT_RESUME: f32 = 0.75;
    /// Reactor heat at zero load as a fraction of full-load heat (drydock: 1/3); rises linearly to 1.
    /// A reactor with no demand at all makes no heat.
    pub const REACTOR_IDLE_HEAT: f32 = 1.0 / 3.0;
    /// Rated-speed falloff (GDR-0041, GDR-0055): a thruster pushing with the piece's motion keeps full thrust
    /// until the piece's whole speed reaches this share of its rated speed, then fades linearly to none at the
    /// rating. Flat first, so the acceleration stays constant for most of the way and the approach to the top
    /// speed is short.
    pub const RATED_KNEE: f32 = 0.8;
}

/// Share of a thruster's force left at `speed` m/s, for rated speed `rated` (GDR-0041): 1 up to `RATED_KNEE ×
/// rated`, then linear to 0 at `rated` and beyond. [`Thruster::falloff`] applies it to a thruster that pushes with
/// the motion (GDR-0055, weighted by the push angle since GDR-0089); braking and sideways thrusters keep full
/// thrust, and [`govern`] keeps what they add to the speed under the ship's top speed. No drag: a coasting ship
/// keeps its speed; only its own thrust stops growing. Only `+ − × ÷` and comparisons (deterministic).
pub fn rated_falloff(speed: f32, rated: f32) -> f32 {
    let knee = tuning::RATED_KNEE * rated;
    if speed <= knee {
        1.0
    } else if speed >= rated {
        0.0
    } else {
        (rated - speed) / (rated - knee)
    }
}

/// The speed governor (GDR-0089; Niklas, 15 Oct 2026: at the top speed a push in a new direction should turn the
/// ship onto it at that speed, the speed the other way falling away, instead of sometimes passing the top speed
/// and sometimes barely answering): the world force a piece's own actuators put on it this tick, `force`, cut so
/// that it never takes the piece's speed past `top` (or past its speed now, if that is higher already). What it
/// keeps turns the velocity towards the push. `vel` is the velocity at the centre of mass, `mass` the piece's.
/// Braking and anything below the top speed pass unchanged; collisions and blasts are not the piece's force and
/// are never cut.
pub fn govern(force: Vec2, vel: Vec2, mass: f32, top: f32) -> Vec2 {
    if top <= 0.0 || mass <= 0.0 || force == Vec2::ZERO {
        return force;
    }
    let next = vel + force * (DT / mass);
    let allowed = top.max(vel.length());
    let speed = next.length();
    if speed <= allowed {
        return force;
    }
    (next * (allowed / speed) - vel) * (mass / DT)
}

/// World pose and velocity of a piece's body. Written by the physics layer after each step; the sim sets it
/// for new pieces on a split.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    /// World position of the piece frame's origin (m).
    pub pos: Vec2,
    /// Counter-clockwise angle of the piece frame (rad). 0 = the ship's nose points to world +y.
    pub angle: f32,
    /// Velocity of the centre of mass (m/s, world).
    pub linvel: Vec2,
    /// Angular velocity (rad/s, counter-clockwise positive).
    pub angvel: f32,
}

impl Body {
    pub fn at(pos: Vec2, angle: f32) -> Body {
        Body {
            pos,
            angle,
            ..Body::default()
        }
    }

    /// Piece-frame point → world.
    pub fn to_world(&self, local: Vec2) -> Vec2 {
        self.pos + Rot::from_angle(self.angle).apply(local)
    }

    /// The body with every NaN or infinite number set to 0 (bodies from outside: spawns, teleports).
    pub fn finite_or_zero(self) -> Body {
        Body {
            pos: self.pos.finite_or_zero(),
            angle: crate::math::finite_or_zero(self.angle),
            linvel: self.linvel.finite_or_zero(),
            angvel: crate::math::finite_or_zero(self.angvel),
        }
    }
}

/// The velocity a new piece inherits when a rigid body breaks: the velocity of its centre of mass as a
/// point of the old body, `v_new = v + ω × (com_new − com_old)`; the angular velocity stays `ω`. All vectors
/// in the same (world) frame.
#[inline]
pub fn inherited_velocity(v: Vec2, omega: f32, com_old: Vec2, com_new: Vec2) -> Vec2 {
    v + Vec2::cross_scalar(omega, com_new - com_old)
}

/// Per-ship controls, set through `TickInputs` (later by the flight computer and behavior trees).
#[derive(Clone, Debug, PartialEq)]
pub struct ShipControls {
    /// Throttle 0..1 per engine slot (`ShipDesign::engines` order).
    pub throttle: Vec<f32>,
    /// Trigger: guns fire (all of them, unless `guns` picks some).
    pub fire: bool,
    /// Which guns fire while `fire` is on, per gun slot (`ShipDesign::guns` order). Empty = all of them (pilot
    /// and helm). Fire at will (behaviors) fills it per gun: each gun fires when the target is inside its own
    /// cone along its facing.
    pub guns: Vec<bool>,
    /// Shield emitters on.
    pub shields: bool,
}

impl ShipControls {
    pub fn idle(engines: usize) -> ShipControls {
        ShipControls {
            throttle: vec![0.0; engines],
            fire: false,
            guns: Vec::new(),
            shields: true,
        }
    }

    /// Does the gun in this slot fire? (`fire`, and `guns` lets it.)
    #[inline]
    pub fn gun_fires(&self, slot: u32) -> bool {
        self.fire && self.guns.get(slot as usize).copied().unwrap_or(true)
    }
}

/// Runtime state of one part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartState {
    pub hp: f32,
    /// Heat level, same scale as the part's heat limit.
    pub heat: f32,
    pub alive: bool,
    /// Shut down by heat until it cools below `limit × OVERHEAT_RESUME`.
    pub overheated: bool,
    pub piece: PieceId,
    /// Sides open to space (straight sides not linked to a neighbour, plus one per slope-face cell).
    /// Cached on topology changes.
    pub open_sides: u16,
    /// Guns: line of fire clear (only parts of the same piece block). True for every other part (engines may
    /// sit anywhere, GDR-0039: their plume burns what is behind them instead).
    pub clear: bool,
    /// Share of its power demand served this tick: 1 = full, between = partial output, 0 = off or no power.
    /// A consumer that asks for nothing shows the level its tier and weight are served at (see
    /// [`crate::ship::power::allocate`]); reactors show 1 (0 while idle), passive parts 1.
    pub power: f32,
    /// How hard it worked this tick, 0..1 (throttle × power for engines). Drives heat and output.
    pub work: f32,
    /// Shield emitters: what the bubble can still absorb (0 for other parts).
    pub shield: f32,
    /// Shield emitters: the bubble collapsed and stays down until it has recharged
    /// `weapons::tuning::SHIELD_RESTART` of its capacity.
    pub shield_down: bool,
    /// Idle: its vessel's control load exceeds the capacity of its running logic cores, and this part is
    /// among the lowest power priorities (see [`crate::ship::control`]). It does nothing and draws no power.
    pub idle: bool,
}

/// Power balance of a piece this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PowerState {
    /// Output of its working reactors (with bonuses).
    pub supply: f32,
    /// What its active consumers asked for.
    pub demand: f32,
    /// What they got (at most `min(supply, demand)`), handed out by tier and weight (`crate::ship::power`).
    pub served: f32,
}

/// Net force (piece frame, kN) and torque about the centre of mass (kN·m, counter-clockwise positive).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wrench {
    pub force: Vec2,
    pub torque: f32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Scratch {
    pub energy: Vec<f32>,
    pub mark: Vec<u32>,
    pub stack: Vec<PartIdx>,
    pub groups: Vec<(u32, u32)>,
    pub affected: Vec<PartIdx>,
    pub colliders: ColliderScratch,
    pub fmax: Vec<f32>,
    /// Per thruster row: its force without the rated-speed falloff (RCS couples).
    pub full: Vec<f32>,
    pub tmax: Vec<f32>,
    /// Per thruster row: the throttle of translation alone (the flight computer's planning view).
    pub translate: Vec<f32>,
    pub activity: Vec<f32>,
    /// Per consumer row: demand this tick, and the level it got.
    pub want: Vec<f32>,
    pub level: Vec<f32>,
}

/// A ship: one blueprint instance, possibly broken into several pieces.
#[derive(Clone, Debug)]
pub struct Ship {
    pub(crate) id: ShipId,
    pub(crate) design: Arc<ShipDesign>,
    /// Occupancy of the alive parts (all pieces share the blueprint frame; pieces are never linked to each
    /// other, so per-cell queries stay correct after splits).
    pub(crate) grid: ShipGrid,
    pub(crate) parts: Vec<PartState>,
    /// Effective stats (base + adjacency bonuses), cached on topology changes.
    pub(crate) stats: Vec<PartStats>,
    /// Heat constants per part, cached with `stats`.
    pub(crate) heat_k: Vec<HeatK>,
    /// Alive pieces, ascending id.
    pub(crate) pieces: Vec<Piece>,
    pub(crate) controls: ShipControls,
    /// Power tier and weight per part (consumers only; the blueprint's at spawn, changed by
    /// `set_power_setting`).
    pub(crate) power_settings: Vec<PowerSetting>,
    pub(crate) scratch: Scratch,
    /// Parts destroyed since the last breakage resolution, in the order they died.
    pub(crate) pending_destroyed: Vec<PartIdx>,
    /// How many of `pending_destroyed` have set off their chain reactions (`breakage.rs`).
    pub(crate) chained: usize,
    /// The controlling piece; `None` = the ship is dark for good (lost every core of that piece).
    pub(crate) control: Option<PieceId>,
    pub(crate) helm: HelmMode,
    /// A behavior program flies the ship (overrides the helm).
    pub(crate) behavior: Option<crate::behavior::BehaviorState>,
    /// Its design's pilot assists (GDR-0088), for while the player flies it (`behavior/assist.rs`).
    pub(crate) assists: Option<crate::behavior::AssistState>,
    /// Whose lock the `locked` target selector follows (an escort's leader).
    pub(crate) leader: Option<ShipId>,
    pub(crate) telemetry: FlightTelemetry,
    /// Global multipliers from `SimConfig` (thrust: engines and RCS; torque: gyros and cores; speed: rated
    /// speeds of engines and RCS).
    pub(crate) thrust_scale: f32,
    pub(crate) torque_scale: f32,
    pub(crate) speed_scale: f32,
    /// Seconds until each gun slot may fire again (≤ 0: ready).
    pub(crate) gun_cooldown: Vec<f32>,
    /// Who the ship belongs to (GDR-0013). Counts for game rules only while the ship is a vessel
    /// ([`Ship::team`]); salvage has no owner.
    pub(crate) owner: crate::ship::vessel::Owner,
    /// The vessel is dormant: over its owner's mind capacity (GDR-0044), dark like salvage but still owned.
    pub(crate) dormant: bool,
    /// Order in which the vessel claims its owner's mind capacity (lower first): its formation order, or a
    /// fresh negative key when it was woken or the focus left it. Set by the sim (`vessel.rs`).
    pub(crate) claim: i64,
    /// Tick the vessel formed (spawned, or split off another vessel).
    pub(crate) formed: u64,
    /// While the vessel is its owner's focus (piloted), the behavior it runs again when the focus leaves.
    pub(crate) parked: Option<crate::behavior::BehaviorState>,
    /// What the ship carries (`salvage.rs`): filled by collecting while it is a vessel, kept by its wreck.
    pub(crate) cargo: crate::combat::salvage::Cargo,
    /// Spawned as a wreck (`Sim::spawn_wreck`): its logic cores never form a vessel.
    pub(crate) burned_out: bool,
    /// Its scan underway or cooling down (`scan.rs`).
    pub(crate) scan: Scan,
    /// The intact hit points its hull share (the `hull` condition) is measured against: the whole design's for
    /// a spawned ship, the parts of its own piece's for a vessel split off another (GDR-0064).
    pub(crate) hull_base: f32,
    /// Seconds each heart (`ShipDesign::hearts`) has worked towards its next re-formed part (`living.rs`).
    pub(crate) regrowth: Vec<f32>,
    /// Each hangar's bay (`ShipDesign::hangars`, `drones.rs`, GDR-0117).
    pub(crate) bays: Vec<drones::Bay>,
    /// A drone: the carrier and hangar it belongs to.
    pub(crate) drone: Option<drones::Tie>,
    /// Each flare launcher's charges and cooldown (`ShipDesign::flares`, `combat/flare.rs`, GDR-0115).
    pub(crate) flares: Vec<crate::combat::flare::FlareSlot>,
    /// A release was asked this tick (handled and cleared in the weapons step).
    pub(crate) flare_asked: bool,
    /// Static from disruption guns and its blinding (`combat/disruption.rs`, GDR-0125).
    pub(crate) jam: crate::combat::disruption::Jam,
    /// Seconds until each blink drive (`ShipDesign::blinks`) is cool (`flight/blink.rs`, GDR-0127).
    pub(crate) blink_cool: Vec<f32>,
    /// The blink charging, if any.
    pub(crate) blink: crate::flight::blink::Blink,
    /// A blink was asked this tick (started and cleared in the blink step).
    pub(crate) blink_asked: Option<crate::flight::blink::BlinkAim>,
}

impl Ship {
    /// Builds a ship from a design. `body` is the pose of the blueprint's main piece (the one with a logic
    /// core, else the largest); the frame origin of every piece is its centre of mass. The main piece is the
    /// controlling piece if it has a core.
    pub(crate) fn new(
        id: ShipId,
        design: Arc<ShipDesign>,
        owner: crate::ship::vessel::Owner,
        body: Body,
        ids: &mut IdAlloc,
        cfg: &crate::config::SimConfig,
    ) -> Ship {
        let n = design.parts.len();
        let parts = design
            .parts
            .iter()
            .map(|s| PartState {
                hp: s.hp,
                heat: 0.0,
                alive: true,
                overheated: false,
                piece: PieceId(u32::MAX),
                open_sides: 0,
                clear: true,
                power: 1.0,
                work: 0.0,
                shield: 0.0,
                shield_down: false,
                idle: false,
            })
            .collect();
        let mut ship = Ship {
            id,
            controls: ShipControls::idle(design.engines.len()),
            grid: design.grid.clone(),
            stats: design.parts.iter().map(|s| s.base).collect(),
            heat_k: vec![HeatK::default(); n],
            parts,
            pieces: Vec::new(),
            power_settings: design.parts.iter().map(|s| s.power_setting).collect(),
            scratch: Scratch::default(),
            pending_destroyed: Vec::new(),
            chained: 0,
            control: None,
            helm: HelmMode::Raw,
            behavior: None,
            assists: None,
            leader: None,
            telemetry: FlightTelemetry::default(),
            thrust_scale: cfg.thrust_scale,
            torque_scale: cfg.torque_scale,
            speed_scale: cfg.speed_scale,
            gun_cooldown: vec![0.0; design.guns.len()],
            design,
            owner,
            dormant: false,
            claim: 0,
            formed: 0,
            parked: None,
            cargo: crate::combat::salvage::Cargo::default(),
            burned_out: false,
            scan: Scan::Ready,
            hull_base: 0.0,
            regrowth: Vec::new(),
            bays: Vec::new(),
            drone: None,
            flares: Vec::new(),
            flare_asked: false,
            jam: Default::default(),
            blink_cool: Vec::new(),
            blink: Default::default(),
            blink_asked: None,
        };
        ship.hull_base = ship.design.hp_total;
        ship.regrowth = vec![0.0; ship.design.hearts.len()];
        ship.bays = ship.design.hangars.iter().map(|h| drones::Bay::full(h.capacity)).collect();
        ship.blink_cool = vec![0.0; ship.design.blinks.len()];
        ship.flares = ship.design.flares.iter().map(|f| crate::combat::flare::FlareSlot { charges: f.spec.charges, cooldown: 0.0 }).collect();
        let all: Vec<PartIdx> = (0..n as u32).collect();
        for &p in &all {
            ship.refresh_exposure(p);
        }
        for &p in &all {
            ship.refresh_stats(p);
        }
        for &p in &all {
            ship.parts[p as usize].shield = ship.stats[p as usize].get(Stat::ShieldCapacity);
        }
        // Pieces in the blueprint's order (row-major by their first cell, as `data::ShipGrid::pieces`), so the
        // main piece is the drydock's (`ShipGrid::main_piece`, the same rule and order) and piece ids follow it.
        let mut comps = ship.components(&all);
        let d = &ship.design;
        comps.sort_by_key(|c| {
            c.iter()
                .map(|&p| (d.parts[p as usize].shape.y, d.parts[p as usize].shape.x))
                .min()
        });
        let main = ship.main_component(&comps);
        let origins: Vec<[f32; 2]> = comps.iter().map(|c| ship.com_bp(c)).collect();
        for (k, comp) in comps.into_iter().enumerate() {
            let pid = ids.piece();
            for &p in &comp {
                ship.parts[p as usize].piece = pid;
            }
            // Pose of this piece relative to the main piece.
            let d = to_piece(origins[k], origins[main]);
            let rot = Rot::from_angle(body.angle);
            let b = Body {
                pos: body.pos + rot.apply(d),
                angle: body.angle,
                linvel: inherited_velocity(body.linvel, body.angvel, Vec2::ZERO, rot.apply(d)),
                angvel: body.angvel,
            };
            ship.pieces.push(Piece::new(pid, comp, origins[k], b));
        }
        if let Some(&first) = ship.pieces.get(main).map(|p| &p.id)
            && ship.pieces[main]
                .parts
                .iter()
                .any(|&p| ship.design.parts[p as usize].logic)
        {
            ship.control = Some(first);
            ship.pieces[main].controlled = true;
        }
        for k in 0..ship.pieces.len() {
            ship.rebuild_piece(k);
        }
        // Stagger the guns' first shots over one period (deterministic, from ship id and part index), so
        // the guns of a fleet do not fire in lock-step salvos.
        for piece in &ship.pieces {
            for g in &piece.guns {
                if g.rate > 0.0 {
                    let h = crate::rng::splitmix64(((id.0 as u64) << 32) | g.part as u64);
                    let frac = (h >> 40) as f32 * (1.0 / 16_777_216.0);
                    ship.gun_cooldown[g.slot as usize] = frac / g.rate;
                }
            }
        }
        ship
    }

    pub fn id(&self) -> ShipId {
        self.id
    }

    pub fn design(&self) -> &Arc<ShipDesign> {
        &self.design
    }

    /// Occupancy grid of the alive parts (blueprint frame).
    pub fn grid(&self) -> &ShipGrid {
        &self.grid
    }

    /// The beam path of gun `slot` on piece `piece` now (GDR-0126, `data::BeamPath`): through the prisms that take
    /// its beam, past only the alive parts of that piece (a prism shot out leaves a gap the beam crosses).
    pub fn beam_path(&self, slot: u32, piece: PieceId) -> data::BeamPath {
        let d = &*self.design;
        let g = &d.guns[slot as usize];
        self.grid.beam_path(
            g.part as usize,
            |o| self.parts[o].piece == piece,
            |o| g.prisms.binary_search(&(o as PartIdx)).ok().map(|_| d.grid.shape(o).facing()),
        )
    }

    pub fn parts(&self) -> &[PartState] {
        &self.parts
    }

    pub fn part(&self, i: PartIdx) -> &PartState {
        &self.parts[i as usize]
    }

    /// Effective stats of a part after adjacency bonuses.
    pub fn stats(&self, i: PartIdx) -> &PartStats {
        &self.stats[i as usize]
    }

    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    pub fn piece(&self, id: PieceId) -> Option<&Piece> {
        self.piece_index(id).map(|k| &self.pieces[k])
    }

    pub(crate) fn piece_mut(&mut self, id: PieceId) -> Option<&mut Piece> {
        self.piece_index(id).map(move |k| &mut self.pieces[k])
    }

    /// The controlling piece (`None`: the ship is dark for good).
    pub fn control_piece(&self) -> Option<PieceId> {
        self.control
    }

    /// Is the ship flying this tick (controlling piece with a powered core)?
    pub fn is_active(&self) -> bool {
        self.control
            .and_then(|c| self.piece(c))
            .is_some_and(|p| p.active)
    }

    /// Armed (GDR-0068): the controlling piece still has a power source and a gun, or a ram prow and an engine
    /// (GDR-0109: a rammer is a weapon) (alive parts). Only losses that never come back count: a gun that
    /// overheats, idles, browns out or has its line of fire blocked keeps its ship armed (the blocker can be shot
    /// off); a piece without a reactor never fires again. One look-up, no allocation. False for salvage.
    pub fn is_armed(&self) -> bool {
        self.control.and_then(|c| self.piece(c)).is_some_and(|p| {
            !p.reactors.is_empty() && (!p.guns.is_empty() || (p.rams && p.thrusters.iter().any(|t| !t.rcs)))
        })
    }

    /// World position of part `p`'s centroid in the pose of the piece it is (or, destroyed, was last) on, and the
    /// velocity of that point of the piece (`v + ω × r`). `None` if that piece is gone. The one rule for
    /// fragments, collected parts and the behaviors' part aim.
    pub fn part_world(&self, p: PartIdx) -> Option<(Vec2, Vec2)> {
        let piece = self.piece(self.parts[p as usize].piece)?;
        let rot = Rot::from_angle(piece.body.angle);
        let local = rot.apply(to_piece(self.design.parts[p as usize].centroid, piece.origin));
        let b = &piece.body;
        let vel = inherited_velocity(b.linvel, b.angvel, rot.apply(piece.mass.com), local);
        Some((b.pos + local, vel))
    }

    pub fn helm(&self) -> &HelmMode {
        &self.helm
    }

    /// The running behavior and its debug view (active rule, why the others failed, target), if any.
    pub fn behavior(&self) -> Option<&crate::behavior::BehaviorState> {
        self.behavior.as_ref()
    }

    /// Its pilot assists and their switches for the mission (GDR-0088), if it was given a list.
    pub fn assists(&self) -> Option<&crate::behavior::AssistState> {
        self.assists.as_ref()
    }

    /// The ship whose lock this ship's `locked` target selector follows.
    pub fn leader(&self) -> Option<ShipId> {
        self.leader
    }

    /// Behavior rule slots now: the `rule_slots` of the working logic cores of the controlling piece (GDR-0035:
    /// alive, not overheated, powered; and not idle, [`producing`]); 0 while the ship is dark.
    pub fn rule_slots(&self) -> u32 {
        self.control.map_or(0, |c| self.rule_slots_of(c))
    }

    /// Rule slots of one piece's own working logic cores (0 unless the piece is active). The slot limit is
    /// per piece, so a split-off piece with its own cores can run a copy of the behavior within its slots.
    pub fn rule_slots_of(&self, piece: PieceId) -> u32 {
        let Some(p) = self.piece(piece) else {
            return 0;
        };
        if !p.active {
            return 0;
        }
        p.cores
            .iter()
            .filter(|&&c| producing(&self.parts[c as usize]))
            .map(|&c| self.design.parts[c as usize].rule_slots)
            .sum()
    }

    /// What the flight computer did last tick.
    pub fn telemetry(&self) -> &FlightTelemetry {
        &self.telemetry
    }

    pub(crate) fn piece_index(&self, id: PieceId) -> Option<usize> {
        self.pieces.binary_search_by_key(&id, |p| p.id).ok()
    }

    /// The main piece (`data::design::geometry::main_of`): a piece with a logic core beats one without, then more parts,
    /// then the lower id.
    pub fn main_piece(&self) -> Option<PieceId> {
        data::design::geometry::main_of(self.pieces.iter().map(|p| (p.has_core, p.parts.len())))
            .map(|k| self.pieces[k].id)
    }

    pub fn controls(&self) -> &ShipControls {
        &self.controls
    }

    /// Replaces the controls (the throttle list is resized to the engine count; a NaN or infinite throttle
    /// counts as 0) and switches the helm to `HelmMode::Raw` (no flight computer).
    pub fn set_controls(&mut self, c: &ShipControls) {
        self.helm = HelmMode::Raw;
        self.controls.throttle.clear();
        self.controls
            .throttle
            .extend(c.throttle.iter().map(|&t| crate::math::finite_or_zero(t)));
        self.controls
            .throttle
            .resize(self.design.engines.len(), 0.0);
        self.controls.fire = c.fire;
        self.controls.guns.clone_from(&c.guns);
        self.controls.shields = c.shields;
    }

    /// Outer hull edge of one piece in the blueprint frame (for the hull glow; chain with
    /// `data::design::geometry::hull_loops`). Salvage pieces have no glow; the client decides.
    pub fn piece_hull_edges(&self, piece: PieceId) -> Vec<data::HullSegment> {
        self.grid
            .hull_edges(|i| self.parts[i].alive && self.parts[i].piece == piece)
    }

    /// The part under a piece-frame point (for mapping physics hits to parts), checking slope triangles.
    pub fn part_at_local(&self, piece: PieceId, local: Vec2) -> Option<PartIdx> {
        let p = self.piece(piece)?;
        let bx = p.origin[0] + local.x;
        let by = p.origin[1] - local.y;
        let cell = (bx.floor() as i32, by.floor() as i32);
        let i = self.grid.occupant(cell)?;
        let st = &self.parts[i];
        if !st.alive || st.piece != piece {
            return None;
        }
        let spec = &self.design.parts[i];
        if spec.slope {
            let poly = spec.shape.polygon();
            let mut sign = 0.0f32;
            for k in 0..poly.len() {
                let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
                let c =
                    (b.x - a.x) as f32 * (by - a.y as f32) - (b.y - a.y) as f32 * (bx - a.x as f32);
                if c != 0.0 {
                    if sign != 0.0 && c.signum() != sign {
                        return None;
                    }
                    sign = c.signum();
                }
            }
        }
        Some(i as PartIdx)
    }
}

/// Unit vector of a blueprint side in the piece frame (y up).
pub fn dir_vec(d: Dir) -> Vec2 {
    match d {
        Dir::Up => Vec2::new(0.0, 1.0),
        Dir::Right => Vec2::new(1.0, 0.0),
        Dir::Down => Vec2::new(0.0, -1.0),
        Dir::Left => Vec2::new(-1.0, 0.0),
    }
}

impl StateHash for Body {
    fn hash_state(&self, h: &mut StateHasher) {
        h.vec2(self.pos);
        h.f32(self.angle);
        h.vec2(self.linvel);
        h.f32(self.angvel);
    }
}

impl StateHash for Piece {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.id.0);
        h.u32(self.parts.len() as u32);
        for &p in &self.parts {
            h.u32(p);
        }
        h.f32(self.origin[0]);
        h.f32(self.origin[1]);
        h.f32(self.mass.mass);
        h.vec2(self.mass.com);
        h.f32(self.mass.inertia);
        h.u32(self.has_core as u32 | (self.controlled as u32) << 1 | (self.active as u32) << 2);
        h.u32(self.control.load);
        h.u32(self.control.capacity);
        h.u32(self.control.idle);
        h.u64(self.ghost_until);
        self.body.hash_state(h);
        h.f32(self.power.supply);
        h.f32(self.power.demand);
        h.f32(self.power.served);
        crate::ship::power::hash_consumers(self, h);
        h.vec2(self.wrench.force);
        h.f32(self.wrench.torque);
        h.u32(self.colliders.len() as u32);
        for c in &self.colliders {
            for &p in c.points() {
                h.vec2(p);
            }
        }
        for g in &self.guns {
            h.f32(g.output);
        }
        h.u32(self.act.throttle.len() as u32);
        for i in 0..self.act.throttle.len() {
            h.f32(self.act.throttle[i]);
            h.f32(self.act.deflect[i]);
            h.f32(self.act.couple[i]);
        }
        h.u32(self.assist.ticks);
        h.bool(self.assist.on);
        h.u32(self.assist.still);
        h.i32(self.assist.long_way.into());
        for &t in &self.act.torque {
            h.f32(t);
        }
    }
}

impl StateHash for Ship {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.id.0);
        h.u32(self.controls.throttle.len() as u32);
        for &t in &self.controls.throttle {
            h.f32(t);
        }
        h.bool(self.controls.fire);
        h.u32(self.controls.guns.len() as u32);
        for &g in &self.controls.guns {
            h.bool(g);
        }
        h.bool(self.controls.shields);
        h.u32(self.control.map_or(u32::MAX, |c| c.0));
        for &c in &self.gun_cooldown {
            h.f32(c);
        }
        self.helm.hash_state(h);
        match &self.behavior {
            Some(b) => {
                h.u8(1);
                b.hash_state(h);
            }
            None => h.u8(0),
        }
        // Nothing for a ship without assists, so worlds without them keep the hashes they had before assists.
        if let Some(a) = &self.assists {
            h.u8(1);
            a.hash_state(h);
        }
        h.u32(self.leader.map_or(u32::MAX, |l| l.0));
        // Nothing while no scan is underway, so worlds without scans keep their hashes.
        if self.scan != Scan::Ready {
            self.scan.hash_state(h);
        }
        h.u8(self.owner.0);
        h.bool(self.dormant);
        h.u64(self.claim as u64);
        h.u64(self.formed);
        match &self.parked {
            Some(b) => {
                h.u8(1);
                b.hash_state(h);
            }
            None => h.u8(0),
        }
        h.bool(self.burned_out);
        h.f32(self.hull_base);
        // Nothing for a ship without hearts, so worlds without them keep their hashes.
        for &t in &self.regrowth {
            h.f32(t);
        }
        // Nothing for a ship without hangars that is no drone either, likewise.
        for b in &self.bays {
            b.hash_state(h);
        }
        if let Some(t) = &self.drone {
            h.u32(t.carrier.0);
            h.u32(t.hangar);
        }
        // Nothing for a ship without flare launchers, likewise.
        for f in &self.flares {
            f.hash_state(h);
        }
        // Nothing while it holds no static and runs no blinding, likewise (GDR-0125).
        if !self.jam.is_clear() {
            self.jam.hash_state(h);
        }
        // Nothing for a ship without blink drives, and its blink only while one charges, likewise (GDR-0127).
        for &c in &self.blink_cool {
            h.f32(c);
        }
        if self.blink != crate::flight::blink::Blink::Ready {
            self.blink.hash_state(h);
        }
        self.cargo.hash_state(h);
        h.u32(self.parts.len() as u32);
        for ps in &self.parts {
            h.u32(ps.alive as u32 | (ps.overheated as u32) << 1 | (ps.idle as u32) << 2);
            if !ps.alive {
                continue;
            }
            h.f32(ps.hp);
            h.f32(ps.heat);
            h.u32(ps.piece.0);
            h.f32(ps.power);
            h.f32(ps.work);
            h.f32(ps.shield);
            h.bool(ps.shield_down);
        }
        h.u32(self.pieces.len() as u32);
        for p in &self.pieces {
            p.hash_state(h);
        }
    }
}
