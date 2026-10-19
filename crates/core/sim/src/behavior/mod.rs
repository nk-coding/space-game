//! Behaviors: the player-edited top layer. **behavior → built-in maneuvers → always-on flight computer.**
//!
//! A behavior (content `behaviors/*.ron`, or text typed by the player) is a target line and a priority list
//! of rules (`docs/behavior-format.md`). At each think the ship picks its target, walks the rules in order
//! and the first rule whose conditions all hold decides the maneuver; guns fire at will at the target. Rules
//! beyond the working rule slots of the ship's logic cores are inactive.
//!
//! * **Vocabulary**: every condition, action and target selector is a content entry (`behavior_nodes/`).
//!   [`BUILTIN_NODES`] lists what the sim implements; [`check_vocabulary`] checks that content and code
//!   agree in both directions (ids, roles, value types, parameter types and options) and compiles the Rhai
//!   scripts of custom conditions.
//! * **Library**: [`BehaviorLibrary::new`] compiles the vocabulary and every content behavior;
//!   [`BehaviorLibrary::add_text`] adds a player's program, [`BehaviorLibrary::add_assists`] a design's pilot
//!   assists. Hand it to the sim with `Sim::set_behaviors`.
//! * **Commands**: `ShipCommand::Behavior(Some(id))` (overrides the helm), `Behavior(None)`,
//!   `ShipCommand::Leader(Some(ship))` (whose lock the `locked` selector follows, and whom `escort` keeps near),
//!   `ShipCommand::Assists(Some(id))` and `ShipCommand::SwitchAssists` (pilot assists, `assist.rs`).
//! * **Cadence**: every ship thinks on the same ticks, every [`tuning::THINK_PERIOD`] (10 Hz,
//!   `tick % period == 0`; not staggered by ship id, ADR-0037), and on the first tick after it gets a
//!   behavior. Between thinks the chosen maneuver is flown every tick (target position, lead and fire-at-will
//!   are per tick).
//! * **Debug view**: `Ship::behavior()` → [`BehaviorState`]: active rule, per-rule [`RuleStatus`] (which
//!   condition failed and what it measured), target, aim part, slots; [`BehaviorState::describe`].
//! * **Determinism**: ordered containers, index-order loops, one snapshot per pass, integer tick timers,
//!   `sim::math` only; Rhai is sandboxed and restricted to deterministic math (`script.rs`).

mod assist;
mod compile;
mod eval;
mod script;

use std::collections::BTreeMap;
use std::sync::Arc;

use data::behavior::{LineError, Program, TextError};
use data::{AssistList, BehaviorNode, Content, Kind, NodeRole, ParamType, Problem, Quantity, Table};

pub use assist::{AssistState, AssistSwitch, Channel, StationPoint};
pub(crate) use assist::{assist_all, assist_plan};
pub use compile::{AimAt, action_decides, action_needs_target};
pub use eval::{BehaviorState, RuleStatus, Why};
pub(crate) use eval::{Scratch, aims_at, fly, guns_at_will, guns_mining, think_all};
pub use script::{SCRIPT_API, ScriptFault, ShipView};

use crate::ship::design::ShipDesign;

/// Placeholder numbers of behavior evaluation.
pub mod tuning {
    /// Ticks between two thinks (3 = 10 Hz at 30 Hz); every ship thinks on the same ticks.
    pub const THINK_PERIOD: u64 = 3;
    /// Fire at will: the target must be within this share of a gun's reach (measured from its muzzle).
    pub const FIRE_RANGE: f32 = 0.933;
    /// Fire at will: and its aim point (lead point, for projectile guns) within this angle of the gun's
    /// facing (rad).
    pub const FIRE_CONE: f32 = 0.12;
    /// Reach (m) assumed for a ship without working guns, for `%` distances and `me.reach`.
    pub const DEFAULT_REACH: f32 = 60.0;
    /// `mine` (GDR-0114): a miner stands this share of its shortest mining reach off the aim point on the rock's
    /// circle (the rock's outline lies inside it, so the beam reaches it with room to spare).
    pub const MINE_STANDOFF: f32 = 0.5;
    /// The current target keeps its place unless another one's cost is lower than `STICKY` × its own.
    pub const STICKY: f32 = 0.8;
    /// `threat` cost = (distance + THREAT_DISTANCE) / (firepower × aiming).
    pub const THREAT_DISTANCE: f32 = 50.0;
    /// `threat`: firepower counts this much more if the ship aims at me.
    pub const THREAT_AIMING: f32 = 2.0;
    /// A thrust assist turns the ship (while turning is free) only for a velocity change of at least this (m/s): small
    /// corrections never spin it around.
    pub const ASSIST_TURN_DV: f32 = 0.5;
    /// `hold_station` on a ship that must turn around to stop (turning free): within this distance (m) of the station
    /// it only stops.
    pub const STATION_SLACK: f32 = 1.0;
    /// `avoid_collisions` looks this far ahead (s): a closest approach later than that is no threat yet.
    pub const AVOID_AHEAD: f32 = 6.0;
    /// `avoid_collisions` keeps this gap (m) between the bounding circles of the two pieces.
    pub const AVOID_CLEARANCE: f32 = 2.0;
    /// Operation budget of one Rhai condition evaluation.
    pub const SCRIPT_MAX_OPS: u64 = 2_000;
}

/// Handle of a behavior in a [`BehaviorLibrary`]: its index, stable for the library's lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BehaviorId(pub u32);

/// Type of a parameter as the sim reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamSig {
    Number(Quantity),
    /// The option ids (any order in content).
    Choice(&'static [&'static str]),
}

/// What the sim implements for one built-in node id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeSig {
    pub id: &'static str,
    pub role: NodeRole,
    pub value: Option<Quantity>,
    pub params: &'static [ParamSig],
    /// Only in pilot assists (`BehaviorNode::pilot`, GDR-0088).
    pub pilot: bool,
}

const AIM: &[ParamSig] = &[ParamSig::Choice(&[
    "centre", "engines", "weapons", "power", "core",
])];
const DIST: &[ParamSig] = &[ParamSig::Number(Quantity::Distance)];
/// An assist's toggle slot and whether it starts on (the assist actions' last two parameters).
const TOGGLE: ParamSig = ParamSig::Choice(&["none", "toggle1", "toggle2", "toggle3", "toggle4"]);
const START: ParamSig = ParamSig::Choice(&["on", "off"]);

const fn node(
    id: &'static str,
    role: NodeRole,
    value: Option<Quantity>,
    params: &'static [ParamSig],
) -> NodeSig {
    NodeSig {
        id,
        role,
        value,
        params,
        pilot: false,
    }
}

/// A node of pilot assists only.
const fn pilot(
    id: &'static str,
    role: NodeRole,
    value: Option<Quantity>,
    params: &'static [ParamSig],
) -> NodeSig {
    NodeSig {
        pilot: true,
        ..node(id, role, value, params)
    }
}

/// Every node the sim implements. Content must define exactly these (plus Rhai-scripted conditions).
pub const BUILTIN_NODES: &[NodeSig] = {
    use NodeRole::{Action, Condition, Target};
    use Quantity::{Count, Percent, Seconds};
    &[
        node("hull", Condition, Some(Percent), &[]),
        node("shield", Condition, Some(Percent), &[]),
        node("shield_down", Condition, None, &[]),
        node("heat", Condition, Some(Percent), &[]),
        node("power", Condition, Some(Percent), &[]),
        node("target_in_range", Condition, None, DIST),
        node("target_hull", Condition, Some(Percent), &[]),
        node("target_on_me", Condition, None, &[]),
        node("under_attack", Condition, None, &[]),
        node("off_leash", Condition, None, &[]),
        node("enemies_near", Condition, Some(Count), DIST),
        node("missiles_incoming", Condition, Some(Count), &[]),
        node(
            "cycle",
            Condition,
            Some(Seconds),
            &[ParamSig::Number(Seconds)],
        ),
        node("target_time", Condition, Some(Seconds), &[]),
        node("speed", Condition, Some(Percent), &[]),
        node("can_stop_straight", Condition, None, &[]),
        pilot("lock_held", Condition, None, &[]),
        pilot(
            "idle",
            Condition,
            Some(Seconds),
            &[ParamSig::Choice(&["any", "turn", "thrust", "fire"])],
        ),
        node("approach", Action, None, DIST),
        node(
            "orbit",
            Action,
            None,
            &[
                ParamSig::Number(Quantity::Distance),
                ParamSig::Choice(&["auto", "ccw", "cw"]),
            ],
        ),
        node("kite", Action, None, DIST),
        node("face", Action, None, &[]),
        node("flee", Action, None, &[]),
        node("ram", Action, None, &[]),
        node("hold", Action, None, &[]),
        node("escort", Action, None, DIST),
        node("scan", Action, None, &[]),
        node("mine", Action, None, &[]),
        node("flare", Action, None, &[]),
        pilot(
            "auto_lock",
            Action,
            None,
            &[ParamSig::Choice(&["nearest", "weakest", "threat", "attacker"]), TOGGLE, START],
        ),
        pilot("brake", Action, None, &[TOGGLE, START]),
        pilot(
            "hold_station",
            Action,
            None,
            &[ParamSig::Choice(&["point", "ally"]), TOGGLE, START],
        ),
        pilot("avoid_collisions", Action, None, &[TOGGLE, START]),
        pilot("auto_flare", Action, None, &[TOGGLE, START]),
        pilot(
            "auto_fire",
            Action,
            None,
            &[ParamSig::Number(Count), TOGGLE, START],
        ),
        node("nearest", Target, None, AIM),
        node("weakest", Target, None, AIM),
        node("threat", Target, None, AIM),
        node("attacker", Target, None, AIM),
        node("locked", Target, None, AIM),
    ]
};

/// The vocabulary of a library: the node table and the compiled Rhai conditions.
#[derive(Debug)]
pub(crate) struct Vocab {
    pub nodes: Table<BehaviorNode>,
    pub scripts: script::ScriptHost,
    /// Node id → script index.
    script_ids: BTreeMap<String, u32>,
}

impl Vocab {
    pub fn script_of(&self, id: &str) -> Option<u32> {
        self.script_ids.get(id).copied()
    }
}

fn node_problem(n: &BehaviorNode, field: impl Into<String>, msg: impl Into<String>) -> Problem {
    use data::Entry;
    Problem::error(msg)
        .at(n.source())
        .entry(Kind::BehaviorNode, &n.id)
        .field(field)
}

fn sig_matches(n: &BehaviorNode, sig: &NodeSig) -> Vec<Problem> {
    let mut out = Vec::new();
    if n.role != sig.role {
        out.push(node_problem(
            n,
            "role",
            format!(
                "the sim implements `{}` as {}, not {}",
                n.id,
                sig.role.noun(),
                n.role.noun()
            ),
        ));
    }
    if n.pilot != sig.pilot {
        out.push(node_problem(
            n,
            "pilot",
            format!(
                "the sim implements `{}` for {}",
                n.id,
                if sig.pilot { "pilot assists only (`pilot: true`)" } else { "behaviors and pilot assists" }
            ),
        ));
    }
    if n.value != sig.value {
        out.push(node_problem(
            n,
            "value",
            format!(
                "the sim implements `{}` with value {:?}, the content says {:?}",
                n.id, sig.value, n.value
            ),
        ));
    }
    if n.params.len() != sig.params.len() {
        out.push(node_problem(
            n,
            "params",
            format!(
                "the sim implements `{}` with {} parameter(s), the content declares {}",
                n.id,
                sig.params.len(),
                n.params.len()
            ),
        ));
        return out;
    }
    for (k, (p, s)) in n.params.iter().zip(sig.params).enumerate() {
        let ok = match (&p.ty, s) {
            (ParamType::Choice(opts), ParamSig::Choice(ids)) => {
                opts.len() == ids.len() && ids.iter().all(|id| opts.iter().any(|o| o.id == *id))
            }
            (ty, ParamSig::Number(q)) => ty.quantity() == Some(*q),
            _ => false,
        };
        if !ok {
            out.push(node_problem(
                n,
                format!("params[{k}].type"),
                format!("the sim implements this parameter as {s:?}"),
            ));
        }
    }
    out
}

/// Builds the vocabulary from content: checks that content nodes and [`BUILTIN_NODES`] agree in both
/// directions and compiles every Rhai condition. Errors are content problems (kind `behavior_node`).
pub(crate) fn build_vocab(nodes: &Table<BehaviorNode>) -> Result<Vocab, Vec<Problem>> {
    let mut problems = Vec::new();
    let mut scripts = script::ScriptHost::new();
    let mut script_ids = BTreeMap::new();
    for n in nodes {
        let builtin = BUILTIN_NODES.iter().find(|s| s.id == n.id);
        match (&n.script, builtin) {
            (None, Some(sig)) => problems.extend(sig_matches(n, sig)),
            (None, None) => {
                let ids: Vec<&str> = BUILTIN_NODES.iter().map(|s| s.id).collect();
                problems.push(node_problem(
                    n,
                    "id",
                    format!(
                        "`{}` has no implementation in the sim; built-in nodes are {}; a custom condition needs a `script`",
                        n.id,
                        ids.join(", ")
                    ),
                ));
            }
            (Some(_), Some(_)) => problems.push(node_problem(
                n,
                "script",
                format!(
                    "`{}` is a built-in node; give the scripted condition its own id",
                    n.id
                ),
            )),
            (Some(src), None) => {
                if n.role != NodeRole::Condition || n.value.is_some() {
                    problems.push(node_problem(
                        n,
                        "script",
                        "only flag conditions can be scripted",
                    ));
                    continue;
                }
                let mut ok = true;
                for (k, p) in n.params.iter().enumerate() {
                    if SCRIPT_API.contains(&p.id.as_str()) {
                        problems.push(node_problem(
                            n,
                            format!("params[{k}].id"),
                            format!(
                                "`{}` is a name of the script API; rename the parameter",
                                p.id
                            ),
                        ));
                        ok = false;
                    }
                    if p.ty.quantity().is_none() {
                        problems.push(node_problem(
                            n,
                            format!("params[{k}].type"),
                            "script parameters must be numbers (choices come later)",
                        ));
                        ok = false;
                    }
                }
                if !ok {
                    continue;
                }
                let params = n.params.iter().map(|p| p.id.clone()).collect();
                match scripts.add(&n.id, src, params) {
                    Ok(k) => {
                        script_ids.insert(n.id.clone(), k);
                    }
                    Err(e) => problems.push(node_problem(n, "script", format!("Rhai: {e}"))),
                }
            }
        }
    }
    for sig in BUILTIN_NODES {
        if !nodes.get(sig.id).is_some_and(|n| n.script.is_none()) {
            problems.push(
                Problem::error(format!(
                    "the sim implements `{}`, but no content node defines it (the editor and wiki need its name and summary)",
                    sig.id
                ))
                .entry(Kind::BehaviorNode, sig.id),
            );
        }
    }
    if problems.is_empty() {
        Ok(Vocab {
            nodes: nodes.clone(),
            scripts,
            script_ids,
        })
    } else {
        Err(problems)
    }
}

/// Checks the behavior vocabulary of loaded content against the sim (see [`build_vocab`]) and that every
/// content behavior compiles. For content tools; `BehaviorLibrary::new` runs the same checks.
pub fn check_vocabulary(content: &Content) -> Vec<Problem> {
    match BehaviorLibrary::new(content) {
        Ok(_) => Vec::new(),
        Err(p) => p,
    }
}

/// A compiled behavior program.
#[derive(Debug)]
pub struct Behavior {
    /// Content id, or the name a player program was added under.
    pub id: String,
    pub name: String,
    /// The source (print it with `to_text()` for the editor's text view or a scan report).
    pub program: Program,
    /// FNV-1a 64 of the canonical text: part of the state hash, so two peers running different programs
    /// under the same id diverge visibly.
    pub fingerprint: u64,
    pub(crate) code: compile::Code,
}

impl Behavior {
    /// Number of rules.
    pub fn rule_count(&self) -> usize {
        self.program.rules.len()
    }
}

/// Handle of a design's pilot assists in a [`BehaviorLibrary`]: its index, stable for the library's lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssistsId(pub u32);

/// A design's compiled pilot assists (GDR-0088; `assist.rs`).
#[derive(Debug)]
pub struct Assists {
    /// The source, as checked (`data::behavior::check_assists`).
    pub program: Program,
    /// FNV-1a 64 of the canonical text: part of the state hash, like a behavior's.
    pub fingerprint: u64,
    pub(crate) code: compile::AssistCode,
}

/// The vocabulary plus every compiled behavior and pilot assist list. Cheap to clone (shared by `Arc`). The sim
/// holds one; ships refer to its programs by [`BehaviorId`] and [`AssistsId`].
#[derive(Clone, Default)]
pub struct BehaviorLibrary {
    vocab: Option<Arc<Vocab>>,
    programs: Vec<Arc<Behavior>>,
    by_id: BTreeMap<String, BehaviorId>,
    assists: Vec<Arc<Assists>>,
}

impl std::fmt::Debug for BehaviorLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BehaviorLibrary")
            .field("behaviors", &self.by_id)
            .field(
                "scripts",
                &self.vocab.as_ref().map_or(0, |v| v.scripts.len()),
            )
            .finish()
    }
}

impl BehaviorLibrary {
    /// Compiles the vocabulary (checked against the sim's implementations, Rhai scripts compiled) and every
    /// content behavior, in load order (their ids are `BehaviorId(0..)` in that order).
    pub fn new(content: &Content) -> Result<BehaviorLibrary, Vec<Problem>> {
        let vocab = Arc::new(build_vocab(&content.behavior_nodes)?);
        let mut lib = BehaviorLibrary {
            vocab: Some(vocab),
            programs: Vec::new(),
            by_id: BTreeMap::new(),
            assists: Vec::new(),
        };
        let mut problems = Vec::new();
        for b in &content.behaviors {
            let result = b.program().and_then(|p| lib.add_program(&b.id, &b.name, p));
            if let Err(errors) = result {
                use data::Entry;
                for e in errors {
                    problems.push(
                        Problem::error(format!("column {}: {}", e.col, e.message))
                            .at(b.source())
                            .entry(Kind::Behavior, &b.id)
                            .field(e.line.field()),
                    );
                }
            }
        }
        if problems.is_empty() {
            Ok(lib)
        } else {
            Err(problems)
        }
    }

    /// Checks and compiles a program and adds it under `id` (a later add with the same id takes over the
    /// name; earlier handles keep working).
    pub fn add_program(
        &mut self,
        id: &str,
        name: &str,
        program: Program,
    ) -> Result<BehaviorId, Vec<LineError>> {
        let Some(vocab) = &self.vocab else {
            return Err(vec![LineError {
                line: data::behavior::LineRef::Whole,
                col: 1,
                message: "this library has no vocabulary (build it with BehaviorLibrary::new)"
                    .into(),
            }]);
        };
        let code = compile::compile(&program, vocab)?;
        let fingerprint = crate::hash::fnv1a64(program.to_text().as_bytes());
        let handle = BehaviorId(self.programs.len() as u32);
        self.programs.push(Arc::new(Behavior {
            id: id.to_string(),
            name: name.to_string(),
            program,
            fingerprint,
            code,
        }));
        self.by_id.insert(id.to_string(), handle);
        Ok(handle)
    }

    /// Checks and compiles a design's pilot assists (its blueprint's `assists`) and adds them; hand the handle to
    /// the design's ships with `ShipCommand::Assists`. Errors by line of the list (`rules[i]`).
    pub fn add_assists(&mut self, list: &AssistList) -> Result<AssistsId, Vec<LineError>> {
        let program = list.program()?;
        let Some(vocab) = &self.vocab else {
            return Err(vec![LineError {
                line: data::behavior::LineRef::Whole,
                col: 1,
                message: "this library has no vocabulary (build it with BehaviorLibrary::new)".into(),
            }]);
        };
        let code = compile::compile_assists(&program, vocab)?;
        let fingerprint = crate::hash::fnv1a64(program.to_text().as_bytes());
        self.assists.push(Arc::new(Assists { program, fingerprint, code }));
        Ok(AssistsId(self.assists.len() as u32 - 1))
    }

    pub fn assists(&self, id: AssistsId) -> Option<&Arc<Assists>> {
        self.assists.get(id.0 as usize)
    }

    /// Parses, checks and adds a program in the text form (a player's edit). Errors by text line and column.
    pub fn add_text(&mut self, id: &str, text: &str) -> Result<BehaviorId, Vec<TextError>> {
        let (program, map) = Program::parse_with_lines(text)?;
        self.add_program(id, id, program)
            .map_err(|errs| errs.into_iter().map(|e| map.to_text_error(e)).collect())
    }

    /// The handle of the newest behavior added under this id.
    pub fn id(&self, id: &str) -> Option<BehaviorId> {
        self.by_id.get(id).copied()
    }

    pub fn get(&self, id: BehaviorId) -> Option<&Arc<Behavior>> {
        self.programs.get(id.0 as usize)
    }

    /// The default behavior of a design (its blueprint's or its faction's), if the library has it.
    pub fn default_for(&self, design: &ShipDesign) -> Option<BehaviorId> {
        design.behavior.as_deref().and_then(|id| self.id(id))
    }

    pub fn len(&self) -> usize {
        self.programs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.programs.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (BehaviorId, &Arc<Behavior>)> {
        self.programs
            .iter()
            .enumerate()
            .map(|(k, b)| (BehaviorId(k as u32), b))
    }

    /// The node table of the vocabulary (empty for an empty library).
    pub fn nodes(&self) -> Option<&Table<BehaviorNode>> {
        self.vocab.as_ref().map(|v| &v.nodes)
    }

    /// Fresh per-script scopes (for the sim's scratch).
    pub(crate) fn script_scopes(&self) -> Vec<rhai::Scope<'static>> {
        self.vocab
            .as_ref()
            .map_or_else(Vec::new, |v| v.scripts.scopes())
    }
}
