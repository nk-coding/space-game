//! What the drydock's widgets show, as resources any system can drive: the editing state ([`DrydockUi`]), the
//! status line ([`DrydockStatus`]) and the stats grid ([`DrydockStats`]). The widgets only display these and
//! send [`DrydockAction`](super::DrydockAction)s; they never decide anything.

use data::{Content, Material, Part};
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

/// The overlays (pills top right), drawn over the design by `overlays.rs`; any combination at once.
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
    /// Quarter turns clockwise of the part being placed (Q / E). The mirror and bevel modes are the editor's
    /// (`PlayerDesign::dock().mirror`, `.bevel`).
    pub rot: u8,
    /// The part picked in the palette (content id).
    pub picked: Option<String>,
    /// The palette's category (content id).
    pub category: String,
    /// The palette search; non-empty lists matching parts of every category.
    pub search: String,
}

impl DrydockUi {
    /// Starts on the first category with its first part picked and the armour the design's hull is made of (else
    /// the first armour material), both among what the player has (not locked).
    pub fn new(content: &Content, design: &PlayerDesign) -> Self {
        let category = content.categories.iter().next().map(|c| c.id.clone()).unwrap_or_default();
        let picked = palette_parts(content).find(|p| p.category == category).map(|p| p.id.clone());
        let material = design
            .blueprint()
            .parts
            .iter()
            .filter_map(|p| p.material.as_deref())
            .find(|&m| armor_materials(content).any(|a| a.id == m))
            .map(str::to_string)
            .or_else(|| armor_materials(content).next().map(|m| m.id.clone()))
            .unwrap_or_default();
        Self { tool: Tool::Paint, material, overlays: 0, rot: 0, picked, category, search: String::new() }
    }

    pub fn overlay(&self, o: Overlay) -> bool {
        self.overlays & o.bit() != 0
    }

    pub fn toggle_overlay(&mut self, o: Overlay) {
        self.overlays ^= o.bit();
    }
}

/// The parts the player can build with, in content order: locked parts (content `locked`) wait for salvage or
/// research.
pub fn palette_parts(content: &Content) -> impl Iterator<Item = &Part> + Clone {
    content.parts.iter().filter(|p| !p.locked)
}

/// The armour materials the player can build hull parts in, in content order (the ARMOR pills, keys 1–9).
pub fn armor_materials(content: &Content) -> impl Iterator<Item = &Material> + Clone {
    content.materials.iter().filter(|m| m.armor.is_some() && !m.locked)
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
    /// A thin bar under the cell: the share filled (over 1: over a limit, a notch marks 100 %) and its colour.
    pub bar: Option<(f32, Tone)>,
}

/// The stats grid (`stats.rs` fills it from the design report): the first `stats::MAIN_CELLS` cells are the
/// main grid, two per row with bars, the rest the compact block under it.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct DrydockStats {
    pub cells: Vec<StatCell>,
}
