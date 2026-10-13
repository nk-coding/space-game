//! What the behavior screen's buttons and keys mean. Both send one message, [`BehaviorAction`]: a click on a
//! node with a [`BehaviorButton`] sends its action, the keys send the same ones. Handlers read the message and
//! never touch widget code; this module handles the UI state (view, selection, test mode) and the way back.
//! The editing model handles `AddRule` and `AddNode`, the range scenes `TestInRange`; the ring menu has no
//! handler yet (as in the drydock).
//!
//! Keys (only while no text field or the text view has the keyboard): Esc back to the drydock, T test in
//! range, / search the palette.

use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::drydock::actions::DrydockAction;
use crate::mode::AppMode;
use crate::ui_kit::{Pill, TextField, UiClick, UiPointer};

use super::graph::NodeRef;
use super::inspect::Selection;
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
    /// `+ RULE`: the editing model adds a rule.
    AddRule,
    /// A palette entry (content id): the editing model adds the node. Until it does, the inspector shows it.
    AddNode(String),
}

/// A widget that sends an action when clicked.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct BehaviorButton(pub BehaviorAction);

/// Clicks on action widgets (disabled pills send nothing).
pub(super) fn clicks(mut clicks: MessageReader<UiClick>, buttons: Query<(&BehaviorButton, Option<&Pill>)>, mut out: MessageWriter<BehaviorAction>) {
    for c in clicks.read() {
        if let Ok((b, pill)) = buttons.get(c.0)
            && !pill.is_some_and(|p| p.disabled)
        {
            out.write(b.0.clone());
        }
    }
}

/// The keys.
pub(super) fn shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<UiPointer>,
    mut contexts: EguiContexts,
    mut search: Query<&mut TextField, With<SearchField>>,
    mut out: MessageWriter<BehaviorAction>,
) {
    // The text view's editor (egui) or the search field has the keyboard.
    let egui_typing = contexts.ctx_mut().is_ok_and(|c| c.egui_wants_keyboard_input());
    if pointer.typing || egui_typing {
        return;
    }
    for key in keys.get_just_pressed() {
        match key {
            KeyCode::Escape => {
                out.write(BehaviorAction::Back);
            }
            KeyCode::KeyT => {
                out.write(BehaviorAction::TestInRange);
            }
            KeyCode::Slash | KeyCode::NumpadDivide => {
                for mut f in &mut search {
                    f.focused = true;
                }
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
            BehaviorAction::AddNode(id) => ui.selected = Some(Selection::Vocabulary(id.clone())),
            // The editing model (AddRule), the range scenes (TestInRange); the menu has no handler yet.
            BehaviorAction::Menu | BehaviorAction::TestInRange | BehaviorAction::AddRule => {}
        }
    }
}

/// The drydock's BEHAVIOR button opens the screen.
pub(super) fn open_from_drydock(mut actions: MessageReader<DrydockAction>, mut next: ResMut<NextState<AppMode>>) {
    if actions.read().any(|a| *a == DrydockAction::Behavior) {
        next.set(AppMode::Behavior);
    }
}
