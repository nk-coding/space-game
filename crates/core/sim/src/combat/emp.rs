//! EMP bursts (GDR-0018's second disruption weapon; GDR-0132): an EMP emitter (content `emp`) releases a ring of
//! static round its vessel when asked (the EMP key, the behavior action `emp`), then recharges.
//!
//! * **Release**: the first emitter (by part) on the vessel's active controlling piece that is alive, not overheated,
//!   not idle and recharged. The burst's centre is the emitter's centre.
//! * **Reach**: every other ship that is a vessel, not friendly to the emitter's owner (never the own ship, the own
//!   fleet or its drones: the burst knows its own) and whose controlling piece's centre of mass lies within `radius`
//!   m of the centre. Salvage, wrecks, rocks and missiles take nothing.
//! * **Static**: each takes `static` (no core bonus: the ring is everywhere at once). Its shield bubbles that cover
//!   its centre of mass but not the burst's centre take it first, point for point as they take a shot's damage, in
//!   their `Disruption` share (GDR-0125: every emitter absorbs it whole); what is left fills its cores like an ion
//!   gun's static ([`Sim::add_static`]): at its limit (2 per alive core) it is blinded for 3 s, then immune for 4 s.
//!   A drone holds 2 static, so a burst it takes in full blinds it, and a blinded drone falls apart (`ship/drones.rs`).
//! * **Recharge**: the emitter then recharges for `cooldown` seconds at full power, drawing its power only now
//!   (`ConsumerKind::Emp`); with less power, slower; overdriven, faster. A dead or idle emitter keeps what it still
//!   needs.
//!
//! The burst is instant (the client draws the ring running out). Deterministic: ship, emitter and bubble order, IEEE
//! `+ − × ÷` and comparisons only. Hashed per ship only if it has emitters, so worlds without them keep their hashes.

use crate::DT;
use crate::events::SimEvent;
use crate::math::Vec2;
use crate::ship::Ship;
use crate::ship::design::{PartIdx, ShipDesign, family_index};
use crate::ship::mass::to_piece;
use crate::ship::vessel::friendly;
use crate::world::Sim;

/// Is EMP emitter `part` recharging now (it draws its power then)?
pub(crate) fn recharging(cool: &[f32], d: &ShipDesign, part: PartIdx) -> bool {
    d.emps.iter().zip(cool).any(|(m, &c)| m.part == part && c > 0.0)
}

impl Ship {
    /// The emitter (index into `ShipDesign::emps`) a burst would use now: the first on the active controlling piece
    /// that is alive, not overheated, not idle and recharged.
    fn ready_emp(&self) -> Option<usize> {
        let control = self.control_piece()?;
        if !self.is_active() || !self.piece(control).is_some_and(|p| p.active) {
            return None;
        }
        let mounts = &self.design().emps;
        (0..mounts.len()).find(|&i| {
            let ps = &self.parts[mounts[i].part as usize];
            ps.alive && !ps.overheated && !ps.idle && ps.piece == control && self.emp_cool[i] <= 0.0
        })
    }

    /// Whether a burst would happen now (a working emitter recharged).
    pub fn can_emp(&self) -> bool {
        self.ready_emp().is_some()
    }

    /// Does the ship carry an EMP emitter on its controlling piece (alive)?
    pub fn has_emp(&self) -> bool {
        let Some(control) = self.control else { return false };
        self.design.emps.iter().any(|m| {
            let ps = &self.parts[m.part as usize];
            ps.alive && ps.piece == control
        })
    }

    /// How far the readiest emitter on the controlling piece has recharged (0..1; 1: ready; `None` without one).
    pub fn emp_charge(&self) -> Option<f32> {
        let control = self.control?;
        let mounts = &self.design.emps;
        (0..mounts.len())
            .filter(|&i| {
                let ps = &self.parts[mounts[i].part as usize];
                ps.alive && ps.piece == control
            })
            .map(|i| 1.0 - (self.emp_cool[i] / mounts[i].spec.cooldown.max(1e-6)).clamp(0.0, 1.0))
            .reduce(f32::max)
    }

    /// Asks for a burst this tick (handled in the weapons step); ignored while none is ready.
    pub(crate) fn ask_emp(&mut self) {
        self.emp_asked |= self.can_emp();
    }
}

impl Sim {
    /// Once per tick in the weapons step, after the bubbles are collected: emitters recharge by their work, asked
    /// bursts go off (see the module docs).
    pub(crate) fn emp_step(&mut self) {
        for k in 0..self.ships.len() {
            let s = &mut self.ships[k];
            if s.emp_cool.is_empty() {
                s.emp_asked = false;
                continue;
            }
            for i in 0..s.emp_cool.len() {
                if s.emp_cool[i] > 0.0 {
                    let work = s.parts[s.design.emps[i].part as usize].work;
                    s.emp_cool[i] = (s.emp_cool[i] - work * DT).max(0.0);
                }
            }
            if std::mem::take(&mut self.ships[k].emp_asked)
                && let Some(i) = self.ships[k].ready_emp()
            {
                self.burst(k, i);
            }
        }
    }

    /// Emitter `i` of ship `k` releases its burst (it was ready).
    fn burst(&mut self, k: usize, i: usize) {
        let ship = &self.ships[k];
        let mount = ship.design().emps[i];
        let Some(piece) = ship.piece(ship.parts[mount.part as usize].piece) else {
            return;
        };
        let at = piece.body.pos + crate::math::Rot::from_angle(piece.body.angle).apply(to_piece(mount.centre, piece.origin));
        let (by, team) = (ship.id(), ship.team());
        let (r2, charge) = (mount.spec.radius * mount.spec.radius, mount.spec.charge);
        let fi = family_index(data::WeaponFamily::Disruption);
        let mut struck = 0;
        for j in 0..self.ships.len() {
            let s = &self.ships[j];
            if j == k || !s.is_vessel() || friendly(team, s.team()) {
                continue;
            }
            let Some(point) = s.control.and_then(|c| s.piece(c)).map(|p| p.com_world()) else {
                continue;
            };
            if (point - at).length_squared() > r2 {
                continue;
            }
            struck += 1;
            let left = self.emp_shields(j, by, at, point, fi, charge);
            if left > 0.0 {
                self.add_static(by, j, left);
            }
        }
        self.ships[k].emp_cool[i] = mount.spec.cooldown;
        self.events.push(SimEvent::EmpBurst { ship: by, part: mount.part, at, radius: mount.spec.radius, struck });
    }

    /// The bubbles of ship `j` that cover `point` but not the burst's `centre` take their share of `amount` static
    /// first (bubble order); returns what is left.
    fn emp_shields(&mut self, j: usize, by: crate::ids::ShipId, centre: Vec2, point: Vec2, fi: usize, mut amount: f32) -> f32 {
        for b in 0..self.bubbles.len() {
            if amount <= 1e-6 {
                return 0.0;
            }
            let bub = &self.bubbles[b];
            let r2 = bub.radius * bub.radius;
            if bub.ship_index != j
                || bub.left <= 0.0
                || bub.absorb[fi] <= 0.0
                || !self.ships[j].parts[bub.part as usize].alive
                || (point - bub.centre).length_squared() > r2
                || (centre - bub.centre).length_squared() <= r2
            {
                continue;
            }
            amount -= self.absorb(b, fi, by, point, amount);
        }
        amount
    }
}
