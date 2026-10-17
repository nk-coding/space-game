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
//! 14 PARTS DISMANTLED
//!   Automaton cannon ×6              KINETIC WEAPONS +18
//! RESEARCH DATA
//!   • KINETIC WEAPONS                  +18   IN STORE 18
//! ◆ PROGRAMMABLE MATTER +1
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

use bevy::prelude::*;
use data::career::Outcome;
use kit::style::{Style, ui_color};
use kit::ui_kit::{
    HudFonts, Edge, Hot, Pill, ScreenFx, ScreenPart, Scrollbar, Tip, Tone, show_screen, UiBlock, UiClick, UiInputSet, UiPointer, UiStyleSet, label, number, prose, spawn_pill,
};
use world::mode::AppMode;
use world::settings::SettingsLayer;
use world::voyage::clock;
use world::voyage::debrief::{Debrief, DebriefData, amount};

use crate::fleet::Fleet;
use crate::voyage_log::VoyageLog;

/// Rows of parts shown (the rest are counted in one line).
const MAX_ROWS: usize = 6;
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

pub struct DebriefPlugin;

impl Plugin for DebriefPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Debrief>()
            .add_systems(Startup, spawn.after(world::flight_hud::spawn_hud))
            .add_systems(Update, (dismiss, rebuild, show).chain().after(UiInputSet).before(UiStyleSet));
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
        BackgroundColor(ui_color(clear.0, 0.94)),
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
    fonts: Res<HudFonts>,
    style: Res<Style>,
    panel: Single<Entity, With<PanelRoot>>,
) {
    if !debrief.is_changed() {
        return;
    }
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
        .with_children(|body| fill(body, &fonts, &style, d));
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
fn fill(p: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, d: &DebriefData) {
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
        let parts = format!("{} {} DISMANTLED", d.items, if d.items == 1 { "PART" } else { "PARTS" });
        p.spawn(label(fonts, &parts, 9.5, c(Tone::Label)));
        if d.rows.is_empty() {
            p.spawn(prose(fonts, "The holds came home empty.", 13.0, c(Tone::Dim)));
        }
        for r in d.rows.iter().take(MAX_ROWS) {
            p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(10.0) }).with_children(|line| {
                line.spawn(row(8.0)).with_children(|l| {
                    l.spawn((Node { width: px(7), height: px(7), border_radius: BorderRadius::MAX, ..default() }, BackgroundColor(fam(&r.color))));
                    l.spawn(prose(fonts, &format!("{} ×{}", r.name, r.count), 14.0, c(Tone::Number)));
                });
                line.spawn(label(fonts, &r.gives, 8.5, fam(&r.color)));
            });
        }
        if d.rows.len() > MAX_ROWS {
            let more: u32 = d.rows[MAX_ROWS..].iter().map(|r| r.count).sum();
            p.spawn(label(fonts, &format!("+ {more} MORE"), 9.0, c(Tone::Dim)));
        }
        if !d.families.is_empty() || !d.materials.is_empty() {
            rule(p);
            p.spawn(label(fonts, "RESEARCH DATA", 9.5, c(Tone::Label)));
            for f in &d.families {
                p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(10.0) }).with_children(|line| {
                    line.spawn(row(8.0)).with_children(|l| {
                        l.spawn((Node { width: px(7), height: px(7), border_radius: BorderRadius::MAX, ..default() }, BackgroundColor(fam(&f.color))));
                        l.spawn(label(fonts, &f.name, 10.0, c(Tone::Number)));
                    });
                    line.spawn(row(14.0)).with_children(|l| {
                        l.spawn(number(fonts, &format!("+{}", amount(f.gained)), 15.0, c(Tone::Number)));
                        l.spawn(label(fonts, &format!("IN STORE {}", amount(f.store)), 8.5, c(Tone::Dim)));
                    });
                });
            }
            for (m, n) in &d.materials {
                p.spawn(row(7.0)).with_children(|l| {
                    l.spawn(diamond(c(Tone::Gold), 8.0));
                    l.spawn(label(fonts, &format!("{} +{n}", m.to_uppercase()), 9.5, c(Tone::Gold)));
                });
            }
        }
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

/// VOYAGE MAP (after a voyage that found a signal) and DISMISS, at the panel's bottom.
fn buttons(p: &mut ChildSpawnerCommands, fonts: &HudFonts, d: &DebriefData) {
    p.spawn(Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd, column_gap: px(10), flex_shrink: 0.0, ..default() }).with_children(|r| {
        if d.to_map() {
            spawn_pill(r, fonts, "VOYAGE MAP", 10.0, 36.0, None, Pill::new(Tone::Teal), MapButton);
        }
        spawn_pill(r, fonts, "DISMISS", 10.0, 36.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, (DismissButton, Tip::key("DISMISS", "ESC / ENTER")));
    });
}
