//! The WORKSHOP (GDR-0108): a layer over home at the workshop under the bottom wall. Left, the parts list: every part
//! with a recipe that is unlocked or in stock, in the kit's grouped list like the drydock's palette (Niklas, 18 Oct
//! 2026: the search, a section per category with its chip; `kit::ui_kit::GroupedList`), what the fleet is short of
//! first in each section (FABRICATE MISSING for those, above the list), its stock (against what the fleet carries),
//! MAKE and DISMANTLE (a share of its recipe, no data; never below what the fleet carries). Right, the materials in
//! store, one row each (there will be more than a top bar holds); the part under the pointer shows what it needs
//! there, coral where the store is short (Niklas, 18 Oct 2026). Every row leads with the part's icon as the drydock's
//! palette draws it (`drydock::panels::part_icon`); the pointer resting on a row shows the part's hover card
//! (`world::part_card`, the drydock's too) in the room between the two columns.
//!
//! The haul's intact parts are no longer sorted here: the voyage's report keeps or dismantles them, and what it leaves
//! goes to the stock when it is dismissed (`debrief.rs`; Niklas, 18 Oct 2026: "actions like take / dismantle for
//! returned salvage belong in the return-report sidebar"). Parts made go straight into the list's stock.
//!
//! How many (Niklas, 18 Oct 2026): MAKE and DISMANTLE open a strip under the row: − n + (shift: five at a time) and
//! MAX, what it costs or gives in all, then the confirming pill (MAKE 3) or CANCEL (Esc). It opens at the fleet's
//! shortfall or one.
//!
//! ```text
//! (o) SHIPYARD › WORKSHOP                                                                 [CLOSE]
//!     Workshop
//!   PARTS  ·  THE FLEET IS SHORT OF 2   [FABRICATE MISSING]                     MATERIALS IN STORE
//!   CAPACITOR 2 / 4                                                             PULSE LASER  ·  NEEDS
//!   [ Search parts                                  ]   ┌ hover card ┐          COPPER          7   NEEDS 3
//!   [POWER][WEAPONS][DEFENSE][…]                         │ [▣] Pulse  │          LATTICE CRYSTAL 4   NEEDS 2
//!   WEAPONS                                     2       │ stats …    │          STEEL           6   NEEDS 1
//!   [] Pulse laser   2 / 4      [MAKE] [DISMANTLE]       └────────────┘          CERAMIC         2
//!      (−) 2 (+) [MAX]         [MAKE 2] [CANCEL]
//!      COSTS COPPER 6 · CRYSTAL 4
//!   FABRICATED 2 PULSE LASER  ·  IN STOCK
//! ```
//!
//! * Home's workshop (W, once the camera has flown there) opens it; the ring button, CLOSE and Esc close it.
//! * Every action is the career's (`data::career::Career::fabricate`, `dismantle_stock`) and writes the save; the
//!   lists are built again after each (and after each step of a count), the materials when the pointer moves to
//!   another part.
//! * The fleet is the drydock's working design and the escorts: what it carries is what FABRICATE MISSING makes and
//!   what DISMANTLE leaves.
//!
//! UI words are chrome in code, like the LOG's; the parts' and materials' names are content.

use std::collections::BTreeMap;

use bevy::prelude::*;
use data::career::{HaulReport, Refusal, Shortfall, dismantle_yield, fleet_parts};
use data::{Career, Content};
use kit::style::{Style, ui_color};
use kit::ui_icons::{Glyph, UiIcon};
use kit::ui_kit::{
    Edge, Hot, HudFonts, ListRow, ListSpec, Pill, Ring, ScreenFx, ScreenPart, Scrollbar, Tip, Tone, UiClick, UiInputSet, UiPointer, UiStyleSet, fill_list, frame, label, number, prose,
    ring_glyph, ring_node, show_screen, spawn_close, spawn_grouped_list, spawn_head, spawn_pill, tracked,
};
use world::career::{EscortDesigns, Note, PlayerCareer};
use world::mode::AppMode;
use world::part_card::{Held, PartHover};
use world::player::PlayerDesign;
use world::sim_bridge::GameContent;
use world::storage::Storage;
use world::voyage::debrief::amount;

use crate::drydock::panels::part_icon;
use crate::home::HomeAction;

/// The layout's size (logical px), like the LOG's.
const STAGE: Vec2 = Vec2::new(1280.0, 720.0);
/// The two columns' boxes on the stage: the parts, the materials; the hover card shows in the room between them.
const PARTS: (Vec2, Vec2) = (Vec2::new(40.0, 84.0), Vec2::new(640.0, 568.0));
const MATERIALS: (Vec2, Vec2) = (Vec2::new(960.0, 84.0), Vec2::new(280.0, 568.0));
const ROW: f32 = 44.0;
/// The most a count goes to, and a shift-click's step.
const COUNT_MAX: u32 = 99;
const COUNT_STEP: u32 = 5;

/// One part with an intact count, as the voyage's report sorts it (`debrief.rs`): how many came home, the stock, the
/// suggestion.
#[derive(Clone, Debug, PartialEq)]
pub struct SortRow {
    pub part: String,
    pub name: String,
    pub count: u32,
    pub stock: u32,
    pub keep: u32,
    pub dismantle: u32,
}

/// One part the workshop can fabricate or holds in stock: its stock, what the fleet carries, why it can't be made
/// now, and the most it can make and dismantle.
#[derive(Clone, Debug, PartialEq)]
pub struct MakeRow {
    pub part: String,
    pub name: String,
    /// Its category (the list's section).
    pub category: String,
    pub stock: u32,
    pub need: u32,
    pub why: Option<String>,
    pub make: u32,
    pub spare: u32,
}

/// One material as the materials column shows it: its name, what the store holds and, for the part under the
/// pointer, what it needs.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialRow {
    pub name: String,
    pub have: u32,
    pub need: Option<u32>,
}

/// What a count is chosen for: fabricating, dismantling stock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Make,
    Scrap,
}

/// A count being chosen: for what, of which part, how many.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub act: Act,
    pub part: String,
    pub n: u32,
}

/// The most a count may reach (pure): what the materials pay for, the stock beyond what the fleet carries; at most
/// [`COUNT_MAX`].
pub fn most(content: &Content, career: &Career, need: &BTreeMap<String, u32>, act: Act, part: &str) -> u32 {
    match act {
        Act::Make => career.fabricable(content, part),
        Act::Scrap => career.spare(part, need),
    }
    .min(COUNT_MAX)
}

/// The count a choice opens at: the fleet's shortfall for MAKE, else one; within one and [`most`].
pub fn first_count(content: &Content, career: &Career, need: &BTreeMap<String, u32>, act: Act, part: &str) -> u32 {
    let want = match act {
        Act::Make => need.get(part).copied().unwrap_or(0).saturating_sub(career.stock().get(part).copied().unwrap_or(0)),
        Act::Scrap => 1,
    };
    want.clamp(1, most(content, career, need, act, part).max(1))
}

/// What confirming a choice costs or gives in all, in words: `COSTS COPPER 6  ·  STEEL 2`, `GIVES STEEL +2  ·  NO
/// DATA` (stock).
pub fn choice_words(content: &Content, c: &Choice) -> String {
    let share = content.workshop().map_or(0.0, |(_, r)| r.dismantle);
    match c.act {
        Act::Make => {
            let cost: Vec<String> = content.parts.get(&c.part).and_then(|p| p.recipe.as_ref()).map_or_else(Vec::new, |r| {
                r.materials.iter().map(|(m, &a)| format!("{} {}", content.materials.get(m).map_or(m.as_str(), |x| x.name.as_str()).to_uppercase(), a * c.n)).collect()
            });
            format!("COSTS {}", cost.join("  ·  "))
        }
        Act::Scrap => {
            let got = HaulReport { materials: dismantle_yield(content, &c.part, c.n, share).materials, ..HaulReport::default() };
            format!("GIVES {}  ·  NO DATA", gave_words(content, &got))
        }
    }
}

/// What the fleet carries (the working design and the escorts), by part id.
pub(crate) fn fleet_need(content: &Content, design: &PlayerDesign, escorts: &EscortDesigns) -> BTreeMap<String, u32> {
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

/// The parts with a recipe that are unlocked or in stock, by category in content order (the list's sections): in
/// each, those the fleet is short of first, then in content order.
pub fn make_rows(content: &Content, career: &Career, need: &BTreeMap<String, u32>) -> Vec<MakeRow> {
    let unlocks = career.unlocks(content);
    let mut rows: Vec<MakeRow> = content
        .parts
        .iter()
        .filter(|p| !p.hull && p.recipe.is_some() && (unlocks.has_part(&p.id) || career.stock().contains_key(&p.id)))
        .map(|p| MakeRow {
            part: p.id.clone(),
            name: p.name.clone(),
            category: p.category.clone(),
            stock: career.stock().get(&p.id).copied().unwrap_or(0),
            need: need.get(&p.id).copied().unwrap_or(0),
            why: career.can_fabricate(content, &p.id, 1).err().map(|r| refusal_words(content, &r)),
            make: most(content, career, need, Act::Make, &p.id),
            spare: most(content, career, need, Act::Scrap, &p.id),
        })
        .collect();
    rows.sort_by_key(|r| (content.categories.iter().position(|c| c.id == r.category), r.need <= r.stock));
    rows
}

/// The materials column (pure): every material the store holds or `part` (and how many of it) needs, in content
/// order, with that need.
pub fn material_rows(content: &Content, career: &Career, part: Option<(&str, u32)>) -> Vec<MaterialRow> {
    let recipe = part.and_then(|(p, n)| content.parts.get(p).and_then(|p| p.recipe.as_ref()).map(|r| (r, n)));
    content
        .materials
        .iter()
        .filter_map(|m| {
            let have = career.materials().get(&m.id).copied().unwrap_or(0);
            let need = recipe.and_then(|(r, n)| r.materials.get(&m.id).map(|&a| a * n));
            (have > 0 || need.is_some()).then(|| MaterialRow { name: m.name.clone(), have, need })
        })
        .collect()
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
        Refusal::NotInStock(p) => format!("NO {} IN STOCK", part(p)),
        Refusal::FleetNeeds { part: p, need, stock } => format!("THE FLEET CARRIES {need} {}  ·  {stock} IN STOCK", part(p)),
        Refusal::NoWorkshop => "THE CONTENT HAS NO WORKSHOP".into(),
        other => format!("{other:?}").to_uppercase(),
    }
}

/// What a dismantling gave, in words: `PROPULSION +15  ·  STEEL +6`.
pub(crate) fn gave_words(content: &Content, got: &HaulReport) -> String {
    let data = got.data.iter().map(|(f, x)| format!("{} +{}", content.families.get(f).map_or(f.as_str(), |f| f.name.as_str()).to_uppercase(), amount(*x)));
    let mats = got.materials.iter().map(|(m, n)| format!("{} +{n}", content.materials.get(m).map_or(m.as_str(), |m| m.name.as_str()).to_uppercase()));
    let all: Vec<String> = data.chain(mats).collect();
    if all.is_empty() { "NOTHING".into() } else { all.join("  ·  ") }
}

/// The WORKSHOP's state (a resource): open or not, what the last action said, the count being chosen and the part
/// whose materials the right column shows.
#[derive(Resource, Debug, Default)]
pub struct WorkshopLayer {
    pub open: bool,
    said: Option<(String, Tone)>,
    choice: Option<Choice>,
    shown: Option<(String, u32)>,
}

/// Is the WORKSHOP open (home's input waits then)?
pub fn is_open(layer: Res<WorkshopLayer>) -> bool {
    layer.open
}

#[derive(Component)]
struct WorkshopRoot;

/// The parts list (the kit's grouped list's area) and the lines over it (what the fleet is short of).
#[derive(Component)]
struct PartList;

#[derive(Component)]
struct PartsHead;

#[derive(Component)]
struct MaterialList;

#[derive(Component)]
struct Said;

#[derive(Component)]
struct CloseButton;

/// A part's row in the list (the pointer over it shows the part's materials).
#[derive(Component)]
struct PartRowOf(String);

/// A button's action: a row's MAKE or DISMANTLE (a count to choose), the count's − and + (by one, shift by five),
/// MAX, the confirming pill and CANCEL, and FABRICATE MISSING.
#[derive(Component, Clone, Debug, PartialEq)]
enum Do {
    Choose(Act, String),
    Less,
    More,
    Max,
    Confirm,
    Cancel,
    MakeMissing,
}

/// What changes the career: a chosen count, fabricating what the fleet lacks.
enum Work {
    Count(Choice),
    MakeMissing,
}

pub struct WorkshopPlugin;

impl Plugin for WorkshopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorkshopLayer>()
            .add_systems(Startup, spawn.after(world::flight_hud::spawn_hud))
            .add_systems(OnExit(AppMode::Home), |mut layer: ResMut<WorkshopLayer>| layer.open = false)
            .add_systems(Update, show.run_if(state_changed::<AppMode>.or_eager(resource_changed::<WorkshopLayer>)))
            .add_systems(Update, (open, input, hover).chain().after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Home)));
    }
}

/// The lists' nodes, for a refill.
type Lists<'w, 's> = (
    Single<'w, 's, Entity, With<PartList>>,
    Single<'w, 's, Entity, With<MaterialList>>,
    Single<'w, 's, Entity, With<Said>>,
    Single<'w, 's, Entity, With<PartsHead>>,
);

/// What a refill reads.
#[derive(bevy::ecs::system::SystemParam)]
struct Fill<'w> {
    content: Res<'w, GameContent>,
    design: Res<'w, PlayerDesign>,
    escorts: Res<'w, EscortDesigns>,
    fonts: Res<'w, HudFonts>,
    style: Res<'w, Style>,
}

/// The workshop (W): open, with the lists as they are now.
fn open(mut commands: Commands, mut actions: MessageReader<HomeAction>, mut layer: ResMut<WorkshopLayer>, career: Res<PlayerCareer>, fill: Fill, lists: Lists) {
    if !actions.read().any(|a| *a == HomeAction::Workshop) {
        return;
    }
    layer.said = None;
    layer.choice = None;
    layer.shown = None;
    refill(&mut commands, &lists, &fill, career.career(), &layer);
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
    pointer: Res<UiPointer>,
    mut typing: Local<bool>,
    fill: Fill,
    lists: Lists,
) {
    // The Esc that lets the search field go (it had the keyboard last frame) closes nothing.
    let was_typing = std::mem::replace(&mut *typing, pointer.typing);
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
    let c = &fill.content.0;
    let need = fleet_need(c, &fill.design, &fill.escorts);
    // Esc cancels a count being chosen first, then closes.
    let esc = keys.just_pressed(KeyCode::Escape) && !was_typing && !pointer.typing;
    let cancel = esc && layer.choice.is_some();
    let acted = todo.is_some() || cancel;
    if cancel {
        layer.choice = None;
    }
    let work = match todo {
        Some(Do::Choose(act, part)) => {
            // none to make or spare tries one, so the career says why not
            if most(c, career.career(), &need, act, &part) == 0 {
                Some(Work::Count(Choice { act, part, n: 1 }))
            } else {
                layer.choice = Some(Choice { n: first_count(c, career.career(), &need, act, &part), act, part });
                None
            }
        }
        Some(d @ (Do::Less | Do::More | Do::Max)) => {
            let step = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) { COUNT_STEP } else { 1 };
            if let Some(ch) = layer.choice.as_mut() {
                let top = most(c, career.career(), &need, ch.act, &ch.part).max(1);
                ch.n = match d {
                    Do::Less => ch.n.saturating_sub(step),
                    Do::More => ch.n + step,
                    _ => top,
                }
                .clamp(1, top);
            }
            None
        }
        Some(Do::Confirm) => layer.choice.take().map(Work::Count),
        Some(Do::Cancel) => {
            layer.choice = None;
            None
        }
        Some(Do::MakeMissing) => Some(Work::MakeMissing),
        None => None,
    };
    if let Some(w) = work {
        layer.choice = None;
        let name = |p: &str| c.parts.get(p).map_or_else(|| p.to_uppercase(), |x| x.name.to_uppercase());
        let (said, written) = career.update(&mut storage, |career| match &w {
            Work::Count(Choice { act, part: p, n }) => match act {
                Act::Make => career.fabricate(c, p, *n).map(|()| (format!("FABRICATED {n} {}  ·  IN STOCK", name(p)), Tone::Teal)),
                Act::Scrap => career.dismantle_stock(c, p, *n, &need).map(|got| (format!("DISMANTLED {n} {} FROM STOCK  ·  {}", name(p), gave_words(c, &got)), Tone::Gold)),
            },
            Work::MakeMissing => {
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
    }
    if acted {
        refill(&mut commands, &lists, &fill, career.career(), &layer);
    }
    if closed || (esc && !cancel) {
        layer.open = false;
    }
}

/// The part whose materials the right column shows: the one under the pointer (its row, its pills, its strip),
/// else the one a count is chosen for; how many with the count. Refills the column when it changes.
#[allow(clippy::too_many_arguments)]
fn hover(mut commands: Commands, rows: Query<(&PartRowOf, &Hot)>, buttons: Query<(&Do, &Hot)>, mut layer: ResMut<WorkshopLayer>, career: Res<PlayerCareer>, fill: Fill, lists: Lists) {
    if !layer.open {
        return;
    }
    let choice = layer.choice.as_ref();
    let counted = |part: &str| choice.filter(|c| c.part == part && c.act == Act::Make).map_or(1, |c| c.n);
    let over = rows.iter().find(|(_, hot)| hot.hovered).map(|(r, _)| r.0.clone()).or_else(|| {
        buttons.iter().find(|(_, hot)| hot.hovered).and_then(|(d, _)| match d {
            Do::Choose(_, part) => Some(part.clone()),
            Do::MakeMissing => None,
            _ => choice.map(|c| c.part.clone()),
        })
    });
    let shown = over.or_else(|| choice.map(|c| c.part.clone())).map(|p| {
        let n = counted(&p);
        (p, n)
    });
    if layer.shown != shown {
        layer.shown = shown;
        let ui = fill.style.ui;
        let mats = *lists.1;
        commands.entity(mats).despawn_children();
        commands.entity(mats).with_children(|p| fill_materials(p, &fill.fonts, &ui, &fill.content.0, career.career(), layer.shown.as_ref()));
    }
}

/// Shown at home while open, with its transition; home's words step aside meanwhile (`home::show`).
fn show(mode: Res<State<AppMode>>, layer: Res<WorkshopLayer>, mut root: Single<&mut ScreenFx, With<WorkshopRoot>>) {
    show_screen(&mut root, *mode.get() == AppMode::Home && layer.open);
}

/// The parts, the materials and the last action's words, anew.
fn refill(commands: &mut Commands, lists: &Lists, fill: &Fill, career: &Career, layer: &WorkshopLayer) {
    let (fonts, style, content) = (&*fill.fonts, &*fill.style, &fill.content.0);
    let need = fleet_need(content, &fill.design, &fill.escorts);
    let ui = style.ui;
    let (parts, mats, line, head) = (*lists.0, *lists.1, *lists.2, *lists.3);
    let strip = layer.choice.as_ref().map(|ch| Strip { choice: ch, top: most(content, career, &need, ch.act, &ch.part), words: choice_words(content, ch) });
    let short = missing(career, &need);
    let rows = make_rows(content, career, &need);
    commands.entity(head).despawn_children();
    commands.entity(head).with_children(|p| fill_head(p, fonts, style, content, &short));
    let cx = Rows { fonts, style, content, strip: strip.as_ref() };
    let list: Vec<ListRow> = rows
        .iter()
        .map(|r| ListRow { key: r.part.clone(), group: r.category.clone(), find: r.name.to_lowercase(), aside: false })
        .collect();
    fill_list(commands, parts, fonts, &ui, &list, |p, l| {
        let r = rows.iter().find(|r| r.part == l.key).expect("a row per list row");
        spawn_part_row(p, &cx, r)
    });
    commands.entity(mats).despawn_children();
    commands.entity(mats).with_children(|p| fill_materials(p, fonts, &ui, content, career, layer.shown.as_ref()));
    commands.entity(line).despawn_children();
    if let Some((words, tone)) = &layer.said {
        commands.entity(line).with_child(tracked(fonts, words, 9.5, ui_color(tone.color(&ui), 1.0), 0.18));
    }
}

/// The count being chosen as its strip shows it: the choice, the most it may reach, what it costs or gives.
struct Strip<'a> {
    choice: &'a Choice,
    top: u32,
    words: String,
}

/// What filling the parts list needs besides the rows.
struct Rows<'a> {
    fonts: &'a HudFonts,
    style: &'a Style,
    content: &'a Content,
    strip: Option<&'a Strip<'a>>,
}

/// The strip under a row whose count is being chosen: − n + and MAX, the confirming pill (MAKE 3) and CANCEL, and
/// under them what it costs or gives in all.
fn spawn_strip(p: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, s: &Strip) {
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    let (verb, tone) = match s.choice.act {
        Act::Make => ("MAKE", Tone::Teal),
        Act::Scrap => ("DISMANTLE", Tone::Gold),
    };
    let n = s.choice.n;
    p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(6), padding: UiRect { left: px(44), bottom: px(10), ..default() }, ..default() }).with_children(|col| {
        col.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8), ..default() }).with_children(|r| {
            r.spawn((ring_node(22.0), Do::Less, Tip::new("ONE LESS  ·  SHIFT: FIVE"))).insert(Ring { disabled: n <= 1, active: false }).with_child(ring_glyph(Glyph::Minus, 10.0));
            r.spawn((number(fonts, &n.to_string(), 14.0, c(Tone::Number)), Node { min_width: px(26), justify_content: JustifyContent::Center, ..default() }));
            r.spawn((ring_node(22.0), Do::More, Tip::new("ONE MORE  ·  SHIFT: FIVE"))).insert(Ring { disabled: n >= s.top, active: false }).with_child(ring_glyph(Glyph::Plus, 10.0));
            spawn_pill(r, fonts, "MAX", 8.5, 22.0, None, Pill { disabled: n >= s.top, ..Pill::new(Tone::Teal) }, (Do::Max, Tip::new(format!("AS MANY AS CAN BE  ·  {}", s.top))));
            r.spawn(Node { flex_grow: 1.0, ..default() });
            spawn_pill(r, fonts, &format!("{verb} {n}"), 9.0, 26.0, None, Pill { active: true, disabled: s.top == 0, ..Pill::new(tone) }, (Do::Confirm, Tip::new("CONFIRM")));
            spawn_pill(r, fonts, "CANCEL", 9.0, 26.0, None, Pill::new(Tone::Teal), (Do::Cancel, Tip::key("CANCEL", "ESC")));
        });
        col.spawn(tracked(fonts, &s.words, 8.5, c(tone), 0.16));
    });
}

/// A part's icon as the drydock's palette shows it (hull parts in the first armour), in a list row.
pub(crate) fn icon(content: &Content, part: &str) -> (UiIcon, Node) {
    icon_sized(content, part, Vec2::new(32.0, 28.0))
}

/// A part's icon in a node of `size` (logical px).
pub(crate) fn icon_sized(content: &Content, part: &str, size: Vec2) -> (UiIcon, Node) {
    let armor = content.materials.iter().find(|m| m.armor.is_some()).map_or("", |m| m.id.as_str());
    (part_icon(content, part, armor), Node { width: px(size.x), height: px(size.y), flex_shrink: 0.0, ..default() })
}

/// Over the parts list: what the fleet is short of and FABRICATE MISSING.
fn fill_head(p: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, content: &Content, short: &[(String, u32, u32)]) {
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    p.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(14), flex_shrink: 0.0, ..default() }).with_children(|h| {
        if short.is_empty() {
            h.spawn(label(fonts, "PARTS  ·  THE FLEET IS IN STOCK", 9.5, c(Tone::Teal)));
        } else {
            let n: u32 = short.iter().map(|(_, need, have)| need - have).sum();
            h.spawn(label(fonts, &format!("PARTS  ·  THE FLEET IS SHORT OF {n}"), 9.5, c(Tone::Coral)));
            spawn_pill(h, fonts, "FABRICATE MISSING", 9.0, 26.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, (Do::MakeMissing, Tip::new("FABRICATE WHAT THE FLEET LACKS")));
        }
    });
    if !short.is_empty() {
        let words: Vec<String> = short
            .iter()
            .map(|(part, need, have)| format!("{} {have} / {need}", content.parts.get(part).map_or_else(|| part.to_uppercase(), |p| p.name.to_uppercase())))
            .collect();
        p.spawn((tracked(fonts, &words.join("  ·  "), 8.5, c(Tone::Coral), 0.16), Node { flex_shrink: 0.0, ..default() }));
    }
}

/// A part's row in the list: its icon, name and stock, why it can't be made, MAKE and DISMANTLE, and the strip of a
/// count being chosen for it.
fn spawn_part_row(p: &mut ChildSpawnerCommands, cx: &Rows, r: &MakeRow) -> Entity {
    let (fonts, style, content) = (cx.fonts, cx.style, cx.content);
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    let strip = cx.strip.filter(|s| s.choice.part == r.part);
    let on = |act: Act| strip.is_some_and(|s| s.choice.act == act);
    let wrap = (
        PartRowOf(r.part.clone()),
        PartHover { part: r.part.clone(), material: None, held: Held::Fleet },
        Hot::default(),
        Node { flex_shrink: 0.0, flex_direction: FlexDirection::Column, border: UiRect::bottom(px(1)), ..default() },
        BorderColor::all(ui_color(ui.dim, 0.25)),
    );
    p.spawn(wrap)
        .with_children(|wrap| {
            wrap.spawn(Node { min_height: px(ROW), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(12), ..default() }).with_children(|row| {
                row.spawn(icon(content, &r.part));
                row.spawn((prose(fonts, &r.name, 15.0, c(Tone::Number)), Node { width: px(150), flex_shrink: 0.0, ..default() }));
                let (stock, tone) = if r.need > 0 { (format!("{} / {}", r.stock, r.need), if r.stock < r.need { Tone::Coral } else { Tone::Teal }) } else { (r.stock.to_string(), Tone::Number) };
                row.spawn((number(fonts, &stock, 14.0, c(tone)), Node { width: px(56), flex_shrink: 0.0, ..default() }));
                let why = r.why.as_ref().filter(|w| !w.starts_with("SHORT OF")).cloned().unwrap_or_default();
                row.spawn((label(fonts, &why, 8.0, c(Tone::Dim)), Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() }));
                let make = Pill { disabled: r.make == 0, active: on(Act::Make), ..Pill::new(Tone::Teal) };
                spawn_pill(row, fonts, "MAKE", 9.0, 26.0, None, make, (Do::Choose(Act::Make, r.part.clone()), Tip::new("FABRICATE  ·  HOW MANY")));
                let tip = if r.spare > 0 {
                    "DISMANTLE FROM STOCK  ·  MATERIALS, NO DATA".to_string()
                } else if r.stock > 0 {
                    format!("THE FLEET CARRIES ALL {}", r.stock)
                } else {
                    "NONE IN STOCK".to_string()
                };
                let scrap = Pill { disabled: r.spare == 0, active: on(Act::Scrap), ..Pill::new(Tone::Gold) };
                spawn_pill(row, fonts, "DISMANTLE", 9.0, 26.0, None, scrap, (Do::Choose(Act::Scrap, r.part.clone()), Tip::new(tip)));
            });
            if let Some(s) = strip {
                spawn_strip(wrap, fonts, style, s);
            }
        })
        .id()
}

/// The materials column: a row per material in store (and per one the shown part needs), and what that part needs
/// beside each, coral where the store is short; the others dim while a part is shown.
fn fill_materials(p: &mut ChildSpawnerCommands, fonts: &HudFonts, ui: &kit::style::UiPalette, content: &Content, career: &Career, shown: Option<&(String, u32)>) {
    let c = |tone: Tone| ui_color(tone.color(ui), 1.0);
    p.spawn((label(fonts, "MATERIALS IN STORE", 9.5, c(Tone::Label)), Node { margin: UiRect::bottom(px(4)), flex_shrink: 0.0, ..default() }));
    let head = match shown {
        Some((part, n)) => {
            let name = content.parts.get(part).map_or_else(|| part.to_uppercase(), |p| p.name.to_uppercase());
            match content.parts.get(part).and_then(|p| p.recipe.as_ref()) {
                Some(_) if *n > 1 => (format!("{n} {name}  ·  NEED"), Tone::Teal),
                Some(_) => (format!("{name}  ·  NEEDS"), Tone::Teal),
                None => (format!("{name}  ·  NO RECIPE"), Tone::Dim),
            }
        }
        None => ("POINT AT A PART FOR WHAT IT NEEDS".into(), Tone::Dim),
    };
    p.spawn((label(fonts, &head.0, 8.5, c(head.1)), Node { margin: UiRect::bottom(px(8)), flex_shrink: 0.0, ..default() }));
    let rows = material_rows(content, career, shown.map(|(p, n)| (p.as_str(), *n)));
    if rows.is_empty() {
        p.spawn(label(fonts, "NO MATERIALS IN STORE", 9.0, c(Tone::Dim)));
    }
    let row = Node { min_height: px(30), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), border: UiRect::bottom(px(1)), ..default() };
    for m in rows {
        let lit = shown.is_none() || m.need.is_some();
        p.spawn((row.clone(), BorderColor::all(ui_color(ui.dim, 0.2)))).with_children(|r| {
            // One line: a long name is cut at its room, never wrapped (the column is narrow beside the hover card's room).
            r.spawn(Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), overflow: Overflow::clip_x(), ..default() })
                .with_child((label(fonts, &m.name.to_uppercase(), 9.0, c(if lit { Tone::Label } else { Tone::Dim })), TextLayout::no_wrap()));
            r.spawn((number(fonts, &m.have.to_string(), 15.0, c(if lit { Tone::Number } else { Tone::Dim })), Node { min_width: px(36), justify_content: JustifyContent::FlexEnd, ..default() }));
            let (need, tone) = match m.need {
                Some(n) => (format!("NEEDS {n}"), if m.have < n { Tone::Coral } else { Tone::Teal }),
                None => (String::new(), Tone::Dim),
            };
            r.spawn((label(fonts, &need, 8.5, c(tone)), Node { width: px(76), flex_shrink: 0.0, ..default() }));
        });
    }
}

fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, content: Res<GameContent>) {
    let ui = style.ui;
    let fonts = &*fonts;
    let abs = |at: Vec2, size: Option<Vec2>| Node {
        position_type: PositionType::Absolute,
        left: px(at.x),
        top: px(at.y),
        width: size.map_or(Val::Auto, |s| px(s.x)),
        height: size.map_or(Val::Auto, |s| px(s.y)),
        ..default()
    };
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
                    // The parts: what the fleet is short of, then the kit's grouped list (search, chips, sections).
                    stage.spawn((ScreenPart::new(Edge::Fade, 1), Node { flex_direction: FlexDirection::Column, row_gap: px(10), ..abs(PARTS.0, Some(PARTS.1)) })).with_children(|col| {
                        col.spawn((PartsHead, Node { flex_direction: FlexDirection::Column, row_gap: px(8), flex_shrink: 0.0, ..default() }));
                        let spec = ListSpec {
                            placeholder: "Search parts".into(),
                            max_chars: 24,
                            groups: content.0.categories.iter().map(|c| (c.id.clone(), c.name.clone())).collect(),
                            aside: "",
                            row_gap: 0.0,
                            pad_right: 14.0,
                        };
                        let area = spawn_grouped_list(col, fonts, &ui, spec, Tip::new("SEARCH PARTS"), |_| {});
                        col.commands().entity(area).insert(PartList);
                    });
                    stage.spawn((MaterialList, ScreenPart::new(Edge::Right, 1), Scrollbar::vertical(), ScrollPosition::default(), list(MATERIALS)));
                    stage.spawn((Said, ScreenPart::new(Edge::Bottom, 0), abs(Vec2::new(40.0, 664.0), None)));
                });
            });
            // Top left: the way back, where we are; top right: CLOSE (the frame's corners).
            let f = frame();
            root.spawn((ScreenPart::new(Edge::Top, 0), f.top_left())).with_children(|tl| {
                spawn_head(tl, fonts, &ui, (CloseButton, Tip::key("SHIPYARD", "ESC")), "SHIPYARD  ›  WORKSHOP", "Workshop");
                crate::evolution::spawn_chip(tl, fonts);
            });
            root.spawn((ScreenPart::new(Edge::Top, 0), f.top_right())).with_children(|tr| {
                spawn_close(tr, fonts, CloseButton);
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::career::{Haul, HaulItem, VoyageEnd};

    /// A haul of intact thrusters and a pulse laser: the report's rows and suggestion, what the fleet lacks, why a
    /// part can't be made, in words, and the materials a part needs.
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
        let short_first = |cat: &str| {
            let section: Vec<&MakeRow> = make.iter().filter(|r| r.category == cat).collect();
            section.windows(2).all(|w| w[0].need > w[0].stock || w[1].need <= w[1].stock)
        };
        assert!(content.categories.iter().all(|c| short_first(&c.id)), "in each section what the fleet lacks comes first");
        let order: Vec<usize> = make.iter().map(|r| content.categories.iter().position(|c| c.id == r.category).unwrap()).collect();
        assert!(order.windows(2).all(|w| w[0] <= w[1]), "the sections in content order");
        assert!(make.iter().any(|r| r.part == "capacitor" && r.need > r.stock));
        assert!(make.iter().all(|r| r.part != "beam_laser" && r.part != "armor_block"), "locked and hull parts are not offered");
        let laser = make.iter().find(|r| r.part == "pulse_laser").unwrap();
        assert_eq!(laser.why, None);
        // The materials column: what the store holds, and for two pulse lasers what they need.
        let mats = material_rows(&content, &c, Some(("pulse_laser", 2)));
        let need_of = |name: &str| mats.iter().find(|m| m.name == name).and_then(|m| m.need);
        assert_eq!((need_of("Copper"), need_of("Lattice crystal"), need_of("Steel"), need_of("Ceramic")), (Some(6), Some(4), Some(2), None));
        assert!(material_rows(&content, &c, None).iter().all(|m| m.need.is_none() && m.have > 0));
        assert_eq!(refusal_words(&content, &Refusal::NeedsFacility("fusion_workshop".into())), "NEEDS THE FUSION WORKSHOP");
        let short = Refusal::CannotPay(vec![Shortfall::Material { material: "copper".into(), have: 1, cost: 3 }]);
        assert_eq!(refusal_words(&content, &short), "SHORT OF COPPER 1 / 3");
        let fleet = Refusal::FleetNeeds { part: "thruster".into(), need: 2, stock: 2 };
        assert_eq!(refusal_words(&content, &fleet), "THE FLEET CARRIES 2 THRUSTER  ·  2 IN STOCK");
    }

    /// A count opens at the shortfall or one, reaches at most what the materials pay for or the fleet spares, and
    /// says what it costs or gives in all.
    #[test]
    fn counts_and_their_words() {
        let content = data::base_content();
        let c = Career::new(&content, "rim_awakening", 4207).unwrap();
        let skiff = content.ships.get("rim_skiff").unwrap();
        let need = fleet_parts(&content, [skiff, skiff]);
        let cap = need["capacitor"] - c.stock()["capacitor"];
        assert_eq!(first_count(&content, &c, &need, Act::Make, "capacitor"), cap.min(c.fabricable(&content, "capacitor")).max(1));
        assert_eq!(most(&content, &c, &need, Act::Scrap, "thruster"), 0, "the fleet carries every thruster in stock");
        assert_eq!(most(&content, &c, &BTreeMap::new(), Act::Scrap, "thruster"), 2);
        let make = Choice { act: Act::Make, part: "pulse_laser".into(), n: 2 };
        assert_eq!(choice_words(&content, &make), "COSTS COPPER 6  ·  LATTICE CRYSTAL 4  ·  STEEL 2");
        let scrap = Choice { act: Act::Scrap, part: "thruster".into(), n: 2 };
        assert_eq!(choice_words(&content, &scrap), "GIVES CERAMIC +2  ·  COPPER +1  ·  STEEL +2  ·  NO DATA");
    }
}
