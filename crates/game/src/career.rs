//! The career in the game (track 7F, ADR-0033): the player's save, a new game, the development switches, and what
//! the career grants the drydock and FLEET. The rules are the pure model `data::career` (`docs/career.md`); this
//! module keeps it in the player's storage and hands its answers to the screens.
//!
//! * **The save** ([`CareerDoc`]) is `Career::to_ron` as a document `saves/<slot>` (`saves/career`, or another
//!   slot with `?save=<slot>`), with the storage layer's header and format (`data::career::FORMAT`).
//! * **At start** ([`load`], before the first frame, so the drydock opens the right ship): the save is read and
//!   reconciled with the content (what changed comes back as notes, never a panic). Without a save, or with
//!   `?newgame=1`, a new game begins from the content's first start and is written at once; the drydock then
//!   docks a copy of the start's ship (`drydock::blueprints`). A save that doesn't read (broken, or written by a
//!   newer build) is kept as it is: the game runs a new career in memory, never writes it, and says so.
//! * **Writes** (`docs/plans/first-playable-loop.md` 5.2): at a new game, after a FLEET change, when the drydock
//!   first shows a fresh part ([`SeenPart`]); later after UNLOCK and when a voyage ends. Never during a voyage.
//! * **`?unlock=all`** (free mode): the drydock has every part, material and frame. Only what is offered changes:
//!   no unlock is ever written into the save.
//! * **[`Granted`]**: the unlocks, the fresh parts (NEW), the mind capacity and the free flag, recomputed whenever
//!   the career changes; the drydock and FLEET read it. **[`EscortDesigns`]**: the escorts' saved designs, re-read
//!   when home or the drydock opens (a deleted design empties its slot with a note).
//! * **Notes** ([`Note`]) go to home's note block and the console.

use std::collections::BTreeSet;

use bevy::prelude::*;
use data::career::{Readiness, Unlocks, voyage_ready};
use data::{Blueprint, Career, Content, ResearchNode, Unlock};

use crate::drydock::library::{DesignDoc, problems_text};
use crate::mode::AppMode;
use crate::params::Params;
use crate::sim_bridge::GameContent;
use crate::storage::{Document, Storage, StorageError, now_unix, valid_name};
use crate::ui_kit::Tone;

/// The save's name when `?save=` doesn't give another.
pub const DEFAULT_SLOT: &str = "career";

/// The save as a stored document: `saves/<slot>`.
#[derive(Clone, Debug, PartialEq)]
pub struct CareerDoc(pub Career);

impl Document for CareerDoc {
    const NAMESPACE: &'static str = "saves";
    const FORMAT: u32 = data::career::FORMAT;

    fn to_ron(&self) -> String {
        self.0.to_ron()
    }

    fn from_ron(text: &str, _format: u32) -> Result<Self, String> {
        Career::from_ron(text).map(CareerDoc).map_err(|p| problems_text(&p))
    }
}

/// A line for home's note block: a head (a label) and its words.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub head: String,
    pub body: String,
    pub tone: Tone,
}

impl Note {
    pub fn new(head: impl Into<String>, body: impl Into<String>, tone: Tone) -> Self {
        Self { head: head.into(), body: body.into(), tone }
    }
}

/// Where the save is and the development switches (`?save= ?newgame= ?unlock=all`).
#[derive(Clone, Debug, PartialEq)]
pub struct Switches {
    /// The save's name in `saves/`.
    pub slot: String,
    /// Start over: a new game replaces the save.
    pub new_game: bool,
    /// Every part, material and frame in the drydock; the save is untouched.
    pub free: bool,
}

impl Switches {
    /// From the launch parameters; a slot that is no storage name is refused with a note.
    pub fn from_params(p: &Params) -> (Switches, Option<Note>) {
        let mut note = None;
        let slot = match p.text("save") {
            Some(s) if valid_name(s) => s.to_string(),
            Some(s) => {
                note = Some(Note::new(
                    "SAVE SLOT",
                    format!("`{s}` is no save name (a-z, 0-9, _ and -): playing saves/{DEFAULT_SLOT}."),
                    Tone::Gold,
                ));
                DEFAULT_SLOT.to_string()
            }
            None => DEFAULT_SLOT.to_string(),
        };
        let switches = Switches { slot, new_game: p.flag("newgame").unwrap_or(false), free: p.text("unlock") == Some("all") };
        (switches, note)
    }
}

/// The player's career (a resource): the save's facts, where they are stored, the switches and the notes.
#[derive(Resource, Debug)]
pub struct PlayerCareer {
    career: Career,
    slot: String,
    /// Why the stored save is kept as it is (it didn't read): this game is never written.
    kept: Option<String>,
    free: bool,
    /// This run began a new game (no save, or `?newgame=1`; not the stand-in for a kept save): the drydock docks
    /// the start's ship.
    new_game: bool,
    /// For home's note block.
    pub notes: Vec<Note>,
}

/// Reads the save `saves/<slot>` (or starts a new game) at start. `seed` is a new game's seed, `now` the time
/// it is written at.
pub fn load(storage: &mut Storage, content: &Content, switches: Switches, seed: u64, now: u64) -> PlayerCareer {
    let Switches { slot, new_game, free } = switches;
    let key = format!("{}/{slot}", CareerDoc::NAMESPACE);
    let mut notes = Vec::new();
    if let Some(why) = &storage.fallback {
        notes.push(Note::new("NO STORAGE", format!("The game is kept only until it closes: {why}."), Tone::Gold));
    }
    let stored = if new_game { Ok(None) } else { storage.read::<CareerDoc>(&slot) };
    let mut kept = None;
    let loaded = match stored {
        Ok(Some(s)) => Some(s.doc.0),
        Ok(None) => None,
        Err(e) => {
            let why = match &e {
                StorageError::Newer { .. } => e.to_string(),
                _ => format!("{key} doesn't read: {e}"),
            };
            notes.push(Note::new(
                "THE SAVE IS KEPT AS IT IS",
                format!("{why}. This game is a new one and is not saved; ?newgame=1 starts over in its place."),
                Tone::Coral,
            ));
            kept = Some(why);
            None
        }
    };
    let (career, new_game) = match loaded {
        Some(mut career) => {
            let changed = career.reconcile(content);
            if let Some(first) = changed.first() {
                let more = if changed.len() > 1 { format!(" (and {} more)", changed.len() - 1) } else { String::new() };
                notes.push(Note::new("THE SAVE MEETS CHANGED CONTENT", format!("{}{more}", first.message), Tone::Gold));
            }
            (career, false)
        }
        // A stand-in for a kept save is no new game: the drydock keeps its design.
        None => (new_career(content, seed, &mut notes, &mut kept), kept.is_none()),
    };
    if slot != DEFAULT_SLOT {
        let body = format!("This game is {key}; the usual one, {}/{DEFAULT_SLOT}, is untouched.", CareerDoc::NAMESPACE);
        notes.push(Note::new(format!("SAVE SLOT  ·  ?SAVE={}", slot.to_uppercase()), body, Tone::Gold));
    }
    if free {
        notes.push(Note::new("ALL UNLOCKED  ·  ?UNLOCK=ALL", "Every part, material and frame is in the drydock; the save is untouched.", Tone::Gold));
    }
    let mut out = PlayerCareer { career, slot, kept, free, new_game, notes };
    if new_game && let Err(e) = out.write(storage, now) {
        out.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
    }
    out
}

/// A new game from the content's first start. Content without a start gets a career that unlocks only what the
/// content doesn't lock, with no mind and no voyages; it is never written.
fn new_career(content: &Content, seed: u64, notes: &mut Vec<Note>, kept: &mut Option<String>) -> Career {
    if let Some(start) = content.starts.iter().next()
        && let Ok(career) = Career::new(content, &start.id, seed)
    {
        if kept.is_none() {
            notes.push(Note::new(format!("NEW GAME  ·  {}", start.name.to_uppercase()), start.summary.clone(), Tone::Teal));
        }
        return career;
    }
    let why = "the content has no start".to_string();
    notes.push(Note::new("NO CAREER", format!("{why}: nothing is saved."), Tone::Coral));
    *kept = Some(why);
    Career::from_ron(&format!("(start: \"\", seed: {seed}, generator: {})", data::career::GENERATOR))
        .expect("a career of three fields reads")
}

impl PlayerCareer {
    pub fn career(&self) -> &Career {
        &self.career
    }

    /// This run began a new game.
    pub fn new_game(&self) -> bool {
        self.new_game
    }

    /// The ship whose copy a new game docks: the career's start's.
    pub fn start_ship<'c>(&self, content: &'c Content) -> &'c str {
        content.starts.get(self.career.start()).map_or("", |s| s.ship.as_str())
    }

    /// What the career grants now.
    pub fn granted(&self, content: &Content) -> Granted {
        let (unlocks, fresh) = if self.free { (Unlocks::all(content), BTreeSet::new()) } else { (self.career.unlocks(content), self.career.fresh_parts(content)) };
        Granted { unlocks, fresh, mind_capacity: self.career.mind_capacity(content), free: self.free }
    }

    /// Changes the career and writes the save; `Err` says why it was not written (the change stays for this
    /// game).
    pub fn update<R>(&mut self, storage: &mut Storage, f: impl FnOnce(&mut Career) -> R) -> (R, Result<(), String>) {
        let r = f(&mut self.career);
        (r, self.write(storage, now_unix()))
    }

    fn write(&self, storage: &mut Storage, now: u64) -> Result<(), String> {
        if let Some(why) = &self.kept {
            return Err(format!("{why}: this game is not saved"));
        }
        storage.write(&self.slot, &CareerDoc(self.career.clone()), now).map_err(|e| e.to_string())
    }
}

/// What the career grants the screens now (a resource, derived from [`PlayerCareer`]).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Granted {
    /// The parts, materials and frames the drydock offers (everything in free mode).
    pub unlocks: Unlocks,
    /// Parts research unlocked that the drydock hasn't shown yet (NEW tags; none in free mode).
    pub fresh: BTreeSet<String>,
    /// Logic cores the ship-mind runs at once.
    pub mind_capacity: u32,
    /// `?unlock=all`.
    pub free: bool,
}

impl Granted {
    /// Is the content frame of this size open? (Sizes no content frame has are open: they came from elsewhere.)
    pub fn frame_open(&self, content: &Content, size: (u32, u32)) -> bool {
        content.frames.iter().find(|f| f.size == size).is_none_or(|f| self.unlocks.has_frame(&f.id))
    }
}

/// The escorts' saved designs, in the career's order (a resource): what the readiness checks and FLEET count.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct EscortDesigns(pub Vec<Blueprint>);

/// The drydock picked or selected a part: if it was fresh, it isn't any more (and the save is written).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct SeenPart(pub String);

/// Reads the escorts' designs; escorts whose design is gone or doesn't read leave the fleet (written, with a note).
pub fn read_escorts(storage: &mut Storage, career: &mut PlayerCareer) -> Vec<Blueprint> {
    let mut designs = Vec::new();
    let mut keep = Vec::new();
    for id in career.career.escorts() {
        match storage.read::<DesignDoc>(id) {
            Ok(Some(d)) => {
                designs.push(d.doc.0);
                keep.push(id.clone());
            }
            Ok(None) => career.notes.push(Note::new("FLEET", format!("The escort design `{id}` is gone: its slot is empty."), Tone::Gold)),
            Err(e) => career.notes.push(Note::new("FLEET", format!("The escort design `{id}` doesn't read ({e}): its slot is empty."), Tone::Gold)),
        }
    }
    if keep.len() != career.career.escorts().len() {
        let ((), written) = career.update(storage, |c| c.set_escorts(keep));
        if let Err(e) = written {
            career.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
        }
    }
    designs
}

/// The first research node that unlocks this (content order).
pub fn unlocked_by<'c>(content: &'c Content, unlock: &Unlock) -> Option<&'c ResearchNode> {
    content.research.iter().find(|n| n.unlocks.contains(unlock))
}

/// One readiness entry in words (uppercase): `USES BEAM LASER`, `ESCORT 2 USES COMPOSITE`, `NEEDS THE CRUISER
/// FRAME`, `3 CORES, YOUR MIND RUNS 2`, `NO CARGO · YOU CAN'T BRING SALVAGE HOME`.
pub fn readiness_words(content: &Content, r: &Readiness) -> String {
    use data::career::FleetSlot;
    let who = |s: &FleetSlot| match s {
        FleetSlot::Flagship => String::new(),
        FleetSlot::Escort(k) => format!("ESCORT {} ", k + 1),
    };
    let name = |table_name: Option<&str>, id: &str| table_name.unwrap_or(id).to_uppercase();
    match r {
        Readiness::LockedPart { ship, part } => format!("{}USES {}", who(ship), name(content.parts.get(part).map(|p| p.name.as_str()), part)),
        Readiness::LockedMaterial { ship, material } => {
            format!("{}USES {}", who(ship), name(content.materials.get(material).map(|m| m.name.as_str()), material))
        }
        Readiness::LockedFrame { ship, frame: Some(f), .. } => {
            format!("{}NEEDS THE {} FRAME", who(ship), name(content.frames.get(f).map(|f| f.name.as_str()), f))
        }
        Readiness::LockedFrame { ship, size, frame: None } => format!("{}IS TOO BIG FOR EVERY FRAME ({}×{})", who(ship), size.0, size.1),
        Readiness::TooManyCores { cores, capacity } => format!("{cores} CORES, YOUR MIND RUNS {capacity}"),
        Readiness::NoCargo => "NO CARGO  ·  YOU CAN'T BRING SALVAGE HOME".into(),
    }
}

/// The logic cores of a fleet.
pub fn fleet_cores(content: &Content, flagship: &Blueprint, escorts: &[Blueprint]) -> u32 {
    std::iter::once(flagship).chain(escorts).map(|b| content.ship_totals(b).logic_cores).sum()
}

/// Whether the fleet may launch, as one line and its tone: `VOYAGE-READY · CORES 2 / 2` (teal; gold with the
/// no-cargo warning after it), or `NOT VOYAGE-READY · <the first blocker>` and how many more (coral).
pub fn voyage_line(content: &Content, granted: &Granted, flagship: &Blueprint, escorts: &[Blueprint]) -> (String, Tone) {
    let list = voyage_ready(content, &granted.unlocks, granted.mind_capacity, flagship, escorts);
    let blockers: Vec<&Readiness> = list.iter().filter(|r| r.blocks()).collect();
    if let Some(first) = blockers.first() {
        let mut line = format!("NOT VOYAGE-READY  ·  {}", readiness_words(content, first));
        if blockers.len() > 1 {
            line.push_str(&format!("  ·  +{}", blockers.len() - 1));
        }
        return (line, Tone::Coral);
    }
    let cores = fleet_cores(content, flagship, escorts);
    let mut line = format!("VOYAGE-READY  ·  CORES {cores} / {}", granted.mind_capacity);
    match list.first() {
        Some(warning) => {
            line.push_str(&format!("  ·  {}", readiness_words(content, warning)));
            (line, Tone::Gold)
        }
        None => (line, Tone::Teal),
    }
}

pub struct CareerPlugin;

impl Plugin for CareerPlugin {
    fn build(&self, app: &mut App) {
        // Before any system runs: the drydock (added next) docks the career's start ship at a new game.
        let (switches, slot_note) = Switches::from_params(app.world().resource::<Params>());
        let now = now_unix();
        // The storage comes from StoragePlugin (added before this one).
        let (career, granted, escorts) = app.world_mut().resource_scope(|world, mut storage: Mut<Storage>| {
            let content = &world.resource::<GameContent>().0;
            let mut career = load(&mut storage, content, switches, now, now);
            career.notes.extend(slot_note);
            let escorts = EscortDesigns(read_escorts(&mut storage, &mut career));
            let granted = career.granted(content);
            (career, granted, escorts)
        });
        for n in &career.notes {
            match n.tone {
                Tone::Coral | Tone::Gold => warn!("career: {}: {}", n.head, n.body),
                _ => info!("career: {}: {}", n.head, n.body),
            }
        }
        info!("career: saves/{} ({})", career.slot, if career.new_game { "a new game" } else { "loaded" });
        app.insert_resource(career)
            .insert_resource(granted)
            .insert_resource(escorts)
            .add_message::<SeenPart>()
            .add_systems(PreUpdate, grant)
            .add_systems(Update, seen.run_if(in_state(AppMode::Drydock)))
            .add_systems(OnEnter(AppMode::Home), reread_escorts)
            .add_systems(OnEnter(AppMode::Drydock), reread_escorts);
    }
}

/// [`Granted`] follows the career.
fn grant(career: Res<PlayerCareer>, content: Res<GameContent>, mut granted: ResMut<Granted>) {
    if career.is_changed() {
        let want = career.granted(&content.0);
        if *granted != want {
            *granted = want;
        }
    }
}

/// Fresh parts the drydock showed are fresh no more.
fn seen(mut parts: MessageReader<SeenPart>, mut career: ResMut<PlayerCareer>, mut storage: ResMut<Storage>, content: Res<GameContent>) {
    for SeenPart(part) in parts.read() {
        if career.free || !career.career.fresh_parts(&content.0).contains(part) {
            continue;
        }
        let (_, written) = career.update(&mut storage, |c| c.mark_seen(&content.0, part));
        if let Err(e) = written {
            warn!("career: not saved: {e}");
        }
    }
}

/// Saved designs may have changed (saved, deleted) since the escorts were read.
fn reread_escorts(mut storage: ResMut<Storage>, mut career: ResMut<PlayerCareer>, mut escorts: ResMut<EscortDesigns>) {
    let designs = read_escorts(&mut storage, &mut career);
    if escorts.0 != designs {
        escorts.0 = designs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryBackend;

    fn switches(q: &str) -> Switches {
        Switches::from_params(&Params::parse(q)).0
    }

    fn stored_text(storage: &Storage, slot: &str) -> Option<String> {
        storage.read::<CareerDoc>(slot).ok().flatten().map(|d| d.doc.0.to_ron())
    }

    #[test]
    fn the_url_picks_the_slot_and_the_switches() {
        assert_eq!(switches(""), Switches { slot: "career".into(), new_game: false, free: false });
        assert_eq!(switches("?save=test_2&newgame=1&unlock=all"), Switches { slot: "test_2".into(), new_game: true, free: true });
        assert!(!switches("?newgame=0&unlock=some").new_game);
        let (s, note) = Switches::from_params(&Params::parse("?save=Bad%20Name"));
        assert_eq!(s.slot, DEFAULT_SLOT);
        assert!(note.unwrap().body.contains("Bad%20Name"));
    }

    /// No save: a new game from the first start, written at once; the next start loads it.
    #[test]
    fn a_first_start_begins_a_new_game_and_writes_it() {
        let content = data::base_content();
        let mut storage = Storage::new(MemoryBackend::default());
        let c = load(&mut storage, &content, switches(""), 77, 1000);
        assert!(c.new_game());
        assert_eq!((c.career().start(), c.career().seed()), ("rim_awakening", 77));
        assert_eq!(c.start_ship(&content), "rim_skiff");
        assert!(c.notes.iter().any(|n| n.head.starts_with("NEW GAME") && n.tone == Tone::Teal), "{:?}", c.notes);
        let saved = storage.read::<CareerDoc>("career").unwrap().unwrap();
        assert_eq!((saved.doc.0.seed(), saved.saved), (77, Some(1000)));
        let again = load(&mut storage, &content, switches(""), 5, 2000);
        assert!(!again.new_game() && again.notes.is_empty());
        assert_eq!(again.career(), c.career());
        // The granted kit: the early game, mind capacity 2, nothing fresh.
        let g = again.granted(&content);
        assert!(g.unlocks.has_part("pulse_laser") && !g.unlocks.has_part("beam_laser"));
        assert!(!g.frame_open(&content, (40, 40)) && g.frame_open(&content, (24, 24)) && g.frame_open(&content, (7, 7)));
        assert_eq!((g.mind_capacity, g.fresh.len(), g.free), (2, 0, false));
    }

    /// A save that doesn't read (broken, or from a newer build) is kept; the game runs a new career and never
    /// writes it. `?newgame=1` replaces it.
    #[test]
    fn a_broken_or_newer_save_is_kept_and_not_overwritten() {
        let content = data::base_content();
        for text in ["// space-game: saves, format 1, saved 5\n(start: \"rim_awakening\", seed: ", "// space-game: saves, format 9, saved 5\n()"] {
            let mut mem = MemoryBackend::default();
            crate::storage::Backend::write(&mut mem, "saves", "career", text).unwrap();
            let mut storage = Storage::new(mem);
            let mut c = load(&mut storage, &content, switches(""), 1, 10);
            // A stand-in, not a new game: the drydock keeps its design.
            assert!(!c.new_game());
            assert_eq!(c.career().start(), "rim_awakening");
            let note = c.notes.iter().find(|n| n.tone == Tone::Coral).expect("a note");
            assert!(note.head.contains("KEPT") && note.body.contains("not saved"), "{note:?}");
            // FLEET's change stays in this game only; the stored text is untouched.
            let ((), written) = c.update(&mut storage, |k| k.set_escorts(vec!["x".into()]));
            assert!(written.is_err());
            assert!(storage.read::<CareerDoc>("career").is_err(), "still the broken text");
            // Starting over replaces it.
            let fresh = load(&mut storage, &content, switches("?newgame=1"), 2, 20);
            assert!(fresh.notes.iter().all(|n| n.tone != Tone::Coral), "{:?}", fresh.notes);
            assert_eq!(stored_text(&storage, "career"), Some(fresh.career().to_ron()));
        }
    }

    /// Another slot, a new game over an old one, free mode leaving the save as it is.
    #[test]
    fn slots_new_games_and_free_mode() {
        let content = data::base_content();
        let mut storage = Storage::new(MemoryBackend::default());
        let mut c = load(&mut storage, &content, switches(""), 1, 10);
        let ((), w) = c.update(&mut storage, |k| k.set_escorts(vec!["drone".into()]));
        w.unwrap();
        let career_text = stored_text(&storage, "career").unwrap();
        // Another slot is another game.
        let t = load(&mut storage, &content, switches("?save=trial"), 2, 20);
        assert!(t.new_game() && t.career().escorts().is_empty());
        assert_eq!(storage.list("saves").unwrap(), ["career", "trial"]);
        // Free mode: everything granted, nothing written.
        let f = load(&mut storage, &content, switches("?unlock=all"), 3, 30);
        assert!(!f.new_game());
        let g = f.granted(&content);
        assert!(g.free && g.unlocks.has_part("beam_laser") && g.unlocks.has_frame("capital") && g.fresh.is_empty());
        assert!(f.notes.iter().any(|n| n.head.contains("ALL UNLOCKED")));
        assert_eq!(stored_text(&storage, "career").unwrap(), career_text);
        // A new game replaces the old one.
        let n = load(&mut storage, &content, switches("?newgame=1"), 4, 40);
        assert!(n.new_game() && n.career().escorts().is_empty());
        assert_eq!(stored_text(&storage, "career").unwrap(), n.career().to_ron());
    }

    /// Content changes reach a loaded save as a note (no panic); a gone escort design empties its slot.
    #[test]
    fn changed_content_and_gone_escorts_are_notes() {
        let content = data::base_content();
        let mut mem = MemoryBackend::default();
        let text = "// space-game: saves, format 1, saved 5\n(start: \"rim_awakening\", seed: 3, generator: 1, researched: [\"warp_theory\"], escorts: [\"kept\", \"gone\"])";
        crate::storage::Backend::write(&mut mem, "saves", "career", text).unwrap();
        let mut storage = Storage::new(mem);
        let skiff = content.ships.get("rim_skiff").unwrap().clone();
        storage.write("kept", &DesignDoc(Blueprint { id: "kept".into(), ..skiff.clone() }), 1).unwrap();
        let mut c = load(&mut storage, &content, switches(""), 1, 10);
        assert!(!c.new_game());
        assert!(c.notes.iter().any(|n| n.tone == Tone::Gold && n.body.contains("warp_theory")), "{:?}", c.notes);
        let designs = read_escorts(&mut storage, &mut c);
        assert_eq!(designs.len(), 1);
        assert_eq!(c.career().escorts(), ["kept"]);
        assert!(c.notes.iter().any(|n| n.head == "FLEET" && n.body.contains("`gone`")));
        // The emptied slot is written.
        assert_eq!(storage.read::<CareerDoc>("career").unwrap().unwrap().doc.0.escorts(), ["kept"]);
    }

    /// The voyage-ready line: ready with the cores, the first blocker with how many more, the cargo warning.
    #[test]
    fn the_voyage_line_names_the_first_blocker() {
        let content = data::base_content();
        let mut storage = Storage::new(MemoryBackend::default());
        let c = load(&mut storage, &content, switches(""), 1, 10);
        let g = c.granted(&content);
        let skiff = content.ships.get("rim_skiff").unwrap();
        assert_eq!(voyage_line(&content, &g, skiff, &[]), ("VOYAGE-READY  ·  CORES 2 / 2".into(), Tone::Teal));
        let runner = content.ships.get("halvex_runner").unwrap();
        let (line, tone) = voyage_line(&content, &g, runner, &[]);
        assert_eq!(tone, Tone::Coral);
        assert!(line.starts_with("NOT VOYAGE-READY  ·  USES ") && line.contains("  ·  +"), "{line}");
        let (line, _) = voyage_line(&content, &g, skiff, std::slice::from_ref(skiff));
        assert_eq!(line, "NOT VOYAGE-READY  ·  4 CORES, YOUR MIND RUNS 2");
        let mut no_cargo = skiff.clone();
        no_cargo.parts.retain(|p| !p.part.starts_with("cargo"));
        let (line, tone) = voyage_line(&content, &g, &no_cargo, &[]);
        assert_eq!((line.as_str(), tone), ("VOYAGE-READY  ·  CORES 2 / 2  ·  NO CARGO  ·  YOU CAN'T BRING SALVAGE HOME", Tone::Gold));
        // Free mode: only the mind and the frame limit.
        let free = Granted { unlocks: Unlocks::all(&content), ..g.clone() };
        assert_eq!(voyage_line(&content, &free, runner, &[]).1, Tone::Teal);
        assert_eq!(unlocked_by(&content, &Unlock::Part("beam_laser".into())).map(|n| n.id.as_str()), Some("beam_laser"));
        assert!(unlocked_by(&content, &Unlock::Material("reactive".into())).is_none());
    }
}
