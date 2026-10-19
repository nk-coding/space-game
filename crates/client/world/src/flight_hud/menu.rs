//! The flight menu (GDR-0090): Esc with nothing locked opens it (`controls`), Esc again or RESUME closes it. In the
//! test range it pauses the flight; a voyage never pauses, so there it lies over the running fight. SETTINGS opens
//! the SETTINGS screen over it (`settings::OpenSettings`), CONTROLS the controls overlay over it (`help.rs`; Esc
//! comes back to the menu), SHIPYARD goes home (the test range), ABANDON VOYAGE… opens the ABANDON VOYAGE? panel (a
//! voyage). On a touch screen (docs/plans/android.md 4.2, GDR-0120; the ring top left opens it there) a second row
//! holds what only keys do: in the test range DRYDOCK (or BEHAVIOR, where T goes), WAVES OFF / ON (C) and RESTART
//! (Shift+R), and for a ship with pilot assists ASSISTS OFF / ON (Z), each through `ActionTaps`.
//!
//! ```text
//!          PAUSED                    (a voyage: MENU · THE FLIGHT GOES ON)
//!   [ RESUME ]  [ CONTROLS ]  [ SETTINGS ]  [ SHIPYARD ]          ([ ABANDON VOYAGE… ] in a voyage)
//!        [ DRYDOCK ]  [ WAVES OFF ]  [ RESTART ]  [ ASSISTS OFF ]  (touch)
//! ```

use bevy::prelude::*;
use kit::style::{Style, ui_color};
use data::input::Action;
use kit::ui_kit::{Edge, Hot, HudFonts, Pill, PillText, ScreenFx, ScreenPart, Tip, Tone, TouchScreen, UiClick, frame, label, panel_colors, panel_node, panel_title, set_text, show_screen, spawn_pill};

use super::{HudButton, HudZone};
use crate::mode::{AppMode, FlightUi};
use crate::scene::Scene;
use crate::settings::{ActionTaps, ActionTip, OpenSettings};
use crate::voyage::VoyageRun;

/// Is the flight menu open, and did it pause the flight (so closing it resumes)?
#[derive(Resource, Debug, Default)]
pub struct FlightMenu {
    pub open: bool,
    /// The controls overlay is open (`help.rs`): over the menu when it was opened there, which then waits under it.
    pub help: bool,
    paused: bool,
}

#[derive(Component)]
pub(super) struct MenuRoot;

#[derive(Component)]
pub(super) struct MenuTitle;

#[derive(Component)]
pub(super) struct MenuSub;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MenuButton {
    Resume,
    Controls,
    Settings,
    Shipyard,
    Abandon,
    /// Touch: T, C and Shift+R.
    Back,
    Waves,
    Restart,
    /// Touch: ALL ASSISTS (Z).
    Assists,
}

/// The touch screen's second row.
#[derive(Component)]
pub(super) struct TouchRow;

/// The menu: a dark plate in the middle of the view (a [`HudButton`]: clicks on it never reach the world).
pub(super) fn spawn_menu(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, clear: Res<ClearColor>) {
    let ui = &style.ui;
    commands
        .spawn((
            MenuRoot,
            FlightUi,
            ScreenFx::closed().dimming(),
            Visibility::Hidden,
            GlobalZIndex(12),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        ))
        .with_children(|root| {
            root.spawn((ScreenPart::new(Edge::Fade, 0), HudButton, HudZone, Hot::default(), panel_colors(clear.0, ui.teal), panel_node())).with_children(|p| {
                let f = frame();
                p.spawn((MenuTitle, panel_title(&fonts, "PAUSED", ui_color(ui.number, 1.0))));
                p.spawn((MenuSub, label(&fonts, "", f.hud.sub, ui_color(ui.dim, 1.0))));
                p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(f.pill_gap), margin: UiRect::top(px(f.panel.gap)), ..default() }).with_children(|row| {
                    let resume = Pill { active: true, ..Pill::new(Tone::Teal) };
                    spawn_pill(row, &fonts, "RESUME", 11.0, 36.0, None, resume, (MenuButton::Resume, Tip::key("RESUME", "ESC")));
                    spawn_pill(row, &fonts, "CONTROLS", 11.0, 36.0, None, Pill::new(Tone::Teal), (MenuButton::Controls, Tip::new("THE CONTROLS HERE"), ActionTip(data::input::Action::ControlsHelp)));
                    spawn_pill(row, &fonts, "SETTINGS", 11.0, 36.0, None, Pill::new(Tone::Teal), (MenuButton::Settings, Tip::new("CONTROLS AND DISPLAY")));
                    spawn_pill(row, &fonts, "SHIPYARD", 11.0, 36.0, None, Pill::new(Tone::Teal), (MenuButton::Shipyard, Tip::new("SHIPYARD"), ActionTip(data::input::Action::Leave)));
                    spawn_pill(row, &fonts, "ABANDON VOYAGE…", 11.0, 36.0, None, Pill::new(Tone::Coral), (MenuButton::Abandon, Tip::new("ABANDON VOYAGE"), ActionTip(data::input::Action::Leave)));
                });
                p.spawn((TouchRow, Node { display: Display::None, flex_direction: FlexDirection::Row, column_gap: px(f.pill_gap), margin: UiRect::top(px(f.panel.gap)), ..default() })).with_children(|row| {
                    for (b, text) in [(MenuButton::Back, "DRYDOCK"), (MenuButton::Waves, "WAVES OFF"), (MenuButton::Restart, "RESTART"), (MenuButton::Assists, "ASSISTS OFF")] {
                        spawn_pill(row, &fonts, text, 11.0, 36.0, None, Pill::new(Tone::Teal), b);
                    }
                });
            });
        });
}

/// Clicks, the pause (the test range only), what the menu shows.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn flight_menu(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<&MenuButton>,
    mut menu: ResMut<FlightMenu>,
    mut run: ResMut<VoyageRun>,
    scene: Res<Scene>,
    mut time: ResMut<Time<Virtual>>,
    mut next: ResMut<NextState<AppMode>>,
    mut settings: MessageWriter<OpenSettings>,
    mut root: Single<&mut ScreenFx, With<MenuRoot>>,
    mut texts: ParamSet<(Single<&mut Text, With<MenuTitle>>, Single<&mut Text, With<MenuSub>>, Query<(&ChildOf, &mut Text), With<PillText>>)>,
    mut shown: Query<(Entity, &MenuButton, &mut Node), Without<TouchRow>>,
    layer: Res<crate::settings::SettingsLayer>,
    (touch, exit, mut taps): (Res<TouchScreen>, Res<crate::mode::FlightExit>, ResMut<ActionTaps>),
    (focus, world): (Res<crate::sim_bridge::PilotFocus>, Res<crate::sim_bridge::SimWorld>),
    mut touch_row: Single<&mut Node, With<TouchRow>>,
) {
    let voyage = scene.voyage().is_some();
    for c in clicks.read() {
        match buttons.get(c.0) {
            Ok(MenuButton::Resume) => menu.open = false,
            Ok(MenuButton::Controls) => menu.help = true,
            Ok(MenuButton::Settings) => {
                settings.write(OpenSettings);
            }
            Ok(MenuButton::Shipyard) if !voyage => {
                menu.open = false;
                next.set(AppMode::Home);
            }
            Ok(MenuButton::Abandon) if voyage => {
                menu.open = false;
                run.asking = true;
            }
            Ok(b @ (MenuButton::Back | MenuButton::Waves | MenuButton::Restart | MenuButton::Assists)) => {
                menu.open = false;
                taps.tap(match b {
                    MenuButton::Back => Action::TestReturn,
                    MenuButton::Waves => Action::Waves,
                    MenuButton::Restart => Action::ResetScene,
                    _ => Action::AllAssists,
                });
            }
            _ => {}
        }
    }
    // The ABANDON VOYAGE? panel takes the menu's place.
    if run.asking && menu.open {
        menu.open = false;
    }
    // The test range pauses under the menu; a voyage never pauses.
    if menu.open && !voyage && !time.is_paused() {
        time.pause();
        menu.paused = true;
    } else if !menu.open && menu.paused {
        menu.paused = false;
        time.unpause();
    }
    // SETTINGS or the controls opened from it cover it; it comes back when they close.
    show_screen(&mut root, menu.open && !menu.help && !layer.open);
    if menu.open {
        let (title, sub) = match (voyage, scene.kind == crate::scene::SceneKind::Fight) {
            (true, _) => ("MENU", "THE FLIGHT GOES ON"),
            (false, true) => ("PAUSED", "THE REPLAY WAITS"),
            (false, false) => ("PAUSED", "THE TEST RANGE WAITS"),
        };
        if texts.p0().0 != title {
            texts.p0().0 = title.into();
        }
        if texts.p1().0 != sub {
            texts.p1().0 = sub.into();
        }
        let range = scene.kind == crate::scene::SceneKind::Sandbox;
        let assists = focus.0.and_then(|id| world.sim.ship(id)).and_then(|s| s.assists()).filter(|a| !a.program().program.rules.is_empty());
        let mut touch_buttons = false;
        for (e, b, mut node) in &mut shown {
            let on = match b {
                MenuButton::Shipyard => !voyage,
                MenuButton::Abandon => voyage,
                MenuButton::Back | MenuButton::Restart => range,
                MenuButton::Waves => range && scene.test_mode().is_none(),
                MenuButton::Assists => assists.is_some(),
                _ => true,
            };
            touch_buttons |= on && matches!(b, MenuButton::Back | MenuButton::Waves | MenuButton::Restart | MenuButton::Assists);
            let d = if on { Display::Flex } else { Display::None };
            if node.display != d {
                node.display = d;
            }
            // What the touch row's buttons do here.
            let word = match b {
                MenuButton::Back if exit.0 == AppMode::Behavior => "BEHAVIOR",
                MenuButton::Back => "DRYDOCK",
                MenuButton::Waves if scene.waves => "WAVES OFF",
                MenuButton::Waves => "WAVES ON",
                MenuButton::Assists if assists.is_some_and(|a| a.all_on()) => "ASSISTS OFF",
                MenuButton::Assists => "ASSISTS ON",
                _ => continue,
            };
            for (parent, mut text) in &mut texts.p2() {
                if parent.parent() == e {
                    set_text(&mut text, word);
                }
            }
        }
        let d = if touch.on() && touch_buttons { Display::Flex } else { Display::None };
        if touch_row.display != d {
            touch_row.display = d;
        }
    }
}

/// Leaving flight closes the menu (and gives the clock back).
pub(super) fn close_menu(mut menu: ResMut<FlightMenu>, mut time: ResMut<Time<Virtual>>) {
    if menu.paused {
        time.unpause();
    }
    *menu = FlightMenu::default();
}
