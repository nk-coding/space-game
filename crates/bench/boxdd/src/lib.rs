//! Box2D C reference adapter (raw FFI via boxdd-sys; Box2D v3.2-dev commit 56edae7, the commit box2d-rust
//! 1.4.0 ports). Reference number only: native only, never part of the game.
//!
//! Multithreading uses Box2D's built-in scheduler (`workerCount > 1`, no task callbacks).

use bench_core::{BodyDef, BodyKind, BreakEvent, EngineConfig, PhysicsEngine, Ray, RayStats, Shape, StateHasher};
use bench_core::{scenario, velocity_at};
use boxdd_sys::ffi::*;
use std::os::raw::c_void;

pub struct BoxddEngine {
    world: b2WorldId,
    dt: f32,
    substeps: i32,
    threads: u32,
    bodies: Vec<b2BodyId>,
    shapes: Vec<Vec<b2ShapeId>>,
    shape_count: usize,
}

fn v2(x: f32, y: f32) -> b2Vec2 {
    b2Vec2 { x, y }
}

impl BoxddEngine {
    pub fn new(cfg: EngineConfig) -> Self {
        unsafe {
            let mut wd = b2DefaultWorldDef();
            wd.gravity = v2(0.0, 0.0);
            wd.workerCount = cfg.threads.max(1) as i32;
            let world = b2CreateWorld(&wd);
            BoxddEngine { world, dt: cfg.dt, substeps: cfg.substeps as i32, threads: cfg.threads, bodies: Vec::new(), shapes: Vec::new(), shape_count: 0 }
        }
    }

    pub fn supports_threads() -> bool {
        true
    }

    fn attach(&mut self, id: u32, body: b2BodyId, shapes: &[Shape]) -> Vec<b2ShapeId> {
        unsafe {
            let mut sd = b2DefaultShapeDef();
            sd.density = scenario::DENSITY;
            sd.material.friction = scenario::FRICTION;
            sd.material.restitution = scenario::RESTITUTION;
            sd.userData = id as usize as *mut c_void;
            sd.updateBodyMass = false;
            let out: Vec<b2ShapeId> = shapes
                .iter()
                .map(|s| {
                    let poly = match s {
                        Shape::Box { cx, cy, hx, hy } => b2MakeOffsetBox(*hx, *hy, v2(*cx, *cy), b2Rot { c: 1.0, s: 0.0 }),
                        Shape::Polygon { verts } => {
                            let pts: Vec<b2Vec2> = verts.iter().map(|v| v2(v[0], v[1])).collect();
                            let hull = b2ComputeHull(pts.as_ptr(), pts.len() as i32);
                            assert!(hull.count >= 3, "bad hull");
                            b2MakePolygon(&hull, 0.0)
                        }
                    };
                    b2CreatePolygonShape(body, &sd, &poly)
                })
                .collect();
            b2Body_ApplyMassFromShapes(body);
            self.shape_count += out.len();
            out
        }
    }
}

impl Drop for BoxddEngine {
    fn drop(&mut self) {
        unsafe { b2DestroyWorld(self.world) }
    }
}

impl PhysicsEngine for BoxddEngine {
    fn name(&self) -> String {
        "box2d-c".into()
    }

    fn add_body(&mut self, b: &BodyDef) {
        unsafe {
            let mut bd = b2DefaultBodyDef();
            bd.type_ = match b.kind {
                BodyKind::Dynamic => b2BodyType_b2_dynamicBody,
                BodyKind::Static => b2BodyType_b2_staticBody,
            };
            bd.position = v2(b.pos[0], b.pos[1]);
            bd.rotation = b2Rot { c: b.rot[0], s: b.rot[1] };
            bd.linearVelocity = v2(b.linvel[0], b.linvel[1]);
            bd.angularVelocity = b.angvel;
            bd.userData = b.id as usize as *mut c_void;
            let id = b2CreateBody(self.world, &bd);
            let s = self.attach(b.id, id, &b.shapes);
            self.bodies.push(id);
            self.shapes.push(s);
        }
    }

    fn step(&mut self) {
        unsafe { b2World_Step(self.world, self.dt, self.substeps) }
    }

    fn cast_rays(&mut self, rays: &[Ray], stats: &mut RayStats) {
        unsafe {
            let filter = b2DefaultQueryFilter();
            for (i, r) in rays.iter().enumerate() {
                let res = b2World_CastRayClosest(
                    self.world,
                    v2(r.origin[0], r.origin[1]),
                    v2(r.dir[0] * r.max_dist, r.dir[1] * r.max_dist),
                    filter,
                );
                if res.hit {
                    let body = b2Shape_GetUserData(res.shapeId) as usize as u32;
                    stats.hit(i as u32, body, res.fraction);
                }
            }
        }
    }

    fn split(&mut self, ev: &BreakEvent) {
        unsafe {
            let pb = self.bodies[ev.parent as usize];
            let pos = b2Body_GetPosition(pb);
            let rot = b2Body_GetRotation(pb);
            let v = b2Body_GetLinearVelocity(pb);
            let ang = b2Body_GetAngularVelocity(pb);
            let c = b2Body_GetWorldCenter(pb);
            let (v, com_old) = ([v.x, v.y], [c.x, c.y]);
            let old = std::mem::take(&mut self.shapes[ev.parent as usize]);
            self.shape_count -= old.len();
            for s in old {
                b2DestroyShape(s, false);
            }
            let new = self.attach(ev.parent, pb, &ev.parent_shapes);
            self.shapes[ev.parent as usize] = new;
            let c = b2Body_GetWorldCenter(pb);
            let nv = velocity_at(v, ang, com_old, [c.x, c.y]);
            b2Body_SetLinearVelocity(pb, v2(nv[0], nv[1]));
            b2Body_SetAngularVelocity(pb, ang);
            for (id, shapes) in &ev.pieces {
                let mut bd = b2DefaultBodyDef();
                bd.type_ = b2BodyType_b2_dynamicBody;
                bd.position = pos;
                bd.rotation = rot;
                bd.angularVelocity = ang;
                bd.userData = *id as usize as *mut c_void;
                let b = b2CreateBody(self.world, &bd);
                let s = self.attach(*id, b, shapes);
                let c = b2Body_GetWorldCenter(b);
                let nv = velocity_at(v, ang, com_old, [c.x, c.y]);
                b2Body_SetLinearVelocity(b, v2(nv[0], nv[1]));
                self.bodies.push(b);
                self.shapes.push(s);
            }
        }
    }

    fn hash_state(&self, h: &mut StateHasher) {
        unsafe {
            for &b in &self.bodies {
                let p = b2Body_GetPosition(b);
                let r = b2Body_GetRotation(b);
                let v = b2Body_GetLinearVelocity(b);
                h.body([p.x, p.y], [r.c, r.s], [v.x, v.y], b2Body_GetAngularVelocity(b));
            }
        }
    }

    fn body_count(&self) -> usize {
        self.bodies.len()
    }

    fn collider_count(&self) -> usize {
        self.shape_count
    }

    fn pose(&self, id: u32) -> ([f32; 2], [f32; 2]) {
        unsafe {
            let b = self.bodies[id as usize];
            let (p, r) = (b2Body_GetPosition(b), b2Body_GetRotation(b));
            ([p.x, p.y], [r.c, r.s])
        }
    }
}

impl BoxddEngine {
    pub fn threads(&self) -> u32 {
        self.threads
    }
}
