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
//! Algorithm (fixed passes, no iteration to convergence, so it is cheap and deterministic):
//!
//! 1. **Translation**: per axis group, the requested force (clamped to the group's capacity). Within a group,
//!    filling thrusters in order of torque-per-force gives the least and the most torque that force can make;
//!    a common blend `λ` of the two fills hits the torque closest to the request. This is free: it costs no
//!    translation. A symmetric ship asked for pure translation gets exactly symmetric throttles.
//! 2. **Pure torque** (gyros, cores) covers what is left, shared evenly.
//! 3. **Gimbals** of firing engines cover what is still left (adds a small lateral force).
//! 4. **Differential thrust**: thrusters whose torque has the needed sign are throttled up (strongest lever first).
//!    Rotation has priority over translation here: it adds unrequested force.

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
            if !x.clear {
                continue;
            }
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
    /// Throttle per thruster row.
    pub throttle: Vec<f32>,
    /// Gimbal deflection per thruster row, −1..1 of its lateral capacity.
    pub deflect: Vec<f32>,
    /// Use per torque source, −1..1 of its max torque.
    pub torque: Vec<f32>,
}

impl Actuation {
    pub fn resize(&mut self, rows: usize, sources: usize) {
        self.throttle.clear();
        self.throttle.resize(rows, 0.0);
        self.deflect.clear();
        self.deflect.resize(rows, 0.0);
        self.torque.clear();
        self.torque.resize(sources, 0.0);
    }
}

/// Force (piece frame) and torque of an actuation, given the available force of each row (`fmax`) and the
/// available torque of each source (`tmax`).
pub fn wrench_of(th: &[Thruster], fmax: &[f32], sources_max: &[f32], a: &Actuation) -> Wrench {
    let mut w = Wrench::default();
    for (i, t) in th.iter().enumerate() {
        let f = a.throttle[i] * fmax[i];
        if f == 0.0 {
            continue;
        }
        w.force += t.dir * f;
        w.torque += t.torque_per_force * f;
        let lat = a.deflect[i] * f * t.gimbal;
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

/// Allocates `force` (piece frame, kN) and `torque` (kN·m) over the actuators. `fmax[i]`: available force of
/// row `i` now (0 for blocked, overheated or dead engines); `tmax[j]`: available torque of source `j`.
/// `differential`: may add unrequested thrust to turn (step 4); the flight computer allows it only for real
/// turns, not for holding a heading. Writes `out` (resized by the caller) and returns the expected wrench.
#[allow(clippy::too_many_arguments)]
pub fn allocate(
    th: &[Thruster],
    table: &AllocTable,
    fmax: &[f32],
    tmax: &[f32],
    force: Vec2,
    torque: f32,
    differential: bool,
    out: &mut Actuation,
) -> Wrench {
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
    let mut made = 0.0f32;
    for (i, t) in th.iter().enumerate() {
        made += out.throttle[i] * fmax[i] * t.torque_per_force;
    }
    let mut rest = torque - made;
    let tol = 1e-6 * (1.0 + torque.abs());

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

    // 3. Gimbals of firing engines.
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

    // 4. Differential thrust: rotation over translation.
    if !differential {
        // nothing
    } else if rest > tol {
        for &i in &table.pos {
            let (f, k) = (fmax[i as usize], th[i as usize].torque_per_force);
            let room = (1.0 - out.throttle[i as usize]) * f * k;
            if room <= 0.0 {
                continue;
            }
            let take = if room < rest { room } else { rest };
            out.throttle[i as usize] += take / (f * k);
            rest -= take;
            if rest <= tol {
                break;
            }
        }
    } else if rest < -tol {
        for &i in &table.neg {
            let (f, k) = (fmax[i as usize], th[i as usize].torque_per_force);
            let room = (1.0 - out.throttle[i as usize]) * f * -k;
            if room <= 0.0 {
                continue;
            }
            let take = if room < -rest { room } else { -rest };
            out.throttle[i as usize] += take / (f * -k);
            rest += take;
            if rest >= -tol {
                break;
            }
        }
    }
    for t in &mut out.throttle {
        *t = t.clamp(0.0, 1.0);
    }
    wrench_of(th, fmax, tmax, out)
}
