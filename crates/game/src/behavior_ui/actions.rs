//! What the behavior screen's buttons and keys mean. Both send one message, [`BehaviorAction`]: a click on a
//! node with a [`BehaviorButton`] sends its action, the keys send the same ones. Handlers read the message and
//! never touch widget code; this module handles the UI state (view, selection, test mode) and the way back.
//! Editing (`edit.rs`) handles the editing group (`AddRule`, `AddNode`, `Edit`, `Undo`, ...), `saving.rs` SAVE and
//! `TestInRange`, `list.rs` the BEHAVIORS list; the ring menu has no handler yet (as in the drydock).
//!
//! Keys (only while no text field or the text view has the keyboard, and the BEHAVIORS list is closed): Esc
//! cancels REPLACE, else back to the drydock; T test in range; / search the palette; Ctrl+Z undo, Ctrl+Shift+Z or
//! Ctrl+Y redo; Delete or Backspace removes the selected condition, rule or target line. Ctrl+S saves, also in the
//! text view. With the list open, the list handles its keys (Esc closes it).

use bevy::prelude::*;
use bevy_egui::EguiContexts;
use data::behavior_edit::{NodeRef, RuleId};

use crate::drydock::actions::DrydockAction;
use crate::mode::AppMode;
use crate::ui_kit::{Pill, Ring, TextField, UiClick, UiPointer};

use super::edit::{Edit, EditState};
use super::inspect::Selection;
use super::list::BehaviorList;
use super::screen::SearchField;
use super::{BehaviorDraft, BehaviorUi, BehaviorView, TestMode};

#[derive(Message, Clone, Debug, PartialEq)]
pub enum BehaviorAction {
    /// Back to the drydock (Esc, the DRYDOCK pill); the design is untouched.
    Back,
    /// The ring button top left (the ring menu).
    Menu,
    /// GRAPH or TEXT.
    View(BehaviorView),
    /// A click on a node of the graph: the inspector shows it.
    Select(NodeRef),
    /// Nothing selected: the inspector shows the behavior.
    Deselect,
    /// WATCH or FLY ALONGSIDE, the mode TEST IN RANGE starts in.
    TestMode(TestMode),
    /// TEST IN RANGE (T) in [`BehaviorUi::test_mode`]: the range scenes start it.
    TestInRange,

    // Editing (`edit.rs`), through the editing model.
    /// `+ RULE`: `always ->` the palette's first action, above a trailing `always` rule.
    AddRule,
    /// A palette entry (content id): added to the selected rule, or replacing (see `edit::palette_click`); with
    /// nothing to add it to, the inspector shows it.
    AddNode(String),
    /// A rule's row or its `always` chip: the inspector shows the rule.
    SelectRule(RuleId),
    /// An inspector control's edit.
    Edit(Edit),
    /// Arms REPLACE on the selected condition (the next palette condition takes its place), or disarms it.
    Replace,
    /// Delete / Backspace: removes the selected condition, deletes the selected rule, clears the target line.
    Delete,
    Undo,
    Redo,
    // Behaviors as saved documents (`saving.rs`, `list.rs`).
    /// SAVE (Ctrl+S): saves the draft (a template as the player's copy).
    Save,
    /// BEHAVIORS: opens or closes the list of the player's behaviors and the templates.
    Behaviors,
}

/// A widget that sends an action when clicked.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct BehaviorButton(pub BehaviorAction);

/// Clicks on action widgets (disabled pills and rings send nothing).
#[allow(clippy::type_complexity)]
pub(super) fn clicks(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(&BehaviorButton, Option<&Pill>, Option<&Ring>)>,
    mut out: MessageWriter<BehaviorAction>,
) {
    for c in clicks.read() {
        if let Ok((b, pill, ring)) = buttons.get(c.0)
            && !pill.is_some_and(|p| p.disabled)
            && !ring.is_some_and(|r| r.disabled)
        {
            out.write(b.0.clone());
        }
    }
}

/// The keys.
pub(super) fn shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<UiPointer>,
    edit: Res<EditState>,
    list: Res<BehaviorList>,
    mut contexts: EguiContexts,
    mut search: Query<&mut TextField, With<SearchField>>,
    mut out: MessageWriter<BehaviorAction>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
    // Ctrl+S also while the text view types (not in a one-line field, which would take the S).
    if ctrl && keys.just_pressed(KeyCode::KeyS) && !pointer.typing {
        out.write(BehaviorAction::Save);
    }
    // The text view's editor (egui) or a field has the keyboard, or the BEHAVIORS list handles its own keys.
    let egui_typing = contexts.ctx_mut().is_ok_and(|c| c.egui_wants_keyboard_input());
    if pointer.typing || egui_typing || list.is_open() {
        return;
    }
    // `/` gives the search field the keyboard; the other keys of that frame are typing, not shortcuts (keys of
    // one frame come in no order).
    if !ctrl && keys.any_just_pressed([KeyCode::Slash, KeyCode::NumpadDivide]) {
        for mut f in &mut search {
            f.focused = true;
        }
        return;
    }
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    for key in keys.get_just_pressed() {
        match (*key, ctrl) {
            (KeyCode::KeyZ, true) => {
                out.write(if shift { BehaviorAction::Redo } else { BehaviorAction::Undo });
            }
            (KeyCode::KeyY, true) => {
                out.write(BehaviorAction::Redo);
            }
            (_, true) => {}
            (KeyCode::Escape, _) => {
                out.write(if edit.replacing.is_some() { BehaviorAction::Replace } else { BehaviorAction::Back });
            }
            (KeyCode::Delete | KeyCode::Backspace, _) => {
                out.write(BehaviorAction::Delete);
            }
            (KeyCode::KeyT, _) => {
                out.write(BehaviorAction::TestInRange);
            }
            _ => {}
        }
    }
}

/// The UI-state actions and the way back.
pub(super) fn apply(
    mut actions: MessageReader<BehaviorAction>,
    mut ui: ResMut<BehaviorUi>,
    mut draft: ResMut<BehaviorDraft>,
    mut next: ResMut<NextState<AppMode>>,
) {
    for a in actions.read() {
        match a {
            BehaviorAction::Back => next.set(AppMode::Drydock),
            BehaviorAction::View(v) => {
                if ui.view == BehaviorView::Text && *v != BehaviorView::Text {
                    draft.tidy_text();
                }
                ui.view = *v;
            }
            BehaviorAction::Select(n) => ui.selected = Some(Selection::Node(*n)),
            BehaviorAction::Deselect => ui.selected = None,
            BehaviorAction::TestMode(m) => ui.test_mode = *m,
            // `edit::apply` (editing), `saving.rs` (Save, TestInRange), `list.rs` (Behaviors); the menu has no
            // handler yet.
            BehaviorAction::Menu
            | BehaviorAction::TestInRange
            | BehaviorAction::Save
            | BehaviorAction::Behaviors
            | BehaviorAction::AddRule
            | BehaviorAction::AddNode(_)
            | BehaviorAction::SelectRule(_)
            | BehaviorAction::Edit(_)
            | BehaviorAction::Replace
            | BehaviorAction::Delete
            | BehaviorAction::Undo
            | BehaviorAction::Redo => {}
        }
    }
}

/// The drydock's BEHAVIOR button opens the screen.
pub(super) fn open_from_drydock(mut actions: MessageReader<DrydockAction>, mut next: ResMut<NextState<AppMode>>) {
    if actions.read().any(|a| *a == DrydockAction::Behavior) {
        next.set(AppMode::Behavior);
    }
}
