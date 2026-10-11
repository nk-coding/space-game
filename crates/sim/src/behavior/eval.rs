//! Evaluation: the think pass (every [`tuning::THINK_PERIOD`] ticks per ship: pick the target, walk the
//! rules, choose the maneuver) and the per-tick flight of the chosen maneuver with fire at will.
//!
//! Every think reads the same snapshot of the world (the state after the previous tick's breakage), writes
//! its result to a per-ship scratch slot, and all results are applied afterwards, so ship order never
//! matters. Nothing here allocates once warmed up, except inside Rhai conditions.

use std::sync::Arc;

use data::Quantity;
use rhai::Scope;

use super::compile::{Action, AimAt, Cond, Flag, OrbitDir, SelectorKind, Sensor, Test};
use super::script::{ScriptFault, ShipView};
use super::{Behavior, BehaviorId, BehaviorLibrary, tuning};
use crate::DT;
use crate::config::{FlightTuning, SimConfig};
use crate::design::{PartIdx, Stat};
use crate::flight::{
    AimMode, FlightGoal, HelmMode, Kin, Target, TargetState, find_ship, resolve_target,
};
use crate::hash::{StateHash, StateHasher};
use crate::ids::ShipId;
use crate::maneuver::Maneuver;
use crate::mass::to_piece;
use crate::math::Vec2;
use crate::physics::wrap_angle;
use crate::ship::Ship;

/// Why a condition did not hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Why {
    /// It was false. `value` is what it measured, in base units (metres, a share 0..1, seconds, a count),
    /// for conditions that measure something.
    False { value: Option<f32> },
    /// It is about the target and there is none (false with or without `not`).
    NoTarget,
    /// There is nothing to measure (shield charge without working shield emitters).
    NoValue,
    /// A Rhai condition failed and counts as false.
    Script(ScriptFault),
}

/// What happened to one rule at the last think (debug view, "test in range").
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RuleStatus {
    /// Not evaluated: a rule above it decided (or the ship has not thought yet).
    Skipped,
    /// This rule decides: its action is being flown.
    Active,
    /// Condition `cond` (0-based within the rule) did not hold.
    Failed { cond: u8, why: Why },
    /// Every condition held, but the action needs a target and there is none.
    NoTarget,
    /// Beyond the ship's working rule slots: inactive.
    NoSlot,
}

/// A ship's running behavior: the program, what it decided at its last think, and why.
#[derive(Clone, Debug)]
pub struct BehaviorState {
    pub(crate) id: BehaviorId,
    pub(crate) program: Arc<Behavior>,
    pub(crate) focus: Option<ShipId>,
    pub(crate) aim_part: Option<PartIdx>,
    /// Tick the current target was picked (timers count from here).
    pub(crate) acquired: u64,
    pub(crate) active: Option<u16>,
    pub(crate) maneuver: Maneuver,
    pub(crate) slots: u32,
    /// Tick of the last think (`None`: thinks on the next tick).
    pub(crate) thought: Option<u64>,
    pub(crate) status: Vec<RuleStatus>,
    /// Message of the last failing Rhai condition, if any failed at the last think.
    pub(crate) script_error: Option<String>,
}

impl BehaviorState {
    pub(crate) fn new(id: BehaviorId, program: Arc<Behavior>) -> BehaviorState {
        let n = program.code.rules.len();
        BehaviorState {
            id,
            program,
            focus: None,
            aim_part: None,
            acquired: 0,
            active: None,
            maneuver: Maneuver::Brake,
            slots: 0,
            thought: None,
            status: vec![RuleStatus::Skipped; n],
            script_error: None,
        }
    }

    /// A copy for a ship that takes over this one's mind (a split-off vessel, a captured hull): same program,
    /// same target and timers, thinks on its next tick. Cheap: the program is shared.
    pub fn inherit(&self) -> BehaviorState {
        BehaviorState {
            thought: None,
            active: None,
            maneuver: Maneuver::Brake,
            status: vec![RuleStatus::Skipped; self.status.len()],
            script_error: None,
            ..self.clone()
        }
    }

    /// The error message of the last failing Rhai condition (at the last think), if any.
    pub fn script_error(&self) -> Option<&str> {
        self.script_error.as_deref()
    }

    pub fn id(&self) -> BehaviorId {
        self.id
    }

    /// The compiled program (its `program` field is the source, for display).
    pub fn program(&self) -> &Arc<Behavior> {
        &self.program
    }

    /// Index of the rule that decides, if any (none: the ship brakes and holds).
    pub fn active_rule(&self) -> Option<usize> {
        self.active.map(usize::from)
    }

    /// Status of every rule at the last think, in program order.
    pub fn rules(&self) -> &[RuleStatus] {
        &self.status
    }

    /// The target ship the rules are about.
    pub fn target(&self) -> Option<ShipId> {
        self.focus
    }

    /// The part of the target being aimed at (target priority), if any.
    pub fn aim_part(&self) -> Option<PartIdx> {
        self.aim_part
    }

    /// The maneuver being flown.
    pub fn maneuver(&self) -> Maneuver {
        self.maneuver
    }

    /// Working rule slots at the last think (from the controlling piece's working logic cores).
    pub fn slots(&self) -> u32 {
        self.slots
    }

    /// Tick of the last think.
    pub fn last_think(&self) -> Option<u64> {
        self.thought
    }

    /// One line for a debug view: why rule `i` is (not) active. Allocates; UI only.
    pub fn describe(&self, i: usize) -> String {
        let Some(rule) = self.program.program.rules.get(i) else {
            return String::new();
        };
        match self.status.get(i).copied().unwrap_or(RuleStatus::Skipped) {
            RuleStatus::Active => format!("active: {}", rule.action),
            RuleStatus::Skipped => match self.active {
                Some(a) => format!("not reached: rule {} decides", a + 1),
                None => "not evaluated yet".into(),
            },
            RuleStatus::NoSlot => format!(
                "inactive: the working logic cores give {} rule slot(s)",
                self.slots
            ),
            RuleStatus::NoTarget => format!("no target for `{}`", rule.action.name),
            RuleStatus::Failed { cond, why } => {
                let c = &rule.when[cond as usize];
                let measure = self
                    .program
                    .code
                    .rules
                    .get(i)
                    .and_then(|r| {
                        self.program
                            .code
                            .conds
                            .get((r.start + cond as u32) as usize)
                    })
                    .and_then(|c| c.measure);
                match why {
                    Why::False { value: Some(v) } => {
                        format!(
                            "`{c}` is false ({} is {})",
                            c.call.name,
                            format_measure(v, measure)
                        )
                    }
                    Why::False { value: None } => format!("`{c}` is false"),
                    Why::NoTarget => format!("`{c}`: no target"),
                    Why::NoValue => format!("`{c}`: nothing to measure"),
                    Why::Script(f) => format!("`{c}`: script failed ({f:?}), counts as false"),
                }
            }
        }
    }
}

fn format_measure(v: f32, q: Option<Quantity>) -> String {
    match q {
        Some(Quantity::Percent) => format!("{:.0}%", v * 100.0),
        Some(Quantity::Distance) => format!("{v:.0}m"),
        Some(Quantity::Seconds) => format!("{v:.1}s"),
        Some(Quantity::Count) => format!("{v:.0}"),
        None => format!("{v}"),
    }
}

impl StateHash for BehaviorState {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.id.0);
        h.u64(self.program.fingerprint);
        h.u32(self.focus.map_or(u32::MAX, |s| s.0));
        h.u32(self.aim_part.unwrap_or(u32::MAX));
        h.u64(self.acquired);
        h.u32(self.active.map_or(u32::MAX, u32::from));
        self.maneuver.hash_state(h);
        h.u32(self.slots);
        h.u64(self.thought.unwrap_or(u64::MAX));
    }
}

/// The result of one think, before it is applied.
#[derive(Clone, Debug)]
pub(crate) struct Out {
    valid: bool,
    focus: Option<ShipId>,
    aim_part: Option<PartIdx>,
    acquired: u64,
    active: Option<u16>,
    maneuver: Maneuver,
    slots: u32,
    status: Vec<RuleStatus>,
    script_error: Option<String>,
}

impl Default for Out {
    fn default() -> Out {
        Out {
            valid: false,
            focus: None,
            aim_part: None,
            acquired: 0,
            active: None,
            maneuver: Maneuver::Brake,
            slots: 0,
            status: Vec::new(),
            script_error: None,
        }
    }
}

/// Reused buffers of the think pass (owned by the `Sim`; not part of the state).
#[derive(Clone, Debug, Default)]
pub(crate) struct Scratch {
    outs: Vec<Out>,
    /// Hull share per ship index for this pass (NaN = not computed yet).
    hull: Vec<f32>,
    /// One scope per Rhai script of the library.
    pub(crate) scopes: Vec<Scope<'static>>,
    /// Rhai evaluations so far (diagnostics; not part of the state).
    pub(crate) script_evals: u64,
}

/// Does this ship think on this tick? Staggered by ship id so that a fleet spreads its thinks.
#[inline]
fn due(st: &BehaviorState, id: ShipId, tick: u64) -> bool {
    st.thought.is_none() || (tick + id.0 as u64).is_multiple_of(tuning::THINK_PERIOD)
}

/// The think pass: every ship with a behavior whose turn it is (or every one, with `all`) decides, from one
/// snapshot of the world; then the decisions are applied.
pub(crate) fn think_all(
    ships: &mut [Ship],
    lib: &BehaviorLibrary,
    scratch: &mut Scratch,
    tick: u64,
    cfg: &SimConfig,
    all: bool,
) {
    let n = ships.len();
    if scratch.outs.len() < n {
        scratch.outs.resize_with(n, Out::default);
    }
    scratch.hull.clear();
    scratch.hull.resize(n, f32::NAN);
    let Scratch {
        outs,
        hull,
        scopes,
        script_evals,
    } = scratch;
    {
        let world: &[Ship] = ships;
        for k in 0..n {
            let out = &mut outs[k];
            out.valid = false;
            let me = &world[k];
            let Some(st) = &me.behavior else {
                continue;
            };
            if !(all || due(st, me.id(), tick)) || !me.is_active() {
                continue;
            }
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
                lazy: Lazy::default(),
                lib,
                scopes,
                script_evals,
                script_error: None,
            };
            cx.think(st, cfg, out);
            out.script_error = cx.script_error.take();
        }
    }
    for k in 0..n {
        let out = &mut outs[k];
        if !out.valid {
            continue;
        }
        if let Some(st) = ships[k].behavior.as_mut() {
            st.focus = out.focus;
            st.aim_part = out.aim_part;
            st.acquired = out.acquired;
            st.active = out.active;
            st.maneuver = out.maneuver;
            st.slots = out.slots;
            st.thought = Some(tick);
            std::mem::swap(&mut st.status, &mut out.status);
            st.script_error = out.script_error.take();
        }
    }
}

#[derive(Default)]
struct Lazy {
    shield: Option<Option<f32>>,
    shield_down: Option<bool>,
    heat: Option<f32>,
    power: Option<f32>,
    under_attack: Option<bool>,
}

struct Ctx<'a> {
    me: &'a Ship,
    /// Index of `me` in `world`.
    my_k: usize,
    world: &'a [Ship],
    tick: u64,
    kin: Option<Kin>,
    reach: f32,
    /// Index, centre and distance of the target.
    focus: Option<(usize, TargetState, f32)>,
    acquired: u64,
    hull: &'a mut [f32],
    lazy: Lazy,
    lib: &'a BehaviorLibrary,
    scopes: &'a mut [Scope<'static>],
    script_evals: &'a mut u64,
    script_error: Option<String>,
}

/// The ship a ship aims at: its brain's or behavior's target, or its pilot's lock.
pub(crate) fn aims_at(s: &Ship) -> Option<ShipId> {
    if let Some(b) = &s.brain {
        return b.target;
    }
    if let Some(b) = &s.behavior {
        return b.focus;
    }
    match &s.helm {
        HelmMode::Pilot(p) => match p.lock {
            Some(Target::Ship(id)) | Some(Target::Piece(id, _)) => Some(id),
            _ => None,
        },
        _ => None,
    }
}

fn is_enemy(me: &Ship, s: &Ship) -> bool {
    s.id() != me.id() && s.design().faction != me.design().faction && s.is_active()
}

/// Centre of mass and velocity of a ship's controlling piece.
fn centre(s: &Ship) -> Option<TargetState> {
    let p = s.piece(s.control_piece()?)?;
    Some(TargetState {
        pos: p.com_world(),
        vel: p.body.linvel,
    })
}

/// Hit points left on the controlling piece as a share of the intact ship.
pub(crate) fn hull_share(s: &Ship) -> f32 {
    let total = s.design().hp_total;
    let Some(p) = s.control_piece().and_then(|c| s.piece(c)) else {
        return 0.0;
    };
    if total <= 0.0 {
        return 0.0;
    }
    let mut hp = 0.0;
    for &i in &p.parts {
        let ps = s.part(i);
        if ps.alive {
            hp += ps.hp;
        }
    }
    hp / total
}

fn shield_share(s: &Ship) -> Option<f32> {
    let p = s.piece(s.control_piece()?)?;
    let (mut left, mut cap) = (0.0, 0.0);
    for &i in &p.shields {
        let ps = s.part(i);
        if ps.alive && !ps.overheated {
            left += ps.shield;
            cap += s.stats(i).get(Stat::ShieldCapacity);
        }
    }
    (cap > 0.0).then(|| left / cap)
}

fn shield_down(s: &Ship) -> bool {
    s.control_piece().and_then(|c| s.piece(c)).is_some_and(|p| {
        p.shields
            .iter()
            .any(|&i| s.part(i).alive && s.part(i).shield_down)
    })
}

fn heat_share(s: &Ship) -> f32 {
    let Some(p) = s.control_piece().and_then(|c| s.piece(c)) else {
        return 0.0;
    };
    let mut worst = 0.0f32;
    for &i in &p.parts {
        let limit = s.stats(i).get(Stat::HeatLimit);
        if limit.is_finite() && limit > 0.0 {
            worst = worst.max(s.part(i).heat / limit);
        }
    }
    worst
}

fn power_share(s: &Ship) -> f32 {
    match s.control_piece().and_then(|c| s.piece(c)) {
        Some(p) if p.power.demand > 0.0 => p.power.served / p.power.demand,
        _ => 1.0,
    }
}

/// Damage per second of the controlling piece's clear guns.
fn firepower(s: &Ship) -> f32 {
    s.control_piece().and_then(|c| s.piece(c)).map_or(0.0, |p| {
        p.guns.iter().filter(|g| g.clear).map(|g| g.dps).sum()
    })
}

fn reach_of(s: &Ship) -> f32 {
    s.control_piece().and_then(|c| s.piece(c)).map_or(0.0, |p| {
        p.guns
            .iter()
            .filter(|g| g.clear)
            .fold(0.0, |r, g| r.max(g.range))
    })
}

/// World position of a part's centroid (its piece must exist).
fn part_pos(s: &Ship, i: PartIdx) -> Option<(Vec2, &crate::ship::Piece)> {
    let ps = s.part(i);
    if !ps.alive {
        return None;
    }
    let piece = s.piece(ps.piece)?;
    let local = to_piece(s.design().parts[i as usize].centroid, piece.origin);
    Some((piece.body.to_world(local), piece))
}

/// The nearest working part of a kind on a ship's controlling piece (target priority).
fn pick_part(s: &Ship, aim: AimAt, from: Vec2) -> Option<PartIdx> {
    if aim == AimAt::Centre {
        return None;
    }
    let p = s.piece(s.control_piece()?)?;
    let mut best: Option<(f32, PartIdx)> = None;
    for &i in &p.parts {
        let spec = &s.design().parts[i as usize];
        let ps = s.part(i);
        let kind = match aim {
            AimAt::Engines => spec.is_engine,
            AimAt::Weapons => spec.weapon.is_some(),
            AimAt::Power => spec.reactor,
            AimAt::Core => spec.logic,
            AimAt::Centre => false,
        };
        if !kind || !ps.alive || (ps.overheated && aim != AimAt::Core) {
            continue;
        }
        let local = to_piece(spec.centroid, p.origin);
        let d = (p.body.to_world(local) - from).length_squared();
        if best.is_none_or(|b| d < b.0) {
            best = Some((d, i));
        }
    }
    best.map(|b| b.1)
}

/// Position and velocity of the point a behavior aims at: a part of the target (if it still works) or the
/// target ship's centre.
pub(crate) fn aim_state(
    ships: &[Ship],
    focus: ShipId,
    part: Option<PartIdx>,
) -> Option<TargetState> {
    if let Some(i) = part {
        let s = find_ship(ships, focus)?;
        if (i as usize) < s.parts().len()
            && let Some((pos, piece)) = part_pos(s, i)
        {
            let vel =
                piece.body.linvel + Vec2::cross_scalar(piece.body.angvel, pos - piece.com_world());
            return Some(TargetState { pos, vel });
        }
    }
    resolve_target(ships, &Target::Ship(focus))
}

/// The per-tick part: the maneuver's goal (relative to the aim point) and fire at will: fire while the aim
/// point (or its lead point) is within [`tuning::FIRE_CONE`] of the nose and within
/// [`tuning::FIRE_RANGE`] × reach.
pub(crate) fn fly(
    st: &BehaviorState,
    kin: &Kin,
    ships: &[Ship],
    tuning_f: &FlightTuning,
) -> (FlightGoal, bool) {
    let ts = st.focus.and_then(|f| aim_state(ships, f, st.aim_part));
    let goal = st.maneuver.goal_for(kin, ts, tuning_f);
    let fire = match ts {
        Some(ts) if kin.reach > 0.0 => {
            let aim = kin.aim_point(&ts, AimMode::Auto);
            let dist = (ts.pos - kin.pos).length();
            let off = wrap_angle(Kin::nose_heading(aim - kin.pos) - kin.angle).abs();
            dist < kin.reach * tuning::FIRE_RANGE && off < tuning::FIRE_CONE
        }
        _ => false,
    };
    (goal, fire)
}

impl Ctx<'_> {
    fn kin(&self) -> &Kin {
        self.kin.as_ref().expect("set at the start of a think")
    }

    fn hull_of(&mut self, k: usize) -> f32 {
        if self.hull[k].is_nan() {
            self.hull[k] = hull_share(&self.world[k]);
        }
        self.hull[k]
    }

    fn think(&mut self, st: &BehaviorState, cfg: &SimConfig, out: &mut Out) {
        let me = self.me;
        let Some(piece) = me.control_piece().and_then(|c| me.piece(c)) else {
            return;
        };
        let kin = Kin::of(piece, &cfg.flight);
        self.reach = if kin.reach > 0.0 {
            kin.reach
        } else {
            tuning::DEFAULT_REACH
        };
        self.kin = Some(kin);
        let code = &st.program.code;

        // Target.
        let focus = self.select(code.target.kind, st.focus);
        self.focus = focus;
        let focus_id = focus.map(|(k, _, _)| self.world[k].id());
        self.acquired = if focus_id == st.focus {
            st.acquired
        } else {
            self.tick
        };
        let aim_part =
            focus.and_then(|(k, _, _)| pick_part(&self.world[k], code.target.aim, kin.pos));

        // Rules, in priority order, within the working slots.
        let slots = me.rule_slots();
        out.status.clear();
        out.status.resize(code.rules.len(), RuleStatus::Skipped);
        let mut active: Option<usize> = None;
        for (i, rule) in code.rules.iter().enumerate() {
            if i as u32 >= slots {
                out.status[i] = RuleStatus::NoSlot;
                continue;
            }
            if active.is_some() {
                continue;
            }
            let conds = &code.conds[rule.start as usize..(rule.start + rule.len) as usize];
            let mut failed = None;
            for (j, c) in conds.iter().enumerate() {
                match self.cond(c) {
                    Ok((true, _)) => {}
                    Ok((false, value)) => {
                        failed = Some((j, Why::False { value }));
                        break;
                    }
                    Err(why) => {
                        failed = Some((j, why));
                        break;
                    }
                }
            }
            out.status[i] = match failed {
                Some((j, why)) => RuleStatus::Failed { cond: j as u8, why },
                None if rule.action.needs_target() && focus.is_none() => RuleStatus::NoTarget,
                None => {
                    active = Some(i);
                    RuleStatus::Active
                }
            };
        }
        let maneuver = match (active, focus_id) {
            (Some(i), focus) => self.maneuver(code.rules[i].action, focus),
            (None, _) => Maneuver::Brake,
        };
        out.valid = true;
        out.focus = focus_id;
        out.aim_part = aim_part;
        out.acquired = self.acquired;
        out.active = active.map(|i| i as u16);
        out.maneuver = maneuver;
        out.slots = slots;
    }

    fn maneuver(&self, action: Action, focus: Option<ShipId>) -> Maneuver {
        let Some(id) = focus else {
            return Maneuver::Brake;
        };
        let target = Target::Ship(id);
        let r = self.reach;
        match action {
            Action::Approach(d) => Maneuver::Approach {
                target,
                range: d.metres(r),
                aim: AimMode::Auto,
            },
            Action::Orbit(d, dir) => Maneuver::Orbit {
                target,
                radius: d.metres(r),
                ccw: match dir {
                    OrbitDir::Auto => self.me.id().0.is_multiple_of(2),
                    OrbitDir::Ccw => true,
                    OrbitDir::Cw => false,
                },
            },
            Action::Kite(d) => Maneuver::KeepRange {
                target,
                range: d.metres(r),
                aim: AimMode::Auto,
            },
            Action::Face => Maneuver::FaceTarget {
                target,
                aim: AimMode::Auto,
            },
            Action::Flee => Maneuver::Flee { from: target },
            Action::Hold => Maneuver::Brake,
        }
    }

    /// Picks the target ship (index, centre, distance). The current target keeps its place unless another
    /// is better by the stickiness margin.
    fn select(
        &mut self,
        kind: SelectorKind,
        current: Option<ShipId>,
    ) -> Option<(usize, TargetState, f32)> {
        let me = self.me;
        let from = self.kin().pos;
        if kind == SelectorKind::Locked {
            let leader = me.leader.and_then(|l| find_ship(self.world, l))?;
            let id = aims_at(leader)?;
            let k = self.world.binary_search_by_key(&id, |s| s.id()).ok()?;
            if id == me.id() {
                return None;
            }
            let ts =
                centre(&self.world[k]).or_else(|| resolve_target(self.world, &Target::Ship(id)))?;
            return Some((k, ts, (ts.pos - from).length()));
        }
        let mut best: Option<(f32, f32, usize, TargetState)> = None;
        for k in 0..self.world.len() {
            let s = &self.world[k];
            if !is_enemy(me, s) {
                continue;
            }
            let Some(ts) = centre(s) else {
                continue;
            };
            let d = (ts.pos - from).length();
            let mut cost = match kind {
                SelectorKind::Nearest | SelectorKind::Locked => d,
                SelectorKind::Weakest => self.hull_of(k),
                SelectorKind::Threat => {
                    let aiming = if aims_at(s) == Some(me.id()) {
                        tuning::THREAT_AIMING
                    } else {
                        1.0
                    };
                    (d + tuning::THREAT_DISTANCE) / (firepower(s) * aiming + 1e-3)
                }
                SelectorKind::Attacker => {
                    if aims_at(s) != Some(me.id()) {
                        continue;
                    }
                    d
                }
            };
            if Some(s.id()) == current {
                cost *= tuning::STICKY;
            }
            if best.is_none_or(|b| cost < b.0 || (cost == b.0 && d < b.1)) {
                best = Some((cost, d, k, ts));
            }
        }
        best.map(|(_, d, k, ts)| (k, ts, d))
    }

    fn under_attack(&mut self) -> bool {
        if let Some(v) = self.lazy.under_attack {
            return v;
        }
        let me = self.me;
        let v = self
            .world
            .iter()
            .any(|s| is_enemy(me, s) && aims_at(s) == Some(me.id()));
        self.lazy.under_attack = Some(v);
        v
    }

    fn shield(&mut self) -> Option<f32> {
        *self
            .lazy
            .shield
            .get_or_insert_with(|| shield_share(self.me))
    }

    fn heat(&mut self) -> f32 {
        *self.lazy.heat.get_or_insert_with(|| heat_share(self.me))
    }

    fn power(&mut self) -> f32 {
        *self.lazy.power.get_or_insert_with(|| power_share(self.me))
    }

    fn since_acquired(&self) -> u64 {
        self.tick.saturating_sub(self.acquired)
    }

    /// `Ok((holds, measured value))`, or why it cannot be decided (counts as false).
    fn cond(&mut self, c: &Cond) -> Result<(bool, Option<f32>), Why> {
        let (holds, value) = self.test(&c.test)?;
        Ok((holds != c.not, value))
    }

    fn test(&mut self, t: &Test) -> Result<(bool, Option<f32>), Why> {
        match *t {
            Test::Flag(Flag::ShieldDown) => {
                let v = *self
                    .lazy
                    .shield_down
                    .get_or_insert_with(|| shield_down(self.me));
                Ok((v, None))
            }
            Test::Flag(Flag::UnderAttack) => Ok((self.under_attack(), None)),
            Test::Flag(Flag::TargetOnMe) => {
                let (k, _, _) = self.focus.ok_or(Why::NoTarget)?;
                Ok((aims_at(&self.world[k]) == Some(self.me.id()), None))
            }
            Test::InRange(d) => {
                let (_, _, dist) = self.focus.ok_or(Why::NoTarget)?;
                Ok((dist <= d.metres(self.reach), Some(dist)))
            }
            Test::Value { sensor, op, rhs } => {
                let v = self.sensor(sensor)?;
                Ok((op.test(v, rhs.metres(self.reach)), Some(v)))
            }
            Test::Script { script, params, n } => {
                let mut values = [0.0f32; data::behavior::MAX_ARGS];
                for (k, p) in params.iter().take(n as usize).enumerate() {
                    values[k] = p.metres(self.reach);
                }
                let me_view = self.view_me();
                let target_view = self.view_target();
                let vocab = self
                    .lib
                    .vocab
                    .as_ref()
                    .ok_or(Why::Script(ScriptFault::Error))?;
                let scope = self
                    .scopes
                    .get_mut(script as usize)
                    .ok_or(Why::Script(ScriptFault::Error))?;
                *self.script_evals += 1;
                match vocab.scripts.eval(
                    script,
                    scope,
                    &me_view,
                    &target_view,
                    &values[..n as usize],
                ) {
                    Ok(b) => Ok((b, None)),
                    Err(fault) => {
                        // Rare: only now is the message built (allocates).
                        self.script_error = Some(vocab.scripts.error_message(script, scope));
                        Err(Why::Script(fault))
                    }
                }
            }
        }
    }

    fn sensor(&mut self, s: Sensor) -> Result<f32, Why> {
        Ok(match s {
            Sensor::Hull => self.hull_of(self.my_k),
            Sensor::Shield => self.shield().ok_or(Why::NoValue)?,
            Sensor::Heat => self.heat(),
            Sensor::Power => self.power(),
            Sensor::TargetHull => {
                let (k, _, _) = self.focus.ok_or(Why::NoTarget)?;
                self.hull_of(k)
            }
            Sensor::EnemiesNear(d) => {
                let r = d.metres(self.reach);
                let from = self.kin().pos;
                let me = self.me;
                self.world
                    .iter()
                    .filter(|s| is_enemy(me, s))
                    .filter_map(centre)
                    .filter(|ts| (ts.pos - from).length() <= r)
                    .count() as f32
            }
            Sensor::Cycle(period) => {
                self.focus.ok_or(Why::NoTarget)?;
                (self.since_acquired() % period) as f32 * DT
            }
            Sensor::TargetTime => {
                self.focus.ok_or(Why::NoTarget)?;
                self.since_acquired() as f32 * DT
            }
        })
    }

    fn view_me(&mut self) -> ShipView {
        let kin = *self.kin();
        let me = self.me;
        ShipView {
            exists: true,
            hull: self.hull_of(self.my_k),
            shield: self.shield().unwrap_or(0.0),
            heat: self.heat(),
            power: self.power(),
            speed: kin.vel.length(),
            reach: kin.reach,
            x: kin.pos.x,
            y: kin.pos.y,
            vx: kin.vel.x,
            vy: kin.vel.y,
            angle: kin.angle,
            enemies: self.world.iter().filter(|s| is_enemy(me, s)).count() as i64,
            ..ShipView::default()
        }
    }

    fn view_target(&mut self) -> ShipView {
        let Some((k, ts, dist)) = self.focus else {
            return ShipView::default();
        };
        let kin = *self.kin();
        let s = &self.world[k];
        let rel = ts.pos - kin.pos;
        let closing = if dist > 1e-6 {
            -rel.dot(ts.vel - kin.vel) / dist
        } else {
            0.0
        };
        let (shield, heat, power, reach, angle) = (
            shield_share(s).unwrap_or(0.0),
            heat_share(s),
            power_share(s),
            reach_of(s),
            s.piece(s.control_piece().unwrap_or(crate::ids::PieceId(u32::MAX)))
                .map_or(0.0, |p| p.body.angle),
        );
        let aims_at_me = aims_at(s) == Some(self.me.id());
        ShipView {
            exists: true,
            hull: self.hull_of(k),
            shield,
            heat,
            power,
            speed: ts.vel.length(),
            reach,
            x: ts.pos.x,
            y: ts.pos.y,
            vx: ts.vel.x,
            vy: ts.vel.y,
            angle,
            distance: dist,
            bearing: wrap_angle(Kin::nose_heading(rel) - kin.angle),
            closing,
            aims_at_me,
            enemies: 0,
        }
    }
}
