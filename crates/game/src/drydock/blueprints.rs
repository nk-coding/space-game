//! Saving, the BLUEPRINTS and EXPORT screens, the editable design name and the autosave (written 13 Oct 2026;
//! the storage is `crate::storage`, the design library `library.rs`, share codes and files `share.rs`).
//!
//! * **SAVE** (Ctrl+S, the button) writes the design under its id (`library::save`); a design with problems
//!   still saves, and the status line names the first problem.
//! * **Autosave**: the working design (with its frame and the id it is saved under) is written about a second
//!   after the last change and when a test flight starts, and comes back at the next start. The first start is
//!   a copy of the starter ship ("Halvex Runner copy").
//! * **BLUEPRINTS** floats over the screen above the bottom bar: your designs (name, frame, size, parts, when
//!   saved) and the content ships as templates; OPEN, DUPLICATE, RENAME, DELETE (with a confirm step) on the
//!   picked one; NEW in a frame; FRAME of the design in the drydock (refused with a message when it doesn't
//!   fit); IMPORT a `.ron` file (web: a file chooser; desktop: drop it onto the window) or paste a share code.
//!   Opening another design replaces the one in the drydock; when that one has unsaved changes, it is saved
//!   first.
//! * **EXPORT** shows the design's `.ron` file (download on the web, a file in the data directory on desktop)
//!   and its share code in a selectable box (click it, then Ctrl+C) with COPY.
//! * **The name** top left: a click turns it into a text field; Enter (or a click elsewhere) renames the design
//!   (`PlayerDesign::edit_header`), Esc keeps the old name.

use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::text::{Justify, LineBreak};
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use data::{Blueprint, Content};

use crate::clipboard::Clipboard;
use crate::flight_hud::HudFonts;
use crate::sim_bridge::GameContent;
use crate::storage::{Storage, now_unix};
use crate::style::{Style, ui_color};
use crate::ui_kit::{Hot, Pill, TextField, TextFieldText, Tone, UiBlock, UiClick, label, number, prose, set_color, set_text, spawn_pill, tracked};

use super::actions::DrydockAction;
use super::behaviors::{PlayerBehaviors, validate_design};
use super::library::{self, Autosave, DesignSlot, Entry};
use super::panels::PanelText;
use super::share;
use super::state::DrydockStatus;
use super::view::DockCamera;
use super::{DockSet, PlayerDesign, stats};

/// Rows per list (designs, templates); the wheel scrolls longer lists.
const LIST_ROWS: usize = 6;
const ROW_H: f32 = 46.0;
const ROW_GAP: f32 = 4.0;
/// The longest design name.
const MAX_NAME: usize = 40;

pub(super) struct BlueprintsPlugin;

impl Plugin for BlueprintsPlugin {
    fn build(&self, app: &mut App) {
        // Before any system runs, so the first flight (a plain sandbox start) flies the restored design too.
        let (slot, shelf) = restore(app.world_mut());
        app.insert_resource(slot)
            .insert_resource(shelf)
            .init_resource::<Saver>()
            .add_systems(Startup, spawn_shelf.after(crate::flight_hud::spawn_hud))
            .add_systems(
                Update,
                (
                    // Before the editing tools and the report: a design opened, renamed or moved to another frame
                    // this frame is what they, the report and the status see in the same frame.
                    (title, apply).chain().after(super::actions::apply).before(super::tools::apply),
                    // After the status of this revision is made, so a message is not overwritten.
                    (autosave, announce).chain().after(stats::update),
                )
                    .in_set(DockSet::Apply),
            )
            .add_systems(Update, (scroll_lists, sync_shelf, sync_title).in_set(DockSet::Show))
            .add_systems(OnExit(crate::mode::AppMode::Drydock), leave)
            .add_systems(Last, save_on_exit);
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, dropped_files.in_set(DockSet::Input));
    }
}

// ---------------------------------------------------------------------------------------------------------
// State

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Blueprints,
    Export,
}

/// The picked row: a saved design or a content ship (by id).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Pick {
    Design(String),
    Template(String),
}

/// What the action bar under the lists shows.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Browse,
    /// Renaming this saved design (the field has the keyboard).
    Rename(String),
    /// Asking before deleting this saved design.
    Delete(String),
}

/// The export page's texts.
#[derive(Clone, Debug, Default)]
struct Export {
    file: String,
    ron: String,
    code: String,
    made_with: String,
    /// The design revision they were made from.
    revision: u64,
}

impl Export {
    /// The export of the design in the drydock (with the player behavior it names, if any).
    fn of(lib: &Lib, content: &Content) -> Self {
        let bp = lib.export(content);
        let behavior = lib.behaviors.named_by(content, &bp);
        let made_with: Vec<String> = content.packs.iter().map(|p| format!("{} {}", p.id, p.version)).collect();
        Export {
            file: format!("{}.ron", bp.id),
            ron: share::export_text(&bp, behavior),
            code: share::encode(content, &bp, behavior),
            made_with: made_with.join(", ").to_uppercase(),
            revision: lib.design.revision(),
        }
    }
}

/// The BLUEPRINTS and EXPORT screens' state.
#[derive(Resource, Default)]
pub(super) struct Shelf {
    page: Option<Page>,
    designs: Vec<Entry>,
    /// Content ship ids, in content order.
    templates: Vec<String>,
    pick: Option<Pick>,
    /// First row shown, per list.
    offset: [usize; 2],
    mode: Mode,
    export: Export,
    /// The share code is selected: Ctrl+C copies it.
    code_selected: bool,
    /// The paste box has the focus: Ctrl+V imports.
    paste_armed: bool,
    /// When the list was made (Unix seconds), for "5 MIN AGO".
    now: u64,
    /// Files picked or dropped, waiting to be imported: (file name, text or why not).
    incoming: Vec<(String, Result<String, String>)>,
    /// A message for the status line, shown once the design check has seen the current revision.
    say: Option<(String, Tone)>,
}

impl Shelf {
    fn say(&mut self, text: impl AsRef<str>, tone: Tone) {
        self.say = Some((text.as_ref().to_uppercase(), tone));
    }

    fn entry(&self, id: &str) -> Option<&Entry> {
        self.designs.iter().find(|e| e.id == id)
    }
}

/// The autosave timer and the keyboard state the screens need between frames.
#[derive(Resource, Default)]
pub(super) struct Saver {
    timer: Autosave,
    /// The name top left is being edited (its field is open).
    editing_title: bool,
    /// A text field had the keyboard last frame (so this frame's Esc was for it).
    field_had_keys: bool,
}

/// The resources the library operations work on.
#[derive(SystemParam)]
pub(super) struct Lib<'w> {
    storage: ResMut<'w, Storage>,
    design: ResMut<'w, PlayerDesign>,
    slot: ResMut<'w, DesignSlot>,
    cam: ResMut<'w, DockCamera>,
    behaviors: ResMut<'w, PlayerBehaviors>,
}

impl Lib<'_> {
    /// The names of the saved designs (for fresh names).
    fn names(&self, shelf: &Shelf) -> Vec<String> {
        shelf.designs.iter().map(|e| e.name.clone()).collect()
    }

    /// Puts another design into the drydock. The one there is saved first when it has unsaved changes (its
    /// name is returned); if that fails, nothing changes.
    fn switch(&mut self, content: &Content, next: PlayerDesign, slot: DesignSlot) -> Result<Option<String>, String> {
        let kept = if self.slot.dirty(&self.design) {
            library::save(&mut self.storage, content, &self.design, &mut self.slot, now_unix())
                .map_err(|e| format!("couldn't save {} first: {e}", self.design.blueprint().name))?;
            Some(self.design.blueprint().name.clone())
        } else {
            None
        };
        self.design.replace(next);
        *self.slot = slot;
        self.cam.reset_view();
        Ok(kept)
    }

    /// The design as it is exported: trimmed, under its id (or a new one from its name).
    fn export(&self, content: &Content) -> Blueprint {
        let id = self.slot.id.clone().unwrap_or_else(|| {
            library::new_id(&self.storage, content, &self.design.blueprint().name)
                .unwrap_or_else(|_| library::slug(&self.design.blueprint().name))
        });
        Blueprint { id, ..self.design.export() }
    }

    /// Saves the design in the drydock; the message says how it went.
    fn save(&mut self, content: &Content) -> (String, Tone) {
        match library::save(&mut self.storage, content, &self.design, &mut self.slot, now_unix()) {
            Ok(id) => {
                let name = &self.design.blueprint().name;
                let problems = validate_design(content, &self.behaviors, &Blueprint { id, ..self.design.export() });
                if problems.is_empty() {
                    (format!("saved  ·  {name}"), Tone::Teal)
                } else {
                    (format!("saved as work in progress  ·  {}", problems_note(&problems)), Tone::Gold)
                }
            }
            Err(e) => (format!("not saved: {e}"), Tone::Coral),
        }
    }

    /// Imports a share code or blueprint text: saved under a new id and opened. A behavior it carries is saved
    /// too (under a new id if its own is taken) and the design names it.
    fn import(&mut self, content: &Content, text: &str) -> Result<String, String> {
        let shared = share::read(text).map_err(|e| e.to_string())?;
        let faction = self.design.blueprint().faction.clone();
        let (mut bp, carried, mut notes) = share::check(content, shared, &faction).map_err(|e| e.to_string())?;
        if bp.summary.trim().is_empty() {
            bp.summary = library::NEW_SUMMARY.into();
        }
        let mut opened = library::open_in_first_frame(content, &bp)?;
        if let Some(b) = carried {
            let (id, note) = self.behaviors.import(&mut self.storage, content, b, now_unix())?;
            opened.edit_header(|h| h.behavior = Some(id));
            notes.push(note);
        }
        let mut slot = DesignSlot::default();
        let id = library::save(&mut self.storage, content, &opened, &mut slot, now_unix()).map_err(|e| e.to_string())?;
        let problems = validate_design(content, &self.behaviors, &Blueprint { id: id.clone(), ..opened.export() });
        if !problems.is_empty() {
            notes.push(problems_note(&problems));
        }
        let kept = self.switch(content, opened, slot)?;
        let mut msg = format!("imported {}", self.design.blueprint().name);
        for n in kept.iter().map(|k| format!("{k} saved")).chain(notes) {
            msg.push_str("  ·  ");
            msg.push_str(&n);
        }
        Ok(msg)
    }
}

/// "1 problem: …" or "3 problems: …" with the first one.
fn problems_note(problems: &[data::Problem]) -> String {
    let s = if problems.len() == 1 { "" } else { "s" };
    format!("{} problem{s}: {}", problems.len(), library::problems_text(&problems[..1.min(problems.len())]))
}

/// The frame's content name for the breadcrumb ("FRIGATE FRAME"), or its size when no content frame has it.
pub(super) fn frame_name(content: &Content, frame: (u32, u32)) -> String {
    match content.frames.iter().find(|f| f.size == frame) {
        Some(f) => format!("{} FRAME", f.name.to_uppercase()),
        None => format!("{}×{} FRAME", frame.0, frame.1),
    }
}

// ---------------------------------------------------------------------------------------------------------
// Start, autosave, leaving

/// At start (the plugin's build, after `DrydockPlugin` put the starter design in): the working design of the
/// last session, else a copy of the starter ship. Needs the `Storage` (`StoragePlugin` comes first).
fn restore(world: &mut World) -> (DesignSlot, Shelf) {
    let mut shelf = Shelf::default();
    let mut storage = world.remove_resource::<Storage>().expect("StoragePlugin is added before DrydockPlugin");
    if let Some(why) = &storage.fallback {
        shelf.say(format!("designs are kept only until the game closes: {why}"), Tone::Gold);
    }
    let restored = library::restore_working(&mut storage, &world.resource::<GameContent>().0);
    world.insert_resource(storage);
    let mut design = world.resource_mut::<PlayerDesign>();
    match restored {
        Some(Ok((next, slot, note))) => {
            design.replace(next);
            if let Some(note) = note {
                shelf.say(format!("the last design came back, but {note}"), Tone::Gold);
            }
            return (slot, shelf);
        }
        Some(Err(e)) => shelf.say(format!("the last design didn't come back: {e}"), Tone::Coral),
        None => {}
    }
    // The first start: the starter ship (`PlayerDesign::starter`) as "<name> copy", not saved yet.
    let name = format!("{} copy", design.blueprint().name);
    let id = library::slug(&name);
    design.edit_header(|h| {
        h.name = name;
        h.id = id;
    });
    (DesignSlot::opened(None, &design), shelf)
}

/// Writes the working design a second after the last change.
fn autosave(time: Res<Time<Real>>, mut saver: ResMut<Saver>, mut lib: Lib, mut shelf: ResMut<Shelf>) {
    let now = time.elapsed_secs_f64();
    saver.timer.note(lib.design.revision(), now);
    // A save, an open or a delete changes the id the working design is saved under: written at once.
    if saver.timer.due(now) || lib.slot.is_changed() {
        write_working(&mut saver, &mut lib, &mut shelf);
    }
}

fn write_working(saver: &mut Saver, lib: &mut Lib, shelf: &mut Shelf) {
    saver.timer.written();
    if let Err(e) = library::write_working(&mut lib.storage, &lib.design, &lib.slot, now_unix()) {
        shelf.say(format!("autosave failed: {e}"), Tone::Coral);
    }
}

/// Leaving the drydock (a test flight, the BEHAVIOR screen): the working design is written now, the screens close
/// and their layer hides at once ([`sync_shelf`] runs only in the drydock; a layer left visible would block the
/// pointer elsewhere), and the clipboard stops offering the code and taking pastes.
fn leave(mut saver: ResMut<Saver>, mut lib: Lib, mut shelf: ResMut<Shelf>, mut clip: ResMut<Clipboard>, mut root: Single<&mut Visibility, With<ShelfRoot>>) {
    if saver.timer.pending() {
        write_working(&mut saver, &mut lib, &mut shelf);
    }
    shelf.page = None;
    shelf.mode = Mode::Browse;
    shelf.code_selected = false;
    shelf.paste_armed = false;
    clip.offer = None;
    clip.accept = false;
    **root = Visibility::Hidden;
}

/// Closing the game (desktop): a waiting autosave is written.
fn save_on_exit(mut exit: MessageReader<AppExit>, mut saver: ResMut<Saver>, mut lib: Lib, mut shelf: ResMut<Shelf>) {
    if exit.read().next().is_some() && saver.timer.pending() {
        write_working(&mut saver, &mut lib, &mut shelf);
    }
}

/// Status messages go out once the design check has seen the current revision (else it would overwrite them).
fn announce(design: Res<PlayerDesign>, mut status: ResMut<DrydockStatus>, mut shelf: ResMut<Shelf>) {
    if status.revision != Some(design.revision()) || shelf.say.is_none() {
        return;
    }
    if let Some((line, tone)) = shelf.say.take() {
        status.line = line;
        status.tone = tone;
    }
}

/// Files dropped onto the window (desktop) are imported; one larger than any design is not read.
#[cfg(not(target_arch = "wasm32"))]
fn dropped_files(mut drops: MessageReader<bevy::window::FileDragAndDrop>, mut shelf: ResMut<Shelf>) {
    for d in drops.read() {
        if let bevy::window::FileDragAndDrop::DroppedFile { path_buf, .. } = d {
            let name = path_buf.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let text = match std::fs::metadata(path_buf) {
                Ok(m) if m.len() > share::MAX_TEXT as u64 => Err(share::ShareError::TooLarge.to_string()),
                _ => std::fs::read_to_string(path_buf).map_err(|e| e.to_string()),
            };
            shelf.incoming.push((name, text));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// The name top left

#[derive(Component)]
pub(super) struct TitleButton;

#[derive(Component)]
pub(super) struct TitleField;

/// "UNSAVED" or "SAVED" after the name.
#[derive(Component)]
pub(super) struct TitleNote;

/// The design's name (the `PanelText::Name` text `panels` writes), the field that replaces it while it is
/// edited, and the saved state after it.
pub(super) fn spawn_title(parent: &mut ChildSpawnerCommands, fonts: &HudFonts, name: Color, dim: Color, edge: Color) {
    parent
        .spawn((TitleButton, Hot::default(), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(12), ..default() }))
        .with_children(|t| {
            t.spawn((PanelText::Name, number(fonts, "", 20.0, name)));
            t.spawn((
                TitleField,
                TextField { placeholder: "Name".into(), max_chars: MAX_NAME, ..default() },
                Hot::default(),
                Node {
                    display: Display::None,
                    width: px(300),
                    height: px(30),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(px(12)),
                    border: UiRect::all(px(1.2)),
                    border_radius: BorderRadius::MAX,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(edge),
            ))
            .with_children(|f| {
                f.spawn((TextFieldText, prose(fonts, "", 16.0, name)));
            });
            t.spawn((TitleNote, label(fonts, "", 8.5, dim)));
        });
}

/// A click on the name opens the field; Enter or a click elsewhere renames, Esc cancels.
fn title(
    mut clicks: MessageReader<UiClick>,
    keys: Res<ButtonInput<KeyCode>>,
    button: Single<Entity, With<TitleButton>>,
    mut field: Single<(&mut TextField, &mut Node), With<TitleField>>,
    mut texts: Query<(&PanelText, &mut Node), Without<TitleField>>,
    mut saver: ResMut<Saver>,
    mut design: ResMut<PlayerDesign>,
) {
    let (f, node) = &mut *field;
    let mut show = |editing: bool, node: &mut Node| {
        node.display = if editing { Display::Flex } else { Display::None };
        for (t, mut n) in &mut texts {
            if *t == PanelText::Name {
                n.display = if editing { Display::None } else { Display::Flex };
            }
        }
    };
    if saver.editing_title && !f.focused {
        saver.editing_title = false;
        let new = f.value.trim().to_string();
        if !keys.just_pressed(KeyCode::Escape) && !new.is_empty() && new != design.blueprint().name {
            design.edit_header(|h| h.name = new);
        }
        show(false, node);
    }
    if clicks.read().any(|c| c.0 == *button) && !saver.editing_title {
        saver.editing_title = true;
        f.value = design.blueprint().name.clone();
        f.focused = true;
        show(true, node);
    }
}

/// "UNSAVED" while the design differs from its saved (or opened) version.
fn sync_title(design: Res<PlayerDesign>, slot: Res<DesignSlot>, mut note: Single<&mut Text, With<TitleNote>>, mut done: Local<bool>) {
    if *done && !design.is_changed() && !slot.is_changed() {
        return;
    }
    *done = true;
    let text = match (slot.id.is_some(), slot.dirty(&design)) {
        (_, true) => "UNSAVED",
        (true, false) => "SAVED",
        (false, false) => "",
    };
    set_text(&mut note, text);
}

// ---------------------------------------------------------------------------------------------------------
// The screens

#[derive(Component)]
pub(super) struct ShelfRoot;

#[derive(Component)]
pub(super) struct PageNode(Page);

/// The action bar's variants (browse pills, the rename field, the delete question).
#[derive(Component)]
pub(super) struct BarNode(u8);

#[derive(Component)]
pub(super) struct RenameField;

/// A list's rows container (0 designs, 1 templates), for the wheel.
#[derive(Component)]
pub(super) struct ListNode(usize);

#[derive(Component)]
pub(super) struct RowNode(usize, usize);

#[derive(Component)]
pub(super) struct EmptyHint;

#[derive(Component)]
pub(super) struct CodeBox;

#[derive(Component)]
pub(super) struct PasteBox;

/// Every button of the screens.
#[derive(Component, Clone, Debug, PartialEq)]
pub(super) enum ShelfButton {
    Close,
    Row(usize, usize),
    Open,
    Duplicate,
    Rename,
    Delete,
    ConfirmDelete,
    KeepDelete,
    New((u32, u32)),
    Frame((u32, u32)),
    ImportFile,
    Download,
    Copy,
    CodeBox,
    PasteBox,
}

/// Every text the screens update.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShelfText {
    Crumb,
    Title,
    Section(usize),
    RowName(usize, usize),
    RowSub(usize, usize),
    RowTag(usize, usize),
    DeleteQuestion,
    FileNote,
    Code,
    CodeNote,
    Paste,
}

fn spawn_shelf(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, content: Res<GameContent>, clear: Res<ClearColor>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let content = &content.0;
    let pill = Pill::new(Tone::Teal);
    let row = |gap: f32| Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), flex_shrink: 0.0, ..default() };
    let head = |text: &str, width: f32| (label(fonts, text, 9.0, c(Tone::Dim, 1.0)), Node { width: px(width), flex_shrink: 0.0, ..default() });
    let frames: Vec<(String, (u32, u32))> = content.frames.iter().map(|f| (f.name.to_uppercase(), f.size)).collect();

    commands
        .spawn((
            ShelfRoot,
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
            // The header: breadcrumb and title, CLOSE.
            root.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(14.0) }).with_children(|h| {
                h.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                    col.spawn((ShelfText::Crumb, label(fonts, "", 10.0, c(Tone::Label, 1.0))));
                    col.spawn((ShelfText::Title, number(fonts, "", 20.0, c(Tone::Number, 1.0))));
                });
                spawn_pill(h, fonts, "CLOSE ESC", 9.5, 30.0, None, pill, ShelfButton::Close);
            });

            // BLUEPRINTS: the two lists, the action bar, NEW and FRAME, IMPORT.
            root.spawn((PageNode(Page::Blueprints), Node { flex_direction: FlexDirection::Column, row_gap: px(16), flex_grow: 1.0, min_height: px(0), ..default() }))
                .with_children(|page| {
                    page.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(48), flex_grow: 1.0, min_height: px(0), ..default() }).with_children(|lists| {
                        for (list, width) in [(0, 600.0), (1, 432.0)] {
                            lists.spawn(Node { width: px(width), flex_direction: FlexDirection::Column, row_gap: px(10), min_height: px(0), ..default() }).with_children(|col| {
                                col.spawn((ShelfText::Section(list), label(fonts, "", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::left(px(12)), ..default() }));
                                col.spawn((
                                    ListNode(list),
                                    Node { flex_direction: FlexDirection::Column, row_gap: px(ROW_GAP), flex_grow: 1.0, min_height: px(0), overflow: Overflow::clip(), ..default() },
                                ))
                                .with_children(|l| {
                                    if list == 0 {
                                        l.spawn((EmptyHint, label(fonts, "NOTHING SAVED YET  ·  SAVE KEEPS THE DESIGN IN THE DRYDOCK (CTRL S)", 8.5, c(Tone::Dim, 1.0)), Node { margin: UiRect::new(px(12), px(0), px(14), px(0)), ..default() }));
                                    }
                                    for k in 0..LIST_ROWS {
                                        spawn_row(l, fonts, &c, list, k);
                                    }
                                });
                            });
                        }
                    });
                    // The action bar: browse / rename / delete.
                    page.spawn(Node { height: px(34), ..row(10.0) }).with_children(|bar| {
                        bar.spawn((BarNode(0), row(8.0))).with_children(|b| {
                            for (text, button) in [("OPEN", ShelfButton::Open), ("DUPLICATE", ShelfButton::Duplicate), ("RENAME", ShelfButton::Rename), ("DELETE", ShelfButton::Delete)] {
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
                                f.spawn((TextFieldText, prose(fonts, "", 13.0, c(Tone::Number, 1.0))));
                            });
                            b.spawn(label(fonts, "ENTER RENAMES  ·  ESC CANCELS", 8.5, c(Tone::Dim, 1.0)));
                        });
                        bar.spawn((BarNode(2), Node { display: Display::None, ..row(10.0) })).with_children(|b| {
                            b.spawn((ShelfText::DeleteQuestion, label(fonts, "", 9.5, c(Tone::Coral, 1.0)), Node { margin: UiRect::right(px(8)), ..default() }));
                            spawn_pill(b, fonts, "DELETE", 9.5, 30.0, None, Pill { active: true, ..Pill::new(Tone::Coral) }, ShelfButton::ConfirmDelete);
                            spawn_pill(b, fonts, "KEEP", 9.5, 30.0, None, pill, ShelfButton::KeepDelete);
                        });
                    });
                    page.spawn((Node { height: px(1), flex_shrink: 0.0, ..default() }, BackgroundColor(c(Tone::Label, 0.14))));
                    // NEW in a frame; the frame of the design in the drydock.
                    page.spawn(row(8.0)).with_children(|r| {
                        r.spawn(head("NEW", 80.0));
                        for (name, size) in &frames {
                            spawn_pill(r, fonts, name, 9.5, 30.0, None, pill, ShelfButton::New(*size));
                        }
                        r.spawn(Node { width: px(40), ..default() });
                        r.spawn(head("FRAME", 70.0));
                        for (name, size) in &frames {
                            let text = format!("{name} {}×{}", size.0, size.1);
                            spawn_pill(r, fonts, &text, 9.5, 30.0, None, pill, ShelfButton::Frame(*size));
                        }
                    });
                    // IMPORT: a file, or paste a code.
                    page.spawn(row(8.0)).with_children(|r| {
                        r.spawn(head("IMPORT", 80.0));
                        spawn_pill(r, fonts, ".RON FILE", 9.5, 30.0, None, pill, ShelfButton::ImportFile);
                        r.spawn((
                            PasteBox,
                            ShelfButton::PasteBox,
                            Hot::default(),
                            Node {
                                width: px(520),
                                height: px(32),
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(px(14)),
                                border: UiRect::all(px(1.2)),
                                border_radius: BorderRadius::MAX,
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BorderColor::all(c(Tone::Label, 0.3)),
                            BackgroundColor(Color::NONE),
                        ))
                        .with_children(|b| {
                            b.spawn((ShelfText::Paste, tracked(fonts, "", 8.5, c(Tone::Dim, 1.0), 0.2)));
                        });
                    });
                });

            // EXPORT: the file and the share code.
            root.spawn((PageNode(Page::Export), Node { display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(22), ..default() }))
                .with_children(|page| {
                    page.spawn(row(12.0)).with_children(|r| {
                        r.spawn(head("FILE", 110.0));
                        let text = if cfg!(target_arch = "wasm32") { "DOWNLOAD .RON" } else { "WRITE .RON FILE" };
                        spawn_pill(r, fonts, text, 9.5, 30.0, None, pill, ShelfButton::Download);
                        r.spawn((ShelfText::FileNote, label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                    });
                    page.spawn(Node { align_items: AlignItems::FlexStart, ..row(12.0) }).with_children(|r| {
                        r.spawn((label(fonts, "SHARE CODE", 9.0, c(Tone::Dim, 1.0)), Node { width: px(110), flex_shrink: 0.0, margin: UiRect::top(px(12)), ..default() }));
                        r.spawn((
                            CodeBox,
                            ShelfButton::CodeBox,
                            Hot::default(),
                            Node { width: px(760), padding: UiRect::axes(px(16), px(12)), border: UiRect::all(px(1.2)), border_radius: BorderRadius::all(px(16)), ..default() },
                            BorderColor::all(c(Tone::Label, 0.3)),
                            BackgroundColor(Color::NONE),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                ShelfText::Code,
                                prose(fonts, "", 11.0, c(Tone::Number, 0.9)),
                                TextLayout::new(Justify::Left, LineBreak::AnyCharacter),
                                Node { width: percent(100), ..default() },
                            ));
                        });
                        spawn_pill(r, fonts, "COPY", 9.5, 30.0, None, pill, ShelfButton::Copy);
                    });
                    page.spawn((ShelfText::CodeNote, label(fonts, "", 8.5, c(Tone::Dim, 1.0)), Node { margin: UiRect::left(px(122)), ..default() }));
                });
        });
}

fn spawn_row(l: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, list: usize, k: usize) {
    l.spawn((
        RowNode(list, k),
        ShelfButton::Row(list, k),
        Pill { bare: true, ..Pill::new(Tone::Teal) },
        Hot::default(),
        Node {
            display: Display::None,
            height: px(ROW_H),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::horizontal(px(16)),
            border: UiRect::all(px(1.2)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    ))
    .with_children(|r| {
        r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|t| {
            t.spawn((ShelfText::RowName(list, k), prose(fonts, "", 14.0, c(Tone::Number, 1.0))));
            t.spawn((ShelfText::RowSub(list, k), tracked(fonts, "", 8.5, c(Tone::Dim, 1.0), 0.2)));
        });
        r.spawn((ShelfText::RowTag(list, k), label(fonts, "", 8.5, c(Tone::Teal, 1.0))));
    });
}

/// Opens a page, or closes it when it is open.
fn toggle(shelf: &mut Shelf, page: Page, lib: &Lib, content: &Content) {
    if shelf.page == Some(page) {
        shelf.page = None;
        return;
    }
    shelf.page = Some(page);
    shelf.mode = Mode::Browse;
    shelf.code_selected = false;
    shelf.paste_armed = false;
    match page {
        Page::Blueprints => {
            refresh(shelf, lib, content);
            shelf.pick = lib.slot.id.clone().map(Pick::Design);
        }
        Page::Export => shelf.export = Export::of(lib, content),
    }
}

/// Re-reads the saved designs.
fn refresh(shelf: &mut Shelf, lib: &Lib, content: &Content) {
    shelf.now = now_unix();
    shelf.templates = content.ships.iter().map(|s| s.id.clone()).collect();
    match library::list_designs(&lib.storage, content) {
        Ok(list) => shelf.designs = list,
        Err(e) => shelf.say(format!("can't list the designs: {e}"), Tone::Coral),
    }
    if let Some(Pick::Design(id)) = &shelf.pick
        && shelf.entry(id).is_none()
    {
        shelf.pick = None;
    }
}

/// The drydock's SAVE / EXPORT / BLUEPRINTS, the screens' buttons, keys, pasted text and picked files.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply(
    mut actions: MessageReader<DrydockAction>,
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(&ShelfButton, Option<&Pill>)>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    boxes: Query<(&Hot, Has<CodeBox>), Or<(With<CodeBox>, With<PasteBox>)>>,
    mut rename: Single<&mut TextField, With<RenameField>>,
    fields: Query<&TextField, Without<RenameField>>,
    mut clip: ResMut<Clipboard>,
    mut saver: ResMut<Saver>,
    mut shelf: ResMut<Shelf>,
    mut lib: Lib,
    content: Res<GameContent>,
) {
    let content = &content.0;
    // A rename ends when its field lets go of the keyboard: Esc cancels, anything else renames.
    if let Mode::Rename(id) = shelf.mode.clone()
        && !rename.focused
    {
        shelf.mode = Mode::Browse;
        let name = rename.value.trim().to_string();
        if !keys.just_pressed(KeyCode::Escape) && !name.is_empty() {
            rename_design(&mut shelf, &mut lib, content, &id, &name);
        }
    }
    let field_keys = saver.field_had_keys;
    saver.field_had_keys = rename.focused || fields.iter().any(|f| f.focused);

    let mut todo: Vec<ShelfButton> = Vec::new();
    for a in actions.read() {
        match a {
            DrydockAction::Save => {
                let (line, tone) = lib.save(content);
                shelf.say(line, tone);
                if shelf.page == Some(Page::Blueprints) {
                    refresh(&mut shelf, &lib, content);
                }
            }
            DrydockAction::Blueprints => toggle(&mut shelf, Page::Blueprints, &lib, content),
            DrydockAction::Export => toggle(&mut shelf, Page::Export, &lib, content),
            _ => {}
        }
    }
    for c in clicks.read() {
        if let Ok((b, pill)) = buttons.get(c.0)
            && !pill.is_some_and(|p| p.disabled)
        {
            todo.push(b.clone());
        }
    }
    #[cfg(target_arch = "wasm32")]
    shelf.incoming.extend(share::take_picked());
    for (file, text) in std::mem::take(&mut shelf.incoming) {
        match text {
            Ok(text) => import(&mut shelf, &mut lib, content, &text, &format!("{file}: ")),
            Err(e) => shelf.say(format!("{file} doesn't read: {e}"), Tone::Coral),
        }
    }
    if let Some(done) = clip.copied.take() {
        match done {
            Ok(()) => shelf.say("share code copied", Tone::Teal),
            Err(e) => shelf.say(format!("copy failed ({e}): click the code and press ctrl+c"), Tone::Coral),
        }
    }
    let Some(page) = shelf.page else {
        clip.offer = None;
        clip.accept = false;
        return;
    };
    // The design changed under the export page (undo, redo): the file and the code follow.
    if page == Page::Export && shelf.export.revision != lib.design.revision() {
        shelf.export = Export::of(&lib, content);
    }
    // A press anywhere else lets go of the code box and the paste box.
    if mouse.just_pressed(MouseButton::Left) {
        for (hot, code) in &boxes {
            if !hot.hovered {
                if code {
                    shelf.code_selected = false;
                } else {
                    shelf.paste_armed = false;
                }
            }
        }
    }
    if keys.just_pressed(KeyCode::Escape) && !field_keys {
        if shelf.code_selected || shelf.paste_armed {
            shelf.code_selected = false;
            shelf.paste_armed = false;
        } else if shelf.mode != Mode::Browse {
            shelf.mode = Mode::Browse;
        } else {
            shelf.page = None;
        }
    }
    for b in todo {
        press(b, &mut shelf, &mut lib, content, &mut rename, &mut clip);
    }
    for text in clip.take_pasted() {
        if shelf.page == Some(Page::Blueprints) && shelf.paste_armed {
            shelf.paste_armed = false;
            import(&mut shelf, &mut lib, content, &text, "");
        }
    }
    clip.offer = (page == Page::Export && shelf.code_selected).then(|| shelf.export.code.clone());
    clip.accept = page == Page::Blueprints && shelf.paste_armed;
}

fn import(shelf: &mut Shelf, lib: &mut Lib, content: &Content, text: &str, from: &str) {
    match lib.import(content, text) {
        Ok(msg) => {
            shelf.say(msg, Tone::Teal);
            refresh(shelf, lib, content);
            shelf.pick = lib.slot.id.clone().map(Pick::Design);
        }
        Err(e) => shelf.say(format!("{from}not imported: {e}"), Tone::Coral),
    }
}

fn rename_design(shelf: &mut Shelf, lib: &mut Lib, content: &Content, id: &str, name: &str) {
    if let Err(e) = library::rename(&mut lib.storage, id, name, now_unix()) {
        shelf.say(format!("not renamed: {e}"), Tone::Coral);
        return;
    }
    if lib.slot.id.as_deref() == Some(id) {
        // The design in the drydock follows; it stays unsaved only if it had other changes.
        let clean = !lib.slot.dirty(&lib.design);
        lib.design.edit_header(|h| h.name = name.to_string());
        if clean {
            *lib.slot = DesignSlot::opened(Some(id.to_string()), &lib.design);
        }
    }
    shelf.say(format!("renamed to {name}"), Tone::Teal);
    refresh(shelf, lib, content);
}

/// One button of the screens.
fn press(b: ShelfButton, shelf: &mut Shelf, lib: &mut Lib, content: &Content, rename: &mut TextField, clip: &mut Clipboard) {
    match b {
        ShelfButton::Close => shelf.page = None,
        ShelfButton::Row(list, k) => {
            let i = shelf.offset[list] + k;
            shelf.pick = match list {
                0 => shelf.designs.get(i).map(|e| Pick::Design(e.id.clone())),
                _ => shelf.templates.get(i).map(|id| Pick::Template(id.clone())),
            };
            shelf.mode = Mode::Browse;
        }
        ShelfButton::Open => {
            let opened = match shelf.pick.clone() {
                Some(Pick::Design(id)) if lib.slot.id.as_deref() == Some(id.as_str()) => {
                    shelf.say("it is in the drydock already", Tone::Label);
                    return;
                }
                Some(Pick::Design(id)) => library::open_saved(&lib.storage, content, &id).map(|d| (DesignSlot::opened(Some(id), &d), d)),
                Some(Pick::Template(id)) => {
                    library::open_template(content, &id, &lib.names(shelf)).map(|d| (DesignSlot::opened(None, &d), d))
                }
                None => return,
            };
            match opened.and_then(|(slot, d)| lib.switch(content, d, slot)) {
                Ok(kept) => {
                    let name = lib.design.blueprint().name.clone();
                    let kept = kept.map_or(String::new(), |k| format!("{k} saved  ·  "));
                    shelf.say(format!("{kept}opened {name}"), Tone::Teal);
                    shelf.page = None;
                }
                Err(e) => shelf.say(format!("not opened: {e}"), Tone::Coral),
            }
        }
        ShelfButton::Duplicate => {
            let names = lib.names(shelf);
            let now = now_unix();
            let made = match shelf.pick.clone() {
                Some(Pick::Design(id)) => library::duplicate(&mut lib.storage, content, &id, &names, now),
                Some(Pick::Template(id)) => library::open_template(content, &id, &names).and_then(|copy| {
                    library::save(&mut lib.storage, content, &copy, &mut DesignSlot::default(), now).map_err(|e| e.to_string())
                }),
                None => return,
            };
            match made {
                Ok(id) => {
                    refresh(shelf, lib, content);
                    let name = shelf.entry(&id).map_or(id.clone(), |e| e.name.clone());
                    shelf.say(format!("saved a copy: {name}"), Tone::Teal);
                    shelf.offset[0] = 0;
                    shelf.pick = Some(Pick::Design(id));
                }
                Err(e) => shelf.say(format!("no copy made: {e}"), Tone::Coral),
            }
        }
        ShelfButton::Rename => {
            if let Some(Pick::Design(id)) = &shelf.pick
                && let Some(e) = shelf.entry(id)
            {
                rename.value = e.name.clone();
                rename.focused = true;
                shelf.mode = Mode::Rename(id.clone());
            }
        }
        ShelfButton::Delete => {
            if let Some(Pick::Design(id)) = &shelf.pick {
                shelf.mode = Mode::Delete(id.clone());
            }
        }
        ShelfButton::ConfirmDelete => {
            let Mode::Delete(id) = std::mem::take(&mut shelf.mode) else { return };
            match lib.storage.delete::<library::DesignDoc>(&id) {
                Ok(()) => {
                    let name = shelf.entry(&id).map_or(id.clone(), |e| e.name.clone());
                    if lib.slot.id.as_deref() == Some(id.as_str()) {
                        // The design stays in the drydock, unsaved; it is not saved again unless it changes.
                        *lib.slot = DesignSlot::opened(None, &lib.design);
                    }
                    shelf.say(format!("deleted {name}"), Tone::Teal);
                    refresh(shelf, lib, content);
                }
                Err(e) => shelf.say(format!("not deleted: {e}"), Tone::Coral),
            }
        }
        ShelfButton::KeepDelete => shelf.mode = Mode::Browse,
        ShelfButton::New(frame) => {
            let made = library::new_design(content, frame, lib.design.blueprint(), &lib.names(shelf)).and_then(|d| {
                let slot = DesignSlot::opened(None, &d);
                lib.switch(content, d, slot)
            });
            match made {
                Ok(kept) => {
                    let kept = kept.map_or(String::new(), |k| format!("{k} saved  ·  "));
                    shelf.say(format!("{kept}new design in the {}", frame_name(content, frame)), Tone::Teal);
                    shelf.page = None;
                }
                Err(e) => shelf.say(format!("no new design: {e}"), Tone::Coral),
            }
        }
        ShelfButton::Frame(frame) => {
            if lib.design.frame() == frame {
                return;
            }
            let size = lib.design.export().size;
            // The camera fits the new frame by itself (`view::camera` follows the frame, undo included).
            match lib.design.edit(|d| d.set_frame(frame)) {
                Ok(_) => {
                    shelf.say(format!("moved to the {}", frame_name(content, frame)), Tone::Teal);
                }
                Err(_) => shelf.say(
                    format!("doesn't fit the {} ({}×{}): the design is {}×{}", frame_name(content, frame), frame.0, frame.1, size.0, size.1),
                    Tone::Coral,
                ),
            }
        }
        ShelfButton::ImportFile => {
            #[cfg(target_arch = "wasm32")]
            {
                if let Err(e) = share::pick_file() {
                    shelf.say(format!("no file chooser: {e}"), Tone::Coral);
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                shelf.say("drop a .ron file onto the window to import it", Tone::Label);
            }
        }
        ShelfButton::Download => match share::write_export(&shelf.export.file, &shelf.export.ron) {
            Ok(where_) => shelf.say(where_, Tone::Teal),
            Err(e) => shelf.say(format!("not exported: {e}"), Tone::Coral),
        },
        ShelfButton::Copy => {
            shelf.code_selected = true;
            clip.copy = Some(shelf.export.code.clone());
        }
        ShelfButton::CodeBox => shelf.code_selected = true,
        ShelfButton::PasteBox => shelf.paste_armed = true,
    }
}

/// The wheel over a list scrolls it.
fn scroll_lists(
    window: Single<&Window, With<PrimaryWindow>>,
    scroll: Res<AccumulatedMouseScroll>,
    lists: Query<(&ListNode, &ComputedNode, &UiGlobalTransform)>,
    mut shelf: ResMut<Shelf>,
) {
    if shelf.page != Some(Page::Blueprints) || scroll.delta.y == 0.0 {
        return;
    }
    let Some(cursor) = window.physical_cursor_position() else { return };
    for (list, node, tf) in &lists {
        if !node.contains_point(*tf, cursor) {
            continue;
        }
        let len = if list.0 == 0 { shelf.designs.len() } else { shelf.templates.len() };
        let steps = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        let max = len.saturating_sub(LIST_ROWS) as f32;
        let offset = (shelf.offset[list.0] as f32 - steps.signum() * steps.abs().max(1.0).round()).clamp(0.0, max) as usize;
        if offset != shelf.offset[list.0] {
            shelf.offset[list.0] = offset;
        }
    }
}

/// The screens follow [`Shelf`]: visibility, rows, pills, texts, the boxes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_shelf(
    shelf: Res<Shelf>,
    lib_slot: Res<DesignSlot>,
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    style: Res<Style>,
    mut root: Single<&mut Visibility, With<ShelfRoot>>,
    mut nodes: Query<(&mut Node, Option<&PageNode>, Option<&BarNode>, Option<&RowNode>, Has<EmptyHint>), Without<TextField>>,
    mut pills: Query<(&ShelfButton, &mut Pill)>,
    mut boxes: Query<(&mut BorderColor, &mut BackgroundColor, Has<CodeBox>, &Hot), Or<(With<CodeBox>, With<PasteBox>)>>,
    mut texts: Query<(&ShelfText, &mut Text, &mut TextColor)>,
) {
    let want = if shelf.page.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    if **root != want {
        **root = want;
    }
    let Some(page) = shelf.page else { return };
    let ui = &style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(ui), a);
    for (mut border, mut bg, code, hot) in &mut boxes {
        let on = if code { shelf.code_selected } else { shelf.paste_armed };
        let (b, f) = match (on, hot.hovered) {
            (true, _) => (c(Tone::Teal, 0.8), c(Tone::Teal, 0.07)),
            (false, true) => (c(Tone::Label, 0.6), Color::NONE),
            (false, false) => (c(Tone::Label, 0.3), Color::NONE),
        };
        if border.top != b {
            *border = BorderColor::all(b);
        }
        if bg.0 != f {
            bg.0 = f;
        }
    }
    if !shelf.is_changed() && !design.is_changed() && !lib_slot.is_changed() {
        return;
    }
    let content = &content.0;
    let bar = match shelf.mode {
        Mode::Browse => 0,
        Mode::Rename(_) => 1,
        Mode::Delete(_) => 2,
    };
    let row_len = [shelf.designs.len(), shelf.templates.len()];
    for (mut n, p, b, r, empty) in &mut nodes {
        let show = match (p, b, r) {
            (Some(p), ..) => p.0 == page,
            (_, Some(b), _) => b.0 == bar,
            (_, _, Some(r)) => shelf.offset[r.0] + r.1 < row_len[r.0],
            _ if empty => shelf.designs.is_empty(),
            _ => continue,
        };
        let d = if show { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    let picked = |list: usize, k: usize| {
        let i = shelf.offset[list] + k;
        match (&shelf.pick, list) {
            (Some(Pick::Design(id)), 0) => shelf.designs.get(i).is_some_and(|e| e.id == *id),
            (Some(Pick::Template(id)), 1) => shelf.templates.get(i) == Some(id),
            _ => false,
        }
    };
    let readable = matches!(&shelf.pick, Some(Pick::Design(id)) if shelf.entry(id).is_some_and(|e| e.problem.is_none()));
    let is_design = matches!(shelf.pick, Some(Pick::Design(_)));
    for (b, mut pill) in &mut pills {
        let (active, disabled) = match b {
            ShelfButton::Row(list, k) => (picked(*list, *k), false),
            ShelfButton::Open | ShelfButton::Duplicate => (false, !(readable || matches!(shelf.pick, Some(Pick::Template(_))))),
            ShelfButton::Rename => (false, !readable),
            ShelfButton::Delete => (false, !is_design),
            ShelfButton::Frame(f) => (design.frame() == *f, false),
            ShelfButton::ConfirmDelete => (true, false),
            ShelfButton::Copy => (false, shelf.export.code.is_empty()),
            _ => (false, false),
        };
        if pill.active != active || pill.disabled != disabled {
            pill.active = active;
            pill.disabled = disabled;
        }
    }
    let working = lib_slot.id.as_deref();
    let working_dirty = lib_slot.dirty(&design);
    let mut s = String::new();
    for (t, mut text, mut color) in &mut texts {
        s.clear();
        let tone = match *t {
            ShelfText::Crumb => {
                s.push_str(if page == Page::Blueprints { "SHIPYARD  ›  BLUEPRINTS" } else { "SHIPYARD  ›  EXPORT" });
                Tone::Label
            }
            ShelfText::Title => {
                s.push_str(if page == Page::Blueprints { "Your designs and templates" } else { design.blueprint().name.as_str() });
                Tone::Number
            }
            ShelfText::Section(0) => {
                s.push_str(&format!("YOUR DESIGNS  ·  {}", shelf.designs.len()));
                Tone::Dim
            }
            ShelfText::Section(_) => {
                s.push_str(&format!("TEMPLATES  ·  {}  ·  OPEN AS A COPY", shelf.templates.len()));
                Tone::Dim
            }
            ShelfText::RowName(0, k) => {
                s.push_str(shelf.designs.get(shelf.offset[0] + k).map_or("", |e| e.name.as_str()));
                Tone::Number
            }
            ShelfText::RowName(_, k) => {
                let ship = shelf.templates.get(shelf.offset[1] + k).and_then(|id| content.ships.get(id));
                s.push_str(ship.map_or("", |b| b.name.as_str()));
                Tone::Number
            }
            ShelfText::RowSub(0, k) => match shelf.designs.get(shelf.offset[0] + k) {
                Some(Entry { problem: Some(p), .. }) => {
                    s.push_str(&format!("UNREADABLE  ·  {}", p.to_uppercase()));
                    Tone::Coral
                }
                Some(e) => {
                    let frame = frame_name(content, e.frame).replace(" FRAME", "");
                    s.push_str(&format!("{frame}  ·  {}×{}  ·  {} PARTS", e.size.0, e.size.1, e.parts));
                    if let Some(t) = e.saved {
                        s.push_str(&format!("  ·  {}", library::ago(shelf.now, t)));
                    }
                    Tone::Dim
                }
                None => Tone::Dim,
            },
            ShelfText::RowSub(_, k) => {
                if let Some(b) = shelf.templates.get(shelf.offset[1] + k).and_then(|id| content.ships.get(id)) {
                    let faction = content.factions.get(&b.faction).map_or(String::new(), |f| f.name.to_uppercase());
                    let (w, h) = library::parts_size(content, b);
                    s.push_str(&format!("{w}×{h}  ·  {} PARTS  ·  {faction}", b.parts.len()));
                }
                Tone::Dim
            }
            ShelfText::RowTag(list, k) => {
                if list == 0 && shelf.designs.get(shelf.offset[0] + k).is_some_and(|e| Some(e.id.as_str()) == working) {
                    s.push_str(if working_dirty { "IN THE DRYDOCK  ·  UNSAVED" } else { "IN THE DRYDOCK" });
                }
                Tone::Teal
            }
            ShelfText::DeleteQuestion => {
                if let Mode::Delete(id) = &shelf.mode {
                    let name = shelf.entry(id).map_or(id.as_str(), |e| e.name.as_str());
                    s.push_str(&format!("DELETE {}? THERE IS NO UNDO", name.to_uppercase()));
                }
                Tone::Coral
            }
            ShelfText::FileNote => {
                s.push_str(&format!("{}  ·  THE CONTENT SHIP FORMAT: IT GOES INTO A PACK'S SHIPS FOLDER AS IS", shelf.export.file.to_uppercase()));
                Tone::Dim
            }
            ShelfText::Code => {
                s.push_str(&shelf.export.code);
                Tone::Number
            }
            ShelfText::CodeNote => {
                s.push_str(&format!(
                    "CLICK THE CODE, THEN CTRL+C, OR COPY  ·  IMPORT IT IN BLUEPRINTS  ·  {} CHARACTERS  ·  MADE WITH {}",
                    shelf.export.code.len(),
                    shelf.export.made_with
                ));
                Tone::Dim
            }
            ShelfText::Paste => {
                if shelf.paste_armed {
                    s.push_str("PASTE NOW: CTRL+V");
                    Tone::Teal
                } else {
                    s.push_str("CLICK HERE, THEN PASTE A SHARE CODE OR BLUEPRINT TEXT (CTRL+V)");
                    Tone::Dim
                }
            }
        };
        set_text(&mut text, &s);
        set_color(&mut color, c(tone, if matches!(t, ShelfText::Code) { 0.9 } else { 1.0 }));
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::storage::MemoryBackend;

    /// A world with what the screens' systems need (no window, no fonts).
    fn world() -> World {
        let content = data::base_content();
        let mut world = World::new();
        world.insert_resource(PlayerDesign::starter(&content));
        world.insert_resource(GameContent(content));
        world.insert_resource(Storage::new(MemoryBackend::default()));
        world.insert_resource(DockCamera::new(1.0));
        world.init_resource::<PlayerBehaviors>();
        world.init_resource::<DesignSlot>();
        world.init_resource::<Saver>();
        world.init_resource::<Shelf>();
        world.init_resource::<Clipboard>();
        world
    }

    /// Leaving the drydock with BLUEPRINTS open (BEHAVIOR or TEST-FLY in the bottom bar, T) hides the layer and lets
    /// go of the clipboard: the layer would block the pointer on the next screen, the clipboard would swallow
    /// Ctrl+C / Ctrl+V in flight.
    #[test]
    fn leaving_the_drydock_hides_the_screens_and_lets_go_of_the_clipboard() {
        let mut world = world();
        {
            let mut shelf = world.resource_mut::<Shelf>();
            shelf.page = Some(Page::Blueprints);
            shelf.paste_armed = true;
            shelf.code_selected = true;
        }
        world.resource_mut::<Clipboard>().accept = true;
        world.resource_mut::<Clipboard>().offer = Some("SGD2:x".into());
        let root = world.spawn((ShelfRoot, Visibility::Inherited)).id();
        world.run_system_once(leave).unwrap();
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden));
        let shelf = world.resource::<Shelf>();
        assert!(shelf.page.is_none() && !shelf.paste_armed && !shelf.code_selected);
        let clip = world.resource::<Clipboard>();
        assert!(clip.offer.is_none() && !clip.accept);
    }
}
