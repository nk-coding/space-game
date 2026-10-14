//! Ships: blueprints (content ships and drydock designs), their placements, frames and icons.

use super::*;

/// One part in a blueprint. Coordinates in cells; see `geometry` for the conventions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub part: String,
    /// Left column of the rotated footprint (reading refuses values beyond ±[`crate::content::limits::MAX_COORD`]).
    #[serde(deserialize_with = "crate::content::limits::coord")]
    pub x: i32,
    /// Top row of the rotated footprint (likewise within ±[`crate::content::limits::MAX_COORD`]).
    #[serde(deserialize_with = "crate::content::limits::coord")]
    pub y: i32,
    /// Quarter turns clockwise (0..=3).
    #[serde(default)]
    pub rot: u8,
    /// Mirror left-right before rotating.
    #[serde(default)]
    pub flip: bool,
    /// Armour material; required for hull parts, not allowed on other parts.
    #[serde(default)]
    pub material: Option<String>,
    /// Power tier and weight of this placement, overriding the part's defaults (parts that draw power only).
    #[serde(default)]
    pub power: Option<PowerOverride>,
}

/// `frames.ron` / `frames/*.ron`: a drydock frame (GDR-0006), the grid a design is built on. Bigger frames come
/// with drydock expansions. Order in the files = order in the drydock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Grid size `(w, h)` in cells, each side 1 to [`crate::content::validate::MAX_FRAME_SIZE`].
    pub size: (u32, u32),
    /// Not available to the player until unlocked (research): a new game cannot build in it. Content ships of
    /// its size still open and fly.
    #[serde(default)]
    pub locked: bool,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Frame, Kind::Frame);

/// An icon of a pack: `icons/<key>.svg`, kept as SVG text (rasterised by the client at runtime, shown as is
/// by the wiki). Later packs replace icons with the same key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Icon {
    pub key: String,
    pub svg: String,
    pub source: Source,
}

/// `ships/*.ron`: a ship blueprint (one per file by convention).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blueprint {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub faction: String,
    /// Drydock frame `(w, h)` in cells; every placement must fit inside.
    pub size: (u32, u32),
    pub parts: Vec<Placement>,
    /// Behavior (id) this ship flies under when it is not piloted; default: its faction's behavior.
    #[serde(default)]
    pub behavior: Option<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Blueprint, Kind::Ship);
