//! Tractor beams (GDR-0131, `docs/sim.md` "Tractor beams"): a part with a content `tractor` block pulls salvage towards
//! its vessel, so collecting (GDR-0063, `salvage.rs`: the hull takes what drifts slowly within reach) no longer needs
//! the ship to fly over every piece.
//!
//! * **Who pulls**: every running vessel of an owner that collects (`SalvageTuning::collects`) with a free hold unit
//!   and alive, working tractors on its controlling piece, in ship order, every tick (just before the physics step).
//! * **What**: salvage only, never a vessel: loose items (fragments, spilled cargo, ore) and salvage pieces (wrecks and
//!   what broke off a ship), each only while something on it fits the free hold (`Ship::has_salvage`, as collecting
//!   takes it). Within a tractor's `range` of the controlling piece's centre of mass (a loose item's point, a piece's
//!   bounding circle), nearest first (ties: loose items, then ship and piece order); each tractor (ascending part)
//!   holds up to `beams` of them, the nearest not yet held.
//! * **How**: towards the ship's centre of mass, closing at most `speed` m/s relative to the ship and easing off as
//!   the gap between the ship's and the salvage's bounding circles shrinks (so it stops at the hull, as the beam can
//!   brake it: [`tuning::BRAKE`] of its pull's deceleration), never slower than [`tuning::CRAWL`], so it always reaches
//!   the hull. The change of velocity that asks for is capped at `force × work` kN over the salvage's mass (a piece's
//!   mass; a loose item [`tuning::LOOSE_MASS`] t a cargo unit): a light fragment follows at once, a heavy wreck piece
//!   slowly. Salvage held by a beam does not settle (`SalvageTuning::settle`); a piece gets the pull as a force at its
//!   centre of mass in the physics step, a loose item as a change of its velocity. A beam never pushes salvage away
//!   from the ship: along the line to the ship it only brakes the salvage's own motion towards it (a ship flying at a
//!   piece faster than the beam would close leaves it where it is). The puller feels nothing back.
//!
//! Nothing new enters the state hash: the pull lives in the velocities of the salvage it moves (already hashed), and
//! the beams of a tick ([`Sim::tractor_beams`], for the client) are worked out from the state each tick. Worlds
//! without tractors run exactly as before. Cost per pulling vessel and tick: one distance test per loose item and
//! salvage piece, a sort of those in reach; no allocation once the buffers are warm.

use crate::DT;
use crate::combat::salvage::LooseId;
use crate::ids::{PieceId, ShipId};
use crate::math::{Vec2, sqrt};
use crate::ship::Ship;
use crate::ship::design::PartIdx;
use crate::ship::vessel::VesselState;
use crate::world::Sim;

/// Lead defaults (20 Oct 2026, GDR-0131). PLACEHOLDERS.
pub mod tuning {
    /// A loose item weighs this many tonnes a cargo unit to a beam.
    pub const LOOSE_MASS: f32 = 1.0;
    /// A beam never closes slower than this (m/s), so what it holds reaches the hull.
    pub const CRAWL: f32 = 1.0;
    /// A beam plans to stop what it holds at the hull with this share of its pull's deceleration.
    pub const BRAKE: f32 = 0.5;
    /// With a tractor, the behaviors' `salvage` and the voyage runner stand this share of the shortest tractor reach
    /// off the salvage and let the beam do the rest.
    pub const STANDOFF: f32 = 0.5;
}

/// What a beam holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pulled {
    /// A loose item (`Sim::loose`).
    Loose(LooseId),
    /// A salvage piece.
    Piece(ShipId, PieceId),
}

/// One beam of this tick: the vessel, its tractor part and what it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TractorBeam {
    pub ship: ShipId,
    pub part: PartIdx,
    pub target: Pulled,
}

/// Salvage in reach of one puller.
#[derive(Clone, Copy, Debug)]
enum Cand {
    /// `SalvageState::loose[i]`.
    Loose(usize),
    /// Piece `q` of ship `j`.
    Piece(usize, usize),
}

impl Cand {
    /// Tie order: loose items first (by index, which is id order), then ship and piece order.
    fn key(self) -> (u8, usize, usize) {
        match self {
            Cand::Loose(i) => (0, i, 0),
            Cand::Piece(j, q) => (1, j, q),
        }
    }
}

/// This tick's beams and pulls (worked out from the state each tick; not hashed).
#[derive(Clone, Debug, Default)]
pub(crate) struct TractorState {
    beams: Vec<TractorBeam>,
    /// The pull on salvage pieces (world frame, kN), by `(ship index, piece index)`, ascending (a piece two
    /// vessels hold appears twice).
    pub(crate) forces: Vec<((usize, usize), Vec2)>,
    /// Loose items a beam holds this tick, ascending id (they do not settle).
    pub(crate) loose: Vec<LooseId>,
    cand: Vec<(f32, Cand)>,
}

impl TractorState {
    /// Is piece `(ship index, piece index)` held by a beam this tick?
    pub(crate) fn holds_piece(&self, key: (usize, usize)) -> bool {
        self.forces.binary_search_by(|f| f.0.cmp(&key)).is_ok()
    }

    /// The summed pull on piece `(ship index, piece index)` this tick.
    pub(crate) fn force_on(&self, key: (usize, usize)) -> Vec2 {
        let start = self.forces.partition_point(|f| f.0 < key);
        self.forces[start..]
            .iter()
            .take_while(|f| f.0 == key)
            .fold(Vec2::ZERO, |a, f| a + f.1)
    }
}

impl Ship {
    /// The shortest reach of the alive tractors on the controlling piece (GDR-0131), if any: the behaviors'
    /// `salvage` and the voyage runner stand [`tuning::STANDOFF`] of it off the salvage.
    pub fn tractor_reach(&self) -> Option<f32> {
        let c = self.control_piece()?;
        self.design()
            .tractors
            .iter()
            .filter(|m| {
                let ps = self.part(m.part);
                ps.alive && ps.piece == c
            })
            .map(|m| m.spec.range)
            .reduce(f32::min)
    }
}

/// Where a ship at `from` holds to let a beam pull salvage at `at` in: `standoff` metres short of it on the way, or
/// where it is once that close (the behaviors' `salvage` and the voyage runner, [`tuning::STANDOFF`]).
pub fn short_of(from: Vec2, at: Vec2, standoff: f32) -> Vec2 {
    let way = at - from;
    let d = way.length();
    if d <= standoff { from } else { at - way * (standoff / d) }
}

/// The velocity change a beam gives salvage at `at` moving at `vel` (mass `mass` t, bounding radius `radius`) this
/// tick, held by a ship at `from` moving at `ship_vel` (bounding radius `ship_radius`): see the module docs.
#[allow(clippy::too_many_arguments)]
fn pull(from: Vec2, ship_vel: Vec2, ship_radius: f32, at: Vec2, vel: Vec2, radius: f32, mass: f32, force: f32, speed: f32) -> Vec2 {
    let accel = force / mass.max(1e-3);
    let to = from - at;
    let dist = to.length();
    let gap = (dist - ship_radius - radius).max(0.0);
    let closing = speed.min(sqrt(2.0 * tuning::BRAKE * accel * gap).max(tuning::CRAWL));
    let want = if dist > 0.5 { ship_vel + to * (closing / dist) } else { ship_vel };
    let mut dv = want - vel;
    // A beam pulls; it never pushes salvage away from the ship (a ship closing faster than the beam would, flying at
    // it): towards the ship it may only take back the salvage's own motion towards the ship (braking it at the hull).
    if dist > 0.5 {
        let dir = to * (1.0 / dist);
        let along = dv.dot(dir);
        let floor = -vel.dot(dir).max(0.0);
        if along < floor {
            dv += dir * (floor - along);
        }
    }
    let cap = accel * DT;
    let len = dv.length();
    if len > cap { dv * (cap / len) } else { dv }
}

impl Sim {
    /// This tick's tractor beams (GDR-0131): which vessel's tractor holds what, for drawing.
    pub fn tractor_beams(&self) -> &[TractorBeam] {
        &self.tractor.beams
    }

    /// The tractor pass (see the module docs): every pulling vessel's beams take hold of the nearest salvage and
    /// pull it; pieces get their pull in the physics step (`TractorState::forces`), loose items at once.
    pub(crate) fn tractor_step(&mut self) {
        let collects = self.config().salvage;
        let st = &mut self.tractor;
        st.beams.clear();
        st.forces.clear();
        st.loose.clear();
        for k in 0..self.ships.len() {
            let s = &self.ships[k];
            if s.design().tractors.is_empty()
                || !collects.collects(s.owner())
                || s.vessel_state() != Some(VesselState::Running)
            {
                continue;
            }
            let free = s.cargo_capacity().saturating_sub(s.cargo().used());
            let Some(c) = s.control_piece() else { continue };
            let Some(cp) = s.piece(c) else { continue };
            let working = |p: PartIdx| {
                let ps = s.part(p);
                ps.alive && ps.piece == c && ps.work > 0.0
            };
            let Some(range) = s
                .design()
                .tractors
                .iter()
                .filter(|m| working(m.part))
                .map(|m| m.spec.range)
                .reduce(f32::max)
            else {
                continue;
            };
            if free == 0 {
                continue;
            }
            let (from, ship_vel, ship_radius) = (cp.com_world(), cp.body.linvel, cp.radius);

            // Salvage in reach, nearest first.
            st.cand.clear();
            for (i, l) in self.salvage.loose.iter().enumerate() {
                let d = (l.pos - from).length();
                if l.units <= free && d <= range {
                    st.cand.push((d, Cand::Loose(i)));
                }
            }
            for (j, o) in self.ships.iter().enumerate() {
                for (q, p) in o.pieces().iter().enumerate() {
                    if !p.salvage() {
                        continue;
                    }
                    let d = ((p.body.pos - from).length() - p.radius).max(0.0);
                    if d <= range && o.has_salvage(q, free) {
                        st.cand.push((d, Cand::Piece(j, q)));
                    }
                }
            }
            st.cand
                .sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.key().cmp(&b.1.key())));

            // Each working tractor (ascending part) holds the nearest not yet held, up to its beams.
            let mut next = 0;
            for m in s.design().tractors.iter().filter(|m| working(m.part)) {
                let force = m.spec.force * s.part(m.part).work;
                for _ in 0..m.spec.beams {
                    let Some(&(d, cand)) = st.cand.get(next) else { break };
                    if d > m.spec.range {
                        break;
                    }
                    next += 1;
                    let target = match cand {
                        Cand::Loose(i) => {
                            let l = &mut self.salvage.loose[i];
                            let mass = l.units as f32 * tuning::LOOSE_MASS;
                            l.vel += pull(from, ship_vel, ship_radius, l.pos, l.vel, 0.0, mass, force, m.spec.speed);
                            st.loose.push(l.id);
                            Pulled::Loose(l.id)
                        }
                        Cand::Piece(j, q) => {
                            let o = &self.ships[j];
                            let p = &o.pieces()[q];
                            let mass = p.mass.mass;
                            let dv = pull(from, ship_vel, ship_radius, p.com_world(), p.body.linvel, p.radius, mass, force, m.spec.speed);
                            st.forces.push(((j, q), dv * (mass / DT)));
                            Pulled::Piece(o.id(), p.id)
                        }
                    };
                    st.beams.push(TractorBeam { ship: s.id(), part: m.part, target });
                }
            }
        }
        st.forces.sort_by(|a, b| a.0.cmp(&b.0));
        st.loose.sort_unstable();
        st.loose.dedup();
    }
}
