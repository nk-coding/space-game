//! A part's information, one source for every screen that shows it (Niklas, 18 Oct 2026): [`part_info`] (name, size
//! and category, the stat lines from content, what it is fabricated from and the stock, the bonuses it gives), the
//! stat block that shows the lines ([`StatBlock`]: the drydock's SELECTED PART and the hover card), and the hover card
//! beside a part list ([`PartHover`]: the drydock's palette, the workshop's parts).
//!
//! * **The stat block**: two columns, each line a label over its value. A label stays on one line (it never wraps
//!   into its value: Niklas, "the power output looks shit", POWER OUTPUT broke over its 12 MW in a third of the
//!   column), so every value sits at the same height in its row; a long value (the recipe, the stock) spans both
//!   columns. Its sizes are the style tokens' `stats.*`.
//! * **The hover card**: a row that carries a [`PartHover`] (or holds the node under the pointer) shows the card after
//!   the pointer rests on it for `card.delay`; from one row to the next it follows at once. It sits beside the row,
//!   outside the list (`card.offset` from the row's right edge, or left of it when the window has no room), level
//!   with the row, moved up when it would run past the bottom of the row's list (each screen ends its list above
//!   its bottom bars and status lines) and never above the top bar ([`kit::ui_kit::Frame::below_top`]; Niklas, 18 Oct 2026: "it
//!   should not cover the bottom area"). A part too tall for that room shows its stat rows closer ([`Fit::Tight`]),
//!   then in three columns ([`Fit::Wide`], when the card still fits beside the row). Its icon, size and category,
//!   stat block and bonuses.
//! * **Layering**: part icons are world meshes under bevy_ui. The card's backing is a world mesh too (a filled
//!   [`UiIcon::Scrim`]) one icon layer up ([`UiIconLayer`]), its icon on that layer, so the card covers the icons
//!   below it and shows its own; its words are bevy_ui. bevy_ui draws over every icon, so the card lies beside the
//!   list, over the world (the drydock's grid, the workshop's room between its columns), never over a list's text.
//!
//! Its words are chrome in code; names, stat labels and units come from content.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale, UiTransform, Val2};
use bevy::window::PrimaryWindow;
use data::career::fleet_parts;
use data::{Content, Material, Part, Unlock};
use kit::style::{Style, ui_color};
use kit::ui_icons::{UiIcon, UiIconLayer};
use kit::ui_kit::{GroupedList, Hot, HudFonts, Tone, UiInputSet, UiStyleSet, frame, load_fonts, number, prose, set_color, set_text, spawn_brackets, tracked};

use crate::career::{EscortDesigns, Granted, unlocked_by};
use crate::player::PlayerDesign;
use crate::sim_bridge::GameContent;

/// One line of a stat block: its label (uppercase), value and the value's tone; `wide` spans both columns.
#[derive(Clone, Debug, PartialEq)]
pub struct StatLine {
    pub label: String,
    pub value: String,
    pub tone: Tone,
    pub wide: bool,
}

impl StatLine {
    pub fn new(label: impl Into<String>, value: impl Into<String>, tone: Tone) -> Self {
        Self { label: label.into(), value: value.into(), tone, wide: false }
    }
}

/// What a part is, as the drydock's SELECTED PART and the hover card show it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PartInfo {
    pub part: String,
    /// The armour material its icon shows in (hull parts).
    pub material: Option<String>,
    pub name: String,
    /// `2×1  ·  WEAPONS` (`·  STEEL` for a hull part), or `LOCKED  ·  <research node>`.
    pub sub: String,
    pub sub_tone: Tone,
    pub summary: String,
    pub stats: Vec<StatLine>,
    /// Gold bonus lines (uppercase, without the diamond).
    pub bonuses: Vec<String>,
}

/// What a stock line counts against: the design in the drydock, the fleet in the workshop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Design,
    Fleet,
}

impl Held {
    fn words(self) -> &'static str {
        match self {
            Held::Design => "in the design",
            Held::Fleet => "in the fleet",
        }
    }
}

/// A part's information (pure): `material` for a hull part's numbers; `held` how many of it the design or the fleet
/// holds. A part that is not hull ends with the workshop's lines (GDR-0108): what it is fabricated from and the
/// stock against `held` (unless every part is granted, `?unlock=all`). A locked part says which research unlocks it.
pub fn part_info(content: &Content, granted: &Granted, part: &Part, material: Option<&Material>, held: (u32, Held)) -> PartInfo {
    let material = material.filter(|_| part.hull);
    let cat = content.categories.get(&part.category).map_or(String::new(), |c| c.name.to_uppercase());
    let open = granted.unlocks.has_part(&part.id);
    let (sub, sub_tone) = if open {
        let mut sub = format!("{}×{}  ·  {cat}", part.size.0, part.size.1);
        if let Some(m) = material {
            sub.push_str(&format!("  ·  {}", m.name.to_uppercase()));
        }
        (sub, Tone::Dim)
    } else {
        (locked_sub(content, part), Tone::Dim)
    };
    let lines = content.part_stats(part, material);
    let mut stats: Vec<StatLine> = lines.iter().filter(|l| l.bonus.is_none()).map(|l| StatLine::new(l.label.to_uppercase(), l.text.clone(), Tone::Number)).collect();
    if !part.hull {
        let recipe = part.recipe.as_ref().map_or_else(
            || "Salvage only".to_string(),
            // An entry never breaks over two lines (no-break spaces in it); the line breaks between entries.
            |r| r.materials.iter().map(|(m, n)| format!("{n} {}", content.materials.get(m).map_or(m.as_str(), |x| x.name.as_str())).replace(' ', "\u{a0}")).collect::<Vec<_>>().join("  ·  "),
        );
        stats.push(StatLine { wide: true, ..StatLine::new("RECIPE", recipe, Tone::Number) });
        if let Some(n) = granted.in_stock(&part.id) {
            let (count, whose) = held;
            let tone = if count > n { Tone::Coral } else { Tone::Number };
            stats.push(StatLine { wide: true, ..StatLine::new("IN STOCK", format!("{n}  ·  {count} {}", whose.words()), tone) });
        }
    }
    let bonuses = lines
        .iter()
        .filter_map(|l| {
            let b = l.bonus.as_ref()?;
            let mut s = format!("{} {} TO ADJACENT {}", l.text, l.label, b.target_label).to_uppercase();
            if let Some(n) = b.max_stacks {
                s.push_str(&format!("  ·  MAX {n}"));
            }
            Some(s)
        })
        .collect();
    PartInfo {
        part: part.id.clone(),
        material: material.map(|m| m.id.clone()),
        name: part.name.clone(),
        sub,
        sub_tone,
        summary: part.summary.clone(),
        stats,
        bonuses,
    }
}

/// What a locked part says under its name: `LOCKED  ·  <the research node that unlocks it>`.
pub fn locked_sub(content: &Content, part: &Part) -> String {
    match unlocked_by(content, &Unlock::Part(part.id.clone())) {
        Some(node) => format!("LOCKED  ·  {}", node.name.to_uppercase()),
        None => "LOCKED".into(),
    }
}

// ---------------------------------------------------------------------------------------------------------
// The stat block

/// A stat block's lines (write them; the block shows them, [`spawn_stat_block`]).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct StatBlock(pub Vec<StatLine>);

/// A cell of a stat block, and its texts.
#[derive(Component)]
struct StatCell;

#[derive(Component)]
enum CellText {
    Label,
    Value,
}

/// A stat block of `cells` lines at most into `parent`, with `extra` on it (its lines: [`StatBlock`]).
pub fn spawn_stat_block(parent: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, cells: usize, extra: impl Bundle) -> Entity {
    let s = &frame().stats;
    let c = |tone: Tone| ui_color(tone.color(&style.ui), 1.0);
    parent
        .spawn((
            StatBlock::default(),
            Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                grid_auto_flow: GridAutoFlow::RowDense,
                column_gap: px(s.col_gap),
                row_gap: px(s.row_gap),
                flex_shrink: 0.0,
                ..default()
            },
            extra,
        ))
        .with_children(|g| {
            for _ in 0..cells {
                // One line for the label, clipped at the cell's edge (never wrapped into the value); the value under it.
                g.spawn((StatCell, Node { display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(s.line_gap), min_width: px(0), overflow: Overflow::clip_x(), ..default() }))
                    .with_children(|cell| {
                        cell.spawn((CellText::Label, tracked(fonts, "", s.label, c(Tone::Label), s.label_em), TextLayout::no_wrap()));
                        cell.spawn((CellText::Value, number(fonts, "", s.value, c(Tone::Number))));
                    });
            }
        })
        .id()
}

/// The blocks show their lines: a cell per line (a wide one across both columns), the rest hidden.
fn show_stat_blocks(
    style: Res<Style>,
    blocks: Query<(&StatBlock, &Children), Changed<StatBlock>>,
    mut cells: Query<(&mut Node, &Children), With<StatCell>>,
    mut texts: Query<(&CellText, &mut Text, &mut TextColor)>,
) {
    for (block, kids) in &blocks {
        for (i, cell) in kids.iter().enumerate() {
            let Ok((mut node, cell_kids)) = cells.get_mut(cell) else { continue };
            let line = block.0.get(i);
            let display = if line.is_some() { Display::Flex } else { Display::None };
            let column = if line.is_some_and(|l| l.wide) { GridPlacement::span(2) } else { GridPlacement::DEFAULT };
            if node.display != display || node.grid_column != column {
                node.display = display;
                node.grid_column = column;
            }
            let Some(line) = line else { continue };
            for t in cell_kids.iter() {
                let Ok((which, mut text, mut color)) = texts.get_mut(t) else { continue };
                match which {
                    CellText::Label => set_text(&mut text, &line.label),
                    CellText::Value => {
                        set_text(&mut text, &line.value);
                        set_color(&mut color, ui_color(line.tone.color(&style.ui), 1.0));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// The hover card

/// A list row the hover card tells about: its part, the armour its icon shows in, what its stock counts against.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct PartHover {
    pub part: String,
    pub material: Option<String>,
    pub held: Held,
}

/// The card's root, its icon and its texts.
#[derive(Component)]
struct Card;

#[derive(Component)]
struct CardIcon;

#[derive(Component)]
struct CardStats;

#[derive(Component)]
enum CardText {
    Name,
    Sub,
    Bonuses,
}

/// The row the pointer rests on, since when, what the card shows and how it fits its room.
#[derive(Default)]
struct Resting {
    row: Option<Entity>,
    since: f32,
    shown: Option<(Entity, PartHover)>,
    fit: Fit,
}

/// How a card fits its room: its stat rows `stats.row_gap` apart, `card.tight_gap` apart, or that and in three
/// columns (the card a column wider).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fit {
    #[default]
    Normal,
    Tight,
    Wide,
}

impl Fit {
    fn columns(self) -> u16 {
        if self == Fit::Wide { 3 } else { 2 }
    }

    fn row_gap(self) -> f32 {
        if self == Fit::Normal { frame().stats.row_gap } else { frame().card.tight_gap }
    }

    /// The card's width: `card.width` with two columns, a column and its gap more with three.
    pub fn width(self) -> f32 {
        let (card, s) = (&frame().card, &frame().stats);
        let column = (card.width - 2.0 * card.pad - 2.0 - s.col_gap) / 2.0;
        card.width + f32::from(self.columns() - 2) * (column + s.col_gap)
    }
}

/// The most stat lines a card shows (a part has fewer).
const CARD_CELLS: usize = 16;
/// How far up from the node under the pointer a row's [`PartHover`] is looked for (a pill in the row).
const ROW_DEPTH: usize = 6;
/// The card over the screens' menus, under the tooltip.
const CARD_Z: i32 = i32::MAX - 16;

pub struct PartCardPlugin;

impl Plugin for PartCardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_card.after(load_fonts)).add_systems(Update, (show_card.after(UiInputSet), show_stat_blocks.after(show_card)).before(UiStyleSet));
    }
}

fn spawn_card(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let (f, ui) = (frame(), style.ui);
    let (card, panel) = (&f.card, &f.panel);
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    commands
        .spawn((
            Card,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: px(card.width),
                flex_direction: FlexDirection::Column,
                row_gap: px(card.gap),
                padding: UiRect::all(px(card.pad)),
                border: UiRect::all(px(1)),
                ..default()
            },
            // The backing is a world mesh one icon layer up (see the module doc), the border bevy_ui's.
            UiIcon::Scrim { color: style.background.outer().with_alpha(panel.fill), hole: [f32::INFINITY; 4], out: 0.0, into: 0.0 },
            UiIconLayer(1),
            BorderColor::all(c(Tone::Teal, panel.border)),
            GlobalZIndex(CARD_Z),
            UiTransform::default(),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(card.gap + 2.0), ..default() }).with_children(|h| {
                h.spawn(Node { width: px(card.icon), height: px(card.icon), flex_shrink: 0.0, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() })
                    .with_children(|b| {
                        spawn_brackets(b, 7.0, 1.2, c(Tone::Label, 0.45));
                        let inner = card.icon - 12.0;
                        b.spawn((CardIcon, UiIcon::Part { part: String::new(), material: None }, UiIconLayer(1), Node { width: px(inner), height: px(inner), ..default() }));
                    });
                h.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(5), flex_shrink: 1.0, min_width: px(0), ..default() }).with_children(|t| {
                    t.spawn((CardText::Name, prose(&fonts, "", 16.0, c(Tone::Number, 1.0))));
                    t.spawn((CardText::Sub, tracked(&fonts, "", 8.0, c(Tone::Dim, 1.0), 0.2)));
                });
            });
            spawn_stat_block(p, &fonts, &style, CARD_CELLS, CardStats);
            p.spawn((CardText::Bonuses, tracked(&fonts, "", 8.0, c(Tone::Gold, 1.0), 0.16)));
        });
}

/// The card for the row the pointer rests on (after `card.delay`; from a shown card's row to the next at once), beside
/// the row and inside the window; hidden when the pointer leaves the rows.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn show_card(
    time: Res<Time<Real>>,
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    (content, granted, design, escorts): (Res<GameContent>, Res<Granted>, Res<PlayerDesign>, Res<EscortDesigns>),
    hots: Query<(Entity, &Hot)>,
    parents: Query<&ChildOf>,
    rows: Query<(&PartHover, &ComputedNode, &UiGlobalTransform, &InheritedVisibility)>,
    lists: Query<(&ComputedNode, &UiGlobalTransform), With<GroupedList>>,
    mut card: Single<(&mut Visibility, &mut UiTransform, &ComputedNode, &mut Node), With<Card>>,
    mut icon: Single<&mut UiIcon, With<CardIcon>>,
    mut block: Single<(&mut StatBlock, &mut Node), (With<CardStats>, Without<Card>)>,
    mut texts: Query<(&CardText, &mut Text, &mut TextColor, &mut Node), (Without<Card>, Without<CardStats>)>,
    style: Res<Style>,
    mut rest: Local<Resting>,
) {
    let now = time.elapsed_secs();
    // The row of the node under the pointer: it, or the nearest above it that tells about a part.
    let row = hots.iter().find(|(_, h)| h.hovered).and_then(|(e, _)| {
        std::iter::successors(Some(e), |e| parents.get(*e).ok().map(ChildOf::parent)).take(ROW_DEPTH).find(|e| rows.get(*e).is_ok_and(|(.., v)| v.get()))
    });
    if row != rest.row {
        rest.row = row;
        rest.since = now;
    }
    let (vis, tf, node, card_node) = &mut *card;
    let (block, block_node) = &mut *block;
    let want = row.filter(|_| rest.shown.is_some() || now - rest.since >= frame().card.delay).and_then(|e| rows.get(e).ok().map(|(h, ..)| (e, h.clone())));
    let Some((row_e, hover)) = want else {
        rest.shown = None;
        if **vis != Visibility::Hidden {
            **vis = Visibility::Hidden;
        }
        return;
    };
    // New words when the row, its part or what they count changes.
    if rest.shown.as_ref() != Some(&(row_e, hover.clone())) || granted.is_changed() || design.is_changed() || escorts.is_changed() {
        let c = &content.0;
        if let Some(part) = c.parts.get(&hover.part) {
            let held = match hover.held {
                Held::Design => design.dock().parts().iter().filter(|p| p.part == part.id).count() as u32,
                Held::Fleet => fleet_parts(c, std::iter::once(design.blueprint()).chain(&escorts.0)).get(&part.id).copied().unwrap_or(0),
            };
            let info = part_info(c, &granted, part, hover.material.as_deref().and_then(|m| c.materials.get(m)), (held, hover.held));
            let want_icon = UiIcon::Part { part: part.id.clone(), material: info.material.clone() };
            if **icon != want_icon {
                **icon = want_icon;
            }
            if block.0 != info.stats {
                block.0 = info.stats.clone();
            }
            for (t, mut text, mut color, mut n) in &mut texts {
                let (s, tone) = match t {
                    CardText::Name => (info.name.clone(), Tone::Number),
                    CardText::Sub => (info.sub.clone(), info.sub_tone),
                    CardText::Bonuses => (info.bonuses.iter().map(|b| format!("◆  {b}")).collect::<Vec<_>>().join("\n"), Tone::Gold),
                };
                let d = if s.is_empty() { Display::None } else { Display::Flex };
                if n.display != d {
                    n.display = d;
                }
                set_text(&mut text, &s);
                set_color(&mut color, ui_color(tone.color(&style.ui), 1.0));
            }
        }
        let first = rest.shown.as_ref().is_none_or(|(e, h)| *e != row_e || h.part != hover.part);
        rest.shown = Some((row_e, hover));
        if first {
            // Laid out this frame (at the normal fit), placed and shown from the next.
            rest.fit = Fit::Normal;
            lay_fit(rest.fit, card_node, block_node);
            if **vis != Visibility::Hidden {
                **vis = Visibility::Hidden;
            }
            return;
        }
    }
    let Ok((_, row_node, row_at, _)) = rows.get(row_e) else { return };
    let k = ui_scale.0.max(1e-3);
    let scale = window.scale_factor() * k;
    let mut row_rect = Rect::from_center_size(row_at.translation / scale, row_node.size / scale);
    let size = node.size * node.inverse_scale_factor;
    // The room: the window inside the margin, under the top bar and down to the bottom of the row's list (each screen
    // ends it above its bottom bars and status lines).
    let (f, m) = (frame(), frame().card.margin);
    let mut room = Rect::from_corners(Vec2::new(m, f.below_top().max(m)), window.size() / k - m);
    let list = std::iter::successors(Some(row_e), |e| parents.get(*e).ok().map(ChildOf::parent)).find_map(|e| lists.get(e).ok());
    if let Some((n, at)) = list {
        room.max.y = room.max.y.min((at.translation.y + n.size.y / 2.0) / scale);
        // Beside the list, not just its row: its scrollbar runs down the area's right edge, in the room the area
        // keeps right of its rows (`ListSpec::pad_right`), so a card on the right starts past it (Niklas, 18 Oct
        // 2026: the workshop's card covered the bar).
        row_rect.max.x = row_rect.max.x.max((at.translation.x + n.size.x / 2.0) / scale);
    }
    // Too tall for the room: closer rows, then three columns; laid out this frame, placed from the next.
    if let Some(fit) = next_fit(rest.fit, row_rect, size, room, f.card.offset) {
        rest.fit = fit;
        lay_fit(fit, card_node, block_node);
        if **vis != Visibility::Hidden {
            **vis = Visibility::Hidden;
        }
        return;
    }
    let at = card_position(row_rect, size, room, f.card.offset);
    let t = Val2::px(at.x, at.y);
    if tf.translation != t {
        tf.translation = t;
    }
    if **vis != Visibility::Inherited {
        **vis = Visibility::Inherited;
    }
}

/// The card's and its stat block's layout for a fit (written only when it changes).
fn lay_fit(fit: Fit, card: &mut Node, block: &mut Node) {
    let (width, gap, columns) = (px(fit.width()), px(fit.row_gap()), RepeatedGridTrack::flex(fit.columns(), 1.0));
    if card.width != width {
        card.width = width;
    }
    if block.row_gap != gap || block.grid_template_columns != columns {
        block.row_gap = gap;
        block.grid_template_columns = columns;
    }
}

/// The next fit for a card of `size` at `fit` that is taller than its `room` (UI px): closer rows, then three columns
/// if the wider card fits beside the `row` (by `offset`) on either side; `None` when it fits or nothing is left.
pub fn next_fit(fit: Fit, row: Rect, size: Vec2, room: Rect, offset: f32) -> Option<Fit> {
    if size.y <= room.height() {
        return None;
    }
    match fit {
        Fit::Normal => Some(Fit::Tight),
        Fit::Tight => {
            let w = Fit::Wide.width();
            (row.max.x + offset + w <= room.max.x || row.min.x - offset - w >= room.min.x).then_some(Fit::Wide)
        }
        Fit::Wide => None,
    }
}

/// Where a card of `size` goes beside a row (all UI px, y down; the card's top-left corner): right of the row by
/// `offset`, level with its top; left of it when `room` (the window inside its margin, under the top bar and down to
/// the bottom of the row's list) has no space on the right; moved up to end inside the room (never above its top).
pub fn card_position(row: Rect, size: Vec2, room: Rect, offset: f32) -> Vec2 {
    let mut x = row.max.x + offset;
    if x + size.x > room.max.x {
        x = row.min.x - offset - size.x;
    }
    let y = row.min.y.min(room.max.y - size.y).max(room.min.y);
    Vec2::new(x.max(room.min.x), y)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The card sits right of its row, level with it; left of it at the window's right edge; up from the bottom edge.
    #[test]
    fn the_card_sits_beside_its_row() {
        // The window inside an 8 px margin, under the top bar at 68 and down to a list's bottom at 634.
        let room = Rect::new(8.0, 68.0, 1272.0, 634.0);
        let size = Vec2::new(272.0, 300.0);
        let row = Rect::new(20.0, 200.0, 268.0, 242.0);
        assert_eq!(card_position(row, size, room, 10.0), Vec2::new(278.0, 200.0));
        let low = Rect::new(20.0, 560.0, 268.0, 602.0);
        assert_eq!(card_position(low, size, room, 10.0).y, 634.0 - 300.0, "it ends with the list, off the bar under it");
        let tall = Vec2::new(272.0, 700.0);
        assert_eq!(card_position(row, tall, room, 10.0).y, 68.0, "never above the top bar");
        let right = Rect::new(900.0, 100.0, 1200.0, 142.0);
        assert_eq!(card_position(right, size, room, 10.0).x, 900.0 - 10.0 - 272.0);
    }

    /// A card taller than its room tightens its rows, then takes three columns while that fits beside the row.
    #[test]
    fn a_tall_card_compacts() {
        let room = Rect::new(8.0, 68.0, 1272.0, 434.0);
        let row = Rect::new(20.0, 380.0, 268.0, 422.0);
        let fits = Vec2::new(272.0, 300.0);
        let tall = Vec2::new(272.0, 420.0);
        assert_eq!(next_fit(Fit::Normal, row, fits, room, 10.0), None);
        assert_eq!(next_fit(Fit::Normal, row, tall, room, 10.0), Some(Fit::Tight));
        assert_eq!(next_fit(Fit::Tight, row, tall, room, 10.0), Some(Fit::Wide));
        assert_eq!(next_fit(Fit::Wide, row, tall, room, 10.0), None);
        let narrow = Rect::new(8.0, 68.0, 560.0, 434.0);
        assert_eq!(next_fit(Fit::Tight, row, tall, narrow, 10.0), None, "no room beside the row for three columns");
        assert!(Fit::Wide.width() > Fit::Normal.width() && Fit::Tight.width() == Fit::Normal.width());
    }

    /// Every part's stat labels fit one line of the card's narrower column (the drydock's is wider): estimated at
    /// Barlow's widest capitals plus the tracking, so no label is clipped. The words the screens add too.
    #[test]
    fn every_label_fits_its_column() {
        let content = data::base_content();
        let (card, s) = (&frame().card, &frame().stats);
        let column = (card.width - 2.0 * card.pad - 2.0 - s.col_gap) / 2.0;
        let width = |label: &str| label.chars().count() as f32 * s.label * (0.62 + s.label_em);
        let granted = Granted { unlocks: data::career::Unlocks::all(&content), ..default() };
        let mut labels: Vec<String> = ["PEAK HEAT", "POWER LEVEL"].map(String::from).to_vec();
        for part in content.parts.iter() {
            let materials: Vec<Option<&Material>> = if part.hull { content.materials.iter().filter(|m| m.armor.is_some()).map(Some).collect() } else { vec![None] };
            for m in materials {
                labels.extend(part_info(&content, &granted, part, m, (0, Held::Design)).stats.into_iter().filter(|l| !l.wide).map(|l| l.label));
            }
        }
        let longest = labels.iter().max_by(|a, b| width(a).total_cmp(&width(b))).unwrap();
        assert!(width(longest) <= column, "{longest}: {} px in a {column} px column", width(longest));
    }

    /// A part's lines: its stats from content, then its recipe and stock across both columns; a locked one names its
    /// research.
    #[test]
    fn a_part_tells_its_stats_and_recipe() {
        let content = data::base_content();
        let career = data::Career::new(&content, "rim_awakening", 1).unwrap();
        let granted = Granted { unlocks: career.unlocks(&content), ..default() };
        let reactor = content.parts.iter().find(|p| p.power.as_ref().is_some_and(|x| x.output > 0.0) && !p.hull).unwrap();
        let info = part_info(&content, &granted, reactor, None, (0, Held::Design));
        assert!(info.stats.iter().any(|l| l.label == "POWER OUTPUT" && !l.wide));
        let tail: Vec<(&str, bool)> = info.stats.iter().rev().take(2).map(|l| (l.label.as_str(), l.wide)).collect();
        assert_eq!(tail, [("IN STOCK", true), ("RECIPE", true)]);
        let beam = content.parts.get("beam_laser").unwrap();
        assert!(part_info(&content, &granted, beam, None, (0, Held::Fleet)).sub.starts_with("LOCKED"));
    }
}
