//! box2d-rust adapter (box2d-rust 1.4.0: pure Rust port of Box2D v3.1/3.2-dev, serial by design).
//!
//! Box2D has no compound shape: every cell is its own polygon shape (and broad-phase proxy) on the body.

use bench_core::{BodyDef, BodyKind, BreakEvent, EngineConfig, PhysicsEngine, Ray, RayStats, Shape, StateHasher};
use bench_core::{scenario, velocity_at};
use box2d_rust::body::*;
use box2d_rust::geometry::{make_offset_box, make_polygon};
use box2d_rust::hull::compute_hull;
use box2d_rust::shape::{create_polygon_shape, destroy_shape, shape_get_user_data};
use box2d_rust::types::{BodyType, ShapeDef, default_body_def, default_query_filter, default_shape_def, default_world_def};
use box2d_rust::world::{World, world_cast_ray_closest, world_step};
use box2d_rust::{BodyId, Rot, ShapeId, Vec2};

pub struct Box2dEngine {
    world: World,
    dt: f32,
    substeps: i32,
    bodies: Vec<BodyId>,
    shapes: Vec<Vec<ShapeId>>,
    shape_count: usize,
}

impl Box2dEngine {
    pub fn new(cfg: EngineConfig) -> Self {
        let mut wd = default_world_def();
        wd.gravity = Vec2::new(0.0, 0.0);
        wd.enable_sleep = cfg.sleep;
        // Defaults kept: sleep on, continuous (vs static) on, contact hertz 30, damping 10, push 3 m/s.
        let world = World::new(&wd);
        Box2dEngine { world, dt: cfg.dt, substeps: cfg.substeps as i32, bodies: Vec::new(), shapes: Vec::new(), shape_count: 0 }
    }

    pub fn supports_threads() -> bool {
        false
    }

    fn shape_def(id: u32) -> ShapeDef {
        let mut sd = default_shape_def();
        sd.density = scenario::DENSITY;
        sd.material.friction = scenario::FRICTION;
        sd.material.restitution = scenario::RESTITUTION;
        sd.user_data = id as u64;
        // Mass is applied once after all shapes are added (otherwise every add recomputes: O(n^2)).
        sd.update_body_mass = false;
        sd
    }

    fn attach(&mut self, id: u32, body: BodyId, shapes: &[Shape]) -> Vec<ShapeId> {
        let sd = Self::shape_def(id);
        let out: Vec<ShapeId> = shapes
            .iter()
            .map(|s| {
                let poly = match s {
                    Shape::Box { cx, cy, hx, hy } => make_offset_box(*hx, *hy, Vec2::new(*cx, *cy), Rot::new(1.0, 0.0)),
                    Shape::Polygon { verts } => {
                        let pts: Vec<Vec2> = verts.iter().map(|v| Vec2::new(v[0], v[1])).collect();
                        let hull = compute_hull(&pts);
                        assert!(hull.count >= 3, "bad hull");
                        make_polygon(&hull, 0.0)
                    }
                };
                create_polygon_shape(&mut self.world, body, &sd, &poly)
            })
            .collect();
        body_apply_mass_from_shapes(&mut self.world, body);
        self.shape_count += out.len();
        out
    }
}

impl PhysicsEngine for Box2dEngine {
    fn name(&self) -> String {
        "box2d-rust".into()
    }

    fn add_body(&mut self, b: &BodyDef) {
        let mut bd = default_body_def();
        bd.type_ = match b.kind {
            BodyKind::Dynamic => BodyType::Dynamic,
            BodyKind::Static => BodyType::Static,
        };
        bd.position = Vec2::new(b.pos[0], b.pos[1]);
        bd.rotation = Rot::new(b.rot[0], b.rot[1]);
        bd.linear_velocity = Vec2::new(b.linvel[0], b.linvel[1]);
        bd.angular_velocity = b.angvel;
        bd.user_data = b.id as u64;
        let id = create_body(&mut self.world, &bd);
        let s = self.attach(b.id, id, &b.shapes);
        self.bodies.push(id);
        self.shapes.push(s);
    }

    fn step(&mut self) {
        world_step(&mut self.world, self.dt, self.substeps);
    }

    fn cast_rays(&mut self, rays: &[Ray], stats: &mut RayStats) {
        let filter = default_query_filter();
        for (i, r) in rays.iter().enumerate() {
            let o = Vec2::new(r.origin[0], r.origin[1]);
            let t = Vec2::new(r.dir[0] * r.max_dist, r.dir[1] * r.max_dist);
            let res = world_cast_ray_closest(&mut self.world, o, t, filter);
            if res.hit {
                let body = shape_get_user_data(&self.world, res.shape_id) as u32;
                stats.hit(i as u32, body, res.fraction);
            }
        }
    }

    fn split(&mut self, ev: &BreakEvent) {
        let pb = self.bodies[ev.parent as usize];
        let w = &mut self.world;
        let pos = body_get_position(w, pb);
        let rot = body_get_rotation(w, pb);
        let v = body_get_linear_velocity(w, pb);
        let ang = body_get_angular_velocity(w, pb);
        let c = body_get_world_center(w, pb);
        let (v, com_old) = ([v.x, v.y], [c.x, c.y]);

        let old = std::mem::take(&mut self.shapes[ev.parent as usize]);
        self.shape_count -= old.len();
        for s in old {
            destroy_shape(&mut self.world, s, false);
        }
        let new = self.attach(ev.parent, pb, &ev.parent_shapes);
        self.shapes[ev.parent as usize] = new;
        let c = body_get_world_center(&self.world, pb);
        let nv = velocity_at(v, ang, com_old, [c.x, c.y]);
        body_set_linear_velocity(&mut self.world, pb, Vec2::new(nv[0], nv[1]));
        body_set_angular_velocity(&mut self.world, pb, ang);

        for (id, shapes) in &ev.pieces {
            let mut bd = default_body_def();
            bd.type_ = BodyType::Dynamic;
            bd.position = pos;
            bd.rotation = rot;
            bd.angular_velocity = ang;
            bd.user_data = *id as u64;
            let b = create_body(&mut self.world, &bd);
            let s = self.attach(*id, b, shapes);
            let c = body_get_world_center(&self.world, b);
            let nv = velocity_at(v, ang, com_old, [c.x, c.y]);
            body_set_linear_velocity(&mut self.world, b, Vec2::new(nv[0], nv[1]));
            self.bodies.push(b);
            self.shapes.push(s);
        }
    }

    fn hash_state(&self, h: &mut StateHasher) {
        for &b in &self.bodies {
            let p = body_get_position(&self.world, b);
            let r = body_get_rotation(&self.world, b);
            let v = body_get_linear_velocity(&self.world, b);
            let w = body_get_angular_velocity(&self.world, b);
            h.body([p.x, p.y], [r.c, r.s], [v.x, v.y], w);
        }
    }

    fn body_count(&self) -> usize {
        self.bodies.len()
    }

    fn collider_count(&self) -> usize {
        self.shape_count
    }

    fn pose(&self, id: u32) -> ([f32; 2], [f32; 2]) {
        let b = self.bodies[id as usize];
        let (p, r) = (body_get_position(&self.world, b), body_get_rotation(&self.world, b));
        ([p.x, p.y], [r.c, r.s])
    }
}
