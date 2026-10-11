//! The common interface every engine adapter implements.

use crate::hash::StateHasher;
use crate::scenario::{BodyDef, BreakEvent, Ray};

/// Per-run engine configuration. Every engine gets the same values.
#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub dt: f32,
    pub substeps: u32,
    /// 1 = single thread. >1 = engine's own multithreading with that many worker threads (if supported).
    pub threads: u32,
    /// Engine's default sleeping on (true, the benchmark default) or disabled for every body.
    pub sleep: bool,
}

/// Accumulates raycast results so the work can't be optimised away and so query determinism can be checked.
#[derive(Clone, Debug, Default)]
pub struct RayStats {
    pub rays: u64,
    pub hits: u64,
    pub hasher: StateHasher,
}

impl RayStats {
    /// Record a closest hit: ray index, hit body id and the engine's raw hit parameter (toi / fraction).
    #[inline]
    pub fn hit(&mut self, ray_index: u32, body_id: u32, param: f32) {
        self.hits += 1;
        self.hasher.u32(ray_index);
        self.hasher.u32(body_id);
        self.hasher.f32(param);
    }
}

/// Engine snapshot of one body, used for velocity hand-over in splits.
#[derive(Clone, Copy, Debug)]
pub struct BodyState {
    pub pos: [f32; 2],
    pub rot: [f32; 2],
    pub linvel: [f32; 2],
    pub angvel: f32,
    /// World-space centre of mass.
    pub com: [f32; 2],
}

pub trait PhysicsEngine {
    /// Short engine id, e.g. "rapier".
    fn name(&self) -> String;
    /// Add a body. `body.id` equals the number of bodies added so far (stable order for hashing).
    fn add_body(&mut self, body: &BodyDef);
    /// Advance one fixed step.
    fn step(&mut self);
    /// Closest-hit raycasts against all colliders.
    fn cast_rays(&mut self, rays: &[Ray], stats: &mut RayStats);
    /// Apply a breakage event (see `BreakEvent`). Must hand the parent's velocity field to every piece:
    /// linvel(piece) = v_parent_com + w x (com_piece - com_parent), angvel(piece) = w; the parent keeps its
    /// rigid motion too (its linvel is corrected for the shift of its centre of mass).
    fn split(&mut self, ev: &BreakEvent);
    /// Hash every body in id order: position, rotation (cos, sin), linear velocity, angular velocity (raw bits).
    fn hash_state(&self, h: &mut StateHasher);
    fn body_count(&self) -> usize;
    fn collider_count(&self) -> usize;
    /// Pose of one body: (position, (cos, sin)). Debug/visual checks only (not timed).
    fn pose(&self, id: u32) -> ([f32; 2], [f32; 2]);
}

/// Helper for adapters: velocity at a point of a rigid motion.
#[inline]
pub fn velocity_at(linvel_com: [f32; 2], angvel: f32, com: [f32; 2], p: [f32; 2]) -> [f32; 2] {
    let rx = p[0] - com[0];
    let ry = p[1] - com[1];
    [linvel_com[0] - angvel * ry, linvel_com[1] + angvel * rx]
}

/// Time source injected by the runner (native: Instant, wasm: performance.now()). Milliseconds.
pub trait Clock {
    fn now_ms(&self) -> f64;
}
