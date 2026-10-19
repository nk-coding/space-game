//! Missiles and point defence (GDR-0115; `docs/sim.md` "Weapons").
//!
//! * **Missiles** are projectiles with a seeker ([`Seeker`]): a launcher (`Delivery::Missile`) fires `salvo` of
//!   them per shot, fanned out over [`tuning::SALVO_FAN`] with a little jitter from the `Weapons` stream. Each
//!   leaves with the shooter's velocity plus [`tuning::LAUNCH_SPEED`] of its top speed, accelerates to its top
//!   speed and turns its course at up to its turn rate towards the lead point of its target: the nearest hostile
//!   vessel within [`tuning::SEEK_CONE`] of its course and within what it can still fly, picked at launch and
//!   again whenever the target is gone (no longer a hostile vessel). It flies like a projectile (ray-cast per
//!   tick, no friendly fire, shield bubbles take their `Missile` share: the base emitter absorbs it whole) and
//!   ends after its `life`.
//! * **Warhead**: where a missile strikes a hull or a rock it blasts `damage` within `radius` m of the point by the
//!   chain-reaction rule (GDR-0085: every alive part with a cell in reach, times its `blast_factor` and that of
//!   the armour of its piece on the line), on every piece of every ship except the shooter's own and its
//!   owner's other vessels (an aimed weapon, like every shot, GDR-0048). Bubbles the blast crosses (they cover a
//!   target cell and not the blast's centre) absorb first. Parts it destroys explode at breakage resolution as
//!   usual. A missile that runs out of life or is shot down does not go off.
//! * **Point defence** (`WeaponSpec::defence`): a pulse gun that aims itself within its arc and fires whenever
//!   its piece is active, each shot at the nearest hostile live missile within its reach (its hit points go down
//!   by the shot's damage; at 0 it is shot down), else at the nearest hostile vessel of at most `ship_mass`
//!   (a hit-scan pulse at its centre of mass, as any pulse). No target: it holds, ready.
//! * **Cap**: at most [`tuning::MAX_MISSILES`] missiles live in the world; a launcher waits, ready, while it is
//!   full.
//!
//! Deterministic: software trig (`math`), ship and projectile order, the `Weapons` stream for jitter.

use data::WeaponFamily;

use crate::DT;
use crate::events::SimEvent;
use crate::flight::{TargetState, lead_point};
use crate::hash::{StateHash, StateHasher};
use crate::ids::{PieceId, ShipId};
use crate::math::{Rot, Vec2, atan2, sqrt};
use crate::rng::Stream;
use crate::ship::GunMount;
use crate::ship::design::PartIdx;
use crate::ship::vessel::{Owner, friendly};
use crate::world::Sim;

use super::weapons::{GoneReason, Projectile, ProjectileId};

/// Missile tuning. **PLACEHOLDERS** (lead defaults, GDR-0115).
pub mod tuning {
    /// Live missiles in the whole world at most; launchers wait while it is full.
    pub const MAX_MISSILES: usize = 96;
    /// A seeker sees hostile vessels within this angle of the missile's course (radians, to either side; 60°).
    pub const SEEK_CONE: f32 = 1.047_197_6;
    /// A missile leaves the launcher at this share of its top speed, on top of the shooter's velocity.
    pub const LAUNCH_SPEED: f32 = 0.35;
    /// A salvo fans out over this angle (radians, to either side of the launcher's facing).
    pub const SALVO_FAN: f32 = 0.14;
    /// Random turn of each missile at launch (radians, to either side; the `Weapons` stream).
    pub const LAUNCH_JITTER: f32 = 0.03;
    /// Behaviors and auto-fire launch when the target is within this angle of the launcher's facing (radians,
    /// to either side; the seeker does the rest). Guns fire within `behavior::tuning::FIRE_CONE`.
    pub const FIRE_CONE: f32 = 0.25;
    /// The warhead goes off this far inside the hull it struck (m), so the struck cell is the blast's.
    pub const FUSE_DEPTH: f32 = 0.05;
}

/// What makes a projectile a missile (GDR-0115).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seeker {
    /// The vessel it homes on.
    pub target: Option<ShipId>,
    /// Top speed (m/s) and acceleration (m/s²).
    pub speed: f32,
    pub accel: f32,
    /// Turn rate (rad/s).
    pub turn: f32,
    /// Hit points left (point defence).
    pub hp: f32,
    /// Warhead blast radius (m); its damage is the projectile's.
    pub radius: f32,
}

impl StateHash for Seeker {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.target.map_or(u32::MAX, |s| s.0));
        h.f32(self.speed);
        h.f32(self.accel);
        h.f32(self.turn);
        h.f32(self.hp);
        h.f32(self.radius);
    }
}

/// A missile launcher's numbers (`Delivery::Missile`), for [`Sim::launch_missiles`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Launch {
    pub salvo: u32,
    pub speed: f32,
    pub accel: f32,
    /// Degrees per second, as in content.
    pub turn: f32,
    pub life: f32,
    pub hp: f32,
    pub radius: f32,
}

/// Where a point-defence shot goes: at a missile, or a pulse at a small vessel's centre of mass.
pub(crate) enum DefenceShot {
    Missile(usize),
    Vessel(Vec2),
}

impl Sim {
    /// Counts the live missiles homing on each ship (by index) for the behaviors' `missiles_incoming`.
    pub(crate) fn count_incoming(&mut self) {
        let incoming = &mut self.behavior_scratch.incoming;
        incoming.clear();
        incoming.resize(self.ships.len(), 0);
        for p in &self.projectiles {
            if let Some(t) = p.missile.and_then(|m| m.target)
                && let Ok(k) = self.ships.binary_search_by_key(&t, |s| s.id())
            {
                incoming[k] += 1;
            }
        }
    }

    /// The live missiles.
    fn missiles_live(&self) -> usize {
        self.projectiles.iter().filter(|p| p.missile.is_some() && p.life > 0.0).count()
    }

    /// The centre of mass and velocity of ship `id`'s vessel if it is a vessel hostile to `team` (not `owner`).
    fn hostile_vessel(&self, id: ShipId, owner: ShipId, team: Option<Owner>) -> Option<TargetState> {
        let k = self.ship_index(id)?;
        let s = &self.ships[k];
        if s.id() == owner || friendly(team, s.team()) {
            return None;
        }
        let p = s.piece(s.control_piece()?)?;
        Some(TargetState { pos: p.com_world(), vel: p.body.linvel })
    }

    /// The nearest hostile vessel (to the shooter `owner` of `team`) within [`tuning::SEEK_CONE`] of `dir` (unit)
    /// seen from `pos` and within `reach` m; ties go to the lower ship index.
    fn seek(&self, pos: Vec2, dir: Vec2, owner: ShipId, team: Option<Owner>, reach: f32) -> Option<ShipId> {
        let cone = crate::math::cos(tuning::SEEK_CONE);
        let mut best: Option<(f32, ShipId)> = None;
        for s in &self.ships {
            let Some(t) = self.hostile_vessel(s.id(), owner, team) else {
                continue;
            };
            let to = t.pos - pos;
            let d2 = to.length_squared();
            if d2 > reach * reach || best.is_some_and(|(b, _)| d2 >= b) {
                continue;
            }
            if dir.dot(to) >= cone * sqrt(d2) {
                best = Some((d2, s.id()));
            }
        }
        best.map(|b| b.1)
    }

    /// Ship `k` fires a salvo from `from` along `dir` (unit, world) with the muzzle's velocity `v_muzzle`: `salvo`
    /// missiles of `damage` each. Returns false (nothing fired) while the world holds [`tuning::MAX_MISSILES`].
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn launch_missiles(
        &mut self,
        k: usize,
        piece: PieceId,
        gun: PartIdx,
        family: WeaponFamily,
        from: Vec2,
        dir: Vec2,
        v_muzzle: Vec2,
        damage: f32,
        l: Launch,
    ) -> bool {
        let live = self.missiles_live();
        if live >= tuning::MAX_MISSILES {
            return false;
        }
        let owner = self.ships[k].id();
        let team = self.ships[k].team();
        let n = l.salvo.max(1).min((tuning::MAX_MISSILES - live) as u32);
        let reach = l.speed * l.life;
        let crossed = self.crossed_at(owner, team, family, from);
        for i in 0..n {
            let fan = if n > 1 {
                tuning::SALVO_FAN * (2.0 * i as f32 / (n - 1) as f32 - 1.0)
            } else {
                0.0
            };
            let jitter = self
                .rng(Stream::Weapons)
                .range_f32(-tuning::LAUNCH_JITTER, tuning::LAUNCH_JITTER);
            let d = Rot::from_angle(fan + jitter).apply(dir);
            let vel = v_muzzle + d * (l.speed * tuning::LAUNCH_SPEED);
            let id = self.ids.projectile();
            let target = self.seek(from, d, owner, team, reach);
            self.projectiles.push(Projectile {
                id,
                owner,
                team,
                family,
                pos: from,
                prev_pos: from,
                vel,
                damage,
                pierce: 1,
                life: l.life,
                ricochets: 0,
                crossed,
                missile: Some(Seeker {
                    target,
                    speed: l.speed,
                    accel: l.accel,
                    turn: l.turn * (core::f32::consts::PI / 180.0),
                    hp: l.hp,
                    radius: l.radius,
                }),
            });
            self.events.push(SimEvent::ShotFired {
                ship: owner,
                piece,
                gun,
                kind: super::weapons::ShotKind::Projectile,
                from,
                to: from + d * reach,
                projectile: Some(id),
            });
            self.events.push(SimEvent::ProjectileSpawned { id, owner, pos: from, vel });
        }
        true
    }

    /// A missile's course for this tick: its target kept while it is a hostile vessel, else the seeker looks
    /// again; the course turns towards the target's lead point by at most the turn rate, and the speed grows by
    /// the acceleration up to the top speed (a missile launched faster keeps its speed).
    pub(crate) fn steer_missile(&self, p: &mut Projectile) {
        let Some(mut s) = p.missile else {
            return;
        };
        let mut speed = p.vel.length();
        let mut dir = if speed > 1e-6 { p.vel * (1.0 / speed) } else { Vec2::Y };
        let mut aim = s.target.and_then(|t| self.hostile_vessel(t, p.owner, p.team));
        if aim.is_none() {
            let reach = s.speed.max(speed) * p.life;
            s.target = self.seek(p.pos, dir, p.owner, p.team, reach);
            aim = s.target.and_then(|t| self.hostile_vessel(t, p.owner, p.team));
        }
        if let Some(t) = aim {
            let to = lead_point(p.pos, Vec2::ZERO, t.pos, t.vel, speed.max(0.5 * s.speed)) - p.pos;
            let err = atan2(dir.cross(to), dir.dot(to));
            let step = s.turn * DT;
            dir = Rot::from_angle(err.clamp(-step, step)).apply(dir);
        }
        if speed < s.speed {
            speed = (speed + s.accel * DT).min(s.speed);
        }
        p.vel = dir * speed;
        p.missile = Some(s);
    }

    /// Missile `id` of ship `by` (of `team`) strikes at `at` (module docs: the warhead): `damage` within `radius`.
    pub(crate) fn warhead(
        &mut self,
        by: ShipId,
        team: Option<Owner>,
        id: ProjectileId,
        at: Vec2,
        damage: f32,
        radius: f32,
    ) {
        let mut sc = std::mem::take(&mut self.chain_scratch);
        sc.hits.clear();
        for (j, s) in self.ships.iter().enumerate() {
            if s.id() == by {
                continue;
            }
            for q in &s.pieces {
                if !friendly(team, s.piece_team(q.id)) {
                    super::chain::piece_hits(s, j, q, at, damage, radius, &mut sc);
                }
            }
        }
        for n in 0..sc.hits.len() {
            let h = sc.hits[n];
            let left = self.blast_shields(by, team, at, h.point, h.damage);
            if left > 0.0 {
                let off = h.point - at;
                let len = off.length();
                let normal = if len > 1e-6 { off * (-1.0 / len) } else { Vec2::ZERO };
                self.damage_part(by, WeaponFamily::Missile, h.k as usize, h.part, h.point, normal, left, h.damage - left);
            }
        }
        self.chain_scratch = sc;
        self.events.push(SimEvent::Warhead { by, id, at, radius, damage });
    }

    /// The target of a point-defence gun `g` of ship `owner` (of `team`) firing from `from` along its facing
    /// `dir` (unit, world): the nearest hostile live missile within its reach and arc (ties: the lower id), else
    /// the nearest hostile vessel of at most its `ship_mass` whose centre of mass is (ties: the lower ship index).
    pub(crate) fn defence_target(
        &self,
        owner: ShipId,
        team: Option<Owner>,
        g: &GunMount,
        from: Vec2,
        dir: Vec2,
    ) -> Option<DefenceShot> {
        let d = g.defence?;
        let within = |to: Vec2, d2: f32| d2 <= g.range * g.range && dir.dot(to) >= d.arc_cos * sqrt(d2);
        let mut best: Option<(f32, usize)> = None;
        for (i, p) in self.projectiles.iter().enumerate() {
            if p.missile.is_none() || p.life <= 0.0 || p.owner == owner || friendly(team, p.team) {
                continue;
            }
            let to = p.pos - from;
            let d2 = to.length_squared();
            if within(to, d2) && best.is_none_or(|(b, _)| d2 < b) {
                best = Some((d2, i));
            }
        }
        if let Some((_, i)) = best {
            return Some(DefenceShot::Missile(i));
        }
        if d.ship_mass <= 0.0 {
            return None;
        }
        let mut near: Option<(f32, Vec2)> = None;
        for s in &self.ships {
            let Some(c) = s.control_piece().and_then(|c| s.piece(c)) else {
                continue;
            };
            if c.mass.mass > d.ship_mass || self.hostile_vessel(s.id(), owner, team).is_none() {
                continue;
            }
            let at = c.com_world();
            let to = at - from;
            let d2 = to.length_squared();
            if within(to, d2) && near.is_none_or(|(b, _)| d2 < b) {
                near = Some((d2, at));
            }
        }
        near.map(|(_, at)| DefenceShot::Vessel(at))
    }

    /// A point-defence shot of `damage` from `from` takes hit points off missile `i` (the index in the live
    /// list); at none left it is shot down (gone at once). Returns where the shot ended.
    pub(crate) fn shoot_missile(&mut self, i: usize, damage: f32) -> Vec2 {
        let p = &mut self.projectiles[i];
        let Some(s) = p.missile.as_mut() else {
            return p.pos;
        };
        s.hp -= damage;
        if s.hp <= 0.0 {
            p.life = -1.0;
            let (id, pos) = (p.id, p.pos);
            self.events.push(SimEvent::ProjectileGone { id, pos, reason: GoneReason::ShotDown });
        }
        self.projectiles[i].pos
    }
}
