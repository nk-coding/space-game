//! The research screen's widgets: built once from the content's tree ([`spawn`]), then kept in step with the
//! career and the selection ([`sync`], [`style_cards`]). Laid out for 1280 × 720 logical px (the UI scale grows it
//! with the window, as the drydock's); the lanes scroll with the wheel when the content has more than fit.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::text::LetterSpacing;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use data::{Content, ResearchNode, Unlock};

use crate::career::{Granted, PlayerCareer};
use crate::flight_hud::HudFonts;
use crate::mode::AppMode;
use crate::sim_bridge::GameContent;
use crate::style::{Style, ui_color};
use crate::ui_icons::{Glyph, UiIcon};
use crate::ui_kit::{
    Hot, Pill, PillText, Tone, UiBlock, UiClick, WHEEL_LINE_PX, glow, label, number, prose, ring_node, set_color, set_text, spawn_brackets, spawn_pill,
    spawn_scroll_cue, tracked, wheel_rows,
};

use super::tree::{self, CardState, Metrics, Tree, family_label, kind_words};
use super::{ResearchAction, ResearchTree, ResearchView};

/// The layout's size (logical px), like the other screens'.
const STAGE: Vec2 = Vec2::new(1280.0, 720.0);
/// The lanes' heads start here; the first column at [`COLUMNS_X`]; the tree ends at [`TREE_RIGHT`].
const TREE_LEFT: f32 = 24.0;
const COLUMNS_X: f32 = 168.0;
const TREE_RIGHT: f32 = 878.0;
/// The tier heads, then the tree's area (the lanes scroll inside it).
const TIER_Y: f32 = 84.0;
const TREE_TOP: f32 = 104.0;
const TREE_BOTTOM: f32 = 664.0;
/// A card: its height, its widest, the least gap between columns; a lane row's pitch (least, most); the card's
/// place in its row.
const CARD_H: f32 = 40.0;
const CARD_MAX_W: f32 = 210.0;
const COLUMN_GAP: f32 = 40.0;
const ROW_PITCH: (f32, f32) = (66.0, 96.0);
const CARD_TOP: f32 = 8.0;
/// The detail panel on the right.
const PANEL_X: f32 = 912.0;
const PANEL_W: f32 = 344.0;
/// UNLOCK's top.
const UNLOCK_Y: f32 = 614.0;
/// Stat cells and bonus lines of the preview; cost chips of the detail.
const STATS: usize = 8;
const BONUSES: usize = 2;
const COST_CHIPS: usize = 4;
/// Connectors: line width, and a dashed one's dash and gap.
const LINE: f32 = 1.4;
const DASH: (f32, f32) = (4.0, 4.0);

/// The research screen's UI root (shown only on the research screen).
#[derive(Component)]
pub struct ResearchRoot;

/// A widget that sends an action when clicked.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ResearchButton(pub ResearchAction);

/// The lanes' scrolled area.
#[derive(Component)]
pub struct TreeArea;

/// Texts the screen writes.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RText {
    LaneAmount(usize),
    AlsoInStore,
    Material(usize),
    CardName(usize),
    CardDone(usize),
    CardTag(usize),
    Chip(usize, usize),
    Kind,
    Name,
    Summary,
    PreviewHead,
    StatLabel(usize),
    StatValue(usize),
    Bonus(usize),
    CostChip(usize),
    InStore,
    Gives,
    Requires,
    Say,
    Status,
}

/// Nodes the screen shows, hides or colours.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RNode {
    Card(usize),
    Chip(usize, usize),
    LinkSolid(usize),
    LinkDash(usize),
    Preview,
    Stat(usize),
    Bonus(usize),
    CostChip(usize),
    CostDiamond(usize),
    RequiresRow,
    UnlockMatter,
}

/// Icons the screen sets.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RIcon {
    Card(usize),
    Mark(usize),
    Preview,
}

/// The UNLOCK button.
#[derive(Component)]
pub(super) struct UnlockButton;

/// UNLOCK's texts: the words with the research data, the materials after the diamond.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnlockText {
    Words,
    Matter,
}

/// What each card shows now (from the career), for the card styles.
#[derive(Component, Default)]
pub struct CardStates(Vec<CardState>);

/// Where the tree's cards go for this content.
pub fn metrics(tree: &Tree) -> Metrics {
    let columns = tree.columns.max(1) as f32;
    let pitch = ((TREE_RIGHT - COLUMNS_X + COLUMN_GAP) / columns).min(CARD_MAX_W + COLUMN_GAP + 10.0);
    let rows = tree.rows().max(1) as f32;
    Metrics {
        origin: Vec2::new(COLUMNS_X, 0.0),
        inset: CARD_TOP,
        card: Vec2::new((pitch - COLUMN_GAP).min(CARD_MAX_W), CARD_H),
        column_pitch: pitch,
        row_pitch: ((TREE_BOTTOM - TREE_TOP) / rows).clamp(ROW_PITCH.0, ROW_PITCH.1),
    }
}

/// Roman numerals for the tier heads (1 to 39 is plenty).
fn roman(n: usize) -> String {
    let mut out = String::new();
    let mut n = n;
    for (v, s) in [(10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")] {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}

/// A node's icon: the part it unlocks (hull parts in the first armour), the armour block in the material it
/// unlocks, the formation for a frame, the mind's ring for mind capacity.
fn node_icon(content: &Content, style: &Style, node: &ResearchNode) -> UiIcon {
    let first_armor = || content.materials.iter().find(|m| m.armor.is_some()).map(|m| m.id.clone());
    match node.unlocks.first() {
        Some(Unlock::Part(p)) => {
            let hull = content.parts.get(p).is_some_and(|p| p.hull);
            UiIcon::Part { part: p.clone(), material: if hull { first_armor() } else { None } }
        }
        Some(Unlock::Material(m)) => match content.parts.iter().find(|p| p.hull) {
            Some(block) => UiIcon::Part { part: block.id.clone(), material: Some(m.clone()) },
            None => UiIcon::Glyph { glyph: Glyph::Diamond, color: style.ui.label },
        },
        Some(Unlock::Frame(_)) => UiIcon::Glyph { glyph: Glyph::Formation, color: style.ui.teal },
        Some(Unlock::MindCapacity(_)) | None => UiIcon::Glyph { glyph: Glyph::Menu, color: style.ui.violet },
    }
}

/// A family's colour (its system colour; the label grey without one, like the hull category).
fn family_color(content: &Content, style: &Style, family: &str) -> Color {
    content.families.get(family).and_then(|f| f.color.as_deref()).map_or(style.ui.label, |k| style.system_color(k))
}

/// The materials any node costs, in content order (shown on top).
fn cost_materials(content: &Content) -> Vec<String> {
    content.materials.iter().filter(|m| content.research.iter().any(|n| n.cost.materials.contains_key(&m.id))).map(|m| m.id.clone()).collect()
}

fn abs(at: Vec2, size: Option<Vec2>) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(at.x),
        top: px(at.y),
        width: size.map_or(Val::Auto, |s| px(s.x)),
        height: size.map_or(Val::Auto, |s| px(s.y)),
        ..default()
    }
}

/// A small outlined chip with a text (and a diamond before it for a material).
fn spawn_chip(parent: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, marker: impl Bundle, text: RText, diamond: Option<impl Bundle>) {
    let c = ui_color(style.ui.label, 0.4);
    parent
        .spawn((
            marker,
            Node {
                height: px(17),
                padding: UiRect::horizontal(px(7)),
                column_gap: px(4),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::MAX,
                flex_shrink: 0.0,
                ..default()
            },
            BorderColor::all(c),
        ))
        .with_children(|chip| {
            if let Some(d) = diamond {
                chip.spawn((d, UiIcon::Glyph { glyph: Glyph::Diamond, color: style.ui.gold }, Node { width: px(7), height: px(7), flex_shrink: 0.0, ..default() }));
            }
            chip.spawn((text, tracked(fonts, "", 7.5, ui_color(style.ui.label, 1.0), 0.2)));
        });
}

/// Spawns the screen (hidden), from the content's tree.
pub fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, content: Res<GameContent>, tree: Res<ResearchTree>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let (fonts, style, content, tree) = (&*fonts, &*style, &content.0, &tree.0);
    let m = metrics(tree);
    let placed = tree.place(&m);
    let inner_h = placed.lanes.last().map_or(0.0, |l| l.0 + l.1);
    commands
        .spawn((ResearchRoot, CardStates::default(), Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() }, Visibility::Hidden))
        .with_children(|root| {
            root.spawn(Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() })
                .with_children(|centre| {
                    centre.spawn(Node { width: px(STAGE.x), height: px(STAGE.y), flex_shrink: 0.0, ..default() }).with_children(|stage| {
                        // Four roots (the top bar, the tree, the detail, the bottom bar), so each can slide in on its own.
                        stage.spawn((UiBlock, abs(Vec2::ZERO, Some(Vec2::new(STAGE.x, 70.0))))).with_children(|top| {
                            // Left: the way back, where we are.
                            top.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(14), ..abs(Vec2::new(22.0, 14.0), None) }).with_children(|tl| {
                                tl.spawn((ring_node(40.0), ResearchButton(ResearchAction::Home))).with_child((UiIcon::Glyph { glyph: Glyph::Menu, color: ui.violet }, Node { width: px(22), height: px(22), ..default() }));
                                tl.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                                    col.spawn(label(fonts, "SHIPYARD  ›  RESEARCH", 10.0, c(Tone::Label, 1.0)));
                                    col.spawn(prose(fonts, "Research", 22.0, c(Tone::Number, 1.0)));
                                });
                            });
                            // Right: data of families without a lane, the materials research costs.
                            top.spawn(Node { position_type: PositionType::Absolute, right: px(24), top: px(24), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() })
                                .with_children(|tr| {
                                    tr.spawn((RText::AlsoInStore, label(fonts, "", 8.5, c(Tone::Dim, 1.0)), Node { margin: UiRect::right(px(16)), ..default() }));
                                    for (k, mat) in cost_materials(content).iter().enumerate() {
                                        let name = content.materials.get(mat).map_or(mat.to_uppercase(), |m| m.name.to_uppercase());
                                        tr.spawn((UiIcon::Glyph { glyph: Glyph::Diamond, color: ui.gold }, Node { width: px(9), height: px(9), ..default() }));
                                        tr.spawn(label(fonts, &name, 9.0, c(Tone::Label, 1.0)));
                                        tr.spawn((RText::Material(k), number(fonts, "", 18.0, c(Tone::Gold, 1.0)), Node { margin: UiRect::right(px(6)), ..default() }));
                                    }
                                });
                        });

                        // The tree: tier heads over the columns, the lanes (scrolled when they don't fit).
                        stage.spawn(abs(Vec2::new(0.0, TIER_Y), Some(Vec2::new(TREE_RIGHT + 8.0, TREE_BOTTOM - TIER_Y)))).with_children(|left| {
                            for col in 0..tree.columns {
                                left.spawn((label(fonts, &format!("TIER {}", roman(col + 1)), 8.5, c(Tone::Dim, 1.0)), abs(Vec2::new(COLUMNS_X + col as f32 * m.column_pitch + 14.0, 0.0), None)));
                            }
                            let area = left
                                .spawn((
                                    TreeArea,
                                    ScrollPosition::default(),
                                    Node { overflow: Overflow::scroll_y(), ..abs(Vec2::new(0.0, TREE_TOP - TIER_Y), Some(Vec2::new(TREE_RIGHT + 8.0, TREE_BOTTOM - TREE_TOP))) },
                                ))
                                .with_children(|area| {
                                    area.spawn(Node { width: percent(100), height: px(inner_h), flex_shrink: 0.0, ..default() }).with_children(|lanes| {
                                        spawn_lanes(lanes, fonts, style, content, tree, &m, &placed);
                                    });
                                })
                                .id();
                            spawn_scroll_cue(left, fonts, style, area);
                        });

                        spawn_detail(stage, fonts, style);

                        // Bottom: the way home, the line, the drydock.
                        stage.spawn((UiBlock, abs(Vec2::new(0.0, 674.0), Some(Vec2::new(STAGE.x, 32.0))))).with_children(|bottom| {
                            bottom.spawn(abs(Vec2::new(22.0, 0.0), None)).with_children(|b| {
                                spawn_pill(b, fonts, "‹  HOME", 9.5, 32.0, None, Pill::new(Tone::Teal), ResearchButton(ResearchAction::Home));
                            });
                            bottom.spawn((RText::Status, label(fonts, "", 9.0, c(Tone::Teal, 1.0)), TextLayout::no_wrap(), abs(Vec2::new(150.0, 11.0), None)));
                            bottom.spawn(Node { position_type: PositionType::Absolute, right: px(24), top: px(0), ..default() }).with_children(|b| {
                                spawn_pill(b, fonts, "DRYDOCK", 9.5, 32.0, None, Pill::new(Tone::Teal), ResearchButton(ResearchAction::Drydock));
                            });
                        });
                    });
                });
        });
}

/// The lanes' heads, separators and dots, the connectors, the cards and the lines under them.
fn spawn_lanes(lanes: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, content: &Content, tree: &Tree, m: &Metrics, placed: &tree::Placed) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    for (k, lane) in tree.lanes.iter().enumerate() {
        let (top, h) = placed.lanes[k];
        let colour = family_color(content, style, &lane.family);
        let mid = top + m.inset + m.card.y / 2.0;
        lanes.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), ..abs(Vec2::new(TREE_LEFT, top + m.inset), None) }).with_children(|head| {
            head.spawn(label(fonts, &family_label(&lane.family), 9.0, c(Tone::Label, 1.0)));
            head.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Baseline, column_gap: px(7), ..default() }).with_children(|row| {
                row.spawn((RText::LaneAmount(k), number(fonts, "", 21.0, c(Tone::Number, 1.0))));
                row.spawn(label(fonts, "DATA", 7.5, c(Tone::Dim, 1.0)));
            });
        });
        // The family's dot, joined to the lane's first card.
        lanes.spawn((Node { border_radius: BorderRadius::MAX, ..abs(Vec2::new(COLUMNS_X - 22.0, mid - 3.5), Some(Vec2::splat(7.0))) }, BackgroundColor(ui_color(colour, 0.95))));
        lanes.spawn((abs(Vec2::new(COLUMNS_X - 15.0, mid - LINE / 2.0), Some(Vec2::new(15.0, LINE))), BackgroundColor(ui_color(colour, 0.45))));
        if k + 1 < tree.lanes.len() {
            lanes.spawn((abs(Vec2::new(TREE_LEFT, top + h - 1.0), Some(Vec2::new(TREE_RIGHT - TREE_LEFT, 1.0))), BackgroundColor(c(Tone::Label, 0.07))));
        }
    }
    // Connectors under the cards: each run a node holding a solid line and its dashes (one of them shows).
    for (k, points) in placed.links.iter().enumerate() {
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let across = (a.y - b.y).abs() < 0.5;
            let len = if across { (b.x - a.x).abs() } else { (b.y - a.y).abs() };
            let at = Vec2::new(a.x.min(b.x), a.y.min(b.y)) - Vec2::splat(LINE / 2.0);
            let size = if across { Vec2::new(len + LINE, LINE) } else { Vec2::new(LINE, len + LINE) };
            lanes.spawn(abs(at, Some(size))).with_children(|run| {
                run.spawn((RNode::LinkSolid(k), Node { width: percent(100), height: percent(100), ..default() }, BackgroundColor(c(Tone::Teal, 0.6))));
                let mut t = 0.0;
                while t < len {
                    let d = DASH.0.min(len - t);
                    let (pos, dims) = if across { (Vec2::new(t, 0.0), Vec2::new(d, LINE)) } else { (Vec2::new(0.0, t), Vec2::new(LINE, d)) };
                    run.spawn((RNode::LinkDash(k), abs(pos, Some(dims)), BackgroundColor(c(Tone::Label, 0.4))));
                    t += DASH.0 + DASH.1;
                }
            });
        }
    }
    for (k, card) in tree.cards.iter().enumerate() {
        let Some(node) = content.research.get(&card.node) else { continue };
        let r = placed.cards[k];
        lanes
            .spawn((
                RNode::Card(k),
                ResearchButton(ResearchAction::Select(node.id.clone())),
                Hot::default(),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    padding: UiRect::horizontal(px(14)),
                    border: UiRect::all(px(1.2)),
                    border_radius: BorderRadius::MAX,
                    ..abs(r.min, Some(r.size()))
                },
                BorderColor::all(c(Tone::Label, 0.35)),
                BackgroundColor(Color::NONE),
                glow(ui.teal, 0.0),
            ))
            .with_children(|b| {
                b.spawn((RIcon::Card(k), node_icon(content, style, node), Node { width: px(20), height: px(20), flex_shrink: 0.0, ..default() }));
                b.spawn((RText::CardName(k), prose(fonts, &node.name, 12.5, c(Tone::Number, 1.0)), Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), ..default() }));
                b.spawn((RText::CardDone(k), label(fonts, "DONE", 7.5, c(Tone::Teal, 1.0)), Node { display: Display::None, flex_shrink: 0.0, ..default() }));
                b.spawn((RIcon::Mark(k), UiIcon::Glyph { glyph: Glyph::Diamond, color: ui.gold }, Node { display: Display::None, width: px(9), height: px(9), flex_shrink: 0.0, ..default() }));
            });
        // Under the card: its kind and its cost.
        lanes
            .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), ..abs(Vec2::new(r.min.x + 16.0, r.max.y + 6.0), None) })
            .with_children(|under| {
                under.spawn((RText::CardTag(k), tracked(fonts, kind_words(node.kind), 7.5, c(Tone::Dim, 1.0), 0.25), Node { margin: UiRect::right(px(4)), ..default() }));
                let datas = node.cost.data.len();
                for j in 0..datas + node.cost.materials.len() {
                    let diamond = (j >= datas).then_some(());
                    spawn_chip(under, fonts, style, RNode::Chip(k, j), RText::Chip(k, j), diamond);
                }
            });
    }
}

/// The selected node's detail on the right, the line over UNLOCK, and UNLOCK.
fn spawn_detail(stage: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    stage.spawn((UiBlock, abs(Vec2::new(PANEL_X, TIER_Y), Some(Vec2::new(PANEL_W, UNLOCK_Y + 44.0 - TIER_Y))))).with_children(|panel| {
        // The divider between the tree and the detail.
        panel.spawn((abs(Vec2::new(-20.0, 8.0), Some(Vec2::new(1.0, TREE_BOTTOM - TIER_Y - 8.0))), BackgroundColor(c(Tone::Label, 0.12))));
        panel
            .spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(8), ..abs(Vec2::ZERO, Some(Vec2::new(PANEL_W, 500.0))) })
            .with_children(|p| {
                p.spawn((RText::Kind, label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                p.spawn((RText::Name, prose(fonts, "", 24.0, c(Tone::Number, 1.0))));
                p.spawn((RText::Summary, prose(fonts, "", 12.0, c(Tone::Label, 1.0)), Node { margin: UiRect::bottom(px(6)), ..default() }));
                // The preview: the icon in brackets, the stat cells beside it, the bonuses under them.
                p.spawn((RNode::Preview, Node { flex_direction: FlexDirection::Column, row_gap: px(8), ..default() })).with_children(|pv| {
                    pv.spawn((RText::PreviewHead, label(fonts, "", 7.5, c(Tone::Dim, 1.0))));
                    pv.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(18), ..default() }).with_children(|row| {
                        row.spawn(Node { width: px(84), height: px(110), flex_shrink: 0.0, ..default() }).with_children(|b| {
                            spawn_brackets(b, 10.0, 1.2, c(Tone::Label, 0.45));
                            b.spawn((RIcon::Preview, UiIcon::Glyph { glyph: Glyph::Menu, color: ui.dim }, Node { position_type: PositionType::Absolute, left: px(12), top: px(12), width: px(60), height: px(86), ..default() }));
                        });
                        row.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, row_gap: px(9), width: px(PANEL_W - 102.0), align_content: AlignContent::FlexStart, ..default() })
                            .with_children(|grid| {
                                for k in 0..STATS {
                                    grid.spawn((RNode::Stat(k), Node { flex_direction: FlexDirection::Column, row_gap: px(3), width: px(118), ..default() })).with_children(|cell| {
                                        cell.spawn((RText::StatLabel(k), tracked(fonts, "", 7.0, c(Tone::Dim, 1.0), 0.25)));
                                        cell.spawn((RText::StatValue(k), number(fonts, "", 14.0, c(Tone::Number, 1.0))));
                                    });
                                }
                            });
                    });
                    for k in 0..BONUSES {
                        pv.spawn((RNode::Bonus(k), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8), ..default() })).with_children(|l| {
                            l.spawn((UiIcon::Glyph { glyph: Glyph::Diamond, color: ui.gold }, Node { width: px(8), height: px(8), flex_shrink: 0.0, ..default() }));
                            l.spawn((RText::Bonus(k), tracked(fonts, "", 7.5, c(Tone::Gold, 1.0), 0.25)));
                        });
                    }
                });
                p.spawn((Node { height: px(1), margin: UiRect::vertical(px(8)), flex_shrink: 0.0, ..default() }, BackgroundColor(c(Tone::Label, 0.12))));
                let row = |gap: f32| Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), ..default() };
                let head = |text: &str| (label(fonts, text, 8.0, c(Tone::Dim, 1.0)), Node { width: px(80), flex_shrink: 0.0, ..default() });
                p.spawn(row(6.0)).with_children(|r| {
                    r.spawn(head("COST"));
                    for k in 0..COST_CHIPS {
                        spawn_chip(r, fonts, style, RNode::CostChip(k), RText::CostChip(k), Some(RNode::CostDiamond(k)));
                    }
                    r.spawn((RText::InStore, tracked(fonts, "", 7.5, c(Tone::Dim, 1.0), 0.25), Node { margin: UiRect::left(px(6)), ..default() }));
                });
                p.spawn(Node { align_items: AlignItems::FlexStart, ..row(0.0) }).with_children(|r| {
                    r.spawn(head("UNLOCKS"));
                    r.spawn((RText::Gives, prose(fonts, "", 12.0, c(Tone::Number, 0.95)), Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0), ..default() }));
                });
                p.spawn((RNode::RequiresRow, Node { align_items: AlignItems::FlexStart, ..row(0.0) })).with_children(|r| {
                    r.spawn(head("REQUIRES"));
                    r.spawn((RText::Requires, prose(fonts, "", 12.0, c(Tone::Label, 1.0)), Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0), ..default() }));
                });
            });
        panel.spawn((RText::Say, label(fonts, "", 8.5, c(Tone::Teal, 1.0)), abs(Vec2::new(0.0, UNLOCK_Y - 22.0 - TIER_Y), Some(Vec2::new(PANEL_W, 14.0)))));
        // UNLOCK: a primary pill (the kit styles it), its cost in words and the materials after a diamond.
        panel.spawn(Node { flex_direction: FlexDirection::Column, ..abs(Vec2::new(0.0, UNLOCK_Y - TIER_Y), Some(Vec2::new(PANEL_W, 44.0))) }).with_children(|b| {
            b.spawn((
                Pill { primary: true, ..Pill::new(Tone::Teal) },
                Hot::default(),
                ResearchButton(ResearchAction::Unlock),
                UnlockButton,
                Node {
                    height: px(44),
                    border: UiRect::all(px(1.2)),
                    border_radius: BorderRadius::MAX,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    ..default()
                },
                BorderColor::all(Color::NONE),
                BackgroundColor(Color::NONE),
            ))
            .with_children(|p| {
                let pill_text = |text: &str| {
                    (PillText, Text::new(text), TextFont { font: fonts.medium.clone().into(), font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::NONE), LetterSpacing::Px(3.0))
                };
                p.spawn((UnlockText::Words, pill_text("UNLOCK")));
                p.spawn((RNode::UnlockMatter, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), ..default() })).with_children(|m| {
                    m.spawn((UiIcon::Glyph { glyph: Glyph::Diamond, color: ui.gold }, Node { width: px(9), height: px(9), ..default() }));
                    m.spawn((UnlockText::Matter, pill_text("")));
                });
            });
        });
    });
}

/// The screen shows only on the research screen.
pub fn show(mode: Res<State<AppMode>>, mut root: Single<&mut Visibility, With<ResearchRoot>>) {
    let want = if *mode.get() == AppMode::Research { Visibility::Inherited } else { Visibility::Hidden };
    if **root != want {
        **root = want;
    }
}

/// Clicks on the screen's buttons (a disabled pill sends nothing).
pub fn clicks(mut clicks: MessageReader<UiClick>, buttons: Query<(&ResearchButton, Option<&Pill>)>, mut out: MessageWriter<ResearchAction>) {
    for c in clicks.read() {
        if let Ok((b, pill)) = buttons.get(c.0)
            && !pill.is_some_and(|p| p.disabled)
        {
            out.write(b.0.clone());
        }
    }
}

/// The wheel over the lanes scrolls them (when the content has more lanes than fit).
pub fn scroll(window: Single<&Window, With<PrimaryWindow>>, wheel: Res<AccumulatedMouseScroll>, area: Single<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition), With<TreeArea>>) {
    let (node, tf, mut pos) = area.into_inner();
    if wheel.delta.y == 0.0 || !window.physical_cursor_position().is_some_and(|c| node.contains_point(*tf, c)) {
        return;
    }
    let max = ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0);
    let y = (pos.0.y - wheel_rows(&wheel) * WHEEL_LINE_PX).clamp(0.0, max);
    if pos.0.y != y {
        pos.0.y = y;
    }
}

/// One stat cell of the preview.
#[derive(Clone, Debug, PartialEq)]
pub struct Stat {
    pub label: String,
    pub value: String,
}

/// What the preview shows for a node: a head, the icon, stat cells and bonus lines. A part: its stat lines from
/// content (as the drydock's palette part); a material: the armour block in it; a frame: its size; mind
/// capacity: the mind before and after.
pub fn preview(content: &Content, style: &Style, node: &ResearchNode, capacity: u32) -> (String, UiIcon, Vec<Stat>, Vec<String>) {
    let icon = node_icon(content, style, node);
    let lines = |part: &data::Part, material: Option<&data::Material>| {
        let all = content.part_stats(part, material);
        let stats = all.iter().filter(|l| l.bonus.is_none()).map(|l| Stat { label: l.label.to_uppercase(), value: l.text.clone() }).collect();
        let bonuses = all
            .iter()
            .filter_map(|l| {
                let b = l.bonus.as_ref()?;
                Some(format!("{} {} to adjacent {}", l.text, l.label, b.target_label).to_uppercase())
            })
            .collect();
        (stats, bonuses)
    };
    match node.unlocks.first() {
        Some(Unlock::Part(p)) => match content.parts.get(p) {
            Some(part) => {
                let armor = content.materials.iter().find(|m| m.armor.is_some()).filter(|_| part.hull);
                let (stats, bonuses) = lines(part, armor);
                let cat = content.categories.get(&part.category).map_or(String::new(), |c| c.name.to_uppercase());
                (format!("{}  ·  {}×{}  ·  {cat}", part.name.to_uppercase(), part.size.0, part.size.1), icon, stats, bonuses)
            }
            None => (p.to_uppercase(), icon, Vec::new(), Vec::new()),
        },
        Some(Unlock::Material(m)) => {
            let mat = content.materials.get(m);
            match (content.parts.iter().find(|p| p.hull), mat) {
                (Some(block), Some(mat)) => {
                    let (stats, bonuses) = lines(block, Some(mat));
                    (format!("{} IN {}", block.name.to_uppercase(), mat.name.to_uppercase()), icon, stats, bonuses)
                }
                _ => (m.to_uppercase(), icon, Vec::new(), Vec::new()),
            }
        }
        Some(Unlock::Frame(f)) => {
            let frame = content.frames.get(f);
            let stats = frame.map_or(Vec::new(), |fr| vec![Stat { label: "GRID".into(), value: format!("{} × {}", fr.size.0, fr.size.1) }]);
            let head = frame.map_or(f.to_uppercase(), |fr| format!("{} FRAME", fr.name.to_uppercase()));
            (head, icon, stats, Vec::new())
        }
        Some(Unlock::MindCapacity(n)) => {
            let stats = vec![Stat { label: "MIND CAPACITY".into(), value: format!("{capacity} → {} cores", capacity + n) }];
            ("THE SHIP-MIND".into(), icon, stats, Vec::new())
        }
        None => (String::new(), icon, Vec::new(), Vec::new()),
    }
}

/// The screen follows the career and the selection: the stores, every card's state and cost, the connectors,
/// the detail, the line over UNLOCK, UNLOCK and the line at the bottom.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync(
    mode: Res<State<AppMode>>,
    view: Res<ResearchView>,
    career: Res<PlayerCareer>,
    granted: Res<Granted>,
    content: Res<GameContent>,
    tree: Res<ResearchTree>,
    style: Res<Style>,
    mut states: Single<&mut CardStates>,
    mut texts: Query<(&RText, &mut Text, &mut TextColor)>,
    mut nodes: Query<(&RNode, &mut Node, Option<&mut BorderColor>, Option<&mut BackgroundColor>)>,
    mut icons: Query<(&RIcon, &mut UiIcon)>,
    mut unlock: Single<&mut Pill, With<UnlockButton>>,
    mut unlock_texts: Query<(&UnlockText, &mut Text), Without<RText>>,
) {
    if !(mode.is_changed() || view.is_changed() || career.is_changed() || granted.is_changed() || states.0.is_empty()) {
        return;
    }
    let (content, tree, ui) = (&content.0, &tree.0, &style.ui);
    let c = career.career();
    let now: Vec<CardState> = tree.cards.iter().map(|card| tree::state(content, c, &card.node)).collect();
    if states.0 != now {
        states.0 = now;
    }
    let states = &states.0;
    let node_of = |k: usize| content.research.get(&tree.cards[k].node);
    let selected = view.selected.as_deref().and_then(|s| content.research.get(s));
    let sel_state = selected.map(|n| tree::state(content, c, &n.id));
    let chips_of: Vec<Vec<tree::Chip>> = (0..tree.cards.len()).map(|k| node_of(k).map_or(Vec::new(), |n| tree::chips(c, n))).collect();
    // A researched node shows what it cost, not what the stores hold now.
    let done = matches!(sel_state, Some(CardState::Done));
    let sel_chips = selected.map_or(Vec::new(), |n| if done { tree::cost_chips(n) } else { tree::chips(c, n) });
    let (head, icon, stats, bonuses) = selected.map_or((String::new(), UiIcon::Glyph { glyph: Glyph::Menu, color: ui.dim }, Vec::new(), Vec::new()), |n| preview(content, &style, n, granted.mind_capacity));
    let ready = tree::ready(content, c);
    let lane_families: Vec<&str> = tree.lanes.iter().map(|l| l.family.as_str()).collect();
    let mats = cost_materials(content);

    for (t, mut text, mut color) in &mut texts {
        let (s, tone) = match *t {
            RText::LaneAmount(k) => (tree::amount(c.data().get(&tree.lanes[k].family).copied().unwrap_or(0.0)), Tone::Number),
            RText::AlsoInStore => {
                let others: Vec<String> = c
                    .data()
                    .iter()
                    .filter(|(f, x)| !lane_families.contains(&f.as_str()) && **x > 0.0)
                    .map(|(f, x)| format!("{} {}", family_label(f), tree::amount(*x)))
                    .collect();
                (if others.is_empty() { String::new() } else { format!("ALSO IN STORE  ·  {}", others.join("  ·  ")) }, Tone::Dim)
            }
            RText::Material(k) => (mats.get(k).and_then(|m| c.materials().get(m)).copied().unwrap_or(0).to_string(), Tone::Gold),
            RText::CardName(k) => (node_of(k).map_or(String::new(), |n| n.name.clone()), if matches!(states[k], CardState::Locked(_)) { Tone::Dim } else { Tone::Number }),
            RText::CardDone(_) => ("DONE".into(), Tone::Teal),
            RText::CardTag(k) => {
                let kind = node_of(k).map_or("", |n| kind_words(n.kind));
                match states[k] {
                    CardState::Done => (format!("{kind}  ·  DONE"), Tone::Teal),
                    _ => (kind.to_string(), Tone::Dim),
                }
            }
            RText::Chip(k, j) => {
                let chip = chips_of[k].get(j);
                let tone = match (&states[k], chip) {
                    (CardState::Locked(_), _) => Tone::Dim,
                    (_, Some(ch)) if ch.payable => Tone::Gold,
                    _ => Tone::Label,
                };
                (chip.map_or(String::new(), |ch| ch.text.clone()), tone)
            }
            RText::Kind => (selected.map_or(String::new(), |n| format!("{}  ·  {}", kind_words(n.kind), content.families.get(&n.family).map_or(family_label(&n.family), |f| f.name.to_uppercase()))), Tone::Dim),
            RText::Name => (selected.map_or("Nothing to research".into(), |n| n.name.clone()), Tone::Number),
            RText::Summary => (selected.map_or(String::new(), |n| n.summary.clone()), Tone::Label),
            RText::PreviewHead => (head.clone(), Tone::Dim),
            RText::StatLabel(k) => (stats.get(k).map_or(String::new(), |s| s.label.clone()), Tone::Dim),
            RText::StatValue(k) => (stats.get(k).map_or(String::new(), |s| s.value.clone()), Tone::Number),
            RText::Bonus(k) => (bonuses.get(k).cloned().unwrap_or_default(), Tone::Gold),
            RText::CostChip(k) => {
                let chip = sel_chips.get(k);
                let tone = if chip.is_some_and(|ch| ch.payable) && !matches!(sel_state, Some(CardState::Done)) { Tone::Gold } else { Tone::Label };
                (chip.map_or(String::new(), |ch| ch.text.clone()), tone)
            }
            RText::InStore => (selected.filter(|_| !done).map_or(String::new(), |n| in_store(content, c, n)), Tone::Dim),
            RText::Gives => (selected.map_or(String::new(), |n| tree::gives(content, n, granted.mind_capacity)), Tone::Number),
            RText::Requires => {
                let words = selected.map_or(Vec::new(), |n| {
                    n.requires
                        .iter()
                        .map(|r| {
                            let name = content.research.get(r).map_or(r.clone(), |x| x.name.clone());
                            format!("{name}{}", if c.is_researched(r) { "  ·  done" } else { "  ·  not yet" })
                        })
                        .collect()
                });
                let met = selected.is_some_and(|n| n.requires.iter().all(|r| c.is_researched(r)));
                (words.join("\n"), if met { Tone::Teal } else { Tone::Label })
            }
            RText::Say => match (&view.say, &sel_state, selected) {
                (Some((s, tone)), ..) => (s.clone(), *tone),
                (None, Some(CardState::Done), _) => ("RESEARCHED  ·  IN USE".into(), Tone::Teal),
                (None, Some(CardState::Ready), _) => ("READY  ·  UNLOCK PAYS AND UNLOCKS AT ONCE".into(), Tone::Gold),
                (None, Some(CardState::Locked(missing)), _) => (tree::refusal_words(content, &data::career::Refusal::Requires(missing.clone())), Tone::Label),
                (None, Some(CardState::Short(short)), _) => (tree::refusal_words(content, &data::career::Refusal::CannotPay(short.clone())), Tone::Label),
                _ => (String::new(), Tone::Dim),
            },
            RText::Status => {
                if ready.is_empty() {
                    ("NOTHING READY  ·  SALVAGE WRECKS FOR RESEARCH DATA".into(), Tone::Dim)
                } else {
                    let names: Vec<String> = ready.iter().map(|n| n.name.to_uppercase()).collect();
                    (format!("{} READY  ·  {}", ready.len(), names.join("  ·  ")), Tone::Teal)
                }
            }
        };
        set_text(&mut text, &s);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
    }

    for (n, mut node, border, bg) in &mut nodes {
        let (show, edge, fill) = match *n {
            RNode::Card(_) => continue,
            RNode::Chip(k, j) => {
                let chip = chips_of[k].get(j);
                let edge = match (&states[k], chip) {
                    (CardState::Locked(_), _) => ui_color(ui.dim, 0.3),
                    (_, Some(ch)) if ch.payable => ui_color(ui.gold, 0.8),
                    _ => ui_color(ui.label, 0.35),
                };
                (chip.is_some() && states[k] != CardState::Done, Some(edge), None)
            }
            RNode::LinkSolid(k) => (link_done(tree, states, k), None, Some(ui_color(ui.teal, 0.6))),
            RNode::LinkDash(k) => (!link_done(tree, states, k), None, Some(ui_color(ui.label, 0.4))),
            RNode::Preview => (selected.is_some(), None, None),
            RNode::Stat(k) => (k < stats.len(), None, None),
            RNode::Bonus(k) => (k < bonuses.len(), None, None),
            RNode::CostChip(k) => {
                let chip = sel_chips.get(k);
                let payable = chip.is_some_and(|ch| ch.payable) && !matches!(sel_state, Some(CardState::Done));
                (chip.is_some(), Some(if payable { ui_color(ui.gold, 0.8) } else { ui_color(ui.label, 0.35) }), None)
            }
            RNode::CostDiamond(k) => (sel_chips.get(k).is_some_and(|ch| ch.material), None, None),
            RNode::RequiresRow => (selected.is_some_and(|n| !n.requires.is_empty()), None, None),
            RNode::UnlockMatter => (selected.is_some_and(|n| !n.cost.materials.is_empty()) && sel_state != Some(CardState::Done), None, None),
        };
        let d = if show { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
        if let (Some(e), Some(mut b)) = (edge, border)
            && b.top != e
        {
            *b = BorderColor::all(e);
        }
        if let (Some(f), Some(mut b)) = (fill, bg)
            && b.0 != f
        {
            b.0 = f;
        }
    }

    for (i, mut ic) in &mut icons {
        let want = match *i {
            RIcon::Card(k) => match (&states[k], node_of(k)) {
                (CardState::Locked(_), _) => UiIcon::Glyph { glyph: Glyph::Lock, color: ui.dim },
                (_, Some(n)) => node_icon(content, &style, n),
                (_, None) => continue,
            },
            RIcon::Mark(_) => UiIcon::Glyph { glyph: Glyph::Diamond, color: ui.gold },
            RIcon::Preview => icon.clone(),
        };
        if *ic != want {
            *ic = want;
        }
    }

    // UNLOCK: enabled while the selected node can be researched; it names the cost.
    let disabled = sel_state != Some(CardState::Ready);
    if unlock.disabled != disabled {
        unlock.disabled = disabled;
    }
    let (words, matter) = match (&sel_state, selected) {
        (Some(CardState::Done), _) => ("RESEARCHED".to_string(), String::new()),
        (_, Some(n)) => unlock_words(n),
        _ => ("UNLOCK".into(), String::new()),
    };
    for (t, mut text) in &mut unlock_texts {
        set_text(&mut text, if *t == UnlockText::Words { &words } else { &matter });
    }
}

/// UNLOCK's words and its materials: (`UNLOCK · LOGIC 10`, `2`).
pub fn unlock_words(node: &ResearchNode) -> (String, String) {
    let words = std::iter::once("UNLOCK".to_string())
        .chain(node.cost.data.iter().map(|(f, x)| format!("{} {}", family_label(f), tree::amount(*x))))
        .collect::<Vec<_>>()
        .join("  ·  ");
    let matter = node.cost.materials.values().map(u32::to_string).collect::<Vec<_>>().join(" + ");
    (words, matter)
}

/// What the stores hold of a node's costs: `IN STORE 19`, `IN STORE 10 · 1 PROGRAMMABLE MATTER`.
fn in_store(content: &Content, career: &data::Career, node: &ResearchNode) -> String {
    let mut parts: Vec<String> = node.cost.data.keys().map(|f| tree::amount(career.data().get(f).copied().unwrap_or(0.0))).collect();
    parts.extend(node.cost.materials.keys().map(|m| {
        let name = content.materials.get(m).map_or(m.as_str(), |m| m.name.as_str()).to_uppercase();
        format!("{} {name}", career.materials().get(m).copied().unwrap_or(0))
    }));
    if parts.is_empty() { String::new() } else { format!("IN STORE {}", parts.join("  ·  ")) }
}

/// A connector is drawn solid once its requirement is researched.
fn link_done(tree: &Tree, states: &[CardState], link: usize) -> bool {
    tree.links.get(link).is_some_and(|l| states.get(l.from) == Some(&CardState::Done))
}

/// The cards' outlines from their states, the selection and the pointer: ready gold and glowing, researched teal,
/// the rest grey (dim while locked); the selected card lit, a hovered one brighter.
#[allow(clippy::type_complexity)]
pub(super) fn style_cards(
    view: Res<ResearchView>,
    tree: Res<ResearchTree>,
    style: Res<Style>,
    states: Single<Ref<CardStates>>,
    mut cards: Query<(&RNode, Ref<Hot>, &mut BorderColor, &mut BackgroundColor, &mut BoxShadow)>,
) {
    let changed = view.is_changed() || states.is_changed() || cards.iter().any(|(_, h, ..)| h.is_changed());
    if !changed {
        return;
    }
    let ui = &style.ui;
    for (n, hot, mut border, mut bg, mut shadow) in &mut cards {
        let RNode::Card(k) = *n else { continue };
        let Some(state) = states.0.get(k) else { continue };
        let picked = view.selected.as_deref() == Some(tree.0.cards[k].node.as_str());
        let lift = if hot.hovered { 0.25 } else { 0.0 };
        let (accent, edge, glowing): (Color, f32, f32) = match state {
            CardState::Ready => (ui.gold, 0.8, 0.2),
            CardState::Done => (ui.teal, 0.5, 0.0),
            CardState::Short(_) => (ui.label, 0.35, 0.0),
            CardState::Locked(_) => (ui.dim, 0.3, 0.0),
        };
        let (edge, glowing) = if picked { (1.0, glowing.max(0.16)) } else { (edge + lift, glowing) };
        let b = ui_color(accent, edge.min(1.0));
        let f = if picked { ui_color(accent, 0.08) } else if hot.hovered { ui_color(ui.number, 0.04) } else { Color::NONE };
        let s = glow(if picked && *state != CardState::Ready { ui.teal } else { accent }, glowing);
        if border.top != b {
            *border = BorderColor::all(b);
        }
        if bg.0 != f {
            bg.0 = f;
        }
        if *shadow != s {
            *shadow = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The base pack's tree fits the 1280 × 720 layout: three columns of full-width cards end before the detail
    /// panel, six lanes fit without scrolling.
    #[test]
    fn the_tree_fits_the_layout() {
        let content = data::base_content();
        let tree = Tree::new(&content);
        let m = metrics(&tree);
        assert_eq!(m.card.x, CARD_MAX_W);
        let placed = tree.place(&m);
        let right = placed.cards.iter().map(|r| r.max.x).fold(0.0, f32::max);
        assert!(right <= TREE_RIGHT && right < PANEL_X - 20.0, "{right}");
        let bottom = placed.lanes.last().map(|l| l.0 + l.1).unwrap();
        assert!(bottom <= TREE_BOTTOM - TREE_TOP, "{bottom}");
        // More columns: narrower cards, still inside.
        let wide = Tree { columns: 5, ..tree.clone() };
        let m = metrics(&wide);
        assert!(COLUMNS_X + 4.0 * m.column_pitch + m.card.x <= TREE_RIGHT + 0.01);
        assert_eq!(roman(1), "I");
        assert_eq!(roman(14), "XIV");
    }

    /// UNLOCK names the cost: the data in words, the materials after a diamond.
    #[test]
    fn unlock_names_the_cost() {
        let content = data::base_content();
        let words = |n: &str| unlock_words(content.research.get(n).unwrap());
        assert_eq!(words("beam_laser"), ("UNLOCK  ·  ENERGY 10".into(), String::new()));
        assert_eq!(words("mind_expansion_1"), ("UNLOCK  ·  LOGIC 10".into(), "3".into()));
    }
}
