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
//!   bubbles along the path are crossed in order, so overlapping bubbles stack. A shot fired from inside a
//!   bubble (its muzzle in it: point blank) passes it. A projectile is taken once by each bubble, also by one it
//!   finds itself inside in flight (that ship moved onto it since the last tick: ships move after the weapons).
//!   Ram damage (GDR-0045) at a contact point inside bubbles is absorbed the same way, with the kinetic share,
//!   by every bubble covering the point that is not friendly to the rammer ([`Sim::ram_shields`]). Shots, plumes
//!   and rams share one rule for which bubbles take damage ([`absorbs_from`], GDR-0067).
//! * **Ram crush** (GDR-0047): what is left of the ram damage hits the touching part and carries inward past
//!   the parts it destroys, along the same grid walk as a penetrating shot ([`Sim::ram_crush`]).
//!
//! Order inside a tick (after the ship systems, before physics): bubbles are collected, live projectiles fly,
//! then every gun fires, then engine plumes burn (`plume.rs`); breakage is resolved afterwards. All loops run in
//! id/index order. **Fire is mutual** (ADR-0037): every gun's shot is traced against the state at the start of
//! the fire pass and the hits are applied once all guns have fired ([`Sim::fire_guns`]), as plumes are, so a ship
//! never shoots an enemy's gun off before it fires in the same tick, whatever their ids.
//! Numbers in [`tuning`] are **placeholders**.

use data::{Delivery, WeaponFamily};

use crate::DT;
use crate::ship::breakage::{DamageOutcome, DamageTarget};
use crate::ship::design::{FAMILIES, PartIdx, family_index};
use crate::events::SimEvent;
use crate::hash::{StateHash, StateHasher};
use crate::ids::{PieceId, ShipId};
use crate::ship::mass::to_piece;
use crate::math::{Rot, Vec2};
use crate::flight::physics::BodyKey;
use crate::flight::physics::RayHit;
use crate::ship::Ship;
use crate::ship::vessel::{Owner, friendly};
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
    /// A projectile remembers this many bubbles it is done with ([`super::Crossed`]).
    pub const MAX_CROSSED: usize = 8;
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
    /// A missile that point defence shot down (`missile.rs`).
    ShotDown,
    /// A missile spent on the flare decoy that lured it (`flare.rs`).
    Decoyed,
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
    pub(crate) crossed: Crossed,
    /// A missile's seeker (GDR-0115, `missile.rs`): it homes, can be shot down, and its warhead blasts where it
    /// strikes. `None`: a shell.
    pub missile: Option<super::missile::Seeker>,
}

/// The shield bubbles a projectile is done with, as `(ship, emitter)`: those its muzzle was inside when it was
/// fired (point blank: it passes them) and every one that has taken its share since, so each bubble takes a share
/// of it once. The last [`tuning::MAX_CROSSED`] of them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Crossed {
    list: [(ShipId, PartIdx); tuning::MAX_CROSSED],
    len: u8,
}

impl Crossed {
    fn has(&self, b: &Bubble) -> bool {
        self.list[..self.len as usize].contains(&(b.ship, b.part))
    }

    /// Adds `b`, dropping the oldest when full.
    fn add(&mut self, b: &Bubble) {
        if self.len as usize == tuning::MAX_CROSSED {
            self.list.copy_within(1.., 0);
            self.len -= 1;
        }
        self.list[self.len as usize] = (b.ship, b.part);
        self.len += 1;
    }
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
    pub(crate) team: Option<crate::ship::vessel::Owner>,
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
        h.u8(self.crossed.len);
        for &(ship, part) in &self.crossed.list[..self.crossed.len as usize] {
            h.u32(ship.0);
            h.u32(part);
        }
        match &self.missile {
            Some(m) => {
                h.u8(1);
                m.hash_state(h);
            }
            None => h.u8(0),
        }
    }
}

/// A straight walk through the blueprint grid cells of a piece (a DDA), for damage that carries on through the
/// hull: penetrating shots and ram crush. Blueprint frame (y down).
pub(crate) struct GridWalk {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    pub(crate) cx: i32,
    pub(crate) cy: i32,
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
    pub(crate) fn new(origin: [f32; 2], local: Vec2, dir: Vec2) -> GridWalk {
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
    pub(crate) fn step(&mut self) -> ((i32, i32), f32) {
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
    pub(crate) fn local_at(&self, origin: [f32; 2], t: f32) -> Vec2 {
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

/// What a ray met first ([`Sim::cast_part`]): an alive part, or an asteroid rock (GDR-0104), which stops it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Cast {
    Part(PartRayHit),
    Rock(RayHit),
}

impl Cast {
    /// Distance from the ray's origin.
    pub(crate) fn distance(&self) -> f32 {
        match self {
            Cast::Part(p) => p.hit.distance,
            Cast::Rock(h) => h.distance,
        }
    }

    /// The part hit, if it was one.
    pub(crate) fn part(self) -> Option<PartRayHit> {
        match self {
            Cast::Part(p) => Some(p),
            Cast::Rock(_) => None,
        }
    }
}

/// What a ray hit on a body lands on ([`Sim::land`]).
enum Landing {
    Part(PartRayHit),
    /// An asteroid rock: the ray stops on it.
    Rock(RayHit),
    /// The ray passes through the piece (destroyed parts only) and leaves it at this distance.
    Through(f32),
}

/// A gun's hit on a part, traced against the state at the start of the fire pass and applied once every gun
/// has fired ([`Sim::fire_guns`]): the shot from `from` along `dir` (unit, up to `range`) of ship `by`, its
/// damage after shields.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GunHit {
    by: ShipId,
    team: Option<Owner>,
    family: WeaponFamily,
    from: Vec2,
    dir: Vec2,
    range: f32,
    hit: PartRayHit,
    damage: f32,
    absorbed: f32,
    /// Static per point of `damage` (disruption, GDR-0125; 0: none).
    disrupt: f32,
    /// A lance's fall-off through the struck piece (GDR-0127; `None`: the first block only).
    lance: Option<data::LanceSpec>,
}

/// Where a shot went, and the asteroid rock it stopped on, if any (mining, GDR-0114).
struct ShotEnd {
    to: Vec2,
    hit: bool,
    rock: Option<u32>,
}

impl Sim {
    /// Weapons for one tick: bubbles, projectile flight, gun fire, engine plumes (`plume.rs`). Called by `step`
    /// after the systems.
    pub(crate) fn weapons_step(&mut self) {
        self.beams.clear();
        self.jam_step();
        self.collect_bubbles();
        self.flares_step();
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

    /// The bubbles a shot of the shooter `owner` of `team` fired at `point` starts inside, of those that take its
    /// damage ([`absorbs_from`]): a projectile passes them (point blank).
    pub(crate) fn crossed_at(
        &self,
        owner: ShipId,
        team: Option<Owner>,
        family: WeaponFamily,
        point: Vec2,
    ) -> Crossed {
        let fi = family_index(family);
        let mut crossed = Crossed {
            list: [(ShipId(0), 0); tuning::MAX_CROSSED],
            len: 0,
        };
        for bub in &self.bubbles {
            if absorbs_from(bub, &self.ships, owner, team, fi)
                && (point - bub.centre).length_squared() <= bub.radius * bub.radius
            {
                crossed.add(bub);
            }
        }
        crossed
    }

    /// Bubbles that take damage from the shooter `owner` of `team` ([`absorbs_from`]) along the segment
    /// `from + t·dir, t ∈ [0, len]` take their share of `damage`, in order along the path: those the segment
    /// enters, and for a projectile (its `crossed`) also those it starts inside, at its start. A hit-scan shot
    /// starts at its muzzle, so it passes a bubble it starts in (point blank); a projectile inside a bubble it has
    /// not been taken by was reached by that ship moving onto it since the last tick. A projectile is taken once by
    /// each bubble: those in `crossed` are skipped and those that take their share added. Returns the distance
    /// where the shot was fully absorbed, if it was.
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
        mut crossed: Option<&mut Crossed>,
    ) -> Option<f32> {
        if self.bubbles.is_empty() || *damage <= 0.0 {
            return None;
        }
        let fi = family_index(family);
        let entries = &mut self.shield_scratch;
        entries.clear();
        for (b, bub) in self.bubbles.iter().enumerate() {
            if !absorbs_from(bub, &self.ships, owner, team, fi)
                || crossed.as_ref().is_some_and(|c| c.has(bub))
            {
                continue;
            }
            let oc = from - bub.centre;
            let c = oc.length_squared() - bub.radius * bub.radius;
            if c <= 0.0 {
                if crossed.is_some() {
                    entries.push((0.0, b as u32));
                }
                continue;
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
            if let Some(c) = crossed.as_deref_mut() {
                c.add(&self.bubbles[b as usize]);
            }
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
    ) -> Option<Cast> {
        let mut start = 0.0f32;
        for _ in 0..=tuning::MAX_RECASTS {
            let h = self
                .physics
                .raycast_skipping(from + dir * start, dir, len - start, &skip)?;
            match self.land(&h, dir)? {
                Landing::Part(mut p) => {
                    p.hit.distance += start;
                    return Some(Cast::Part(p));
                }
                Landing::Rock(mut h) => {
                    h.distance += start;
                    return Some(Cast::Rock(h));
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
        if h.key.as_rock().is_some() {
            return Some(Landing::Rock(*h));
        }
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

    /// A shot of `by` stopped on an asteroid rock (GDR-0104): reported, nothing damaged.
    fn rock_hit(&mut self, by: ShipId, family: WeaponFamily, h: &RayHit) {
        if let Some(rock) = h.key.as_rock() {
            self.events.push(SimEvent::RockHit { by, rock, point: h.point, normal: h.normal, family });
        }
    }

    /// Damages one part and reports it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn damage_part(
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
    ) -> Option<Cast> {
        self.cast_part(from, dir, len, |key| {
            key.ship == owner || friendly(team, self.body_team(key))
        })
    }

    /// A hit-scan shot (beam tick or pulse): shields along the way, then the first block only (a lance: every part
    /// on its line through that piece, GDR-0127). The block's damage (and the static it carries, `disrupt` per point:
    /// disruption, GDR-0125) waits in `gun_hits` until every gun has fired ([`Sim::fire_guns`]).
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
        disrupt: f32,
        lance: Option<data::LanceSpec>,
    ) -> ShotEnd {
        let hit = self.shot_cast(owner, team, from, dir, range);
        let len = hit.map_or(range, |h| h.distance());
        let mut left = damage;
        if let Some(t) = self.shield_pass(owner, team, family, from, dir, len, &mut left, None) {
            return ShotEnd {
                to: from + dir * t,
                hit: false,
                rock: None,
            };
        }
        match hit {
            Some(Cast::Rock(h)) => {
                self.rock_hit(owner, family, &h);
                ShotEnd { to: h.point, hit: true, rock: h.key.as_rock() }
            }
            Some(Cast::Part(h)) => {
                self.gun_hits.push(GunHit {
                    by: owner,
                    team,
                    family,
                    from,
                    dir,
                    range,
                    hit: h,
                    damage: left,
                    absorbed: damage - left,
                    disrupt,
                    lance,
                });
                ShotEnd {
                    to: h.hit.point,
                    hit: true,
                    rock: None,
                }
            }
            None => ShotEnd {
                to: from + dir * len,
                hit: false,
                rock: None,
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
            let decoyed = self.steer_missile(p);
            let mut speed = p.vel.length();
            if speed <= 0.0 {
                p.life = 0.0;
            }
            let mut dir = if speed > 0.0 {
                p.vel * (1.0 / speed)
            } else {
                Vec2::Y
            };
            let mut remaining = if decoyed { 0.0 } else { speed * DT };
            let mut gone = decoyed.then_some(GoneReason::Decoyed);
            for _ in 0..tuning::MAX_SEGMENTS {
                if remaining <= 0.0 {
                    break;
                }
                let hit = self.shot_cast(p.owner, p.team, p.pos, dir, remaining);
                let len = hit.map_or(remaining, |h| h.distance());
                let before = p.damage;
                if let Some(t) = self.shield_pass(
                    p.owner,
                    p.team,
                    p.family,
                    p.pos,
                    dir,
                    len,
                    &mut p.damage,
                    Some(&mut p.crossed),
                ) {
                    p.pos += dir * t;
                    gone = Some(GoneReason::Absorbed);
                    break;
                }
                let absorbed = before - p.damage;
                let (h, k, part) = match hit {
                    None => {
                        p.pos += dir * remaining;
                        break;
                    }
                    Some(Cast::Rock(h)) => {
                        self.rock_hit(p.owner, p.family, &h);
                        p.pos = h.point;
                        if let Some(m) = p.missile {
                            self.warhead(p.owner, p.team, p.id, h.point, p.damage, m.radius);
                        }
                        gone = Some(GoneReason::Hit);
                        break;
                    }
                    Some(Cast::Part(PartRayHit { hit: h, k, part })) => (h, k, part),
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
                match p.missile {
                    Some(m) => {
                        let at = h.point + dir * super::missile::tuning::FUSE_DEPTH;
                        self.warhead(p.owner, p.team, p.id, at, p.damage, m.radius);
                    }
                    None => self.penetrate(
                        p.owner, p.family, &h, k, part, dir, p.damage, absorbed, p.pierce,
                    ),
                }
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

    /// Every gun fires at once: all shots are traced against the state at the start of the pass (which guns
    /// fire, where they point, which parts are alive), then their hits are applied in firing order (ship, piece,
    /// gun). So a ship never silences an enemy gun before it fires in the same tick: fire within a tick is
    /// mutual, whatever the ship ids. A hit whose part an earlier hit of the pass destroyed goes on along its
    /// shot to the part behind (as `cast_part` passes destroyed parts; no second shield pass).
    fn fire_guns(&mut self) {
        self.gun_hits.clear();
        for k in 0..self.ships.len() {
            let owner = self.ships[k].id();
            for pi in 0..self.ships[k].pieces.len() {
                for gi in 0..self.ships[k].pieces[pi].guns.len() {
                    self.fire_gun(k, owner, pi, gi);
                }
            }
        }
        // Missiles point defence shot down are gone (ids stay ascending).
        self.projectiles.retain(|p| p.life > 0.0);
        let hits = std::mem::take(&mut self.gun_hits);
        for g in &hits {
            let mut h = g.hit;
            if !self.ships[h.k].parts[h.part as usize].alive {
                match self.shot_cast(g.by, g.team, g.from, g.dir, g.range) {
                    Some(Cast::Part(next)) => h = next,
                    Some(Cast::Rock(r)) => {
                        self.rock_hit(g.by, g.family, &r);
                        continue;
                    }
                    None => continue,
                }
            }
            if let Some(spec) = g.lance {
                self.lance(g.by, g.family, &h, g.dir, g.damage, g.absorbed, spec);
                continue;
            }
            self.damage_part(
                g.by,
                g.family,
                h.k,
                h.part,
                h.hit.point,
                h.hit.normal,
                g.damage,
                g.absorbed,
            );
            if g.disrupt > 0.0 {
                self.disrupt(g.by, h.k, h.part, g.damage * g.disrupt);
            }
        }
        self.gun_hits = hits;
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
                let end = self.hitscan(owner, team, g.family, from, dir, g.range, g.output * DT, g.disrupt, None);
                // A mining beam cuts the rock it rests on (GDR-0114), as hard as the gun works.
                if let Some(rock) = end.rock.filter(|_| g.cut > 0.0) {
                    self.mine(k, rock, g.cut * work * DT, end.to, dir);
                }
                self.beams.push(BeamTrace {
                    ship: owner,
                    piece: pid,
                    gun: g.part,
                    from,
                    to: end.to,
                    hit: end.hit,
                });
            }
            Delivery::Pulse { .. } | Delivery::Projectile { .. } | Delivery::Missile { .. } => {
                if g.rate <= 0.0 {
                    return;
                }
                self.ships[k].gun_cooldown[slot] -= DT * work;
                let mut shots = 0;
                while self.ships[k].gun_cooldown[slot] <= 0.0 && shots < tuning::MAX_SHOTS_PER_TICK {
                    shots += 1;
                    self.ships[k].gun_cooldown[slot] += 1.0 / g.rate;
                    match g.delivery {
                        // Point defence (`missile.rs`): at a missile, else a small vessel; no target, it holds.
                        Delivery::Pulse { .. } if g.defence.is_some() => {
                            use super::missile::DefenceShot;
                            let to = match self.defence_target(k, &g, from, dir) {
                                Some(DefenceShot::Missile(i)) => self.shoot_missile(i, g.shot_damage),
                                Some(DefenceShot::Vessel(at)) => {
                                    let off = at - from;
                                    let aim = off * (1.0 / off.length().max(1e-6));
                                    let shot = g.shot_damage;
                                    self.hitscan(owner, team, g.family, from, aim, g.range, shot, 0.0, None).to
                                }
                                None => {
                                    self.ships[k].gun_cooldown[slot] = 0.0;
                                    break;
                                }
                            };
                            self.events.push(SimEvent::ShotFired {
                                ship: owner,
                                piece: pid,
                                gun: g.part,
                                kind: ShotKind::Pulse,
                                from,
                                to,
                                projectile: None,
                            });
                        }
                        Delivery::Missile { salvo, speed, accel, turn, life, hp, radius, .. } => {
                            let launch = super::missile::Launch { salvo, speed, accel, turn, life, hp, radius };
                            let (family, damage) = (g.family, g.shot_damage);
                            if !self.launch_missiles(k, pid, g.part, family, from, dir, v_muzzle, damage, launch) {
                                self.ships[k].gun_cooldown[slot] = 0.0;
                                break;
                            }
                        }
                        Delivery::Pulse { .. } => {
                            let end = self.hitscan(
                                owner,
                                team,
                                g.family,
                                from,
                                dir,
                                g.range,
                                g.shot_damage,
                                g.disrupt,
                                g.lance,
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
                            let crossed = self.crossed_at(owner, team, g.family, from);
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
                                crossed,
                                missile: None,
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
    use crate::ship::design::ShipDesign;
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
                    group: None,
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
            assists: None,
            anchored: false,
            origin: None,
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
