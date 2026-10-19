//! Evaluation: the think pass (every [`tuning::THINK_PERIOD`] ticks per ship: pick the target, walk the
//! rules, choose the maneuver) and the per-tick flight of the chosen maneuver with fire at will.
//!
//! Every think reads the same snapshot of the world (the state after this tick's inputs and their
//! breakage), writes its result to a per-ship scratch slot, and all results are applied afterwards, so ship
//! order never matters. Nothing here allocates once warmed up, except inside Rhai conditions.

use std::sync::Arc;

use data::{Delivery, Quantity};
use rhai::Scope;

use super::compile::{Action, AimAt, Cond, Flag, OrbitDir, SelectorKind, Sensor, Test};
use super::script::{ScriptFault, ShipView};
use super::{Behavior, BehaviorId, BehaviorLibrary, tuning};
use crate::DT;
use crate::config::FlightTuning;
use crate::ship::design::{PartIdx, Stat};
use crate::flight::{
    AimMode, Fire, FlightGoal, HelmMode, Kin, Target, TargetState, find_ship, lead_point,
    resolve_target,
};
use crate::hash::{StateHash, StateHasher};
use crate::ids::ShipId;
use crate::flight::maneuver::Maneuver;
use crate::ship::mass::to_piece;
use crate::math::{Rot, Vec2, atan2};
use crate::flight::physics::wrap_angle;
use crate::ship::{Piece, Ship};
use crate::voyage::rocks::Rock;

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
    /// A `scan` rule held and stepped aside for the next rule: `started` a scan, or the scanner was not ready (none
    /// working, or a scan charging or cooling down).
    Scan { started: bool },
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
    /// Where a call sent it (GDR-0111: an enemy's scan, where the scanner saw its target): with no target in
    /// sight it flies there and holds; the first target it picks clears it.
    pub(crate) rally: Option<Vec2>,
    /// The rock a `mine` rule cuts while it decides (GDR-0114): its index and the point the mining guns aim at.
    pub(crate) mine: Option<(u32, Vec2)>,
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
            rally: None,
            mine: None,
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
            RuleStatus::Scan { started: true } => "started a scan".into(),
            RuleStatus::Scan { started: false } => "the scanner is not ready".into(),
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
        // Only when set, so streams without a call hash as before.
        if let Some(r) = self.rally {
            h.u8(1);
            h.vec2(r);
        }
        // Only while mining, so behaviors that never mine hash as before (GDR-0114).
        if let Some((rock, at)) = self.mine {
            h.u8(2);
            h.u32(rock);
            h.vec2(at);
        }
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
    /// A `scan` rule started a scan.
    scan: bool,
    /// The rock a deciding `mine` rule cuts (GDR-0114).
    mine: Option<(u32, Vec2)>,
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
            scan: false,
            mine: None,
        }
    }
}

/// Reused buffers of the think pass (owned by the `Sim`; not part of the state).
#[derive(Clone, Debug, Default)]
pub(crate) struct Scratch {
    outs: Vec<Out>,
    /// The pilot assists' think results (`assist.rs`).
    pub(super) assist_outs: Vec<super::assist::AssistOut>,
    /// Hull share per ship index for this pass (NaN = not computed yet).
    pub(super) hull: Vec<f32>,
    /// Centre of mass and velocity of each ship's controlling piece, once per pass (each costs a sin/cos).
    pub(super) centres: Vec<Option<TargetState>>,
    /// One scope per Rhai script of the library.
    pub(crate) scopes: Vec<Scope<'static>>,
    /// Rhai evaluations so far (diagnostics; not part of the state).
    pub(crate) script_evals: u64,
    /// Live missiles homing on each ship (by index), counted before each think pass (`Sim::count_incoming`).
    pub(crate) incoming: Vec<u32>,
}

/// Does this ship think on this tick? Every ship on the same ticks (not staggered by id: a ship that thinks a
/// tick after its enemy reacts a tick sooner, which decided mirror fights, ADR-0037), and on its first tick.
#[inline]
fn due(st: &BehaviorState, tick: u64) -> bool {
    st.thought.is_none() || tick.is_multiple_of(tuning::THINK_PERIOD)
}

/// The snapshot a think pass reads besides the ships: hull shares (filled as asked) and every ship's centre.
pub(super) fn snapshot(ships: &[Ship], hull: &mut Vec<f32>, centres: &mut Vec<Option<TargetState>>) {
    hull.clear();
    hull.resize(ships.len(), f32::NAN);
    centres.clear();
    centres.extend(ships.iter().map(centre));
}

/// The think pass: every ship with a behavior whose turn it is (or every one, with `all`) decides, from one
/// snapshot of the world and the voyage's rocks (`mine`, GDR-0114); then the decisions are applied.
pub(crate) fn think_all(
    ships: &mut [Ship],
    rocks: &[Rock],
    lib: &BehaviorLibrary,
    scratch: &mut Scratch,
    tick: u64,
    all: bool,
) {
    let n = ships.len();
    if scratch.outs.len() < n {
        scratch.outs.resize_with(n, Out::default);
    }
    let any_due = ships.iter().any(|s| {
        s.behavior
            .as_ref()
            .is_some_and(|st| all || due(st, tick))
    });
    if !any_due {
        return;
    }
    snapshot(ships, &mut scratch.hull, &mut scratch.centres);
    let Scratch {
        outs,
        hull,
        centres,
        scopes,
        script_evals,
        incoming,
        ..
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
            if !(all || due(st, tick)) || !me.is_active() {
                continue;
            }
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
                pilot: None,
                incoming: incoming.as_slice(),
            };
            cx.think(st, out, rocks);
            out.script_error = cx.script_error.take();
        }
    }
    for k in 0..n {
        let out = &mut outs[k];
        if !std::mem::take(&mut out.valid) {
            continue;
        }
        if let Some(st) = ships[k].behavior.as_mut() {
            st.focus = out.focus;
            if out.focus.is_some() {
                st.rally = None;
            }
            st.aim_part = out.aim_part;
            st.acquired = out.acquired;
            st.active = out.active;
            st.maneuver = out.maneuver;
            st.mine = out.mine;
            st.slots = out.slots;
            st.thought = Some(tick);
            std::mem::swap(&mut st.status, &mut out.status);
            st.script_error = out.script_error.take();
        }
        if std::mem::take(&mut out.scan) {
            ships[k].start_scan();
        }
    }
}

#[derive(Default)]
pub(super) struct Lazy {
    shield: Option<Option<f32>>,
    shield_down: Option<bool>,
    heat: Option<f32>,
    power: Option<f32>,
    under_attack: Option<bool>,
}

/// One ship's evaluation of its rules (a behavior's think, or the pilot assists' in `assist.rs`) against one
/// snapshot of the world.
pub(super) struct Ctx<'a> {
    pub me: &'a Ship,
    /// Index of `me` in `world`.
    pub my_k: usize,
    pub world: &'a [Ship],
    pub tick: u64,
    pub kin: Option<Kin>,
    pub reach: f32,
    /// How far the ship sees (`Ship::sensor_range`): targets farther away are not picked.
    pub sensor: f32,
    /// Index, centre and distance of the target.
    pub focus: Option<(usize, TargetState, f32)>,
    pub acquired: u64,
    pub hull: &'a mut [f32],
    pub centres: &'a [Option<TargetState>],
    pub lazy: Lazy,
    pub lib: &'a BehaviorLibrary,
    pub scopes: &'a mut [Scope<'static>],
    pub script_evals: &'a mut u64,
    pub script_error: Option<String>,
    /// What the pilot's own conditions read (pilot assists; `None` for a behavior).
    pub pilot: Option<Pilot>,
    /// Live missiles homing on each ship, by index (`missiles_incoming`).
    pub incoming: &'a [u32],
}

/// The pilot as the assists' conditions see it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Pilot {
    pub lock: bool,
    /// Seconds since the pilot last used each control, by [`super::compile::Control`] (`Any`, `Turn`, `Thrust`, `Fire`).
    pub idle: [f32; 4],
}

/// The ship a ship aims at: its behavior's target, or its pilot's lock.
pub(crate) fn aims_at(s: &Ship) -> Option<ShipId> {
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

/// An active vessel of another owner (salvage, dark and dormant vessels are no targets).
fn is_enemy(me: &Ship, s: &Ship) -> bool {
    s.id() != me.id() && !crate::ship::vessel::friendly(me.team(), s.team()) && s.is_active()
}

/// Centre of mass and velocity of a ship's controlling piece.
fn centre(s: &Ship) -> Option<TargetState> {
    let p = s.piece(s.control_piece()?)?;
    Some(TargetState {
        pos: p.com_world(),
        vel: p.body.linvel,
    })
}

/// Hit points left on the controlling piece as a share of the intact ship: the whole design, or for a vessel
/// split off another the intact parts it took along (GDR-0064; it counted against the whole design and fled at
/// once).
pub(crate) fn hull_share(s: &Ship) -> f32 {
    let total = s.hull_base;
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

/// Damage per second of the controlling piece's working guns.
fn firepower(s: &Ship) -> f32 {
    s.control_piece()
        .and_then(|c| s.piece(c))
        .map_or(0.0, |p| p.firepower(s.parts()))
}

/// Longest reach of the controlling piece's working guns (0 without).
fn reach_of(s: &Ship) -> f32 {
    s.control_piece().and_then(|c| s.piece(c)).map_or(0.0, |p| {
        p.guns
            .iter()
            .filter(|g| crate::ship::gun_works(g, s.part(g.part)))
            .fold(0.0, |r, g| r.max(g.range))
    })
}

/// The nearest working part of a kind on a ship's controlling piece (target priority).
fn pick_part(s: &Ship, aim: AimAt, from: Vec2) -> Option<PartIdx> {
    if aim == AimAt::Centre {
        return None;
    }
    let p = s.piece(s.control_piece()?)?;
    let rot = crate::math::Rot::from_angle(p.body.angle);
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
        let d = (p.body.pos + rot.apply(local) - from).length_squared();
        if best.is_none_or(|b| d < b.0) {
            best = Some((d, i));
        }
    }
    best.map(|b| b.1)
}

/// Position and velocity of the point a behavior aims at: a part of the target (if it still works) or the
/// target ship's centre.
fn aim_state(
    ships: &[Ship],
    focus: ShipId,
    part: Option<PartIdx>,
) -> Option<TargetState> {
    if let Some(i) = part {
        let s = find_ship(ships, focus)?;
        if (i as usize) < s.parts().len()
            && s.part(i).alive
            && let Some((pos, vel)) = s.part_world(i)
        {
            return Some(TargetState { pos, vel });
        }
    }
    resolve_target(ships, &Target::Ship(focus))
}

/// The per-tick part: the maneuver's goal (relative to the aim point; `escort` relative to the leader) and fire
/// at will at the aim point (each gun checks its own facing in [`guns_at_will`]).
pub(crate) fn fly(
    st: &BehaviorState,
    kin: &Kin,
    ships: &[Ship],
    tuning_f: &FlightTuning,
) -> (FlightGoal, Fire) {
    // Mining (GDR-0114): only the mining guns fire, at the aim point on the rock.
    if let Some((_, at)) = st.mine {
        let ts = TargetState { pos: at, vel: Vec2::ZERO };
        return (st.maneuver.goal_for(kin, Some(ts), tuning_f), Fire::Mine(ts));
    }
    let ts = st.focus.and_then(|f| aim_state(ships, f, st.aim_part));
    let goal = match st.maneuver {
        m @ Maneuver::Escort { .. } => m.goal(kin, ships, tuning_f),
        m => m.goal_for(kin, ts, tuning_f),
    };
    (goal, ts.map_or(Fire::Hold, Fire::AtWill))
}

/// Fire at will, per gun: a gun fires when the target (its lead point, for projectile guns) is within
/// [`tuning::FIRE_CONE`] of the gun's facing (a missile launcher: the wider `missile::tuning::FIRE_CONE`, its
/// seeker does the rest) and within [`tuning::FIRE_RANGE`] × the gun's reach, measured from its muzzle.
/// Broadside and rear guns fire whenever the target crosses their line (GDR-0034). Point defence fires on its
/// own and is left out (GDR-0115). Fills
/// `guns` per gun slot (`slots` = the design's gun count; no allocation once warmed up) and returns whether
/// any gun fires.
pub(crate) fn guns_at_will(
    piece: &Piece,
    ts: &TargetState,
    slots: usize,
    guns: &mut Vec<bool>,
) -> bool {
    guns.clear();
    guns.resize(slots, false);
    let rot = Rot::from_angle(piece.body.angle);
    let com = piece.body.pos + rot.apply(piece.mass.com);
    let mut any = false;
    for g in piece.guns.iter().filter(|g| g.clear && g.defence.is_none()) {
        let muzzle = piece.body.pos + rot.apply(g.muzzle);
        if (ts.pos - muzzle).length() >= g.range * tuning::FIRE_RANGE {
            continue;
        }
        let aim = match g.delivery {
            Delivery::Projectile { speed, .. } => {
                let v_muzzle =
                    piece.body.linvel + Vec2::cross_scalar(piece.body.angvel, muzzle - com);
                lead_point(muzzle, v_muzzle, ts.pos, ts.vel, speed)
            }
            Delivery::Beam | Delivery::Pulse { .. } | Delivery::Missile { .. } => ts.pos,
        };
        let cone = match g.delivery {
            Delivery::Missile { .. } => crate::combat::missile::tuning::FIRE_CONE,
            _ => tuning::FIRE_CONE,
        };
        let (dir, to) = (rot.apply(g.dir), aim - muzzle);
        if atan2(dir.cross(to), dir.dot(to)).abs() < cone
            && let Some(f) = guns.get_mut(g.slot as usize)
        {
            *f = true;
            any = true;
        }
    }
    any
}

/// Mining fire (GDR-0114): [`guns_at_will`] at the aim point on a rock, the mining guns only.
pub(crate) fn guns_mining(piece: &Piece, ts: &TargetState, slots: usize, guns: &mut Vec<bool>) -> bool {
    guns_at_will(piece, ts, slots, guns);
    for g in piece.guns.iter().filter(|g| g.cut <= 0.0) {
        if let Some(f) = guns.get_mut(g.slot as usize) {
            *f = false;
        }
    }
    guns.contains(&true)
}

/// Below this speed (m/s) a ship counts as standing still: nothing to stop.
const STILL: f32 = 0.1;
/// A body axis whose share of the motion is below this needs no thruster of its own to stop (about 14°).
const STRAIGHT_SHARE: f32 = 0.25;

/// The ship can stop without turning: for each body axis that carries a real share of its motion, a thrust group
/// pushes against it (`can_stop_straight`).
fn can_stop_straight(kin: &Kin) -> bool {
    let speed = kin.vel.length();
    if speed < STILL {
        return true;
    }
    let v = kin.rot.apply_inverse(kin.vel) * (1.0 / speed);
    // Groups: forward (+y), backward (−y), right (+x), left (−x); stopping pushes against the motion.
    let against = |share: f32, plus: usize, minus: usize| {
        share.abs() < STRAIGHT_SHARE || kin.caps[if share > 0.0 { minus } else { plus }] > 0.0
    };
    against(v.y, 0, 1) && against(v.x, 2, 3)
}

impl Ctx<'_> {
    fn kin(&self) -> &Kin {
        self.kin.as_ref().expect("set at the start of a think")
    }

    /// The start of every think: the ship's kinematics and the reach `%` distances resolve against. `None` for a
    /// ship without a controlling piece.
    pub(super) fn prepare(&mut self) -> Option<Kin> {
        let me = self.me;
        let piece = me.control_piece().and_then(|c| me.piece(c))?;
        let kin = Kin::of(piece, me.parts());
        self.reach = if kin.reach > 0.0 {
            kin.reach
        } else {
            tuning::DEFAULT_REACH
        };
        self.sensor = me.sensor_range();
        self.kin = Some(kin);
        Some(kin)
    }

    fn hull_of(&mut self, k: usize) -> f32 {
        if self.hull[k].is_nan() {
            self.hull[k] = hull_share(&self.world[k]);
        }
        self.hull[k]
    }

    fn think(&mut self, st: &BehaviorState, out: &mut Out, rocks: &[Rock]) {
        let me = self.me;
        let Some(kin) = self.prepare() else {
            return;
        };
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
        out.mine = None;
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
                None if rule.action == Action::Scan => {
                    let started = !out.scan && me.can_scan();
                    out.scan |= started;
                    RuleStatus::Scan { started }
                }
                None if rule.action == Action::Mine => match self.rock_to_mine(rocks) {
                    Some(m) => {
                        out.mine = Some(m);
                        active = Some(i);
                        RuleStatus::Active
                    }
                    None => RuleStatus::NoTarget,
                },
                None if rule.needs_target && focus.is_none() => RuleStatus::NoTarget,
                None => {
                    active = Some(i);
                    RuleStatus::Active
                }
            };
        }
        let maneuver = match (active, focus_id, st.rally) {
            // Called with no one in sight: to the rally point (GDR-0111).
            (_, None, Some(at)) => Maneuver::Hold { at },
            (Some(i), focus, _) => match out.mine {
                // Mining: close to the stand-off from the aim point on the rock and face it (GDR-0114).
                Some((_, at)) => Maneuver::Approach {
                    target: Target::Point(at),
                    range: self.mine_standoff(),
                    aim: AimMode::Target,
                },
                None => self.maneuver(code.rules[i].action, focus),
            },
            (None, _, _) => Maneuver::Brake,
        };
        if focus_id.is_none() && st.rally.is_some() {
            out.mine = None;
        }
        out.valid = true;
        out.focus = focus_id;
        out.aim_part = aim_part;
        out.acquired = self.acquired;
        out.active = active.map(|i| i as u16);
        out.maneuver = maneuver;
        out.slots = slots;
    }

    /// The rock a `mine` rule cuts (GDR-0114): the nearest spawned rock with ore left within sensor range (from its
    /// circle), while the controlling piece has a working mining gun with a clear line of fire and the hold a free
    /// unit. Its index and the aim point: the point of its circle nearest the ship.
    fn rock_to_mine(&self, rocks: &[Rock]) -> Option<(u32, Vec2)> {
        let me = self.me;
        let piece = me.control_piece().and_then(|c| me.piece(c))?;
        if me.cargo().used() >= me.cargo_capacity() || !piece.guns.iter().any(|g| g.cut > 0.0 && g.clear && me.part(g.part).alive) {
            return None;
        }
        let from = self.kin().pos;
        let mut best: Option<(f32, usize)> = None;
        for (i, r) in rocks.iter().enumerate() {
            let d = (r.pos - from).length() - r.radius;
            if r.ore > 0 && d <= self.sensor && best.is_none_or(|b| d < b.0) {
                best = Some((d, i));
            }
        }
        let (_, k) = best?;
        let (r, away) = (&rocks[k], from - rocks[k].pos);
        Some((k as u32, r.pos + away * (r.radius / away.length().max(1e-3))))
    }

    /// How far from the aim point on a rock a miner stands (GDR-0114): [`tuning::MINE_STANDOFF`] of its shortest
    /// mining reach.
    fn mine_standoff(&self) -> f32 {
        let me = self.me;
        let piece = me.control_piece().and_then(|c| me.piece(c));
        let reach = piece.map_or(0.0, |p| p.guns.iter().filter(|g| g.cut > 0.0).map(|g| g.range).fold(f32::INFINITY, f32::min));
        reach.min(tuning::DEFAULT_REACH) * tuning::MINE_STANDOFF
    }

    fn maneuver(&self, action: Action, focus: Option<ShipId>) -> Maneuver {
        if let Action::Escort(d) = action {
            // The leader (`ShipCommand::Leader`, GDR-0065) while it is a vessel other than this one; else hold.
            let me = self.me;
            return match me.leader.and_then(|l| find_ship(self.world, l)) {
                Some(l) if l.id() != me.id() && l.is_vessel() => Maneuver::Escort {
                    leader: Target::Ship(l.id()),
                    distance: d.metres(self.reach),
                },
                _ => Maneuver::Brake,
            };
        }
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
                    // By the parity of both ids: half of a fleet circles a target each way, and two ships
                    // circling each other go the same way, whichever has the lower id (ADR-0037).
                    OrbitDir::Auto => self.me.id().0.wrapping_add(id.0).is_multiple_of(2),
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
            Action::Ram => Maneuver::Ram { target },
            Action::Hold | Action::Escort(_) | Action::Scan | Action::Mine => Maneuver::Brake,
        }
    }

    /// Picks the target ship (index, centre, distance) among the enemies within sensor range (the `locked`
    /// selector: the leader's lock, wherever it is). The current target keeps its place unless another is better by
    /// the stickiness margin.
    pub(super) fn select(
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
            let Some(ts) = self.centres[k] else {
                continue;
            };
            let d = (ts.pos - from).length();
            if d > self.sensor {
                continue;
            }
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

    /// A drone more than its hangar's leash from its carrier (centres of mass of the controlling pieces; GDR-0117).
    /// False for every other ship, and for a drone whose carrier is gone (it dies in the drone pass).
    fn off_leash(&self) -> bool {
        let me = self.me;
        let Some(t) = me.drone() else { return false };
        let Some(c) = find_ship(self.world, t.carrier) else { return false };
        let (Some(leash), Some(ts)) = (c.design().hangars.get(t.hangar as usize).map(|h| h.leash), centre(c)) else {
            return false;
        };
        (ts.pos - self.kin().pos).length_squared() > leash * leash
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
    pub(super) fn cond(&mut self, c: &Cond) -> Result<(bool, Option<f32>), Why> {
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
            Test::Flag(Flag::OffLeash) => Ok((self.off_leash(), None)),
            Test::Flag(Flag::CanStopStraight) => Ok((can_stop_straight(self.kin()), None)),
            Test::Flag(Flag::LockHeld) => Ok((self.pilot.ok_or(Why::NoValue)?.lock, None)),
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
                    .zip(self.centres)
                    .filter(|(s, _)| is_enemy(me, s))
                    .filter_map(|(_, c)| *c)
                    .filter(|ts| (ts.pos - from).length() <= r)
                    .count() as f32
            }
            Sensor::MissilesIncoming => self.incoming.get(self.my_k).copied().unwrap_or(0) as f32,
            Sensor::Cycle(period) => {
                self.focus.ok_or(Why::NoTarget)?;
                (self.since_acquired() % period) as f32 * DT
            }
            Sensor::TargetTime => {
                self.focus.ok_or(Why::NoTarget)?;
                self.since_acquired() as f32 * DT
            }
            Sensor::Speed => {
                let kin = self.kin();
                if kin.top_speed <= 0.0 {
                    return Err(Why::NoValue);
                }
                kin.vel.length() / kin.top_speed
            }
            Sensor::Idle(c) => self.pilot.ok_or(Why::NoValue)?.idle[c as usize],
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
            // The reach `%` distances resolve against (the default without working guns).
            reach: self.reach,
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
