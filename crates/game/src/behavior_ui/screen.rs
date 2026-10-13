//! The BEHAVIOR screen's nodes, laid out after the mockup (`docs/concept/boards/ui-mockups/phone-behavior-editor.png`)
//! for 1280 × 720 logical pixels and up (larger windows scale it):
//!
//! ```text
//! (o) BEHAVIOR · HALVEX RUNNER   [6 / 12 RULE SLOTS]            [GRAPH][TEXT]   [▶ TEST IN RANGE T]
//!     Skirmisher  UNSAVED                                                       [WATCH][FLY ALONGSIDE]
//!             TARGET ── [Nearest]                                          │ ACTION · MANEUVER
//!                     ┌● [Hull < 25%] ──────────────── [Flee]  too hurt    │ Orbit
//!   ( Priority   )────┼● [Shield down] [Target on me] ─ [Orbit 80% · ccw]  │ Circle the target at …
//!   ( FIRST MATCH)    ┊ ┄┄ 2 RULE SLOTS · THE RULES BELOW DO NOT RUN ┄┄┄   │ RADIUS · 80%   …
//!                     └● [always] ─────────────────── [Approach 67%]      │ ADD NODE  [search]
//!                        ( + RULE · NO SLOT LEFT )                         │ [HULL][SHIELD CHARGE]…
//! [‹ DRYDOCK ESC]  status line                                                 [BEHAVIORS] [SAVE CTRL S]
//! ```
//!
//! The frame (header, pills, columns) is spawned once at start, hidden outside the screen; the graph, the
//! inspector and the palette are rebuilt from `graph` / `inspect` data when what they show changes (a new
//! program revision, another selection, another search). Colours are written only when they change. UI words
//! are chrome in code (like the drydock); node names, summaries and parameter names come from content. The
//! inspector's controls are spawned by `controls.rs`.

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::PrimaryWindow;
use data::NodeRole;
use data::behavior_edit::RuleId;

use crate::camera::CameraRig;
use crate::drydock::behaviors::PlayerBehaviors;
use crate::flight_hud::HudFonts;
use crate::fx::{FxQueue, FxRegions};
use crate::sim_bridge::GameContent;
use crate::style::{Style, UiPalette, ui_color};
use crate::ui_icons::{Glyph, UiIcon};
use crate::ui_kit::{Hot, Pill, TextField, TextFieldText, Tone, glow, label, prose, ring_node, set_color, set_text, spawn_pill, tracked};

use super::actions::{BehaviorAction, BehaviorButton};
use bevy::ui_render::prelude::MaterialNode;

use super::controls::{DesignReach, SliderDrag, spawn_inspection};
use super::dashed::{DashedOutline, outline_color};
use super::draft::BehaviorDraft;
use super::edit::EditState;
use super::graph::{Chip, GraphView, add_rule_label, slot_line_label, slots_pill};
use super::inspect::{self, Selection, behavior_inspection, inspect, palette, palette_hint};
use super::saving::ScreenNote;
use super::text_view::TextViewport;
use super::{BehaviorUi, BehaviorView, EditorContext, TestMode};

/// Where the columns sit (logical px at 1280 × 720).
const TOP: f32 = 104.0;
const BOTTOM: f32 = 66.0;
const SIDE: f32 = 22.0;
const RIGHT_W: f32 = 296.0;
/// The graph area ends this far from the right edge (the right column, its divider and a gap).
const GRAPH_RIGHT: f32 = SIDE + RIGHT_W + 34.0;
/// The graph's columns: the root, the trunk; its rows.
const ROOT_W: f32 = 150.0;
const TRUNK_W: f32 = 30.0;
const ROW_H: f32 = 52.0;
const SLOT_ROW_H: f32 = 30.0;
const CHIP_H: f32 = 34.0;
const CHIP_FONT: f32 = 14.5;
const LINE_W: f32 = 1.2;
/// How much inactive rules (no rule slot) are dimmed.
const INACTIVE: f32 = 0.4;
/// Dashes in a dashed line (more than the widest line needs; the rest is clipped).
const DASHES: usize = 160;
/// Wheel steps in px.
const WHEEL_LINE: f32 = 40.0;
/// The palette keeps at least this height (about three rows of pills); a taller inspector scrolls.
const PALETTE_MIN: f32 = 120.0;
/// The dot grid behind the graph: spacing in logical px, FX layer (under ships and effects; the world is empty).
const DOT_STEP: f32 = 28.0;
const Z_DOTS: f32 = 4.0;

/// The screen's root (shown only on the screen).
#[derive(Component)]
pub struct BehaviorRoot;

/// The graph area: scrolls, and hides in the TEXT view (the editor covers the same rectangle).
#[derive(Component)]
pub(super) struct GraphViewport;

/// The graph's nodes (rebuilt).
#[derive(Component)]
pub(super) struct GraphContent;

/// The inspector block (rebuilt).
#[derive(Component)]
pub(super) struct InspectorBody;

/// The palette's pills (rebuilt; scrolls).
#[derive(Component)]
pub(super) struct PaletteList;

/// The palette search (`/` focuses it).
#[derive(Component)]
pub struct SearchField;

/// What the graph was built from: the program revision and the rule slots.
#[derive(Resource, Default)]
pub struct GraphBuilt(Option<(u64, u32)>);

/// Texts that follow the state.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScreenText {
    Breadcrumb,
    Name,
    Slots,
    Status,
    /// The line above the palette: where a pick goes (`ADD NODE · TO RULE 3`).
    AddNode,
}

/// The coral refusal after the status line (fades after a few seconds).
#[derive(Component)]
pub(super) struct NoteSpan;

/// The look of a chip (graph nodes, `always`, palette entries, `+ RULE`), coloured by [`style_chips`] from its
/// state.
#[derive(Component, Clone, Debug)]
pub(super) struct ChipLook {
    /// The role colour (a palette colour; `ui_color` is applied when styling).
    color: Color,
    /// A dashed outline instead of the border (Rhai nodes, `+ RULE`).
    dashed: Option<Handle<DashedOutline>>,
    /// A faint fill when idle (graph nodes).
    fill: bool,
    /// Dimmed when false (a rule without a slot).
    active: bool,
    /// What it shows (a node, or a rule for `always`): glows while that is selected.
    select: Option<Selection>,
}

/// A cell of a rule's row: the row is lit while the rule (or one of its nodes) is selected.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct RowBand(RuleId);

impl RowBand {
    pub(super) fn rule(&self) -> RuleId {
        self.0
    }
}

/// Fonts, colours and materials for spawning.
pub(super) struct Kit<'a> {
    pub(super) fonts: &'a HudFonts,
    pub(super) ui: UiPalette,
}

impl Kit<'_> {
    pub(super) fn c(&self, tone: Tone, a: f32) -> Color {
        ui_color(tone.color(&self.ui), a)
    }
}

fn abs(l: Option<f32>, r: Option<f32>, t: Option<f32>, b: Option<f32>) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: l.map(px).unwrap_or(Val::Auto),
        right: r.map(px).unwrap_or(Val::Auto),
        top: t.map(px).unwrap_or(Val::Auto),
        bottom: b.map(px).unwrap_or(Val::Auto),
        ..default()
    }
}

/// The colour of a role: conditions violet, actions teal, Rhai gold, target selectors white.
pub(super) fn role_color(ui: &UiPalette, role: NodeRole, rhai: bool) -> Color {
    match role {
        _ if rhai => ui.gold,
        NodeRole::Condition => ui.violet,
        NodeRole::Action => ui.teal,
        NodeRole::Target => ui.number,
    }
}

// ---------------------------------------------------------------------------------------------------------
// The frame

pub(super) fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let kit = Kit { fonts: &fonts, ui: style.ui };
    let fonts = &*fonts;
    commands
        .spawn((
            BehaviorRoot,
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // Top left: the ring menu, the breadcrumb and the behavior's name, the rule slots.
            root.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(14), ..abs(Some(SIDE), None, Some(14.0), None) })
                .with_children(|tl| {
                    tl.spawn((ring_node(40.0), BehaviorButton(BehaviorAction::Menu))).with_children(|r| {
                        r.spawn((UiIcon::Glyph { glyph: Glyph::Menu, color: kit.ui.violet }, Node { width: px(22), height: px(22), ..default() }));
                    });
                    tl.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                        col.spawn((ScreenText::Breadcrumb, label(fonts, "", 10.0, kit.c(Tone::Label, 1.0))));
                        // The name (a click renames it) and its saved state (`saving.rs`).
                        super::saving::spawn_title(col, fonts, kit.c(Tone::Number, 1.0), kit.c(Tone::Dim, 1.0), kit.c(Tone::Label, 0.3));
                    });
                    tl.spawn((
                        Node {
                            height: px(30),
                            padding: UiRect::horizontal(px(16)),
                            margin: UiRect::left(px(22)),
                            border: UiRect::all(px(LINE_W)),
                            border_radius: BorderRadius::MAX,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(kit.c(Tone::Violet, 0.7)),
                        BackgroundColor(kit.c(Tone::Violet, 0.06)),
                    ))
                    .with_children(|p| {
                        p.spawn((ScreenText::Slots, tracked(fonts, "", 9.5, kit.c(Tone::Violet, 1.0), 0.3)));
                    });
                });

            // Top right: GRAPH / TEXT, TEST IN RANGE and its two modes under it.
            root.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::FlexStart, column_gap: px(28), ..abs(None, Some(20.0), Some(16.0), None) })
                .with_children(|tr| {
                    tr.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(8), margin: UiRect::top(px(3)), ..default() }).with_children(|v| {
                        for (text, view) in [("GRAPH", BehaviorView::Graph), ("TEXT", BehaviorView::Text)] {
                            spawn_pill(v, fonts, text, 10.0, 34.0, None, Pill::new(Tone::Teal), BehaviorButton(BehaviorAction::View(view)));
                        }
                    });
                    tr.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: px(8), ..default() }).with_children(|t| {
                        let play = UiIcon::Glyph { glyph: Glyph::Play, color: kit.ui.teal };
                        spawn_pill(t, fonts, "TEST IN RANGE  T", 12.0, 40.0, Some(play), Pill { primary: true, ..Pill::new(Tone::Teal) }, BehaviorButton(BehaviorAction::TestInRange));
                        t.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(6), ..default() }).with_children(|m| {
                            for (text, mode) in [("WATCH", TestMode::Watch), ("FLY ALONGSIDE", TestMode::Alongside)] {
                                spawn_pill(m, fonts, text, 8.5, 24.0, None, Pill::new(Tone::Teal), BehaviorButton(BehaviorAction::TestMode(mode)));
                            }
                        });
                    });
                });

            // The graph area (and, in the TEXT view, the editor's rectangle).
            // A click on its empty space deselects.
            root.spawn((
                GraphViewport,
                TextViewport,
                Hot::default(),
                BehaviorButton(BehaviorAction::Deselect),
                ScrollPosition::default(),
                // Long rules are clipped at the right edge, not drawn over the inspector.
                Node {
                    overflow: Overflow { x: OverflowAxis::Clip, y: OverflowAxis::Scroll },
                    flex_direction: FlexDirection::Column,
                    ..abs(Some(SIDE), Some(GRAPH_RIGHT), Some(TOP), Some(BOTTOM))
                },
            ))
            .with_children(|g| {
                g.spawn((GraphContent, Node { flex_direction: FlexDirection::Column, flex_shrink: 0.0, padding: UiRect::vertical(px(6)), ..default() }));
            });

            // The divider and the right column: the inspector, ADD NODE with its search and the palette.
            root.spawn((Node { width: px(1), ..abs(None, Some(SIDE + RIGHT_W + 16.0), Some(TOP), Some(BOTTOM)) }, BackgroundColor(kit.c(Tone::Label, 0.14))));
            root.spawn(Node { width: px(RIGHT_W), flex_direction: FlexDirection::Column, ..abs(None, Some(SIDE), Some(TOP), Some(BOTTOM)) }).with_children(|rp| {
                rp.spawn((InspectorBody, ScrollPosition::default(), Node { flex_direction: FlexDirection::Column, min_height: px(0), overflow: Overflow::scroll_y(), ..default() }));
                rp.spawn((Node { height: px(1), margin: UiRect::vertical(px(18)), flex_shrink: 0.0, ..default() }, BackgroundColor(kit.c(Tone::Label, 0.14))));
                rp.spawn((ScreenText::AddNode, label(fonts, "ADD NODE", 9.0, kit.c(Tone::Label, 1.0)), Node { margin: UiRect::bottom(px(10)), flex_shrink: 0.0, ..default() }));
                rp.spawn((
                    SearchField,
                    TextField { placeholder: "Search nodes".into(), max_chars: 24, ..default() },
                    Hot::default(),
                    Node {
                        height: px(32),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(px(14)),
                        column_gap: px(10),
                        margin: UiRect::bottom(px(12)),
                        border: UiRect::all(px(LINE_W)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(kit.c(Tone::Label, 0.3)),
                ))
                .with_children(|f| {
                    f.spawn((UiIcon::Glyph { glyph: Glyph::Search, color: kit.ui.label }, Node { width: px(13), height: px(13), flex_shrink: 0.0, ..default() }));
                    f.spawn((TextFieldText, prose(fonts, "", 12.0, kit.c(Tone::Dim, 1.0)), Node { flex_grow: 1.0, ..default() }));
                    f.spawn(label(fonts, "/", 10.0, kit.c(Tone::Dim, 1.0)));
                });
                rp.spawn((
                    PaletteList,
                    ScrollPosition::default(),
                    Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        align_content: AlignContent::FlexStart,
                        column_gap: px(6),
                        row_gap: px(7),
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        min_height: px(PALETTE_MIN),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                ));
            });

            // Bottom: back to the drydock, undo / redo, the status line (and a refusal after it); the behavior list
            // and SAVE (as in the drydock).
            root.spawn(Node { height: px(40), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..abs(Some(SIDE), Some(20.0), None, Some(14.0)) })
                .with_children(|bb| {
                    spawn_pill(bb, fonts, "‹  DRYDOCK  ESC", 10.0, 32.0, None, Pill::new(Tone::Teal), BehaviorButton(BehaviorAction::Back));
                    bb.spawn(Node { width: px(8), ..default() });
                    for (g, action) in [(Glyph::Undo, BehaviorAction::Undo), (Glyph::Redo, BehaviorAction::Redo)] {
                        bb.spawn((ring_node(34.0), BehaviorButton(action))).with_children(|r| {
                            r.spawn((UiIcon::Glyph { glyph: g, color: kit.ui.dim }, Node { width: px(18), height: px(18), ..default() }));
                        });
                    }
                    bb.spawn((
                        ScreenText::Status,
                        label(fonts, "", 10.0, kit.c(Tone::Label, 1.0)),
                        Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), margin: UiRect::left(px(14)), ..default() },
                    ))
                        .with_child((
                            NoteSpan,
                            TextSpan::new(""),
                            TextFont { font: fonts.regular.clone().into(), font_size: FontSize::Px(10.0), ..default() },
                            TextColor(kit.c(Tone::Coral, 1.0)),
                            bevy::text::LetterSpacing::Px(3.0),
                        ));
                    bb.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(10), flex_shrink: 0.0, ..default() }).with_children(|r| {
                        spawn_pill(r, fonts, "BEHAVIORS", 9.5, 30.0, None, Pill::new(Tone::Teal), BehaviorButton(BehaviorAction::Behaviors));
                        spawn_pill(r, fonts, "SAVE  CTRL S", 9.5, 30.0, None, Pill::new(Tone::Teal), BehaviorButton(BehaviorAction::Save));
                    });
                });
        });
}

// ---------------------------------------------------------------------------------------------------------
// The graph

fn spawn_line(p: &mut ChildSpawnerCommands, node: Node, color: Color) {
    p.spawn((node, BackgroundColor(color)));
}

/// A dashed horizontal line: `width` long, or (`None`) filling what the parent row leaves (it adds nothing to
/// the row's own size).
fn spawn_dashes(p: &mut ChildSpawnerCommands, width: Option<f32>, color: Color) {
    p.spawn(Node {
        width: width.map_or(Val::Auto, px),
        flex_grow: if width.is_none() { 1.0 } else { 0.0 },
        flex_shrink: 0.0,
        flex_basis: if width.is_none() { px(0) } else { Val::Auto },
        min_width: px(0),
        height: px(LINE_W),
        flex_direction: FlexDirection::Row,
        column_gap: px(4),
        overflow: Overflow::clip(),
        ..default()
    })
    .with_children(|d| {
        for _ in 0..DASHES {
            d.spawn((Node { width: px(6), height: px(LINE_W), flex_shrink: 0.0, ..default() }, BackgroundColor(color)));
        }
    });
}

/// A chip of the graph: the node's name, its values dim, the comparison, `not` and the RHAI tag.
fn spawn_chip(p: &mut ChildSpawnerCommands, kit: &Kit, materials: &mut Assets<DashedOutline>, chip: &Chip, active: bool) {
    let color = role_color(&kit.ui, chip.role, false);
    let k = if active { 1.0 } else { INACTIVE };
    let dashed = chip.rhai.then(|| materials.add(DashedOutline { color: outline_color(ui_color(color, 0.6 * k)) }));
    let mut e = p.spawn((
        ChipLook { color, dashed: dashed.clone(), fill: true, active, select: Some(Selection::Node(chip.node)) },
        Hot::default(),
        BehaviorButton(BehaviorAction::Select(chip.node)),
        Node {
            height: px(CHIP_H),
            padding: UiRect::horizontal(px(16)),
            border: UiRect::all(px(LINE_W)),
            border_radius: BorderRadius::MAX,
            align_items: AlignItems::Center,
            column_gap: px(7),
            flex_shrink: 0.0,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    ));
    if let Some(m) = dashed {
        e.insert(MaterialNode(m));
    }
    e.with_children(|c| {
        if chip.not {
            c.spawn(prose(kit.fonts, "not", CHIP_FONT, kit.c(Tone::Violet, k)));
        }
        c.spawn(prose(kit.fonts, &chip.name, CHIP_FONT, kit.c(Tone::Number, 0.95 * k)));
        if !chip.args.is_empty() {
            c.spawn(prose(kit.fonts, &chip.args, CHIP_FONT, kit.c(Tone::Label, k)));
        }
        if !chip.compare.is_empty() {
            c.spawn(prose(kit.fonts, &chip.compare, CHIP_FONT, kit.c(Tone::Number, 0.95 * k)));
        }
        if chip.rhai {
            c.spawn((tracked(kit.fonts, "RHAI", 8.5, kit.c(Tone::Gold, k), 0.3), Node { margin: UiRect::left(px(3)), ..default() }));
        }
    });
}

/// The `always` of a rule without conditions: a plain grey chip, not a node; a click selects the rule.
fn spawn_always(p: &mut ChildSpawnerCommands, kit: &Kit, rule: RuleId, active: bool) {
    let k = if active { 1.0 } else { INACTIVE };
    p.spawn((
        ChipLook { color: kit.ui.label, dashed: None, fill: false, active, select: Some(Selection::Rule(rule)) },
        Hot::default(),
        BehaviorButton(BehaviorAction::SelectRule(rule)),
        Node {
            height: px(CHIP_H),
            padding: UiRect::horizontal(px(16)),
            border: UiRect::all(px(LINE_W)),
            border_radius: BorderRadius::MAX,
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    ))
    .with_child(prose(kit.fonts, "always", CHIP_FONT, kit.c(Tone::Label, k)));
}

/// A cell of a rule's row: a click on its empty space selects the rule; lit while the rule is selected.
fn row_cell(rule: RuleId, node: Node) -> impl Bundle {
    (RowBand(rule), Hot::default(), BehaviorButton(BehaviorAction::SelectRule(rule)), node, BackgroundColor(Color::NONE))
}

/// The trunk cell of a rule: the vertical line (from the first rule's centre to the last one's), the dot and
/// the stub to the first chip.
fn spawn_trunk(p: &mut ChildSpawnerCommands, kit: &Kit, height: f32, line: (bool, bool), dot: Option<bool>) {
    p.spawn(Node { width: px(TRUNK_W), height: px(height), ..default() }).with_children(|t| {
        let mid = height / 2.0;
        let (above, below) = line;
        if above || below {
            let top = if above { 0.0 } else { mid };
            let bottom = if below { 0.0 } else { mid };
            spawn_line(t, Node { width: px(LINE_W), ..abs(Some(0.0), None, Some(top), Some(bottom)) }, kit.c(Tone::Label, 0.4));
        }
        if let Some(active) = dot {
            let k = if active { 1.0 } else { INACTIVE };
            spawn_line(t, Node { height: px(LINE_W), ..abs(Some(0.0), Some(0.0), Some(mid - LINE_W / 2.0), None) }, kit.c(Tone::Label, 0.4 * k));
            t.spawn((
                Node { width: px(7), height: px(7), border_radius: BorderRadius::MAX, ..abs(Some(-3.4), None, Some(mid - 3.5), None) },
                BackgroundColor(kit.c(Tone::Teal, 0.9 * k)),
            ));
        }
    });
}

/// The slot line: dashed, with its label, above the first rule without a slot.
fn spawn_slot_line(p: &mut ChildSpawnerCommands, kit: &Kit, slots: u32, span: bool) {
    let mut node = Node { height: px(SLOT_ROW_H), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() };
    if span {
        node.grid_column = GridPlacement::span(3);
    }
    p.spawn(node).with_children(|s| {
        let coral = kit.c(Tone::Coral, 0.55);
        spawn_dashes(s, Some(26.0), coral);
        s.spawn((tracked(kit.fonts, &slot_line_label(slots), 8.5, kit.c(Tone::Coral, 0.85), 0.3), Node { flex_shrink: 0.0, ..default() }));
        spawn_dashes(s, None, coral);
    });
}

/// Rebuilds the graph when the program or the rule slots change.
#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild_graph(
    mut commands: Commands,
    draft: Res<BehaviorDraft>,
    context: Res<EditorContext>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    mut built: ResMut<GraphBuilt>,
    mut materials: ResMut<Assets<DashedOutline>>,
    graph: Single<Entity, With<GraphContent>>,
) {
    let key = (draft.revision(), context.slots);
    let Some(ed) = draft.editor() else { return };
    if built.0 == Some(key) {
        return;
    }
    built.0 = Some(key);
    let g = GraphView::of(ed, context.slots);
    let kit = Kit { fonts: &fonts, ui: style.ui };
    let materials = &mut *materials;
    commands.entity(*graph).despawn_children().with_children(|gc| {
        // The target line.
        gc.spawn(Node { height: px(ROW_H), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, flex_shrink: 0.0, ..default() }).with_children(|r| {
            r.spawn(Node { width: px(ROOT_W + TRUNK_W), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::FlexEnd, column_gap: px(10), flex_shrink: 0.0, ..default() })
                .with_children(|l| {
                    l.spawn(tracked(kit.fonts, "TARGET", 8.5, kit.c(Tone::Dim, 1.0), 0.3));
                    spawn_line(l, Node { width: px(26), height: px(LINE_W), margin: UiRect::right(px(10)), ..default() }, kit.c(Tone::Label, 0.4));
                });
            spawn_chip(r, &kit, materials, &g.target, true);
            if g.default_target {
                r.spawn((tracked(kit.fonts, "DEFAULT", 8.0, kit.c(Tone::Dim, 1.0), 0.3), Node { margin: UiRect::left(px(12)), ..default() }));
            }
        });
        let slot_line = g.slot_line();
        if slot_line == Some(0) {
            gc.spawn(Node { flex_direction: FlexDirection::Row, flex_shrink: 0.0, padding: UiRect::left(px(ROOT_W + TRUNK_W)), ..default() })
                .with_children(|s| spawn_slot_line(s, &kit, g.slots, false));
        }
        // The rules: the root, the trunk and one row per rule, actions in one column.
        gc.spawn(Node { flex_direction: FlexDirection::Row, flex_shrink: 0.0, ..default() }).with_children(|block| {
            block.spawn(Node { width: px(ROOT_W), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() }).with_children(|rc| {
                rc.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::new(px(18), px(22), px(10), px(11)),
                        row_gap: px(3),
                        border: UiRect::all(px(LINE_W)),
                        border_radius: BorderRadius::all(px(24)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BorderColor::all(kit.c(Tone::Label, 0.5)),
                ))
                .with_children(|root| {
                    root.spawn(prose(kit.fonts, "Priority", 16.0, kit.c(Tone::Number, 1.0)));
                    root.spawn(tracked(kit.fonts, "FIRST MATCH", 7.5, kit.c(Tone::Label, 1.0), 0.3));
                });
                if !g.rules.is_empty() {
                    spawn_line(rc, Node { flex_grow: 1.0, height: px(LINE_W), ..default() }, kit.c(Tone::Label, 0.4));
                }
            });
            block
                .spawn(Node {
                    display: Display::Grid,
                    grid_template_columns: vec![GridTrack::px(TRUNK_W), GridTrack::max_content(), GridTrack::max_content(), GridTrack::flex(1.0)],
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|grid| {
                    let last = g.rules.len().saturating_sub(1);
                    for (i, row) in g.rules.iter().enumerate() {
                        if slot_line == Some(i) && i > 0 {
                            spawn_trunk(grid, &kit, SLOT_ROW_H, (true, true), None);
                            spawn_slot_line(grid, &kit, g.slots, true);
                        }
                        let k = if row.active { 1.0 } else { INACTIVE };
                        spawn_trunk(grid, &kit, ROW_H, (i > 0, i < last), Some(row.active));
                        let cell = Node { height: px(ROW_H), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() };
                        grid.spawn(row_cell(row.id, Node { column_gap: px(8), ..cell.clone() })).with_children(|cc| {
                            if row.conds.is_empty() {
                                spawn_always(cc, &kit, row.id, row.active);
                            }
                            for c in &row.conds {
                                spawn_chip(cc, &kit, materials, c, row.active);
                            }
                            spawn_line(cc, Node { flex_grow: 1.0, min_width: px(26), height: px(LINE_W), margin: UiRect::horizontal(px(8)), ..default() }, kit.c(Tone::Label, 0.4 * k));
                        });
                        grid.spawn(row_cell(row.id, cell.clone())).with_children(|ac| spawn_chip(ac, &kit, materials, &row.action, row.active));
                        grid.spawn(row_cell(row.id, Node { padding: UiRect::new(px(16), px(8), px(0), px(0)), overflow: Overflow::clip(), ..cell }))
                            .with_children(|nc| {
                                if let Some(note) = &row.note {
                                    nc.spawn(prose(kit.fonts, note, 12.0, kit.c(Tone::Dim, k)));
                                }
                            });
                    }
                });
        });
        // + RULE.
        gc.spawn(Node { flex_direction: FlexDirection::Row, flex_shrink: 0.0, padding: UiRect::new(px(ROOT_W + TRUNK_W), px(0), px(12), px(8)), ..default() })
            .with_children(|r| {
                let m = materials.add(DashedOutline { color: outline_color(kit.c(Tone::Label, 0.5)) });
                r.spawn((
                    ChipLook { color: kit.ui.label, dashed: Some(m.clone()), fill: false, active: true, select: None },
                    MaterialNode(m),
                    Hot::default(),
                    BehaviorButton(BehaviorAction::AddRule),
                    Node {
                        height: px(CHIP_H),
                        padding: UiRect::horizontal(px(20)),
                        border: UiRect::all(px(LINE_W)),
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::NONE),
                    BackgroundColor(Color::NONE),
                ))
                .with_child(tracked(kit.fonts, &add_rule_label(g.rules.len(), g.slots), 9.0, kit.c(Tone::Label, 1.0), 0.3));
            });
    });
}

// ---------------------------------------------------------------------------------------------------------
// The right column

/// What the inspector was built from.
#[derive(Clone, PartialEq)]
pub(super) struct InspectorKey {
    selected: Option<Selection>,
    revision: u64,
    cx: inspect::Context,
    name: String,
    source: Option<String>,
}

/// Rebuilds the inspector when the selection, the program, REPLACE, the slots or the behavior (its name, which one
/// it is) change; not while a slider is held (it keeps its node and follows in place).
#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild_inspector(
    mut commands: Commands,
    ui: Res<BehaviorUi>,
    draft: Res<BehaviorDraft>,
    context: Res<EditorContext>,
    edit: Res<EditState>,
    reach: Res<DesignReach>,
    drag: Res<SliderDrag>,
    content: Res<GameContent>,
    behaviors: Res<PlayerBehaviors>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    body: Single<(Entity, &mut ScrollPosition), With<InspectorBody>>,
    mut built: Local<Option<InspectorKey>>,
) {
    let Some(ed) = draft.editor() else { return };
    if drag.0.is_some() {
        return;
    }
    let cx = inspect::Context { slots: context.slots, reach: reach.0, replacing: edit.replacing };
    let key = InspectorKey {
        selected: ui.selected.clone(),
        revision: draft.revision(),
        cx,
        name: draft.name.clone(),
        source: draft.source.clone(),
    };
    if built.as_ref() == Some(&key) {
        return;
    }
    let (body, mut scroll) = body.into_inner();
    // Another selection starts at the top; an edit keeps the place.
    if built.as_ref().is_none_or(|b| b.selected != key.selected) {
        scroll.0 = Vec2::ZERO;
    }
    *built = Some(key);
    let i = ui.selected.as_ref().and_then(|s| inspect(ed, s, &cx)).unwrap_or_else(|| {
        let source = draft.source.as_deref().and_then(|id| behaviors.behavior(&content.0, id));
        behavior_inspection(&draft.name, source, ed.rules().len())
    });
    let kit = Kit { fonts: &fonts, ui: style.ui };
    commands.entity(body).despawn_children().with_children(|p| spawn_inspection(p, &kit, &i));
}

/// Rebuilds the palette when the search changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild_palette(
    mut commands: Commands,
    ui: Res<BehaviorUi>,
    content: Res<GameContent>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    list: Single<(Entity, &mut ScrollPosition), With<PaletteList>>,
    mut built: Local<Option<String>>,
) {
    if built.as_deref() == Some(ui.search.as_str()) {
        return;
    }
    *built = Some(ui.search.clone());
    let kit = Kit { fonts: &fonts, ui: style.ui };
    let (list, mut scroll) = list.into_inner();
    scroll.0 = Vec2::ZERO;
    let entries = palette(&content.0.behavior_nodes, &ui.search);
    commands.entity(list).despawn_children().with_children(|l| {
        if entries.is_empty() {
            l.spawn(tracked(kit.fonts, "NO NODE MATCHES", 8.5, kit.c(Tone::Dim, 1.0), 0.3));
        }
        for e in &entries {
            let color = role_color(&kit.ui, e.role, e.rhai);
            l.spawn((
                ChipLook { color, dashed: None, fill: false, active: true, select: None },
                Hot::default(),
                BehaviorButton(BehaviorAction::AddNode(e.id.clone())),
                Node { height: px(28), padding: UiRect::horizontal(px(13)), border: UiRect::all(px(LINE_W)), border_radius: BorderRadius::MAX, align_items: AlignItems::Center, ..default() },
                BorderColor::all(Color::NONE),
                BackgroundColor(Color::NONE),
            ))
            .with_child(tracked(kit.fonts, &e.label, 8.5, ui_color(color, 1.0), 0.28));
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
// Following the state

/// The search field and [`BehaviorUi::search`] stay equal.
pub(super) fn sync_search(mut ui: ResMut<BehaviorUi>, mut field: Single<&mut TextField, With<SearchField>>) {
    if field.is_changed() && field.value != ui.search {
        ui.search.clone_from(&field.value);
    } else if ui.is_changed() && ui.search != field.value {
        field.value.clone_from(&ui.search);
    }
}

/// GRAPH shows the graph; TEXT hides it (the editor draws over its rectangle).
pub(super) fn show_view(ui: Res<BehaviorUi>, mut graph: Single<&mut Visibility, With<GraphViewport>>) {
    let want = if ui.view == BehaviorView::Graph { Visibility::Inherited } else { Visibility::Hidden };
    if **graph != want {
        **graph = want;
    }
}

/// The status line under the screen: errors in the text, rules without a slot, else how the rules run.
pub fn status_line(draft: &BehaviorDraft, view: BehaviorView, slots: u32) -> (String, Tone) {
    let errors = draft.errors.len();
    let s = if errors == 1 { "" } else { "S" };
    if let Some(e) = draft.errors.first() {
        return match view {
            BehaviorView::Text => (format!("{errors} ERROR{s}  ·  FIRST ON LINE {}, COLUMN {}", e.line, e.col), Tone::Coral),
            BehaviorView::Graph => (format!("THE TEXT HAS {errors} ERROR{s}  ·  THE GRAPH SHOWS THE LAST VALID RULES"), Tone::Gold),
        };
    }
    let rules = draft.program().rules.len();
    if slots == 0 && rules > 0 {
        return ("NO LOGIC CORE  ·  THIS DESIGN CANNOT RUN RULES".into(), Tone::Coral);
    }
    let over = rules.saturating_sub(slots as usize);
    if over > 0 {
        let s = if over == 1 { "" } else { "S" };
        return (format!("{over} RULE{s} WITHOUT A SLOT  ·  LOGIC CORES GIVE RULE SLOTS"), Tone::Gold);
    }
    match view {
        BehaviorView::Text => (format!("VALID  ·  {rules} RULE{}", if rules == 1 { "" } else { "S" }), Tone::Teal),
        BehaviorView::Graph => ("THE FIRST RULE THAT MATCHES DECIDES  ·  GUNS FIRE AT WILL".into(), Tone::Label),
    }
}

/// The header and the status line (a message from saving, opening or testing first, until the draft changes).
pub(super) fn sync_texts(
    draft: Res<BehaviorDraft>,
    ui: Res<BehaviorUi>,
    context: Res<EditorContext>,
    edit: Res<EditState>,
    note: Res<ScreenNote>,
    style: Res<Style>,
    mut texts: Query<(&ScreenText, &mut Text, &mut TextColor)>,
) {
    if !draft.is_changed() && !ui.is_changed() && !context.is_changed() && !edit.is_changed() && !note.is_changed() {
        return;
    }
    for (t, mut text, mut color) in &mut texts {
        let (s, tone) = match t {
            ScreenText::Breadcrumb => (format!("BEHAVIOR  ·  {}", context.design_name.to_uppercase()), Tone::Label),
            ScreenText::Name => (draft.name.clone(), Tone::Number),
            ScreenText::Slots => (slots_pill(draft.program().rules.len(), context.slots), Tone::Violet),
            ScreenText::Status => note.show(&draft).unwrap_or_else(|| status_line(&draft, ui.view, context.slots)),
            ScreenText::AddNode => match draft.editor() {
                Some(ed) => (palette_hint(ed, ui.selected.as_ref(), edit.replacing), if edit.replacing.is_some() { Tone::Teal } else { Tone::Label }),
                None => ("ADD NODE".into(), Tone::Label),
            },
        };
        set_text(&mut text, &s);
        set_color(&mut color, ui_color(tone.color(&style.ui), 1.0));
    }
}

/// The last refusal after the status line, while it is fresh.
pub(super) fn sync_note(edit: Res<EditState>, time: Res<Time<Real>>, mut span: Single<&mut TextSpan, With<NoteSpan>>) {
    let want = edit.current_note(time.elapsed_secs()).map_or(String::new(), |t| format!("  ·  {}", t.to_uppercase()));
    if span.0 != want {
        span.0 = want;
    }
}

/// Active states of the view and test mode pills.
pub(super) fn sync_pills(ui: Res<BehaviorUi>, mut pills: Query<(&BehaviorButton, &mut Pill)>) {
    if !ui.is_changed() {
        return;
    }
    for (b, mut pill) in &mut pills {
        let active = match &b.0 {
            BehaviorAction::View(v) => ui.view == *v,
            BehaviorAction::TestMode(m) => ui.test_mode == *m,
            _ => continue,
        };
        if pill.active != active {
            pill.active = active;
        }
    }
}

/// Colours of chips from their state: idle, hovered, pressed, selected (a glow), dimmed without a slot.
#[allow(clippy::type_complexity)]
pub(super) fn style_chips(
    ui: Res<BehaviorUi>,
    mut chips: Query<(Ref<ChipLook>, Ref<Hot>, &mut BorderColor, &mut BackgroundColor, Option<&mut BoxShadow>, Entity)>,
    mut materials: ResMut<Assets<DashedOutline>>,
    mut commands: Commands,
) {
    let selection_changed = ui.is_changed();
    for (look, hot, mut border, mut bg, shadow, e) in &mut chips {
        if !selection_changed && !look.is_changed() && !hot.is_changed() {
            continue;
        }
        let selected = look.select.is_some() && ui.selected == look.select;
        let k = if look.active { 1.0 } else { INACTIVE };
        let idle_fill = if look.fill { 0.05 } else { 0.0 };
        let (edge, fill, glow_alpha) = if selected {
            (1.0, 0.16, 0.35)
        } else if hot.pressed {
            (1.0, 0.2, 0.0)
        } else if hot.hovered {
            (0.95, idle_fill + 0.06, 0.0)
        } else {
            (if look.fill { 0.6 } else { 0.5 }, idle_fill, 0.0)
        };
        let edge_c = ui_color(look.color, edge * k);
        let b = if look.dashed.is_some() { Color::NONE } else { edge_c };
        if border.top != b {
            *border = BorderColor::all(b);
        }
        if let Some(h) = &look.dashed
            && let Some(mut m) = materials.get_mut(h)
        {
            let want = outline_color(edge_c);
            if m.color != want {
                m.color = want;
            }
        }
        let f = ui_color(look.color, fill * k);
        if bg.0 != f {
            bg.0 = f;
        }
        match (shadow, glow_alpha > 0.0) {
            (Some(mut s), true) => *s = glow(look.color, glow_alpha),
            (Some(_), false) => {
                commands.entity(e).remove::<BoxShadow>();
            }
            (None, true) => {
                commands.entity(e).insert(glow(look.color, glow_alpha));
            }
            (None, false) => {}
        }
    }
}

/// A faint band behind the selected rule's row (the rule, or one of its nodes, is selected); fainter on hover.
pub(super) fn style_rows(
    ui: Res<BehaviorUi>,
    draft: Res<BehaviorDraft>,
    style: Res<Style>,
    mut cells: Query<(&RowBand, Ref<Hot>, &mut BackgroundColor)>,
) {
    let selected = draft.editor().and_then(|ed| ui.selected.as_ref()?.rule(ed));
    let all = ui.is_changed() || draft.is_changed();
    for (band, hot, mut bg) in &mut cells {
        if !all && !hot.is_changed() {
            continue;
        }
        let a = if selected == Some(band.0) { 0.05 } else if hot.hovered { 0.025 } else { 0.0 };
        let want = ui_color(style.ui.number, a);
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// The dot grid behind the graph (as in the mockup): immediate-mode FX dots at fixed screen spacing inside the
/// graph area, drawn in the world under the UI.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_dots(
    ui: Res<BehaviorUi>,
    rig: Res<CameraRig>,
    style: Res<Style>,
    ui_scale: Res<UiScale>,
    window: Single<&Window, With<PrimaryWindow>>,
    graph: Single<(&ComputedNode, &UiGlobalTransform), With<GraphViewport>>,
    regions: Option<Res<FxRegions>>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(r) = regions else { return };
    if ui.view != BehaviorView::Graph {
        return;
    }
    let (node, tf) = *graph;
    let scale = window.scale_factor();
    let area = Rect::from_center_size(tf.translation / scale, node.size / scale);
    let step = DOT_STEP * ui_scale.0;
    let (nx, ny) = ((area.width() / step) as i32, (area.height() / step) as i32);
    let size = 2.0 * ui_scale.0 / rig.zoom;
    let color = style.dotted_ring.with_alpha(0.8);
    for y in 0..=ny {
        for x in 0..=nx {
            let at = area.min + Vec2::new(x as f32, y as f32) * step;
            queue.dot(&r.hard, rig.screen_to_world(at), size, color, Z_DOTS);
        }
    }
}

/// The wheel scrolls the graph or the palette under the pointer.
#[allow(clippy::type_complexity)]
pub(super) fn scroll(
    window: Single<&Window, With<PrimaryWindow>>,
    wheel: Res<AccumulatedMouseScroll>,
    ui: Res<BehaviorUi>,
    mut areas: Query<
        (&ComputedNode, &UiGlobalTransform, &mut ScrollPosition, &Children, Has<GraphViewport>),
        Or<(With<GraphViewport>, With<PaletteList>, With<InspectorBody>)>,
    >,
    contents: Query<&ComputedNode>,
) {
    if wheel.delta.y == 0.0 {
        return;
    }
    let Some(cursor) = window.physical_cursor_position() else { return };
    let dy = match wheel.unit {
        MouseScrollUnit::Line => wheel.delta.y * WHEEL_LINE,
        MouseScrollUnit::Pixel => wheel.delta.y,
    };
    for (node, tf, mut pos, children, is_graph) in &mut areas {
        if (is_graph && ui.view != BehaviorView::Graph) || !node.contains_point(*tf, cursor) {
            continue;
        }
        // The content's height: the graph has one content node; the palette's pills and the inspector's blocks are
        // laid out inside their node.
        let content_h = if is_graph {
            children.iter().filter_map(|c| contents.get(c).ok()).map(|c| c.size.y).sum::<f32>()
        } else {
            node.content_size.y
        };
        let max = ((content_h - node.size.y) * node.inverse_scale_factor).max(0.0);
        let y = (pos.0.y - dy).clamp(0.0, max);
        if pos.0.y != y {
            pos.0.y = y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::behavior::TextError;

    #[test]
    fn the_status_line_says_what_matters_most() {
        let content = data::base_content();
        let mut d = BehaviorDraft::from_behavior(content.behaviors.get("skirmisher").unwrap(), &content.behavior_nodes).unwrap();
        assert_eq!(status_line(&d, BehaviorView::Graph, 12), ("THE FIRST RULE THAT MATCHES DECIDES  ·  GUNS FIRE AT WILL".into(), Tone::Label));
        assert_eq!(status_line(&d, BehaviorView::Text, 12), ("VALID  ·  6 RULES".into(), Tone::Teal));
        assert_eq!(status_line(&d, BehaviorView::Graph, 4), ("2 RULES WITHOUT A SLOT  ·  LOGIC CORES GIVE RULE SLOTS".into(), Tone::Gold));
        assert_eq!(status_line(&d, BehaviorView::Graph, 0).1, Tone::Coral);
        d.errors = vec![TextError { line: 4, col: 6, message: "x".into() }];
        assert_eq!(status_line(&d, BehaviorView::Text, 12), ("1 ERROR  ·  FIRST ON LINE 4, COLUMN 6".into(), Tone::Coral));
        assert_eq!(status_line(&d, BehaviorView::Graph, 12).1, Tone::Gold);
    }
}
