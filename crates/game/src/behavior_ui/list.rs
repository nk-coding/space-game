//! The BEHAVIORS list (GDR-0057; written 13 Oct 2026), the BEHAVIOR screen's counterpart of the drydock's
//! BLUEPRINTS: one layer over the screen above the bottom bar (Esc or CLOSE closes it; Esc first leaves a rename or
//! a delete question).
//!
//! * **Your behaviors** (most recently saved first: name; rules, when saved, or the first error; "THIS DESIGN" for
//!   the one the design flies, "IN THE EDITOR" for the open one) and **templates** (the content behaviors).
//! * On the picked row: **OPEN** (edit it; the design keeps flying its own), **USE FOR THIS DESIGN** (the design
//!   names it from now on, and it opens), **DUPLICATE** (a saved copy with a fresh name), **RENAME** (saved at once;
//!   the open draft follows), **DELETE** (asks first; a design naming it flies its faction's behavior, as flights
//!   do, and the status line says so; deleting the open one leaves it on the screen, not saved).
//! * **NEW BEHAVIOR**: a new draft with one rule (`always -> ` the vocabulary's first action), not saved until SAVE.
//! * Opening, USE and NEW replace the draft on the screen; unsaved edits are saved first (`saving.rs`).

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use data::Content;

use crate::drydock::PlayerDesign;
use crate::drydock::behaviors::{BehaviorEntry, PlayerBehaviors};
use crate::drydock::library::{self, MAX_NAME, fresh_name};
use crate::flight_hud::HudFonts;
use crate::sim_bridge::GameContent;
use crate::storage::now_unix;
use crate::style::{Style, ui_color};
use crate::ui_kit::{
    Hot, Pill, ScrollRows, Scrollbar, TextField, Tip, Tone, UiBlock, UiClick, UiPointer, field_text, label, number, prose, set_color, set_text, spawn_pill,
    sub_tone, tracked,
};

use super::draft::{BehaviorDraft, design_behavior};
use super::saving::{Lib, opened_note};
use super::{BehaviorAction, BehaviorUi, BehaviorView};

/// Rows per list; longer lists scroll (by pixels: the pool has one row more, for a row cut at an edge).
const LIST_ROWS: usize = 7;
const ROW_H: f32 = 46.0;
const ROW_GAP: f32 = 4.0;
/// A list's height: [`LIST_ROWS`] rows.
const LIST_H: f32 = LIST_ROWS as f32 * (ROW_H + ROW_GAP) - ROW_GAP;

/// What the action bar shows.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Browse,
    /// Renaming this saved behavior (the field has the keyboard).
    Rename(String),
    /// Asking before deleting this saved behavior.
    Delete(String),
}

/// The list's state.
#[derive(Resource, Default)]
pub struct BehaviorList {
    open: bool,
    /// The player's behaviors (`PlayerBehaviors::list`).
    mine: Vec<BehaviorEntry>,
    /// Content behavior ids, in content order.
    templates: Vec<String>,
    /// The picked row's id (content and player ids never clash).
    pick: Option<String>,
    /// First row shown, per list.
    offset: [usize; 2],
    mode: Mode,
    /// When the list was made (Unix seconds), for "5 MIN AGO".
    now: u64,
    /// The view the screen had when the list opened (the TEXT view's editor would draw over the list).
    view: BehaviorView,
    /// The draft's generation then: another draft opened from the list shows its errors in the TEXT view.
    generation: u64,
}

impl BehaviorList {
    pub fn is_open(&self) -> bool {
        self.open
    }

    fn entry(&self, id: &str) -> Option<&BehaviorEntry> {
        self.mine.iter().find(|e| e.id == id)
    }

    /// The id on row `k` of a list (0 yours, 1 templates).
    fn row(&self, list: usize, k: usize) -> Option<&str> {
        let i = self.offset[list] + k;
        match list {
            0 => self.mine.get(i).map(|e| e.id.as_str()),
            _ => self.templates.get(i).map(String::as_str),
        }
    }

    fn refresh(&mut self, behaviors: &PlayerBehaviors, content: &Content) {
        self.now = now_unix();
        self.mine = behaviors.list();
        self.templates = content.behaviors.iter().map(|b| b.id.clone()).collect();
        if let Some(id) = &self.pick
            && self.entry(id).is_none()
            && !content.behaviors.contains(id)
        {
            self.pick = None;
        }
        for (list, len) in [self.mine.len(), self.templates.len()].into_iter().enumerate() {
            self.offset[list] = self.offset[list].min(len.saturating_sub(LIST_ROWS));
        }
    }
}

/// The tag on a row: whether the design flies it and whether it is open on the screen (with unsaved edits).
pub fn row_tag(id: &str, flown: Option<&str>, open: Option<&str>, dirty: bool) -> String {
    let mut parts = Vec::new();
    if flown == Some(id) {
        parts.push("THIS DESIGN");
    }
    if open == Some(id) {
        parts.push(if dirty { "IN THE EDITOR  ·  UNSAVED" } else { "IN THE EDITOR" });
    }
    parts.join("  ·  ")
}

// ---------------------------------------------------------------------------------------------------------
// The nodes

#[derive(Component)]
pub(super) struct ListRoot;

/// The action bar's variants (browse pills, the rename field, the delete question).
#[derive(Component)]
pub(super) struct BarNode(u8);

#[derive(Component)]
pub(super) struct RenameField;

/// A list's rows container (0 yours, 1 templates), for the wheel.
#[derive(Component)]
pub(super) struct ListNode(usize);

#[derive(Component)]
pub(super) struct RowNode(usize, usize);

#[derive(Component)]
pub(super) struct EmptyHint;

/// Every button of the list.
#[derive(Component, Clone, Debug, PartialEq)]
pub(super) enum ListButton {
    Close,
    Row(usize, usize),
    Open,
    Use,
    Duplicate,
    Rename,
    Delete,
    ConfirmDelete,
    KeepDelete,
    New,
}

/// Every text the list updates.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ListText {
    Section(usize),
    RowName(usize, usize),
    RowSub(usize, usize),
    RowTag(usize, usize),
    DeleteQuestion,
}

pub(super) fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, clear: Res<ClearColor>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let pill = Pill::new(Tone::Teal);
    let row = |gap: f32| Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), flex_shrink: 0.0, ..default() };
    let head = |text: &str, width: f32| (label(fonts, text, 9.0, c(Tone::Dim, 1.0)), Node { width: px(width), flex_shrink: 0.0, ..default() });
    commands
        .spawn((
            ListRoot,
            UiBlock,
            // Hot, so a click on the veil reaches no widget under it.
            Hot::default(),
            GlobalZIndex(20),
            Visibility::Hidden,
            BackgroundColor(ui_color(clear.0, 0.985)),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(66),
                flex_direction: FlexDirection::Column,
                padding: UiRect::new(px(100), px(100), px(18), px(10)),
                row_gap: px(18),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(14.0) }).with_children(|h| {
                h.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                    col.spawn(label(fonts, "BEHAVIOR  ›  BEHAVIORS", 10.0, c(Tone::Label, 1.0)));
                    col.spawn(number(fonts, "Your behaviors and templates", 20.0, c(Tone::Number, 1.0)));
                });
                spawn_pill(h, fonts, "CLOSE", 9.5, 30.0, None, pill, (ListButton::Close, Tip::key("CLOSE", "ESC")));
            });
            root.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(48), flex_grow: 1.0, min_height: px(0), ..default() }).with_children(|lists| {
                for (list, width) in [(0, 600.0), (1, 432.0)] {
                    lists.spawn(Node { width: px(width), flex_direction: FlexDirection::Column, row_gap: px(10), min_height: px(0), ..default() }).with_children(|col| {
                        col.spawn((ListText::Section(list), label(fonts, "", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::left(px(12)), ..default() }));
                        col.spawn((
                            ListNode(list),
                            Scrollbar::vertical(),
                            ScrollRows::default(),
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: px(ROW_GAP),
                                flex_grow: 1.0,
                                min_height: px(0),
                                max_height: px(LIST_H),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                        ))
                        .with_children(|l| {
                            if list == 0 {
                                let hint = "NOTHING SAVED YET  ·  SAVE KEEPS THE BEHAVIOR ON THE SCREEN";
                                l.spawn((EmptyHint, label(fonts, hint, 8.5, c(Tone::Dim, 1.0)), Node { margin: UiRect::new(px(12), px(0), px(14), px(0)), ..default() }));
                            }
                            for k in 0..=LIST_ROWS {
                                spawn_row(l, fonts, &c, list, k);
                            }
                        });
                    });
                }
            });
            // The action bar: browse / rename / delete.
            root.spawn(Node { height: px(34), ..row(10.0) }).with_children(|bar| {
                bar.spawn((BarNode(0), row(8.0))).with_children(|b| {
                    for (text, button) in [
                        ("OPEN", ListButton::Open),
                        ("USE FOR THIS DESIGN", ListButton::Use),
                        ("DUPLICATE", ListButton::Duplicate),
                        ("RENAME", ListButton::Rename),
                        ("DELETE", ListButton::Delete),
                    ] {
                        spawn_pill(b, fonts, text, 9.5, 30.0, None, pill, button);
                    }
                });
                bar.spawn((BarNode(1), Node { display: Display::None, ..row(12.0) })).with_children(|b| {
                    b.spawn(head("NEW NAME", 90.0));
                    b.spawn((
                        RenameField,
                        TextField { placeholder: "Name".into(), max_chars: MAX_NAME, ..default() },
                        Hot::default(),
                        Node {
                            width: px(320),
                            height: px(32),
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(px(14)),
                            border: UiRect::all(px(1.2)),
                            border_radius: BorderRadius::MAX,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BorderColor::all(c(Tone::Label, 0.3)),
                    ))
                    .with_children(|f| {
                        f.spawn(field_text(fonts, 13.0));
                    });
                    b.spawn(label(fonts, "TYPE A NEW NAME", 8.5, c(Tone::Dim, 1.0)));
                });
                bar.spawn((BarNode(2), Node { display: Display::None, ..row(10.0) })).with_children(|b| {
                    b.spawn((ListText::DeleteQuestion, label(fonts, "", 9.5, c(Tone::Coral, 1.0)), Node { margin: UiRect::right(px(8)), ..default() }));
                    spawn_pill(b, fonts, "DELETE", 9.5, 30.0, None, Pill { active: true, ..Pill::new(Tone::Coral) }, ListButton::ConfirmDelete);
                    spawn_pill(b, fonts, "KEEP", 9.5, 30.0, None, pill, ListButton::KeepDelete);
                });
            });
            root.spawn((Node { height: px(1), flex_shrink: 0.0, ..default() }, BackgroundColor(c(Tone::Label, 0.14))));
            root.spawn(row(8.0)).with_children(|r| {
                r.spawn(head("NEW", 80.0));
                spawn_pill(r, fonts, "NEW BEHAVIOR  ·  ONE RULE", 9.5, 30.0, None, pill, ListButton::New);
            });
        });
}

fn spawn_row(l: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, list: usize, k: usize) {
    l.spawn((
        RowNode(list, k),
        ListButton::Row(list, k),
        Pill { bare: true, ..Pill::new(Tone::Teal) },
        Hot::default(),
        Node {
            display: Display::None,
            height: px(ROW_H),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(12),
            padding: UiRect::horizontal(px(16)),
            border: UiRect::all(px(1.2)),
            border_radius: BorderRadius::MAX,
            overflow: Overflow::clip(),
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    ))
    .with_children(|r| {
        r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), min_width: px(0), flex_shrink: 1.0, overflow: Overflow::clip(), ..default() }).with_children(|t| {
            t.spawn((ListText::RowName(list, k), prose(fonts, "", 14.0, c(Tone::Number, 1.0))));
            t.spawn((ListText::RowSub(list, k), tracked(fonts, "", 8.5, c(Tone::Dim, 1.0), 0.2)));
        });
        r.spawn((ListText::RowTag(list, k), label(fonts, "", 8.5, c(Tone::Teal, 1.0)), Node { flex_shrink: 0.0, ..default() }));
    });
}

// ---------------------------------------------------------------------------------------------------------
// Handling

/// Opens the list (re-read, the open behavior picked; the TEXT view steps aside) or closes it.
fn set_open(list: &mut BehaviorList, open: bool, ui: &mut BehaviorUi, lib: &mut Lib, content: &Content) {
    if open == list.open {
        return;
    }
    list.open = open;
    list.mode = Mode::Browse;
    if open {
        list.refresh(&lib.behaviors, content);
        list.pick.clone_from(&lib.draft.source);
        list.view = ui.view;
        list.generation = lib.draft.generation();
        if ui.view == BehaviorView::Text {
            lib.draft.tidy_text();
            ui.view = BehaviorView::Graph;
        }
    } else {
        ui.view = closed_view(list.view, list.generation != lib.draft.generation(), !lib.draft.errors.is_empty());
    }
}

/// The view after the list closes: the one it opened from; but another draft opened from the list with errors
/// shows them in the TEXT view. (The same draft with errors stays in the GRAPH it was looked at in: switching would
/// put the caret in the text, and the next T would be typed.)
fn closed_view(opened_from: BehaviorView, other_draft: bool, errors: bool) -> BehaviorView {
    if other_draft && errors { BehaviorView::Text } else { opened_from }
}

/// BEHAVIORS, the list's buttons and keys.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    mut actions: MessageReader<BehaviorAction>,
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(&ListButton, Option<&Pill>)>,
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<UiPointer>,
    mut rename: Single<&mut TextField, With<RenameField>>,
    mut list: ResMut<BehaviorList>,
    mut ui: ResMut<BehaviorUi>,
    mut lib: Lib,
    content: Res<GameContent>,
) {
    let content = &content.0;
    // A rename ends when its field lets go of the keyboard: Esc cancels, anything else renames.
    if let Mode::Rename(id) = list.mode.clone()
        && !rename.focused
    {
        list.mode = Mode::Browse;
        let name = rename.value.trim().to_string();
        if !keys.just_pressed(KeyCode::Escape) && !name.is_empty() {
            rename_behavior(&mut list, &mut lib, content, &id, &name);
        }
    }
    for a in actions.read() {
        if *a == BehaviorAction::Behaviors {
            let open = !list.open;
            set_open(&mut list, open, &mut ui, &mut lib, content);
        }
    }
    let todo: Vec<ListButton> = clicks
        .read()
        .filter_map(|c| buttons.get(c.0).ok())
        .filter(|(_, pill)| !pill.is_some_and(|p| p.disabled))
        .map(|(b, _)| b.clone())
        .collect();
    if !list.open {
        return;
    }
    // Esc (not one a text field had): first out of a rename or a delete question, then the list closes.
    if keys.just_pressed(KeyCode::Escape) && !pointer.typing {
        if list.mode != Mode::Browse {
            list.mode = Mode::Browse;
        } else {
            set_open(&mut list, false, &mut ui, &mut lib, content);
        }
    }
    for b in todo {
        press(b, &mut list, &mut ui, &mut lib, content, &mut rename);
    }
    // A save while the list is open (Ctrl+S, SAVE in the bottom bar) shows in its rows at once.
    if list.open && lib.behaviors.is_changed() {
        list.refresh(&lib.behaviors, content);
    }
}

/// Leaving the screen closes the list and hides its layer at once ([`sync`] runs only on the screen; a layer left
/// visible would block the pointer in the drydock and in flight).
pub(super) fn leave(
    mut list: ResMut<BehaviorList>,
    mut ui: ResMut<BehaviorUi>,
    mut lib: Lib,
    content: Res<GameContent>,
    mut root: Single<&mut Visibility, With<ListRoot>>,
) {
    set_open(&mut list, false, &mut ui, &mut lib, &content.0);
    **root = Visibility::Hidden;
}

fn rename_behavior(list: &mut BehaviorList, lib: &mut Lib, content: &Content, id: &str, name: &str) {
    // The open draft follows; it stays unsaved only if it had other edits.
    let follows = lib.draft.source.as_deref() == Some(id);
    let clean = follows && !lib.draft.dirty();
    if let Err(e) = lib.behaviors.rename(&mut lib.storage, id, name, now_unix()) {
        lib.say(format!("not renamed: {e}"), Tone::Coral);
        return;
    }
    if follows {
        lib.draft.name = name.to_string();
        if clean {
            lib.draft.mark_clean();
        }
        lib.slot.write_now = true;
    }
    lib.say(format!("renamed to {name}"), Tone::Teal);
    list.refresh(&lib.behaviors, content);
}

/// One button of the list.
fn press(b: ListButton, list: &mut BehaviorList, ui: &mut BehaviorUi, lib: &mut Lib, content: &Content, rename: &mut TextField) {
    let pick = list.pick.clone();
    match b {
        ListButton::Close => set_open(list, false, ui, lib, content),
        ListButton::Row(l, k) => {
            list.pick = list.row(l, k).map(str::to_string);
            list.mode = Mode::Browse;
        }
        ListButton::Open => {
            let Some(id) = pick else { return };
            if lib.draft.source.as_deref() == Some(id.as_str()) {
                lib.say("it is open already", Tone::Label);
            } else {
                match lib.open(content, &id) {
                    Ok(msg) => lib.say(msg, Tone::Teal),
                    Err(e) => return lib.say(format!("not opened: {e}"), Tone::Coral),
                }
            }
            set_open(list, false, ui, lib, content);
        }
        ListButton::Use => {
            let Some(id) = pick else { return };
            let mut kept = None;
            if lib.draft.source.as_deref() != Some(id.as_str()) {
                let Some(next) = lib.behaviors.behavior(content, &id).and_then(|b| BehaviorDraft::from_behavior(b, &content.behavior_nodes)) else {
                    return lib.say(format!("{id} doesn't open"), Tone::Coral);
                };
                match lib.switch(content, next) {
                    Ok(k) => kept = k,
                    Err(e) => return lib.say(format!("not used: {e}"), Tone::Coral),
                }
            }
            lib.link(&id);
            let mut msg = format!("{} flies {} now", lib.design.blueprint().name, lib.name_of(content, &id));
            if let Some(k) = kept {
                msg = format!("your edits saved as {k}  ·  {msg}");
            }
            let tone = match lib.behaviors.get(&id) {
                Some(p) if !p.runs() => {
                    msg.push_str("  ·  it has errors: flights fly the faction's behavior until they are fixed");
                    Tone::Gold
                }
                _ => Tone::Teal,
            };
            lib.say(msg, tone);
            set_open(list, false, ui, lib, content);
        }
        ListButton::Duplicate => {
            let Some(id) = pick else { return };
            match lib.behaviors.duplicate(&mut lib.storage, content, &id, now_unix()) {
                Ok(new) => {
                    let name = lib.name_of(content, &new);
                    lib.say(format!("saved a copy: {name}"), Tone::Teal);
                    list.refresh(&lib.behaviors, content);
                    list.offset[0] = 0;
                    list.pick = Some(new);
                }
                Err(e) => lib.say(format!("no copy made: {e}"), Tone::Coral),
            }
        }
        ListButton::Rename => {
            if let Some(id) = pick
                && let Some(p) = lib.behaviors.get(&id)
            {
                rename.value.clone_from(&p.behavior.name);
                rename.focus();
                list.mode = Mode::Rename(id);
            }
        }
        ListButton::Delete => {
            if let Some(id) = pick.filter(|id| list.entry(id).is_some()) {
                list.mode = Mode::Delete(id);
            }
        }
        ListButton::ConfirmDelete => {
            let Mode::Delete(id) = std::mem::take(&mut list.mode) else { return };
            let name = list.entry(&id).map_or(id.clone(), |e| e.name.clone());
            let named = lib.design.blueprint().behavior.as_deref() == Some(id.as_str());
            if let Err(e) = lib.behaviors.delete(&mut lib.storage, &id) {
                return lib.say(format!("not deleted: {e}"), Tone::Coral);
            }
            if lib.draft.source.as_deref() == Some(id.as_str()) {
                // It stays on the screen, not saved; it is not saved again unless it changes.
                lib.draft.source = None;
            }
            let mut msg = format!("deleted {name}");
            if named {
                // The design keeps naming it and flies its faction's behavior, as flights do.
                let flies = lib.flown(content).map_or("no behavior".into(), |f| lib.name_of(content, &f));
                msg.push_str(&format!("  ·  {} named it: it flies {flies} until you use another", lib.design.blueprint().name));
            }
            lib.slot.flown = lib.flown(content);
            lib.slot.write_now = true;
            lib.say(msg, if named { Tone::Gold } else { Tone::Teal });
            list.refresh(&lib.behaviors, content);
        }
        ListButton::KeepDelete => list.mode = Mode::Browse,
        ListButton::New => {
            let Some(mut next) = BehaviorDraft::empty(&content.behavior_nodes) else { return };
            next.name = fresh_name(super::draft::NEW_NAME, &lib.behaviors.names());
            next.mark_clean();
            match lib.switch(content, next) {
                Ok(kept) => {
                    let msg = opened_note(kept.as_deref(), &format!("a new behavior: {}", lib.draft.name));
                    lib.say(msg, Tone::Teal);
                    set_open(list, false, ui, lib, content);
                }
                Err(e) => lib.say(format!("no new behavior: {e}"), Tone::Coral),
            }
        }
    }
}

/// Lays each list out from its scroll (the kit's wheel and scrollbar move it): the first row its pool shows. A first
/// row set here (a new behavior at the top) scrolls the list there.
pub(super) fn scroll(mut lists: Query<(&ListNode, &ComputedNode, &mut ScrollRows)>, mut list: ResMut<BehaviorList>, mut laid: Local<[usize; 2]>) {
    if !list.open {
        return;
    }
    for (node, computed, mut rows) in &mut lists {
        let len = if node.0 == 0 { list.mine.len() } else { list.templates.len() };
        if list.offset[node.0] != laid[node.0] {
            rows.scroll = list.offset[node.0] as f32 * (ROW_H + ROW_GAP);
        }
        let w = ScrollRows::lay(&mut rows, &vec![ROW_H; len], ROW_GAP, computed.size.y * computed.inverse_scale_factor);
        laid[node.0] = w.first;
        if list.offset[node.0] != w.first {
            list.offset[node.0] = w.first;
        }
    }
}

/// The list follows [`BehaviorList`]: visibility, rows, pills, texts.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync(
    list: Res<BehaviorList>,
    draft: Res<BehaviorDraft>,
    design: Res<PlayerDesign>,
    behaviors: Res<PlayerBehaviors>,
    content: Res<GameContent>,
    style: Res<Style>,
    mut root: Single<&mut Visibility, With<ListRoot>>,
    mut nodes: Query<(&mut Node, Option<&BarNode>, Option<&RowNode>, Has<EmptyHint>), (Without<TextField>, Or<(With<BarNode>, With<RowNode>, With<EmptyHint>)>)>,
    mut pills: Query<(&ListButton, &mut Pill)>,
    mut texts: Query<(&ListText, &mut Text, &mut TextColor)>,
) {
    let want = if list.open { Visibility::Inherited } else { Visibility::Hidden };
    if **root != want {
        **root = want;
    }
    if !list.open || (!list.is_changed() && !draft.is_changed() && !design.is_changed() && !behaviors.is_changed()) {
        return;
    }
    let content = &content.0;
    let bar = match list.mode {
        Mode::Browse => 0,
        Mode::Rename(_) => 1,
        Mode::Delete(_) => 2,
    };
    let row_len = [list.mine.len(), list.templates.len()];
    for (mut n, b, r, empty) in &mut nodes {
        let show = match (b, r) {
            (Some(b), _) => b.0 == bar,
            (_, Some(r)) => list.offset[r.0] + r.1 < row_len[r.0],
            _ if empty => list.mine.is_empty(),
            _ => continue,
        };
        let d = if show { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    let flown = design_behavior(content, &behaviors, design.blueprint()).map(|b| b.id.clone());
    let pick = list.pick.as_deref();
    let opens = pick.is_some_and(|id| behaviors.behavior(content, id).is_some());
    let mine = pick.is_some_and(|id| list.entry(id).is_some());
    // USE is for another behavior than the design names (or than its faction's default, when it names none);
    // a design naming one that is gone can use any.
    let used = design.blueprint().behavior.as_deref().or(flown.as_deref());
    for (b, mut pill) in &mut pills {
        let (active, disabled) = match b {
            ListButton::Row(l, k) => (pick.is_some() && list.row(*l, *k) == pick, false),
            ListButton::Open | ListButton::Duplicate => (false, !opens),
            ListButton::Use => (false, !opens || pick == used),
            ListButton::Rename => (false, !(mine && opens)),
            ListButton::Delete => (false, !mine),
            ListButton::ConfirmDelete => (true, false),
            _ => (false, false),
        };
        if pill.active != active || pill.disabled != disabled {
            pill.active = active;
            pill.disabled = disabled;
        }
    }
    let ui = &style.ui;
    let open = draft.source.as_deref();
    let dirty = draft.dirty();
    let mut s = String::new();
    for (t, mut text, mut color) in &mut texts {
        s.clear();
        let tone = match *t {
            ListText::Section(0) => {
                s.push_str(&format!("YOUR BEHAVIORS  ·  {}", list.mine.len()));
                Tone::Dim
            }
            ListText::Section(_) => {
                s.push_str(&format!("TEMPLATES  ·  {}  ·  SAVE MAKES YOUR COPY", list.templates.len()));
                Tone::Dim
            }
            ListText::RowName(l, k) => {
                let id = list.row(l, k);
                let name = match l {
                    0 => id.and_then(|id| list.entry(id)).map(|e| e.name.as_str()),
                    _ => id.and_then(|id| content.behaviors.get(id)).map(|b| b.name.as_str()),
                };
                s.push_str(name.unwrap_or(""));
                Tone::Number
            }
            ListText::RowSub(0, k) => match list.row(0, k).and_then(|id| list.entry(id)) {
                Some(e) => {
                    let rules = plural(e.rules, "RULE");
                    match &e.problem {
                        Some(p) if behaviors.get(&e.id).is_none() => {
                            s.push_str(&format!("UNREADABLE  ·  {}", p.to_uppercase()));
                            Tone::Coral
                        }
                        Some(p) => {
                            s.push_str(&format!("{rules}  ·  {}", p.to_uppercase()));
                            Tone::Coral
                        }
                        None => {
                            s.push_str(&rules);
                            if let Some(t) = e.saved {
                                s.push_str(&format!("  ·  {}", library::ago(list.now, t)));
                            }
                            sub_tone(pick.is_some() && list.row(0, k) == pick)
                        }
                    }
                }
                None => Tone::Dim,
            },
            ListText::RowSub(_, k) => {
                if let Some(b) = list.row(1, k).and_then(|id| content.behaviors.get(id)) {
                    s.push_str(&plural(b.rules.len(), "RULE"));
                    for tag in &b.tags {
                        s.push_str(&format!("  ·  {}", tag.to_uppercase()));
                    }
                }
                sub_tone(pick.is_some() && list.row(1, k) == pick)
            }
            ListText::RowTag(l, k) => {
                if let Some(id) = list.row(l, k) {
                    s.push_str(&row_tag(id, flown.as_deref(), open, dirty));
                }
                Tone::Teal
            }
            ListText::DeleteQuestion => {
                if let Mode::Delete(id) = &list.mode {
                    let name = list.entry(id).map_or(id.as_str(), |e| e.name.as_str());
                    s.push_str(&format!("DELETE {}? THERE IS NO UNDO", name.to_uppercase()));
                    if flown.as_deref() == Some(id.as_str()) {
                        s.push_str("  ·  THIS DESIGN FLIES IT");
                    }
                }
                Tone::Coral
            }
        };
        set_text(&mut text, &s);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
    }
}

/// "1 RULE", "6 RULES".
fn plural(n: usize, word: &str) -> String {
    if n == 1 { format!("1 {word}") } else { format!("{n} {word}S") }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::drydock::library::DesignSlot;
    use crate::storage::{MemoryBackend, Storage};

    /// Leaving the screen with the list open (the DRYDOCK pill under it, T) hides its layer: it would cover the
    /// drydock and the flight and take their clicks.
    #[test]
    fn leaving_the_screen_hides_the_list_layer() {
        let content = data::base_content();
        let mut world = World::new();
        world.insert_resource(PlayerDesign::starter(&content, "halvex_runner"));
        world.insert_resource(GameContent(content));
        world.insert_resource(Storage::new(MemoryBackend::default()));
        world.init_resource::<PlayerBehaviors>();
        world.init_resource::<DesignSlot>();
        world.init_resource::<BehaviorDraft>();
        world.init_resource::<super::super::saving::DraftSlot>();
        world.init_resource::<super::super::saving::ScreenNote>();
        world.init_resource::<BehaviorUi>();
        world.insert_resource(BehaviorList { open: true, ..default() });
        let root = world.spawn((ListRoot, Visibility::Inherited)).id();
        world.run_system_once(leave).unwrap();
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden));
        assert!(!world.resource::<BehaviorList>().is_open());
    }

    /// A save while the list is open (Ctrl+S, SAVE) shows in the rows at once.
    #[test]
    fn the_open_list_follows_a_save() {
        let content = data::base_content();
        let mut world = World::new();
        world.insert_resource(PlayerDesign::starter(&content, "halvex_runner"));
        world.insert_resource(Storage::new(MemoryBackend::default()));
        world.init_resource::<PlayerBehaviors>();
        world.init_resource::<DesignSlot>();
        world.init_resource::<BehaviorDraft>();
        world.init_resource::<super::super::saving::DraftSlot>();
        world.init_resource::<super::super::saving::ScreenNote>();
        world.init_resource::<BehaviorUi>();
        world.init_resource::<BehaviorList>();
        world.init_resource::<Messages<BehaviorAction>>();
        world.init_resource::<Messages<UiClick>>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<UiPointer>();
        world.spawn((RenameField, TextField::default()));
        world.insert_resource(GameContent(content.clone()));
        let apply = world.register_system(apply);
        world.write_message(BehaviorAction::Behaviors);
        world.run_system(apply).unwrap();
        assert!(world.resource::<BehaviorList>().is_open() && world.resource::<BehaviorList>().mine.is_empty());
        let copy = world.resource::<PlayerBehaviors>().copy_of(&content, "skirmisher").unwrap();
        world.resource_scope(|world, mut storage: Mut<Storage>| {
            world.resource_mut::<PlayerBehaviors>().save(&mut storage, &content, &copy, 1).unwrap();
        });
        world.run_system(apply).unwrap();
        assert_eq!(world.resource::<BehaviorList>().mine.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["skirmisher_copy"]);
    }

    /// Closing the list goes back to the view it opened from; only another draft with errors (opened from the list)
    /// switches to the TEXT view. (GRAPH with a broken text → BEHAVIORS → Esc used to land in TEXT with the caret
    /// in, and the next T typed a `t`.)
    #[test]
    fn closing_the_list_keeps_the_view() {
        use BehaviorView::{Graph, Text};
        assert_eq!(closed_view(Graph, false, true), Graph);
        assert_eq!(closed_view(Text, false, false), Text);
        assert_eq!(closed_view(Graph, true, false), Graph);
        assert_eq!(closed_view(Graph, true, true), Text);
    }

    #[test]
    fn row_tags_say_what_the_design_flies_and_what_is_open() {
        assert_eq!(row_tag("careful", Some("careful"), Some("careful"), false), "THIS DESIGN  ·  IN THE EDITOR");
        assert_eq!(row_tag("careful", Some("skirmisher"), Some("careful"), true), "IN THE EDITOR  ·  UNSAVED");
        assert_eq!(row_tag("skirmisher", Some("skirmisher"), Some("careful"), true), "THIS DESIGN");
        assert_eq!(row_tag("raider_basic", Some("skirmisher"), None, false), "");
    }
}
