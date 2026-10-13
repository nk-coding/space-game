//! The always-on flight computer: helm input (pilot stick and lock, a maneuver, or a direct goal) →
//! [`FlightGoal`] (heading + velocity) → heading and velocity controllers → control allocation
//! (`alloc.rs`) → per-actuator commands of the ship's controlling piece.
//!
//! * **Heading**: time-optimal-ish turn rate `ω_des = sign(e)·min(ω_max, √(2·margin·α·|e|), gain·|e|)` with
//!   `α` the piece's planning angular acceleration (pure torque + the RCS couples that lasting translation
//!   leaves free, plus differential thrust while the last resort is on), then `τ = I·(ω_des − ω)/DT`. Big ships
//!   plan with their small `α` and do not overshoot; small ones snap.
//! * **Last resort** (GDR-0055): gimbals and differential thrust push the ship sideways, so they help only
//!   after gyros, cores and RCS couples have been at their limit for `FlightTuning::assist_after` ticks
//!   ([`TurnAssist`]); never for keyboard rotation (GDR-0058).
//! * **Velocity**: dead-beat `F = m·(v_des − v)/DT` in the piece frame, clamped by what the thrusters can do
//!   at this tick's velocity (each row's rated-speed falloff, `Thruster::falloff`), so it never overshoots.
//! * **Top speed** (GDR-0041, GDR-0055): a thruster pushing with the motion stops pushing once the piece's whole
//!   speed reaches its rating, so a ship is never faster than its fastest thrusters, and diagonally no faster
//!   than the slowest group still pushing that way (`Kin::speeds`, `Kin::top_along`). No drag.
//! * **Cruise flame**: the share of each engine group's top speed that the velocity goal asks along the
//!   engine's push direction (`Actuation::cruise`), so engines held at a speed do not look dead.
//! * The plan runs on the state at the end of the last tick, so every ship sees the same world (order-free).

use core::f32::consts::{FRAC_PI_2, PI};

use crate::DT;
use crate::alloc::{allocate, assist, couples, group_of, lasting, translate, wrench_of};
use crate::config::{FlightTuning, SimConfig};
use crate::design::NO_SLOT;
use crate::hash::{StateHash, StateHasher};
use crate::ids::{PieceId, ShipId};
use crate::maneuver::Maneuver;
use crate::math::{Rot, Vec2, atan2, cos, sqrt};
use crate::physics::wrap_angle;
use crate::ship::{PartState, Piece, Ship, Wrench, gun_works, producing};

/// What a lock or a maneuver points at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// A ship: its controlling piece, else its largest piece.
    Ship(ShipId),
    Piece(ShipId, PieceId),
    Point(Vec2),
}

impl Target {
    /// No NaN or infinite number in it (commands are checked at the boundary, `Sim::command`).
    pub(crate) fn is_finite(&self) -> bool {
        match self {
            Target::Point(p) => p.is_finite(),
            Target::Ship(_) | Target::Piece(..) => true,
        }
    }
}

/// Where a target is now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetState {
    pub pos: Vec2,
    pub vel: Vec2,
}

/// Where a lock (or an aiming maneuver) points the nose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AimMode {
    /// Lead point if the ship's strongest gun (by damage per second) fires projectiles, else the target.
    #[default]
    Auto,
    /// Straight at the target (lasers hit instantly).
    Target,
    /// Where the target will be when a shot of the ship's strongest projectile gun gets there.
    Lead,
}

/// Pilot input: the touch stick (world terms) or the PC keyboard axes (ship terms), target lock, drift toggle,
/// fire.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotInput {
    /// Touch stick, world direction, length 0..1 (longer is clamped). Without a lock: "fly that way", the length
    /// sets the speed as a fraction of the top speed. With a lock: strafe that way (up to the top speed the
    /// thrusters reach that way). Ignored when `axes` is set.
    pub stick: Vec2,
    /// Keep the nose on this target.
    pub lock: Option<Target>,
    /// Released stick coasts instead of braking.
    pub drift: bool,
    pub fire: bool,
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

/// How the flight computer turns the ship.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HeadingGoal {
    /// Stop turning.
    Keep,
    /// Nose at this body angle (radians, counter-clockwise; 0 = nose along world +y).
    Angle(f32),
    /// Point the ship's best thrust axis along this world direction.
    Thrust(Vec2),
    /// Keep the heading if the ship can push along the needed acceleration from here, else turn the best
    /// thrust axis that way (flip-and-burn for ships with engines only at the back).
    Auto,
    /// Turn at this rate (rad/s, counter-clockwise; capped at the max turn rate). Keyboard rotate (up to
    /// `FlightTuning::key_turn_rate`).
    Rate(f32),
}

/// What the flight computer should achieve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightGoal {
    pub heading: HeadingGoal,
    /// Desired world velocity; `None` = coast (no translation control).
    pub velocity: Option<Vec2>,
}

impl FlightGoal {
    /// No NaN or infinite number in it (commands are checked at the boundary, `Sim::command`).
    pub(crate) fn is_finite(&self) -> bool {
        let heading = match self.heading {
            HeadingGoal::Keep | HeadingGoal::Auto => true,
            HeadingGoal::Angle(a) | HeadingGoal::Rate(a) => a.is_finite(),
            HeadingGoal::Thrust(d) => d.is_finite(),
        };
        heading && self.velocity.is_none_or(Vec2::is_finite)
    }
}

/// Who flies the ship.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HelmMode {
    /// No flight computer: engine throttles straight from `ShipControls` (tests, tools).
    Raw,
    Pilot(PilotInput),
    Maneuver {
        maneuver: Maneuver,
        fire: bool,
    },
    /// A goal set directly (tools, tests).
    Goal {
        goal: FlightGoal,
        fire: bool,
    },
}

/// What the flight computer did this tick (for HUDs and tuning).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlightTelemetry {
    /// Top speed along the best thrust axis (m/s, `Kin::top_speed`).
    pub top_speed: f32,
    pub goal: Option<FlightGoal>,
    /// The nose angle it steers to, if any.
    pub heading_target: Option<f32>,
    /// Requested force (piece frame) and torque.
    pub force: Vec2,
    pub torque: f32,
    /// What the allocation should produce at full power.
    pub expected: Wrench,
    /// The last resort (gimbals, differential thrust) was on this tick ([`TurnAssist`]).
    pub assist: bool,
}

/// `HeadingGoal::Auto` stops turning once the velocity error is below this (m/s).
const AUTO_SETTLED: f32 = 0.05;
/// A heading error above this (rad) is a turn: once on, the last resort stays on until the turn is done
/// ([`TurnAssist`]).
pub(crate) const TURNING: f32 = 0.05;

/// Last-resort turning (GDR-0055): gimbals and differential thrust push the ship sideways, so the flight
/// computer opens them only after the first line (gyros, cores, RCS couples) has been unable to give the torque
/// it asks for `FlightTuning::assist_after` ticks in a row. Once on, the last resort stays on while the first
/// line is still at its limit or the ship is still turning ([`TURNING`]), so the end of a turn planned with it
/// can use it too; then it is off again. Differential thrust itself helps only while the flight computer steers
/// to a heading, never to stop a spin without one. Keyboard rotation (`HeadingGoal::Rate`) never uses it
/// (GDR-0058): the gate is closed and reset while a key turns the ship. While it is on, the turn planning counts
/// the differential thrust in. Kept per piece (`Piece::assist`), part of the state hash.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TurnAssist {
    /// Ticks in a row the first line has been at its limit (up to `assist_after`).
    pub ticks: u32,
    pub on: bool,
}

impl TurnAssist {
    /// The state after a tick in which the first line was at its limit or not (`saturated`) while the ship was
    /// turning or not.
    pub fn step(self, saturated: bool, turning: bool, after: u32) -> TurnAssist {
        if self.on {
            if saturated || turning {
                self
            } else {
                TurnAssist::default()
            }
        } else if saturated {
            let ticks = (self.ticks + 1).min(after);
            TurnAssist {
                ticks,
                on: ticks >= after,
            }
        } else {
            TurnAssist::default()
        }
    }
}
/// Translation thrust fades in as the heading error drops from 25° (cos = ALIGN_NONE) to 8° (ALIGN_FULL)
/// while the ship turns its thrust towards the needed direction.
const ALIGN_NONE: f32 = 0.906_307_8;
const ALIGN_FULL: f32 = 0.990_268_1;

/// Angle of the body axes of the four thrust groups (forward +y, backward −y, right +x, left −x) at body angle 0.
const AXIS_ANGLE: [f32; 4] = [FRAC_PI_2, -FRAC_PI_2, 0.0, PI];

/// Kinematics and capabilities of a controlling piece, as the controllers see them.
#[derive(Clone, Copy, Debug)]
pub struct Kin {
    /// Centre of mass, world.
    pub pos: Vec2,
    pub vel: Vec2,
    pub angle: f32,
    pub omega: f32,
    pub rot: Rot,
    pub mass: f32,
    pub inertia: f32,
    /// Thrust per group (forward, backward, right, left), kN: only what can push now (`ThrustAuthority`).
    pub caps: [f32; 4],
    /// Top speed per group (m/s): where its fastest thrusters stop pushing (`ThrustAuthority::speed`).
    pub speeds: [f32; 4],
    /// Group with the most thrust (forward wins ties).
    pub best: usize,
    /// Best group's acceleration (m/s²).
    pub accel: f32,
    /// Top speed along the best group (m/s): the stick's full length, `Flee`, the cap of every maneuver.
    pub top_speed: f32,
    /// The first line's planning angular acceleration (rad/s²), from the torque sources and thrusters that can
    /// produce now: pure torque and RCS couples ([`Kin::plan_alpha`] without the last resort). Each tick the
    /// flight computer plans its turn with the couples that translation leaves free, plus differential thrust
    /// while the last resort is on.
    pub alpha: f32,
    /// Longest reach of the piece's working guns (m; 0 = none; [`crate::ship::gun_works`]).
    pub reach: f32,
    /// Shot speed of its strongest projectile gun (by damage per second), if it has one.
    pub projectile_speed: Option<f32>,
    /// Its strongest gun overall fires projectiles (`AimMode::Auto` leads).
    pub projectile_main: bool,
}

impl Kin {
    /// The kinematics of piece `p` of a ship whose part states are `parts`.
    pub fn of(p: &Piece, parts: &[PartState]) -> Kin {
        let a = &p.authority;
        let caps = [a.forward, a.backward, a.right, a.left];
        let mut best = 0;
        for g in 1..4 {
            if caps[g] > caps[best] {
                best = g;
            }
        }
        let mass = p.mass.mass.max(1e-6);
        let inertia = p.mass.inertia.max(1e-6);
        let accel = caps[best] / mass;
        let rot = Rot::from_angle(p.body.angle);
        let (mut reach, mut best_dps, mut best_proj_dps) = (0.0f32, -1.0f32, -1.0f32);
        let (mut projectile_speed, mut projectile_main) = (None, false);
        for g in p.guns.iter().filter(|g| gun_works(g, &parts[g.part as usize])) {
            reach = reach.max(g.range);
            let speed = match g.delivery {
                data::Delivery::Projectile { speed, .. } => Some(speed),
                _ => None,
            };
            if g.dps > best_dps {
                best_dps = g.dps;
                projectile_main = speed.is_some();
            }
            if let Some(v) = speed
                && g.dps > best_proj_dps
            {
                best_proj_dps = g.dps;
                projectile_speed = Some(v);
            }
        }
        Kin {
            pos: p.body.pos + rot.apply(p.mass.com),
            vel: p.body.linvel,
            angle: p.body.angle,
            omega: p.body.angvel,
            rot,
            mass,
            inertia,
            caps,
            speeds: a.speed,
            best,
            accel,
            top_speed: a.speed[best],
            alpha: Kin::plan_alpha(a, inertia, false),
            reach,
            projectile_speed,
            projectile_main,
        }
    }

    /// The angular acceleration turns are planned with (rad/s²): pure torque and RCS couples, plus differential
    /// thrust while the last resort is on (`assist`). A source that turns both ways counts with its weaker
    /// sense; one that turns only one way with half of it.
    pub fn plan_alpha(a: &crate::ship::ThrustAuthority, inertia: f32, assist: bool) -> f32 {
        Kin::alpha_with(a, [a.couple_ccw, a.couple_cw], inertia, assist)
    }

    /// [`Kin::plan_alpha`] with the RCS couple torques `couples` (counter-clockwise, clockwise) in place of the
    /// authority's: what this tick's translation leaves free of them.
    fn alpha_with(
        a: &crate::ship::ThrustAuthority,
        [couple_ccw, couple_cw]: [f32; 2],
        inertia: f32,
        assist: bool,
    ) -> f32 {
        let both = |ccw: f32, cw: f32| {
            let m = ccw.min(cw);
            if m > 0.0 { m } else { 0.5 * ccw.max(cw) }
        };
        let mut torque = a.torque_pure + both(couple_ccw, couple_cw);
        if assist {
            torque += both(a.torque_ccw, a.torque_cw);
        }
        torque / inertia.max(1e-6)
    }

    /// The point to aim the nose at for a target, by aim mode (see [`AimMode`]).
    pub fn aim_point(&self, t: &TargetState, mode: AimMode) -> Vec2 {
        let lead = match mode {
            AimMode::Target => None,
            AimMode::Lead => self.projectile_speed,
            AimMode::Auto => self.projectile_speed.filter(|_| self.projectile_main),
        };
        match lead {
            Some(speed) => lead_point(self.pos, self.vel, t.pos, t.vel, speed),
            None => t.pos,
        }
    }

    /// Body angle that points the nose (+y) along world direction `d`.
    pub fn nose_heading(d: Vec2) -> f32 {
        wrap_angle(atan2(d.y, d.x) - FRAC_PI_2)
    }

    /// Body angle that points the best thrust axis along world direction `d`.
    pub fn axis_heading(&self, d: Vec2) -> f32 {
        wrap_angle(atan2(d.y, d.x) - AXIS_ANGLE[self.best])
    }

    /// Most force the thrust groups can push along body-frame unit vector `u`.
    pub fn cap_along(&self, u: Vec2) -> f32 {
        let cx = if u.x >= 0.0 {
            self.caps[2]
        } else {
            self.caps[3]
        };
        let cy = if u.y >= 0.0 {
            self.caps[0]
        } else {
            self.caps[1]
        };
        let (ax, ay) = (u.x.abs(), u.y.abs());
        let mut c = f32::INFINITY;
        if ax > 1e-6 {
            c = c.min(cx / ax);
        }
        if ay > 1e-6 {
            c = c.min(cy / ay);
        }
        if c.is_finite() { c } else { 0.0 }
    }

    /// The fastest velocity goal along body-frame unit vector `u` (m/s) that the ship reaches from rest
    /// (GDR-0055). A thrust group pushing with the motion stops once the whole speed reaches its top speed, and
    /// the flight computer pushes each group flat out until its share of the goal is there: the group that
    /// finishes last caps the speed at its own top speed (so diagonally the slowest group pushing that way
    /// sets the cap), and the group that finishes first must get there before the whole speed passes its top
    /// speed (so a small sideways share does not cut the top speed of the main axis). Groups without thrusters
    /// are left out (that part of `u` cannot be pushed at all).
    pub fn top_along(&self, u: Vec2) -> f32 {
        let gx = if u.x >= 0.0 { 2 } else { 3 };
        let gy = if u.y >= 0.0 { 0 } else { 1 };
        // (share of u, top speed, thrust) of the two groups that push along u.
        let x = (u.x.abs(), self.speeds[gx], self.caps[gx]);
        let y = (u.y.abs(), self.speeds[gy], self.caps[gy]);
        let pushes = |g: (f32, f32, f32)| g.0 > 1e-6 && g.1 > 0.0 && g.2 > 0.0;
        match (pushes(x), pushes(y)) {
            (false, false) => 0.0,
            (true, false) => x.1 / x.0,
            (false, true) => y.1 / y.0,
            (true, true) => {
                // Time to finish ∝ share / thrust. When the first is done, the last has share × its thrust /
                // the first's thrust of the goal speed along its own axis.
                let (first, last) = if x.0 * y.2 <= y.0 * x.2 {
                    (x, y)
                } else {
                    (y, x)
                };
                let r = last.2 / first.2;
                last.1.min(first.1 / (first.0 * sqrt(1.0 + r * r)))
            }
        }
    }

    /// Deceleration the ship can plan on for stopping: the axis opposite its best one if strong enough,
    /// else half the best axis (it has to turn around first).
    pub fn stop_accel(&self) -> f32 {
        let opposite = [1, 0, 3, 2][self.best];
        let best = self.caps[self.best];
        let opp = self.caps[opposite];
        let f = if opp >= 0.5 * best { opp } else { 0.5 * best };
        f / self.mass
    }

    /// Speed to approach a point `dist` metres away so that the ship can still stop there.
    pub fn arrive_speed(&self, dist: f32, tuning: &FlightTuning) -> f32 {
        let a = tuning.stop_margin * self.stop_accel();
        let v = sqrt(2.0 * a * dist).min(tuning.position_gain * dist);
        v.min(self.top_speed)
    }
}

/// First-order intercept: the time `t > 0` at which a shot fired now at `speed` (relative to the shooter, who
/// moves with the shot's frame) meets a target at relative position `r` moving at relative velocity `v`, i.e.
/// the smallest positive root of `|r + v·t| = speed·t`. `None` if it can never catch up.
pub fn intercept_time(r: Vec2, v: Vec2, speed: f32) -> Option<f32> {
    let a = v.length_squared() - speed * speed;
    let b = 2.0 * r.dot(v);
    let c = r.length_squared();
    if a.abs() < 1e-6 {
        // Same speed: linear.
        return (b < 0.0).then(|| -c / b);
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let sq = sqrt(disc);
    let (t1, t2) = ((-b - sq) / (2.0 * a), (-b + sq) / (2.0 * a));
    let (lo, hi) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
    if lo > 0.0 {
        Some(lo)
    } else if hi > 0.0 {
        Some(hi)
    } else {
        None
    }
}

/// Where to aim so that a projectile of `speed` (on top of the shooter's velocity, as the sim fires them) meets
/// a target moving at constant velocity. Falls back to the target's position if it cannot be caught.
pub fn lead_point(
    shooter: Vec2,
    shooter_vel: Vec2,
    target: Vec2,
    target_vel: Vec2,
    speed: f32,
) -> Vec2 {
    let r = target - shooter;
    let v = target_vel - shooter_vel;
    match intercept_time(r, v, speed) {
        Some(t) => shooter + r + v * t,
        None => target,
    }
}

/// Finds a target's position and velocity now.
pub fn resolve_target(ships: &[Ship], t: &Target) -> Option<TargetState> {
    let piece_state = |p: &Piece| TargetState {
        pos: p.com_world(),
        vel: p.body.linvel,
    };
    match *t {
        Target::Point(p) => Some(TargetState {
            pos: p,
            vel: Vec2::ZERO,
        }),
        Target::Ship(id) => {
            let s = find_ship(ships, id)?;
            let p = match s.control_piece() {
                Some(c) => s.piece(c)?,
                None => s
                    .pieces()
                    .iter()
                    .max_by_key(|p| (p.parts.len(), std::cmp::Reverse(p.id)))?,
            };
            Some(piece_state(p))
        }
        Target::Piece(id, piece) => find_ship(ships, id)?.piece(piece).map(piece_state),
    }
}

pub(crate) fn find_ship(ships: &[Ship], id: ShipId) -> Option<&Ship> {
    ships
        .binary_search_by_key(&id, |s| s.id())
        .ok()
        .map(|k| &ships[k])
}

/// What the guns do this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Fire {
    Hold,
    /// Every gun fires (pilot trigger, helm `fire: true`).
    All,
    /// Fire at will at a target (behaviors): each gun fires when the target is inside its own cone along its
    /// facing and within its reach (`behavior::guns_at_will`).
    AtWill(TargetState),
}

impl Fire {
    fn all_if(fire: bool) -> Fire {
        if fire { Fire::All } else { Fire::Hold }
    }
}

/// The outcome of planning one ship (computed for all ships before any is changed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Plan {
    pub goal: Option<FlightGoal>,
    pub fire: Fire,
    pub raw: bool,
}

impl Plan {
    const DARK: Plan = Plan {
        goal: None,
        fire: Fire::Hold,
        raw: false,
    };
}

/// Pass 1: decide the goal of one ship from its helm (or behavior), reading the whole world.
pub(crate) fn plan(ship: &Ship, ships: &[Ship], cfg: &SimConfig) -> Plan {
    let Some(piece) = ship.control_piece().and_then(|c| ship.piece(c)) else {
        return Plan::DARK;
    };
    if !piece.active {
        return Plan::DARK;
    }
    let tuning = &cfg.flight;
    let kin = Kin::of(piece, &ship.parts);
    if let Some(b) = &ship.behavior {
        let (goal, fire) = crate::behavior::fly(b, &kin, ships, tuning);
        return Plan {
            goal: Some(goal),
            fire,
            raw: false,
        };
    }
    match &ship.helm {
        HelmMode::Raw => Plan {
            goal: None,
            fire: Fire::all_if(ship.controls.fire),
            raw: true,
        },
        HelmMode::Goal { goal, fire } => Plan {
            goal: Some(*goal),
            fire: Fire::all_if(*fire),
            raw: false,
        },
        HelmMode::Maneuver { maneuver, fire } => Plan {
            goal: Some(maneuver.goal(&kin, ships, tuning)),
            fire: Fire::all_if(*fire),
            raw: false,
        },
        HelmMode::Pilot(input) => Plan {
            goal: Some(pilot_goal(input, &kin, ship, piece, ships, tuning)),
            fire: Fire::all_if(input.fire),
            raw: false,
        },
    }
}

/// Body angle that brings the strongest working guns to bear on a target (GDR-0040): guns are grouped by the
/// side they face; the group with the most damage per second among working guns (clear, alive, not overheated,
/// not idle: [`gun_works`]) wins (ties: front, back, right, left), and it aims at the lead point if that group's
/// strongest gun fires projectiles. `None` without working guns.
pub fn smart_aim(ship: &Ship, piece: &Piece, kin: &Kin, t: &TargetState) -> Option<f32> {
    // [dps, best gun dps, its projectile speed (0 = hit-scan)] per facing group (forward, backward, right, left).
    let mut groups = [[0.0f32; 3]; 4];
    for g in piece.guns.iter().filter(|g| gun_works(g, ship.part(g.part))) {
        let k = if g.dir.y.abs() >= g.dir.x.abs() {
            if g.dir.y >= 0.0 { 0 } else { 1 }
        } else if g.dir.x >= 0.0 {
            2
        } else {
            3
        };
        groups[k][0] += g.dps;
        if g.dps > groups[k][1] {
            groups[k][1] = g.dps;
            groups[k][2] = match g.delivery {
                data::Delivery::Projectile { speed, .. } => speed,
                _ => 0.0,
            };
        }
    }
    let mut best = 0;
    for k in 1..4 {
        if groups[k][0] > groups[best][0] {
            best = k;
        }
    }
    if groups[best][0] <= 0.0 {
        return None;
    }
    let aim = if groups[best][2] > 0.0 {
        lead_point(kin.pos, kin.vel, t.pos, t.vel, groups[best][2])
    } else {
        t.pos
    };
    let to = aim - kin.pos;
    if to.length_squared() < 1e-6 {
        return None;
    }
    // Rotate the group's facing onto `to`.
    Some(wrap_angle(atan2(to.y, to.x) - AXIS_ANGLE[best]))
}

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
    let lock_heading = |t: &TargetState| -> HeadingGoal {
        let base = match input.aim {
            AimMode::Auto => smart_aim(ship, piece, kin, t),
            _ => None,
        };
        let base = base.or_else(|| {
            let to = kin.aim_point(t, input.aim) - kin.pos;
            (to.length_squared() > 1e-6).then(|| Kin::nose_heading(to))
        });
        match base {
            Some(a) => HeadingGoal::Angle(wrap_angle(a + input.aim_offset)),
            None => HeadingGoal::Keep,
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
    let released_velocity = if input.drift { None } else { Some(Vec2::ZERO) };
    if let Some(t) = lock {
        // Strafe: the heading belongs to the lock, so the full stick is the top speed the thrusters reach
        // along the stick from this heading.
        let heading = lock_heading(&t);
        let velocity = if pushed {
            let u = kin.rot.apply_inverse(stick) * (1.0 / stick.length());
            Some(stick * kin.top_along(u))
        } else {
            released_velocity
        };
        return FlightGoal { heading, velocity };
    }
    if !pushed {
        return FlightGoal {
            heading: if input.drift {
                HeadingGoal::Keep
            } else {
                HeadingGoal::Auto
            },
            velocity: released_velocity,
        };
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
/// computer flies the ship where it points (sideways drift is cancelled) and brakes when the keys are released;
/// with drift every key is pure thrust (momentum is kept). A/D rotate, or, under a lock (`locked` = the lock
/// heading), the aim is the lock's and A/D only nudge it (`aim_offset`).
fn keyboard_goal(
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
    } else {
        // Go where it points: the velocity goal lies along the pressed axes only (zero = brake).
        Some(nose * (thrust * along) + side * (strafe * across))
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
        // Nothing pressed and not drifting: brake as with a released stick (flip-and-burn if needed).
        None if !moving && !input.drift => HeadingGoal::Auto,
        None => HeadingGoal::Keep,
    };
    FlightGoal { heading, velocity }
}

/// Desired turn rate towards a heading error `e` (rad).
pub fn turn_rate(e: f32, alpha: f32, tuning: &FlightTuning) -> f32 {
    let mag = e.abs();
    let w = sqrt(2.0 * tuning.turn_margin * alpha * mag)
        .min(tuning.turn_gain * mag)
        .min(tuning.max_turn_rate);
    if e < 0.0 { -w } else { w }
}

/// What the RCS rows can add to the pure torque, counter-clockwise and clockwise (kN·m), while translation that
/// lasts (`alloc::lasting`: a thrust group pushing flat out, e.g. W held or a full strafe) keeps its share of
/// them: for each sense, the torque of translation's fill that turns most that way (`alloc::translate` asked for
/// the most torque of that sense, as the allocation fills when it needs it) plus the couples of that sense in the
/// room it leaves, each row at its last power level as in the authority (never below 0). A nozzle pushing the
/// ship along cannot also turn it in a couple. Without lasting translation, or without couples, the authority's
/// couples: a dead-beat correction is done within the tick and frees its nozzles for the rest of the turn.
fn free_couples(
    piece: &Piece,
    parts: &[PartState],
    force: Vec2,
    fmax: &[f32],
    full: &[f32],
    throttle: &mut Vec<f32>,
) -> [f32; 2] {
    let a = &piece.authority;
    if a.couple_ccw <= 0.0 && a.couple_cw <= 0.0 {
        return [a.couple_ccw, a.couple_cw];
    }
    let force = lasting(&piece.alloc, fmax, force);
    if force == Vec2::ZERO {
        return [a.couple_ccw, a.couple_cw];
    }
    let th = &piece.thrusters;
    let mut free = [0.0f32; 2];
    for (k, ccw) in [true, false].into_iter().enumerate() {
        throttle.clear();
        throttle.resize(th.len(), 0.0);
        let most = if ccw { f32::MAX } else { -f32::MAX };
        let made = translate(th, &piece.alloc, fmax, force, most, throttle);
        let room = |i: usize| parts[th[i].part as usize].power * (full[i] - throttle[i] * fmax[i]);
        let couple = couples(th, &piece.alloc, ccw, &room, None);
        free[k] = (if ccw { made } else { -made } + couple).max(0.0);
    }
    free
}

/// Pass 2: turn the plan into actuator commands for the controlling piece; other pieces get nothing.
pub(crate) fn execute(ship: &mut Ship, plan: &Plan, cfg: &SimConfig) {
    let control = ship.control_piece();
    let Ship {
        design,
        pieces,
        parts,
        controls,
        scratch,
        telemetry,
        ..
    } = ship;
    *telemetry = FlightTelemetry::default();
    for piece in pieces.iter_mut() {
        let rows = piece.thrusters.len();
        let sources = piece.torque_sources.len();
        piece.act.resize(rows, sources);
        piece.falloff.clear();
        piece.falloff.resize(rows, 1.0);
        if Some(piece.id) != control || !piece.active {
            piece.assist = TurnAssist::default();
            continue;
        }
        // Rated-speed falloff of every row at this tick's velocity (by the whole speed: one square root per
        // piece), for the allocation below and for the systems (raw throttles too): the same numbers both times.
        let kin = Kin::of(piece, parts);
        let v_local = kin.rot.apply_inverse(kin.vel);
        let speed = v_local.length();
        for (f, t) in piece.falloff.iter_mut().zip(&piece.thrusters) {
            *f = t.falloff(v_local, speed);
        }
        if plan.raw {
            piece.assist = TurnAssist::default();
            for (i, t) in piece.thrusters.iter().enumerate() {
                if t.slot != NO_SLOT {
                    piece.act.throttle[i] = controls
                        .throttle
                        .get(t.slot as usize)
                        .copied()
                        .unwrap_or(0.0)
                        .clamp(0.0, 1.0);
                }
            }
            continue;
        }
        match plan.fire {
            Fire::Hold | Fire::All => {
                controls.fire = plan.fire == Fire::All;
                controls.guns.clear();
            }
            Fire::AtWill(ts) => {
                controls.fire = crate::behavior::guns_at_will(
                    piece,
                    &ts,
                    design.guns.len(),
                    &mut controls.guns,
                );
            }
        }
        let Some(goal) = plan.goal else {
            piece.assist = TurnAssist::default();
            continue;
        };
        let tuning = &cfg.flight;

        // Velocity → force in the piece frame.
        let (a_world, dv_len) = match goal.velocity {
            Some(v) => {
                let dv = v - kin.vel;
                (dv * (1.0 / DT), dv.length())
            }
            None => (Vec2::ZERO, 0.0),
        };
        let force = kin.rot.apply_inverse(a_world) * kin.mass;

        // Heading → torque.
        let target = match goal.heading {
            HeadingGoal::Keep | HeadingGoal::Rate(_) => None,
            HeadingGoal::Angle(a) => Some(a),
            HeadingGoal::Thrust(d) => (d.length_squared() > 1e-12).then(|| kin.axis_heading(d)),
            HeadingGoal::Auto => {
                if dv_len < AUTO_SETTLED {
                    None
                } else {
                    let u = kin.rot.apply_inverse(a_world) * (1.0 / a_world.length());
                    if kin.cap_along(u) >= tuning.keep_heading_fraction * kin.caps[kin.best] {
                        None
                    } else {
                        Some(kin.axis_heading(a_world))
                    }
                }
            }
        };
        let error = target.map_or(0.0, |t| wrap_angle(t - kin.angle));
        // Turning the thrust towards where it is needed: burn only once roughly aligned (flip, then burn), so
        // slow-turning ships do not chase their own velocity vector.
        let force = match goal.heading {
            HeadingGoal::Thrust(_) | HeadingGoal::Auto if target.is_some() => {
                force * ((cos(error) - ALIGN_NONE) / (ALIGN_FULL - ALIGN_NONE)).clamp(0.0, 1.0)
            }
            _ => force,
        };

        // Availability now, after the speed falloff (`fmax`; `full` without it, for RCS couples): only actuators
        // that can produce (not idle, powered, not overheated, alive: `ship::producing`). Partial power is not
        // scaled in here: it caps what the piece's movers draw together, so the dead-beat request is exact
        // whenever the power suffices, and the systems settle the rest (planning, `Kin`, uses the power levels).
        let full = &mut scratch.full;
        full.clear();
        full.extend(piece.thrusters.iter().map(|t| {
            if producing(&parts[t.part as usize]) {
                t.max_force
            } else {
                0.0
            }
        }));
        let fmax = &mut scratch.fmax;
        fmax.clear();
        fmax.extend(full.iter().zip(&piece.falloff).map(|(&f, &k)| f * k));
        let tmax = &mut scratch.tmax;
        tmax.clear();
        tmax.extend(piece.torque_sources.iter().map(|s| {
            if producing(&parts[s.part as usize]) {
                s.max
            } else {
                0.0
            }
        }));

        // Heading → torque, planned with what the first line can turn with this tick: pure torque and the RCS
        // couples that translation leaves free (a nozzle pushing the ship along cannot also turn it), plus
        // differential thrust while the last resort is on. Keyboard rotation never uses the last resort
        // (GDR-0058): its gate stays closed while a key turns the ship.
        let free = free_couples(piece, parts, force, fmax, full, &mut scratch.translate);
        let key = matches!(goal.heading, HeadingGoal::Rate(_));
        if key {
            piece.assist = TurnAssist::default();
        }
        let alpha = Kin::alpha_with(&piece.authority, free, kin.inertia, piece.assist.on);
        let omega_des = match goal.heading {
            HeadingGoal::Rate(r) => r.clamp(-tuning.max_turn_rate, tuning.max_turn_rate),
            _ => turn_rate(error, alpha, tuning),
        };
        let torque = kin.inertia * (omega_des - kin.omega) * (1.0 / DT);
        // Keyboard rotation (GDR-0055) changes the turn rate only within the turn margin of what the first line
        // gives this tick in that sense (pure torque and the free couples that turn that way), so gyros, cores and
        // RCS couples serve it alone: no sideways push from the keys.
        let torque = if key {
            let couple = if torque > 0.0 { free[0] } else { free[1] };
            let cap = tuning.turn_margin * (piece.authority.torque_pure + couple);
            torque.clamp(-cap, cap)
        } else {
            torque
        };
        let missing = allocate(
            &piece.thrusters,
            &piece.alloc,
            fmax,
            full,
            tmax,
            force,
            torque,
            &mut piece.act,
        );
        // The last resort (GDR-0055): only once the first line has been at its limit for a few ticks, and
        // differential thrust only while steering to a heading (to the end of the turn, so a turn planned with it
        // can brake with it), never to stop a spin without one (`Keep`, a settled `Auto`).
        if !key {
            piece.assist =
                piece
                    .assist
                    .step(missing != 0.0, error.abs() > TURNING, tuning.assist_after);
            if piece.assist.on && missing != 0.0 {
                assist(
                    &piece.thrusters,
                    &piece.alloc,
                    fmax,
                    full,
                    missing,
                    target.is_some(),
                    &mut piece.act,
                );
            }
        }
        let expected = wrench_of(&piece.thrusters, fmax, full, tmax, &piece.act);
        // Cruise flame (GDR-0041): how much of its group's top speed the velocity goal asks along each
        // engine's push direction. Holding the stick at any speed shows it (full stick at the top speed: 1);
        // braking (goal 0) and coasting (no goal) show none.
        if let Some(v) = goal.velocity {
            let v_goal = kin.rot.apply_inverse(v);
            for (c, t) in piece.act.cruise.iter_mut().zip(&piece.thrusters) {
                let top = kin.speeds[group_of(t.dir)];
                if !t.rcs && top > 0.0 {
                    *c = (v_goal.dot(t.dir) / top).clamp(0.0, 1.0);
                }
            }
        }
        *telemetry = FlightTelemetry {
            top_speed: kin.top_speed,
            goal: Some(goal),
            heading_target: target,
            force,
            torque,
            expected,
            assist: piece.assist.on,
        };
    }
}

impl StateHash for Target {
    fn hash_state(&self, h: &mut StateHasher) {
        match *self {
            Target::Ship(s) => {
                h.u8(0);
                h.u32(s.0);
            }
            Target::Piece(s, p) => {
                h.u8(1);
                h.u32(s.0);
                h.u32(p.0);
            }
            Target::Point(p) => {
                h.u8(2);
                h.vec2(p);
            }
        }
    }
}

impl StateHash for FlightGoal {
    fn hash_state(&self, h: &mut StateHasher) {
        match self.heading {
            HeadingGoal::Keep => h.u8(0),
            HeadingGoal::Angle(a) => {
                h.u8(1);
                h.f32(a);
            }
            HeadingGoal::Thrust(d) => {
                h.u8(2);
                h.vec2(d);
            }
            HeadingGoal::Auto => h.u8(3),
            HeadingGoal::Rate(r) => {
                h.u8(4);
                h.f32(r);
            }
        }
        match self.velocity {
            Some(v) => {
                h.u8(1);
                h.vec2(v);
            }
            None => h.u8(0),
        }
    }
}

impl StateHash for HelmMode {
    fn hash_state(&self, h: &mut StateHasher) {
        match self {
            HelmMode::Raw => h.u8(0),
            HelmMode::Pilot(p) => {
                h.u8(1);
                h.vec2(p.stick);
                match &p.lock {
                    Some(t) => {
                        h.u8(1);
                        t.hash_state(h);
                    }
                    None => h.u8(0),
                }
                h.bool(p.drift);
                h.bool(p.fire);
                h.u8(p.aim as u8);
                match &p.axes {
                    Some(a) => {
                        h.u8(1);
                        h.f32(a.turn);
                        h.f32(a.thrust);
                        h.f32(a.strafe);
                    }
                    None => h.u8(0),
                }
                h.f32(p.aim_offset);
            }
            HelmMode::Maneuver { maneuver, fire } => {
                h.u8(2);
                maneuver.hash_state(h);
                h.bool(*fire);
            }
            HelmMode::Goal { goal, fire } => {
                h.u8(3);
                goal.hash_state(h);
                h.bool(*fire);
            }
        }
    }
}
