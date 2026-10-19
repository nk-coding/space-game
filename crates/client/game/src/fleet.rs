//! FLEET (GDR-0065): a layer over home that shows the fleet on the dock. The
//! flagship is the drydock's working design; around it are the escort slots, each a saved design (`designs/`)
//! picked from the list at the bottom. The fleet's logic cores must fit the ship-mind's capacity: a pick that
//! would exceed it is refused (`CORES n / m` top right). Every change is written into the career's save
//! (`escorts`). Escorts fly the behavior their design names and follow the flagship's lock (the voyage, track 7H).
//!
//! ```text
//! (o) SHIPYARD › FLEET  [EVOLVING 12 / 50]                           CORES 2 / 3 ●●○   [CLOSE ESC]
//!     Fleet                 . · ˙ ˙ ˙ ˙ ˙ ˙ ˙ ˙ ˙ ˙ · .
//!  [ escort 1 ]────────────[      flagship      ]- - - - - -[ escort 2 ]
//!  ESCORT 1                 FLAGSHIP · THE DRYDOCK'S DESIGN   ESCORT 2 · 1 CORE FREE
//!  Drone wing  [REMOVE]     Rim skiff copy                    Picket
//!  1 CORE · FLIES ESCORT    2 CORES · CARGO 8                 PREVIEW · ENTER ADDS IT
//! YOUR DESIGNS · PICK ONE FOR ESCORT 2
//! [ card ][ card ][ card ][ card ]
//! ENTER ADDS PICKET AS ESCORT 2
//! ```
//!
//! Home's moorings (F, once the camera has flown there) open it; the ring button top left, CLOSE and Esc close it
//! (Esc first drops a preview). A click on a slot picks the slot; a click on a design (or ←/→ through the designs)
//! previews it in the picked slot, and Enter (or a second click on it) puts it there (the empty slot adds one);
//! REMOVE empties a slot. Shown slots: the escorts and one more while there is room ([`MAX_ESCORTS`]). A dotted
//! formation ring runs round the flagship, inside the slots, and a line from each escort to the flagship (dashed
//! from a preview). UI words are chrome in code, like home's.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use data::career::voyage_ready;
use data::{Blueprint, Content};
use kit::style::{Style, ui_color};
use kit::ui_icons::{Glyph, Stroke, UiIcon};
use kit::ui_kit::{
    CompactText, LayoutClass, HudFonts, Edge, Hot, Pill, ScreenFx, ScreenPart, ScrollRows, Scrollbar, Tip, Tone, UiClick, UiInputSet, show_screen, UiStyleSet, label, number, prose, set_color, set_text,
    spawn_brackets, spawn_close, spawn_head, spawn_pill, frame, tracked,
};
use world::career::{EscortDesigns, Granted, PlayerCareer, readiness_words, voyage_line};
use world::mode::AppMode;
use world::player::PlayerDesign;
use world::player::behaviors::PlayerBehaviors;
use world::player::design::flight_ready;
use world::player::library::{DesignDoc, parts_size};
use world::sim_bridge::GameContent;
use world::storage::{Document, Storage};

use crate::home::{COMPACT_TEXT, HomeAction, by_class};

/// Escort slots the layout has (placeholder: the slice's mind grows to 4 cores, so at most 3 escorts fly).
pub const MAX_ESCORTS: usize = 4;
/// Card nodes in the row's pool: a card node that shows another design builds its ship icon again (a blink), so
/// the pool moves on rarely, by many cards at once.
const CARD_POOL: usize = 12;

/// Where the layer's parts go (stage px). [`Lay::WIDE`]: a 1280 × 720 stage (home's UI scale grows it with the
/// window). [`Lay::COMPACT`]: a phone's 844 × 390 at `compact_scale` (docs/client.md "Phones"): the slots' words
/// beside their boxes (left of the left ones), a smaller flagship and ring, three narrower cards, no line about
/// escorts.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Lay {
    /// The phone's: a slot's words beside its box rather than under it, tighter cards.
    compact: bool,
    stage: Vec2,
    /// The flagship's box (centre and size) and the escort slots' boxes (centres, one size).
    flagship: (Vec2, Vec2),
    slots: [Vec2; MAX_ESCORTS],
    slot_box: Vec2,
    /// The dotted formation ring round the flagship and its words, inside the slots, clear of every slot's words
    /// (centre and radii).
    ring: (Vec2, Vec2),
    /// Design cards that fit side by side (the rest scroll by), a card's size and the gap after it, the row's top.
    cards: usize,
    card: Vec2,
    card_gap: f32,
    cards_y: f32,
    /// The flagship's words' top, the list's head's top, the status line's top; the stage's side margin.
    ship_words_y: f32,
    list_head_y: f32,
    status_y: f32,
    side: f32,
}

impl Lay {
    const WIDE: Lay = Lay {
        compact: false,
        stage: Vec2::new(1280.0, 720.0),
        flagship: (Vec2::new(640.0, 250.0), Vec2::new(320.0, 190.0)),
        slots: [Vec2::new(250.0, 215.0), Vec2::new(1030.0, 215.0), Vec2::new(250.0, 395.0), Vec2::new(1030.0, 395.0)],
        slot_box: Vec2::new(150.0, 96.0),
        ring: (Vec2::new(640.0, 300.0), Vec2::new(280.0, 205.0)),
        cards: 4,
        card: Vec2::new(292.0, 94.0),
        card_gap: 14.0,
        cards_y: 562.0,
        ship_words_y: 359.0,
        list_head_y: 540.0,
        status_y: 676.0,
        side: 36.0,
    };
    const COMPACT: Lay = Lay {
        compact: true,
        stage: Vec2::new(844.0, 390.0),
        flagship: (Vec2::new(422.0, 116.0), Vec2::new(160.0, 88.0)),
        slots: [Vec2::new(270.0, 104.0), Vec2::new(574.0, 104.0), Vec2::new(270.0, 190.0), Vec2::new(574.0, 190.0)],
        slot_box: Vec2::new(96.0, 60.0),
        ring: (Vec2::new(422.0, 132.0), Vec2::new(100.0, 110.0)),
        cards: 3,
        card: Vec2::new(262.0, 68.0),
        card_gap: 10.0,
        cards_y: 268.0,
        ship_words_y: 166.0,
        list_head_y: 250.0,
        status_y: 346.0,
        side: 16.0,
    };

    fn of(class: LayoutClass) -> Lay {
        if class.is_compact() { Lay::COMPACT } else { Lay::WIDE }
    }
}
/// The capacity dots top right.
const DOTS: usize = 8;

/// One of the player's saved designs, as FLEET lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct FleetDesign {
    pub id: String,
    pub blueprint: Blueprint,
    pub cores: u32,
    pub cargo: f32,
    /// Why it can't fly (the drydock's words), if it can't.
    pub grounded: Option<String>,
}

impl FleetDesign {
    pub fn of(content: &Content, id: String, blueprint: Blueprint) -> Self {
        let t = content.ship_totals(&blueprint);
        let report = sim::design_report(content, &blueprint, &sim::SimConfig::default());
        let grounded = flight_ready(&report).err();
        Self { id, blueprint, cores: t.logic_cores, cargo: t.cargo_capacity, grounded }
    }
}

/// Why FLEET refuses a pick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The design has no logic core: it would be salvage, not a ship.
    NoCore,
    /// The design can't fly (the drydock's reason).
    CantFly(String),
    /// The mind doesn't run that many cores: `needs` for the design, `free` left with the rest of the fleet.
    Cores { needs: u32, free: u32 },
    /// No such slot.
    NoSlot,
}

/// The escorts after putting `design` into `slot` (the slot after the last one adds an escort), if the ship-mind
/// runs the fleet then: the flagship's cores, every other escort's and the design's must fit `capacity`.
/// `escorts` are the escorts' ids and cores.
pub fn pick(escorts: &[(String, u32)], flagship_cores: u32, capacity: u32, slot: usize, design: &FleetDesign) -> Result<Vec<String>, Refused> {
    if slot > escorts.len() || slot >= MAX_ESCORTS {
        return Err(Refused::NoSlot);
    }
    if design.cores == 0 {
        return Err(Refused::NoCore);
    }
    if let Some(why) = &design.grounded {
        return Err(Refused::CantFly(why.clone()));
    }
    let others: u32 = escorts.iter().enumerate().filter(|(k, _)| *k != slot).map(|(_, e)| e.1).sum();
    let free = capacity.saturating_sub(flagship_cores + others);
    if design.cores > free {
        return Err(Refused::Cores { needs: design.cores, free });
    }
    let mut out: Vec<String> = escorts.iter().map(|e| e.0.clone()).collect();
    if slot == out.len() {
        out.push(design.id.clone());
    } else {
        out[slot].clone_from(&design.id);
    }
    Ok(out)
}

fn plural(n: u32, one: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {one}S") }
}

/// The name of the behavior an escort of design `bp` flies: the one it names when that flies (a content behavior,
/// or one of the player's without errors), else its faction's, as a voyage flies it (`behaviors::fallback`).
pub fn flies<'a>(content: &'a Content, behaviors: &'a PlayerBehaviors, bp: &Blueprint) -> Option<&'a str> {
    let named = bp.behavior.as_deref().and_then(|id| match behaviors.get(id) {
        Some(p) => p.runs().then_some(&p.behavior),
        None => content.behaviors.get(id),
    });
    let faction = || content.factions.get(&bp.faction).and_then(|f| content.behaviors.get(f.behavior.as_deref()?));
    named.or_else(faction).map(|b| b.name.as_str())
}

/// The formation's line from the escort slot `k` to the flagship (stage px): a curve leaving the slot's box and
/// the flagship's box sideways.
fn formation_line(lay: &Lay, k: usize) -> Vec<Vec2> {
    let (ship, size) = lay.flagship;
    let at = lay.slots[k];
    let side = if at.x < ship.x { 1.0 } else { -1.0 };
    let a = at + Vec2::new(side * lay.slot_box.x / 2.0, 0.0);
    let b = Vec2::new(ship.x - side * size.x / 2.0, ship.y + (at.y - lay.ring.0.y) * 0.4);
    let pull = Vec2::new((b.x - a.x) * 0.5, 0.0);
    (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            let u = 1.0 - t;
            a * (u * u * u) + (a + pull) * (3.0 * u * u * t) + (b - pull) * (3.0 * u * t * t) + b * (t * t * t)
        })
        .collect()
}

/// A refusal in words (uppercase).
pub fn refused_words(name: &str, r: &Refused) -> String {
    let name = name.to_uppercase();
    match r {
        Refused::NoCore => format!("{name} HAS NO LOGIC CORE"),
        Refused::CantFly(why) => format!("{name} CAN'T FLY  ·  {}", why.to_uppercase()),
        Refused::Cores { needs, free } => format!("{name} NEEDS {}  ·  {free} FREE", plural(*needs, "CORE")),
        Refused::NoSlot => "NO SLOT FOR AN ESCORT".into(),
    }
}

/// FLEET's state (a resource): open or not, the slot picks go to, the saved designs, the first card shown and
/// the last message.
#[derive(Resource, Debug, Default)]
pub struct Fleet {
    pub open: bool,
    slot: usize,
    designs: Vec<FleetDesign>,
    /// The first design in the cards' pool, and one past the last card in view.
    offset: usize,
    seen_to: usize,
    /// Show the cards from the first again (FLEET opened).
    top: bool,
    /// The design previewed in the picked slot (index into the designs): Enter puts it there.
    preview: Option<usize>,
    say: Option<(String, Tone)>,
}

#[derive(Component)]
struct FleetRoot;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum FleetButton {
    Close,
    Slot(usize),
    Remove(usize),
    Card(usize),
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum FleetText {
    Cores,
    ShipName,
    ShipSub,
    ShipVoyage,
    SlotHead(usize),
    SlotName(usize),
    SlotSub(usize),
    ListHead,
    CardName(usize),
    CardSub(usize),
    CardState(usize),
    Status,
}

/// The nodes shown or hidden with the slots and cards, and the icons.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum FleetNode {
    Slot(usize),
    Remove(usize),
    Card(usize),
    Dot(usize),
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum FleetIcon {
    Flagship,
    Slot(usize),
    Card(usize),
}

/// The card row (the wheel over it scrolls, and a drag on its scrollbar).
#[derive(Component)]
struct CardRow;

/// The formation ring and lines, a sketch over the whole stage.
#[derive(Component)]
struct Formation;

/// The veil that dims a slot's ship while it shows a preview (UI draws over the icons).
#[derive(Component)]
struct PreviewVeil(usize);

pub struct FleetPlugin;

impl Plugin for FleetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Fleet>()
            .add_systems(Startup, spawn.after(world::flight_hud::spawn_hud))
            .add_systems(OnExit(AppMode::Home), |mut fleet: ResMut<Fleet>| fleet.open = false)
            .add_systems(Update, show.run_if(state_changed::<AppMode>.or_eager(resource_changed::<Fleet>)))
            .add_systems(
                Update,
                (open, input, sync).chain().after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Home)),
            );
    }
}

/// Is FLEET open (home's input waits then)?
pub fn is_open(fleet: Res<Fleet>) -> bool {
    fleet.open
}

/// Home's moorings (F): open, with the saved designs read fresh.
fn open(
    mut actions: MessageReader<HomeAction>,
    mut fleet: ResMut<Fleet>,
    storage: Res<Storage>,
    content: Res<GameContent>,
    escorts: Res<EscortDesigns>,
) {
    if !actions.read().any(|a| *a == HomeAction::Fleet) {
        return;
    }
    let content = &content.0;
    let mut designs: Vec<(Option<u64>, FleetDesign)> = Vec::new();
    match storage.list(DesignDoc::NAMESPACE) {
        Ok(ids) => {
            for id in ids {
                if let Ok(Some(d)) = storage.read::<DesignDoc>(&id) {
                    designs.push((d.saved, FleetDesign::of(content, id, d.doc.0)));
                }
            }
            fleet.say = None;
        }
        Err(e) => fleet.say = Some((format!("YOUR DESIGNS DON'T READ: {e}").to_uppercase(), Tone::Coral)),
    }
    // Most recently saved first, as BLUEPRINTS lists them.
    designs.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.blueprint.name.cmp(&b.1.blueprint.name)));
    fleet.designs = designs.into_iter().map(|d| d.1).collect();
    fleet.slot = escorts.0.len().min(MAX_ESCORTS - 1);
    fleet.top = true;
    fleet.preview = None;
    fleet.open = true;
}

/// What a click or a key asks of FLEET.
enum Op {
    Button(FleetButton),
    /// Put the design (index into the designs) into the picked slot.
    Add(usize),
}

/// Clicks, keys (Esc, ←/→, Enter), and the cards the row shows (from its scroll: the kit's wheel and scrollbar move
/// it, and a preview stepped out of view).
#[allow(clippy::too_many_arguments)]
fn input(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(&FleetButton, Option<&Pill>)>,
    keys: Res<ButtonInput<KeyCode>>,
    row: Single<(&ComputedNode, &mut ScrollRows), With<CardRow>>,
    mut fleet: ResMut<Fleet>,
    mut career: ResMut<PlayerCareer>,
    mut storage: ResMut<Storage>,
    mut escorts: ResMut<EscortDesigns>,
    design: Res<PlayerDesign>,
    granted: Res<Granted>,
    content: Res<GameContent>,
    class: Res<LayoutClass>,
) {
    if !fleet.open {
        return;
    }
    let lay = Lay::of(*class);
    if keys.just_pressed(KeyCode::Escape) {
        match fleet.preview {
            Some(_) => fleet.preview = None,
            None => fleet.open = false,
        }
        return;
    }
    let (node, mut rows) = row.into_inner();
    if fleet.top {
        fleet.top = false;
        rows.scroll = 0.0;
    }
    let view = node.size.x * node.inverse_scale_factor;
    let step = keys.just_pressed(KeyCode::ArrowRight) as i32 - keys.just_pressed(KeyCode::ArrowLeft) as i32;
    if step != 0 && !fleet.designs.is_empty() {
        // From the preview, else from the first card in view; the row scrolls to show it.
        let pitch = lay.card.x + lay.card_gap;
        let last = fleet.designs.len() - 1;
        let next = match fleet.preview {
            Some(p) => (p as i32 + step).clamp(0, last as i32) as usize,
            None => ((rows.scroll / pitch).ceil() as usize).min(last),
        };
        fleet.preview = Some(next);
        fleet.say = None;
        let x = next as f32 * pitch;
        rows.scroll = rows.scroll.min(x).max(x + lay.card.x - view);
    }
    let w = ScrollRows::lay(&mut rows, &vec![lay.card.x; fleet.designs.len()], lay.card_gap, view, CARD_POOL);
    if (fleet.offset, fleet.seen_to) != (w.first, w.seen.1) {
        (fleet.offset, fleet.seen_to) = (w.first, w.seen.1);
    }
    let content = &content.0;
    let mut ops: Vec<Op> = clicks
        .read()
        .filter_map(|c| buttons.get(c.0).ok())
        .filter(|(_, pill)| !pill.is_some_and(|p| p.disabled))
        .map(|(b, _)| match *b {
            // A second click on the previewed design puts it there, like Enter.
            FleetButton::Card(k) if fleet.preview == Some(fleet.offset + k) => Op::Add(fleet.offset + k),
            b => Op::Button(b),
        })
        .collect();
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter])
        && let Some(p) = fleet.preview
    {
        ops.push(Op::Add(p));
    }
    for op in ops {
        let ids = career.career().escorts().to_vec();
        let changed = match op {
            Op::Button(b) => match b {
                FleetButton::Close => {
                    fleet.open = false;
                    return;
                }
                FleetButton::Slot(k) => {
                    fleet.slot = k.min(ids.len());
                    None
                }
                FleetButton::Remove(k) if k < ids.len() => {
                    let mut next = ids.clone();
                    let gone = escorts.0.get(k).map_or(next[k].clone(), |b| b.name.clone());
                    next.remove(k);
                    fleet.say = Some((format!("{} LEFT THE FLEET", gone.to_uppercase()), Tone::Teal));
                    fleet.slot = next.len().min(MAX_ESCORTS - 1);
                    Some(next)
                }
                FleetButton::Remove(_) => None,
                // A click on another design previews it in the picked slot.
                FleetButton::Card(k) => {
                    if fleet.designs.get(fleet.offset + k).is_some() {
                        fleet.preview = Some(fleet.offset + k);
                        fleet.say = None;
                    }
                    None
                }
            },
            Op::Add(i) => {
                let Some(d) = fleet.designs.get(i).cloned() else { continue };
                let with_cores: Vec<(String, u32)> =
                    ids.iter().zip(&escorts.0).map(|(id, b)| (id.clone(), content.ship_totals(b).logic_cores)).collect();
                let flagship = content.ship_totals(design.blueprint()).logic_cores;
                // The picked slot, as the layer shows it (escorts may have left since it was picked).
                let slot = fleet.slot.min(ids.len()).min(MAX_ESCORTS - 1);
                match pick(&with_cores, flagship, granted.mind_capacity, slot, &d) {
                    Ok(next) => {
                        fleet.preview = None;
                        fleet.say = Some((format!("{} FLIES AS ESCORT {}", d.blueprint.name.to_uppercase(), slot + 1), Tone::Teal));
                        fleet.slot = (slot + 1).min(next.len()).min(MAX_ESCORTS - 1);
                        Some(next)
                    }
                    Err(r) => {
                        fleet.say = Some((refused_words(&d.blueprint.name, &r), Tone::Coral));
                        None
                    }
                }
            }
        };
        if let Some(next) = changed {
            // The escorts' designs: as listed now, else as they were read (the career's ids and the designs align).
            let blueprints: Vec<Blueprint> = next
                .iter()
                .filter_map(|id| {
                    let listed = fleet.designs.iter().find(|d| d.id == *id).map(|d| d.blueprint.clone());
                    listed.or_else(|| ids.iter().position(|i| i == id).and_then(|k| escorts.0.get(k).cloned()))
                })
                .collect();
            let ((), written) = career.update(&mut storage, |c| c.set_escorts(next));
            escorts.0 = blueprints;
            if let Err(e) = written {
                fleet.say = Some((format!("NOT SAVED: {e}").to_uppercase(), Tone::Coral));
            }
        }
    }
}

/// What a design card says about putting it into the picked slot (`free`: the cores left for it there), and its
/// tone: refusals in coral, a locked part in gold (it may fly, but the fleet is not voyage-ready then), else
/// whether it fits.
pub fn card_state(content: &Content, granted: &Granted, d: &FleetDesign, in_fleet: bool, free: u32) -> (String, Tone) {
    if d.cores == 0 {
        return ("NO LOGIC CORE".into(), Tone::Coral);
    }
    if let Some(why) = &d.grounded {
        return (format!("CAN'T FLY  ·  {}", why.to_uppercase()), Tone::Coral);
    }
    if d.cores > free {
        return (format!("NEEDS {}  ·  {free} FREE", plural(d.cores, "CORE")), Tone::Coral);
    }
    if let Some(locked) = voyage_ready(content, &granted.unlocks, u32::MAX, None, &d.blueprint, &[]).iter().find(|r| r.blocks()) {
        return (readiness_words(content, locked), Tone::Gold);
    }
    if in_fleet { (format!("IN THE FLEET  ·  {}", plural(d.cores, "CORE")), Tone::Label) } else { (format!("FITS  ·  {}", plural(d.cores, "CORE")), Tone::Teal) }
}

/// FLEET is shown at home while it is open, with its transition (`ui_kit::ScreenFx`; it covers the station, which
/// fades out under it); home's words step aside meanwhile (`home::show`).
fn show(mode: Res<State<AppMode>>, fleet: Res<Fleet>, mut root: Single<&mut ScreenFx, With<FleetRoot>>) {
    show_screen(&mut root, *mode.get() == AppMode::Home && fleet.open);
}

fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, clear: Res<ClearColor>) {
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
    let column = |gap: f32, at: Node| Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(gap), ..at };
    let placeholder = |size: f32| UiIcon::Glyph { glyph: Glyph::Formation, color: ui.dim.with_alpha(size) };
    let boxed = |accent: Tone| Pill { bare: true, ..Pill::new(accent) };
    // Every part is laid out for both layout classes (`home::ByClass`).
    let (wl, cl) = (Lay::WIDE, Lay::COMPACT);
    commands
        .spawn((
            FleetRoot,
            CompactText(COMPACT_TEXT),
            world::backdrop::VeilsBackdrop,
            ScreenFx::closed().dimming(),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn(Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|centre| {
                let stage_node = |l: &Lay| Node { width: px(l.stage.x), height: px(l.stage.y), flex_shrink: 0.0, ..default() };
                centre.spawn(by_class(stage_node(&wl), stage_node(&cl))).with_children(|stage| {
                    // The formation ring and lines, under everything else.
                    stage.spawn((Formation, ScreenPart::new(Edge::Fade, 1), UiIcon::Sketch(Vec::new()), by_class(abs(Vec2::ZERO, Some(wl.stage)), abs(Vec2::ZERO, Some(cl.stage)))));
                    // On a phone the line about escorts is left out.
                    stage.spawn((
                        ScreenPart::new(Edge::Fade, 1),
                        prose(fonts, "Escorts fly the behavior their design names and follow the flagship's lock.", 12.0, c(Tone::Label, 1.0)),
                        by_class(abs(Vec2::new(28.0, 84.0), Some(Vec2::new(300.0, 40.0))), Node { display: Display::None, ..default() }),
                    ));

                    // The flagship on the dock.
                    let flagship = |l: &Lay| abs(l.flagship.0 - l.flagship.1 / 2.0, Some(l.flagship.1));
                    stage.spawn((ScreenPart::new(Edge::Fade, 1), by_class(flagship(&wl), flagship(&cl)))).with_children(|b| {
                        spawn_brackets(b, 16.0, 1.4, c(Tone::Teal, 0.55));
                        let icon = |l: &Lay, inset: Vec2| Node {
                            position_type: PositionType::Absolute,
                            left: px(inset.x),
                            top: px(inset.y),
                            width: px(l.flagship.1.x - 2.0 * inset.x),
                            height: px(l.flagship.1.y - 2.0 * inset.y),
                            ..default()
                        };
                        b.spawn((FleetIcon::Flagship, placeholder(0.5), by_class(icon(&wl, Vec2::new(24.0, 20.0)), icon(&cl, Vec2::new(14.0, 10.0)))));
                    });
                    let words = |l: &Lay, width: f32| column(if l.compact { 4.0 } else { 6.0 }, abs(Vec2::new(l.flagship.0.x - width / 2.0, l.ship_words_y), Some(Vec2::new(width, 90.0))));
                    stage.spawn((ScreenPart::new(Edge::Fade, 1), by_class(words(&wl, 480.0), words(&cl, 200.0))))
                        .with_children(|t| {
                            // On a phone the short form, between the lower slots.
                            let hidden = Node { display: Display::None, ..default() };
                            t.spawn((label(fonts, "FLAGSHIP  ·  THE DRYDOCK'S DESIGN", 8.5, c(Tone::Dim, 1.0)), by_class(Node::default(), hidden.clone())));
                            t.spawn((label(fonts, "FLAGSHIP", 8.5, c(Tone::Dim, 1.0)), by_class(hidden, Node::default())));
                            t.spawn((FleetText::ShipName, prose(fonts, "", 18.0, c(Tone::Number, 1.0))));
                            t.spawn((FleetText::ShipSub, label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                            t.spawn((FleetText::ShipVoyage, label(fonts, "", 8.5, c(Tone::Teal, 1.0))));
                        });

                    // The escort slots: their words under the box, on a phone beside it (left of the left ones).
                    for slot in 0..MAX_ESCORTS {
                        let left = wl.slots[slot].x < wl.stage.x / 2.0;
                        let side = if left { Edge::Left } else { Edge::Right };
                        // Gaps of 4 px: with 6 an upper slot's REMOVE ran into the box of the slot below it.
                        let wide = column(4.0, abs(Vec2::new(wl.slots[slot].x - 120.0, wl.slots[slot].y - wl.slot_box.y / 2.0), Some(Vec2::new(240.0, 170.0))));
                        const BESIDE: Vec2 = Vec2::new(302.0, 80.0);
                        let at = cl.slots[slot];
                        let x = if left { at.x + cl.slot_box.x / 2.0 - BESIDE.x } else { at.x - cl.slot_box.x / 2.0 };
                        let compact = Node {
                            flex_direction: if left { FlexDirection::RowReverse } else { FlexDirection::Row },
                            align_items: AlignItems::Center,
                            column_gap: px(10),
                            ..abs(Vec2::new(x, at.y - BESIDE.y / 2.0), Some(BESIDE))
                        };
                        let boxed_node = |l: &Lay| Node {
                            width: px(l.slot_box.x),
                            height: px(l.slot_box.y),
                            flex_shrink: 0.0,
                            border: UiRect::all(px(1.2)),
                            border_radius: BorderRadius::all(px(12)),
                            margin: UiRect::bottom(px(if l.compact { 0.0 } else { 2.0 })),
                            ..default()
                        };
                        let icon = |l: &Lay, inset: Vec2| Node {
                            position_type: PositionType::Absolute,
                            left: px(inset.x),
                            top: px(inset.y),
                            width: px(l.slot_box.x - 2.0 * inset.x),
                            height: px(l.slot_box.y - 2.0 * inset.y),
                            ..default()
                        };
                        let texts = |align: AlignItems| Node { flex_direction: FlexDirection::Column, align_items: align, row_gap: px(4), ..default() };
                        stage.spawn((FleetNode::Slot(slot), ScreenPart::new(side, 2), by_class(wide, compact)))
                            .with_children(|s| {
                                s.spawn((
                                    FleetButton::Slot(slot),
                                    boxed(Tone::Teal),
                                    Hot::default(),
                                    by_class(boxed_node(&wl), boxed_node(&cl)),
                                    BorderColor::all(Color::NONE),
                                    BackgroundColor(Color::NONE),
                                ))
                                .with_children(|b| {
                                    spawn_brackets(b, 10.0, 1.2, c(Tone::Label, 0.45));
                                    b.spawn((FleetIcon::Slot(slot), placeholder(0.6), by_class(icon(&wl, Vec2::new(16.0, 12.0)), icon(&cl, Vec2::new(10.0, 8.0)))));
                                    b.spawn((
                                        PreviewVeil(slot),
                                        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), border_radius: BorderRadius::all(px(12)), display: Display::None, ..default() },
                                        BackgroundColor(clear.0.with_alpha(0.5)),
                                    ));
                                });
                                let compact_align = if left { AlignItems::FlexEnd } else { AlignItems::FlexStart };
                                s.spawn(by_class(texts(AlignItems::Center), texts(compact_align))).with_children(|t| {
                                    t.spawn((FleetText::SlotHead(slot), label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                                    t.spawn((FleetText::SlotName(slot), prose(fonts, "", 15.0, c(Tone::Number, 1.0))));
                                    t.spawn((FleetText::SlotSub(slot), label(fonts, "", 8.0, c(Tone::Dim, 1.0))));
                                    t.spawn((FleetNode::Remove(slot), Node::default())).with_children(|r| {
                                        spawn_pill(r, fonts, "REMOVE", 8.0, 22.0, None, Pill::new(Tone::Coral), FleetButton::Remove(slot));
                                    });
                                });
                            });
                    }

                    // The player's designs.
                    let head = |l: &Lay| abs(Vec2::new(l.side, l.list_head_y), None);
                    stage.spawn((FleetText::ListHead, ScreenPart::new(Edge::Bottom, 3), label(fonts, "", 9.0, c(Tone::Dim, 1.0)), by_class(head(&wl), head(&cl))));
                    let row = |l: &Lay| Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: px(l.card_gap),
                                overflow: Overflow::scroll_x(),
                                ..abs(Vec2::new(l.side, l.cards_y), Some(Vec2::new(l.cards as f32 * (l.card.x + l.card_gap) - l.card_gap, l.card.y)))
                            };
                    stage
                        .spawn((CardRow, Scrollbar::horizontal().outside(), ScrollRows::default(), ScreenPart::new(Edge::Bottom, 3), by_class(row(&wl), row(&cl))))
                        .with_children(|row| {
                            let card = |l: &Lay| Node {
                                width: px(l.card.x),
                                height: px(l.card.y),
                                flex_shrink: 0.0,
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: px(if l.compact { 10.0 } else { 14.0 }),
                                padding: UiRect::horizontal(px(if l.compact { 10.0 } else { 12.0 })),
                                border: UiRect::all(px(1.2)),
                                border_radius: BorderRadius::all(px(14)),
                                ..default()
                            };
                            // The ship's box: its size, the icon's inset.
                            let ship = |size: Vec2, _: f32| Node { width: px(size.x), height: px(size.y), flex_shrink: 0.0, ..default() };
                            let icon = |size: Vec2, inset: f32| Node {
                                position_type: PositionType::Absolute,
                                left: px(inset),
                                top: px(inset),
                                width: px(size.x - 2.0 * inset),
                                height: px(size.y - 2.0 * inset),
                                ..default()
                            };
                            let (wide_ship, phone_ship) = ((Vec2::new(104.0, 72.0), 8.0), (Vec2::new(78.0, 54.0), 6.0));
                            for k in 0..CARD_POOL {
                                row.spawn((
                                    FleetNode::Card(k),
                                    FleetButton::Card(k),
                                    boxed(Tone::Teal),
                                    Hot::default(),
                                    by_class(card(&wl), card(&cl)),
                                    BorderColor::all(Color::NONE),
                                    BackgroundColor(Color::NONE),
                                ))
                                .with_children(|card| {
                                    card.spawn(by_class(ship(wide_ship.0, wide_ship.1), ship(phone_ship.0, phone_ship.1))).with_children(|b| {
                                        spawn_brackets(b, 8.0, 1.0, c(Tone::Label, 0.35));
                                        b.spawn((FleetIcon::Card(k), placeholder(0.5), by_class(icon(wide_ship.0, wide_ship.1), icon(phone_ship.0, phone_ship.1))));
                                    });
                                    let texts = |gap: f32| Node { flex_direction: FlexDirection::Column, row_gap: px(gap), flex_shrink: 1.0, min_width: px(0), ..default() };
                                    card.spawn(by_class(texts(6.0), texts(3.0)))
                                        .with_children(|t| {
                                            t.spawn((FleetText::CardName(k), prose(fonts, "", 14.0, c(Tone::Number, 1.0))));
                                            t.spawn((FleetText::CardSub(k), tracked(fonts, "", 7.5, c(Tone::Dim, 1.0), 0.15)));
                                            t.spawn((FleetText::CardState(k), tracked(fonts, "", 7.5, c(Tone::Teal, 1.0), 0.15)));
                                        });
                                });
                            }
                        });
                    let status = |l: &Lay| abs(Vec2::new(l.side, l.status_y), Some(Vec2::new(l.stage.x - 2.0 * l.side, 30.0)));
                    stage.spawn((FleetText::Status, ScreenPart::new(Edge::Bottom, 3), label(fonts, "", 9.5, c(Tone::Teal, 1.0)), by_class(status(&wl), status(&cl))));
                });
            });
            // Top left: the way back, where we are; top right: the mind's cores, CLOSE (the frame's corners, `ui_kit::frame`).
            let f = frame();
            root.spawn((ScreenPart::new(Edge::Top, 0), f.top_left())).with_children(|tl| {
                spawn_head(tl, fonts, &ui, (FleetButton::Close, Tip::key("SHIPYARD", "ESC")), "SHIPYARD  ›  FLEET", "Fleet");
                crate::evolution::spawn_chip(tl, fonts);
            });
            root.spawn((ScreenPart::new(Edge::Top, 0), f.top_right())).with_children(|tr| {
                tr.spawn(label(fonts, "CORES", 9.0, c(Tone::Label, 1.0)));
                tr.spawn((FleetText::Cores, number(fonts, "", 17.0, c(Tone::Violet, 1.0))));
                tr.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(5), margin: UiRect::right(px(14)), ..default() }).with_children(|dots| {
                    for k in 0..DOTS {
                        dots.spawn((
                            FleetNode::Dot(k),
                            Node { width: px(8), height: px(8), border: UiRect::all(px(1.2)), border_radius: BorderRadius::MAX, ..default() },
                            BorderColor::all(c(Tone::Violet, 0.9)),
                            BackgroundColor(Color::NONE),
                        ));
                    }
                });
                spawn_close(tr, fonts, FleetButton::Close);
            });
        });
}

/// The layer follows the fleet: texts, icons, which slots and cards show, the dots, the preview, the formation.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync(
    fleet: Res<Fleet>,
    career: Res<PlayerCareer>,
    granted: Res<Granted>,
    escorts: Res<EscortDesigns>,
    design: Res<PlayerDesign>,
    behaviors: Res<PlayerBehaviors>,
    content: Res<GameContent>,
    style: Res<Style>,
    mut texts: Query<(&FleetText, &mut Text, &mut TextColor)>,
    mut icons: Query<(&FleetIcon, &mut UiIcon), Without<Formation>>,
    mut formation: Single<&mut UiIcon, With<Formation>>,
    mut nodes: Query<(&FleetNode, &mut Node, Option<&mut BackgroundColor>)>,
    mut veils: Query<(&PreviewVeil, &mut Node), Without<FleetNode>>,
    mut pills: Query<(&FleetButton, &mut Pill)>,
    class: Res<LayoutClass>,
) {
    if !fleet.open
        || !(fleet.is_changed() || class.is_changed() || career.is_changed() || granted.is_changed() || escorts.is_changed() || design.is_changed() || behaviors.is_changed())
    {
        return;
    }
    let lay = Lay::of(*class);
    let content = &content.0;
    let ui = &style.ui;
    let ids = career.career().escorts();
    let cores_of = |b: &Blueprint| content.ship_totals(b).logic_cores;
    let flagship = design.blueprint();
    let ship = content.ship_totals(flagship);
    let escort_cores: Vec<u32> = escorts.0.iter().map(cores_of).collect();
    let used = ship.logic_cores + escort_cores.iter().sum::<u32>();
    let capacity = granted.mind_capacity;
    let slots = (escorts.0.len() + 1).min(MAX_ESCORTS);
    let picked = fleet.slot.min(slots - 1);
    let free = |slot: usize| {
        let others: u32 = escort_cores.iter().enumerate().filter(|(k, _)| *k != slot).map(|(_, c)| c).sum();
        capacity.saturating_sub(ship.logic_cores + others)
    };
    let card = |k: usize| fleet.designs.get(fleet.offset + k);
    let cargo = |c: f32| data::design::stats::format_number(c, 0);
    // The design previewed in the picked slot, and what putting it there would do.
    let previewed = fleet.preview.and_then(|p| fleet.designs.get(p));
    let with_cores: Vec<(String, u32)> = ids.iter().cloned().zip(escort_cores.iter().copied()).collect();
    let preview_pick = previewed.map(|d| pick(&with_cores, ship.logic_cores, capacity, picked, d));
    let previewing = |k: usize| k == picked && previewed.is_some();

    for (t, mut text, mut color) in &mut texts {
        let (s, tone) = match *t {
            FleetText::Cores => (format!("{used} / {capacity}"), if used > capacity { Tone::Coral } else { Tone::Violet }),
            FleetText::ShipName => (flagship.name.clone(), Tone::Number),
            FleetText::ShipSub => (format!("{}  ·  CARGO {}", plural(ship.logic_cores, "CORE"), cargo(ship.cargo_capacity)), Tone::Dim),
            FleetText::ShipVoyage => voyage_line(content, &granted, flagship, &escorts.0),
            FleetText::SlotHead(k) if k < escorts.0.len() => (format!("ESCORT {}", k + 1), if k == picked { Tone::Teal } else { Tone::Dim }),
            FleetText::SlotHead(k) => match free(k) {
                0 => (format!("ESCORT {}  ·  NO CORE FREE", k + 1), Tone::Coral),
                n => (format!("ESCORT {}  ·  {} FREE", k + 1, plural(n, "CORE")), Tone::Teal),
            },
            FleetText::SlotName(k) => match (previewed.filter(|_| previewing(k)), escorts.0.get(k)) {
                (Some(d), _) => (d.blueprint.name.clone(), Tone::Number),
                (None, Some(b)) => (b.name.clone(), Tone::Number),
                (None, None) => ("Empty".into(), Tone::Dim),
            },
            FleetText::SlotSub(k) => match (previewed.filter(|_| previewing(k)), escorts.0.get(k)) {
                (Some(d), _) => match card_state(content, &granted, d, ids.contains(&d.id), free(picked)) {
                    (why, Tone::Coral) => (why, Tone::Coral),
                    (_, tone) => ("PREVIEW  ·  ENTER ADDS IT".into(), if tone == Tone::Gold { Tone::Gold } else { Tone::Teal }),
                },
                (None, Some(b)) => {
                    let behavior = flies(content, &behaviors, b).unwrap_or("no behavior").to_uppercase();
                    (format!("{}  ·  FLIES {behavior}", plural(cores_of(b), "CORE")), Tone::Dim)
                }
                (None, None) => ("PICK A DESIGN BELOW".into(), Tone::Dim),
            },
            FleetText::ListHead if fleet.designs.is_empty() => ("YOUR DESIGNS  ·  NONE SAVED YET  ·  SAVE ONE IN THE DRYDOCK".into(), Tone::Dim),
            FleetText::ListHead => {
                let mut s = format!("YOUR DESIGNS  ·  PICK ONE FOR ESCORT {}", picked + 1);
                if fleet.designs.len() > lay.cards {
                    s.push_str(&format!("  ·  {} OF {}", fleet.seen_to, fleet.designs.len()));
                }
                (s, Tone::Dim)
            }
            FleetText::CardName(k) => (card(k).map_or(String::new(), |d| d.blueprint.name.clone()), Tone::Number),
            FleetText::CardSub(k) => match card(k) {
                Some(d) => {
                    let (w, h) = parts_size(content, &d.blueprint);
                    (format!("{w}×{h}  ·  {}  ·  CARGO {}", plural(d.cores, "CORE"), cargo(d.cargo)), Tone::Dim)
                }
                None => (String::new(), Tone::Dim),
            },
            FleetText::CardState(k) => match card(k) {
                Some(d) => card_state(content, &granted, d, ids.contains(&d.id), free(picked)),
                None => (String::new(), Tone::Dim),
            },
            FleetText::Status => match (&fleet.say, previewed.zip(preview_pick.as_ref())) {
                (Some((s, tone)), _) => (s.clone(), *tone),
                (None, Some((d, Ok(_)))) => (format!("ENTER ADDS {} AS ESCORT {}", d.blueprint.name.to_uppercase(), picked + 1), Tone::Teal),
                (None, Some((d, Err(r)))) => (refused_words(&d.blueprint.name, r), Tone::Coral),
                (None, None) => (format!("CLICK A DESIGN TO PREVIEW IT AS ESCORT {}  ·  THE ARROW KEYS STEP THROUGH THEM", picked + 1), Tone::Label),
            },
        };
        set_text(&mut text, &s);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
    }
    for (i, mut icon) in &mut icons {
        let want = match *i {
            FleetIcon::Flagship => Some(flagship),
            FleetIcon::Slot(k) => previewed.filter(|_| previewing(k)).map(|d| &d.blueprint).or(escorts.0.get(k)),
            FleetIcon::Card(k) => card(k).map(|d| &d.blueprint),
        };
        let want = match want {
            Some(b) => UiIcon::Ship(Box::new(b.clone())),
            None => UiIcon::Glyph { glyph: Glyph::Formation, color: ui.dim.with_alpha(0.6) },
        };
        if *icon != want {
            *icon = want;
        }
    }
    for (n, mut node, bg) in &mut nodes {
        let show = match *n {
            FleetNode::Slot(k) => k < slots,
            FleetNode::Remove(k) => k < escorts.0.len(),
            FleetNode::Card(k) => card(k).is_some(),
            FleetNode::Dot(k) => k < (capacity as usize).max(used as usize).min(DOTS),
        };
        let d = if show { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
        if let (FleetNode::Dot(k), Some(mut bg)) = (*n, bg) {
            let fill = if (k as u32) < used { ui_color(if k as u32 >= capacity { ui.coral } else { ui.violet }, 0.95) } else { Color::NONE };
            if bg.0 != fill {
                bg.0 = fill;
            }
        }
    }
    for (v, mut node) in &mut veils {
        let d = if previewing(v.0) { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
    for (b, mut pill) in &mut pills {
        let active = match *b {
            FleetButton::Slot(k) => k == picked,
            FleetButton::Card(k) => fleet.preview == Some(fleet.offset + k),
            _ => continue,
        };
        if pill.active != active {
            pill.active = active;
        }
    }
    // The formation: the dotted ring, a line from each escort to the flagship, dashed from the preview (coral when
    // it would be refused).
    let mut strokes = vec![Stroke::Dots { centre: lay.ring.0, radii: lay.ring.1, step: 7.0, dot: 1.6, color: ui.label.with_alpha(0.4) }];
    for k in 0..escorts.0.len().min(MAX_ESCORTS) {
        if !previewing(k) {
            strokes.push(Stroke::Line { points: formation_line(&lay, k), width: 1.2, dash: None, color: ui.teal.with_alpha(0.45) });
        }
    }
    if let Some(p) = &preview_pick {
        let color = if p.is_ok() { ui.teal.with_alpha(0.6) } else { ui.coral.with_alpha(0.5) };
        strokes.push(Stroke::Line { points: formation_line(&lay, picked), width: 1.2, dash: Some((5.0, 4.0)), color });
    }
    let sketch = UiIcon::Sketch(strokes);
    if **formation != sketch {
        **formation = sketch;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn design(content: &Content, id: &str, ship: &str) -> FleetDesign {
        FleetDesign::of(content, id.into(), content.ships.get(ship).unwrap().clone())
    }

    /// Picks fill the picked slot or add one; the mind refuses what doesn't fit, counting the slot's own escort out.
    #[test]
    fn picks_must_fit_the_mind() {
        let content = data::base_content();
        let skiff = design(&content, "skiff", "rim_skiff");
        assert_eq!(skiff.cores, 2);
        // A new game: the 2-core skiff fills a mind of 2.
        assert_eq!(pick(&[], 2, 2, 0, &skiff), Err(Refused::Cores { needs: 2, free: 0 }));
        assert_eq!(refused_words("Rim skiff", &Refused::Cores { needs: 2, free: 0 }), "RIM SKIFF NEEDS 2 CORES  ·  0 FREE");
        // A mind of 4: one skiff escort fits, a second one doesn't.
        let one = pick(&[], 2, 4, 0, &skiff).unwrap();
        assert_eq!(one, ["skiff"]);
        let with = [("skiff".to_string(), 2)];
        assert_eq!(pick(&with, 2, 4, 1, &skiff), Err(Refused::Cores { needs: 2, free: 0 }));
        // Replacing the escort in its own slot frees its cores first.
        let other = FleetDesign { id: "other".into(), ..skiff.clone() };
        assert_eq!(pick(&with, 2, 4, 0, &other).unwrap(), ["other"]);
        // No slot past the end, nor past the layout.
        assert_eq!(pick(&with, 2, 9, 3, &skiff), Err(Refused::NoSlot));
        let full: Vec<(String, u32)> = (0..MAX_ESCORTS).map(|k| (format!("e{k}"), 1)).collect();
        assert_eq!(pick(&full, 1, 99, MAX_ESCORTS, &skiff), Err(Refused::NoSlot));
        // Ships without a core, or that can't fly, never join.
        let mut hulk = skiff.clone();
        hulk.cores = 0;
        assert_eq!(pick(&[], 2, 9, 0, &hulk), Err(Refused::NoCore));
        let grounded = FleetDesign { grounded: Some("no thrust".into()), ..skiff.clone() };
        assert_eq!(pick(&[], 2, 9, 0, &grounded), Err(Refused::CantFly("no thrust".into())));
    }

    /// An escort flies the behavior its design names when that flies (a content one here), else its faction's.
    #[test]
    fn escorts_fly_their_designs_behavior() {
        let content = data::base_content();
        let none = PlayerBehaviors::default();
        let mut skiff = content.ships.get("rim_skiff").unwrap().clone();
        skiff.behavior = None;
        let faction = content.factions.get(&skiff.faction).and_then(|f| f.behavior.clone()).unwrap();
        assert_eq!(flies(&content, &none, &skiff), Some(content.behaviors.get(&faction).unwrap().name.as_str()));
        let other = content.behaviors.iter().find(|b| b.id != faction).unwrap();
        skiff.behavior = Some(other.id.clone());
        assert_eq!(flies(&content, &none, &skiff), Some(other.name.as_str()));
        skiff.behavior = Some("gone".into());
        assert_eq!(flies(&content, &none, &skiff), Some(content.behaviors.get(&faction).unwrap().name.as_str()));
    }

    /// The formation's lines leave each slot's box on the flagship's side and end on the flagship's box; the ring
    /// holds the flagship's box and leaves every slot's box outside.
    #[test]
    fn formation_lines_join_the_boxes() {
        for lay in [Lay::WIDE, Lay::COMPACT] {
            let (ship, size) = lay.flagship;
            let inside = |p: Vec2| ((p - lay.ring.0) / lay.ring.1).length_squared() < 1.0;
            let corners = |c: Vec2, s: Vec2| [c - s / 2.0, c + s / 2.0, c + Vec2::new(s.x, -s.y) / 2.0, c - Vec2::new(s.x, -s.y) / 2.0];
            assert!(corners(ship, size).into_iter().all(inside));
            assert!(lay.slots.iter().all(|&at| corners(at, lay.slot_box).into_iter().all(|p| !inside(p))));
            for (k, at) in lay.slots.iter().enumerate() {
                let line = formation_line(&lay, k);
                let (a, b) = (line[0], line[line.len() - 1]);
                assert!((a.x - at.x).abs() == lay.slot_box.x / 2.0 && a.y == at.y, "{k} {a}");
                assert_eq!((b.x - ship.x).abs(), size.x / 2.0, "{k}");
                assert!((b.y - ship.y).abs() < size.y / 2.0 && (a.x - ship.x).abs() > (b.x - ship.x).abs(), "{k} {b}");
            }
        }
    }

    /// A card says why a pick would be refused, warns about locked parts, and says when it fits.
    #[test]
    fn cards_say_what_a_pick_would_do() {
        let content = data::base_content();
        let career = data::Career::new(&content, "rim_awakening", 1).unwrap();
        let granted = Granted { unlocks: career.unlocks(&content), mind_capacity: 4, ..Default::default() };
        let skiff = design(&content, "skiff", "rim_skiff");
        let runner = design(&content, "runner", "halvex_runner");
        assert_eq!(card_state(&content, &granted, &skiff, false, 2), ("FITS  ·  2 CORES".into(), Tone::Teal));
        assert_eq!(card_state(&content, &granted, &skiff, true, 2).0, "IN THE FLEET  ·  2 CORES");
        assert_eq!(card_state(&content, &granted, &skiff, false, 1), ("NEEDS 2 CORES  ·  1 FREE".into(), Tone::Coral));
        let (words, tone) = card_state(&content, &granted, &runner, false, 2);
        assert!(words.starts_with("USES ") && tone == Tone::Gold, "{words}");
        let free = Granted { unlocks: data::career::Unlocks::all(&content), ..granted };
        assert_eq!(card_state(&content, &free, &runner, false, 2).1, Tone::Teal);
    }
}
