//! Chain reactions (GDR-0019, GDR-0084, GDR-0085; `docs/sim.md` "Damage and breakage"). A part with an
//! `explosion: (damage, radius)` block that damage destroys blasts every alive part with a cell whose nearest point
//! lies within `radius` m of its centre, on any piece of any ship:
//!
//! * **Its own piece** (as it was before this resolution): the targets come compiled with the design
//!   (`ShipDesign::blast`), each with the armour on the line to it (`ShipDesign::blast_cover`).
//! * **Every other piece** (its own ship's split-off pieces, other ships, wrecks, salvage): found when the blast goes
//!   off, by a scan of the pieces' bounding circles in ship and piece order, then of the grid cells in reach in the
//!   piece's own blueprint frame. The armour on the line is that of the target's piece and of the exploding piece.
//! * **Damage**: `damage × the target's blast_factor`, times the `blast_factor` of every armour part (a part with a
//!   `blast_factor` below 1, alive on its piece when the blast goes off) whose cell the straight line from the
//!   blast's centre to the target's nearest cell centre passes through (not the target itself, not the exploding
//!   part). No falloff. Touching a cell's corner or edge is not passing through it.
//! * **Shields**: bubbles of other ships that the blast crosses (they cover the target's cell centre but not the
//!   blast's centre) absorb its [`BLAST_FAMILY`] share first, as they do a shot's; the exploding ship's own never
//!   (the blast goes off inside them). Blasts spare nobody: friendly vessels are hit as everyone else.
//! * **Order**: one queue over every ship's destroyed parts (ships in index order, each ship's parts in the order
//!   they died), then every part a blast destroys joins its end at once, so a whole chain resolves in the same
//!   tick, first destroyed first, before the fragments, splits and events. Each blast computes all its hits from the
//!   state before it (what is alive and covers what), then applies them in order: its own piece's targets
//!   ascending, then the other pieces in ship and piece order, their targets ascending.
//!
//! Deterministic: index order everywhere, IEEE `+ − × ÷` and comparisons only, no allocation once warmed up.

use data::{Cell, ShipGrid, WeaponFamily};

use crate::ship::breakage::DamageTarget;
use crate::ship::design::{PartIdx, family_index};
use crate::ids::ShipId;
use crate::ship::mass::to_piece;
use crate::math::{Rot, Vec2};
use crate::ship::Ship;
use crate::world::Sim;

/// The weapon family whose shield share a blast loses to a bubble it crosses (lead default, GDR-0085: an
/// explosion is what a warhead is, so bubbles take it as they take missiles).
pub const BLAST_FAMILY: WeaponFamily = WeaponFamily::Missile;

/// Slack on a piece's bounding radius (m) in the proximity scan: the radius bounds its colliders, a slope's cell
/// reaches past its collider.
const REACH_SLACK: f32 = 1.0;

/// One hit of a blast, computed before any is applied.
#[derive(Clone, Copy, Debug)]
struct Hit {
    /// Index of the ship hit.
    k: u32,
    part: PartIdx,
    damage: f32,
    /// The target cell's centre (world); bubbles covering it and not the blast's centre absorb first.
    point: Vec2,
    /// On another piece: bubbles may absorb it.
    shields: bool,
}

/// Reused buffers of the chain pass.
#[derive(Clone, Debug, Default)]
pub(crate) struct ChainScratch {
    /// `(ship index, part)` in the order the blasts go off.
    queue: Vec<(u32, PartIdx)>,
    hits: Vec<Hit>,
    own: Vec<(PartIdx, f32)>,
    reach: Vec<(PartIdx, [f32; 2])>,
    cover: Vec<PartIdx>,
}

/// Distance from `c` to the nearest point of the span `[a, a + 1]` along one axis.
#[inline]
fn gap(a: i32, c: f32) -> f32 {
    (a as f32 - c).max(c - (a + 1) as f32).max(0.0)
}

/// The parts of `grid` with a cell whose nearest point lies within `r` of the blueprint point `c` and that `keep`
/// lets through, ascending, each with the centre of its cell nearest to `c` (the first in row order on a tie).
pub(crate) fn in_reach(
    grid: &ShipGrid,
    c: [f32; 2],
    r: f32,
    keep: impl Fn(usize) -> bool,
    out: &mut Vec<(PartIdx, [f32; 2])>,
) {
    out.clear();
    let d2 = |p: [f32; 2]| {
        let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
        dx * dx + dy * dy
    };
    // Cells reach from `floor(c - r) - 1` (one that ends exactly at the radius) to `floor(c + r)`.
    for y in (c[1] - r).floor() as i32 - 1..=(c[1] + r).floor() as i32 {
        for x in (c[0] - r).floor() as i32 - 1..=(c[0] + r).floor() as i32 {
            let (dx, dy) = (gap(x, c[0]), gap(y, c[1]));
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let Some(o) = grid.occupant((x, y)) else {
                continue;
            };
            if !keep(o) {
                continue;
            }
            let centre = [x as f32 + 0.5, y as f32 + 0.5];
            match out.iter_mut().find(|h| h.0 == o as PartIdx) {
                Some(h) => {
                    if d2(centre) < d2(h.1) {
                        h.1 = centre;
                    }
                }
                None => out.push((o as PartIdx, centre)),
            }
        }
    }
    out.sort_unstable_by_key(|h| h.0);
}

/// Does the segment `a → b` pass through the open interior of `cell` (a stretch of positive length, so touching
/// a corner or running along an edge does not count)? Liang–Barsky clipping.
fn crosses(a: [f32; 2], b: [f32; 2], (x, y): Cell) -> bool {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-dx, a[0] - x as f32),
        (dx, (x + 1) as f32 - a[0]),
        (-dy, a[1] - y as f32),
        (dy, (y + 1) as f32 - a[1]),
    ] {
        if p == 0.0 {
            if q <= 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    t0 < t1
}

/// The parts of `grid` whose cell the segment `a → b` (blueprint frame) passes through ([`crosses`]) and that
/// `keep` lets through, ascending, each once.
pub(crate) fn cover(
    grid: &ShipGrid,
    a: [f32; 2],
    b: [f32; 2],
    keep: impl Fn(usize) -> bool,
    out: &mut Vec<PartIdx>,
) {
    out.clear();
    let (x0, x1) = (a[0].min(b[0]).floor() as i32, a[0].max(b[0]).floor() as i32);
    let (y0, y1) = (a[1].min(b[1]).floor() as i32, a[1].max(b[1]).floor() as i32);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if let Some(o) = grid.occupant((x, y))
                && keep(o)
                && crosses(a, b, (x, y))
            {
                out.push(o as PartIdx);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
}

/// A world point in the blueprint frame of a piece at `pos`, turned by `rot`, with frame origin `origin`.
#[inline]
fn to_blueprint(world: Vec2, pos: Vec2, rot: Rot, origin: [f32; 2]) -> [f32; 2] {
    let l = rot.apply_inverse(world - pos);
    [origin[0] + l.x, origin[1] - l.y]
}

impl Ship {
    /// The hits of part `p`'s blast on its own piece: the compiled targets ([`crate::ShipDesign::blast`]) still
    /// alive on it, each damage times the `blast_factor` of its cover still alive on it. Appended to `out`.
    pub(crate) fn own_blast(&self, p: usize, out: &mut Vec<(PartIdx, f32)>) {
        let d = &*self.design;
        let piece = self.parts[p].piece;
        for t in d.blast.get(p) {
            let ts = &self.parts[t.part as usize];
            if !ts.alive || ts.piece != piece {
                continue;
            }
            let mut damage = t.damage;
            for &c in d.blast_cover.get(t.cover as usize) {
                let cs = &self.parts[c as usize];
                if cs.alive && cs.piece == piece {
                    damage *= d.parts[c as usize].blast_factor;
                }
            }
            out.push((t.part, damage));
        }
    }

    /// Queues the parts destroyed since the last call (`pending_destroyed` past `chained`) as ship `k`'s.
    fn queue_destroyed(&mut self, k: usize, queue: &mut Vec<(u32, PartIdx)>) {
        while self.chained < self.pending_destroyed.len() {
            queue.push((k as u32, self.pending_destroyed[self.chained]));
            self.chained += 1;
        }
    }
}

impl Sim {
    /// Sets off every chain reaction of the parts destroyed since the last breakage resolution, on every ship
    /// (module docs). Called before any ship's breakage is resolved; a no-op when no part is waiting.
    pub(crate) fn chain_reactions(&mut self) {
        if self
            .ships
            .iter()
            .all(|s| s.chained >= s.pending_destroyed.len())
        {
            return;
        }
        let mut sc = std::mem::take(&mut self.chain_scratch);
        sc.queue.clear();
        for (k, s) in self.ships.iter_mut().enumerate() {
            s.queue_destroyed(k, &mut sc.queue);
        }
        let mut next = 0;
        while next < sc.queue.len() {
            let (k, p) = sc.queue[next];
            next += 1;
            self.blast(k as usize, p, &mut sc);
        }
        self.chain_scratch = sc;
    }

    /// Part `p` of ship `k` explodes (if it has an explosion): every hit computed first, then applied in order;
    /// parts it destroys join the queue.
    fn blast(&mut self, k: usize, p: PartIdx, sc: &mut ChainScratch) {
        let ship = &self.ships[k];
        let Some(e) = ship.design.parts[p as usize].explosion.as_ref() else {
            return;
        };
        sc.hits.clear();
        sc.own.clear();
        ship.own_blast(p as usize, &mut sc.own);
        sc.hits.extend(sc.own.iter().map(|&(part, damage)| Hit {
            k: k as u32,
            part,
            damage,
            point: Vec2::ZERO,
            shields: false,
        }));
        let centre = self.blast_other_pieces(k, p, e.damage, e.radius, sc);
        let by = self.ships[k].id();
        for n in 0..sc.hits.len() {
            let h = sc.hits[n];
            let mut damage = h.damage;
            if h.shields {
                damage = self.blast_shields(by, centre, h.point, damage);
            }
            if damage > 0.0 {
                let target = &mut self.ships[h.k as usize];
                target.apply_damage(DamageTarget::Part(h.part), damage);
                target.queue_destroyed(h.k as usize, &mut sc.queue);
            }
        }
    }

    /// The hits of part `p` of ship `k`'s blast (`damage` within `r` m) on every other piece, appended to
    /// `sc.hits` in ship and piece order, targets ascending. Returns the blast's centre in the world.
    fn blast_other_pieces(
        &self,
        k: usize,
        p: PartIdx,
        damage: f32,
        r: f32,
        sc: &mut ChainScratch,
    ) -> Vec2 {
        let ship = &self.ships[k];
        let d = &*ship.design;
        let pid = ship.parts[p as usize].piece;
        // The exploding part's piece is still there: pieces are dropped only after the chain.
        let Some(src) = ship.piece(pid) else {
            return Vec2::ZERO;
        };
        let centre_bp = d.parts[p as usize].shape.center();
        let src_rot = Rot::from_angle(src.body.angle);
        let centre = src.body.pos + src_rot.apply(to_piece(centre_bp, src.origin));
        for (j, s) in self.ships.iter().enumerate() {
            for q in &s.pieces {
                if j == k && q.id == pid {
                    continue;
                }
                let reach = q.radius + r + REACH_SLACK;
                let off = q.body.pos - centre;
                if off.x.abs() > reach
                    || off.y.abs() > reach
                    || off.length_squared() > reach * reach
                {
                    continue;
                }
                let rot = Rot::from_angle(q.body.angle);
                let c = to_blueprint(centre, q.body.pos, rot, q.origin);
                in_reach(&s.grid, c, r, |o| s.parts[o].piece == q.id, &mut sc.reach);
                for &(t, cell) in &sc.reach {
                    let td = &s.design.parts;
                    let mut hit = damage * td[t as usize].blast_factor;
                    // Armour of the target's piece on the line, then of the exploding piece (the exploding part
                    // itself is out of the grid already).
                    cover(
                        &s.grid,
                        c,
                        cell,
                        |o| {
                            o != t as usize && s.parts[o].piece == q.id && td[o].blast_factor < 1.0
                        },
                        &mut sc.cover,
                    );
                    for &o in &sc.cover {
                        hit *= td[o as usize].blast_factor;
                    }
                    let point = q.body.pos + rot.apply(to_piece(cell, q.origin));
                    let b = to_blueprint(point, src.body.pos, src_rot, src.origin);
                    cover(
                        &ship.grid,
                        centre_bp,
                        b,
                        |o| ship.parts[o].piece == pid && d.parts[o].blast_factor < 1.0,
                        &mut sc.cover,
                    );
                    for &o in &sc.cover {
                        hit *= d.parts[o as usize].blast_factor;
                    }
                    sc.hits.push(Hit {
                        k: j as u32,
                        part: t,
                        damage: hit,
                        point,
                        shields: true,
                    });
                }
            }
        }
        centre
    }

    /// The bubbles a blast from `centre` crosses on its way to `point` (they cover the point, not the centre)
    /// take their [`BLAST_FAMILY`] share of `damage` first, in bubble order (ship, piece, emitter): every bubble of
    /// this tick with charge left whose emitter is still alive, except those of the exploding ship `by`, friend or
    /// foe (blasts spare nobody, so nobody's bubble lets them through). Returns the damage left for the part.
    fn blast_shields(&mut self, by: ShipId, centre: Vec2, point: Vec2, mut damage: f32) -> f32 {
        let fi = family_index(BLAST_FAMILY);
        for b in 0..self.bubbles.len() {
            if damage <= 1e-6 {
                return 0.0;
            }
            let bub = &self.bubbles[b];
            let r2 = bub.radius * bub.radius;
            // The bubbles are this tick's (or the last one's, before the weapons phase): the ship may be gone.
            let live = self
                .ships
                .get(bub.ship_index)
                .is_some_and(|s| s.id() == bub.ship && s.parts[bub.part as usize].alive);
            if bub.ship == by
                || bub.left <= 0.0
                || bub.absorb[fi] <= 0.0
                || !live
                || (point - bub.centre).length_squared() > r2
                || (centre - bub.centre).length_squared() <= r2
            {
                continue;
            }
            damage -= self.absorb(b, fi, by, point, damage);
        }
        damage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_through_a_corner_crosses_neither_side_cell() {
        // From the centre of (1, 1) to the centre of (0, 0): through the shared corner only.
        assert!(!crosses([1.5, 1.5], [0.5, 0.5], (0, 1)));
        assert!(!crosses([1.5, 1.5], [0.5, 0.5], (1, 0)));
        assert!(crosses([1.5, 1.5], [0.5, 0.5], (0, 0)));
        // Straight across a row: the cell between, not the ones beside it.
        assert!(crosses([0.5, 0.5], [2.5, 0.5], (1, 0)));
        assert!(!crosses([0.5, 0.5], [2.5, 0.5], (1, 1)));
        // A knight's move clips both cells it passes.
        assert!(crosses([0.5, 0.5], [1.5, 2.5], (0, 1)));
        assert!(crosses([0.5, 0.5], [1.5, 2.5], (1, 1)));
        assert!(!crosses([0.5, 0.5], [1.5, 2.5], (1, 0)));
    }
}
