//! Damage and breakage (the engine-independent part): parts lose hit points, destroyed parts leave the grid,
//! connectivity is recomputed per damaged piece, and splits are reported as events with everything the
//! physics layer needs to create the new bodies.
//!
//! Damage is applied immediately; topology changes are batched: [`Ship::resolve_breakage`] processes every
//! damaged piece once, however many of its parts died this tick.

use std::sync::Arc;

use data::Cell;

use crate::ship::design::PartIdx;
use crate::events::{PieceSnapshot, SimEvent, SplitEvent};
use crate::ids::IdAlloc;
use crate::ship::mass::to_piece;
use crate::math::{Rot, Vec2};
use crate::ship::{Body, Piece, Ship, inherited_velocity};

/// What a hit did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DamageOutcome {
    /// Nothing there (dead part, empty cell, unknown ship) or an amount that is not positive and finite.
    Ignored,
    /// The part survived with this many hit points.
    Damaged { hp_left: f32 },
    /// The part was destroyed; `excess` is the damage beyond its hit points (it carries on in penetrating shots
    /// and ram crush).
    Destroyed { excess: f32 },
}

/// Where damage lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageTarget {
    Part(PartIdx),
    /// A blueprint cell; hits whatever part occupies it.
    Cell(Cell),
}

impl Ship {
    /// Applies damage to a part. A destroyed part is removed from the grid at once; its piece is
    /// re-checked for splits in the next [`Ship::resolve_breakage`].
    pub fn apply_damage(&mut self, target: DamageTarget, amount: f32) -> DamageOutcome {
        let part = match target {
            DamageTarget::Part(p) => p as usize,
            DamageTarget::Cell(c) => match self.grid.occupant(c) {
                Some(p) => p,
                None => return DamageOutcome::Ignored,
            },
        };
        if part >= self.parts.len()
            || !self.parts[part].alive
            || !(amount > 0.0 && amount.is_finite())
        {
            return DamageOutcome::Ignored;
        }
        let ps = &mut self.parts[part];
        ps.hp -= amount;
        if ps.hp > 0.0 {
            return DamageOutcome::Damaged { hp_left: ps.hp };
        }
        let excess = -ps.hp;
        self.remove_part(part);
        self.pending_destroyed.push(part as PartIdx);
        DamageOutcome::Destroyed { excess }
    }

    /// Takes an alive part out of the ship: dead, out of the grid, its neighbours' exposure and bonuses
    /// refreshed, its piece re-checked in the next [`Ship::resolve_breakage`]. The destroy path without the
    /// destruction: no `PartDestroyed` event, no chain reaction, no fragment. Damage adds those
    /// ([`Ship::apply_damage`]); salvage strips parts and pre-breaks wrecks with this alone (`salvage.rs`).
    pub(crate) fn remove_part(&mut self, part: usize) {
        let ps = &mut self.parts[part];
        ps.hp = 0.0;
        ps.alive = false;
        ps.heat = 0.0;
        ps.overheated = false;
        ps.power = 0.0;
        ps.work = 0.0;
        let piece = ps.piece;
        self.grid.remove(part);
        // Exposure changes only for the linked neighbours; bonuses only for them and their neighbours
        // (a source died, or a source's venting changed).
        let design = Arc::clone(&self.design);
        let nb = design.neighbors.get(part);
        let mut affected = std::mem::take(&mut self.scratch.affected);
        affected.clear();
        for &n in nb {
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
            self.pieces[k].dirty = true;
        }
    }

    /// The controlling piece keeps control only while it has a core. Returns true if control was lost now.
    fn check_control(&mut self, k: usize) -> bool {
        let p = &mut self.pieces[k];
        if p.controlled && !p.has_core {
            p.controlled = false;
            p.active = false;
            self.control = None;
            return true;
        }
        false
    }

    /// Chain reactions of a ship on its own, without a world (GDR-0084, GDR-0085): every destroyed part not yet
    /// blasted that has an `explosion` hits its own piece ([`Ship::own_blast`]), and what it destroys explodes in
    /// turn, first destroyed first. In a [`Sim`](crate::Sim) the world's chain pass (`chain.rs`, which also
    /// reaches other pieces and ships) has run before, so this finds nothing left to do.
    fn chain_reactions(&mut self) {
        let mut hits = Vec::new();
        while self.chained < self.pending_destroyed.len() {
            let p = self.pending_destroyed[self.chained] as usize;
            self.chained += 1;
            hits.clear();
            self.own_blast(p, &mut hits);
            for &(t, damage) in &hits {
                self.apply_damage(DamageTarget::Part(t), damage);
            }
        }
    }

    /// Processes all pieces that lost parts: sets off the chain reactions left ([`Ship::chain_reactions`]), drops
    /// empty pieces, recomputes connectivity, rebuilds cached data and splits pieces that came apart. Emits events
    /// in a deterministic order (destroyed parts in the order they died, each exploding one followed by its
    /// `Explosion`, then pieces by ascending id).
    pub fn resolve_breakage(&mut self, ids: &mut IdAlloc, events: &mut Vec<SimEvent>) {
        self.chain_reactions();
        let ship = self.id;
        let destroyed = std::mem::take(&mut self.pending_destroyed);
        for &p in &destroyed {
            let piece = self.parts[p as usize].piece;
            let explosion = self.design.parts[p as usize].explosion.as_ref();
            events.push(SimEvent::PartDestroyed {
                ship,
                piece,
                part: p,
                volatile: explosion.is_some(),
            });
            if let (Some(e), Some((at, _))) = (explosion, self.part_world(p)) {
                events.push(SimEvent::Explosion {
                    ship,
                    piece,
                    part: p,
                    at,
                    radius: e.radius,
                    damage: e.damage,
                });
            }
        }
        self.pending_destroyed = destroyed;
        self.pending_destroyed.clear();
        self.chained = 0;

        let mut k = 0;
        while k < self.pieces.len() {
            if !self.pieces[k].dirty {
                k += 1;
                continue;
            }
            // Only this resolution clears it: a rebuild for other reasons (`Sim::set_config`) must not drop
            // a part destroyed since the last resolution.
            self.pieces[k].dirty = false;
            let alive: Vec<PartIdx> = self.pieces[k]
                .parts
                .iter()
                .copied()
                .filter(|&p| self.parts[p as usize].alive)
                .collect();
            if alive.is_empty() {
                let id = self.pieces[k].id;
                self.pieces.remove(k);
                events.push(SimEvent::PieceDestroyed { ship, piece: id });
                if self.control == Some(id) {
                    self.control = None;
                    events.push(SimEvent::ControlLost { ship });
                }
                continue;
            }
            let mut comps = self.components(&alive);
            let parent_id = self.pieces[k].id;
            let parent_body = self.pieces[k].body;
            let parent_origin = self.pieces[k].origin;
            let com_old = self.pieces[k].mass.com;
            let rot = Rot::from_angle(parent_body.angle);

            if comps.len() == 1 {
                self.pieces[k].parts = comps.pop().unwrap();
                self.rebuild_piece(k);
                let lost = self.check_control(k);
                let p = &mut self.pieces[k];
                p.body.linvel = inherited_velocity(
                    parent_body.linvel,
                    parent_body.angvel,
                    rot.apply(com_old),
                    rot.apply(p.mass.com),
                );
                events.push(SimEvent::PieceChanged {
                    ship,
                    piece: snapshot(p, Vec2::ZERO),
                });
                if lost {
                    events.push(SimEvent::ControlLost { ship });
                }
                k += 1;
                continue;
            }

            // Split: the main component keeps the parent's id, frame and body.
            let main = self.main_component(&comps);
            let kept_parts = std::mem::take(&mut comps[main]);
            let mut new_ids = Vec::with_capacity(comps.len() - 1);
            for (c, comp) in comps.iter().enumerate() {
                if c == main {
                    continue;
                }
                let id = ids.piece();
                for &p in comp {
                    self.parts[p as usize].piece = id;
                }
                new_ids.push((c, id));
            }
            self.pieces[k].parts = kept_parts;
            self.rebuild_piece(k);
            let lost = self.check_control(k);
            {
                let p = &mut self.pieces[k];
                p.body.linvel = inherited_velocity(
                    parent_body.linvel,
                    parent_body.angvel,
                    rot.apply(com_old),
                    rot.apply(p.mass.com),
                );
            }
            let kept = snapshot(&self.pieces[k], Vec2::ZERO);
            let mut spawned = Vec::with_capacity(new_ids.len());
            for (c, id) in new_ids {
                let parts = std::mem::take(&mut comps[c]);
                let origin = self.com_bp(&parts);
                // New frame origin = its centre of mass, expressed in the parent's frame.
                let d = to_piece(origin, parent_origin);
                let body = Body {
                    pos: parent_body.pos + rot.apply(d),
                    angle: parent_body.angle,
                    linvel: inherited_velocity(
                        parent_body.linvel,
                        parent_body.angvel,
                        rot.apply(com_old),
                        rot.apply(d),
                    ),
                    angvel: parent_body.angvel,
                };
                // Ids grow monotonically, so pushing keeps `pieces` sorted by id.
                self.pieces.push(Piece::new(id, parts, origin, body));
                let last = self.pieces.len() - 1;
                self.rebuild_piece(last);
                spawned.push(snapshot(&self.pieces[last], d));
            }
            events.push(SimEvent::Split(SplitEvent {
                ship,
                parent: parent_id,
                kept,
                spawned,
            }));
            if lost {
                events.push(SimEvent::ControlLost { ship });
            }
            k += 1;
        }
    }
}

fn snapshot(p: &Piece, offset: Vec2) -> PieceSnapshot {
    PieceSnapshot {
        id: p.id,
        parts: p.parts.clone(),
        origin: p.origin,
        offset_in_parent: offset,
        mass: p.mass,
        colliders: p.colliders.clone(),
        body: p.body,
        salvage: p.salvage(),
    }
}
