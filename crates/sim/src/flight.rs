//! The always-on flight computer: helm input (pilot stick and lock, a maneuver, or a direct goal) →
//! [`FlightGoal`] (heading + velocity) → heading and velocity controllers → control allocation
//! (`alloc.rs`) → per-actuator commands of the ship's controlling piece.
//!
//! * **Heading**: time-optimal-ish turn rate `ω_des = sign(e)·min(ω_max, √(2·margin·α·|e|), gain·|e|)` with
//!   `α` the piece's planning angular acceleration (pure torque + differential thrust), then
//!   `τ = I·(ω_des − ω)/DT`. Big ships plan with their small `α` and do not overshoot; small ones snap.
//! * **Velocity**: dead-beat `F = m·(v_des − v)/DT` in the piece frame, clamped by what the thrusters can do.
//! * **Soft top speed**: `v_top = top_speed_k·√(a)`, `a` = force of the best thrust axis / mass.
//! * The plan runs on the state at the end of the last tick, so every ship sees the same world (order-free).

use core::f32::consts::{FRAC_PI_2, PI};

use crate::alloc::allocate;
use crate::config::{FlightTuning, SimConfig};
use crate::design::NO_SLOT;
use crate::hash::{StateHash, StateHasher};
use crate::ids::{PieceId, ShipId};
use crate::maneuver::{Maneuver, RaiderBrain};
use crate::math::{Rot, Vec2, atan2, cos, sqrt};
use crate::physics::wrap_angle;
use crate::ship::{Piece, Ship, Wrench};
use crate::DT;

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

/// Pilot input (touch stick or WASD, target lock, drift toggle, fire), in world terms.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotInput {
    /// World direction, length 0..1 (longer is clamped). Without a lock: "fly that way", the length sets the
    /// speed as a fraction of the soft top speed. With a lock: strafe that way.
    pub stick: Vec2,
    /// Keep the nose on this target.
    pub lock: Option<Target>,
    /// Released stick coasts instead of braking.
    pub drift: bool,
    pub fire: bool,
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
    Maneuver { maneuver: Maneuver, fire: bool },
    /// A goal set directly (tools, tests).
    Goal { goal: FlightGoal, fire: bool },
}

/// What the flight computer did this tick (for HUDs and tuning).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlightTelemetry {
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
    /// Group with the most thrust (forward wins ties).
    pub best: usize,
    /// Best group's acceleration (m/s²).
    pub accel: f32,
    pub top_speed: f32,
    /// Planning angular acceleration (rad/s²).
    pub alpha: f32,
}

impl Kin {
    pub fn of(p: &Piece, tuning: &FlightTuning) -> Kin {
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
        let diff = if diff > 0.0 { diff } else { 0.5 * a.torque_ccw.max(a.torque_cw) };
        let rot = Rot::from_angle(p.body.angle);
        Kin {
            pos: p.body.pos + rot.apply(p.mass.com),
            vel: p.body.linvel,
            angle: p.body.angle,
            omega: p.body.angvel,
            rot,
            mass,
            inertia,
            caps,
            best,
            accel,
            top_speed: tuning.top_speed_k * sqrt(accel),
            alpha: (a.torque_pure + diff) / inertia,
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
        let cx = if u.x >= 0.0 { self.caps[2] } else { self.caps[3] };
        let cy = if u.y >= 0.0 { self.caps[0] } else { self.caps[1] };
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
                None => s.pieces().iter().max_by_key(|p| (p.parts.len(), std::cmp::Reverse(p.id)))?,
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

/// The outcome of planning one ship (computed for all ships before any is changed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Plan {
    pub goal: Option<FlightGoal>,
    pub fire: bool,
    pub raw: bool,
    pub brain: Option<RaiderBrain>,
}

impl Plan {
    const DARK: Plan = Plan {
        goal: None,
        fire: false,
        raw: false,
        brain: None,
    };
}

/// Pass 1: decide the goal of one ship from its helm (or brain), reading the whole world.
pub(crate) fn plan(ship: &Ship, ships: &[Ship], cfg: &SimConfig) -> Plan {
    let Some(piece) = ship.control_piece().and_then(|c| ship.piece(c)) else {
        return Plan::DARK;
    };
    if !piece.active {
        return Plan::DARK;
    }
    let tuning = &cfg.flight;
    let kin = Kin::of(piece, tuning);
    if let Some(brain) = &ship.brain {
        let (next, maneuver, fire) = brain.think(ship, &kin, ships);
        return Plan {
            goal: Some(maneuver.goal(&kin, ships, tuning)),
            fire,
            raw: false,
            brain: Some(next),
        };
    }
    match &ship.helm {
        HelmMode::Raw => Plan {
            goal: None,
            fire: ship.controls.fire,
            raw: true,
            brain: None,
        },
        HelmMode::Goal { goal, fire } => Plan {
            goal: Some(*goal),
            fire: *fire,
            raw: false,
            brain: None,
        },
        HelmMode::Maneuver { maneuver, fire } => Plan {
            goal: Some(maneuver.goal(&kin, ships, tuning)),
            fire: *fire,
            raw: false,
            brain: None,
        },
        HelmMode::Pilot(input) => Plan {
            goal: Some(pilot_goal(input, &kin, ships, tuning)),
            fire: input.fire,
            raw: false,
            brain: None,
        },
    }
}

/// Stick and lock → goal.
pub fn pilot_goal(input: &PilotInput, kin: &Kin, ships: &[Ship], tuning: &FlightTuning) -> FlightGoal {
    let mut stick = input.stick;
    let len = stick.length();
    if len > 1.0 {
        stick = stick * (1.0 / len);
    }
    let pushed = len > tuning.stick_deadzone;
    let released_velocity = if input.drift { None } else { Some(Vec2::ZERO) };
    if let Some(t) = input.lock.as_ref().and_then(|t| resolve_target(ships, t)) {
        let to = t.pos - kin.pos;
        let heading = if to.length_squared() > 1e-6 {
            HeadingGoal::Angle(Kin::nose_heading(to))
        } else {
            HeadingGoal::Keep
        };
        let velocity = if pushed {
            Some(stick * kin.top_speed)
        } else {
            released_velocity
        };
        return FlightGoal { heading, velocity };
    }
    if !pushed {
        return FlightGoal {
            heading: if input.drift { HeadingGoal::Keep } else { HeadingGoal::Auto },
            velocity: released_velocity,
        };
    }
    // Fly that way. For choosing the heading, a small overspeed along the stick (up to 10% of top speed) is
    // ignored, so a ship with engines only at the back does not flip around for it (soft top speed); the
    // velocity goal stays exact, so nothing thrusts while the ship is too fast.
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
    let w = if kin.top_speed > 0.0 { (dv_len / (0.1 * kin.top_speed)).min(1.0) } else { 0.0 };
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
    if let Some(b) = plan.brain {
        ship.brain = Some(b);
    }
    let control = ship.control_piece();
    let Ship {
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
        if Some(piece.id) != control || !piece.active {
            continue;
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
        controls.fire = plan.fire;
        let Some(goal) = plan.goal else {
            continue;
        };
        let tuning = &cfg.flight;
        let kin = Kin::of(piece, tuning);

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
            HeadingGoal::Keep => None,
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
        let omega_des = turn_rate(error, kin.alpha, tuning);
        // Turning the thrust towards where it is needed: burn only once roughly aligned (flip, then burn), so
        // slow-turning ships do not chase their own velocity vector.
        let force = match goal.heading {
            HeadingGoal::Thrust(_) | HeadingGoal::Auto if target.is_some() => {
                force * ((cos(error) - ALIGN_NONE) / (ALIGN_FULL - ALIGN_NONE)).clamp(0.0, 1.0)
            }
            _ => force,
        };
        let torque = kin.inertia * (omega_des - kin.omega) * (1.0 / DT);

        // Availability now (power is settled later by the systems).
        let fmax = &mut scratch.fmax;
        fmax.clear();
        fmax.extend(piece.thrusters.iter().map(|t| {
            let ps = &parts[t.part as usize];
            if t.clear && ps.alive && !ps.overheated { t.max_force } else { 0.0 }
        }));
        let tmax = &mut scratch.tmax;
        tmax.clear();
        tmax.extend(piece.torque_sources.iter().map(|s| {
            let ps = &parts[s.part as usize];
            if ps.alive && !ps.overheated { s.max } else { 0.0 }
        }));
        let expected = allocate(
            &piece.thrusters,
            &piece.alloc,
            fmax,
            tmax,
            force,
            torque,
            error.abs() > TURN_WITH_THRUST,
            &mut piece.act,
        );
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
