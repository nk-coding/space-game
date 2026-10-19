//! What the behavior screen's buttons and keys mean. Both send one message, [`BehaviorAction`]: a click on a
//! node with a [`BehaviorButton`] sends its action, the keys send the same ones. Handlers read the message and
//! never touch widget code; this module handles the UI state (view, selection, test mode), the way back and the
//! ring menu (home). Editing (`edit.rs`) handles the editing group (`AddRule`, `AddNode`, `Edit`, `Undo`, ...),
//! `saving.rs` SAVE and `TestInRange`, `list.rs` the BEHAVIORS list.
//!
//! Keys (only while no text field or the text view has the keyboard, and the BEHAVIORS list is closed): Esc
//! cancels REPLACE, else back the way the screen was opened (home or the drydock); T test in range; / search the palette; Ctrl+Z undo, Ctrl+Shift+Z or
//! Ctrl+Y redo; Delete or Backspace removes the selected condition, rule or target line. Ctrl+S saves, also in the
//! text view. With the list open, the list handles its keys (Esc closes it).

use bevy::prelude::*;
use data::input::Action;
use world::settings::Act;
use bevy_egui::EguiContexts;
use data::behavior::edit::{NodeRef, RuleId};
use kit::ui_kit::{LayoutClass, Pill, Ring, TextField, UiClick, UiPointer};
use world::mode::AppMode;

use crate::drydock::actions::DrydockAction;

use super::edit::{Edit, EditState};
use super::inspect::Selection;
use super::list::BehaviorList;
use super::screen::SearchField;
use super::touch::SwallowClick;
use super::{BehaviorDraft, BehaviorTab, BehaviorUi, BehaviorView, TestMode};

#[derive(Message, Clone, Debug, PartialEq)]
pub enum BehaviorAction {
    /// Back to the drydock (Esc, the DRYDOCK pill); the design is untouched.
    Back,
    /// The ring button top left: home (the ring menu).
    Menu,
    /// BEHAVIOR or PILOT ASSISTS.
    Tab(BehaviorTab),
    /// GRAPH or TEXT.
    View(BehaviorView),
    /// A click on a node of the graph: the inspector shows it.
    Select(NodeRef),
    /// Nothing selected: the inspector shows the behavior.
    Deselect,
    /// WATCH or FLY ALONGSIDE, the mode TEST IN RANGE starts in.
    TestMode(TestMode),
    /// TEST IN RANGE (T) in [`BehaviorUi::test_mode`] (`saving.rs` starts the test).
    TestInRange,
    /// The phone's caret ring beside TEST IN RANGE: opens or closes the popover of its modes.
    Modes,

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
    mut swallow: ResMut<SwallowClick>,
    mut out: MessageWriter<BehaviorAction>,
) {
    // A finger lifted after panning the graph or dragging a rule clicks nothing.
    if std::mem::take(&mut swallow.0) {
        clicks.clear();
        return;
    }
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
#[allow(clippy::too_many_arguments)]
pub(super) fn shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    act: Act,
    pointer: Res<UiPointer>,
    edit: Res<EditState>,
    ui: Res<BehaviorUi>,
    list: Res<BehaviorList>,
    class: Res<LayoutClass>,
    mut contexts: EguiContexts,
    mut search: Query<&mut TextField, With<SearchField>>,
    mut out: MessageWriter<BehaviorAction>,
) {
    // Save also while the text view types (not in a one-line field, which would take its key).
    if act.just_pressed(Action::Save) && !pointer.typing {
        out.write(BehaviorAction::Save);
    }
    // The text view's editor (egui) or a field has the keyboard, or the BEHAVIORS list handles its own keys.
    let egui_typing = contexts.ctx_mut().is_ok_and(|c| c.egui_wants_keyboard_input());
    if pointer.typing || egui_typing || list.is_open() {
        return;
    }
    // Search gives the search field the keyboard; the other keys of that frame are typing, not shortcuts (keys of
    // one frame come in no order).
    // (A phone's layout has no search field.)
    if act.just_pressed(Action::Search) && !class.is_compact() {
        for mut f in &mut search {
            f.focus();
        }
        return;
    }
    // Esc (fixed): ends REPLACE, else goes back (home or the drydock).
    if keys.just_pressed(KeyCode::Escape) && !act.ctrl() {
        out.write(match (edit.replacing.is_some(), ui.from_home) {
            (true, _) => BehaviorAction::Replace,
            (false, true) => BehaviorAction::Menu,
            (false, false) => BehaviorAction::Back,
        });
    }
    for (a, action) in [(Action::Undo, BehaviorAction::Undo), (Action::Redo, BehaviorAction::Redo), (Action::Delete, BehaviorAction::Delete), (Action::TestFlight, BehaviorAction::TestInRange)] {
        if act.just_pressed(a) {
            out.write(action);
        }
    }
}

/// The UI-state actions, the way back and the way home.
pub(super) fn apply(
    mut actions: MessageReader<BehaviorAction>,
    mut ui: ResMut<BehaviorUi>,
    mut draft: ResMut<BehaviorDraft>,
    mut next: ResMut<NextState<AppMode>>,
) {
    for a in actions.read() {
        // Anything else closes the modes' popover.
        if ui.modes_open && !matches!(a, BehaviorAction::Modes | BehaviorAction::TestMode(_)) {
            ui.modes_open = false;
        }
        match a {
            BehaviorAction::Back => next.set(AppMode::Drydock),
            BehaviorAction::Menu => next.set(AppMode::Home),
            BehaviorAction::View(v) => {
                if ui.view == BehaviorView::Text && *v != BehaviorView::Text {
                    draft.tidy_text();
                }
                ui.view = *v;
            }
            // The other list comes in (`assists::switch`); the one going out keeps its text as typed only if it has
            // errors, as on leaving the TEXT view.
            BehaviorAction::Tab(t) => {
                if ui.tab != *t {
                    draft.tidy_text();
                    ui.tab = *t;
                }
            }
            // The list opens and replaces behaviors: the behavior's draft comes back first (`assists::switch`).
            BehaviorAction::Behaviors if ui.tab == BehaviorTab::Assists => ui.tab = BehaviorTab::Behavior,
            BehaviorAction::Select(n) => ui.selected = Some(Selection::Node(*n)),
            BehaviorAction::Deselect => ui.selected = None,
            BehaviorAction::TestMode(m) => {
                ui.test_mode = *m;
                ui.modes_open = false;
            }
            BehaviorAction::Modes => ui.modes_open = !ui.modes_open,
            // `edit::apply` (editing), `saving.rs` (Save, TestInRange), `list.rs` (Behaviors).
            BehaviorAction::TestInRange
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

/// Remembers whether the screen was opened from home or from the drydock (Esc goes back there); coming back from a
/// test flight keeps it.
pub(super) fn way_in(mut transitions: MessageReader<StateTransitionEvent<AppMode>>, mut ui: ResMut<BehaviorUi>) {
    match transitions.read().last().and_then(|t| t.exited) {
        Some(AppMode::Home) => ui.from_home = true,
        Some(AppMode::Drydock) => ui.from_home = false,
        _ => {}
    }
}

/// The drydock's BEHAVIOR button opens the screen.
pub(super) fn open_from_drydock(mut actions: MessageReader<DrydockAction>, mut next: ResMut<NextState<AppMode>>) {
    if actions.read().any(|a| *a == DrydockAction::Behavior) {
        next.set(AppMode::Behavior);
    }
}
