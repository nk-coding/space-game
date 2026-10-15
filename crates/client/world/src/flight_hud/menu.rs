//! The flight menu (GDR-0090): Esc with nothing locked opens it (`controls`), Esc again or RESUME closes it. In the
//! test range it pauses the flight; a voyage never pauses, so there it lies over the running fight. SETTINGS opens
//! the SETTINGS screen over it (`settings::OpenSettings`), SHIPYARD goes home (the test range), ABANDON VOYAGE…
//! opens the ABANDON VOYAGE? panel (a voyage).
//!
//! ```text
//!          PAUSED                    (a voyage: MENU · THE FLIGHT GOES ON)
//!   [ RESUME ]  [ SETTINGS ]  [ SHIPYARD ]          ([ ABANDON VOYAGE… ] in a voyage)
//! ```

use bevy::prelude::*;
use kit::style::{Style, ui_color};
use kit::ui_kit::{Edge, Hot, HudFonts, Pill, ScreenFx, ScreenPart, Tip, Tone, UiClick, label, show_screen, spawn_pill, tracked};

use super::{HudButton, HudZone};
use crate::mode::{AppMode, FlightUi};
use crate::scene::Scene;
use crate::settings::{ActionTip, OpenSettings};
use crate::voyage::VoyageRun;

/// Is the flight menu open, and did it pause the flight (so closing it resumes)?
#[derive(Resource, Debug, Default)]
pub struct FlightMenu {
    pub open: bool,
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
    Settings,
    Shipyard,
    Abandon,
}

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
            root.spawn((
                ScreenPart::new(Edge::Fade, 0),
                HudButton,
                HudZone,
                Hot::default(),
                BackgroundColor(ui_color(clear.0, 0.94)),
                BorderColor::all(ui_color(ui.teal, 0.35)),
                Node {
                    border: UiRect::all(px(1)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::new(px(36), px(36), px(26), px(26)),
                    row_gap: px(10),
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn((MenuTitle, tracked(&fonts, "PAUSED", 24.0, ui_color(ui.number, 1.0), 0.3)));
                p.spawn((MenuSub, label(&fonts, "", 9.0, ui_color(ui.dim, 1.0))));
                p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(12), margin: UiRect::top(px(10)), ..default() }).with_children(|row| {
                    let resume = Pill { active: true, ..Pill::new(Tone::Teal) };
                    spawn_pill(row, &fonts, "RESUME", 11.0, 36.0, None, resume, (MenuButton::Resume, Tip::key("RESUME", "ESC")));
                    spawn_pill(row, &fonts, "SETTINGS", 11.0, 36.0, None, Pill::new(Tone::Teal), (MenuButton::Settings, Tip::new("CONTROLS AND DISPLAY")));
                    spawn_pill(row, &fonts, "SHIPYARD", 11.0, 36.0, None, Pill::new(Tone::Teal), (MenuButton::Shipyard, Tip::new("SHIPYARD"), ActionTip(data::input::Action::Leave)));
                    spawn_pill(row, &fonts, "ABANDON VOYAGE…", 11.0, 36.0, None, Pill::new(Tone::Coral), (MenuButton::Abandon, Tip::new("ABANDON VOYAGE"), ActionTip(data::input::Action::Leave)));
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
    mut texts: ParamSet<(Single<&mut Text, With<MenuTitle>>, Single<&mut Text, With<MenuSub>>)>,
    mut shown: Query<(&MenuButton, &mut Node)>,
    layer: Res<crate::settings::SettingsLayer>,
) {
    let voyage = scene.voyage().is_some();
    for c in clicks.read() {
        match buttons.get(c.0) {
            Ok(MenuButton::Resume) => menu.open = false,
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
    // SETTINGS opened from it covers it; it comes back when SETTINGS closes.
    show_screen(&mut root, menu.open && !layer.open);
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
        for (b, mut node) in &mut shown {
            let on = match b {
                MenuButton::Shipyard => !voyage,
                MenuButton::Abandon => voyage,
                _ => true,
            };
            let d = if on { Display::Flex } else { Display::None };
            if node.display != d {
                node.display = d;
            }
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
