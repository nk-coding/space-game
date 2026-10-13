//! The drydock's screen, laid out after the mockup (`docs/concept/boards/ui-mockups/desktop-drydock.png`) for
//! 1280 × 720 logical pixels and up (larger windows scale it: `view::ui_scale`):
//!
//! ```text
//! (o) SHIPYARD › DRYDOCK · 24×24 FRAME          ARMOR [STEEL 1][COMPOSITE 2].. | OVERLAY [HEAT][MASS][BONUS]..
//!     Halvex Runner
//! [ Search parts      / ]   [PAINT B][FILL G][ERASE X][SELECT V][ROTATE Q E]   POWER  —   PEAK HEAT  —
//! [HULL][POWER][WEAPONS]..                                                     THRUST/MASS — YAW —   ...
//! HULL · 4                              (the grid: view.rs)                    SELECTED PART
//! [▣ Armor block   1×1 · ..]                                                   [▣] Armor block
//! ...                                   CELL 5,2 · MIRROR ON · AXIS 12 ...  (−) 100 % (+)
//! (↶)(↷) [MIRROR M][BEVEL SHIFT B]  FLIGHT-READY         [BLUEPRINTS][EXPORT][SAVE CTRL S]  [▶ TEST-FLY T]
//! ```
//!
//! Everything is spawned once at start (hidden outside the drydock) and updated in place from the drydock's
//! resources; texts and colours are written only when they change. UI words are chrome in code (like the flight
//! HUD); part names, summaries and stat labels come from content.

use std::fmt::Write as _;

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use data::{Content, Part};

use crate::camera::CameraRig;
use crate::flight_hud::HudFonts;
use crate::sim_bridge::GameContent;
use crate::style::{Style, ui_color};
use crate::ui_icons::{Glyph, UiIcon};
use crate::ui_kit::{
    Hot, Pill, PillText, TextField, TextFieldText, Tone, UiBlock, ValueBar, label, number, prose, ring_node, set_color, set_text,
    spawn_bar, spawn_brackets, spawn_pill, tracked,
};

use super::actions::{ActionButton, DrydockAction, SearchField};
use super::inspector::Inspector;
use super::state::{DrydockStats, DrydockStatus, DrydockUi, Overlay, Tool, armor_materials, palette_parts};
use super::view::{DockCamera, DrydockCursor, GridViewport};
use super::PlayerDesign;

/// Palette rows in the pool (as many show as fit; the wheel scrolls).
const ROWS: usize = 24;
const ROW_H: f32 = 48.0;
const ROW_GAP: f32 = 4.0;
/// Stat cells (right panel) and the selected part's stat lines and bonus lines.
const STAT_CELLS: usize = 8;
const SEL_STATS: usize = 12;
const SEL_BONUSES: usize = 3;

/// The drydock's UI root (shown only in the drydock).
#[derive(Component)]
pub(super) struct DrydockRoot;

#[derive(Component)]
pub(super) struct PaletteList;

#[derive(Component)]
pub(super) struct PaletteRow(usize);

#[derive(Component)]
pub(super) struct RowIcon(usize);

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

/// Every text the drydock updates.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelText {
    Breadcrumb,
    Name,
    Section,
    Status,
    Info,
    Zoom,
    SelName,
    SelSub,
    SelSummary,
    RowName(usize),
    RowSub(usize),
    StatLabel(usize),
    StatValue(usize),
    SelStatLabel(usize),
    SelStatValue(usize),
    SelBonus(usize),
}

/// The palette list: the parts it shows (category or search), the first row shown and how many fit.
#[derive(Resource, Default, Debug)]
pub(super) struct Palette {
    parts: Vec<String>,
    offset: usize,
    visible: usize,
    /// What the list was made from: category, search, material.
    made_from: Option<(String, String, String)>,
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
    let fonts = &*fonts;

    commands
        .spawn((
            DrydockRoot,
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // Top left: the ring menu, the breadcrumb and the design's name.
            root.spawn((UiBlock, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(14), ..abs(Some(22.0), None, Some(14.0), None) }))
                .with_children(|tl| {
                    tl.spawn((ring_node(40.0), ActionButton(DrydockAction::Menu))).with_children(|r| {
                        r.spawn(glyph(Glyph::Menu, Tone::Violet, 22.0));
                    });
                    tl.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                        col.spawn((PanelText::Breadcrumb, label(fonts, "", 10.0, c(Tone::Label, 1.0))));
                        col.spawn((PanelText::Name, number(fonts, "", 20.0, c(Tone::Number, 1.0))));
                    });
                });

            // Top right: armour materials and overlays.
            root.spawn((UiBlock, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(5), ..abs(None, Some(20.0), Some(19.0), None) }))
                .with_children(|tr| {
                    tr.spawn((label(fonts, "ARMOR", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::right(px(5)), ..default() }));
                    for (k, m) in armor_materials(content).enumerate() {
                        let text = if k < 9 { format!("{} {}", m.name.to_uppercase(), k + 1) } else { m.name.to_uppercase() };
                        spawn_pill(tr, fonts, &text, 9.0, 28.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Material(m.id.clone())));
                    }
                    tr.spawn((Node { width: px(1), height: px(22), margin: UiRect::horizontal(px(8)), ..default() }, BackgroundColor(c(Tone::Label, 0.25))));
                    tr.spawn((label(fonts, "OVERLAY", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::right(px(5)), ..default() }));
                    for o in Overlay::ALL {
                        let accent = if o == Overlay::Bonus { Tone::Gold } else { Tone::Teal };
                        spawn_pill(tr, fonts, overlay_name(o), 9.0, 28.0, None, pill(accent), ActionButton(DrydockAction::Overlay(o)));
                    }
                });

            // Left: search, categories, the part list.
            root.spawn((UiBlock, Node { width: px(248), flex_direction: FlexDirection::Column, row_gap: px(12), ..abs(Some(20.0), None, Some(74.0), Some(70.0)) }))
                .with_children(|lp| {
                    lp.spawn((
                        SearchField,
                        TextField { placeholder: "Search parts".into(), max_chars: 24, ..default() },
                        Hot::default(),
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
                        f.spawn((TextFieldText, prose(fonts, "", 12.5, c(Tone::Dim, 1.0)), Node { flex_grow: 1.0, ..default() }));
                        f.spawn(label(fonts, "/", 11.0, c(Tone::Dim, 1.0)));
                    });
                    lp.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: px(6), row_gap: px(6), flex_shrink: 0.0, ..default() })
                        .with_children(|cats| {
                            for cat in content.categories.iter() {
                                spawn_pill(cats, fonts, &cat.name.to_uppercase(), 9.0, 26.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Category(cat.id.clone())));
                            }
                        });
                    lp.spawn((PanelText::Section, label(fonts, "", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::new(px(6), px(0), px(8), px(0)), ..default() }));
                    lp.spawn((
                        PaletteList,
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
                        for k in 0..ROWS {
                            list.spawn((
                                PaletteRow(k),
                                ActionButton(DrydockAction::PickPart(String::new())),
                                Pill { bare: true, ..pill(Tone::Teal) },
                                Hot::default(),
                                Node {
                                    display: Display::None,
                                    height: px(ROW_H),
                                    flex_shrink: 0.0,
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::horizontal(px(12)),
                                    column_gap: px(14),
                                    border: UiRect::all(px(1.2)),
                                    border_radius: BorderRadius::MAX,
                                    ..default()
                                },
                                BorderColor::all(Color::NONE),
                                BackgroundColor(Color::NONE),
                            ))
                            .with_children(|row| {
                                row.spawn((
                                    RowIcon(k),
                                    UiIcon::Part { part: String::new(), material: None },
                                    Node { width: px(34), height: px(30), flex_shrink: 0.0, ..default() },
                                ));
                                row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|t| {
                                    t.spawn((PanelText::RowName(k), prose(fonts, "", 14.0, c(Tone::Number, 1.0))));
                                    t.spawn((PanelText::RowSub(k), tracked(fonts, "", 8.5, c(Tone::Dim, 1.0), 0.2)));
                                });
                            });
                        }
                    });
                });

            // Right: the stats grid and the selected part.
            root.spawn((UiBlock, Node { width: px(292), flex_direction: FlexDirection::Column, ..abs(None, Some(20.0), Some(78.0), Some(70.0)) }))
                .with_children(|rp| {
                    rp.spawn(Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                        column_gap: px(28),
                        row_gap: px(16),
                        ..default()
                    })
                    .with_children(|g| {
                        for k in 0..STAT_CELLS {
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
                    rp.spawn((Node { height: px(1), margin: UiRect::vertical(px(20)), flex_shrink: 0.0, ..default() }, BackgroundColor(c(Tone::Label, 0.14))));
                    rp.spawn((label(fonts, "SELECTED PART", 9.0, c(Tone::Label, 1.0)), Node { margin: UiRect::bottom(px(14)), ..default() }));
                    rp.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(16), ..default() }).with_children(|h| {
                        // The icon in a bracketed box (like the fleet bar's chips).
                        h.spawn(Node { width: px(60), height: px(60), flex_shrink: 0.0, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() })
                            .with_children(|b| {
                                spawn_brackets(b, 9.0, 1.2, c(Tone::Label, 0.45));
                                b.spawn((SelIcon, UiIcon::Part { part: String::new(), material: None }, Node { width: px(44), height: px(44), ..default() }));
                            });
                        h.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(6), ..default() }).with_children(|t| {
                            t.spawn((PanelText::SelName, prose(fonts, "", 18.0, c(Tone::Number, 1.0))));
                            t.spawn((PanelText::SelSub, label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                        });
                    });
                    rp.spawn((PanelText::SelSummary, prose(fonts, "", 11.5, c(Tone::Label, 1.0)), Node { margin: UiRect::top(px(12)), ..default() }));
                    rp.spawn(Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
                        column_gap: px(12),
                        row_gap: px(10),
                        margin: UiRect::top(px(14)),
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
                    rp.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(8), margin: UiRect::top(px(12)), ..default() }).with_children(|b| {
                        for k in 0..SEL_BONUSES {
                            b.spawn((SelBonus(k), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() }))
                                .with_children(|r| {
                                    r.spawn(glyph(Glyph::Diamond, Tone::Gold, 9.0));
                                    r.spawn((PanelText::SelBonus(k), tracked(fonts, "", 8.5, c(Tone::Gold, 1.0), 0.2)));
                                });
                        }
                    });
                    super::inspector::spawn_controls(rp, fonts, &style, content);
                });

            // Centre: the tools above the grid; the grid viewport; the info line and the zoom.
            root.spawn(Node { flex_direction: FlexDirection::Column, ..abs(Some(290.0), Some(332.0), Some(76.0), Some(66.0)) }).with_children(|ga| {
                ga.spawn((UiBlock, Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::Center, column_gap: px(8), flex_shrink: 0.0, ..default() }))
                    .with_children(|tb| {
                        for (text, action) in [
                            ("PAINT B", DrydockAction::Tool(Tool::Paint)),
                            ("FILL G", DrydockAction::Tool(Tool::Fill)),
                            ("ERASE X", DrydockAction::Tool(Tool::Erase)),
                            ("SELECT V", DrydockAction::Tool(Tool::Select)),
                            ("ROTATE Q E", DrydockAction::RotateCw),
                        ] {
                            spawn_pill(tb, fonts, text, 9.5, 30.0, None, pill(Tone::Teal), ActionButton(action));
                        }
                    });
                ga.spawn((GridViewport, Node { flex_grow: 1.0, ..default() }));
                ga.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, height: px(30), flex_shrink: 0.0, ..default() })
                    .with_children(|b| {
                        b.spawn((PanelText::Info, label(fonts, "", 9.0, c(Tone::Dim, 1.0)))).with_child(super::ghost::hint_span(fonts, &style));
                        b.spawn((UiBlock, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() })).with_children(|z| {
                            z.spawn((ring_node(28.0), ActionButton(DrydockAction::ZoomOut))).with_children(|r| {
                                r.spawn((PillText, number(fonts, "−", 15.0, c(Tone::Number, 1.0))));
                            });
                            z.spawn((PanelText::Zoom, number(fonts, "", 13.0, c(Tone::Number, 1.0)), Node { min_width: px(46), ..default() }));
                            z.spawn((ring_node(28.0), ActionButton(DrydockAction::ZoomIn))).with_children(|r| {
                                r.spawn((PillText, number(fonts, "+", 15.0, c(Tone::Number, 1.0))));
                            });
                        });
                    });
            });

            // Bottom: undo / redo, mirror and bevel, the status line, blueprints / export / save, the test flight.
            root.spawn((UiBlock, Node { height: px(42), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..abs(Some(22.0), Some(20.0), None, Some(14.0)) }))
                .with_children(|bb| {
                    for (g, action) in [(Glyph::Undo, DrydockAction::Undo), (Glyph::Redo, DrydockAction::Redo)] {
                        bb.spawn((ring_node(38.0), ActionButton(action))).with_children(|r| {
                            r.spawn(glyph(g, Tone::Number, 20.0));
                        });
                    }
                    bb.spawn(Node { width: px(14), ..default() });
                    spawn_pill(bb, fonts, "MIRROR M", 10.0, 32.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Mirror));
                    spawn_pill(bb, fonts, "BEVEL SHIFT B", 10.0, 32.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Bevel));
                    bb.spawn((PanelText::Status, label(fonts, "", 10.0, c(Tone::Teal, 1.0)), Node { flex_grow: 1.0, margin: UiRect::left(px(22)), ..default() }));
                    spawn_pill(bb, fonts, "BLUEPRINTS", 10.0, 32.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Blueprints));
                    spawn_pill(bb, fonts, "EXPORT", 10.0, 32.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Export));
                    spawn_pill(bb, fonts, "SAVE CTRL S", 10.0, 32.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Save));
                    bb.spawn(Node { width: px(10), ..default() });
                    let play = UiIcon::Glyph { glyph: Glyph::Play, color: ui.teal };
                    spawn_pill(bb, fonts, "TEST-FLY T", 12.0, 40.0, Some(play), Pill { primary: true, ..pill(Tone::Teal) }, ActionButton(DrydockAction::TestFly));
                });
        });
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
/// the editor's; the inspector keeps its own pills).
pub(super) fn sync_pills(ui: Res<DrydockUi>, design: Res<PlayerDesign>, status: Res<DrydockStatus>, mut pills: Query<(&ActionButton, &mut Pill)>) {
    for (b, mut pill) in &mut pills {
        let (active, disabled) = match &b.0 {
            DrydockAction::Tool(t) => (ui.tool == *t, false),
            DrydockAction::Mirror => (design.dock().mirror, false),
            DrydockAction::Bevel => (design.dock().bevel, false),
            DrydockAction::Material(m) => (ui.material == *m, false),
            DrydockAction::Overlay(o) => (ui.overlay(*o), false),
            DrydockAction::Category(cat) => (ui.search.is_empty() && ui.category == *cat, false),
            DrydockAction::PickPart(p) => (ui.picked.as_deref() == Some(p.as_str()), false),
            DrydockAction::TestFly => (false, status.grounded.is_some()),
            _ => continue,
        };
        if pill.active != active || pill.disabled != disabled {
            pill.active = active;
            pill.disabled = disabled;
        }
    }
}

/// The part list: which parts (category or search), how many rows fit, the wheel over the list scrolls it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync_palette(
    ui: Res<DrydockUi>,
    content: Res<GameContent>,
    mut pal: ResMut<Palette>,
    window: Single<&Window, With<PrimaryWindow>>,
    list: Single<(&ComputedNode, &UiGlobalTransform), With<PaletteList>>,
    scroll: Res<AccumulatedMouseScroll>,
    mut rows: Query<(&PaletteRow, &mut Node, &mut ActionButton)>,
    mut icons: Query<(&RowIcon, &mut UiIcon)>,
) {
    let content = &content.0;
    let made_from = (ui.is_changed() || pal.made_from.is_none())
        .then(|| (ui.category.clone(), ui.search.to_lowercase(), ui.material.clone()))
        .filter(|m| pal.made_from.as_ref() != Some(m));
    if let Some(made_from) = made_from {
        let parts: Vec<String> = if made_from.1.is_empty() {
            palette_parts(content).filter(|p| p.category == ui.category).map(|p| p.id.clone()).collect()
        } else {
            palette_parts(content).filter(|p| p.name.to_lowercase().contains(&made_from.1)).map(|p| p.id.clone()).collect()
        };
        // A new list starts at the top; a new material only recolours it.
        if pal.made_from.as_ref().is_none_or(|m| m.0 != made_from.0 || m.1 != made_from.1) {
            pal.offset = 0;
        }
        pal.parts = parts;
        pal.made_from = Some(made_from);
    }
    let (node, tf) = *list;
    let height = node.size.y * node.inverse_scale_factor;
    let visible = (((height + ROW_GAP) / (ROW_H + ROW_GAP)).floor() as usize).clamp(1, ROWS);
    let max_offset = pal.parts.len().saturating_sub(visible);
    let mut offset = pal.offset.min(max_offset);
    if scroll.delta.y != 0.0
        && let Some(c) = window.physical_cursor_position()
        && node.contains_point(*tf, c)
    {
        let steps = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        offset = (offset as f32 - steps.signum() * steps.abs().max(1.0).round()).clamp(0.0, max_offset as f32) as usize;
    }
    if offset != pal.offset || visible != pal.visible {
        pal.offset = offset;
        pal.visible = visible;
    }
    if !pal.is_changed() && !ui.is_changed() {
        return;
    }
    let shown = |k: usize| (k < pal.visible).then(|| pal.parts.get(pal.offset + k)).flatten();
    for (row, mut n, mut action) in &mut rows {
        let part = shown(row.0);
        let display = if part.is_some() { Display::Flex } else { Display::None };
        if n.display != display {
            n.display = display;
        }
        let want = DrydockAction::PickPart(part.cloned().unwrap_or_default());
        if action.0 != want {
            action.0 = want;
        }
    }
    for (row, mut icon) in &mut icons {
        let Some(part) = shown(row.0) else { continue };
        let want = part_icon(content, part, &ui.material);
        if *icon != want {
            *icon = want;
        }
    }
}

/// A part's icon: hull parts in the chosen armour.
fn part_icon(content: &Content, part: &str, material: &str) -> UiIcon {
    let hull = content.parts.get(part).is_some_and(|p| p.hull);
    UiIcon::Part { part: part.to_string(), material: hull.then(|| material.to_string()) }
}

/// The selected part's icon, its stat and bonus rows (from [`Inspector`]), the stats grid's bars and cells.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync_blocks(
    insp: Res<Inspector>,
    stats: Res<DrydockStats>,
    mut sel_icon: Single<&mut UiIcon, With<SelIcon>>,
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
    style: Res<Style>,
    mut texts: Query<(&PanelText, &mut Text, &mut TextColor)>,
    mut done: Local<bool>,
) {
    if *done && !design.is_changed() && !ui.is_changed() && !status.is_changed() && !stats.is_changed() && !insp.is_changed() && !pal.is_changed() {
        return;
    }
    *done = true;
    let content = &content.0;
    let row_part = |k: usize| (k < pal.visible).then(|| pal.parts.get(pal.offset + k)).flatten().and_then(|p| content.parts.get(p));
    let mut s = String::new();
    for (t, mut text, mut color) in &mut texts {
        s.clear();
        let tone = match *t {
            PanelText::Breadcrumb => {
                s.push_str(&format!("SHIPYARD  ›  DRYDOCK  ·  {}×{} FRAME", design.frame().0, design.frame().1));
                Tone::Label
            }
            PanelText::Name => {
                s.push_str(&design.blueprint().name);
                Tone::Number
            }
            PanelText::Section => {
                let title = if ui.search.is_empty() {
                    content.categories.get(&ui.category).map_or(String::new(), |c| c.name.to_uppercase())
                } else {
                    "SEARCH".into()
                };
                s.push_str(&format!("{title}  ·  {}", pal.parts.len()));
                Tone::Dim
            }
            PanelText::Status => {
                s.push_str(&status.line);
                status.tone
            }
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
                s.push_str(row_part(k).map_or("", |p| p.name.as_str()));
                Tone::Number
            }
            PanelText::RowSub(k) => {
                if let Some(p) = row_part(k) {
                    s.push_str(&footprint(p));
                    if let Some(stat) = key_stat(content, p, &ui.material) {
                        s.push_str(" · ");
                        s.push_str(&stat);
                    }
                }
                Tone::Dim
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
            PanelText::Info | PanelText::Zoom => continue,
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
