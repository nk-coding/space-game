//! Avian2D adapter (avian2d 0.7.0 on Bevy 0.19.1, `enhanced-determinism`; `parallel` behind a feature).
//!
//! Headless: a bare `App` with `TaskPoolPlugin` + `PhysicsPlugins` (interpolation disabled), physics running
//! in its own schedule that we run by hand with a fixed 1/30 s delta. Only that schedule is timed (it contains
//! Avian's whole step: prepare, broad/narrow phase, substeps, writeback incl. Position -> Transform sync).
//!
//! Two ways to build a ship, both measured:
//! - `avian`: one child entity per shape (`ChildOf(body)` + `Transform` offset), Avian's hierarchy way;
//! - `avian-compound`: one `Collider::compound` on the body entity.

use avian2d::math::Vector;
use avian2d::prelude::*;
use bench_core::scenario::{self, shapes_centroid};
use bench_core::{BodyDef, BodyKind, BreakEvent, EngineConfig, PhysicsEngine, Ray, RayStats, Shape, StateHasher, velocity_at};
use bevy::app::{TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy};
use bevy::ecs::schedule::{ScheduleLabel, SingleThreadedExecutor};
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use core::time::Duration;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct BenchPhysics;

/// Stable benchmark id on body and collider entities (for ray hit -> body mapping).
#[derive(Component, Clone, Copy)]
struct BodyId(u32);

pub struct AvianEngine {
    app: App,
    compound: bool,
    dt: Duration,
    bodies: Vec<Entity>,
    /// Child collider entities per body (child mode only).
    children: Vec<Vec<Entity>>,
    collider_count: usize,
    spatial: SystemState<SpatialQuery<'static, 'static>>,
}

impl AvianEngine {
    pub fn new(cfg: EngineConfig, compound: bool) -> Self {
        let mut app = App::new();
        let mut opts = TaskPoolOptions::default();
        if cfg!(feature = "parallel") && cfg.threads > 1 {
            let n = cfg.threads as usize;
            opts.compute = TaskPoolThreadAssignmentPolicy { min_threads: n, max_threads: n, percent: 1.0, on_thread_spawn: None, on_thread_destroy: None };
        }
        app.add_plugins((
            TaskPoolPlugin { task_pool_options: opts },
            PhysicsPlugins::new(BenchPhysics).build().disable::<PhysicsInterpolationPlugin>(),
        ));
        app.init_resource::<Time>();
        app.insert_resource(Gravity::ZERO);
        app.insert_resource(SubstepCount(cfg.substeps));
        if !(cfg!(feature = "parallel") && cfg.threads > 1) {
            // Single-thread runs: no async tree optimisation tasks.
            app.insert_resource(avian2d::collider_tree::ColliderTreeOptimization { use_async_tasks: false, ..Default::default() });
        }
        app.edit_schedule(BenchPhysics, |s| {
            s.set_executor(SingleThreadedExecutor::new());
        });
        app.finish();
        app.cleanup();
        let spatial = SystemState::<SpatialQuery<'static, 'static>>::new(app.world_mut());
        AvianEngine {
            app,
            compound,
            dt: Duration::from_secs_f64(scenario::DT as f64),
            bodies: Vec::new(),
            children: Vec::new(),
            collider_count: 0,
            spatial,
        }
    }

    pub fn supports_threads() -> bool {
        cfg!(feature = "parallel")
    }

    fn collider(s: &Shape, offset: bool) -> Collider {
        match s {
            Shape::Box { cx, cy, hx, hy } => {
                let r = Collider::rectangle(2.0 * hx, 2.0 * hy);
                if offset && (*cx != 0.0 || *cy != 0.0) {
                    Collider::compound(vec![(Position::new(Vector::new(*cx, *cy)), Rotation::IDENTITY, r)])
                } else {
                    r
                }
            }
            Shape::Polygon { verts } => {
                let p: Vec<Vector> = verts.iter().map(|v| Vector::new(v[0], v[1])).collect();
                if p.len() == 3 { Collider::triangle(p[0], p[1], p[2]) } else { Collider::convex_polyline(p).expect("convex") }
            }
        }
    }

    /// Compound collider for a body's shapes (a single centred shape stays a plain collider).
    fn compound_collider(shapes: &[Shape]) -> Collider {
        if shapes.len() == 1 {
            return Self::collider(&shapes[0], true);
        }
        let parts: Vec<(Position, Rotation, Collider)> = shapes
            .iter()
            .map(|s| match s {
                Shape::Box { cx, cy, .. } => (Position::new(Vector::new(*cx, *cy)), Rotation::IDENTITY, Self::collider(s, false)),
                Shape::Polygon { .. } => (Position::new(Vector::ZERO), Rotation::IDENTITY, Self::collider(s, false)),
            })
            .collect();
        Collider::compound(parts)
    }

    fn material() -> (ColliderDensity, Friction, Restitution) {
        (ColliderDensity(scenario::DENSITY), Friction::new(scenario::FRICTION), Restitution::new(scenario::RESTITUTION))
    }

    /// Spawn child collider entities (child mode).
    fn spawn_children(&mut self, id: u32, body: Entity, shapes: &[Shape]) -> Vec<Entity> {
        let w = self.app.world_mut();
        let out = shapes
            .iter()
            .map(|s| {
                let (collider, tf) = match s {
                    Shape::Box { cx, cy, .. } => (Self::collider(s, false), Transform::from_xyz(*cx, *cy, 0.0)),
                    Shape::Polygon { .. } => (Self::collider(s, false), Transform::IDENTITY),
                };
                w.spawn((collider, tf, Self::material(), BodyId(id), ChildOf(body))).id()
            })
            .collect();
        self.collider_count += shapes.len();
        out
    }

    fn spawn_body(&mut self, id: u32, kind: BodyKind, pos: [f32; 2], rot: Rotation, linvel: [f32; 2], angvel: f32, shapes: &[Shape]) -> Entity {
        let rb = match kind {
            BodyKind::Dynamic => RigidBody::Dynamic,
            BodyKind::Static => RigidBody::Static,
        };
        let base = (
            rb,
            Position::new(Vector::new(pos[0], pos[1])),
            rot,
            LinearVelocity(Vector::new(linvel[0], linvel[1])),
            AngularVelocity(angvel),
            BodyId(id),
        );
        let e = if self.compound {
            self.collider_count += 1;
            let c = Self::compound_collider(shapes);
            self.app.world_mut().spawn((base, c, Self::material())).id()
        } else {
            self.app.world_mut().spawn(base).id()
        };
        let ch = if self.compound { Vec::new() } else { self.spawn_children(id, e, shapes) };
        self.bodies.push(e);
        self.children.push(ch);
        e
    }

    /// World-space centre of mass. In compound mode recompute now (exact); in child mode the children's
    /// collider transforms are only propagated in the next step, so use the analytic centroid of the shapes.
    fn com_world(&mut self, e: Entity, shapes: &[Shape]) -> [f32; 2] {
        let local = if self.compound {
            self.app
                .world_mut()
                .run_system_once(move |mut h: MassPropertyHelper| h.update_mass_properties(e))
                .expect("mass helper");
            let c = self.app.world().get::<ComputedCenterOfMass>(e).unwrap().0;
            [c.x, c.y]
        } else {
            shapes_centroid(shapes)
        };
        let w = self.app.world();
        let p = w.get::<Position>(e).unwrap().0;
        let r = *w.get::<Rotation>(e).unwrap();
        let c = r * Vector::new(local[0], local[1]);
        [p.x + c.x, p.y + c.y]
    }
}

use bevy::ecs::system::RunSystemOnce;

impl PhysicsEngine for AvianEngine {
    fn name(&self) -> String {
        if self.compound { "avian-compound".into() } else { "avian".into() }
    }

    fn add_body(&mut self, b: &BodyDef) {
        self.spawn_body(b.id, b.kind, b.pos, Rotation::from_sin_cos(b.rot[1], b.rot[0]), b.linvel, b.angvel, &b.shapes);
    }

    fn step(&mut self) {
        let w = self.app.world_mut();
        w.resource_mut::<Time>().advance_by(self.dt);
        w.run_schedule(BenchPhysics);
        w.clear_trackers();
    }

    fn cast_rays(&mut self, rays: &[Ray], stats: &mut RayStats) {
        let world = self.app.world();
        let sq = self.spatial.get(world).expect("spatial query");
        let filter = SpatialQueryFilter::default();
        for (i, r) in rays.iter().enumerate() {
            let dir = Dir2::new_unchecked(Vector::new(r.dir[0], r.dir[1]));
            if let Some(hit) = sq.cast_ray(Vector::new(r.origin[0], r.origin[1]), dir, r.max_dist, true, &filter) {
                let id = world.get::<BodyId>(hit.entity).map(|b| b.0).unwrap_or(u32::MAX);
                stats.hit(i as u32, id, hit.distance);
            }
        }
    }

    fn split(&mut self, ev: &BreakEvent) {
        let pe = self.bodies[ev.parent as usize];
        let (rot, v, ang, com_old) = {
            let w = self.app.world();
            let p = w.get::<Position>(pe).unwrap().0;
            let r = *w.get::<Rotation>(pe).unwrap();
            let v = w.get::<LinearVelocity>(pe).unwrap().0;
            let ang = w.get::<AngularVelocity>(pe).unwrap().0;
            let c = r * w.get::<ComputedCenterOfMass>(pe).unwrap().0;
            (r, [v.x, v.y], ang, [p.x + c.x, p.y + c.y])
        };
        let pos = self.app.world().get::<Position>(pe).unwrap().0;

        // Rebuild the parent.
        if self.compound {
            let c = Self::compound_collider(&ev.parent_shapes);
            self.app.world_mut().entity_mut(pe).insert(c);
        } else {
            let old = std::mem::take(&mut self.children[ev.parent as usize]);
            self.collider_count -= old.len();
            for c in old {
                self.app.world_mut().entity_mut(c).despawn();
            }
            let new = self.spawn_children(ev.parent, pe, &ev.parent_shapes);
            self.children[ev.parent as usize] = new;
        }
        let com_new = self.com_world(pe, &ev.parent_shapes);
        let nv = velocity_at(v, ang, com_old, com_new);
        self.app.world_mut().get_mut::<LinearVelocity>(pe).unwrap().0 = Vector::new(nv[0], nv[1]);

        for (id, shapes) in &ev.pieces {
            let e = self.spawn_body(*id, BodyKind::Dynamic, [pos.x, pos.y], rot, [0.0, 0.0], ang, shapes);
            let com = self.com_world(e, shapes);
            let nv = velocity_at(v, ang, com_old, com);
            self.app.world_mut().get_mut::<LinearVelocity>(e).unwrap().0 = Vector::new(nv[0], nv[1]);
        }
    }

    fn hash_state(&self, h: &mut StateHasher) {
        let w = self.app.world();
        for &e in &self.bodies {
            let p = w.get::<Position>(e).unwrap().0;
            let r = w.get::<Rotation>(e).unwrap();
            let v = w.get::<LinearVelocity>(e).unwrap().0;
            let a = w.get::<AngularVelocity>(e).unwrap().0;
            h.body([p.x, p.y], [r.cos, r.sin], [v.x, v.y], a);
        }
    }

    fn body_count(&self) -> usize {
        self.bodies.len()
    }

    fn collider_count(&self) -> usize {
        self.collider_count
    }

    fn pose(&self, id: u32) -> ([f32; 2], [f32; 2]) {
        let w = self.app.world();
        let e = self.bodies[id as usize];
        let p = w.get::<Position>(e).unwrap().0;
        let r = w.get::<Rotation>(e).unwrap();
        ([p.x, p.y], [r.cos, r.sin])
    }
}
