//! The always-on flight computer: helm input (pilot stick and lock, a maneuver, or a direct goal) →
//! [`FlightGoal`] (heading + velocity) → heading and velocity controllers → control allocation
//! (`alloc.rs`) → per-actuator commands of the ship's controlling piece.
//!
//! * **Heading**: time-optimal-ish turn rate `ω_des = sign(e)·min(ω_max, √(2·margin·α·|e|), gain·|e|)` with
//!   `α` the piece's planning angular acceleration (pure torque + differential thrust), then
//!   `τ = I·(ω_des − ω)/DT`. Big ships plan with their small `α` and do not overshoot; small ones snap.
//! * **Velocity**: dead-beat `F = m·(v_des − v)/DT` in the piece frame, clamped by what the thrusters can do
//!   at this tick's velocity (each row's rated-speed falloff, `ship::rated_falloff`), so it never overshoots.
//! * **Top speed** (GDR-0041): each thrust group stops pushing at the rated speed of its fastest thrusters, so
//!   the reachable velocities form a box in the piece frame (`Kin::speeds`, `Kin::top_along`). No drag.
//! * **Cruise flame**: the share of each engine group's top speed that the velocity goal asks along the
//!   engine's push direction (`Actuation::cruise`), so engines held at a speed do not look dead.
//! * The plan runs on the state at the end of the last tick, so every ship sees the same world (order-free).

use core::f32::consts::{FRAC_PI_2, PI};

use crate::DT;
use crate::alloc::{allocate, group_of};
use crate::config::{FlightTuning, SimConfig};
use crate::design::NO_SLOT;
use crate::hash::{StateHash, StateHasher};
use crate::ids::{PieceId, ShipId};
use crate::maneuver::Maneuver;
use crate::math::{Rot, Vec2, atan2, cos, sqrt};
use crate::physics::wrap_angle;
use crate::ship::{Piece, Ship, Wrench};

/// What a lock or a maneuver points at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// A ship: its controlling piece, else its largest piece.
    Ship(ShipId),
    Piece(ShipId, PieceId),
    Point(Vec2),
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

/// Ship-relative pilot axes (PC keyboard), each -1..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotAxes {
    /// A/D: +1 turns counter-clockwise (left), -1 clockwise. Under a lock it drives the aim nudge instead.
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
    /// Turn at this rate (rad/s, counter-clockwise; capped at the max turn rate). Keyboard rotate.
    Rate(f32),
}

/// What the flight computer should achieve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightGoal {
    pub heading: HeadingGoal,
    /// Desired world velocity; `None` = coast (no translation control).
    pub velocity: Option<Vec2>,
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
}

/// `HeadingGoal::Auto` stops turning once the velocity error is below this (m/s).
const AUTO_SETTLED: f32 = 0.05;
/// Differential thrust (which pushes the ship) is used for turning only above this heading error (rad); below
/// it (holding a heading, stopping a spin), gyros, cores and gimbals do the work. Every active piece has a
/// core, and cores have reaction torque, so there is always some.
const TURN_WITH_THRUST: f32 = 0.05;
/// Translation thrust fades in as the heading error drops from 25° (cos = ALIGN_NONE) to 8° (ALIGN_FULL)
/// while the ship turns its thrust towards the needed direction.
const ALIGN_NONE: f32 = 0.906_307_8;
const ALIGN_FULL: f32 = 0.990_268_1;

/// Body axes of the four thrust groups and their angle at body angle 0.
pub const AXES: [Vec2; 4] = [
    Vec2::new(0.0, 1.0),
    Vec2::new(0.0, -1.0),
    Vec2::new(1.0, 0.0),
    Vec2::new(-1.0, 0.0),
];
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
    /// Thrust per group (forward, backward, right, left), kN.
    pub caps: [f32; 4],
    /// Top speed per group (m/s): where its fastest thrusters stop pushing (`ThrustAuthority::speed`).
    pub speeds: [f32; 4],
    /// Group with the most thrust (forward wins ties).
    pub best: usize,
    /// Best group's acceleration (m/s²).
    pub accel: f32,
    /// Top speed along the best group (m/s): the stick's full length, `Flee`, the cap of every maneuver.
    pub top_speed: f32,
    /// Planning angular acceleration (rad/s²).
    pub alpha: f32,
    /// Longest reach of the piece's clear guns (m; 0 = no guns).
    pub reach: f32,
    /// Shot speed of its strongest projectile gun (by damage per second), if it has one.
    pub projectile_speed: Option<f32>,
    /// Its strongest gun overall fires projectiles (`AimMode::Auto` leads).
    pub projectile_main: bool,
}

impl Kin {
    pub fn of(p: &Piece) -> Kin {
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
        let diff = a.torque_ccw.min(a.torque_cw);
        let diff = if diff > 0.0 {
            diff
        } else {
            0.5 * a.torque_ccw.max(a.torque_cw)
        };
        let rot = Rot::from_angle(p.body.angle);
        let (mut reach, mut best_dps, mut best_proj_dps) = (0.0f32, -1.0f32, -1.0f32);
        let (mut projectile_speed, mut projectile_main) = (None, false);
        for g in p.guns.iter().filter(|g| g.clear) {
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
            alpha: (a.torque_pure + diff) / inertia,
            reach,
            projectile_speed,
            projectile_main,
        }
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

    /// Top speed reachable along body-frame unit vector `u` (m/s). Each group stops pushing at its own top
    /// speed, so the reachable velocities form a box; this is the farthest point of the box along `u`. Groups
    /// without thrusters are left out (that part of `u` cannot be pushed at all, as before).
    pub fn top_along(&self, u: Vec2) -> f32 {
        let sx = if u.x >= 0.0 {
            self.speeds[2]
        } else {
            self.speeds[3]
        };
        let sy = if u.y >= 0.0 {
            self.speeds[0]
        } else {
            self.speeds[1]
        };
        let (ax, ay) = (u.x.abs(), u.y.abs());
        let mut s = f32::INFINITY;
        if ax > 1e-6 && sx > 0.0 {
            s = s.min(sx / ax);
        }
        if ay > 1e-6 && sy > 0.0 {
            s = s.min(sy / ay);
        }
        if s.is_finite() { s } else { 0.0 }
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
    let kin = Kin::of(piece);
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
/// side they face; the group with the most damage per second among clear, alive, not overheated guns wins (ties:
/// front, back, right, left), and it aims at the lead point if that group's strongest gun fires projectiles.
/// `None` without working guns.
pub fn smart_aim(ship: &Ship, piece: &Piece, kin: &Kin, t: &TargetState) -> Option<f32> {
    // [dps, best gun dps, its projectile speed (0 = hit-scan)] per facing group (AXES order).
    let mut groups = [[0.0f32; 3]; 4];
    for g in piece.guns.iter().filter(|g| g.clear) {
        let st = ship.part(g.part);
        if !st.alive || st.overheated {
            continue;
        }
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
            // A comfortable turn rate this ship can also stop from within about a quarter turn.
            let rate = tuning.max_turn_rate.min(sqrt(tuning.turn_margin * kin.alpha * PI));
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
            continue;
        }
        // Rated-speed falloff of every row at this tick's velocity, for the allocation below and for the
        // systems (raw throttles too): the same numbers both times.
        let kin = Kin::of(piece);
        let v_local = kin.rot.apply_inverse(kin.vel);
        for (f, t) in piece.falloff.iter_mut().zip(&piece.thrusters) {
            *f = t.falloff(v_local);
        }
        if plan.raw {
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
        let (omega_des, turning) = match goal.heading {
            HeadingGoal::Rate(r) => {
                let r = r.clamp(-tuning.max_turn_rate, tuning.max_turn_rate);
                (r, r.abs() > TURN_WITH_THRUST)
            }
            _ => (turn_rate(error, kin.alpha, tuning), error.abs() > TURN_WITH_THRUST),
        };
        // Turning the thrust towards where it is needed: burn only once roughly aligned (flip, then burn), so
        // slow-turning ships do not chase their own velocity vector.
        let force = match goal.heading {
            HeadingGoal::Thrust(_) | HeadingGoal::Auto if target.is_some() => {
                force * ((cos(error) - ALIGN_NONE) / (ALIGN_FULL - ALIGN_NONE)).clamp(0.0, 1.0)
            }
            _ => force,
        };
        let torque = kin.inertia * (omega_des - kin.omega) * (1.0 / DT);

        // Availability now, after the speed falloff (power is settled later by the systems).
        let fmax = &mut scratch.fmax;
        fmax.clear();
        fmax.extend(piece.thrusters.iter().zip(&piece.falloff).map(|(t, &f)| {
            let ps = &parts[t.part as usize];
            if t.clear && ps.alive && !ps.overheated {
                t.max_force * f
            } else {
                0.0
            }
        }));
        let tmax = &mut scratch.tmax;
        tmax.clear();
        tmax.extend(piece.torque_sources.iter().map(|s| {
            let ps = &parts[s.part as usize];
            if ps.alive && !ps.overheated {
                s.max
            } else {
                0.0
            }
        }));
        let expected = allocate(
            &piece.thrusters,
            &piece.alloc,
            fmax,
            tmax,
            force,
            torque,
            turning,
            &mut piece.act,
        );
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
