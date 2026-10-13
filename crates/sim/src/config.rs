//! Simulation-wide tuning. Part of the hashed state: both sides of a determinism check (and later all co-op
//! peers) must use the same config. All numbers are **placeholders** for feel.

use crate::hash::{StateHash, StateHasher};
use crate::vessel::{MAX_OWNERS, Owner};

/// Flight-computer tuning (see `flight.rs` and `docs/sim.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightTuning {
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
    /// Keyboard rotation (A/D held to the end of its ramp, GDR-0055) turns at this rate (rad/s), or slower if
    /// the ship could not stop from it within about a quarter turn.
    pub key_turn_rate: f32,
    /// The last resort (gimbals, differential thrust) helps turning only after the first line (pure torque,
    /// RCS couples) has been at its limit for this many ticks in a row (GDR-0055).
    pub assist_after: u32,
}

impl Default for FlightTuning {
    fn default() -> Self {
        FlightTuning {
            max_turn_rate: 6.0,
            turn_margin: 0.7,
            turn_gain: 12.0,
            stop_margin: 0.6,
            position_gain: 1.2,
            stick_deadzone: 0.05,
            keep_heading_fraction: 0.5,
            key_turn_rate: 1.5,
            assist_after: 6,
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
    /// Ramming: the impulse in a tick (kN·s) above which a contact deals damage (GDR-0047: 100, bumps below
    /// it are free).
    pub ram_threshold: f32,
    /// Ramming: damage per kN·s of impulse above `ram_threshold`, dealt to each side: to the touching part,
    /// the excess carried inward (GDR-0047).
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
            ram_threshold: 100.0,
            ram_damage: 0.02,
        }
    }
}

/// Salvage (GDR-0063; see `salvage.rs` and `docs/sim.md` "Salvage"). All numbers are **placeholders** (lead,
/// 13 Oct 2026).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SalvageTuning {
    /// Collecting runs every this many ticks (10 = 3 parts per second at most per vessel); 0 = never.
    pub interval: u32,
    /// A collecting vessel strips salvage whose hull is at most this far from its own hull (m).
    pub reach: f32,
    /// Ticks a loose fragment drifts before it is gone (1800 = 60 s).
    pub fragment_ticks: u32,
    /// Owners whose running vessels collect, by `Owner.0`: the player (owner 0) only by default; owners
    /// beyond the table never collect.
    pub collects: [bool; MAX_OWNERS],
}

impl SalvageTuning {
    /// Do this owner's running vessels collect?
    pub fn collects(&self, owner: Owner) -> bool {
        self.collects
            .get(owner.0 as usize)
            .copied()
            .unwrap_or(false)
    }
}

impl Default for SalvageTuning {
    fn default() -> Self {
        let mut collects = [false; MAX_OWNERS];
        collects[Owner::PLAYER.0 as usize] = true;
        SalvageTuning {
            interval: 10,
            reach: 5.0,
            fragment_ticks: 1800,
            collects,
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
    /// Global multiplier on the rated speed of every engine and RCS nozzle, and so on every ship's top speed
    /// (1 = content values). For feel tests.
    pub speed_scale: f32,
    /// Ticks after a split during which the ship's pieces ignore each other in collisions.
    pub split_ghost_ticks: u32,
    pub flight: FlightTuning,
    pub physics: PhysicsTuning,
    /// Mind capacity per owner (GDR-0013, GDR-0038): how many logic cores of its vessels run at once. Over
    /// it the newest vessels go dormant (GDR-0044); the focus never does. Index = `Owner.0`; owners beyond
    /// the table have no limit. **PLACEHOLDERS** (lead, 12 Oct 2026): owner 0 (the player by convention) 24
    /// cores (twelve Halvex Runners), AI owners 1000.
    pub mind_capacity: [u32; MAX_OWNERS],
    pub salvage: SalvageTuning,
}

impl SimConfig {
    /// The mind capacity of an owner (`u32::MAX` beyond the table).
    pub fn mind_capacity(&self, owner: Owner) -> u32 {
        self.mind_capacity
            .get(owner.0 as usize)
            .copied()
            .unwrap_or(u32::MAX)
    }
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            thrust_scale: 1.0,
            torque_scale: 1.0,
            speed_scale: 1.0,
            split_ghost_ticks: 10,
            flight: FlightTuning::default(),
            physics: PhysicsTuning::default(),
            mind_capacity: [24, 1000, 1000, 1000, 1000, 1000, 1000, 1000],
            salvage: SalvageTuning::default(),
        }
    }
}

impl StateHash for SimConfig {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.thrust_scale);
        h.f32(self.torque_scale);
        h.f32(self.speed_scale);
        h.u32(self.split_ghost_ticks);
        let f = &self.flight;
        for v in [
            f.max_turn_rate,
            f.turn_margin,
            f.turn_gain,
            f.stop_margin,
            f.position_gain,
            f.stick_deadzone,
            f.keep_heading_fraction,
            f.key_turn_rate,
        ] {
            h.f32(v);
        }
        h.u32(f.assist_after);
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
        for &c in &self.mind_capacity {
            h.u32(c);
        }
        let s = &self.salvage;
        h.u32(s.interval);
        h.f32(s.reach);
        h.u32(s.fragment_ticks);
        for &c in &s.collects {
            h.bool(c);
        }
    }
}
