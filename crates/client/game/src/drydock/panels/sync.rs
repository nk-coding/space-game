//! The screen follows the state (`DockSet::Show`): the search, the pills, the palette and its chips, the armour
//! swatches, the bottom bar folding to fit, the menus, the selected part's blocks, the texts, the live line.

use super::*;

/// The search field and [`DrydockUi::search`] stay equal: typing writes the resource, other systems may too.
pub(in crate::drydock) fn sync_search(mut ui: ResMut<DrydockUi>, mut field: Single<&mut TextField, With<SearchField>>) {
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
pub(in crate::drydock) fn sync_pills(
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
                set_display(&mut node, crate::drydock::state::material_shown(&content.0, unlocks, m));
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
                set_display(&mut node, crate::drydock::state::material_shown(&content.0, unlocks, m));
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

/// The part list: its entries (sections of the search's parts, the locked ones last in each), which show from its
/// scroll (the kit's wheel and scrollbar move it; a chip glides it to its section); the lit chip; the slots'
/// headings and rows, the rows' icons (a lock for a locked part) and NEW tags.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(in crate::drydock) fn sync_palette(
    mut ui: ResMut<DrydockUi>,
    content: Res<GameContent>,
    granted: Res<Granted>,
    style: Res<Style>,
    mut pal: ResMut<Palette>,
    list: Single<(&ComputedNode, &mut ScrollRows, &mut SmoothScroll), With<PaletteList>>,
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
    let (node, mut bar, mut smooth) = list.into_inner();
    if let Some(made_from) = made_from {
        let mut entries = Vec::new();
        for s in palette_sections(content, &granted.unlocks, &made_from.0) {
            entries.push(Entry::Head { category: s.category, open: s.parts.len() - s.locked, locked: s.locked });
            entries.extend(s.parts.into_iter().map(Entry::Part));
        }
        // Another search starts at the top; a new material only recolours the list.
        if pal.made_from.as_ref().is_none_or(|m| m.0 != made_from.0) {
            bar.scroll = 0.0;
            pal.jumped = None;
        }
        pal.entries = entries;
        pal.made_from = Some(made_from);
    }
    let lengths: Vec<f32> = pal.entries.iter().map(Entry::height).collect();
    let mut jumped = pal.jumped.clone();
    let jump = if ui.jump.is_some() { ui.jump.take() } else { None };
    if let Some(cat) = jump
        && let Some(i) = pal.entries.iter().position(|e| matches!(e, Entry::Head { category, .. } if *category == cat))
    {
        // Its heading's top: the entries above it and their gaps.
        smooth.to(lengths[..i].iter().sum::<f32>() + ROW_GAP * i as f32);
        jumped = Some(cat);
    }
    let w = ScrollRows::lay(&mut bar, &lengths, ROW_GAP, node.size.y * node.inverse_scale_factor, SLOTS);
    let (offset, visible, (top, end)) = (w.first, w.count.min(SLOTS), w.seen);
    // The lit chip: the one clicked while the list glides to it and while its heading is in view, else the section
    // at the top.
    let in_view = |cat: &str| pal.entries[top..end].iter().any(|e| matches!(e, Entry::Head { category, .. } if category == cat));
    let jumped = jumped.filter(|c| smooth.gliding() || in_view(c));
    let lit = jumped.clone().or_else(|| {
        pal.entries[..(top + 1).min(pal.entries.len())].iter().rev().find_map(|e| match e {
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
        // A locked row keeps its part id: `sync_pills` disables it, so a click opens its research (`actions::clicks`).
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

/// The scrim's hole is the grid viewport as laid out (UI px from the scrim's edges), so it follows any layout change
/// and any UI scale instead of copying the grid's numbers (Niklas, 14 Oct 2026: "Read rects, not constants").
pub(in crate::drydock) fn sync_scrim(
    viewport: Single<(&ComputedNode, &UiGlobalTransform), With<GridViewport>>,
    scrim: Single<(&ComputedNode, &UiGlobalTransform, &mut UiIcon), With<GridScrim>>,
) {
    let (vp, vp_at) = *viewport;
    let (node, at, mut icon) = scrim.into_inner();
    let k = node.inverse_scale_factor;
    let (outer, inner) = (Rect::from_center_size(at.translation, node.size), Rect::from_center_size(vp_at.translation, vp.size));
    let want = [inner.min.x - outer.min.x, inner.min.y - outer.min.y, outer.max.x - inner.max.x, outer.max.y - inner.max.y].map(|v| (v * k).round());
    if let UiIcon::Scrim { hole, .. } = &*icon
        && *hole != want
        && let UiIcon::Scrim { hole, .. } = &mut *icon
    {
        *hole = want;
    }
}

/// The armour swatches: a locked material's tile is a lock (the strip scrolls through the kit's [`Scrollbar`]).
pub(in crate::drydock) fn sync_swatches(granted: Res<Granted>, style: Res<Style>, content: Res<GameContent>, mut tiles: Query<(&SwatchTile, &mut UiIcon)>) {
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
}

/// The category chips' line: a drag over it scrolls it sideways (at once; the wheel is the kit's), a lit chip
/// glides into view, and the kit's fades show over an edge with more chips behind it.
#[allow(clippy::type_complexity)]
pub(in crate::drydock) fn scroll_chips(
    pal: Res<Palette>,
    window: Single<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    line: Single<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition, &mut SmoothScroll, &Children), With<ChipLine>>,
    chips: Query<(&ComputedNode, &UiGlobalTransform, &ActionButton)>,
    mut drag: Local<Option<(f32, f32)>>,
) {
    let (node, tf, mut pos, mut smooth, children) = line.into_inner();
    let k = node.inverse_scale_factor;
    let max = ((node.content_size.x - node.size.x) * k).max(0.0);
    let cursor = window.physical_cursor_position();
    let over = cursor.is_some_and(|c| node.contains_point(*tf, c));
    let x = pos.0.x;
    if buttons.just_pressed(MouseButton::Left) && over {
        *drag = cursor.map(|c| (c.x, x));
    }
    if !buttons.pressed(MouseButton::Left) {
        *drag = None;
    }
    if let (Some((from, start)), Some(c)) = (*drag, cursor) {
        let x = (start - (c.x - from) * k).clamp(0.0, max);
        if pos.0.x != x {
            pos.0.x = x;
        }
    } else if pal.is_changed()
        && let Some(lit) = &pal.lit
        && let Some((chip, at, _)) = children.iter().filter_map(|e| chips.get(e).ok()).find(|(_, _, b)| b.0 == DrydockAction::Category(lit.clone()))
    {
        // The lit chip into view (positions in the line's content, logical px).
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

/// Folds the bottom bar's tools into their dropdown when the status line would get less than [`MIN_STATUS`] px,
/// and unfolds them when there is room again (from last frame's layout; the wide tools' width is remembered while
/// they are folded).
#[allow(clippy::type_complexity)]
pub(in crate::drydock) fn fit_bar(
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
pub(in crate::drydock) fn fit_status(
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
pub(super) const SEP: &str = "  ·  ";

/// A status line shortened to about `share` of its characters: its first parts and `…`, else its first characters
/// and `…`.
pub(super) fn shorten(line: &str, share: f32) -> String {
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
pub(in crate::drydock) fn sync_menus(ui: Res<DrydockUi>, mut plates: Query<(&MenuPlate, &mut Node), Without<MenuVeil>>, mut veil: Single<&mut Node, With<MenuVeil>>) {
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
pub(in crate::drydock) fn see_fresh(insp: Res<Inspector>, granted: Res<Granted>, mut out: MessageWriter<SeenPart>) {
    if insp.is_changed() && granted.fresh.contains(&insp.part) {
        out.write(SeenPart(insp.part.clone()));
    }
}

/// What a locked palette row says under its name: `LOCKED · <the research node that unlocks it>`.
pub(super) fn locked_sub(content: &Content, part: &Part) -> String {
    match unlocked_by(content, &Unlock::Part(part.id.clone())) {
        Some(node) => format!("LOCKED  ·  {}", node.name.to_uppercase()),
        None => "LOCKED".into(),
    }
}

/// A part's icon: hull parts in the chosen armour.
pub(crate) fn part_icon(content: &Content, part: &str, material: &str) -> UiIcon {
    let hull = content.parts.get(part).is_some_and(|p| p.hull);
    UiIcon::Part { part: part.to_string(), material: hull.then(|| material.to_string()) }
}

/// Another kind of part (or a palette part instead of a placed one) shows its lines from the top (they scroll
/// through the kit's [`Scrollbar`]).
pub(in crate::drydock) fn scroll_selected(insp: Res<Inspector>, mut area: Single<&mut ScrollPosition, With<SelScroll>>, mut shown: Local<(String, bool)>) {
    let kind = (insp.part.clone(), insp.installed.is_some());
    if insp.is_changed() && *shown != kind {
        *shown = kind;
        area.0.y = 0.0;
    }
}

/// The selected part's icon, its summary (none for a placed part), its stat and bonus rows (from [`Inspector`]),
/// the stats grid's bars and cells.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(in crate::drydock) fn sync_blocks(
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
        for (t, mut n) in &mut texts {
            if let PanelText::StatNote(k) = *t {
                set_display(&mut n, stats.cells.get(k).is_some_and(|c| c.note.is_some()));
            }
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

pub(super) fn set_display(n: &mut Node, show: bool) {
    let d = if show { Display::Flex } else { Display::None };
    if n.display != d {
        n.display = d;
    }
}

/// The one stat a palette row shows: the part's first own stat after the common ones (build points, control,
/// mass, hit points, power draw, heat), else the bonus it gives, else its hit points.
pub(super) fn key_stat(content: &Content, part: &Part, material: &str) -> Option<String> {
    const COMMON: [&str; 8] = ["build_points", "control_cost", "mass", "hp", "power_draw", "power_min_level", "heat_output", "heat_limit"];
    let lines = content.part_stats(part, content.materials.get(material).filter(|_| part.hull));
    let pick = lines
        .iter()
        .find(|l| l.bonus.is_none() && !COMMON.contains(&l.stat.as_str()))
        .or_else(|| lines.iter().find(|l| l.bonus.is_some()))
        .or_else(|| lines.iter().find(|l| l.stat == "hp"))?;
    Some(format!("{} {}", pick.label, pick.text).to_uppercase())
}

pub(super) fn footprint(part: &Part) -> String {
    format!("{}×{}", part.size.0, part.size.1)
}

/// Every text that changes only with the drydock's state.
#[allow(clippy::too_many_arguments)]
pub(in crate::drydock) fn sync_texts(
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
                s.push_str(&format!("SHIPYARD  ›  DRYDOCK  ·  {}", crate::drydock::blueprints::frame_name(content, design.frame())));
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
            // The workshop's stock less what the design holds (GDR-0108): coral when the design is short of it,
            // dim when none is left; hull plating is not counted.
            PanelText::RowCount(k) => match row_part(k).filter(|p| granted.unlocks.has_part(&p.id)) {
                Some(p) => match granted.in_stock(&p.id).filter(|_| !p.hull) {
                    None => {
                        s.push('∞');
                        Tone::Label
                    }
                    Some(n) => {
                        let used = design.dock().parts().iter().filter(|x| x.part == p.id).count() as i64;
                        let left = i64::from(n) - used;
                        let _ = write!(s, "{left}");
                        match left {
                            ..0 => Tone::Coral,
                            0 => Tone::Dim,
                            _ => Tone::Label,
                        }
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
            PanelText::StatNote(k) => {
                s.push_str(stats.cells.get(k).and_then(|c| c.note.as_deref()).unwrap_or_default());
                Tone::Label
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
pub(in crate::drydock) fn sync_live(
    design: Res<PlayerDesign>,
    cursor: Res<DrydockCursor>,
    rig: Res<CameraRig>,
    cam: Res<DockCamera>,
    hint: Single<&TextSpan, With<crate::drydock::ghost::HintText>>,
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
