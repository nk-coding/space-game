//! Built-in maneuvers: the layer between behaviors (`behavior/`) and the flight computer. Each maneuver turns
//! into a [`FlightGoal`] every tick, never faster than the ship's top speed (GDR-0064). Ranges are
//! **placeholders**.

use crate::config::FlightTuning;
use crate::flight::{AimMode, FlightGoal, HeadingGoal, Kin, Target, TargetState, resolve_target};
use crate::hash::{StateHash, StateHasher};
use crate::math::Vec2;
use crate::ship::Ship;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Maneuver {
    /// Kill all velocity.
    Brake,
    /// Fly to a point and stay there.
    Hold { at: Vec2 },
    /// Turn the nose onto a target (or its lead point) and match its velocity.
    FaceTarget { target: Target, aim: AimMode },
    /// Close in to `range` metres, then face the target (or its lead point) and keep pace.
    Approach {
        target: Target,
        range: f32,
        aim: AimMode,
    },
    /// Circle the target at `radius`, counter-clockwise if `ccw`.
    Orbit {
        target: Target,
        radius: f32,
        ccw: bool,
    },
    /// Hold `range` metres from the target: back off when it is closer (`HeadingGoal::Auto`, so a ship that
    /// cannot thrust backwards turns away), close in like `Approach` when it is farther, face it at range.
    KeepRange {
        target: Target,
        range: f32,
        aim: AimMode,
    },
    /// Run straight away from the target at top speed.
    Flee { from: Target },
}

impl Maneuver {
    /// The target this maneuver flies relative to, if any.
    pub fn target(&self) -> Option<Target> {
        match *self {
            Maneuver::Brake | Maneuver::Hold { .. } => None,
            Maneuver::FaceTarget { target, .. }
            | Maneuver::Approach { target, .. }
            | Maneuver::Orbit { target, .. }
            | Maneuver::KeepRange { target, .. } => Some(target),
            Maneuver::Flee { from } => Some(from),
        }
    }

    /// No NaN or infinite number in it (commands are checked at the boundary, `Sim::command`).
    pub(crate) fn is_finite(&self) -> bool {
        match *self {
            Maneuver::Brake => true,
            Maneuver::Hold { at } => at.is_finite(),
            Maneuver::FaceTarget { target, .. } | Maneuver::Flee { from: target } => {
                target.is_finite()
            }
            Maneuver::Approach { target, range, .. } | Maneuver::KeepRange { target, range, .. } => {
                target.is_finite() && range.is_finite()
            }
            Maneuver::Orbit { target, radius, .. } => target.is_finite() && radius.is_finite(),
        }
    }

    /// This tick's goal. An unresolvable target (destroyed ship) brakes.
    pub fn goal(&self, kin: &Kin, ships: &[Ship], tuning: &FlightTuning) -> FlightGoal {
        let ts = self.target().and_then(|t| resolve_target(ships, &t));
        self.goal_for(kin, ts, tuning)
    }

    /// This tick's goal with the target already resolved (`ts`; ignored by maneuvers without a target).
    /// Behaviors use it to fly relative to an aim point on the target ship (e.g. its engines).
    ///
    /// The velocity goal is capped at the ship's top speed (`Kin::top_speed`, GDR-0064): matching a faster
    /// target, or an orbit around a moving one, asked for more, and a ship at its top speed then pushed a hair
    /// "with" its motion, got no thrust from the whole-speed fade and flew straight on.
    pub fn goal_for(
        &self,
        kin: &Kin,
        ts: Option<TargetState>,
        tuning: &FlightTuning,
    ) -> FlightGoal {
        let mut goal = self.uncapped(kin, ts, tuning);
        if let Some(v) = goal.velocity {
            let speed = v.length();
            if speed > kin.top_speed && kin.top_speed > 0.0 {
                goal.velocity = Some(v * (kin.top_speed / speed));
            }
        }
        goal
    }

    fn uncapped(&self, kin: &Kin, ts: Option<TargetState>, tuning: &FlightTuning) -> FlightGoal {
        let brake = FlightGoal {
            heading: HeadingGoal::Auto,
            velocity: Some(Vec2::ZERO),
        };
        match *self {
            Maneuver::Brake => brake,
            Maneuver::Hold { at } => FlightGoal {
                heading: HeadingGoal::Auto,
                velocity: Some(seek(kin, at, Vec2::ZERO, 0.0, tuning)),
            },
            Maneuver::FaceTarget { aim, .. } => match ts {
                Some(ts) => FlightGoal {
                    heading: face(kin, &ts, aim),
                    velocity: Some(ts.vel),
                },
                None => brake,
            },
            Maneuver::Approach { range, aim, .. } => match ts {
                Some(ts) => approach(kin, &ts, range, aim, tuning),
                None => brake,
            },
            Maneuver::Orbit { radius, ccw, .. } => match ts {
                Some(ts) => {
                    let r = kin.pos - ts.pos;
                    let dist = r.length();
                    let out = if dist > 1e-3 {
                        r * (1.0 / dist)
                    } else {
                        Vec2::Y
                    };
                    let tangent = if ccw { out.perp() } else { -out.perp() };
                    // Tangential speed: a share of top speed, limited by the centripetal acceleration the ship
                    // can hold.
                    let lateral = 0.5 * kin.accel * radius.max(1.0);
                    let speed = (0.6 * kin.top_speed).min(crate::math::sqrt(lateral));
                    let err = dist - radius;
                    let radial = kin.arrive_speed(err.abs(), tuning);
                    let v =
                        tangent * speed + out * (if err > 0.0 { -radial } else { radial }) + ts.vel;
                    FlightGoal {
                        heading: HeadingGoal::Auto,
                        velocity: Some(v),
                    }
                }
                None => brake,
            },
            Maneuver::KeepRange { range, aim, .. } => match ts {
                Some(ts) => {
                    let r = kin.pos - ts.pos;
                    let dist = r.length();
                    if dist < range && dist > 1e-3 {
                        let out = r * (1.0 / dist);
                        let speed = kin.arrive_speed(range - dist, tuning);
                        FlightGoal {
                            heading: HeadingGoal::Auto,
                            velocity: Some(ts.vel + out * speed),
                        }
                    } else {
                        approach(kin, &ts, range, aim, tuning)
                    }
                }
                None => brake,
            },
            Maneuver::Flee { .. } => match ts {
                Some(ts) => {
                    let r = kin.pos - ts.pos;
                    let dist = r.length();
                    let out = if dist > 1e-3 {
                        r * (1.0 / dist)
                    } else {
                        Vec2::Y
                    };
                    FlightGoal {
                        heading: HeadingGoal::Auto,
                        velocity: Some(out * kin.top_speed),
                    }
                }
                None => brake,
            },
        }
    }
}

fn approach(
    kin: &Kin,
    ts: &TargetState,
    range: f32,
    aim: AimMode,
    tuning: &FlightTuning,
) -> FlightGoal {
    let dist = (ts.pos - kin.pos).length();
    if dist > range {
        FlightGoal {
            heading: HeadingGoal::Auto,
            velocity: Some(seek(kin, ts.pos, ts.vel, range, tuning)),
        }
    } else {
        FlightGoal {
            heading: face(kin, ts, aim),
            velocity: Some(ts.vel),
        }
    }
}

/// Nose on the target (or its lead point), its bearing rate fed forward (GDR-0064).
fn face(kin: &Kin, ts: &TargetState, aim: AimMode) -> HeadingGoal {
    kin.track(kin.aim_point(ts, aim), ts.vel, 0)
        .unwrap_or(HeadingGoal::Keep)
}

/// Velocity that brings the ship to `standoff` metres from `at` (moving with `at_vel`) and stops it there.
fn seek(kin: &Kin, at: Vec2, at_vel: Vec2, standoff: f32, tuning: &FlightTuning) -> Vec2 {
    let to = at - kin.pos;
    let dist = to.length();
    let gap = dist - standoff;
    if dist < 1e-4 || gap <= 0.0 {
        return at_vel;
    }
    to * (kin.arrive_speed(gap, tuning) / dist) + at_vel
}

impl StateHash for Maneuver {
    fn hash_state(&self, h: &mut StateHasher) {
        match self {
            Maneuver::Brake => h.u8(0),
            Maneuver::Hold { at } => {
                h.u8(1);
                h.vec2(*at);
            }
            Maneuver::FaceTarget { target, aim } => {
                h.u8(2);
                target.hash_state(h);
                h.u8(*aim as u8);
            }
            Maneuver::Approach { target, range, aim } => {
                h.u8(3);
                target.hash_state(h);
                h.f32(*range);
                h.u8(*aim as u8);
            }
            Maneuver::Orbit {
                target,
                radius,
                ccw,
            } => {
                h.u8(4);
                target.hash_state(h);
                h.f32(*radius);
                h.bool(*ccw);
            }
            Maneuver::KeepRange { target, range, aim } => {
                h.u8(5);
                target.hash_state(h);
                h.f32(*range);
                h.u8(*aim as u8);
            }
            Maneuver::Flee { from } => {
                h.u8(6);
                from.hash_state(h);
            }
        }
    }
}
