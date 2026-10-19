//! The screen follows the state (`DockSet::Show`): the pills, the palette's rows, the armour swatches, the bottom bar
//! folding to fit, the menus, the selected part's blocks, the texts, the live line.

use super::*;

/// Active and disabled states of the action widgets, from the drydock's state (the mirror and bevel modes are
/// the editor's; the inspector keeps its own pills, but locked armour is dimmed here too). Locked parts,
/// materials and frames are disabled: they send nothing.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(in crate::drydock) fn sync_pills(
    ui: Res<DrydockUi>,
    design: Res<PlayerDesign>,
    status: Res<DrydockStatus>,
    granted: Res<Granted>,
    content: Res<GameContent>,
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

/// The palette's rows anew (the kit's grouped list: its search, sections and chips) at the start and when the unlocks
/// change: a section per category, the unlocked parts then the locked ones research can unlock (put aside: counted as
/// LOCKED in the heading).
pub(in crate::drydock) fn fill_palette(
    mut commands: Commands,
    granted: Res<Granted>,
    content: Res<GameContent>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    list: Single<Entity, With<PaletteList>>,
    mut filled: Local<bool>,
) {
    if *filled && !granted.is_changed() {
        return;
    }
    *filled = true;
    let content = &content.0;
    let rows: Vec<ListRow> = palette_sections(content, &granted.unlocks)
        .into_iter()
        .flat_map(|s| {
            let open = s.parts.len() - s.locked;
            let category = s.category;
            s.parts.into_iter().enumerate().map(move |(i, p)| (category.clone(), p, i >= open))
        })
        .map(|(group, key, aside)| ListRow { find: content.parts.get(&key).map_or_else(String::new, |p| p.name.to_lowercase()), key, group, aside })
        .collect();
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    fill_list(&mut commands, *list, &fonts, &ui, &rows, |l, r| super::spawn::spawn_row(l, &fonts, &c, &r.key));
}

/// The palette rows follow the state: the icon (a lock for a locked part; hull parts in the chosen armour), the
/// hover card's armour, the NEW tag, the name, the subtitle (`2×1 · <key stat>`, or `LOCKED · <research node>`) and
/// how many are left.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(in crate::drydock) fn sync_rows(
    ui: Res<DrydockUi>,
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    granted: Res<Granted>,
    style: Res<Style>,
    added: Query<(), Added<PaletteRow>>,
    mut hovers: Query<(&PaletteRow, &mut PartHover)>,
    mut icons: Query<(&RowIcon, &mut UiIcon)>,
    mut tags: Query<(&RowNew, &mut Node)>,
    mut texts: Query<(&RowText, &mut Text, &mut TextColor)>,
) {
    if !(ui.is_changed() || design.is_changed() || granted.is_changed() || !added.is_empty()) {
        return;
    }
    let content = &content.0;
    let open = |p: &str| granted.unlocks.has_part(p);
    for (row, mut hover) in &mut hovers {
        let material = content.parts.get(&row.0).filter(|p| p.hull).map(|_| ui.material.clone());
        if hover.material != material {
            hover.material = material;
        }
    }
    for (row, mut icon) in &mut icons {
        let want = if open(&row.0) { part_icon(content, &row.0, &ui.material) } else { UiIcon::Glyph { glyph: Glyph::Lock, color: style.ui.dim } };
        if *icon != want {
            *icon = want;
        }
    }
    for (tag, mut n) in &mut tags {
        set_display(&mut n, granted.fresh.contains(&tag.0));
    }
    let mut s = String::new();
    for (t, mut text, mut color) in &mut texts {
        s.clear();
        let (RowText::Name(id) | RowText::Sub(id) | RowText::Count(id)) = t;
        let Some(p) = content.parts.get(id) else { continue };
        let tone = match t {
            RowText::Name(_) => {
                s.push_str(&p.name);
                if open(id) { Tone::Number } else { Tone::Dim }
            }
            RowText::Sub(_) => {
                let mut upgraded = false;
                if open(id) {
                    s.push_str(&footprint(p));
                    if let Some((stat, raised)) = key_stat(content, p, &ui.material, &granted.upgrades) {
                        s.push_str(" · ");
                        s.push_str(&stat);
                        upgraded = raised;
                    }
                } else {
                    s.push_str(&locked_sub(content, p));
                }
                // A stat the career's steps raised reads in teal, like the hover card's.
                if upgraded { Tone::Teal } else { sub_tone(ui.picked.as_deref() == Some(id.as_str())) }
            }
            // The workshop's stock less what the design holds (GDR-0108): coral when the design is short of it, dim
            // when none is left; hull plating is not counted.
            RowText::Count(_) if !open(id) => Tone::Label,
            RowText::Count(_) => match granted.in_stock(id).filter(|_| !p.hull) {
                None => {
                    s.push('∞');
                    Tone::Label
                }
                Some(n) => {
                    let used = design.dock().parts().iter().filter(|x| x.part == *id).count() as i64;
                    let left = i64::from(n) - used;
                    let _ = write!(s, "{left}");
                    match left {
                        ..0 => Tone::Coral,
                        0 => Tone::Dim,
                        _ => Tone::Label,
                    }
                }
            },
        };
        set_text(&mut text, &s);
        set_color(&mut color, ui_color(tone.color(&style.ui), 1.0));
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

/// Folds the bottom bar's tools into their dropdown when the status line would get less than [`MIN_STATUS`] px,
/// and unfolds them when there is room again (from last frame's layout; the wide tools' width is remembered while
/// they are folded).
#[allow(clippy::type_complexity)]
pub(in crate::drydock) fn fit_bar(
    class: Res<LayoutClass>,
    area: Single<&ComputedNode, With<StatusArea>>,
    mut wide: Single<(&ComputedNode, &mut Node), (With<WideTools>, Without<FoldedTools>)>,
    mut folded: Single<(&ComputedNode, &mut Node), (With<FoldedTools>, Without<WideTools>)>,
    mut ui: ResMut<DrydockUi>,
    mut wide_w: Local<f32>,
) {
    // On a phone the tools are in their strip: the bar keeps only the folded mirror and bevel (its dropdown hidden).
    if class.is_compact() {
        set_display(&mut wide.1, false);
        set_display(&mut folded.1, true);
        if ui.menu == Some(BarMenu::Tools) {
            ui.menu = None;
        }
        return;
    }
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

/// The selected part's icon, its summary (none for a placed part), its stat block and bonus rows (from
/// [`Inspector`]), the stats grid's bars and cells.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(in crate::drydock) fn sync_blocks(
    insp: Res<Inspector>,
    stats: Res<DrydockStats>,
    mut sel_icon: Single<&mut UiIcon, With<SelIcon>>,
    mut sel_stats: Single<&mut StatBlock, With<SelStats>>,
    mut texts: Query<(&PanelText, &mut Node), (Without<SelBonus>, Without<StatCellNode>, Without<StatBar>)>,
    mut sel_bonuses: Query<(&SelBonus, &mut Node), Without<StatCellNode>>,
    mut cells: Query<(&StatCellNode, &mut Node), Without<SelBonus>>,
    mut bars: Query<(&StatBar, &mut ValueBar, &mut Node), (Without<SelBonus>, Without<StatCellNode>)>,
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
        if sel_stats.0 != insp.stats {
            sel_stats.0.clone_from(&insp.stats);
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

/// The right column on a phone: the stats, or (a placed part selected) the selected part; both in a wide window.
pub(in crate::drydock) fn sync_column(class: Res<LayoutClass>, sel: Res<Selection>, mut parts: Query<(&ColumnPart, &mut Node)>) {
    let selected = sel.cell.is_some();
    for (part, mut n) in &mut parts {
        let show = !class.is_compact() || (*part == ColumnPart::Selected) == selected;
        set_display(&mut n, show);
    }
}

pub(super) fn set_display(n: &mut Node, show: bool) {
    let d = if show { Display::Flex } else { Display::None };
    if n.display != d {
        n.display = d;
    }
}

/// The one stat a palette row shows: the part's first own stat after the common ones (build points, control,
/// mass, hit points, power draw, heat), else the bonus it gives, else its hit points; with the career's part-stat
/// steps (as the hover card shows it) and whether they raised it.
pub(super) fn key_stat(content: &Content, part: &Part, material: &str, upgrades: &data::Upgrades) -> Option<(String, bool)> {
    const COMMON: [&str; 8] = ["build_points", "control_cost", "mass", "hp", "power_draw", "power_min_level", "heat_output", "heat_limit"];
    let lines = content.part_stats_upgraded(part, content.materials.get(material).filter(|_| part.hull), upgrades);
    let pick = lines
        .iter()
        .find(|l| l.bonus.is_none() && !COMMON.contains(&l.stat.as_str()))
        .or_else(|| lines.iter().find(|l| l.bonus.is_some()))
        .or_else(|| lines.iter().find(|l| l.stat == "hp"))?;
    Some((format!("{} {}", pick.label, pick.text).to_uppercase(), pick.upgraded_from.is_some()))
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
    content: Res<GameContent>,
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
    {
        return;
    }
    *done = true;
    let content = &content.0;
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

/// The info line (hover cell, mirror, axis, and the pan help unless the coral hint after it needs the room; on a
/// phone the cell and the mirror) and the zoom, every frame.
pub(in crate::drydock) fn sync_live(
    design: Res<PlayerDesign>,
    cursor: Res<DrydockCursor>,
    rig: Res<CameraRig>,
    cam: Res<DockCamera>,
    hint: Single<&TextSpan, With<crate::drydock::ghost::HintText>>,
    class: Res<LayoutClass>,
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
                // A phone's grid is narrower and has no Space key: the cell and the mirror only.
                if class.is_compact() {
                    let _ = write!(s, "  ·  MIRROR {mirror}");
                } else {
                    let _ = write!(s, "  ·  MIRROR {mirror}  ·  AXIS {}", design.frame().0 as f32 / 2.0);
                    if hint.0.is_empty() {
                        s.push_str("  ·  SPACE + DRAG TO PAN");
                    }
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
