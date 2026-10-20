//! Blink drives (GDR-0127, the Phase walkers): a drive (content `blink`) jumps its vessel's controlling piece a short
//! way, keeping its velocity, heading and spin.
//!
//! * **Asking**: the BLINK key (`ShipCommand::Blink` with a world direction), the behavior action `blink(behind)` /
//!   `blink(away)` (relative to the target). [`Ship::ask_blink`] keeps the ask only while [`Ship::can_blink`].
//! * **Which drives**: the working drives (alive, not overheated, not idle, cooled down) on the active controlling
//!   piece, in part order, until their `mass` covers the piece's mass; with too few, no blink. A heavy ship needs
//!   several drives, and a blink takes them all.
//! * **Charge**: the drives charge together for the longest `charge` of them at full power, drawing their power only
//!   now (`ConsumerKind::Blink`); with less power, slower (the lowest work of them counts). The client shimmers the
//!   hull meanwhile ([`Ship::blink_charge`]). A drive that dies or leaves the controlling piece, or a vessel that
//!   stops running, breaks the charge off (no cooldown).
//! * **Jump**: at full charge the piece's frame moves by the jump: `Dir` the full `range` (the shortest of the
//!   drives) along the direction; `Behind` towards a spot [`tuning::BEHIND`] m past the target's tail (beyond its
//!   bounding radius), at most `range`; `Away` the full range straight away from the target. A target that is gone
//!   breaks the charge off. The landing spot must be clear: no other ship's body (vessel, salvage, fragment) and no
//!   rock within the piece's bounding radius plus [`tuning::CLEARANCE`], and no spot another blink took this tick.
//!   The full jump is tried first, then [`tuning::TRIES`] of it; none clear, the blink fizzles
//!   (`SimEvent::BlinkBlocked`) and the drives cool down [`tuning::FIZZLE`] of their cooldown. A jump cools every drive
//!   it took for its `cooldown` (`SimEvent::Blinked`). The path is not checked: a phase jump passes through
//!   anything.
//!
//! The step runs after the weapons and salvage, before the physics step, in ship order, so the next physics step
//! starts from the new pose. Deterministic: no randomness; the clearance test asks the physics (Rapier's query
//! pipeline, deterministic mode). Hashed only on ships with drives (their cooldowns) and while a charge runs.

use crate::DT;
use crate::events::SimEvent;
use crate::hash::{StateHash, StateHasher};
use crate::ids::ShipId;
use crate::math::{Rot, Vec2};
use crate::ship::Ship;
use crate::world::Sim;

/// Blink tuning. **PLACEHOLDERS** (lead defaults, GDR-0127).
pub mod tuning {
    /// Landing spots tried, as shares of the jump; the first clear one wins.
    pub const TRIES: [f32; 4] = [1.0, 0.75, 0.5, 0.25];
    /// Room kept free round the piece's bounding circle at a landing spot (m).
    pub const CLEARANCE: f32 = 2.0;
    /// `blink(behind)` aims this far past the target's tail, beyond its bounding radius (m).
    pub const BEHIND: f32 = 12.0;
    /// A blink with no clear landing spot cools its drives down this share of their cooldown.
    pub const FIZZLE: f32 = 0.5;
}

/// Where a blink goes ([`Ship::ask_blink`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlinkAim {
    /// Along this world direction, the full range.
    Dir(Vec2),
    /// Towards a spot behind this ship's tail, as far as the range reaches.
    Behind(ShipId),
    /// Straight away from this ship, the full range.
    Away(ShipId),
}

impl StateHash for BlinkAim {
    fn hash_state(&self, h: &mut StateHasher) {
        match *self {
            BlinkAim::Dir(d) => {
                h.u8(0);
                h.vec2(d);
            }
            BlinkAim::Behind(s) => {
                h.u8(1);
                h.u32(s.0);
            }
            BlinkAim::Away(s) => {
                h.u8(2);
                h.u32(s.0);
            }
        }
    }
}

/// A ship's blink ([`Ship::blink`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Blink {
    /// No blink charging (its drives may still be cooling down).
    #[default]
    Ready,
    /// Charging on the drives in `drives` (a bit per index into `ShipDesign::blinks`): `charged` seconds at full power
    /// of the `charge` they need.
    Charging { drives: u64, charged: f32, charge: f32, aim: BlinkAim },
}

impl StateHash for Blink {
    fn hash_state(&self, h: &mut StateHasher) {
        match *self {
            Blink::Ready => h.u8(0),
            Blink::Charging { drives, charged, charge, aim } => {
                h.u8(1);
                h.u64(drives);
                h.f32(charged);
                h.f32(charge);
                aim.hash_state(h);
            }
        }
    }
}

impl Ship {
    /// The blink underway, if one charges.
    pub fn blink(&self) -> Blink {
        self.blink
    }

    /// How far the charge has come (0..1) while a blink charges: the hull shimmers.
    pub fn blink_charge(&self) -> Option<f32> {
        match self.blink {
            Blink::Charging { charged, charge, .. } => Some((charged / charge.max(1e-6)).clamp(0.0, 1.0)),
            Blink::Ready => None,
        }
    }

    /// Seconds until the ship's drives could blink again, by the longest cooldown among its drives on the
    /// controlling piece (0: cool; the HUD).
    pub fn blink_cooldown(&self) -> f32 {
        let Some(control) = self.control else { return 0.0 };
        let mounts = &self.design.blinks;
        (0..mounts.len())
            .filter(|&i| {
                let ps = &self.parts[mounts[i].part as usize];
                ps.alive && ps.piece == control
            })
            .fold(0.0, |m, i| m.max(self.blink_cool[i]))
    }

    /// Does the ship carry a blink drive on its controlling piece (alive)?
    pub fn has_blink(&self) -> bool {
        let Some(control) = self.control else { return false };
        self.design.blinks.iter().any(|m| {
            let ps = &self.parts[m.part as usize];
            ps.alive && ps.piece == control
        })
    }

    /// The drives a blink would take now (a bit per mount) and the charge they need, if their load covers the
    /// controlling piece's mass (see the module docs).
    fn blink_drives(&self) -> Option<(u64, f32)> {
        let control = self.control?;
        let piece = self.piece(control).filter(|p| p.active)?;
        let need = piece.mass.mass;
        let (mut mask, mut load, mut charge) = (0u64, 0.0f32, 0.0f32);
        for (i, m) in self.design.blinks.iter().enumerate().take(64) {
            let ps = &self.parts[m.part as usize];
            if !ps.alive || ps.piece != control || ps.overheated || ps.idle || self.blink_cool[i] > 0.0 {
                continue;
            }
            mask |= 1 << i;
            load += m.spec.mass;
            charge = charge.max(m.spec.charge);
            if load >= need {
                return Some((mask, charge));
            }
        }
        None
    }

    /// Could a blink start now: none charging, and enough working, cool drives on the running vessel to carry it?
    pub fn can_blink(&self) -> bool {
        self.blink == Blink::Ready && self.is_active() && self.blink_drives().is_some()
    }

    /// Asks for a blink this tick (started in the blink step); ignored while none can start.
    pub(crate) fn ask_blink(&mut self, aim: BlinkAim) {
        if self.blink_asked.is_none() && self.can_blink() {
            self.blink_asked = Some(aim);
        }
    }
}

/// Is a blink charging on `part` (its power row draws only then)? For the systems pass, which holds the ship's
/// fields apart.
pub(crate) fn blink_charging(blink: &Blink, design: &crate::ship::design::ShipDesign, part: crate::ship::design::PartIdx) -> bool {
    match *blink {
        Blink::Charging { drives, .. } => design
            .blinks
            .iter()
            .enumerate()
            .take(64)
            .any(|(i, m)| m.part == part && drives & (1 << i) != 0),
        Blink::Ready => false,
    }
}

impl Sim {
    /// Once per tick after the weapons and salvage, before the physics step: drives cool down, charges fill and jump,
    /// asked blinks start charging (the module docs).
    pub(crate) fn blinks_step(&mut self) {
        let mut landed: Vec<(Vec2, f32)> = Vec::new();
        for k in 0..self.ships.len() {
            if self.ships[k].design.blinks.is_empty() {
                continue;
            }
            let (blink, asked) = {
                let s = &mut self.ships[k];
                for c in &mut s.blink_cool {
                    *c = (*c - DT).max(0.0);
                }
                (s.blink, s.blink_asked.take())
            };
            if let Blink::Charging { drives, charged, charge, aim } = blink {
                match charge_work(&self.ships[k], drives) {
                    None => self.ships[k].blink = Blink::Ready,
                    Some(work) => {
                        let charged = charged + work * DT;
                        self.ships[k].blink = Blink::Charging { drives, charged, charge, aim };
                        if charged >= charge {
                            self.jump(k, drives, aim, &mut landed);
                        }
                    }
                }
                continue;
            }
            let s = &mut self.ships[k];
            if let Some(aim) = asked
                && let Some((drives, charge)) = s.blink_drives()
            {
                s.blink = Blink::Charging { drives, charged: 0.0, charge, aim };
            }
        }
    }

    /// Ship `k`'s charge is full: it jumps (or fizzles) and its drives cool down.
    fn jump(&mut self, k: usize, drives: u64, aim: BlinkAim, landed: &mut Vec<(Vec2, f32)>) {
        self.ships[k].blink = Blink::Ready;
        let ship = &self.ships[k];
        let id = ship.id();
        let Some(piece) = ship.control_piece().and_then(|c| ship.piece(c)) else { return };
        let (pid, body, from, radius) = (piece.id, piece.body, piece.com_world(), piece.radius + tuning::CLEARANCE);
        let mounts = &ship.design.blinks;
        let range = (0..mounts.len().min(64))
            .filter(|&i| drives & (1 << i) != 0)
            .map(|i| mounts[i].spec.range)
            .fold(f32::INFINITY, f32::min);
        let nose = Rot::from_angle(body.angle).apply(Vec2::new(0.0, 1.0));
        let unit = |v: Vec2, or: Vec2| {
            let l = v.length();
            if l > 1e-4 { v * (1.0 / l) } else { or }
        };
        let centre_of = |s: ShipId| -> Option<(Vec2, f32, f32)> {
            let t = self.ship(s)?;
            let p = t.control_piece().and_then(|c| t.piece(c)).or(t.pieces().first())?;
            Some((p.com_world(), p.body.angle, p.radius))
        };
        let jump = match aim {
            BlinkAim::Dir(d) => Some(unit(d, nose) * range),
            BlinkAim::Behind(t) => centre_of(t).map(|(at, angle, r)| {
                let tail = Rot::from_angle(angle).apply(Vec2::new(0.0, -1.0));
                let v = at + tail * (r + tuning::BEHIND) - from;
                let l = v.length();
                if l > range { v * (range / l) } else { v }
            }),
            BlinkAim::Away(t) => centre_of(t).map(|(at, _, _)| unit(from - at, -nose) * range),
        };
        // A target that is gone: the charge breaks off, nothing spent.
        let Some(jump) = jump else { return };
        let clear = |d: Vec2| {
            let at = body.pos + d;
            self.physics.circle_clear(id, at, radius)
                && landed.iter().all(|&(c, r)| (c - at).length() > r + radius)
        };
        let to = tuning::TRIES.iter().map(|&f| jump * f).find(|&d| clear(d));
        let cool = |s: &mut Ship, share: f32| {
            for i in 0..s.design.blinks.len().min(64) {
                if drives & (1 << i) != 0 {
                    s.blink_cool[i] = s.design.blinks[i].spec.cooldown * share;
                }
            }
        };
        match to {
            Some(d) => {
                landed.push((body.pos + d, radius));
                cool(&mut self.ships[k], 1.0);
                self.teleport(id, pid, crate::ship::Body { pos: body.pos + d, ..body });
                self.events.push(SimEvent::Blinked { ship: id, piece: pid, from, to: from + d });
            }
            None => {
                cool(&mut self.ships[k], tuning::FIZZLE);
                self.events.push(SimEvent::BlinkBlocked { ship: id, at: from, to: from + jump });
            }
        }
    }
}

/// The work the charge fills by this tick (the lowest of its drives'), or `None` when it breaks off: a drive dead,
/// off the controlling piece, or the vessel not running.
fn charge_work(s: &Ship, drives: u64) -> Option<f32> {
    let control = s.control_piece()?;
    if !s.is_active() {
        return None;
    }
    let mut work = f32::INFINITY;
    for (i, m) in s.design.blinks.iter().enumerate().take(64) {
        if drives & (1 << i) == 0 {
            continue;
        }
        let ps = &s.parts[m.part as usize];
        if !ps.alive || ps.piece != control {
            return None;
        }
        work = work.min(ps.work);
    }
    work.is_finite().then_some(work)
}
