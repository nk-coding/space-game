//! The seam between a fixed-rate simulation and the 60 fps renderer.
//!
//! The simulation (later the `sim` crate, for now the showcase's scripted motion) runs in `FixedUpdate` at
//! [`SIM_HZ`]. Each tick it calls [`SimPose::push`] with the new pose and writes per-engine throttles into
//! [`EngineThrottle`]. The renderer never reads simulation state directly: [`interpolate_poses`] blends the
//! last two ticks by `Time<Fixed>::overstep_fraction()` into the entity's `Transform` every frame.

use bevy::prelude::*;
use bevy::transform::TransformSystems;

/// Fixed simulation rate (settled: 30 Hz).
pub const SIM_HZ: f64 = 30.0;

/// A 2D pose in world metres (y up) and radians (counter-clockwise).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose2 {
    pub pos: Vec2,
    pub angle: f32,
}

impl Pose2 {
    pub fn new(pos: Vec2, angle: f32) -> Self {
        Self { pos, angle }
    }

    pub fn lerp(self, to: Pose2, t: f32) -> Pose2 {
        // shortest way round for the angle
        let mut d = (to.angle - self.angle) % std::f32::consts::TAU;
        if d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        } else if d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        Pose2 { pos: self.pos.lerp(to.pos, t), angle: self.angle + d * t }
    }
}

/// The last two simulated poses of an entity. Written by the sim bridge once per tick.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SimPose {
    pub prev: Pose2,
    pub curr: Pose2,
}

impl SimPose {
    pub fn at(p: Pose2) -> Self {
        Self { prev: p, curr: p }
    }

    /// Records a new simulation tick.
    pub fn push(&mut self, p: Pose2) {
        self.prev = self.curr;
        self.curr = p;
    }

    /// World velocity over the last tick (m/s).
    pub fn velocity(&self) -> Vec2 {
        (self.curr.pos - self.prev.pos) * SIM_HZ as f32
    }

    /// Angular velocity over the last tick (rad/s).
    pub fn angular_velocity(&self) -> f32 {
        let p = self.prev.lerp(self.curr, 1.0);
        (p.angle - self.prev.angle) * SIM_HZ as f32
    }
}

/// Engine throttles (0..1) in the order of the ship's engine mounts. Written by the sim bridge.
#[derive(Component, Clone, Debug, Default)]
pub struct EngineThrottle(pub Vec<f32>);

/// The interpolated pose of the current frame (also handy for effects that need world positions).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct RenderPose(pub Pose2);

pub struct SimBridgePlugin;

impl Plugin for SimBridgePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(SIM_HZ))
            .add_systems(PostUpdate, interpolate_poses.before(TransformSystems::Propagate));
    }
}

pub fn interpolate_poses(time: Res<Time<Fixed>>, mut q: Query<(&SimPose, &mut Transform, &mut RenderPose)>) {
    let t = time.overstep_fraction().clamp(0.0, 1.0);
    for (pose, mut tf, mut rp) in &mut q {
        let p = pose.prev.lerp(pose.curr, t);
        rp.0 = p;
        tf.translation.x = p.pos.x;
        tf.translation.y = p.pos.y;
        tf.rotation = Quat::from_rotation_z(p.angle);
    }
}
