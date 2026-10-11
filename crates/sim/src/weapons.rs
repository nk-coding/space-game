//! Weapons v1 (every hit test is a ray cast through the physics world, `Physics::raycast`):
//!
//! * **Beams** (continuous hit-scan): `dps × work × DT` per tick on the first block in range.
//! * **Pulses** (discrete hit-scan): a cooldown per gun; each pulse deals `damage / rate` to the first block.
//! * **Projectiles** (kinetic): world entities with the shooter's velocity, a lifetime of `range / speed`; each
//!   tick the segment they travel is ray-cast. On a hit they damage the block and, if it breaks, carry the
//!   excess through up to `penetration` blocks of that piece (grid walk). Recoil pushes the shooter. Shallow
//!   hits on a slope face (more than 60° from its normal) ricochet with less speed and damage.
//! * **Shields**: every working emitter projects a bubble (`shield_radius`) around itself. A shot that enters a
//!   bubble of another ship loses `damage × absorb[family]` to it (up to what the bubble has left); bubbles
//!   along the path are crossed in order, so overlapping bubbles stack. A shot starting inside a bubble passes.
//!
//! Order inside a tick (after the ship systems, before physics): bubbles are collected, live projectiles fly,
//! then every gun fires; breakage is resolved afterwards. All loops run in id/index order.
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
use crate::physics::RayHit;
use crate::ship::Ship;
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
    /// Grid cells a penetrating shot may walk through after its first hit.
    pub const MAX_WALK: usize = 64;
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
    pub owner: ShipId,
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
}

impl StateHash for Projectile {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.id.0);
        h.u32(self.owner.0);
        h.u32(family_index(self.family) as u32);
        h.vec2(self.pos);
        h.vec2(self.vel);
        h.f32(self.damage);
        h.u32(self.pierce);
        h.f32(self.life);
        h.u8(self.ricochets);
    }
}

/// Where a shot went.
struct ShotEnd {
    to: Vec2,
    hit: bool,
}

impl Sim {
    /// Weapons for one tick: bubbles, projectile flight, gun fire. Called by `step` after the systems.
    pub(crate) fn weapons_step(&mut self) {
        self.beams.clear();
        self.collect_bubbles();
        self.advance_projectiles();
        self.fire_guns();
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
                    });
                }
            }
        }
    }

    /// Bubbles of other ships the segment `from + t·dir, t ∈ [0, len]` enters take their share of `damage`, in
    /// order along the path. Returns the distance where the shot was fully absorbed, if it was.
    fn shield_pass(
        &mut self,
        owner: ShipId,
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
            if bub.ship == owner || bub.left <= 0.0 || bub.absorb[fi] <= 0.0 {
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
            let bub = &mut self.bubbles[b as usize];
            let take = (*damage * bub.absorb[fi]).min(bub.left);
            if take <= 0.0 {
                continue;
            }
            bub.left -= take;
            if bub.left < 1e-6 {
                bub.left = 0.0;
            }
            *damage -= take;
            let (ship_index, part, left) = (bub.ship_index, bub.part, bub.left);
            let ps = &mut self.ships[ship_index].parts[part as usize];
            ps.shield = left;
            if left <= 0.0 {
                ps.shield_down = true;
            }
            let bub = self.bubbles[b as usize];
            self.events.push(SimEvent::ShieldHit {
                by: owner,
                ship: bub.ship,
                piece: bub.piece,
                part,
                point: from + dir * t,
                absorbed: take,
                left,
                depleted: left <= 0.0,
            });
            if *damage <= 1e-6 {
                *damage = 0.0;
                return Some(t);
            }
        }
        None
    }

    /// The part under a ray hit and its ship index.
    fn hit_target(&self, h: &RayHit) -> Option<(usize, PartIdx)> {
        let k = self
            .ships
            .binary_search_by_key(&h.key.ship, |s| s.id())
            .ok()?;
        let ship = &self.ships[k];
        let piece = ship.piece(h.key.piece)?;
        let n_local = Rot::from_angle(piece.body.angle).apply_inverse(h.normal);
        let part = ship.part_at_local(h.key.piece, h.local_point - n_local * 1e-3)?;
        Some((k, part))
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

    /// A hit-scan shot (beam tick or pulse): shields along the way, then the first block only.
    fn hitscan(
        &mut self,
        owner: ShipId,
        family: WeaponFamily,
        from: Vec2,
        dir: Vec2,
        range: f32,
        damage: f32,
    ) -> ShotEnd {
        let hit = self.physics.raycast(from, dir, range, Some(owner));
        let len = hit.map_or(range, |h| h.distance);
        let mut left = damage;
        if let Some(t) = self.shield_pass(owner, family, from, dir, len, &mut left) {
            return ShotEnd {
                to: from + dir * t,
                hit: false,
            };
        }
        match hit {
            Some(h) => {
                if let Some((k, part)) = self.hit_target(&h) {
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
                }
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
        let dl = rot.apply_inverse(dir);
        let (dx, dy) = (dl.x, -dl.y);
        let (mut x, mut y) = (origin[0] + h.local_point.x, origin[1] - h.local_point.y);
        // Start just inside the first cell.
        x += dx * 1e-3;
        y += dy * 1e-3;
        let (mut cx, mut cy) = (x.floor() as i32, y.floor() as i32);
        let (sx, sy) = (if dx > 0.0 { 1 } else { -1 }, if dy > 0.0 { 1 } else { -1 });
        let inv = |d: f32| {
            if d.abs() > 1e-9 {
                1.0 / d.abs()
            } else {
                f32::INFINITY
            }
        };
        let (tdx, tdy) = (inv(dx), inv(dy));
        let next = |c: i32, p: f32, d: f32| {
            if d > 0.0 {
                (c as f32 + 1.0 - p) * inv(d)
            } else if d < 0.0 {
                (p - c as f32) * inv(d)
            } else {
                f32::INFINITY
            }
        };
        let (mut tx, mut ty) = (next(cx, x, dx), next(cy, y, dy));
        let mut last = first;
        let mut travelled: f32;
        for _ in 0..tuning::MAX_WALK {
            let carry = match out {
                DamageOutcome::Destroyed { excess } if excess > 0.0 && pierce > 0 => excess,
                _ => return,
            };
            // Next cell.
            if tx < ty {
                cx += sx;
                travelled = tx;
                tx += tdx;
            } else {
                cy += sy;
                travelled = ty;
                ty += tdy;
            }
            let ship = &self.ships[k];
            let Some(o) = ship.grid().occupant((cx, cy)) else {
                // Left the hull: the shot stops (it does not fly on to other ships).
                return;
            };
            let o = o as PartIdx;
            if o == last || !ship.part(o).alive || ship.part(o).piece != pid {
                continue;
            }
            last = o;
            pierce -= 1;
            let local = Vec2::new(
                x + dx * travelled - origin[0],
                origin[1] - (y + dy * travelled),
            );
            let point = h.point + rot.apply(local - h.local_point);
            out = self.damage_part(owner, family, k, o, point, -dir, carry, 0.0);
        }
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
                let hit = self.physics.raycast(p.pos, dir, remaining, Some(p.owner));
                let len = hit.map_or(remaining, |h| h.distance);
                let before = p.damage;
                if let Some(t) = self.shield_pass(p.owner, p.family, p.pos, dir, len, &mut p.damage)
                {
                    p.pos += dir * t;
                    gone = Some(GoneReason::Absorbed);
                    break;
                }
                let absorbed = before - p.damage;
                let Some(h) = hit else {
                    p.pos += dir * remaining;
                    break;
                };
                let Some((k, part)) = self.hit_target(&h) else {
                    p.pos = h.point;
                    gone = Some(GoneReason::Hit);
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
                let end = self.hitscan(owner, g.family, from, dir, g.range, g.output * DT);
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
                while self.ships[k].gun_cooldown[slot] <= 0.0 {
                    self.ships[k].gun_cooldown[slot] += 1.0 / g.rate;
                    match g.delivery {
                        Delivery::Pulse { .. } => {
                            let end =
                                self.hitscan(owner, g.family, from, dir, g.range, g.shot_damage);
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
