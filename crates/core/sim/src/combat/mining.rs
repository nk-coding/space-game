//! Mining (GDR-0114, `docs/sim.md` "Mining"): a mining beam (a gun whose content `weapon.cut` is above 0) held on an
//! asteroid rock (GDR-0104) cuts its ore. Every tick the beam rests on a rock with ore left, it adds `cut × work × DT`
//! to the rock's cut; each whole unit leaves the rock (`Rock::ore` counts down) as an [`Item::Ore`] of the rock's
//! material: into the shooter's hold while it has a free unit, else loose beside the rock (drifting back along the
//! beam, collected like any loose item). A rock with no ore left is spent: it stays, as cover, and gives nothing.
//! Several beams on one rock share its cut; whoever completes a unit gets it. Deterministic: the guns fire in ship and
//! slot order, the cut is `+ − ×` only, no random draws.

use crate::combat::salvage::Item;
use crate::events::SimEvent;
use crate::math::Vec2;
use crate::world::Sim;

/// A unit of ore's value in a hold (build points per cargo unit, as salvage counts it, `salvage.rs`): below every
/// part and fragment, so a shrinking hold sheds ore first and collecting takes a wreck's parts before it (lead
/// default).
pub const ORE_VALUE: f32 = 0.25;
/// Ore a full hold can't take drifts off the rock at this speed (m/s), back along the beam.
const SPILL_SPEED: f32 = 0.5;

impl Sim {
    /// Ship `k`'s mining beam cut `cut` ore units from rock `rock`, striking it at `at` along `dir` (see the module
    /// docs).
    pub(crate) fn mine(&mut self, k: usize, rock: u32, cut: f32, at: Vec2, dir: Vec2) {
        let Some(r) = self.voyage.as_deref_mut().and_then(|v| v.rock_mut(rock)) else { return };
        if r.ore == 0 {
            return;
        }
        r.cut += cut;
        let mut units = 0u32;
        while r.cut >= 1.0 && r.ore > 0 {
            r.cut -= 1.0;
            r.ore -= 1;
            units += 1;
        }
        let spent = r.ore == 0;
        if spent {
            r.cut = 0.0;
        }
        let material = r.material.clone();
        for u in 0..units {
            let item = Item::Ore { material: material.clone() };
            let ship = &mut self.ships[k];
            let stowed = ship.cargo.used() < ship.cargo_capacity();
            if stowed {
                ship.cargo.add(item.clone(), 1, ORE_VALUE);
            } else {
                self.set_loose(item.clone(), 1, ORE_VALUE, at - dir * 0.5, dir * -SPILL_SPEED);
            }
            let ship = self.ships[k].id();
            self.events.push(SimEvent::Mined { ship, rock, item, at, stowed, spent: spent && u + 1 == units });
        }
    }
}
