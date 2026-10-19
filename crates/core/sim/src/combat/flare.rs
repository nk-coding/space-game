//! Flares (GDR-0115; GDR-0018's other counter to missiles): a flare launcher (content `countermeasure`) releases
//! `decoys` hot decoys at once when its vessel asks (the FLARE key, a behavior's `flare` action, the `auto_flare`
//! assist), then cools down for `cooldown` seconds; it has `charges` releases per fight.
//!
//! * **Release**: the first launcher (by part) on the vessel's controlling piece that is alive, charged and cool.
//!   Its decoys leave the launcher's centre along its facing, fanned over ±[`tuning::FAN`], at `speed` m/s on top of
//!   the launcher's own velocity, slow down by [`tuning::DRAG`] and burn for `life` seconds.
//! * **Lure**: every tick a homing missile hostile to a decoy's owner that is not yet lured turns onto the nearest
//!   such decoy within the decoy's `lure` m (ties: the lower id); it no longer homes on a ship (`missiles_incoming`
//!   drops). It flies at the decoy as at a target and is spent when it reaches it (within [`tuning::FUSE`] m:
//!   `GoneReason::Decoyed`, no blast); if the decoy burns out first, its seeker looks for a ship again.
//!
//! Deterministic: ship, slot and decoy order; decoys take projectile ids. Hashed with the world only while any
//! exist, and a ship's launchers only if it has any, so fights without flares keep their hashes.

use crate::DT;
use crate::events::SimEvent;
use crate::hash::{StateHash, StateHasher};
use crate::ids::ShipId;
use crate::math::{Rot, Vec2};
use crate::ship::vessel::{Owner, friendly};
use crate::ship::{Ship, dir_vec, mass::to_piece};
use crate::world::Sim;

use super::weapons::ProjectileId;

/// Flare tuning. **PLACEHOLDERS** (lead defaults).
pub mod tuning {
    /// A release's decoys fan out over this angle (radians, to either side of the launcher's facing).
    pub const FAN: f32 = 0.6;
    /// Share of a decoy's velocity (relative to where it was released) it loses per second.
    pub const DRAG: f32 = 0.8;
    /// A lured missile is spent when it comes this close to its decoy (m).
    pub const FUSE: f32 = 1.0;
}

/// A burning decoy in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decoy {
    pub id: ProjectileId,
    /// The ship that released it, and the owner it was released for: missiles hostile to it are lured.
    pub owner: ShipId,
    pub team: Option<Owner>,
    /// Now and at the end of the previous tick (render: lerp `prev_pos → pos` by alpha).
    pub pos: Vec2,
    pub prev_pos: Vec2,
    pub vel: Vec2,
    /// Velocity it slows down towards (the launcher's at release).
    pub drift: Vec2,
    /// Seconds left to burn.
    pub life: f32,
    /// Metres within which it lures missiles.
    pub lure: f32,
}

impl StateHash for Decoy {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.id.0);
        h.u32(self.owner.0);
        h.vec2(self.pos);
        h.vec2(self.vel);
        h.vec2(self.drift);
        h.f32(self.life);
        h.f32(self.lure);
    }
}

/// One flare launcher's state for the fight (per `ShipDesign::flares` entry).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlareSlot {
    /// Releases left.
    pub charges: u32,
    /// Seconds until it can release again.
    pub cooldown: f32,
}

impl StateHash for FlareSlot {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.charges);
        h.f32(self.cooldown);
    }
}

impl Ship {
    /// The launcher (index into `ShipDesign::flares`) a release would use now: the first on the active
    /// controlling piece that is alive, charged and cool.
    fn ready_flare(&self) -> Option<usize> {
        let control = self.control_piece()?;
        if !self.is_active() || !self.piece(control).is_some_and(|p| p.active) {
            return None;
        }
        let mounts = &self.design().flares;
        (0..mounts.len()).find(|&i| {
            let ps = &self.parts[mounts[i].part as usize];
            let s = &self.flares[i];
            ps.alive && ps.piece == control && s.charges > 0 && s.cooldown <= 0.0
        })
    }

    /// Whether a release would happen now (a working launcher charged and cool).
    pub fn can_flare(&self) -> bool {
        self.ready_flare().is_some()
    }

    /// Releases left over all of the ship's launchers (the HUD).
    pub fn flare_charges(&self) -> u32 {
        self.flares.iter().map(|s| s.charges).sum()
    }

    /// Asks for a release this tick (handled in the weapons step); ignored while none is ready.
    pub(crate) fn ask_flare(&mut self) {
        self.flare_asked |= self.can_flare();
    }
}

impl Sim {
    /// The decoys burning now.
    pub fn decoys(&self) -> &[Decoy] {
        &self.decoys
    }

    /// Once per tick before the projectiles fly: launchers cool down, asked releases happen, decoys fly and burn
    /// out, and missiles near a hostile decoy turn onto it (the module docs).
    pub(crate) fn flares_step(&mut self) {
        for k in 0..self.ships.len() {
            for s in &mut self.ships[k].flares {
                s.cooldown = (s.cooldown - DT).max(0.0);
            }
            if std::mem::take(&mut self.ships[k].flare_asked)
                && let Some(i) = self.ships[k].ready_flare()
            {
                self.release(k, i);
            }
        }
        for d in &mut self.decoys {
            d.prev_pos = d.pos;
            d.pos += d.vel * DT;
            d.vel = d.drift + (d.vel - d.drift) * (1.0 - tuning::DRAG * DT).max(0.0);
            d.life -= DT;
        }
        self.decoys.retain(|d| d.life > 0.0);
        if self.decoys.is_empty() {
            return;
        }
        for p in &mut self.projectiles {
            let Some(m) = p.missile.as_mut() else {
                continue;
            };
            if m.decoy.is_some() || p.life <= 0.0 {
                continue;
            }
            let mut best: Option<(f32, ProjectileId)> = None;
            for d in &self.decoys {
                if d.owner == p.owner || friendly(p.team, d.team) {
                    continue;
                }
                let d2 = (d.pos - p.pos).length_squared();
                if d2 <= d.lure * d.lure && best.is_none_or(|b| d2 < b.0) {
                    best = Some((d2, d.id));
                }
            }
            if let Some((_, id)) = best {
                m.decoy = Some(id);
                m.target = None;
            }
        }
    }

    /// Launcher `i` of ship `k` releases its decoys (it was ready).
    fn release(&mut self, k: usize, i: usize) {
        let ship = &self.ships[k];
        let mount = &ship.design().flares[i];
        let Some(piece) = ship.piece(ship.parts[mount.part as usize].piece) else {
            return;
        };
        let rot = Rot::from_angle(piece.body.angle);
        let at = piece.body.pos + rot.apply(to_piece(mount.centre, piece.origin));
        let dir = rot.apply(dir_vec(mount.facing));
        let drift = piece.body.linvel + Vec2::cross_scalar(piece.body.angvel, at - piece.com_world());
        let (owner, team, part, spec) = (ship.id(), ship.team(), mount.part, mount.spec);
        let n = spec.decoys.max(1);
        for j in 0..n {
            let fan = if n > 1 { tuning::FAN * (2.0 * j as f32 / (n - 1) as f32 - 1.0) } else { 0.0 };
            let vel = drift + Rot::from_angle(fan).apply(dir) * spec.speed;
            let id = self.ids.projectile();
            self.decoys.push(Decoy { id, owner, team, pos: at, prev_pos: at, vel, drift, life: spec.life, lure: spec.lure });
        }
        let s = &mut self.ships[k].flares[i];
        s.charges -= 1;
        s.cooldown = spec.cooldown;
        self.events.push(SimEvent::FlaresReleased { ship: owner, part, at, decoys: n });
    }

    /// Where a missile lured by decoy `id` flies (its position and velocity), if the decoy still burns.
    pub(crate) fn decoy_state(&self, id: ProjectileId) -> Option<(Vec2, Vec2)> {
        let k = self.decoys.binary_search_by_key(&id, |d| d.id).ok()?;
        Some((self.decoys[k].pos, self.decoys[k].vel))
    }
}
