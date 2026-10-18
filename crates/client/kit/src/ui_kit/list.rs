//! A grouped list ([`GroupedList`]; Niklas, 18 Oct 2026: the workshop's parts list wants the drydock palette's search
//! and sections, from one component): a search field, one line of group chips that scrolls sideways, and the rows in
//! sections under a heading per group, in an area that scrolls. The screen supplies the rows; the kit does the rest.
//!
//! * **Spawn** it with [`spawn_grouped_list`] into a column: the search field (the screen's components on it, such as a
//!   marker and its tooltip), whatever the screen puts between the field and the chips, the chips, the scrolling area.
//!   It returns the area, which holds the [`GroupedList`].
//! * **Rows**: [`fill_list`] spawns them anew (rows sorted by group, in the groups' order): a heading before each
//!   group's first row, then each row as the screen's closure spawns it; the kit puts the [`ListRow`] on it (its key,
//!   group, the lowercase text the search looks in, and whether it is put aside: the drydock's locked parts, counted
//!   apart in the heading). A screen whose rows change keeps them and rewrites their insides itself; it fills the list
//!   again when the set of rows changes.
//! * **Search**: a row shows when its text contains the field's text (case-insensitive); a heading and its chip show
//!   while their group has a row shown, the heading with the count (`4  ·  2 LOCKED`). Another search starts at the
//!   top.
//! * **Chips**: a click glides the list to its group's heading; the lit chip is the group at the top of the view (the
//!   clicked one while the list glides to it and while its heading is in view). The line scrolls sideways by wheel and
//!   by drag, and the lit chip glides into view.
//! * **Icons** in the rows are cut where the list cuts its rows (`ui_icons`: a clipping parent cuts them too).

use super::*;
use super::widgets::{Pill, spawn_pill};

/// What a grouped list is spawned with.
#[derive(Clone, Debug)]
pub struct ListSpec {
    /// The search field's placeholder and its longest text.
    pub placeholder: String,
    pub max_chars: usize,
    /// The groups in order: id and name (the chips and headings show it uppercase).
    pub groups: Vec<(String, String)>,
    /// The word after a heading's count of rows put aside (`2 LOCKED`).
    pub aside: &'static str,
    /// The gap between rows, and the room the area keeps at its right for its scrollbar.
    pub row_gap: f32,
    pub pad_right: f32,
}

/// A grouped list's state, on its scrolling area: its groups, the search it shows, the chip clicked last and the lit
/// one.
#[derive(Component, Debug, Default)]
pub struct GroupedList {
    groups: Vec<(String, String)>,
    aside: &'static str,
    /// The search the rows show (lowercase); `None` before the first pass.
    query: Option<String>,
    jumped: Option<String>,
    /// The lit chip's group: the section in view.
    pub lit: Option<String>,
}

/// A row of a grouped list (the kit puts it on the node the screen spawns, [`fill_list`]).
#[derive(Component, Clone, Debug, PartialEq, Eq, Default)]
pub struct ListRow {
    /// The screen's id of the row (a part id).
    pub key: String,
    pub group: String,
    /// What the search looks in, lowercase (a part's name).
    pub find: String,
    /// Put aside: counted apart in its heading (`2 LOCKED`).
    pub aside: bool,
}

/// The search field of the list (its area).
#[derive(Component)]
struct ListSearch(Entity);

/// The chips' line of the list, and a chip of a group.
#[derive(Component)]
struct ChipLine(Entity);

#[derive(Component)]
struct ListChip {
    list: Entity,
    group: String,
}

/// A group's heading in the list, and its texts.
#[derive(Component)]
struct ListHead(String);

#[derive(Component)]
enum HeadText {
    Name,
    Count,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (chip_clicks, filter, light, scroll_chips).chain().after(UiInputSet).before(UiStyleSet));
}

/// Spawns a grouped list into `parent` (a column): the search field with `search` on it, `between`, the chips, the
/// scrolling area. Returns the area (it holds the [`GroupedList`]; fill it with [`fill_list`]).
pub fn spawn_grouped_list(
    parent: &mut ChildSpawnerCommands,
    fonts: &HudFonts,
    ui: &UiPalette,
    spec: ListSpec,
    search: impl Bundle,
    between: impl FnOnce(&mut ChildSpawnerCommands),
) -> Entity {
    let c = |tone: Tone, a: f32| ui_color(tone.color(ui), a);
    // The area's entity first, so the field and the chips know it; it is put in after them.
    let area = parent.commands().spawn_empty().id();
    parent
        .spawn((
            ListSearch(area),
            TextField { placeholder: spec.placeholder.clone(), max_chars: spec.max_chars, ..default() },
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
            search,
        ))
        .with_children(|f| {
            f.spawn((UiIcon::Glyph { glyph: Glyph::Search, color: ui.label }, Node { width: px(14), height: px(14), flex_shrink: 0.0, ..default() }));
            f.spawn((field_text(fonts, 12.5), Node { flex_grow: 1.0, ..default() }));
        });
    between(parent);
    parent.spawn(Node { width: percent(100), height: px(24), flex_shrink: 0.0, ..default() }).with_children(|w| {
        w.spawn((
            ChipLine(area),
            ScrollPosition::default(),
            Scrollbar::horizontal().outside(),
            Node { flex_direction: FlexDirection::Row, column_gap: px(6), width: percent(100), height: percent(100), overflow: Overflow::scroll_x(), ..default() },
        ))
        .with_children(|line| {
            for (id, name) in &spec.groups {
                spawn_pill(line, fonts, &name.to_uppercase(), 8.5, 24.0, None, Pill::new(Tone::Teal), ListChip { list: area, group: id.clone() });
            }
        });
    });
    let column = parent.target_entity();
    parent.commands().entity(area).insert((
        GroupedList { groups: spec.groups, aside: spec.aside, ..default() },
        Scrollbar::vertical(),
        ScrollPosition::default(),
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: px(0),
            min_height: px(0),
            row_gap: px(spec.row_gap),
            padding: UiRect::right(px(spec.pad_right)),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ChildOf(column),
    ));
    area
}

/// The list's rows anew: a heading before each group's first row, then each row as `row` spawns it (it returns the
/// row's node; the kit puts its [`ListRow`] on it). `rows` come sorted by group, in the groups' order.
pub fn fill_list(commands: &mut Commands, list: Entity, fonts: &HudFonts, ui: &UiPalette, rows: &[ListRow], mut row: impl FnMut(&mut ChildSpawnerCommands, &ListRow) -> Entity) {
    let c = |tone: Tone| ui_color(tone.color(ui), 1.0);
    commands.entity(list).despawn_children();
    commands.entity(list).with_children(|l| {
        let mut group: Option<&str> = None;
        for r in rows {
            if group != Some(r.group.as_str()) {
                group = Some(r.group.as_str());
                l.spawn((
                    ListHead(r.group.clone()),
                    Node {
                        height: px(26),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::FlexEnd,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::new(px(6), px(12), px(0), px(5)),
                        ..default()
                    },
                ))
                .with_children(|h| {
                    h.spawn((HeadText::Name, label(fonts, "", 9.0, c(Tone::Label))));
                    h.spawn((HeadText::Count, label(fonts, "", 8.5, c(Tone::Dim))));
                });
            }
            let e = row(l, r);
            l.commands().entity(e).insert(r.clone());
        }
    });
}

/// Does a row's text hold the search (lowercase)?
pub fn row_shown(find: &str, query: &str) -> bool {
    find.contains(query)
}

/// A heading's count: the rows shown, and those put aside after them (`4  ·  2 LOCKED`).
pub fn head_count(shown: usize, aside: usize, word: &str) -> String {
    if aside > 0 { format!("{shown}  ·  {aside} {word}") } else { shown.to_string() }
}

/// The lit group from the shown headings' tops in the view (UI px, from the view's top, in list order) and the view's
/// height: the clicked group while the list glides to it or its heading is in view, else the last heading at or
/// above the top, else the first.
pub fn lit_group(heads: &[(String, f32)], jumped: Option<&str>, gliding: bool, view: f32) -> (Option<String>, Option<String>) {
    let jumped = jumped.filter(|j| gliding || heads.iter().any(|(g, top)| g == j && *top >= -1.0 && *top < view)).map(str::to_string);
    let at_top = heads.iter().rev().find(|(_, top)| *top <= 1.0).or(heads.first()).map(|(g, _)| g.clone());
    (jumped.clone().or(at_top), jumped)
}

/// A chip's click glides its list to the group's heading.
fn chip_clicks(
    mut clicks: MessageReader<UiClick>,
    chips: Query<&ListChip>,
    mut lists: Query<(&mut GroupedList, &ComputedNode, &UiGlobalTransform, &ScrollPosition, &mut SmoothScroll, &Children)>,
    heads: Query<(&ListHead, &ComputedNode, &UiGlobalTransform)>,
) {
    for c in clicks.read() {
        let Ok(chip) = chips.get(c.0) else { continue };
        let Ok((mut list, node, at, pos, mut smooth, kids)) = lists.get_mut(chip.list) else { continue };
        let Some((_, head, head_at)) = kids.iter().filter_map(|e| heads.get(e).ok()).find(|(h, ..)| h.0 == chip.group) else { continue };
        smooth.to(top_in(node, at, head, head_at) + pos.0.y);
        list.jumped = Some(chip.group.clone());
    }
}

/// A node's top below its list's top (UI px).
fn top_in(list: &ComputedNode, list_at: &UiGlobalTransform, node: &ComputedNode, at: &UiGlobalTransform) -> f32 {
    ((at.translation.y - node.size.y / 2.0) - (list_at.translation.y - list.size.y / 2.0)) * list.inverse_scale_factor
}

/// The rows the search shows, the headings with their names and counts, the chips of the groups with rows shown;
/// another search starts at the top. Only when the search or the rows change.
#[allow(clippy::type_complexity)]
fn filter(
    fields: Query<(&ListSearch, &TextField)>,
    mut lists: Query<(Entity, &mut GroupedList, &mut ScrollPosition, &Children)>,
    added: Query<&ChildOf, Or<(Added<ListRow>, Changed<ListRow>, Added<ListHead>)>>,
    mut rows: Query<(&ListRow, &mut Node), Without<ListHead>>,
    mut heads: Query<(&ListHead, &mut Node, &Children), Without<ListRow>>,
    mut texts: Query<(&HeadText, &mut Text)>,
    mut chips: Query<(&ListChip, &mut Node), (Without<ListRow>, Without<ListHead>)>,
) {
    for (search, field) in &fields {
        let Ok((e, mut list, mut pos, kids)) = lists.get_mut(search.0) else { continue };
        let query = field.value.trim().to_lowercase();
        let searched = list.query.as_ref() != Some(&query);
        if !searched && !added.iter().any(|p| p.parent() == e) {
            continue;
        }
        if searched {
            // Not the first pass: a new search starts at the top.
            if list.query.is_some() && pos.0.y != 0.0 {
                pos.0.y = 0.0;
            }
            list.jumped = None;
            list.query = Some(query.clone());
        }
        let mut counts: Vec<(String, usize, usize)> = Vec::new();
        for k in kids.iter() {
            let Ok((r, mut n)) = rows.get_mut(k) else { continue };
            let show = row_shown(&r.find, &query);
            set_display(&mut n, show);
            if !show {
                continue;
            }
            match counts.iter_mut().find(|(g, ..)| *g == r.group) {
                Some((_, open, aside)) => *if r.aside { aside } else { open } += 1,
                None => counts.push((r.group.clone(), usize::from(!r.aside), usize::from(r.aside))),
            }
        }
        let count = |g: &str| counts.iter().find(|(c, ..)| c == g).map(|(_, o, a)| (*o, *a));
        for k in kids.iter() {
            let Ok((h, mut n, head_kids)) = heads.get_mut(k) else { continue };
            let c = count(&h.0);
            set_display(&mut n, c.is_some());
            let name = list.groups.iter().find(|(id, _)| *id == h.0).map_or_else(|| h.0.to_uppercase(), |(_, name)| name.to_uppercase());
            for t in head_kids.iter() {
                if let Ok((which, mut text)) = texts.get_mut(t) {
                    match which {
                        HeadText::Name => set_text(&mut text, &name),
                        HeadText::Count => set_text(&mut text, &c.map_or(String::new(), |(o, a)| head_count(o, a, list.aside))),
                    }
                }
            }
        }
        for (chip, mut n) in &mut chips {
            if chip.list == e {
                set_display(&mut n, count(&chip.group).is_some());
            }
        }
    }
}

/// The lit chip: the group at the top of the view, or the clicked one while the list glides there (every frame, from
/// the laid-out headings).
#[allow(clippy::type_complexity)]
fn light(
    mut lists: Query<(Entity, &mut GroupedList, &ComputedNode, &UiGlobalTransform, &SmoothScroll, &Children)>,
    heads: Query<(&ListHead, &Node, &ComputedNode, &UiGlobalTransform)>,
    mut chips: Query<(&ListChip, &mut Pill)>,
) {
    for (e, mut list, node, at, smooth, kids) in &mut lists {
        let tops: Vec<(String, f32)> = kids
            .iter()
            .filter_map(|k| heads.get(k).ok())
            .filter(|(_, n, ..)| n.display != Display::None)
            .map(|(h, _, hn, hat)| (h.0.clone(), top_in(node, at, hn, hat)))
            .collect();
        let (lit, jumped) = lit_group(&tops, list.jumped.as_deref(), smooth.gliding(), node.size.y * node.inverse_scale_factor);
        if list.lit != lit || list.jumped != jumped {
            list.lit = lit;
            list.jumped = jumped;
        }
        for (chip, mut pill) in &mut chips {
            let active = chip.list == e && list.lit.as_deref() == Some(chip.group.as_str());
            if chip.list == e && pill.active != active {
                pill.active = active;
            }
        }
    }
}

/// The chips' line: a drag over it scrolls it sideways (at once; the wheel is the kit's), the lit chip glides into
/// view, and the kit's fades show over an edge with more chips behind it.
#[allow(clippy::type_complexity)]
fn scroll_chips(
    lists: Query<Ref<GroupedList>>,
    window: Single<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut lines: Query<(Entity, &ChipLine, &ComputedNode, &UiGlobalTransform, &mut ScrollPosition, &mut SmoothScroll, &Children)>,
    chips: Query<(&ComputedNode, &UiGlobalTransform, &ListChip)>,
    mut drag: Local<Option<(Entity, f32, f32)>>,
) {
    let cursor = window.physical_cursor_position();
    if !buttons.pressed(MouseButton::Left) {
        *drag = None;
    }
    for (e, line, node, tf, mut pos, mut smooth, children) in &mut lines {
        let k = node.inverse_scale_factor;
        let max = ((node.content_size.x - node.size.x) * k).max(0.0);
        let x = pos.0.x;
        if buttons.just_pressed(MouseButton::Left)
            && let Some(c) = cursor.filter(|c| node.contains_point(*tf, *c))
        {
            *drag = Some((e, c.x, x));
        }
        let Ok(list) = lists.get(line.0) else { continue };
        if let (Some((on, from, start)), Some(c)) = (*drag, cursor)
            && on == e
        {
            let x = (start - (c.x - from) * k).clamp(0.0, max);
            if pos.0.x != x {
                pos.0.x = x;
            }
        } else if list.is_changed()
            && let Some(lit) = &list.lit
            && let Some((chip, at, _)) = children.iter().filter_map(|c| chips.get(c).ok()).find(|(_, _, c)| c.group == *lit)
        {
            // The lit chip into view (positions in the line's content, UI px).
            let left = ((at.translation.x - chip.size.x / 2.0) - (tf.translation.x - node.size.x / 2.0)) * k + x;
            let right = left + chip.size.x * k;
            let width = node.size.x * k;
            if left < x {
                smooth.to(left);
            } else if right > x + width {
                smooth.to(right - width);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The search looks in a row's lowercase text; a heading counts what is put aside apart.
    #[test]
    fn search_and_counts() {
        assert!(row_shown("pulse laser", "laser") && row_shown("pulse laser", "") && !row_shown("thruster", "laser"));
        assert_eq!(head_count(4, 0, "LOCKED"), "4");
        assert_eq!(head_count(1, 2, "LOCKED"), "1  ·  2 LOCKED");
    }

    /// The lit chip: the last heading at or above the view's top, the first one when none is; the clicked one while
    /// the list glides there and while its heading is in view, then the one at the top again.
    #[test]
    fn the_lit_chip_follows_the_view() {
        let heads = |tops: [f32; 3]| -> Vec<(String, f32)> { ["hull", "power", "weapons"].iter().zip(tops).map(|(g, t)| (g.to_string(), t)).collect() };
        assert_eq!(lit_group(&heads([0.0, 200.0, 400.0]), None, false, 500.0), (Some("hull".into()), None));
        assert_eq!(lit_group(&heads([-300.0, -40.0, 120.0]), None, false, 500.0), (Some("power".into()), None));
        assert_eq!(lit_group(&heads([20.0, 200.0, 400.0]), None, false, 500.0).0.as_deref(), Some("hull"));
        // Clicked: lit while gliding, and after it while its heading shows (the list may not reach it to the top).
        assert_eq!(lit_group(&heads([0.0, 200.0, 400.0]), Some("weapons"), true, 500.0), (Some("weapons".into()), Some("weapons".into())));
        assert_eq!(lit_group(&heads([-300.0, -100.0, 300.0]), Some("weapons"), false, 500.0).0.as_deref(), Some("weapons"));
        assert_eq!(lit_group(&heads([-900.0, -700.0, -500.0]), Some("hull"), false, 500.0), (Some("weapons".into()), None));
        assert_eq!(lit_group(&[], None, false, 500.0), (None, None));
    }
}
