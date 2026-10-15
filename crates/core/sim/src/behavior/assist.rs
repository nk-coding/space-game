//! Pilot assists (GDR-0088 step 3): a design's PILOT ASSISTS list, rules in the behavior format that help the player
//! fly the ship they pilot. They run only on a piloted ship (the focus: `HelmMode::Pilot`, no behavior) that was
//! given a list (`ShipCommand::Assists`); every other ship, and a piloted one without a list, flies as before.
//!
//! * **Channels**: each assist acts on one control channel: turning (`face_target`), thrust (`brake`) or the trigger
//!   of one weapon group (`auto_fire(g)`). The pilot always wins: a channel the player uses this tick (A/D or the
//!   stick, or a held lock, which aims by itself; W/S/Q/E or the stick; a group's trigger) is the player's, and
//!   letting go hands it back on the next tick.
//! * **Rules**: at every think (with the behaviors, every [`tuning::THINK_PERIOD`] ticks, and on the tick after the
//!   list is set or switched) the rules are read from the top; a rule that is switched on, whose conditions all hold
//!   (and that has a target, if it needs one) decides its channel unless a rule above it already does. So "face" and
//!   "fire group 2" run together, and two brakes with different conditions are a priority list of their own.
//! * **Target**: the pilot's lock while one is held, else the list's target line (default `nearest`; `locked`, the
//!   leader's lock, finds nothing for the focus, which has no leader).
//! * **Switches**: each rule starts a mission on or off (its `start` value); `ShipCommand::SwitchAssists` flips the
//!   rules of a toggle slot (`toggle1`..`toggle4`) or the master switch, ALL ASSISTS (off: no assist acts; on: they
//!   are back as their toggles left them). The state lives on the ship for the mission.
//! * **Per tick** ([`assist_plan`]): the deciding rules shape the pilot's goal: face turns the strongest guns onto the
//!   target (smart aim, as a lock does), brake asks for zero velocity without touching the heading (only thrusters
//!   that push against the motion brake), auto-fire fires the group's guns that bear, with fire at will's per-gun
//!   test.
//! * **Determinism**: integer tick counters, index-order loops, one snapshot per think; the state is hashed with the
//!   ship's, and only when a ship has a list, so worlds without assists keep their hashes.

use std::sync::Arc;

use data::MAX_WEAPON_GROUP;

use super::compile::{AssistAction, SelectorKind};
use super::eval::{Ctx, Lazy, Pilot, aim_state, pick_part, snapshot};
use super::{Assists, AssistsId, BehaviorLibrary, Scratch, tuning};
use crate::DT;
use crate::config::FlightTuning;
use crate::flight::{AimMode, FIRE_ALL, Fire, FlightGoal, HelmMode, Kin, PilotInput, Target, resolve_target, smart_aim};
use crate::hash::{StateHash, StateHasher};
use crate::ids::ShipId;
use crate::math::Vec2;
use crate::ship::design::PartIdx;
use crate::ship::{Piece, Ship};

/// A control channel an assist acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Turning,
    Thrust,
    /// The trigger of a weapon group (1 to `data::MAX_WEAPON_GROUP`).
    Fire(u8),
}

impl Channel {
    fn of(a: AssistAction) -> Channel {
        match a {
            AssistAction::FaceTarget => Channel::Turning,
            AssistAction::Brake => Channel::Thrust,
            AssistAction::AutoFire(g) => Channel::Fire(g),
        }
    }

    fn index(self) -> usize {
        match self {
            Channel::Turning => 0,
            Channel::Thrust => 1,
            Channel::Fire(g) => 1 + g as usize,
        }
    }
}

/// Turning, thrust and one channel per weapon group.
const CHANNELS: usize = 2 + MAX_WEAPON_GROUP as usize;

/// What `ShipCommand::SwitchAssists` flips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssistSwitch {
    /// The rules on this toggle slot (1 to `data::MAX_ASSIST_TOGGLES`): ASSIST TOGGLE 1..4.
    Toggle(u8),
    /// ALL ASSISTS, the master switch.
    All,
}

/// A piloted ship's assists for the mission: the list, the switches, the pilot's last use of each control and what
/// the last think decided.
#[derive(Clone, Debug)]
pub struct AssistState {
    pub(crate) id: AssistsId,
    pub(crate) program: Arc<Assists>,
    /// Bit `i`: rule `i` is switched on (each starts as its `start` value says; its toggle flips it).
    pub(crate) on: u64,
    /// ALL ASSISTS: while off, no rule acts.
    pub(crate) all: bool,
    /// The tick the pilot last used each control (turning, thrust, a trigger); the tick the list was set until then.
    pub(crate) used: [u64; 3],
    /// The target at the last think: the pilot's lock, else the target line's pick (and its aim part).
    pub(crate) focus: Option<ShipId>,
    pub(crate) aim_part: Option<PartIdx>,
    /// Tick the current target was picked (the target timers count from here).
    pub(crate) acquired: u64,
    /// Per channel (turning, thrust, weapon groups 1..), the rule that decides it.
    pub(crate) decides: [Option<u16>; CHANNELS],
    /// Tick of the last think (`None`: thinks on the next tick).
    pub(crate) thought: Option<u64>,
}

impl AssistState {
    pub(crate) fn new(id: AssistsId, program: Arc<Assists>, tick: u64) -> AssistState {
        let on = program
            .code
            .rules
            .iter()
            .enumerate()
            .fold(0u64, |m, (i, r)| m | (r.on as u64) << i);
        AssistState {
            id,
            program,
            on,
            all: true,
            used: [tick; 3],
            focus: None,
            aim_part: None,
            acquired: tick,
            decides: [None; CHANNELS],
            thought: None,
        }
    }

    /// A copy for a vessel split off this one: the same list and switches, nothing decided, thinks on its next tick.
    pub(crate) fn inherit(&self) -> AssistState {
        AssistState {
            decides: [None; CHANNELS],
            thought: None,
            ..self.clone()
        }
    }

    /// Flips a toggle slot's rules or the master switch; the rules are read again on the next tick.
    pub(crate) fn switch(&mut self, s: AssistSwitch) {
        match s {
            AssistSwitch::All => self.all = !self.all,
            AssistSwitch::Toggle(k) => {
                for (i, r) in self.program.code.rules.iter().enumerate() {
                    if r.toggle == k && k != 0 {
                        self.on ^= 1 << i;
                    }
                }
            }
        }
        self.thought = None;
    }

    pub fn id(&self) -> AssistsId {
        self.id
    }

    /// The list (its `program` is the source, for names).
    pub fn program(&self) -> &Arc<Assists> {
        &self.program
    }

    /// ALL ASSISTS is on.
    pub fn all_on(&self) -> bool {
        self.all
    }

    /// Rule `i` is switched on (by its start and its toggle; ALL ASSISTS aside).
    pub fn is_on(&self, i: usize) -> bool {
        i < 64 && self.on & (1 << i) != 0
    }

    /// The toggle slot of rule `i` (1 to `data::MAX_ASSIST_TOGGLES`), if it has one.
    pub fn toggle(&self, i: usize) -> Option<u8> {
        self.program.code.rules.get(i).map(|r| r.toggle).filter(|&t| t != 0)
    }

    /// The channel rule `i` acts on.
    pub fn channel(&self, i: usize) -> Option<Channel> {
        self.program.code.rules.get(i).map(|r| Channel::of(r.action))
    }

    /// Rule `i` decides its channel (at the last think): it acts whenever the pilot leaves that channel free.
    pub fn decides(&self, i: usize) -> bool {
        self.decides.contains(&Some(i as u16))
    }

    /// The target the assists aim at (at the last think).
    pub fn target(&self) -> Option<ShipId> {
        self.focus
    }
}

impl StateHash for AssistState {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.id.0);
        h.u64(self.program.fingerprint);
        h.u64(self.on);
        h.bool(self.all);
        for &u in &self.used {
            h.u64(u);
        }
        h.u32(self.focus.map_or(u32::MAX, |s| s.0));
        h.u32(self.aim_part.unwrap_or(u32::MAX));
        h.u64(self.acquired);
        for d in &self.decides {
            h.u32(d.map_or(u32::MAX, u32::from));
        }
        h.u64(self.thought.unwrap_or(u64::MAX));
    }
}

/// The controls the pilot uses this tick: turning (A/D, or the touch stick without a lock), thrust (W/S/Q/E, or the
/// stick), a trigger. A key counts from the flight computer's dead zone, as `pilot_goal` reads it.
fn controls_used(input: &PilotInput, deadzone: f32) -> [bool; 3] {
    let pushed = |v: f32| v.abs() >= deadzone;
    let (turn, thrust) = match input.axes {
        Some(a) => (pushed(a.turn), pushed(a.thrust) || pushed(a.strafe)),
        None => {
            let stick = input.stick.length() >= deadzone;
            (stick && input.lock.is_none(), stick)
        }
    };
    [turn, thrust, input.fire & FIRE_ALL != 0]
}

/// The pilot of a ship whose assists run now (the focus: piloted, no behavior), with its input.
fn piloted(s: &Ship) -> Option<&PilotInput> {
    match (&s.helm, &s.behavior, &s.assists) {
        (HelmMode::Pilot(p), None, Some(_)) => Some(p),
        _ => None,
    }
}

/// The result of one think, before it is applied.
#[derive(Clone, Debug, Default)]
pub(crate) struct AssistOut {
    valid: bool,
    focus: Option<ShipId>,
    aim_part: Option<PartIdx>,
    acquired: u64,
    decides: [Option<u16>; CHANNELS],
}

/// The assist pass, every tick after the behaviors' think: each piloted ship with assists notes which controls its
/// pilot uses; on think ticks (or right after a switch) its rules decide the channels, all from one snapshot.
pub(crate) fn assist_all(ships: &mut [Ship], lib: &BehaviorLibrary, scratch: &mut Scratch, tick: u64, flight: &FlightTuning) {
    let mut any_due = false;
    for s in ships.iter_mut() {
        let Some(used) = piloted(s).map(|p| controls_used(p, flight.stick_deadzone)) else {
            continue;
        };
        let st = s.assists.as_mut().expect("piloted() checked it");
        for (u, now) in st.used.iter_mut().zip(used) {
            if now {
                *u = tick;
            }
        }
        any_due |= st.thought.is_none() || tick.is_multiple_of(tuning::THINK_PERIOD);
    }
    if !any_due {
        return;
    }
    let n = ships.len();
    if scratch.assist_outs.len() < n {
        scratch.assist_outs.resize_with(n, AssistOut::default);
    }
    snapshot(ships, &mut scratch.hull, &mut scratch.centres);
    let Scratch {
        assist_outs,
        hull,
        centres,
        scopes,
        script_evals,
        ..
    } = scratch;
    let world: &[Ship] = ships;
    for k in 0..n {
        let out = &mut assist_outs[k];
        out.valid = false;
        let me = &world[k];
        let Some(input) = piloted(me) else {
            continue;
        };
        let st = me.assists.as_ref().expect("piloted() checked it");
        if !(st.thought.is_none() || tick.is_multiple_of(tuning::THINK_PERIOD)) || !me.is_active() {
            continue;
        }
        let secs = |since: u64| tick.saturating_sub(since) as f32 * DT;
        let last = st.used.iter().copied().max().unwrap_or(tick);
        let pilot = Pilot {
            lock: input.lock.is_some(),
            idle: [secs(last), secs(st.used[0]), secs(st.used[1]), secs(st.used[2])],
        };
        let mut cx = Ctx {
            me,
            my_k: k,
            world,
            tick,
            kin: None,
            reach: 0.0,
            focus: None,
            acquired: 0,
            hull,
            centres,
            lazy: Lazy::default(),
            lib,
            scopes,
            script_evals,
            script_error: None,
            pilot: Some(pilot),
        };
        think(&mut cx, st, input, out);
    }
    for (s, out) in ships.iter_mut().zip(assist_outs.iter_mut()) {
        if !std::mem::take(&mut out.valid) {
            continue;
        }
        if let Some(st) = s.assists.as_mut() {
            st.focus = out.focus;
            st.aim_part = out.aim_part;
            st.acquired = out.acquired;
            st.decides = out.decides;
            st.thought = Some(tick);
        }
    }
}

/// One ship's think: the target (the lock first), then the rules in order, each deciding its channel if it is
/// switched on, its conditions hold and no rule above decided that channel.
fn think(cx: &mut Ctx, st: &AssistState, input: &PilotInput, out: &mut AssistOut) {
    let Some(kin) = cx.prepare() else {
        return;
    };
    let code = &st.program.code;
    let locked = match input.lock {
        Some(Target::Ship(id) | Target::Piece(id, _)) => Some(id),
        _ => None,
    };
    let focus = match locked {
        Some(id) => cx.world.binary_search_by_key(&id, |s| s.id()).ok().and_then(|k| {
            let ts = cx.centres[k].or_else(|| resolve_target(cx.world, &Target::Ship(id)))?;
            Some((k, ts, (ts.pos - kin.pos).length()))
        }),
        // `locked` follows a leader's lock; the focus has none (its own lock came first).
        None if code.target.kind == SelectorKind::Locked => None,
        None => cx.select(code.target.kind, st.focus),
    };
    cx.focus = focus;
    let focus_id = focus.map(|(k, _, _)| cx.world[k].id());
    cx.acquired = if focus_id == st.focus { st.acquired } else { cx.tick };
    out.aim_part = match locked {
        Some(_) => None,
        None => focus.and_then(|(k, _, _)| pick_part(&cx.world[k], code.target.aim, kin.pos)),
    };
    out.decides = [None; CHANNELS];
    if st.all {
        for (i, rule) in code.rules.iter().enumerate() {
            let ch = Channel::of(rule.action).index();
            if !st.is_on(i) || out.decides[ch].is_some() || (rule.action.needs_target() && focus.is_none()) {
                continue;
            }
            let conds = &code.conds[rule.start as usize..(rule.start + rule.len) as usize];
            if conds.iter().all(|c| matches!(cx.cond(c), Ok((true, _)))) {
                out.decides[ch] = Some(i as u16);
            }
        }
    }
    out.valid = true;
    out.focus = focus_id;
    out.acquired = cx.acquired;
}

/// The per-tick part: the deciding assists take the channels the pilot leaves free (see the module docs).
#[allow(clippy::too_many_arguments)]
pub(crate) fn assist_plan(
    st: &AssistState,
    input: &PilotInput,
    kin: &Kin,
    ship: &Ship,
    piece: &Piece,
    ships: &[Ship],
    flight: &FlightTuning,
    goal: &mut FlightGoal,
    fire: &mut Fire,
) {
    if !st.all {
        return;
    }
    let [turning, thrust, _] = controls_used(input, flight.stick_deadzone);
    let at = match &input.lock {
        Some(t) => resolve_target(ships, t),
        None => st.focus.and_then(|f| aim_state(ships, f, st.aim_part)),
    };
    // A lock aims the ship itself: the turning channel is the pilot's while one is held.
    if st.decides[Channel::Turning.index()].is_some()
        && !turning
        && input.lock.is_none()
        && let Some(t) = at
        && let Some(h) = smart_aim(ship, piece, kin, &t).or_else(|| kin.track(kin.aim_point(&t, AimMode::Auto), t.vel, 0))
    {
        goal.heading = h;
    }
    if st.decides[Channel::Thrust.index()].is_some() && !thrust {
        goal.velocity = Some(Vec2::ZERO);
    }
    let held = input.fire & FIRE_ALL;
    let auto = (1..=MAX_WEAPON_GROUP)
        .filter(|&g| st.decides[Channel::Fire(g).index()].is_some())
        .fold(0u8, |m, g| m | 1 << (g - 1))
        & !held;
    if auto != 0
        && let Some(t) = at
    {
        *fire = Fire::Assisted { held, auto, at: t };
    }
}
