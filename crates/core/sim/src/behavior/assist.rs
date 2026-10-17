//! Pilot assists (GDR-0088 step 3): a design's PILOT ASSISTS list, rules in the behavior format that help the player
//! fly the ship they pilot. They run only on a piloted ship (the focus: `HelmMode::Pilot`, no behavior) that was
//! given a list (`ShipCommand::Assists`); every other ship, and a piloted one without a list, flies as before.
//!
//! * **Channels**: each assist acts on one control channel: the lock (`auto_lock`), thrust (`brake`, `hold_station`,
//!   `avoid_collisions`) or the trigger of one weapon group (`auto_fire(g)`). The pilot always wins: a channel the
//!   player uses this tick (a held lock; W/S/Q/E or the stick; a group's trigger) is the player's, and letting go
//!   hands it back on the next tick.
//! * **Rules**: at every think (with the behaviors, every [`tuning::THINK_PERIOD`] ticks, on the tick after the
//!   list is set or switched and on the tick auto-lock pauses or resumes) the rules are read from the top; a rule that is switched on, whose conditions all hold
//!   (and that has a target, if it needs one; for `avoid_collisions`, something on a collision course; for
//!   `auto_lock`, a ship to lock) decides its channel unless a rule above it already does. So "brake" and "fire group
//!   2" run together, and two brakes with different conditions are a priority list of their own.
//! * **Target**: the pilot's lock, the one the HUD shows, and nothing else (Niklas, 15 Oct 2026: no target the
//!   assists pick on their own and don't show). Without a lock `auto_fire` does nothing; `auto_lock` sets one.
//! * **Auto-lock**: while the pilot holds no lock it picks a ship with its selector ([`AssistState::auto_lock`]); the
//!   client locks it as a click would (`PilotInput::lock` from the next tick), so the HUD, the aim and every assist
//!   see the one lock. A lock the pilot drops while its ship is still there (Esc, a click on empty space) pauses
//!   auto-lock until the pilot locks again or switches an auto-lock rule (its toggle, ALL ASSISTS); a lock that goes
//!   because its ship did (destroyed, gone dark) doesn't, so auto-lock moves on to the next.
//! * **Switches**: each rule starts a mission on or off (its `start` value); `ShipCommand::SwitchAssists` flips the
//!   rules of a toggle slot (`toggle1`..`toggle4`) or the master switch, ALL ASSISTS (off: no assist acts; on: they
//!   are back as their toggles left them). The state lives on the ship for the mission.
//! * **Per tick** ([`assist_plan`]): the deciding rules shape the pilot's goal: brake asks for zero velocity,
//!   auto-fire fires the group's guns that bear on the lock, with fire at will's per-gun test.
//! * **Thrust assists turn while turning is free** (brake, hold station, avoid collisions; Niklas, 15 Oct 2026, as
//!   no base ship has RCS): while the pilot neither steers nor holds a lock (which aims the ship), and the velocity
//!   change asked is at least [`tuning::ASSIST_TURN_DV`], the heading becomes `HeadingGoal::Auto` (the behaviors' flip and burn: kept while the thrusters facing the needed way give half the
//!   best axis). Otherwise the heading stays the pilot's and only the thrusters pushing the needed way act.
//! * **Hold station** (GDR-0088 step 4): the station is taken on the first tick the rule has thrust while the pilot
//!   doesn't thrust: the ship's centre of mass, or its offset from the nearest other active vessel of its owner (with
//!   `ally`; a point without one). The pilot's thrust drops it, so thrusting moves the station; a rule above that
//!   takes thrust for a while (a dodge, a brake) leaves it waiting, unless the rule that took it is switched off; an
//!   ally that is no vessel any more is replaced. Per tick the velocity goal flies back to the station and keeps pace
//!   with it (an escort at distance 0).
//! * **Avoid collisions** (step 4): at the think, the piece (of any ship, wrecks and the ship's own debris included)
//!   whose bounding circle the ship's would meet first within [`tuning::AVOID_AHEAD`] seconds, on straight lines and
//!   with [`tuning::AVOID_CLEARANCE`] to spare; per tick the velocity goal adds the sidestep that makes the closest
//!   approach that clearance (along the miss, or to the left of the relative motion head-on), until it is clear.
//! * **Determinism**: integer tick counters, index-order loops, one snapshot per think; the state is hashed with the
//!   ship's, and only when a ship has a list, so worlds without assists keep their hashes.

use core::f32::consts::PI;
use std::sync::Arc;

use data::MAX_WEAPON_GROUP;

use super::compile::{AssistAction, Station};
use super::eval::{Ctx, Lazy, Pilot, snapshot};
use super::{Assists, AssistsId, BehaviorLibrary, Scratch, tuning};
use crate::DT;
use crate::config::FlightTuning;
use crate::flight::maneuver::Maneuver;
use crate::flight::{
    FIRE_ALL, Fire, FlightGoal, HeadingGoal, HelmMode, Kin, PilotInput, Target, TargetState, find_ship, resolve_target,
};
use crate::hash::{StateHash, StateHasher};
use crate::ids::{PieceId, ShipId};
use crate::math::{Vec2, sqrt};
use crate::ship::vessel::friendly;
use crate::ship::{Piece, Ship};
use crate::flight::physics::BodyKey;
use crate::voyage::rocks::Rock;

/// A control channel an assist acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// The pilot's lock (`auto_lock`).
    Lock,
    Thrust,
    /// The trigger of a weapon group (1 to `data::MAX_WEAPON_GROUP`).
    Fire(u8),
}

impl Channel {
    fn of(a: AssistAction) -> Channel {
        match a {
            AssistAction::AutoLock(_) => Channel::Lock,
            AssistAction::Brake | AssistAction::HoldStation(_) | AssistAction::AvoidCollisions => Channel::Thrust,
            AssistAction::AutoFire(g) => Channel::Fire(g),
        }
    }

    fn index(self) -> usize {
        match self {
            Channel::Lock => 0,
            Channel::Thrust => 1,
            Channel::Fire(g) => 1 + g as usize,
        }
    }
}

/// The lock, thrust and one channel per weapon group.
const CHANNELS: usize = 2 + MAX_WEAPON_GROUP as usize;

/// What `ShipCommand::SwitchAssists` flips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssistSwitch {
    /// The rules on this toggle slot (1 to `data::MAX_ASSIST_TOGGLES`): ASSIST TOGGLE 1..4.
    Toggle(u8),
    /// ALL ASSISTS, the master switch.
    All,
}

/// Where `hold_station` keeps the ship: a world point, or an offset from an ally's centre of mass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StationPoint {
    /// The rule that took it.
    pub rule: u16,
    pub ally: Option<ShipId>,
    /// The point, or with an ally the offset from its centre of mass (world frame).
    pub at: Vec2,
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
    /// The target at the last think: the ship of the pilot's lock.
    pub(crate) focus: Option<ShipId>,
    /// Tick the current target was picked (the target timers count from here).
    pub(crate) acquired: u64,
    /// The pilot's lock and the last tick it was held (a drop is seen on the tick after).
    pub(crate) held: Option<(Target, u64)>,
    /// The pilot dropped a lock whose ship is still there: auto-lock waits until the pilot locks again or switches it.
    pub(crate) paused: bool,
    /// The ship auto-lock asks the pilot's lock to take (from the last think; none while a lock is held).
    pub(crate) pick: Option<ShipId>,
    /// Per channel (the lock, thrust, weapon groups 1..), the rule that decides it.
    pub(crate) decides: [Option<u16>; CHANNELS],
    /// Tick of the last think (`None`: thinks on the next tick).
    pub(crate) thought: Option<u64>,
    /// The station `hold_station` keeps (taken and dropped tick by tick, [`keep_stations`]).
    pub(crate) station: Option<StationPoint>,
    /// The piece `avoid_collisions` steers clear of (from the last think).
    pub(crate) avoid: Option<(ShipId, PieceId)>,
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
            acquired: tick,
            held: None,
            paused: false,
            pick: None,
            decides: [None; CHANNELS],
            thought: None,
            station: None,
            avoid: None,
        }
    }

    /// A copy for a vessel split off this one: the same list and switches, nothing decided, thinks on its next tick.
    pub(crate) fn inherit(&self) -> AssistState {
        AssistState {
            decides: [None; CHANNELS],
            thought: None,
            station: None,
            avoid: None,
            pick: None,
            ..self.clone()
        }
    }

    /// Flips a toggle slot's rules or the master switch; the rules are read again on the next tick. Switching an
    /// auto-lock rule (ALL ASSISTS switches every one) ends its pause.
    pub(crate) fn switch(&mut self, s: AssistSwitch) {
        match s {
            AssistSwitch::All => {
                self.all = !self.all;
                self.paused = false;
            }
            AssistSwitch::Toggle(k) => {
                for (i, r) in self.program.code.rules.iter().enumerate() {
                    if r.toggle == k && k != 0 {
                        self.on ^= 1 << i;
                        self.paused &= Channel::of(r.action) != Channel::Lock;
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

    /// The target the assists act on (at the last think): the ship of the pilot's lock.
    pub fn target(&self) -> Option<ShipId> {
        self.focus
    }

    /// The ship auto-lock asks to be locked while the pilot holds no lock: the client locks it, as a click does.
    pub fn auto_lock(&self) -> Option<ShipId> {
        self.pick
    }

    /// Auto-lock waits: the pilot dropped a lock while its ship was still there.
    pub fn lock_paused(&self) -> bool {
        self.paused
    }

    /// The station `hold_station` keeps now, if it has one.
    pub fn station(&self) -> Option<StationPoint> {
        self.station
    }

    /// The ship and piece `avoid_collisions` steers clear of (at the last think); an asteroid rock as
    /// `(BodyKey::ROCKS, rock)` (GDR-0104).
    pub fn avoiding(&self) -> Option<(ShipId, PieceId)> {
        self.avoid
    }

    /// The rule that decides `c` (at the last think), with its action.
    fn decider(&self, c: Channel) -> Option<(u16, AssistAction)> {
        let i = self.decides[c.index()]?;
        Some((i, self.program.code.rules[i as usize].action))
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
        h.u64(self.acquired);
        match &self.held {
            Some((t, at)) => {
                h.u8(1);
                t.hash_state(h);
                h.u64(*at);
            }
            None => h.u8(0),
        }
        h.bool(self.paused);
        h.u32(self.pick.map_or(u32::MAX, |s| s.0));
        for d in &self.decides {
            h.u32(d.map_or(u32::MAX, u32::from));
        }
        h.u64(self.thought.unwrap_or(u64::MAX));
        match &self.station {
            Some(s) => {
                h.u8(1);
                h.u32(u32::from(s.rule));
                h.u32(s.ally.map_or(u32::MAX, |a| a.0));
                h.vec2(s.at);
            }
            None => h.u8(0),
        }
        let (ship, piece) = self.avoid.map_or((u32::MAX, u32::MAX), |(s, p)| (s.0, p.0));
        h.u32(ship);
        h.u32(piece);
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
    acquired: u64,
    decides: [Option<u16>; CHANNELS],
    avoid: Option<(ShipId, PieceId)>,
    pick: Option<ShipId>,
}

/// A lock's ship (or piece) is still there: a ship still active, a piece not gone; a point always is. The client
/// drops a lock whose ship isn't, so a lock gone while its ship is there was dropped by the pilot.
fn lock_there(ships: &[Ship], t: &Target) -> bool {
    match *t {
        Target::Ship(id) => find_ship(ships, id).is_some_and(Ship::is_active),
        Target::Piece(id, piece) => find_ship(ships, id).is_some_and(|s| s.piece(piece).is_some()),
        Target::Point(_) => true,
    }
}

/// The assist pass, every tick after the behaviors' think: each piloted ship with assists notes which controls its
/// pilot uses and whether the pilot dropped a lock (auto-lock's pause); on think ticks (or right after a switch) its
/// rules decide the channels, all from one snapshot; then the stations are kept.
pub(crate) fn assist_all(ships: &mut [Ship], rocks: &[Rock], lib: &BehaviorLibrary, scratch: &mut Scratch, tick: u64, flight: &FlightTuning) {
    let mut any_due = false;
    for k in 0..ships.len() {
        let Some(&input) = piloted(&ships[k]) else {
            continue;
        };
        let used = controls_used(&input, flight.stick_deadzone);
        let held = ships[k].assists.as_ref().expect("piloted() checked it").held;
        // Held on the tick before and gone now while its ship is still there: the pilot let go of it.
        let dropped = input.lock.is_none() && held.is_some_and(|(t, at)| at + 1 == tick && lock_there(ships, &t));
        let st = ships[k].assists.as_mut().expect("piloted() checked it");
        for (u, now) in st.used.iter_mut().zip(used) {
            if now {
                *u = tick;
            }
        }
        match input.lock {
            // A lock of the pilot's: auto-lock has nothing to ask, and its pause ends.
            Some(t) => {
                st.held = Some((t, tick));
                if std::mem::take(&mut st.paused) {
                    st.thought = None;
                }
                st.pick = None;
            }
            // The rules are read again at once, so the pause shows on this tick.
            None if dropped => {
                st.paused = true;
                st.pick = None;
                st.thought = None;
            }
            None => {}
        }
        any_due |= st.thought.is_none() || tick.is_multiple_of(tuning::THINK_PERIOD);
    }
    if any_due {
        think_all(ships, rocks, lib, scratch, tick);
    }
    keep_stations(ships, flight);
}

/// The think part of [`assist_all`]: every due piloted ship's rules, from one snapshot.
fn think_all(ships: &mut [Ship], rocks: &[Rock], lib: &BehaviorLibrary, scratch: &mut Scratch, tick: u64) {
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
            sensor: 0.0,
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
        think(&mut cx, rocks, st, input, out);
    }
    for (s, out) in ships.iter_mut().zip(assist_outs.iter_mut()) {
        if !std::mem::take(&mut out.valid) {
            continue;
        }
        if let Some(st) = s.assists.as_mut() {
            st.focus = out.focus;
            st.acquired = out.acquired;
            st.decides = out.decides;
            st.avoid = out.avoid;
            st.pick = out.pick;
            st.thought = Some(tick);
        }
    }
}

/// Every tick after the thinks, for each piloted ship: the pilot's thrust drops the station; else a `hold_station`
/// rule that decides thrust keeps the one it took (its ally still a vessel) or takes one, and while another rule (or
/// none) has thrust the station waits as long as the rule that took it is switched on.
fn keep_stations(ships: &mut [Ship], flight: &FlightTuning) {
    for k in 0..ships.len() {
        let me = &ships[k];
        let Some(input) = piloted(me) else {
            continue;
        };
        let st = me.assists.as_ref().expect("piloted() checked it");
        let kept = st.station.filter(|s| s.ally.is_none_or(|a| find_ship(ships, a).is_some_and(Ship::is_vessel)));
        let station = if controls_used(input, flight.stick_deadzone)[1] {
            None
        } else {
            match st.decider(Channel::Thrust) {
                Some((rule, AssistAction::HoldStation(kind))) => match kept {
                    Some(s) if s.rule == rule => Some(s),
                    _ => take_station(me, ships, rule, kind),
                },
                _ => kept.filter(|s| st.all && st.is_on(s.rule as usize)),
            }
        };
        ships[k].assists.as_mut().expect("piloted() checked it").station = station;
    }
}

/// The station `me` takes now for rule `rule`: its centre of mass, or with `Station::Ally` its offset from the
/// nearest other active vessel of its owner (the first in id order on a tie).
fn take_station(me: &Ship, ships: &[Ship], rule: u16, kind: Station) -> Option<StationPoint> {
    let pos = me.piece(me.control_piece()?)?.com_world();
    let ally = match kind {
        Station::Point => None,
        Station::Ally => ships
            .iter()
            .filter(|s| s.id() != me.id() && s.is_active() && friendly(me.team(), s.team()))
            .filter_map(|s| Some((s.id(), resolve_target(ships, &Target::Ship(s.id()))?.pos)))
            .fold(None, |best: Option<(ShipId, Vec2, f32)>, (id, p)| {
                let d = (p - pos).length_squared();
                if best.is_none_or(|b| d < b.2) { Some((id, p, d)) } else { best }
            }),
    };
    Some(match ally {
        Some((id, p, _)) => StationPoint { rule, ally: Some(id), at: pos - p },
        None => StationPoint { rule, ally: None, at: pos },
    })
}

/// Bounding circle of a piece: its frame origin, its radius about it, its velocity.
fn circle(p: &Piece) -> (Vec2, f32, Vec2) {
    (p.body.pos, p.radius, p.body.linvel)
}

/// The closest approach of two circles moving in straight lines, if they would come within
/// [`tuning::AVOID_CLEARANCE`] of touching within [`tuning::AVOID_AHEAD`] seconds: its time (s) and the miss vector
/// (from `other`'s centre to `me`'s then), with the distance it must be at least.
fn course(me: (Vec2, f32, Vec2), other: (Vec2, f32, Vec2)) -> Option<(f32, Vec2, f32)> {
    let d = other.0 - me.0;
    let w = me.2 - other.2;
    let ww = w.length_squared();
    if ww < 1e-6 {
        return None;
    }
    let t = d.dot(w) / ww;
    let reach = me.1 + other.1 + tuning::AVOID_CLEARANCE;
    let miss = w * t - d;
    (t > 0.0 && t <= tuning::AVOID_AHEAD && miss.length_squared() < reach * reach).then_some((t, miss, reach))
}

/// The piece the ship would meet first (`avoid_collisions`): of every ship, its own other pieces included, or an
/// asteroid rock (GDR-0104; as the body key's pair `(BodyKey::ROCKS, rock)`).
fn threat(me: &Ship, world: &[Ship], rocks: &[Rock]) -> Option<(ShipId, PieceId)> {
    let mine = me.control_piece()?;
    let from = circle(me.piece(mine)?);
    let mut first: Option<(f32, ShipId, PieceId)> = None;
    for s in world {
        for p in s.pieces() {
            if s.id() == me.id() && p.id == mine {
                continue;
            }
            if let Some((t, _, _)) = course(from, circle(p))
                && first.is_none_or(|f| t < f.0)
            {
                first = Some((t, s.id(), p.id));
            }
        }
    }
    for (k, r) in (0u32..).zip(rocks) {
        if let Some((t, _, _)) = course(from, (r.pos, r.radius, Vec2::ZERO))
            && first.is_none_or(|f| t < f.0)
        {
            first = Some((t, BodyKey::ROCKS, PieceId(k)));
        }
    }
    first.map(|(_, s, p)| (s, p))
}

/// The velocity that steers `piece` clear of the piece `avoid`: its own plus the sidestep that makes the closest
/// approach the clearance, along the miss (to the left of the relative motion when head-on). `None` once clear.
fn evade(piece: &Piece, avoid: (ShipId, PieceId), ships: &[Ship], rocks: &[Rock]) -> Option<Vec2> {
    let other = match (BodyKey { ship: avoid.0, piece: avoid.1 }).as_rock() {
        Some(k) => rocks.get(k as usize).map(|r| (r.pos, r.radius, Vec2::ZERO))?,
        None => circle(find_ship(ships, avoid.0)?.piece(avoid.1)?),
    };
    let me = circle(piece);
    let (t, miss, reach) = course(me, other)?;
    let len = miss.length();
    // Head-on: to the left of the relative motion (never zero: `course` needs some).
    let side = (me.2 - other.2).perp();
    let away = if len > 1e-3 { miss * (1.0 / len) } else { side * (1.0 / side.length()) };
    Some(me.2 + away * ((reach - len) / t.max(DT)))
}

/// One ship's think: the target (the pilot's lock), then the rules in order, each deciding its channel if it is
/// switched on, its conditions hold and no rule above decided that channel.
fn think(cx: &mut Ctx, rocks: &[Rock], st: &AssistState, input: &PilotInput, out: &mut AssistOut) {
    let Some(kin) = cx.prepare() else {
        return;
    };
    let code = &st.program.code;
    let focus = match input.lock {
        Some(Target::Ship(id) | Target::Piece(id, _)) => cx.world.binary_search_by_key(&id, |s| s.id()).ok().and_then(|k| {
            let ts = cx.centres[k].or_else(|| resolve_target(cx.world, &Target::Ship(id)))?;
            Some((k, ts, (ts.pos - kin.pos).length()))
        }),
        _ => None,
    };
    cx.focus = focus;
    let focus_id = focus.map(|(k, _, _)| cx.world[k].id());
    cx.acquired = if focus_id == st.focus { st.acquired } else { cx.tick };
    out.decides = [None; CHANNELS];
    out.avoid = None;
    out.pick = None;
    // What `avoid_collisions` would steer clear of, looked for once.
    let mut threat_of: Option<Option<(ShipId, PieceId)>> = None;
    if st.all {
        for (i, rule) in code.rules.iter().enumerate() {
            let channel = Channel::of(rule.action);
            let ch = channel.index();
            if !st.is_on(i) || out.decides[ch].is_some() || (rule.action.needs_target() && focus.is_none()) {
                continue;
            }
            // The pilot dropped a lock: no auto-lock until they lock again or switch it.
            if channel == Channel::Lock && st.paused {
                continue;
            }
            let conds = &code.conds[rule.start as usize..(rule.start + rule.len) as usize];
            if !conds.iter().all(|c| matches!(cx.cond(c), Ok((true, _)))) {
                continue;
            }
            match rule.action {
                AssistAction::AvoidCollisions => {
                    // Nothing on a collision course: the rules below keep the channel.
                    let Some(t) = *threat_of.get_or_insert_with(|| threat(cx.me, cx.world, rocks)) else {
                        continue;
                    };
                    out.avoid = Some(t);
                }
                AssistAction::AutoLock(kind) => {
                    // No ship it would lock (no attacker, say): the rules below may pick another way.
                    let Some((k, _, _)) = cx.select(kind, None) else {
                        continue;
                    };
                    if input.lock.is_none() {
                        out.pick = Some(cx.world[k].id());
                    }
                }
                _ => {}
            }
            out.decides[ch] = Some(i as u16);
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
    piece: &Piece,
    ships: &[Ship],
    rocks: &[Rock],
    flight: &FlightTuning,
    goal: &mut FlightGoal,
    fire: &mut Fire,
) {
    if !st.all {
        return;
    }
    let [turning, thrust, _] = controls_used(input, flight.stick_deadzone);
    // The target is the pilot's lock, which also aims the ship (`pilot_goal`).
    let at = input.lock.as_ref().and_then(|t| resolve_target(ships, t));
    // Turning left free: the pilot doesn't steer or hold a lock.
    let turn_free = !turning && input.lock.is_none();
    let pushed = match st.decider(Channel::Thrust) {
        _ if thrust => None,
        Some((_, AssistAction::Brake)) => Some(Vec2::ZERO),
        Some((_, AssistAction::HoldStation(_))) => st.station.and_then(|s| station_goal(&s, kin, ships, flight, turn_free)),
        Some((_, AssistAction::AvoidCollisions)) => st.avoid.and_then(|a| evade(piece, a, ships, rocks)).map(|v| cap(v, kin.top_speed)),
        _ => None,
    };
    if let Some(v) = pushed {
        goal.velocity = Some(v);
        // With turning free the thrust assist may turn the ship to bring its thrust to bear, as the behaviors'
        // maneuvers do (`HeadingGoal::Auto`: only when the thrusters facing the needed way give less than half the
        // best axis). Only for a change of at least `ASSIST_TURN_DV`, so small corrections the ship can't push never
        // spin it around.
        if turn_free && (v - kin.vel).length() >= tuning::ASSIST_TURN_DV {
            goal.heading = HeadingGoal::Auto;
        }
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

/// The velocity back to the station and with it: an escort of the station point at distance 0 (capped at the top
/// speed). With turning free (`turn`) a ship that must turn around to stop (`Kin::stop_accel` plans with half its best
/// axis then) comes back no faster than it can still turn around in and stop from: the half turn, at its planning
/// angular acceleration, takes `2·√(π / α)`, and the escort's arrival speed alone overshot the station by about the
/// distance flown meanwhile; and within [`tuning::STATION_SLACK`] of the station it only stops (matches the station's
/// velocity), as every small correction there meant two half turns. `None` while its ally can't be found.
fn station_goal(s: &StationPoint, kin: &Kin, ships: &[Ship], flight: &FlightTuning, turn: bool) -> Option<Vec2> {
    let at = match s.ally {
        Some(a) => {
            let ts = resolve_target(ships, &Target::Ship(a))?;
            TargetState { pos: ts.pos + s.at, vel: ts.vel }
        }
        None => TargetState { pos: s.at, vel: Vec2::ZERO },
    };
    let escort = Maneuver::Escort { leader: Target::Point(at.pos), distance: 0.0 };
    let v = escort.goal_for(kin, Some(at), flight).velocity?;
    let opposite = [1, 0, 3, 2][kin.best];
    if !turn || kin.caps[opposite] >= 0.5 * kin.caps[kin.best] || kin.alpha <= 0.0 {
        return Some(v);
    }
    // Near the station it only stops: chasing the last metre turned it around and back for good.
    let d = (at.pos - kin.pos).length();
    if d < tuning::STATION_SLACK {
        return Some(at.vel);
    }
    let t = 2.0 * sqrt(PI / kin.alpha);
    let a = flight.stop_margin * kin.stop_accel();
    let most = a * (sqrt(t * t + 2.0 * d / a) - t);
    let back = v - at.vel;
    let speed = back.length();
    Some(if speed > most { at.vel + back * (most / speed) } else { v })
}

/// `v`, scaled down to `top` if faster (as maneuvers cap their goals, GDR-0064).
fn cap(v: Vec2, top: f32) -> Vec2 {
    let speed = v.length();
    if speed > top && top > 0.0 { v * (top / speed) } else { v }
}
