//! PILOT ASSISTS (GDR-0088): the tab next to BEHAVIOR where the design's pilot assists are edited with the same GRAPH
//! and TEXT views, inspector and palette as its behavior (the TEXT view edits the list in the blueprint's rule text,
//! e.g. `brake(toggle2, off)`). An assist rule's action is one of the assists (face target, brake, hold station, avoid
//! collisions, auto-fire); its TOGGLE and AT START values (the inspector's pills) say which ASSIST TOGGLE key switches
//! it in flight and whether a mission starts with it on. The conditions and target selectors are the behaviors', plus
//! the pilot's own (`lock_held`, `idle`).
//!
//! * **The tab** ([`switch`]): opening it puts the design's assists on the screen as the draft (`BehaviorDraft::assists`)
//!   and parks the behavior's draft in [`AssistTab`]; BEHAVIOR (or BEHAVIORS) brings the behavior back and parks the
//!   assists' draft, so a text with errors is still there on the way back while the screen stays open (the design
//!   keeps the last valid list). While the tab is open, SAVE says where the assists are saved, the title can't be
//!   renamed, and the working behavior draft's autosave waits (`saving.rs`); TEST IN RANGE tests the behavior, and
//!   the design flies with its assists.
//! * **Saving** ([`write`]): the assists are part of the design (its blueprint's `assists`), like the behavior it
//!   names: every change goes into the working design at once (`PlayerDesign::edit_header`, written to the storage
//!   when no slider is held), and the drydock shows the design UNSAVED until it is saved there.
//! * Leaving the screen ([`leave`]) writes them and brings the behavior's draft back, so everything else on the
//!   screen (opening, the working draft) only ever sees the behavior; coming back reopens the tab.

use bevy::prelude::*;
use data::AssistList;
use kit::ui_kit::Tone;
use world::player::PlayerDesign;
use world::player::library::{self, DesignSlot};
use world::sim_bridge::GameContent;
use world::storage::{Storage, now_unix};

use super::controls::SliderDrag;
use super::draft::BehaviorDraft;
use super::saving::ScreenNote;
use super::{BehaviorTab, BehaviorUi};

/// The tab's state.
#[derive(Resource, Default)]
pub struct AssistTab {
    /// The behavior's draft while the tab is open (the screen's draft is then the design's assists).
    parked: Option<BehaviorDraft>,
    /// The assists' draft while the BEHAVIOR tab is open, for the rest of the screen's visit.
    assists: Option<BehaviorDraft>,
    /// The assists' draft revision last put into the design (opening the tab changes nothing in it).
    put: u64,
    /// The working design has assist changes the storage hasn't got yet (written once no slider is held).
    unwritten: bool,
}

impl AssistTab {
    /// The tab is open: the screen's draft is the design's assists.
    pub fn open(&self) -> bool {
        self.parked.is_some()
    }

    /// The behavior's draft: the parked one while the tab is open, else the screen's.
    pub fn behavior<'a>(&'a self, draft: &'a BehaviorDraft) -> &'a BehaviorDraft {
        self.parked.as_ref().unwrap_or(draft)
    }
}

/// Opens or closes the tab with the view (after `actions::apply`, before everything that saves or opens drafts).
pub(super) fn switch(
    ui: Res<BehaviorUi>,
    mut tab: ResMut<AssistTab>,
    mut draft: ResMut<BehaviorDraft>,
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    mut note: ResMut<ScreenNote>,
) {
    let want = ui.tab == BehaviorTab::Assists;
    if want == tab.open() {
        return;
    }
    if want {
        let (assists, dropped) = match tab.assists.take() {
            Some(a) => (a, 0),
            None => BehaviorDraft::assists(design.blueprint(), &content.0.behavior_nodes),
        };
        tab.parked = Some(draft.clone());
        draft.replace(assists);
        tab.put = draft.revision();
        if dropped > 0 {
            let s = if dropped == 1 { "" } else { "S" };
            note.say(&draft, format!("{dropped} ASSIST LINE{s} NO LONGER CHECK: LEFT OUT UNTIL YOU CHANGE THE LIST"), Tone::Gold);
        }
    } else if let Some(behavior) = tab.parked.take() {
        tab.assists = Some(draft.clone());
        draft.replace(behavior);
    }
}

/// Every change of the assists goes into the working design at once; the storage gets it once no slider is held.
pub(super) fn write(
    mut tab: ResMut<AssistTab>,
    draft: Res<BehaviorDraft>,
    drag: Res<SliderDrag>,
    mut design: ResMut<PlayerDesign>,
    slot: Res<DesignSlot>,
    mut storage: ResMut<Storage>,
) {
    if !tab.open() {
        return;
    }
    if draft.revision() != tab.put {
        tab.put = draft.revision();
        let list = AssistList::of(draft.program());
        design.edit_header(|h| h.assists = list);
        tab.unwritten = true;
    }
    if tab.unwritten && drag.0.is_none() {
        tab.unwritten = false;
        if let Err(e) = library::write_working(&mut storage, &design, &slot, now_unix()) {
            warn!("pilot assists: the working design was not written: {e}");
        }
    }
}

/// Leaving the screen with the tab open: the last change is written and the behavior's draft comes back (the tab
/// reopens on the way back in).
pub(super) fn leave(mut tab: ResMut<AssistTab>, mut draft: ResMut<BehaviorDraft>, design: Res<PlayerDesign>, slot: Res<DesignSlot>, mut storage: ResMut<Storage>) {
    if std::mem::take(&mut tab.unwritten)
        && let Err(e) = library::write_working(&mut storage, &design, &slot, now_unix())
    {
        warn!("pilot assists: the working design was not written: {e}");
    }
    tab.assists = None;
    if let Some(behavior) = tab.parked.take() {
        draft.replace(behavior);
    }
}
