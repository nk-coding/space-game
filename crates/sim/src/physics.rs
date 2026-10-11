//! The physics seam: a small trait the simulation drives, shaped around what the sim needs (bodies per piece
//! with explicit mass properties and convex colliders, wrenches, a fixed step, pose read-back, ray casts and
//! contact reports). The chosen engine (Rapier2D, box2d-rust or Avian) implements it later; [`FreeFlight`]
//! is the engine-free stand-in used until then and by tests and headless tools.
//!
//! Conventions: world frame y up, angles counter-clockwise; a body's origin is its piece-frame origin
//! (`Body::pos`), its linear velocity is the velocity of its centre of mass (`MassProps::com`).

use crate::collider::ConvexPolygon;
use crate::hash::StateHasher;
use crate::ids::{PieceId, ShipId};
use crate::mass::MassProps;
use crate::math::{Rot, Vec2};
use crate::ship::Body;

/// One body = one piece of one ship. Orders by ship, then piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyKey {
    pub ship: ShipId,
    pub piece: PieceId,
}

/// What a backend needs to build or rebuild a body. Mass properties are explicit (the sim is the authority;
/// never derive them from collider density).
#[derive(Clone, Copy, Debug)]
pub struct BodyDesc<'a> {
    pub body: Body,
    pub mass: MassProps,
    /// Convex, counter-clockwise, in the piece frame.
    pub colliders: &'a [ConvexPolygon],
}

/// First hit of a ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub key: BodyKey,
    /// Distance along the (unit) ray.
    pub distance: f32,
    /// Hit point, world frame.
    pub point: Vec2,
    /// Hit point in the piece frame.
    pub local_point: Vec2,
    /// Surface normal at the hit, world frame, pointing out of the body.
    pub normal: Vec2,
}

/// A contact that started during the last step (for ramming damage later).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub a: BodyKey,
    pub b: BodyKey,
    /// World contact point.
    pub point: Vec2,
    /// World normal from `a` to `b`.
    pub normal: Vec2,
    /// Normal impulse of the contact (kN·s).
    pub impulse: f32,
    /// Closing speed along the normal when it started (m/s).
    pub relative_speed: f32,
}

/// The physics engine as the simulation sees it. All calls come from `Sim` in a fixed order, so a
/// deterministic engine gives deterministic results.
pub trait PhysicsBackend {
    /// Adds a body (after `spawn_ship`, and for new pieces of a split).
    fn create_body(&mut self, key: BodyKey, desc: &BodyDesc);
    /// Replaces colliders and mass properties of an existing body and sets its velocities from `desc.body`
    /// (its centre of mass moved). The pose stays.
    fn update_body(&mut self, key: BodyKey, desc: &BodyDesc);
    fn remove_body(&mut self, key: BodyKey);
    /// Bodies of the same ship that are both ghosts do not collide with each other (set on all pieces of a
    /// split for a few ticks, so freshly separated pieces never push each other apart).
    fn set_ghost(&mut self, key: BodyKey, ghost: bool);
    /// Moves a body (tools, tests, respawn). Velocities are set too.
    fn set_body(&mut self, key: BodyKey, body: Body);
    /// Force (world frame, at the centre of mass) and torque for the next step. Cleared by `step`.
    fn apply_wrench(&mut self, key: BodyKey, force: Vec2, torque: f32);
    /// Advances by `dt` seconds.
    fn step(&mut self, dt: f32);
    /// Pose and velocities after the last step.
    fn body(&self, key: BodyKey) -> Option<Body>;
    /// First hit along a ray from `origin` in unit direction `dir`, up to `max_distance`, ignoring the
    /// bodies of `exclude`. Ties go to the lower key.
    fn raycast(
        &self,
        origin: Vec2,
        dir: Vec2,
        max_distance: f32,
        exclude: Option<ShipId>,
    ) -> Option<RayHit>;
    /// Contacts that started during the last step.
    fn contacts(&self) -> &[Contact];
    /// Feeds engine-internal state into the state hash (optional; poses are hashed by the sim anyway).
    fn hash_state(&self, _h: &mut StateHasher) {}
}

/// Keeps angles in `(−π, π]`. No loops: a non-finite angle (a bug) maps to 0 instead of hanging.
pub fn wrap_angle(a: f32) -> f32 {
    use core::f32::consts::{PI, TAU};
    if a > -PI && a <= PI {
        return a;
    }
    if !a.is_finite() {
        return 0.0;
    }
    let r = a - TAU * ((a + PI) / TAU).floor();
    if r <= -PI {
        r + TAU
    } else if r > PI {
        r - TAU
    } else {
        r
    }
}

#[derive(Clone, Debug)]
struct FfBody {
    key: BodyKey,
    body: Body,
    inv_mass: f32,
    inv_inertia: f32,
    com: Vec2,
    /// Bounding radius around the centre of mass.
    radius: f32,
    colliders: Vec<ConvexPolygon>,
    force: Vec2,
    torque: f32,
    ghost: bool,
}

impl FfBody {
    fn set_shape(&mut self, desc: &BodyDesc) {
        let m = desc.mass;
        self.inv_mass = if m.mass > 0.0 { 1.0 / m.mass } else { 0.0 };
        self.inv_inertia = if m.inertia > 0.0 {
            1.0 / m.inertia
        } else {
            0.0
        };
        self.com = m.com;
        self.colliders.clear();
        self.colliders.extend_from_slice(desc.colliders);
        let mut r2 = 0.0f32;
        for c in &self.colliders {
            for &p in c.points() {
                let d = (p - m.com).length_squared();
                if d > r2 {
                    r2 = d;
                }
            }
        }
        self.radius = r2.sqrt();
    }
}

/// Engine-free backend: semi-implicit Euler for every body about its centre of mass, no collisions,
/// brute-force ray casts against the convex colliders with bounding-circle culling. Deterministic.
#[derive(Clone, Debug, Default)]
pub struct FreeFlight {
    /// Sorted by key.
    bodies: Vec<FfBody>,
    contacts: Vec<Contact>,
}

impl FreeFlight {
    pub fn new() -> FreeFlight {
        FreeFlight::default()
    }

    fn index(&self, key: BodyKey) -> Result<usize, usize> {
        self.bodies.binary_search_by_key(&key, |b| b.key)
    }

    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// Is this body a ghost (tests)?
    pub fn is_ghost(&self, key: BodyKey) -> Option<bool> {
        self.index(key).ok().map(|i| self.bodies[i].ghost)
    }
}

impl PhysicsBackend for FreeFlight {
    fn create_body(&mut self, key: BodyKey, desc: &BodyDesc) {
        let mut b = FfBody {
            key,
            body: desc.body,
            inv_mass: 0.0,
            inv_inertia: 0.0,
            com: Vec2::ZERO,
            radius: 0.0,
            colliders: Vec::new(),
            force: Vec2::ZERO,
            torque: 0.0,
            ghost: false,
        };
        b.set_shape(desc);
        match self.index(key) {
            Ok(i) => self.bodies[i] = b,
            Err(i) => self.bodies.insert(i, b),
        }
    }

    fn update_body(&mut self, key: BodyKey, desc: &BodyDesc) {
        if let Ok(i) = self.index(key) {
            let b = &mut self.bodies[i];
            b.set_shape(desc);
            b.body.linvel = desc.body.linvel;
            b.body.angvel = desc.body.angvel;
        }
    }

    fn remove_body(&mut self, key: BodyKey) {
        if let Ok(i) = self.index(key) {
            self.bodies.remove(i);
        }
    }

    fn set_ghost(&mut self, key: BodyKey, ghost: bool) {
        if let Ok(i) = self.index(key) {
            self.bodies[i].ghost = ghost;
        }
    }

    fn set_body(&mut self, key: BodyKey, body: Body) {
        if let Ok(i) = self.index(key) {
            self.bodies[i].body = body;
        }
    }

    fn apply_wrench(&mut self, key: BodyKey, force: Vec2, torque: f32) {
        if let Ok(i) = self.index(key) {
            let b = &mut self.bodies[i];
            b.force += force;
            b.torque += torque;
        }
    }

    fn step(&mut self, dt: f32) {
        self.contacts.clear();
        for b in &mut self.bodies {
            let rot = Rot::from_angle(b.body.angle);
            let com = b.body.pos + rot.apply(b.com);
            b.body.linvel += b.force * (b.inv_mass * dt);
            b.body.angvel += b.torque * (b.inv_inertia * dt);
            let com = com + b.body.linvel * dt;
            b.body.angle = wrap_angle(b.body.angle + b.body.angvel * dt);
            b.body.pos = com - Rot::from_angle(b.body.angle).apply(b.com);
            b.force = Vec2::ZERO;
            b.torque = 0.0;
        }
    }

    fn body(&self, key: BodyKey) -> Option<Body> {
        self.index(key).ok().map(|i| self.bodies[i].body)
    }

    fn raycast(
        &self,
        origin: Vec2,
        dir: Vec2,
        max_distance: f32,
        exclude: Option<ShipId>,
    ) -> Option<RayHit> {
        let mut best: Option<RayHit> = None;
        let mut best_t = max_distance;
        for b in &self.bodies {
            if Some(b.key.ship) == exclude {
                continue;
            }
            let rot = Rot::from_angle(b.body.angle);
            // Bounding circle around the centre of mass.
            let c = b.body.pos + rot.apply(b.com);
            let oc = c - origin;
            let along = oc.dot(dir);
            let perp2 = oc.length_squared() - along * along;
            let r2 = b.radius * b.radius;
            if perp2 > r2 || along + b.radius < 0.0 || along - b.radius > best_t {
                continue;
            }
            // Ray in the piece frame.
            let o = rot.apply_inverse(origin - b.body.pos);
            let d = rot.apply_inverse(dir);
            for poly in &b.colliders {
                if let Some((t, n)) = ray_convex(o, d, poly.points(), best_t)
                    && t < best_t
                {
                    best_t = t;
                    let local = o + d * t;
                    let nw = rot.apply(n);
                    best = Some(RayHit {
                        key: b.key,
                        distance: t,
                        point: origin + dir * t,
                        local_point: local,
                        normal: nw * (1.0 / nw.length()),
                    });
                }
            }
        }
        best
    }

    fn contacts(&self) -> &[Contact] {
        &self.contacts
    }
}

/// Ray (origin `o`, direction `d`) against a counter-clockwise convex polygon (Cyrus–Beck). Returns the entry
/// distance and the (unnormalised) outward normal of the entry edge; `None` if missed, beyond `max`, or if the
/// ray starts inside.
fn ray_convex(o: Vec2, d: Vec2, pts: &[Vec2], max: f32) -> Option<(f32, Vec2)> {
    let n = pts.len();
    let (mut t_in, mut t_out) = (0.0f32, max);
    let mut normal: Option<Vec2> = None;
    for k in 0..n {
        let (p, q) = (pts[k], pts[(k + 1) % n]);
        let e = q - p;
        let nrm = Vec2::new(e.y, -e.x); // outward for CCW
        let denom = nrm.dot(d);
        let num = nrm.dot(p - o);
        if denom == 0.0 {
            if num < 0.0 {
                return None;
            }
            continue;
        }
        let t = num / denom;
        if denom < 0.0 {
            if t > t_in {
                t_in = t;
                normal = Some(nrm);
            }
        } else if t < t_out {
            t_out = t;
        }
        if t_in > t_out {
            return None;
        }
    }
    normal.map(|nrm| (t_in, nrm))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> ConvexPolygon {
        crate::collider::ConvexPolygon::rect(Vec2::new(-1.0, -1.0), Vec2::new(1.0, 1.0))
    }

    #[test]
    fn wrap_angle_stays_in_range() {
        use core::f32::consts::PI;
        for a in [0.0f32, 3.0, -3.0, PI, -PI, 7.0, -7.0, 100.0, -1000.5] {
            let w = wrap_angle(a);
            assert!(w > -PI && w <= PI, "{a} -> {w}");
            assert!((crate::math::sin(w) - crate::math::sin(a)).abs() < 1e-3);
        }
        assert_eq!(wrap_angle(f32::INFINITY), 0.0);
    }

    #[test]
    fn free_flight_integrates_force_and_torque() {
        let mut ff = FreeFlight::new();
        let key = BodyKey {
            ship: ShipId(0),
            piece: PieceId(0),
        };
        let cols = [square()];
        let mass = MassProps {
            mass: 2.0,
            com: Vec2::ZERO,
            inertia: 4.0,
        };
        ff.create_body(
            key,
            &BodyDesc {
                body: Body::default(),
                mass,
                colliders: &cols,
            },
        );
        for _ in 0..30 {
            ff.apply_wrench(key, Vec2::new(4.0, 0.0), 0.0);
            ff.step(1.0 / 30.0);
        }
        let b = ff.body(key).unwrap();
        // a = 2 m/s² for 1 s: v = 2; semi-implicit Euler: x = a·dt²·n(n+1)/2.
        assert!((b.linvel.x - 2.0).abs() < 1e-5);
        assert!((b.pos.x - 2.0 * (1.0 / 900.0) * 465.0).abs() < 1e-4);
        ff.apply_wrench(key, Vec2::ZERO, 4.0 * 30.0);
        ff.step(1.0 / 30.0);
        assert!((ff.body(key).unwrap().angvel - 1.0).abs() < 1e-5);
    }

    #[test]
    fn raycast_hits_the_nearest_face() {
        let mut ff = FreeFlight::new();
        let cols = [square()];
        let mass = MassProps {
            mass: 1.0,
            com: Vec2::ZERO,
            inertia: 1.0,
        };
        for (k, x) in [(0u32, 10.0f32), (1, 5.0)] {
            let key = BodyKey {
                ship: ShipId(k),
                piece: PieceId(k),
            };
            ff.create_body(
                key,
                &BodyDesc {
                    body: Body::at(Vec2::new(x, 0.0), 0.3),
                    mass,
                    colliders: &cols,
                },
            );
        }
        let hit = ff.raycast(Vec2::ZERO, Vec2::X, 100.0, None).unwrap();
        assert_eq!(hit.key.ship, ShipId(1));
        assert!(hit.distance > 3.5 && hit.distance < 4.0, "{}", hit.distance);
        assert!(hit.normal.x < -0.9);
        let hit = ff
            .raycast(Vec2::ZERO, Vec2::X, 100.0, Some(ShipId(1)))
            .unwrap();
        assert_eq!(hit.key.ship, ShipId(0));
        assert!(ff.raycast(Vec2::ZERO, Vec2::Y, 100.0, None).is_none());
        assert!(ff.raycast(Vec2::ZERO, Vec2::X, 3.0, None).is_none());
    }
}
