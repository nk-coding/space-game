//! What the drydock's buttons and keys mean. Both send one message, [`DrydockAction`]: a click on a node with an
//! [`ActionButton`] sends its action, the keyboard shortcuts send the same ones. Handlers read the message and
//! never touch widget code; this module handles the UI state (tool, material, overlays, category, picked part),
//! the test flight and the ring menu (home). Zoom is handled by the camera (`view.rs`), editing (undo / redo,
//! mirror, bevel, rotate, the selected part) by `tools.rs`, saving, export and the blueprint list by
//! `blueprints/`, the BEHAVIOR button by `behavior_ui`, a click on a locked part or armour by `research`.
//!
//! Keys (only while no text field has the keyboard; the action map's, GDR-0090, defaults named here): B paint,
//! Shift+B bevel, G fill, X erase, V select, R / Shift+R (or Shift+wheel down / up over the grid) and E / Q turn (the
//! selected part, else the brush), F flip, Delete / Backspace delete, M mirror, 1–9 armour material (unlocked ones),
//! Ctrl+Z undo, Ctrl+Shift+Z or Ctrl+Y redo, Ctrl+S save, T test flight, + / − zoom, / search; Esc (fixed) closes the
//! open menu, else deselects.

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use data::{Content, PowerTier, Unlock};
use kit::ui_kit::{Pill, TextField, UiClick, UiPointer, WHEEL_LINE_PX};
use data::input::Action;
use world::career::{Granted, unlocked_by};
use world::settings::Act;
use world::mode::AppMode;
use world::player::PlayerDesign;
use world::scene::{Launch, Scene};
use world::sim_bridge::GameContent;

use super::blueprints::Shelf;
use super::state::{BarMenu, DrydockStatus, DrydockUi, Overlay, Tool, armor_materials};

#[derive(Message, Clone, Debug, PartialEq)]
pub enum DrydockAction {
    Tool(Tool),
    RotateCcw,
    RotateCw,
    Mirror,
    Bevel,
    Undo,
    Redo,
    Save,
    Export,
    Blueprints,
    TestFly,
    /// BEHAVIOR: open the BEHAVIOR screen for the design (`behavior_ui` handles it).
    Behavior,
    /// The ring button top left: home (the shipyard's ring menu).
    Menu,
    /// Armour material for hull parts (content id).
    Material(String),
    Overlay(Overlay),
    /// A category chip: the palette's list jumps to the category's section (content id).
    Category(String),
    /// Pick a part in the palette (content id).
    PickPart(String),
    ZoomIn,
    ZoomOut,
    /// The selected part: flip it (F), delete it (Delete / Backspace), deselect (Esc).
    Flip,
    Delete,
    Deselect,
    /// The selected part's armour material, power tier, power weight (up or down a step) and weapon group: the
    /// inspector.
    PartMaterial(String),
    PartTier(PowerTier),
    PartWeight(bool),
    PartGroup(u8),
    /// A click on a LOCKED palette row or a locked armour swatch: the research screen on the node (id) that unlocks
    /// it (`research` handles it).
    Research(String),
    /// Opens a bottom bar menu, or closes it when it is open.
    OpenMenu(BarMenu),
    /// Closes the open menu (Esc, a click outside it).
    CloseMenu,
    /// One of the ⋯ menu's rarer actions (`blueprints/`).
    Rare(Rare),
}

/// The ⋯ menu's rarer actions (GDR-0082): shortcuts to what BLUEPRINTS and EXPORT offer, carried out the same way
/// (`blueprints/`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rare {
    /// An empty design in the frame of the one in the drydock (that one is saved first when it has changes).
    NewDesign,
    /// Moves the design to the content frame of this size.
    Frame((u32, u32)),
    /// Copies the design's share code.
    CopyCode,
    /// Downloads the design's `.ron` file (desktop: writes it to the data directory).
    Download,
    /// Imports a `.ron` file (web: the file chooser).
    ImportFile,
}

/// A widget that sends an action when clicked (pills, rings, palette rows).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ActionButton(pub DrydockAction);

/// The search field (`/` focuses it).
#[derive(Component)]
pub struct SearchField;

/// Clicks on action widgets. A disabled pill sends nothing, except a locked part's row or a locked armour pill:
/// that opens the research screen on the node that unlocks it.
pub(super) fn clicks(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(&ActionButton, Option<&Pill>)>,
    content: Res<GameContent>,
    mut out: MessageWriter<DrydockAction>,
) {
    for c in clicks.read() {
        let Ok((b, pill)) = buttons.get(c.0) else { continue };
        if !pill.is_some_and(|p| p.disabled) {
            out.write(b.0.clone());
        } else if let Some(node) = locked_research(&content.0, &b.0) {
            out.write(DrydockAction::Research(node));
        }
    }
}

/// The research node that unlocks what a disabled widget picks (a locked part or armour material), if any.
pub fn locked_research(content: &Content, action: &DrydockAction) -> Option<String> {
    let unlock = match action {
        DrydockAction::PickPart(p) if !p.is_empty() => Unlock::Part(p.clone()),
        DrydockAction::Material(m) | DrydockAction::PartMaterial(m) => Unlock::Material(m.clone()),
        _ => return None,
    };
    unlocked_by(content, &unlock).map(|n| n.id.clone())
}

/// The keyboard shortcuts, and Shift+wheel over the grid (turns, like R / Shift+R; the plain wheel zooms). While
/// BLUEPRINTS or EXPORT covers the design only Ctrl+S acts (the pages handle Esc, Ctrl+C and Ctrl+V themselves): a
/// key must not edit, select or fly a design nobody sees.
#[allow(clippy::too_many_arguments)]
pub(super) fn shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    act: Act,
    scroll: Option<Res<AccumulatedMouseScroll>>,
    pointer: Res<UiPointer>,
    content: Res<GameContent>,
    shelf: Res<Shelf>,
    ui: Res<DrydockUi>,
    mut search: Query<&mut TextField, With<SearchField>>,
    mut wheel: Local<f32>,
    mut out: MessageWriter<DrydockAction>,
) {
    if pointer.typing {
        return;
    }
    let covered = shelf.is_open();
    let mods = act.mods();
    let mut send = |a: DrydockAction| {
        out.write(a);
    };
    let over_grid = mods.shift && !mods.ctrl && !covered && !pointer.blocked;
    if let Some(forward) = wheel_turn(&mut wheel, scroll.as_deref().filter(|_| over_grid)) {
        send(if forward { DrydockAction::RotateCw } else { DrydockAction::RotateCcw });
    }
    // Search gives the search field the keyboard; the other keys of that frame are typing, not shortcuts (keys of
    // one frame come in no order).
    if !covered && act.just_pressed(Action::Search) {
        for mut f in &mut search {
            f.focus();
        }
        return;
    }
    if act.just_pressed(Action::Save) {
        send(DrydockAction::Save);
    }
    if covered {
        return;
    }
    // Esc (fixed): closes the open menu, else deselects.
    if keys.just_pressed(KeyCode::Escape) {
        send(if ui.menu.is_some() { DrydockAction::CloseMenu } else { DrydockAction::Deselect });
    }
    let pairs = [
        (Action::Undo, DrydockAction::Undo),
        (Action::Redo, DrydockAction::Redo),
        (Action::ToolBevel, DrydockAction::Bevel),
        (Action::ToolPaint, DrydockAction::Tool(Tool::Paint)),
        (Action::ToolFill, DrydockAction::Tool(Tool::Fill)),
        (Action::ToolErase, DrydockAction::Tool(Tool::Erase)),
        (Action::ToolSelect, DrydockAction::Tool(Tool::Select)),
        (Action::RotateLeft, DrydockAction::RotateCcw),
        (Action::RotateRight, DrydockAction::RotateCw),
        (Action::Mirror, DrydockAction::Mirror),
        (Action::Flip, DrydockAction::Flip),
        (Action::Delete, DrydockAction::Delete),
        (Action::TestFlight, DrydockAction::TestFly),
        (Action::ZoomIn, DrydockAction::ZoomIn),
        (Action::ZoomOut, DrydockAction::ZoomOut),
    ];
    for (a, action) in pairs {
        if act.just_pressed(a) {
            send(action);
        }
    }
    for (n, m) in armor_materials(&content.0).take(9).enumerate() {
        if Action::armour(n + 1).is_some_and(|a| act.just_pressed(a)) {
            send(DrydockAction::Material(m.id.clone()));
        }
    }
}

/// Shift+wheel turns: `Some(true)` for the wheel turned down (the next orientation, like R), `Some(false)` up (like
/// Shift+R), at most one turn a frame. Browsers often send Shift+wheel as a sideways scroll, so the sideways delta
/// counts when there is no vertical one (both negative for down / right). Touchpad pixels add up in `acc` to a turn
/// every [`WHEEL_LINE_PX`]; `None` (no Shift, or not over the grid) drops what was added up.
pub fn wheel_turn(acc: &mut f32, scroll: Option<&AccumulatedMouseScroll>) -> Option<bool> {
    let Some(scroll) = scroll else {
        *acc = 0.0;
        return None;
    };
    let d = if scroll.delta.y != 0.0 { scroll.delta.y } else { scroll.delta.x };
    if d == 0.0 {
        return None;
    }
    *acc += match scroll.unit {
        MouseScrollUnit::Line => d.signum(),
        MouseScrollUnit::Pixel => d / WHEEL_LINE_PX,
    };
    if acc.abs() < 1.0 {
        return None;
    }
    let forward = *acc < 0.0;
    *acc = 0.0;
    Some(forward)
}

/// The UI-state actions, the menus, the test flight (only a design that can fly takes off) and the way home.
/// Locked parts and materials are never picked (their rows and swatches are disabled; the keys 1–9 reach locked ones
/// too). Any action but opening a menu closes the open menu (its own items' actions too).
pub(super) fn apply(
    mut actions: MessageReader<DrydockAction>,
    mut ui: ResMut<DrydockUi>,
    status: Res<DrydockStatus>,
    granted: Res<Granted>,
    design: Res<PlayerDesign>,
    mut scene: ResMut<Scene>,
    mut next: ResMut<NextState<AppMode>>,
) {
    for a in actions.read() {
        if ui.menu.is_some() && !matches!(a, DrydockAction::OpenMenu(_)) {
            ui.menu = None;
        }
        match a {
            DrydockAction::OpenMenu(m) => ui.menu = if ui.menu == Some(*m) { None } else { Some(*m) },
            DrydockAction::Tool(t) => ui.tool = *t,
            DrydockAction::Material(m) if granted.unlocks.has_material(m) => ui.material.clone_from(m),
            DrydockAction::Overlay(o) => ui.toggle_overlay(*o),
            DrydockAction::Category(c) => ui.jump = Some(c.clone()),
            // A picked slope orients itself to the hull again.
            DrydockAction::PickPart(p) if granted.unlocks.has_part(p) => {
                ui.picked = Some(p.clone());
                ui.slope_turn = None;
            }
            // Only on a status made for the design as it is now (one changed on the BEHAVIOR screen gets its status
            // in the first drydock frame).
            DrydockAction::TestFly if status.grounded.is_none() && status.revision == Some(design.revision()) => {
                scene.launch = Some(Launch::TestFlight);
                next.set(AppMode::Flight);
            }
            DrydockAction::Menu => next.set(AppMode::Home),
            // Editing (tools.rs), zoom (the camera), save / export / blueprints and the ⋯ menu's actions
            // (blueprints/), the BEHAVIOR screen (`behavior_ui`).
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;
    use data::Career;

    use super::*;
    use world::player::PlayerDesign;
    use crate::drydock::panels::sync_pills;
    use world::scene::SceneKind;

    /// A drydock with a new game's unlocks: a locked part's row and a locked armour pill, and the handlers.
    fn app() -> App {
        let content = data::base_content();
        let career = Career::new(&content, "rim_awakening", 1).unwrap();
        let granted = Granted { unlocks: career.unlocks(&content), mind_capacity: 2, ..Default::default() };
        let design = PlayerDesign::starter(&content, "rim_skiff");
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_state(AppMode::Drydock)
            .insert_resource(Scene::new(SceneKind::Sandbox, 1))
            .insert_resource(DrydockUi::new(&content, &design, &granted.unlocks))
            .insert_resource(design)
            .insert_resource(granted)
            .init_resource::<DrydockStatus>()
            .init_resource::<crate::drydock::panels::Palette>()
            .insert_resource(GameContent(content))
            .add_message::<DrydockAction>()
            .add_message::<UiClick>()
            .add_systems(Update, (clicks, apply, sync_pills).chain());
        app
    }

    /// Locked rows and pills are disabled, so clicks on them pick nothing; a locked part or material sent anyway
    /// (keys 1–9) is not taken. Unlocked ones are.
    #[test]
    fn locked_parts_and_materials_cannot_be_picked() {
        let mut app = app();
        let beam = app.world_mut().spawn((ActionButton(DrydockAction::PickPart("beam_laser".into())), Pill::new(kit::ui_kit::Tone::Teal), Node::default())).id();
        let composite = app.world_mut().spawn((ActionButton(DrydockAction::Material("composite".into())), Pill::new(kit::ui_kit::Tone::Teal), Node::default())).id();
        let pulse = app.world_mut().spawn((ActionButton(DrydockAction::PickPart("pulse_laser".into())), Pill::new(kit::ui_kit::Tone::Teal), Node::default())).id();
        app.update();
        let pill = |app: &App, e: Entity| *app.world().get::<Pill>(e).unwrap();
        assert!(pill(&app, beam).disabled && pill(&app, composite).disabled && !pill(&app, pulse).disabled);
        let before = app.world().resource::<DrydockUi>().clone();
        app.world_mut().write_message(UiClick(beam));
        app.world_mut().write_message(UiClick(composite));
        app.update();
        app.world_mut().write_message(DrydockAction::PickPart("beam_laser".into()));
        app.world_mut().write_message(DrydockAction::Material("composite".into()));
        app.update();
        let ui = app.world().resource::<DrydockUi>();
        assert_eq!((&ui.picked, &ui.material), (&before.picked, &before.material));
        assert_eq!(ui.material, "steel");
        app.world_mut().write_message(UiClick(pulse));
        app.update();
        assert_eq!(app.world().resource::<DrydockUi>().picked.as_deref(), Some("pulse_laser"));
        assert!(pill(&app, pulse).active);
    }

    /// Shift+wheel down turns forward (like R), up back; a sideways scroll (what browsers make of Shift+wheel) counts
    /// the same; touchpad pixels add up to a turn; letting go of Shift drops the sum.
    #[test]
    fn shift_wheel_turns_one_step_a_notch() {
        let wheel = |x: f32, y: f32, unit| AccumulatedMouseScroll { unit, delta: Vec2::new(x, y) };
        let mut acc = 0.0;
        assert_eq!(wheel_turn(&mut acc, Some(&wheel(0.0, -3.0, MouseScrollUnit::Line))), Some(true));
        assert_eq!(wheel_turn(&mut acc, Some(&wheel(0.0, 1.0, MouseScrollUnit::Line))), Some(false));
        assert_eq!(wheel_turn(&mut acc, Some(&wheel(-100.0, 0.0, MouseScrollUnit::Pixel))), Some(true));
        assert_eq!(wheel_turn(&mut acc, Some(&wheel(0.0, 0.0, MouseScrollUnit::Line))), None);
        assert_eq!(wheel_turn(&mut acc, Some(&wheel(0.0, 25.0, MouseScrollUnit::Pixel))), None);
        assert_eq!(wheel_turn(&mut acc, Some(&wheel(0.0, 25.0, MouseScrollUnit::Pixel))), Some(false));
        assert_eq!(wheel_turn(&mut acc, Some(&wheel(0.0, 25.0, MouseScrollUnit::Pixel))), None);
        assert_eq!(wheel_turn(&mut acc, None), None);
        assert_eq!(wheel_turn(&mut acc, Some(&wheel(0.0, 25.0, MouseScrollUnit::Pixel))), None, "the sum was dropped");
    }

    /// A click on a locked part's row or a locked armour pill asks for the research screen on the node that
    /// unlocks it; an unlocked one doesn't.
    #[test]
    fn locked_rows_lead_to_their_research() {
        let mut app = app();
        let beam = app.world_mut().spawn((ActionButton(DrydockAction::PickPart("beam_laser".into())), Pill::new(kit::ui_kit::Tone::Teal), Node::default())).id();
        let composite = app.world_mut().spawn((ActionButton(DrydockAction::Material("composite".into())), Pill::new(kit::ui_kit::Tone::Teal), Node::default())).id();
        let pulse = app.world_mut().spawn((ActionButton(DrydockAction::PickPart("pulse_laser".into())), Pill::new(kit::ui_kit::Tone::Teal), Node::default())).id();
        app.update();
        for e in [beam, composite, pulse] {
            app.world_mut().write_message(UiClick(e));
        }
        app.update();
        let sent: Vec<DrydockAction> = app.world_mut().resource_mut::<Messages<DrydockAction>>().drain().collect();
        let research: Vec<&DrydockAction> = sent.iter().filter(|a| matches!(a, DrydockAction::Research(_))).collect();
        assert_eq!(research, [&DrydockAction::Research("beam_laser".into()), &DrydockAction::Research("composite_plating".into())]);
        let content = data::base_content();
        assert_eq!(locked_research(&content, &DrydockAction::PickPart(String::new())), None);
        assert_eq!(locked_research(&content, &DrydockAction::Material("reactive".into())), None, "no research unlocks it");
    }
}
