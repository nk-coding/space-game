//! Engine exhaust plumes (GDR-0039, GDR-0046, GDR-0049; `docs/sim.md` "Engine plumes"). Engines may sit anywhere;
//! each firing engine's plume burns the first part in its path:
//!
//! * **Lanes.** One lane per cell of the engine's back side (`ShipDesign::plume_lanes`), a straight line from the
//!   centre of that cell's back face along the exhaust, `Thruster::plume_length` long. The engine's plume damage
//!   and heat per second are shared evenly over its lanes and scale with its thrust output this tick
//!   (`Piece::output`: throttle × power × rated-speed falloff), so an idle engine burns nothing and one holding
//!   its rated speed (no thrust left, a cruise flame only) next to nothing. A blocked plume costs no thrust.
//! * **Own vessel.** The parts of the engine's own piece in the lane's path come precomputed from the blueprint
//!   (`ShipDesign::plume_path`): the first alive one within reach is hit. Its own shield bubbles never stop it.
//! * **Other bodies.** Enemy vessels, salvage and debris (other owners, or none) are found with one ray cast per
//!   lane (`Sim::cast_part`, the shots' hit rule: parts the guns destroyed earlier in the tick are passed through
//!   to the part behind), skipping the engine's own piece and every vessel of the same owner
//!   (GDR-0046). The cast runs only when another body's bounding circle can be within reach of the lane, and a
//!   piece whose plumes reach neither a part of its own (`Piece::plume_blocked`, cached on rebuilds) nor another
//!   body (one scan of the bounding circles against `Piece::plume_reach`) is skipped as a whole, so plumes into
//!   empty space cost a few comparisons per ship. Shield bubbles of non-friendly ships covering the hit point
//!   absorb the plume's damage first, like energy damage (their `absorb[Energy]` share), and the heat in
//!   proportion.
//! * The lane stops at the first part it hits, on whichever ship (its own or another); that part takes the lane's
//!   damage and heat. Once it is destroyed the plume reaches further, from the next tick on.
//!
//! Runs in the weapons phase, after the guns (bubbles collected, bodies at their start-of-tick poses); breakage
//! is resolved afterwards with the weapons'. Events: `SimEvent::PlumeHit` per part burned per lane and tick,
//! `ShieldHit` for what bubbles took. No allocation once warmed up; every loop runs in ship, piece, row and lane
//! order.

use data::WeaponFamily;

use crate::DT;
use crate::design::{NO_SLOT, PartIdx, family_index};
use crate::events::SimEvent;
use crate::ids::{PieceId, ShipId};
use crate::mass::to_piece;
use crate::math::{Rot, Vec2};
use crate::physics::BodyKey;
use crate::ship::PartState;
use crate::vessel::{Owner, friendly};
use crate::world::Sim;

/// A body's bounding circle this tick (proximity test before any ray cast).
#[derive(Clone, Copy, Debug)]
struct Circle {
    centre: Vec2,
    radius: f32,
    key: BodyKey,
    team: Option<Owner>,
}

/// One lane's hit, applied after every plume was traced.
#[derive(Clone, Copy, Debug)]
struct Burn {
    by: ShipId,
    team: Option<Owner>,
    engine: PartIdx,
    /// Index of the ship hit, and its part.
    k: usize,
    part: PartIdx,
    point: Vec2,
    /// The engine's own piece (no shields).
    own: bool,
    damage: f32,
    heat: f32,
}

/// Reused buffers of the plume pass.
#[derive(Clone, Debug, Default)]
pub(crate) struct PlumeScratch {
    circles: Vec<Circle>,
    near: Vec<u32>,
    burns: Vec<Burn>,
}

/// The part a plume lane burns on the engine's own piece: the first part on the lane's `path`
/// (`ShipDesign::plume_path`) nearer than `length` that is alive and in `piece`, with its distance from the
/// lane's origin. The one rule for the plume pass, `Piece::plume_blocked` and the design report.
pub(crate) fn own_burn(
    path: &[(f32, PartIdx)],
    length: f32,
    parts: &[PartState],
    piece: PieceId,
) -> Option<(f32, PartIdx)> {
    for &(dist, part) in path {
        if dist >= length {
            break;
        }
        let ps = &parts[part as usize];
        if ps.alive && ps.piece == piece {
            return Some((dist, part));
        }
    }
    None
}

/// Does the segment `from + t·dir, t ∈ [0, len]` come within `r` of `c`?
#[inline]
fn segment_near(c: Vec2, r: f32, from: Vec2, dir: Vec2, len: f32) -> bool {
    let t = (c - from).dot(dir).clamp(0.0, len);
    (c - (from + dir * t)).length_squared() <= r * r
}

impl Sim {
    /// The owner of a body for game rules: the vessel's owner for its controlling piece, `None` for salvage.
    pub(crate) fn body_team(&self, key: BodyKey) -> Option<Owner> {
        self.ship_index(key.ship)
            .and_then(|k| self.ships[k].piece_team(key.piece))
    }

    /// Every firing engine's plume for one tick: trace the lanes, then burn what they hit.
    pub(crate) fn plumes(&mut self) {
        let mut sc = std::mem::take(&mut self.plume_scratch);
        sc.burns.clear();
        sc.circles.clear();
        let mut circles_ready = false;
        for k in 0..self.ships.len() {
            let ship = &self.ships[k];
            let Some(pi) = ship.control.and_then(|c| ship.piece_index(c)) else {
                continue;
            };
            let piece = &ship.pieces[pi];
            let reach = piece.plume_reach;
            if !piece.active || reach <= 0.0 {
                continue;
            }
            if !circles_ready {
                circles_ready = true;
                for s in &self.ships {
                    for p in &s.pieces {
                        sc.circles.push(Circle {
                            centre: p.body.pos,
                            radius: p.radius,
                            key: BodyKey {
                                ship: s.id,
                                piece: p.id,
                            },
                            team: s.piece_team(p.id),
                        });
                    }
                }
            }
            let own_key = BodyKey {
                ship: ship.id,
                piece: piece.id,
            };
            let team = ship.piece_team(piece.id);
            // Bodies a plume of this piece could reach (never its own piece or a friendly vessel).
            sc.near.clear();
            let pos = piece.body.pos;
            for (b, c) in sc.circles.iter().enumerate() {
                if c.key == own_key || friendly(team, c.team) {
                    continue;
                }
                let r = piece.radius + reach + c.radius;
                let d = c.centre - pos;
                if d.x.abs() <= r && d.y.abs() <= r && d.length_squared() <= r * r {
                    sc.near.push(b as u32);
                }
            }
            if sc.near.is_empty() && !piece.plume_blocked {
                continue; // no part of its own in a plume, nobody else in reach
            }
            let d = &*ship.design;
            let rot = Rot::from_angle(piece.body.angle);
            for (i, t) in piece.thrusters.iter().enumerate() {
                let out = piece.output[i];
                if t.slot == NO_SLOT || out <= 0.0 || t.plume_length <= 0.0 {
                    continue;
                }
                let [l0, l1] = d.engines[t.slot as usize].lanes;
                if l1 <= l0 {
                    continue;
                }
                let share = out * DT / (l1 - l0) as f32;
                let (damage, heat) = (t.plume_damage * share, t.plume_heat * share);
                if damage <= 0.0 && heat <= 0.0 {
                    continue;
                }
                let exhaust = rot.apply(-t.dir);
                for l in l0..l1 {
                    let path = d.plume_path.get(l as usize);
                    if path.is_empty() && sc.near.is_empty() {
                        continue; // nothing of its own in the way, nobody else in reach
                    }
                    // Its own piece: the first alive part in the lane within reach.
                    let own = own_burn(path, t.plume_length, &ship.parts, piece.id);
                    let len = own.map_or(t.plume_length, |(dist, _)| dist);
                    let from = pos + rot.apply(to_piece(d.plume_lanes[l as usize].origin, piece.origin));
                    // Other bodies, nearer than that: one ray cast, only if one can be in reach.
                    let near = len > 0.0
                        && sc.near.iter().any(|&b| {
                            let c = &sc.circles[b as usize];
                            segment_near(c.centre, c.radius, from, exhaust, len)
                        });
                    let other = if near {
                        self.cast_part(from, exhaust, len, |key| {
                            key == own_key || friendly(team, self.body_team(key))
                        })
                    } else {
                        None
                    };
                    let burn = |k: usize, part: PartIdx, point: Vec2, own: bool| Burn {
                        by: ship.id,
                        team,
                        engine: t.part,
                        k,
                        part,
                        point,
                        own,
                        damage,
                        heat,
                    };
                    if let Some(h) = other {
                        sc.burns.push(burn(h.k, h.part, h.hit.point, false));
                    } else if let Some((dist, part)) = own {
                        sc.burns.push(burn(k, part, from + exhaust * dist, true));
                    }
                }
            }
        }
        for n in 0..sc.burns.len() {
            let b = sc.burns[n];
            self.burn(&b);
        }
        self.plume_scratch = sc;
    }

    /// One lane's damage and heat on the part it hit: the shields covering the point that take damage from the
    /// engine's ship first ([`crate::weapons::absorbs_from`]: never its own or a friendly vessel's; energy share;
    /// the heat goes with the damage). A plume that does only heat loses each covering bubble's energy share of
    /// it, without draining the bubble (heat is not damage).
    fn burn(&mut self, b: &Burn) {
        let mut damage = b.damage;
        let mut heat = b.heat;
        if !b.own && (damage > 0.0 || heat > 0.0) {
            let fi = family_index(WeaponFamily::Energy);
            let heat_only = damage <= 0.0;
            for n in 0..self.bubbles.len() {
                if (if heat_only { heat } else { damage }) <= 1e-6 {
                    break;
                }
                let bub = &self.bubbles[n];
                if !crate::weapons::absorbs_from(bub, &self.ships, b.by, b.team, fi)
                    || (b.point - bub.centre).length_squared() > bub.radius * bub.radius
                {
                    continue;
                }
                if heat_only {
                    heat *= 1.0 - bub.absorb[fi].min(1.0);
                    continue;
                }
                let took = self.absorb(n, fi, b.by, b.point, damage);
                heat *= (damage - took) / damage;
                damage -= took;
            }
        }
        let ship = &mut self.ships[b.k];
        if !ship.parts[b.part as usize].alive {
            return;
        }
        if damage > 1e-6 {
            ship.apply_damage(crate::breakage::DamageTarget::Part(b.part), damage);
        } else {
            damage = 0.0;
        }
        let cap = ship.design.parts[b.part as usize].heat_capacity;
        let ps = &mut ship.parts[b.part as usize];
        if ps.alive && heat > 0.0 && cap > 0.0 {
            ps.heat += heat / cap;
        }
        if damage > 0.0 || heat > 0.0 {
            let (sid, piece) = (ship.id, ps.piece);
            self.events.push(SimEvent::PlumeHit {
                by: b.by,
                engine: b.engine,
                ship: sid,
                piece,
                part: b.part,
                point: b.point,
                damage,
                heat,
                absorbed: b.damage - damage,
            });
        }
    }
}
