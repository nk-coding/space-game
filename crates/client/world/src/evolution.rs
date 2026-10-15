//! The evolution lab's runs (GDR-0092 step 4; `docs/client.md` "Evolution"): what the EVOLUTION building at home
//! computes, keeps and saves. The layer over home (`game::evolution`) shows it; this module holds the runs, steps
//! them in the background and writes them.
//!
//! * **Runs** are `arena::Run`s, built from a [`Setup`] (the setup page's choices) as an `arena::RunConfig`: one to
//!   four starts (YOUR DESIGNS, templates, the drydock's working design) with their locks, the career's limits, the
//!   ENEMY SHIPS trial and the run's own hall of fame, a size and a seed. Every player design and behavior a run uses
//!   is inlined as text, so deleting or editing one later changes nothing in it.
//! * **One run computes at a time** ([`Lab::running`]); starting or resuming another pauses it. It computes at home
//!   and on every screen within a time budget per frame ([`BUDGET_OPEN_MS`] with the layer open, else
//!   [`BUDGET_MS`]) and pauses in flight: the test range, test flights, replays and voyages ([`compute`]). The
//!   budget is turned into ticks of work from the measured cost per tick ([`Rate`]); `arena::Run::step` breeds and
//!   fights within it, so frames keep their time whatever the fight. The same slices on every platform (the native
//!   thread executor of GDR-0092 comes later).
//! * **Storage**: each run is a document `lab/<id>` ([`RunDoc`], format 1): the run file (`arena::RunSave`) with its
//!   head's name, creation time and status, written after every generation (its only checkpoint: a reload breeds the
//!   open generation again from the same random state, the same run) and when it is paused, resumed or made. A run
//!   whose rules' fingerprint differs from today's is stale: read-only, it doesn't compute. At most [`MAX_RUNS`]
//!   (the web's local storage holds about 5 MB per origin).
//! * **SAVE AS DESIGN** ([`save_as_design`]): a genome into YOUR DESIGNS in its run's frame with an `origin` and the
//!   `evolved` tag; an evolved behavior is saved beside it as a player behavior.
//! * **`?lab=1`** opens the building until its research node comes (step 8); without it the building is dark.

use arena::evolve::{ArchiveShape, Axis, Bound, Evolve, Fitness, Genes, Limits, Locks, Measure, RunStatus, Start};
use arena::{BehaviorRef, DesignRef, Entry, Run, RunConfig, RunSave};
use bevy::platform::time::Instant;
use bevy::prelude::*;
use data::{Behavior, BehaviorEditor, Blueprint, Content, Origin};
use kit::params::Params;

use crate::career::Granted;
use crate::mode::AppMode;
use crate::player::PlayerDesign;
use crate::player::behaviors::{PlayerBehaviors, set_text, validate_design};
use crate::player::library::{self, DesignDoc, fresh_name, slug_or, unique_id};
use crate::sim_bridge::GameContent;
use crate::storage::{Document, Storage, now_unix};

/// The time a frame gives the run (ms; GDR-0092 "Running it", lead defaults, PLACEHOLDER): with the EVOLUTION layer
/// open, and anywhere else at home and on the screens. In flight it pauses.
pub const BUDGET_OPEN_MS: f32 = 10.0;
pub const BUDGET_MS: f32 = 4.0;
/// The cost of a tick of work assumed before one is measured (µs): wasm's, the slower (natively 5–6 µs).
const FIRST_US_PER_TICK: f32 = 9.0;
/// How fast the measured cost and throughput follow a frame (a running mean).
const RATE_FOLLOW: f32 = 0.1;
/// Frames with less work than this don't measure the cost (their timer's noise is larger than the work).
const RATE_MIN_TICKS: u64 = 200;
/// Most runs the lab keeps: the web's local storage holds about 5 MB per origin, shared with the designs, and a
/// STANDARD run is about 0.7 MB. PLACEHOLDER.
pub const MAX_RUNS: usize = 8;
/// Children, generations and seeds per opponent of the size presets (GDR-0092 "SIZE", PLACEHOLDER).
pub const SIZES: [(&str, Size); 3] = [
    ("QUICK", Size { population: 16, generations: 20, seeds: 2 }),
    ("STANDARD", Size { population: 32, generations: 50, seeds: 4 }),
    ("DEEP", Size { population: 64, generations: 100, seeds: 6 }),
];
/// The fights' time limit (s) and the share of it a fight lasts on average, for the setup's estimate. PLACEHOLDER.
pub const TIME_LIMIT: f32 = 60.0;
const MEAN_FIGHT_SHARE: f32 = 0.6;

/// A run as a stored document (`lab/<id>`): the run file, the CLI's format (`arena evolve --resume` reads it).
#[derive(Clone, Debug, PartialEq)]
pub struct RunDoc(pub RunSave);

impl Document for RunDoc {
    const NAMESPACE: &'static str = "lab";
    const FORMAT: u32 = 1;

    fn to_ron(&self) -> String {
        self.0.to_ron()
    }

    fn from_ron(text: &str, _format: u32) -> Result<Self, String> {
        RunSave::from_ron(text).map(RunDoc).map_err(|e| e.to_string())
    }
}

/// The measured cost of the run's work on this device.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rate {
    /// A tick of work (a fight's tick, breeding at `BREED_COST` a child), in µs.
    pub us_per_tick: f32,
    /// Ticks of work per second of wall time while a run computes (frames that compute nothing aside).
    pub ticks_per_second: f32,
    /// Measured at least once.
    pub measured: bool,
}

impl Default for Rate {
    fn default() -> Self {
        Rate { us_per_tick: FIRST_US_PER_TICK, ticks_per_second: 0.0, measured: false }
    }
}

impl Rate {
    /// The ticks of work a budget of `ms` buys.
    pub fn ticks_for(&self, ms: f32) -> u64 {
        ((ms * 1000.0 / self.us_per_tick.max(0.01)) as u64).max(1)
    }

    /// A frame did `ticks` of work in `spent` seconds, in a frame `frame` seconds long.
    fn note(&mut self, ticks: u64, spent: f32, frame: f32) {
        let follow = |old: f32, new: f32, first: bool| if first { new } else { old + (new - old) * RATE_FOLLOW };
        if ticks >= RATE_MIN_TICKS && spent > 0.0 {
            self.us_per_tick = follow(self.us_per_tick, spent * 1e6 / ticks as f32, !self.measured);
            if frame > 0.0 {
                self.ticks_per_second = follow(self.ticks_per_second, ticks as f32 / frame, !self.measured);
            }
            self.measured = true;
        }
    }
}

/// One run as the RUNS list shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct RunEntry {
    pub id: String,
    pub name: String,
    /// When it was made (Unix seconds).
    pub created: Option<u64>,
    pub status: RunStatus,
    /// Generations done, and in all.
    pub generation: u32,
    pub generations: u32,
    /// The archive's best score after the last generation done, and the best of generation 0 (the starts and
    /// their first mutants): `START 0.64 → BEST 0.91`.
    pub best: Option<f32>,
    pub first: Option<f32>,
    /// Its starts' names.
    pub starts: Vec<String>,
    /// Fought under rules that have changed since: read-only.
    pub stale: bool,
    /// Why it doesn't read or go on (listed anyway, so it can be deleted).
    pub problem: Option<String>,
}

impl RunEntry {
    pub fn done(&self) -> bool {
        self.generation >= self.generations
    }

    /// The run file's head and numbers as an entry; `stale` against today's rules.
    fn of(content: &Content, id: &str, save: &RunSave, rules: u64) -> RunEntry {
        RunEntry {
            id: id.to_string(),
            name: save.name.clone().unwrap_or_else(|| id.to_string()),
            created: save.created,
            status: save.status.unwrap_or(RunStatus::Paused),
            generation: save.history.len() as u32,
            generations: save.config.schedule().map_or(0, |s| s.iter().map(|g| g.1).sum()),
            best: save.history.last().map(|g| g.archive_best),
            first: save.history.first().map(|g| g.best),
            starts: save.config.all_starts().map(|s| start_name(content, s)).collect(),
            stale: save.rules != rules,
            problem: None,
        }
    }
}

/// A start's name: the content ship's, the blueprint's, or "Random design".
fn start_name(content: &Content, s: &Start) -> String {
    match s {
        Start::Design { design: DesignRef::Ship(id), .. } => content.ships.get(id).map_or_else(|| id.clone(), |b| b.name.clone()),
        Start::Design { design: DesignRef::Blueprint(text), .. } => data::parse_blueprint(text).map_or_else(|_| "A design".into(), |b| b.name),
        Start::Random { .. } => "Random design".into(),
    }
}

/// A run in memory: the run, its id and its head.
pub struct Loaded {
    pub id: String,
    pub run: Run,
    pub name: String,
    pub created: u64,
    pub status: RunStatus,
}

impl Loaded {
    /// The run file as it is written: the snapshot (the state after its last closed generation) with the head.
    pub fn save(&self) -> RunSave {
        RunSave { name: Some(self.name.clone()), created: Some(self.created), status: Some(self.status), ..self.run.snapshot() }
    }

    /// It computes when it may: running, not finished, not stale.
    pub fn computes(&self) -> bool {
        self.status == RunStatus::Running && !self.run.finished() && !self.run.stale()
    }
}

/// A start as the setup picks it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartPick {
    /// The drydock's working design (EVOLVE THIS DESIGN).
    Working,
    /// One of YOUR DESIGNS (`designs/<id>`).
    Design(String),
    /// A content ship of the player's faction (TEMPLATES).
    Template(String),
}

/// A start resolved for a run: what the setup lists of it (its parts, its rules) and what the run gets.
#[derive(Clone, Debug, PartialEq)]
pub struct StartDesign {
    pub name: String,
    /// What a saved result names as its origin: the saved design's id, the content ship's, else the name.
    pub from: String,
    /// The design as written (frame coordinates for a saved or working design), its `behavior` field empty.
    pub blueprint: Blueprint,
    /// The behavior it flies, as the run gets it, and as text (its rules listed for the locks).
    pub behavior: Option<BehaviorRef>,
    pub program: String,
    /// A content ship: the run names it by id.
    pub template: bool,
}

impl StartDesign {
    /// The run's start, with these locks.
    pub fn start(&self, locks: Locks) -> Start {
        let design = match self.template {
            true => DesignRef::Ship(self.from.clone()),
            false => DesignRef::Blueprint(data::blueprint_to_ron(&self.blueprint)),
        };
        Start::Design { design, behavior: self.behavior.clone(), locks }
    }
}

impl StartPick {
    /// The design and behavior of this start now: a saved design read from the storage, a content ship, the working
    /// design. A player behavior is inlined as text (the run never reads the player's storage); a content one goes
    /// by id.
    pub fn resolve(&self, storage: &Storage, content: &Content, behaviors: &PlayerBehaviors, working: &PlayerDesign) -> Result<StartDesign, String> {
        let (bp, from) = match self {
            StartPick::Working => {
                let bp = working.blueprint().clone();
                let from = library::slug(&bp.name);
                (bp, from)
            }
            StartPick::Design(id) => {
                let doc = storage.read::<DesignDoc>(id).map_err(|e| e.to_string())?.ok_or_else(|| format!("{id} is gone"))?;
                (doc.doc.0, id.clone())
            }
            StartPick::Template(id) => {
                let bp = content.ships.get(id).ok_or_else(|| format!("no ship {id}"))?.clone();
                let program = content.ship_behavior(&bp).and_then(|b| b.program().ok()).map(|p| p.to_text()).unwrap_or_default();
                return Ok(StartDesign { name: bp.name.clone(), from: id.clone(), behavior: None, program, blueprint: bp, template: true });
            }
        };
        let flies = bp.behavior.as_deref().map_or_else(|| content.ship_behavior(&bp), |id| behaviors.behavior(content, id));
        let program = flies.map(behavior_text).unwrap_or_default();
        let behavior = match bp.behavior.as_deref() {
            Some(id) if content.behaviors.contains(id) => Some(BehaviorRef::Content(id.to_string())),
            Some(_) if !program.is_empty() => Some(BehaviorRef::Text(program.clone())),
            _ => None,
        };
        // The run's text form names no behavior: the run carries it beside the design.
        let blueprint = Blueprint { behavior: None, origin: None, source: Default::default(), ..bp };
        Ok(StartDesign { name: blueprint.name.clone(), from, blueprint, behavior, program, template: false })
    }
}

/// A behavior's program text (its lines, the target line first).
fn behavior_text(b: &Behavior) -> String {
    b.program().map_or_else(|_| crate::player::behaviors::text_of(b), |p| p.to_text())
}

/// A behavior text in its canonical form without its notes (as a run keeps it), to tell whether two are the same.
fn bare(content: &Content, text: &str) -> Option<String> {
    let mut ed = BehaviorEditor::from_text(&content.behavior_nodes, text).ok()?;
    let rules: Vec<_> = ed.rules().map(|r| r.id).collect();
    for id in rules {
        ed.set_rule_note(id, None).ok()?;
    }
    ed.set_target_note(None).ok()?;
    Some(ed.text())
}

/// Children, generations and seeds per opponent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub population: u32,
    pub generations: u32,
    pub seeds: u32,
}

/// What the setup page has picked (GDR-0092 "The run setup", step 4's part of it).
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub name: String,
    /// One to four starts and their locks.
    pub starts: Vec<(StartPick, Locks)>,
    /// What may change: the design, the behavior or both.
    pub genes: Genes,
    /// ENEMY SHIPS: content ships tagged `enemy` and how many of each a fight has (0: not fought).
    pub opponents: Vec<(String, u32)>,
    /// The run's own best so far as opponents (HALL OF FAME).
    pub hall_of_fame: u32,
    pub size: Size,
    pub seed: u64,
    /// BUILD POINTS ≤ the start's, a number, or no cap; the hold at least the start's or not.
    pub build_points: Bound,
    pub min_hold: Bound,
    /// VOYAGE-READY: logic cores within the mind capacity less FLEET's escorts'.
    pub voyage_ready: bool,
    pub mirror: bool,
}

impl Setup {
    /// The defaults (GDR-0092, lead defaults until step 6: design and behavior, STANDARD): the starts given, two
    /// raiders and a hunter, a hall of fame of two, the start's build points and hold, voyage-ready.
    pub fn new(content: &Content, starts: Vec<StartPick>, seed: u64) -> Setup {
        let opponents = enemies(content).into_iter().map(|id| {
            let n = match id.as_str() {
                "automaton_raider" => 2,
                "automaton_hunter" => 1,
                _ => 0,
            };
            (id, n)
        });
        Setup {
            name: String::new(),
            starts: starts.into_iter().map(|s| (s, Locks::default())).collect(),
            genes: Genes::Both,
            opponents: opponents.collect(),
            hall_of_fame: 2,
            size: SIZES[1].1,
            seed,
            build_points: Bound::Start,
            min_hold: Bound::Start,
            voyage_ready: true,
            mirror: false,
        }
    }

    /// The fights a child has in a generation.
    pub fn jobs_per_child(&self) -> u32 {
        let opponents = self.opponents.iter().filter(|o| o.1 > 0).count() as u32;
        (opponents + self.hall_of_fame) * self.size.seeds
    }

    /// About how many ticks of work the whole run is (fights lasting [`MEAN_FIGHT_SHARE`] of their time limit).
    pub fn ticks(&self) -> f64 {
        let fight = (TIME_LIMIT * MEAN_FIGHT_SHARE * sim::TICK_HZ as f32) as f64;
        self.size.generations as f64 * self.size.population as f64 * (self.jobs_per_child() as f64 * fight + arena::evolve::BREED_COST as f64)
    }

    /// The run's configuration with these starts (resolved, in order), under the career's limits: only the parts,
    /// materials and counts it has (`?unlock=all` lifts them), and with VOYAGE-READY at most `free_cores` cores.
    pub fn config(&self, starts: &[StartDesign], granted: &Granted, free_cores: u32) -> RunConfig {
        let opponents = self.opponents.iter().filter(|o| o.1 > 0).map(|(id, n)| Entry { count: *n, ..Entry::ship(id) }).collect();
        let free = granted.free;
        let limits = Limits {
            frame: None,
            build_points: self.build_points,
            min_hold: self.min_hold,
            max_cores: self.voyage_ready.then_some(free_cores),
            counts: (!free && !granted.counts.is_empty()).then(|| granted.counts.clone()),
            parts: (!free).then(|| granted.unlocks.parts.clone()),
            materials: (!free).then(|| granted.unlocks.materials.clone()),
            mirror: self.mirror,
        };
        // A behavior run is spread by how it fights, a design run by what it is (`docs/arena.md` "The archive").
        let archive = match self.genes {
            Genes::Behavior => ArchiveShape {
                axes: vec![
                    Axis { measure: Measure::EngageDistance, min: 0.0, max: 160.0, cells: 8 },
                    Axis { measure: Measure::Rules, min: 0.0, max: 12.0, cells: 6 },
                ],
            },
            _ => ArchiveShape::default(),
        };
        RunConfig {
            seed: self.seed,
            evolve: match self.genes {
                Genes::Behavior => Evolve::Behavior,
                Genes::Design => Evolve::Design,
                Genes::Both => Evolve::Both,
            },
            generations: Some(self.size.generations),
            population: self.size.population.max(starts.len() as u32),
            fitness: Fitness::ArenaPool {
                opponents,
                seeds: self.size.seeds,
                hall_of_fame: self.hall_of_fame,
                count: 1,
                start_distance: arena::DEFAULT_START_DISTANCE,
                time_limit: TIME_LIMIT,
            },
            limits,
            start: None,
            starts: starts.iter().zip(&self.starts).map(|(s, (_, locks))| s.start(locks.clone())).collect(),
            archive,
            crossover: arena::evolve::DEFAULT_CROSSOVER,
            mutations: arena::evolve::DEFAULT_MUTATIONS,
        }
    }
}

/// The content ships an ENEMY SHIPS trial offers: those tagged `enemy`, in content order.
pub fn enemies(content: &Content) -> Vec<String> {
    content.ships.iter().filter(|s| s.tags.iter().any(|t| t == "enemy")).map(|s| s.id.clone()).collect()
}

/// The templates START FROM offers: the content ships of the player's faction (`faction`), in content order.
pub fn templates(content: &Content, faction: &str) -> Vec<String> {
    content.ships.iter().filter(|s| s.faction == faction).map(|s| s.id.clone()).collect()
}

/// The lab (a resource): the runs, the one loaded for the page and the one that computes, the measured rate.
#[derive(Resource, Default)]
pub struct Lab {
    /// `?lab=1`: the building works (its research node comes with GDR-0092 step 8).
    pub enabled: bool,
    /// Every run, newest first.
    runs: Vec<RunEntry>,
    /// The runs in memory: the one that computes and the one the page shows (at most these two).
    loaded: Vec<Loaded>,
    running: Option<String>,
    viewed: Option<String>,
    /// Today's rules' fingerprint (`arena::rules_fingerprint`, a few ms: once, when a run is first listed).
    rules: Option<u64>,
    pub rate: Rate,
    /// The EVOLUTION layer is open: the run gets [`BUDGET_OPEN_MS`].
    pub layer_open: bool,
    /// EVOLVE THIS DESIGN: the setup opens with this start.
    pub request: Option<StartPick>,
    /// A fight of a run is being watched: home comes back to the lab when flight ends.
    pub watching: bool,
    /// A run that finished in this session and hasn't been opened since: the building glows gold.
    pub fresh: Option<String>,
    /// What went wrong while computing (a write that failed), for the layer.
    pub trouble: Option<String>,
}

impl Lab {
    /// Lists the stored runs (newest first) and restores the one that was computing, so it goes on.
    pub fn load(storage: &Storage, content: &Content, enabled: bool) -> Lab {
        let mut lab = Lab { enabled, ..default() };
        let ids = match storage.list(RunDoc::NAMESPACE) {
            Ok(ids) => ids,
            Err(e) => {
                lab.trouble = Some(format!("the runs don't read: {e}"));
                return lab;
            }
        };
        let mut computing = None;
        for id in ids {
            let entry = match storage.read::<RunDoc>(&id) {
                Ok(Some(doc)) => {
                    let rules = lab.today(content);
                    let e = RunEntry::of(content, &id, &doc.doc.0, rules);
                    if e.status == RunStatus::Running && !e.done() && !e.stale && computing.is_none() {
                        computing = Some(id.clone());
                    }
                    e
                }
                Ok(None) => continue,
                Err(e) => RunEntry {
                    name: id.clone(),
                    id,
                    created: None,
                    status: RunStatus::Paused,
                    generation: 0,
                    generations: 0,
                    best: None,
                    first: None,
                    starts: Vec::new(),
                    stale: false,
                    problem: Some(e.to_string()),
                },
            };
            lab.runs.push(entry);
        }
        lab.runs.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.name.cmp(&b.name)));
        if let Some(id) = computing {
            match lab.load_run(storage, content, &id) {
                Ok(()) => lab.running = Some(id),
                Err(e) => lab.problem(&id, e),
            }
        }
        lab
    }

    fn today(&mut self, content: &Content) -> u64 {
        *self.rules.get_or_insert_with(|| arena::rules_fingerprint(content))
    }

    fn problem(&mut self, id: &str, why: String) {
        if let Some(e) = self.runs.iter_mut().find(|e| e.id == id) {
            e.problem = Some(why);
        }
    }

    /// Every run, newest first.
    pub fn runs(&self) -> &[RunEntry] {
        &self.runs
    }

    pub fn entry(&self, id: &str) -> Option<&RunEntry> {
        self.runs.iter().find(|e| e.id == id)
    }

    /// A run in memory.
    pub fn get(&self, id: &str) -> Option<&Loaded> {
        self.loaded.iter().find(|l| l.id == id)
    }

    /// The run that computes, if one does.
    pub fn running(&self) -> Option<&Loaded> {
        self.running.as_deref().and_then(|id| self.get(id))
    }

    /// The run the page shows, if it shows one.
    pub fn viewed(&self) -> Option<&Loaded> {
        self.viewed.as_deref().and_then(|id| self.get(id))
    }

    /// Reads and restores a stored run into memory (a no-op when it is there).
    fn load_run(&mut self, storage: &Storage, content: &Content, id: &str) -> Result<(), String> {
        if self.get(id).is_some() {
            return Ok(());
        }
        let doc = storage.read::<RunDoc>(id).map_err(|e| e.to_string())?.ok_or_else(|| format!("{id} is gone"))?;
        let save = doc.doc.0;
        let run = Run::restore(content, &save).map_err(|e| e.to_string())?;
        self.loaded.push(Loaded {
            id: id.to_string(),
            name: save.name.unwrap_or_else(|| id.to_string()),
            created: save.created.unwrap_or(0),
            status: save.status.unwrap_or(RunStatus::Paused),
            run,
        });
        Ok(())
    }

    /// Shows a run on the page (restored when it isn't in memory); `None` closes it. Runs neither shown nor
    /// computing leave memory.
    pub fn view(&mut self, storage: &Storage, content: &Content, id: Option<&str>) -> Result<(), String> {
        if let Some(id) = id {
            self.load_run(storage, content, id).inspect_err(|e| self.problem(id, e.clone()))?;
            if self.fresh.as_deref() == Some(id) {
                self.fresh = None;
            }
        }
        self.viewed = id.map(str::to_string);
        let (running, viewed) = (self.running.clone(), self.viewed.clone());
        self.loaded.retain(|l| Some(&l.id) == running.as_ref() || Some(&l.id) == viewed.as_ref());
        Ok(())
    }

    /// A new run from `config`, named `name`: it computes from now on (another that did is paused) and the page
    /// shows it. Refused when it doesn't build (why, in words) or [`MAX_RUNS`] runs are kept.
    pub fn create(&mut self, storage: &mut Storage, content: &Content, config: RunConfig, name: &str, now: u64) -> Result<String, String> {
        if self.runs.len() >= MAX_RUNS {
            return Err(format!("the lab keeps at most {MAX_RUNS} runs: delete one first"));
        }
        let run = Run::new(content, config).map_err(|e| e.to_string())?;
        let stored = storage.list(RunDoc::NAMESPACE).map_err(|e| e.to_string())?;
        let id = unique_id(&slug_or(name, "run"), |id| stored.iter().any(|s| s == id) || self.runs.iter().any(|e| e.id == id));
        self.pause_running(storage)?;
        let rules = self.today(content);
        let loaded = Loaded { id: id.clone(), run, name: name.to_string(), created: now, status: RunStatus::Running };
        let save = loaded.save();
        storage.write(&id, &RunDoc(save.clone()), now).map_err(|e| e.to_string())?;
        self.runs.insert(0, RunEntry::of(content, &id, &save, rules));
        self.loaded.push(loaded);
        self.running = Some(id.clone());
        self.view(storage, content, Some(&id))?;
        Ok(id)
    }

    /// Pauses the run that computes, if one does (written).
    fn pause_running(&mut self, storage: &mut Storage) -> Result<(), String> {
        match self.running.clone() {
            Some(id) => self.set_status(storage, &id, RunStatus::Paused),
            None => Ok(()),
        }
    }

    /// PAUSE and RESUME: resuming pauses the run that computed. A finished or stale run doesn't resume.
    pub fn set_status(&mut self, storage: &mut Storage, id: &str, status: RunStatus) -> Result<(), String> {
        if status == RunStatus::Running {
            let entry = self.entry(id).ok_or_else(|| format!("{id} is gone"))?;
            if entry.done() || entry.stale || entry.problem.is_some() {
                return Err("that run doesn't go on".into());
            }
            if self.running.as_deref() != Some(id) {
                self.pause_running(storage)?;
            }
        }
        let k = self.loaded.iter().position(|l| l.id == id);
        match k {
            Some(k) => {
                self.loaded[k].status = status;
                let save = self.loaded[k].save();
                storage.write(id, &RunDoc(save), now_unix()).map_err(|e| e.to_string())?;
            }
            // Not in memory (a paused run in the list): its document's head changes, the rest stays.
            None => {
                let doc = storage.read::<RunDoc>(id).map_err(|e| e.to_string())?.ok_or_else(|| format!("{id} is gone"))?;
                let save = RunSave { status: Some(status), ..doc.doc.0 };
                storage.write(id, &RunDoc(save), now_unix()).map_err(|e| e.to_string())?;
            }
        }
        if let Some(e) = self.runs.iter_mut().find(|e| e.id == id) {
            e.status = status;
        }
        match status {
            RunStatus::Running => self.running = Some(id.to_string()),
            RunStatus::Paused if self.running.as_deref() == Some(id) => self.running = None,
            RunStatus::Paused => {}
        }
        Ok(())
    }

    /// A paused run that is to go on must be in memory first.
    pub fn resume(&mut self, storage: &mut Storage, content: &Content, id: &str) -> Result<(), String> {
        self.load_run(storage, content, id)?;
        self.set_status(storage, id, RunStatus::Running)
    }

    /// DELETE (the page asked first): the document goes, and the run with it.
    pub fn delete(&mut self, storage: &mut Storage, id: &str) -> Result<(), String> {
        storage.delete::<RunDoc>(id).map_err(|e| e.to_string())?;
        self.runs.retain(|e| e.id != id);
        self.loaded.retain(|l| l.id != id);
        for slot in [&mut self.running, &mut self.viewed, &mut self.fresh] {
            if slot.as_deref() == Some(id) {
                *slot = None;
            }
        }
        Ok(())
    }

    /// One frame's work on the run that computes: `budget_ms` of it (turned into ticks with the measured rate),
    /// in a frame `frame` seconds long. A generation that closes is written (the run's checkpoint), and so is the
    /// run's end. `true` when a generation closed (the page and the building follow).
    pub fn compute(&mut self, storage: &mut Storage, budget_ms: f32, frame: f32) -> bool {
        let Some(id) = self.running.clone() else { return false };
        let ticks = self.rate.ticks_for(budget_ms);
        let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) else { return false };
        if !l.computes() {
            return false;
        }
        let (before, generation) = (l.run.worked(), l.run.generation());
        let t0 = Instant::now();
        let p = l.run.step(ticks);
        let spent = t0.elapsed().as_secs_f32();
        let used = l.run.worked() - before;
        self.rate.note(used, spent, frame);
        if p.generation == generation {
            return false;
        }
        let save = l.save();
        let written = storage.write(&id, &RunDoc(save.clone()), now_unix());
        let done = p.finished;
        if let Some(e) = self.runs.iter_mut().find(|e| e.id == id) {
            e.generation = save.history.len() as u32;
            e.best = save.history.last().map(|g| g.archive_best);
            e.first = save.history.first().map(|g| g.best);
        }
        if let Err(e) = written {
            // Not kept: the run waits, so no more work is lost than this generation's.
            self.trouble = Some(format!("not saved: {e}; the run is paused"));
            l.status = RunStatus::Paused;
            self.running = None;
            if let Some(e) = self.runs.iter_mut().find(|e| e.id == id) {
                e.status = RunStatus::Paused;
            }
        } else if done {
            self.running = None;
            self.fresh = Some(id);
        }
        true
    }
}

/// A saved result's name: `<start> · evolved g<generation>` (GDR-0092, lead default), at most the drydock's name
/// length.
pub fn result_name(start: &str, generation: u32) -> String {
    let mut name = format!("{start} · evolved g{generation}");
    if name.chars().count() > library::MAX_NAME {
        name = name.chars().take(library::MAX_NAME).collect();
    }
    name
}

/// SAVE AS DESIGN: genome `genome` of run `loaded` into YOUR DESIGNS as `name`, in its start's frame (as the drydock
/// saves designs), tagged `evolved`, with its `origin`. When its behavior evolved (or its start flew a player
/// behavior no longer saved), the behavior is saved beside it as a player behavior, `<name> (evolved)`, and the
/// design names it; else the design names its start's. Returns the design's id and the problems
/// `validate_design` finds (none for a genome of a run: the pre-filter checked it).
#[allow(clippy::too_many_arguments)]
pub fn save_as_design(
    storage: &mut Storage,
    content: &Content,
    behaviors: &mut PlayerBehaviors,
    loaded: &Loaded,
    genome: u32,
    name: &str,
    now: u64,
) -> Result<(String, Vec<data::Problem>), String> {
    let run = &loaded.run;
    let g = run.genome(genome).ok_or_else(|| format!("genome {genome} doesn't rebuild"))?;
    let frame = run.frame(g.root).ok_or("its start is gone")?;
    let (generation, score) = match run.scored(genome) {
        Some((generation, child)) => (generation, run.state().records[generation as usize].children[child].score),
        None => (0, 0.0),
    };
    let starts: Vec<&Start> = run.config().all_starts().collect();
    let root = *starts.get(g.root as usize).ok_or("its start is gone")?;
    let start_text = run.starts().get(g.root as usize).map(|s| s.behavior_text().to_string()).unwrap_or_default();
    let evolved = g.behavior_text() != start_text;
    let Start::Design { design, behavior: flew, .. } = root else { return Err("a random start has no design to keep".into()) };
    // The player behavior a start flew (inlined as text): the saved one with the same rules, if it is still there.
    let player = match flew {
        Some(BehaviorRef::Text(_)) => behaviors.iter().find(|p| bare(content, &behavior_text(&p.behavior)).as_deref() == Some(start_text.as_str())).map(|p| p.behavior.clone()),
        _ => None,
    };
    let behavior = match (evolved, flew, &player) {
        // As the start flew it: the content behavior it named, a template's own (else its faction's), the player's.
        (false, Some(BehaviorRef::Content(id)), _) => Some(id.clone()),
        (false, None, _) => match design {
            DesignRef::Ship(id) => content.ships.get(id).and_then(|b| b.behavior.clone()),
            DesignRef::Blueprint(_) => None,
        },
        (false, Some(BehaviorRef::Text(_)), Some(p)) => Some(p.id.clone()),
        // Evolved (or the player's is gone): saved beside the design.
        _ => {
            let base = player.as_ref().map(|p| p.name.clone()).or_else(|| content.ship_behavior(&g.blueprint).map(|b| b.name.clone())).unwrap_or_else(|| "Behavior".into());
            let mut b = Behavior {
                id: String::new(),
                name: fresh_name(&format!("{base} (evolved)"), &behaviors.names()),
                summary: format!("Evolved in the evolution lab ({}).", loaded.name),
                description: None,
                lore: None,
                tags: vec!["evolved".into()],
                target: None,
                rules: Vec::new(),
                source: Default::default(),
            };
            set_text(&mut b, g.behavior_text());
            Some(behaviors.save(storage, content, &b, now)?)
        }
    };
    let from = starts.iter().map(|s| match s {
        Start::Design { design: DesignRef::Ship(id), .. } => id.clone(),
        Start::Design { design: DesignRef::Blueprint(text), .. } => data::parse_blueprint(text).map_or_else(|_| String::new(), |b| if b.id.is_empty() { library::slug(&b.name) } else { b.id }),
        Start::Random { faction } => faction.clone(),
    });
    let mut from: Vec<String> = from.collect();
    // Its own start's first.
    if (g.root as usize) < from.len() {
        let own = from.remove(g.root as usize);
        from.insert(0, own);
    }
    let id = library::new_id(storage, content, name).map_err(|e| e.to_string())?;
    let bp = Blueprint {
        id: id.clone(),
        name: name.to_string(),
        behavior,
        origin: Some(Origin { run: loaded.id.clone(), genome, generation, from, score }),
        ..g.placed(frame)
    };
    let problems = validate_design(content, behaviors, &bp);
    storage.write(&id, &DesignDoc(bp), now).map_err(|e| e.to_string())?;
    Ok((id, problems))
}

/// The budget the run gets this frame: none in flight (it pauses), more while the layer is open.
pub fn budget(mode: AppMode, layer_open: bool) -> f32 {
    match mode {
        AppMode::Flight => 0.0,
        _ if layer_open => BUDGET_OPEN_MS,
        _ => BUDGET_MS,
    }
}

/// Every frame at home and on the screens: the run that computes gets its budget (none in flight).
pub fn compute(mode: Res<State<AppMode>>, time: Res<Time<Real>>, mut lab: ResMut<Lab>, mut storage: ResMut<Storage>) {
    let ms = budget(*mode.get(), lab.layer_open);
    if ms <= 0.0 || lab.running.is_none() {
        return;
    }
    // Only a closed generation is news for the screens that show the lab (they read the progress inside one
    // themselves while they show it).
    if lab.bypass_change_detection().compute(&mut storage, ms, time.delta_secs()) {
        lab.set_changed();
    }
}

pub struct EvolutionPlugin;

impl Plugin for EvolutionPlugin {
    fn build(&self, app: &mut App) {
        let enabled = app.world().resource::<Params>().flag("lab").unwrap_or(false);
        let lab = {
            let world = app.world();
            Lab::load(world.resource::<Storage>(), &world.resource::<GameContent>().0, enabled)
        };
        if let Some(t) = &lab.trouble {
            warn!("lab: {t}");
        }
        app.insert_resource(lab).add_systems(Update, compute);
    }
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;

    use super::*;
    use crate::storage::MemoryBackend;

    /// A small run of the starter: two generations of four children, one raider, short fights.
    fn setup(content: &Content) -> Setup {
        let mut s = Setup::new(content, vec![StartPick::Template("rim_skiff".into())], 7);
        s.size = Size { population: 4, generations: 3, seeds: 1 };
        s.hall_of_fame = 1;
        for o in &mut s.opponents {
            o.1 = u32::from(o.0 == "automaton_raider");
        }
        s
    }

    fn config(content: &Content, s: &Setup, storage: &Storage) -> RunConfig {
        let design = PlayerDesign::starter(content, "rim_skiff");
        let starts: Vec<StartDesign> = s.starts.iter().map(|(p, _)| p.resolve(storage, content, &PlayerBehaviors::default(), &design).unwrap()).collect();
        let granted = Granted { unlocks: data::career::Unlocks::all(content), mind_capacity: 4, free: true, ..default() };
        s.config(&starts, &granted, 4)
    }

    fn mem() -> Storage {
        Storage::new(MemoryBackend::default())
    }

    /// A frame's work is the ticks its budget buys at the measured rate, no more (a generation that closes ends it
    /// early); the rate follows what frames measure.
    #[test]
    fn a_frame_works_within_its_budget() {
        let content = data::base_content();
        let mut storage = mem();
        let s = setup(&content);
        let mut lab = Lab::load(&storage, &content, true);
        let cfg = config(&content, &s, &storage);
        let id = lab.create(&mut storage, &content, cfg, "Small", 10).unwrap();
        let mut k = 0;
        while lab.running().is_some() {
            k += 1;
            let ticks = lab.rate.ticks_for(BUDGET_MS);
            let before = lab.get(&id).unwrap().run.worked();
            let closed = lab.compute(&mut storage, BUDGET_MS, 1.0 / 60.0);
            let used = lab.get(&id).unwrap().run.worked() - before;
            assert!(used <= ticks + arena::evolve::BREED_COST, "frame {k}: {used} ticks of a budget of {ticks}");
            assert!(closed || used >= ticks, "frame {k}: an open generation uses its whole budget ({used} of {ticks})");
        }
        assert!(lab.rate.measured && lab.rate.us_per_tick > 0.0 && lab.rate.ticks_per_second > 0.0);
        // Budgets: none in flight, more with the layer open.
        assert_eq!(budget(AppMode::Flight, true), 0.0);
        assert_eq!((budget(AppMode::Home, true), budget(AppMode::Drydock, false), budget(AppMode::Map, false)), (BUDGET_OPEN_MS, BUDGET_MS, BUDGET_MS));
        let r = Rate { us_per_tick: 5.0, ..default() };
        assert_eq!((r.ticks_for(10.0), r.ticks_for(4.0)), (2000, 800));
    }

    fn app(storage: Storage, lab: Lab, mode: AppMode) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin)).insert_state(mode).insert_resource(storage).insert_resource(lab).add_systems(Update, compute);
        app
    }

    /// The run computes at home and on the screens, and waits in flight (where it was, nothing lost).
    #[test]
    fn the_run_pauses_in_flight() {
        let content = data::base_content();
        let mut storage = mem();
        let s = setup(&content);
        let mut lab = Lab::load(&storage, &content, true);
        let cfg = config(&content, &s, &storage);
        let id = lab.create(&mut storage, &content, cfg, "Small", 10).unwrap();
        let mut app = app(storage, lab, AppMode::Flight);
        let worked = |app: &App| app.world().resource::<Lab>().get(&id).unwrap().run.worked();
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(worked(&app), 0, "nothing in flight");
        app.world_mut().resource_mut::<NextState<AppMode>>().set(AppMode::Drydock);
        app.update();
        app.update();
        let at_dock = worked(&app);
        assert!(at_dock > 0, "it computes on the screens");
        app.world_mut().resource_mut::<NextState<AppMode>>().set(AppMode::Flight);
        app.update();
        let flying = worked(&app);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(worked(&app), flying, "and waits again in flight");
    }

    /// A run started, reloaded after a generation and part of another (a new lab from the same storage: a reload of
    /// the page) and run to its end is the run that ran through: the same run file byte for byte.
    #[test]
    fn a_reloaded_run_goes_on_as_the_uninterrupted_one() {
        let content = data::base_content();
        let s = setup(&content);
        let run_to_end = |lab: &mut Lab, storage: &mut Storage| {
            while lab.running().is_some() {
                lab.compute(storage, BUDGET_OPEN_MS, 1.0 / 60.0);
            }
        };
        let mut storage = mem();
        let mut lab = Lab::load(&storage, &content, true);
        let cfg = config(&content, &s, &storage);
        let id = lab.create(&mut storage, &content, cfg, "Small", 10).unwrap();
        run_to_end(&mut lab, &mut storage);
        let straight = storage.read::<RunDoc>(&id).unwrap().unwrap().doc.0;
        assert_eq!(straight.history.len(), 3);
        assert_eq!(lab.fresh.as_deref(), Some(id.as_str()), "a finished run glows until it is opened");

        let mut storage = mem();
        let mut lab = Lab::load(&storage, &content, true);
        let cfg = config(&content, &s, &storage);
        let id = lab.create(&mut storage, &content, cfg, "Small", 10).unwrap();
        while lab.get(&id).unwrap().run.generation() < 1 {
            lab.compute(&mut storage, BUDGET_MS, 1.0 / 60.0);
        }
        for _ in 0..3 {
            lab.compute(&mut storage, BUDGET_MS, 1.0 / 60.0);
        }
        // The page reloads: the lab is read from the storage again and the run goes on.
        let mut lab = Lab::load(&storage, &content, true);
        let entry = lab.entry(&id).unwrap().clone();
        assert_eq!((entry.generation, entry.status, entry.stale, entry.name.as_str()), (1, RunStatus::Running, false, "Small"));
        assert_eq!(lab.running().map(|l| l.id.as_str()), Some(id.as_str()));
        run_to_end(&mut lab, &mut storage);
        let resumed = storage.read::<RunDoc>(&id).unwrap().unwrap().doc.0;
        assert_eq!(resumed.to_ron(), straight.to_ron(), "byte for byte");
    }

    /// Pausing waits and is written; resuming another pauses the one that computed; a finished run doesn't resume;
    /// DELETE removes the document; old storage without `lab/` reads as no runs.
    #[test]
    fn runs_pause_resume_and_delete() {
        let content = data::base_content();
        let mut storage = mem();
        // An old storage: designs and a save, no lab.
        storage.write("old", &DesignDoc(content.ships.get("rim_skiff").unwrap().clone()), 1).unwrap();
        let mut lab = Lab::load(&storage, &content, true);
        assert!(lab.runs().is_empty() && lab.trouble.is_none() && lab.running().is_none());
        let s = setup(&content);
        let cfg = config(&content, &s, &storage);
        let a = lab.create(&mut storage, &content, cfg, "Run A", 10).unwrap();
        let cfg = config(&content, &s, &storage);
        let b = lab.create(&mut storage, &content, cfg, "Run A", 20).unwrap();
        assert_ne!(a, b);
        assert_eq!(lab.runs().iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), [b.as_str(), a.as_str()], "newest first");
        assert_eq!(lab.entry(&a).unwrap().status, RunStatus::Paused, "the new run paused the other");
        assert_eq!(storage.read::<RunDoc>(&a).unwrap().unwrap().doc.0.status, Some(RunStatus::Paused));
        lab.resume(&mut storage, &content, &a).unwrap();
        assert_eq!((lab.entry(&b).unwrap().status, lab.running().unwrap().id.as_str()), (RunStatus::Paused, a.as_str()));
        lab.set_status(&mut storage, &a, RunStatus::Paused).unwrap();
        assert!(lab.running().is_none());
        assert!(!lab.compute(&mut storage, BUDGET_MS, 0.016));
        lab.delete(&mut storage, &a).unwrap();
        assert_eq!(storage.list(RunDoc::NAMESPACE).unwrap(), [b.clone()]);
        assert!(lab.entry(&a).is_none() && lab.get(&a).is_none());
        // A stored run that doesn't read is listed with why.
        let mut text_store = MemoryBackend::default();
        crate::storage::Backend::write(&mut text_store, "lab", "broken", "// space-game: lab, format 1\n(format: 1").unwrap();
        let broken = Lab::load(&Storage::new(text_store), &content, true);
        assert!(broken.entry("broken").unwrap().problem.is_some());
    }

    /// SAVE AS DESIGN: the best genome becomes a saved design in the run's frame with its origin and the EVOLVED tag;
    /// it reads back, validates and opens in the drydock as it was saved; an evolved behavior is saved beside it.
    #[test]
    fn save_as_design_keeps_its_origin_and_opens_in_the_drydock() {
        let content = data::base_content();
        let mut storage = mem();
        let s = setup(&content);
        let mut lab = Lab::load(&storage, &content, true);
        let cfg = config(&content, &s, &storage);
        let id = lab.create(&mut storage, &content, cfg, "Small", 10).unwrap();
        while lab.running().is_some() {
            lab.compute(&mut storage, BUDGET_OPEN_MS, 0.016);
        }
        let loaded = lab.get(&id).unwrap();
        let best = loaded.run.results().elites[0].clone();
        let mut behaviors = PlayerBehaviors::default();
        let name = result_name("Rim skiff", best.generation);
        assert_eq!(name, format!("Rim skiff · evolved g{}", best.generation));
        let (saved, problems) = save_as_design(&mut storage, &content, &mut behaviors, loaded, best.id, &name, 99).unwrap();
        assert!(problems.iter().all(|p| !p.is_error()), "{problems:?}");
        let doc = storage.read::<DesignDoc>(&saved).unwrap().unwrap();
        let bp = doc.doc.0;
        assert_eq!((bp.id.as_str(), bp.name.as_str(), doc.saved), (saved.as_str(), name.as_str(), Some(99)));
        assert!(bp.tags.contains(&"evolved".to_string()));
        let origin = bp.origin.clone().unwrap();
        assert_eq!((origin.run.as_str(), origin.genome, origin.generation, origin.from.as_slice()), (id.as_str(), best.id, best.generation, ["rim_skiff".to_string()].as_slice()));
        assert_eq!(origin.score, best.score);
        assert_eq!(bp.size, loaded.run.frame(best.genome.root).unwrap(), "in the run's frame, as the drydock saves");
        let opened = library::open_saved(&storage, &content, &saved).unwrap();
        assert_eq!(opened.blueprint().parts.len(), bp.parts.len());
        // As the drydock checks what it opened (`blueprints/`): exported, under its id.
        let reopened = validate_design(&content, &behaviors, &Blueprint { id: saved.clone(), ..opened.export() });
        assert_eq!(opened.export().origin, bp.origin, "the drydock keeps the origin");
        assert!(reopened.iter().all(|p| !p.is_error()), "{reopened:?}");
        match &bp.behavior {
            // The behavior evolved: saved beside it, and it runs.
            Some(b) if behaviors.get(b).is_some() => {
                let p = behaviors.get(b).unwrap();
                assert!(p.runs() && p.behavior.name.ends_with("(evolved)"), "{p:?}");
                assert_eq!(bare(&content, &behavior_text(&p.behavior)).unwrap(), best.genome.behavior_text());
            }
            // It didn't: the template's own (none named: its faction's).
            other => assert_eq!(other, &content.ships.get("rim_skiff").unwrap().behavior),
        }
        // Old designs (format 1, no origin) still read.
        let mut old = MemoryBackend::default();
        let text = data::blueprint_to_ron(content.ships.get("rim_skiff").unwrap());
        crate::storage::Backend::write(&mut old, "designs", "older", &format!("// space-game: designs, format 1, saved 5\n{text}")).unwrap();
        let old = Storage::new(old);
        assert_eq!(old.read::<DesignDoc>("older").unwrap().unwrap().doc.0.origin, None);
    }

    /// A start picked from the working design inlines it (no behavior field: the run carries the behavior beside it)
    /// and names it by its name; a template goes by id.
    #[test]
    fn starts_resolve_self_contained() {
        let content = data::base_content();
        let storage = mem();
        let design = PlayerDesign::starter(&content, "rim_skiff");
        let behaviors = PlayerBehaviors::default();
        let w = StartPick::Working.resolve(&storage, &content, &behaviors, &design).unwrap();
        assert_eq!((w.name.as_str(), w.blueprint.behavior.as_deref()), (design.blueprint().name.as_str(), None));
        assert!(matches!(w.start(Locks::default()), Start::Design { design: DesignRef::Blueprint(_), .. }));
        assert!(!w.program.is_empty());
        let t = StartPick::Template("halvex_runner".into()).resolve(&storage, &content, &behaviors, &design).unwrap();
        assert!(matches!(t.start(Locks::default()), Start::Design { design: DesignRef::Ship(ref id), .. } if id == "halvex_runner"));
        assert_eq!(templates(&content, "ship_mind").first().map(String::as_str), Some("halvex_runner"));
        assert!(enemies(&content).contains(&"automaton_raider".to_string()));
    }
}
