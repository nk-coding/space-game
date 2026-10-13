//! Weapons v1 (every hit test is a ray cast through the physics world, resolved to a part by
//! [`Sim::cast_part`]: parts destroyed earlier in the tick, whose colliders stay until breakage is resolved, are
//! passed through to the part behind):
//!
//! * **Beams** (continuous hit-scan): `dps × work × DT` per tick on the first block in range.
//! * **Pulses** (discrete hit-scan): a cooldown per gun; each pulse deals `damage / rate` to the first block.
//! * **Projectiles** (kinetic): world entities with the shooter's velocity, a lifetime of `range / speed`; each
//!   tick the segment they travel is ray-cast. On a hit they damage the block and, if it breaks, carry the
//!   excess through up to `penetration` blocks of that piece (grid walk). Recoil pushes the shooter. Shallow
//!   hits on a slope face (more than 60° from its normal) ricochet with less speed and damage.
//! * **No friendly fire** (GDR-0048): shots ignore the shooter's own ship and every vessel of its owner (the
//!   owner it was fired with, kept by projectiles): no hit, no damage, no shield drain; they fly on. Salvage and
//!   other owners are hit.
//! * **Shields**: every working emitter projects a bubble (`shield_radius`) around itself. A shot that enters a
//!   bubble of a non-friendly ship loses `damage × absorb[family]` to it (up to what the bubble has left);
//!   bubbles along the path are crossed in order, so overlapping bubbles stack. A shot starting inside a bubble
//!   passes.
//!   Ram damage (GDR-0045) at a contact point inside bubbles is absorbed the same way, with the kinetic share,
//!   by every bubble covering the point that is not friendly to the rammer ([`Sim::ram_shields`]). Shots, plumes
//!   and rams share one rule for which bubbles take damage ([`absorbs_from`], GDR-0067).
//! * **Ram crush** (GDR-0047): what is left of the ram damage hits the touching part and carries inward past
//!   the parts it destroys, along the same grid walk as a penetrating shot ([`Sim::ram_crush`]).
//!
//! Order inside a tick (after the ship systems, before physics): bubbles are collected, live projectiles fly,
//! then every gun fires, then engine plumes burn (`plume.rs`); breakage is resolved afterwards. All loops run in
//! id/index order.
//! Numbers in [`tuning`] are **placeholders**.

use data::{Delivery, WeaponFamily};

use crate::DT;
use crate::breakage::{DamageOutcome, DamageTarget};
use crate::design::{FAMILIES, PartIdx, family_index};
use crate::events::SimEvent;
use crate::hash::{StateHash, StateHasher};
use crate::ids::{PieceId, ShipId};
use crate::mass::to_piece;
use crate::math::{Rot, Vec2};
use crate::physics::BodyKey;
use crate::physics::RayHit;
use crate::ship::Ship;
use crate::vessel::{Owner, friendly};
use crate::world::Sim;

/// Weapon tuning. **PLACEHOLDERS.**
pub mod tuning {
    /// A kinetic shot ricochets off a slope face when it comes in more than 60° from the face normal
    /// (`cos(angle) < RICOCHET_COS`).
    pub const RICOCHET_COS: f32 = 0.5;
    /// Speed and damage kept after a ricochet.
    pub const RICOCHET_SPEED: f32 = 0.6;
    pub const RICOCHET_DAMAGE: f32 = 0.5;
    /// At most this many ricochets per projectile.
    pub const MAX_RICOCHETS: u8 = 2;
    /// Ray segments per projectile per tick (ricochets add segments).
    pub const MAX_SEGMENTS: usize = 4;
    /// Grid cells a penetrating shot may walk through after its first hit (also the walk through parts destroyed
    /// earlier in the tick, `Sim::cast_part`).
    pub const MAX_WALK: usize = 64;
    /// A ray that passed through parts destroyed earlier in the tick is cast again from where it left their
    /// piece, at most this many times (`Sim::cast_part`).
    pub const MAX_RECASTS: usize = 4;
    /// A pulse or projectile gun fires at most this many shots per tick (240 per second); a faster rate (mod
    /// content, stacked fire-rate bonuses) is capped there and the rest waits in its cooldown. Without the cap a
    /// rate whose period is below half an ulp of the cooldown never ended the firing loop.
    pub const MAX_SHOTS_PER_TICK: u32 = 8;
    /// A collapsed shield bubble comes back once it has recharged this share of its capacity.
    pub const SHIELD_RESTART: f32 = 0.3;
}

/// Stable id of a projectile (monotonic, never reused).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectileId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotKind {
    Beam,
    Pulse,
    Projectile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoneReason {
    /// Stopped in a hull.
    Hit,
    /// A shield took all its damage.
    Absorbed,
    /// Flew its full range.
    Expired,
}

/// A flying shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projectile {
    pub id: ProjectileId,
    /// The ship that fired it (never hit by it).
    pub owner: ShipId,
    /// The owner it was fired for (GDR-0048): it passes through every vessel of this owner, even if the ship
    /// that fired it has become salvage since. `None`: fired by salvage (tools), friendly to nobody.
    pub team: Option<Owner>,
    pub family: WeaponFamily,
    /// World position now and at the end of the previous tick (render: lerp `prev_pos → pos` by alpha).
    pub pos: Vec2,
    pub prev_pos: Vec2,
    pub vel: Vec2,
    /// Damage it deals on its next hit.
    pub damage: f32,
    /// Blocks it may still go through (1 = stops at the next).
    pub pierce: u32,
    /// Seconds left.
    pub life: f32,
    pub ricochets: u8,
}

/// A beam that fired this tick (for drawing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamTrace {
    pub ship: ShipId,
    pub piece: PieceId,
    pub gun: PartIdx,
    pub from: Vec2,
    /// Where it ended: a hull, a shield, or full range.
    pub to: Vec2,
    pub hit: bool,
}

/// A shield bubble this tick (for drawing and hit tests).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bubble {
    pub ship: ShipId,
    pub piece: PieceId,
    /// The emitter.
    pub part: PartIdx,
    pub centre: Vec2,
    pub radius: f32,
    /// Capacity left and full capacity.
    pub left: f32,
    pub capacity: f32,
    pub absorb: [f32; FAMILIES],
    pub(crate) ship_index: usize,
    /// The bubble's vessel owner (bubbles exist only on running vessels).
    pub(crate) team: Option<crate::vessel::Owner>,
}

/// Does bubble `bub` take damage of family index `fi` from a source on ship `by` of team `team`? One rule for
/// shots, plumes and rams (GDR-0067): never the bubbles of the source's own ship nor of vessels friendly to it
/// (no friendly fire, GDR-0048, so a wingman's bubble never shields an enemy from a friend's ram), and only a
/// bubble with charge left that absorbs the family and whose emitter is still alive (it may have died, or left
/// with a split-off vessel, since the bubbles were collected this tick). The geometry (a shot's path entering
/// the bubble, a plume's or a ram's point inside it) is the caller's.
pub(crate) fn absorbs_from(
    bub: &Bubble,
    ships: &[Ship],
    by: ShipId,
    team: Option<Owner>,
    fi: usize,
) -> bool {
    bub.ship != by
        && !friendly(team, bub.team)
        && bub.left > 0.0
        && bub.absorb[fi] > 0.0
        && ships[bub.ship_index].parts[bub.part as usize].alive
}

impl StateHash for Projectile {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.id.0);
        h.u32(self.owner.0);
        h.u32(self.team.map_or(u32::MAX, |o| o.0 as u32));
        h.u32(family_index(self.family) as u32);
        h.vec2(self.pos);
        h.vec2(self.vel);
        h.f32(self.damage);
        h.u32(self.pierce);
        h.f32(self.life);
        h.u8(self.ricochets);
    }
}

/// A straight walk through the blueprint grid cells of a piece (a DDA), for damage that carries on through the
/// hull: penetrating shots and ram crush. Blueprint frame (y down).
struct GridWalk {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    cx: i32,
    cy: i32,
    sx: i32,
    sy: i32,
    tx: f32,
    ty: f32,
    tdx: f32,
    tdy: f32,
}

impl GridWalk {
    /// From the piece-frame point `local` of a piece whose frame origin is the blueprint point `origin`, along
    /// the piece-frame direction `dir` (unit); the walk starts just inside the cell under the point.
    fn new(origin: [f32; 2], local: Vec2, dir: Vec2) -> GridWalk {
        let (dx, dy) = (dir.x, -dir.y);
        let (mut x, mut y) = (origin[0] + local.x, origin[1] - local.y);
        x += dx * 1e-3;
        y += dy * 1e-3;
        let (cx, cy) = (x.floor() as i32, y.floor() as i32);
        let (sx, sy) = (if dx > 0.0 { 1 } else { -1 }, if dy > 0.0 { 1 } else { -1 });
        let inv = |d: f32| {
            if d.abs() > 1e-9 {
                1.0 / d.abs()
            } else {
                f32::INFINITY
            }
        };
        let next = |c: i32, p: f32, d: f32| {
            if d > 0.0 {
                (c as f32 + 1.0 - p) * inv(d)
            } else if d < 0.0 {
                (p - c as f32) * inv(d)
            } else {
                f32::INFINITY
            }
        };
        GridWalk {
            x,
            y,
            dx,
            dy,
            cx,
            cy,
            sx,
            sy,
            tx: next(cx, x, dx),
            ty: next(cy, y, dy),
            tdx: inv(dx),
            tdy: inv(dy),
        }
    }

    /// Steps into the next cell: the cell, and the distance along the walk where it is entered.
    fn step(&mut self) -> ((i32, i32), f32) {
        let travelled;
        if self.tx < self.ty {
            self.cx += self.sx;
            travelled = self.tx;
            self.tx += self.tdx;
        } else {
            self.cy += self.sy;
            travelled = self.ty;
            self.ty += self.tdy;
        }
        ((self.cx, self.cy), travelled)
    }

    /// The piece-frame point at distance `t` along the walk.
    fn local_at(&self, origin: [f32; 2], t: f32) -> Vec2 {
        Vec2::new(
            self.x + self.dx * t - origin[0],
            origin[1] - (self.y + self.dy * t),
        )
    }
}

/// A ray hit resolved to the alive part it lands on ([`Sim::cast_part`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct PartRayHit {
    /// Where the ray enters that part (the body hit, the distance from the ray's origin, the point and its
    /// piece-frame position, the surface normal; `−dir` when the ray walked through destroyed parts to it).
    pub hit: RayHit,
    /// Index of the ship hit.
    pub k: usize,
    pub part: PartIdx,
}

/// What a ray hit on a body lands on ([`Sim::land`]).
enum Landing {
    Part(PartRayHit),
    /// The ray passes through the piece (destroyed parts only) and leaves it at this distance.
    Through(f32),
}

/// Where a shot went.
struct ShotEnd {
    to: Vec2,
    hit: bool,
}

impl Sim {
    /// Weapons for one tick: bubbles, projectile flight, gun fire, engine plumes (`plume.rs`). Called by `step`
    /// after the systems.
    pub(crate) fn weapons_step(&mut self) {
        self.beams.clear();
        self.collect_bubbles();
        self.advance_projectiles();
        self.fire_guns();
        self.plumes();
    }

    fn collect_bubbles(&mut self) {
        self.bubbles.clear();
        for (k, ship) in self.ships.iter().enumerate() {
            let d = ship.design();
            for piece in &ship.pieces {
                if !piece.active {
                    continue;
                }
                let rot = Rot::from_angle(piece.body.angle);
                for &e in &piece.shields {
                    let ps = &ship.parts[e as usize];
                    if !ps.alive || ps.work <= 0.0 || ps.shield <= 0.0 || ps.shield_down {
                        continue;
                    }
                    let spec = &d.parts[e as usize];
                    let st = &ship.stats[e as usize];
                    let local = to_piece(spec.shape.center(), piece.origin);
                    self.bubbles.push(Bubble {
                        ship: ship.id(),
                        piece: piece.id,
                        part: e,
                        centre: piece.body.pos + rot.apply(local),
                        radius: st.get(crate::Stat::ShieldRadius),
                        left: ps.shield,
                        capacity: st.get(crate::Stat::ShieldCapacity),
                        absorb: spec.absorb,
                        ship_index: k,
                        team: ship.piece_team(piece.id),
                    });
                }
            }
        }
    }

    /// Bubbles that take damage from the shooter `owner` of `team` ([`absorbs_from`]) that the segment
    /// `from + t·dir, t ∈ [0, len]` enters take their share of `damage`, in order along the path. Returns the
    /// distance where the shot was fully absorbed, if it was.
    #[allow(clippy::too_many_arguments)]
    fn shield_pass(
        &mut self,
        owner: ShipId,
        team: Option<Owner>,
        family: WeaponFamily,
        from: Vec2,
        dir: Vec2,
        len: f32,
        damage: &mut f32,
    ) -> Option<f32> {
        if self.bubbles.is_empty() || *damage <= 0.0 {
            return None;
        }
        let fi = family_index(family);
        let entries = &mut self.shield_scratch;
        entries.clear();
        for (b, bub) in self.bubbles.iter().enumerate() {
            if !absorbs_from(bub, &self.ships, owner, team, fi) {
                continue;
            }
            let oc = from - bub.centre;
            let c = oc.length_squared() - bub.radius * bub.radius;
            if c <= 0.0 {
                continue; // starts inside: passes
            }
            let bq = oc.dot(dir);
            let disc = bq * bq - c;
            if disc < 0.0 {
                continue;
            }
            let t = -bq - crate::math::sqrt(disc);
            if t >= 0.0 && t <= len {
                entries.push((t, b as u32));
            }
        }
        if entries.is_empty() {
            return None;
        }
        entries.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for n in 0..self.shield_scratch.len() {
            let (t, b) = self.shield_scratch[n];
            *damage -= self.absorb(b as usize, fi, owner, from + dir * t, *damage);
            if *damage <= 1e-6 {
                *damage = 0.0;
                return Some(t);
            }
        }
        None
    }

    /// Bubble `b` takes its share of `damage` (family index `fi`): `damage × absorb[fi]`, up to what it has
    /// left. Updates the emitter (collapses at 0), reports a `ShieldHit` at `point` by `by` and returns what
    /// it took.
    pub(crate) fn absorb(
        &mut self,
        b: usize,
        fi: usize,
        by: ShipId,
        point: Vec2,
        damage: f32,
    ) -> f32 {
        let bub = &mut self.bubbles[b];
        let take = (damage * bub.absorb[fi]).min(bub.left);
        if take <= 0.0 {
            return 0.0;
        }
        bub.left -= take;
        if bub.left < 1e-6 {
            bub.left = 0.0;
        }
        let (ship_index, part, left) = (bub.ship_index, bub.part, bub.left);
        let ps = &mut self.ships[ship_index].parts[part as usize];
        ps.shield = left;
        if left <= 0.0 {
            ps.shield_down = true;
        }
        let bub = self.bubbles[b];
        self.events.push(SimEvent::ShieldHit {
            by,
            ship: bub.ship,
            piece: bub.piece,
            part,
            point,
            absorbed: take,
            left,
            depleted: left <= 0.0,
        });
        take
    }

    /// Ram damage at a contact `point` (GDR-0045): every bubble of this tick that covers the point takes its
    /// kinetic share of what reaches it (in bubble order: ship, piece, emitter), except the bubbles friendly to
    /// the rammer `by` (its own ship's and its owner's other vessels', GDR-0067: as a shot passes them, so a
    /// wingman's bubble never shields an enemy from the ram; [`absorbs_from`]). Returns the damage left for the
    /// hull.
    pub(crate) fn ram_shields(&mut self, by: BodyKey, point: Vec2, mut damage: f32) -> f32 {
        let fi = family_index(WeaponFamily::Kinetic);
        let team = self.piece_team(by);
        for b in 0..self.bubbles.len() {
            if damage <= 1e-6 {
                return 0.0;
            }
            let bub = &self.bubbles[b];
            if !absorbs_from(bub, &self.ships, by.ship, team, fi)
                || (point - bub.centre).length_squared() > bub.radius * bub.radius
            {
                continue;
            }
            damage -= self.absorb(b, fi, by.ship, point, damage);
        }
        damage
    }

    /// The first alive part along a ray from `from` (unit `dir`, up to `len`) among the bodies `skip` lets
    /// through: a ray cast through the physics, resolved to a part by [`Sim::land`]. A part destroyed earlier in
    /// the tick keeps its collider until breakage is resolved, so a ray that hits it walks on through the piece
    /// to the next alive part of that piece; if the walk leaves the piece first, the ray is cast again from there
    /// (at most `tuning::MAX_RECASTS` times). The one hit rule of shots, plumes and [`Sim::raycast`].
    pub(crate) fn cast_part(
        &self,
        from: Vec2,
        dir: Vec2,
        len: f32,
        skip: impl Fn(BodyKey) -> bool,
    ) -> Option<PartRayHit> {
        let mut start = 0.0f32;
        for _ in 0..=tuning::MAX_RECASTS {
            let h = self
                .physics
                .raycast_skipping(from + dir * start, dir, len - start, &skip)?;
            match self.land(&h, dir)? {
                Landing::Part(mut p) => {
                    p.hit.distance += start;
                    return Some(p);
                }
                // Cast again from just past where the ray left the piece.
                Landing::Through(t) => start += t + 1e-2,
            }
            if start >= len {
                return None;
            }
        }
        None
    }

    /// What a ray hit (unit `dir`) lands on: the alive part under the hit point; else, when that cell's part was
    /// destroyed earlier in the tick (its collider stays until breakage is resolved), the first alive part of
    /// the same piece further along the ray (a grid walk through the destroyed cells), entered at its cell's
    /// boundary; or where the walk leaves the piece. `None` for a body the sim does not know.
    fn land(&self, h: &RayHit, dir: Vec2) -> Option<Landing> {
        let k = self
            .ships
            .binary_search_by_key(&h.key.ship, |s| s.id())
            .ok()?;
        let ship = &self.ships[k];
        let piece = ship.piece(h.key.piece)?;
        let rot = Rot::from_angle(piece.body.angle);
        let n_local = rot.apply_inverse(h.normal);
        if let Some(part) = ship.part_at_local(piece.id, h.local_point - n_local * 1e-3) {
            return Some(Landing::Part(PartRayHit { hit: *h, k, part }));
        }
        let mut walk = GridWalk::new(piece.origin, h.local_point, rot.apply_inverse(dir));
        let (mut cell, mut t) = ((walk.cx, walk.cy), 0.0f32);
        for _ in 0..tuning::MAX_WALK {
            match ship.grid().occupant(cell) {
                Some(o) if ship.parts[o].piece == piece.id => {
                    let local = walk.local_at(piece.origin, t);
                    let hit = RayHit {
                        distance: h.distance + t,
                        point: piece.body.pos + rot.apply(local),
                        local_point: local,
                        normal: dir * -1.0,
                        ..*h
                    };
                    return Some(Landing::Part(PartRayHit { hit, k, part: o as PartIdx }));
                }
                // Another piece's part: the ray has left this piece.
                Some(_) => return Some(Landing::Through(h.distance + t)),
                // Empty: a destroyed part of this piece lets the walk go on, anything else is outside it.
                None => match ship.design().grid.occupant(cell) {
                    Some(o) if !ship.parts[o].alive && ship.parts[o].piece == piece.id => {}
                    _ => return Some(Landing::Through(h.distance + t)),
                },
            }
            (cell, t) = walk.step();
        }
        Some(Landing::Through(h.distance + t))
    }

    /// Damages one part and reports it.
    #[allow(clippy::too_many_arguments)]
    fn damage_part(
        &mut self,
        by: ShipId,
        family: WeaponFamily,
        k: usize,
        part: PartIdx,
        point: Vec2,
        normal: Vec2,
        damage: f32,
        absorbed: f32,
    ) -> DamageOutcome {
        let ship = &mut self.ships[k];
        let piece = ship.part(part).piece;
        let out = ship.apply_damage(DamageTarget::Part(part), damage);
        if out != DamageOutcome::Ignored {
            self.events.push(SimEvent::Hit {
                by,
                ship: ship.id(),
                piece,
                part,
                point,
                normal,
                damage,
                absorbed,
                family,
            });
        }
        out
    }

    /// The first part a shot of ship `owner` (fired for `team`) hits: never its own ship nor a vessel of its
    /// owner (GDR-0048).
    fn shot_cast(
        &self,
        owner: ShipId,
        team: Option<Owner>,
        from: Vec2,
        dir: Vec2,
        len: f32,
    ) -> Option<PartRayHit> {
        self.cast_part(from, dir, len, |key| {
            key.ship == owner || friendly(team, self.body_team(key))
        })
    }

    /// A hit-scan shot (beam tick or pulse): shields along the way, then the first block only.
    #[allow(clippy::too_many_arguments)]
    fn hitscan(
        &mut self,
        owner: ShipId,
        team: Option<Owner>,
        family: WeaponFamily,
        from: Vec2,
        dir: Vec2,
        range: f32,
        damage: f32,
    ) -> ShotEnd {
        let hit = self.shot_cast(owner, team, from, dir, range);
        let len = hit.map_or(range, |h| h.hit.distance);
        let mut left = damage;
        if let Some(t) = self.shield_pass(owner, team, family, from, dir, len, &mut left) {
            return ShotEnd {
                to: from + dir * t,
                hit: false,
            };
        }
        match hit {
            Some(PartRayHit { hit: h, k, part }) => {
                self.damage_part(
                    owner,
                    family,
                    k,
                    part,
                    h.point,
                    h.normal,
                    left,
                    damage - left,
                );
                ShotEnd {
                    to: h.point,
                    hit: true,
                }
            }
            None => ShotEnd {
                to: from + dir * len,
                hit: false,
            },
        }
    }

    /// Does a kinetic shot coming along `dir` glance off the slope face it hit?
    fn ricochets(&self, h: &RayHit, k: usize, part: PartIdx, dir: Vec2) -> bool {
        let ship = &self.ships[k];
        let spec = &ship.design().parts[part as usize];
        let Some(n_bp) = spec.shape.slope_normal() else {
            return false;
        };
        let Some(piece) = ship.piece(h.key.piece) else {
            return false;
        };
        let n_world = Rot::from_angle(piece.body.angle).apply(Vec2::new(n_bp[0], -n_bp[1]));
        // Only the slope face, not its straight sides.
        if n_world.dot(h.normal) < 0.99 {
            return false;
        }
        (-dir).dot(h.normal) < tuning::RICOCHET_COS
    }

    /// A projectile hit `part`: damage it, and while blocks break and penetration is left, carry the excess
    /// through the next blocks of the same piece (grid walk along the shot).
    #[allow(clippy::too_many_arguments)]
    fn penetrate(
        &mut self,
        owner: ShipId,
        family: WeaponFamily,
        h: &RayHit,
        k: usize,
        first: PartIdx,
        dir: Vec2,
        damage: f32,
        absorbed: f32,
        pierce: u32,
    ) {
        let mut out =
            self.damage_part(owner, family, k, first, h.point, h.normal, damage, absorbed);
        let mut pierce = pierce.saturating_sub(1);
        let (origin, rot) = match self.ships[k].piece(h.key.piece) {
            Some(p) => (p.origin, Rot::from_angle(p.body.angle)),
            None => return,
        };
        let pid = h.key.piece;
        // Walk the blueprint grid from the hit point along the shot.
        let mut walk = GridWalk::new(origin, h.local_point, rot.apply_inverse(dir));
        let mut last = first;
        for _ in 0..tuning::MAX_WALK {
            let carry = match out {
                DamageOutcome::Destroyed { excess } if excess > 0.0 && pierce > 0 => excess,
                _ => return,
            };
            let (cell, travelled) = walk.step();
            let ship = &self.ships[k];
            let Some(o) = ship.grid().occupant(cell) else {
                // Left the hull: the shot stops (it does not fly on to other ships).
                return;
            };
            let o = o as PartIdx;
            if o == last || !ship.part(o).alive || ship.part(o).piece != pid {
                continue;
            }
            last = o;
            pierce -= 1;
            let local = walk.local_at(origin, travelled);
            let point = h.point + rot.apply(local - h.local_point);
            out = self.damage_part(owner, family, k, o, point, -dir, carry, 0.0);
        }
    }

    /// Ram damage to the body `key` of ship `k` (GDR-0047): `damage` hits the touching part `first`; what
    /// exceeds its hit points carries on to the next part inward (along `inward`, world, unit, from the contact
    /// `point`), and so on until it is used up or the walk leaves the piece's hull (an empty cell or another
    /// piece's). The emptied cells of a big part just crushed are passed through. A touching part that an earlier
    /// contact of this tick destroyed passes all the damage inward. Returns false if the damage was ignored
    /// (nothing positive to deal). (Shields have taken their share already: [`Sim::ram_shields`].)
    pub(crate) fn ram_crush(
        &mut self,
        k: usize,
        key: BodyKey,
        first: PartIdx,
        point: Vec2,
        inward: Vec2,
        damage: f32,
    ) -> bool {
        let ship = &mut self.ships[k];
        let mut out = if ship.part(first).alive || damage <= 0.0 {
            ship.apply_damage(DamageTarget::Part(first), damage)
        } else {
            DamageOutcome::Destroyed { excess: damage }
        };
        if out == DamageOutcome::Ignored {
            return false;
        }
        let Some(piece) = self.ships[k].piece(key.piece) else {
            return true;
        };
        let (origin, rot) = (piece.origin, Rot::from_angle(piece.body.angle));
        let local = rot.apply_inverse(point - piece.body.pos);
        let mut walk = GridWalk::new(origin, local, rot.apply_inverse(inward));
        let mut last = first;
        for _ in 0..tuning::MAX_WALK {
            let carry = match out {
                DamageOutcome::Destroyed { excess } if excess > 0.0 => excess,
                _ => break,
            };
            let (cell, _) = walk.step();
            let ship = &self.ships[k];
            let o = match ship.grid().occupant(cell) {
                Some(o) => o as PartIdx,
                None if ship.design().grid.occupant(cell) == Some(last as usize) => continue,
                None => break,
            };
            if o == last {
                continue;
            }
            if ship.part(o).piece != key.piece {
                break;
            }
            last = o;
            out = self.ships[k].apply_damage(DamageTarget::Part(o), carry);
        }
        true
    }

    fn advance_projectiles(&mut self) {
        let mut list = std::mem::take(&mut self.projectiles);
        for p in list.iter_mut() {
            p.prev_pos = p.pos;
            p.life -= DT;
            let mut speed = p.vel.length();
            if speed <= 0.0 {
                p.life = 0.0;
            }
            let mut dir = if speed > 0.0 {
                p.vel * (1.0 / speed)
            } else {
                Vec2::Y
            };
            let mut remaining = speed * DT;
            let mut gone: Option<GoneReason> = None;
            for _ in 0..tuning::MAX_SEGMENTS {
                if remaining <= 0.0 {
                    break;
                }
                let hit = self.shot_cast(p.owner, p.team, p.pos, dir, remaining);
                let len = hit.map_or(remaining, |h| h.hit.distance);
                let before = p.damage;
                if let Some(t) =
                    self.shield_pass(p.owner, p.team, p.family, p.pos, dir, len, &mut p.damage)
                {
                    p.pos += dir * t;
                    gone = Some(GoneReason::Absorbed);
                    break;
                }
                let absorbed = before - p.damage;
                let Some(PartRayHit { hit: h, k, part }) = hit else {
                    p.pos += dir * remaining;
                    break;
                };
                if p.family == WeaponFamily::Kinetic
                    && p.ricochets < tuning::MAX_RICOCHETS
                    && self.ricochets(&h, k, part, dir)
                {
                    let n = h.normal;
                    dir = dir - n * (2.0 * dir.dot(n));
                    speed *= tuning::RICOCHET_SPEED;
                    p.damage *= tuning::RICOCHET_DAMAGE;
                    p.ricochets += 1;
                    p.pos = h.point + n * 1e-3;
                    remaining = (remaining - h.distance) * tuning::RICOCHET_SPEED;
                    self.events.push(SimEvent::Ricochet {
                        id: p.id,
                        point: h.point,
                        normal: n,
                        vel: dir * speed,
                    });
                    continue;
                }
                self.penetrate(
                    p.owner, p.family, &h, k, part, dir, p.damage, absorbed, p.pierce,
                );
                p.pos = h.point;
                gone = Some(GoneReason::Hit);
                break;
            }
            p.vel = dir * speed;
            if gone.is_none() && p.life <= 0.0 {
                gone = Some(GoneReason::Expired);
            }
            if let Some(reason) = gone {
                p.life = -1.0;
                self.events.push(SimEvent::ProjectileGone {
                    id: p.id,
                    pos: p.pos,
                    reason,
                });
            }
        }
        // Gone or expired projectiles have `life <= 0`; ids stay ascending.
        list.retain(|p| p.life > 0.0);
        list.append(&mut self.projectiles);
        self.projectiles = list;
    }

    fn fire_guns(&mut self) {
        for k in 0..self.ships.len() {
            let owner = self.ships[k].id();
            for pi in 0..self.ships[k].pieces.len() {
                for gi in 0..self.ships[k].pieces[pi].guns.len() {
                    self.fire_gun(k, owner, pi, gi);
                }
            }
        }
    }

    fn fire_gun(&mut self, k: usize, owner: ShipId, pi: usize, gi: usize) {
        let ship: &Ship = &self.ships[k];
        let team = ship.team();
        let piece = &ship.pieces[pi];
        let g = piece.guns[gi];
        let slot = g.slot as usize;
        let ps = ship.part(g.part);
        let firing = piece.active && ps.alive && g.output > 0.0;
        if !firing {
            let cd = &mut self.ships[k].gun_cooldown[slot];
            *cd = (*cd - DT).max(0.0);
            return;
        }
        let work = ps.work;
        let pid = piece.id;
        let rot = Rot::from_angle(piece.body.angle);
        let from = piece.body.pos + rot.apply(g.muzzle);
        let dir = rot.apply(g.dir);
        let com = piece.com_world();
        let v_muzzle = piece.body.linvel + Vec2::cross_scalar(piece.body.angvel, from - com);
        let com_local = piece.mass.com;
        match g.delivery {
            Delivery::Beam => {
                let end = self.hitscan(owner, team, g.family, from, dir, g.range, g.output * DT);
                self.beams.push(BeamTrace {
                    ship: owner,
                    piece: pid,
                    gun: g.part,
                    from,
                    to: end.to,
                    hit: end.hit,
                });
            }
            Delivery::Pulse { .. } | Delivery::Projectile { .. } => {
                if g.rate <= 0.0 {
                    return;
                }
                self.ships[k].gun_cooldown[slot] -= DT * work;
                let mut shots = 0;
                while self.ships[k].gun_cooldown[slot] <= 0.0 && shots < tuning::MAX_SHOTS_PER_TICK {
                    shots += 1;
                    self.ships[k].gun_cooldown[slot] += 1.0 / g.rate;
                    match g.delivery {
                        Delivery::Pulse { .. } => {
                            let end = self.hitscan(
                                owner,
                                team,
                                g.family,
                                from,
                                dir,
                                g.range,
                                g.shot_damage,
                            );
                            self.events.push(SimEvent::ShotFired {
                                ship: owner,
                                piece: pid,
                                gun: g.part,
                                kind: ShotKind::Pulse,
                                from,
                                to: end.to,
                                projectile: None,
                            });
                        }
                        Delivery::Projectile {
                            speed,
                            penetration,
                            recoil,
                            ..
                        } => {
                            let id = self.ids.projectile();
                            let vel = v_muzzle + dir * speed;
                            self.projectiles.push(Projectile {
                                id,
                                owner,
                                team,
                                family: g.family,
                                pos: from,
                                prev_pos: from,
                                vel,
                                damage: g.shot_damage,
                                pierce: penetration.max(1),
                                life: if speed > 0.0 { g.range / speed } else { 0.0 },
                                ricochets: 0,
                            });
                            self.events.push(SimEvent::ShotFired {
                                ship: owner,
                                piece: pid,
                                gun: g.part,
                                kind: ShotKind::Projectile,
                                from,
                                to: from + dir * g.range,
                                projectile: Some(id),
                            });
                            self.events.push(SimEvent::ProjectileSpawned {
                                id,
                                owner,
                                pos: from,
                                vel,
                            });
                            // Recoil: the shot's impulse pushes back at the muzzle for one tick.
                            if recoil > 0.0 {
                                let f = g.dir * (-recoil / DT);
                                let piece = &mut self.ships[k].pieces[pi];
                                piece.wrench.force += f;
                                piece.wrench.torque += (g.muzzle - com_local).cross(f);
                            }
                        }
                        Delivery::Beam => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::design::ShipDesign;
    use crate::ship::Body;

    /// A 5×4 wall of steel blocks (1 hp each), salvage, its frame origin at (0.4, 20).
    fn wall() -> (Sim, usize, BodyKey, Arc<ShipDesign>) {
        let content = data::base_content();
        let mut parts = Vec::new();
        for y in 0..4 {
            for x in 0..5 {
                parts.push(data::Placement {
                    part: "armor_block".into(),
                    x,
                    y,
                    rot: 0,
                    flip: false,
                    material: Some("steel".into()),
                    power: None,
                });
            }
        }
        let bp = data::Blueprint {
            id: "wall".into(),
            name: "Wall".into(),
            summary: "Test wall.".into(),
            description: None,
            lore: None,
            tags: vec![],
            faction: "ship_mind".into(),
            size: (5, 4),
            parts,
            behavior: None,
            source: Default::default(),
        };
        let d = Arc::new(ShipDesign::compile(&content, &bp).unwrap());
        let mut sim = Sim::new(1);
        let id = sim.spawn_ship(d.clone(), Owner(1), Body::at(Vec2::new(0.4, 20.0), 0.0));
        let key = BodyKey {
            ship: id,
            piece: sim.ship(id).unwrap().pieces()[0].id,
        };
        (sim, 0, key, d)
    }

    /// Two rams on the same touching part in one tick (13 Oct 2026 review): the second finds it destroyed and
    /// crushes on inward from its cells instead of being dropped.
    #[test]
    fn a_second_ram_on_a_destroyed_part_crushes_on_inward() {
        let (mut sim, k, key, d) = wall();
        let cell = |x: i32, y: i32| d.grid.occupant((x, y)).unwrap() as PartIdx;
        let hp = |sim: &Sim, x: i32, y: i32| {
            let p = sim.ships[k].part(cell(x, y));
            if p.alive { p.hp } else { 0.0 }
        };
        // The bottom face of column 2 (blueprint row 3), pushed up into the wall (+y).
        let (point, inward) = (Vec2::new(0.4, 18.0), Vec2::Y);
        assert!(sim.ram_crush(k, key, cell(2, 3), point, inward, 1.5));
        assert_eq!((hp(&sim, 2, 3), hp(&sim, 2, 2)), (0.0, 0.5));
        assert!(sim.ram_crush(k, key, cell(2, 3), point, inward, 1.5));
        assert_eq!((hp(&sim, 2, 2), hp(&sim, 2, 1)), (0.0, 0.0), "the second ram crushes on");
        assert_eq!(hp(&sim, 2, 0), 1.0);
        assert!(!sim.ram_crush(k, key, cell(2, 3), point, inward, 0.0), "nothing to deal");
    }
}
