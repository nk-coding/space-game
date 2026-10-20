//! SETTINGS (GDR-0090): a layer over home (the gear ring top right, or its key, O) and over flight (the flight menu's
//! SETTINGS), with three tabs.
//!
//! ```text
//! (o) SHIPYARD › SETTINGS  [EVOLVING 12 / 50]                  [CONTROLS] [DISPLAY]   [CLOSE]
//!     Settings
//!   FLIGHT
//!   Turn left      Rotates the ship to the left; …            [   A   ] [   ←   ]
//!   Thrust forward Pushes along the nose.                     [   W   ] [   ↑   ]
//!   …                                                                    (scrolls)
//!   CLICK AN INPUT, THEN PRESS A KEY OR A MOUSE BUTTON              [CLEAR] [RESET TO DEFAULTS]
//! ```
//!
//! * **CONTROLS**: every action of the action map (`world::settings::Controls`) by section, with its two inputs.
//!   A click on an input waits for the next key or mouse button (with Ctrl, Shift or Alt held: a combination; a
//!   modifier alone binds when it is released); Esc cancels, CLEAR empties the input, the left mouse button
//!   (the pointer) cancels elsewhere. An input another action on the same screen had moves over, and the line
//!   says so. RESET TO DEFAULTS brings back the content's defaults. Every change is saved at once
//!   (`settings/controls`).
//! * **DISPLAY**: GLOW and BLOOM on or off, the UI size (100 %, 90 %, 80 % of what fits the window); saved at once
//!   (`settings/display`).
//! * **SAVES** (home only, ADR-0050; [`saves`]): EXPORT everything the game keeps into one file, IMPORT one back.
//!
//! Esc (not while waiting for an input) and CLOSE close it; home's places and flight's keys wait meanwhile
//! (`SettingsLayer`). UI words are chrome in code, like home's; the actions' names and summaries are content.

use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use data::input::{Action, Button, Chord, MAX_INPUTS, Section};
use kit::params::RenderToggles;
use kit::style::{Style, ui_color};
use kit::ui_icons::Glyph;
use kit::ui_kit::{
    CompactText, LayoutClass, compact_scale, Edge, HudFonts, Pill, PillText, ScreenFx, ScreenPart, Scrollbar, Tip, Tone, TouchScreen, UiClick, UiInputSet, UiPointer, UiSize, UiStyleSet, label, prose, ring_glyph,
    ring_node, set_text, show_screen, spawn_pill, ui_scale_for, crumb, frame, spawn_back, spawn_close, title, top_pill,
};
use world::mode::AppMode;
use world::settings::{Act, ActionTip, Controls, DisplaySettings, OpenSettings, SettingsLayer, UI_SIZES, button_of, mods_of, mouse_of};
use world::sim_bridge::GameContent;
use world::storage::Storage;

use crate::fleet::Fleet;
use crate::home::{COMPACT_TEXT, by_class};
use crate::voyage_log::VoyageLog;

mod saves;

/// Where the screen's parts go (stage px), like FLEET's: [`Lay::WIDE`] a 1280 × 720 stage, [`Lay::COMPACT`] a
/// phone's 844 × 390 at `compact_scale` (docs/client.md "Phones": the list across the stage, narrower names and
/// chips).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Lay {
    stage: Vec2,
    /// The controls list's box; the bottom line's gap to the stage's bottom.
    list: (Vec2, Vec2),
    bottom: f32,
    /// The widths of an action's name, a display choice's words and an input chip.
    name_w: f32,
    about_w: f32,
    chip_w: f32,
}

impl Lay {
    const WIDE: Lay = Lay { stage: Vec2::new(1280.0, 720.0), list: (Vec2::new(120.0, 104.0), Vec2::new(1040.0, 540.0)), bottom: 22.0, name_w: 220.0, about_w: 420.0, chip_w: 132.0 };
    const COMPACT: Lay = Lay { stage: Vec2::new(844.0, 390.0), list: (Vec2::new(16.0, 64.0), Vec2::new(812.0, 276.0)), bottom: 10.0, name_w: 150.0, about_w: 300.0, chip_w: 96.0 };
}

/// A row's height in the controls list.
const ROW: f32 = 34.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Controls,
    Display,
    Saves,
}

/// The screen's state: the tab, the input waiting for a key, what the status line says.
#[derive(Resource, Debug, Default)]
struct SettingsUi {
    tab: Tab,
    /// The input waiting for a key: action and slot.
    capture: Option<(Action, usize)>,
    /// The modifier key held alone since the capture began (it binds on release if nothing else came).
    lone: Option<KeyCode>,
    say: Option<(String, Tone)>,
    /// Where the UI scale was before the screen opened over flight (it fits the stage while open).
    flight_scale: Option<f32>,
}

#[derive(Component)]
struct SettingsRoot;

/// The gear ring on home's screen (spawned by home, top right).
#[derive(Component)]
pub struct SettingsRing;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
enum SettingsButton {
    Close,
    Tab(Tab),
    Chip(Action, usize),
    Clear,
    Reset,
    Glow(bool),
    Bloom(bool),
    Size(usize),
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum SettingsText {
    Crumb,
    Status,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct Pane(Tab);

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SettingsUi>()
            .add_systems(Startup, spawn.after(world::flight_hud::spawn_hud))
            .add_systems(OnExit(AppMode::Home), close)
            .add_systems(OnExit(AppMode::Flight), close)
            .add_systems(Update, (open, input, capture, sync, show, fit).chain().after(UiInputSet).before(UiStyleSet));
        saves::plugin(app);
    }
}

/// Is SETTINGS open (home's places wait then)?
pub fn is_open(layer: Res<SettingsLayer>) -> bool {
    layer.open
}

/// Opens it: at home the gear ring or its key (not while FLEET, the LOG or EVOLUTION is open or a field types), in flight the
/// menu's SETTINGS.
#[allow(clippy::too_many_arguments)]
fn open(
    mode: Res<State<AppMode>>,
    mut clicks: MessageReader<UiClick>,
    ring: Query<(), With<SettingsRing>>,
    act: Act,
    pointer: Res<UiPointer>,
    (fleet, log, evo, ws): (Res<Fleet>, Res<VoyageLog>, Res<crate::evolution::EvolutionLayer>, Res<crate::workshop::WorkshopLayer>),
    mut asked: MessageReader<OpenSettings>,
    mut layer: ResMut<SettingsLayer>,
    mut ui: ResMut<SettingsUi>,
) {
    let mode = *mode.get();
    let home = mode == AppMode::Home && !fleet.open && !log.open && !evo.open && !ws.open && !layer.open;
    let by_ring = clicks.read().any(|c| ring.contains(c.0));
    let by_key = home && !pointer.typing && act.just_pressed(Action::OpenSettings);
    let by_menu = asked.read().count() > 0 && mode == AppMode::Flight;
    if (home && (by_ring || by_key)) || by_menu {
        layer.open = true;
        ui.capture = None;
        ui.say = None;
        // SAVES is home's only.
        if by_menu && ui.tab == Tab::Saves {
            ui.tab = Tab::Controls;
        }
    }
}

/// Leaving the mode closes it.
fn close(mut layer: ResMut<SettingsLayer>, mut ui: ResMut<SettingsUi>) {
    layer.open = false;
    ui.capture = None;
}

/// Clicks and Esc.
#[allow(clippy::too_many_arguments)]
fn input(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<&SettingsButton>,
    keys: Res<ButtonInput<KeyCode>>,
    mut layer: ResMut<SettingsLayer>,
    mut ui: ResMut<SettingsUi>,
    mut controls: ResMut<Controls>,
    mut display: ResMut<DisplaySettings>,
    mut toggles: ResMut<RenderToggles>,
    mut size: ResMut<UiSize>,
    mut storage: ResMut<Storage>,
    content: Res<GameContent>,
    touch: Res<TouchScreen>,
) {
    if !layer.open {
        clicks.clear();
        return;
    }
    if keys.just_pressed(KeyCode::Escape) && ui.capture.is_none() {
        layer.open = false;
        return;
    }
    let mut display_changed = false;
    for c in clicks.read() {
        let Ok(b) = buttons.get(c.0) else { continue };
        match *b {
            SettingsButton::Close => {
                layer.open = false;
                ui.capture = None;
                return;
            }
            SettingsButton::Tab(t) => {
                ui.tab = t;
                ui.capture = None;
                ui.say = None;
            }
            SettingsButton::Chip(a, slot) => {
                ui.capture = if ui.capture == Some((a, slot)) { None } else { Some((a, slot)) };
                ui.lone = None;
                let name = content.0.controls.get(a.id()).map_or(a.id().to_string(), |c| c.name.to_uppercase());
                let how = touch.says("PRESS A KEY OR A MOUSE BUTTON  ·  ESC CANCELS", "PRESS A KEY ON A KEYBOARD  ·  A TAP ON IT AGAIN CANCELS");
                ui.say = ui.capture.map(|_| (format!("{name}  ·  {how}"), Tone::Teal));
            }
            SettingsButton::Clear => {
                if let Some((a, slot)) = ui.capture.take() {
                    controls.0.clear(a, slot);
                    ui.say = Some(saved(&controls, &mut storage, "CLEARED".into()));
                }
            }
            SettingsButton::Reset => {
                controls.0.reset();
                ui.capture = None;
                ui.say = Some(saved(&controls, &mut storage, "EVERY CONTROL IS BACK TO ITS DEFAULT".into()));
            }
            SettingsButton::Glow(on) => {
                display.glow = on;
                display_changed = true;
            }
            SettingsButton::Bloom(on) => {
                display.bloom = on;
                display_changed = true;
            }
            SettingsButton::Size(k) => {
                display.ui_size = UI_SIZES[k];
                display_changed = true;
            }
        }
    }
    if display_changed {
        display.apply(&mut toggles, &mut size);
        ui.say = Some(match display.save(&mut storage) {
            Ok(()) => ("SAVED ON THIS DEVICE".into(), Tone::Label),
            Err(e) => (format!("NOT SAVED: {e}").to_uppercase(), Tone::Coral),
        });
    }
}

/// Saves the bindings and says what happened (or why they were not saved).
fn saved(controls: &Controls, storage: &mut Storage, what: String) -> (String, Tone) {
    match controls.save(storage) {
        Ok(()) => (what, Tone::Label),
        Err(e) => (format!("{what}  ·  NOT SAVED: {e}").to_uppercase(), Tone::Coral),
    }
}

/// While an input waits: the next key (with the modifiers held) or mouse button binds; Esc cancels; a modifier
/// alone binds when it is released; the left button (the pointer) cancels unless it hit a chip or CLEAR.
#[allow(clippy::too_many_arguments)]
fn capture(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    layer: Res<SettingsLayer>,
    mut ui: ResMut<SettingsUi>,
    mut controls: ResMut<Controls>,
    mut storage: ResMut<Storage>,
    content: Res<GameContent>,
    pointer: Res<UiPointer>,
) {
    let Some((action, slot)) = ui.capture.filter(|_| layer.open) else { return };
    if keys.just_pressed(KeyCode::Escape) {
        ui.capture = None;
        ui.say = None;
        return;
    }
    let mods = mods_of(&keys);
    let modifier = |k: KeyCode| {
        matches!(k, KeyCode::ShiftLeft | KeyCode::ShiftRight | KeyCode::ControlLeft | KeyCode::ControlRight | KeyCode::AltLeft | KeyCode::AltRight)
    };
    let mut got: Option<Button> = None;
    for k in keys.get_just_pressed() {
        if modifier(*k) {
            ui.lone = Some(*k);
        } else if !matches!(k, KeyCode::Enter | KeyCode::NumpadEnter) {
            ui.lone = None;
            got = got.or(button_of(*k));
        }
    }
    if got.is_none()
        && let Some(k) = ui.lone
        && keys.just_released(k)
    {
        got = button_of(k);
    }
    for b in mouse.get_just_pressed() {
        if *b == MouseButton::Left {
            // The pointer: a click elsewhere cancels (a chip or CLEAR handles its own click).
            if !pointer.blocked {
                ui.capture = None;
                ui.say = None;
                return;
            }
        } else {
            got = got.or(mouse_of(*b).map(Button::Mouse));
        }
    }
    let Some(button) = got else { return };
    let chord = Chord::new(button, mods);
    let moved = controls.0.set(action, slot, chord);
    ui.capture = None;
    ui.lone = None;
    let name = |a: Action| content.0.controls.get(a.id()).map_or(a.id().to_string(), |c| c.name.to_uppercase());
    let what = match moved {
        Some((other, _)) => format!("{} IS {} NOW  ·  {} LOST IT", chord.label(), name(action), name(other)),
        None => format!("{} IS {} NOW", chord.label(), name(action)),
    };
    ui.say = Some(saved(&controls, &mut storage, what));
}

/// The chips' words and lights, the tabs, the display's choices, the status line (written when something changed).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync(
    mode: Res<State<AppMode>>,
    ui: Res<SettingsUi>,
    controls: Res<Controls>,
    display: Res<DisplaySettings>,
    layer: Res<SettingsLayer>,
    mut buttons: Query<(&SettingsButton, &mut Pill, &Children)>,
    mut chip_texts: Query<&mut Text, With<PillText>>,
    mut texts: Query<(&SettingsText, &mut Text, &mut TextColor), Without<PillText>>,
    mut panes: Query<(&Pane, &mut Node)>,
    style: Res<Style>,
    touch: Res<TouchScreen>,
) {
    if !(ui.is_changed() || controls.is_changed() || display.is_changed() || layer.is_changed() || mode.is_changed()) {
        return;
    }
    for (b, mut pill, children) in &mut buttons {
        let (active, disabled) = match *b {
            SettingsButton::Tab(t) => (ui.tab == t, t == Tab::Saves && *mode.get() == AppMode::Flight),
            SettingsButton::Chip(a, slot) => (ui.capture == Some((a, slot)), false),
            SettingsButton::Clear => (false, ui.capture.is_none()),
            SettingsButton::Glow(on) => (display.glow == on, false),
            SettingsButton::Bloom(on) => (display.bloom == on, false),
            SettingsButton::Size(k) => ((display.ui_size - UI_SIZES[k]).abs() < 1e-3, false),
            SettingsButton::Close | SettingsButton::Reset => (false, false),
        };
        if pill.active != active || pill.disabled != disabled {
            pill.active = active;
            pill.disabled = disabled;
        }
        if let SettingsButton::Chip(a, slot) = *b {
            let word = if ui.capture == Some((a, slot)) {
                "…".to_string()
            } else {
                controls.0.get(a).and_then(|bind| bind.inputs[slot]).map_or_else(|| "—".into(), |c| c.label())
            };
            for child in children {
                if let Ok(mut t) = chip_texts.get_mut(*child) {
                    set_text(&mut t, &word);
                }
            }
        }
    }
    let ui_pal = style.ui;
    for (t, mut text, mut color) in &mut texts {
        match t {
            SettingsText::Crumb => {
                let from = if *mode.get() == AppMode::Flight { "FLIGHT  ›  SETTINGS" } else { "SHIPYARD  ›  SETTINGS" };
                set_text(&mut text, from);
            }
            SettingsText::Status => {
                let (words, tone) = ui.say.clone().unwrap_or_else(|| match ui.tab {
                    Tab::Controls => (touch.says("CLICK AN INPUT, THEN PRESS A KEY OR A MOUSE BUTTON", "TAP AN INPUT, THEN PRESS A KEY ON A KEYBOARD").into(), Tone::Dim),
                    Tab::Display => ("CHANGES SHOW AT ONCE AND ARE SAVED ON THIS DEVICE".into(), Tone::Dim),
                    Tab::Saves => ("EXPORT KEEPS A COPY OF YOUR SAVES  ·  IMPORT BRINGS ONE BACK".into(), Tone::Dim),
                });
                set_text(&mut text, &words);
                let c = ui_color(tone.color(&ui_pal), 1.0);
                if color.0 != c {
                    color.0 = c;
                }
            }
        }
    }
    for (p, mut node) in &mut panes {
        let d = if p.0 == ui.tab { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
}

/// Shown at home and over flight while open, with its transition. Over flight it lays a dark ground over the ships
/// (at home the backdrop's veil and the station's fade do that, as for FLEET).
fn show(
    mode: Res<State<AppMode>>,
    layer: Res<SettingsLayer>,
    clear: Res<ClearColor>,
    root: Single<(&mut ScreenFx, &mut BackgroundColor), With<SettingsRoot>>,
) {
    let (mut fx, mut ground) = root.into_inner();
    let flight = *mode.get() == AppMode::Flight;
    show_screen(&mut fx, layer.open && (flight || *mode.get() == AppMode::Home));
    let want = if flight && fx.visible() { ui_color(clear.0, 0.94 * fx.alpha()) } else { Color::NONE };
    if ground.0 != want {
        ground.0 = want;
    }
}

/// Over flight (whose UI scale is the plain UI size, on a phone the phone HUD's) the stage is fitted to the window while open (on a phone the
/// phone layout's scale, as at home), and the flight's scale comes back when it closes.
fn fit(
    mode: Res<State<AppMode>>,
    layer: Res<SettingsLayer>,
    window: Single<&Window, With<PrimaryWindow>>,
    size: Res<UiSize>,
    class: Res<LayoutClass>,
    mut ui: ResMut<SettingsUi>,
    mut scale: ResMut<UiScale>,
) {
    if *mode.get() != AppMode::Flight {
        ui.flight_scale = None;
        return;
    }
    if layer.open {
        ui.flight_scale.get_or_insert(scale.0);
        let s = if class.is_compact() { compact_scale(window.size()) * size.0 } else { ui_scale_for(&window, *size) };
        if (scale.0 - s).abs() > 1e-3 {
            scale.0 = s;
        }
    } else if let Some(flight) = ui.flight_scale.take()
        && scale.0 != flight
    {
        // The flight's own scale (on a phone the phone HUD's, `world::flight_hud`).
        scale.0 = flight;
    }
}

fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, content: Res<GameContent>, touch: Res<TouchScreen>, params: Option<Res<kit::params::Params>>) {
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
    // Every part is laid out for both layout classes (`kit::ui_kit::ByClass`).
    let (wl, cl) = (Lay::WIDE, Lay::COMPACT);
    let width = |w: f32| Node { width: px(w), flex_shrink: 0.0, ..default() };
    commands
        .spawn((
            SettingsRoot,
            CompactText(COMPACT_TEXT),
            world::backdrop::VeilsBackdrop,
            ScreenFx::closed().dimming(),
            GlobalZIndex(20),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            BackgroundColor(Color::NONE),
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
                let stage_node = |l: Lay| Node { width: px(l.stage.x), height: px(l.stage.y), flex_shrink: 0.0, ..default() };
                centre.spawn(by_class(stage_node(wl), stage_node(cl))).with_children(|stage| {

                    // CONTROLS: every action by section, scrolling.
                    let list = |l: Lay| Node { flex_direction: FlexDirection::Column, overflow: Overflow::scroll_y(), padding: UiRect::right(px(14)), ..abs(l.list.0, Some(l.list.1)) };
                    stage
                        .spawn((Pane(Tab::Controls), ScreenPart::new(Edge::Fade, 1), Scrollbar::vertical(), ScrollPosition::default(), by_class(list(wl), list(cl))))
                        .with_children(|list| {
                            for section in Section::ALL {
                                // The debug keys only in the dev mode (ADR-0046).
                                let here = |e: &&data::Control| Action::from_id(&e.id).is_some_and(|a| world::settings::acts_here(a, params.as_deref()));
                                let entries: Vec<&data::Control> = content.0.controls.iter().filter(|e| e.section == section).filter(here).collect();
                                if entries.is_empty() {
                                    continue;
                                }
                                list.spawn((label(fonts, &section.title().to_uppercase(), 10.0, c(Tone::Gold, 1.0)), Node { margin: UiRect::new(px(0), px(0), px(14), px(6)), flex_shrink: 0.0, ..default() }));
                                for e in entries {
                                    let Some(action) = Action::from_id(&e.id) else { continue };
                                    list.spawn(Node { height: px(ROW), flex_shrink: 0.0, ..row(16.0) }).with_children(|r| {
                                        r.spawn((label(fonts, &e.name.to_uppercase(), 10.0, c(Tone::Number, 1.0)), by_class(width(wl.name_w), width(cl.name_w))));
                                        r.spawn((prose(fonts, &e.summary, 11.0, c(Tone::Label, 1.0)), Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() }));
                                        for slot in 0..MAX_INPUTS {
                                            r.spawn(by_class(width(wl.chip_w), width(cl.chip_w))).with_children(|w| {
                                                let chip = spawn_pill(w, fonts, "—", 10.0, 26.0, None, Pill::new(Tone::Teal), (SettingsButton::Chip(action, slot), Tip::new(touch.says("CLICK, THEN PRESS A KEY", "TAP, THEN PRESS A KEY"))));
                                                // Every chip as wide as its column, so the inputs line up.
                                                w.commands().entity(chip).entry::<Node>().and_modify(|mut n| n.width = percent(100));
                                            });
                                        }
                                    });
                                }
                            }
                        });

                    // DISPLAY: three choices.
                    let pane = |l: Lay| Node { flex_direction: FlexDirection::Column, row_gap: px(18), ..abs(l.list.0 + Vec2::new(0.0, 20.0), Some(Vec2::new(l.list.1.x, 300.0))) };
                    stage
                        .spawn((Pane(Tab::Display), ScreenPart::new(Edge::Fade, 1), by_class(pane(wl), pane(cl))))
                        .with_children(|d| {
                            let choice = |d: &mut ChildSpawnerCommands, name: &str, about: &str, options: Vec<(&str, SettingsButton)>| {
                                d.spawn(row(16.0)).with_children(|r| {
                                    r.spawn((label(fonts, name, 10.0, c(Tone::Number, 1.0)), by_class(width(wl.name_w), width(cl.name_w))));
                                    r.spawn((prose(fonts, about, 11.0, c(Tone::Label, 1.0)), by_class(width(wl.about_w), width(cl.about_w))));
                                    for (word, b) in options {
                                        spawn_pill(r, fonts, word, 9.5, 28.0, None, Pill::new(Tone::Teal), (b, Tip::new(word.to_string())));
                                    }
                                });
                            };
                            choice(d, "GLOW", "The soft glow around ships, lights and icons.", vec![("ON", SettingsButton::Glow(true)), ("OFF", SettingsButton::Glow(false))]);
                            choice(d, "BLOOM", "A full-screen bloom over the bright parts (costs more).", vec![("ON", SettingsButton::Bloom(true)), ("OFF", SettingsButton::Bloom(false))]);
                            choice(d, "UI SIZE", "The screens' size, as a share of what fits the window.", vec![("100 %", SettingsButton::Size(0)), ("90 %", SettingsButton::Size(1)), ("80 %", SettingsButton::Size(2))]);
                        });

                    // SAVES: export and import.
                    saves::spawn_pane(stage, fonts, &style);

                    // Bottom: the status line, CLEAR and RESET.
                    let bottom = |l: Lay| Node {
                        position_type: PositionType::Absolute,
                        left: px(l.list.0.x),
                        right: px(l.stage.x - l.list.0.x - l.list.1.x),
                        bottom: px(l.bottom),
                        justify_content: JustifyContent::SpaceBetween,
                        ..row(12.0)
                    };
                    stage
                        .spawn((ScreenPart::new(Edge::Bottom, 2), by_class(bottom(wl), bottom(cl))))
                        .with_children(|b| {
                            b.spawn((SettingsText::Status, label(fonts, "", 9.5, c(Tone::Dim, 1.0))));
                            b.spawn((Pane(Tab::Controls), row(10.0))).with_children(|r| {
                                spawn_pill(r, fonts, "CLEAR", 9.5, 30.0, None, Pill::new(Tone::Coral), (SettingsButton::Clear, Tip::new("EMPTY THE PICKED INPUT")));
                                spawn_pill(r, fonts, "RESET TO DEFAULTS", 9.5, 30.0, None, Pill::new(Tone::Teal), (SettingsButton::Reset, Tip::new("EVERY CONTROL BACK TO ITS DEFAULT")));
                            });
                        });
                });
            });
            // Top left: the way back, where we are, the title; top right: the tabs, CLOSE (the frame's corners).
            let f = frame();
            root.spawn((ScreenPart::new(Edge::Top, 0), f.top_left())).with_children(|tl| {
                spawn_back(tl, &ui, (SettingsButton::Close, Tip::key("CLOSE", "ESC")));
                tl.spawn(f.heading()).with_children(|col| {
                    col.spawn((SettingsText::Crumb, crumb(fonts, "SHIPYARD  ›  SETTINGS", &ui)));
                    col.spawn(title(fonts, "Settings", &ui));
                });
                crate::evolution::spawn_chip(tl, fonts);
            });
            root.spawn((ScreenPart::new(Edge::Top, 0), f.top_right())).with_children(|tr| {
                top_pill(tr, fonts, "CONTROLS", Pill::new(Tone::Teal), (SettingsButton::Tab(Tab::Controls), Tip::new("CONTROLS")));
                top_pill(tr, fonts, "DISPLAY", Pill::new(Tone::Teal), (SettingsButton::Tab(Tab::Display), Tip::new("DISPLAY")));
                top_pill(tr, fonts, "SAVES", Pill::new(Tone::Teal), (SettingsButton::Tab(Tab::Saves), Tip::new("EXPORT AND IMPORT YOUR SAVES")));
                spawn_close(tr, fonts, SettingsButton::Close);
            });
        });
}

/// Spawns the gear ring for home's top right (home calls this into its screen root).
pub fn spawn_ring(parent: &mut ChildSpawnerCommands) {
    let f = frame();
    parent.spawn((SettingsRing, ring_node(f.ring), Tip::new("SETTINGS"), ActionTip(Action::OpenSettings))).with_child(ring_glyph(Glyph::Gear, f.ring_glyph));
}
