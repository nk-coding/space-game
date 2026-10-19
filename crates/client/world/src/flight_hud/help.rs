//! The controls overlay (18 Oct 2026, in place of the help line along the bottom): CONTROLS in the flight menu (Esc),
//! or its key (`controls_help`, `/`), lays the flight's controls over the view, grouped as SETTINGS groups them, with
//! their bound inputs; Esc closes it (back to the menu when it was opened there; Niklas, 18 Oct 2026: from the menu,
//! not a `?` ring on the HUD), its key again too. It lists what acts in this scene: in a voyage no test-range keys, in
//! the test range no voyage keys, in a replayed fight the fight viewer's keys instead of flying. The flight goes on
//! under it (the test range pauses only under the menu). On a touch screen it lists the touch controls instead of
//! the keys (GDR-0120).
//!
//! ```text
//!                                  CONTROLS
//!   FLIGHT                 MISSIONS                 CAMERA AND VIEWS        DEBUG AND DISPLAY
//!   TURN LEFT     A / LEFT RECALL                R  ZOOM IN    = / NUM +   HIDE THE HUD    H
//!   …                      …                        …                       …
//!                 CLICK A SHIP LOCK · MIDDLE DRAG PAN · WHEEL ZOOM
//! ```

use bevy::prelude::*;
use data::input::{Action, Context, Section};
use kit::style::{Style, ui_color};
use kit::ui_kit::{Edge, Hot, HudFonts, ScreenFx, ScreenPart, frame, label, panel_colors, panel_node, panel_title, set_text, show_screen};

use super::{FlightMenu, HudButton, HudZone};
use crate::mode::FlightUi;
use crate::scene::{Scene, SceneKind};
use crate::settings::Controls;
use crate::sim_bridge::{GameContent, PilotFocus};

#[derive(Component)]
pub(super) struct HelpRoot;

/// A section's column (hidden when none of its actions acts in the scene).
#[derive(Component)]
pub(super) struct HelpColumn(Section);

/// An action's row: its section and whether it acts in flight and in the fight viewer (its control's contexts).
#[derive(Component)]
pub(super) struct HelpRow {
    action: Action,
    section: Section,
    flight: bool,
    replay: bool,
}

/// The text with an action's inputs.
#[derive(Component)]
pub(super) struct HelpKeys(Action);

/// The mouse line under the columns.
#[derive(Component)]
pub(super) struct HelpMouse;

/// The overlay: a dark plate in the middle of the view with a column per section (a [`HudButton`]: clicks on it never
/// reach the world).
pub(super) fn spawn_overlay(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, clear: Res<ClearColor>, content: Res<GameContent>) {
    let ui = &style.ui;
    let (grey, white, dim, teal) = (ui_color(ui.label, 1.0), ui_color(ui.number, 1.0), ui_color(ui.dim, 1.0), ui_color(ui.teal, 1.0));
    commands
        .spawn((
            HelpRoot,
            FlightUi,
            ScreenFx::closed(),
            Visibility::Hidden,
            GlobalZIndex(11),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        ))
        .with_children(|root| {
            root.spawn((ScreenPart::new(Edge::Fade, 0), HudButton, HudZone, Hot::default(), panel_colors(clear.0, ui.teal), Node { max_width: percent(94), ..panel_node() }))
            .with_children(|p| {
                p.spawn(panel_title(&fonts, "CONTROLS", white));
                p.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::Center, column_gap: px(34), row_gap: px(14), ..default() }).with_children(|cols| {
                    for section in [Section::Flight, Section::Replay, Section::Mission, Section::Camera, Section::Debug] {
                        cols.spawn((HelpColumn(section), Node { flex_direction: FlexDirection::Column, row_gap: px(5), min_width: px(200), ..default() })).with_children(|col| {
                            col.spawn((label(&fonts, &section.title().to_uppercase(), frame().hud.sub, teal), Node { margin: UiRect::bottom(px(3)), ..default() }));
                            for c in content.0.controls.iter().filter(|c| c.section == section) {
                                let Some(action) = Action::from_id(&c.id) else { continue };
                                let has = |x: Context| c.contexts.contains(&x) || c.contexts.contains(&Context::Any);
                                let row = HelpRow { action, section, flight: has(Context::Flight), replay: has(Context::Replay) };
                                col.spawn((row, Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween, column_gap: px(16), ..default() })).with_children(|row| {
                                    row.spawn(label(&fonts, &c.name.to_uppercase(), 9.5, grey));
                                    row.spawn((HelpKeys(action), label(&fonts, "", 9.5, white)));
                                });
                            }
                        });
                    }
                });
                p.spawn((HelpMouse, label(&fonts, "", 9.5, dim), TextLayout::justify(bevy::text::Justify::Center), kit::ui_kit::CompactSize(11.0)));
            });
        });
}

/// Does a row act in this scene? Flight's keys where someone flies (a fight's camera and debug keys too); the fight
/// viewer's only in a replay; a voyage's and the test range's each only there.
fn acts(row: &HelpRow, kind: &SceneKind, piloted: bool) -> bool {
    let a = row.action;
    let voyage_only = matches!(a, Action::Recall | Action::ClusterMap | Action::Minimap);
    let range_only = matches!(a, Action::TestReturn | Action::Waves | Action::ResetScene | Action::Pause);
    let flying = row.section != Section::Flight || piloted;
    match kind {
        SceneKind::Fight => row.replay || (row.flight && matches!(row.section, Section::Camera | Section::Debug)) || a == Action::Leave,
        SceneKind::Voyage(_) => row.flight && !range_only && flying,
        SceneKind::Sandbox => row.flight && !voyage_only && flying,
        _ => row.flight && matches!(row.section, Section::Camera | Section::Debug) && !range_only,
    }
}

/// The mouse line for the scene.
fn mouse_line(kind: &SceneKind, piloted: bool) -> &'static str {
    match kind {
        SceneKind::Fight => "CLICK A SHIP FOLLOW  ·  DRAG PAN  ·  WHEEL ZOOM",
        SceneKind::Voyage(_) if piloted => "CLICK A SHIP LOCK  ·  CLICK THE MINIMAP CLUSTER MAP  ·  MIDDLE DRAG PAN  ·  WHEEL ZOOM",
        _ if piloted => "CLICK A SHIP LOCK  ·  MIDDLE DRAG PAN  ·  WHEEL ZOOM",
        _ => "DRAG PAN  ·  WHEEL ZOOM",
    }
}

/// The touch controls for the scene (a touch screen's overlay, in place of the key columns).
fn touch_lines(kind: &SceneKind, piloted: bool) -> &'static str {
    match kind {
        SceneKind::Fight => "TAP A SHIP  FOLLOW  ·  TAP EMPTY SPACE  FRAME THE FIGHT\nDRAG  PAN  ·  PINCH  ZOOM\nTHE BUTTONS BY THE TIMELINE  PLAY, SEEK, SPEED  ·  PRESS THE TIMELINE  SCRUB",
        SceneKind::Voyage(_) if piloted => "STICK (LEFT THUMB)  FLY  ·  FIRE  HOLD  ·  TAP A SHIP  LOCK  ·  TAP THE TARGET  NEXT\nTWO FINGERS  ZOOM AND PAN  ·  TAP THE MINIMAP  CLUSTER MAP  ·  RECALL  HOME",
        _ if piloted => "STICK (LEFT THUMB)  FLY  ·  FIRE  HOLD  ·  TAP A SHIP  LOCK  ·  TAP THE TARGET  NEXT\nTWO FINGERS  ZOOM AND PAN  ·  THE RING TOP LEFT  THIS MENU",
        _ => "DRAG  PAN  ·  PINCH  ZOOM",
    }
}

/// Opening and closing, and what the overlay lists while it is open.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn update(
    menu: Res<FlightMenu>,
    layer: Res<crate::settings::SettingsLayer>,
    scene: Res<Scene>,
    focus: Res<PilotFocus>,
    controls: Res<Controls>,
    mut root: Single<&mut ScreenFx, With<HelpRoot>>,
    mut rows: Query<(&HelpRow, &mut Node), Without<HelpColumn>>,
    mut columns: Query<(&HelpColumn, &mut Node), Without<HelpRow>>,
    mut keys: Query<(&HelpKeys, &mut Text), Without<HelpMouse>>,
    mut mouse: Single<&mut Text, With<HelpMouse>>,
    touch: Res<kit::ui_kit::TouchScreen>,
) {
    show_screen(&mut root, menu.help && !layer.open);
    if !menu.help {
        return;
    }
    let piloted = focus.0.is_some();
    let mut shown = [false; Section::ALL.len()];
    for (row, mut node) in &mut rows {
        let on = acts(row, &scene.kind, piloted);
        shown[row.section as usize] |= on;
        let d = if on { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
    for (col, mut node) in &mut columns {
        let d = if shown[col.0 as usize] && !touch.on() { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
    for (k, mut text) in &mut keys {
        set_text(&mut text, &controls.label(k.0));
    }
    set_text(&mut mouse, if touch.on() { touch_lines(&scene.kind, piloted) } else { mouse_line(&scene.kind, piloted) });
}
