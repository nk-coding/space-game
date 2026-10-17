//! The WORKSHOP (GDR-0108): a layer over home at the workshop under the bottom wall. Left, the haul's intact parts
//! that wait to be sorted: per part KEEP (into stock) or DISMANTLE (research data and a share of its recipe's
//! materials), the workshop's suggestion beside it and SORT AS SUGGESTED for all. Right, fabricating: what the fleet
//! is short of (FABRICATE MISSING) and every unlocked part with a recipe (MAKE), its stock and its recipe, the
//! materials it lacks in coral. Top right: the materials in store.
//!
//! ```text
//! (o) SHIPYARD › WORKSHOP                         COPPER 7  CRYSTAL 4  STEEL 6  …   [CLOSE]
//!     Workshop
//!   TO SORT · 6 PARTS                              FABRICATE · THE FLEET IS SHORT OF 2   [FABRICATE MISSING]
//!   Thruster   ×5  IN STOCK 2  KEEP 0 · DISMANTLE 5  Pulse laser  2 / 4  COPPER 3 · CRYSTAL 2 · STEEL 1  [MAKE]
//!     [KEEP] [DISMANTLE]                            Thruster     4      CERAMIC 2 · COPPER 1 · STEEL 3  [MAKE]
//!   [SORT AS SUGGESTED]                             …
//!   DISMANTLED 5 THRUSTERS  ·  PROPULSION +15  ·  STEEL +6
//! ```
//!
//! * Home's workshop (W, once the camera has flown there) opens it; the ring button, CLOSE and Esc close it.
//! * Every action is the career's (`data::career::Career::keep`, `dismantle`, `sort_suggested`, `fabricate`) and
//!   writes the save; the lists are built again after each.
//! * The fleet is the drydock's working design and the escorts: what it carries is what the suggestion keeps and
//!   what FABRICATE MISSING makes.
//!
//! UI words are chrome in code, like the LOG's; the parts' and materials' names are content.

use std::collections::BTreeMap;

use bevy::prelude::*;
use data::career::{Refusal, Shortfall, fleet_parts};
use data::{Career, Content};
use kit::style::{Style, ui_color};
use kit::ui_icons::{Glyph, UiIcon};
use kit::ui_kit::{Edge, HudFonts, Pill, ScreenFx, ScreenPart, Scrollbar, Tip, Tone, UiClick, UiInputSet, UiStyleSet, label, number, prose, ring_node, show_screen, spawn_pill, tracked};
use world::career::{EscortDesigns, Note, PlayerCareer};
use world::mode::AppMode;
use world::player::PlayerDesign;
use world::sim_bridge::GameContent;
use world::storage::Storage;
use world::voyage::debrief::amount;

use crate::home::HomeAction;

/// The layout's size (logical px), like the LOG's.
const STAGE: Vec2 = Vec2::new(1280.0, 720.0);
/// The two columns' boxes on the stage.
const SORT: (Vec2, Vec2) = (Vec2::new(40.0, 104.0), Vec2::new(560.0, 540.0));
const MAKE: (Vec2, Vec2) = (Vec2::new(640.0, 104.0), Vec2::new(600.0, 540.0));
const ROW: f32 = 44.0;

/// One part of the intake as the workshop shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct SortRow {
    pub part: String,
    pub name: String,
    pub count: u32,
    pub stock: u32,
    pub keep: u32,
    pub dismantle: u32,
}

/// One part the workshop can fabricate: its stock, what the fleet carries, its recipe (material name, amount, short
/// of it) and why it can't be made now.
#[derive(Clone, Debug, PartialEq)]
pub struct MakeRow {
    pub part: String,
    pub name: String,
    pub stock: u32,
    pub need: u32,
    pub recipe: Vec<(String, u32, bool)>,
    pub why: Option<String>,
}

/// What the fleet carries (the working design and the escorts), by part id.
fn fleet_need(content: &Content, design: &PlayerDesign, escorts: &EscortDesigns) -> BTreeMap<String, u32> {
    fleet_parts(content, std::iter::once(design.blueprint()).chain(&escorts.0))
}

/// The intake as rows, in id order, with the suggestion for each.
pub fn sort_rows(content: &Content, career: &Career, need: &BTreeMap<String, u32>) -> Vec<SortRow> {
    let suggested = career.suggested(content, need);
    career
        .intake()
        .iter()
        .map(|(part, &count)| {
            let s = suggested.get(part).copied().unwrap_or_default();
            SortRow {
                part: part.clone(),
                name: content.parts.get(part).map_or_else(|| part.clone(), |p| p.name.clone()),
                count,
                stock: career.stock().get(part).copied().unwrap_or(0),
                keep: s.keep,
                dismantle: s.dismantle,
            }
        })
        .collect()
}

/// The unlocked parts with a recipe: those the fleet is short of first, then in content order.
pub fn make_rows(content: &Content, career: &Career, need: &BTreeMap<String, u32>) -> Vec<MakeRow> {
    let unlocks = career.unlocks(content);
    let mut rows: Vec<MakeRow> = content
        .parts
        .iter()
        .filter(|p| !p.hull && p.recipe.is_some() && unlocks.has_part(&p.id))
        .map(|p| {
            let have = |m: &str| career.materials().get(m).copied().unwrap_or(0);
            let recipe = p.recipe.as_ref().map_or_else(Vec::new, |r| {
                r.materials.iter().map(|(m, &a)| (content.materials.get(m).map_or_else(|| m.clone(), |x| x.name.clone()), a, have(m) < a)).collect()
            });
            MakeRow {
                part: p.id.clone(),
                name: p.name.clone(),
                stock: career.stock().get(&p.id).copied().unwrap_or(0),
                need: need.get(&p.id).copied().unwrap_or(0),
                recipe,
                why: career.can_fabricate(content, &p.id, 1).err().map(|r| refusal_words(content, &r)),
            }
        })
        .collect();
    rows.sort_by_key(|r| r.need <= r.stock);
    rows
}

/// What the fleet carries more of than the stock holds: (part, need, have), in id order.
pub fn missing(career: &Career, need: &BTreeMap<String, u32>) -> Vec<(String, u32, u32)> {
    need.iter()
        .filter_map(|(p, &n)| {
            let have = career.stock().get(p).copied().unwrap_or(0);
            (n > have).then(|| (p.clone(), n, have))
        })
        .collect()
}

/// A refusal in the workshop's words (uppercase).
pub fn refusal_words(content: &Content, r: &Refusal) -> String {
    let part = |id: &str| content.parts.get(id).map_or_else(|| id.to_uppercase(), |p| p.name.to_uppercase());
    match r {
        Refusal::LockedPart(p) => format!("{} IS NOT RESEARCHED", part(p)),
        Refusal::NoRecipe(p) => format!("{} COMES ONLY FROM SALVAGE", part(p)),
        Refusal::NeedsFacility(f) => format!("NEEDS THE {}", content.facilities.get(f).map_or_else(|| f.to_uppercase(), |x| x.name.to_uppercase())),
        Refusal::CannotPay(short) => {
            let words: Vec<String> = short
                .iter()
                .filter_map(|s| match s {
                    Shortfall::Material { material, have, cost } => {
                        Some(format!("{} {have} / {cost}", content.materials.get(material).map_or_else(|| material.to_uppercase(), |m| m.name.to_uppercase())))
                    }
                    Shortfall::Data { .. } => None,
                })
                .collect();
            format!("SHORT OF {}", words.join("  ·  "))
        }
        Refusal::NotInIntake(p) => format!("NO {} LEFT TO SORT", part(p)),
        Refusal::NoWorkshop => "THE CONTENT HAS NO WORKSHOP".into(),
        other => format!("{other:?}").to_uppercase(),
    }
}

/// What a dismantling gave, in words: `PROPULSION +15  ·  STEEL +6`.
fn gave_words(content: &Content, got: &data::career::HaulReport) -> String {
    let data = got.data.iter().map(|(f, x)| format!("{} +{}", content.families.get(f).map_or(f.as_str(), |f| f.name.as_str()).to_uppercase(), amount(*x)));
    let mats = got.materials.iter().map(|(m, n)| format!("{} +{n}", content.materials.get(m).map_or(m.as_str(), |m| m.name.as_str()).to_uppercase()));
    let all: Vec<String> = data.chain(mats).collect();
    if all.is_empty() { "NOTHING".into() } else { all.join("  ·  ") }
}

/// The WORKSHOP's state (a resource): open or not, and what the last action said.
#[derive(Resource, Debug, Default)]
pub struct WorkshopLayer {
    pub open: bool,
    said: Option<(String, Tone)>,
}

/// Is the WORKSHOP open (home's input waits then)?
pub fn is_open(layer: Res<WorkshopLayer>) -> bool {
    layer.open
}

#[derive(Component)]
struct WorkshopRoot;

#[derive(Component)]
struct SortList;

#[derive(Component)]
struct MakeList;

#[derive(Component)]
struct Materials;

#[derive(Component)]
struct Said;

#[derive(Component)]
struct CloseButton;

/// A button's action.
#[derive(Component, Clone, Debug, PartialEq)]
enum Do {
    Keep(String),
    Dismantle(String),
    SortAll,
    Make(String),
    MakeMissing,
}

pub struct WorkshopPlugin;

impl Plugin for WorkshopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorkshopLayer>()
            .add_systems(Startup, spawn.after(world::flight_hud::spawn_hud))
            .add_systems(OnExit(AppMode::Home), |mut layer: ResMut<WorkshopLayer>| layer.open = false)
            .add_systems(Update, show.run_if(state_changed::<AppMode>.or_eager(resource_changed::<WorkshopLayer>)))
            .add_systems(Update, (open, input).chain().after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Home)));
    }
}

/// The lists' and the materials' nodes, for a refill.
type Lists<'w, 's> = (Single<'w, 's, Entity, With<SortList>>, Single<'w, 's, Entity, With<MakeList>>, Single<'w, 's, Entity, With<Materials>>, Single<'w, 's, Entity, With<Said>>);

/// The workshop (W): open, with the lists as they are now.
#[allow(clippy::too_many_arguments)]
fn open(
    mut commands: Commands,
    mut actions: MessageReader<HomeAction>,
    mut layer: ResMut<WorkshopLayer>,
    career: Res<PlayerCareer>,
    content: Res<GameContent>,
    (design, escorts): (Res<PlayerDesign>, Res<EscortDesigns>),
    fonts: Res<HudFonts>,
    style: Res<Style>,
    lists: Lists,
) {
    if !actions.read().any(|a| *a == HomeAction::Workshop) {
        return;
    }
    layer.said = None;
    let need = fleet_need(&content.0, &design, &escorts);
    refill(&mut commands, &lists, &fonts, &style, &content.0, career.career(), &need, &layer.said);
    layer.open = true;
}

/// The buttons, CLOSE, the ring and Esc. Each action changes the career, writes the save and refills the lists.
#[allow(clippy::too_many_arguments)]
fn input(
    mut commands: Commands,
    mut clicks: MessageReader<UiClick>,
    close: Query<(), With<CloseButton>>,
    buttons: Query<&Do>,
    keys: Res<ButtonInput<KeyCode>>,
    mut layer: ResMut<WorkshopLayer>,
    mut career: ResMut<PlayerCareer>,
    mut storage: ResMut<Storage>,
    content: Res<GameContent>,
    (design, escorts): (Res<PlayerDesign>, Res<EscortDesigns>),
    fonts: Res<HudFonts>,
    style: Res<Style>,
    lists: Lists,
) {
    let mut closed = false;
    let mut todo = None;
    for c in clicks.read() {
        closed |= close.contains(c.0);
        if let Ok(d) = buttons.get(c.0) {
            todo = Some(d.clone());
        }
    }
    if !layer.open {
        return;
    }
    if let Some(d) = todo {
        let c = &content.0;
        let need = fleet_need(c, &design, &escorts);
        let name = |p: &str| c.parts.get(p).map_or_else(|| p.to_uppercase(), |x| x.name.to_uppercase());
        let (said, written) = career.update(&mut storage, |career| match &d {
            Do::Keep(p) => career.keep(p, 1).map(|()| (format!("KEPT 1 {}  ·  IN STOCK", name(p)), Tone::Teal)),
            Do::Dismantle(p) => career.dismantle(c, p, 1).map(|got| (format!("DISMANTLED 1 {}  ·  {}", name(p), gave_words(c, &got)), Tone::Gold)),
            Do::SortAll => {
                let kept: u32 = career.suggested(c, &need).values().map(|s| s.keep).sum();
                career.sort_suggested(c, &need).map(|got| (format!("SORTED  ·  KEPT {kept}  ·  {}", gave_words(c, &got)), Tone::Gold))
            }
            Do::Make(p) => career.fabricate(c, p, 1).map(|()| (format!("FABRICATED 1 {}  ·  IN STOCK", name(p)), Tone::Teal)),
            Do::MakeMissing => {
                let mut made = 0;
                let mut stop = None;
                for (p, n, have) in missing(career, &need) {
                    match career.fabricate(c, &p, n - have) {
                        Ok(()) => made += n - have,
                        Err(r) => {
                            stop = Some(r);
                            break;
                        }
                    }
                }
                match (made, stop) {
                    (_, Some(r)) if made == 0 => Err(r),
                    (_, Some(r)) => Ok((format!("FABRICATED {made}  ·  THEN {}", refusal_words(c, &r)), Tone::Gold)),
                    _ => Ok((format!("FABRICATED {made}  ·  THE FLEET IS IN STOCK"), Tone::Teal)),
                }
            }
        });
        layer.said = Some(said.unwrap_or_else(|r| (refusal_words(c, &r), Tone::Coral)));
        if let Err(e) = written {
            warn!("workshop: not saved: {e}");
            career.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
        }
        refill(&mut commands, &lists, &fonts, &style, c, career.career(), &need, &layer.said);
    }
    if closed || keys.just_pressed(KeyCode::Escape) {
        layer.open = false;
    }
}

/// Shown at home while open, with its transition; home's words step aside meanwhile (`home::show`).
fn show(mode: Res<State<AppMode>>, layer: Res<WorkshopLayer>, mut root: Single<&mut ScreenFx, With<WorkshopRoot>>) {
    show_screen(&mut root, *mode.get() == AppMode::Home && layer.open);
}

/// Both lists, the materials in store and the last action's words, anew.
#[allow(clippy::too_many_arguments)]
fn refill(commands: &mut Commands, lists: &Lists, fonts: &HudFonts, style: &Style, content: &Content, career: &Career, need: &BTreeMap<String, u32>, said: &Option<(String, Tone)>) {
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    let (sort, make, mats, line) = (*lists.0, *lists.1, *lists.2, *lists.3);
    let rows = sort_rows(content, career, need);
    commands.entity(sort).despawn_children();
    commands.entity(sort).with_children(|p| fill_sort(p, fonts, style, &rows));
    let short = missing(career, need);
    let rows = make_rows(content, career, need);
    commands.entity(make).despawn_children();
    commands.entity(make).with_children(|p| fill_make(p, fonts, style, content, &short, &rows));
    commands.entity(mats).despawn_children();
    commands.entity(mats).with_children(|p| {
        if career.materials().is_empty() {
            p.spawn(label(fonts, "NO MATERIALS IN STORE", 9.0, c(Tone::Dim)));
        }
        for (m, n) in career.materials() {
            p.spawn(label(fonts, &content.materials.get(m).map_or_else(|| m.to_uppercase(), |x| x.name.to_uppercase()), 8.5, c(Tone::Label)));
            p.spawn((number(fonts, &n.to_string(), 15.0, c(Tone::Number)), Node { margin: UiRect::right(px(6)), ..default() }));
        }
    });
    commands.entity(line).despawn_children();
    if let Some((words, tone)) = said {
        commands.entity(line).with_child(tracked(fonts, words, 9.5, c(*tone), 0.18));
    }
}

/// The intake's rows (or the empty state) and SORT AS SUGGESTED.
fn fill_sort(p: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, rows: &[SortRow]) {
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    let total: u32 = rows.iter().map(|r| r.count).sum();
    let head = if total == 0 { "TO SORT".to_string() } else { format!("TO SORT  ·  {total} INTACT {}", if total == 1 { "PART" } else { "PARTS" }) };
    p.spawn((label(fonts, &head, 9.5, c(if total == 0 { Tone::Dim } else { Tone::Gold })), Node { margin: UiRect::bottom(px(8)), flex_shrink: 0.0, ..default() }));
    if rows.is_empty() {
        p.spawn((prose(fonts, "Intact parts your voyages bring home wait here: keep them for your ships, or dismantle them for research data and materials. Broken parts are dismantled as they come home.", 13.0, c(Tone::Dim)), Node { margin: UiRect::top(px(10)), ..default() }));
        return;
    }
    for r in rows {
        p.spawn((
            Node { min_height: px(ROW), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(12), border: UiRect::bottom(px(1)), ..default() },
            BorderColor::all(ui_color(ui.dim, 0.25)),
        ))
        .with_children(|row| {
            row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), width: px(150), flex_shrink: 0.0, ..default() }).with_children(|col| {
                col.spawn(prose(fonts, &r.name, 15.0, c(Tone::Number)));
                col.spawn(label(fonts, &format!("×{}  ·  IN STOCK {}", r.count, r.stock), 8.5, c(Tone::Label)));
            });
            row.spawn((label(fonts, &format!("KEEP {}  ·  DISMANTLE {}", r.keep, r.dismantle), 8.5, c(Tone::Dim)), Node { flex_grow: 1.0, ..default() }));
            spawn_pill(row, fonts, "KEEP", 9.0, 26.0, None, Pill::new(Tone::Teal), (Do::Keep(r.part.clone()), Tip::new("KEEP ONE  ·  INTO STOCK")));
            spawn_pill(row, fonts, "DISMANTLE", 9.0, 26.0, None, Pill::new(Tone::Gold), (Do::Dismantle(r.part.clone()), Tip::new("DISMANTLE ONE  ·  DATA AND MATERIALS")));
        });
    }
    p.spawn(Node { height: px(14), flex_shrink: 0.0, ..default() });
    p.spawn(Node { flex_direction: FlexDirection::Row, flex_shrink: 0.0, ..default() }).with_children(|b| {
        spawn_pill(b, fonts, "SORT AS SUGGESTED", 9.5, 30.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, (Do::SortAll, Tip::new("KEEP AND DISMANTLE AS SUGGESTED")));
    });
}

/// What the fleet is short of, FABRICATE MISSING, and the parts to make.
fn fill_make(p: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, content: &Content, short: &[(String, u32, u32)], rows: &[MakeRow]) {
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    p.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(14), margin: UiRect::bottom(px(8)), flex_shrink: 0.0, ..default() }).with_children(|h| {
        if short.is_empty() {
            h.spawn(label(fonts, "FABRICATE  ·  THE FLEET IS IN STOCK", 9.5, c(Tone::Teal)));
        } else {
            let n: u32 = short.iter().map(|(_, need, have)| need - have).sum();
            h.spawn(label(fonts, &format!("FABRICATE  ·  THE FLEET IS SHORT OF {n}"), 9.5, c(Tone::Coral)));
            spawn_pill(h, fonts, "FABRICATE MISSING", 9.0, 26.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, (Do::MakeMissing, Tip::new("FABRICATE WHAT THE FLEET LACKS")));
        }
    });
    if !short.is_empty() {
        let words: Vec<String> = short
            .iter()
            .map(|(part, need, have)| format!("{} {have} / {need}", content.parts.get(part).map_or_else(|| part.to_uppercase(), |p| p.name.to_uppercase())))
            .collect();
        p.spawn((tracked(fonts, &words.join("  ·  "), 8.5, c(Tone::Coral), 0.16), Node { margin: UiRect::bottom(px(10)), flex_shrink: 0.0, ..default() }));
    }
    for r in rows {
        p.spawn((
            Node { min_height: px(ROW), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(12), border: UiRect::bottom(px(1)), ..default() },
            BorderColor::all(ui_color(ui.dim, 0.25)),
        ))
        .with_children(|row| {
            row.spawn((prose(fonts, &r.name, 15.0, c(Tone::Number)), Node { width: px(130), flex_shrink: 0.0, ..default() }));
            let (stock, tone) = if r.need > 0 { (format!("{} / {}", r.stock, r.need), if r.stock < r.need { Tone::Coral } else { Tone::Teal }) } else { (r.stock.to_string(), Tone::Number) };
            row.spawn((number(fonts, &stock, 14.0, c(tone)), Node { width: px(48), flex_shrink: 0.0, ..default() }));
            row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), flex_grow: 1.0, flex_shrink: 1.0, ..default() }).with_children(|col| {
                col.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(8), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|m| {
                    for (name, n, lacks) in &r.recipe {
                        m.spawn(label(fonts, &format!("{} {n}", name.to_uppercase()), 8.5, c(if *lacks { Tone::Coral } else { Tone::Label })));
                    }
                });
                if let Some(why) = r.why.as_ref().filter(|w| !w.starts_with("SHORT OF")) {
                    col.spawn(label(fonts, why, 8.0, c(Tone::Dim)));
                }
            });
            let pill = Pill { disabled: r.why.is_some(), ..Pill::new(Tone::Teal) };
            spawn_pill(row, fonts, "MAKE", 9.0, 26.0, None, pill, (Do::Make(r.part.clone()), Tip::new("FABRICATE ONE")));
        });
    }
}

fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let abs = |at: Vec2, size: Option<Vec2>| Node {
        position_type: PositionType::Absolute,
        left: px(at.x),
        top: px(at.y),
        width: size.map_or(Val::Auto, |s| px(s.x)),
        height: size.map_or(Val::Auto, |s| px(s.y)),
        ..default()
    };
    let row = |gap: f32| Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), ..default() };
    let list = |(at, size): (Vec2, Vec2)| Node { flex_direction: FlexDirection::Column, overflow: Overflow::scroll_y(), padding: UiRect::right(px(14)), ..abs(at, Some(size)) };
    commands
        .spawn((
            WorkshopRoot,
            world::backdrop::VeilsBackdrop,
            ScreenFx::closed().dimming(),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn(Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }).with_children(|centre| {
                centre.spawn(Node { width: px(STAGE.x), height: px(STAGE.y), flex_shrink: 0.0, ..default() }).with_children(|stage| {
                    stage
                        .spawn((ScreenPart::new(Edge::Top, 0), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(14), ..abs(Vec2::new(22.0, 14.0), None) }))
                        .with_children(|tl| {
                            tl.spawn((ring_node(40.0), CloseButton, Tip::key("SHIPYARD", "ESC")))
                                .with_child((UiIcon::Glyph { glyph: Glyph::Menu, color: ui.violet }, Node { width: px(22), height: px(22), ..default() }));
                            tl.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                                col.spawn(label(fonts, "SHIPYARD  ›  WORKSHOP", 10.0, c(Tone::Label, 1.0)));
                                col.spawn(prose(fonts, "Workshop", 22.0, c(Tone::Number, 1.0)));
                            });
                            crate::evolution::spawn_chip(tl, fonts);
                        });
                    stage
                        .spawn((ScreenPart::new(Edge::Top, 0), Node { position_type: PositionType::Absolute, right: px(20), top: px(19), ..row(10.0) }))
                        .with_children(|tr| {
                            tr.spawn((Materials, row(8.0)));
                            tr.spawn(Node { width: px(6), ..default() });
                            spawn_pill(tr, fonts, "CLOSE", 9.5, 30.0, None, Pill::new(Tone::Teal), (CloseButton, Tip::key("CLOSE", "ESC")));
                        });
                    stage.spawn((SortList, ScreenPart::new(Edge::Fade, 1), Scrollbar::vertical(), ScrollPosition::default(), list(SORT)));
                    stage.spawn((MakeList, ScreenPart::new(Edge::Fade, 1), Scrollbar::vertical(), ScrollPosition::default(), list(MAKE)));
                    stage.spawn((Said, ScreenPart::new(Edge::Bottom, 0), abs(Vec2::new(40.0, 664.0), None)));
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::career::{Haul, HaulItem, VoyageEnd};

    /// A haul of intact thrusters and a pulse laser: the rows, the suggestion, what the fleet lacks and why a part
    /// can't be made, in words.
    #[test]
    fn the_rows_and_their_words() {
        let content = data::base_content();
        let mut c = Career::new(&content, "rim_awakening", 4207).unwrap();
        c.voyage_launched(&content, "yard_approach").unwrap();
        let haul: Haul = [(HaulItem::Part { part: "thruster".into(), material: None }, 3), (HaulItem::Part { part: "pulse_laser".into(), material: None }, 1)].into_iter().collect();
        c.voyage_ended(&content, "yard_approach", VoyageEnd::Recalled { haul, cleared: false, ticks: 30 }).unwrap();
        let skiff = content.ships.get("rim_skiff").unwrap();
        let need = fleet_parts(&content, [skiff, skiff]);
        let rows = sort_rows(&content, &c, &need);
        assert_eq!(rows.iter().map(|r| (r.name.as_str(), r.count, r.stock, r.keep, r.dismantle)).collect::<Vec<_>>(), [("Pulse laser", 1, 2, 1, 0), ("Thruster", 3, 2, 2, 1)]);
        let short = missing(&c, &need);
        assert_eq!(short[0], ("capacitor".to_string(), 4, 2));
        let make = make_rows(&content, &c, &need);
        assert!(make[0].need > make[0].stock, "what the fleet lacks comes first");
        assert!(make.iter().all(|r| r.part != "beam_laser" && r.part != "armor_block"), "locked and hull parts are not offered");
        let laser = make.iter().find(|r| r.part == "pulse_laser").unwrap();
        assert_eq!(laser.recipe, [("Copper".to_string(), 3, false), ("Lattice crystal".to_string(), 2, false), ("Steel".to_string(), 1, false)]);
        assert_eq!(laser.why, None);
        assert_eq!(refusal_words(&content, &Refusal::NeedsFacility("fusion_workshop".into())), "NEEDS THE FUSION WORKSHOP");
        let short = Refusal::CannotPay(vec![Shortfall::Material { material: "copper".into(), have: 1, cost: 3 }]);
        assert_eq!(refusal_words(&content, &short), "SHORT OF COPPER 1 / 3");
    }
}
