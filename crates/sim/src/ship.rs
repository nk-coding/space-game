//! The runtime ship: per-part state, pieces (one rigid body each) and the per-tick systems (power, heat,
//! engines, guns). Damage and breakage live in `breakage.rs`.
//!
//! # Cost model
//!
//! Everything that depends only on topology (which parts are alive) is cached and updated only when a part
//! is destroyed: per part (exposure, bonus-adjusted stats, heat constants) only around the destroyed part;
//! per piece (mass properties, colliders, power lists, thruster table, gun mounts) once per damaged piece in
//! `resolve_breakage`. The per-tick code walks compact cached rows and allocates nothing.

use std::sync::Arc;

use data::{Delivery, Dir, Effect, ShipGrid, WeaponFamily};

use crate::DT;
use crate::alloc::{Actuation, AllocTable, TorqueSource, wrench_of};
use crate::collider::{ColliderScratch, ConvexPolygon, build_colliders};
use crate::design::{ConsumerKind, NO_SLOT, PartIdx, PartStats, ShipDesign, Stat};
use crate::flight::{FlightTelemetry, HelmMode};
use crate::hash::{StateHash, StateHasher};
use crate::ids::{IdAlloc, PieceId, ShipId};
use crate::mass::{MassProps, combine, to_piece};
use crate::math::{Rot, Vec2, sin};
use crate::power::{Consumer, PowerSetting, Reactor};

/// Heat model constants. **PLACEHOLDERS** for feel; the steady state matches the concept-board drydock
/// (`docs/concept/reference/drydock-model.js`): `loss · T = gen − Σ K (T − T_neighbour)`.
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
    /// Guns: line of fire clear; engines: exhaust clear (only parts of the same piece block). True otherwise.
    pub clear: bool,
    /// Share of its power demand served this tick: 1 = full, between = partial output, 0 = off or no power.
    /// A consumer that asks for nothing shows the level its tier and weight are served at (see
    /// [`crate::power::allocate`]); reactors and passive parts show 1.
    pub power: f32,
    /// How hard it worked this tick, 0..1 (throttle × power for engines). Drives heat and output.
    pub work: f32,
    /// Shield emitters: what the bubble can still absorb (0 for other parts).
    pub shield: f32,
    /// Shield emitters: the bubble collapsed and stays down until it has recharged
    /// `weapons::tuning::SHIELD_RESTART` of its capacity.
    pub shield_down: bool,
}

/// Power balance of a piece this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PowerState {
    /// Output of its working reactors (with bonuses).
    pub supply: f32,
    /// What its active consumers asked for.
    pub demand: f32,
    /// What they got (at most `min(supply, demand)`), handed out by tier and weight (`crate::power`).
    pub served: f32,
}

/// Net force (piece frame, kN) and torque about the centre of mass (kN·m, counter-clockwise positive).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wrench {
    pub force: Vec2,
    pub torque: f32,
}

/// One row of a piece's thruster table (for the flight computer): an engine, or one nozzle of an RCS pod.
/// Piece frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thruster {
    /// Engine slot (index into `ShipControls::throttle`); `design::NO_SLOT` for RCS nozzles.
    pub slot: u32,
    pub part: PartIdx,
    /// Unit direction of the force on the ship (towards the engine's facing side; the exhaust goes the other
    /// way).
    pub dir: Vec2,
    /// Application point (the part's centre; the thrust line runs through it).
    pub point: Vec2,
    /// `point − com`.
    pub arm: Vec2,
    /// Torque per kN of thrust: `arm × dir`.
    pub torque_per_force: f32,
    /// Full-throttle thrust with bonuses (kN), before power and heat.
    pub max_force: f32,
    /// Exhaust clear. Blocked engines stay in the table with `clear == false` and produce nothing.
    pub clear: bool,
    /// `sin` of the max gimbal angle: lateral force up to `gimbal × thrust` (0 = fixed nozzle).
    pub gimbal: f32,
    /// Unit lateral direction of the gimbal force (`dir` turned 90° counter-clockwise).
    pub lat: Vec2,
    /// Torque per kN of lateral force: `arm × lat`.
    pub torque_per_lateral: f32,
    /// An RCS nozzle (else an engine).
    pub rcs: bool,
}

/// Net force and torque of the engines among `thrusters` at the given throttles (by slot), ignoring power,
/// heat, gimbals and RCS nozzles.
pub fn thruster_wrench(thrusters: &[Thruster], throttle: impl Fn(u32) -> f32) -> Wrench {
    let mut w = Wrench::default();
    for t in thrusters {
        if !t.clear || t.slot == NO_SLOT {
            continue;
        }
        let f = t.max_force * throttle(t.slot).clamp(0.0, 1.0);
        w.force += t.dir * f;
        w.torque += t.torque_per_force * f;
    }
    w
}

/// What a piece's clear thrusters can do at most, per direction (kN) and per turning sense (kN·m).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ThrustAuthority {
    /// Along +y (the ship's nose).
    pub forward: f32,
    pub backward: f32,
    /// Along +x.
    pub right: f32,
    pub left: f32,
    /// Sum of positive torques (counter-clockwise).
    pub torque_ccw: f32,
    /// Sum of negative torques, as a positive number.
    pub torque_cw: f32,
    /// Pure torque of gyros and logic cores (either sense), with the global torque scale.
    pub torque_pure: f32,
}

/// A gun of a piece. Piece frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunMount {
    pub slot: u32,
    pub part: PartIdx,
    /// Centre of the gun's facing side.
    pub muzzle: Vec2,
    /// Unit firing direction.
    pub dir: Vec2,
    pub clear: bool,
    /// Fire-rate multiplier from bonuses (1 = base).
    pub fire_rate: f32,
    /// Damage per second at full power with bonuses (`damage × fire_rate`).
    pub dps: f32,
    /// Damage per second this tick (0 unless firing, clear, powered and not overheated).
    pub output: f32,
    pub family: WeaponFamily,
    pub delivery: Delivery,
    /// Reach (m).
    pub range: f32,
    /// Shots per second at full power with bonuses (0 for beams).
    pub rate: f32,
    /// Damage per pulse or projectile (`damage / base rate`; 0 for beams).
    pub shot_damage: f32,
}

/// A connected group of parts: one rigid body.
#[derive(Clone, Debug)]
pub struct Piece {
    pub id: PieceId,
    /// Alive parts, ascending.
    pub parts: Vec<PartIdx>,
    /// Origin of the piece frame as a blueprint point (the piece's centre of mass when it was created).
    pub origin: [f32; 2],
    pub mass: MassProps,
    pub colliders: Vec<ConvexPolygon>,
    /// Has an alive logic core.
    pub has_core: bool,
    /// The ship's controlling piece (holds the controlling core). Every other piece is salvage: dark for good.
    pub controlled: bool,
    /// Controlled, with a working, powered core this tick: systems run and the hull glows. A controlled piece
    /// without power goes dark until power returns.
    pub active: bool,
    /// Pose and velocity of the body (synced from the physics backend after each step).
    pub body: Body,
    /// Pose at the end of the previous tick, for render interpolation (`render_pose`).
    pub prev_pos: Vec2,
    pub prev_angle: f32,
    pub power: PowerState,
    pub thrusters: Vec<Thruster>,
    /// Gyros and logic cores with reaction torque.
    pub torque_sources: Vec<TorqueSource>,
    pub authority: ThrustAuthority,
    pub guns: Vec<GunMount>,
    /// This tick's actuator commands (written by the flight computer or from raw controls).
    pub act: Actuation,
    /// Actual output of each thruster row this tick, 0..1 of its max force (throttle × power). Plumes.
    pub output: Vec<f32>,
    /// Force (piece frame) and torque this tick from engines, RCS, gimbals, gyros and cores, for the physics
    /// layer.
    pub wrench: Wrench,
    /// Collisions with other pieces of the same ship are off until this tick (after a split; 0 = never off).
    pub ghost_until: u64,
    pub(crate) alloc: AllocTable,
    cores: Vec<PartIdx>,
    /// Shield emitters.
    pub(crate) shields: Vec<PartIdx>,
    pub(crate) reactors: Vec<Reactor>,
    /// Consumers in allocation order (tier, weight descending, part).
    pub(crate) consumers: Vec<Consumer>,
    /// Parts that make heat without a power role (e.g. capacitors).
    pub(crate) heaters: Vec<PartIdx>,
    pub(crate) dirty: bool,
}

/// Heat constants of one part, cached when its exposure or bonuses change (compact for the per-tick loop).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct HeatK {
    /// Heat capacity.
    cap: f32,
    /// Loss coefficient to space (base + open sides × side loss or radiator vent).
    loss: f32,
    /// Heat at full work, with bonuses.
    out: f32,
    /// Shutdown level, with bonuses.
    limit: f32,
}

impl Piece {
    pub(crate) fn new(id: PieceId, parts: Vec<PartIdx>, origin: [f32; 2], body: Body) -> Piece {
        Piece {
            id,
            parts,
            origin,
            mass: MassProps::default(),
            colliders: Vec::new(),
            has_core: false,
            controlled: false,
            active: false,
            prev_pos: body.pos,
            prev_angle: body.angle,
            body,
            power: PowerState::default(),
            thrusters: Vec::new(),
            torque_sources: Vec::new(),
            authority: ThrustAuthority::default(),
            guns: Vec::new(),
            act: Actuation::default(),
            output: Vec::new(),
            wrench: Wrench::default(),
            ghost_until: 0,
            alloc: AllocTable::default(),
            cores: Vec::new(),
            shields: Vec::new(),
            reactors: Vec::new(),
            consumers: Vec::new(),
            heaters: Vec::new(),
            dirty: false,
        }
    }

    /// Not the controlling piece: drifting salvage (no hull glow, systems off), even with a core.
    pub fn salvage(&self) -> bool {
        !self.controlled
    }

    /// Pose for rendering between the previous and the current tick: `alpha` 0 = previous, 1 = current.
    /// The angle takes the short way round.
    pub fn render_pose(&self, alpha: f32) -> (Vec2, f32) {
        let pos = self.prev_pos + (self.body.pos - self.prev_pos) * alpha;
        let d = crate::physics::wrap_angle(self.body.angle - self.prev_angle);
        (pos, crate::physics::wrap_angle(self.prev_angle + d * alpha))
    }

    /// Logic cores of this piece (alive when the piece was last rebuilt).
    pub fn cores(&self) -> &[PartIdx] {
        &self.cores
    }

    /// Consumers in the order they get power (tier, weight descending, part).
    pub fn consumers(&self) -> &[Consumer] {
        &self.consumers
    }

    /// Net engine force and torque for the given per-slot throttles, ignoring power and heat.
    pub fn max_wrench(&self, throttle: &[f32]) -> Wrench {
        thruster_wrench(&self.thrusters, |s| {
            throttle.get(s as usize).copied().unwrap_or(0.0)
        })
    }

    /// Centre of mass in world coordinates.
    pub fn com_world(&self) -> Vec2 {
        self.body.to_world(self.mass.com)
    }
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
    pub tmax: Vec<f32>,
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
    /// Parts destroyed since the last breakage resolution.
    pub(crate) pending_destroyed: Vec<PartIdx>,
    /// The controlling piece; `None` = the ship is dark for good (lost every core of that piece).
    pub(crate) control: Option<PieceId>,
    pub(crate) helm: HelmMode,
    /// A behavior program flies the ship (overrides the helm).
    pub(crate) behavior: Option<crate::behavior::BehaviorState>,
    /// Whose lock the `locked` target selector follows (an escort's leader).
    pub(crate) leader: Option<ShipId>,
    pub(crate) telemetry: FlightTelemetry,
    /// Global multipliers from `SimConfig` (thrust: engines and RCS; torque: gyros and cores).
    pub(crate) thrust_scale: f32,
    pub(crate) torque_scale: f32,
    /// Seconds until each gun slot may fire again (≤ 0: ready).
    pub(crate) gun_cooldown: Vec<f32>,
}

impl Ship {
    /// Builds a ship from a design. `body` is the pose of the blueprint's main piece (the one with a logic
    /// core, else the largest); the frame origin of every piece is its centre of mass. The main piece is the
    /// controlling piece if it has a core.
    pub(crate) fn new(
        id: ShipId,
        design: Arc<ShipDesign>,
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
            control: None,
            helm: HelmMode::Raw,
            behavior: None,
            leader: None,
            telemetry: FlightTelemetry::default(),
            thrust_scale: cfg.thrust_scale,
            torque_scale: cfg.torque_scale,
            gun_cooldown: vec![0.0; design.guns.len()],
            design,
        };
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
        let comps = ship.components(&all);
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

    pub fn helm(&self) -> &HelmMode {
        &self.helm
    }

    /// The running behavior and its debug view (active rule, why the others failed, target), if any.
    pub fn behavior(&self) -> Option<&crate::behavior::BehaviorState> {
        self.behavior.as_ref()
    }

    /// The ship whose lock this ship's `locked` target selector follows.
    pub fn leader(&self) -> Option<ShipId> {
        self.leader
    }

    /// Behavior rule slots now: the `rule_slots` of the working (alive, not overheated) logic cores of the
    /// controlling piece; 0 while the ship is dark.
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
            .filter(|&&c| {
                let ps = &self.parts[c as usize];
                ps.alive && !ps.overheated
            })
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

    /// The main piece: a piece with a logic core beats one without, then more parts, then the lower id.
    pub fn main_piece(&self) -> Option<PieceId> {
        let mut best: Option<&Piece> = None;
        for p in &self.pieces {
            best = match best {
                None => Some(p),
                Some(b)
                    if (p.has_core && !b.has_core)
                        || (p.has_core == b.has_core && p.parts.len() > b.parts.len()) =>
                {
                    Some(p)
                }
                b => b,
            };
        }
        best.map(|p| p.id)
    }

    pub fn controls(&self) -> &ShipControls {
        &self.controls
    }

    /// Replaces the controls (the throttle list is resized to the engine count) and switches the helm to
    /// `HelmMode::Raw` (no flight computer).
    pub fn set_controls(&mut self, c: &ShipControls) {
        self.helm = HelmMode::Raw;
        self.controls.throttle.clone_from(&c.throttle);
        self.controls
            .throttle
            .resize(self.design.engines.len(), 0.0);
        self.controls.fire = c.fire;
        self.controls.guns.clone_from(&c.guns);
        self.controls.shields = c.shields;
    }

    /// Outer hull edge of one piece in the blueprint frame (for the hull glow; chain with
    /// `data::geometry::hull_loops`). Salvage pieces have no glow; the client decides.
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

    /// Centre of mass of a set of parts in the blueprint frame.
    pub(crate) fn com_bp(&self, parts: &[PartIdx]) -> [f32; 2] {
        let d = &self.design;
        combine(
            parts.iter().map(|&p| {
                let s = &d.parts[p as usize];
                (s.mass, s.centroid, s.inertia)
            }),
            [0.0, 0.0],
        )
        .1
    }

    /// Connected components of the alive parts in `parts` (static links filtered by alive). Components are
    /// ordered by their lowest part index; parts inside ascending.
    pub(crate) fn components(&mut self, parts: &[PartIdx]) -> Vec<Vec<PartIdx>> {
        let n = self.parts.len();
        let mark = &mut self.scratch.mark;
        mark.clear();
        mark.resize(n, u32::MAX);
        const IN_SET: u32 = u32::MAX - 1;
        for &p in parts {
            if self.parts[p as usize].alive {
                mark[p as usize] = IN_SET;
            }
        }
        let mut comps: Vec<Vec<PartIdx>> = Vec::new();
        let stack = &mut self.scratch.stack;
        for &p in parts {
            if mark[p as usize] != IN_SET {
                continue;
            }
            let c = comps.len() as u32;
            let mut comp = Vec::new();
            mark[p as usize] = c;
            stack.clear();
            stack.push(p);
            while let Some(q) = stack.pop() {
                comp.push(q);
                for &r in self.design.neighbors.get(q as usize) {
                    if mark[r as usize] == IN_SET {
                        mark[r as usize] = c;
                        stack.push(r);
                    }
                }
            }
            comp.sort_unstable();
            comps.push(comp);
        }
        comps
    }

    /// Index of the main component: with a logic core first, then more parts, then the earlier one.
    pub(crate) fn main_component(&self, comps: &[Vec<PartIdx>]) -> usize {
        let has_core = |c: &Vec<PartIdx>| {
            c.iter()
                .any(|&p| self.design.parts[p as usize].logic && self.parts[p as usize].alive)
        };
        let mut best = 0;
        for k in 1..comps.len() {
            let (ck, cb) = (has_core(&comps[k]), has_core(&comps[best]));
            if (ck && !cb) || (ck == cb && comps[k].len() > comps[best].len()) {
                best = k;
            }
        }
        best
    }

    /// Recounts the open sides of part `p` from the grid (no allocation). Exposure only changes when a
    /// linked neighbour is destroyed, so `apply_damage` calls this for those neighbours only.
    pub(crate) fn refresh_exposure(&mut self, p: PartIdx) {
        let s = self.design.parts[p as usize].shape;
        let (w, h) = s.dims();
        let mut open = 0u32;
        for y in s.y..s.y + h as i32 {
            for x in s.x..s.x + w as i32 {
                if self.grid.occupant((x, y)) == Some(p as usize) {
                    open += self.grid.exposure((x, y));
                }
            }
        }
        self.parts[p as usize].open_sides = open.min(u16::MAX as u32) as u16;
    }

    /// Recomputes the effective stats (adjacency bonuses) and heat constants of part `t`. Bonuses change
    /// only when a source dies or a source's venting changes, i.e. around destroyed parts. Sources are linked
    /// to `t`, and linked alive parts are always in the same piece.
    pub(crate) fn refresh_stats(&mut self, t: PartIdx) {
        let d = &*self.design;
        let spec = &d.parts[t as usize];
        let mut add = [0.0f32; Stat::COUNT];
        let mut pct = [0.0f32; Stat::COUNT];
        let groups = &mut self.scratch.groups;
        groups.clear();
        for b in d.bonuses_in.get(t as usize) {
            let src = &self.parts[b.source as usize];
            if !src.alive || (b.needs_venting && src.open_sides == 0) {
                continue;
            }
            let count = match groups.iter_mut().find(|g| g.0 == b.group) {
                Some(g) => &mut g.1,
                None => {
                    groups.push((b.group, 0));
                    &mut groups.last_mut().unwrap().1
                }
            };
            if *count >= b.max_stacks {
                continue;
            }
            *count += 1;
            match b.effect {
                Effect::Add(x) => add[b.stat as usize] += x,
                Effect::Percent(x) => pct[b.stat as usize] += x,
            }
        }
        let mut st = spec.base;
        for k in 0..Stat::COUNT {
            if add[k] != 0.0 || pct[k] != 0.0 {
                st.v[k] = (st.v[k] + add[k]) * (1.0 + pct[k] / 100.0);
            }
        }
        self.stats[t as usize] = st;
        let per_side = if spec.is_radiator {
            st.get(Stat::HeatVent)
        } else {
            tuning::HEAT_LOSS_OPEN_SIDE
        };
        let open = self.parts[t as usize].open_sides as f32;
        self.heat_k[t as usize] = HeatK {
            cap: spec.heat_capacity,
            loss: tuning::HEAT_LOSS_BASE * spec.area + open * per_side,
            out: st.get(Stat::HeatOutput),
            limit: st.get(Stat::HeatLimit),
        };
    }

    /// Test helper: recomputes every per-part cache from scratch and compares it with the incrementally
    /// maintained one. `Err` names the first mismatch.
    #[doc(hidden)]
    pub fn verify_caches(&self) -> Result<(), String> {
        let mut fresh = self.clone();
        let alive: Vec<PartIdx> = (0..self.parts.len() as PartIdx)
            .filter(|&p| self.parts[p as usize].alive)
            .collect();
        for &p in &alive {
            fresh.refresh_exposure(p);
        }
        for &p in &alive {
            fresh.refresh_stats(p);
        }
        for &p in &alive {
            let i = p as usize;
            if fresh.parts[i].open_sides != self.parts[i].open_sides {
                return Err(format!(
                    "part {p}: open sides {} vs fresh {}",
                    self.parts[i].open_sides, fresh.parts[i].open_sides
                ));
            }
            if fresh.stats[i] != self.stats[i] || fresh.heat_k[i] != self.heat_k[i] {
                return Err(format!(
                    "part {p}: stats {:?} vs fresh {:?}",
                    self.stats[i], fresh.stats[i]
                ));
            }
        }
        for (k, piece) in self.pieces.iter().enumerate() {
            let mut f = fresh.clone();
            f.rebuild_piece(k);
            let fp = &f.pieces[k];
            if fp.mass != piece.mass
                || fp.colliders != piece.colliders
                || fp.consumers != piece.consumers
            {
                return Err(format!(
                    "piece {:?}: caches differ from a rebuild",
                    piece.id
                ));
            }
        }
        Ok(())
    }

    /// Recomputes the piece-level caches of piece `k`: core flag, clearances, mass properties, colliders,
    /// power lists, thruster table and gun mounts. Keeps the piece's frame origin. Parts' `piece` ids, exposure
    /// and stats must already be current.
    pub(crate) fn rebuild_piece(&mut self, k: usize) {
        let design = Arc::clone(&self.design);
        let d = &*design;
        let mut piece = std::mem::replace(
            &mut self.pieces[k],
            Piece::new(PieceId(u32::MAX), Vec::new(), [0.0; 2], Body::default()),
        );
        piece.dirty = false;
        let pid = piece.id;

        // Core flag.
        piece.has_core = piece.parts.iter().any(|&p| d.parts[p as usize].logic);

        // Clearances: only alive parts of the same piece block.
        let blocked = |blockers: &[PartIdx], parts: &[PartState]| {
            blockers.iter().any(|&b| {
                let s = &parts[b as usize];
                s.alive && s.piece == pid
            })
        };
        for &p in &piece.parts {
            let slot = d.slot_of_part[p as usize];
            let spec = &d.parts[p as usize];
            let clear = if slot == NO_SLOT {
                true
            } else if spec.is_engine {
                !blocked(d.exhaust_blockers.get(slot as usize), &self.parts)
            } else {
                !blocked(d.gun_blockers.get(slot as usize), &self.parts)
            };
            self.parts[p as usize].clear = clear;
        }

        // Mass properties and colliders.
        piece.mass = combine(
            piece.parts.iter().map(|&p| {
                let s = &d.parts[p as usize];
                (s.mass, s.centroid, s.inertia)
            }),
            piece.origin,
        )
        .0;
        build_colliders(
            d,
            &piece.parts,
            piece.origin,
            &mut self.scratch.colliders,
            &mut piece.colliders,
        );

        // Power lists (reactors, consumers in allocation order, heaters).
        self.rebuild_power_lists(&mut piece);

        // Torque sources (gyros, logic cores) and cores.
        piece.cores.clear();
        piece.shields.clear();
        piece.torque_sources.clear();
        for &p in &piece.parts {
            let spec = &d.parts[p as usize];
            if spec.logic {
                piece.cores.push(p);
            }
            if spec.is_shield {
                piece.shields.push(p);
            }
            let t = self.stats[p as usize].get(Stat::Torque) * self.torque_scale;
            if t > 0.0 {
                piece.torque_sources.push(TorqueSource { part: p, max: t });
            }
        }

        // Thruster table (engines, then RCS nozzles) and gun mounts.
        piece.thrusters.clear();
        piece.guns.clear();
        let com = piece.mass.com;
        let row = |slot: u32,
                   part: PartIdx,
                   dir: Vec2,
                   point: Vec2,
                   max_force: f32,
                   clear: bool,
                   gimbal: f32,
                   rcs: bool| {
            let arm = point - com;
            let lat = dir.perp();
            Thruster {
                slot,
                part,
                dir,
                point,
                arm,
                torque_per_force: arm.cross(dir),
                max_force,
                clear,
                gimbal,
                lat,
                torque_per_lateral: arm.cross(lat),
                rcs,
            }
        };
        for &p in &piece.parts {
            let slot = d.slot_of_part[p as usize];
            let spec = &d.parts[p as usize];
            let st = &self.stats[p as usize];
            let clear = self.parts[p as usize].clear;
            if spec.is_engine && slot != NO_SLOT {
                let e = &d.engines[slot as usize];
                let deg = st.get(Stat::Gimbal).clamp(0.0, 89.0);
                let gimbal = if deg > 0.0 {
                    sin(deg * (core::f32::consts::PI / 180.0))
                } else {
                    0.0
                };
                piece.thrusters.push(row(
                    slot,
                    p,
                    dir_vec(e.facing),
                    to_piece(e.point, piece.origin),
                    st.get(Stat::Thrust) * self.thrust_scale,
                    clear,
                    gimbal,
                    false,
                ));
            } else if spec.weapon.is_some() && slot != NO_SLOT {
                let g = &d.guns[slot as usize];
                let fire_rate = st.get(Stat::FireRate);
                let base_rate = match spec.delivery {
                    Delivery::Beam => 0.0,
                    Delivery::Pulse { rate } | Delivery::Projectile { rate, .. } => rate,
                };
                let damage = st.get(Stat::Damage);
                piece.guns.push(GunMount {
                    slot,
                    part: p,
                    muzzle: to_piece(g.muzzle, piece.origin),
                    dir: dir_vec(g.facing),
                    clear,
                    fire_rate,
                    dps: damage * fire_rate,
                    output: 0.0,
                    family: spec.weapon.unwrap_or(WeaponFamily::Energy),
                    delivery: spec.delivery,
                    range: st.get(Stat::Range),
                    rate: base_rate * fire_rate,
                    shot_damage: if base_rate > 0.0 {
                        damage / base_rate
                    } else {
                        0.0
                    },
                });
            }
        }
        // RCS: one nozzle per straight side whose neighbouring cell is empty or belongs to another piece. It
        // pushes away from its side, at the side's centre.
        for &p in &piece.parts {
            let spec = &d.parts[p as usize];
            if !spec.is_rcs {
                continue;
            }
            let thrust = self.stats[p as usize].get(Stat::RcsThrust) * self.thrust_scale;
            for (c, side) in spec.shape.connectable_sides() {
                let (dx, dy) = side.delta();
                let open = match self.grid.occupant((c.0 + dx, c.1 + dy)) {
                    None => true,
                    Some(o) => o != p as usize && self.parts[o].piece != pid,
                };
                if !open {
                    continue;
                }
                let centre = [
                    c.0 as f32 + 0.5 + 0.5 * dx as f32,
                    c.1 as f32 + 0.5 + 0.5 * dy as f32,
                ];
                piece.thrusters.push(row(
                    NO_SLOT,
                    p,
                    dir_vec(side.opposite()),
                    to_piece(centre, piece.origin),
                    thrust,
                    true,
                    0.0,
                    true,
                ));
            }
        }

        // Authority of the clear rows and the pure torque.
        let mut auth = ThrustAuthority::default();
        for t in &piece.thrusters {
            if !t.clear {
                continue;
            }
            let f = t.max_force;
            match crate::alloc::group_of(t.dir) {
                0 => auth.forward += f,
                1 => auth.backward += f,
                2 => auth.right += f,
                _ => auth.left += f,
            }
            let tq = t.torque_per_force * f;
            if tq > 0.0 {
                auth.torque_ccw += tq;
            } else {
                auth.torque_cw -= tq;
            }
        }
        auth.torque_pure = piece.torque_sources.iter().map(|s| s.max).sum();
        piece.authority = auth;
        piece.alloc = AllocTable::build(&piece.thrusters);
        piece
            .act
            .resize(piece.thrusters.len(), piece.torque_sources.len());
        piece.output.clear();
        piece.output.resize(piece.thrusters.len(), 0.0);
        piece.active = piece.controlled && piece.has_core;
        self.pieces[k] = piece;
    }

    /// One tick of the ship systems: power → heat → wrench (engines, RCS, gimbals, gyros, cores) and gun
    /// output. Reads this tick's actuator commands (`Piece::act`). No allocation once warmed up.
    pub fn update_systems(&mut self) {
        let Ship {
            design,
            parts,
            stats,
            heat_k,
            pieces,
            controls,
            scratch,
            ..
        } = self;
        let d = &**design;
        let activity = &mut scratch.activity;
        if activity.len() != parts.len() {
            activity.clear();
            activity.resize(parts.len(), 0.0);
        }

        for piece in pieces.iter_mut() {
            // Power supply first: a controlled piece is active only with a working core and some power.
            let mut supply = 0.0f32;
            for r in &piece.reactors {
                let ps = &parts[r.part as usize];
                if ps.alive && !ps.overheated {
                    supply += r.output;
                }
            }
            let core_ok = piece.cores.iter().any(|&c| {
                let ps = &parts[c as usize];
                ps.alive && !ps.overheated
            });
            let active = piece.controlled && core_ok && supply > 0.0;
            piece.active = active;

            // What the movers were told to do (engines and RCS pods: summed throttle; gyros: |use|).
            for c in &piece.consumers {
                activity[c.part as usize] = 0.0;
            }
            if active {
                for (i, t) in piece.thrusters.iter().enumerate() {
                    activity[t.part as usize] += piece.act.throttle.get(i).copied().unwrap_or(0.0);
                }
                for (j, src) in piece.torque_sources.iter().enumerate() {
                    let u = piece.act.torque.get(j).copied().unwrap_or(0.0).abs();
                    let a = &mut activity[src.part as usize];
                    *a = a.max(u);
                }
            }

            // What each consumer asks for (draw × activity), then the allocation by tier and weight.
            let want = &mut scratch.want;
            want.clear();
            for c in &piece.consumers {
                let ps = &parts[c.part as usize];
                let act = if !active || ps.overheated || !ps.clear {
                    0.0
                } else {
                    match c.kind {
                        ConsumerKind::Engine => activity[c.part as usize].clamp(0.0, 1.0),
                        ConsumerKind::Weapon => {
                            controls.gun_fires(d.slot_of_part[c.part as usize]) as u8 as f32
                        }
                        ConsumerKind::Shield => controls.shields as u8 as f32,
                        ConsumerKind::Logic | ConsumerKind::Other => 1.0,
                    }
                };
                activity[c.part as usize] = act;
                want.push(c.draw * act);
            }
            let level = &mut scratch.level;
            level.clear();
            level.resize(want.len(), 0.0);
            let fed = if active { supply } else { 0.0 };
            let (demand, served) = crate::power::allocate(&piece.consumers, want, fed, level);
            for (c, &l) in piece.consumers.iter().zip(level.iter()) {
                let ps = &mut parts[c.part as usize];
                ps.power = l;
                ps.work = activity[c.part as usize] * l;
            }
            let reactor_work = if served > 0.0 && supply > 0.0 {
                let load = (served / supply).min(1.0);
                tuning::REACTOR_IDLE_HEAT + (1.0 - tuning::REACTOR_IDLE_HEAT) * load
            } else {
                0.0
            };
            for r in &piece.reactors {
                let ps = &mut parts[r.part as usize];
                ps.power = 1.0;
                ps.work = if ps.overheated { 0.0 } else { reactor_work };
            }
            for &h in &piece.heaters {
                let ps = &mut parts[h as usize];
                ps.work = if active && !ps.overheated { 1.0 } else { 0.0 };
            }
            // Shield bubbles recharge while their emitter works.
            for &e in &piece.shields {
                let ps = &mut parts[e as usize];
                if ps.work > 0.0 {
                    let st = &stats[e as usize];
                    let cap = st.get(Stat::ShieldCapacity);
                    ps.shield = (ps.shield + st.get(Stat::ShieldRegen) * ps.work * DT).min(cap);
                    if ps.shield_down && ps.shield >= cap * crate::weapons::tuning::SHIELD_RESTART {
                        ps.shield_down = false;
                    }
                }
            }
            piece.power = PowerState {
                supply,
                demand,
                served,
            };

            // Wrench at the power each actuator actually got.
            let fmax = &mut scratch.fmax;
            fmax.clear();
            fmax.extend(piece.thrusters.iter().map(|t| {
                let ps = &parts[t.part as usize];
                if active && t.clear && ps.alive && !ps.overheated {
                    t.max_force * ps.power
                } else {
                    0.0
                }
            }));
            let tmax = &mut scratch.tmax;
            tmax.clear();
            tmax.extend(piece.torque_sources.iter().map(|src| {
                let ps = &parts[src.part as usize];
                if active && ps.alive && !ps.overheated {
                    src.max * ps.power
                } else {
                    0.0
                }
            }));
            if piece.act.throttle.len() == piece.thrusters.len()
                && piece.act.torque.len() == tmax.len()
            {
                piece.wrench = wrench_of(&piece.thrusters, fmax, tmax, &piece.act);
                for (i, t) in piece.thrusters.iter().enumerate() {
                    piece.output[i] = if t.max_force > 0.0 {
                        piece.act.throttle[i] * fmax[i] / t.max_force
                    } else {
                        0.0
                    };
                }
            } else {
                piece.wrench = Wrench::default();
            }
            for g in &mut piece.guns {
                g.output = g.dps * parts[g.part as usize].work;
            }
        }

        // Heat (ship-wide; links never cross pieces). Generation and conduction explicit, loss implicit.
        let e = &mut scratch.energy;
        e.clear();
        e.resize(parts.len(), 0.0);
        for ((ps, k), e) in parts.iter().zip(heat_k.iter()).zip(e.iter_mut()) {
            if ps.alive {
                *e = ps.heat * k.cap + k.out * ps.work * DT;
            }
        }
        for l in &d.links {
            let (a, b) = (l.a as usize, l.b as usize);
            if !parts[a].alive || !parts[b].alive {
                continue;
            }
            let q =
                tuning::HEAT_CONDUCTANCE * l.edges as f32 * (parts[a].heat - parts[b].heat) * DT;
            e[a] -= q;
            e[b] += q;
        }
        for ((ps, k), &e) in parts.iter_mut().zip(heat_k.iter()).zip(e.iter()) {
            if !ps.alive {
                continue;
            }
            let h = e / (k.cap + k.loss * DT);
            ps.heat = if h > 0.0 { h } else { 0.0 };
            if ps.heat > k.limit {
                ps.overheated = true;
            } else if ps.overheated && ps.heat < k.limit * tuning::OVERHEAT_RESUME {
                ps.overheated = false;
            }
        }
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
        h.u64(self.ghost_until);
        self.body.hash_state(h);
        h.f32(self.power.supply);
        h.f32(self.power.demand);
        h.f32(self.power.served);
        crate::power::hash_consumers(self, h);
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
        }
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
        h.u32(self.leader.map_or(u32::MAX, |l| l.0));
        h.u32(self.parts.len() as u32);
        for ps in &self.parts {
            h.u32(ps.alive as u32 | (ps.overheated as u32) << 1);
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
