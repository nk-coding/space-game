//! Living parts (GDR-0099, GDR-0102; part of [`Ship`]'s impl): tissue that heals, and hearts that re-grow lost parts.
//!
//! * **Healing**: every part with `heal` (a hull part in a healing armour, or a part with its own `heal`) regains that
//!   many hit points per second while it is alive on the controlling piece of a vessel, up to its full hit points.
//! * **Regrowth**: a heart works while it is alive, not overheated and not idle on the controlling piece of a vessel.
//!   Its timer runs while it works; every `heart.interval` seconds it re-grows one lost part: of the parts of the
//!   design that were destroyed or stripped (0 hit points; a part that left with a split-off vessel keeps its hit
//!   points and lives on there), never a heart, whose cells are free in the ship's grid and that links to an alive
//!   part of the heart's piece and to none of another piece (linked alive parts always share a piece; a broken-off
//!   chunk is never grown back on), the one whose centroid lies nearest the heart's (then the lower index), and whose
//!   place no other body overlaps ([`crate::Sim`] asks the physics). It comes back with `heart.hp` of its hit
//!   points, cold, its bubble down, and the piece is rebuilt in the same tick (mass, colliders, the body). With
//!   nothing to re-grow the timer starts over, so a heart looks at most once an interval.

use super::*;

impl Ship {
    /// Heals the living parts of the controlling piece by one tick (no allocation).
    pub(crate) fn heal(&mut self) {
        let Some(control) = self.control else {
            return;
        };
        for &p in &self.design.healing {
            let spec = &self.design.parts[p as usize];
            let ps = &mut self.parts[p as usize];
            if ps.alive && ps.piece == control && ps.hp < spec.hp {
                ps.hp = (ps.hp + spec.heal * DT).min(spec.hp);
            }
        }
    }

    /// Advances heart `h`'s timer (`ShipDesign::hearts[h]`) by one tick while it works. True when it is due to
    /// re-grow a part now (the timer then starts over).
    pub(crate) fn heart_due(&mut self, h: usize) -> bool {
        let p = self.design.hearts[h];
        let Some(interval) = self.design.parts[p as usize].heart.as_ref().map(|x| x.interval) else {
            return false;
        };
        let ps = &self.parts[p as usize];
        if Some(ps.piece) != self.control || !ps.alive || ps.overheated || ps.idle {
            return false;
        }
        self.regrowth[h] += DT;
        if self.regrowth[h] < interval {
            return false;
        }
        self.regrowth[h] = 0.0;
        true
    }

    /// The parts heart `heart` may re-grow now, best first (see the module docs), into `out`; the caller checks
    /// their places against other bodies.
    pub(crate) fn regrowth_candidates(&self, heart: PartIdx, out: &mut Vec<(f32, PartIdx)>) {
        out.clear();
        let d = &*self.design;
        let piece = self.parts[heart as usize].piece;
        let [hx, hy] = d.parts[heart as usize].centroid;
        for (i, ps) in self.parts.iter().enumerate() {
            let spec = &d.parts[i];
            if ps.alive || ps.hp > 0.0 || spec.heart.is_some() {
                continue;
            }
            // Fed by the heart's piece, and touching no other piece: linked alive parts always share a piece.
            let (mut fed, mut other) = (false, false);
            for &n in d.neighbors.get(i) {
                let q = &self.parts[n as usize];
                if q.alive {
                    fed |= q.piece == piece;
                    other |= q.piece != piece;
                }
            }
            if fed && !other && self.grid.is_free(i) {
                let [x, y] = spec.centroid;
                out.push(((x - hx) * (x - hx) + (y - hy) * (y - hy), i as PartIdx));
            }
        }
        out.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    }

    /// Brings part `p` back on the heart's piece with `share` of its hit points: alive, in the grid, its own and its
    /// neighbours' exposure and bonuses refreshed, the piece marked for a rebuild in the next
    /// [`Ship::resolve_breakage`]. The reverse of [`Ship::remove_part`]. False (nothing changed) when a cell is
    /// taken.
    pub(crate) fn regrow(&mut self, p: PartIdx, heart: PartIdx, share: f32) -> bool {
        let i = p as usize;
        if self.parts[i].alive || !self.grid.restore(i) {
            return false;
        }
        let piece = self.parts[heart as usize].piece;
        let spec = &self.design.parts[i];
        self.parts[i] = PartState {
            hp: spec.hp * share,
            heat: 0.0,
            alive: true,
            overheated: false,
            piece,
            open_sides: 0,
            clear: true,
            power: 0.0,
            work: 0.0,
            shield: 0.0,
            shield_down: spec.is_shield,
            idle: false,
        };
        // Exposure changes for the part and its linked neighbours; bonuses for them and their neighbours.
        let design = Arc::clone(&self.design);
        let mut affected = std::mem::take(&mut self.scratch.affected);
        affected.clear();
        self.refresh_exposure(p);
        affected.push(p);
        for &n in design.neighbors.get(i) {
            if self.parts[n as usize].alive {
                self.refresh_exposure(n);
                affected.push(n);
                affected.extend_from_slice(design.neighbors.get(n as usize));
            }
        }
        affected.sort_unstable();
        affected.dedup();
        for &t in &affected {
            if self.parts[t as usize].alive {
                self.refresh_stats(t);
            }
        }
        self.scratch.affected = affected;
        if let Some(k) = self.piece_index(piece) {
            let parts = &mut self.pieces[k].parts;
            if let Err(at) = parts.binary_search(&p) {
                parts.insert(at, p);
            }
            self.pieces[k].dirty = true;
        }
        true
    }
}

impl crate::Sim {
    /// The living pass of a tick (after the ship systems): tissue heals, and each heart that is due re-grows the
    /// best of its candidates whose place no other body overlaps (`SimEvent::PartRegrown`); a ship that grew is
    /// resolved at once, so its pieces and bodies are rebuilt before the weapons fire.
    pub(crate) fn living(&mut self) {
        let mut found = Vec::new();
        for k in 0..self.ships.len() {
            self.ships[k].heal();
            let mut grew = false;
            for h in 0..self.ships[k].design.hearts.len() {
                if !self.ships[k].heart_due(h) {
                    continue;
                }
                let s = &self.ships[k];
                let heart = s.design.hearts[h];
                let piece = s.parts[heart as usize].piece;
                let (Some(share), Some(pc)) = (s.design.parts[heart as usize].heart.as_ref().map(|x| x.hp), s.piece(piece))
                else {
                    continue;
                };
                s.regrowth_candidates(heart, &mut found);
                let key = crate::flight::physics::BodyKey { ship: s.id, piece };
                let free = found
                    .iter()
                    .map(|&(_, p)| p)
                    .find(|&p| !self.physics.overlaps_other(key, &pc.body, &s.outline(p, pc.origin)));
                if let Some(p) = free
                    && self.ships[k].regrow(p, heart, share)
                {
                    self.events.push(crate::SimEvent::PartRegrown { ship: key.ship, piece, part: p });
                    // A logic core back changes what the mind runs.
                    self.mind_dirty |= self.ships[k].design.parts[p as usize].logic;
                    grew = true;
                }
            }
            if grew {
                self.resolve_ship(k);
            }
        }
        self.update_minds();
    }
}

impl Ship {
    /// Part `p`'s polygon in the frame of a piece with origin `origin`, counter-clockwise, shrunk a tenth towards its
    /// centroid so that touching neighbours do not count as overlapping.
    fn outline(&self, p: PartIdx, origin: [f32; 2]) -> Vec<Vec2> {
        let spec = &self.design.parts[p as usize];
        let c = to_piece(spec.centroid, origin);
        // The blueprint's y points down, the piece frame's up: the order flips.
        let mut pts: Vec<Vec2> = spec
            .shape
            .polygon()
            .iter()
            .map(|q| c + (to_piece([q.x as f32, q.y as f32], origin) - c) * 0.9)
            .collect();
        pts.reverse();
        pts
    }
}
