//! Disruption (GDR-0018's ion and EMP family; the details GDR-0125, lead defaults): guns whose hits carry
//! **static** into the struck vessel's logic cores. Enough static **blinds** the vessel's mind for a while.
//!
//! * A disruption gun's hit (content `weapon.disrupt`, static per second; per point of damage that reaches a part,
//!   `PartSpec::disrupt`) adds `damage × disrupt` static to the struck ship, if the part is on its controlling
//!   piece (the vessel), [`tuning::CORE_HIT`] times that on a logic or automaton core or a sensor. Shields take
//!   the shot as any (the family's `absorb` share, the base emitters whole): static rides only on what reaches the
//!   hull. Armour does not stop it (the hull conducts it to the cores), and it deals the shot's small damage as usual.
//! * The static a vessel holds bleeds off at [`tuning::BLEED`] per second. When it reaches the vessel's limit,
//!   [`tuning::STATIC_PER_CORE`] for each alive core on its controlling piece (bigger minds resist more), the vessel
//!   is **blinded** for [`tuning::BLIND`] seconds (`SimEvent::Blinded`), then **immune** for [`tuning::IMMUNE`]
//!   seconds; meanwhile static does not build up (no endless lock).
//! * **Blinded** (`Ship::blinded`): its sensors see nothing (`Ship::sensor_range` 0), so its behavior and pilot
//!   assists pick no target (the `locked` selector too), conditions about enemies see none (`enemies_near` 0,
//!   `under_attack` and `missiles_incoming` nothing; target conditions fail with no target), and fire at will
//!   holds. It flies the rules that need no target (an escort keeps its leader). The piloted focus keeps its stick,
//!   lock and trigger: only its assists are blind.
//!
//! The per-ship state ([`Jam`]) is hashed only while it is not clear, so worlds without disruption keep their hashes.
//! Numbers in [`tuning`] are **placeholders** (lead defaults, 20 Oct 2026).

use crate::DT;
use crate::events::SimEvent;
use crate::hash::{StateHash, StateHasher};
use crate::ids::ShipId;
use crate::ship::Ship;
use crate::ship::design::PartIdx;
use crate::world::Sim;

/// Disruption tuning. **PLACEHOLDERS** (lead defaults, 20 Oct 2026, GDR-0125).
pub mod tuning {
    /// Static a vessel holds before it is blinded, per alive logic or automaton core on its controlling piece.
    pub const STATIC_PER_CORE: f32 = 2.0;
    /// A hit on a core (or a sensor part) carries this many times the static.
    pub const CORE_HIT: f32 = 2.0;
    /// Static a vessel sheds per second.
    pub const BLEED: f32 = 0.4;
    /// Seconds a vessel stays blinded.
    pub const BLIND: f32 = 3.0;
    /// Seconds after a blinding in which static does not build up.
    pub const IMMUNE: f32 = 4.0;
}

/// A ship's disruption state: the static it holds, and the seconds left blinded or immune.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Jam {
    pub charge: f32,
    pub blind: f32,
    pub immune: f32,
}

impl Jam {
    /// Nothing held, nothing running: the state of every ship no disruption gun has hit (not hashed).
    pub fn is_clear(&self) -> bool {
        *self == Jam::default()
    }
}

impl StateHash for Jam {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.charge);
        h.f32(self.blind);
        h.f32(self.immune);
    }
}

impl Ship {
    /// Its disruption state (GDR-0125).
    pub fn jam(&self) -> Jam {
        self.jam
    }

    /// Its mind is blinded by static (GDR-0125): its sensors, behavior and assists see nothing.
    pub fn blinded(&self) -> bool {
        self.jam.blind > 0.0
    }

    /// The static it holds before it is blinded: [`tuning::STATIC_PER_CORE`] per alive core on its controlling
    /// piece (at least one core's worth).
    pub fn static_limit(&self) -> f32 {
        let cores = self
            .control
            .and_then(|c| self.piece(c))
            .map_or(0, |p| p.cores().iter().filter(|&&c| self.parts[c as usize].alive).count());
        tuning::STATIC_PER_CORE * cores.max(1) as f32
    }
}

impl Sim {
    /// Each tick, before the guns fire: static bleeds off, a blinding runs out into its immunity, the immunity
    /// runs out; a ship that is no vessel any more holds nothing. Ships that hold nothing are skipped.
    pub(crate) fn jam_step(&mut self) {
        for s in &mut self.ships {
            if s.jam.is_clear() {
                continue;
            }
            if !s.is_vessel() {
                s.jam = Jam::default();
                continue;
            }
            let j = &mut s.jam;
            j.charge = (j.charge - tuning::BLEED * DT).max(0.0);
            if j.blind > 0.0 {
                j.blind -= DT;
                if j.blind <= 0.0 {
                    j.blind = 0.0;
                    j.immune = tuning::IMMUNE;
                }
            } else if j.immune > 0.0 {
                j.immune = (j.immune - DT).max(0.0);
            }
        }
    }

    /// A disruption hit of `by` on part `part` of ship `k` carries `amount` static (applied with the gun hits,
    /// after the damage). Only a vessel's controlling piece takes it; a core or sensor takes [`tuning::CORE_HIT`]
    /// times as much. At the ship's limit it is blinded.
    pub(crate) fn disrupt(&mut self, by: ShipId, k: usize, part: PartIdx, amount: f32) {
        let s = &mut self.ships[k];
        if amount <= 0.0 || !s.is_vessel() || s.jam.blind > 0.0 || s.jam.immune > 0.0 {
            return;
        }
        if s.control != Some(s.parts[part as usize].piece) {
            return;
        }
        let spec = &s.design.parts[part as usize];
        let factor = if spec.logic || spec.sensor > 0.0 { tuning::CORE_HIT } else { 1.0 };
        let limit = s.static_limit();
        s.jam.charge += amount * factor;
        if s.jam.charge >= limit {
            s.jam = Jam { charge: 0.0, blind: tuning::BLIND, immune: 0.0 };
            let ship = s.id;
            self.events.push(SimEvent::Blinded { by, ship, seconds: tuning::BLIND });
        }
    }
}
