//! Physics: Rapier2D 0.36 (ADR-0014) in its deterministic mode (`enhanced-determinism`, one thread), wrapped
//! in [`Physics`]. Only the sim's own types cross this module's public API; Rapier stays inside.
//!
//! * **One body per piece** with one collider holding a `SharedShape::compound` of the piece's convex
//!   colliders (greedy-merged rectangles as cuboids, slope triangles as triangles), in the piece frame.
//! * **The sim is the authority on mass**: colliders carry zero mass, and the body gets the piece's
//!   [`MassProps`] as explicit additional mass properties, which Rapier adds to zero bit for bit (so the
//!   centre of mass is exactly the sim's). A change rebuilds the compound (cheap) and resets the mass.
//! * Rapier keeps the frame origin when the centre of mass moves and does not correct the linear velocity;
//!   the sim hands over `v + ω × (com_new − com_old)` itself (`ship::inherited_velocity`).
//! * Zero gravity, `DT` split into `PhysicsTuning::substeps` solver substeps, placeholder friction and
//!   restitution, sleeping below a low speed, soft CCD (speculative contacts) on every body.
//! * **Ghosts**: after a split every piece of the ship carries a ghost group (bits 64..96 of its collider's
//!   user data: the split ship's id + 1) and the contact-pair filter hook; the filter drops pairs of two ghosts
//!   of the same group, so a piece that split off as a vessel of its own (a new ship) still ghosts its parent.
//!   A pure function of the two colliders, so it is deterministic.
//! * **Ray casts** go through Rapier's query pipeline (the broad-phase BVH) with a predicate that skips one
//!   ship. Bodies created or moved between steps are put into the BVH at once, so rays see them.
//! * **Contacts**: after each step every touching pair whose normal impulse reached
//!   `PhysicsTuning::impact_min_impulse` is reported ([`RawContact`], sorted by key); the sim turns them
//!   into `SimEvent::Impact` and ramming damage.
//!
//! Conventions: world frame y up, angles counter-clockwise; a body's origin is its piece-frame origin
//! (`Body::pos`), its linear velocity is the velocity of its centre of mass (`MassProps::com`).

use std::collections::BTreeMap;

use rapier2d::prelude::{
    ActiveHooks, ColliderBuilder, ColliderHandle, MassProperties, PairFilterContext, PhysicsHooks,
    PhysicsPipeline, PhysicsWorld, Pose, QueryFilter, Ray, RigidBody, RigidBodyActivation,
    RigidBodyBuilder, RigidBodyHandle, Rotation, SharedShape, SolverFlags, Vector,
};

use crate::DT;
use crate::ship::collider::ConvexPolygon;
use crate::config::PhysicsTuning;
use crate::hash::{StateHash, StateHasher};
use crate::ids::{PieceId, ShipId};
use crate::ship::mass::MassProps;
use crate::math::{Rot, Vec2, atan2};
use crate::ship::Body;

/// One body = one piece of one ship. Orders by ship, then piece. A voyage's asteroid rocks (GDR-0104) are bodies too,
/// under the ship id [`BodyKey::ROCKS`] that no ship ever gets, so they order after every ship's pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyKey {
    pub ship: ShipId,
    pub piece: PieceId,
}

impl BodyKey {
    /// The ship id of the rocks' bodies: no ship's (ids count up from 0).
    pub const ROCKS: ShipId = ShipId(u32::MAX);

    /// Rock `k` of the voyage (`Voyage::rocks`).
    pub fn rock(k: u32) -> BodyKey {
        BodyKey { ship: Self::ROCKS, piece: PieceId(k) }
    }

    /// The rock this body is, if it is one.
    pub fn as_rock(self) -> Option<u32> {
        (self.ship == Self::ROCKS).then_some(self.piece.0)
    }
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
    /// Surface normal at the hit, world frame, pointing out of the body (zero if the ray started inside).
    pub normal: Vec2,
}

/// Two pieces pushed each other during the last step (`SimEvent::Impact`). `a < b`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub a: BodyKey,
    pub b: BodyKey,
    /// World contact point on `a`'s surface: the impulse-weighted centre of all contact points.
    pub point: Vec2,
    /// World normal from `a` to `b`.
    pub normal: Vec2,
    /// Normal impulse of the whole contact during the step, all points and substeps (kN·s = t·m/s).
    pub impulse: f32,
    /// Closing speed along the normal at the contact point before the step (m/s; > 0 = approaching).
    pub relative_speed: f32,
    /// The part of `a` and of `b` under the contact point (`None` if the point and the grid disagree, and for
    /// `b` when it is an asteroid rock, GDR-0104: a piece against a rock is always `a`, the rock takes nothing).
    pub part_a: Option<crate::ship::design::PartIdx>,
    pub part_b: Option<crate::ship::design::PartIdx>,
    /// Ramming damage to each side, before shields: shield bubbles covering the point take their kinetic
    /// share first (`SimEvent::ShieldHit`), the rest hits `part_a` / `part_b` and what exceeds a part's hit
    /// points crushes on inward (GDR-0047). 0 below `PhysicsTuning::ram_threshold` and between vessels of one
    /// owner (GDR-0045).
    pub damage: f32,
}

/// What the physics needs to build or rebuild a body. Mass properties are explicit (the sim is the
/// authority; never derived from collider density).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BodyDesc<'a> {
    pub body: Body,
    pub mass: MassProps,
    /// Convex, counter-clockwise, in the piece frame.
    pub colliders: &'a [ConvexPolygon],
}

/// A touching pair after a step, before the sim resolves parts and damage. `a < b`.
///
/// The contact point is the impulse-weighted centre of all contact points of the pair: two flat faces
/// touch at the two ends of their overlap, which often sit on a neighbouring part's corner (several
/// sub-shape manifolds then share the load there), while the centre lies inside the parts that took the
/// push. The strongest manifold (one per pair of sub-shapes) is kept as a fallback for contacts whose
/// centre misses the hull (two separate touching spots).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RawContact {
    pub a: BodyKey,
    pub b: BodyKey,
    /// World centre of the contact, on `a`'s surface.
    pub point: Vec2,
    pub normal: Vec2,
    pub impulse: f32,
    /// The centre of the contact on each side, in that piece's frame.
    pub local_a: Vec2,
    pub local_b: Vec2,
    /// The centre of the strongest manifold on each side, in that piece's frame.
    pub strongest_a: Vec2,
    pub strongest_b: Vec2,
    /// The strongest manifold's sub-shapes: indices into each piece's `colliders`.
    pub sub_a: u32,
    pub sub_b: u32,
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

/// Bits 64..96 of a collider's user data: the split ghost group + 1 (0 = no ghost).
const GHOST_SHIFT: u32 = 64;
const GHOST_MASK: u128 = (u32::MAX as u128) << GHOST_SHIFT;

fn ghost_group(data: u128) -> u32 {
    (data >> GHOST_SHIFT) as u32
}

fn user_data(key: BodyKey) -> u128 {
    key.ship.0 as u128 | (key.piece.0 as u128) << 32
}

fn key_of(data: u128) -> BodyKey {
    BodyKey {
        ship: ShipId(data as u32),
        piece: PieceId((data >> 32) as u32),
    }
}

#[inline]
fn vr(p: Vec2) -> Vector {
    Vector::new(p.x, p.y)
}

#[inline]
fn vs(p: Vector) -> Vec2 {
    Vec2::new(p.x, p.y)
}

/// The body pose of a piece (rotation through `sim::math`, never std trig).
fn pose_of(body: &Body) -> Pose {
    let r = Rot::from_angle(body.angle);
    Pose::from_parts(vr(body.pos), Rotation::from_cos_sin_unchecked(r.cos, r.sin))
}

fn mass_of(m: MassProps) -> MassProperties {
    MassProperties::new(vr(m.com), m.mass, m.inertia)
}

/// The compound shape of a piece, sub-shape `k` = `colliders[k]`.
fn compound(colliders: &[ConvexPolygon]) -> SharedShape {
    let parts = colliders
        .iter()
        .map(|c| match *c.points() {
            // `ConvexPolygon::rect`: [min, (max.x, min.y), max, (min.x, max.y)].
            [min, _, max, _] => {
                let (c, h) = ((min + max) * 0.5, (max - min) * 0.5);
                (Pose::translation(c.x, c.y), SharedShape::cuboid(h.x, h.y))
            }
            [a, b, c] => (Pose::IDENTITY, SharedShape::triangle(vr(a), vr(b), vr(c))),
            _ => unreachable!("colliders are triangles or rectangles"),
        })
        .collect();
    SharedShape::compound(parts)
}

fn configure_body(rb: &mut RigidBody, t: &PhysicsTuning) {
    rb.set_soft_ccd_prediction(t.ccd_prediction);
    let a = rb.activation_mut();
    if t.sleep_speed > 0.0 {
        a.normalized_linear_threshold = t.sleep_speed;
        a.angular_threshold = RigidBodyActivation::default_angular_threshold();
        a.time_until_sleep = t.sleep_time;
    } else {
        a.normalized_linear_threshold = -1.0;
        a.angular_threshold = -1.0;
    }
}

/// Drops contacts between two ghosts of the same group (the pieces of one split).
struct GhostFilter;

impl PhysicsHooks for GhostFilter {
    fn filter_contact_pair(&self, c: &PairFilterContext) -> Option<SolverFlags> {
        let (a, b) = (
            ghost_group(c.colliders[c.collider1].user_data),
            ghost_group(c.colliders[c.collider2].user_data),
        );
        if a != 0 && a == b {
            None
        } else {
            Some(SolverFlags::COMPUTE_RIGID_IMPULSES)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Handles {
    body: RigidBodyHandle,
    collider: ColliderHandle,
}

/// The physics world: one Rapier body per piece. `Sim` makes every call in a fixed order, so the
/// deterministic engine gives the same result on every platform.
pub struct Physics {
    world: PhysicsWorld,
    handles: BTreeMap<BodyKey, Handles>,
    tuning: PhysicsTuning,
    /// Touching pairs of the last step (sorted by key).
    pub(crate) contacts: Vec<RawContact>,
}

impl Physics {
    pub(crate) fn new(tuning: PhysicsTuning) -> Physics {
        let mut world = PhysicsWorld::new();
        world.gravity = Vector::ZERO;
        world.integration_parameters.dt = DT;
        world.integration_parameters.num_solver_iterations = tuning.substeps.max(1) as usize;
        Physics {
            world,
            handles: BTreeMap::new(),
            tuning,
            contacts: Vec::new(),
        }
    }

    /// Applies new tuning to the world and every body and collider.
    pub(crate) fn configure(&mut self, tuning: PhysicsTuning) {
        self.tuning = tuning;
        let w = &mut self.world;
        w.integration_parameters.num_solver_iterations = tuning.substeps.max(1) as usize;
        for (key, h) in &self.handles {
            if key.as_rock().is_none() {
                configure_body(&mut w.bodies[h.body], &tuning);
            }
            let co = &mut w.colliders[h.collider];
            co.set_friction(tuning.friction);
            co.set_restitution(tuning.restitution);
        }
    }

    /// Number of bodies.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    /// Adds a body (after `spawn_ship`, and for new pieces of a split).
    pub(crate) fn create_body(&mut self, key: BodyKey, desc: &BodyDesc) {
        self.remove_body(key);
        let t = self.tuning;
        let w = &mut self.world;
        let mut rb = RigidBodyBuilder::dynamic()
            .pose(pose_of(&desc.body))
            .linvel(vr(desc.body.linvel))
            .angvel(desc.body.angvel)
            .additional_mass_properties(mass_of(desc.mass))
            .user_data(user_data(key))
            .build();
        configure_body(&mut rb, &t);
        let body = w.bodies.insert(rb);
        let co = ColliderBuilder::new(compound(desc.colliders))
            .mass_properties(MassProperties::new(Vector::ZERO, 0.0, 0.0))
            .friction(t.friction)
            .restitution(t.restitution)
            .user_data(user_data(key));
        let collider = w.colliders.insert_with_parent(co, body, &mut w.bodies);
        self.handles.insert(key, Handles { body, collider });
        self.refresh_bvh(collider);
    }

    /// Replaces the colliders and mass properties of a body and sets its velocities from `desc.body` (its
    /// centre of mass moved). The pose stays.
    /// Adds an anchored rock (GDR-0104): a fixed body with the rock's convex outline, hull friction and bounce.
    pub(crate) fn create_rock(&mut self, key: BodyKey, rock: &crate::voyage::rocks::Rock) {
        self.remove_body(key);
        let t = self.tuning;
        let w = &mut self.world;
        let r = Rot::from_angle(rock.angle);
        let pose = Pose::from_parts(vr(rock.pos), Rotation::from_cos_sin_unchecked(r.cos, r.sin));
        let rb = RigidBodyBuilder::fixed().pose(pose).user_data(user_data(key)).build();
        let body = w.bodies.insert(rb);
        let shape = SharedShape::convex_polyline(rock.points.iter().map(|&p| vr(p)).collect())
            .unwrap_or_else(|| SharedShape::ball(rock.radius));
        let co = ColliderBuilder::new(shape)
            .friction(t.friction)
            .restitution(t.restitution)
            .user_data(user_data(key));
        let collider = w.colliders.insert_with_parent(co, body, &mut w.bodies);
        self.handles.insert(key, Handles { body, collider });
        self.refresh_bvh(collider);
    }

    pub(crate) fn update_body(&mut self, key: BodyKey, desc: &BodyDesc) {
        let Some(&h) = self.handles.get(&key) else {
            return;
        };
        let w = &mut self.world;
        w.colliders[h.collider].set_shape(compound(desc.colliders));
        let rb = &mut w.bodies[h.body];
        rb.set_additional_mass_properties(mass_of(desc.mass), true);
        rb.set_linvel(vr(desc.body.linvel), true);
        rb.set_angvel(desc.body.angvel, true);
        self.refresh_bvh(h.collider);
    }

    pub(crate) fn remove_body(&mut self, key: BodyKey) {
        if let Some(h) = self.handles.remove(&key) {
            self.world.remove_body(h.body);
        }
    }

    /// Ghost bodies of one group don't collide with each other (set on all pieces of a split for a few
    /// ticks, group = the ship that split, so freshly separated pieces never push each other apart). `None`
    /// ends it.
    pub(crate) fn set_ghost(&mut self, key: BodyKey, group: Option<ShipId>) {
        let Some(h) = self.handles.get(&key) else {
            return;
        };
        let co = &mut self.world.colliders[h.collider];
        co.user_data &= !GHOST_MASK;
        match group {
            Some(g) => {
                co.user_data |= (g.0 as u128 + 1) << GHOST_SHIFT;
                co.set_active_hooks(ActiveHooks::FILTER_CONTACT_PAIRS);
            }
            None => co.set_active_hooks(ActiveHooks::empty()),
        }
    }

    /// Moves a body to a new key (a piece that split off as a vessel of its own changes ship). The Rapier
    /// body, its contacts and its ghost group stay; only the key and the user data change.
    pub(crate) fn rekey(&mut self, from: BodyKey, to: BodyKey) {
        let Some(h) = self.handles.remove(&from) else {
            return;
        };
        let w = &mut self.world;
        let keep = |d: u128| (d & !((1u128 << GHOST_SHIFT) - 1)) | user_data(to);
        w.bodies[h.body].user_data = keep(w.bodies[h.body].user_data);
        w.colliders[h.collider].user_data = keep(w.colliders[h.collider].user_data);
        self.handles.insert(to, h);
    }

    /// Moves a body and sets its velocities (tools, tests, respawn).
    pub(crate) fn set_body(&mut self, key: BodyKey, body: Body) {
        let Some(&h) = self.handles.get(&key) else {
            return;
        };
        let w = &mut self.world;
        let rb = &mut w.bodies[h.body];
        rb.set_position(pose_of(&body), true);
        rb.set_linvel(vr(body.linvel), true);
        rb.set_angvel(body.angvel, true);
        w.bodies
            .propagate_modified_body_positions_to_colliders(&mut w.colliders);
        self.refresh_bvh(h.collider);
    }

    /// Force (world frame, at the centre of mass) and torque for the next step, replacing the last ones.
    /// A non-zero wrench wakes a sleeping body.
    pub(crate) fn set_wrench(&mut self, key: BodyKey, force: Vec2, torque: f32) {
        let Some(h) = self.handles.get(&key) else {
            return;
        };
        let rb = &mut self.world.bodies[h.body];
        rb.reset_forces(false);
        rb.reset_torques(false);
        rb.add_force(vr(force), true);
        rb.add_torque(torque, true);
    }

    /// Puts a collider's current AABB into the broad-phase BVH, so ray casts see a body created or moved
    /// between steps right away.
    fn refresh_bvh(&mut self, collider: ColliderHandle) {
        let w = &mut self.world;
        let aabb =
            w.colliders[collider].compute_broad_phase_aabb(&w.integration_parameters, &w.bodies);
        w.broad_phase
            .set_aabb(&w.integration_parameters, collider, aabb);
    }

    /// Advances by `DT` and collects the contacts.
    pub(crate) fn step(&mut self) {
        self.world.step_with_events(&GhostFilter, &());
        self.collect_contacts();
    }

    fn collect_contacts(&mut self) {
        self.contacts.clear();
        let min = self.tuning.impact_min_impulse;
        let w = &self.world;
        for pair in w.narrow_phase.contact_pairs() {
            if !pair.has_any_active_contact() {
                continue;
            }
            let impulse = pair.total_impulse_magnitude();
            if !(impulse > 0.0 && impulse >= min) {
                continue;
            }
            // Impulse-weighted centres of the whole contact and of its strongest manifold (the first on a
            // tie: manifold order is stable), in the collider (= piece) frames: sub-shape pose × point.
            let (mut c1, mut c2) = (Vector::ZERO, Vector::ZERO);
            let mut best = None;
            let mut best_j = f32::NEG_INFINITY;
            for m in pair.solver_manifolds() {
                let (mut m1, mut m2, mut j) = (Vector::ZERO, Vector::ZERO, 0.0);
                for p in &m.points {
                    let i = p.data.impulse;
                    let p1 = m.subshape_pos1().map_or(p.local_p1, |q| q * p.local_p1);
                    let p2 = m.subshape_pos2().map_or(p.local_p2, |q| q * p.local_p2);
                    m1 += p1 * i;
                    m2 += p2 * i;
                    j += i;
                }
                c1 += m1;
                c2 += m2;
                if j > 0.0 && j > best_j {
                    best_j = j;
                    best = Some((m, m1 / j, m2 / j));
                }
            }
            let Some((m, s1, s2)) = best else {
                continue;
            };
            // `impulse` is the sum of the point impulses (normal impulses are never negative).
            let (c1, c2) = (c1 / impulse, c2 / impulse);
            let (co1, co2) = (&w.colliders[pair.collider1], &w.colliders[pair.collider2]);
            let (k1, k2) = (key_of(co1.user_data), key_of(co2.user_data));
            let normal = vs(m.data.normal);
            self.contacts.push(if k1 <= k2 {
                RawContact {
                    a: k1,
                    b: k2,
                    point: vs(co1.position() * c1),
                    normal,
                    impulse,
                    local_a: vs(c1),
                    local_b: vs(c2),
                    strongest_a: vs(s1),
                    strongest_b: vs(s2),
                    sub_a: m.subshape1,
                    sub_b: m.subshape2,
                }
            } else {
                RawContact {
                    a: k2,
                    b: k1,
                    point: vs(co2.position() * c2),
                    normal: normal * -1.0,
                    impulse,
                    local_a: vs(c2),
                    local_b: vs(c1),
                    strongest_a: vs(s2),
                    strongest_b: vs(s1),
                    sub_a: m.subshape2,
                    sub_b: m.subshape1,
                }
            });
        }
        self.contacts.sort_by_key(|c| (c.a, c.b));
    }

    /// Pose and velocities after the last step (angle via `sim::math::atan2`).
    pub fn body(&self, key: BodyKey) -> Option<Body> {
        let h = self.handles.get(&key)?;
        let rb = &self.world.bodies[h.body];
        let (t, r) = (rb.translation(), rb.rotation());
        Some(Body {
            pos: vs(t),
            angle: atan2(r.im, r.re),
            linvel: vs(rb.linvel()),
            angvel: rb.angvel(),
        })
    }

    /// Is this body a split ghost?
    pub fn is_ghost(&self, key: BodyKey) -> Option<bool> {
        let h = self.handles.get(&key)?;
        Some(ghost_group(self.world.colliders[h.collider].user_data) != 0)
    }

    /// Is this body asleep (not simulated until something wakes it)?
    pub fn is_sleeping(&self, key: BodyKey) -> Option<bool> {
        let h = self.handles.get(&key)?;
        Some(self.world.bodies[h.body].is_sleeping())
    }

    /// The mass properties Rapier uses for this body (equal to the piece's `MassProps`; tests).
    pub fn mass_props(&self, key: BodyKey) -> Option<MassProps> {
        let h = self.handles.get(&key)?;
        let m = self.world.bodies[h.body].mass_properties().local_mprops;
        Some(MassProps {
            mass: m.mass(),
            com: vs(m.local_com),
            inertia: m.principal_inertia(),
        })
    }

    /// Does any body but `own` overlap the convex polygon `points` (counter-clockwise, in `own`'s piece frame) with
    /// that frame at `body`? The regrowth's test (`ship/living.rs`): a part never re-forms inside another hull.
    pub(crate) fn overlaps_other(&self, own: BodyKey, body: &Body, points: &[Vec2]) -> bool {
        let Some(shape) = SharedShape::convex_polyline(points.iter().map(|&p| vr(p)).collect()) else {
            return false;
        };
        let w = &self.world;
        let keep = |_: ColliderHandle, c: &rapier2d::prelude::Collider| key_of(c.user_data) != own;
        let filter = QueryFilter::default().predicate(&keep);
        let qp = w.broad_phase.as_query_pipeline(
            w.narrow_phase.query_dispatcher(),
            &w.bodies,
            &w.colliders,
            filter,
        );
        qp.intersect_shape(pose_of(body), &*shape).next().is_some()
    }

    /// First hit along a ray from `origin` in unit direction `dir`, up to `max_distance`, ignoring the bodies
    /// of `exclude`. A ray that starts inside a body hits it at distance 0.
    pub fn raycast(
        &self,
        origin: Vec2,
        dir: Vec2,
        max_distance: f32,
        exclude: Option<ShipId>,
    ) -> Option<RayHit> {
        self.raycast_skipping(origin, dir, max_distance, |key| {
            exclude.is_some_and(|s| key.ship == s)
        })
    }

    /// [`Physics::raycast`] that ignores every body for which `skip(key)` is true (the sim's shots and plumes
    /// pass through friendly vessels). `skip` runs only for bodies whose bounding box the ray crosses.
    pub(crate) fn raycast_skipping(
        &self,
        origin: Vec2,
        dir: Vec2,
        max_distance: f32,
        skip: impl Fn(BodyKey) -> bool,
    ) -> Option<RayHit> {
        let w = &self.world;
        let keep = |_: ColliderHandle, c: &rapier2d::prelude::Collider| !skip(key_of(c.user_data));
        let filter = QueryFilter::default().predicate(&keep);
        let qp = w.broad_phase.as_query_pipeline(
            w.narrow_phase.query_dispatcher(),
            &w.bodies,
            &w.colliders,
            filter,
        );
        let (h, hit) =
            qp.cast_ray_and_get_normal(&Ray::new(vr(origin), vr(dir)), max_distance, true)?;
        let co = &w.colliders[h];
        let rb = &w.bodies[co.parent()?];
        let point = origin + dir * hit.time_of_impact;
        Some(RayHit {
            key: key_of(co.user_data),
            distance: hit.time_of_impact,
            point,
            local_point: vs(rb.position().inverse_transform_point(vr(point))),
            normal: vs(hit.normal),
        })
    }
}

impl Clone for Physics {
    /// Clones the whole engine state; the pipeline holds only per-step workspace and starts fresh.
    fn clone(&self) -> Physics {
        let w = &self.world;
        Physics {
            world: PhysicsWorld {
                gravity: w.gravity,
                integration_parameters: w.integration_parameters,
                physics_pipeline: PhysicsPipeline::new(),
                collision_pipeline: Default::default(),
                islands: w.islands.clone(),
                broad_phase: w.broad_phase.clone(),
                narrow_phase: w.narrow_phase.clone(),
                bodies: w.bodies.clone(),
                colliders: w.colliders.clone(),
                impulse_joints: w.impulse_joints.clone(),
                multibody_joints: w.multibody_joints.clone(),
                soft_bodies: w.soft_bodies.clone(),
                ccd_solver: w.ccd_solver.clone(),
            },
            handles: self.handles.clone(),
            tuning: self.tuning,
            contacts: self.contacts.clone(),
        }
    }
}

impl core::fmt::Debug for Physics {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Physics")
            .field("bodies", &self.handles.len())
            .field("tuning", &self.tuning)
            .finish_non_exhaustive()
    }
}

impl StateHash for Physics {
    /// Rapier's own state per body, in key order: pose (translation, rotation as cos/sin), velocities,
    /// sleeping flag and ghost group. The sim hashes the derived `Body` of every piece as well.
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.handles.len() as u32);
        for (key, hd) in &self.handles {
            let rb = &self.world.bodies[hd.body];
            let (t, r, v) = (rb.translation(), rb.rotation(), rb.linvel());
            h.u32(key.ship.0);
            h.u32(key.piece.0);
            h.f32(t.x);
            h.f32(t.y);
            h.f32(r.re);
            h.f32(r.im);
            h.f32(v.x);
            h.f32(v.y);
            h.f32(rb.angvel());
            h.bool(rb.is_sleeping());
            h.u32(ghost_group(self.world.colliders[hd.collider].user_data));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(ship: u32, piece: u32) -> BodyKey {
        BodyKey {
            ship: ShipId(ship),
            piece: PieceId(piece),
        }
    }

    fn square() -> [ConvexPolygon; 1] {
        [ConvexPolygon::rect(
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, 1.0),
        )]
    }

    fn add(ph: &mut Physics, k: BodyKey, body: Body, cols: &[ConvexPolygon]) {
        let mass = MassProps {
            mass: 1.0,
            com: Vec2::ZERO,
            inertia: 1.0,
        };
        ph.create_body(
            k,
            &BodyDesc {
                body,
                mass,
                colliders: cols,
            },
        );
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
    fn wrenches_act_at_the_centre_of_mass_and_are_replaced_each_tick() {
        let mut ph = Physics::new(PhysicsTuning::default());
        let k = key(0, 0);
        let cols = square();
        // The centre of mass is off the frame origin.
        let mass = MassProps {
            mass: 2.0,
            com: Vec2::new(0.5, 0.0),
            inertia: 4.0,
        };
        ph.create_body(
            k,
            &BodyDesc {
                body: Body::default(),
                mass,
                colliders: &cols,
            },
        );
        for _ in 0..30 {
            ph.set_wrench(k, Vec2::new(4.0, 0.0), 0.0);
            ph.step();
        }
        // a = 2 m/s² for 1 s, no spin from a force at the centre of mass.
        let b = ph.body(k).unwrap();
        assert!((b.linvel.x - 2.0).abs() < 1e-4, "{b:?}");
        assert!(b.pos.x > 0.95 && b.pos.x < 1.05, "{b:?}");
        assert_eq!((b.angvel, b.angle), (0.0, 0.0));
        // τ·dt / I = 120 / 30 / 4 = 1 rad/s; without a new wrench nothing more is added.
        ph.set_wrench(k, Vec2::ZERO, 120.0);
        ph.step();
        ph.set_wrench(k, Vec2::ZERO, 0.0);
        ph.step();
        let b1 = ph.body(k).unwrap();
        assert!((b1.angvel - 1.0).abs() < 1e-4, "{b1:?}");
        assert!((b1.linvel.x - 2.0).abs() < 1e-4, "{b1:?}");
        // It turns about the centre of mass, not about the frame origin.
        let com = |b: &Body| b.to_world(mass.com);
        let moved = com(&b1) - com(&b);
        assert!(
            moved.y.abs() < 1e-3 && (moved.x - 4.0 / 30.0).abs() < 1e-2,
            "{moved:?}"
        );
    }

    #[test]
    fn mass_properties_are_the_sims_bit_for_bit() {
        let mut ph = Physics::new(PhysicsTuning::default());
        let k = key(0, 0);
        let cols = [
            ConvexPolygon::rect(Vec2::new(-1.5, -0.5), Vec2::new(2.0, 1.0)),
            ConvexPolygon::rect(Vec2::new(-1.5, 1.0), Vec2::new(0.5, 3.0)),
        ];
        let mass = MassProps {
            mass: 106.5,
            com: Vec2::new(0.3, -0.1),
            inertia: 1176.25,
        };
        let mut desc = BodyDesc {
            body: Body::default(),
            mass,
            colliders: &cols,
        };
        ph.create_body(k, &desc);
        ph.step();
        let m = ph.mass_props(k).unwrap();
        assert_eq!(m.com, mass.com);
        assert!((m.mass - mass.mass).abs() <= 1e-6 * mass.mass, "{m:?}");
        assert!(
            (m.inertia - mass.inertia).abs() <= 1e-6 * mass.inertia,
            "{m:?}"
        );
        // A rebuild replaces them (the colliders contribute nothing).
        desc.mass = MassProps {
            mass: 40.0,
            com: Vec2::new(-0.25, 0.75),
            inertia: 90.0,
        };
        desc.colliders = &cols[..1];
        ph.update_body(k, &desc);
        ph.step();
        let m = ph.mass_props(k).unwrap();
        assert_eq!(m.com, desc.mass.com);
        assert!(
            (m.mass - 40.0).abs() <= 1e-5 && (m.inertia - 90.0).abs() <= 1e-4,
            "{m:?}"
        );
    }

    #[test]
    fn raycast_hits_the_nearest_face_and_skips_one_ship() {
        let mut ph = Physics::new(PhysicsTuning::default());
        let cols = square();
        for (k, x) in [(0u32, 10.0f32), (1, 5.0)] {
            add(&mut ph, key(k, k), Body::at(Vec2::new(x, 0.0), 0.3), &cols);
        }
        // Visible at once, before any step.
        let hit = ph.raycast(Vec2::ZERO, Vec2::X, 100.0, None).unwrap();
        assert_eq!(hit.key, key(1, 1));
        assert!(hit.distance > 3.5 && hit.distance < 4.0, "{}", hit.distance);
        assert!(hit.normal.x < -0.9);
        let back = Body::at(Vec2::new(5.0, 0.0), 0.3).to_world(hit.local_point);
        assert!((back - hit.point).length() < 1e-4);
        let hit = ph
            .raycast(Vec2::ZERO, Vec2::X, 100.0, Some(ShipId(1)))
            .unwrap();
        assert_eq!(hit.key, key(0, 0));
        assert!(ph.raycast(Vec2::ZERO, Vec2::Y, 100.0, None).is_none());
        assert!(ph.raycast(Vec2::ZERO, Vec2::X, 3.0, None).is_none());
        // A ray that starts inside a body hits it at once.
        let hit = ph
            .raycast(Vec2::new(5.0, 0.0), Vec2::X, 100.0, None)
            .unwrap();
        assert_eq!((hit.key, hit.distance), (key(1, 1), 0.0));
    }

    #[test]
    fn ghosts_of_one_group_pass_through_each_other() {
        // Two squares 0.5 m apart closing at 4 m/s: ghosts of one group fly through each other (also across
        // ships: a split-off vessel keeps its parent's group, and a rekeyed body keeps its group); ghosts of
        // different groups, and pieces that are no ghosts, collide.
        let run = |a: BodyKey, b: BodyKey, ga: Option<u32>, gb: Option<u32>| {
            let mut ph = Physics::new(PhysicsTuning::default());
            let cols = square();
            let moving = |x: f32, v: f32| Body {
                linvel: Vec2::new(v, 0.0),
                ..Body::at(Vec2::new(x, 0.0), 0.0)
            };
            add(&mut ph, a, moving(0.0, 2.0), &cols);
            add(&mut ph, b, moving(2.5, -2.0), &cols);
            ph.set_ghost(a, ga.map(ShipId));
            ph.set_ghost(b, gb.map(ShipId));
            let moved = key(9, b.piece.0);
            ph.rekey(b, moved);
            assert_eq!(ph.is_ghost(moved), Some(gb.is_some()));
            let mut contacts = 0;
            for _ in 0..10 {
                ph.step();
                contacts += ph.contacts.len();
            }
            (ph.body(a).unwrap().linvel.x, contacts)
        };
        assert_eq!(run(key(0, 0), key(0, 1), Some(0), Some(0)), (2.0, 0));
        assert_eq!(run(key(0, 0), key(1, 1), Some(0), Some(0)), (2.0, 0));
        let (v, n) = run(key(0, 0), key(1, 1), Some(0), Some(1));
        assert!(v < 1.0 && n > 0, "{v} {n}");
        let (v, n) = run(key(0, 0), key(0, 1), None, None);
        assert!(v < 1.0 && n > 0, "{v} {n}");
    }
}
