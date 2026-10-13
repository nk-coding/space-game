//! A checked [`Program`] compiled into flat, allocation-free evaluation data: every node name, parameter
//! default and unit is resolved once here, so a think only reads enums and numbers.

use data::behavior::{
    Amount, Call, CmpOp, LineError, LineRef, MAX_ARGS, Program, check_program, quantity_amount,
    resolve_args,
};
use data::{BehaviorNode, ParamType, Quantity};

use super::Vocab;
use crate::TICK_HZ;

/// A number; for distances either metres or a share of the ship's weapon reach (resolved at each think).
/// Shares, seconds and counts are stored as `Metres(x)` (a plain number).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dist {
    Metres(f32),
    Reach(f32),
}

impl Dist {
    #[inline]
    pub fn metres(self, reach: f32) -> f32 {
        match self {
            Dist::Metres(m) => m,
            Dist::Reach(r) => r * reach,
        }
    }
}

/// Where on the target ship to aim (target selector parameter `aim`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AimAt {
    /// The ship's centre of mass.
    #[default]
    Centre,
    /// Its nearest working engine.
    Engines,
    /// Its nearest working gun.
    Weapons,
    /// Its nearest working reactor.
    Power,
    /// Its nearest logic core.
    Core,
}

/// How the target line picks the target ship.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectorKind {
    #[default]
    Nearest,
    Weakest,
    Threat,
    Attacker,
    Locked,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selector {
    pub kind: SelectorKind,
    pub aim: AimAt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    ShieldDown,
    TargetOnMe,
    UnderAttack,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sensor {
    Hull,
    Shield,
    Heat,
    Power,
    TargetHull,
    EnemiesNear(Dist),
    /// Period in ticks.
    Cycle(u64),
    TargetTime,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Test {
    Flag(Flag),
    InRange(Dist),
    Value {
        sensor: Sensor,
        op: CmpOp,
        rhs: Dist,
    },
    Script {
        script: u32,
        params: [Dist; MAX_ARGS],
        n: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cond {
    pub not: bool,
    pub test: Test,
    /// What the measured value is (for debug text), if the condition measures one.
    pub measure: Option<Quantity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrbitDir {
    Auto,
    Ccw,
    Cw,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Approach(Dist),
    Orbit(Dist, OrbitDir),
    Kite(Dist),
    Face,
    Flee,
    Hold,
    /// Keep within this distance of the leader (GDR-0065).
    Escort(Dist),
}

impl Action {
    /// Needs a target to fly (else the rule fails with `NoTarget`). `escort` flies relative to the leader.
    pub fn needs_target(self) -> bool {
        !matches!(self, Action::Hold | Action::Escort(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    /// Range into [`Code::conds`].
    pub start: u32,
    pub len: u32,
    pub action: Action,
}

/// The compiled program.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Code {
    pub target: Selector,
    pub rules: Vec<Rule>,
    pub conds: Vec<Cond>,
}

fn choice_id(node: &BehaviorNode, param: usize, a: Amount) -> &str {
    match (&node.params[param].ty, a) {
        (ParamType::Choice(opts), Amount::Choice(k)) => opts[k].id.as_str(),
        _ => "",
    }
}

fn dist(a: Amount) -> Dist {
    match a {
        Amount::Metres(m) => Dist::Metres(m),
        Amount::Reach(r) => Dist::Reach(r),
        Amount::Share(x) | Amount::Seconds(x) => Dist::Metres(x),
        Amount::Count(n) => Dist::Metres(n as f32),
        Amount::Choice(k) => Dist::Metres(k as f32),
    }
}

fn seconds(a: Amount) -> f32 {
    match a {
        Amount::Seconds(s) => s,
        _ => 0.0,
    }
}

fn aim(id: &str) -> AimAt {
    match id {
        "engines" => AimAt::Engines,
        "weapons" => AimAt::Weapons,
        "power" => AimAt::Power,
        "core" => AimAt::Core,
        _ => AimAt::Centre,
    }
}

/// Checks `program` against the vocabulary and compiles it. Errors as [`check_program`] reports them.
pub(crate) fn compile(program: &Program, vocab: &Vocab) -> Result<Code, Vec<LineError>> {
    let errors = check_program(program, &vocab.nodes);
    if !errors.is_empty() {
        return Err(errors);
    }
    let fail = |line: LineRef, call: &Call, msg: &str| {
        vec![LineError {
            line,
            col: call.col.0,
            message: format!("`{}`: {msg}", call.name),
        }]
    };
    let lookup =
        |line: LineRef, call: &Call| -> Result<(&BehaviorNode, Vec<Amount>), Vec<LineError>> {
            let node = vocab
                .nodes
                .get(&call.name)
                .ok_or_else(|| fail(line, call, "unknown node"))?;
            let args = resolve_args(call, node).ok_or_else(|| fail(line, call, "bad arguments"))?;
            Ok((node, args))
        };

    let mut code = Code::default();
    if let Some(t) = &program.target {
        let (node, args) = lookup(LineRef::Target, &t.call)?;
        let kind = match node.id.as_str() {
            "nearest" => SelectorKind::Nearest,
            "weakest" => SelectorKind::Weakest,
            "threat" => SelectorKind::Threat,
            "attacker" => SelectorKind::Attacker,
            "locked" => SelectorKind::Locked,
            _ => {
                return Err(fail(
                    LineRef::Target,
                    &t.call,
                    "no implementation in the sim",
                ));
            }
        };
        let aim = args
            .first()
            .map_or(AimAt::Centre, |&a| aim(choice_id(node, 0, a)));
        code.target = Selector { kind, aim };
    }
    for (i, r) in program.rules.iter().enumerate() {
        let line = LineRef::Rule(i);
        let start = code.conds.len() as u32;
        for c in &r.when {
            let (node, args) = lookup(line, &c.call)?;
            let test = if let Some(script) = vocab.script_of(&node.id) {
                let mut params = [Dist::Metres(0.0); MAX_ARGS];
                for (k, &a) in args.iter().enumerate().take(MAX_ARGS) {
                    params[k] = dist(a);
                }
                Test::Script {
                    script,
                    params,
                    n: args.len().min(MAX_ARGS) as u8,
                }
            } else if let Some(q) = node.value {
                let cmp = c
                    .cmp
                    .as_ref()
                    .ok_or_else(|| fail(line, &c.call, "needs a comparison"))?;
                let rhs =
                    quantity_amount(&cmp.rhs.value, q).map_err(|m| fail(line, &c.call, &m))?;
                let sensor = match node.id.as_str() {
                    "hull" => Sensor::Hull,
                    "shield" => Sensor::Shield,
                    "heat" => Sensor::Heat,
                    "power" => Sensor::Power,
                    "target_hull" => Sensor::TargetHull,
                    "enemies_near" => Sensor::EnemiesNear(dist(args[0])),
                    "cycle" => {
                        let ticks = (seconds(args[0]) * TICK_HZ as f32 + 0.5) as u64;
                        Sensor::Cycle(ticks.max(1))
                    }
                    "target_time" => Sensor::TargetTime,
                    _ => return Err(fail(line, &c.call, "no implementation in the sim")),
                };
                Test::Value {
                    sensor,
                    op: cmp.op,
                    rhs: dist(rhs),
                }
            } else {
                match node.id.as_str() {
                    "shield_down" => Test::Flag(Flag::ShieldDown),
                    "target_on_me" => Test::Flag(Flag::TargetOnMe),
                    "under_attack" => Test::Flag(Flag::UnderAttack),
                    "target_in_range" => Test::InRange(dist(args[0])),
                    _ => return Err(fail(line, &c.call, "no implementation in the sim")),
                }
            };
            let measure = match test {
                Test::InRange(_) => Some(Quantity::Distance),
                Test::Value { .. } => node.value,
                _ => None,
            };
            code.conds.push(Cond {
                not: c.not,
                test,
                measure,
            });
        }
        let (node, args) = lookup(line, &r.action)?;
        let action = match node.id.as_str() {
            "approach" => Action::Approach(dist(args[0])),
            "orbit" => Action::Orbit(
                dist(args[0]),
                match choice_id(node, 1, args[1]) {
                    "ccw" => OrbitDir::Ccw,
                    "cw" => OrbitDir::Cw,
                    _ => OrbitDir::Auto,
                },
            ),
            "kite" => Action::Kite(dist(args[0])),
            "face" => Action::Face,
            "flee" => Action::Flee,
            "hold" => Action::Hold,
            "escort" => Action::Escort(dist(args[0])),
            _ => return Err(fail(line, &r.action, "no implementation in the sim")),
        };
        code.rules.push(Rule {
            start,
            len: code.conds.len() as u32 - start,
            action,
        });
    }
    Ok(code)
}
