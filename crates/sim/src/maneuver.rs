//! Built-in maneuvers: the layer between behavior trees (later) and the flight computer. Each maneuver turns
//! into a [`FlightGoal`] every tick. Plus a tiny hard-coded raider brain (approach → attack → orbit) until
//! behavior trees exist. Ranges and timings are **placeholders**.

use crate::config::FlightTuning;
use crate::flight::{FlightGoal, HeadingGoal, Kin, Target, resolve_target};
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
    /// Turn the nose onto a target and match its velocity.
    FaceTarget(Target),
    /// Close in to `range` metres, then face the target and keep pace.
    Approach { target: Target, range: f32 },
    /// Circle the target at `radius`, counter-clockwise if `ccw`.
    Orbit {
        target: Target,
        radius: f32,
        ccw: bool,
    },
}

impl Maneuver {
    /// This tick's goal. An unresolvable target (destroyed ship) brakes.
    pub fn goal(&self, kin: &Kin, ships: &[Ship], tuning: &FlightTuning) -> FlightGoal {
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
            Maneuver::FaceTarget(t) => match resolve_target(ships, &t) {
                Some(ts) => FlightGoal {
                    heading: face(kin, ts.pos),
                    velocity: Some(ts.vel),
                },
                None => brake,
            },
            Maneuver::Approach { target, range } => match resolve_target(ships, &target) {
                Some(ts) => {
                    let dist = (ts.pos - kin.pos).length();
                    if dist > range {
                        FlightGoal {
                            heading: HeadingGoal::Auto,
                            velocity: Some(seek(kin, ts.pos, ts.vel, range, tuning)),
                        }
                    } else {
                        FlightGoal {
                            heading: face(kin, ts.pos),
                            velocity: Some(ts.vel),
                        }
                    }
                }
                None => brake,
            },
            Maneuver::Orbit {
                target,
                radius,
                ccw,
            } => match resolve_target(ships, &target) {
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
        }
    }
}

fn face(kin: &Kin, at: Vec2) -> HeadingGoal {
    let to = at - kin.pos;
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

/// Placeholder numbers of the raider brain.
pub mod raider {
    /// Beyond this, the raider approaches.
    pub const ENGAGE_RANGE: f32 = 90.0;
    /// Attack stand-off distance: outside the beam lasers' reach (45 m), inside its cannon's (75 m).
    pub const ATTACK_RANGE: f32 = 50.0;
    pub const ORBIT_RADIUS: f32 = 45.0;
    /// Fires when the target is closer than this and within `FIRE_CONE` of the nose.
    pub const FIRE_RANGE: f32 = 70.0;
    pub const FIRE_CONE: f32 = 0.12;
    pub const ATTACK_TICKS: u32 = 150;
    pub const ORBIT_TICKS: u32 = 120;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaiderPhase {
    Seek,
    Attack,
    Orbit,
}

/// The trivial AI of automaton raiders: pick the nearest active enemy, approach it, then alternate between
/// attacking from stand-off range and orbiting; fire whenever the nose is on it. No randomness.
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
        let mut best: Option<(f32, ShipId, Vec2)> = None;
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
            let pos = p.com_world();
            let d = (pos - kin.pos).length();
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, s.id(), pos));
            }
        }
        let Some((dist, target, pos)) = best else {
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
        if dist > ENGAGE_RANGE {
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
                range: ATTACK_RANGE,
            },
            RaiderPhase::Orbit => Maneuver::Orbit {
                target: t,
                radius: ORBIT_RADIUS,
                ccw: me.id().0.is_multiple_of(2),
            },
        };
        let off_nose = wrap_angle(Kin::nose_heading(pos - kin.pos) - kin.angle).abs();
        let fire = dist < FIRE_RANGE && off_nose < FIRE_CONE;
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
            Maneuver::FaceTarget(t) => {
                h.u8(2);
                t.hash_state(h);
            }
            Maneuver::Approach { target, range } => {
                h.u8(3);
                target.hash_state(h);
                h.f32(*range);
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
        }
    }
}
