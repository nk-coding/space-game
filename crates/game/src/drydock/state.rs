//! What the drydock's widgets show, as resources any system can drive: the editing state ([`DrydockUi`]), the
//! status line ([`DrydockStatus`]) and the stats grid ([`DrydockStats`]). The widgets only display these and
//! send [`DrydockAction`](super::DrydockAction)s; they never decide anything. What the player may build with comes
//! from the career's unlocks (`career::Granted`; everything in free mode, `?unlock=all`).

use data::career::Unlocks;
use data::{Content, Material, Part, Unlock};
use bevy::prelude::*;

use crate::career::unlocked_by;
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
    /// the first armour material), both among what the player has unlocked.
    pub fn new(content: &Content, design: &PlayerDesign, unlocks: &Unlocks) -> Self {
        let category = content.categories.iter().next().map(|c| c.id.clone()).unwrap_or_default();
        let picked = palette_parts(content, unlocks).find(|p| p.category == category).map(|p| p.id.clone());
        let open = |m: &str| unlocks.has_material(m) && armor_materials(content).any(|a| a.id == m);
        let material = design
            .blueprint()
            .parts
            .iter()
            .filter_map(|p| p.material.as_deref())
            .find(|&m| open(m))
            .map(str::to_string)
            .or_else(|| armor_materials(content).find(|m| unlocks.has_material(&m.id)).map(|m| m.id.clone()))
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

/// The parts the player can build with (unlocked), in content order.
pub fn palette_parts<'c>(content: &'c Content, unlocks: &'c Unlocks) -> impl Iterator<Item = &'c Part> + Clone {
    content.parts.iter().filter(|p| unlocks.has_part(&p.id))
}

/// The palette's list for a category, or for a search (non-empty: every part whose name contains it): the unlocked
/// parts, then the locked ones research can unlock (shown dimmed as `LOCKED · <research node>`, never picked;
/// GDR-0065: the drydock shows what research gives), each in content order. Locked parts no research node unlocks
/// stay hidden. Returns the part ids and how many of them (the last ones) are locked.
pub fn palette_list(content: &Content, unlocks: &Unlocks, category: &str, search: &str) -> (Vec<String>, usize) {
    let search = search.to_lowercase();
    let listed = |p: &&Part| if search.is_empty() { p.category == category } else { p.name.to_lowercase().contains(&search) };
    let shown = |p: &&Part| unlocks.has_part(&p.id) || unlocked_by(content, &Unlock::Part(p.id.clone())).is_some();
    let (open, locked): (Vec<&Part>, Vec<&Part>) = content.parts.iter().filter(listed).filter(shown).partition(|p| unlocks.has_part(&p.id));
    let n = locked.len();
    (open.into_iter().chain(locked).map(|p| p.id.clone()).collect(), n)
}

/// Every armour material, in content order: the ARMOR pills and keys 1–9. Locked ones are dimmed and refused.
pub fn armor_materials(content: &Content) -> impl Iterator<Item = &Material> + Clone {
    content.materials.iter().filter(|m| m.armor.is_some())
}

/// Is an armour material's pill shown: unlocked, or locked and research can unlock it (like the palette's rows).
pub fn material_shown(content: &Content, unlocks: &Unlocks, material: &str) -> bool {
    unlocks.has_material(material) || unlocked_by(content, &Unlock::Material(material.to_string())).is_some()
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
    /// The voyage-ready line after it (`VOYAGE-READY · CORES 2 / 2`, `NOT VOYAGE-READY · USES BEAM LASER`; the
    /// career's readiness of the fleet with this design as the flagship) and its tone.
    pub voyage: String,
    pub voyage_tone: Tone,
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

#[cfg(test)]
mod tests {
    use data::Career;

    use super::*;

    /// A career that has researched `nodes` (the save's text, as a loaded one).
    fn career(nodes: &[&str]) -> Career {
        let list: Vec<String> = nodes.iter().map(|n| format!("{n:?}")).collect();
        Career::from_ron(&format!("(start: \"rim_awakening\", seed: 1, generator: 1, researched: [{}])", list.join(", "))).unwrap()
    }

    /// The palette offers what the career unlocked and lists the rest after it, locked, in content order.
    #[test]
    fn the_palette_follows_the_unlocks() {
        let content = data::base_content();
        let in_order = |ids: &[&str]| -> Vec<String> { content.parts.iter().filter(|p| ids.contains(&p.id.as_str())).map(|p| p.id.clone()).collect() };
        let new_game = career(&[]).unlocks(&content);
        let (parts, locked) = palette_list(&content, &new_game, "weapons", "");
        assert_eq!(parts, [vec!["pulse_laser".to_string()], in_order(&["beam_laser", "automaton_cannon"])].concat());
        assert_eq!(locked, 2);
        assert!(palette_parts(&content, &new_game).all(|p| !p.locked));
        // Research moves a part up among the unlocked ones.
        let cannon = career(&["automaton_cannon"]);
        let (parts, locked) = palette_list(&content, &cannon.unlocks(&content), "weapons", "");
        assert_eq!(parts, [in_order(&["pulse_laser", "automaton_cannon"]), vec!["beam_laser".to_string()]].concat());
        assert_eq!(locked, 1);
        assert!(cannon.fresh_parts(&content).contains("automaton_cannon"));
        // A search lists every category, locked ones last; free mode locks nothing.
        let (parts, locked) = palette_list(&content, &new_game, "hull", "LASER");
        assert_eq!((parts, locked), (vec!["pulse_laser".to_string(), "beam_laser".to_string()], 1));
        let all = Unlocks::all(&content);
        assert_eq!(palette_list(&content, &all, "weapons", "").1, 0);
        // The first part picked and the armour are unlocked ones: the Runner's composite hull gives way to steel.
        let runner = PlayerDesign::starter(&content, "halvex_runner");
        let ui = DrydockUi::new(&content, &runner, &new_game);
        assert!(new_game.has_part(ui.picked.as_deref().unwrap()));
        assert_eq!(ui.material, "steel");
        assert_eq!(DrydockUi::new(&content, &runner, &all).material, "composite");
        // The ARMOR pills: every armour material; a locked one shows (dimmed) only when research can unlock it.
        assert!(armor_materials(&content).any(|m| m.id == "composite") && armor_materials(&content).all(|m| m.armor.is_some()));
        assert!(material_shown(&content, &new_game, "steel") && material_shown(&content, &new_game, "composite"));
        assert!(!material_shown(&content, &new_game, "reactive") && material_shown(&content, &all, "reactive"));
    }
}
