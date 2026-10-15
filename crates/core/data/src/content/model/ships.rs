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
/// that help the player fly the ship; they run only while it is the focus. `rules` are lines as in a content
/// behavior. A list has no target line: the assists act on the pilot's lock (an `auto_lock` rule sets one).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "AssistListText")]
pub struct AssistList {
    /// The assist rules, one line each (`"when idle(thrust) >= 1s -> brake(toggle2)"`).
    pub rules: Vec<String>,
}

/// What a blueprint's `assists` reads. The first lists (GDR-0088 step 3, 15 Oct 2026) had a target line and could
/// hold `face_target`: both are read and left out, so those designs still load and are written back without them.
/// No format version changes: what is written now reads in the builds before.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssistListText {
    /// The old target line (the assists picked a target of their own while nothing was locked): ignored.
    #[serde(default, rename = "target")]
    _target: Option<serde::de::IgnoredAny>,
    #[serde(default)]
    rules: Vec<String>,
}

/// Assists that are gone, whose rules old lists may hold: `face_target` turned the guns onto the target, which
/// the lock it now needs already does by itself.
const RETIRED_ASSISTS: &[&str] = &["face_target"];

impl From<AssistListText> for AssistList {
    fn from(t: AssistListText) -> Self {
        let retired = |r: &String| {
            crate::behavior::Program::from_parts(None, std::slice::from_ref(r))
                .is_ok_and(|p| p.rules.iter().any(|rule| RETIRED_ASSISTS.contains(&rule.action.name.as_str())))
        };
        AssistList { rules: t.rules.into_iter().filter(|r| !retired(r)).collect() }
    }
}

impl AssistList {
    /// Parses `rules` (syntax only; see [`crate::behavior::check_assists`]).
    pub fn program(&self) -> Result<crate::behavior::Program, Vec<crate::behavior::LineError>> {
        crate::behavior::Program::from_parts(None, &self.rules)
    }

    /// The list of a program (`None` for one without rules: no assists).
    pub fn of(program: &crate::behavior::Program) -> Option<AssistList> {
        let (_, rules) = program.to_parts();
        (!rules.is_empty()).then_some(AssistList { rules })
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
