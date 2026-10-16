//! The evolution lab's runs (GDR-0092 step 4; `docs/client.md` "Evolution"): what the EVOLUTION building at home
//! computes, keeps and saves. The layer over home (`game::evolution`) shows it; this module holds the runs, steps
//! them in the background and writes them.
//!
//! * **Runs** are `arena::Run`s, built from a [`Setup`] (the setup page's choices) as an `arena::RunConfig`: one to
//!   four starts (YOUR DESIGNS, templates, the drydock's working design) with their locks, the career's limits, the
//!   trials (`lab.ron`'s presets with their weights and seeds, filled in from the career's open voyages, YOUR
//!   DESIGNS, the lab's earlier runs and FLEET's escorts: [`Sources`]) and the goals' weights, a size and a seed. Every
//!   player design and behavior a run uses is inlined as text, so deleting or editing one later changes nothing in it.
//! * **IMPORT RUN** ([`Lab::import`]) takes a CLI run file (`arena evolve --save`); **DUPLICATE** makes the same
//!   setup again with a new seed, **RENAME** names a run anew; **RE-SCORE** ([`Lab::rescore`]) makes a stale run a new
//!   run under today's rules; **VALIDATE** ([`Lab::validate`]) fights a run's best designs on new seeds and their
//!   recorded fights again, computed like a generation (the run waits meanwhile) and kept in its file.
//! * **One run computes at a time** ([`Lab::running`]); starting or resuming another pauses it. It computes at home
//!   and on every screen within a time budget per frame ([`BUDGET_OPEN_MS`] with the layer open, else
//!   [`BUDGET_MS`]) and pauses in flight: the test range, test flights, replays and voyages ([`compute`]). The
//!   budget is turned into ticks of work from the measured cost per tick ([`Rate`]). On the web `arena::Run::step`
//!   breeds and fights within it on the main thread ([`Lab::compute`]), so frames keep their time whatever the fight;
//!   natively the frame's budget breeds and the fights run on Bevy's compute threads ([`Lab::compute_threads`]). Both
//!   make the same run.
//! * **Storage**: each run is a document `lab/<id>` ([`RunDoc`], format 1): the run file (`arena::RunSave`) with its
//!   head's name, creation time and status, written after every generation (its only checkpoint: a reload breeds the
//!   open generation again from the same random state, the same run) and when it is paused, resumed or made. A run
//!   whose rules' fingerprint differs from today's is stale: read-only, it doesn't compute. At most [`MAX_RUNS`]
//!   (the web's local storage holds about 5 MB per origin).
//! * **SAVE AS DESIGN** ([`save_as_design`]): a genome into YOUR DESIGNS in its run's frame with an `origin` and the
//!   `evolved` tag; an evolved behavior is saved beside it as a player behavior.
//! * **`?lab=1`** opens the building until its research node comes (step 8); without it the building is dark.

use arena::evolve::{ArchiveShape, Axis, Bound, Evolve, Genes, Goals, Limits, Locks, Measure, RunStatus, Sources, Start, TrialSpec, VALIDATE_SEEDS, Validation};
use arena::{BehaviorRef, DesignRef, Entry, Run, RunConfig, RunSave};
use bevy::platform::time::Instant;
use bevy::prelude::*;
use data::{Behavior, BehaviorEditor, Blueprint, Content, Origin, TrialPresetKind};
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
/// Children and generations of the size presets (GDR-0092 "SIZE", PLACEHOLDER; the seeds are each trial's, from
/// `lab.ron`, since step 5).
pub const SIZES: [(&str, Size); 3] = [
    ("QUICK", Size { population: 16, generations: 20 }),
    ("STANDARD", Size { population: 32, generations: 50 }),
    ("DEEP", Size { population: 64, generations: 100 }),
];
/// The share of its time limit a job lasts on average, for the setup's estimate: a fight, a gauntlet (its time limit
/// is for all its waves), a voyage. Measured 16 Oct 2026 (`docs/arena.md` "What a trial costs": 0.40–0.52, 0.25–0.34,
/// 0.13 for designs early in a run); PLACEHOLDER.
const FIGHT_SHARE: f32 = 0.5;
const GAUNTLET_SHARE: f32 = 0.35;
const VOYAGE_SHARE: f32 = 0.25;

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
            // The working design's header id is the drydock's own (`player:design`), no content id: it goes by a slug
            // of its name, as a first save would name it.
            StartPick::Working => {
                let from = library::slug(&working.blueprint().name);
                (Blueprint { id: from.clone(), ..working.blueprint().clone() }, from)
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

/// Children and generations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub population: u32,
    pub generations: u32,
}

/// A trial as the setup picks it (GDR-0092 "TRIALS"): a `lab.ron` preset, on or off, its weight and seeds, and what the
/// setup may change of what it fights: ENEMY SHIPS' count of each enemy ship, how many of the best HALL OF FAME and
/// CHAMPIONS fight, how many waves TEST-RANGE WAVES must clear.
#[derive(Clone, Debug, PartialEq)]
pub struct TrialPick {
    pub preset: String,
    pub on: bool,
    pub weight: f32,
    pub seeds: u32,
    /// ENEMY SHIPS: every content ship tagged `enemy` and how many of it a fight has (0: not fought).
    pub ships: Vec<(String, u32)>,
    /// HALL OF FAME, CHAMPIONS: how many; TEST-RANGE WAVES: the waves to clear.
    pub count: Option<u32>,
}

impl TrialPick {
    /// The preset as the setup has it: weights and seeds aside, its ships and count.
    pub fn preset(&self, content: &Content) -> Option<data::TrialPreset> {
        let mut p = content.lab.trial(&self.preset)?.clone();
        match (&mut p.kind, self.count) {
            (TrialPresetKind::EnemyShips { ships }, _) => {
                *ships = self.ships.iter().filter(|s| s.1 > 0).map(|(ship, count)| data::LabShips { ship: ship.clone(), count: *count }).collect();
            }
            (TrialPresetKind::Waves { target: n } | TrialPresetKind::HallOfFame { count: n } | TrialPresetKind::Champions { count: n }, Some(c)) => *n = c,
            _ => {}
        }
        Some(p)
    }

    /// The jobs it gives a child and how long one lasts on average (ticks), with `starts` starts and what `src` holds
    /// (for the estimate: the hall of fame counted full).
    pub fn cost(&self, content: &Content, starts: usize, src: &Sources) -> (u32, f32) {
        let Some(p) = self.preset(content).filter(|_| self.on && self.weight > 0.0) else { return (0, 0.0) };
        let fight = p.time_limit.unwrap_or(arena::DEFAULT_TIME_LIMIT) * sim::TICK_HZ as f32;
        let (opponents, ticks) = match &p.kind {
            TrialPresetKind::Encounters => (arena::evolve::encounters(content, &src.open_voyages).map_or(0, |e| e.len()), fight * FIGHT_SHARE),
            TrialPresetKind::EnemyShips { ships } => (ships.len(), fight * FIGHT_SHARE),
            TrialPresetKind::Waves { .. } | TrialPresetKind::Gauntlet { .. } => (1, fight * GAUNTLET_SHARE),
            TrialPresetKind::BeatYourStart => (starts, fight * FIGHT_SHARE),
            TrialPresetKind::Fleet => (src.fleet.len(), fight * FIGHT_SHARE),
            TrialPresetKind::HallOfFame { count } => (*count as usize, fight * FIGHT_SHARE),
            TrialPresetKind::Champions { count } => ((*count as usize).min(src.champions.len()), fight * FIGHT_SHARE),
            TrialPresetKind::Voyages => {
                let minutes = p.time_limit.unwrap_or(arena::DEFAULT_VOYAGE_MINUTES);
                (src.open_voyages.len(), minutes * 60.0 * sim::TICK_HZ as f32 * VOYAGE_SHARE)
            }
        };
        (opponents as u32 * self.seeds, ticks)
    }
}

/// What the setup page has picked (GDR-0092 "The run setup").
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub name: String,
    /// One to four starts and their locks.
    pub starts: Vec<(StartPick, Locks)>,
    /// What may change: the design, the behavior or both.
    pub genes: Genes,
    /// Every `lab.ron` trial preset, on or off.
    pub trials: Vec<TrialPick>,
    /// The goals' weights.
    pub goals: Goals,
    /// WITH YOUR ESCORTS: FLEET's escorts fly beside the design in every trial.
    pub escorts: bool,
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
    /// The defaults (GDR-0092, lead defaults until step 6: design and behavior, STANDARD): the starts given, `lab.ron`'s
    /// default trials and goal weights, the start's build points and hold, voyage-ready, no escorts beside it.
    pub fn new(content: &Content, starts: Vec<StartPick>, seed: u64) -> Setup {
        let trials = content
            .lab
            .trials
            .iter()
            .map(|p| {
                let ships = match &p.kind {
                    TrialPresetKind::EnemyShips { ships } => enemies(content)
                        .into_iter()
                        .map(|id| {
                            let n = ships.iter().find(|s| s.ship == id).map_or(0, |s| s.count);
                            (id, n)
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                let count = match &p.kind {
                    TrialPresetKind::Waves { target: n } | TrialPresetKind::HallOfFame { count: n } | TrialPresetKind::Champions { count: n } => Some(*n),
                    _ => None,
                };
                TrialPick { preset: p.id.clone(), on: p.default, weight: p.weight, seeds: p.seeds, ships, count }
            })
            .collect();
        let goals = Goals::of(content.lab.goals.iter().map(|g| (g.id.clone(), g.weight)));
        Setup {
            name: String::new(),
            starts: starts.into_iter().map(|s| (s, Locks::default())).collect(),
            genes: Genes::Both,
            trials,
            goals,
            escorts: false,
            size: SIZES[1].1,
            seed,
            build_points: Bound::Start,
            min_hold: Bound::Start,
            voyage_ready: true,
            mirror: false,
        }
    }

    /// The jobs a child has in a generation, and their ticks on average in all, with what `src` holds.
    pub fn per_child(&self, content: &Content, src: &Sources) -> (u32, f64) {
        self.trials.iter().fold((0, 0.0), |(n, t), p| {
            let (jobs, ticks) = p.cost(content, self.starts.len(), src);
            (n + jobs, t + jobs as f64 * ticks as f64)
        })
    }

    /// About how many ticks of work the whole run is (every job lasting its kind's share of its time limit, breeding at
    /// `BREED_COST` a child).
    pub fn ticks(&self, content: &Content, src: &Sources) -> f64 {
        let (_, child) = self.per_child(content, src);
        self.size.generations as f64 * self.size.population as f64 * (child + arena::evolve::BREED_COST as f64)
    }

    /// The run's configuration with these starts (resolved, in order), under the career's limits: only the parts,
    /// materials and counts it has (`?unlock=all` lifts them), and with VOYAGE-READY at most `free_cores` cores; the
    /// trials that are on, filled in from `src`. Refused in words when a trial has nothing to fight or none is on.
    pub fn config(&self, content: &Content, starts: &[StartDesign], granted: &Granted, free_cores: u32, src: &Sources) -> Result<RunConfig, String> {
        let mut trials = Vec::new();
        for pick in self.trials.iter().filter(|t| t.on && t.weight > 0.0) {
            let p = pick.preset(content).ok_or_else(|| format!("no trial `{}`", pick.preset))?;
            trials.push(TrialSpec::from_preset(&p, pick.weight, pick.seeds, self.escorts, src)?);
        }
        if trials.is_empty() {
            return Err("turn a trial on: nothing would score the designs".into());
        }
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
        Ok(RunConfig {
            seed: self.seed,
            evolve: match self.genes {
                Genes::Behavior => Evolve::Behavior,
                Genes::Design => Evolve::Design,
                Genes::Both => Evolve::Both,
            },
            generations: Some(self.size.generations),
            population: self.size.population.max(starts.len() as u32),
            fitness: None,
            trials,
            goals: self.goals,
            limits,
            start: None,
            starts: starts.iter().zip(&self.starts).map(|(s, (_, locks))| s.start(locks.clone())).collect(),
            archive,
            crossover: arena::evolve::DEFAULT_CROSSOVER,
            mutations: arena::evolve::DEFAULT_MUTATIONS,
        })
    }
}

/// A player design as a fight entry (YOUR FLEET, the escorts): its blueprint and the behavior it flies inlined, as a
/// start is (`StartPick::resolve`); a template by id.
pub fn design_entry(d: &StartDesign) -> Entry {
    let design = match d.template {
        true => DesignRef::Ship(d.from.clone()),
        false => DesignRef::Blueprint(data::blueprint_to_ron(&d.blueprint)),
    };
    arena::evolve::entry_of(design, d.behavior.clone())
}

/// What the trials fight from outside the run, now: the career's open voyages, YOUR DESIGNS for YOUR FLEET (FLEET's
/// escorts first), the lab's earlier runs' best designs for CHAMPIONS ([`Lab::champions`]), FLEET's escorts.
pub fn sources(storage: &Storage, content: &Content, career: &data::Career, behaviors: &PlayerBehaviors, working: &PlayerDesign, lab: &Lab) -> Sources {
    let escort_ids = career.escorts();
    let mut ids: Vec<String> = library::list_designs(storage, content).unwrap_or_default().into_iter().filter(|e| e.problem.is_none()).map(|e| e.id).collect();
    ids.sort_by_key(|id| !escort_ids.contains(id));
    let entry = |id: &String| StartPick::Design(id.clone()).resolve(storage, content, behaviors, working).ok().map(|d| design_entry(&d));
    Sources {
        open_voyages: career.open_voyages(content).into_iter().map(String::from).collect(),
        fleet: ids.iter().filter_map(entry).collect(),
        champions: lab.champions(storage, content),
        escorts: escort_ids.iter().filter_map(entry).collect(),
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
    /// The last generation the run closed: how long it took on the wall clock (s), and writing the run then (ms).
    pub last_generation: Option<(f32, f32)>,
    /// When the generation in progress got its first slice.
    started: Option<Instant>,
    /// Natively: the open generation's fights on the threads, and the content they are fought with.
    #[cfg(not(target_arch = "wasm32"))]
    threads: Option<threads::Threads>,
    #[cfg(not(target_arch = "wasm32"))]
    content: Option<std::sync::Arc<Content>>,
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
    /// VALIDATE underway: the run it validates and the work (it computes in the run's place until done).
    validating: Option<(String, Validation)>,
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
        let keep = [self.running.clone(), self.viewed.clone(), self.validating.as_ref().map(|v| v.0.clone())];
        self.loaded.retain(|l| keep.iter().any(|k| k.as_ref() == Some(&l.id)));
        Ok(())
    }

    /// A new run from `config`, named `name`: it computes from now on (another that did is paused) and the page
    /// shows it. Refused when it doesn't build (why, in words) or [`MAX_RUNS`] runs are kept.
    pub fn create(&mut self, storage: &mut Storage, content: &Content, config: RunConfig, name: &str, now: u64) -> Result<String, String> {
        self.room()?;
        let run = Run::new(content, config).map_err(|e| e.to_string())?;
        self.add(storage, content, run, name, now, RunStatus::Running)
    }

    /// Room for one more run.
    fn room(&self) -> Result<(), String> {
        match self.runs.len() >= MAX_RUNS {
            true => Err(format!("the lab keeps at most {MAX_RUNS} runs: delete one first")),
            false => Ok(()),
        }
    }

    /// A run made (or read) into the lab as `name`: written, listed first, shown; running, it computes from now on
    /// (another that did is paused).
    fn add(&mut self, storage: &mut Storage, content: &Content, run: Run, name: &str, now: u64, status: RunStatus) -> Result<String, String> {
        self.room()?;
        let stored = storage.list(RunDoc::NAMESPACE).map_err(|e| e.to_string())?;
        let id = unique_id(&slug_or(name, "run"), |id| stored.iter().any(|s| s == id) || self.runs.iter().any(|e| e.id == id));
        if status == RunStatus::Running {
            self.pause_running(storage)?;
        }
        let rules = self.today(content);
        let loaded = Loaded { id: id.clone(), run, name: name.to_string(), created: now, status };
        let save = loaded.save();
        storage.write(&id, &RunDoc(save.clone()), now).map_err(|e| e.to_string())?;
        self.runs.insert(0, RunEntry::of(content, &id, &save, rules));
        self.loaded.push(loaded);
        if status == RunStatus::Running {
            self.running = Some(id.clone());
        }
        self.view(storage, content, Some(&id))?;
        Ok(id)
    }

    /// A name no run has yet: `name`, else `name 2`, ...
    fn fresh(&self, name: &str) -> String {
        let names: Vec<String> = self.runs.iter().map(|e| e.name.clone()).collect();
        fresh_name(name, &names)
    }

    /// The stored document of a run.
    fn read(storage: &Storage, id: &str) -> Result<RunSave, String> {
        Ok(storage.read::<RunDoc>(id).map_err(|e| e.to_string())?.ok_or_else(|| format!("{id} is gone"))?.doc.0)
    }

    /// IMPORT RUN: a run file (a CLI's, `arena evolve --save`, or another lab's) read into the lab, paused, under its
    /// head's name or the file's. It must restore on today's content; under other rules it comes in read-only (stale),
    /// to be re-scored. Refused, in words, when it doesn't read.
    pub fn import(&mut self, storage: &mut Storage, content: &Content, file: &str, text: &str, now: u64) -> Result<String, String> {
        self.room()?;
        let save = RunSave::from_ron(text).map_err(|e| e.to_string())?;
        let run = Run::restore(content, &save).map_err(|e| e.to_string())?;
        let stem = file.rsplit(['/', '\\']).next().unwrap_or(file).trim_end_matches(".ron");
        let name = self.fresh(save.name.as_deref().unwrap_or(if stem.is_empty() { "Imported run" } else { stem }));
        self.add(storage, content, run, &name, now, RunStatus::Paused)
    }

    /// DUPLICATE: the same setup with a new seed, as a new run that computes.
    pub fn duplicate(&mut self, storage: &mut Storage, content: &Content, id: &str, now: u64) -> Result<String, String> {
        self.room()?;
        let save = Self::read(storage, id)?;
        let mut config = save.config;
        config.seed = (now.wrapping_mul(2_654_435_761) ^ config.seed.rotate_left(17)) % 1_000_000;
        let run = Run::new(content, config).map_err(|e| e.to_string())?;
        let name = self.fresh(&self.entry(id).map_or_else(|| id.to_string(), |e| e.name.clone()));
        self.add(storage, content, run, &name, now, RunStatus::Running)
    }

    /// RENAME: the run's head gets the new name (in memory, the list and its document).
    pub fn rename(&mut self, storage: &mut Storage, id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("a run needs a name".into());
        }
        let save = match self.loaded.iter_mut().find(|l| l.id == id) {
            Some(l) => {
                l.name = name.to_string();
                l.save()
            }
            None => RunSave { name: Some(name.to_string()), ..Self::read(storage, id)? },
        };
        storage.write(id, &RunDoc(save), now_unix()).map_err(|e| e.to_string())?;
        if let Some(e) = self.runs.iter_mut().find(|e| e.id == id) {
            e.name = name.to_string();
        }
        Ok(())
    }

    /// RE-SCORE: a stale run as a new run under today's rules (`arena::Run::rescored`): its elites are fought again
    /// first, then it goes on. The stale run stays as it is.
    pub fn rescore(&mut self, storage: &mut Storage, content: &Content, id: &str, now: u64) -> Result<String, String> {
        self.room()?;
        let save = Self::read(storage, id)?;
        let run = Run::rescored(content, &save).map_err(|e| e.to_string())?;
        let name = self.fresh(&format!("{} (re-scored)", self.entry(id).map_or(id, |e| e.name.as_str())));
        self.add(storage, content, run, &name, now, RunStatus::Running)
    }

    /// VALIDATE (GDR-0092): `genomes` of run `id` fight its trials on new seeds ([`VALIDATE_SEEDS`] each) and their
    /// recorded fights again; it computes in the run's place until done, then its results go into the run's file.
    /// Returns its jobs. Refused for a stale run (re-score it first) and while another validation is underway.
    pub fn validate(&mut self, storage: &Storage, content: &Content, id: &str, genomes: &[u32]) -> Result<usize, String> {
        if self.validating.is_some() {
            return Err("a validation is underway".into());
        }
        self.load_run(storage, content, id)?;
        let l = self.get(id).ok_or_else(|| format!("{id} is gone"))?;
        if l.run.stale() {
            return Err("its rules have changed since: RE-SCORE it first".into());
        }
        let v = l.run.validation(genomes, VALIDATE_SEEDS).ok_or("that design was never scored")?;
        let n = v.jobs().len();
        self.validating = Some((id.to_string(), v));
        self.started = None;
        Ok(n)
    }

    /// The validation underway: its run, how far it is (0..1), the genomes it validates.
    pub fn validating(&self) -> Option<(&str, f32, Vec<u32>)> {
        self.validating.as_ref().map(|(id, v)| (id.as_str(), v.done(), v.genomes()))
    }

    /// A validation is done: its results into its run, written. `true` (the page follows).
    fn validated(&mut self, storage: &mut Storage) -> bool {
        let Some((id, v)) = self.validating.take() else { return false };
        if let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) {
            l.run.validated_by(v);
            if let Err(e) = storage.write(&id, &RunDoc(l.save()), now_unix()) {
                self.trouble = Some(format!("the validation isn't saved: {e}"));
            }
        }
        true
    }

    /// CHAMPIONS: every run's best design (its archive's best, rebuilt from its file), the best scores first.
    pub fn champions(&self, storage: &Storage, content: &Content) -> Vec<Entry> {
        let mut out: Vec<(f32, Entry)> = Vec::new();
        for e in self.runs.iter().filter(|e| e.problem.is_none()) {
            let (Ok(save), Some(score)) = (Self::read(storage, &e.id), e.best) else { continue };
            let Some(best) = save.best else { continue };
            if let Ok(g) = save.genome(content, best) {
                let entry = arena::evolve::entry_of(DesignRef::Blueprint(g.blueprint_text()), Some(BehaviorRef::Text(g.behavior_text().to_string())));
                out.push((score, entry));
            }
        }
        out.sort_by(|a, b| b.0.total_cmp(&a.0));
        out.into_iter().map(|(_, e)| e).collect()
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
        // A generation's time is measured while it computes in one go.
        self.started = None;
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
        if self.validating.as_ref().is_some_and(|v| v.0 == id) {
            self.validating = None;
        }
        Ok(())
    }

    /// One frame's work on the run that computes, in slices on this thread (the web's executor; every platform's in
    /// the tests): `budget_ms` of it (turned into ticks with the measured rate), in a frame `frame` seconds long. A
    /// generation that closes is written (the run's checkpoint), and so is the run's end. `true` when a generation
    /// closed (the page and the building follow).
    pub fn compute(&mut self, storage: &mut Storage, budget_ms: f32, frame: f32) -> bool {
        let ticks = self.rate.ticks_for(budget_ms);
        // A validation computes in the run's place.
        if let Some((_, v)) = self.validating.as_mut() {
            return v.step(ticks) && self.validated(storage);
        }
        let Some(id) = self.running.clone() else { return false };
        let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) else { return false };
        if !l.computes() {
            return false;
        }
        let (before, generation) = (l.run.worked(), l.run.generation());
        let t0 = Instant::now();
        self.started.get_or_insert(t0);
        let p = l.run.step(ticks);
        let used = l.run.worked() - before;
        self.rate.note(used, t0.elapsed().as_secs_f32(), frame);
        p.generation != generation && self.closed(storage, &id)
    }

    /// The run that computes has closed a generation: it is written (its checkpoint), its entry follows, and its end
    /// or a failed write stops it. `true` (a generation closed).
    fn closed(&mut self, storage: &mut Storage, id: &str) -> bool {
        let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) else { return false };
        let t1 = Instant::now();
        let save = l.save();
        let written = storage.write(id, &RunDoc(save.clone()), now_unix());
        let took = self.started.take().map_or(0.0, |t| (t1 - t).as_secs_f32());
        let write = t1.elapsed().as_secs_f32() * 1000.0;
        self.last_generation = Some((took, write));
        let p = l.run.progress();
        info!(
            "lab: {id} generation {} of {} in {took:.1} s ({:.1} µs a tick of work, {:.0} ticks/s), written in {write:.1} ms ({} kB)",
            p.generation,
            p.generations,
            self.rate.us_per_tick,
            self.rate.ticks_per_second,
            save.payload.len() / 1024
        );
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
        } else if p.finished {
            self.running = None;
            self.fresh = Some(id.to_string());
        }
        true
    }

    /// How far the open generation of a run is, 0..1: its jobs done (natively, on the threads), else
    /// `arena::Run::generation_done`.
    pub fn generation_done(&self, l: &Loaded) -> f32 {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(t) = self.threads.as_ref().filter(|t| t.id == l.id && t.generation == l.run.generation() && self.validating.is_none()) {
            return t.done() as f32 / t.total().max(1) as f32;
        }
        l.run.generation_done()
    }

    /// One frame's work natively (GDR-0092 "The executor"): the open generation is bred on this thread within
    /// `budget_ms`, then its fights go to Bevy's compute threads a few at a time ([`threads`]); the results are taken
    /// as they come and handed to the run in job order once all are in, which closes the generation as the slices
    /// would (the same run). With no budget (in flight) no more fights start and the generation waits to close, so
    /// the cores are flight's once the fights underway end (each under about 50 ms).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn compute_threads(&mut self, storage: &mut Storage, content: &Content, budget_ms: f32, frame: f32) -> bool {
        // A validation's jobs go to the threads in the run's place (as generation `u32::MAX` of its run).
        if let Some((id, v)) = self.validating.as_mut() {
            if !self.threads.as_ref().is_some_and(|t| t.id == *id && t.generation == u32::MAX) {
                if budget_ms <= 0.0 {
                    return false;
                }
                let content = self.content.get_or_insert_with(|| std::sync::Arc::new(content.clone())).clone();
                self.threads = Some(threads::Threads::new(id, u32::MAX, v.jobs().to_vec(), content));
            }
            let t = self.threads.as_mut().expect("the validation's jobs are out");
            t.pump(budget_ms <= 0.0);
            if budget_ms <= 0.0 || !t.finished() {
                return false;
            }
            let results = self.threads.take().expect("the validation's jobs are in").take();
            v.submit(results);
            return self.validated(storage);
        }
        let Some(id) = self.running.clone() else {
            self.threads = None;
            return false;
        };
        let ticks = self.rate.ticks_for(budget_ms);
        let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) else { return false };
        if !l.computes() {
            return false;
        }
        let generation = l.run.generation();
        if !self.threads.as_ref().is_some_and(|t| t.id == id && t.generation == generation) {
            self.threads = None;
            if budget_ms <= 0.0 {
                return false;
            }
            let (before, t0) = (l.run.worked(), Instant::now());
            self.started.get_or_insert(t0);
            let bred = l.run.breed(ticks);
            self.rate.note(l.run.worked() - before, t0.elapsed().as_secs_f32(), frame);
            if !bred {
                return false;
            }
            let content = self.content.get_or_insert_with(|| std::sync::Arc::new(content.clone())).clone();
            self.threads = Some(threads::Threads::new(&id, generation, l.run.jobs().to_vec(), content));
        }
        let t = self.threads.as_mut().expect("the generation's fights are out");
        let ticks_done = t.pump(budget_ms <= 0.0);
        if frame > 0.0 && ticks_done > 0 {
            self.rate.ticks_per_second += (ticks_done as f32 / frame - self.rate.ticks_per_second) * RATE_FOLLOW;
        }
        if budget_ms <= 0.0 || !t.finished() {
            return false;
        }
        let results = self.threads.take().expect("the generation's fights are in").take();
        let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) else { return false };
        l.run.submit(results);
        self.closed(storage, &id)
    }
}

/// The native executor's work: a generation's fights on Bevy's compute threads (`AsyncComputeTaskPool`), a few to a
/// task, as many tasks at once as the machine has cores less one (the frame's); the web has no threads and steps
/// in slices ([`Lab::compute`]). Results are kept in job order, so the run gets them as `Run::jobs` gave the jobs.
#[cfg(not(target_arch = "wasm32"))]
mod threads {
    use std::sync::Arc;

    use arena::evolve::{Job, JobResult};
    use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};
    use data::Content;

    /// Fights a task runs one after the other (about 5–20 ms natively): small enough that flight gets the cores back
    /// soon after it starts, large enough that the tasks' own cost doesn't count.
    const CHUNK: usize = 4;

    /// One generation's fights out on the threads.
    pub struct Threads {
        /// The run and generation they are of.
        pub id: String,
        pub generation: u32,
        jobs: Arc<Vec<Job>>,
        content: Arc<Content>,
        results: Vec<Option<JobResult>>,
        /// The first job not handed out yet.
        next: usize,
        running: Vec<(usize, Task<Vec<JobResult>>)>,
        width: usize,
    }

    impl Threads {
        pub fn new(id: &str, generation: u32, jobs: Vec<Job>, content: Arc<Content>) -> Self {
            let width = std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(1).max(1));
            let n = jobs.len();
            Threads { id: id.to_string(), generation, jobs: Arc::new(jobs), content, results: vec![None; n], next: 0, running: Vec::new(), width }
        }

        /// Takes the results of the tasks that finished and, unless `hold`, hands out more fights; returns the ticks
        /// the finished fights ran.
        pub fn pump(&mut self, hold: bool) -> u64 {
            let mut ticks = 0;
            let mut k = 0;
            while k < self.running.len() {
                match block_on(future::poll_once(&mut self.running[k].1)) {
                    Some(rs) => {
                        let (first, _) = self.running.swap_remove(k);
                        for (j, r) in rs.into_iter().enumerate() {
                            ticks += r.ticks;
                            self.results[first + j] = Some(r);
                        }
                    }
                    None => k += 1,
                }
            }
            let pool = AsyncComputeTaskPool::get();
            while !hold && self.running.len() < self.width && self.next < self.jobs.len() {
                let (first, end) = (self.next, (self.next + CHUNK).min(self.jobs.len()));
                let (jobs, content) = (self.jobs.clone(), self.content.clone());
                self.running.push((first, pool.spawn(async move { jobs[first..end].iter().map(|j| j.run(&content)).collect() })));
                self.next = end;
            }
            ticks
        }

        pub fn done(&self) -> usize {
            self.results.iter().filter(|r| r.is_some()).count()
        }

        pub fn total(&self) -> usize {
            self.results.len()
        }

        /// Every fight's result is in.
        pub fn finished(&self) -> bool {
            self.running.is_empty() && self.results.iter().all(Option::is_some)
        }

        /// The results in job order (all of them: [`Threads::finished`]).
        pub fn take(self) -> Vec<JobResult> {
            self.results.into_iter().map(|r| r.expect("every fight is in")).collect()
        }
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

/// Every frame at home and on the screens: the run that computes gets its budget (none in flight), its fights on the
/// threads ([`Lab::compute_threads`]). Only a closed generation is news for the screens that show the lab (they read
/// the progress inside one themselves while they show it).
#[cfg(not(target_arch = "wasm32"))]
pub fn compute(mode: Res<State<AppMode>>, time: Res<Time<Real>>, mut lab: ResMut<Lab>, mut storage: ResMut<Storage>, content: Res<GameContent>) {
    let ms = budget(*mode.get(), lab.layer_open);
    if (lab.running.is_some() || lab.validating.is_some()) && lab.bypass_change_detection().compute_threads(&mut storage, &content.0, ms, time.delta_secs()) {
        lab.set_changed();
    }
}

/// Every frame at home and on the screens: the run that computes gets its budget in slices on this thread (the web
/// has no threads, [`Lab::compute`]); none in flight.
#[cfg(target_arch = "wasm32")]
pub fn compute(mode: Res<State<AppMode>>, time: Res<Time<Real>>, mut lab: ResMut<Lab>, mut storage: ResMut<Storage>) {
    let ms = budget(*mode.get(), lab.layer_open);
    if ms > 0.0 && (lab.running.is_some() || lab.validating.is_some()) && lab.bypass_change_detection().compute(&mut storage, ms, time.delta_secs()) {
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

    /// A small run of the starter: three generations of four children, against one raider and the hall of fame,
    /// one seed each.
    fn setup(content: &Content) -> Setup {
        let mut s = Setup::new(content, vec![StartPick::Template("rim_skiff".into())], 7);
        s.size = Size { population: 4, generations: 3 };
        for t in &mut s.trials {
            t.on = matches!(t.preset.as_str(), "enemy_ships" | "hall_of_fame");
            t.seeds = 1;
            t.count = t.count.map(|_| 1);
            for o in &mut t.ships {
                o.1 = u32::from(o.0 == "automaton_raider");
            }
        }
        s
    }

    fn config(content: &Content, s: &Setup, storage: &Storage) -> RunConfig {
        let design = PlayerDesign::starter(content, "rim_skiff");
        let starts: Vec<StartDesign> = s.starts.iter().map(|(p, _)| p.resolve(storage, content, &PlayerBehaviors::default(), &design).unwrap()).collect();
        let granted = Granted { unlocks: data::career::Unlocks::all(content), mind_capacity: 4, free: true, ..default() };
        s.config(content, &starts, &granted, 4, &Sources::default()).unwrap()
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
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_state(mode)
            .insert_resource(storage)
            .insert_resource(lab)
            .insert_resource(GameContent(data::base_content()))
            .add_systems(Update, compute);
        app
    }

    /// The run computes at home and on the screens (natively its fights on the threads), and waits in flight: no
    /// breeding, no generation closes, nothing lost.
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
        let flying = (worked(&app), app.world().resource::<Lab>().get(&id).unwrap().run.generation());
        for _ in 0..30 {
            app.update();
        }
        let after = (worked(&app), app.world().resource::<Lab>().get(&id).unwrap().run.generation());
        assert_eq!(after, flying, "and waits again in flight");
    }

    /// Natively a generation's fights run on the threads: the run they make is the one the slices make, file for file.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_threads_make_the_run_the_slices_make() {
        bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
        let content = data::base_content();
        let s = setup(&content);
        let mut sliced = mem();
        let mut lab = Lab::load(&sliced, &content, true);
        let cfg = config(&content, &s, &sliced);
        let a = lab.create(&mut sliced, &content, cfg, "Small", 10).unwrap();
        while lab.running().is_some() {
            lab.compute(&mut sliced, BUDGET_MS, 0.016);
        }
        let mut threaded = mem();
        let mut lab = Lab::load(&threaded, &content, true);
        let cfg = config(&content, &s, &threaded);
        let b = lab.create(&mut threaded, &content, cfg, "Small", 10).unwrap();
        let mut frames = 0;
        while lab.running().is_some() {
            // Now and then a frame in flight: no fight starts, the generation waits.
            let ms = if frames % 7 == 3 { 0.0 } else { BUDGET_MS };
            lab.compute_threads(&mut threaded, &content, ms, 0.016);
            frames += 1;
            assert!(frames < 100_000, "the threads never finished");
        }
        let doc = |st: &Storage, id: &str| st.read::<RunDoc>(id).unwrap().unwrap().doc.0.to_ron();
        assert_eq!(doc(&threaded, &b), doc(&sliced, &a));
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
        assert_eq!(storage.list(RunDoc::NAMESPACE).unwrap(), std::slice::from_ref(&b));
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

    /// IMPORT RUN reads a CLI run file (no head) into the lab, paused, under the file's name, and it writes back the
    /// same run; RENAME names it anew; VALIDATE computes in the run's place and keeps its results in the file;
    /// DUPLICATE makes the same setup with a new seed; RE-SCORE makes a stale run a new run that fights its elites
    /// again first.
    #[test]
    fn runs_import_validate_duplicate_and_rescore() {
        let content = data::base_content();
        let mut made = mem();
        let s = setup(&content);
        let mut lab = Lab::load(&made, &content, true);
        let cfg = config(&content, &s, &made);
        let id = lab.create(&mut made, &content, cfg, "Small", 10).unwrap();
        while lab.running().is_some() {
            lab.compute(&mut made, BUDGET_OPEN_MS, 0.016);
        }
        // What `arena evolve --save` writes: the run file without the game's head.
        let full = made.read::<RunDoc>(&id).unwrap().unwrap().doc.0;
        let cli = RunSave { name: None, created: None, status: None, ..full.clone() }.to_ron();
        let mut storage = mem();
        let mut lab = Lab::load(&storage, &content, true);
        let imported = lab.import(&mut storage, &content, "runs/deep_skiff.ron", &cli, 50).unwrap();
        let e = lab.entry(&imported).unwrap().clone();
        assert_eq!((e.name.as_str(), e.status, e.generation, e.stale), ("deep_skiff", RunStatus::Paused, 3, false));
        let back = storage.read::<RunDoc>(&imported).unwrap().unwrap().doc.0;
        assert_eq!(RunSave { name: None, created: None, status: None, ..back }.to_ron(), cli, "it round-trips");
        assert!(lab.running().is_none(), "an imported run waits");
        assert!(lab.import(&mut storage, &content, "x.ron", "(format: 1", 50).is_err());
        lab.rename(&mut storage, &imported, "  Deep skiff  ").unwrap();
        assert_eq!(lab.entry(&imported).unwrap().name, "Deep skiff");
        assert_eq!(storage.read::<RunDoc>(&imported).unwrap().unwrap().doc.0.name.as_deref(), Some("Deep skiff"));
        // VALIDATE the best: it computes until done, then the run's file keeps it.
        let best = lab.get(&imported).unwrap().run.archive().best().unwrap().id;
        assert!(lab.validate(&storage, &content, &imported, &[best]).unwrap() > 0);
        assert!(lab.validate(&storage, &content, &imported, &[best]).is_err(), "one at a time");
        let mut frames = 0;
        while lab.validating().is_some() {
            lab.compute(&mut storage, BUDGET_OPEN_MS, 0.016);
            frames += 1;
            assert!(frames < 100_000);
        }
        let v = lab.get(&imported).unwrap().run.validated(best).cloned().unwrap();
        assert_eq!(v.as_recorded.0, v.as_recorded.1, "today's rules: every fight as recorded");
        let save = storage.read::<RunDoc>(&imported).unwrap().unwrap().doc.0;
        assert_eq!(Run::restore(&content, &save).unwrap().validated(best), Some(&v));
        // DUPLICATE: the same setup, a new seed, computing.
        let copy = lab.duplicate(&mut storage, &content, &imported, 60).unwrap();
        let c = lab.get(&copy).unwrap();
        assert_eq!((c.run.config().trials.clone(), c.status), (save.config.trials.clone(), RunStatus::Running));
        assert_ne!(c.run.config().seed, save.config.seed);
        assert_eq!(lab.entry(&copy).unwrap().name, "Deep skiff 2");
        // RE-SCORE: a run fought under other rules is stale; re-scored, a new run fights its elites again.
        let stale = RunSave { rules: save.rules ^ 1, ..save };
        storage.write("old_rules", &RunDoc(stale), 70).unwrap();
        let mut lab = Lab::load(&storage, &content, true);
        assert!(lab.entry("old_rules").unwrap().stale);
        assert!(lab.validate(&storage, &content, "old_rules", &[best]).is_err(), "re-score it first");
        let fresh = lab.rescore(&mut storage, &content, "old_rules", 80).unwrap();
        assert_eq!(lab.running().map(|l| l.id.as_str()), Some(fresh.as_str()));
        while lab.get(&fresh).unwrap().run.generation() < 4 {
            lab.compute(&mut storage, BUDGET_OPEN_MS, 0.016);
        }
        let r = &lab.get(&fresh).unwrap().run;
        assert!(r.state().history[3].rescore && !r.stale());
        assert!(lab.entry(&fresh).unwrap().name.ends_with("(re-scored)"));
        assert!(lab.entry("old_rules").unwrap().stale, "the stale run stays as it was");
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
        assert_eq!(w.blueprint.id, library::slug(&design.blueprint().name), "a content id, not the drydock's own");
        assert!(matches!(w.start(Locks::default()), Start::Design { design: DesignRef::Blueprint(_), .. }));
        assert!(!w.program.is_empty());
        // It builds a run as it is (the arena checks the blueprint text like content).
        let mut s = Setup::new(&content, vec![StartPick::Working], 3);
        s.size = Size { population: 2, generations: 1 };
        let granted = Granted { unlocks: data::career::Unlocks::all(&content), mind_capacity: 4, free: true, ..default() };
        let src = Sources { open_voyages: vec!["yard_approach".into()], ..default() };
        let cfg = s.config(&content, std::slice::from_ref(&w), &granted, 4, &src).unwrap();
        // GDR-0092's default selection: the enemies you meet (weight 2), the test-range waves, beat your start, the
        // hall of fame, each a lab.ron preset; WIN alone.
        let ids: Vec<(&str, f32)> = cfg.trials.iter().map(|t| (t.preset.as_deref().unwrap(), t.weight)).collect();
        assert_eq!(ids, [("enemies_you_meet", 2.0), ("test_range_waves", 1.0), ("beat_your_start", 1.0), ("hall_of_fame", 1.0)]);
        assert!(cfg.goals.is_default() && cfg.fitness.is_none());
        Run::new(&content, cfg).unwrap();
        // Without an open voyage THE ENEMIES YOU MEET has nothing to fight; without a trial on, nothing scores.
        assert!(s.config(&content, std::slice::from_ref(&w), &granted, 4, &Sources::default()).unwrap_err().contains("no voyage is open"));
        let mut none = s.clone();
        none.trials.iter_mut().for_each(|t| t.on = false);
        assert!(none.config(&content, &[w], &granted, 4, &src).is_err());
        // The estimate: every trial's jobs a child (two encounters of the first voyage × 4 seeds, the waves 2, the start
        // 2, the hall of fame 4 × 2).
        assert_eq!(s.per_child(&content, &src).0, 8 + 2 + 2 + 8);
        assert!(s.ticks(&content, &src) > 0.0);
        let t = StartPick::Template("halvex_runner".into()).resolve(&storage, &content, &behaviors, &design).unwrap();
        assert!(matches!(t.start(Locks::default()), Start::Design { design: DesignRef::Ship(ref id), .. } if id == "halvex_runner"));
        assert_eq!(templates(&content, "ship_mind").first().map(String::as_str), Some("halvex_runner"));
        assert!(enemies(&content).contains(&"automaton_raider".to_string()));
    }
}
