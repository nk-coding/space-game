//! Behavior nodes (the vocabulary: conditions, actions, targets, their parameters) and behaviors (programs written in
//! it).

use super::*;

/// What a behavior node is in a rule (the editor colours them: conditions violet, actions teal).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NodeRole {
    /// Before the arrow: a flag (`shield_down`) or a value that is compared (`hull < 30%`).
    Condition,
    /// After the arrow: the maneuver the ship flies while the rule is active.
    Action,
    /// On the `target` line: picks the ship the rules are about.
    Target,
}

/// The kind of number a value condition yields or a parameter takes. Fixes the units it may be written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Quantity {
    /// Metres (`45m` or `45`), or a share of the ship's weapon reach (`60%`).
    Distance,
    /// A percentage, always written with `%` (`30%`).
    Percent,
    /// Seconds (`5s` or `5`).
    Seconds,
    /// A whole number without unit.
    Count,
}

impl Quantity {
    pub fn describe(self) -> &'static str {
        match self {
            Quantity::Distance => "a distance",
            Quantity::Percent => "a percentage",
            Quantity::Seconds => "a time",
            Quantity::Count => "a count",
        }
    }

    /// A sample value for messages.
    pub fn example(self) -> &'static str {
        match self {
            Quantity::Distance => "60m",
            Quantity::Percent => "30%",
            Quantity::Seconds => "5s",
            Quantity::Count => "2",
        }
    }

    pub fn unit_hint(self) -> &'static str {
        match self {
            Quantity::Distance => "write metres (45m) or a share of the weapon reach (60%)",
            Quantity::Percent => "write a percentage (30%)",
            Quantity::Seconds => "write seconds (5s)",
            Quantity::Count => "write a whole number without unit",
        }
    }

    /// Unit suffix of the number as written for ranges (`m`, `%`, `s`, none).
    pub fn unit_suffix(self) -> &'static str {
        match self {
            Quantity::Distance => "m",
            Quantity::Percent => "%",
            Quantity::Seconds => "s",
            Quantity::Count => "",
        }
    }
}

/// One option of a choice parameter (written as its id: `orbit(45m, cw)`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOption {
    /// snake_case; what the player types.
    pub id: String,
    /// Label in the editor and wiki.
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
}

/// The type of a node parameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParamType {
    Distance,
    Percent,
    Seconds,
    Count,
    /// One of the listed words.
    Choice(Vec<ChoiceOption>),
}

impl ParamType {
    /// The quantity of a numeric parameter (`None` for choices).
    pub fn quantity(&self) -> Option<Quantity> {
        match self {
            ParamType::Distance => Some(Quantity::Distance),
            ParamType::Percent => Some(Quantity::Percent),
            ParamType::Seconds => Some(Quantity::Seconds),
            ParamType::Count => Some(Quantity::Count),
            ParamType::Choice(_) => None,
        }
    }

    pub fn unit_suffix(&self) -> &'static str {
        self.quantity().map_or("", Quantity::unit_suffix)
    }
}

/// A positional parameter of a behavior node (`orbit(radius, dir)`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeParam {
    /// snake_case, unique within the node; also the variable name in a Rhai script.
    pub id: String,
    /// Label in the editor and wiki (translatable: `behavior_node.<node>.params.<id>.name`).
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(rename = "type")]
    pub ty: ParamType,
    /// Value used when a rule leaves it out, written as in a rule (`67%`, `45m`, `cw`). Parameters without a
    /// default must come first and are required.
    #[serde(default)]
    pub default: Option<String>,
    /// Range of the number as written in the parameter's unit (metres, percent, seconds, count).
    /// Reach-relative distances (`%`) are checked against 0–1000% instead.
    #[serde(default)]
    pub min: Option<f32>,
    #[serde(default)]
    pub max: Option<f32>,
}

/// `behavior_nodes/*.ron`: one word of the behavior language (a condition, an action or a target
/// selector). The editor's palette and the wiki list exactly these; the sim implements every built-in id,
/// and a node with a `script` is a custom condition written in Rhai.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorNode {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub role: NodeRole,
    /// Conditions only: the quantity a value condition yields (`hull` → `Percent`); it must be compared in a
    /// rule. `None` = a flag, used bare (`shield_down`).
    #[serde(default)]
    pub value: Option<Quantity>,
    #[serde(default)]
    pub params: Vec<NodeParam>,
    /// Only in a design's PILOT ASSISTS list (GDR-0088): an assist action, or a condition about the pilot's own
    /// input. Behaviors can't use it, and an assist list takes no other action ([`crate::behavior::ListKind`]).
    #[serde(default)]
    pub pilot: bool,
    /// Rhai source of a custom condition (tagged "Rhai" in the editor): an expression or statements that
    /// end in `true` or `false`. `None` = built into the sim.
    #[serde(default)]
    pub script: Option<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(BehaviorNode, Kind::BehaviorNode);

impl BehaviorNode {
    /// Text keys and texts of the parameters and their options (`params.<id>.name`, `.summary`,
    /// `params.<id>.options.<option>.name`, `.summary`), relative to `behavior_node.<node>.`.
    pub fn param_texts(&self) -> Vec<(String, &str)> {
        let mut v = Vec::new();
        for p in &self.params {
            v.push((format!("params.{}.name", p.id), p.name.as_str()));
            if let Some(s) = &p.summary {
                v.push((format!("params.{}.summary", p.id), s.as_str()));
            }
            if let ParamType::Choice(opts) = &p.ty {
                for o in opts {
                    v.push((
                        format!("params.{}.options.{}.name", p.id, o.id),
                        o.name.as_str(),
                    ));
                    if let Some(s) = &o.summary {
                        v.push((
                            format!("params.{}.options.{}.summary", p.id, o.id),
                            s.as_str(),
                        ));
                    }
                }
            }
        }
        v
    }

    /// Mutable access to the same texts, for locale overrides.
    pub fn param_texts_mut(&mut self) -> Vec<(String, &mut String)> {
        let mut v = Vec::new();
        for p in &mut self.params {
            v.push((format!("params.{}.name", p.id), &mut p.name));
            if let Some(s) = &mut p.summary {
                v.push((format!("params.{}.summary", p.id), s));
            }
            if let ParamType::Choice(opts) = &mut p.ty {
                for o in opts {
                    v.push((
                        format!("params.{}.options.{}.name", p.id, o.id),
                        &mut o.name,
                    ));
                    if let Some(s) = &mut o.summary {
                        v.push((format!("params.{}.options.{}.summary", p.id, o.id), s));
                    }
                }
            }
        }
        v
    }
}

/// `behaviors/*.ron`: a behavior program. `target` and `rules` are lines of the text format
/// (`docs/behavior-format.md`); the text view of the editor is exactly `target <target>` followed by the
/// rules, one per line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Behavior {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The target selector (`"nearest"`, `"weakest(engines)"`); default `nearest`.
    #[serde(default)]
    pub target: Option<String>,
    /// The rules in priority order, one line each (`"when hull < 30% -> flee"`).
    pub rules: Vec<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Behavior, Kind::Behavior);

impl Behavior {
    /// Parses `target` and `rules` (syntax only; see [`crate::behavior::check_program`]).
    pub fn program(&self) -> Result<crate::behavior::Program, Vec<crate::behavior::LineError>> {
        crate::behavior::Program::from_parts(self.target.as_deref(), &self.rules)
    }
}
