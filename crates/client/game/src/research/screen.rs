//! The research screen's widgets: built once from the content's tree ([`spawn`]), then kept in step with the
//! career and the selection ([`sync`], [`style_cards`]). Laid out for 1280 × 720 logical px (the UI scale grows it
//! with the window, as the drydock's); the lanes scroll (the kit's [`Scrollbar`]) when the content has more than fit.

use bevy::prelude::*;
use data::input::Action;
use world::settings::ActionTip;
use bevy::text::LetterSpacing;
use data::{Content, ResearchNode, Unlock};
use kit::style::{Style, ui_color};
use kit::ui_icons::{Glyph, UiIcon};
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use kit::ui_kit::{
    CompactText, LayoutClass, snap_screen, HudFonts, Edge, Hot, Pill, PillText, ScreenFx, ScreenPart, Scrollbar, Tip, Tone, UiBlock, show_screen, UiClick, glow, label, number, prose, set_color, set_text, spawn_brackets, spawn_pill,
    frame, spawn_head, tracked,
};
use world::career::{Granted, PlayerCareer};
use world::mode::AppMode;
use world::sim_bridge::GameContent;

use super::tree::{self, CardState, Metrics, Tree, family_label, kind_words};
use super::{ResearchAction, ResearchTree, ResearchView};

/// Where the screen's parts go (stage px). [`Lay::WIDE`]: a 1280 × 720 stage (the UI scale grows it with the
/// window), as always. [`Lay::COMPACT`]: a phone's 844 × 390 (scaled by `compact_scale`, docs/client.md "Phones"):
/// each lane's name and data over its cards instead of beside them, so three columns of cards fit beside a narrower
/// detail whose rows scroll over UNLOCK (no preview picture there), the materials in up to two rows right of the
/// header (ALSO IN STORE left out), the type at least `COMPACT_TEXT` px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lay {
    pub compact: bool,
    stage: Vec2,
    /// The lanes' heads start at `tree_left`; the first column at `columns_x`; the tree ends at `tree_right`.
    tree_left: f32,
    columns_x: f32,
    tree_right: f32,
    /// The tier heads, then the tree's area (the lanes scroll inside it).
    tier_y: f32,
    tree_top: f32,
    tree_bottom: f32,
    /// A card's widest, the least gap between columns, the room over a lane's cards for its head (0: beside them).
    card_max_w: f32,
    column_gap: f32,
    lane_head: f32,
    /// The detail panel, UNLOCK's top, the bottom bar's top and the top bar's height.
    panel_x: f32,
    panel_w: f32,
    unlock_y: f32,
    bottom_y: f32,
    top_h: f32,
    /// Text sizes: the detail's name, a card's name, a material's count.
    name: f32,
    card_name: f32,
    material: f32,
}

impl Lay {
    pub const WIDE: Lay = Lay {
        compact: false,
        stage: Vec2::new(1280.0, 720.0),
        tree_left: 24.0,
        columns_x: 168.0,
        tree_right: 878.0,
        tier_y: 84.0,
        tree_top: 104.0,
        tree_bottom: 670.0,
        card_max_w: 210.0,
        column_gap: 40.0,
        lane_head: 0.0,
        panel_x: 912.0,
        panel_w: 344.0,
        unlock_y: 614.0,
        bottom_y: 674.0,
        top_h: 70.0,
        name: 24.0,
        card_name: 12.5,
        material: 18.0,
    };
    pub const COMPACT: Lay = Lay {
        compact: true,
        stage: Vec2::new(844.0, 390.0),
        tree_left: 16.0,
        columns_x: 16.0,
        tree_right: 552.0,
        tier_y: 60.0,
        tree_top: 76.0,
        tree_bottom: 344.0,
        card_max_w: 200.0,
        column_gap: 22.0,
        lane_head: 22.0,
        panel_x: 576.0,
        panel_w: 254.0,
        unlock_y: 332.0,
        bottom_y: 352.0,
        top_h: 58.0,
        name: 18.0,
        card_name: 11.5,
        material: 13.0,
    };

    pub fn of(class: LayoutClass) -> Lay {
        if class.is_compact() { Lay::COMPACT } else { Lay::WIDE }
    }
}

/// The least text size on a phone (`kit::ui_kit::CompactText`).
const COMPACT_TEXT: f32 = 9.0;
/// A card: its height, the least gap between columns; the line under it (its kind and cost chips): its
/// gap below the card and in from the card's left, the chips' height, the gap between its two lines when the chips
/// wrap, and the least gap to the next row's card; a lane row's pitch (least, most); the card's place in its row.
/// The least pitch is what a row lays out, the card and the line under it, a wrapped line one more chip line
/// (19 Oct 2026: it was a fixed 60 px, 3 px short of the chips, so every cost line ran into the card below it, and
/// a long line ran on into the next column or the detail panel; the lanes scroll since 18 Oct, so nothing has to be
/// squeezed in).
const CARD_H: f32 = 40.0;
const UNDER_GAP: f32 = 6.0;
const UNDER_INSET: f32 = 16.0;
const CHIP_H: f32 = 17.0;
const CHIP_ROW_GAP: f32 = 3.0;
const ROW_GAP: f32 = 5.0;
const ROW_PITCH: (f32, f32) = (CARD_H + UNDER_GAP + CHIP_H + ROW_GAP, 96.0);
const CARD_TOP: f32 = 4.0;
/// Stat cells and bonus lines of the preview; cost chips of the detail.
const STATS: usize = 8;
const BONUSES: usize = 2;
const COST_CHIPS: usize = 4;
/// Connectors: line width, and a dashed one's dash and gap.
const LINE: f32 = 1.4;
const DASH: (f32, f32) = (4.0, 4.0);

/// The research screen's UI root (shown only on the research screen), and the layout it was built for.
#[derive(Component)]
pub struct ResearchRoot(Lay);

/// A widget that sends an action when clicked.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ResearchButton(pub ResearchAction);

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

/// The materials' bar on top, and the header it keeps clear of on a phone ([`fit_materials`]).
#[derive(Component)]
pub(super) struct MaterialsBar(Entity);

/// The materials' bar starts right of the header (as laid out), so a long list wraps instead of running into the
/// title.
pub(super) fn fit_materials(mut bars: Query<(&MaterialsBar, &mut Node)>, heads: Query<(&ComputedNode, &UiGlobalTransform)>) {
    for (bar, mut node) in &mut bars {
        let Ok((head, tf)) = heads.get(bar.0) else { continue };
        if head.size.x <= 0.0 {
            continue;
        }
        // Physical px → UI px.
        let right = (tf.translation.x + head.size.x / 2.0) * head.inverse_scale_factor;
        let left = px((right + 24.0).round());
        if node.left != left {
            node.left = left;
        }
    }
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

/// Where the tree's cards go for this content in a layout.
pub fn metrics(tree: &Tree, lay: &Lay) -> Metrics {
    let columns = tree.columns.max(1) as f32;
    let pitch = ((lay.tree_right - lay.columns_x + lay.column_gap) / columns).min(lay.card_max_w + lay.column_gap + 10.0);
    let rows = tree.rows().max(1) as f32;
    Metrics {
        origin: Vec2::new(lay.columns_x, 0.0),
        inset: CARD_TOP,
        card: Vec2::new((pitch - lay.column_gap).min(lay.card_max_w), CARD_H),
        column_pitch: pitch,
        row_pitch: ((lay.tree_bottom - lay.tree_top) / rows).clamp(ROW_PITCH.0, ROW_PITCH.1),
        second_line: CHIP_H + CHIP_ROW_GAP,
        lane_head: lay.lane_head,
    }
}

/// The room the line under a card in `column` has: up to the next column's line (less a gap), in the last column
/// up to the tree's right edge.
fn under_room(m: &Metrics, lay: &Lay, columns: usize, column: usize) -> f32 {
    if column + 1 < columns { m.column_pitch - 10.0 } else { lay.tree_right - (m.origin.x + column as f32 * m.column_pitch) - UNDER_INSET }
}

/// The widest the line under a node's card gets: its kind and every cost chip at its longest (held just short of
/// the cost: `GROWN 23.9 / 24`, `1 / 2`). From the characters at a wide average glyph (0.6 em plus the tracking),
/// so it errs long: a line it calls too long for its room wraps to a second one, which its row makes room for.
fn under_width(node: &ResearchNode) -> f32 {
    const EM: f32 = 0.6;
    let text = |s: &str, tracking: f32| s.chars().count() as f32 * 7.5 * (EM + tracking);
    let chip = |s: &str, material: bool| 14.0 + 2.0 + text(s, 0.2) + if material { 7.0 + 4.0 } else { 0.0 };
    let datas = node.cost.data.iter().map(|(f, x)| {
        let cost = tree::amount(*x);
        6.0 + chip(&format!("{} {cost}.9 / {cost}", tree::family_label(f)), false)
    });
    let materials = node.cost.materials.values().map(|n| 6.0 + chip(&format!("{} / {n}", n.saturating_sub(1)), true));
    text(kind_words(node.kind), 0.25) + 4.0 + datas.chain(materials).sum::<f32>()
}

/// Which cards' lines under them take two lines (too long for their room): their rows are taller.
fn two_lines(content: &Content, tree: &Tree, m: &Metrics, lay: &Lay) -> Vec<bool> {
    tree.cards.iter().map(|c| content.research.get(&c.node).is_some_and(|n| under_width(n) > under_room(m, lay, tree.columns, c.column))).collect()
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
/// unlocks, the formation for a frame, the mind's ring for mind capacity, the first part a part-stat step picks.
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
        Some(Unlock::Facility(_)) => UiIcon::Glyph { glyph: Glyph::Diamond, color: style.ui.violet },
        Some(Unlock::JumpRange(_)) => UiIcon::Glyph { glyph: Glyph::Play, color: style.ui.teal },
        Some(Unlock::PartStat { target, .. }) => match content.parts.iter().find(|p| !p.hull && p.matches(target)) {
            Some(p) => UiIcon::Part { part: p.id.clone(), material: None },
            None => UiIcon::Glyph { glyph: Glyph::Diamond, color: style.ui.teal },
        },
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
                height: px(CHIP_H),
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

/// Spawns the screen (hidden), from the content's tree, for the window's layout class.
pub fn spawn(commands: Commands, window: Single<&Window, With<PrimaryWindow>>, fonts: Res<HudFonts>, style: Res<Style>, content: Res<GameContent>, tree: Res<ResearchTree>) {
    build(commands, &Lay::of(LayoutClass::of(window.size())), &fonts, &style, &content.0, &tree.0);
}

/// The window changed its layout class: the screen is built again for the new one (open at once when it was).
#[allow(clippy::too_many_arguments)]
pub fn relayout(
    mut commands: Commands,
    class: Res<LayoutClass>,
    root: Single<(Entity, &ResearchRoot)>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    content: Res<GameContent>,
    tree: Res<ResearchTree>,
) {
    let lay = Lay::of(*class);
    if root.1.0 == lay {
        return;
    }
    commands.entity(root.0).despawn();
    build(commands, &lay, &fonts, &style, &content.0, &tree.0);
}

/// A screen built again while open (`relayout`) shows at once.
pub fn reopen(mode: Res<State<AppMode>>, mut roots: Query<&mut ScreenFx, Added<ResearchRoot>>) {
    for mut fx in &mut roots {
        if *mode.get() == AppMode::Research {
            snap_screen(&mut fx, true);
        }
    }
}

fn build(mut commands: Commands, lay: &Lay, fonts: &HudFonts, style: &Style, content: &Content, tree: &Tree) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let m = metrics(tree, lay);
    let placed = tree.place(&m, &two_lines(content, tree, &m, lay));
    let inner_h = placed.lanes.last().map_or(0.0, |l| l.0 + l.1);
    commands
        .spawn((
            ResearchRoot(*lay),
            CompactText(COMPACT_TEXT),
            world::backdrop::VeilsBackdrop,
            CardStates::default(),
            // Opens and closes with a transition (`ui_kit::ScreenFx`): the four roots slide in from their edges; the
            // lab behind it shows only faintly.
            ScreenFx::closed().dimming(),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn(Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() })
                .with_children(|centre| {
                    centre.spawn(Node { width: px(lay.stage.x), height: px(lay.stage.y), flex_shrink: 0.0, ..default() }).with_children(|stage| {
                        // Three roots on the stage (the tree, the detail, the bottom bar) and the top bar over the window, so each
                        // can slide in on its own.
                        // The tree: tier heads over the columns, the lanes (scrolled when they don't fit).
                        stage.spawn((ScreenPart::new(Edge::Left, 1), abs(Vec2::new(0.0, lay.tier_y), Some(Vec2::new(lay.tree_right + 8.0, lay.tree_bottom - lay.tier_y))))).with_children(|left| {
                            for col in 0..tree.columns {
                                left.spawn((label(fonts, &format!("TIER {}", roman(col + 1)), 8.5, c(Tone::Dim, 1.0)), abs(Vec2::new(lay.columns_x + col as f32 * m.column_pitch + 14.0, 0.0), None)));
                            }
                            left.spawn((
                                ScrollPosition::default(),
                                // On a phone the tier heads sit close over the lanes: no glow room past its top.
                                if lay.compact { Scrollbar::vertical().room(0) } else { Scrollbar::vertical() },
                                Node { overflow: Overflow::scroll_y(), ..abs(Vec2::new(0.0, lay.tree_top - lay.tier_y), Some(Vec2::new(lay.tree_right + 8.0, lay.tree_bottom - lay.tree_top))) },
                            ))
                            .with_children(|area| {
                                area.spawn(Node { width: percent(100), height: px(inner_h), flex_shrink: 0.0, ..default() }).with_children(|lanes| {
                                    spawn_lanes(lanes, lay, fonts, style, content, tree, &m, &placed);
                                });
                            });
                        });

                        spawn_detail(stage, lay, fonts, style);

                        // Bottom: the way home, the line, the drydock (on a phone under the tree only).
                        let bar_w = if lay.compact { lay.panel_x - 16.0 } else { lay.stage.x };
                        let edge = if lay.compact { lay.tree_left } else { 22.0 };
                        stage.spawn((UiBlock, ScreenPart::new(Edge::Bottom, 2), abs(Vec2::new(0.0, lay.bottom_y), Some(Vec2::new(bar_w, 32.0))))).with_children(|bottom| {
                            bottom.spawn(abs(Vec2::new(edge, 0.0), None)).with_children(|b| {
                                spawn_pill(b, fonts, "‹  HOME", 9.5, 32.0, None, Pill::new(Tone::Teal), (ResearchButton(ResearchAction::Home), Tip::key("SHIPYARD", "ESC")));
                            });
                            let status = if lay.compact { abs(Vec2::new(edge + 92.0, 11.0), Some(Vec2::new(bar_w - edge - 92.0 - 110.0, 14.0))) } else { abs(Vec2::new(150.0, 11.0), None) };
                            bottom.spawn((RText::Status, label(fonts, "", 9.0, c(Tone::Teal, 1.0)), TextLayout::no_wrap(), Node { overflow: Overflow::clip(), ..status }));
                            bottom.spawn(Node { position_type: PositionType::Absolute, right: px(if lay.compact { 0.0 } else { 24.0 }), top: px(0), ..default() }).with_children(|b| {
                                spawn_pill(b, fonts, "DRYDOCK", 9.5, 32.0, None, Pill::new(Tone::Teal), (ResearchButton(ResearchAction::Drydock), Tip::new("DRYDOCK"), ActionTip(Action::OpenDrydock)));
                            });
                        });
                    });
                });
            // The top bar across the window: the way back and where we are (the frame's top left), the data of families
            // without a lane and the materials research costs (its top right).
            let f = frame();
            root.spawn((UiBlock, ScreenPart::new(Edge::Top, 0), Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: percent(100), height: px(lay.top_h), ..default() })).with_children(|top| {
                let head = top.spawn(f.top_left()).with_children(|tl| {
                    spawn_head(tl, fonts, &ui, (ResearchButton(ResearchAction::Home), Tip::key("SHIPYARD", "ESC")), "SHIPYARD  ›  RESEARCH", "Research");
                }).id();
                // The materials wrap (on a phone in up to two rows, ALSO IN STORE left out) in the room right of the
                // header, each one whole (`fit_materials` keeps them clear of the header; 19 Oct 2026: a long list
                // ran into the title, on a phone and at 1280 × 720 alike). In one row they sit as the frame's top
                // right does.
                let materials = Node {
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::FlexEnd,
                    align_content: AlignContent::Center,
                    column_gap: px(if lay.compact { 12.0 } else { f.pill_gap }),
                    ..if lay.compact { Node { right: px(lay.tree_left), top: px(4), min_height: px(lay.top_h - 8.0), ..f.top_right() } } else { f.top_right() }
                };
                top.spawn((MaterialsBar(head), materials)).with_children(|tr| {
                    let shown = if lay.compact { Display::None } else { Display::Flex };
                    tr.spawn((RText::AlsoInStore, label(fonts, "", 8.5, c(Tone::Dim, 1.0)), Node { display: shown, margin: UiRect::right(px(16)), ..default() }));
                    for (k, mat) in cost_materials(content).iter().enumerate() {
                        let name = content.materials.get(mat).map_or(mat.to_uppercase(), |m| m.name.to_uppercase());
                        let gap = if lay.compact { 5.0 } else { f.pill_gap };
                        tr.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), ..default() }).with_children(|one| {
                            one.spawn((UiIcon::Glyph { glyph: Glyph::Diamond, color: ui.gold }, Node { width: px(9), height: px(9), ..default() }));
                            one.spawn(label(fonts, &name, 9.0, c(Tone::Label, 1.0)));
                            one.spawn((RText::Material(k), number(fonts, "", lay.material, c(Tone::Gold, 1.0)), Node { margin: UiRect::right(px(if lay.compact { 0.0 } else { 6.0 })), ..default() }));
                        });
                    }
                });
            });
        });
}

/// The lanes' heads, separators and dots, the connectors, the cards and the lines under them.
#[allow(clippy::too_many_arguments)]
fn spawn_lanes(lanes: &mut ChildSpawnerCommands, lay: &Lay, fonts: &HudFonts, style: &Style, content: &Content, tree: &Tree, m: &Metrics, placed: &tree::Placed) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    for (k, lane) in tree.lanes.iter().enumerate() {
        let (top, h) = placed.lanes[k];
        let colour = family_color(content, style, &lane.family);
        let mid = top + m.lane_head + m.inset + m.card.y / 2.0;
        if lay.compact {
            // Over the cards: the family's dot, its name and its data in one line.
            lanes.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(7), ..abs(Vec2::new(lay.tree_left, top + 6.0), Some(Vec2::new(lay.tree_right - lay.tree_left, m.lane_head - 4.0))) }).with_children(|head| {
                head.spawn((Node { width: px(7), height: px(7), border_radius: BorderRadius::MAX, ..default() }, BackgroundColor(ui_color(colour, 0.95))));
                head.spawn(label(fonts, &family_label(&lane.family), 9.0, c(Tone::Label, 1.0)));
                head.spawn((RText::LaneAmount(k), number(fonts, "", 13.0, c(Tone::Number, 1.0))));
                head.spawn(label(fonts, "DATA", 7.5, c(Tone::Dim, 1.0)));
            });
        } else {
            lanes.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), ..abs(Vec2::new(lay.tree_left, top + m.inset), None) }).with_children(|head| {
                head.spawn(label(fonts, &family_label(&lane.family), 9.0, c(Tone::Label, 1.0)));
                head.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Baseline, column_gap: px(7), ..default() }).with_children(|row| {
                    row.spawn((RText::LaneAmount(k), number(fonts, "", 21.0, c(Tone::Number, 1.0))));
                    row.spawn(label(fonts, "DATA", 7.5, c(Tone::Dim, 1.0)));
                });
            });
            // The family's dot, joined to the lane's first card.
            lanes.spawn((Node { border_radius: BorderRadius::MAX, ..abs(Vec2::new(lay.columns_x - 22.0, mid - 3.5), Some(Vec2::splat(7.0))) }, BackgroundColor(ui_color(colour, 0.95))));
            lanes.spawn((abs(Vec2::new(lay.columns_x - 15.0, mid - LINE / 2.0), Some(Vec2::new(15.0, LINE))), BackgroundColor(ui_color(colour, 0.45))));
        }
        if k + 1 < tree.lanes.len() {
            lanes.spawn((abs(Vec2::new(lay.tree_left, top + h - 1.0), Some(Vec2::new(lay.tree_right - lay.tree_left, 1.0))), BackgroundColor(c(Tone::Label, 0.07))));
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
                    column_gap: px(if lay.compact { 7.0 } else { 10.0 }),
                    padding: UiRect::horizontal(px(if lay.compact { 10.0 } else { 14.0 })),
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
                b.spawn((RText::CardName(k), prose(fonts, &node.name, lay.card_name, c(Tone::Number, 1.0)), Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), ..default() }));
                b.spawn((RText::CardDone(k), label(fonts, "DONE", 7.5, c(Tone::Teal, 1.0)), Node { display: Display::None, flex_shrink: 0.0, ..default() }));
                b.spawn((RIcon::Mark(k), UiIcon::Glyph { glyph: Glyph::Diamond, color: ui.gold }, Node { display: Display::None, width: px(9), height: px(9), flex_shrink: 0.0, ..default() }));
            });
        // Under the card: its kind and its cost, wrapping to a second line past its room.
        let room = under_room(m, lay, tree.columns, tree.cards[k].column);
        lanes
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: px(6),
                row_gap: px(CHIP_ROW_GAP),
                max_width: px(room),
                ..abs(Vec2::new(r.min.x + UNDER_INSET, r.max.y + UNDER_GAP), None)
            })
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
fn spawn_detail(stage: &mut ChildSpawnerCommands, lay: &Lay, fonts: &HudFonts, style: &Style) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let (tier_y, panel_w, unlock_y) = (lay.tier_y, lay.panel_w, lay.unlock_y);
    stage.spawn((UiBlock, ScreenPart::new(Edge::Right, 1), abs(Vec2::new(lay.panel_x, tier_y), Some(Vec2::new(panel_w, unlock_y + 44.0 - tier_y))))).with_children(|panel| {
        // The divider between the tree and the detail.
        let gap = (lay.panel_x - lay.tree_right) / 2.0 - 4.0;
        panel.spawn((abs(Vec2::new(-gap.min(20.0), 8.0), Some(Vec2::new(1.0, lay.tree_bottom - tier_y - 8.0))), BackgroundColor(c(Tone::Label, 0.12))));
        // The rows: on a phone they scroll (over the line and UNLOCK).
        let rows = Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            overflow: if lay.compact { Overflow::scroll_y() } else { Overflow::DEFAULT },
            ..abs(Vec2::ZERO, Some(Vec2::new(panel_w, if lay.compact { unlock_y - 28.0 - tier_y } else { 500.0 })))
        };
        let mut rows = panel.spawn(rows);
        if lay.compact {
            // No glow room past its bottom: the rows would show under the line and UNLOCK.
            rows.insert((ScrollPosition::default(), Scrollbar::vertical().room(0)));
        }
        rows
            .with_children(|p| {
                p.spawn((RText::Kind, label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                p.spawn((RText::Name, prose(fonts, "", lay.name, c(Tone::Number, 1.0))));
                p.spawn((RText::Summary, prose(fonts, "", 12.0, c(Tone::Label, 1.0)), Node { margin: UiRect::bottom(px(6)), ..default() }));
                // The preview: the icon in brackets, the stat cells beside it, the bonuses under them.
                p.spawn((RNode::Preview, Node { flex_direction: FlexDirection::Column, row_gap: px(8), ..default() })).with_children(|pv| {
                    pv.spawn((RText::PreviewHead, label(fonts, "", 7.5, c(Tone::Dim, 1.0))));
                    pv.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(18), ..default() }).with_children(|row| {
                        let picture = if lay.compact { Display::None } else { Display::Flex };
                        row.spawn(Node { display: picture, width: px(84), height: px(110), flex_shrink: 0.0, ..default() }).with_children(|b| {
                            spawn_brackets(b, 10.0, 1.2, c(Tone::Label, 0.45));
                            b.spawn((RIcon::Preview, UiIcon::Glyph { glyph: Glyph::Menu, color: ui.dim }, Node { position_type: PositionType::Absolute, left: px(12), top: px(12), width: px(60), height: px(86), ..default() }));
                        });
                        row.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, row_gap: px(9), width: px(if lay.compact { panel_w } else { panel_w - 102.0 }), align_content: AlignContent::FlexStart, ..default() })
                            .with_children(|grid| {
                                for k in 0..STATS {
                                    let cell_w = if lay.compact { ((panel_w - 12.0) / 2.0).floor() } else { 118.0 };
                                    grid.spawn((RNode::Stat(k), Node { flex_direction: FlexDirection::Column, row_gap: px(3), width: px(cell_w), ..default() })).with_children(|cell| {
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
                let head_w = if lay.compact { 70.0 } else { 80.0 };
                let head = |text: &str| (label(fonts, text, 8.0, c(Tone::Dim, 1.0)), Node { width: px(head_w), flex_shrink: 0.0, ..default() });
                let wrap = if lay.compact { FlexWrap::Wrap } else { FlexWrap::NoWrap };
                p.spawn(Node { flex_wrap: wrap, row_gap: px(4), ..row(6.0) }).with_children(|r| {
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
        panel.spawn((RText::Say, label(fonts, "", 8.5, c(Tone::Teal, 1.0)), abs(Vec2::new(0.0, unlock_y - 22.0 - tier_y), Some(Vec2::new(panel_w, 14.0)))));
        // UNLOCK: a primary pill (the kit styles it), its cost in words and the materials after a diamond.
        panel.spawn(Node { flex_direction: FlexDirection::Column, ..abs(Vec2::new(0.0, unlock_y - tier_y), Some(Vec2::new(panel_w, 44.0))) }).with_children(|b| {
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
                // The letter spacing also follows the last letter: the words' own left margin as wide balances it, so the
                // words, with or without the materials after them, sit centred (as `spawn_pill`'s label does).
                p.spawn((UnlockText::Words, pill_text("UNLOCK"), Node { margin: UiRect::left(px(3.0)), ..default() }));
                p.spawn((RNode::UnlockMatter, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), ..default() })).with_children(|m| {
                    m.spawn((UiIcon::Glyph { glyph: Glyph::Diamond, color: ui.gold }, Node { width: px(9), height: px(9), ..default() }));
                    m.spawn((UnlockText::Matter, pill_text("")));
                });
            });
        });
    });
}

/// The screen shows only on the research screen, opening and closing with its transition (`ui_kit::ScreenFx`).
pub fn show(mode: Res<State<AppMode>>, mut root: Single<&mut ScreenFx, With<ResearchRoot>>) {
    show_screen(&mut root, *mode.get() == AppMode::Research);
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

/// One stat cell of the preview.
#[derive(Clone, Debug, PartialEq)]
pub struct Stat {
    pub label: String,
    pub value: String,
}

/// What the preview shows for a node: a head, the icon, stat cells and bonus lines. A part: its stat lines from
/// content (as the drydock's palette part); a material: the armour block in it; a frame: its size; mind
/// capacity: the mind before and after; jump range: the drive's reach before and after; a part-stat step: each part it
/// picks, its stat before and after the node (`has`: the career's steps, `done`: the node's among them).
pub fn preview(content: &Content, style: &Style, node: &ResearchNode, now: tree::Now, has: (&data::Upgrades, bool)) -> (String, UiIcon, Vec<Stat>, Vec<String>) {
    let capacity = now.mind;
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
            let stats = vec![Stat { label: "MIND CAPACITY".into(), value: format!("{capacity} » {} cores", capacity + n) }];
            ("THE SHIP-MIND".into(), icon, stats, Vec::new())
        }
        Some(Unlock::Facility(f)) => (content.facilities.get(f).map_or_else(|| f.to_uppercase(), |x| format!("{} AT HOME", x.name.to_uppercase())), icon, Vec::new(), Vec::new()),
        Some(Unlock::JumpRange(s)) => {
            let label = content.stats.get("jump_range").map_or("JUMP RANGE".into(), |d| d.name.to_uppercase());
            let value = format!("{} » {}", tree::range_text(content, now.range), tree::range_text(content, now.range + s));
            ("THE DRIVE".into(), icon, vec![Stat { label, value }], Vec::new())
        }
        Some(Unlock::PartStat { target, stat, .. }) => {
            let (before, after) = step_around(content, node, has);
            let mut stats = Vec::new();
            for part in content.parts.iter().filter(|p| p.matches(target)) {
                let (b, a) = (content.part_stats_upgraded(part, None, &before), content.part_stats_upgraded(part, None, &after));
                for (b, a) in b.iter().zip(&a).filter(|(b, _)| b.bonus.is_none() && data::design::upgrades::shows_on(stat, &b.stat)) {
                    stats.push(Stat { label: format!("{} {}", part.name, b.label).to_uppercase(), value: content.before_after(b, a) });
                }
            }
            let label = content.stats.get(stat).map_or_else(|| stat.to_uppercase(), |d| d.name.to_uppercase());
            (format!("{label}  ·  {}", content.selector_label(target).to_uppercase()), icon, stats, Vec::new())
        }
        None => (String::new(), icon, Vec::new(), Vec::new()),
    }
}

/// The career's part-stat steps without and with `node`'s (`has`: the career's, `done`: the node's among them).
fn step_around(content: &Content, node: &ResearchNode, has: (&data::Upgrades, bool)) -> (data::Upgrades, data::Upgrades) {
    let (has, done) = has;
    let own = data::Upgrades::of_research(content, [node.id.as_str()]);
    let mut without = has.clone();
    if done {
        for s in &own.0 {
            if let Some(k) = without.0.iter().rposition(|x| x == s) {
                without.0.remove(k);
            }
        }
    }
    let mut with = without.clone();
    with.0.extend(own.0);
    (without, with)
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
    let cards: Vec<CardState> = tree.cards.iter().map(|card| tree::state(content, c, &card.node)).collect();
    if states.0 != cards {
        states.0 = cards;
    }
    let now = tree::Now::of(content, c, granted.mind_capacity);
    let states = &states.0;
    let node_of = |k: usize| content.research.get(&tree.cards[k].node);
    let selected = view.selected.as_deref().and_then(|s| content.research.get(s));
    let sel_state = selected.map(|n| tree::state(content, c, &n.id));
    let chips_of: Vec<Vec<tree::Chip>> = (0..tree.cards.len()).map(|k| node_of(k).map_or(Vec::new(), |n| tree::chips(content, c, n))).collect();
    // A researched node shows what it cost, not what the stores hold now.
    let done = matches!(sel_state, Some(CardState::Done));
    let sel_chips = selected.map_or(Vec::new(), |n| if done { tree::cost_chips(n) } else { tree::chips(content, c, n) });
    let (head, icon, stats, bonuses) = selected.map_or((String::new(), UiIcon::Glyph { glyph: Glyph::Menu, color: ui.dim }, Vec::new(), Vec::new()), |n| preview(content, &style, n, now, (&granted.upgrades, done)));
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
            RText::Gives => (selected.map_or(String::new(), |n| tree::gives(content, n, now)), Tone::Number),
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
    /// panel; its sixteen rows (ten lanes) are more than fit, so they keep the smallest row pitch and the lanes scroll.
    #[test]
    fn the_tree_fits_the_layout() {
        let content = data::base_content();
        let tree = Tree::new(&content);
        let lay = Lay::WIDE;
        let m = metrics(&tree, &lay);
        assert_eq!(m.card.x, lay.card_max_w);
        let two = two_lines(&content, &tree, &m, &lay);
        let placed = tree.place(&m, &two);
        let right = placed.cards.iter().map(|r| r.max.x).fold(0.0, f32::max);
        assert!(lay.tree_right < lay.panel_x - 20.0);
        assert!(right <= lay.tree_right, "{right}");
        let bottom = placed.lanes.last().map(|l| l.0 + l.1).unwrap();
        assert!(bottom > lay.tree_bottom - lay.tree_top && m.row_pitch == ROW_PITCH.0, "{bottom}");
        // A card's line under it (one or two lines of chips) ends before the next row's card starts, in every lane
        // and across lanes; the gate's and the Grown's long lines wrap.
        let mut rows: Vec<(f32, bool)> = placed.cards.iter().zip(&two).map(|(r, &t)| (r.min.y, t)).collect();
        rows.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for w in rows.windows(2).filter(|w| w[1].0 > w[0].0) {
            let end = w[0].0 + CARD_H + UNDER_GAP + CHIP_H + if w[0].1 { CHIP_ROW_GAP + CHIP_H } else { 0.0 };
            assert!(end < w[1].0, "a cost line ending at {end} runs into the card at {}", w[1].0);
        }
        assert!(two[tree.card_of("grown_heart").unwrap()]);
        assert!(!two[tree.card_of("beam_laser").unwrap()]);
        // More columns: narrower cards, still inside.
        let wide = Tree { columns: 5, ..tree.clone() };
        let m = metrics(&wide, &lay);
        assert!(lay.columns_x + 4.0 * m.column_pitch + m.card.x <= lay.tree_right + 0.01);
        assert_eq!(roman(1), "I");
        assert_eq!(roman(14), "XIV");
    }

    /// On a phone (844 × 390): the three columns fit left of the detail with cards wide enough for a name, each
    /// lane's head over its cards; the parts stay inside the stage and clear of each other.
    #[test]
    fn the_tree_fits_a_phone() {
        let content = data::base_content();
        let tree = Tree::new(&content);
        let lay = Lay::COMPACT;
        let m = metrics(&tree, &lay);
        assert!(m.card.x >= 150.0, "{}", m.card.x);
        let placed = tree.place(&m, &two_lines(&content, &tree, &m, &lay));
        let right = placed.cards.iter().map(|r| r.max.x).fold(0.0, f32::max);
        assert!(right <= lay.tree_right && lay.tree_right + 16.0 <= lay.panel_x, "{right}");
        assert!(lay.panel_x + lay.panel_w <= lay.stage.x - 12.0);
        // Every lane's first card sits under its head.
        for (k, &(top, _)) in placed.lanes.iter().enumerate() {
            let first = tree.cards.iter().zip(&placed.cards).filter(|(c, _)| c.lane == k).map(|(_, r)| r.min.y).fold(f32::MAX, f32::min);
            assert!(first >= top + lay.lane_head, "lane {k}");
        }
        assert!(lay.tree_bottom <= lay.bottom_y && lay.bottom_y + 32.0 <= lay.stage.y);
        assert!(lay.unlock_y + 44.0 <= lay.stage.y && lay.top_h <= lay.tier_y);
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
