//! Rapier2D adapter (rapier2d 0.36, `enhanced-determinism`; `parallel` behind a feature).
//!
//! Two ways to build a ship, both measured:
//! - `rapier`: one collider per shape, all attached to the ship's rigid body (each collider is its own
//!   broad-phase leaf);
//! - `rapier-compound`: one collider per body holding a `SharedShape::compound` (parry builds a BVH inside).

use bench_core::{BodyDef, BodyKind, BreakEvent, EngineConfig, PhysicsEngine, Ray, RayStats, Shape, StateHasher};
use bench_core::{scenario, velocity_at};
use rapier2d::prelude::*;

pub struct RapierEngine {
    world: PhysicsWorld,
    compound: bool,
    threads: u32,
    bodies: Vec<RigidBodyHandle>,
    colliders: Vec<Vec<ColliderHandle>>,
}

impl RapierEngine {
    pub fn new(cfg: EngineConfig, compound: bool) -> Self {
        let mut world = PhysicsWorld::new();
        world.gravity = Vector::ZERO;
        world.integration_parameters.dt = cfg.dt;
        // In rapier 0.36 `num_solver_iterations` is the number of substeps (TGS-soft); default is 4.
        world.integration_parameters.num_solver_iterations = cfg.substeps as usize;
        #[cfg(feature = "parallel")]
        if cfg.threads > 1 {
            world.configure_thread_pool(cfg.threads as usize).expect("rayon pool");
        }
        RapierEngine { world, compound, threads: cfg.threads, bodies: Vec::new(), colliders: Vec::new() }
    }

    pub fn supports_threads() -> bool {
        cfg!(feature = "parallel")
    }

    fn shared_shape(s: &Shape) -> SharedShape {
        match s {
            Shape::Box { hx, hy, .. } => SharedShape::cuboid(*hx, *hy),
            Shape::Polygon { verts } => {
                let pts: Vec<Vector> = verts.iter().map(|v| Vector::new(v[0], v[1])).collect();
                if pts.len() == 3 {
                    SharedShape::triangle(pts[0], pts[1], pts[2])
                } else {
                    SharedShape::convex_polyline(pts).expect("convex polygon")
                }
            }
        }
    }

    /// Box shapes are centred at the origin of their own frame; polygons carry their offset in the vertices.
    fn shape_pose(s: &Shape) -> Pose {
        match s {
            Shape::Box { cx, cy, .. } => Pose::translation(*cx, *cy),
            Shape::Polygon { .. } => Pose::IDENTITY,
        }
    }

    fn attach(&mut self, id: u32, handle: RigidBodyHandle, shapes: &[Shape]) -> Vec<ColliderHandle> {
        let w = &mut self.world;
        let mk = |b: ColliderBuilder| {
            b.density(scenario::DENSITY).friction(scenario::FRICTION).restitution(scenario::RESTITUTION).user_data(id as u128)
        };
        if self.compound {
            let parts: Vec<(Pose, SharedShape)> = shapes.iter().map(|s| (Self::shape_pose(s), Self::shared_shape(s))).collect();
            let c = mk(ColliderBuilder::compound(parts));
            vec![w.colliders.insert_with_parent(c, handle, &mut w.bodies)]
        } else {
            shapes
                .iter()
                .map(|s| {
                    let c = mk(ColliderBuilder::new(Self::shared_shape(s)).position(Self::shape_pose(s)));
                    w.colliders.insert_with_parent(c, handle, &mut w.bodies)
                })
                .collect()
        }
    }
}

impl PhysicsEngine for RapierEngine {
    fn name(&self) -> String {
        let base = if self.compound { "rapier-compound" } else { "rapier" };
        base.to_string()
    }

    fn add_body(&mut self, b: &BodyDef) {
        debug_assert_eq!(b.id as usize, self.bodies.len());
        let builder = match b.kind {
            BodyKind::Dynamic => RigidBodyBuilder::dynamic(),
            BodyKind::Static => RigidBodyBuilder::fixed(),
        };
        let rb = builder
            .pose(Pose::from_parts(Vector::new(b.pos[0], b.pos[1]), Rotation::from_cos_sin_unchecked(b.rot[0], b.rot[1])))
            .linvel(Vector::new(b.linvel[0], b.linvel[1]))
            .angvel(b.angvel)
            .user_data(b.id as u128)
            .build();
        let h = self.world.bodies.insert(rb);
        let cs = self.attach(b.id, h, &b.shapes);
        self.bodies.push(h);
        self.colliders.push(cs);
    }

    fn step(&mut self) {
        self.world.step();
    }

    fn cast_rays(&mut self, rays: &[Ray], stats: &mut RayStats) {
        let w = &self.world;
        let qp = w.broad_phase.as_query_pipeline(w.narrow_phase.query_dispatcher(), &w.bodies, &w.colliders, QueryFilter::default());
        for (i, r) in rays.iter().enumerate() {
            let ray = parry2d::query::Ray::new(Vector::new(r.origin[0], r.origin[1]), Vector::new(r.dir[0], r.dir[1]));
            if let Some((h, toi)) = qp.cast_ray(&ray, r.max_dist, true) {
                stats.hit(i as u32, w.colliders[h].user_data as u32, toi);
            }
        }
    }

    fn split(&mut self, ev: &BreakEvent) {
        let ph = self.bodies[ev.parent as usize];
        let (pose, v, w_ang, com_old) = {
            let rb = &self.world.bodies[ph];
            let c = rb.center_of_mass();
            (*rb.position(), rb.linvel(), rb.angvel(), [c.x, c.y])
        };
        let v = [v.x, v.y];

        // Rebuild the parent's colliders.
        let old = std::mem::take(&mut self.colliders[ev.parent as usize]);
        {
            let PhysicsWorld { bodies, colliders, islands, soft_bodies, .. } = &mut self.world;
            for h in old {
                colliders.remove(h, islands, bodies, soft_bodies, true);
            }
        }
        let new = self.attach(ev.parent, ph, &ev.parent_shapes);
        self.colliders[ev.parent as usize] = new;
        {
            let PhysicsWorld { bodies, colliders, .. } = &mut self.world;
            let rb = &mut bodies[ph];
            rb.recompute_mass_properties_from_colliders(colliders);
            let c = rb.center_of_mass();
            let nv = velocity_at(v, w_ang, com_old, [c.x, c.y]);
            rb.set_linvel(Vector::new(nv[0], nv[1]), true);
        }

        // New bodies for the pieces, same pose as the parent, velocity of the parent's motion at their COM.
        for (id, shapes) in &ev.pieces {
            debug_assert_eq!(*id as usize, self.bodies.len());
            let rb = RigidBodyBuilder::dynamic().pose(pose).angvel(w_ang).user_data(*id as u128).build();
            let h = self.world.bodies.insert(rb);
            let cs = self.attach(*id, h, shapes);
            let PhysicsWorld { bodies, colliders, .. } = &mut self.world;
            let rb = &mut bodies[h];
            rb.recompute_mass_properties_from_colliders(colliders);
            let c = rb.center_of_mass();
            let nv = velocity_at(v, w_ang, com_old, [c.x, c.y]);
            rb.set_linvel(Vector::new(nv[0], nv[1]), true);
            self.bodies.push(h);
            self.colliders.push(cs);
        }
    }

    fn hash_state(&self, h: &mut StateHasher) {
        for &bh in &self.bodies {
            let rb = &self.world.bodies[bh];
            let t = rb.translation();
            let r = rb.rotation();
            let v = rb.linvel();
            h.body([t.x, t.y], [r.re, r.im], [v.x, v.y], rb.angvel());
        }
    }

    fn body_count(&self) -> usize {
        self.world.bodies.len()
    }

    fn collider_count(&self) -> usize {
        self.world.colliders.len()
    }

    fn pose(&self, id: u32) -> ([f32; 2], [f32; 2]) {
        let rb = &self.world.bodies[self.bodies[id as usize]];
        let (t, r) = (rb.translation(), rb.rotation());
        ([t.x, t.y], [r.re, r.im])
    }
}

impl RapierEngine {
    pub fn threads(&self) -> u32 {
        self.threads
    }
}
