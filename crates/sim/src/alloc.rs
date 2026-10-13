//! Control allocation: a desired body-frame force and torque → actuator commands.
//!
//! Actuators of a piece (all data-driven, see `docs/sim.md`):
//!
//! * **Thruster rows** ([`crate::ship::Thruster`]): engines and RCS nozzles. Each pushes along one body axis
//!   (+y, −y, +x, −x) with `throttle ∈ [0, 1]`. An engine may **gimbal**: a lateral force up to
//!   `throttle × force × gimbal` (gimbal = sin of the max angle) at the same point, signed by `deflect ∈ [−1, 1]`.
//! * **Torque sources** ([`TorqueSource`]): gyros and logic-core reaction torque: pure torque up to `max`,
//!   signed by `use ∈ [−1, 1]`.
//!
//! Algorithm (fixed passes, no iteration to convergence, so it is cheap and deterministic). The first line
//! ([`allocate`]) adds no force nobody asked for:
//!
//! 1. **Translation**: per axis group, the requested force (clamped to the group's capacity). Within a group,
//!    filling thrusters in order of torque-per-force gives the least and the most torque that force can make;
//!    a common blend `λ` of the two fills hits the torque closest to the request. This is free: it costs no
//!    translation. A symmetric ship asked for pure translation gets exactly symmetric throttles.
//! 2. **Pure torque** (gyros, cores) covers what is left, shared evenly.
//! 3. **RCS couples** (GDR-0055): RCS nozzles of opposite groups (forward and backward, right and left) whose
//!    torques have the needed sign fire with equal force, so their forces cancel and only the torque is left.
//!    A couple adds no net force, so it cannot push the ship past any rated speed: it uses the nozzles' full
//!    force, not the faded one (`Actuation::couple`).
//!
//! The last resort ([`assist`]) adds force nobody asked for; the flight computer opens it only after the first
//! line has been at its limit for a few ticks (GDR-0055, `FlightTuning::assist_after`):
//!
//! 4. **Gimbals** of firing engines (a small lateral force).
//! 5. **Differential thrust**, only while steering to a heading or a rate (not to stop a spin without one):
//!    thrusters whose torque has the needed sign are throttled up (strongest lever first). Rotation has priority
//!    over translation here.

use crate::math::Vec2;
use crate::ship::{Thruster, Wrench};

/// Axis groups: 0 = +y (forward), 1 = −y, 2 = +x, 3 = −x.
pub const GROUPS: usize = 4;

/// The group of an axis-aligned unit direction.
pub fn group_of(dir: Vec2) -> usize {
    if dir.y > 0.5 {
        0
    } else if dir.y < -0.5 {
        1
    } else if dir.x > 0.5 {
        2
    } else {
        3
    }
}

/// A pure torque actuator (gyro, logic core).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TorqueSource {
    pub part: crate::design::PartIdx,
    /// Max |torque| (kN·m) at full power.
    pub max: f32,
}

/// Orders of the thruster rows of one piece, built when the piece changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AllocTable {
    /// Rows per axis group, sorted by torque per force ascending (ties by row).
    groups: [Vec<u32>; GROUPS],
    /// Rows with positive torque per force, strongest first.
    pos: Vec<u32>,
    /// Rows with negative torque per force, strongest (most negative) first.
    neg: Vec<u32>,
}

impl AllocTable {
    pub fn build(th: &[Thruster]) -> AllocTable {
        let mut t = AllocTable::default();
        for (i, x) in th.iter().enumerate() {
            t.groups[group_of(x.dir)].push(i as u32);
            if x.torque_per_force > 0.0 {
                t.pos.push(i as u32);
            } else if x.torque_per_force < 0.0 {
                t.neg.push(i as u32);
            }
        }
        let k = |i: u32| th[i as usize].torque_per_force;
        for g in &mut t.groups {
            g.sort_by(|&a, &b| k(a).total_cmp(&k(b)).then(a.cmp(&b)));
        }
        t.pos
            .sort_by(|&a, &b| k(b).total_cmp(&k(a)).then(a.cmp(&b)));
        t.neg
            .sort_by(|&a, &b| k(a).total_cmp(&k(b)).then(a.cmp(&b)));
        t
    }
}

/// Actuator commands for one piece.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Actuation {
    /// Throttle per thruster row, 0..1 of its force after the rated-speed falloff.
    pub throttle: Vec<f32>,
    /// Gimbal deflection per thruster row, −1..1 of its lateral capacity.
    pub deflect: Vec<f32>,
    /// Use per torque source, −1..1 of its max torque.
    pub torque: Vec<f32>,
    /// Share of each RCS row's full force (not faded) fired in a couple (step 3), 0..1. The row's force is
    /// `throttle × faded force + couple × full force`, never more than its full force.
    pub couple: Vec<f32>,
    /// Cruise-flame level per thruster row, 0..1, set by the flight computer from the velocity goal (a look
    /// only; see `Piece::cruise`). Not an actuator command: it moves nothing.
    pub cruise: Vec<f32>,
}

impl Actuation {
    pub fn resize(&mut self, rows: usize, sources: usize) {
        self.throttle.clear();
        self.throttle.resize(rows, 0.0);
        self.deflect.clear();
        self.deflect.resize(rows, 0.0);
        self.couple.clear();
        self.couple.resize(rows, 0.0);
        self.cruise.clear();
        self.cruise.resize(rows, 0.0);
        self.torque.clear();
        self.torque.resize(sources, 0.0);
    }
}

/// Force (piece frame) and torque of an actuation, given the available force of each row after the rated-speed
/// falloff (`fmax`) and without it (`full`, for RCS couples), and the available torque of each source.
pub fn wrench_of(
    th: &[Thruster],
    fmax: &[f32],
    full: &[f32],
    sources_max: &[f32],
    a: &Actuation,
) -> Wrench {
    let mut w = Wrench::default();
    for (i, t) in th.iter().enumerate() {
        let pushed = a.throttle[i] * fmax[i];
        let f = pushed + a.couple[i] * full[i];
        if f == 0.0 {
            continue;
        }
        w.force += t.dir * f;
        w.torque += t.torque_per_force * f;
        let lat = a.deflect[i] * pushed * t.gimbal;
        if lat != 0.0 {
            w.force += t.lat * lat;
            w.torque += t.torque_per_lateral * lat;
        }
    }
    for (j, &m) in sources_max.iter().enumerate() {
        w.torque += a.torque[j] * m;
    }
    w
}

/// Missing torque (kN·m) up to this much counts as met, for a request of `torque`.
pub(crate) fn tolerance(torque: f32) -> f32 {
    1e-6 * (1.0 + torque.abs())
}

/// Greedy fill of `need` kN over `order`; returns the torque made. Adds `weight × throttle` to `out` if given.
fn greedy<I: Iterator<Item = u32>>(
    order: I,
    th: &[Thruster],
    fmax: &[f32],
    mut need: f32,
    mut out: Option<(&mut [f32], f32)>,
) -> f32 {
    let mut torque = 0.0f32;
    for i in order {
        if need <= 0.0 {
            break;
        }
        let f = fmax[i as usize];
        if f <= 0.0 {
            continue;
        }
        let take = if f < need { f } else { need };
        need -= take;
        torque += take * th[i as usize].torque_per_force;
        if let Some((o, w)) = out.as_mut() {
            o[i as usize] += *w * (take / f);
        }
    }
    torque
}

/// Strongest-lever-first fill of up to `need` kN over the RCS rows of group `g` that turn the `ccw` way, each
/// up to `room(row)` kN. Returns the force taken and the torque made (a magnitude). Adds `weight × take /
/// full[row]` to `out` if given.
fn couple_fill(
    th: &[Thruster],
    table: &AllocTable,
    g: usize,
    ccw: bool,
    room: &impl Fn(usize) -> f32,
    mut need: f32,
    mut out: Option<(&mut [f32], f32, &[f32])>,
) -> (f32, f32) {
    let rows = &table.groups[g];
    let n = rows.len();
    let (mut force, mut torque) = (0.0f32, 0.0f32);
    for k in 0..n {
        // Rows ascend by torque per force: the strongest clockwise levers come first, the strongest
        // counter-clockwise ones last.
        let i = rows[if ccw { n - 1 - k } else { k }] as usize;
        let lever = if ccw {
            th[i].torque_per_force
        } else {
            -th[i].torque_per_force
        };
        if lever <= 0.0 || need <= 0.0 {
            break;
        }
        if !th[i].rcs {
            continue;
        }
        let r = room(i);
        if r <= 0.0 {
            continue;
        }
        let take = if r < need { r } else { need };
        need -= take;
        force += take;
        torque += take * lever;
        if let Some((o, w, full)) = out.as_mut() {
            o[i] += *w * (take / full[i]);
        }
    }
    (force, torque)
}

/// The torque (a magnitude, kN·m) the RCS couples can make turning the `ccw` way when each row has `room(row)`
/// kN to spare: per pair of opposite groups (forward and backward, right and left) equal force from both,
/// strongest levers first, so the forces cancel. With `apply = Some((k, couple, full))` it also adds the share
/// `k` of that to `couple` (per row, a share of `full`).
pub fn couples(
    th: &[Thruster],
    table: &AllocTable,
    ccw: bool,
    room: &impl Fn(usize) -> f32,
    mut apply: Option<(f32, &mut [f32], &[f32])>,
) -> f32 {
    let mut torque = 0.0f32;
    for (a, b) in [(0, 1), (2, 3)] {
        let (fa, _) = couple_fill(th, table, a, ccw, room, f32::INFINITY, None);
        let (fb, _) = couple_fill(th, table, b, ccw, room, f32::INFINITY, None);
        let f = fa.min(fb);
        if f <= 0.0 {
            continue;
        }
        for g in [a, b] {
            let out = apply.as_mut().map(|t| (&mut *t.1, t.0, t.2));
            torque += couple_fill(th, table, g, ccw, room, f, out).1;
        }
    }
    torque
}

/// The first line (steps 1–3): allocates `force` (piece frame, kN) and `torque` (kN·m) over translation, pure
/// torque and RCS couples. `fmax[i]`: available force of row `i` now, after the rated-speed falloff (0 for rows
/// that cannot push this tick, and at the rating); `full[i]`: the same without the falloff (RCS couples);
/// `tmax[j]`: available torque of source `j`. Writes `out` (resized by the caller) and returns the torque still
/// missing (0 when met): what only the last resort ([`assist`]) could add.
#[allow(clippy::too_many_arguments)]
pub fn allocate(
    th: &[Thruster],
    table: &AllocTable,
    fmax: &[f32],
    full: &[f32],
    tmax: &[f32],
    force: Vec2,
    torque: f32,
    out: &mut Actuation,
) -> f32 {
    // 1. Translation per group, with the free torque choice.
    let want = [force.y, -force.y, force.x, -force.x];
    let mut fg = [0.0f32; GROUPS];
    let (mut lo, mut hi) = (0.0f32, 0.0f32);
    for g in 0..GROUPS {
        let cap: f32 = table.groups[g].iter().map(|&i| fmax[i as usize]).sum();
        fg[g] = want[g].clamp(0.0, cap);
        if fg[g] > 0.0 {
            lo += greedy(table.groups[g].iter().copied(), th, fmax, fg[g], None);
            hi += greedy(table.groups[g].iter().rev().copied(), th, fmax, fg[g], None);
        }
    }
    // The two fills can disagree in the last bit when they are mathematically equal.
    if lo > hi {
        std::mem::swap(&mut lo, &mut hi);
    }
    let target = torque.clamp(lo, hi);
    let lam = if hi - lo > 1e-6 {
        (target - lo) / (hi - lo)
    } else {
        0.5
    };
    for (rows, &f) in table.groups.iter().zip(&fg) {
        if f <= 0.0 {
            continue;
        }
        if lam < 1.0 {
            greedy(
                rows.iter().copied(),
                th,
                fmax,
                f,
                Some((&mut out.throttle, 1.0 - lam)),
            );
        }
        if lam > 0.0 {
            greedy(
                rows.iter().rev().copied(),
                th,
                fmax,
                f,
                Some((&mut out.throttle, lam)),
            );
        }
    }
    for t in &mut out.throttle {
        *t = t.clamp(0.0, 1.0);
    }
    let mut made = 0.0f32;
    for (i, t) in th.iter().enumerate() {
        made += out.throttle[i] * fmax[i] * t.torque_per_force;
    }
    let mut rest = torque - made;
    let tol = tolerance(torque);

    // 2. Pure torque sources, shared evenly.
    if rest.abs() > tol {
        let cap: f32 = tmax.iter().sum();
        if cap > 0.0 {
            let frac = (rest / cap).clamp(-1.0, 1.0);
            for (j, &m) in tmax.iter().enumerate() {
                if m > 0.0 {
                    out.torque[j] = frac;
                }
            }
            rest -= frac * cap;
        }
    }

    // 3. RCS couples, all scaled down alike when they could do more than needed (the forces still cancel).
    if rest.abs() > tol {
        let Actuation {
            throttle, couple, ..
        } = out;
        let room = |i: usize| full[i] - throttle[i] * fmax[i];
        let ccw = rest > 0.0;
        let cap = couples(th, table, ccw, &room, None);
        if cap > 0.0 {
            let k = (rest.abs() / cap).min(1.0);
            couples(th, table, ccw, &room, Some((k, couple, full)));
            rest -= if ccw { k * cap } else { -k * cap };
        }
    }
    if rest.abs() > tol { rest } else { 0.0 }
}

/// The last resort (steps 4 and 5) for the torque `rest` that the first line ([`allocate`]) left missing:
/// gimbals of the firing engines, then, if `differential` (the flight computer allows it only while it steers
/// to a heading or a rate, never to stop a spin without one), differential thrust. Both add force nobody asked
/// for. Adds to `out` and returns the torque still missing.
pub fn assist(
    th: &[Thruster],
    table: &AllocTable,
    fmax: &[f32],
    full: &[f32],
    mut rest: f32,
    differential: bool,
    out: &mut Actuation,
) -> f32 {
    let tol = tolerance(rest);

    // 4. Gimbals of firing engines.
    if rest.abs() > tol {
        let mut cap = 0.0f32;
        for (i, t) in th.iter().enumerate() {
            cap += out.throttle[i] * fmax[i] * t.gimbal * t.torque_per_lateral.abs();
        }
        if cap > 0.0 {
            let frac = (rest / cap).clamp(-1.0, 1.0);
            for (i, t) in th.iter().enumerate() {
                if t.gimbal > 0.0 && out.throttle[i] > 0.0 && t.torque_per_lateral != 0.0 {
                    out.deflect[i] = if t.torque_per_lateral > 0.0 {
                        frac
                    } else {
                        -frac
                    };
                }
            }
            rest -= frac * cap;
        }
    }

    // 5. Differential thrust: rotation over translation. A row never pushes more than its faded force, nor more
    // than its full force together with what a couple already takes.
    let room = |i: usize, out: &Actuation| {
        let pushed = out.throttle[i] * fmax[i];
        ((1.0 - out.throttle[i]) * fmax[i]).min(full[i] - pushed - out.couple[i] * full[i])
    };
    if !differential {
        // Stopping a spin without a heading: no unrequested push.
    } else if rest > tol {
        for &i in &table.pos {
            let i = i as usize;
            let k = th[i].torque_per_force;
            let r = room(i, out) * k;
            if r <= 0.0 {
                continue;
            }
            let take = if r < rest { r } else { rest };
            out.throttle[i] += take / (fmax[i] * k);
            rest -= take;
            if rest <= tol {
                break;
            }
        }
    } else if rest < -tol {
        for &i in &table.neg {
            let i = i as usize;
            let k = -th[i].torque_per_force;
            let r = room(i, out) * k;
            if r <= 0.0 {
                continue;
            }
            let take = if r < -rest { r } else { -rest };
            out.throttle[i] += take / (fmax[i] * k);
            rest += take;
            if rest >= -tol {
                break;
            }
        }
    }
    for t in &mut out.throttle {
        *t = t.clamp(0.0, 1.0);
    }
    rest
}
