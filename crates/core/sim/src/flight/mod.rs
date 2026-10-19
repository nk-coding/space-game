//! The always-on flight computer: helm input (pilot stick and lock, a maneuver, or a direct goal) →
//! [`FlightGoal`] (heading + velocity) → heading and velocity controllers → control allocation
//! (`ship/alloc.rs`) → per-actuator commands of the ship's controlling piece.
//!
//! * **Heading**: time-optimal-ish turn rate `ω_des = sign(e)·min(ω_max, √(2·margin·α·|e|), gain·|e|)` with
//!   `α` the piece's planning angular acceleration (pure torque + the RCS couples that lasting translation
//!   leaves free, plus differential thrust while the last resort is on), then `τ = I·(ω_des − ω)/DT`. Big ships
//!   plan with their small `α` and do not overshoot; small ones snap. A lock or `face` on a moving target adds
//!   the target's bearing rate (feed-forward, GDR-0064), so the nose leads a crossing target.
//! * **Turret**: a vessel under a behavior with no working engine turns its strongest guns onto its target
//!   (GDR-0064).
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

mod aim;
mod controllers;
mod pilot;

pub mod maneuver;
pub mod physics;
pub mod stand_in;

use core::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::DT;
use crate::ship::alloc::{allocate, assist, couples, group_of, lasting, translate, wrench_of};
use crate::config::{FlightTuning, SimConfig};
use crate::ship::design::NO_SLOT;
use crate::hash::{StateHash, StateHasher};
use crate::ids::{PieceId, ShipId};
use crate::flight::maneuver::Maneuver;
use crate::math::{Rot, Vec2, atan2, cos, sqrt};
use crate::flight::physics::wrap_angle;
use crate::ship::{PartState, Piece, Ship, Wrench, gun_works, producing};
use crate::voyage::rocks::{Mover, Rock, steer};

pub use aim::{intercept_time, lead_point, resolve_target, smart_aim};
pub use controllers::turn_rate;
pub(crate) use controllers::execute;
pub use pilot::{FIRE_ALL, KEY_TURN_GROWTH, KEY_TURN_START, NUDGE_MAX, NUDGE_RATE, NUDGE_RETURN, PilotAxes, PilotInput, next_lock, nudge_step, pilot_goal, turn_ramp_step};

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
    /// Body angle `angle`, which itself turns at `rate` rad/s (counter-clockwise): a lock or `face` on a moving
    /// target, whose bearing rate is fed forward (GDR-0064). The flight computer asks for that rate (up to one
    /// the ship can stop from within about a quarter turn) plus the usual turn towards `angle`, so the nose
    /// leads a crossing target instead of trailing it.
    Track { angle: f32, rate: f32 },
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
            HeadingGoal::Track { angle, rate } => angle.is_finite() && rate.is_finite(),
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
/// the differential thrust in. A turn that makes no headway with the last resort on (the heading error over
/// [`BLOCKED_TURN`], the ship hardly turning, under [`STILL`] for [`STILL_TICKS`] ticks in a row) is blocked the
/// short way, as a ship pressed against a rock whose hull would swing into it (the balance harness's stuck check,
/// 18 Oct 2026: a flagship pinned to a rock for 5000 ticks): it turns the long way round (`long_way`, the sense)
/// until the error is under [`BLOCKED_TURN`] again or the last resort is off. Kept per piece (`Piece::assist`),
/// part of the state hash.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TurnAssist {
    /// Ticks in a row the first line has been at its limit (up to `assist_after`).
    pub ticks: u32,
    pub on: bool,
    /// Ticks in a row the turn was blocked (up to [`STILL_TICKS`]).
    pub still: u32,
    /// Turning the long way round, counter-clockwise (+1) or clockwise (−1); 0 the short way.
    pub long_way: i8,
}

/// A heading error (rad) over which a turn that does not turn is blocked ([`TurnAssist`]) ...
const BLOCKED_TURN: f32 = 2.0;
/// ... at a turn rate under this (rad/s) ...
const STILL: f32 = 0.005;
/// ... this many ticks in a row (a spin reversing passes through the still rate in a tick).
const STILL_TICKS: u32 = 10;

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
                ..TurnAssist::default()
            }
        } else {
            TurnAssist::default()
        }
    }

    /// The heading error to steer by: the short way's (`short`, wrapped), or the long way round while the short way
    /// is blocked. Latches and clears `long_way` (at the turn rate `omega`).
    pub fn error(&mut self, short: f32, omega: f32) -> f32 {
        if short.abs() <= BLOCKED_TURN || !self.on {
            (self.still, self.long_way) = (0, 0);
        } else if self.long_way == 0 {
            self.still = if omega.abs() < STILL { (self.still + 1).min(STILL_TICKS) } else { 0 };
            if self.still >= STILL_TICKS {
                self.long_way = if short > 0.0 { -1 } else { 1 };
            }
        }
        let sense = f32::from(self.long_way);
        if short * sense < 0.0 { short + sense * TAU } else { short }
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

    /// The heading goal that keeps one side of the ship (`side`: 0 nose, 1 tail, 2 right, 3 left) on the point
    /// `aim`, which moves at `aim_vel`: [`HeadingGoal::Track`] with the bearing rate `(r × v) / |r|²` of the
    /// point as seen from the ship (GDR-0064). `None` when the point is at the centre of mass.
    pub fn track(&self, aim: Vec2, aim_vel: Vec2, side: usize) -> Option<HeadingGoal> {
        let to = aim - self.pos;
        let d2 = to.length_squared();
        (d2 > 1e-6).then(|| HeadingGoal::Track {
            angle: wrap_angle(atan2(to.y, to.x) - AXIS_ANGLE[side]),
            rate: to.cross(aim_vel - self.vel) / d2,
        })
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
    /// The guns of these weapon groups fire (bit g-1 for group g, never 0): the pilot's triggers, or
    /// [`FIRE_ALL`] for a helm with `fire: true`.
    Groups(u8),
    /// Fire at will at a target (behaviors): each gun fires when the target is inside its own cone along its
    /// facing and within its reach (`behavior::guns_at_will`).
    AtWill(TargetState),
    /// Mining (GDR-0114): the mining guns fire at will at a point on an asteroid rock, the others hold.
    Mine(TargetState),
    /// The pilot's assisted fire (GDR-0088): the guns of the `held` groups fire (the triggers), those of the `auto`
    /// groups fire at will at `at` (auto-fire assists; never a held group).
    Assisted { held: u8, auto: u8, at: TargetState },
}

impl Fire {
    fn all_if(fire: bool) -> Fire {
        Fire::groups(if fire { FIRE_ALL } else { 0 })
    }

    fn groups(mask: u8) -> Fire {
        match mask & FIRE_ALL {
            0 => Fire::Hold,
            m => Fire::Groups(m),
        }
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

/// Pass 1: decide the goal of one ship from its helm (or behavior), reading the whole world. A behavior's or a
/// maneuver's velocity goal bends round the voyage's asteroid rocks ahead (`rocks`, GDR-0104,
/// [`crate::voyage::rocks::steer`]); a pilot's does not (the assist `avoid_collisions` sidesteps them).
pub(crate) fn plan(ship: &Ship, ships: &[Ship], rocks: &[Rock], cfg: &SimConfig) -> Plan {
    let Some(piece) = ship.control_piece().and_then(|c| ship.piece(c)) else {
        return Plan::DARK;
    };
    if !piece.active {
        return Plan::DARK;
    }
    let tuning = &cfg.flight;
    let kin = Kin::of(piece, &ship.parts);
    if let Some(b) = &ship.behavior {
        let (mut goal, fire) = crate::behavior::fly(b, &kin, ships, tuning);
        goal.velocity = goal.velocity.map(|v| steer(&mover(piece, &kin), v, rocks));
        // A vessel without a working engine fights on as a turret (GDR-0064): whatever its rule flies, it turns
        // its strongest working guns onto its target with what it has (gyros, cores, RCS); RCS pods still follow
        // the velocity goal. Before, `HeadingGoal::Auto` with nothing to push kept the heading: it never aimed.
        if let Fire::AtWill(ts) = fire
            && !has_engine(piece, &ship.parts)
            && let Some(aim) = smart_aim(ship, piece, &kin, &ts)
        {
            goal.heading = aim;
        }
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
            goal: Some({
                let mut goal = maneuver.goal(&kin, ships, tuning);
                goal.velocity = goal.velocity.map(|v| steer(&mover(piece, &kin), v, rocks));
                goal
            }),
            fire: Fire::all_if(*fire),
            raw: false,
        },
        HelmMode::Pilot(input) => {
            let mut goal = pilot_goal(input, &kin, ship, piece, ships, tuning);
            let mut fire = Fire::groups(input.fire);
            if let Some(a) = &ship.assists {
                crate::behavior::assist_plan(a, input, &kin, piece, ships, rocks, tuning, &mut goal, &mut fire);
            }
            Plan {
                goal: Some(goal),
                fire,
                raw: false,
            }
        }
    }
}

/// The piece flying `kin` as [`steer`] sees it.
fn mover(piece: &Piece, kin: &Kin) -> Mover {
    Mover { pos: piece.body.pos, radius: piece.radius, vel: kin.vel, accel: kin.stop_accel() }
}

/// Does the piece have an engine (not an RCS nozzle) that can push now ([`producing`])?
fn has_engine(piece: &Piece, parts: &[PartState]) -> bool {
    piece
        .thrusters
        .iter()
        .any(|t| !t.rcs && producing(&parts[t.part as usize]))
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
            HeadingGoal::Track { angle, rate } => {
                h.u8(5);
                h.f32(angle);
                h.f32(rate);
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
                h.u8(p.fire);
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
