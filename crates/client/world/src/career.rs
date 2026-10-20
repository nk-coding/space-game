//! The career in the game (track 7F, ADR-0033): the player's save, a new game, the development switches, and what
//! the career grants the drydock and FLEET. The rules are the pure model `data::career` (`docs/career.md`); this
//! module keeps it in the player's storage and hands its answers to the screens.
//!
//! * **The save** ([`CareerDoc`]) is `Career::to_ron` as a document `saves/<slot>` (`saves/career`; the dev mode's
//!   own `saves/dev`), with the storage layer's header and format (`data::career::FORMAT`).
//! * **At start** ([`load`], before the first frame, so the drydock opens the right ship): the save is read and
//!   reconciled with the content (what changed comes back as notes, never a panic). A new game gets the content's
//!   first galaxy, generated once from its game seed and kept in the save (ADR-0035, [`chart`]). Without a save,
//!   with a save of an older format (no save compatibility before release, ADR-0044; a log line and a note say
//!   so), or with `?dev=1&newgame=1` (the dev save only), a new game begins from the content's first start and is written at once; the
//!   drydock then docks a copy of the start's ship (`drydock::blueprints`). A save that doesn't read (broken, or
//!   written by a newer build) is kept as it is: the game runs a new career in memory, never writes it, and says so.
//! * **Writes** (`docs/plans/first-playable-loop.md` 5.2): at a new game, after a FLEET change, when the drydock
//!   first shows a fresh part ([`SeenPart`]); later after UNLOCK and when a voyage ends. Never during a voyage.
//! * **The dev mode** (`?dev=1`, ADR-0046): its own save `saves/dev`, never the career's. At every load it gets
//!   everything (`Career::open_everything`: every research node researched, the stores full, the whole galaxy
//!   charted with every gate open), and the drydock offers every part, material and frame of the content, with
//!   unlimited stock for a launch.
//! * **[`Granted`]**: the unlocks, the fresh parts (NEW), the stock, the mind capacity and the dev flag, recomputed whenever
//!   the career changes; the drydock and FLEET read it. **[`EscortDesigns`]**: the escorts' saved designs, re-read
//!   when home or the drydock opens (a deleted design empties its slot with a note).
//! * **Notes** ([`Note`]) go to home's note block and the console.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use data::career::{Readiness, Unlocks, voyage_ready};
use data::{Blueprint, Career, Content, ResearchNode, Unlock};
use kit::params::Params;
use kit::ui_kit::Tone;

use crate::player::library::{DesignDoc, problems_text};
use crate::mode::AppMode;
use crate::sim_bridge::GameContent;
use crate::storage::{Document, Storage, StorageError, now_unix};

/// The career's save.
pub const DEFAULT_SLOT: &str = "career";

/// The dev mode's own save (`?dev=1`).
pub const DEV_SLOT: &str = "dev";

/// The save as a stored document: `saves/<slot>`.
#[derive(Clone, Debug, PartialEq)]
pub struct CareerDoc(pub Career);

impl Document for CareerDoc {
    const NAMESPACE: &'static str = "saves";
    const FORMAT: u32 = data::career::FORMAT;
    const OLDEST: u32 = data::career::FORMAT;

    fn to_ron(&self) -> String {
        self.0.to_ron()
    }

    fn from_ron(text: &str) -> Result<Self, String> {
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

/// The launch's switches (ADR-0046): the dev mode (`?dev=1`) and, only with it, a new dev game (`?newgame=1`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Switches {
    /// The dev mode: its own save ([`DEV_SLOT`]) with everything open.
    pub dev: bool,
    /// Start the dev save over: a new game replaces it.
    pub new_game: bool,
}

impl Switches {
    /// From the launch parameters; `?newgame=1` without `?dev=1` is ignored, with a note.
    pub fn from_params(p: &Params) -> (Switches, Option<Note>) {
        let dev = p.flag("dev").unwrap_or(false);
        let asked = p.flag("newgame").unwrap_or(false);
        let note = (asked && !dev).then(|| Note::new("NO NEW GAME", "?newgame=1 starts only the dev save over: add ?dev=1.", Tone::Gold));
        (Switches { dev, new_game: dev && asked }, note)
    }

    /// The save's name in `saves/`.
    pub fn slot(&self) -> &'static str {
        if self.dev { DEV_SLOT } else { DEFAULT_SLOT }
    }
}

/// The player's career (a resource): the save's facts, where they are stored, the switches and the notes.
#[derive(Resource, Debug)]
pub struct PlayerCareer {
    career: Career,
    slot: String,
    /// Why the stored save is kept as it is (it didn't read): this game is never written.
    kept: Option<String>,
    /// The dev mode (`?dev=1`): everything open, its own save.
    dev: bool,
    /// This run began a new game (no save, or `?dev=1&newgame=1`; not the stand-in for a kept save): the drydock docks
    /// the start's ship.
    new_game: bool,
    /// For home's note block.
    pub notes: Vec<Note>,
}

/// Reads the save `saves/<slot>` (or starts a new game) at start. `seed` is a new game's seed, `now` the time
/// it is written at.
pub fn load(storage: &mut Storage, content: &Content, switches: Switches, seed: u64, now: u64) -> PlayerCareer {
    let Switches { dev, new_game: start_over } = switches;
    let slot = switches.slot().to_string();
    let key = format!("{}/{slot}", CareerDoc::NAMESPACE);
    let mut notes = Vec::new();
    if let Some(why) = &storage.fallback {
        notes.push(Note::new("NO STORAGE", format!("The game is kept only until it closes: {why}."), Tone::Gold));
    }
    let stored = if start_over { Ok(None) } else { storage.read::<CareerDoc>(&slot) };
    let mut kept = None;
    let mut replaced = false;
    let loaded = match stored {
        Ok(Some(s)) => Some(s.doc.0),
        Ok(None) => None,
        Err(e @ StorageError::Older { .. }) => {
            // No save compatibility before release (ADR-0044): a new game in its place.
            info!("career: {e}: a new game starts in its place");
            notes.push(Note::new("A NEW GAME", format!("{}: a new game starts in its place.", shortened(&e.to_string(), 90)), Tone::Gold));
            replaced = true;
            None
        }
        Err(e) => {
            // "saves/career doesn't read: line 1:35: …", "saves/career is from a newer version of the game (…)".
            let why = e.to_string();
            warn!("career: the save is kept as it is: {why}");
            notes.push(Note::new(
                "THE SAVE IS KEPT AS IT IS",
                format!("{}. This game is a new one and is not saved.", shortened(&why, 90)),
                Tone::Coral,
            ));
            kept = Some(why);
            None
        }
    };
    let (mut career, new_game) = match loaded {
        Some(mut career) => {
            let changed = career.reconcile(content);
            for p in &changed {
                warn!("career: {key}: {p}");
            }
            if let Some(first) = changed.first() {
                let more = if changed.len() > 1 { format!(" (and {} more)", changed.len() - 1) } else { String::new() };
                notes.push(Note::new("THE SAVE MEETS CHANGED CONTENT", format!("{}{more}", shortened(&first.message, 110)), Tone::Gold));
            }
            (career, false)
        }
        // A stand-in for a kept save is no new game: the drydock keeps its design.
        None => (new_career(content, seed, &mut notes, &mut kept, replaced), kept.is_none()),
    };
    let charted = chart(content, &mut career, &mut notes) && !new_game && kept.is_none();
    if dev {
        career.open_everything(content);
        let body = format!("Everything is open and the whole galaxy charted. This game is {key}; the career, {}/{DEFAULT_SLOT}, is untouched.", CareerDoc::NAMESPACE);
        notes.push(Note::new("DEV MODE  ·  ?DEV=1", body, Tone::Gold));
    }
    let mut out = PlayerCareer { career, slot, kept, dev, new_game, notes };
    if (new_game || charted || dev) && let Err(e) = out.write(storage, now) {
        out.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
    }
    out
}

/// Gives a career without a galaxy (a new game) the content's first galaxy, generated from its game seed (ADR-0035:
/// once; the save keeps it). True when it got one. Content without a galaxy leaves it without; a galaxy that doesn't
/// generate too, with a note.
fn chart(content: &Content, career: &mut Career, notes: &mut Vec<Note>) -> bool {
    let Some(g) = content.galaxies.iter().next().filter(|_| career.exploration().is_none()) else {
        return false;
    };
    match galaxy::generate(content, &g.id, career.seed()) {
        Ok(skeleton) => {
            career.begin_galaxy(content, skeleton);
            career.exploration().is_some()
        }
        Err(e) => {
            warn!("career: the galaxy doesn't generate: {e}");
            notes.push(Note::new("NO GALAXY", format!("The galaxy doesn't generate: {}.", shortened(&e.0, 90)), Tone::Coral));
            false
        }
    }
}

/// At most `n` characters of `s`, an ellipsis when cut (notes stay a few lines; the console has them whole).
fn shortened(s: &str, n: usize) -> String {
    match s.char_indices().nth(n) {
        Some((at, _)) => format!("{}…", s[..at].trim_end()),
        None => s.to_string(),
    }
}

/// A new game from the content's first start. Content without a start gets a career that unlocks only what the
/// content doesn't lock, with no mind and no voyages; it is never written.
fn new_career(content: &Content, seed: u64, notes: &mut Vec<Note>, kept: &mut Option<String>, replaced: bool) -> Career {
    if let Some(start) = content.starts.iter().next()
        && let Ok(career) = Career::new(content, &start.id, seed)
    {
        if kept.is_none() && !replaced {
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

    /// The dev mode (`?dev=1`).
    pub fn dev(&self) -> bool {
        self.dev
    }

    /// What the career grants now.
    pub fn granted(&self, content: &Content) -> Granted {
        let (unlocks, fresh) = if self.dev { (Unlocks::all(content), BTreeSet::new()) } else { (self.career.unlocks(content), self.career.fresh_parts(content)) };
        let stock = self.career.stock().clone();
        Granted { unlocks, fresh, stock, mind_capacity: self.career.mind_capacity(content), upgrades: self.career.upgrades(content), dev: self.dev }
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
    /// The parts, materials and frames the drydock offers (everything in the dev mode).
    pub unlocks: Unlocks,
    /// Parts research unlocked that the drydock hasn't shown yet (NEW tags; none in the dev mode).
    pub fresh: BTreeSet<String>,
    /// The workshop's stock (GDR-0108): parts by id the fleet is fitted from. The dev mode ignores it.
    pub stock: BTreeMap<String, u32>,
    /// Logic cores the ship-mind runs at once.
    pub mind_capacity: u32,
    /// The researched part-stat steps (GDR-0024's refinement) the player's designs are built with: in the drydock's
    /// numbers, the voyages, the sandbox and the lab's runs.
    pub upgrades: data::Upgrades,
    /// The dev mode (`?dev=1`).
    pub dev: bool,
}

impl Granted {
    /// The stock a launch needs: `None` (unlimited) in the dev mode.
    pub fn launch_stock(&self) -> Option<&BTreeMap<String, u32>> {
        (!self.dev).then_some(&self.stock)
    }

    /// How many of a part are in stock (`None`: unlimited, the dev mode).
    pub fn in_stock(&self, part: &str) -> Option<u32> {
        self.launch_stock().map(|s| s.get(part).copied().unwrap_or(0))
    }

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
        Readiness::MissingParts { part, need, have } => {
            format!("{} {} OF {need} IN STOCK", name(content.parts.get(part).map(|p| p.name.as_str()), part), have)
        }
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
    let list = voyage_ready(content, &granted.unlocks, granted.mind_capacity, granted.launch_stock(), flagship, escorts);
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
        if career.dev || !career.career.fresh_parts(&content.0).contains(part) {
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

    /// `?dev=1` picks the dev save; `?newgame=1` counts only with it (a note says so without).
    #[test]
    fn the_url_picks_the_dev_mode_and_a_new_dev_game() {
        assert_eq!((switches(""), switches("").slot()), (Switches { dev: false, new_game: false }, DEFAULT_SLOT));
        assert_eq!((switches("?dev=1&newgame=1"), switches("?dev").slot()), (Switches { dev: true, new_game: true }, DEV_SLOT));
        assert!(!switches("?dev=1&newgame=0").new_game && !switches("?dev=0").dev);
        let (s, note) = Switches::from_params(&Params::parse("?newgame=1"));
        assert_eq!(s, Switches::default());
        assert!(note.unwrap().body.contains("?dev=1"));
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
        assert_eq!((g.mind_capacity, g.fresh.len(), g.dev), (2, 0, false));
    }

    /// A save that doesn't read (broken, or from a newer build) is kept; the game runs a new career and never
    /// writes it. A new game replaces it.
    #[test]
    fn a_broken_or_newer_save_is_kept_and_not_overwritten() {
        let content = data::base_content();
        for text in ["// space-game: saves, format 5, saved 5\n(start: \"rim_awakening\", seed: ", "// space-game: saves, format 9, saved 5\n()"] {
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
            let fresh = load(&mut storage, &content, Switches { dev: false, new_game: true }, 2, 20);
            assert!(fresh.notes.iter().all(|n| n.tone != Tone::Coral), "{:?}", fresh.notes);
            assert_eq!(stored_text(&storage, "career"), Some(fresh.career().to_ron()));
        }
    }

    /// A save of an older format (here format 2, before the galaxy) is no longer read (ADR-0044): a new game with
    /// its galaxy starts in its place, with a note, and is written at once; read again, it keeps that galaxy.
    #[test]
    fn an_older_save_starts_a_new_game_in_its_place() {
        let content = data::base_content();
        let mut mem = MemoryBackend::default();
        let text = "// space-game: saves, format 2, saved 5\n(start: \"rim_awakening\", seed: 3, generator: 2, voyages: {\"yard_approach\": (launched: 2, cleared: true)})";
        crate::storage::Backend::write(&mut mem, "saves", "career", text).unwrap();
        let mut storage = Storage::new(mem);
        let c = load(&mut storage, &content, switches(""), 1, 10);
        assert!(c.new_game());
        let note = c.notes.iter().find(|n| n.head == "A NEW GAME").expect("a note on the new game");
        assert!(note.body.contains("older version") && note.body.ends_with("a new game starts in its place."), "{note:?}");
        assert_eq!(c.career().seed(), 1);
        assert_eq!(c.career().voyage("yard_approach"), None);
        let g = c.career().exploration().expect("charted");
        assert_eq!(*g.skeleton(), galaxy::generate(&content, "galaxy", 1).unwrap());
        assert_eq!(g.charted().len(), 1);
        let stored = storage.read::<CareerDoc>("career").unwrap().unwrap();
        assert_eq!((&stored.doc.0, stored.saved), (c.career(), Some(10)), "written at once");
        let again = load(&mut storage, &content, switches(""), 2, 20);
        assert!(again.notes.is_empty(), "{:?}", again.notes);
        assert_eq!(again.career(), c.career());
    }

    /// The dev mode plays its own save with everything open and leaves the career's as it is; `?newgame=1` starts
    /// the dev save over, never the career's.
    #[test]
    fn the_dev_mode_plays_its_own_save_with_everything_open() {
        let content = data::base_content();
        let mut storage = Storage::new(MemoryBackend::default());
        let mut c = load(&mut storage, &content, switches(""), 1, 10);
        let ((), w) = c.update(&mut storage, |k| k.set_escorts(vec!["drone".into()]));
        w.unwrap();
        let career_text = stored_text(&storage, DEFAULT_SLOT).unwrap();
        let mut d = load(&mut storage, &content, switches("?dev=1"), 2, 20);
        assert!(d.new_game() && d.dev() && d.career().escorts().is_empty());
        assert!(content.research.iter().all(|n| d.career().is_researched(&n.id)));
        assert!(d.notes.iter().any(|n| n.head.starts_with("DEV MODE")), "{:?}", d.notes);
        let g = d.granted(&content);
        assert!(g.dev && g.unlocks.has_part("beam_laser") && g.unlocks.has_frame("capital") && g.fresh.is_empty());
        assert_eq!((g.launch_stock(), g.in_stock("thruster")), (None, None));
        assert_eq!(storage.list("saves").unwrap(), [DEFAULT_SLOT, DEV_SLOT]);
        assert_eq!(stored_text(&storage, DEFAULT_SLOT).unwrap(), career_text);
        // The dev save keeps what was done in it; a new dev game starts it over.
        let ((), w) = d.update(&mut storage, |k| k.set_escorts(vec!["wing".into()]));
        w.unwrap();
        assert_eq!(load(&mut storage, &content, switches("?dev=1"), 3, 30).career().escorts(), ["wing"]);
        let n = load(&mut storage, &content, switches("?dev=1&newgame=1"), 4, 40);
        assert!(n.new_game() && n.career().escorts().is_empty() && n.career().seed() == 4);
        assert_eq!(stored_text(&storage, DEFAULT_SLOT).unwrap(), career_text);
    }

    /// Content changes reach a loaded save as a note (no panic); a gone escort design empties its slot.
    #[test]
    fn changed_content_and_gone_escorts_are_notes() {
        let content = data::base_content();
        let mut mem = MemoryBackend::default();
        let text = "// space-game: saves, format 5, saved 5\n(start: \"rim_awakening\", seed: 3, generator: 2, researched: [\"warp_theory\"], escorts: [\"kept\", \"gone\"])";
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
        // A second skiff: not in stock (GDR-0108) and too many cores; the stock is named first.
        let (line, _) = voyage_line(&content, &g, skiff, std::slice::from_ref(skiff));
        assert!(line.starts_with("NOT VOYAGE-READY  ·  CAPACITOR 2 OF 4 IN STOCK  ·  +"), "{line}");
        let stocked = Granted { stock: data::career::fleet_parts(&content, [skiff, skiff]), ..g.clone() };
        let (line, _) = voyage_line(&content, &stocked, skiff, std::slice::from_ref(skiff));
        assert_eq!(line, "NOT VOYAGE-READY  ·  4 CORES, YOUR MIND RUNS 2");
        let mut no_cargo = skiff.clone();
        no_cargo.parts.retain(|p| !p.part.starts_with("cargo"));
        let (line, tone) = voyage_line(&content, &g, &no_cargo, &[]);
        assert_eq!((line.as_str(), tone), ("VOYAGE-READY  ·  CORES 2 / 2  ·  NO CARGO  ·  YOU CAN'T BRING SALVAGE HOME", Tone::Gold));
        // The dev mode: only the mind and the frame limit.
        let dev = Granted { unlocks: Unlocks::all(&content), dev: true, ..g.clone() };
        assert_eq!(voyage_line(&content, &dev, runner, &[]).1, Tone::Teal);
        assert_eq!(unlocked_by(&content, &Unlock::Part("beam_laser".into())).map(|n| n.id.as_str()), Some("beam_laser"));
        assert!(unlocked_by(&content, &Unlock::Material("automaton_plate".into())).is_none());
    }
}
