//! What the drydock's buttons and keys mean. Both send one message, [`DrydockAction`]: a click on a node with an
//! [`ActionButton`] sends its action, the keyboard shortcuts send the same ones. Handlers read the message and
//! never touch widget code; this module handles the UI state (tool, material, overlays, category, picked part)
//! and the test flight. Zoom is handled by the camera (`view.rs`), editing (undo / redo, mirror, bevel, rotate,
//! the selected part) by `tools.rs`; saving, export, the blueprint list and the ring menu come with the RON
//! writer and the menus.
//!
//! Keys (only while no text field has the keyboard): B paint, Shift+B bevel, G fill, X erase, V select, Q / E
//! rotate, F flip, Delete / Backspace delete, Esc deselect, M mirror, 1–9 armour material (unlocked ones), Ctrl+Z
//! undo, Ctrl+Shift+Z or Ctrl+Y redo, Ctrl+S save, T test flight, + / − zoom, / search.

use bevy::prelude::*;
use data::PowerTier;

use crate::mode::AppMode;
use crate::scene::Scene;
use crate::sim_bridge::GameContent;
use crate::ui_kit::{Pill, TextField, UiClick, UiPointer};

use super::PlayerDesign;
use super::blueprints::Shelf;
use super::state::{DrydockStatus, DrydockUi, Overlay, Tool, armor_materials};

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
    /// The ring button top left (the shipyard's ring menu).
    Menu,
    /// Armour material for hull parts (content id).
    Material(String),
    Overlay(Overlay),
    /// Palette category (content id).
    Category(String),
    /// Pick a part in the palette (content id).
    PickPart(String),
    ZoomIn,
    ZoomOut,
    /// The selected part: flip it (F), delete it (Delete / Backspace), deselect (Esc).
    Flip,
    Delete,
    Deselect,
    /// The selected part's armour material, power tier and power weight (up or down a step): the inspector.
    PartMaterial(String),
    PartTier(PowerTier),
    PartWeight(bool),
}

/// A widget that sends an action when clicked (pills, rings, palette rows).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ActionButton(pub DrydockAction);

/// The search field (`/` focuses it).
#[derive(Component)]
pub struct SearchField;

/// Clicks on action widgets (disabled pills send nothing).
pub(super) fn clicks(mut clicks: MessageReader<UiClick>, buttons: Query<(&ActionButton, Option<&Pill>)>, mut out: MessageWriter<DrydockAction>) {
    for c in clicks.read() {
        if let Ok((b, pill)) = buttons.get(c.0)
            && !pill.is_some_and(|p| p.disabled)
        {
            out.write(b.0.clone());
        }
    }
}

/// The keyboard shortcuts. While BLUEPRINTS or EXPORT covers the design only Ctrl+S acts (the pages handle Esc,
/// Ctrl+C and Ctrl+V themselves): a key must not edit, select or fly a design nobody sees.
pub(super) fn shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<UiPointer>,
    content: Res<GameContent>,
    shelf: Res<Shelf>,
    mut search: Query<&mut TextField, With<SearchField>>,
    mut out: MessageWriter<DrydockAction>,
) {
    if pointer.typing {
        return;
    }
    let covered = shelf.is_open();
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
    let mut send = |a: DrydockAction| {
        out.write(a);
    };
    for key in keys.get_just_pressed() {
        if covered && !(ctrl && *key == KeyCode::KeyS) {
            continue;
        }
        match (*key, ctrl, shift) {
            (KeyCode::KeyZ, true, false) => send(DrydockAction::Undo),
            (KeyCode::KeyZ, true, true) | (KeyCode::KeyY, true, _) => send(DrydockAction::Redo),
            (KeyCode::KeyS, true, _) => send(DrydockAction::Save),
            (_, true, _) => {}
            (KeyCode::KeyB, _, true) => send(DrydockAction::Bevel),
            (KeyCode::KeyB, _, false) => send(DrydockAction::Tool(Tool::Paint)),
            (KeyCode::KeyG, ..) => send(DrydockAction::Tool(Tool::Fill)),
            (KeyCode::KeyX, ..) => send(DrydockAction::Tool(Tool::Erase)),
            (KeyCode::KeyV, ..) => send(DrydockAction::Tool(Tool::Select)),
            (KeyCode::KeyQ, ..) => send(DrydockAction::RotateCcw),
            (KeyCode::KeyE, ..) => send(DrydockAction::RotateCw),
            (KeyCode::KeyM, ..) => send(DrydockAction::Mirror),
            (KeyCode::KeyF, ..) => send(DrydockAction::Flip),
            (KeyCode::Delete | KeyCode::Backspace, ..) => send(DrydockAction::Delete),
            (KeyCode::Escape, ..) => send(DrydockAction::Deselect),
            (KeyCode::KeyT, ..) => send(DrydockAction::TestFly),
            (KeyCode::Equal | KeyCode::NumpadAdd, ..) => send(DrydockAction::ZoomIn),
            (KeyCode::Minus | KeyCode::NumpadSubtract, ..) => send(DrydockAction::ZoomOut),
            (KeyCode::Slash | KeyCode::NumpadDivide, ..) => {
                for mut f in &mut search {
                    f.focused = true;
                }
            }
            (k, ..) => {
                if let Some(n) = digit(k)
                    && let Some(m) = armor_materials(&content.0).nth(n)
                {
                    send(DrydockAction::Material(m.id.clone()));
                }
            }
        }
    }
}

/// 0-based index of the digit keys 1–9.
fn digit(k: KeyCode) -> Option<usize> {
    const KEYS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    KEYS.iter().position(|&d| d == k)
}

/// The UI-state actions, and the test flight (only a design that can fly takes off).
pub(super) fn apply(
    mut actions: MessageReader<DrydockAction>,
    mut ui: ResMut<DrydockUi>,
    status: Res<DrydockStatus>,
    design: Res<PlayerDesign>,
    mut scene: ResMut<Scene>,
    mut next: ResMut<NextState<AppMode>>,
) {
    for a in actions.read() {
        match a {
            DrydockAction::Tool(t) => ui.tool = *t,
            DrydockAction::Material(m) => ui.material.clone_from(m),
            DrydockAction::Overlay(o) => ui.toggle_overlay(*o),
            DrydockAction::Category(c) => {
                ui.category.clone_from(c);
                ui.search.clear();
            }
            DrydockAction::PickPart(p) => ui.picked = Some(p.clone()),
            // Only on a status made for the design as it is now (one changed on the BEHAVIOR screen gets its status
            // in the first drydock frame).
            DrydockAction::TestFly if status.grounded.is_none() && status.revision == Some(design.revision()) => {
                scene.launch = true;
                next.set(AppMode::Flight);
            }
            // Editing (tools.rs), zoom (the camera), save / export / blueprints (blueprints.rs), the BEHAVIOR screen
            // (`behavior_ui`); the menu has no handler yet.
            _ => {}
        }
    }
}
