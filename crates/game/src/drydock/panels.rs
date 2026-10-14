//! The drydock's screen, laid out after the mockup (`docs/concept/boards/ui-mockups/desktop-drydock.png`) and layout
//! v2 (GDR-0082, the draft `docs/plans/mockups/drydock-left-column.html`, variant A) for 1280 × 720 logical pixels
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
//! What the career has not unlocked (`career::Granted`) is shown, not hidden (GDR-0065): locked parts close their
//! section dimmed, with a lock and `LOCKED · <research node>`, and can't be picked; locked armour swatches show a
//! lock. Fresh parts (research unlocked them, the drydock hasn't shown them yet) carry a NEW tag until they are
//! picked or selected once.
//!
//! Part icons are world meshes drawn under bevy_ui (`ui_icons`), so the menus (opaque plates over everything) hold
//! text only.

use std::fmt::Write as _;

use bevy::input::mouse::AccumulatedMouseScroll;

use bevy::prelude::*;
use bevy::text::LetterSpacing;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use data::career::Availability;
use data::{Content, Part, Shape, Unlock};

use crate::camera::CameraRig;
use crate::career::{Granted, SeenPart, unlocked_by};
use crate::flight_hud::HudFonts;
use crate::sim_bridge::GameContent;
use crate::style::{Style, ui_color};
use crate::ui_icons::{Glyph, UiIcon};
use crate::ui_kit::{
    Edge, FADE_DEPTH, Hot, Pill, PillText, Ring, ScreenFx, ScreenPart, ScrollRows, Scrollbar, TextField, Tip, Tone, UiBlock, ValueBar, WHEEL_LINE_PX,
    field_text, label, number,
    prose, ring_glyph, ring_node, set_color, set_text, spawn_bar, spawn_brackets, spawn_pill, spawn_scroll_cue, sub_tone, tracked, wheel_rows,
};

use super::PlayerDesign;
use super::actions::{ActionButton, DrydockAction, Rare, SearchField};
use super::inspector::Inspector;
use super::state::{BarMenu, DrydockStats, DrydockStatus, DrydockUi, Overlay, Tool, armor_materials, palette_sections};
use super::stats::{DETAIL_CELLS, MAIN_CELLS};
use super::view::{DockCamera, DrydockCursor, GridViewport};

/// The keys of the first nine armour swatches (in their tooltips).
const MATERIAL_KEYS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// The left column's width.
const LEFT_W: f32 = 248.0;
/// List entries in the pool (headings and part rows; as many show as fit, the wheel scrolls).
const SLOTS: usize = 48;
const ROW_H: f32 = 42.0;
const HEAD_H: f32 = 26.0;
const ROW_GAP: f32 = 3.0;
/// An armour swatch and the gap after it: eight fill the column.
const SWATCH: f32 = 26.0;
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

/// How many entries from `from` fit into `height` px (at most [`SLOTS`]).
fn fitting(entries: &[Entry], height: f32) -> usize {
    let mut used = 0.0;
    let mut n = 0;
    for e in entries.iter().take(SLOTS) {
        let h = e.height() + if n > 0 { ROW_GAP } else { 0.0 };
        if used + h > height + 0.5 {
            break;
        }
        used += h;
        n += 1;
    }
    n
}

/// The last first entry that still fills the list: the smallest offset from which every entry after it fits.
fn max_offset(entries: &[Entry], height: f32) -> usize {
    let mut used = 0.0;
    let mut i = entries.len();
    while i > 0 {
        let h = entries[i - 1].height() + if used > 0.0 { ROW_GAP } else { 0.0 };
        if used + h > height + 0.5 || entries.len() - (i - 1) > SLOTS {
            break;
        }
        used += h;
        i -= 1;
    }
    i
}

pub(super) fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, content: Res<GameContent>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let content = &content.0;
    let abs = |l: Option<f32>, r: Option<f32>, t: Option<f32>, b: Option<f32>| Node {
        position_type: PositionType::Absolute,
        left: l.map(px).unwrap_or(Val::Auto),
        right: r.map(px).unwrap_or(Val::Auto),
        top: t.map(px).unwrap_or(Val::Auto),
        bottom: b.map(px).unwrap_or(Val::Auto),
        ..default()
    };
    let glyph = |glyph: Glyph, tone: Tone, size: f32| {
        (UiIcon::Glyph { glyph, color: tone.color(&ui) }, Node { width: px(size), height: px(size), flex_shrink: 0.0, ..default() })
    };
    let pill = |accent: Tone| Pill::new(accent);
    let sep = || (Node { width: px(1), height: px(24), margin: UiRect::horizontal(px(4)), flex_shrink: 0.0, ..default() }, BackgroundColor(c(Tone::Label, 0.22)));
    let fonts = &*fonts;
    let back = style.background.outer();

    commands
        .spawn((
            DrydockRoot,
            // Opens and closes with a transition (`view::open_close`): the panels slide in from their edges.
            ScreenFx::closed(),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // Top left: the ring menu, the breadcrumb and the design's name.
            root.spawn((UiBlock, ScreenPart::new(Edge::Top, 0), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(14), ..abs(Some(22.0), None, Some(14.0), None) }))
                .with_children(|tl| {
                    tl.spawn((ring_node(40.0), ActionButton(DrydockAction::Menu), Tip::new("SHIPYARD"))).with_children(|r| {
                        r.spawn(glyph(Glyph::Menu, Tone::Violet, 22.0));
                    });
                    tl.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                        col.spawn((PanelText::Breadcrumb, label(fonts, "", 10.0, c(Tone::Label, 1.0))));
                        super::blueprints::spawn_title(col, fonts, c(Tone::Number, 1.0), c(Tone::Dim, 1.0), c(Tone::Label, 0.3));
                    });
                });

            // Top right: the overlays.
            root.spawn((UiBlock, ScreenPart::new(Edge::Top, 0), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(5), ..abs(None, Some(20.0), Some(19.0), None) }))
                .with_children(|tr| {
                    tr.spawn((label(fonts, "OVERLAY", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::right(px(5)), ..default() }));
                    for o in Overlay::ALL {
                        let accent = if o == Overlay::Bonus { Tone::Gold } else { Tone::Teal };
                        spawn_pill(tr, fonts, overlay_name(o), 10.0, 30.0, None, pill(accent), ActionButton(DrydockAction::Overlay(o)));
                    }
                });

            // Left: search, armour, category chips, every part in sections.
            root.spawn((UiBlock, ScreenPart::new(Edge::Left, 1), Node { width: px(LEFT_W), flex_direction: FlexDirection::Column, row_gap: px(10), ..abs(Some(20.0), None, Some(74.0), Some(70.0)) }))
                .with_children(|lp| {
                    lp.spawn((
                        SearchField,
                        TextField { placeholder: "Search parts".into(), max_chars: 24, ..default() },
                        Hot::default(),
                        Tip::key("SEARCH PARTS", "/"),
                        Node {
                            height: px(36),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(px(14)),
                            column_gap: px(10),
                            border: UiRect::all(px(1.2)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BorderColor::all(c(Tone::Label, 0.3)),
                    ))
                    .with_children(|f| {
                        f.spawn(glyph(Glyph::Search, Tone::Label, 14.0));
                        f.spawn((field_text(fonts, 12.5), Node { flex_grow: 1.0, ..default() }));
                    });
                    // ARMOR · <name>, then the swatches: every armour material research can unlock (a locked one
                    // shows a lock; the keys 1–9 count every armour material, hidden ones too).
                    lp.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8), margin: UiRect::new(px(4), px(0), px(4), px(0)), flex_shrink: 0.0, ..default() })
                        .with_children(|r| {
                            r.spawn(label(fonts, "ARMOR  ·", 9.0, c(Tone::Dim, 1.0)));
                            r.spawn((PanelText::Armor, label(fonts, "", 10.0, c(Tone::Number, 1.0))));
                        });
                    let block = content.unit_hull_part(Shape::Rect);
                    lp.spawn((
                        SwatchStrip,
                        ScrollPosition::default(),
                        Scrollbar::horizontal().outside().room(12),
                        Node { flex_direction: FlexDirection::Row, column_gap: px(SWATCH_GAP), width: percent(100), flex_shrink: 0.0, overflow: Overflow::scroll_x(), ..default() },
                    ))
                    .with_children(|strip| {
                        for (k, m) in armor_materials(content).enumerate() {
                            let tip = Tip { name: material_tip(content, block, m).into(), key: MATERIAL_KEYS.get(k).copied() };
                            strip
                                .spawn((
                                    ActionButton(DrydockAction::Material(m.id.clone())),
                                    Pill { bare: true, ..pill(Tone::Teal) },
                                    Hot::default(),
                                    tip,
                                    Node {
                                        width: px(SWATCH),
                                        height: px(SWATCH),
                                        flex_shrink: 0.0,
                                        padding: UiRect::all(px(2)),
                                        border: UiRect::all(px(1.2)),
                                        border_radius: BorderRadius::all(px(7)),
                                        ..default()
                                    },
                                    BorderColor::all(Color::NONE),
                                    BackgroundColor(Color::NONE),
                                ))
                                .with_children(|sw| {
                                    let tile = UiIcon::Part { part: block.map_or(String::new(), |b| b.id.clone()), material: Some(m.id.clone()) };
                                    sw.spawn((
                                        SwatchTile(m.id.clone()),
                                        tile,
                                        Node { width: percent(100), height: percent(100), border: UiRect::all(px(1)), border_radius: BorderRadius::all(px(4)), ..default() },
                                        // A light outline: the near-black armour tones are told apart by it.
                                        BorderColor::all(c(Tone::Number, 0.22)),
                                    ))
                                    .with_children(|t| {
                                        if let Some(key) = MATERIAL_KEYS.get(k) {
                                            t.spawn((label(fonts, key, 7.5, c(Tone::Number, 0.8)), Node { position_type: PositionType::Absolute, right: px(1), bottom: px(-1), ..default() }));
                                        }
                                    });
                                });
                        }
                    });
                    // The category chips: one line that scrolls sideways, faded at a scrolled edge.
                    lp.spawn(Node { width: percent(100), height: px(24), flex_shrink: 0.0, ..default() }).with_children(|w| {
                        w.spawn((
                            ChipLine,
                            ScrollPosition::default(),
                            Scrollbar::horizontal().outside(),
                            Node { flex_direction: FlexDirection::Row, column_gap: px(6), width: percent(100), height: percent(100), overflow: Overflow::scroll_x(), ..default() },
                        ))
                        .with_children(|cats| {
                            for cat in content.categories.iter() {
                                spawn_pill(cats, fonts, &cat.name.to_uppercase(), 8.5, 24.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Category(cat.id.clone())));
                            }
                        });
                    });
                    lp.spawn((
                        PaletteList,
                        Scrollbar::vertical(),
                        ScrollRows::default(),
                        Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            flex_basis: px(0),
                            min_height: px(0),
                            row_gap: px(ROW_GAP),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ))
                    .with_children(|list| {
                        for k in 0..SLOTS {
                            list.spawn((Slot(k), Node { display: Display::None, flex_direction: FlexDirection::Column, flex_shrink: 0.0, ..default() })).with_children(|slot| {
                                slot.spawn((
                                    SlotHead(k),
                                    Node {
                                        height: px(HEAD_H),
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::FlexEnd,
                                        justify_content: JustifyContent::SpaceBetween,
                                        padding: UiRect::new(px(6), px(12), px(0), px(5)),
                                        ..default()
                                    },
                                ))
                                .with_children(|h| {
                                    h.spawn((PanelText::HeadName(k), label(fonts, "", 9.0, c(Tone::Label, 1.0))));
                                    h.spawn((PanelText::HeadCount(k), label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                                });
                                spawn_row(slot, fonts, &c, k);
                            });
                        }
                    });
                });

            // Right: the stats grid and the selected part.
            root.spawn((UiBlock, ScreenPart::new(Edge::Right, 1), Node { width: px(292), flex_direction: FlexDirection::Column, ..abs(None, Some(20.0), Some(78.0), Some(70.0)) }))
                .with_children(|rp| {
                    rp.spawn(Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                        column_gap: px(20),
                        row_gap: px(16),
                        ..default()
                    })
                    .with_children(|g| {
                        for k in 0..MAIN_CELLS {
                            g.spawn((StatCellNode(k), Node { flex_direction: FlexDirection::Column, row_gap: px(7), ..default() })).with_children(|cell| {
                                cell.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, column_gap: px(8), ..default() })
                                    .with_children(|r| {
                                        r.spawn((PanelText::StatLabel(k), label(fonts, "", 9.0, c(Tone::Label, 1.0))));
                                        r.spawn((PanelText::StatValue(k), number(fonts, "", 16.0, c(Tone::Number, 1.0))));
                                    });
                                spawn_bar(cell, ValueBar { share: 0.0, tone: Tone::Gold }, StatBar(k));
                            });
                        }
                    });
                    // The compact block under it: smaller, no bars (top speed, turn, mass, build points).
                    rp.spawn(Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                        column_gap: px(20),
                        row_gap: px(9),
                        margin: UiRect::top(px(18)),
                        ..default()
                    })
                    .with_children(|g| {
                        for k in MAIN_CELLS..MAIN_CELLS + DETAIL_CELLS {
                            g.spawn((StatCellNode(k), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, column_gap: px(8), ..default() }))
                                .with_children(|r| {
                                    r.spawn((PanelText::StatLabel(k), label(fonts, "", 8.0, c(Tone::Label, 1.0))));
                                    r.spawn((PanelText::StatValue(k), number(fonts, "", 12.5, c(Tone::Number, 1.0))));
                                });
                        }
                    });
                    rp.spawn((Node { height: px(1), margin: UiRect::vertical(px(14)), flex_shrink: 0.0, ..default() }, BackgroundColor(c(Tone::Label, 0.14))));
                    rp.spawn((label(fonts, "SELECTED PART", 9.0, c(Tone::Label, 1.0)), Node { margin: UiRect::bottom(px(10)), flex_shrink: 0.0, ..default() }));
                    rp.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(16), flex_shrink: 0.0, ..default() }).with_children(|h| {
                        // The icon in a bracketed box (like the fleet bar's chips).
                        h.spawn(Node { width: px(54), height: px(54), flex_shrink: 0.0, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() })
                            .with_children(|b| {
                                spawn_brackets(b, 9.0, 1.2, c(Tone::Label, 0.45));
                                b.spawn((SelIcon, UiIcon::Part { part: String::new(), material: None }, Node { width: px(40), height: px(40), ..default() }));
                            });
                        h.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(6), ..default() }).with_children(|t| {
                            t.spawn((PanelText::SelName, prose(fonts, "", 18.0, c(Tone::Number, 1.0))));
                            t.spawn((PanelText::SelSub, label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                        });
                    });
                    // The rest scrolls (the wheel) when a part has more lines and controls than the column has room
                    // for; the cue says so.
                    let body = rp
                        .spawn((
                            SelScroll,
                            ScrollPosition::default(),
                            Scrollbar::vertical().room(0),
                            Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: px(0), min_height: px(0), overflow: Overflow::scroll_y(), ..default() },
                        ))
                        .with_children(|sb| {
                            sb.spawn((PanelText::SelSummary, prose(fonts, "", 11.5, c(Tone::Label, 1.0)), Node { margin: UiRect::top(px(12)), flex_shrink: 0.0, ..default() }));
                            sb.spawn(Node {
                                display: Display::Grid,
                                grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
                                column_gap: px(12),
                                row_gap: px(8),
                                margin: UiRect::top(px(14)),
                                flex_shrink: 0.0,
                                ..default()
                            })
                            .with_children(|g| {
                                for k in 0..SEL_STATS {
                                    g.spawn((SelStat(k), Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() })).with_children(|cell| {
                                        cell.spawn((PanelText::SelStatLabel(k), label(fonts, "", 8.0, c(Tone::Label, 1.0))));
                                        cell.spawn((PanelText::SelStatValue(k), number(fonts, "", 14.0, c(Tone::Number, 1.0))));
                                    });
                                }
                            });
                            sb.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(8), margin: UiRect::top(px(12)), flex_shrink: 0.0, ..default() }).with_children(|b| {
                                for k in 0..SEL_BONUSES {
                                    b.spawn((SelBonus(k), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() }))
                                        .with_children(|r| {
                                            r.spawn(glyph(Glyph::Diamond, Tone::Gold, 9.0));
                                            r.spawn((PanelText::SelBonus(k), tracked(fonts, "", 8.5, c(Tone::Gold, 1.0), 0.2)));
                                        });
                                }
                            });
                            super::inspector::spawn_controls(sb, fonts, &style, content);
                        })
                        .id();
                    spawn_scroll_cue(rp, fonts, &style, body);
                });

            // Centre: the grid viewport; the info line and the zoom.
            root.spawn(Node { flex_direction: FlexDirection::Column, ..abs(Some(290.0), Some(332.0), Some(84.0), Some(66.0)) }).with_children(|ga| {
                // The info line moves, the grid viewport never (the camera centres on it).
                ga.spawn((GridViewport, Node { flex_grow: 1.0, ..default() }));
                ga.spawn((ScreenPart::new(Edge::Bottom, 2), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, height: px(30), flex_shrink: 0.0, ..default() }))
                    .with_children(|b| {
                        b.spawn((PanelText::Info, label(fonts, "", 9.0, c(Tone::Dim, 1.0)))).with_child(super::ghost::hint_span(fonts, &style));
                        b.spawn((UiBlock, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() })).with_children(|z| {
                            z.spawn((ring_node(28.0), ActionButton(DrydockAction::ZoomOut), Tip::key("ZOOM OUT", "−"))).with_child(ring_glyph(Glyph::Minus, 13.0));
                            z.spawn((PanelText::Zoom, number(fonts, "", 13.0, c(Tone::Number, 1.0)), Node { min_width: px(46), ..default() }));
                            z.spawn((ring_node(28.0), ActionButton(DrydockAction::ZoomIn), Tip::key("ZOOM IN", "+"))).with_child(ring_glyph(Glyph::Plus, 13.0));
                        });
                    });
            });

            // The veil under an open menu: a click on it (outside the menu) closes the menu and reaches nothing else.
            root.spawn((
                MenuVeil,
                Hot::default(),
                ActionButton(DrydockAction::CloseMenu),
                GlobalZIndex(VEIL_Z),
                Node { display: Display::None, ..abs(Some(0.0), Some(0.0), Some(0.0), Some(0.0)) },
            ));

            // Bottom: undo / redo, the tools, mirror and bevel, the status line, save / blueprints / export / ⋯,
            // the BEHAVIOR screen and the test flight.
            root.spawn((UiBlock, ScreenPart::new(Edge::Bottom, 3), Node { height: px(42), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), ..abs(Some(22.0), Some(20.0), None, Some(14.0)) }))
                .with_children(|bb| {
                    for (g, action, tip) in [(Glyph::Undo, DrydockAction::Undo, Tip::key("UNDO", "CTRL+Z")), (Glyph::Redo, DrydockAction::Redo, Tip::key("REDO", "CTRL+SHIFT+Z"))] {
                        bb.spawn((ring_node(36.0), ActionButton(action), tip)).with_children(|r| {
                            r.spawn(glyph(g, Tone::Number, 19.0));
                        });
                    }
                    bb.spawn(sep());
                    let tools = [
                        ("PAINT", "B", DrydockAction::Tool(Tool::Paint)),
                        ("FILL", "G", DrydockAction::Tool(Tool::Fill)),
                        ("ERASE", "X", DrydockAction::Tool(Tool::Erase)),
                        ("SELECT", "V", DrydockAction::Tool(Tool::Select)),
                        ("ROTATE", "R · Q / E", DrydockAction::RotateCw),
                    ];
                    // Wide: the tool pills, mirror and bevel.
                    bb.spawn((WideTools, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), flex_shrink: 0.0, ..default() })).with_children(|w| {
                        for (text, key, action) in tools.clone() {
                            spawn_pill(w, fonts, text, 9.5, 30.0, None, pill(Tone::Teal), (ActionButton(action), Tip::key(text, key)));
                        }
                        w.spawn(sep());
                        spawn_pill(w, fonts, "MIRROR", 9.5, 30.0, None, pill(Tone::Teal), (ActionButton(DrydockAction::Mirror), Tip::key("MIRROR", "M")));
                        spawn_pill(w, fonts, "BEVEL", 9.5, 30.0, None, pill(Tone::Teal), (ActionButton(DrydockAction::Bevel), Tip::key("BEVEL", "SHIFT+B")));
                    });
                    // Folded: the current tool as a dropdown (its menu opens upward), mirror and bevel as icons.
                    bb.spawn((FoldedTools, Node { display: Display::None, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), flex_shrink: 0.0, ..default() }))
                        .with_children(|f| {
                            f.spawn(Node::default()).with_children(|anchor| {
                                anchor
                                    .spawn((
                                        Pill { active: true, ..pill(Tone::Teal) },
                                        Hot::default(),
                                        ActionButton(DrydockAction::OpenMenu(BarMenu::Tools)),
                                        Tip::new("TOOLS"),
                                        Node {
                                            height: px(30),
                                            padding: UiRect::new(px(14), px(11), px(0), px(0)),
                                            border: UiRect::all(px(1.2)),
                                            border_radius: BorderRadius::MAX,
                                            align_items: AlignItems::Center,
                                            column_gap: px(8),
                                            ..default()
                                        },
                                        BorderColor::all(Color::NONE),
                                        BackgroundColor(Color::NONE),
                                    ))
                                    .with_children(|p| {
                                        p.spawn((PanelText::ToolName, pill_text(fonts, "PAINT", 9.5)));
                                        p.spawn(glyph(Glyph::Caret, Tone::Teal, 9.0));
                                    });
                                spawn_menu(anchor, BarMenu::Tools, ui_color(back, 0.97), c(Tone::Label, 0.3), |m| {
                                    for (text, key, action) in tools.clone() {
                                        menu_item(m, fonts, &c, text, Some(key), action);
                                    }
                                });
                            });
                            f.spawn((ring_node(34.0), ActionButton(DrydockAction::Mirror), Tip::key("MIRROR", "M"))).with_child(ring_glyph(Glyph::Mirror, 16.0));
                            f.spawn((ring_node(34.0), ActionButton(DrydockAction::Bevel), Tip::key("BEVEL", "SHIFT+B"))).with_child(ring_glyph(Glyph::Bevel, 15.0));
                        });
                    // The status line over the voyage-ready line, each shortened when it doesn't fit (`fit_status`).
                    bb.spawn((
                        StatusArea,
                        Hot::default(),
                        Node {
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            min_width: px(0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            // Start, not stretch: a line's laid-out width is its full width (`fit_status`).
                            align_items: AlignItems::FlexStart,
                            row_gap: px(4),
                            margin: UiRect::horizontal(px(12)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ))
                    .with_children(|s| {
                        for t in [PanelText::Status, PanelText::Voyage] {
                            s.spawn((t, label(fonts, "", 9.5, c(Tone::Teal, 1.0)), TextLayout::no_wrap(), Node { flex_shrink: 0.0, ..default() }));
                        }
                    });
                    for (g, size, action, tip) in [
                        (Glyph::Save, 16.0, DrydockAction::Save, Tip::key("SAVE", "CTRL+S")),
                        (Glyph::Blueprints, 16.0, DrydockAction::Blueprints, Tip::new("BLUEPRINTS")),
                        (Glyph::Export, 16.0, DrydockAction::Export, Tip::new("EXPORT")),
                    ] {
                        bb.spawn((ring_node(34.0), ActionButton(action), tip)).with_child(ring_glyph(g, size));
                    }
                    bb.spawn(Node::default()).with_children(|anchor| {
                        anchor.spawn((ring_node(34.0), ActionButton(DrydockAction::OpenMenu(BarMenu::More)), Tip::new("MORE"))).with_child(ring_glyph(Glyph::More, 16.0));
                        spawn_menu(anchor, BarMenu::More, ui_color(back, 0.97), c(Tone::Label, 0.3), |m| {
                            menu_item(m, fonts, &c, "NEW DESIGN", None, DrydockAction::Rare(Rare::NewDesign));
                            // The content frames: the design moves to the one picked (locked ones are dimmed).
                            m.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(5), padding: UiRect::new(px(14), px(8), px(3), px(3)), ..default() })
                                .with_children(|r| {
                                    r.spawn((label(fonts, "FRAME", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::right(px(6)), ..default() }));
                                    for f in content.frames.iter() {
                                        spawn_pill(r, fonts, &f.name.to_uppercase(), 8.5, 24.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Rare(Rare::Frame(f.size))));
                                    }
                                });
                            menu_item(m, fonts, &c, "COPY SHARE CODE", None, DrydockAction::Rare(Rare::CopyCode));
                            let file = if cfg!(target_arch = "wasm32") { "DOWNLOAD .RON FILE" } else { "WRITE .RON FILE" };
                            menu_item(m, fonts, &c, file, None, DrydockAction::Rare(Rare::Download));
                            menu_item(m, fonts, &c, "IMPORT .RON FILE", None, DrydockAction::Rare(Rare::ImportFile));
                        });
                    });
                    bb.spawn(sep());
                    spawn_pill(bb, fonts, "BEHAVIOR", 9.5, 32.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Behavior));
                    bb.spawn(Node { width: px(4), ..default() });
                    let play = UiIcon::Glyph { glyph: Glyph::Play, color: ui.teal };
                    spawn_pill(bb, fonts, "TEST-FLY", 12.0, 40.0, Some(play), Pill { primary: true, ..pill(Tone::Teal) }, (ActionButton(DrydockAction::TestFly), Tip::key("TEST-FLY", "T")));
                });
        });
}

/// One slot's part row: icon, name and subtitle, the NEW tag, how many are left.
fn spawn_row(slot: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, k: usize) {
    slot.spawn((
        PaletteRow(k),
        ActionButton(DrydockAction::PickPart(String::new())),
        Pill { bare: true, ..Pill::new(Tone::Teal) },
        Hot::default(),
        Node {
            display: Display::None,
            height: px(ROW_H),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::new(px(10), px(14), px(0), px(0)),
            column_gap: px(12),
            border: UiRect::all(px(1.2)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    ))
    .with_children(|row| {
        row.spawn((RowIcon(k), UiIcon::Part { part: String::new(), material: None }, Node { width: px(32), height: px(28), flex_shrink: 0.0, ..default() }));
        row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), ..default() }).with_children(|t| {
            t.spawn((PanelText::RowName(k), prose(fonts, "", 13.5, c(Tone::Number, 1.0))));
            t.spawn((PanelText::RowSub(k), tracked(fonts, "", 8.0, c(Tone::Dim, 1.0), 0.2), TextLayout::no_wrap()));
        });
        // The NEW tag (GDR-0065): a small gold-outlined pill.
        row.spawn((
            RowNew(k),
            Node {
                display: Display::None,
                height: px(18),
                padding: UiRect::horizontal(px(8)),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::MAX,
                flex_shrink: 0.0,
                ..default()
            },
            BorderColor::all(c(Tone::Gold, 0.85)),
        ))
        .with_child(label(fonts, "NEW", 7.5, c(Tone::Gold, 1.0)));
        row.spawn((PanelText::RowCount(k), prose(fonts, "", 16.0, c(Tone::Label, 1.0)), Node { flex_shrink: 0.0, ..default() }));
    });
}

/// A bottom bar menu's plate over its anchor (it opens upward; aligned left for the tools, right for ⋯), filled by
/// `items`: text only, part icons would draw under it.
fn spawn_menu(anchor: &mut ChildSpawnerCommands, menu: BarMenu, plate: Color, edge: Color, items: impl FnOnce(&mut ChildSpawnerCommands)) {
    let (left, right) = match menu {
        BarMenu::Tools => (px(0), Val::Auto),
        BarMenu::More => (Val::Auto, px(0)),
    };
    anchor
        .spawn((
            MenuPlate(menu),
            UiBlock,
            GlobalZIndex(MENU_Z),
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left,
                right,
                bottom: percent(100),
                margin: UiRect::bottom(px(10)),
                min_width: px(200),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                padding: UiRect::all(px(6)),
                border: UiRect::all(px(1.2)),
                border_radius: BorderRadius::all(px(14)),
                ..default()
            },
            BackgroundColor(plate),
            BorderColor::all(edge),
        ))
        .with_children(items);
}

/// A menu row: its name (and key) on a bare pill that sends `action`.
fn menu_item(m: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, text: &str, key: Option<&str>, action: DrydockAction) {
    m.spawn((
        ActionButton(action),
        Pill { bare: true, ..Pill::new(Tone::Teal) },
        Hot::default(),
        Node {
            height: px(30),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(18),
            padding: UiRect::horizontal(px(14)),
            border: UiRect::all(px(1.2)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    ))
    .with_children(|r| {
        r.spawn((pill_text(fonts, text, 9.5), TextLayout::no_wrap()));
        if let Some(key) = key {
            r.spawn((number(fonts, key, 10.0, c(Tone::Dim, 1.0)), TextLayout::no_wrap()));
        }
    });
}

/// A text a pill colours with its state (Barlow medium, like `spawn_pill`'s).
fn pill_text(fonts: &HudFonts, text: &str, size: f32) -> impl Bundle + use<> {
    (
        PillText,
        Text::new(text),
        TextFont { font: fonts.medium.clone().into(), font_size: FontSize::Px(size), ..default() },
        TextColor(Color::NONE),
        LetterSpacing::Px(size * 0.25),
    )
}

/// An armour swatch's tooltip: the material's name and the armour block's numbers in it (labels from content).
fn material_tip(content: &Content, block: Option<&Part>, m: &data::Material) -> String {
    let mut s = m.name.to_uppercase();
    if let Some(block) = block {
        for l in content.part_stats(block, Some(m)).iter().filter(|l| ["mass", "hp", "build_points"].contains(&l.stat.as_str())) {
            let _ = write!(s, "  ·  {} {}", l.label.to_uppercase(), l.text.to_uppercase());
        }
    }
    s
}

fn overlay_name(o: Overlay) -> &'static str {
    match o {
        Overlay::Heat => "HEAT",
        Overlay::Mass => "MASS",
        Overlay::Bonus => "BONUS",
        Overlay::Shields => "SHIELDS",
        Overlay::Arcs => "ARCS",
    }
}

/// The search field and [`DrydockUi::search`] stay equal: typing writes the resource, other systems may too.
pub(super) fn sync_search(mut ui: ResMut<DrydockUi>, mut field: Single<&mut TextField, With<SearchField>>) {
    if field.is_changed() && field.value != ui.search {
        ui.search.clone_from(&field.value);
    } else if ui.is_changed() && ui.search != field.value {
        field.value.clone_from(&ui.search);
    }
}

/// Active and disabled states of the action widgets, from the drydock's state (the mirror and bevel modes are
/// the editor's; the inspector keeps its own pills, but locked armour is dimmed here too). Locked parts,
/// materials and frames are disabled: they send nothing. A category chip is lit for the section in view and goes
/// with its section (a search that leaves it empty).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync_pills(
    ui: Res<DrydockUi>,
    design: Res<PlayerDesign>,
    status: Res<DrydockStatus>,
    granted: Res<Granted>,
    content: Res<GameContent>,
    pal: Res<Palette>,
    mut pills: Query<(&ActionButton, &mut Pill, &mut Node)>,
    mut rings: Query<(&ActionButton, &mut Ring), Without<Pill>>,
) {
    let unlocks = &granted.unlocks;
    for (b, mut pill, mut node) in &mut pills {
        let (active, disabled) = match &b.0 {
            DrydockAction::Tool(t) => (ui.tool == *t, false),
            DrydockAction::Mirror => (design.dock().mirror, false),
            DrydockAction::Bevel => (design.dock().bevel, false),
            DrydockAction::Material(m) => {
                set_display(&mut node, super::state::material_shown(&content.0, unlocks, m));
                (ui.material == *m, !unlocks.has_material(m))
            }
            DrydockAction::Overlay(o) => (ui.overlay(*o), false),
            DrydockAction::Category(cat) => {
                set_display(&mut node, pal.has_section(cat));
                (pal.lit.as_deref() == Some(cat.as_str()), false)
            }
            DrydockAction::PickPart(p) => (ui.picked.as_deref() == Some(p.as_str()), !p.is_empty() && !unlocks.has_part(p)),
            DrydockAction::TestFly => (false, status.grounded.is_some()),
            DrydockAction::Rare(Rare::Frame(f)) => (design.frame() == *f, !granted.frame_open(&content.0, *f)),
            DrydockAction::PartMaterial(m) => {
                set_display(&mut node, super::state::material_shown(&content.0, unlocks, m));
                let disabled = !unlocks.has_material(m);
                if pill.disabled != disabled {
                    pill.disabled = disabled;
                }
                continue;
            }
            _ => continue,
        };
        if pill.active != active || pill.disabled != disabled {
            pill.active = active;
            pill.disabled = disabled;
        }
    }
    // The folded bar's mirror and bevel icons.
    for (b, mut ring) in &mut rings {
        let active = match b.0 {
            DrydockAction::Mirror => design.dock().mirror,
            DrydockAction::Bevel => design.dock().bevel,
            _ => continue,
        };
        if ring.active != active {
            ring.active = active;
        }
    }
}

/// The part list: its entries (sections of the search's parts, the locked ones last in each), how many fit, the
/// wheel over the list scrolls it and a chip jumps it to its section; the lit chip; the slots' headings and rows,
/// the rows' icons (a lock for a locked part) and NEW tags.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync_palette(
    mut ui: ResMut<DrydockUi>,
    content: Res<GameContent>,
    granted: Res<Granted>,
    style: Res<Style>,
    mut pal: ResMut<Palette>,
    window: Single<&Window, With<PrimaryWindow>>,
    list: Single<(&ComputedNode, &UiGlobalTransform, &mut ScrollRows), With<PaletteList>>,
    scroll: Res<AccumulatedMouseScroll>,
    mut slots: Query<(&Slot, &mut Node), (Without<SlotHead>, Without<PaletteRow>, Without<RowNew>)>,
    mut heads: Query<(&SlotHead, &mut Node), (Without<Slot>, Without<PaletteRow>, Without<RowNew>)>,
    mut rows: Query<(&PaletteRow, &mut Node, &mut ActionButton), (Without<Slot>, Without<SlotHead>, Without<RowNew>)>,
    mut icons: Query<(&RowIcon, &mut UiIcon)>,
    mut tags: Query<(&RowNew, &mut Node), (Without<Slot>, Without<SlotHead>, Without<PaletteRow>)>,
) {
    let content = &content.0;
    let made_from = (ui.is_changed() || granted.is_changed() || pal.made_from.is_none())
        .then(|| (ui.search.to_lowercase(), ui.material.clone()))
        .filter(|m| granted.is_changed() || pal.made_from.as_ref() != Some(m));
    if let Some(made_from) = made_from {
        let mut entries = Vec::new();
        for s in palette_sections(content, &granted.unlocks, &made_from.0) {
            entries.push(Entry::Head { category: s.category, open: s.parts.len() - s.locked, locked: s.locked });
            entries.extend(s.parts.into_iter().map(Entry::Part));
        }
        // Another search starts at the top; a new material only recolours the list.
        if pal.made_from.as_ref().is_none_or(|m| m.0 != made_from.0) {
            pal.offset = 0;
            pal.jumped = None;
        }
        pal.entries = entries;
        pal.made_from = Some(made_from);
    }
    let (node, tf, mut bar) = list.into_inner();
    let height = node.size.y * node.inverse_scale_factor;
    let max = max_offset(&pal.entries, height);
    let mut offset = pal.offset.min(max);
    let mut jumped = pal.jumped.clone();
    let jump = if ui.jump.is_some() { ui.jump.take() } else { None };
    if let Some(cat) = jump {
        if let Some(i) = pal.entries.iter().position(|e| matches!(e, Entry::Head { category, .. } if *category == cat)) {
            offset = i.min(max);
            jumped = Some(cat);
        }
    } else if let Some(first) = bar.take_want() {
        offset = first.min(max);
        jumped = None;
    } else if scroll.delta.y != 0.0
        && let Some(c) = window.physical_cursor_position()
        && node.contains_point(*tf, c)
    {
        offset = (offset as f32 - wheel_rows(&scroll)).clamp(0.0, max as f32) as usize;
        jumped = None;
    }
    let visible = fitting(&pal.entries[offset.min(pal.entries.len())..], height);
    ScrollRows::show(&mut bar, offset, visible, pal.entries.len());
    // The lit chip: the one clicked while its heading is in view, else the section at the top.
    let in_view = |cat: &str| pal.entries[offset.min(pal.entries.len())..].iter().take(visible).any(|e| matches!(e, Entry::Head { category, .. } if category == cat));
    let lit = jumped.clone().filter(|c| in_view(c)).or_else(|| {
        pal.entries[..(offset + 1).min(pal.entries.len())].iter().rev().find_map(|e| match e {
            Entry::Head { category, .. } => Some(category.clone()),
            Entry::Part(_) => None,
        })
    });
    if offset != pal.offset || visible != pal.visible || jumped != pal.jumped || lit != pal.lit {
        pal.offset = offset;
        pal.visible = visible;
        pal.jumped = jumped;
        pal.lit = lit;
    }
    if !pal.is_changed() && !ui.is_changed() && !granted.is_changed() {
        return;
    }
    for (slot, mut n) in &mut slots {
        set_display(&mut n, pal.shown(slot.0).is_some());
    }
    for (head, mut n) in &mut heads {
        set_display(&mut n, matches!(pal.shown(head.0), Some(Entry::Head { .. })));
    }
    let part = |k: usize| match pal.shown(k) {
        Some(Entry::Part(p)) => Some(p),
        _ => None,
    };
    for (row, mut n, mut action) in &mut rows {
        let p = part(row.0);
        set_display(&mut n, p.is_some());
        // A locked row keeps its part id: `sync_pills` disables it, so it sends nothing.
        let want = DrydockAction::PickPart(p.cloned().unwrap_or_default());
        if action.0 != want {
            action.0 = want;
        }
    }
    for (row, mut icon) in &mut icons {
        let Some(p) = part(row.0) else { continue };
        let want = if granted.unlocks.has_part(p) { part_icon(content, p, &ui.material) } else { UiIcon::Glyph { glyph: Glyph::Lock, color: style.ui.dim } };
        if *icon != want {
            *icon = want;
        }
    }
    for (tag, mut n) in &mut tags {
        set_display(&mut n, part(tag.0).is_some_and(|p| granted.fresh.contains(p)));
    }
}

/// The armour swatches: a locked material's tile is a lock; the wheel over the strip scrolls it by whole swatches.
#[allow(clippy::type_complexity)]
pub(super) fn sync_swatches(
    granted: Res<Granted>,
    style: Res<Style>,
    content: Res<GameContent>,
    window: Single<&Window, With<PrimaryWindow>>,
    scroll: Res<AccumulatedMouseScroll>,
    strip: Single<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition), With<SwatchStrip>>,
    mut tiles: Query<(&SwatchTile, &mut UiIcon)>,
) {
    if granted.is_changed() {
        let block = content.0.unit_hull_part(Shape::Rect).map_or(String::new(), |b| b.id.clone());
        for (tile, mut icon) in &mut tiles {
            let want = if granted.unlocks.has_material(&tile.0) {
                UiIcon::Part { part: block.clone(), material: Some(tile.0.clone()) }
            } else {
                UiIcon::Glyph { glyph: Glyph::Lock, color: style.ui.dim }
            };
            if *icon != want {
                *icon = want;
            }
        }
    }
    let (node, tf, mut pos) = strip.into_inner();
    if scroll.delta.y == 0.0 || !window.physical_cursor_position().is_some_and(|c| node.contains_point(*tf, c)) {
        return;
    }
    let k = node.inverse_scale_factor;
    let max = ((node.content_size.x - node.size.x) * k).max(0.0);
    let step = SWATCH + SWATCH_GAP;
    let x = ((pos.0.x / step).round() - wheel_rows(&scroll)) * step;
    let x = x.clamp(0.0, (max / step).ceil() * step);
    if pos.0.x != x {
        pos.0.x = x;
    }
}

/// The category chips' line: the wheel or a drag over it scrolls it sideways, a lit chip is scrolled into view, and
/// the kit's fades show over an edge with more chips behind it.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn scroll_chips(
    pal: Res<Palette>,
    window: Single<&Window, With<PrimaryWindow>>,
    scroll: Res<AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    line: Single<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition, &Children), With<ChipLine>>,
    chips: Query<(&ComputedNode, &UiGlobalTransform, &ActionButton)>,
    mut drag: Local<Option<(f32, f32)>>,
) {
    let (node, tf, mut pos, children) = line.into_inner();
    let k = node.inverse_scale_factor;
    let max = ((node.content_size.x - node.size.x) * k).max(0.0);
    let cursor = window.physical_cursor_position();
    let over = cursor.is_some_and(|c| node.contains_point(*tf, c));
    let mut x = pos.0.x;
    if buttons.just_pressed(MouseButton::Left) && over {
        *drag = cursor.map(|c| (c.x, x));
    }
    if !buttons.pressed(MouseButton::Left) {
        *drag = None;
    }
    if let (Some((from, start)), Some(c)) = (*drag, cursor) {
        x = start - (c.x - from) * k;
    } else if scroll.delta.y != 0.0 && over {
        x -= wheel_rows(&scroll) * WHEEL_LINE_PX;
    } else if pal.is_changed()
        && let Some(lit) = &pal.lit
        && let Some((chip, at, _)) = children.iter().filter_map(|e| chips.get(e).ok()).find(|(_, _, b)| b.0 == DrydockAction::Category(lit.clone()))
    {
        // The lit chip into view (positions in the line's content, logical px).
        let left = ((at.translation.x - chip.size.x / 2.0) - (tf.translation.x - node.size.x / 2.0)) * k + x;
        let right = left + chip.size.x * k;
        let width = node.size.x * k;
        if left < x {
            x = left;
        } else if right > x + width {
            x = right - width + FADE_DEPTH;
        }
    }
    let x = x.clamp(0.0, max);
    if pos.0.x != x {
        pos.0.x = x;
    }
}

/// Folds the bottom bar's tools into their dropdown when the status line would get less than [`MIN_STATUS`] px,
/// and unfolds them when there is room again (from last frame's layout; the wide tools' width is remembered while
/// they are folded).
#[allow(clippy::type_complexity)]
pub(super) fn fit_bar(
    area: Single<&ComputedNode, With<StatusArea>>,
    mut wide: Single<(&ComputedNode, &mut Node), (With<WideTools>, Without<FoldedTools>)>,
    mut folded: Single<(&ComputedNode, &mut Node), (With<FoldedTools>, Without<WideTools>)>,
    mut ui: ResMut<DrydockUi>,
    mut wide_w: Local<f32>,
) {
    let k = area.inverse_scale_factor;
    let room = area.size.x * k;
    let is_folded = wide.1.display == Display::None;
    let fold = if !is_folded {
        let w = wide.0.size.x * k;
        if w <= 0.0 {
            return;
        }
        *wide_w = w;
        room < MIN_STATUS
    } else {
        let w = folded.0.size.x * k;
        // The room the status line would keep with the wide tools back.
        w <= 0.0 || room - (*wide_w - w) < MIN_STATUS + UNFOLD_MARGIN
    };
    if fold != is_folded {
        set_display(&mut wide.1, !fold);
        set_display(&mut folded.1, fold);
        if !fold && ui.menu == Some(BarMenu::Tools) {
            ui.menu = None;
        }
    }
}

/// The status line and the voyage-ready line, each shortened to its room when it doesn't fit: whole parts dropped
/// from the end (`FLIGHT-READY  ·  …`), else cut with `…`; the tooltip then has them in full. The full line's width
/// is measured while it shows; the room clips what an estimate misses.
#[allow(clippy::type_complexity)]
pub(super) fn fit_status(
    mut commands: Commands,
    status: Res<DrydockStatus>,
    style: Res<Style>,
    area: Single<(Entity, &ComputedNode, Option<&Tip>), With<StatusArea>>,
    mut texts: Query<(&PanelText, &mut Text, &mut TextColor, &ComputedNode), Without<StatusArea>>,
    mut measured: Local<[(String, f32); 2]>,
) {
    let (area, node, tip) = *area;
    let room = node.size.x * node.inverse_scale_factor;
    let mut short = false;
    for (t, mut text, mut color, node) in &mut texts {
        let (i, full, tone) = match t {
            PanelText::Status => (0, &status.line, status.tone),
            PanelText::Voyage => (1, &status.voyage, status.voyage_tone),
            _ => continue,
        };
        let m = &mut measured[i];
        if m.0 != *full {
            *m = (full.clone(), 0.0);
        } else if text.0 == *full {
            m.1 = node.size.x * node.inverse_scale_factor;
        }
        let shown = if m.1 > room && room > 0.0 { shorten(full, room / m.1) } else { full.clone() };
        short |= shown != *full;
        set_text(&mut text, &shown);
        set_color(&mut color, ui_color(tone.color(&style.ui), 1.0));
    }
    let want = short.then(|| format!("{}{SEP}{}", status.line, status.voyage));
    match (want, tip) {
        (Some(w), t) if t.is_none_or(|t| t.name != w) => {
            commands.entity(area).insert(Tip::new(w));
        }
        (None, Some(_)) => {
            commands.entity(area).remove::<Tip>();
        }
        _ => {}
    }
}

/// The separator between the parts of a status line.
const SEP: &str = "  ·  ";

/// A status line shortened to about `share` of its characters: its first parts and `…`, else its first characters
/// and `…`.
fn shorten(line: &str, share: f32) -> String {
    let budget = (line.chars().count() as f32 * share).floor() as usize;
    let parts: Vec<&str> = line.split(SEP).collect();
    for n in (1..parts.len()).rev() {
        let s = format!("{}{SEP}…", parts[..n].join(SEP));
        if s.chars().count() <= budget {
            return s;
        }
    }
    let mut s: String = line.chars().take(budget.saturating_sub(1)).collect();
    s.push('…');
    s
}

/// The open menu's plate and the veil under it.
#[allow(clippy::type_complexity)]
pub(super) fn sync_menus(ui: Res<DrydockUi>, mut plates: Query<(&MenuPlate, &mut Node), Without<MenuVeil>>, mut veil: Single<&mut Node, With<MenuVeil>>) {
    if !ui.is_changed() {
        return;
    }
    for (plate, mut n) in &mut plates {
        set_display(&mut n, ui.menu == Some(plate.0));
    }
    set_display(&mut veil, ui.menu.is_some());
}

/// A fresh part shown in the SELECTED PART block (picked in the palette, or selected on the grid) is fresh no
/// more: its NEW tag goes (and the career writes the save).
pub(super) fn see_fresh(insp: Res<Inspector>, granted: Res<Granted>, mut out: MessageWriter<SeenPart>) {
    if insp.is_changed() && granted.fresh.contains(&insp.part) {
        out.write(SeenPart(insp.part.clone()));
    }
}

/// What a locked palette row says under its name: `LOCKED · <the research node that unlocks it>`.
fn locked_sub(content: &Content, part: &Part) -> String {
    match unlocked_by(content, &Unlock::Part(part.id.clone())) {
        Some(node) => format!("LOCKED  ·  {}", node.name.to_uppercase()),
        None => "LOCKED".into(),
    }
}

/// A part's icon: hull parts in the chosen armour.
fn part_icon(content: &Content, part: &str, material: &str) -> UiIcon {
    let hull = content.parts.get(part).is_some_and(|p| p.hull);
    UiIcon::Part { part: part.to_string(), material: hull.then(|| material.to_string()) }
}

/// The wheel over the selected part's lines scrolls them; another kind of part (or a palette part instead of a
/// placed one) starts at the top.
pub(super) fn scroll_selected(
    insp: Res<Inspector>,
    window: Single<&Window, With<PrimaryWindow>>,
    wheel: Res<AccumulatedMouseScroll>,
    area: Single<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition), With<SelScroll>>,
    mut shown: Local<(String, bool)>,
) {
    let (node, tf, mut pos) = area.into_inner();
    let kind = (insp.part.clone(), insp.installed.is_some());
    if insp.is_changed() && *shown != kind {
        *shown = kind;
        pos.0.y = 0.0;
        return;
    }
    if wheel.delta.y == 0.0 || !window.physical_cursor_position().is_some_and(|c| node.contains_point(*tf, c)) {
        return;
    }
    let max = ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0);
    let y = (pos.0.y - wheel_rows(&wheel) * WHEEL_LINE_PX).clamp(0.0, max);
    if pos.0.y != y {
        pos.0.y = y;
    }
}

/// The selected part's icon, its summary (none for a placed part), its stat and bonus rows (from [`Inspector`]),
/// the stats grid's bars and cells.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync_blocks(
    insp: Res<Inspector>,
    stats: Res<DrydockStats>,
    mut sel_icon: Single<&mut UiIcon, With<SelIcon>>,
    mut texts: Query<(&PanelText, &mut Node), (Without<SelStat>, Without<SelBonus>, Without<StatCellNode>, Without<StatBar>)>,
    mut sel_stats: Query<(&SelStat, &mut Node), (Without<SelBonus>, Without<StatCellNode>)>,
    mut sel_bonuses: Query<(&SelBonus, &mut Node), (Without<SelStat>, Without<StatCellNode>)>,
    mut cells: Query<(&StatCellNode, &mut Node), (Without<SelStat>, Without<SelBonus>)>,
    mut bars: Query<(&StatBar, &mut ValueBar, &mut Node), (Without<SelStat>, Without<SelBonus>, Without<StatCellNode>)>,
) {
    if insp.is_changed() {
        let want = UiIcon::Part { part: insp.part.clone(), material: insp.material.clone() };
        if **sel_icon != want {
            **sel_icon = want;
        }
        for (t, mut n) in &mut texts {
            if *t == PanelText::SelSummary {
                set_display(&mut n, !insp.summary.is_empty());
            }
        }
        for (s, mut n) in &mut sel_stats {
            set_display(&mut n, s.0 < insp.stats.len());
        }
        for (b, mut n) in &mut sel_bonuses {
            set_display(&mut n, b.0 < insp.bonuses.len());
        }
    }
    if stats.is_changed() {
        for (cell, mut n) in &mut cells {
            set_display(&mut n, cell.0 < stats.cells.len());
        }
        for (bar, mut v, mut n) in &mut bars {
            let b = stats.cells.get(bar.0).and_then(|c| c.bar);
            set_display(&mut n, b.is_some());
            if let Some((share, tone)) = b {
                let want = ValueBar { share, tone };
                if *v != want {
                    *v = want;
                }
            }
        }
    }
}

fn set_display(n: &mut Node, show: bool) {
    let d = if show { Display::Flex } else { Display::None };
    if n.display != d {
        n.display = d;
    }
}

/// The one stat a palette row shows: the part's first own stat after the common ones (build points, control,
/// mass, hit points, power draw, heat), else the bonus it gives, else its hit points.
fn key_stat(content: &Content, part: &Part, material: &str) -> Option<String> {
    const COMMON: [&str; 8] = ["build_points", "control_cost", "mass", "hp", "power_draw", "power_min_level", "heat_output", "heat_limit"];
    let lines = content.part_stats(part, content.materials.get(material).filter(|_| part.hull));
    let pick = lines
        .iter()
        .find(|l| l.bonus.is_none() && !COMMON.contains(&l.stat.as_str()))
        .or_else(|| lines.iter().find(|l| l.bonus.is_some()))
        .or_else(|| lines.iter().find(|l| l.stat == "hp"))?;
    Some(format!("{} {}", pick.label, pick.text).to_uppercase())
}

fn footprint(part: &Part) -> String {
    format!("{}×{}", part.size.0, part.size.1)
}

/// Every text that changes only with the drydock's state.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_texts(
    design: Res<PlayerDesign>,
    ui: Res<DrydockUi>,
    status: Res<DrydockStatus>,
    stats: Res<DrydockStats>,
    insp: Res<Inspector>,
    pal: Res<Palette>,
    content: Res<GameContent>,
    granted: Res<Granted>,
    style: Res<Style>,
    mut texts: Query<(&PanelText, &mut Text, &mut TextColor)>,
    mut done: Local<bool>,
) {
    if *done
        && !design.is_changed()
        && !ui.is_changed()
        && !status.is_changed()
        && !stats.is_changed()
        && !insp.is_changed()
        && !pal.is_changed()
        && !granted.is_changed()
    {
        return;
    }
    *done = true;
    let content = &content.0;
    let row_part = |k: usize| match pal.shown(k) {
        Some(Entry::Part(p)) => content.parts.get(p),
        _ => None,
    };
    let head = |k: usize| match pal.shown(k) {
        Some(Entry::Head { category, open, locked }) => Some((category, *open, *locked)),
        _ => None,
    };
    let mut s = String::new();
    for (t, mut text, mut color) in &mut texts {
        s.clear();
        let tone = match *t {
            PanelText::Breadcrumb => {
                s.push_str(&format!("SHIPYARD  ›  DRYDOCK  ·  {}", super::blueprints::frame_name(content, design.frame())));
                Tone::Label
            }
            PanelText::Name => {
                s.push_str(&design.blueprint().name);
                Tone::Number
            }
            PanelText::Armor => {
                s.push_str(&content.materials.get(&ui.material).map_or(String::new(), |m| m.name.to_uppercase()));
                Tone::Number
            }
            PanelText::ToolName => {
                s.push_str(match ui.tool {
                    Tool::Paint => "PAINT",
                    Tool::Fill => "FILL",
                    Tool::Erase => "ERASE",
                    Tool::Select => "SELECT",
                });
                // A pill's text: the kit colours it.
                set_text(&mut text, &s);
                continue;
            }
            PanelText::HeadName(k) => {
                if let Some((category, ..)) = head(k) {
                    s.push_str(&content.categories.get(category).map_or(category.to_uppercase(), |c| c.name.to_uppercase()));
                }
                Tone::Label
            }
            PanelText::HeadCount(k) => {
                if let Some((_, open, locked)) = head(k) {
                    let _ = write!(s, "{open}");
                    if locked > 0 {
                        let _ = write!(s, "  ·  {locked} LOCKED");
                    }
                }
                Tone::Dim
            }
            PanelText::RowCount(k) => match row_part(k).filter(|p| granted.unlocks.has_part(&p.id)) {
                Some(p) => match design.dock().left(&p.id) {
                    Availability::Unlimited => {
                        s.push('∞');
                        Tone::Label
                    }
                    Availability::Count(n) => {
                        let _ = write!(s, "{n}");
                        if n == 0 { Tone::Coral } else { Tone::Label }
                    }
                },
                None => Tone::Label,
            },
            PanelText::SelName => {
                s.push_str(&insp.name);
                insp.name_tone
            }
            PanelText::SelSub => {
                s.push_str(&insp.sub);
                insp.sub_tone
            }
            PanelText::SelSummary => {
                s.push_str(&insp.summary);
                Tone::Label
            }
            PanelText::RowName(k) => {
                let p = row_part(k);
                s.push_str(p.map_or("", |p| p.name.as_str()));
                if p.is_some_and(|p| !granted.unlocks.has_part(&p.id)) { Tone::Dim } else { Tone::Number }
            }
            PanelText::RowSub(k) => {
                match row_part(k) {
                    Some(p) if !granted.unlocks.has_part(&p.id) => s.push_str(&locked_sub(content, p)),
                    Some(p) => {
                        s.push_str(&footprint(p));
                        if let Some(stat) = key_stat(content, p, &ui.material) {
                            s.push_str(" · ");
                            s.push_str(&stat);
                        }
                    }
                    None => {}
                }
                sub_tone(row_part(k).is_some_and(|p| ui.picked.as_deref() == Some(p.id.as_str())))
            }
            PanelText::StatLabel(k) => {
                s.push_str(stats.cells.get(k).map_or("", |c| c.label.as_str()));
                Tone::Label
            }
            PanelText::StatValue(k) => {
                let cell = stats.cells.get(k);
                s.push_str(cell.map_or("", |c| c.value.as_str()));
                cell.map_or(Tone::Number, |c| c.tone)
            }
            PanelText::SelStatLabel(k) => {
                s.push_str(insp.stats.get(k).map_or("", |l| l.label.as_str()));
                Tone::Label
            }
            PanelText::SelStatValue(k) => {
                let line = insp.stats.get(k);
                s.push_str(line.map_or("", |l| l.value.as_str()));
                line.map_or(Tone::Number, |l| l.tone)
            }
            PanelText::SelBonus(k) => {
                s.push_str(insp.bonuses.get(k).map_or("", |b| b.as_str()));
                Tone::Gold
            }
            // The status lines are fitted to their room (`fit_status`), the info line and the zoom written live.
            PanelText::Status | PanelText::Voyage | PanelText::Info | PanelText::Zoom => continue,
        };
        set_text(&mut text, &s);
        set_color(&mut color, ui_color(tone.color(&style.ui), 1.0));
    }
}

/// The info line (hover cell, mirror, axis, and the pan help unless the coral hint after it needs the room) and
/// the zoom, every frame.
pub(super) fn sync_live(
    design: Res<PlayerDesign>,
    cursor: Res<DrydockCursor>,
    rig: Res<CameraRig>,
    cam: Res<DockCamera>,
    hint: Single<&TextSpan, With<super::ghost::HintText>>,
    mut texts: Query<(&PanelText, &mut Text)>,
    mut s: Local<String>,
) {
    for (t, mut text) in &mut texts {
        s.clear();
        match *t {
            PanelText::Info => {
                let _ = match cursor.cell {
                    Some((x, y)) => write!(s, "CELL {x},{y}"),
                    None => write!(s, "CELL —"),
                };
                let mirror = if design.dock().mirror { "ON" } else { "OFF" };
                let _ = write!(s, "  ·  MIRROR {mirror}  ·  AXIS {}", design.frame().0 as f32 / 2.0);
                if hint.0.is_empty() {
                    s.push_str("  ·  SPACE + DRAG TO PAN");
                }
            }
            PanelText::Zoom => {
                if cam.fit > 0.0 {
                    let _ = write!(s, "{:.0} %", rig.zoom / cam.fit * 100.0);
                }
            }
            _ => continue,
        }
        set_text(&mut text, &s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A status line too long for its room drops whole parts from the end, else is cut, always ending in `…`.
    #[test]
    fn long_status_lines_are_shortened() {
        let line = "FLIGHT-READY  ·  RUNS HOT AT FULL LOAD  ·  +1";
        assert_eq!(shorten(line, 0.6), "FLIGHT-READY  ·  …");
        assert_eq!(shorten(line, 0.99), "FLIGHT-READY  ·  RUNS HOT AT FULL LOAD  ·  …");
        assert_eq!(shorten("SAVED AS WORK IN PROGRESS", 0.4), "SAVED AS …");
        assert!(shorten(line, 0.0).ends_with('…'));
    }

    /// The list shows the entries that fit from its first one, and scrolls no further than the last screenful.
    #[test]
    fn the_list_fits_headings_and_rows() {
        let head = || Entry::Head { category: "hull".into(), open: 2, locked: 0 };
        let part = || Entry::Part("armor_block".into());
        let entries = vec![head(), part(), part(), head(), part()];
        let all = 3.0 * ROW_H + 2.0 * HEAD_H + 4.0 * ROW_GAP;
        assert_eq!(fitting(&entries, all), 5);
        assert_eq!(fitting(&entries, all - 1.0), 4);
        assert_eq!(max_offset(&entries, all), 0);
        // Room for a heading and a row: the last screenful starts at the second heading.
        assert_eq!(max_offset(&entries, HEAD_H + ROW_H + ROW_GAP), 3);
        assert_eq!((fitting(&[], 100.0), max_offset(&[], 100.0)), (0, 0));
    }
}
