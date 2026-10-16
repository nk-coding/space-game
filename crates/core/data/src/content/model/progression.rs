//! Progression: research families, research nodes and what they unlock, starts.

use super::*;

/// `families.ron` / `families/*.ron`: a research family (GDR-0024: one tech tree per system family). Salvaged parts
/// give research data per family; research nodes belong to one and are paid in its data. Order in the files =
/// display order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Family {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// System colour key (see [`crate::content::art_keys::SYSTEM_COLORS`]); none = the neutral colour, as for the hull
    /// category.
    #[serde(default)]
    pub color: Option<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Family, Kind::Family);

/// The kind of a research node (GDR-0024): how the research screen draws it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResearchKind {
    /// A new part with a new silhouette and role.
    NewPart,
    /// A sidegrade of an existing part with a trade-off.
    Variant,
    /// A bigger footprint or frame for bigger hulls.
    SizeClass,
    /// A small step (one of a chain), such as a stat step or more mind capacity.
    Refinement,
    /// Needs reverse-engineered alien tech.
    Alien,
    /// Opens a facility of the home shipyard (GDR-0092: the evolution lab).
    Facility,
}

/// What a research node costs: research data per family and materials, paid at once (GDR-0063).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchCost {
    /// Research data per family (id → amount).
    #[serde(default)]
    pub data: BTreeMap<String, f32>,
    /// Materials (id → amount).
    #[serde(default)]
    pub materials: BTreeMap<String, u32>,
}

/// What a research node unlocks for the player who researched it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Unlock {
    /// A `locked` part (id).
    Part(String),
    /// A `locked` material (id).
    Material(String),
    /// A `locked` drydock frame (id).
    Frame(String),
    /// More mind capacity: this many more logic cores the ship-mind runs at once (GDR-0013).
    MindCapacity(u32),
    /// A `locked` facility (id): its building at home opens (GDR-0092: the evolution lab).
    Facility(String),
}

/// `research/*.ron`: a research node. Unlocks at once when paid (GDR-0063); the player's set of researched nodes
/// lives in the save, never in content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchNode {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The family (id) whose tree the node sits in.
    pub family: String,
    pub kind: ResearchKind,
    pub cost: ResearchCost,
    /// Nodes (ids, any family) that must be researched first.
    #[serde(default)]
    pub requires: Vec<String>,
    pub unlocks: Vec<Unlock>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(ResearchNode, Kind::Research);

/// `facilities.ron` / `facilities/*.ron`: a building of the home shipyard that the player opens (GDR-0071: "each area
/// needs states from content"; GDR-0092): its name, the label over it at home, and whether research must open it
/// first. The shipyard places and draws it (`game::shipyard`); the evolution lab is the only one so far.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Facility {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Dark until a research node's [`Unlock::Facility`] opens it.
    #[serde(default)]
    pub locked: bool,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Facility, Kind::Facility);

/// `starts.ron` / `starts/*.ron`: how a new game begins. A new game has every part, material and frame that is
/// not `locked` (a start names no unlocks of its own).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The ship (id) whose copy is the first working design.
    pub ship: String,
    /// Logic cores the ship-mind runs at once at the start (GDR-0013); research adds to it.
    pub mind_capacity: u32,
    /// Voyages (ids) open at the start; clearing them opens the voyages that name them in `opens_after`.
    #[serde(default)]
    pub voyages: Vec<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Start, Kind::Start);
