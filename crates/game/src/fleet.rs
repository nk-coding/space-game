//! FLEET (GDR-0065, `docs/plans/mockups/fleet-b.png`): a layer over home that shows the fleet on the dock. The
//! flagship is the drydock's working design; around it are the escort slots, each a saved design (`designs/`)
//! picked from the list at the bottom. The fleet's logic cores must fit the ship-mind's capacity: a pick that
//! would exceed it is refused (`CORES n / m` top right). Every change is written into the career's save
//! (`escorts`). Escorts fly the behavior their design names and follow the flagship's lock (the voyage, track 7H).
//!
//! ```text
//! (o) SHIPYARD › FLEET                                              CORES 2 / 3 ●●○   [CLOSE ESC]
//!     Fleet
//!  [ escort 1 ]            [      flagship      ]            [ escort 2 ]
//!  ESCORT 1                 FLAGSHIP · THE DRYDOCK'S DESIGN   ESCORT 2 · 1 CORE FREE
//!  Drone wing  [REMOVE]     Rim skiff copy                    Empty
//!  [ escort 3 ]            2 CORES · CARGO 8                [ escort 4 ]
//! YOUR DESIGNS · PICK ONE FOR ESCORT 2
//! [ card ][ card ][ card ][ card ]
//! VOYAGE-READY · CORES 3 / 3
//! ```
//!
//! Home's moorings (F, once the camera has flown there) open it; the ring button top left, CLOSE and Esc close it.
//! A click on a slot picks the slot, a click on a design puts it there (the empty slot adds one), REMOVE empties a
//! slot. Shown slots: the escorts and one more while there is room ([`MAX_ESCORTS`]). UI words are chrome in code,
//! like home's.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use data::career::voyage_ready;
use data::{Blueprint, Content};

use crate::career::{EscortDesigns, Granted, PlayerCareer, readiness_words, voyage_line};
use crate::drydock::PlayerDesign;
use crate::drydock::library::{DesignDoc, parts_size};
use crate::drydock::stats::status_line;
use crate::flight_hud::HudFonts;
use crate::home::HomeAction;
use crate::mode::AppMode;
use crate::sim_bridge::GameContent;
use crate::storage::{Document, Storage};
use crate::style::{Style, ui_color};
use crate::ui_icons::{Glyph, UiIcon};
use crate::ui_kit::{
    Edge, Hot, Pill, ScreenFx, ScreenPart, ScrollRows, Scrollbar, Tip, Tone, UiClick, UiInputSet, show_screen, UiStyleSet, label, number, prose, ring_node, set_color, set_text,
    spawn_brackets, spawn_pill, tracked,
};

/// Escort slots the layout has (placeholder: the slice's mind grows to 4 cores, so at most 3 escorts fly).
pub const MAX_ESCORTS: usize = 4;
/// Design cards shown at once (the rest scroll by; the pool has one card more, for a card cut at an edge).
const CARDS: usize = 4;
/// A design card's width and the gap after it (logical px).
const CARD_W: f32 = 292.0;
const CARD_GAP: f32 = 14.0;
/// The layout's size (logical px), like home's.
const STAGE: Vec2 = Vec2::new(1280.0, 720.0);
/// The flagship's box (centre and size) and the escort slots' boxes (centres, one size).
const FLAGSHIP: (Vec2, Vec2) = (Vec2::new(640.0, 250.0), Vec2::new(320.0, 190.0));
const SLOTS: [Vec2; MAX_ESCORTS] = [Vec2::new(250.0, 215.0), Vec2::new(1030.0, 215.0), Vec2::new(250.0, 395.0), Vec2::new(1030.0, 395.0)];
const SLOT_BOX: Vec2 = Vec2::new(150.0, 96.0);
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
        let grounded = status_line(content, &report).grounded;
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
    offset: usize,
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

pub struct FleetPlugin;

impl Plugin for FleetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Fleet>()
            .add_systems(Startup, spawn.after(crate::flight_hud::spawn_hud))
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
    fleet.offset = 0;
    fleet.open = true;
}

/// Clicks, Esc, and the cards the row shows (from its scroll: the kit's wheel and scrollbar move it).
#[allow(clippy::too_many_arguments)]
fn input(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(&FleetButton, Option<&Pill>)>,
    keys: Res<ButtonInput<KeyCode>>,
    row: Single<(&ComputedNode, &mut ScrollRows), With<CardRow>>,
    mut laid: Local<usize>,
    mut fleet: ResMut<Fleet>,
    mut career: ResMut<PlayerCareer>,
    mut storage: ResMut<Storage>,
    mut escorts: ResMut<EscortDesigns>,
    design: Res<PlayerDesign>,
    granted: Res<Granted>,
    content: Res<GameContent>,
) {
    if !fleet.open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        fleet.open = false;
        return;
    }
    let (node, mut rows) = row.into_inner();
    // A first card set elsewhere (opening starts at the first) scrolls the row there.
    if fleet.offset != *laid {
        rows.scroll = fleet.offset as f32 * (CARD_W + CARD_GAP);
    }
    let w = ScrollRows::lay(&mut rows, &vec![CARD_W; fleet.designs.len()], CARD_GAP, node.size.x * node.inverse_scale_factor);
    *laid = w.first;
    if fleet.offset != w.first {
        fleet.offset = w.first;
    }
    let content = &content.0;
    for c in clicks.read() {
        let Ok((b, pill)) = buttons.get(c.0) else { continue };
        if pill.is_some_and(|p| p.disabled) {
            continue;
        }
        let ids = career.career().escorts().to_vec();
        let changed = match *b {
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
            FleetButton::Card(k) => {
                let Some(d) = fleet.designs.get(fleet.offset + k).cloned() else { continue };
                let with_cores: Vec<(String, u32)> =
                    ids.iter().zip(&escorts.0).map(|(id, b)| (id.clone(), content.ship_totals(b).logic_cores)).collect();
                let flagship = content.ship_totals(design.blueprint()).logic_cores;
                // The picked slot, as the layer shows it (escorts may have left since it was picked).
                let slot = fleet.slot.min(ids.len()).min(MAX_ESCORTS - 1);
                match pick(&with_cores, flagship, granted.mind_capacity, slot, &d) {
                    Ok(next) => {
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
    if let Some(locked) = voyage_ready(content, &granted.unlocks, u32::MAX, &d.blueprint, &[]).iter().find(|r| r.blocks()) {
        return (readiness_words(content, locked), Tone::Gold);
    }
    if in_fleet { (format!("IN THE FLEET  ·  {}", plural(d.cores, "CORE")), Tone::Label) } else { (format!("FITS  ·  {}", plural(d.cores, "CORE")), Tone::Teal) }
}

/// FLEET is shown at home while it is open, with its transition (`ui_kit::ScreenFx`; it covers the station, which
/// fades out under it); home's words step aside meanwhile (`home::show`).
fn show(mode: Res<State<AppMode>>, fleet: Res<Fleet>, mut root: Single<&mut ScreenFx, With<FleetRoot>>) {
    show_screen(&mut root, *mode.get() == AppMode::Home && fleet.open);
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
    let column = |gap: f32, at: Node| Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(gap), ..at };
    let placeholder = |size: f32| UiIcon::Glyph { glyph: Glyph::Formation, color: ui.dim.with_alpha(size) };
    let boxed = |accent: Tone| Pill { bare: true, ..Pill::new(accent) };
    commands
        .spawn((
            FleetRoot,
            crate::backdrop::VeilsBackdrop,
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
                centre.spawn(Node { width: px(STAGE.x), height: px(STAGE.y), flex_shrink: 0.0, ..default() }).with_children(|stage| {
                    // Top left: the way back, where we are, the words.
                    stage
                        .spawn((ScreenPart::new(Edge::Top, 0), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(14), ..abs(Vec2::new(22.0, 14.0), None) }))
                        .with_children(|tl| {
                            tl.spawn((ring_node(40.0), FleetButton::Close, Tip::key("SHIPYARD", "ESC"))).with_child((
                                UiIcon::Glyph { glyph: Glyph::Menu, color: ui.violet },
                                Node { width: px(22), height: px(22), ..default() },
                            ));
                            tl.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                                col.spawn(label(fonts, "SHIPYARD  ›  FLEET", 10.0, c(Tone::Label, 1.0)));
                                col.spawn(prose(fonts, "Fleet", 22.0, c(Tone::Number, 1.0)));
                            });
                        });
                    stage.spawn((
                        ScreenPart::new(Edge::Fade, 1),
                        prose(fonts, "Escorts fly the behavior their design names and follow the flagship's lock.", 12.0, c(Tone::Label, 1.0)),
                        abs(Vec2::new(28.0, 84.0), Some(Vec2::new(300.0, 40.0))),
                    ));
                    // Top right: the mind's cores, CLOSE.
                    stage
                        .spawn((ScreenPart::new(Edge::Top, 0), Node {
                            position_type: PositionType::Absolute,
                            right: px(20),
                            top: px(19),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: px(10),
                            ..default()
                        }))
                        .with_children(|tr| {
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
                            spawn_pill(tr, fonts, "CLOSE", 9.5, 30.0, None, Pill::new(Tone::Teal), (FleetButton::Close, Tip::key("CLOSE", "ESC")));
                        });

                    // The flagship on the dock.
                    let (centre_at, size) = FLAGSHIP;
                    stage.spawn((ScreenPart::new(Edge::Fade, 1), abs(centre_at - size / 2.0, Some(size)))).with_children(|b| {
                        spawn_brackets(b, 16.0, 1.4, c(Tone::Teal, 0.55));
                        b.spawn((
                            FleetIcon::Flagship,
                            placeholder(0.5),
                            Node { position_type: PositionType::Absolute, left: px(24), top: px(20), width: px(size.x - 48.0), height: px(size.y - 40.0), ..default() },
                        ));
                    });
                    stage.spawn((ScreenPart::new(Edge::Fade, 1), column(6.0, abs(Vec2::new(centre_at.x - 240.0, centre_at.y + size.y / 2.0 + 14.0), Some(Vec2::new(480.0, 90.0))))))
                        .with_children(|t| {
                            t.spawn(label(fonts, "FLAGSHIP  ·  THE DRYDOCK'S DESIGN", 8.5, c(Tone::Dim, 1.0)));
                            t.spawn((FleetText::ShipName, prose(fonts, "", 18.0, c(Tone::Number, 1.0))));
                            t.spawn((FleetText::ShipSub, label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                            t.spawn((FleetText::ShipVoyage, label(fonts, "", 8.5, c(Tone::Teal, 1.0))));
                        });

                    // The escort slots.
                    for (k, at) in SLOTS.iter().enumerate() {
                        let side = if at.x < STAGE.x / 2.0 { Edge::Left } else { Edge::Right };
                        stage.spawn((FleetNode::Slot(k), ScreenPart::new(side, 2), column(6.0, abs(Vec2::new(at.x - 120.0, at.y - SLOT_BOX.y / 2.0), Some(Vec2::new(240.0, 170.0))))))
                            .with_children(|s| {
                                s.spawn((
                                    FleetButton::Slot(k),
                                    boxed(Tone::Teal),
                                    Hot::default(),
                                    Node {
                                        width: px(SLOT_BOX.x),
                                        height: px(SLOT_BOX.y),
                                        flex_shrink: 0.0,
                                        border: UiRect::all(px(1.2)),
                                        border_radius: BorderRadius::all(px(12)),
                                        margin: UiRect::bottom(px(4)),
                                        ..default()
                                    },
                                    BorderColor::all(Color::NONE),
                                    BackgroundColor(Color::NONE),
                                ))
                                .with_children(|b| {
                                    spawn_brackets(b, 10.0, 1.2, c(Tone::Label, 0.45));
                                    b.spawn((
                                        FleetIcon::Slot(k),
                                        placeholder(0.6),
                                        Node { position_type: PositionType::Absolute, left: px(16), top: px(12), width: px(SLOT_BOX.x - 32.0), height: px(SLOT_BOX.y - 24.0), ..default() },
                                    ));
                                });
                                s.spawn((FleetText::SlotHead(k), label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                                s.spawn((FleetText::SlotName(k), prose(fonts, "", 15.0, c(Tone::Number, 1.0))));
                                s.spawn((FleetText::SlotSub(k), label(fonts, "", 8.0, c(Tone::Dim, 1.0))));
                                s.spawn((FleetNode::Remove(k), Node { margin: UiRect::top(px(2)), ..default() })).with_children(|r| {
                                    spawn_pill(r, fonts, "REMOVE", 8.0, 22.0, None, Pill::new(Tone::Coral), FleetButton::Remove(k));
                                });
                            });
                    }

                    // The player's designs.
                    stage.spawn((FleetText::ListHead, ScreenPart::new(Edge::Bottom, 3), label(fonts, "", 9.0, c(Tone::Dim, 1.0)), abs(Vec2::new(36.0, 540.0), None)));
                    stage
                        .spawn((
                            CardRow,
                            Scrollbar::horizontal().outside(),
                            ScrollRows::default(),
                            ScreenPart::new(Edge::Bottom, 3),
                            Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: px(CARD_GAP),
                                overflow: Overflow::scroll_x(),
                                ..abs(Vec2::new(36.0, 562.0), Some(Vec2::new(CARDS as f32 * (CARD_W + CARD_GAP) - CARD_GAP, 94.0)))
                            },
                        ))
                        .with_children(|row| {
                            for k in 0..=CARDS {
                                row.spawn((
                                    FleetNode::Card(k),
                                    FleetButton::Card(k),
                                    boxed(Tone::Teal),
                                    Hot::default(),
                                    Node {
                                        width: px(CARD_W),
                                        height: px(94),
                                        flex_shrink: 0.0,
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        column_gap: px(14),
                                        padding: UiRect::horizontal(px(12)),
                                        border: UiRect::all(px(1.2)),
                                        border_radius: BorderRadius::all(px(14)),
                                        ..default()
                                    },
                                    BorderColor::all(Color::NONE),
                                    BackgroundColor(Color::NONE),
                                ))
                                .with_children(|card| {
                                    card.spawn(Node { width: px(104), height: px(72), flex_shrink: 0.0, ..default() }).with_children(|b| {
                                        spawn_brackets(b, 8.0, 1.0, c(Tone::Label, 0.35));
                                        b.spawn((
                                            FleetIcon::Card(k),
                                            placeholder(0.5),
                                            Node { position_type: PositionType::Absolute, left: px(8), top: px(8), width: px(88), height: px(56), ..default() },
                                        ));
                                    });
                                    card.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(6), flex_shrink: 1.0, min_width: px(0), ..default() })
                                        .with_children(|t| {
                                            t.spawn((FleetText::CardName(k), prose(fonts, "", 14.0, c(Tone::Number, 1.0))));
                                            t.spawn((FleetText::CardSub(k), tracked(fonts, "", 7.5, c(Tone::Dim, 1.0), 0.15)));
                                            t.spawn((FleetText::CardState(k), tracked(fonts, "", 7.5, c(Tone::Teal, 1.0), 0.15)));
                                        });
                                });
                            }
                        });
                    stage.spawn((FleetText::Status, ScreenPart::new(Edge::Bottom, 3), label(fonts, "", 9.5, c(Tone::Teal, 1.0)), abs(Vec2::new(36.0, 676.0), Some(Vec2::new(1208.0, 30.0)))));
                });
            });
        });
}

/// The layer follows the fleet: texts, icons, which slots and cards show, the dots.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync(
    fleet: Res<Fleet>,
    career: Res<PlayerCareer>,
    granted: Res<Granted>,
    escorts: Res<EscortDesigns>,
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    style: Res<Style>,
    mut texts: Query<(&FleetText, &mut Text, &mut TextColor)>,
    mut icons: Query<(&FleetIcon, &mut UiIcon)>,
    mut nodes: Query<(&FleetNode, &mut Node, Option<&mut BackgroundColor>)>,
    mut pills: Query<(&FleetButton, &mut Pill)>,
) {
    if !fleet.open || !(fleet.is_changed() || career.is_changed() || granted.is_changed() || escorts.is_changed() || design.is_changed()) {
        return;
    }
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
    let cargo = |c: f32| data::stats::format_number(c, 0);

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
            FleetText::SlotName(k) => match escorts.0.get(k) {
                Some(b) => (b.name.clone(), Tone::Number),
                None => ("Empty".into(), Tone::Dim),
            },
            FleetText::SlotSub(k) => match escorts.0.get(k) {
                Some(b) => {
                    let t = content.ship_totals(b);
                    (format!("{}  ·  CARGO {}", plural(t.logic_cores, "CORE"), cargo(t.cargo_capacity)), Tone::Dim)
                }
                None => ("PICK A DESIGN BELOW".into(), Tone::Dim),
            },
            FleetText::ListHead if fleet.designs.is_empty() => ("YOUR DESIGNS  ·  NONE SAVED YET  ·  SAVE ONE IN THE DRYDOCK".into(), Tone::Dim),
            FleetText::ListHead => {
                let mut s = format!("YOUR DESIGNS  ·  PICK ONE FOR ESCORT {}", picked + 1);
                if fleet.designs.len() > CARDS {
                    s.push_str(&format!("  ·  {} OF {}", (fleet.offset + CARDS).min(fleet.designs.len()), fleet.designs.len()));
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
            FleetText::Status => match &fleet.say {
                Some((s, tone)) => (s.clone(), *tone),
                None => (format!("CLICK A DESIGN TO FLY IT AS ESCORT {}", picked + 1), Tone::Label),
            },
        };
        set_text(&mut text, &s);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
    }
    for (i, mut icon) in &mut icons {
        let want = match *i {
            FleetIcon::Flagship => Some(flagship),
            FleetIcon::Slot(k) => escorts.0.get(k),
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
    for (b, mut pill) in &mut pills {
        if let FleetButton::Slot(k) = *b {
            let active = k == picked;
            if pill.active != active {
                pill.active = active;
            }
        }
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
