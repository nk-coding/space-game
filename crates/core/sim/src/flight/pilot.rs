//! The pilot's input (stick and lock, keys with the nudge and the turn ramp) and the goal it asks of the flight
//! computer.

use super::*;

/// Pilot input → goal: the touch stick ("fly that way") or the keyboard axes (rotate / thrust / strafe), with or
/// without a lock.
pub fn pilot_goal(
    input: &PilotInput,
    kin: &Kin,
    ship: &Ship,
    piece: &Piece,
    ships: &[Ship],
    tuning: &FlightTuning,
) -> FlightGoal {
    let lock = input.lock.as_ref().and_then(|t| resolve_target(ships, t));
    // The lock follows the target's bearing (fed forward, GDR-0064); the A/D nudge offsets the angle only.
    let lock_heading = |t: &TargetState| -> HeadingGoal {
        let base = match input.aim {
            AimMode::Auto => smart_aim(ship, piece, kin, t),
            _ => None,
        };
        match base.or_else(|| kin.track(kin.aim_point(t, input.aim), t.vel, 0)) {
            Some(HeadingGoal::Track { angle, rate }) => HeadingGoal::Track {
                angle: wrap_angle(angle + input.aim_offset),
                rate,
            },
            _ => HeadingGoal::Keep,
        }
    };
    if let Some(axes) = input.axes {
        return keyboard_goal(input, &axes, kin, lock.as_ref().map(lock_heading), tuning);
    }
    let mut stick = input.stick;
    let len = stick.length();
    if len > 1.0 {
        stick = stick * (1.0 / len);
    }
    let pushed = len > tuning.stick_deadzone;
    if let Some(t) = lock {
        // Strafe: the heading belongs to the lock, so the full stick is the top speed the thrusters reach
        // along the stick from this heading.
        let heading = lock_heading(&t);
        let velocity = if pushed {
            let u = kin.rot.apply_inverse(stick) * (1.0 / stick.length());
            Some(stick * kin.top_along(u))
        } else {
            None
        };
        return FlightGoal { heading, velocity };
    }
    // A released stick coasts and keeps the heading (GDR-0086: no braking on release, no flip-and-burn).
    if !pushed {
        return FlightGoal { heading: HeadingGoal::Keep, velocity: None };
    }
    // Fly that way: the ship turns its best axis onto the stick, so the full stick is that axis's top speed.
    // For choosing the heading, a small overspeed along the stick (up to 10% of top speed, e.g. after a push)
    // is ignored, so a ship with engines only at the back does not flip around for it; the velocity goal stays
    // exact, so whatever can brake brakes.
    let v_des = stick * kin.top_speed;
    let s_dir = stick * (1.0 / stick.length());
    let mut dv = v_des - kin.vel;
    let along = dv.dot(s_dir);
    if along < 0.0 && -along < 0.1 * kin.top_speed {
        dv -= s_dir * along;
    }
    // Point the thrust along the needed change of velocity while it is large, and blend back to the stick
    // direction as it shrinks (w ∝ |dv|): the nose settles exactly on the stick once the velocity has arrived,
    // and small sideways errors still get some thrust along them.
    let dv_len = dv.length();
    let w = if kin.top_speed > 0.0 {
        (dv_len / (0.1 * kin.top_speed)).min(1.0)
    } else {
        0.0
    };
    let mut h = s_dir * (1.0 - w);
    if dv_len > 1e-6 {
        h += dv * (w / dv_len);
    }
    if h.length_squared() < 1e-9 {
        h = s_dir;
    }
    FlightGoal {
        heading: HeadingGoal::Thrust(h),
        velocity: Some(v_des),
    }
}

/// Keyboard axes → goal (GDR-0040). W/S thrust along the nose and Q/E strafe; without drift the flight
/// computer flies the ship where it points (sideways drift is cancelled) while keys are held; released keys
/// coast with the heading kept (GDR-0086); with drift every key is pure thrust (momentum is kept). A/D rotate, or, under a lock (`locked` = the lock
/// heading), the aim is the lock's and A/D only nudge it (`aim_offset`).
pub(super) fn keyboard_goal(
    input: &PilotInput,
    axes: &PilotAxes,
    kin: &Kin,
    locked: Option<HeadingGoal>,
    tuning: &FlightTuning,
) -> FlightGoal {
    let dz = |v: f32| if v.abs() < tuning.stick_deadzone { 0.0 } else { v.clamp(-1.0, 1.0) };
    let (turn, thrust, strafe) = (dz(axes.turn), dz(axes.thrust), dz(axes.strafe));
    let nose = kin.rot.apply(Vec2::new(0.0, 1.0));
    let side = kin.rot.apply(Vec2::new(1.0, 0.0));
    // Each key's full press is the top speed of the thrusters that push that way.
    let along = if thrust >= 0.0 {
        kin.speeds[0]
    } else {
        kin.speeds[1]
    };
    let across = if strafe >= 0.0 {
        kin.speeds[2]
    } else {
        kin.speeds[3]
    };
    let moving = thrust != 0.0 || strafe != 0.0;
    let velocity = if input.drift {
        // Pure thrust along the pressed axes up to their top speed; nothing else is corrected.
        if moving {
            let push = |key: f32, top: f32, now: f32| {
                let want = key * top;
                if key > 0.0 {
                    (want - now).max(0.0)
                } else if key < 0.0 {
                    (want - now).min(0.0)
                } else {
                    0.0
                }
            };
            Some(
                kin.vel
                    + nose * push(thrust, along, kin.vel.dot(nose))
                    + side * push(strafe, across, kin.vel.dot(side)),
            )
        } else {
            None
        }
    } else if moving {
        // Go where it points: the velocity goal lies along the pressed axes only.
        Some(nose * (thrust * along) + side * (strafe * across))
    } else {
        // Released keys coast (GDR-0086).
        None
    };
    let heading = match locked {
        Some(h) => h,
        None if turn != 0.0 => {
            // The key turn rate (GDR-0055; the client ramps `turn` up while A/D are held), or slower if the ship
            // could not stop from it within about a quarter turn on its first line (`Kin::alpha`: never the last
            // resort, GDR-0058).
            let rate = tuning.key_turn_rate.min(sqrt(tuning.turn_margin * kin.alpha * PI));
            HeadingGoal::Rate(turn * rate)
        }
        None => HeadingGoal::Keep,
    };
    FlightGoal { heading, velocity }
}

/// Pilot input: the touch stick (world terms) or the PC keyboard axes (ship terms), target lock, drift toggle,
/// the fire groups.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotInput {
    /// Touch stick, world direction, length 0..1 (longer is clamped). Without a lock: "fly that way", the length
    /// sets the speed as a fraction of the top speed. With a lock: strafe that way (up to the top speed the
    /// thrusters reach that way). Ignored when `axes` is set.
    pub stick: Vec2,
    /// Keep the nose on this target.
    pub lock: Option<Target>,
    /// Pure thrust while pushing: no drift cancellation, momentum kept (released input always coasts, GDR-0086).
    pub drift: bool,
    /// The weapon groups whose triggers are held (GDR-0088): bit g-1 for group g, [`FIRE_ALL`] for every gun.
    pub fire: u8,
    /// Under a lock: which way to aim (`Auto`: bring the strongest working guns to bear, GDR-0040).
    pub aim: AimMode,
    /// PC keyboard (GDR-0040): ship-relative rotate / thrust / strafe. `Some` replaces the stick.
    pub axes: Option<PilotAxes>,
    /// Under a lock: extra aim angle (rad, counter-clockwise), the A/D nudge. The client integrates it every tick
    /// with [`nudge_step`], so it springs back when released.
    pub aim_offset: f32,
}

impl PilotInput {
    /// No NaN or infinite number in it (commands are checked at the boundary, `Sim::command`).
    pub(crate) fn is_finite(&self) -> bool {
        self.stick.is_finite()
            && self.aim_offset.is_finite()
            && self.lock.is_none_or(|t| t.is_finite())
            && self.axes.is_none_or(|a| {
                a.turn.is_finite() && a.thrust.is_finite() && a.strafe.is_finite()
            })
    }
}

/// Every weapon group's trigger ([`PilotInput::fire`]).
pub const FIRE_ALL: u8 = (1 << data::MAX_WEAPON_GROUP) - 1;

/// Ship-relative pilot axes (PC keyboard), each -1..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotAxes {
    /// A/D: +1 turns counter-clockwise (left) at the key turn rate, -1 clockwise, between = that share of it.
    /// The client ramps a held key up with [`turn_ramp_step`]. Under a lock the A/D keys drive the aim nudge
    /// instead and this is ignored.
    pub turn: f32,
    /// W/S: +1 thrusts forward along the nose, -1 backward.
    pub thrust: f32,
    /// Q/E: +1 strafes right (the ship's +x), -1 left.
    pub strafe: f32,
}

/// Aim nudge (rad) reached while A/D are held under a lock, how fast it grows (rad/s) and how fast it springs
/// back (1/s). Placeholders for feel.
pub const NUDGE_MAX: f32 = 0.6;
pub const NUDGE_RATE: f32 = 1.2;
pub const NUDGE_RETURN: f32 = 5.0;

/// One tick of the aim nudge: grows with `turn` (A/D) up to ±[`NUDGE_MAX`], springs back to 0 when released.
/// Deterministic (no transcendentals); the client calls it once per sim tick.
pub fn nudge_step(nudge: f32, turn: f32) -> f32 {
    let turn = turn.clamp(-1.0, 1.0);
    if turn.abs() > 0.05 {
        (nudge + turn * NUDGE_RATE * DT).clamp(-NUDGE_MAX, NUDGE_MAX)
    } else {
        let k = (NUDGE_RETURN * DT).min(1.0);
        let n = nudge - nudge * k;
        if n.abs() < 1e-4 { 0.0 } else { n }
    }
}

/// Keyboard rotation ramp (GDR-0055): a fresh A/D press starts at this share of the key turn rate
/// (`FlightTuning::key_turn_rate`) and grows by `KEY_TURN_GROWTH` per second while held, so a tap nudges the nose
/// a little and holding turns faster up to the full rate (0.2 → 1 in 0.8 s). The growth stays below what the
/// Halvex Runner's gyros and cores follow (1.5 rad/s² of the 0.7 × 2.2 the flight computer allows a key turn),
/// so its held key turns at the ramp's rate. Placeholders for feel.
pub const KEY_TURN_START: f32 = 0.2;
pub const KEY_TURN_GROWTH: f32 = 1.0;

/// One tick of the keyboard rotation ramp: `key` is the A/D axis (-1..1), `level` the last tick's result, which
/// the client sends as `PilotAxes::turn`. A press (or a change of direction) starts at ±[`KEY_TURN_START`], a held
/// key grows by [`KEY_TURN_GROWTH`]·DT per tick up to `|key|`, a release drops to 0. Deterministic (no
/// transcendentals); the client calls it once per sim tick, like [`nudge_step`].
pub fn turn_ramp_step(level: f32, key: f32) -> f32 {
    let key = key.clamp(-1.0, 1.0);
    if key.abs() < 0.05 {
        return 0.0;
    }
    let grown = if level * key > 0.0 {
        level.abs() + KEY_TURN_GROWTH * DT
    } else {
        KEY_TURN_START
    };
    let m = grown.min(key.abs());
    if key > 0.0 { m } else { -m }
}
