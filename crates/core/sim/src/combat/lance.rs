//! Lances (GDR-0127, the Phase walkers' weapon; content `weapon.lance`): a lance is a hit-scan pulse that does not
//! stop at the first block. What of the shot passes the shields strikes the part it hits, and then every part on its
//! line through that piece, in order, each with what is left of the shot's strength: after each cell of a machine part
//! the strength keeps `keep` of itself, after each cell of a hull part (armour) `armor`. A part that spans several
//! cells on the line is struck once and slows the lance once per cell. The lance stops where its line leaves the
//! piece (an empty cell or another piece's: it never goes on to another body), and once its strength is below
//! [`tuning::SPENT`] of the shot's. Cells of parts destroyed earlier (still the piece's) let it pass without loss.
//!
//! Applied with the other gun hits once every gun has fired (`weapons.rs`, ADR-0037), in firing order. Each struck
//! part is a `Hit`; then one `SimEvent::Lanced` (where it entered, where it left, how many parts) for the client's
//! line through the cells. Deterministic: one grid walk (`weapons::GridWalk`), no randomness.

use data::{LanceSpec, WeaponFamily};

use super::weapons::{GridWalk, PartRayHit};
use crate::events::SimEvent;
use crate::ids::ShipId;
use crate::math::{Rot, Vec2};
use crate::ship::design::PartIdx;
use crate::world::Sim;

/// Lance tuning. **PLACEHOLDERS** (lead defaults, GDR-0127).
pub mod tuning {
    /// A lance stops once its strength is below this share of the shot's.
    pub const SPENT: f32 = 0.02;
}

impl Sim {
    /// Ship `by`'s lance hit `h` (along unit `dir`) with `damage` after shields (`absorbed` taken by them): strike every
    /// part on its line through the struck piece (the module docs).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lance(
        &mut self,
        by: ShipId,
        family: WeaponFamily,
        h: &PartRayHit,
        dir: Vec2,
        damage: f32,
        absorbed: f32,
        spec: LanceSpec,
    ) {
        let k = h.k;
        let pid = h.hit.key.piece;
        let Some(piece) = self.ships[k].piece(pid) else {
            return;
        };
        let (origin, pos, rot) = (piece.origin, piece.body.pos, Rot::from_angle(piece.body.angle));
        let mut walk = GridWalk::new(origin, h.hit.local_point, rot.apply_inverse(dir));
        let (mut cell, mut t) = ((walk.cx, walk.cy), 0.0f32);
        let mut strength = damage;
        let mut last: Option<usize> = None;
        let mut struck = 0u32;
        for _ in 0..super::weapons::tuning::MAX_WALK {
            let ship = &self.ships[k];
            let Some(o) = ship.design().grid.occupant(cell) else {
                break;
            };
            let ps = &ship.parts()[o];
            if ps.piece != pid {
                break;
            }
            if ps.alive || last == Some(o) {
                let hull = ship.design().parts[o].hull;
                if last != Some(o) {
                    let (point, normal, shield) = if struck == 0 {
                        (h.hit.point, h.hit.normal, absorbed)
                    } else {
                        (pos + rot.apply(walk.local_at(origin, t)), -dir, 0.0)
                    };
                    self.damage_part(by, family, k, o as PartIdx, point, normal, strength, shield);
                    struck += 1;
                    last = Some(o);
                }
                strength *= if hull { spec.armor } else { spec.keep };
            }
            (cell, t) = walk.step();
            if strength < damage * tuning::SPENT {
                break;
            }
        }
        let to = pos + rot.apply(walk.local_at(origin, t));
        let ship = self.ships[k].id();
        self.events.push(SimEvent::Lanced { by, ship, from: h.hit.point, to, parts: struck });
    }
}
