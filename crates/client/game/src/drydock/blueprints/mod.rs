//! Saving, the BLUEPRINTS and EXPORT screens, the editable design name and the autosave (written 13 Oct 2026;
//! the storage is `world::storage`, the design library `library.rs`, share codes and files `share.rs`).
//!
//! * **SAVE** (Ctrl+S, the button) writes the design under its id (`library::save`); a design with problems
//!   still saves, and the status line names the first problem.
//! * **Autosave**: the working design (with its frame and the id it is saved under) is written about a second
//!   after the last change and when a test flight starts, and comes back at the next start. The first start, and
//!   every new game, is a copy of the career's start ship ("Rim skiff copy"; a new game saves the last design's
//!   unsaved changes first).
//! * **BLUEPRINTS** floats over the screen above the bottom bar: your designs (name, frame, size, parts, when
//!   saved) and the content ships as templates; OPEN, DUPLICATE, RENAME, DELETE (with a confirm step) on the
//!   picked one; NEW in a frame; FRAME of the design in the drydock (refused with a message when it doesn't
//!   fit; frames the career hasn't unlocked are dimmed with a lock); IMPORT a `.ron` file (web: a file chooser; desktop: drop it onto the window) or paste a share code.
//!   Opening another design replaces the one in the drydock; when that one has unsaved changes, it is saved
//!   first.
//! * **EXPORT** shows the design's `.ron` file (download on the web, a file in the data directory on desktop)
//!   and its share code in a selectable box (click it, then Ctrl+C) with COPY.
//! * **The name** top left: a click turns it into a text field; Enter (or a click elsewhere) renames the design
//!   (`PlayerDesign::edit_header`), Esc keeps the old name.
//! * **The ⋯ menu** of the bottom bar (GDR-0082) offers shortcuts to the screens' rarer actions, carried out as
//!   there: a new design in this frame, the frame, copying the share code, the `.ron` file, importing one.

mod autosave;
mod handlers;
mod screens;
mod sync;
mod title;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::{Justify, LineBreak};
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use data::{Blueprint, Content};
use kit::clipboard::Clipboard;
use kit::style::{Style, ui_color};
use kit::ui_icons::{Glyph, UiIcon};
use kit::ui_kit::{
    HudFonts, Hot, Pill, PillLock, ScrollRows, Scrollbar, TextField, Tip, Tone, UiBlock, UiClick, field_text, label, name_in_place, number, prose,
    set_color, set_text, spawn_pill, sub_tone, tracked,
};
use world::career::{Granted, Note, PlayerCareer};
use world::player::PlayerDesign;
use world::player::behaviors::{PlayerBehaviors, validate_design};
use world::player::library::{self, Autosave, DesignDoc, DesignSlot, Entry, MAX_NAME};
use world::sim_bridge::GameContent;
use world::storage::{Storage, now_unix};

use super::actions::{DrydockAction, Rare};
use super::panels::{PanelText, UnderShelf};
use super::report::DrydockReport;
use super::share;
use super::state::DrydockStatus;
use super::view::DockCamera;
use super::{DockSet, stats};

use autosave::{announce, autosave, leave, restore, save_on_exit};
#[cfg(not(target_arch = "wasm32"))]
use autosave::dropped_files;
use screens::{
    BarNode, CodeBox, EmptyHint, ListNode, PageNode, PasteBox, PreviewCard, PreviewColumn, PreviewShip, PreviewStat, RenameField, RowNode, ShelfButton, ShelfRoot,
    ShelfText, spawn_shelf,
};
use sync::{scroll_lists, sync_preview, sync_shelf};
use title::{sync_title, title};
pub(in crate::drydock) use handlers::apply;
pub(in crate::drydock) use title::spawn_title;

/// Rows per list (designs, templates); longer lists scroll (by pixels: the pool has one row more, for a row cut at
/// an edge).
const LIST_ROWS: usize = 6;
const ROW_H: f32 = 46.0;
const ROW_GAP: f32 = 4.0;
/// A list's height: [`LIST_ROWS`] rows.
const LIST_H: f32 = LIST_ROWS as f32 * (ROW_H + ROW_GAP) - ROW_GAP;
/// The width of the heads before the NEW, FRAME and IMPORT rows (their pills line up).
const HEAD_W: f32 = 176.0;
/// The lists' widths, the preview's column between them and the gaps (1080 px with the screen's margins in 1280).
const DESIGNS_W: f32 = 480.0;
const TEMPLATES_W: f32 = 344.0;
const PREVIEW_W: f32 = 208.0;
const LISTS_GAP: f32 = 24.0;
/// The preview's ship drawing's height (it is fitted into the card's width and this) and the card's height (fixed,
/// so the card is placed before its first layout: the ship, its four numbers and the padding).
const PREVIEW_SHIP_H: f32 = 96.0;
const PREVIEW_CARD_H: f32 = 240.0;

pub(super) struct BlueprintsPlugin;

impl Plugin for BlueprintsPlugin {
    fn build(&self, app: &mut App) {
        // Before any system runs, so the first flight (a plain sandbox start) flies the restored design too.
        let (slot, shelf) = restore(app.world_mut());
        app.insert_resource(slot)
            .insert_resource(shelf)
            .init_resource::<Saver>()
            .add_systems(Startup, spawn_shelf.after(world::flight_hud::spawn_hud))
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
            .add_systems(Update, (scroll_lists, sync_shelf, sync_preview, sync_title).in_set(DockSet::Show))
            .add_systems(OnExit(world::mode::AppMode::Drydock), leave)
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
    /// The first row in each list's pool of row nodes.
    offset: [usize; 2],
    /// A list to show from its top again (a copy saved there).
    top: [bool; 2],
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
    /// BLUEPRINTS or EXPORT covers the design.
    pub(super) fn is_open(&self) -> bool {
        self.page.is_some()
    }

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
    /// The design revision last kept for the page going away (`Storage::keep_on_leave`).
    kept: Option<u64>,
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
        let kept = self.keep_current(content)?;
        self.put(next, slot);
        Ok(kept)
    }

    /// Saves the design in the drydock when it has unsaved changes, before something replaces it; returns its name
    /// when it was saved.
    fn keep_current(&mut self, content: &Content) -> Result<Option<String>, String> {
        if !self.slot.dirty(&self.design) {
            return Ok(None);
        }
        library::save(&mut self.storage, content, &self.design, &mut self.slot, now_unix())
            .map_err(|e| format!("couldn't save {} first: {e}", self.design.blueprint().name))?;
        Ok(Some(self.design.blueprint().name.clone()))
    }

    /// Replaces the design in the drydock (after [`Lib::keep_current`]).
    fn put(&mut self, next: PlayerDesign, slot: DesignSlot) {
        self.design.replace(next);
        *self.slot = slot;
        self.cam.reset_view();
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
    /// too (under a new id if its own is taken) and the design names it. The design in the drydock is saved first:
    /// if that fails, nothing of the import is written (a retry would have made second copies).
    fn import(&mut self, content: &Content, text: &str) -> Result<String, String> {
        let shared = share::read(text).map_err(|e| e.to_string())?;
        let faction = self.design.blueprint().faction.clone();
        let (mut bp, carried, mut notes) = share::check(content, shared, &faction).map_err(|e| e.to_string())?;
        if bp.summary.trim().is_empty() {
            bp.summary = library::NEW_SUMMARY.into();
        }
        let mut opened = library::open_in_first_frame(content, &bp)?;
        let kept = self.keep_current(content)?;
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
        self.put(opened, slot);
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

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use super::title::title_tag;
    use world::storage::MemoryBackend;

    /// A world with what the screens' systems need (no window, no fonts).
    fn world() -> World {
        let content = data::base_content();
        let mut world = World::new();
        let design = PlayerDesign::starter(&content, "halvex_runner");
        world.insert_resource(super::super::state::DrydockUi::new(&content, &design, &data::career::Unlocks::all(&content)));
        world.insert_resource(design);
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

    /// Storage that refuses to write one design (nearly full: the import fits, the design in the drydock not).
    struct Refusing(MemoryBackend, &'static str);

    impl world::storage::Backend for Refusing {
        fn list(&self, namespace: &str) -> Result<Vec<String>, world::storage::StorageError> {
            self.0.list(namespace)
        }
        fn read(&self, namespace: &str, name: &str) -> Result<Option<String>, world::storage::StorageError> {
            self.0.read(namespace, name)
        }
        fn write(&mut self, namespace: &str, name: &str, text: &str) -> Result<(), world::storage::StorageError> {
            if name == self.1 {
                return Err(world::storage::StorageError::Full);
            }
            self.0.write(namespace, name, text)
        }
        fn delete(&mut self, namespace: &str, name: &str) -> Result<(), world::storage::StorageError> {
            self.0.delete(namespace, name)
        }
        fn location(&self) -> String {
            "refusing".into()
        }
    }

    /// An import whose save-first of the design in the drydock fails writes nothing (before, the imported design
    /// and its behavior were saved anyway, and a retry made second copies).
    #[test]
    fn a_failed_import_writes_nothing() {
        let mut world = world();
        // The starter, unsaved and changed: it is saved before the import opens, as `halvex_runner_2`.
        world.insert_resource(Storage::new(Refusing(MemoryBackend::default(), "halvex_runner_2")));
        let content = data::base_content();
        let mut careful = content.behaviors.get("skirmisher").unwrap().clone();
        careful.id = "careful".into();
        careful.name = "Careful".into();
        careful.source = Default::default();
        let ship = Blueprint { behavior: Some("careful".into()), source: Default::default(), ..content.ships.get("sample_mid").unwrap().clone() };
        let code = share::encode(&content, &ship, Some(&careful));
        let result = world.run_system_once(move |mut lib: Lib, content: Res<GameContent>| lib.import(&content.0, &code)).unwrap();
        assert!(result.as_ref().is_err_and(|e| e.starts_with("couldn't save Halvex Runner first")), "{result:?}");
        let storage = world.resource::<Storage>();
        assert!(storage.list("designs").unwrap().is_empty() && storage.list("behaviors").unwrap().is_empty());
        assert_eq!(world.resource::<PlayerDesign>().blueprint().name, "Halvex Runner");
    }

    /// The saved state after the name reads like the BEHAVIOR screen's: UNSAVED gold, SAVED grey.
    #[test]
    fn the_title_tag_matches_the_behavior_screen() {
        assert_eq!(title_tag(true, true), ("UNSAVED", Tone::Gold));
        assert_eq!(title_tag(false, true), ("UNSAVED", Tone::Gold));
        assert_eq!(title_tag(true, false), ("SAVED", Tone::Label));
        assert_eq!(title_tag(false, false).0, "");
    }

    /// Delete, a tool key or T while BLUEPRINTS covers the design does nothing to it; Ctrl+S still saves.
    #[test]
    fn keys_behind_the_screens_only_save() {
        let mut world = world();
        world.resource_mut::<Shelf>().page = Some(Page::Blueprints);
        world.init_resource::<Messages<DrydockAction>>();
        world.init_resource::<kit::ui_kit::UiPointer>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.insert_resource(world::settings::Controls::defaults(&data::base_content()));
        let mut keys = ButtonInput::<KeyCode>::default();
        for k in [KeyCode::Delete, KeyCode::KeyT, KeyCode::KeyX, KeyCode::ControlLeft, KeyCode::KeyS] {
            keys.press(k);
        }
        world.insert_resource(keys);
        world.run_system_once(super::super::actions::shortcuts).unwrap();
        let sent: Vec<DrydockAction> = world.resource::<Messages<DrydockAction>>().iter_current_update_messages().cloned().collect();
        assert_eq!(sent, [DrydockAction::Save]);
        // Without the screens the same keys act.
        world.resource_mut::<Shelf>().page = None;
        world.resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::ControlLeft);
        world.resource_mut::<Messages<DrydockAction>>().clear();
        world.run_system_once(super::super::actions::shortcuts).unwrap();
        let sent = world.resource::<Messages<DrydockAction>>().len();
        assert!(sent >= 3, "{sent}");
    }

    /// A frame without input leaves the shelf untouched, so its texts are not made again every frame.
    #[test]
    fn an_idle_frame_leaves_the_shelf_unchanged() {
        let mut world = world();
        world.init_resource::<Messages<DrydockAction>>();
        world.init_resource::<Messages<UiClick>>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.spawn((RenameField, TextField::default()));
        world.resource_mut::<Shelf>().page = Some(Page::Blueprints);
        let apply = world.register_system(apply);
        let changed = world.register_system(|shelf: Res<Shelf>| shelf.is_changed());
        world.run_system(apply).unwrap();
        world.run_system(changed).unwrap();
        world.run_system(apply).unwrap();
        assert!(!world.run_system(changed).unwrap());
    }

    /// `/` and T in one (slow) frame: the search field takes the keyboard, no test flight starts.
    #[test]
    fn a_slash_takes_the_other_keys_of_its_frame_into_the_search() {
        let mut world = world();
        world.init_resource::<Messages<DrydockAction>>();
        world.init_resource::<kit::ui_kit::UiPointer>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.insert_resource(world::settings::Controls::defaults(&data::base_content()));
        let search = world.spawn((super::super::actions::SearchField, TextField::default())).id();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyT);
        keys.press(KeyCode::Slash);
        world.insert_resource(keys);
        world.run_system_once(super::super::actions::shortcuts).unwrap();
        assert!(world.resource::<Messages<DrydockAction>>().is_empty());
        assert!(world.get::<TextField>(search).unwrap().focused);
    }

    /// Hovering a row builds the preview of its entry (the ship and its numbers) once; leaving the row takes it
    /// away again.
    #[test]
    fn the_preview_shows_the_hovered_entry_and_goes_on_leave() {
        let mut world = world();
        world.init_resource::<DrydockReport>();
        {
            let mut shelf = world.resource_mut::<Shelf>();
            shelf.page = Some(Page::Blueprints);
            shelf.templates = vec!["rim_skiff".into(), "halvex_runner".into()];
            shelf.offset = [0, 1];
        }
        world.spawn((Window::default(), PrimaryWindow));
        let row = world.spawn((RowNode(1, 0), Hot { hovered: true, pressed: false }, Node::default())).id();
        world.spawn((PreviewColumn, Node::default()));
        let card = world.spawn((PreviewCard::default(), Node { display: Display::None, ..default() })).id();
        let ship = world.spawn(PreviewShip).id();
        let stats: Vec<Entity> = (0..4).map(|k| world.spawn((PreviewStat(k), Text::new(""))).id()).collect();
        let preview = world.register_system(sync_preview);
        world.run_system(preview).unwrap();

        let runner = world.resource::<GameContent>().0.ships.get("halvex_runner").unwrap().clone();
        assert_eq!(world.get::<UiIcon>(ship), Some(&UiIcon::Ship(Box::new(runner))));
        assert_eq!(world.get::<Node>(card).unwrap().display, Display::Flex);
        let text = |world: &World, k: usize| world.get::<Text>(stats[k]).unwrap().0.clone();
        assert_eq!(text(&world, 0), "Frigate");
        assert!(text(&world, 1).ends_with(" t") && text(&world, 2).ends_with("m/s"), "{} {}", text(&world, 1), text(&world, 2));
        assert_ne!(text(&world, 3), "0");
        assert_eq!(world.get::<PreviewCard>(card).unwrap().shown, Some(Pick::Template("halvex_runner".into())));

        // Built once: another frame on the same row leaves the icon untouched.
        let tick = world.change_tick();
        world.run_system(preview).unwrap();
        assert!(!world.entity(ship).get_ref::<UiIcon>().unwrap().last_changed().is_newer_than(tick, world.change_tick()));

        // Over the gap to the next row it stays (moving down the list doesn't flash it); further off it goes.
        let window = world.query_filtered::<Entity, With<PrimaryWindow>>().single(&world).unwrap();
        let cursor = |world: &mut World, y: f64| world.get_mut::<Window>(window).unwrap().set_physical_cursor_position(Some(bevy::math::DVec2::new(0.0, y)));
        world.get_mut::<Hot>(row).unwrap().hovered = false;
        cursor(&mut world, f64::from(ROW_GAP) - 1.0);
        world.run_system(preview).unwrap();
        assert!(world.get::<UiIcon>(ship).is_some());
        cursor(&mut world, 50.0);
        world.run_system(preview).unwrap();
        assert!(world.get::<UiIcon>(ship).is_none());
        assert_eq!(world.get::<Node>(card).unwrap().display, Display::None);
        assert!(world.get::<PreviewCard>(card).unwrap().shown.is_none());
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
