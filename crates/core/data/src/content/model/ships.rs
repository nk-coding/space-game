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
    /// The weapon group of a gun (1 to [`MAX_WEAPON_GROUP`]): which fire input fires it when the player flies the
    /// ship (GDR-0088; behaviors fire every gun at will). `None` is group 1. Weapons only.
    #[serde(default)]
    pub group: Option<u8>,
}

/// Weapon groups a gun can be in (GDR-0088).
pub const MAX_WEAPON_GROUP: u8 = 3;

/// Toggle slots for pilot assists (GDR-0088): the actions ASSIST TOGGLE 1 to 4; an assist rule names the one that
/// switches it in flight (the assist actions' `toggle` parameter, `toggle1` to `toggle4`).
pub const MAX_ASSIST_TOGGLES: u8 = 4;

/// A design's PILOT ASSISTS (GDR-0088): rules in the behavior format (`docs/behavior-format.md` "Pilot assists")
/// that help the player fly the ship; they run only while it is the focus. `target` and `rules` are lines as in a
/// content behavior.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistList {
    /// The target selector (`"nearest"`); default `nearest`. The pilot's lock, while one is held, comes first.
    #[serde(default)]
    pub target: Option<String>,
    /// The assist rules, one line each (`"when idle(thrust) >= 1s -> brake(toggle2)"`).
    #[serde(default)]
    pub rules: Vec<String>,
}

impl AssistList {
    /// Parses `target` and `rules` (syntax only; see [`crate::behavior::check_assists`]).
    pub fn program(&self) -> Result<crate::behavior::Program, Vec<crate::behavior::LineError>> {
        crate::behavior::Program::from_parts(self.target.as_deref(), &self.rules)
    }

    /// The list of a program (`None` for one without rules and target: no assists).
    pub fn of(program: &crate::behavior::Program) -> Option<AssistList> {
        let (target, rules) = program.to_parts();
        (target.is_some() || !rules.is_empty()).then_some(AssistList { target, rules })
    }
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
    /// Its pilot assists (GDR-0088); none: the ship coasts when the player lets go.
    #[serde(default)]
    pub assists: Option<AssistList>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Blueprint, Kind::Ship);
