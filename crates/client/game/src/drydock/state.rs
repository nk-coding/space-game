//! What the drydock's widgets show, as resources any system can drive: the editing state ([`DrydockUi`]), the
//! status line ([`DrydockStatus`]) and the stats grid ([`DrydockStats`]). The widgets only display these and
//! send [`DrydockAction`](super::DrydockAction)s; they never decide anything. What the player may build with comes
//! from the career's unlocks (`career::Granted`; everything in free mode, `?unlock=all`).

use data::career::Unlocks;
use data::{Content, Material, Part, Unlock};
use bevy::prelude::*;
use kit::ui_kit::Tone;
use world::career::unlocked_by;
use world::player::PlayerDesign;

/// The editing tools (pills in the bottom bar, or its dropdown when the bar is narrow).
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

/// The bottom bar's menus, opening upward: the tools (when the bar is too narrow for their pills) and ⋯ (the rarer
/// actions). One is open at a time; Esc, a click outside and any action close it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarMenu {
    Tools,
    More,
}

/// The drydock's editing state.
#[derive(Resource, Clone, Debug)]
pub struct DrydockUi {
    pub tool: Tool,
    /// The armour material hull parts are placed in (content id; the swatches over the palette).
    pub material: String,
    overlays: u8,
    /// Quarter turns clockwise of the part being placed (R / Shift+R, Q / E, Shift+wheel). The mirror and bevel
    /// modes are the editor's (`PlayerDesign::dock().mirror`, `.bevel`).
    pub rot: u8,
    /// A slope brush's orientation `(rot, flip)` once turned (the same keys step through `slope_orientations`);
    /// `None`: the slope orients itself to the hull. Picking a part sets it back to `None`.
    pub slope_turn: Option<(u8, bool)>,
    /// The part picked in the palette (content id).
    pub picked: Option<String>,
    /// A category chip was clicked: the palette's list jumps to its section (content id; the list takes it).
    pub jump: Option<String>,
    /// The palette search; non-empty lists only the parts whose name contains it.
    pub search: String,
    /// The bottom bar's open menu.
    pub menu: Option<BarMenu>,
}

impl DrydockUi {
    /// Starts with the palette's first unlocked part picked and the armour the design's hull is made of (else the
    /// first armour material), both among what the player has unlocked.
    pub fn new(content: &Content, design: &PlayerDesign, unlocks: &Unlocks) -> Self {
        let picked = palette_sections(content, unlocks, "").into_iter().find_map(|s| s.parts.into_iter().next().filter(|p| unlocks.has_part(p)));
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
        Self { tool: Tool::Paint, material, overlays: 0, rot: 0, slope_turn: None, picked, jump: None, search: String::new(), menu: None }
    }

    pub fn overlay(&self, o: Overlay) -> bool {
        self.overlays & o.bit() != 0
    }

    pub fn toggle_overlay(&mut self, o: Overlay) {
        self.overlays ^= o.bit();
    }
}

/// One section of the palette's list (GDR-0082: every part in sections, a heading per category).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// The category (content id).
    pub category: String,
    /// The unlocked parts, then the locked ones, each in content order.
    pub parts: Vec<String>,
    /// How many of `parts` (the last ones) are locked.
    pub locked: usize,
}

/// The palette's list: a section per category in content order, with its parts that match the search (empty:
/// all; else every part whose name contains it), sections left empty dropped. A section lists the unlocked parts,
/// then the locked ones research can unlock (shown dimmed as `LOCKED · <research node>`, never picked; GDR-0065:
/// the drydock shows what research gives). Locked parts no research node unlocks stay hidden.
pub fn palette_sections(content: &Content, unlocks: &Unlocks, search: &str) -> Vec<Section> {
    let search = search.to_lowercase();
    let listed = |p: &&Part| p.name.to_lowercase().contains(&search);
    let shown = |p: &&Part| unlocks.has_part(&p.id) || unlocked_by(content, &Unlock::Part(p.id.clone())).is_some();
    content
        .categories
        .iter()
        .filter_map(|c| {
            let in_section = |p: &&Part| p.category == c.id;
            let (open, locked): (Vec<&Part>, Vec<&Part>) =
                content.parts.iter().filter(in_section).filter(listed).filter(shown).partition(|p| unlocks.has_part(&p.id));
            let n = locked.len();
            let parts: Vec<String> = open.into_iter().chain(locked).map(|p| p.id.clone()).collect();
            (!parts.is_empty()).then(|| Section { category: c.id.clone(), parts, locked: n })
        })
        .collect()
}

/// Every armour material, in content order: the ARMOR swatches and keys 1–9. Locked ones are dimmed and refused.
pub fn armor_materials(content: &Content) -> impl Iterator<Item = &Material> + Clone {
    content.materials.iter().filter(|m| m.armor.is_some())
}

/// Is an armour material's swatch shown: unlocked, or locked and research can unlock it (like the palette's rows).
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
    /// A small line under the bar: the power drawn in bursts beside full use ("+3 WHILE SCANNING").
    pub note: Option<String>,
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

    /// The section of a category in the palette's list.
    fn section(content: &Content, unlocks: &Unlocks, category: &str, search: &str) -> Option<Section> {
        palette_sections(content, unlocks, search).into_iter().find(|s| s.category == category)
    }

    /// The palette lists every category's parts in sections, in content order: what the career unlocked, then the
    /// rest, locked.
    #[test]
    fn the_palette_follows_the_unlocks() {
        let content = data::base_content();
        let in_order = |ids: &[&str]| -> Vec<String> { content.parts.iter().filter(|p| ids.contains(&p.id.as_str())).map(|p| p.id.clone()).collect() };
        let new_game = career(&[]).unlocks(&content);
        let weapons = section(&content, &new_game, "weapons", "").unwrap();
        assert_eq!(weapons.parts, [vec!["pulse_laser".to_string()], in_order(&["beam_laser", "automaton_cannon"])].concat());
        assert_eq!(weapons.locked, 2);
        let all_sections: Vec<String> = palette_sections(&content, &new_game, "").into_iter().map(|s| s.category).collect();
        assert_eq!(all_sections, content.categories.iter().map(|c| c.id.clone()).collect::<Vec<_>>());
        // Research moves a part up among the unlocked ones.
        let cannon = career(&["automaton_cannon"]);
        let weapons = section(&content, &cannon.unlocks(&content), "weapons", "").unwrap();
        assert_eq!(weapons.parts, [in_order(&["pulse_laser", "automaton_cannon"]), vec!["beam_laser".to_string()]].concat());
        assert_eq!(weapons.locked, 1);
        assert!(cannon.fresh_parts(&content).contains("automaton_cannon"));
        // A search filters the whole list, locked ones last in their section, empty sections dropped; free mode
        // locks nothing.
        let found = palette_sections(&content, &new_game, "LASER");
        assert_eq!(found, vec![Section { category: "weapons".into(), parts: vec!["pulse_laser".to_string(), "beam_laser".to_string()], locked: 1 }]);
        assert!(palette_sections(&content, &new_game, "no such part").is_empty());
        let all = Unlocks::all(&content);
        assert!(palette_sections(&content, &all, "").iter().all(|s| s.locked == 0));
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
