//! The debrief (GDR-0065): after a voyage, a full panel over home's right
//! side until it is dismissed (DISMISS, Esc or Enter). A layer of its own over [`AppMode::Home`] (the shipyard),
//! independent of home's layout; hidden while FLEET, SETTINGS or the LOG is open. What came home stays in the LOG
//! (`voyage_log.rs`, GDR-0091; it replaced the one-line summary that sat over home after the panel).
//!
//! ```text
//! VOYAGE HOME · HAUL UNLOADED
//! Shipyard approach
//! [RIM] [GOAL CLEARED] [FIRST CLEAR]
//! TIME 7:42   SITES 4 / 4   KILLS 9   SHIPS LOST 0
//! INTACT · 2 PARTS                          KEPT UNLESS YOU DISMANTLE
//!   [icon] Thruster ×2   IN STOCK 1          [KEEP] [DISMANTLE]
//!   [SORT AS SUGGESTED]
//! GAINED                                    FROM 14 PARTS DISMANTLED
//!   RESEARCH DATA                 MATERIALS
//!   • Kinetic weapons     +18     ◆ Programmable matter  +1
//!   TOTAL                 +18     TOTAL                  +1
//! 14 PARTS DISMANTLED                                          3 KINDS
//!   [icon ×6] [icon ×4] [icon ×4]    (a part's yield on hover)
//! RESEARCH READY · 1
//!   Reverse-engineered cannon
//! ((·)) SIGNAL FOUND · HALVEX DRIFT
//! The lanes around the shipyard are quiet again. Further out, ...
//!                               [VOYAGE MAP] [DISMISS ESC]
//! ```
//!
//! * What it shows is [`DebriefData`] (`voyage::debrief`): built at the voyage's end from the career before and
//!   after it, the haul and what the flight counted. Kills and ships lost are not in the save.
//! * It is not saved: the next start shows no panel (the LOG has the voyage).
//! * The report scrolls when it is longer than the panel (a long haul, a low window; Niklas, 14 Oct 2026: "the
//!   voyage report is not scrollable, and thus can cut things off"): everything above the buttons is one area with
//!   the kit's [`Scrollbar`] (smooth wheel, the overlay bar, edge fades); the buttons stay put at the bottom.
//! * A voyage its first clear opened is a signal found (GDR-0078): the panel names it (from the content) above the
//!   voyage's `cleared` words, and adds VOYAGE MAP beside DISMISS: it dismisses the panel and flies to the gate
//!   (`map::OpenMap`), where the map selects the new signal. A voyage into a cluster of the galaxy lists what it found
//!   there the same way (galaxy plan E6): `CHARTED · VARDIS`, the signals heard and how far each came, in gold
//!   (`SIGNAL NARROWED · A GATE`), the logs to decode at the LOG; VOYAGE MAP shows for anything on the map.
//! * The haul's intact parts come first and are sorted here (Niklas, 18 Oct 2026: take and dismantle "belong in the
//!   return-report sidebar", not in the workshop's main area; later that day, variant A of the report study: intact
//!   first, a plain keep-or-dismantle per part): a line per part with its icon and stock, KEEP (into stock; lit, as
//!   it is what happens when nothing is pressed) and DISMANTLE (its tooltip says what dismantling all of it gives:
//!   research data and a share of its recipe), then SORT AS SUGGESTED (the workshop's suggestion in its tooltip); it
//!   is refilled whenever the career changes. What the panel leaves is kept (`Career::keep_intake`) when it is
//!   dismissed, and at any time at home without an open report (a report never seen). Dismantling later in the
//!   WORKSHOP is always possible.
//! * GAINED sums what the dismantled parts gave in two columns (research data by family, materials), then a grid
//!   of the dismantled parts' icons (`workshop::icon`, the drydock's part icons) with their counts; a tile's tooltip
//!   is what that part gave. The icons are world meshes, so the panel's backing is one too (`UiIcon::Scrim`) and
//!   home's words step aside while the report shows (`home::show`).

use bevy::prelude::*;
use data::career::{Outcome, dismantle_yield};
use kit::style::{Style, ui_color};
use kit::ui_icons::UiIcon;
use kit::ui_kit::{
    HudFonts, Edge, Hot, Pill, ScreenFx, ScreenPart, Scrollbar, Tip, Tone, show_screen, UiBlock, UiClick, UiInputSet, UiPointer, UiStyleSet, label, number, prose, spawn_pill, tracked,
};
use world::career::{EscortDesigns, Note, PlayerCareer};
use world::mode::AppMode;
use world::player::PlayerDesign;
use world::settings::SettingsLayer;
use world::sim_bridge::GameContent;
use world::storage::Storage;
use world::voyage::clock;
use world::voyage::debrief::{Debrief, DebriefData, amount};

use crate::fleet::Fleet;
use crate::voyage_log::VoyageLog;
use crate::workshop::{fleet_need, gave_words, icon, refusal_words, sort_rows};

/// Tiles per row of the dismantled parts' grid (more wrap to the next row).
const TILES: usize = 6;
/// The gap between the grid's tiles (logical px).
const TILE_GAP: f32 = 6.0;
/// The panel's width (logical px).
const WIDTH: f32 = 420.0;
/// The gap between the report's lines, and between the report and the buttons (logical px).
const GAP: f32 = 14.0;

/// The panel's root (its content is rebuilt whenever the debrief changes).
#[derive(Component)]
struct PanelRoot;

#[derive(Component)]
struct DismissButton;

/// VOYAGE MAP (after a voyage that opened another).
#[derive(Component)]
struct MapButton;

/// The haul's intact parts to sort, at the top (refilled when the career changes).
#[derive(Component)]
struct IntakeBox;

/// KEEP or DISMANTLE all of one intact part, or SORT AS SUGGESTED.
#[derive(Component, Clone, Debug, PartialEq)]
enum SortButton {
    Keep(String),
    Dismantle(String),
    All,
}

/// What the last sort said (cleared with a new report).
#[derive(Resource, Default)]
struct Sorted(Option<(String, Tone)>);

pub struct DebriefPlugin;

impl Plugin for DebriefPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Debrief>()
            .init_resource::<Sorted>()
            .add_systems(Startup, spawn.after(world::flight_hud::spawn_hud))
            .add_systems(Update, (dismiss, sort, settle, rebuild, fill_intake, show).chain().after(UiInputSet).before(UiStyleSet));
    }
}

fn spawn(mut commands: Commands, style: Res<Style>, clear: Res<ClearColor>) {
    commands.spawn((
        PanelRoot,
        // Slides in from the right and fades in (`ui_kit::ScreenFx`); out the same way when dismissed.
        ScreenFx::closed(),
        ScreenPart::new(Edge::Right, 0),
        UiBlock,
        // Hot, so a click on the panel reaches nothing under it.
        Hot::default(),
        GlobalZIndex(10),
        Visibility::Hidden,
        // The backing is a world mesh, so the part icons (world meshes too) draw over it.
        UiIcon::Scrim { color: clear.0.with_alpha(0.94), hole: [f32::INFINITY; 4], out: 0.0, into: 0.0 },
        BorderColor::all(ui_color(style.ui.dim, 0.5)),
        Node {
            position_type: PositionType::Absolute,
            right: px(0),
            top: px(0),
            bottom: px(0),
            width: px(WIDTH),
            border: UiRect::left(px(1)),
            flex_direction: FlexDirection::Column,
            padding: UiRect::new(px(30), px(30), px(26), px(24)),
            row_gap: px(GAP),
            overflow: Overflow::clip(),
            ..default()
        },
    ));
}

/// DISMISS, Esc or Enter close the panel while it shows: not while a layer over home hides it (their Esc is
/// theirs, also in the frame it closes them).
#[allow(clippy::too_many_arguments)]
fn dismiss(
    mode: Res<State<AppMode>>,
    panel: Single<&ScreenFx, With<PanelRoot>>,
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<UiPointer>,
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(), With<DismissButton>>,
    map_buttons: Query<(), With<MapButton>>,
    mut debrief: ResMut<Debrief>,
    mut open_map: MessageWriter<crate::map::OpenMap>,
) {
    let (mut clicked, mut to_map) = (false, false);
    for c in clicks.read() {
        clicked |= buttons.contains(c.0);
        to_map |= map_buttons.contains(c.0);
    }
    if !debrief.open || *mode.get() != AppMode::Home || !panel.is_open() {
        return;
    }
    let key = !pointer.typing && keys.any_just_pressed([KeyCode::Escape, KeyCode::Enter, KeyCode::NumpadEnter]);
    if to_map {
        open_map.write(crate::map::OpenMap);
    }
    if clicked || key || to_map {
        debrief.open = false;
    }
}

/// Shown at home while open, and not under FLEET, SETTINGS, the LOG, EVOLUTION or the WORKSHOP (the layers over home leave the world
/// behind them barely there, so a panel left showing would read through them).
#[allow(clippy::type_complexity)]
fn show(
    mode: Res<State<AppMode>>,
    (fleet, settings, log, evo, ws): (Res<Fleet>, Res<SettingsLayer>, Res<VoyageLog>, Res<crate::evolution::EvolutionLayer>, Res<crate::workshop::WorkshopLayer>),
    debrief: Res<Debrief>,
    mut panel: Single<&mut ScreenFx, With<PanelRoot>>,
) {
    let home = *mode.get() == AppMode::Home && !fleet.open && !settings.open && !log.open && !evo.open && !ws.open;
    show_screen(&mut panel, home && debrief.data.is_some() && debrief.open);
}

/// Rebuilds the panel when the debrief changes.
fn rebuild(
    mut commands: Commands,
    debrief: Res<Debrief>,
    content: Res<GameContent>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    panel: Single<Entity, With<PanelRoot>>,
    mut sorted: ResMut<Sorted>,
) {
    if !debrief.is_changed() {
        return;
    }
    sorted.0 = None;
    let Some(d) = &debrief.data else { return };
    commands.entity(*panel).despawn_children();
    commands.entity(*panel).with_children(|p| {
        // The report (it scrolls when it is longer than the panel; a new one starts at the top), the buttons under
        // it.
        p.spawn((
            ScrollPosition::default(),
            Scrollbar::vertical(),
            Node { flex_direction: FlexDirection::Column, row_gap: px(GAP), flex_grow: 1.0, flex_basis: px(0), min_height: px(0), overflow: Overflow::scroll_y(), ..default() },
        ))
        .with_children(|body| fill(body, &content.0, &fonts, &style, d));
        buttons(p, &fonts, d);
    });
}

/// Barlow has no ◆ and no signal mark, and the panel can't take the vector glyphs (`ui_icons`): they are world meshes
/// that its opaque backing covers (it has to be opaque: home's words under it are UI too, so a world backing would not
/// hide them). So both are bevy_ui nodes, drawn like the glyphs: ◆ a square turned 45°, `side` px across.
fn diamond(color: Color, side: f32) -> impl Bundle {
    let square = side * 0.42 * std::f32::consts::SQRT_2;
    (
        Node { width: px(side), height: px(side), justify_content: JustifyContent::Center, align_items: AlignItems::Center, flex_shrink: 0.0, ..default() },
        children![(Node { width: px(square), height: px(square), ..default() }, BackgroundColor(color), UiTransform { rotation: Rot2::degrees(45.0), ..default() })],
    )
}

/// A beacon sending, `side` px across: a dot between two pairs of arcs. An arc is a ring's left or right border (a
/// border's sides meet on the diagonals, so each spans 90°).
fn signal(color: Color, side: f32) -> impl Bundle {
    let w = (side * 0.07).max(1.2);
    let ring = |r: f32| {
        let d = 2.0 * r * side + w;
        (
            Node {
                position_type: PositionType::Absolute,
                left: px((side - d) / 2.0),
                top: px((side - d) / 2.0),
                width: px(d),
                height: px(d),
                border: UiRect::all(px(w)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor { left: color, right: color, ..BorderColor::all(Color::NONE) },
        )
    };
    let dot = 0.18 * side;
    (
        Node { width: px(side), height: px(side), flex_shrink: 0.0, ..default() },
        children![
            ring(0.22),
            ring(0.4),
            (
                Node { position_type: PositionType::Absolute, left: px((side - dot) / 2.0), top: px((side - dot) / 2.0), width: px(dot), height: px(dot), border_radius: BorderRadius::MAX, ..default() },
                BackgroundColor(color),
            ),
        ],
    )
}

/// The report's lines.
fn fill(p: &mut ChildSpawnerCommands, content: &data::Content, fonts: &HudFonts, style: &Style, d: &DebriefData) {
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    let fam = |key: &Option<String>| key.as_deref().map_or(ui_color(ui.label, 1.0), |k| ui_color(style.system_color(k), 1.0));
    let row = |gap: f32| Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), flex_shrink: 0.0, ..default() };
    let rule = |p: &mut ChildSpawnerCommands| {
        p.spawn((Node { height: px(1), flex_shrink: 0.0, ..default() }, BackgroundColor(ui_color(ui.dim, 0.35))));
    };
    let (head, tone) = d.head();
    p.spawn(label(fonts, head, 10.5, c(tone)));
    p.spawn(prose(fonts, &d.name, 26.0, c(Tone::Number)));
    // Chips: band, outcome, first clear.
    p.spawn(row(8.0)).with_children(|r| {
        let mut chip = |text: &str, tone: Tone| {
            r.spawn((
                Node { padding: UiRect::axes(px(10), px(4)), border: UiRect::all(px(1)), border_radius: BorderRadius::MAX, ..default() },
                BorderColor::all(ui_color(tone.color(&ui), 0.7)),
            ))
            .with_children(|ch| {
                ch.spawn(label(fonts, text, 9.0, c(tone)));
            });
        };
        if !d.band.is_empty() {
            chip(&d.band, Tone::Label);
        }
        let (o, t) = d.outcome_chip();
        chip(o, t);
        if d.first_clear {
            chip("FIRST CLEAR", Tone::Gold);
        }
    });
    // Time, sites, kills, ships lost.
    p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(10.0) }).with_children(|r| {
        let opt = |v: Option<u32>| v.map_or("–".to_string(), |v| v.to_string());
        let sites = d.stats.sites.map_or("–".to_string(), |(a, b)| format!("{a} / {b}"));
        for (head, value, tone) in [
            ("TIME", clock(d.ticks), Tone::Number),
            ("SITES", sites, Tone::Teal),
            ("KILLS", opt(d.stats.kills), Tone::Number),
            ("SHIPS LOST", opt(d.stats.ships_lost), if d.stats.ships_lost.unwrap_or(0) > 0 { Tone::Coral } else { Tone::Number }),
        ] {
            r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                col.spawn(label(fonts, head, 9.0, c(Tone::Label)));
                col.spawn(number(fonts, &value, 20.0, c(tone)));
            });
        }
    });
    rule(p);
    if d.outcome == Outcome::Lost {
        let left = if d.stats.left > 0 { format!("{} PARTS STAYED IN THE WRECK", d.stats.left) } else { "NOTHING CAME HOME".into() };
        p.spawn(label(fonts, &left, 9.5, c(Tone::Coral)));
        p.spawn(prose(fonts, "The haul of a lost voyage stays behind. Your ships re-formed at home, whole.", 13.0, c(Tone::Label)));
    } else {
        // Intact parts first (`fill_intake`), then what the dismantled ones gave, then the parts themselves.
        p.spawn((IntakeBox, Node { flex_direction: FlexDirection::Column, row_gap: px(10), flex_shrink: 0.0, ..default() }));
        let dismantled: Vec<_> = d.rows.iter().filter(|r| !r.intact).collect();
        let n: u32 = dismantled.iter().map(|r| r.count).sum();
        let parts = format!("{n} {} DISMANTLED", if n == 1 { "PART" } else { "PARTS" });
        let head = |p: &mut ChildSpawnerCommands, left: &str, right: &str| {
            p.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Baseline, ..row(10.0) }).with_children(|h| {
                h.spawn(label(fonts, left, 9.5, c(Tone::Label)));
                h.spawn(tracked(fonts, right, 8.5, c(Tone::Dim), 0.2));
            });
        };
        if !d.families.is_empty() || !d.materials.is_empty() {
            head(p, "GAINED", &format!("FROM {}", parts));
            p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(22), flex_shrink: 0.0, ..default() }).with_children(|cols| {
                let col = |cols: &mut ChildSpawnerCommands, title: &str, lines: Vec<(Option<Color>, Color, String, String)>, total: String, tone: Color| {
                    cols.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(7), flex_grow: 1.0, flex_basis: px(0), ..default() }).with_children(|k| {
                        k.spawn(label(fonts, title, 8.5, c(Tone::Dim)));
                        if lines.is_empty() {
                            k.spawn(label(fonts, "NONE", 9.0, c(Tone::Dim)));
                        }
                        for (dot, color, name, value) in &lines {
                            k.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(8.0) }).with_children(|l| {
                                l.spawn(row(8.0)).with_children(|l| {
                                    match dot {
                                        Some(f) => l.spawn((Node { width: px(7), height: px(7), border_radius: BorderRadius::MAX, flex_shrink: 0.0, ..default() }, BackgroundColor(*f))),
                                        None => l.spawn(diamond(*color, 8.0)),
                                    };
                                    l.spawn(label(fonts, name, 10.0, *color));
                                });
                                l.spawn(number(fonts, value, 15.0, *color));
                            });
                        }
                        if !lines.is_empty() {
                            k.spawn((Node { height: px(1), flex_shrink: 0.0, ..default() }, BackgroundColor(ui_color(ui.dim, 0.35))));
                            k.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(8.0) }).with_children(|l| {
                                l.spawn(label(fonts, "TOTAL", 9.0, c(Tone::Label)));
                                l.spawn(number(fonts, &total, 15.0, tone));
                            });
                        }
                    });
                };
                let research: Vec<_> = d.families.iter().map(|f| (Some(fam(&f.color)), c(Tone::Number), f.name.clone(), format!("+{}", amount(f.gained)))).collect();
                let data: f32 = d.families.iter().map(|f| f.gained).sum();
                col(cols, "RESEARCH DATA", research, format!("+{}", amount(data)), c(Tone::Number));
                let gold = c(Tone::Gold);
                let materials: Vec<_> = d.materials.iter().map(|(m, n)| (None, gold, m.to_uppercase(), format!("+{n}"))).collect();
                let units: u32 = d.materials.iter().map(|(_, n)| n).sum();
                col(cols, "MATERIALS", materials, format!("+{units}"), gold);
            });
            rule(p);
        }
        let kinds = dismantled.len();
        head(p, &parts, &if kinds > 0 { format!("{kinds} {}", if kinds == 1 { "KIND" } else { "KINDS" }) } else { String::new() });
        if dismantled.is_empty() {
            let empty = if d.rows.is_empty() { "The holds came home empty." } else { "Every part came home intact." };
            p.spawn(prose(fonts, empty, 13.0, c(Tone::Dim)));
        }
        // The grid: an icon and its count per part, what it gave in its tooltip; more wrap onto the next row.
        let side = (WIDTH - 60.0 - TILE_GAP * (TILES - 1) as f32) / TILES as f32;
        p.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: px(TILE_GAP), row_gap: px(TILE_GAP), flex_shrink: 0.0, ..default() }).with_children(|grid| {
            for r in &dismantled {
                let gave = if r.gives.is_empty() { "NOTHING".to_string() } else { r.gives.clone() };
                grid.spawn((
                    Hot::default(),
                    Tip::key(format!("{} ×{}", r.name.to_uppercase(), r.count), gave),
                    Node {
                        width: px(side),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::new(px(0), px(0), px(6), px(4)),
                        row_gap: px(2),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BorderColor::all(ui_color(ui.dim, 0.28)),
                ))
                .with_children(|t| {
                    if r.color.is_some() {
                        t.spawn((
                            Node { position_type: PositionType::Absolute, top: px(5), right: px(5), width: px(5), height: px(5), border_radius: BorderRadius::MAX, ..default() },
                            BackgroundColor(fam(&r.color)),
                        ));
                    }
                    t.spawn(icon(content, &r.part));
                    t.spawn(number(fonts, &format!("×{}", r.count), 11.0, c(Tone::Label)));
                });
            }
        });
    }
    if !d.ready.is_empty() || !d.signals().is_empty() || d.cleared_text.is_some() {
        rule(p);
    }
    if !d.ready.is_empty() {
        p.spawn(label(fonts, &format!("RESEARCH READY  ·  {}", d.ready.len()), 9.5, c(Tone::Teal)));
        for name in d.ready.iter().take(3) {
            p.spawn(prose(fonts, name, 15.0, c(Tone::Number)));
        }
    }
    for (line, tone) in d.signals() {
        p.spawn(row(7.0)).with_children(|l| {
            l.spawn(signal(c(tone), 16.0));
            l.spawn(label(fonts, &line, 10.5, c(tone)));
        });
    }
    if let Some(t) = &d.cleared_text {
        p.spawn(prose(fonts, t, 13.0, c(Tone::Label)));
    }
    if let Some(why) = &d.practice {
        p.spawn(label(fonts, why, 9.0, c(Tone::Gold)));
    }
}

/// KEEP, DISMANTLE and SORT AS SUGGESTED: the career sorts the intake and writes the save; the box says what it gave.
#[allow(clippy::too_many_arguments)]
fn sort(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<&SortButton>,
    mut career: ResMut<PlayerCareer>,
    mut storage: ResMut<Storage>,
    content: Res<GameContent>,
    (design, escorts): (Res<PlayerDesign>, Res<EscortDesigns>),
    mut sorted: ResMut<Sorted>,
) {
    let Some(b) = clicks.read().find_map(|c| buttons.get(c.0).ok()).cloned() else { return };
    let c = &content.0;
    let need = fleet_need(c, &design, &escorts);
    let name = |p: &str| c.parts.get(p).map_or_else(|| p.to_uppercase(), |x| x.name.to_uppercase());
    let (said, written) = career.update(&mut storage, |career| {
        let n = |p: &str| career.intake().get(p).copied().unwrap_or(0);
        match &b {
            SortButton::Keep(p) => {
                let k = n(p);
                career.keep(p, k).map(|()| (format!("KEPT {k} {}  ·  IN STOCK", name(p)), Tone::Teal))
            }
            SortButton::Dismantle(p) => {
                let k = n(p);
                career.dismantle(c, p, k).map(|got| (format!("DISMANTLED {k} {}  ·  {}", name(p), gave_words(c, &got)), Tone::Gold))
            }
            SortButton::All => {
                let kept: u32 = career.suggested(c, &need).values().map(|s| s.keep).sum();
                career.sort_suggested(c, &need).map(|got| (format!("SORTED  ·  KEPT {kept}  ·  {}", gave_words(c, &got)), Tone::Gold))
            }
        }
    });
    sorted.0 = Some(said.unwrap_or_else(|r| (refusal_words(c, &r), Tone::Coral)));
    if let Err(e) = written {
        warn!("debrief: not saved: {e}");
        career.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
    }
}

/// At home without an open report, the intake goes to the stock: what a dismissed report left unsorted, a report
/// never seen.
fn settle(mode: Res<State<AppMode>>, debrief: Res<Debrief>, mut career: ResMut<PlayerCareer>, mut storage: ResMut<Storage>) {
    if *mode.get() != AppMode::Home || (debrief.open && debrief.data.is_some()) || career.career().intake().is_empty() {
        return;
    }
    let (kept, written) = career.update(&mut storage, |c| c.keep_intake());
    info!("debrief: {kept} intact parts left unsorted went to the stock");
    if let Err(e) = written {
        warn!("debrief: not saved: {e}");
        career.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
    }
}

/// The intact parts' box, anew when it is spawned, the career changes or a sort said something.
#[allow(clippy::too_many_arguments)]
fn fill_intake(
    mut commands: Commands,
    boxes: Query<(Entity, Ref<IntakeBox>)>,
    career: Res<PlayerCareer>,
    content: Res<GameContent>,
    (design, escorts): (Res<PlayerDesign>, Res<EscortDesigns>),
    sorted: Res<Sorted>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
) {
    let Ok((entity, added)) = boxes.single() else { return };
    if !(added.is_added() || career.is_changed() || sorted.is_changed()) {
        return;
    }
    let (content, ui) = (&content.0, style.ui);
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    let rows = sort_rows(content, career.career(), &fleet_need(content, &design, &escorts));
    let share = content.workshop().map_or(0.0, |(_, r)| r.dismantle);
    let row = |gap: f32| Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), flex_shrink: 0.0, ..default() };
    commands.entity(entity).despawn_children();
    commands.entity(entity).with_children(|p| {
        if !rows.is_empty() {
            let n: u32 = rows.iter().map(|r| r.count).sum();
            p.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Baseline, ..row(10.0) }).with_children(|h| {
                h.spawn(label(&fonts, &format!("INTACT  ·  {n} {}", if n == 1 { "PART" } else { "PARTS" }), 9.5, c(Tone::Gold)));
                h.spawn(tracked(&fonts, "KEPT UNLESS YOU DISMANTLE", 8.5, c(Tone::Dim), 0.2));
            });
        }
        for r in &rows {
            p.spawn(row(10.0)).with_children(|line| {
                line.spawn(icon(content, &r.part));
                line.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), flex_grow: 1.0, ..default() }).with_children(|col| {
                    col.spawn(prose(&fonts, &format!("{} ×{}", r.name, r.count), 14.0, c(Tone::Number)));
                    col.spawn(label(&fonts, &format!("IN STOCK {}", r.stock), 8.0, c(Tone::Dim)));
                });
                // KEEP is lit: it is what happens when nothing is pressed.
                spawn_pill(line, &fonts, "KEEP", 9.0, 26.0, None, Pill { active: true, ..Pill::new(Tone::Teal) }, (SortButton::Keep(r.part.clone()), Tip::new(format!("KEEP ALL {}  ·  INTO STOCK", r.count))));
                let gives = gave_words(content, &dismantle_yield(content, &r.part, r.count, share));
                spawn_pill(line, &fonts, "DISMANTLE", 9.0, 26.0, None, Pill::new(Tone::Gold), (SortButton::Dismantle(r.part.clone()), Tip::key(format!("DISMANTLE ALL {}", r.count), format!("GIVES {gives}"))));
            });
        }
        let (keep, dismantle) = rows.iter().fold((0, 0), |(k, d), r| (k + r.keep, d + r.dismantle));
        if dismantle > 0 {
            p.spawn(row(0.0)).with_children(|b| {
                let tip = Tip::key("SORT AS SUGGESTED", format!("KEEP {keep}  ·  DISMANTLE {dismantle}"));
                spawn_pill(b, &fonts, "SORT AS SUGGESTED", 9.0, 26.0, None, Pill::new(Tone::Teal), (SortButton::All, tip));
            });
        }
        if let Some((words, tone)) = &sorted.0 {
            p.spawn(tracked(&fonts, words, 8.5, c(*tone), 0.16));
        }
        if !rows.is_empty() || sorted.0.is_some() {
            p.spawn((Node { height: px(1), flex_shrink: 0.0, ..default() }, BackgroundColor(ui_color(ui.dim, 0.35))));
        }
    });
}

/// VOYAGE MAP (after a voyage that found a signal) and DISMISS, at the panel's bottom.
fn buttons(p: &mut ChildSpawnerCommands, fonts: &HudFonts, d: &DebriefData) {
    p.spawn(Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd, column_gap: px(10), flex_shrink: 0.0, ..default() }).with_children(|r| {
        if d.to_map() {
            spawn_pill(r, fonts, "VOYAGE MAP", 10.0, 36.0, None, Pill::new(Tone::Teal), MapButton);
        }
        spawn_pill(r, fonts, "DISMISS", 10.0, 36.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, (DismissButton, Tip::key("DISMISS", "ESC / ENTER")));
    });
}
