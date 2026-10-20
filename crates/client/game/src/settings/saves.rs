//! SETTINGS' SAVES tab (ADR-0050): EXPORT writes everything this game keeps into one file, IMPORT replaces it with a
//! file's (`world::backup`). Home only: over flight the tab is off.
//!
//! ```text
//!   THIS GAME   The career (saves/career). The dev mode keeps its own save; …
//!   IT HOLDS    1 SAVE · 6 DESIGNS · 2 DRAFTS · 3 BEHAVIORS · 2 SETTINGS · 4 LAB RUNS (5.4 MB) · 2 RATINGS
//!   LAB RUNS    Large, and the lab can breed them again …                     [WITH] [WITHOUT]
//!   EXPORT      One file with all of it …                                      [EXPORT]
//!   IMPORT      Replaces all of it with a file's …                             [IMPORT]
//!   THE FILE    space-game-career-2026-10-19-1403.ron · 0.1.0, 2026-10-19 14:03 UTC · the career's save
//!   IT HOLDS    1 SAVE · 4 DESIGNS · …
//!   REPLACES    Everything above … A copy of it is saved first.                [REPLACE] [CANCEL]
//! ```
//!
//! * **EXPORT**: a download on the web, `<data dir>/exports/` on desktop (`share::save_file`); the status line names
//!   the file and what was left out (documents that don't read in this build).
//! * **IMPORT**: the web's file chooser; on desktop a backup dropped onto the window while the tab is open. The file
//!   is checked in full before anything shows (`Backup::read`); then what it holds and what it replaces.
//! * **REPLACE**: a copy of what the game holds now is saved first (an export with everything:
//!   `space-game-<save>-before-import-<date>.ron`); if that fails nothing is replaced. Then `Importing` writes the
//!   file's documents and deletes the rest, and the game starts again (the web reloads the page; desktop asks the
//!   player to start it again, QUIT).

use bevy::prelude::*;
use kit::style::{Style, ui_color};
use kit::ui_kit::{HudFonts, Pill, Scrollbar, ScreenPart, Edge, Tip, Tone, UiClick, label, prose, set_text, spawn_pill};
use world::backup::{self, Backup, BackupKinds, Group, Importing, Include};
use world::career::PlayerCareer;
use world::settings::SettingsLayer;
use world::storage::{Storage, now_unix};

use super::{Lay, Pane, SettingsUi, Tab};
use crate::drydock::share;
use crate::home::by_class;

/// How long the web waits after an import before the page reloads (seconds; the status line says it meanwhile).
const RESTART_AFTER: f64 = 1.5;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
enum SavesButton {
    Lab(bool),
    Export,
    Import,
    Replace,
    Cancel,
    /// After an import: start again (the web reloads, desktop quits).
    Restart,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum SavesText {
    Game,
    Holds,
    File,
    FileHolds,
    Replaces,
}

/// The rows about a picked file.
#[derive(Component)]
struct Preview;

/// The row with the button that starts the game again after an import.
#[derive(Component)]
struct RestartRow;

/// A file read and checked, waiting for REPLACE.
struct Picked {
    file: String,
    backup: Backup,
}

#[derive(Resource)]
struct SavesUi {
    /// Exports hold the lab's runs.
    lab: bool,
    /// What the storage holds (counted when the tab shows and after an import).
    held: Vec<Group>,
    picked: Option<Picked>,
    /// The tab showed last frame.
    shown: bool,
    /// The copy saved before the import ("downloaded …", "written to …").
    before: Option<String>,
    /// When the page reloads after an import (web; the app's elapsed seconds).
    restart_at: Option<f64>,
}

impl Default for SavesUi {
    fn default() -> Self {
        Self { lab: true, held: Vec::new(), picked: None, shown: false, before: None, restart_at: None }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(backup::BackupPlugin)
        .init_resource::<SavesUi>()
        .add_systems(Update, (count, input, incoming, watch, sync).chain().after(super::input).before(super::sync));
}

/// The tab is showing.
fn showing(layer: &SettingsLayer, ui: &SettingsUi) -> bool {
    layer.open && ui.tab == Tab::Saves
}

/// Counts what the storage holds when the tab shows.
fn count(layer: Res<SettingsLayer>, ui: Res<SettingsUi>, mut saves: ResMut<SavesUi>, storage: Res<Storage>, kinds: Res<BackupKinds>, career: Res<PlayerCareer>) {
    let now = showing(&layer, &ui);
    if now && !saves.shown {
        saves.held = backup::stored(&storage, &kinds, career.slot());
    }
    if saves.shown != now {
        saves.shown = now;
    }
}

fn say(ui: &mut SettingsUi, words: impl Into<String>, tone: Tone) {
    ui.say = Some((words.into().to_uppercase(), tone));
}

#[allow(clippy::too_many_arguments)]
fn input(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(&SavesButton, &Pill)>,
    layer: Res<SettingsLayer>,
    mut ui: ResMut<SettingsUi>,
    mut saves: ResMut<SavesUi>,
    mut storage: ResMut<Storage>,
    kinds: Res<BackupKinds>,
    career: Res<PlayerCareer>,
    mut importing: ResMut<Importing>,
    mut exit: MessageWriter<AppExit>,
) {
    if !showing(&layer, &ui) {
        clicks.clear();
        return;
    }
    let slot = career.slot().to_string();
    for c in clicks.read() {
        let Ok((b, pill)) = buttons.get(c.0) else { continue };
        if pill.disabled {
            continue;
        }
        match *b {
            SavesButton::Lab(on) => saves.lab = on,
            SavesButton::Export => {
                let (words, tone) = export(&storage, &kinds, &slot, Include { lab: saves.lab }, "");
                say(&mut ui, words, tone);
            }
            SavesButton::Import => {
                saves.picked = None;
                #[cfg(target_arch = "wasm32")]
                match share::pick_file(share::Picking::Backup, backup::MAX_FILE, backup::BackupError::TooLarge.to_string()) {
                    Ok(()) => say(&mut ui, "pick a backup file  ·  EXPORT writes them", Tone::Teal),
                    Err(e) => say(&mut ui, format!("no file chooser: {e}"), Tone::Coral),
                }
                #[cfg(not(target_arch = "wasm32"))]
                say(&mut ui, "drop a backup file onto the window  ·  EXPORT writes them", Tone::Teal);
            }
            SavesButton::Cancel => {
                saves.picked = None;
                ui.say = None;
            }
            SavesButton::Replace => {
                let Some(picked) = saves.picked.take() else { continue };
                let plan = match backup::plan(&storage, &kinds, &picked.backup, &slot) {
                    Ok(p) => p,
                    Err(e) => {
                        say(&mut ui, format!("not imported: {e}"), Tone::Coral);
                        continue;
                    }
                };
                // A copy of everything there is now, first: no copy, no import.
                let (words, tone) = export(&storage, &kinds, &slot, Include { lab: true }, "-before-import");
                if tone == Tone::Coral {
                    say(&mut ui, format!("not imported: the copy of your saves wasn't saved: {words}"), Tone::Coral);
                    continue;
                }
                saves.before = Some(words);
                *importing = Importing::start(&mut storage, plan);
                say(&mut ui, format!("importing {}", picked.file), Tone::Teal);
            }
            SavesButton::Restart => {
                #[cfg(target_arch = "wasm32")]
                backup::restart();
                #[cfg(not(target_arch = "wasm32"))]
                {
                    exit.write(AppExit::Success);
                }
            }
        }
    }
    let _ = &mut exit;
}

/// Exports and says how it went: (words, Teal or Coral). `suffix` goes after the save's name in the file name.
fn export(storage: &Storage, kinds: &BackupKinds, slot: &str, include: Include, suffix: &str) -> (String, Tone) {
    let got = match backup::gather(storage, kinds, slot, include, now_unix()) {
        Ok(g) => g,
        Err(e) => return (format!("not exported: {e}"), Tone::Coral),
    };
    let name = got.backup.file_name(&format!("{slot}{suffix}"));
    match share::save_file(&name, &got.backup.to_text()) {
        Ok(done) => {
            let left = match got.skipped.as_slice() {
                [] => String::new(),
                [(key, _)] => format!("  ·  left out {key}: it doesn't read in this version"),
                [(key, _), rest @ ..] => format!("  ·  left out {key} and {} more: they don't read in this version", rest.len()),
            };
            for (key, why) in &got.skipped {
                warn!("backup: left out {key}: {why}");
            }
            (format!("{done}{left}"), if left.is_empty() { Tone::Teal } else { Tone::Gold })
        }
        Err(e) => (format!("not exported: {e}"), Tone::Coral),
    }
}

/// A backup file that came in (the web's chooser, a file dropped onto the window on desktop): checked in full.
#[allow(clippy::too_many_arguments)]
fn incoming(
    layer: Res<SettingsLayer>,
    mut ui: ResMut<SettingsUi>,
    mut saves: ResMut<SavesUi>,
    kinds: Res<BackupKinds>,
    importing: Res<Importing>,
    storage: Res<Storage>,
    career: Res<PlayerCareer>,
    #[cfg(not(target_arch = "wasm32"))] mut drops: MessageReader<bevy::window::FileDragAndDrop>,
) {
    #[cfg(target_arch = "wasm32")]
    let files = share::take_picked(share::Picking::Backup);
    #[cfg(not(target_arch = "wasm32"))]
    let files: Vec<(String, Result<String, String>)> = drops
        .read()
        .filter_map(|d| match d {
            bevy::window::FileDragAndDrop::DroppedFile { path_buf, .. } => Some(path_buf.clone()),
            _ => None,
        })
        .filter(|_| showing(&layer, &ui))
        .map(|path| {
            let name = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let text = match std::fs::metadata(&path) {
                Ok(m) if m.len() > backup::MAX_FILE as u64 => Err(backup::BackupError::TooLarge.to_string()),
                _ => std::fs::read_to_string(&path).map_err(|e| e.to_string()),
            };
            (name, text)
        })
        .collect();
    let _ = &layer;
    for (file, text) in files {
        if *importing != Importing::Idle {
            continue;
        }
        // Read and checked in full, and whether it fits this game (a dev-mode save never becomes the career).
        let checked = text.and_then(|t| Backup::read(&t, &kinds).map_err(|e| e.to_string())).and_then(|b| match backup::plan(&storage, &kinds, &b, career.slot()) {
            Ok(_) => Ok(b),
            Err(e) => Err(e.to_string()),
        });
        match checked {
            Ok(backup) => {
                saves.picked = Some(Picked { file: file.clone(), backup });
                say(&mut ui, format!("{file}: every document reads  ·  REPLACE puts it in place of your saves"), Tone::Teal);
            }
            Err(e) => {
                saves.picked = None;
                say(&mut ui, format!("{file} doesn't import: {e}"), Tone::Coral);
            }
        }
    }
}

/// Follows the import: done, the game starts again (the web after a moment); failed, the status line says why.
fn watch(importing: Res<Importing>, mut ui: ResMut<SettingsUi>, mut saves: ResMut<SavesUi>, time: Res<Time<Real>>) {
    if importing.is_changed() {
        let before = saves.before.clone().map_or(String::new(), |b| format!("  ·  your saves before it: {b}"));
        match &*importing {
            Importing::Done => {
                let how = if cfg!(target_arch = "wasm32") { "the game starts again" } else { "quit and start the game again" };
                say(&mut ui, format!("imported  ·  {how}{before}"), Tone::Teal);
                if cfg!(target_arch = "wasm32") {
                    saves.restart_at = Some(time.elapsed_secs_f64() + RESTART_AFTER);
                }
            }
            Importing::Failed(why) => {
                let how = if cfg!(target_arch = "wasm32") { "start again: the stored saves are as they were" } else { "start the game again" };
                say(&mut ui, format!("{why}  ·  {how}{before}"), Tone::Coral);
            }
            _ => {}
        }
    }
    if let Some(at) = saves.restart_at
        && time.elapsed_secs_f64() >= at
    {
        saves.restart_at = None;
        #[cfg(target_arch = "wasm32")]
        backup::restart();
    }
}

/// The words, the pills' states and which rows show.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync(
    saves: Res<SavesUi>,
    importing: Res<Importing>,
    career: Res<PlayerCareer>,
    mut buttons: Query<(&SavesButton, &mut Pill)>,
    mut texts: Query<(&SavesText, &mut Text)>,
    mut rows: Query<(&mut Node, Has<Preview>), Or<(With<Preview>, With<RestartRow>)>>,
) {
    if !(saves.is_changed() || importing.is_changed()) {
        return;
    }
    let busy = *importing != Importing::Idle;
    for (b, mut pill) in &mut buttons {
        let (active, disabled) = match *b {
            SavesButton::Lab(on) => (saves.lab == on, busy),
            SavesButton::Export | SavesButton::Import | SavesButton::Cancel => (false, busy),
            SavesButton::Replace => (false, busy || saves.picked.is_none()),
            SavesButton::Restart => (false, importing.running()),
        };
        if pill.active != active || pill.disabled != disabled {
            pill.active = active;
            pill.disabled = disabled;
        }
    }
    let slot = career.slot();
    let groups = |g: &[Group]| if g.is_empty() { "NOTHING YET".to_string() } else { g.iter().map(Group::words).collect::<Vec<_>>().join("  ·  ") };
    for (t, mut text) in &mut texts {
        let words = match t {
            SavesText::Game => {
                if career.dev() {
                    format!("The dev mode's save (saves/{slot}); the career's is never touched here. Designs, behaviors, settings and lab runs are shared by both.")
                } else {
                    format!("The career (saves/{slot}). The dev mode (?dev=1) keeps a save of its own; designs, behaviors, settings and lab runs are shared by both.")
                }
            }
            SavesText::Holds => groups(&saves.held),
            SavesText::File => saves.picked.as_ref().map_or(String::new(), |p| {
                let save = match p.backup.save.as_deref() {
                    Some(s) if s == world::career::DEV_SLOT => "the dev mode's save".to_string(),
                    Some(s) if s == world::career::DEFAULT_SLOT => "the career's save".to_string(),
                    Some(s) => format!("the save {s}"),
                    None => "no save".to_string(),
                };
                format!("{}  ·  {}  ·  {save}", p.file, p.backup.origin())
            }),
            SavesText::FileHolds => saves.picked.as_ref().map_or(String::new(), |p| groups(&p.backup.contents())),
            SavesText::Replaces => saves.picked.as_ref().map_or(String::new(), |p| replaces(&p.backup, slot)),
        };
        set_text(&mut text, &words);
    }
    let after = matches!(*importing, Importing::Done | Importing::Failed(_));
    for (mut node, preview) in &mut rows {
        let show = if preview { saves.picked.is_some() } else { after };
        let d = if show { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
}

/// What REPLACE does with this file, in words.
fn replaces(file: &Backup, slot: &str) -> String {
    let mut words = String::from("Everything above, with the file's documents.");
    if file.left_out.iter().any(|n| n == backup::LAB) {
        words.push_str(" Your lab runs stay: the file has none.");
    }
    match file.save.as_deref() {
        None => words.push_str(" The file has no save: a new game starts."),
        Some(s) if s != slot => words.push_str(&format!(" Its save ({s}) becomes this game's ({slot}).")),
        _ => {}
    }
    words.push_str(" A copy of what is there now is saved first.");
    words
}

/// The SAVES pane (in the settings' stage; `Pane(Tab::Saves)`).
pub(super) fn spawn_pane(stage: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let (wl, cl) = (Lay::WIDE, Lay::COMPACT);
    let width = |w: f32| Node { width: px(w), flex_shrink: 0.0, ..default() };
    let row = Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(16), min_height: px(34), flex_shrink: 0.0, ..default() };
    let pane = |l: Lay| Node {
        position_type: PositionType::Absolute,
        left: px(l.list.0.x),
        top: px(l.list.0.y + 20.0),
        width: px(l.list.1.x),
        height: px(l.list.1.y - 20.0),
        flex_direction: FlexDirection::Column,
        row_gap: px(10),
        overflow: Overflow::scroll_y(),
        padding: UiRect::right(px(14)),
        ..default()
    };
    let import_about = if cfg!(target_arch = "wasm32") {
        "Replaces all of it with a backup file's. You see what the file holds first, and a copy of your saves is made before they are replaced."
    } else {
        "Drop a backup file onto the window: you see what it holds first, and a copy of your saves is made before they are replaced."
    };
    stage.spawn((Pane(Tab::Saves), ScreenPart::new(Edge::Fade, 1), Scrollbar::vertical(), ScrollPosition::default(), by_class(pane(wl), pane(cl)))).with_children(|p| {
        // A row: a name, words (fixed, or a text the screen sets), and pills.
        let line = |p: &mut ChildSpawnerCommands, preview: bool, name: &str, about: Result<&str, SavesText>, tone: Tone, pills: &[(&str, SavesButton, Tone, &str)]| {
            let mut r = p.spawn(row.clone());
            if preview {
                r.insert((Preview, Node { display: Display::None, ..row.clone() }));
            }
            r.with_children(|r| {
                r.spawn((label(fonts, name, 10.0, c(Tone::Number, 1.0)), by_class(width(wl.name_w), width(cl.name_w))));
                let words = Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() };
                match about {
                    Ok(t) => {
                        r.spawn((prose(fonts, t, 11.0, c(tone, 1.0)), words));
                    }
                    Err(t) => {
                        r.spawn((t, prose(fonts, "", 11.0, c(tone, 1.0)), words));
                    }
                }
                for (word, b, accent, tip) in pills {
                    spawn_pill(r, fonts, word, 9.5, 28.0, None, Pill::new(*accent), (*b, Tip::new(tip.to_string())));
                }
            });
        };
        line(p, false, "THIS GAME", Err(SavesText::Game), Tone::Label, &[]);
        line(p, false, "IT HOLDS", Err(SavesText::Holds), Tone::Number, &[]);
        line(
            p,
            false,
            "LAB RUNS",
            Ok("Large, and the lab can breed them again: leave them out for a small file."),
            Tone::Label,
            &[("WITH", SavesButton::Lab(true), Tone::Teal, "EXPORT WITH THE LAB'S RUNS"), ("WITHOUT", SavesButton::Lab(false), Tone::Teal, "EXPORT WITHOUT THEM")],
        );
        line(
            p,
            false,
            "EXPORT",
            Ok("One file with all of it, to keep safe or to take to another browser or device. A browser may clear what it stores."),
            Tone::Label,
            &[("EXPORT", SavesButton::Export, Tone::Teal, "SAVE A BACKUP FILE")],
        );
        line(p, false, "IMPORT", Ok(import_about), Tone::Label, &[("IMPORT", SavesButton::Import, Tone::Teal, "PICK A BACKUP FILE")]);
        line(p, true, "THE FILE", Err(SavesText::File), Tone::Number, &[]);
        line(p, true, "IT HOLDS", Err(SavesText::FileHolds), Tone::Number, &[]);
        line(
            p,
            true,
            "REPLACES",
            Err(SavesText::Replaces),
            Tone::Label,
            &[("REPLACE", SavesButton::Replace, Tone::Coral, "REPLACE YOUR SAVES WITH THE FILE'S"), ("CANCEL", SavesButton::Cancel, Tone::Teal, "KEEP YOUR SAVES")],
        );
        let restart = if cfg!(target_arch = "wasm32") { "START AGAIN" } else { "QUIT" };
        p.spawn((RestartRow, Node { display: Display::None, ..row.clone() })).with_children(|r| {
            r.spawn(by_class(width(wl.name_w), width(cl.name_w)));
            r.spawn((prose(fonts, "The game runs on the saves it started with until it starts again.", 11.0, c(Tone::Label, 1.0)), Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() }));
            spawn_pill(r, fonts, restart, 9.5, 28.0, None, Pill::new(Tone::Teal), (SavesButton::Restart, Tip::new(restart.to_string())));
        });
    });
}
