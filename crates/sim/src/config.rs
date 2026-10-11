//! Simulation-wide tuning. Part of the hashed state: both sides of a determinism check (and later all co-op
//! peers) must use the same config. All numbers are **placeholders** for feel.

use crate::hash::{StateHash, StateHasher};

/// Flight-computer tuning (see `flight.rs` and `docs/sim.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightTuning {
    /// Soft top speed: `v_top = top_speed_k × sqrt(a)`, `a` = the best thrust axis's force / mass
    /// (m/s per √(m/s²)). Big, sluggish ships get a lower top speed and take longer to reach it.
    pub top_speed_k: f32,
    /// Hard cap on the commanded turn rate (rad/s).
    pub max_turn_rate: f32,
    /// Fraction of the angular acceleration the heading controller plans with when braking a turn (< 1 avoids
    /// overshoot).
    pub turn_margin: f32,
    /// Gain of the heading controller's linear zone near the target (1/s).
    pub turn_gain: f32,
    /// Fraction of the deceleration the position controller plans with when stopping.
    pub stop_margin: f32,
    /// Gain of the position controller's linear zone near the goal (1/s).
    pub position_gain: f32,
    /// Stick inputs shorter than this count as released.
    pub stick_deadzone: f32,
    /// `HeadingGoal::Auto` keeps the current heading if the ship can push along the needed direction with at
    /// least this fraction of its best axis; otherwise it turns its best axis that way.
    pub keep_heading_fraction: f32,
}

impl Default for FlightTuning {
    fn default() -> Self {
        FlightTuning {
            top_speed_k: 12.0,
            max_turn_rate: 3.0,
            turn_margin: 0.7,
            turn_gain: 8.0,
            stop_margin: 0.6,
            position_gain: 1.2,
            stick_deadzone: 0.05,
            keep_heading_fraction: 0.5,
        }
    }
}

/// Simulation-wide settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimConfig {
    /// Global multiplier on all engine and RCS thrust (1 = content values). For feel tests.
    pub thrust_scale: f32,
    /// Global multiplier on all pure torque (gyros, logic cores; 1 = content values). For feel tests.
    pub torque_scale: f32,
    /// Ticks after a split during which the ship's pieces ignore each other in collisions.
    pub split_ghost_ticks: u32,
    pub flight: FlightTuning,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            thrust_scale: 1.0,
            torque_scale: 1.0,
            split_ghost_ticks: 10,
            flight: FlightTuning::default(),
        }
    }
}

impl StateHash for SimConfig {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.thrust_scale);
        h.f32(self.torque_scale);
        h.u32(self.split_ghost_ticks);
        let f = &self.flight;
        for v in [
            f.top_speed_k,
            f.max_turn_rate,
            f.turn_margin,
            f.turn_gain,
            f.stop_margin,
            f.position_gain,
            f.stick_deadzone,
            f.keep_heading_fraction,
        ] {
            h.f32(v);
        }
    }
}
