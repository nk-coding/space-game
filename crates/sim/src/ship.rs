//! The runtime ship: per-part state, pieces (one rigid body each) and the per-tick systems (power, heat,
//! engines, guns). Damage and breakage live in `breakage.rs`.
//!
//! # Cost model
//!
//! Everything that depends only on topology (which parts are alive) is cached per piece and rebuilt only
//! when the piece loses a part ([`Piece`] fields: mass properties, colliders, bonuses, power lists, thruster
//! table, gun mounts). The per-tick code walks flat arrays and allocates nothing.

use std::sync::Arc;

use data::{Dir, Effect, ShipGrid};

use crate::collider::{ColliderScratch, ConvexPolygon, build_colliders};
use crate::design::{
    ConsumerKind, DEFAULT_POWER_PRIORITY, NO_SLOT, PartIdx, PartStats, ShipDesign, Stat,
};
use crate::hash::{StateHash, StateHasher};
use crate::ids::{IdAlloc, PieceId, ShipId};
use crate::mass::{MassProps, combine, to_piece};
use crate::math::{Rot, Vec2};
use crate::DT;

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
    /// All guns fire (fire groups come later).
    pub fire: bool,
    /// Shield emitters on.
    pub shields: bool,
}

impl ShipControls {
    pub fn idle(engines: usize) -> ShipControls {
        ShipControls {
            throttle: vec![0.0; engines],
            fire: false,
            shields: true,
        }
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
    /// Share of its power demand served this tick (1 when it wants nothing).
    pub power: f32,
    /// How hard it worked this tick, 0..1 (throttle × power for engines). Drives heat and output.
    pub work: f32,
}

/// Power balance of a piece this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PowerState {
    /// Output of its working reactors (with bonuses).
    pub supply: f32,
    /// What its active consumers asked for.
    pub demand: f32,
    /// What they got (`min(supply, demand)`), handed out in brown-out priority order.
    pub served: f32,
}

/// Net force (piece frame, kN) and torque about the centre of mass (kN·m, counter-clockwise positive).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wrench {
    pub force: Vec2,
    pub torque: f32,
}

/// One row of a piece's thruster table (for the flight computer). Piece frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thruster {
    /// Engine slot (index into `ShipControls::throttle`).
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
}

/// Net force and torque of `thrusters` at the given throttles (by slot), ignoring power and heat.
pub fn thruster_wrench(thrusters: &[Thruster], throttle: impl Fn(u32) -> f32) -> Wrench {
    let mut w = Wrench::default();
    for t in thrusters {
        if !t.clear {
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
    /// Has an alive logic core. Pieces without one are salvage: systems idle, no hull glow.
    pub has_core: bool,
    pub body: Body,
    pub power: PowerState,
    pub thrusters: Vec<Thruster>,
    pub authority: ThrustAuthority,
    pub guns: Vec<GunMount>,
    /// Engine force and torque this tick, for the physics layer.
    pub wrench: Wrench,
    reactors: Vec<PartIdx>,
    /// Consumers in brown-out priority order.
    consumers: Vec<PartIdx>,
    /// Parts that make heat without a power role (e.g. capacitors).
    heaters: Vec<PartIdx>,
    pub(crate) dirty: bool,
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
            body,
            power: PowerState::default(),
            thrusters: Vec::new(),
            authority: ThrustAuthority::default(),
            guns: Vec::new(),
            wrench: Wrench::default(),
            reactors: Vec::new(),
            consumers: Vec::new(),
            heaters: Vec::new(),
            dirty: false,
        }
    }

    /// No logic core: drifting salvage.
    pub fn salvage(&self) -> bool {
        !self.has_core
    }

    /// Consumers in the order they get power.
    pub fn consumers(&self) -> &[PartIdx] {
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
    pub colliders: ColliderScratch,
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
    /// Alive pieces, ascending id.
    pub(crate) pieces: Vec<Piece>,
    pub(crate) controls: ShipControls,
    /// Brown-out order (default [`DEFAULT_POWER_PRIORITY`]).
    pub(crate) power_priority: [ConsumerKind; ConsumerKind::COUNT],
    pub(crate) scratch: Scratch,
    /// Parts destroyed since the last breakage resolution.
    pub(crate) pending_destroyed: Vec<PartIdx>,
}

impl Ship {
    /// Builds a ship from a design. `body` is the pose of the blueprint's main piece (the one with a logic
    /// core, else the largest); the frame origin of every piece is its centre of mass.
    pub fn new(id: ShipId, design: Arc<ShipDesign>, body: Body, ids: &mut IdAlloc) -> Ship {
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
            })
            .collect();
        let mut ship = Ship {
            id,
            controls: ShipControls::idle(design.engines.len()),
            grid: design.grid.clone(),
            stats: design.parts.iter().map(|s| s.base).collect(),
            parts,
            pieces: Vec::new(),
            power_priority: DEFAULT_POWER_PRIORITY,
            scratch: Scratch::default(),
            pending_destroyed: Vec::new(),
            design,
        };
        let all: Vec<PartIdx> = (0..n as u32).collect();
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
        for k in 0..ship.pieces.len() {
            ship.rebuild_piece(k);
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

    /// Mutable access for the physics layer (to write `body` back after a physics step).
    pub fn piece_mut(&mut self, id: PieceId) -> Option<&mut Piece> {
        self.piece_index(id).map(move |k| &mut self.pieces[k])
    }

    /// Bodies of all pieces, for the physics layer's sync-back.
    pub fn pieces_mut(&mut self) -> impl Iterator<Item = (PieceId, &mut Body)> {
        self.pieces.iter_mut().map(|p| (p.id, &mut p.body))
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

    /// Replaces the controls (the throttle list is resized to the engine count).
    pub fn set_controls(&mut self, c: &ShipControls) {
        self.controls.throttle.clone_from(&c.throttle);
        self.controls
            .throttle
            .resize(self.design.engines.len(), 0.0);
        self.controls.fire = c.fire;
        self.controls.shields = c.shields;
    }

    pub fn power_priority(&self) -> [ConsumerKind; ConsumerKind::COUNT] {
        self.power_priority
    }

    /// Changes the brown-out order (a permutation of all kinds). Takes effect after the next rebuild of each
    /// piece; call before the first step.
    pub fn set_power_priority(&mut self, order: [ConsumerKind; ConsumerKind::COUNT]) {
        self.power_priority = order;
        for k in 0..self.pieces.len() {
            self.rebuild_piece(k);
        }
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
                let c = (b.x - a.x) as f32 * (by - a.y as f32) - (b.y - a.y) as f32 * (bx - a.x as f32);
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

    /// Recomputes everything topology-derived for piece `k`: core flag, exposure, bonuses, clearances, mass
    /// properties, colliders, power lists, thruster table and gun mounts. Keeps the piece's frame origin.
    /// Parts' `piece` ids must already be set.
    pub(crate) fn rebuild_piece(&mut self, k: usize) {
        let design = Arc::clone(&self.design);
        let d = &*design;
        let mut piece = std::mem::replace(
            &mut self.pieces[k],
            Piece::new(PieceId(u32::MAX), Vec::new(), [0.0; 2], Body::default()),
        );
        piece.dirty = false;
        let pid = piece.id;

        // Core flag and exposure.
        piece.has_core = false;
        for &p in &piece.parts {
            let spec = &d.parts[p as usize];
            piece.has_core |= spec.logic;
            let open: u32 = spec
                .shape
                .cells()
                .into_iter()
                .filter(|&c| self.grid.occupant(c) == Some(p as usize))
                .map(|c| self.grid.exposure(c))
                .sum();
            self.parts[p as usize].open_sides = open.min(u16::MAX as u32) as u16;
        }

        // Adjacency bonuses → effective stats.
        for &t in &piece.parts {
            let spec = &d.parts[t as usize];
            let mut add = [0.0f32; Stat::COUNT];
            let mut pct = [0.0f32; Stat::COUNT];
            let groups = &mut self.scratch.groups;
            groups.clear();
            for b in d.bonuses_in.get(t as usize) {
                let src = &self.parts[b.source as usize];
                if !src.alive || src.piece != pid {
                    continue;
                }
                if b.needs_venting && src.open_sides == 0 {
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
            for s in 0..Stat::COUNT {
                if add[s] != 0.0 || pct[s] != 0.0 {
                    st.v[s] = (st.v[s] + add[s]) * (1.0 + pct[s] / 100.0);
                }
            }
            self.stats[t as usize] = st;
        }

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

        // Power lists.
        piece.reactors.clear();
        piece.consumers.clear();
        piece.heaters.clear();
        for &p in &piece.parts {
            let spec = &d.parts[p as usize];
            if spec.reactor {
                piece.reactors.push(p);
            } else if spec.consumer.is_some() {
                piece.consumers.push(p);
            } else if spec.base.get(Stat::HeatOutput) > 0.0 {
                piece.heaters.push(p);
            }
        }
        let rank = |p: PartIdx| {
            let kind = d.parts[p as usize].consumer.unwrap_or(ConsumerKind::Other);
            self.power_priority
                .iter()
                .position(|&k| k == kind)
                .unwrap_or(ConsumerKind::COUNT)
        };
        piece.consumers.sort_by_key(|&p| (rank(p), p));

        // Thruster table and gun mounts.
        piece.thrusters.clear();
        piece.guns.clear();
        let mut auth = ThrustAuthority::default();
        for &p in &piece.parts {
            let slot = d.slot_of_part[p as usize];
            if slot == NO_SLOT {
                continue;
            }
            let spec = &d.parts[p as usize];
            let st = &self.stats[p as usize];
            let clear = self.parts[p as usize].clear;
            if spec.is_engine {
                let e = &d.engines[slot as usize];
                let dir = dir_vec(e.facing);
                let point = to_piece(e.point, piece.origin);
                let arm = point - piece.mass.com;
                let t = Thruster {
                    slot,
                    part: p,
                    dir,
                    point,
                    arm,
                    torque_per_force: arm.cross(dir),
                    max_force: st.get(Stat::Thrust),
                    clear,
                };
                if clear {
                    let f = t.max_force;
                    match e.facing {
                        Dir::Up => auth.forward += f,
                        Dir::Down => auth.backward += f,
                        Dir::Right => auth.right += f,
                        Dir::Left => auth.left += f,
                    }
                    let tq = t.torque_per_force * f;
                    if tq > 0.0 {
                        auth.torque_ccw += tq;
                    } else {
                        auth.torque_cw -= tq;
                    }
                }
                piece.thrusters.push(t);
            } else {
                let g = &d.guns[slot as usize];
                let fire_rate = st.get(Stat::FireRate);
                piece.guns.push(GunMount {
                    slot,
                    part: p,
                    muzzle: to_piece(g.muzzle, piece.origin),
                    dir: dir_vec(g.facing),
                    clear,
                    fire_rate,
                    dps: st.get(Stat::Damage) * fire_rate,
                    output: 0.0,
                });
            }
        }
        piece.authority = auth;
        self.pieces[k] = piece;
    }

    /// One tick of the ship systems: power → heat → engine wrench and gun output. No allocation.
    pub fn update_systems(&mut self) {
        let Ship {
            design,
            parts,
            stats,
            pieces,
            controls,
            scratch,
            ..
        } = self;
        let d = &**design;

        for piece in pieces.iter_mut() {
            let active = piece.has_core;
            // Power: reactors fill the pool, consumers draw in priority order.
            let mut supply = 0.0f32;
            for &r in &piece.reactors {
                let ps = &parts[r as usize];
                if ps.alive && !ps.overheated {
                    supply += stats[r as usize].get(Stat::PowerOutput);
                }
            }
            let mut remaining = supply;
            let (mut demand, mut served) = (0.0f32, 0.0f32);
            for &c in &piece.consumers {
                let ps = &mut parts[c as usize];
                let act = if !active || ps.overheated || !ps.clear {
                    0.0
                } else {
                    match d.parts[c as usize].consumer {
                        Some(ConsumerKind::Engine) => controls
                            .throttle
                            .get(d.slot_of_part[c as usize] as usize)
                            .copied()
                            .unwrap_or(0.0)
                            .clamp(0.0, 1.0),
                        Some(ConsumerKind::Weapon) => controls.fire as u8 as f32,
                        Some(ConsumerKind::Shield) => controls.shields as u8 as f32,
                        _ => 1.0,
                    }
                };
                let want = stats[c as usize].get(Stat::PowerDraw) * act;
                let got = want.min(remaining);
                remaining -= got;
                demand += want;
                served += got;
                ps.power = if want > 0.0 { got / want } else { 1.0 };
                ps.work = act * ps.power;
            }
            let reactor_work = if served > 0.0 && supply > 0.0 {
                let load = (served / supply).min(1.0);
                tuning::REACTOR_IDLE_HEAT + (1.0 - tuning::REACTOR_IDLE_HEAT) * load
            } else {
                0.0
            };
            for &r in &piece.reactors {
                let ps = &mut parts[r as usize];
                ps.power = 1.0;
                ps.work = if ps.overheated { 0.0 } else { reactor_work };
            }
            for &h in &piece.heaters {
                let ps = &mut parts[h as usize];
                ps.work = if active && !ps.overheated { 1.0 } else { 0.0 };
            }
            piece.power = PowerState {
                supply,
                demand,
                served,
            };

            // Engines → wrench; guns → output.
            let mut w = Wrench::default();
            for t in &piece.thrusters {
                let work = parts[t.part as usize].work;
                if work > 0.0 {
                    let f = t.max_force * work;
                    w.force += t.dir * f;
                    w.torque += t.torque_per_force * f;
                }
            }
            piece.wrench = w;
            for g in &mut piece.guns {
                g.output = g.dps * parts[g.part as usize].work;
            }
        }

        // Heat (ship-wide; links never cross pieces). Generation and conduction explicit, loss implicit.
        let e = &mut scratch.energy;
        e.clear();
        e.resize(parts.len(), 0.0);
        for (i, ps) in parts.iter().enumerate() {
            if ps.alive {
                e[i] = ps.heat * d.parts[i].heat_capacity
                    + stats[i].get(Stat::HeatOutput) * ps.work * DT;
            }
        }
        for l in &d.links {
            let (a, b) = (l.a as usize, l.b as usize);
            if !parts[a].alive || !parts[b].alive {
                continue;
            }
            let q = tuning::HEAT_CONDUCTANCE * l.edges as f32 * (parts[a].heat - parts[b].heat) * DT;
            e[a] -= q;
            e[b] += q;
        }
        for (i, ps) in parts.iter_mut().enumerate() {
            if !ps.alive {
                continue;
            }
            let spec = &d.parts[i];
            let st = &stats[i];
            let per_side = if spec.is_radiator {
                st.get(Stat::HeatVent)
            } else {
                tuning::HEAT_LOSS_OPEN_SIDE
            };
            let loss = tuning::HEAT_LOSS_BASE * spec.area + ps.open_sides as f32 * per_side;
            let h = e[i] / (spec.heat_capacity + loss * DT);
            ps.heat = if h > 0.0 { h } else { 0.0 };
            let limit = st.get(Stat::HeatLimit);
            if ps.heat > limit {
                ps.overheated = true;
            } else if ps.overheated && ps.heat < limit * tuning::OVERHEAT_RESUME {
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
        h.bool(self.has_core);
        self.body.hash_state(h);
        h.f32(self.power.supply);
        h.f32(self.power.demand);
        h.f32(self.power.served);
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
        h.bool(self.controls.shields);
        h.u32(self.parts.len() as u32);
        for ps in &self.parts {
            h.bool(ps.alive);
            if !ps.alive {
                continue;
            }
            h.f32(ps.hp);
            h.f32(ps.heat);
            h.bool(ps.overheated);
            h.u32(ps.piece.0);
            h.f32(ps.power);
            h.f32(ps.work);
        }
        h.u32(self.pieces.len() as u32);
        for p in &self.pieces {
            p.hash_state(h);
        }
    }
}
