//! Rhai conditions: content nodes with a `script` field, run sandboxed and deterministically.
//!
//! # Sandbox
//!
//! * `Engine::new_raw()`: no standard packages at all, so no `print`, no time, no file or network access; the
//!   crate is built with `no_module` (no `import`) and `no_time`, and `eval` is disabled.
//! * A script sees only `me` and `target` ([`ShipView`]s with getters, no setters) and its node's
//!   parameters by id, all copies that are overwritten before each run. Strict variables: a typo in a
//!   variable name is a load-time error.
//! * Budget: [`tuning::SCRIPT_MAX_OPS`] operations per evaluation, plus small limits on call depth,
//!   expression depth, strings, arrays, maps, variables and functions. Running out counts as false.
//!
//! # Determinism
//!
//! * Floats are `f32` (`f32_float`), like the sim. `+ − × ÷ %`, comparisons, `abs`, `floor`, `sqrt`, `min`,
//!   `max` are exact IEEE operations, identical on every platform.
//! * The only float operator Rhai would route through the platform's libm is `**` (`powf`); it is disabled.
//!   Transcendental functions come from `sim::math` (pure-Rust libm): `sin`, `cos`, `atan2`, `exp`.
//! * No randomness, no time, no hash-ordered iteration is reachable from a script (object maps are ordered).
//!
//! # Cost
//!
//! A small condition costs about 0.2–0.6 µs native. Rhai allocates a little per evaluation (about one small
//! allocation per native function or getter call, for its function-resolution cache); the rest of the
//! behavior evaluation allocates nothing.

use rhai::{AST, Dynamic, Engine, EvalAltResult, Scope};

use super::tuning;
use crate::math;

/// What a script sees of a ship: `me` (the ship running the behavior) and `target` (its current target;
/// `exists` is false without one, and every number is 0). Angles in radians, counter-clockwise; `bearing` is
/// the target's angle off the nose (−π..π, positive = to the left). Hull, shield, heat and power are shares
/// (0..1), like the `hull`, `shield`, `heat` and `power` conditions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShipView {
    pub exists: bool,
    pub hull: f32,
    pub shield: f32,
    pub heat: f32,
    pub power: f32,
    pub speed: f32,
    /// Weapon reach (m): the range of the longest clear gun.
    pub reach: f32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// Nose angle (0 = world +y).
    pub angle: f32,
    /// Target only: distance from me (m).
    pub distance: f32,
    /// Target only: angle off my nose.
    pub bearing: f32,
    /// Target only: how fast the distance shrinks (m/s; negative = opening).
    pub closing: f32,
    /// Target only: it aims at me.
    pub aims_at_me: bool,
    /// Me only: number of active enemy ships.
    pub enemies: i64,
}

/// Names a script may not use for parameters (they are the API).
pub const SCRIPT_API: &[&str] = &["me", "target"];

/// Why a script condition counted as false.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptFault {
    /// It ran out of its operation budget.
    OpLimit,
    /// It returned something other than `true` or `false`.
    NotBool,
    /// Any other runtime error (unknown property, type mismatch, ...).
    Error,
}

/// The sandboxed engine and every compiled script of the vocabulary. Immutable once built; shared by `Arc`.
pub(crate) struct ScriptHost {
    engine: Engine,
    scripts: Vec<Script>,
}

pub(crate) struct Script {
    /// Content node id.
    pub node: String,
    pub ast: AST,
    pub params: Vec<String>,
}

impl std::fmt::Debug for ScriptHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptHost")
            .field(
                "scripts",
                &self.scripts.iter().map(|s| &s.node).collect::<Vec<_>>(),
            )
            .finish()
    }
}

fn sandboxed_engine() -> Engine {
    let mut e = Engine::new_raw();
    e.set_max_operations(tuning::SCRIPT_MAX_OPS)
        .set_max_call_levels(8)
        .set_max_expr_depths(32, 16)
        .set_max_string_size(256)
        .set_max_array_size(64)
        .set_max_map_size(32)
        .set_max_variables(32)
        .set_max_functions(16)
        .set_max_strings_interned(0)
        .set_strict_variables(true);
    // `**` on floats is `powf`, which goes through the platform libm (not bit-identical everywhere).
    e.disable_symbol("**")
        .disable_symbol("**=")
        .disable_symbol("eval");
    e.on_print(|_| {}).on_debug(|_, _, _| {});

    e.register_type_with_name::<ShipView>("Ship");
    macro_rules! getters {
        ($($field:ident),*) => { $( e.register_get(stringify!($field), |s: &mut ShipView| s.$field); )* };
    }
    getters!(
        exists, hull, shield, heat, power, speed, reach, x, y, vx, vy, angle, distance, bearing,
        closing, aims_at_me, enemies
    );

    // Deterministic math only (exact IEEE operations, or libm through `sim::math`).
    e.register_fn("abs", |x: f32| x.abs())
        .register_fn("abs", |x: i64| x.wrapping_abs())
        .register_fn("min", |a: f32, b: f32| a.min(b))
        .register_fn("max", |a: f32, b: f32| a.max(b))
        .register_fn("min", |a: i64, b: i64| a.min(b))
        .register_fn("max", |a: i64, b: i64| a.max(b))
        .register_fn("clamp", |x: f32, lo: f32, hi: f32| x.max(lo).min(hi))
        .register_fn("floor", |x: f32| x.floor())
        .register_fn("ceil", |x: f32| x.ceil())
        .register_fn("sqrt", math::sqrt)
        .register_fn("sin", math::sin)
        .register_fn("cos", math::cos)
        .register_fn("atan2", math::atan2)
        .register_fn("exp", math::exp)
        .register_fn("pi", || core::f32::consts::PI)
        .register_fn("to_float", |x: i64| x as f32)
        .register_fn("to_int", |x: f32| x as i64);
    e
}

/// The scope a script is compiled and run with: `me`, `target`, then its parameters. Plain variables, not
/// constants: Rhai's optimizer would fold constants into the compiled script. A script may assign to them,
/// but that only changes its own copy, which is overwritten before every run.
fn base_scope(params: &[String]) -> Scope<'static> {
    let mut s = Scope::new();
    s.push("me", ShipView::default());
    s.push("target", ShipView::default());
    for p in params {
        s.push(p.clone(), 0.0f32);
    }
    s
}

macro_rules! setter {
    ($name:ident, $t:ty) => {
        fn $name(scope: &mut Scope<'static>, name: &str, value: $t) {
            if let Some(v) = scope.get_value_mut::<$t>(name) {
                *v = value;
            } else if let Some(d) = scope.get_mut(name) {
                *d = Dynamic::from(value);
            }
        }
    };
}
setter!(set_view, ShipView);
setter!(set_num, f32);

impl ScriptHost {
    pub fn new() -> ScriptHost {
        ScriptHost {
            engine: sandboxed_engine(),
            scripts: Vec::new(),
        }
    }

    /// Compiles a script for a node with these parameter ids. `Err` = a message with line and column.
    pub fn add(&mut self, node: &str, source: &str, params: Vec<String>) -> Result<u32, String> {
        let scope = base_scope(&params);
        let ast = self
            .engine
            .compile_with_scope(&scope, source)
            .map_err(|e| e.to_string())?;
        self.scripts.push(Script {
            node: node.to_string(),
            ast,
            params,
        });
        Ok(self.scripts.len() as u32 - 1)
    }

    pub fn len(&self) -> usize {
        self.scripts.len()
    }

    /// One reusable scope per script (kept by the sim's behavior scratch, so evaluation allocates no scope).
    pub fn scopes(&self) -> Vec<Scope<'static>> {
        self.scripts.iter().map(|s| base_scope(&s.params)).collect()
    }

    /// Runs script `k` with these views and parameter values (in the node's parameter order).
    pub fn eval(
        &self,
        k: u32,
        scope: &mut Scope<'static>,
        me: &ShipView,
        target: &ShipView,
        params: &[f32],
    ) -> Result<bool, ScriptFault> {
        let script = &self.scripts[k as usize];
        // Values are updated in place (no new scope entries, so nothing is allocated). A script that
        // assigned another type to one of them gets a fresh value (allocates, only then).
        set_view(scope, "me", *me);
        set_view(scope, "target", *target);
        for (name, &x) in script.params.iter().zip(params) {
            set_num(scope, name, x);
        }
        let base = scope.len();
        let r = self
            .engine
            .eval_ast_with_scope::<Dynamic>(scope, &script.ast);
        scope.rewind(base);
        match r {
            Ok(d) => d.as_bool().map_err(|_| ScriptFault::NotBool),
            Err(e) => Err(match *e {
                EvalAltResult::ErrorTooManyOperations(_) => ScriptFault::OpLimit,
                _ => ScriptFault::Error,
            }),
        }
    }

    /// The full error message of script `k` run again on the scope as the last `eval` left it (call right
    /// after a failed `eval`; allocates, so only on failure).
    pub fn error_message(&self, k: u32, scope: &mut Scope<'static>) -> String {
        let script = &self.scripts[k as usize];
        let base = scope.len();
        let r = self
            .engine
            .eval_ast_with_scope::<Dynamic>(scope, &script.ast);
        scope.rewind(base);
        match r {
            Ok(d) => format!(
                "`{}` returned {} instead of true or false",
                script.node,
                d.type_name()
            ),
            Err(e) => format!("`{}`: {e}", script.node),
        }
    }
}
