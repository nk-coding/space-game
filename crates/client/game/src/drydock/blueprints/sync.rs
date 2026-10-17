//! The BLUEPRINTS and EXPORT screens follow the state every frame: the lists scrolled by pixels, rows, buttons, texts.

use super::*;

/// Lays each list out from its scroll (the kit's wheel and scrollbar move it): the first row its pool shows; a list
/// asked to start at the top (a copy saved there) scrolls there.
pub(super) fn scroll_lists(mut lists: Query<(&ListNode, &ComputedNode, &mut ScrollRows)>, mut shelf: ResMut<Shelf>) {
    if shelf.page != Some(Page::Blueprints) {
        return;
    }
    for (list, node, mut rows) in &mut lists {
        let len = if list.0 == 0 { shelf.designs.len() } else { shelf.templates.len() };
        if shelf.top[list.0] {
            shelf.top[list.0] = false;
            rows.scroll = 0.0;
        }
        let w = ScrollRows::lay(&mut rows, &vec![ROW_H; len], ROW_GAP, node.size.y * node.inverse_scale_factor, LIST_ROWS + 1);
        if shelf.offset[list.0] != w.first {
            shelf.offset[list.0] = w.first;
        }
    }
}

/// The screens follow [`Shelf`]: visibility, rows, pills, texts, the boxes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync_shelf(
    shelf: Res<Shelf>,
    lib_slot: Res<DesignSlot>,
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    granted: Res<Granted>,
    style: Res<Style>,
    mut root: Single<&mut Visibility, With<ShelfRoot>>,
    mut covered: Query<&mut Visibility, (With<UnderShelf>, Without<ShelfRoot>)>,
    mut nodes: Query<
        (&mut Node, Option<&PageNode>, Option<&BarNode>, Option<&RowNode>, Has<EmptyHint>),
        (Without<TextField>, Or<(With<PageNode>, With<BarNode>, With<RowNode>, With<EmptyHint>)>),
    >,
    mut pills: Query<(&ShelfButton, &mut Pill)>,
    mut boxes: Query<(&mut BorderColor, &mut BackgroundColor, Has<CodeBox>, &Hot), Or<(With<CodeBox>, With<PasteBox>)>>,
    mut texts: Query<(&ShelfText, &mut Text, &mut TextColor)>,
) {
    let want = if shelf.page.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    if **root != want {
        **root = want;
    }
    let under = if shelf.page.is_some() { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in &mut covered {
        if *v != under {
            *v = under;
        }
    }
    let Some(page) = shelf.page else { return };
    let ui = &style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(ui), a);
    for (mut border, mut bg, code, hot) in &mut boxes {
        let on = if code { shelf.code_selected } else { shelf.paste_armed };
        let (b, f) = match (on, hot.hovered) {
            (true, _) => (c(Tone::Teal, 0.8), c(Tone::Teal, 0.07)),
            (false, true) => (c(Tone::Label, 0.6), Color::NONE),
            (false, false) => (c(Tone::Label, 0.3), Color::NONE),
        };
        if border.top != b {
            *border = BorderColor::all(b);
        }
        if bg.0 != f {
            bg.0 = f;
        }
    }
    if !shelf.is_changed() && !design.is_changed() && !lib_slot.is_changed() && !granted.is_changed() {
        return;
    }
    let content = &content.0;
    let bar = match shelf.mode {
        Mode::Browse => 0,
        Mode::Rename(_) => 1,
        Mode::Delete(_) => 2,
    };
    let row_len = [shelf.designs.len(), shelf.templates.len()];
    for (mut n, p, b, r, empty) in &mut nodes {
        let show = match (p, b, r) {
            (Some(p), ..) => p.0 == page,
            (_, Some(b), _) => b.0 == bar,
            (_, _, Some(r)) => shelf.offset[r.0] + r.1 < row_len[r.0],
            _ if empty => shelf.designs.is_empty(),
            _ => continue,
        };
        let d = if show { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    let picked = |list: usize, k: usize| {
        let i = shelf.offset[list] + k;
        match (&shelf.pick, list) {
            (Some(Pick::Design(id)), 0) => shelf.designs.get(i).is_some_and(|e| e.id == *id),
            (Some(Pick::Template(id)), 1) => shelf.templates.get(i) == Some(id),
            _ => false,
        }
    };
    let readable = matches!(&shelf.pick, Some(Pick::Design(id)) if shelf.entry(id).is_some_and(|e| e.problem.is_none()));
    let is_design = matches!(shelf.pick, Some(Pick::Design(_)));
    for (b, mut pill) in &mut pills {
        let (active, disabled) = match b {
            ShelfButton::Row(list, k) => (picked(*list, *k), false),
            ShelfButton::Open | ShelfButton::Duplicate => (false, !(readable || matches!(shelf.pick, Some(Pick::Template(_))))),
            ShelfButton::Rename => (false, !readable),
            ShelfButton::Delete => (false, !is_design),
            // A locked frame (the career's) can't be built in; a design already in one keeps it.
            ShelfButton::Frame(f) => (design.frame() == *f, !granted.frame_open(content, *f)),
            ShelfButton::New(f) => (false, !granted.frame_open(content, *f)),
            ShelfButton::ConfirmDelete => (true, false),
            ShelfButton::Copy => (false, shelf.export.code.is_empty()),
            _ => (false, false),
        };
        if pill.active != active || pill.disabled != disabled {
            pill.active = active;
            pill.disabled = disabled;
        }
    }
    let working = lib_slot.id.as_deref();
    let working_dirty = lib_slot.dirty(&design);
    let mut s = String::new();
    for (t, mut text, mut color) in &mut texts {
        s.clear();
        let tone = match *t {
            ShelfText::Crumb => {
                s.push_str(if page == Page::Blueprints { "SHIPYARD  ›  BLUEPRINTS" } else { "SHIPYARD  ›  EXPORT" });
                Tone::Label
            }
            ShelfText::Title => {
                s.push_str(if page == Page::Blueprints { "Your designs and templates" } else { design.blueprint().name.as_str() });
                Tone::Number
            }
            ShelfText::Section(0) => {
                s.push_str(&format!("YOUR DESIGNS  ·  {}", shelf.designs.len()));
                Tone::Dim
            }
            ShelfText::Section(_) => {
                s.push_str(&format!("TEMPLATES  ·  {}  ·  OPEN AS A COPY", shelf.templates.len()));
                Tone::Dim
            }
            ShelfText::RowName(0, k) => {
                s.push_str(shelf.designs.get(shelf.offset[0] + k).map_or("", |e| e.name.as_str()));
                Tone::Number
            }
            ShelfText::RowName(_, k) => {
                let ship = shelf.templates.get(shelf.offset[1] + k).and_then(|id| content.ships.get(id));
                s.push_str(ship.map_or("", |b| b.name.as_str()));
                Tone::Number
            }
            ShelfText::RowSub(0, k) => match shelf.designs.get(shelf.offset[0] + k) {
                Some(Entry { problem: Some(p), .. }) => {
                    s.push_str(&format!("UNREADABLE  ·  {}", p.to_uppercase()));
                    Tone::Coral
                }
                Some(e) => {
                    s.push_str(&row_head(content, &shelf, e));
                    s.push_str(&format!("  ·  {}×{}  ·  {} PARTS", e.size.0, e.size.1, e.parts));
                    if let Some(t) = e.saved {
                        s.push_str(&format!("  ·  {}", library::ago(shelf.now, t)));
                    }
                    sub_tone(picked(0, k))
                }
                None => Tone::Dim,
            },
            ShelfText::RowSub(_, k) => {
                if let Some(b) = shelf.templates.get(shelf.offset[1] + k).and_then(|id| content.ships.get(id)) {
                    let faction = content.factions.get(&b.faction).map_or(String::new(), |f| f.name.to_uppercase());
                    let (w, h) = library::parts_size(content, b);
                    s.push_str(&format!("{w}×{h}  ·  {} PARTS  ·  {faction}", b.parts.len()));
                }
                sub_tone(picked(1, k))
            }
            ShelfText::RowTag(list, k) => match shelf.designs.get(shelf.offset[0] + k).filter(|_| list == 0) {
                Some(e) => {
                    let (tag, tone) = row_tag(e, Some(e.id.as_str()) == working, working_dirty);
                    s.push_str(&tag);
                    tone
                }
                None => Tone::Teal,
            },
            ShelfText::DeleteQuestion => {
                if let Mode::Delete(id) = &shelf.mode {
                    let name = shelf.entry(id).map_or(id.as_str(), |e| e.name.as_str());
                    s.push_str(&format!("DELETE {}? THERE IS NO UNDO", name.to_uppercase()));
                }
                Tone::Coral
            }
            ShelfText::FileNote => {
                s.push_str(&format!("{}  ·  THE CONTENT SHIP FORMAT: IT GOES INTO A PACK'S SHIPS FOLDER AS IS", shelf.export.file.to_uppercase()));
                Tone::Dim
            }
            ShelfText::Code => {
                s.push_str(&shelf.export.code);
                Tone::Number
            }
            ShelfText::CodeNote => {
                s.push_str(&format!(
                    "CLICK THE CODE, THEN CTRL+C, OR COPY  ·  IMPORT IT IN BLUEPRINTS  ·  {} CHARACTERS  ·  MADE WITH {}",
                    shelf.export.code.len(),
                    shelf.export.made_with
                ));
                Tone::Dim
            }
            ShelfText::Paste => {
                if shelf.paste_armed {
                    s.push_str("PASTE NOW: CTRL+V");
                    Tone::Teal
                } else {
                    s.push_str("CLICK HERE, THEN PASTE A SHARE CODE OR BLUEPRINT TEXT (CTRL+V)");
                    Tone::Dim
                }
            }
        };
        set_text(&mut text, &s);
        set_color(&mut color, c(tone, if matches!(t, ShelfText::Code) { 0.9 } else { 1.0 }));
    }
}

/// The start of a saved design's line: its frame, or for an evolved design where it came from (GDR-0092: `FROM RIM
/// SKIFF COPY · G37`; its origin's first design by the name of a saved design or a content ship, else its id in words:
/// an unsaved working design goes by the slug of its name).
pub(super) fn row_head(content: &Content, shelf: &Shelf, e: &Entry) -> String {
    let Some(o) = &e.origin else { return frame_name(content, e.frame).replace(" FRAME", "") };
    let id = o.from.first().map_or("", String::as_str);
    let name = shelf.designs.iter().find(|d| d.id == id).map(|d| d.name.clone()).or_else(|| content.ships.get(id).map(|b| b.name.clone()));
    format!("FROM {}  ·  G{}", name.unwrap_or_else(|| id.replace('_', " ")).to_uppercase(), o.generation)
}

/// A saved design's tags and their tone: EVOLVED (gold) for one the evolution lab saved, IN THE DRYDOCK (teal, and
/// UNSAVED) for the one being edited.
pub(super) fn row_tag(e: &Entry, docked: bool, dirty: bool) -> (String, Tone) {
    let tags = [e.origin.is_some().then_some("EVOLVED"), docked.then_some(if dirty { "IN THE DRYDOCK  ·  UNSAVED" } else { "IN THE DRYDOCK" })];
    let tone = if docked { Tone::Teal } else { Tone::Gold };
    (tags.into_iter().flatten().collect::<Vec<_>>().join("  ·  "), tone)
}

/// The preview follows the hovered row: built once per entry (the ship and its numbers, from a `sim::design_report`
/// made with the drydock's content and config, about 25 µs; an evolved design's ORIGIN), gone when the pointer
/// leaves the row (it stays over the gap to the next row, so moving down the list doesn't flash it, and over the
/// card and the way to it, so its ORIGIN can be clicked), and kept level with the row inside its column. The card is
/// as tall as [`PREVIEW_CARD_H`] (and [`PREVIEW_ORIGIN_H`] with an ORIGIN) and the rows' places are laid out already,
/// so it is placed right in the frame it appears in (with the height of its own first layout it showed a frame lower,
/// then jumped).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync_preview(
    mut commands: Commands,
    shelf: Res<Shelf>,
    storage: Res<Storage>,
    content: Res<GameContent>,
    report: Res<DrydockReport>,
    mut lab: ResMut<Lab>,
    (behaviors, working): (Res<PlayerBehaviors>, Res<PlayerDesign>),
    window: Single<&Window, With<PrimaryWindow>>,
    rows: Query<(Entity, &RowNode, &Hot, &ComputedNode, &UiGlobalTransform)>,
    column: Single<(&ComputedNode, &UiGlobalTransform), With<PreviewColumn>>,
    card: Single<(&mut PreviewCard, &mut Node, Option<&ComputedNode>, Option<&UiGlobalTransform>)>,
    ship: Single<Entity, With<PreviewShip>>,
    mut values: Query<(&PreviewStat, &mut Text), Without<OriginText>>,
    mut origin: Query<(&OriginPart, &mut Node, Option<&mut Pill>), Without<PreviewCard>>,
    mut origin_texts: Query<(&OriginText, &mut Text)>,
) {
    let (mut card, mut node, card_node, card_at) = card.into_inner();
    let open = shelf.page == Some(Page::Blueprints);
    let hovered = rows.iter().find(|(_, _, hot, ..)| hot.hovered).filter(|_| open);
    let card_rect = card_node.zip(card_at).filter(|_| node.display == Display::Flex).map(|(n, at)| Rect::from_center_size(at.translation, n.size));
    let near_shown = |(_, _, _, n, at): &(Entity, &RowNode, &Hot, &ComputedNode, &UiGlobalTransform)| {
        let gap = Vec2::new(0.0, ROW_GAP / n.inverse_scale_factor);
        let r = Rect::from_center_size(at.translation, n.size + 2.0 * gap);
        let r = card_rect.map_or(r, |c| r.union(c));
        window.physical_cursor_position().is_some_and(|c| r.contains(c))
    };
    let (row, pick) = match hovered {
        Some((e, r, ..)) => {
            let i = shelf.offset[r.0] + r.1;
            let pick = match r.0 {
                0 => shelf.designs.get(i).filter(|e| e.problem.is_none()).map(|e| Pick::Design(e.id.clone())),
                _ => shelf.templates.get(i).map(|id| Pick::Template(id.clone())),
            };
            (Some(e), pick)
        }
        None => match card.row.and_then(|e| rows.get(e).ok()).filter(|r| open && near_shown(r)) {
            Some(_) => (card.row, card.shown.clone()),
            None => (None, None),
        },
    };
    if card.shown != pick {
        let shown = pick.as_ref().and_then(|p| preview_of(&content.0, &storage, &report, p));
        let display = if shown.is_some() { Display::Flex } else { Display::None };
        let evolved = shown.as_ref().and_then(|(bp, _)| bp.origin.clone());
        match shown {
            Some((bp, texts)) => {
                commands.entity(*ship).insert(UiIcon::Ship(Box::new(bp)));
                // A saved design's rating on THE LADDER (the lab rates it), `OLD` when it is older than a change.
                let ladder = match &pick {
                    Some(Pick::Design(id)) => {
                        let rules = lab.bypass_change_detection().rules(&content.0);
                        match Standing::of(&storage, &content.0, &behaviors, &working, rules, id) {
                            Standing::Unrated => "—".to_string(),
                            Standing::Rated(d) => crate::evolution::rating_words(&d),
                            Standing::Changed(d) => format!("{}  ·  OLD", crate::evolution::rating_words(&d)),
                        }
                    }
                    _ => "—".to_string(),
                };
                for (k, mut text) in &mut values {
                    set_text(&mut text, texts.get(k.0).unwrap_or(&ladder));
                }
            }
            None => {
                commands.entity(*ship).remove::<UiIcon>();
            }
        }
        // An evolved design's ORIGIN: its generation and score, its run by name (a click opens it, with the lab
        // open), or that the run is gone.
        let run = evolved.as_ref().and_then(|o| lab.entry(&o.run));
        card.origin = evolved.as_ref().filter(|_| lab.enabled && run.is_some()).map(|o| (o.run.clone(), o.genome));
        if let Some(o) = &evolved {
            for (t, mut text) in &mut origin_texts {
                let words = match (t.0, run) {
                    (0, _) => format!("G{}  ·  {:.2}", o.generation, o.score),
                    (_, e) => e.map_or_else(String::new, |e| format!("{}  ›", e.name.to_uppercase())),
                };
                set_text(&mut text, &words);
            }
        }
        for (part, mut n, pill) in &mut origin {
            let on = match part {
                OriginPart::Block => evolved.is_some(),
                OriginPart::Link => run.is_some(),
                OriginPart::Gone => run.is_none(),
            };
            let d = if on { Display::Flex } else { Display::None };
            if n.display != d {
                n.display = d;
            }
            if let Some(mut pill) = pill
                && pill.disabled != card.origin.is_none()
            {
                pill.disabled = card.origin.is_none();
            }
        }
        let height = px(PREVIEW_CARD_H + if evolved.is_some() { PREVIEW_ORIGIN_H } else { 0.0 });
        if node.height != height {
            node.height = height;
        }
        if node.display != display {
            node.display = display;
        }
        card.shown = pick;
    }
    card.row = row;
    let Some((.., row_at)) = row.and_then(|e| rows.get(e).ok()).filter(|_| card.shown.is_some()) else { return };
    let (col, col_at) = *column;
    let k = col.inverse_scale_factor;
    let h = match node.height {
        Val::Px(h) => h,
        _ => PREVIEW_CARD_H,
    };
    let top = (row_at.translation.y - (col_at.translation.y - col.size.y / 2.0)) * k - h / 2.0;
    let top = px(top.clamp(0.0, (col.size.y * k - h).max(0.0)).round());
    if node.top != top {
        node.top = top;
    }
}

/// A click on an evolved design's ORIGIN: home, where the camera flies on to the lab and its run's page opens with
/// the design picked (`evolution::open`).
pub(super) fn open_origin(mut clicks: MessageReader<UiClick>, links: Query<(&OriginPart, &Pill)>, card: Single<&PreviewCard>, mut lab: ResMut<Lab>, mut next: ResMut<NextState<AppMode>>) {
    for c in clicks.read() {
        if links.get(c.0).is_ok_and(|(part, p)| *part == OriginPart::Link && !p.disabled)
            && let Some((run, genome)) = card.origin.clone()
        {
            lab.request = Some(LabRequest::Origin { run, genome });
            next.set(AppMode::Home);
        }
    }
}

/// What the preview shows of an entry: its blueprint and the values of its numbers (the size class: the frame a
/// saved design is in, the first one a template fits, as OPEN opens it; mass, top speed and weapons from the
/// design report, dashes when it doesn't compile). `None` when a saved design can't be read.
fn preview_of(content: &Content, storage: &Storage, report: &DrydockReport, pick: &Pick) -> Option<(Blueprint, [String; 4])> {
    let (bp, frame) = match pick {
        Pick::Design(id) => {
            let bp = storage.read::<DesignDoc>(id).ok()??.doc.0;
            let frame = bp.size;
            (bp, frame)
        }
        Pick::Template(id) => {
            let bp = content.ships.get(id)?.clone();
            let frame = library::open_in_first_frame(content, &bp).map_or(bp.size, |d| d.frame());
            (bp, frame)
        }
    };
    let class = content.frames.iter().find(|f| f.size == frame).map_or_else(|| format!("{}×{}", frame.0, frame.1), |f| f.name.clone());
    let dash = || "—".to_string();
    let [mass, speed, guns] = match sim::design_report(report.content().unwrap_or(content), &bp, report.config()) {
        Ok(r) => {
            let f = &r.flight;
            let moves = f.thrust.iter().any(|&t| t > 0.0);
            let speed = if moves { stats::stat_value(content, "top_speed", f.top_speed[f.best]) } else { dash() };
            [stats::stat_value(content, "mass", r.mass), speed, r.guns.len().to_string()]
        }
        Err(_) => [dash(), dash(), dash()],
    };
    Some((bp, [class, mass, speed, guns]))
}
