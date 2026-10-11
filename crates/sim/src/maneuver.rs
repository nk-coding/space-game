//! Built-in maneuvers: the layer between behaviors (`behavior.rs`) and the flight computer. Each maneuver turns
//! into a [`FlightGoal`] every tick. Plus the old hard-coded combat brain (approach → attack → orbit, aimed
//! with lead), kept for API stability; the content behavior `raider_basic` replaces it. Ranges and timings are
//! **placeholders**.

use crate::config::FlightTuning;
use crate::flight::{AimMode, FlightGoal, HeadingGoal, Kin, Target, TargetState, resolve_target};
use crate::hash::{StateHash, StateHasher};
use crate::ids::ShipId;
use crate::math::Vec2;
use crate::physics::wrap_angle;
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

    /// This tick's goal. An unresolvable target (destroyed ship) brakes.
    pub fn goal(&self, kin: &Kin, ships: &[Ship], tuning: &FlightTuning) -> FlightGoal {
        let ts = self.target().and_then(|t| resolve_target(ships, &t));
        self.goal_for(kin, ts, tuning)
    }

    /// This tick's goal with the target already resolved (`ts`; ignored by maneuvers without a target).
    /// Behaviors use it to fly relative to an aim point on the target ship (e.g. its engines).
    pub fn goal_for(
        &self,
        kin: &Kin,
        ts: Option<TargetState>,
        tuning: &FlightTuning,
    ) -> FlightGoal {
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

fn face(kin: &Kin, ts: &TargetState, aim: AimMode) -> HeadingGoal {
    let to = kin.aim_point(ts, aim) - kin.pos;
    if to.length_squared() > 1e-6 {
        HeadingGoal::Angle(Kin::nose_heading(to))
    } else {
        HeadingGoal::Keep
    }
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

/// Placeholder numbers of the combat brain. Distances scale with the ship's reach (the longest range of its
/// clear guns), so the same brain works for raiders (cannon, 75 m) and runners (pulse lasers, 60 m).
pub mod raider {
    /// Beyond `reach × ENGAGE`, the ship approaches.
    pub const ENGAGE: f32 = 1.2;
    /// Attack stand-off: `reach × STANDOFF` (a raider: 50 m, outside the beam lasers' 45 m).
    pub const STANDOFF: f32 = 0.667;
    pub const ORBIT: f32 = 0.6;
    /// Fires when the target is within `reach × FIRE` and the aim point within `FIRE_CONE` of the nose.
    pub const FIRE: f32 = 0.933;
    pub const FIRE_CONE: f32 = 0.12;
    /// Reach assumed for a ship without clear guns.
    pub const DEFAULT_REACH: f32 = 60.0;
    pub const ATTACK_TICKS: u32 = 150;
    pub const ORBIT_TICKS: u32 = 120;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaiderPhase {
    Seek,
    Attack,
    Orbit,
}

/// The trivial combat AI (named for the automaton raiders, works for any armed ship): pick the nearest active
/// ship of another faction, approach it, then alternate between attacking from stand-off range and orbiting;
/// aim with lead for projectile guns (`AimMode::Auto`) and fire whenever the aim point is on the nose. No
/// randomness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaiderBrain {
    pub target: Option<ShipId>,
    pub phase: RaiderPhase,
    pub phase_ticks: u32,
}

impl Default for RaiderBrain {
    fn default() -> Self {
        RaiderBrain {
            target: None,
            phase: RaiderPhase::Seek,
            phase_ticks: 0,
        }
    }
}

impl RaiderBrain {
    /// Next brain state, the maneuver to fly and whether to fire.
    pub(crate) fn think(
        &self,
        me: &Ship,
        kin: &Kin,
        ships: &[Ship],
    ) -> (RaiderBrain, Maneuver, bool) {
        use raider::*;
        let faction = &me.design().faction;
        let mut best: Option<(f32, ShipId, TargetState)> = None;
        for s in ships {
            if s.id() == me.id() || &s.design().faction == faction {
                continue;
            }
            let Some(p) = s.control_piece().and_then(|c| s.piece(c)) else {
                continue;
            };
            if !p.active {
                continue;
            }
            let ts = TargetState {
                pos: p.com_world(),
                vel: p.body.linvel,
            };
            let d = (ts.pos - kin.pos).length();
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, s.id(), ts));
            }
        }
        let Some((dist, target, ts)) = best else {
            return (RaiderBrain::default(), Maneuver::Brake, false);
        };
        let ticks = if self.target == Some(target) {
            self.phase_ticks + 1
        } else {
            0
        };
        let mut next = RaiderBrain {
            target: Some(target),
            phase: self.phase,
            phase_ticks: ticks,
        };
        let switch = |phase| RaiderBrain {
            target: Some(target),
            phase,
            phase_ticks: 0,
        };
        let reach = if kin.reach > 0.0 {
            kin.reach
        } else {
            DEFAULT_REACH
        };
        if dist > reach * ENGAGE {
            if self.phase != RaiderPhase::Seek {
                next = switch(RaiderPhase::Seek);
            }
        } else {
            match self.phase {
                RaiderPhase::Seek => next = switch(RaiderPhase::Attack),
                RaiderPhase::Attack if ticks >= ATTACK_TICKS => next = switch(RaiderPhase::Orbit),
                RaiderPhase::Orbit if ticks >= ORBIT_TICKS => next = switch(RaiderPhase::Attack),
                _ => {}
            }
        }
        let t = Target::Ship(target);
        let maneuver = match next.phase {
            RaiderPhase::Seek | RaiderPhase::Attack => Maneuver::Approach {
                target: t,
                range: reach * STANDOFF,
                aim: AimMode::Auto,
            },
            RaiderPhase::Orbit => Maneuver::Orbit {
                target: t,
                radius: reach * ORBIT,
                ccw: me.id().0.is_multiple_of(2),
            },
        };
        let aim = kin.aim_point(&ts, AimMode::Auto);
        let off_nose = wrap_angle(Kin::nose_heading(aim - kin.pos) - kin.angle).abs();
        let fire = dist < reach * FIRE && off_nose < FIRE_CONE;
        (next, maneuver, fire)
    }
}

impl StateHash for RaiderBrain {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.target.map_or(u32::MAX, |t| t.0));
        h.u8(self.phase as u8);
        h.u32(self.phase_ticks);
    }
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
