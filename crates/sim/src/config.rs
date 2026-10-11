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

/// Physics, collisions and ramming (see `physics.rs` and `docs/sim.md`). All numbers are **placeholders**.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsTuning {
    /// Solver substeps per tick (Rapier's `num_solver_iterations`; 4 as benchmarked).
    pub substeps: u32,
    /// Contact friction coefficient of every hull.
    pub friction: f32,
    /// Contact restitution of every hull (0 = no bounce).
    pub restitution: f32,
    /// A body falls asleep after its fastest point stayed below this speed (m/s) for `sleep_time`; ≤ 0 =
    /// never. Kept low because nothing slows down in space: only bodies that really stopped sleep.
    pub sleep_speed: f32,
    /// Seconds below `sleep_speed` before a body sleeps.
    pub sleep_time: f32,
    /// Soft continuous collision detection: how far ahead (m per tick) contacts are predicted from the
    /// velocity, so fast pieces don't pass through thin ones (speculative contacts; 4 m = 120 m/s).
    pub ccd_prediction: f32,
    /// Contacts whose impulse in a tick stays below this (kN·s) are not reported (resting contacts).
    pub impact_min_impulse: f32,
    /// Ramming: the impulse in a tick (kN·s) above which a contact damages the touching parts.
    pub ram_threshold: f32,
    /// Ramming: damage per kN·s of impulse above `ram_threshold`, dealt to the touching part of each side.
    pub ram_damage: f32,
}

impl Default for PhysicsTuning {
    fn default() -> Self {
        PhysicsTuning {
            substeps: 4,
            friction: 0.5,
            restitution: 0.2,
            sleep_speed: 0.01,
            sleep_time: 2.0,
            ccd_prediction: 4.0,
            impact_min_impulse: 1.0,
            ram_threshold: 20.0,
            ram_damage: 0.02,
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
    pub physics: PhysicsTuning,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            thrust_scale: 1.0,
            torque_scale: 1.0,
            split_ghost_ticks: 10,
            flight: FlightTuning::default(),
            physics: PhysicsTuning::default(),
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
        let p = &self.physics;
        h.u32(p.substeps);
        for v in [
            p.friction,
            p.restitution,
            p.sleep_speed,
            p.sleep_time,
            p.ccd_prediction,
            p.impact_min_impulse,
            p.ram_threshold,
            p.ram_damage,
        ] {
            h.f32(v);
        }
    }
}
