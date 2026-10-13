//! What the drydock's widgets show, as resources any system can drive: the editing state ([`DrydockUi`]), the
//! status line ([`DrydockStatus`]) and the stats grid ([`DrydockStats`]). The widgets only display these and
//! send [`DrydockAction`](super::DrydockAction)s; they never decide anything.

use data::Content;
use bevy::prelude::*;

use crate::ui_kit::Tone;

use super::PlayerDesign;

/// The editing tools (pills above the grid).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Paint,
    Fill,
    Erase,
    Select,
}

/// The overlays (pills top right). Toggle state only until `sim::report` draws them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Heat,
    Mass,
    Bonus,
    Shields,
    Arcs,
}

impl Overlay {
    pub const ALL: [Overlay; 5] = [Overlay::Heat, Overlay::Mass, Overlay::Bonus, Overlay::Shields, Overlay::Arcs];

    fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// The drydock's editing state.
#[derive(Resource, Clone, Debug)]
pub struct DrydockUi {
    pub tool: Tool,
    /// The armour material hull parts are placed in (content id; the ARMOR pills).
    pub material: String,
    overlays: u8,
    pub mirror: bool,
    pub bevel: bool,
    /// Quarter turns clockwise of the part being placed (Q / E).
    pub rot: u8,
    /// The part picked in the palette (content id).
    pub picked: Option<String>,
    /// The palette's category (content id).
    pub category: String,
    /// The palette search; non-empty lists matching parts of every category.
    pub search: String,
}

impl DrydockUi {
    /// Starts on the first category with its first part picked, mirror on, and the armour the design's hull is
    /// made of (else the first armour material).
    pub fn new(content: &Content, design: &PlayerDesign) -> Self {
        let category = content.categories.iter().next().map(|c| c.id.clone()).unwrap_or_default();
        let picked = content.parts.iter().find(|p| p.category == category).map(|p| p.id.clone());
        let material = design
            .blueprint()
            .parts
            .iter()
            .find_map(|p| p.material.clone())
            .or_else(|| content.materials.iter().find(|m| m.armor.is_some()).map(|m| m.id.clone()))
            .unwrap_or_default();
        Self { tool: Tool::Paint, material, overlays: 0, mirror: true, bevel: false, rot: 0, picked, category, search: String::new() }
    }

    pub fn overlay(&self, o: Overlay) -> bool {
        self.overlays & o.bit() != 0
    }

    pub fn toggle_overlay(&mut self, o: Overlay) {
        self.overlays ^= o.bit();
    }
}

/// The status line under the grid and whether the design can fly.
#[derive(Resource, Debug, Default)]
pub struct DrydockStatus {
    /// Why the design can't fly (TEST-FLY is disabled then); `None` = flight-ready.
    pub grounded: Option<String>,
    /// The status line (uppercase).
    pub line: String,
    pub tone: Tone,
    /// The design revision these were made for (`None`: not yet).
    pub revision: Option<u64>,
}

/// One cell of the stats grid (right panel).
#[derive(Clone, Debug, PartialEq)]
pub struct StatCell {
    pub label: String,
    pub value: String,
    pub tone: Tone,
    /// A thin bar under the cell: the share filled and its colour.
    pub bar: Option<(f32, Tone)>,
}

/// The stats grid, two cells per row. Until `sim::report` fills it: the mockup's labels and "—".
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct DrydockStats {
    pub cells: Vec<StatCell>,
}

impl Default for DrydockStats {
    fn default() -> Self {
        // UI chrome until the ship-level stat labels come from content (`stats.ron` on the data branch).
        let cell = |label: &str, tone: Tone, bar: Option<Tone>| StatCell {
            label: label.into(),
            value: "—".into(),
            tone,
            bar: bar.map(|t| (0.0, t)),
        };
        Self {
            cells: vec![
                cell("POWER", Tone::Teal, Some(Tone::Gold)),
                cell("PEAK HEAT", Tone::Coral, Some(Tone::Coral)),
                cell("THRUST/MASS", Tone::Teal, None),
                cell("YAW", Tone::Teal, None),
                cell("FIREPOWER", Tone::Number, None),
                cell("CARGO", Tone::Number, None),
                cell("CONTROL", Tone::Violet, Some(Tone::Violet)),
                cell("DEVICE LOAD", Tone::Teal, None),
            ],
        }
    }
}
