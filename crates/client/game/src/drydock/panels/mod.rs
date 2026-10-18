//! The drydock's screen, laid out after the concept's desktop mockup and layout
//! v2 (GDR-0082, variant A) for 1280 × 720 logical pixels
//! and up (larger windows scale it: `view::ui_scale`):
//!
//! ```text
//! (o) SHIPYARD › DRYDOCK · FRIGATE FRAME                               OVERLAY [HEAT][MASS][BONUS][SHIELDS][ARCS]
//!     Halvex Runner
//! [ Search parts        ]                                                  POWER  —   PEAK HEAT  —
//! ARMOR · COMPOSITE                                                        THRUST/MASS — YAW —   ...
//! [■][■][■][■]                          (the grid: view.rs)                SELECTED PART
//! [HULL][POWER][WEAPONS][DEFEN›                                            [▣] Armor block
//! HULL                         4                                           ...
//! [▣ Armor block   1×1 · ..  ∞]
//! ...                                   CELL 5,2 · MIRROR ON · AXIS 12 ...  (−) 100 % (+)
//! (↶)(↷) | [PAINT][FILL][ERASE][SELECT][ROTATE] | [MIRROR][BEVEL]  FLIGHT-READY      (▤)(▦)(⇪)(⋯) | [BEHAVIOR] [▶ TEST-FLY]
//!                                                                  VOYAGE-READY · …
//! ```
//!
//! * **Left column**: the search; `ARMOR · <name>` over a strip of material swatches (the armour block in each
//!   material, a light outline, the picked one outlined teal; it scrolls sideways past eight; the tooltip names the
//!   material with its numbers and key); one line of category chips that scrolls sideways (wheel or drag; a fade at
//!   the edges) and jumps the list to its section, the chip of the section in view lit; then every part in
//!   sections, a heading per category with its count. The search filters the whole list (empty sections and their
//!   chips go). Each row ends with how many of the part are left (`∞`: part availability, GDR-0082).
//! * **Bottom bar**: undo and redo; the tools; mirror and bevel; the status line over the voyage-ready line (each
//!   shortened with `…` when it doesn't fit); SAVE, BLUEPRINTS and EXPORT as icons and the ⋯ menu of rarer actions
//!   (opening upward: NEW DESIGN, FRAME, COPY SHARE CODE, the `.ron` file, IMPORT); BEHAVIOR and TEST-FLY. When the
//!   status line would get less than [`MIN_STATUS`] px the tool pills fold into one dropdown (`PAINT ▾`, opening
//!   upward) and mirror and bevel into icon toggles. A menu closes on Esc, a click outside it or any action.
//!
//! Labels carry no key letters: a control's key is in its tooltip (`ui_kit::Tip`, Niklas 13 Oct 2026).
//!
//! Everything is spawned once at start (hidden outside the drydock) and updated in place from the drydock's
//! resources; texts and colours are written only when they change. UI words are chrome in code (like the flight
//! HUD); part names, summaries and stat labels come from content.
//!
//! What the career has not unlocked (`career::Granted`) is shown, not hidden (GDR-0065), as far as research unlocks it
//! (`state::palette_sections`, `state::material_shown`; the rest stays hidden): locked parts close their section
//! dimmed, with a lock and `LOCKED · <research node>`, and can't be picked (a click opens that node); locked armour
//! swatches show a lock. Fresh parts (research unlocked them, the drydock hasn't shown them yet) carry a NEW tag until they are
//! picked or selected once.
//!
//! Part icons are world meshes drawn under bevy_ui (`ui_icons`), so the menus (opaque plates over everything) hold
//! text only.

mod spawn;
mod sync;

pub(crate) use sync::part_icon;

use std::fmt::Write as _;

use bevy::prelude::*;
use bevy::text::LetterSpacing;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use data::{Content, Part, Shape, Unlock};
use kit::style::{Style, ui_color};
use kit::ui_icons::{Glyph, UiIcon};
use kit::ui_kit::{
    HudFonts, Edge, Hot, Pill, PillText, Ring, ScreenFx, ScreenPart, ScrollRows, Scrollbar, SmoothScroll, TextField, Tip, Tone, UiBlock, ValueBar,
    field_text, label, number,
    prose, ring_glyph, ring_node, set_color, set_text, spawn_bar, spawn_brackets, spawn_pill, sub_tone, tracked,
};
use world::camera::CameraRig;
use world::career::{Granted, SeenPart, unlocked_by};
use world::player::PlayerDesign;
use world::sim_bridge::GameContent;

use super::actions::{ActionButton, DrydockAction, Rare, SearchField};
use super::inspector::Inspector;
use super::state::{BarMenu, DrydockStats, DrydockStatus, DrydockUi, Overlay, Tool, armor_materials, palette_sections};
use super::stats::{DETAIL_CELLS, MAIN_CELLS};
use super::view::{DockCamera, DrydockCursor, GridViewport};

pub(in crate::drydock) use spawn::spawn;
pub(in crate::drydock) use sync::{fit_bar, fit_status, scroll_chips, scroll_selected, see_fresh, sync_blocks, sync_live, sync_menus, sync_palette, sync_pills, sync_scrim, sync_search, sync_swatches, sync_texts};

/// The left column's width.
const LEFT_W: f32 = 248.0;
const RIGHT_W: f32 = 292.0;
/// The side columns' distance from the window's edges, and the grid area's (logical px).
const SIDE: f32 = 20.0;
const GRID_LEFT: f32 = 290.0;
const GRID_RIGHT: f32 = 332.0;
/// The grid area's distance from the window's bottom (2 px above the bottom bar), and its info line under the grid
/// (as tall as its zoom rings): the left column ends where the grid does, above that line (Niklas, 14 Oct 2026:
/// "the parts list spills into that bottom area"). Tight on the bars, top and bottom (Niklas: "both the top and
/// bottom panel take too much space"; they were 84 and 96 px).
const GRID_BOTTOM: f32 = 58.0;
const INFO_H: f32 = 28.0;
/// The grid area's distance from the window's top (just under the overlay pills).
const GRID_TOP: f32 = 66.0;
/// The scrim around the grid; its hole follows the grid viewport's laid-out rectangle ([`sync_scrim`]).
#[derive(Component)]
pub(in crate::drydock) struct GridScrim;

/// The scrim around the grid: its alpha, the same all round (Niklas, 14 Oct 2026: "I still want them to be a bit
/// transparent (just everywhere the same)"; half a column looked darker only while the scrim's hole ignored the UI
/// scale), even right up to the grid's edge and fading out only inside the grid, over this far. A fade that began
/// outside the grid let the dock's lit wall show through more in a strip beside the left column, which stopped where
/// the solid bottom band began (Niklas, 15 Oct 2026: "at the border it is slightly more see through").
const SCRIM_ALPHA: f32 = 0.85;
const SCRIM_INTO: f32 = 16.0;
/// List entries in the pool (headings and part rows; the list scrolls by pixels). More than the content has, so
/// the pool never shows other entries while it scrolls (that rebuilt the part icons, a blink).
const SLOTS: usize = 48;
const ROW_H: f32 = 42.0;
const HEAD_H: f32 = 26.0;
const ROW_GAP: f32 = 3.0;
/// An armour swatch's tile, the room around it for its ring (its margin, out to the ring's outer edge) and the ring's
/// width, and the gap between two swatches' margins: eight fill the column (26 px each with their margins, as before).
const SWATCH_TILE: f32 = 20.0;
const SWATCH_RING: f32 = 3.0;
const SWATCH_RING_W: f32 = 1.2;
const SWATCH_GAP: f32 = 5.0;
/// The least room the status line gets in the bottom bar before the tools fold into their dropdown, and how much
/// more it needs to unfold them again (so a width near the edge doesn't flip them every frame).
pub const MIN_STATUS: f32 = 120.0;
const UNFOLD_MARGIN: f32 = 24.0;
/// The bottom bar's menus over everything (the BLUEPRINTS screen is at 20), the veil that closes them just below.
const MENU_Z: i32 = 41;
const VEIL_Z: i32 = 40;
/// The selected part's stat lines and bonus lines.
const SEL_STATS: usize = 12;
const SEL_BONUSES: usize = 3;

/// The drydock's UI root (shown only in the drydock).
#[derive(Component)]
pub struct DrydockRoot;

/// A part of the screen BLUEPRINTS and EXPORT cover (all but the bottom bar and the grid's scrim): hidden while they
/// are open, since their backing is a world mesh (so their own icons show) and these parts would draw over it. The
/// scrim stays: it is a world mesh too, and it keeps the world from showing between their edge and the bottom bar.
#[derive(Component)]
pub(in crate::drydock) struct UnderShelf;

#[derive(Component)]
pub(super) struct PaletteList;

/// A slot of the list: a section heading or a part row.
#[derive(Component)]
pub(super) struct Slot(usize);

#[derive(Component)]
pub(super) struct SlotHead(usize);

#[derive(Component)]
pub(super) struct PaletteRow(usize);

#[derive(Component)]
pub(super) struct RowIcon(usize);

/// A palette row's NEW tag (a fresh part: research unlocked it and the drydock hasn't shown it yet).
#[derive(Component)]
pub(super) struct RowNew(usize);

/// The armour swatches' line (scrolls sideways) and a swatch's tile (the armour block in its material, a lock when
/// locked).
#[derive(Component)]
pub(super) struct SwatchStrip;

#[derive(Component)]
pub(super) struct SwatchTile(String);

/// The category chips' line (scrolls sideways; the kit's fades show over an edge with more chips past it).
#[derive(Component)]
pub(super) struct ChipLine;

/// The bottom bar's tool pills with mirror and bevel (wide), and the dropdown with the two icon toggles (folded).
#[derive(Component)]
pub(super) struct WideTools;

#[derive(Component)]
pub(super) struct FoldedTools;

/// The status line's room in the bottom bar.
#[derive(Component)]
pub(super) struct StatusArea;

/// A bottom bar menu's plate, and the veil under the open one that takes a click outside it.
#[derive(Component)]
pub(super) struct MenuPlate(BarMenu);

#[derive(Component)]
pub(super) struct MenuVeil;

#[derive(Component)]
pub(super) struct SelIcon;

#[derive(Component)]
pub(super) struct StatCellNode(usize);

#[derive(Component)]
pub(super) struct StatBar(usize);

#[derive(Component)]
pub(super) struct SelStat(usize);

#[derive(Component)]
pub(super) struct SelBonus(usize);

/// The selected part's lines and controls under its name: they scroll when they don't fit.
#[derive(Component)]
pub(super) struct SelScroll;

/// Every text the drydock updates.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelText {
    Breadcrumb,
    Name,
    /// The armour material's name after `ARMOR`.
    Armor,
    /// The current tool in the folded tools' dropdown.
    ToolName,
    Status,
    Voyage,
    Info,
    Zoom,
    SelName,
    SelSub,
    SelSummary,
    HeadName(usize),
    HeadCount(usize),
    RowName(usize),
    RowSub(usize),
    /// How many of the part are left (`∞` while parts are unlimited).
    RowCount(usize),
    StatLabel(usize),
    StatValue(usize),
    /// A main cell's small line under its bar (the power drawn in bursts, "+3 WHILE SCANNING").
    StatNote(usize),
    SelStatLabel(usize),
    SelStatValue(usize),
    SelBonus(usize),
}

/// One entry of the palette's list.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Entry {
    /// A section heading: the category, its unlocked and locked parts listed.
    Head { category: String, open: usize, locked: usize },
    Part(String),
}

impl Entry {
    fn height(&self) -> f32 {
        match self {
            Entry::Head { .. } => HEAD_H,
            Entry::Part(_) => ROW_H,
        }
    }
}

/// The palette list: its entries (sections of the search's parts, the locked ones last in each), the first entry
/// shown and how many fit, and which section's chip is lit.
#[derive(Resource, Default, Debug)]
pub(super) struct Palette {
    entries: Vec<Entry>,
    offset: usize,
    visible: usize,
    /// What the list was made from: search, material.
    made_from: Option<(String, String)>,
    /// The chip clicked last: lit while its heading is in view (the list may not reach it to the top).
    jumped: Option<String>,
    /// The lit chip: the section in view.
    lit: Option<String>,
}

impl Palette {
    /// Is there a section of this category in the list?
    fn has_section(&self, category: &str) -> bool {
        self.entries.iter().any(|e| matches!(e, Entry::Head { category: c, .. } if c == category))
    }

    /// The entry shown in slot `k`.
    fn shown(&self, k: usize) -> Option<&Entry> {
        (k < self.visible).then(|| self.entries.get(self.offset + k)).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::sync::shorten;

    /// A status line too long for its room drops whole parts from the end, else is cut, always ending in `…`.
    #[test]
    fn long_status_lines_are_shortened() {
        let line = "FLIGHT-READY  ·  RUNS HOT AT FULL LOAD  ·  +1";
        assert_eq!(shorten(line, 0.6), "FLIGHT-READY  ·  …");
        assert_eq!(shorten(line, 0.99), "FLIGHT-READY  ·  RUNS HOT AT FULL LOAD  ·  …");
        assert_eq!(shorten("SAVED AS WORK IN PROGRESS", 0.4), "SAVED AS …");
        assert!(shorten(line, 0.0).ends_with('…'));
    }

}
