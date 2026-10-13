//! Behaviors as saved documents on the BEHAVIOR screen (GDR-0057; written 13 Oct 2026; the storage is
//! `drydock/behaviors.rs`): the editable name and the saved state in the header, SAVE, the working draft, linking a
//! behavior to the design, and TEST IN RANGE.
//!
//! * **SAVE** (Ctrl+S, the button): a content template becomes the player's copy under a new id ("Skirmisher copy",
//!   unless the draft was renamed) and, when the design flew the template, the design names the copy
//!   (`PlayerDesign::edit_header`; the working design is written at once, the drydock shows it UNSAVED like any
//!   header change); a saved behavior is updated under its id; a new one gets an id from its name.
//! * **Unsaved edits are never lost**: the draft stays in memory while the screen is closed and goes to
//!   `drydock/behavior` ([`WorkingBehavior`]) a second after the last change and when the screen closes, so it
//!   comes back after a reload. While the design flies the behavior the draft was opened for, the screen keeps the
//!   draft (also one picked in the BEHAVIORS list); opening another one (the list, or the screen for a design that
//!   flies another behavior) saves unsaved edits first, as the drydock does with designs.
//! * **The name** top left: a click turns it into a field; Enter (or a click elsewhere) renames the draft (saved
//!   with SAVE), Esc keeps the old name. After it: TEMPLATE, SAVED, UNSAVED or NOT SAVED YET, and which behavior
//!   the design flies when that is another one ([`header_tag`]).
//! * **TEST IN RANGE** (T, the button; WATCH or FLY ALONGSIDE): the draft as it is ([`test_program`]: by id when
//!   saved and unchanged, else its text), refused with the reason in the status line when it has errors, can't fly
//!   (`test_range::check`) or the design can't fly. T in the test comes back here, the draft untouched.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use data::{Behavior, Content};
use sim::DesignReport;

use crate::drydock::PlayerDesign;
use crate::drydock::report::DrydockReport;
use crate::drydock::behaviors::{NEW_SUMMARY, PlayerBehaviors, errors_note, set_program, set_text};
use crate::drydock::library::{self, Autosave, DesignSlot, MAX_NAME};
use crate::flight_hud::HudFonts;
use crate::mode::AppMode;
use crate::params::Params;
use crate::sim_bridge::GameContent;
use crate::storage::{Document, Storage, now_unix};
use crate::style::{Style, ui_color};
use crate::test_range::{self, BehaviorTest, StartTest, TestProgram};
use crate::ui_kit::{Hot, TextField, Tone, UiClick, field_text, label, name_in_place, prose, set_color, set_text as set_ui_text};

use super::draft::{BehaviorDraft, Snapshot, design_behavior};
use super::screen::ScreenText;
use super::{BehaviorAction, BehaviorUi, BehaviorView, EditorContext};

/// The working draft's name in the `drydock` namespace (next to the working design, `drydock/working`).
const WORKING: &str = "behavior";
/// Where a working draft that doesn't read is kept before the next autosave writes over it.
const WORKING_ASIDE: &str = "behavior-unread";

// ---------------------------------------------------------------------------------------------------------
// State

/// The working draft as stored (`drydock/behavior`): `(source: "<id>" or None, name: "…", unsaved: <bool>,
/// flown: "<id>" or None, text: "…")`: the behavior it was opened from, its name and text, whether it has edits not
/// saved, and the behavior the design flew when it was opened.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkingBehavior {
    pub source: Option<String>,
    pub name: String,
    pub unsaved: bool,
    pub flown: Option<String>,
    pub text: String,
}

impl Document for WorkingBehavior {
    const NAMESPACE: &'static str = "drydock";
    const FORMAT: u32 = 1;

    fn to_ron(&self) -> String {
        let q = |s: &str| ron::to_string(s).unwrap_or_default();
        let opt = |s: &Option<String>| s.as_deref().map_or("None".into(), q);
        format!(
            "(\n    source: {},\n    name: {},\n    unsaved: {},\n    flown: {},\n    text: {},\n)\n",
            opt(&self.source),
            q(&self.name),
            self.unsaved,
            opt(&self.flown),
            q(&self.text)
        )
    }

    fn from_ron(text: &str, _format: u32) -> Result<Self, String> {
        data::ron_options().from_str(text).map_err(|e| e.to_string())
    }
}

/// The draft's place: the behavior the design flew when the draft was opened for it, and the working draft's
/// autosave.
#[derive(Resource, Default)]
pub struct DraftSlot {
    /// While the design still flies this one, opening the screen keeps the draft.
    pub flown: Option<String>,
    timer: Autosave,
    /// Changes noted for the autosave (goes up when the saved parts of the draft change).
    changes: u64,
    noted: Option<Snapshot>,
    /// Write the working draft at once (a save, an open, a link).
    pub write_now: bool,
    /// The change last kept for the page going away (`Storage::keep_on_leave`).
    kept: Option<u64>,
}

/// A message for the status line, shown until the draft changes.
#[derive(Resource, Default)]
pub struct ScreenNote {
    line: Option<(String, Tone)>,
    key: Snapshot,
}

impl ScreenNote {
    /// The message, while the draft is as it was when it was said.
    pub fn show(&self, draft: &BehaviorDraft) -> Option<(String, Tone)> {
        self.line.as_ref().filter(|_| self.key == draft.snapshot()).cloned()
    }
}

/// The resources saving, opening and linking work on.
#[derive(SystemParam)]
pub(super) struct Lib<'w> {
    pub storage: ResMut<'w, Storage>,
    pub behaviors: ResMut<'w, PlayerBehaviors>,
    pub design: ResMut<'w, PlayerDesign>,
    pub design_slot: Res<'w, DesignSlot>,
    pub draft: ResMut<'w, BehaviorDraft>,
    pub slot: ResMut<'w, DraftSlot>,
    pub note: ResMut<'w, ScreenNote>,
}

impl Lib<'_> {
    /// A message for the status line (uppercase), until the draft changes.
    pub fn say(&mut self, line: impl AsRef<str>, tone: Tone) {
        self.note.line = Some((line.as_ref().to_uppercase(), tone));
        self.note.key = self.draft.snapshot();
    }

    /// The id of the behavior the design flies.
    pub fn flown(&self, content: &Content) -> Option<String> {
        design_behavior(content, &self.behaviors, self.design.blueprint()).map(|b| b.id.clone())
    }

    /// The name of a content or saved behavior (its id if there is none).
    pub fn name_of(&self, content: &Content, id: &str) -> String {
        self.behaviors.behavior(content, id).map_or_else(|| id.to_string(), |b| b.name.clone())
    }

    /// SAVE: the draft is saved ([`save_draft`]); a copy of the template the design flew becomes the design's.
    pub fn save(&mut self, content: &Content) -> (String, Tone) {
        let flown = self.flown(content);
        let out = match save_draft(&mut self.draft, &mut self.behaviors, &mut self.storage, content, now_unix()) {
            Ok(out) => out,
            Err(e) => return (format!("not saved: {e}"), Tone::Coral),
        };
        self.slot.write_now = true;
        let errors = self.behaviors.get(&out.id).map_or(0, |p| p.errors.len());
        let mut line = match &out.template {
            Some(_) => format!("saved as your copy: {}", out.name),
            None => format!("saved  ·  {}", out.name),
        };
        if out.template.is_some() && out.template == flown {
            self.link(&out.id);
            line.push_str(&format!("  ·  {} flies it now", self.design.blueprint().name));
        }
        if errors > 0 {
            let s = if errors == 1 { "" } else { "s" };
            return (format!("{line}  ·  {errors} error{s}: it flies once fixed"), Tone::Gold);
        }
        (line, Tone::Teal)
    }

    /// Puts another draft on the screen. Unsaved edits of the one there are saved first (its name is returned);
    /// if that fails, nothing changes.
    pub fn switch(&mut self, content: &Content, next: BehaviorDraft) -> Result<Option<String>, String> {
        let kept = if self.draft.editor().is_some() && self.draft.dirty() {
            let out = save_draft(&mut self.draft, &mut self.behaviors, &mut self.storage, content, now_unix())
                .map_err(|e| format!("couldn't save {} first: {e}", self.draft.name))?;
            Some(out.name)
        } else {
            None
        };
        self.draft.replace(next);
        self.slot.write_now = true;
        Ok(kept)
    }

    /// Opens a content or saved behavior on the screen ([`Lib::switch`]); the message says how it went.
    pub fn open(&mut self, content: &Content, id: &str) -> Result<String, String> {
        let b = self.behaviors.behavior(content, id).ok_or_else(|| format!("{id} is gone"))?;
        let next = BehaviorDraft::from_behavior(b, &content.behavior_nodes).ok_or("the vocabulary has no action")?;
        let kept = self.switch(content, next)?;
        Ok(opened_note(kept.as_deref(), &self.draft.name))
    }

    /// The design flies this behavior from now on: its header names it, the working design is written at once.
    pub fn link(&mut self, id: &str) {
        let id = id.to_string();
        self.design.edit_header(|h| h.behavior = Some(id.clone()));
        self.slot.flown = Some(id);
        self.slot.write_now = true;
        if let Err(e) = library::write_working(&mut self.storage, &self.design, &self.design_slot, now_unix()) {
            warn!("behavior link: the working design was not written: {e}");
        }
    }
}

/// "Careful saved · opened Skirmisher" or "opened Skirmisher".
pub fn opened_note(kept: Option<&str>, opened: &str) -> String {
    match kept {
        Some(k) => format!("your edits saved as {k}  ·  opened {opened}"),
        None => format!("opened {opened}"),
    }
}

// ---------------------------------------------------------------------------------------------------------
// Saving (plain functions, tested)

/// What a save did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveOutcome {
    /// The saved behavior's id and name.
    pub id: String,
    pub name: String,
    /// The content template the draft was a copy of, when it was one: the save made a new player behavior.
    pub template: Option<String>,
}

/// Saves the draft (see the module docs): a template's draft becomes a player copy under a new id (the copy's
/// name is the template's with " copy", or the draft's own when it was renamed); a saved behavior's draft updates
/// it under its id (its summary, description and tags stay); a new draft gets an id from its name. A draft with
/// errors is saved as its lines. Afterwards the draft is the saved behavior (its source, nothing unsaved).
pub fn save_draft(draft: &mut BehaviorDraft, behaviors: &mut PlayerBehaviors, storage: &mut Storage, content: &Content, now: u64) -> Result<SaveOutcome, String> {
    let name = draft.name.trim().to_string();
    if name.is_empty() {
        return Err("a behavior needs a name".into());
    }
    let template = draft.source.clone().filter(|id| content.behaviors.contains(id));
    let saved = draft.source.as_deref().and_then(|id| behaviors.get(id)).map(|p| p.behavior.clone());
    let mut b = match (&template, saved) {
        (Some(t), _) => {
            let mut copy = behaviors.copy_of(content, t)?;
            if content.behaviors.get(t).is_some_and(|tb| tb.name != name) {
                copy.id = behaviors.new_id(content, &name);
                copy.name = name;
            }
            copy
        }
        (None, Some(saved)) => Behavior { name, ..saved },
        (None, None) => Behavior {
            id: String::new(),
            name,
            summary: NEW_SUMMARY.into(),
            description: None,
            lore: None,
            tags: Vec::new(),
            target: None,
            rules: Vec::new(),
            source: Default::default(),
        },
    };
    if draft.errors.is_empty() {
        set_program(&mut b, draft.program());
    } else {
        set_text(&mut b, &draft.text);
    }
    let id = behaviors.save(storage, content, &b, now)?;
    draft.source = Some(id.clone());
    draft.name.clone_from(&b.name);
    draft.mark_clean();
    Ok(SaveOutcome { id, name: b.name, template })
}

/// What TEST IN RANGE flies: the saved behavior by id when the draft is one and unchanged, else the draft's rules
/// (not saved yet); `Err` when the text has errors.
pub fn test_program(draft: &BehaviorDraft) -> Result<TestProgram, String> {
    if !draft.errors.is_empty() {
        return Err(format!("the text has {}", errors_note(&draft.errors)));
    }
    Ok(match &draft.source {
        Some(id) if !draft.dirty() => TestProgram::Id(id.clone()),
        _ => TestProgram::Text { name: draft.name.trim().to_string(), text: draft.program().to_text() },
    })
}

/// The saved state after the name: TEMPLATE, SAVED, UNSAVED or NOT SAVED YET, and, when the design flies another
/// behavior, which one (`flown`: its name).
pub fn header_tag(draft: &BehaviorDraft, content: &Content, behaviors: &PlayerBehaviors, flown: Option<&Behavior>) -> (String, Tone) {
    let source = draft.source.as_deref();
    let is_flown = source.is_some() && source == flown.map(|b| b.id.as_str());
    let template = source.is_some_and(|id| content.behaviors.contains(id));
    let saved = source.is_some_and(|id| behaviors.get(id).is_some());
    let (mut s, tone) = match (draft.dirty(), template, saved) {
        (true, ..) => ("UNSAVED".to_string(), Tone::Gold),
        (false, true, _) if is_flown => ("TEMPLATE  ·  SAVE MAKES THIS DESIGN'S OWN COPY".into(), Tone::Dim),
        (false, true, _) => ("TEMPLATE  ·  SAVE MAKES A COPY".into(), Tone::Dim),
        (false, false, true) => ("SAVED".into(), Tone::Label),
        (false, false, false) => ("NOT SAVED YET".into(), Tone::Dim),
    };
    if !is_flown {
        let other = flown.map_or("NO BEHAVIOR".into(), |b| b.name.to_uppercase());
        s.push_str(&format!("  ·  THE DESIGN FLIES {other}"));
    }
    (s, tone)
}

/// The working draft as stored.
fn working_of(draft: &BehaviorDraft, flown: Option<String>) -> WorkingBehavior {
    WorkingBehavior { source: draft.source.clone(), name: draft.name.clone(), unsaved: draft.dirty(), flown, text: draft.text.clone() }
}

/// A draft back from the working draft: its source as saved, with the stored name and text over it. A source that
/// is gone leaves a new draft, unsaved.
pub fn restore_draft(w: &WorkingBehavior, content: &Content, behaviors: &PlayerBehaviors) -> Option<BehaviorDraft> {
    let nodes = &content.behavior_nodes;
    let base = w.source.as_deref().and_then(|id| behaviors.behavior(content, id));
    let mut d = match base {
        Some(b) => BehaviorDraft::from_behavior(b, nodes)?,
        None => BehaviorDraft::empty(nodes)?,
    };
    d.name.clone_from(&w.name);
    d.set_text(&w.text);
    if !w.unsaved {
        d.mark_clean();
    } else if base.is_none() {
        d.mark_unsaved();
    }
    Some(d)
}

// ---------------------------------------------------------------------------------------------------------
// Systems

/// Opening the screen: the design's name and rule slots; the draft (see the module docs): restored from the
/// working draft on the first visit, kept while the design flies the behavior it was opened for, else the design's
/// behavior (unsaved edits saved first). A draft with errors opens in the TEXT view.
pub(super) fn open(mut lib: Lib, content: Res<GameContent>, params: Res<Params>, mut context: ResMut<EditorContext>, mut ui: ResMut<BehaviorUi>) {
    let content = &content.0;
    let bp = lib.design.blueprint();
    *context = EditorContext { design_name: bp.name.clone(), slots: content.ship_rule_slots(bp) };
    let (design_name, named) = (bp.name.clone(), bp.behavior.clone());
    // Messages from the last visit are old news.
    lib.note.line = None;
    let first = lib.draft.editor().is_none();
    // A working draft that doesn't read is kept aside before the new draft's autosave writes over it; the message is
    // said last, once the draft it is shown with is on the screen.
    let mut lost = None;
    if first {
        match lib.storage.read::<WorkingBehavior>(WORKING) {
            Ok(Some(w)) => {
                if let Some(d) = restore_draft(&w.doc, content, &lib.behaviors) {
                    lib.draft.replace(d);
                    lib.slot.flown = w.doc.flown;
                }
            }
            Ok(None) => {}
            Err(e) => {
                let kept = match lib.storage.set_aside::<WorkingBehavior>(WORKING, WORKING_ASIDE) {
                    Ok(()) => format!("it is kept as {}/{WORKING_ASIDE}", WorkingBehavior::NAMESPACE),
                    Err(e) => format!("it could not be kept: {e}"),
                };
                lost = Some(format!("the last behavior draft didn't come back: {e}; {kept}"));
            }
        }
    }
    let flown = lib.flown(content);
    let wanted = if first { params.text("behavior") } else { None };
    if lib.draft.editor().is_none() || lib.slot.flown != flown || wanted.is_some() {
        let next = BehaviorDraft::source_for(content, &lib.behaviors, lib.design.blueprint(), wanted).cloned();
        let mut switched = true;
        if lib.draft.editor().is_none() || lib.draft.source != next.as_ref().map(|b| b.id.clone()) {
            let nodes = &content.behavior_nodes;
            if let Some(d) = next.and_then(|b| BehaviorDraft::from_behavior(&b, nodes)).or_else(|| BehaviorDraft::empty(nodes)) {
                match lib.switch(content, d) {
                    Ok(Some(kept)) => lib.say(format!("your edits saved as {kept}"), Tone::Teal),
                    Ok(None) => {}
                    Err(e) => {
                        // Nothing was replaced: the draft still belongs to the behavior it was opened for, so the
                        // next visit tries again.
                        lib.say(e, Tone::Coral);
                        switched = false;
                    }
                }
            }
        }
        if switched {
            lib.slot.flown = flown;
            lib.slot.write_now = true;
        }
    }
    // A design naming a behavior that is gone flies its faction's (as flights do).
    if let Some(id) = named.as_deref()
        && lib.behaviors.behavior(content, id).is_none()
    {
        let flies = lib.flown(content).map_or("no behavior".into(), |f| lib.name_of(content, &f));
        lib.say(format!("behavior {id} is gone: {design_name} flies {flies}"), Tone::Gold);
    }
    if let Some(lost) = lost {
        lib.say(lost, Tone::Coral);
    }
    if !lib.draft.errors.is_empty() {
        ui.view = BehaviorView::Text;
    }
    ui.selected = None;
}

/// SAVE and TEST IN RANGE (with the design report of the design as it is: `drydock::report::update` runs before).
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    mut actions: MessageReader<BehaviorAction>,
    mut lib: Lib,
    content: Res<GameContent>,
    ui: Res<BehaviorUi>,
    report: Res<DrydockReport>,
    mut start: MessageWriter<StartTest>,
) {
    for a in actions.read() {
        match a {
            BehaviorAction::Save => {
                let (line, tone) = lib.save(&content.0);
                lib.say(line, tone);
            }
            BehaviorAction::TestInRange => match report.report.as_ref().ok_or("the design has no report yet").map_err(String::from).and_then(|r| test_request(&lib.draft, &lib.behaviors, &content.0, r)) {
                Ok(program) => {
                    start.write(StartTest { test: BehaviorTest { mode: ui.test_mode, program }, back: AppMode::Behavior });
                }
                Err(why) => lib.say(format!("can't test: {why}"), Tone::Coral),
            },
            _ => {}
        }
    }
}

/// What TEST IN RANGE flies, or why not: the text has errors, the behavior can't fly (`test_range::check`), or the
/// design can't (`report`: its design report, as the drydock's status line reads it).
pub fn test_request(draft: &BehaviorDraft, behaviors: &PlayerBehaviors, content: &Content, report: &Result<DesignReport, String>) -> Result<TestProgram, String> {
    let program = test_program(draft)?;
    test_range::check(content, behaviors, &program)?;
    if let Some(why) = crate::drydock::stats::status_line(content, report).grounded {
        return Err(format!("the design can't fly ({why})"));
    }
    Ok(program)
}

/// The working draft is written a second after the last change, at once after a save, an open or a link.
pub(super) fn autosave(time: Res<Time<Real>>, mut lib: Lib) {
    let now = time.elapsed_secs_f64();
    if lib.draft.is_changed() && lib.draft.editor().is_some() {
        let snap = lib.draft.snapshot();
        if lib.slot.noted.as_ref() != Some(&snap) {
            lib.slot.noted = Some(snap);
            lib.slot.changes += 1;
        }
    }
    let changes = lib.slot.changes;
    lib.slot.timer.note(changes, now);
    if lib.slot.timer.due(now) || std::mem::take(&mut lib.slot.write_now) {
        write_working(&mut lib);
    } else if lib.slot.timer.pending() && lib.slot.kept != Some(changes) && lib.draft.editor().is_some() {
        // Until it is written, the web page keeps it to write if it goes away first (a reload within the second).
        lib.slot.kept = Some(changes);
        let Lib { storage, draft, slot, .. } = &mut lib;
        storage.keep_on_leave(WORKING, || working_of(draft, slot.flown.clone()));
    }
}

/// Leaving the screen: the working draft is written now (also an edit the autosave has not noted yet).
pub(super) fn leave(mut lib: Lib) {
    let unnoted = lib.draft.editor().is_some() && lib.slot.noted.as_ref() != Some(&lib.draft.snapshot());
    if lib.slot.timer.pending() || lib.slot.write_now || unnoted {
        lib.slot.write_now = false;
        write_working(&mut lib);
    }
}

fn write_working(lib: &mut Lib) {
    lib.slot.timer.written();
    if lib.draft.editor().is_none() {
        return;
    }
    let w = working_of(&lib.draft, lib.slot.flown.clone());
    if let Err(e) = lib.storage.write(WORKING, &w, now_unix()) {
        lib.say(format!("the behavior draft was not kept: {e}"), Tone::Coral);
    }
}

// ---------------------------------------------------------------------------------------------------------
// The name top left

#[derive(Component)]
pub(super) struct TitleButton;

#[derive(Component)]
pub(super) struct TitleField;

/// The saved state after the name.
#[derive(Component)]
pub(super) struct TitleNote;

/// The behavior's name (the `ScreenText::Name` text), the field that replaces it while it is edited, and the
/// saved state after it.
pub(super) fn spawn_title(parent: &mut ChildSpawnerCommands, fonts: &HudFonts, name: Color, dim: Color, edge: Color) {
    parent.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(12), ..default() }).with_children(|t| {
        t.spawn((TitleButton, Hot::default(), Node::default())).with_child((ScreenText::Name, prose(fonts, "", 21.0, name)));
        t.spawn((
            TitleField,
            TextField { placeholder: "Name".into(), max_chars: MAX_NAME, ..default() },
            Hot::default(),
            Node {
                display: Display::None,
                width: px(280),
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
            f.spawn(field_text(fonts, 16.0));
        });
        t.spawn((TitleNote, label(fonts, "", 8.5, dim)));
    });
}

/// A click on the name opens the field; Enter or a click elsewhere renames the draft, Esc cancels.
#[allow(clippy::type_complexity)]
pub(super) fn title(
    mut clicks: MessageReader<UiClick>,
    keys: Res<ButtonInput<KeyCode>>,
    button: Single<(Entity, &mut Node), (With<TitleButton>, Without<TitleField>)>,
    field: Single<(&mut TextField, &mut Node), With<TitleField>>,
    mut editing: Local<bool>,
    mut draft: ResMut<BehaviorDraft>,
) {
    let (button, mut button_node) = button.into_inner();
    let (mut f, mut node) = field.into_inner();
    let clicked = clicks.read().any(|c| c.0 == button);
    let (open, renamed) = name_in_place(&mut editing, &mut f, clicked, keys.just_pressed(KeyCode::Escape), &draft.name);
    if let Some(new) = renamed {
        draft.name = new;
    }
    if let Some(open) = open {
        node.display = if open { Display::Flex } else { Display::None };
        button_node.display = if open { Display::None } else { Display::Flex };
    }
}

/// The saved state after the name follows the draft, the design and the saved behaviors.
pub(super) fn sync_title(
    draft: Res<BehaviorDraft>,
    design: Res<PlayerDesign>,
    behaviors: Res<PlayerBehaviors>,
    content: Res<GameContent>,
    style: Res<Style>,
    note: Single<(&mut Text, &mut TextColor), With<TitleNote>>,
) {
    if !draft.is_changed() && !design.is_changed() && !behaviors.is_changed() {
        return;
    }
    let content = &content.0;
    let flown = design_behavior(content, &behaviors, design.blueprint());
    let (s, tone) = header_tag(&draft, content, &behaviors, flown);
    let (mut text, mut color) = note.into_inner();
    set_ui_text(&mut text, &s);
    set_color(&mut color, ui_color(tone.color(&style.ui), 1.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryBackend;

    fn setup() -> (Content, Storage, PlayerBehaviors) {
        (data::base_content(), Storage::new(MemoryBackend::default()), PlayerBehaviors::default())
    }

    fn draft_of(content: &Content, behaviors: &PlayerBehaviors, id: &str) -> BehaviorDraft {
        BehaviorDraft::from_behavior(behaviors.behavior(content, id).unwrap(), &content.behavior_nodes).unwrap()
    }

    /// A world with what the screen's saving systems need (no window).
    fn world(storage: Storage) -> World {
        let content = data::base_content();
        let mut world = World::new();
        world.insert_resource(PlayerDesign::starter(&content));
        world.insert_resource(GameContent(content));
        world.insert_resource(storage);
        world.init_resource::<Params>();
        world.init_resource::<PlayerBehaviors>();
        world.init_resource::<DesignSlot>();
        world.init_resource::<BehaviorDraft>();
        world.init_resource::<DraftSlot>();
        world.init_resource::<ScreenNote>();
        world.init_resource::<EditorContext>();
        world.init_resource::<BehaviorUi>();
        world.init_resource::<Time<Real>>();
        world
    }

    /// Storage that reads but can't write (a full disk, the browser's quota).
    #[derive(Default)]
    struct FullBackend(MemoryBackend);

    impl crate::storage::Backend for FullBackend {
        fn list(&self, namespace: &str) -> Result<Vec<String>, crate::storage::StorageError> {
            self.0.list(namespace)
        }
        fn read(&self, namespace: &str, name: &str) -> Result<Option<String>, crate::storage::StorageError> {
            self.0.read(namespace, name)
        }
        fn write(&mut self, _: &str, _: &str, _: &str) -> Result<(), crate::storage::StorageError> {
            Err(crate::storage::StorageError::Full)
        }
        fn delete(&mut self, namespace: &str, name: &str) -> Result<(), crate::storage::StorageError> {
            self.0.delete(namespace, name)
        }
        fn location(&self) -> String {
            "full".into()
        }
    }

    /// A working draft that doesn't read (a newer build's) is said after the draft that replaces it is on the
    /// screen (said before, it was keyed to the empty draft and never showed), and kept aside before the autosave.
    #[test]
    fn an_unreadable_working_draft_is_said_and_kept() {
        use bevy::ecs::system::RunSystemOnce;
        let mut mem = MemoryBackend::default();
        crate::storage::Backend::write(&mut mem, "drydock", WORKING, "// space-game: drydock, format 9\n(later: 1)\n").unwrap();
        let mut world = world(Storage::new(mem));
        world.run_system_once(open).unwrap();
        let (line, tone) = world.resource::<ScreenNote>().show(world.resource::<BehaviorDraft>()).unwrap();
        assert!(line.starts_with("THE LAST BEHAVIOR DRAFT DIDN'T COME BACK") && line.ends_with("KEPT AS DRYDOCK/BEHAVIOR-UNREAD"), "{line}");
        assert_eq!(tone, Tone::Coral);
        world.run_system_once(autosave).unwrap();
        let storage = world.resource::<Storage>();
        assert!(storage.read::<WorkingBehavior>(WORKING).unwrap().is_some());
        assert!(matches!(storage.read::<WorkingBehavior>(WORKING_ASIDE), Err(crate::storage::StorageError::Newer { format: 9, .. })));
    }

    /// When the design flies another behavior but the unsaved draft can't be saved first, nothing is replaced and
    /// the draft still counts as the old behavior's, so the next visit tries again.
    #[test]
    fn a_failed_switch_keeps_the_draft_where_it_was() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = world(Storage::new(FullBackend::default()));
        world.run_system_once(open).unwrap();
        assert_eq!(world.resource::<DraftSlot>().flown.as_deref(), Some("skirmisher"));
        assert!(world.resource_mut::<BehaviorDraft>().set_text("always -> orbit(50%)"));
        world.resource_mut::<PlayerDesign>().edit_header(|h| h.behavior = Some("raider_basic".into()));
        world.run_system_once(open).unwrap();
        assert_eq!(world.resource::<DraftSlot>().flown.as_deref(), Some("skirmisher"));
        let draft = world.resource::<BehaviorDraft>();
        assert_eq!((draft.source.as_deref(), draft.dirty()), (Some("skirmisher"), true));
        let (line, _) = world.resource::<ScreenNote>().show(draft).unwrap();
        assert!(line.starts_with("COULDN'T SAVE"), "{line}");
    }

    /// An edit in the frame the screen is left (a note committed by the same tap as the DRYDOCK pill) is written
    /// with the working draft.
    #[test]
    fn leaving_writes_an_edit_the_autosave_has_not_seen() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = world(Storage::new(MemoryBackend::default()));
        world.run_system_once(open).unwrap();
        world.run_system_once(autosave).unwrap();
        world.run_system_once(leave).unwrap();
        // Typed after the last autosave of the visit.
        assert!(world.resource_mut::<BehaviorDraft>().set_text("always -> orbit(50%)"));
        world.run_system_once(leave).unwrap();
        let w = world.resource::<Storage>().read::<WorkingBehavior>(WORKING).unwrap().unwrap().doc;
        assert_eq!((w.text.as_str(), w.unsaved), ("always -> orbit(50%)", true));
    }

    #[test]
    fn saving_a_template_makes_a_copy_saving_a_player_behavior_updates_it() {
        let (content, mut storage, mut lib) = setup();
        // An unchanged template: SAVE still makes the copy, named after it.
        let mut d = draft_of(&content, &lib, "skirmisher");
        let out = save_draft(&mut d, &mut lib, &mut storage, &content, 1).unwrap();
        assert_eq!(out, SaveOutcome { id: "skirmisher_copy".into(), name: "Skirmisher copy".into(), template: Some("skirmisher".into()) });
        assert_eq!((d.source.as_deref(), d.name.as_str(), d.dirty()), (Some("skirmisher_copy"), "Skirmisher copy", false));
        // A renamed, edited template: the copy takes the new name and the rules.
        let mut d = draft_of(&content, &lib, "raider_basic");
        d.name = "Hit and run".into();
        assert!(d.set_text("target weakest\nwhen hull < 50% -> flee\nalways -> orbit(60%)"));
        assert!(d.dirty());
        let out = save_draft(&mut d, &mut lib, &mut storage, &content, 2).unwrap();
        assert_eq!((out.id.as_str(), out.template.as_deref()), ("hit_and_run", Some("raider_basic")));
        let saved = lib.get("hit_and_run").unwrap();
        assert_eq!(saved.behavior.target.as_deref(), Some("weakest"));
        assert_eq!(saved.behavior.rules, ["when hull < 50% -> flee", "always -> orbit(60%)"]);
        assert_eq!(saved.behavior.summary, content.behaviors.get("raider_basic").unwrap().summary);
        // Saving it again updates it under its id; the summary stays.
        d.name = "Hit & run".into();
        let out = save_draft(&mut d, &mut lib, &mut storage, &content, 3).unwrap();
        assert_eq!((out.id.as_str(), out.template), ("hit_and_run", None));
        assert_eq!(lib.get("hit_and_run").unwrap().behavior.name, "Hit & run");
        assert_eq!(storage.list("behaviors").unwrap(), ["hit_and_run", "skirmisher_copy"]);
        // A draft with errors is saved as its lines; a new one gets an id from its name.
        let mut d = BehaviorDraft::empty(&content.behavior_nodes).unwrap();
        d.set_text("when sheild_down -> flee\nalways -> hold");
        let out = save_draft(&mut d, &mut lib, &mut storage, &content, 4).unwrap();
        assert_eq!(out.id, "new_behavior");
        let p = lib.get("new_behavior").unwrap();
        assert_eq!((p.behavior.rules.len(), p.errors.len(), p.behavior.summary.as_str()), (2, 1, NEW_SUMMARY));
        assert!(!d.dirty());
        // No name, no save.
        d.name = "  ".into();
        assert!(save_draft(&mut d, &mut lib, &mut storage, &content, 5).is_err());
    }

    #[test]
    fn tests_fly_the_saved_behavior_by_id_and_edits_as_text() {
        let (content, mut storage, mut lib) = setup();
        let mut d = draft_of(&content, &lib, "skirmisher");
        assert_eq!(test_program(&d), Ok(TestProgram::Id("skirmisher".into())));
        assert!(d.set_text("always -> orbit(50%)"));
        assert_eq!(test_program(&d), Ok(TestProgram::Text { name: "Skirmisher".into(), text: "always -> orbit(50%)\n".into() }));
        let id = save_draft(&mut d, &mut lib, &mut storage, &content, 1).unwrap().id;
        assert_eq!(test_program(&d), Ok(TestProgram::Id(id)));
        // Errors: no test, the first error says why.
        d.set_text("always -> orbit(50%)\nwhen sheild_down -> flee");
        assert!(test_program(&d).unwrap_err().starts_with("the text has 1 error: line 2, column 6"));
        // A renamed but otherwise unchanged behavior flies as text under its new name.
        d.set_text("always -> orbit(50%)");
        d.name = "Circler".into();
        assert!(matches!(test_program(&d), Ok(TestProgram::Text { name, .. }) if name == "Circler"));
    }

    #[test]
    fn tests_are_refused_with_the_reason() {
        let (content, mut storage, mut lib) = setup();
        let config = sim::SimConfig::default();
        let runner = content.ships.get("halvex_runner").unwrap();
        let flies = sim::design_report(&content, runner, &config);
        let mut d = draft_of(&content, &lib, "skirmisher");
        assert_eq!(test_request(&d, &lib, &content, &flies), Ok(TestProgram::Id("skirmisher".into())));
        // A design that can't fly (no logic core, no thrust: one armor block).
        let mut block = runner.clone();
        block.parts.truncate(1);
        let grounded = sim::design_report(&content, &block, &config);
        assert!(test_request(&d, &lib, &content, &grounded).unwrap_err().starts_with("the design can't fly ("));
        // Errors in the text.
        d.set_text("when sheild_down -> flee");
        assert!(test_request(&d, &lib, &content, &flies).unwrap_err().contains("unknown condition"));
        // A saved behavior with errors, unchanged on the screen: refused too (by `test_range::check` it would be).
        let id = save_draft(&mut d, &mut lib, &mut storage, &content, 1).unwrap().id;
        assert!(test_range::check(&content, &lib, &TestProgram::Id(id)).is_err());
        assert!(test_request(&d, &lib, &content, &flies).is_err());
    }

    #[test]
    fn the_header_says_saved_unsaved_template_and_what_the_design_flies() {
        let (content, mut storage, mut lib) = setup();
        let skirm = content.behaviors.get("skirmisher");
        let mut d = draft_of(&content, &lib, "skirmisher");
        assert_eq!(header_tag(&d, &content, &lib, skirm), ("TEMPLATE  ·  SAVE MAKES THIS DESIGN'S OWN COPY".into(), Tone::Dim));
        let raider = content.behaviors.get("raider_basic");
        assert_eq!(header_tag(&d, &content, &lib, raider).0, "TEMPLATE  ·  SAVE MAKES A COPY  ·  THE DESIGN FLIES RAIDER PACK");
        d.name = "Mine".into();
        assert_eq!(header_tag(&d, &content, &lib, skirm), ("UNSAVED".into(), Tone::Gold));
        save_draft(&mut d, &mut lib, &mut storage, &content, 1).unwrap();
        let mine = lib.behavior(&content, "mine").cloned();
        assert_eq!(header_tag(&d, &content, &lib, mine.as_ref()), ("SAVED".into(), Tone::Label));
        assert_eq!(header_tag(&d, &content, &lib, None).0, "SAVED  ·  THE DESIGN FLIES NO BEHAVIOR");
        let new = BehaviorDraft::empty(&content.behavior_nodes).unwrap();
        assert_eq!(header_tag(&new, &content, &lib, skirm).0, "NOT SAVED YET  ·  THE DESIGN FLIES SKIRMISHER");
    }

    #[test]
    fn the_working_draft_comes_back_with_its_unsaved_edits() {
        let (content, mut storage, mut lib) = setup();
        let mut d = draft_of(&content, &lib, "skirmisher");
        assert!(d.set_text("always -> orbit(50%)"));
        d.set_text("always -> orbit(50%)\nwhen sheild");
        storage.write(WORKING, &working_of(&d, Some("skirmisher".into())), 1).unwrap();
        let w = storage.read::<WorkingBehavior>(WORKING).unwrap().unwrap().doc;
        assert_eq!((w.source.as_deref(), w.unsaved, w.flown.as_deref()), (Some("skirmisher"), true, Some("skirmisher")));
        let back = restore_draft(&w, &content, &lib).unwrap();
        assert_eq!((back.text.as_str(), back.errors.len(), back.dirty()), (d.text.as_str(), d.errors.len(), true));
        // The text has errors: the graph shows the source's rules until it is fixed.
        assert_eq!(back.program().rules.len(), 4);
        // Saved and unchanged: it comes back clean; a source that is gone leaves a new draft, unsaved.
        d.set_text("always -> orbit(50%)");
        save_draft(&mut d, &mut lib, &mut storage, &content, 2).unwrap();
        let w = working_of(&d, None);
        assert!(!restore_draft(&w, &content, &lib).unwrap().dirty());
        lib.delete(&mut storage, d.source.as_deref().unwrap()).unwrap();
        let back = restore_draft(&w, &content, &lib).unwrap();
        assert_eq!((back.source.as_deref(), back.name.as_str()), (None, d.name.as_str()));
        let gone = WorkingBehavior { unsaved: true, ..w };
        assert!(restore_draft(&gone, &content, &lib).unwrap().dirty());
    }
}
