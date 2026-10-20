//! Overdrive (GDR-0004's burst output past overheat at the risk of damage; GDR-0132): any vessel can drive every
//! part past its limits for a few seconds, ship-wide (the OVERDRIVE key, the `overdrive` behavior action).
//!
//! * **Asking**: [`Ship::ask_overdrive`] keeps the ask only while [`Ship::can_overdrive`]: a running vessel (its
//!   controlling piece active) neither overdriven nor cooling down. It starts in the systems pass of that tick
//!   (`SimEvent::Overdrive`).
//! * **For [`tuning::SECONDS`]** every piece of the ship works at [`tuning::BOOST`] times its output: its reactors'
//!   supply, every consumer's and reactor's work (guns fire faster and beams burn harder, shields regenerate faster,
//!   scans, blinks and EMPs charge faster), and the force and torque of its engines, RCS pods, gyros and cores (the
//!   flight computer plans with the boosted authority; rated speeds stay, so it accelerates and turns harder, it
//!   flies no faster).
//! * **Heat**: every part makes [`tuning::HEAT`] times the heat of its (boosted) work, so heat climbs fast. Nothing
//!   shuts down for heat meanwhile: a part past its heat limit keeps working and **burns**, losing [`tuning::BURN`]
//!   hit points per second times its heat over its limit (at the limit 0.5, at twice the limit 1). A part that
//!   burns away is destroyed like any other: an exploding part (a reactor, a capacitor, a drive) blasts its
//!   neighbours, and the blast can chain (GDR-0084, GDR-0085).
//! * **Then** the boost ends (`SimEvent::OverdriveEnded`); a part still past its limit shuts down as usual until it
//!   cools (the crash after the burst), and the ship cannot overdrive again for [`tuning::COOLDOWN`] seconds.
//!   A ship that stops being a vessel ends it at once.
//!
//! Hashed only while overdriven or cooling down, so worlds without overdrive keep their hashes. Numbers in
//! [`tuning`] are **placeholders** (lead defaults, 20 Oct 2026).

use crate::DT;
use crate::hash::{StateHash, StateHasher};

use super::*;

/// Overdrive tuning. **PLACEHOLDERS** (lead defaults, 20 Oct 2026, GDR-0132).
pub mod tuning {
    /// Seconds an overdrive lasts.
    pub const SECONDS: f32 = 5.0;
    /// Output multiplier while overdriven (power, work, thrust, torque).
    pub const BOOST: f32 = 1.5;
    /// Heat multiplier on top of the boosted work while overdriven.
    pub const HEAT: f32 = 1.5;
    /// Hit points per second a part past its heat limit loses while overdriven, per unit of `heat / limit`.
    pub const BURN: f32 = 0.5;
    /// Seconds after an overdrive ends before the ship can overdrive again.
    pub const COOLDOWN: f32 = 30.0;
}

/// A ship's overdrive: seconds left overdriven, then seconds left cooling down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overdrive {
    pub left: f32,
    pub cool: f32,
}

impl Overdrive {
    /// Neither overdriven nor cooling down (not hashed).
    pub fn is_clear(&self) -> bool {
        *self == Overdrive::default()
    }
}

impl StateHash for Overdrive {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.left);
        h.f32(self.cool);
    }
}

/// What a ship's overdrive did this tick ([`Ship::overdrive_tick`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverdriveTick {
    Nothing,
    Started,
    Ended,
}

impl Ship {
    /// Its overdrive state.
    pub fn overdrive(&self) -> Overdrive {
        self.overdrive
    }

    /// Is it overdriven now?
    pub fn overdriven(&self) -> bool {
        self.overdrive.left > 0.0
    }

    /// Could it overdrive now: a running vessel (its controlling piece active), neither overdriven nor cooling down.
    pub fn can_overdrive(&self) -> bool {
        self.overdrive.is_clear()
            && !self.dormant
            && self.control.and_then(|c| self.piece(c)).is_some_and(|p| p.active)
    }

    /// Asks for an overdrive this tick (started in the systems pass); ignored while it cannot.
    pub(crate) fn ask_overdrive(&mut self) {
        self.overdrive_asked |= self.can_overdrive();
    }

    /// Before the systems pass: an asked overdrive starts, a running one counts down into its cooldown, the cooldown
    /// runs out; a ship that is no vessel any more ends it.
    pub(crate) fn overdrive_tick(&mut self) -> OverdriveTick {
        let asked = std::mem::take(&mut self.overdrive_asked);
        let o = &mut self.overdrive;
        if o.left > 0.0 {
            o.left -= DT;
            if o.left <= 0.0 || self.control.is_none() {
                *o = Overdrive { left: 0.0, cool: tuning::COOLDOWN };
                return OverdriveTick::Ended;
            }
        } else if o.cool > 0.0 {
            o.cool = (o.cool - DT).max(0.0);
        } else if asked && self.can_overdrive() {
            self.overdrive.left = tuning::SECONDS;
            return OverdriveTick::Started;
        }
        OverdriveTick::Nothing
    }

    /// While overdriven, after the heat step: every alive part past its heat limit burns ([`tuning::BURN`]).
    pub(crate) fn overdrive_burn(&mut self) {
        if !self.overdriven() {
            return;
        }
        for p in 0..self.parts.len() {
            let ps = &self.parts[p];
            let limit = self.heat_k[p].limit;
            if ps.alive && limit > 0.0 && ps.heat > limit {
                let burn = tuning::BURN * ps.heat / limit * DT;
                self.apply_damage(crate::ship::breakage::DamageTarget::Part(p as PartIdx), burn);
            }
        }
    }
}
