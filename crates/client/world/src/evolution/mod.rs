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
//!   and on every screen within a time budget per frame (`lab.ron`'s `budget`: more with the layer open, [`budget`])
//!   and pauses in flight: the test range, test flights, replays and voyages ([`compute`]). The
//!   budget is turned into ticks of work from the measured cost per tick ([`Rate`]). The frame's budget breeds, and
//!   the jobs go out of the frame ([`Lab::compute_out`], [`out`]): natively to Bevy's compute threads, on the web to
//!   the lab's web workers (`workers`, ADR-0043). Without a worker (none loads, `?lab_workers=0`) `arena::Run::step`
//!   breeds and fights within the budget on the main thread ([`Lab::compute`]). All of them make the same run.
//! * **THE LADDER** ([`ladder`], GDR-0092 step 9): RATE ON THE LADDER rates saved designs on `lab.ron`'s ladder in
//!   the run's place, as VALIDATE does ([`Lab::rate`]); each rating is kept in `ladder/<design id>`.
//! * **Storage** ([`store`]): each run in parts, `lab/<id>` its head (with its name, creation time and status),
//!   `lab/<id>/config` and one part `lab/<id>/g<n>` per generation done; after every generation its part and the head
//!   are written (its only checkpoint: a reload breeds the open generation again from the same random state, the
//!   same run), the head again when it is paused, resumed or renamed. A checkpoint the web's storage failed to keep
//!   (it writes behind) pauses the run as a write that fails at once does. A run whose rules' fingerprint differs
//!   from today's is stale: read-only, it doesn't compute. At most [`MAX_RUNS`].
//! * **SAVE AS DESIGN** ([`save_as_design`]): a genome into YOUR DESIGNS in its run's frame with an `origin` and the
//!   `evolved` tag; an evolved behavior is saved beside it as a player behavior.
//! * **Locked** until the career opens the facility [`FACILITY`] (the research node Simulation vault, or
//!   `?unlock=all`): [`Lab::enabled`] follows [`Granted`], and a locked lab computes nothing.

pub mod ladder;
mod out;
pub mod store;
#[cfg(target_arch = "wasm32")]
mod workers;

use arena::evolve::{ArchiveShape, Axis, Bound, Camp, Evolve, FlownBy, Goals, Job, Limits, Locks, MayChange, Measure, Population, RunHead, RunParts, RunStatus, Sources, Start, TrialSpec, VALIDATE_SEEDS, Validation};
use arena::{BehaviorRef, DesignRef, Entry, Run, RunConfig};
use bevy::platform::time::Instant;
use bevy::prelude::*;
use data::{Behavior, BehaviorEditor, Blueprint, Content, LabSize, Origin, TrialPresetKind};

use crate::career::Granted;
use crate::mode::AppMode;
use crate::player::PlayerDesign;
use crate::player::behaviors::{PlayerBehaviors, set_text, validate_design};
use crate::player::library::{self, DesignDoc, fresh_name, slug_or, unique_id};
use crate::sim_bridge::GameContent;
use crate::storage::{Storage, StorageError, now_unix};

/// The facility (`facilities.ron`) whose building the lab is: locked until research opens it.
pub const FACILITY: &str = "evolution";
/// The cost of a tick of work assumed before one is measured (µs): wasm's, the slower (natively 5–6 µs).
const FIRST_US_PER_TICK: f32 = 9.0;
/// How fast the measured cost and throughput follow a frame (a running mean).
const RATE_FOLLOW: f32 = 0.1;
/// Frames with less work than this don't measure the cost (their timer's noise is larger than the work).
const RATE_MIN_TICKS: u64 = 200;
/// Most runs the lab keeps (GDR-0092, ADR-0041; PLACEHOLDER). Not the storage's room, which IndexedDB has plenty of,
/// but its memory: the web build reads every document into memory at start, and a DEEP run with the default trials
/// is about 5 MB stored (measured 16 Oct 2026; a STANDARD one about a quarter of that), so 32 of the largest are
/// about 160 MB read at every start. RUNS shows what the storage holds.
pub const MAX_RUNS: usize = 32;
/// The share of its time limit a job lasts on average, for the setup's estimate: a fight, a gauntlet (its time limit
/// is for all its waves), a voyage. Measured 16 Oct 2026 (`docs/arena.md` "What a trial costs": 0.40–0.52, 0.25–0.34,
/// 0.13 for designs early in a run); PLACEHOLDER.
const FIGHT_SHARE: f32 = 0.5;
const GAUNTLET_SHARE: f32 = 0.35;
const VOYAGE_SHARE: f32 = 0.25;

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
    /// An arms race's: its CO-EVOLUTION trial's name (`lab.ron`'s ARMS RACE).
    pub race: Option<String>,
    /// Fought under rules that have changed since: read-only.
    pub stale: bool,
    /// Why it doesn't read or go on (listed anyway, so it can be deleted).
    pub problem: Option<String>,
}

impl RunEntry {
    pub fn done(&self) -> bool {
        self.generation >= self.generations
    }

    /// A stored run's head and numbers as an entry; `stale` against today's rules.
    fn of(content: &Content, id: &str, parts: &RunParts, rules: u64) -> RunEntry {
        let RunParts { head, config, generations } = parts;
        RunEntry {
            id: id.to_string(),
            name: head.name.clone().unwrap_or_else(|| id.to_string()),
            created: head.created,
            status: head.status.unwrap_or(RunStatus::Paused),
            generation: generations.len() as u32,
            generations: config.schedule().map_or(0, |s| s.iter().map(|g| g.1).sum()),
            best: generations.last().map(|g| g.stats.archive_best),
            first: generations.first().map(|g| g.stats.best),
            starts: config.all_starts().map(|s| start_name(content, s)).collect(),
            race: config.enemy.as_ref().map(|_| {
                let preset = config.trials.iter().find(|t| matches!(t.kind, arena::evolve::Trial::CoEvolution)).and_then(|t| t.preset.as_deref());
                preset.and_then(|id| content.lab.trial(id)).map_or_else(|| "Co-evolution".into(), |p| p.name.clone())
            }),
            stale: head.rules != rules,
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
    /// Its generations whose parts are written (the ones after go out with the next checkpoint).
    stored: u32,
}

impl Loaded {
    /// The head as it is written: the run's (the state after its last closed generation) with the game's fields.
    fn head(&self) -> RunHead {
        RunHead { name: Some(self.name.clone()), created: Some(self.created), status: Some(self.status), ..self.run.head() }
    }

    /// The run in parts as it is written, all of it.
    fn parts(&self) -> RunParts {
        RunParts { head: self.head(), ..self.run.parts() }
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

/// What the drydock asks the EVOLUTION layer to open on when home comes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabRequest {
    /// EVOLVE THIS DESIGN: the setup with this start.
    Setup(StartPick),
    /// A click on an evolved design's ORIGIN: its run's page with its genome picked.
    Origin { run: String, genome: u32 },
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

/// Children and generations (SIZE: a `lab.ron` preset's, or the player's own).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub population: u32,
    pub generations: u32,
}

impl Size {
    pub fn of(preset: &LabSize) -> Size {
        Size { population: preset.population, generations: preset.generations }
    }
}

/// A trial as the setup picks it (GDR-0092 "TRIALS"): a `lab.ron` preset, on or off, its weight and seeds, and what the
/// setup may change of what it fights: ENEMY SHIPS' count of each enemy ship, the ARMS RACE's enemy ships it starts
/// from (1: picked), how many of the best HALL OF FAME and CHAMPIONS fight, how many waves TEST-RANGE WAVES must clear.
#[derive(Clone, Debug, PartialEq)]
pub struct TrialPick {
    pub preset: String,
    pub on: bool,
    pub weight: f32,
    pub seeds: u32,
    /// ENEMY SHIPS: every content ship tagged `enemy` and how many of it a fight has (0: not fought); the ARMS RACE:
    /// each of them 1 when the enemy starts from it.
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
            TrialPresetKind::Ladder { rungs } => (rungs.len(), fight * FIGHT_SHARE),
            TrialPresetKind::Voyages => {
                let minutes = p.time_limit.unwrap_or(arena::DEFAULT_VOYAGE_MINUTES);
                (src.open_voyages.len(), minutes * 60.0 * sim::TICK_HZ as f32 * VOYAGE_SHARE)
            }
            // The enemy's best and its past champions (its starts in generation 0).
            TrialPresetKind::CoEvolution { elites, hall, .. } => ((elites + hall) as usize, fight * FIGHT_SHARE),
        };
        (opponents as u32 * self.seeds, ticks)
    }

    /// The ARMS RACE's enemy ships it starts from.
    pub fn picked(&self) -> Vec<String> {
        self.ships.iter().filter(|s| s.1 > 0).map(|s| s.0.clone()).collect()
    }

    /// The ARMS RACE, on: its enemy's children a generation, the jobs each gets and their ticks on average (your best,
    /// your past champions and, as the anchor, your `starts`).
    pub fn enemy_cost(&self, content: &Content, population: u32, starts: usize) -> Option<(u32, u32, f32)> {
        let p = self.preset(content).filter(|_| self.on && self.weight > 0.0)?;
        let TrialPresetKind::CoEvolution { size, elites, hall, anchor, .. } = p.kind else { return None };
        let children = ((population as f32 * size).round() as u32).max(self.picked().len() as u32);
        let rivals = elites + hall + if anchor { starts as u32 } else { 0 };
        let fight = p.time_limit.unwrap_or(arena::DEFAULT_TIME_LIMIT) * sim::TICK_HZ as f32;
        Some((children, rivals * self.seeds, fight * FIGHT_SHARE))
    }
}

/// FLOWN BY as the setup picks it (GDR-0092): who flies the design's first ship in every fight trial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flier {
    /// Its behavior.
    Behavior,
    /// The stand-in pilot, with its assists.
    StandIn,
    /// The behavior on odd seeds, the stand-in on even ones.
    Both,
}

impl Flier {
    /// FLOWN BY's default (GDR-0092): both while the assists may change (only a pilot uses them), else the behavior.
    pub fn default_for(may: &MayChange) -> Flier {
        if may.assists() { Flier::Both } else { Flier::Behavior }
    }
}

/// What the setup page has picked (GDR-0092 "The run setup").
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub name: String,
    /// One to four starts and their locks.
    pub starts: Vec<(StartPick, Locks)>,
    /// What may change (MAY CHANGE): the genes on, the part categories kept.
    pub may_change: MayChange,
    /// FLOWN BY, and the stand-in pilot (a `lab.ron` pilot's id).
    pub flown_by: Flier,
    pub pilot: String,
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
    /// The defaults (GDR-0092): the starts given, `lab.ron`'s first gene preset (EVERYTHING) flown by both the behavior
    /// and its first stand-in pilot, its default trials, goal weights and size, the start's build points and hold,
    /// voyage-ready, no escorts beside it.
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
                    // The enemy starts from the first enemy faction's ships, its bosses only when picked.
                    TrialPresetKind::CoEvolution { .. } => {
                        let picked = arena::evolve::enemy_starts(content);
                        enemies(content).into_iter().map(|id| (id.clone(), picked.contains(&id) as u32)).collect()
                    }
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
        let may_change = content.lab.gene_presets.first().map_or_else(MayChange::default, |p| MayChange::only(&p.genes));
        Setup {
            name: String::new(),
            starts: starts.into_iter().map(|s| (s, Locks::default())).collect(),
            flown_by: Flier::default_for(&may_change),
            may_change,
            pilot: content.lab.pilots.first().map(|p| p.id.clone()).unwrap_or_default(),
            trials,
            goals,
            escorts: false,
            size: content.lab.default_size().map(Size::of).unwrap_or_default(),
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

    /// The ARMS RACE's enemy, when it is on: its children a generation, the jobs each gets and their ticks on average.
    pub fn enemy_cost(&self, content: &Content) -> Option<(u32, u32, f32)> {
        self.trials.iter().find_map(|t| t.enemy_cost(content, self.size.population, self.starts.len()))
    }

    /// About how many ticks of work the whole run is (every job lasting its kind's share of its time limit, breeding at
    /// `BREED_COST` a child; an arms race's enemy too).
    pub fn ticks(&self, content: &Content, src: &Sources) -> f64 {
        let (_, child) = self.per_child(content, src);
        let breed = arena::evolve::BREED_COST as f64;
        let enemy = self.enemy_cost(content).map_or(0.0, |(n, jobs, ticks)| n as f64 * (jobs as f64 * ticks as f64 + breed));
        self.size.generations as f64 * (self.size.population as f64 * (child + breed) + enemy)
    }

    /// The run's configuration with these starts (resolved, in order), under the career's limits: only the parts,
    /// materials and counts it has (`?unlock=all` lifts them), and with VOYAGE-READY at most `free_cores` cores; the
    /// trials that are on, filled in from `src`. Refused in words when a trial has nothing to fight or none is on.
    pub fn config(&self, content: &Content, starts: &[StartDesign], granted: &Granted, free_cores: u32, src: &Sources) -> Result<RunConfig, String> {
        let pilot = || {
            let p = content.lab.pilot(&self.pilot).ok_or_else(|| format!("no stand-in pilot `{}`", self.pilot))?;
            Ok::<_, String>(arena::Pilot { profile: p.id.clone(), flies: p.flies.clone() })
        };
        let flown_by = match self.flown_by {
            Flier::Behavior => FlownBy::Behavior,
            Flier::StandIn => FlownBy::StandIn(pilot()?),
            Flier::Both => FlownBy::Both(pilot()?),
        };
        let mut trials = Vec::new();
        let mut enemy = None;
        let population = self.size.population.max(starts.len() as u32);
        for pick in self.trials.iter().filter(|t| t.on && t.weight > 0.0) {
            let p = pick.preset(content).ok_or_else(|| format!("no trial `{}`", pick.preset))?;
            let t = TrialSpec::from_preset(&p, pick.weight, pick.seeds, self.escorts, src)?;
            if matches!(p.kind, TrialPresetKind::CoEvolution { .. }) {
                enemy = Some(Population::from_preset(content, &p, &pick.picked(), population)?);
            }
            trials.push(TrialSpec { flown_by: if t.fights() { flown_by.clone() } else { FlownBy::Behavior }, ..t });
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
            counts: None,
            parts: (!free).then(|| granted.unlocks.parts.clone()),
            materials: (!free).then(|| granted.unlocks.materials.clone()),
            mirror: self.mirror,
        };
        // A run that leaves the design as it is is spread by how it fights, a design run by what it is (`docs/arena.md`
        // "The archive"; `lab.ron`'s spreads).
        let spread = if self.may_change.design() { "design" } else { "behavior" };
        let archive = content.lab.spread(spread).map_or_else(ArchiveShape::default, |s| ArchiveShape {
            axes: s.axes.iter().filter_map(|a| Some(Axis { measure: Measure::from_id(&a.measure)?, min: a.min, max: a.max, cells: a.cells })).collect(),
        });
        Ok(RunConfig {
            seed: self.seed,
            evolve: Evolve::MayChange(self.may_change.clone()),
            generations: Some(self.size.generations),
            population,
            fitness: None,
            trials,
            goals: self.goals,
            limits,
            start: None,
            starts: starts.iter().zip(&self.starts).map(|(s, (_, locks))| s.start(locks.clone())).collect(),
            archive,
            crossover: arena::evolve::DEFAULT_CROSSOVER,
            mutations: arena::evolve::DEFAULT_MUTATIONS,
            enemy,
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
    /// The career has the facility [`FACILITY`] (its research node, or `?unlock=all`): the building works.
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
    /// What the open generation has cost the frames so far (ms on the main thread: breeding, handing out, taking
    /// in; with the jobs out of the frame this is all of it), for the log.
    frame_ms: f32,
    /// The open generation's jobs (or a validation's, or a rating's) out on the threads or the web workers.
    out: Option<out::Out>,
    /// Batches handed out so far (each [`out::Out`]'s number).
    batches: u64,
    /// The EVOLUTION layer is open: the run gets the larger budget ([`budget`]).
    pub layer_open: bool,
    /// What the drydock asks the layer to open on: EVOLVE THIS DESIGN's setup, or a saved design's origin.
    pub request: Option<LabRequest>,
    /// A fight of a run is being watched: home comes back to the lab when flight ends.
    pub watching: bool,
    /// A run that finished in this session and hasn't been opened since: the building glows gold.
    pub fresh: Option<String>,
    /// What went wrong while computing (a write that failed), for the layer.
    pub trouble: Option<String>,
    /// VALIDATE underway: the run it validates and the work (it computes in the run's place until done).
    validating: Option<(String, Validation)>,
    /// RATE ON THE LADDER underway (in the run's place too, after a validation), and the designs waiting for it.
    rating: Option<ladder::Rating>,
    rate_queue: std::collections::VecDeque<ladder::Rating>,
}

impl Lab {
    /// Lists the stored runs (newest first) and restores the one that was computing, so it goes on.
    pub fn load(storage: &Storage, content: &Content) -> Lab {
        let mut lab = Lab::default();
        let ids = match store::ids(storage) {
            Ok(ids) => ids,
            Err(e) => {
                lab.trouble = Some(format!("the runs don't read: {e}"));
                return lab;
            }
        };
        let mut computing = None;
        for id in ids {
            let entry = match store::read(storage, &id) {
                Ok(parts) => {
                    let rules = lab.today(content);
                    let e = RunEntry::of(content, &id, &parts, rules);
                    if e.status == RunStatus::Running && !e.done() && !e.stale && computing.is_none() {
                        computing = Some(id.clone());
                    }
                    e
                }
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
                    race: None,
                    stale: false,
                    problem: Some(e),
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
        let parts = store::read(storage, id)?;
        let run = Run::restore_parts(content, &parts).map_err(|e| e.to_string())?;
        let RunHead { name, created, status, generations, .. } = parts.head;
        self.loaded.push(Loaded {
            id: id.to_string(),
            name: name.unwrap_or_else(|| id.to_string()),
            created: created.unwrap_or(0),
            status: status.unwrap_or(RunStatus::Paused),
            run,
            stored: generations,
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
        let stored = store::ids(storage).map_err(|e| e.to_string())?;
        let id = unique_id(&slug_or(name, "run"), |id| stored.iter().any(|s| s == id) || self.runs.iter().any(|e| e.id == id));
        if status == RunStatus::Running {
            self.pause_running(storage)?;
        }
        let rules = self.today(content);
        let mut loaded = Loaded { id: id.clone(), stored: 0, run, name: name.to_string(), created: now, status };
        let parts = loaded.parts();
        if let Err(e) = store::write_new(storage, &id, &parts, now) {
            // Not added: what was kept of it goes again.
            let _ = store::delete(storage, &id);
            return Err(not_saved(&e));
        }
        loaded.stored = parts.head.generations;
        self.runs.insert(0, RunEntry::of(content, &id, &parts, rules));
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

    /// IMPORT RUN: a run file (a CLI's, `arena evolve --save`, or another lab's) read into the lab, paused, under its
    /// head's name or the file's. It must restore on today's content; under other rules it comes in read-only (stale),
    /// to be re-scored. Refused, in words, when it doesn't read.
    pub fn import(&mut self, storage: &mut Storage, content: &Content, file: &str, text: &str, now: u64) -> Result<String, String> {
        self.room()?;
        let save = arena::RunSave::from_ron(text).map_err(|e| e.to_string())?;
        let run = Run::restore(content, &save).map_err(|e| e.to_string())?;
        let stem = file.rsplit(['/', '\\']).next().unwrap_or(file).trim_end_matches(".ron");
        let name = self.fresh(save.name.as_deref().unwrap_or(if stem.is_empty() { "Imported run" } else { stem }));
        self.add(storage, content, run, &name, now, RunStatus::Paused)
    }

    /// DUPLICATE: the same setup with a new seed, as a new run that computes.
    pub fn duplicate(&mut self, storage: &mut Storage, content: &Content, id: &str, now: u64) -> Result<String, String> {
        self.room()?;
        let mut config = store::read(storage, id)?.config;
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
        match self.loaded.iter_mut().find(|l| l.id == id) {
            Some(l) => {
                l.name = name.to_string();
                store::write_head(storage, id, &l.head(), now_unix()).map_err(|e| not_saved(&e))?;
            }
            None => store::change_head(storage, id, now_unix(), |h| h.name = Some(name.to_string()))?,
        }
        if let Some(e) = self.runs.iter_mut().find(|e| e.id == id) {
            e.name = name.to_string();
        }
        Ok(())
    }

    /// RE-SCORE: a stale run as a new run under today's rules (`arena::Run::rescored`): its elites are fought again
    /// first, then it goes on. The stale run stays as it is.
    pub fn rescore(&mut self, storage: &mut Storage, content: &Content, id: &str, now: u64) -> Result<String, String> {
        self.room()?;
        // The stale run's file as it stands (its rules and all), re-scored.
        let stale = Run::restore_parts(content, &store::read(storage, id)?).map_err(|e| e.to_string())?;
        let run = Run::rescored(content, &stale.snapshot()).map_err(|e| e.to_string())?;
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
        let n = v.batch.jobs().len();
        self.validating = Some((id.to_string(), v));
        self.started = None;
        Ok(n)
    }

    /// The validation underway: its run, how far it is (0..1), the genomes it validates.
    pub fn validating(&self) -> Option<(&str, f32, Vec<u32>)> {
        self.validating.as_ref().map(|(id, v)| (id.as_str(), v.batch.done(), v.genomes()))
    }

    /// A validation is done: its results into its run, written. `true` (the page follows).
    fn validated(&mut self, storage: &mut Storage) -> bool {
        let Some((id, v)) = self.validating.take() else { return false };
        if let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) {
            l.run.validated_by(v);
            if let Err(e) = store::write_head(storage, &id, &l.head(), now_unix()) {
                self.trouble = Some(format!("the validation isn't saved: {}", not_saved(&e)));
            }
        }
        true
    }

    /// CHAMPIONS: every run's best design (its archive's best, rebuilt from its file), the best scores first.
    pub fn champions(&self, storage: &Storage, content: &Content) -> Vec<Entry> {
        let mut out: Vec<(f32, Entry)> = Vec::new();
        for e in self.runs.iter().filter(|e| e.problem.is_none()) {
            let (Ok(parts), Some(score)) = (store::read(storage, &e.id), e.best) else { continue };
            let Some(best) = parts.head.best else { continue };
            if let Ok(g) = parts.genome(content, best) {
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
                let head = RunHead { status: Some(status), ..self.loaded[k].head() };
                store::write_head(storage, id, &head, now_unix()).map_err(|e| not_saved(&e))?;
                self.loaded[k].status = status;
            }
            // Not in memory (a paused run in the list): its head changes, the rest stays.
            None => store::change_head(storage, id, now_unix(), |h| h.status = Some(status))?,
        }
        if let Some(e) = self.runs.iter_mut().find(|e| e.id == id) {
            e.status = status;
        }
        // A generation's time is measured while it computes in one go.
        self.started = None;
        match status {
            RunStatus::Running => {
                self.running = Some(id.to_string());
                // It goes on: what stopped it is past.
                self.trouble = None;
            }
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

    /// DELETE (the page asked first): its documents go, and the run with them.
    pub fn delete(&mut self, storage: &mut Storage, id: &str) -> Result<(), String> {
        store::delete(storage, id).map_err(|e| e.to_string())?;
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
        if self.checkpoint_lost(storage) {
            return true;
        }
        let ticks = self.rate.ticks_for(budget_ms);
        // A validation or a rating computes in the run's place.
        if let Some((_, b)) = aside(&mut self.validating, &mut self.rating) {
            return b.step(ticks) && self.aside_done(storage);
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

    /// The run that computes has closed a generation: its part and its head are written (its checkpoint), its entry
    /// follows, and its end or a failed write stops it. `true` (a generation closed).
    fn closed(&mut self, storage: &mut Storage, id: &str) -> bool {
        let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) else { return false };
        let t1 = Instant::now();
        let (done, now) = (l.run.state().generation, now_unix());
        // The parts not written yet (one, unless a checkpoint failed before), then the head.
        let mut written = (l.stored..done).try_fold(0, |bytes, g| {
            let part = l.run.part(g).expect("a closed generation has its part");
            store::write_part(storage, id, g, &part, now).map(|b| bytes + b)
        });
        if written.is_ok() {
            l.stored = done;
            written = written.and_then(|bytes| store::write_head(storage, id, &l.head(), now).map(|b| bytes + b));
        }
        let took = self.started.take().map_or(0.0, |t| (t1 - t).as_secs_f32());
        let write = t1.elapsed().as_secs_f32() * 1000.0;
        self.last_generation = Some((took, write));
        let p = l.run.progress();
        info!(
            "lab: {id} generation {} of {} in {took:.1} s ({:.1} µs a tick of work, {:.0} ticks/s; {:.0} ms of it in the frames), written in {write:.1} ms ({:.1} kB)",
            p.generation,
            p.generations,
            self.rate.us_per_tick,
            self.rate.ticks_per_second,
            std::mem::take(&mut self.frame_ms),
            written.as_ref().map_or(0.0, |b| *b as f32 / 1024.0)
        );
        let state = l.run.state();
        if let Some(e) = self.runs.iter_mut().find(|e| e.id == id) {
            e.generation = state.history.len() as u32;
            e.best = state.history.last().map(|g| g.archive_best);
            e.first = state.history.first().map(|g| g.best);
        }
        match written {
            Err(e) => self.stop(id, &e),
            Ok(_) if p.finished => {
                self.running = None;
                self.fresh = Some(id.to_string());
            }
            Ok(_) => {}
        }
        true
    }

    /// The run that computes lost its last checkpoint: the web writes behind, and the storage failed to keep what it
    /// wrote (the quota). It stops as it does when a write fails at once. `true` when it did.
    fn checkpoint_lost(&mut self, storage: &Storage) -> bool {
        let Some(id) = self.running.clone() else { return false };
        let Some(e) = storage.unsaved::<store::RunDoc>(&id) else { return false };
        self.stop(&id, &e);
        true
    }

    /// A checkpoint isn't kept: the run waits, so no more work is lost than this generation's.
    fn stop(&mut self, id: &str, e: &StorageError) {
        self.trouble = Some(format!("{}; the run is paused", not_saved(e)));
        if let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) {
            l.status = RunStatus::Paused;
        }
        if self.running.as_deref() == Some(id) {
            self.running = None;
        }
        if let Some(e) = self.runs.iter_mut().find(|e| e.id == id) {
            e.status = RunStatus::Paused;
        }
    }

    /// How far the open generation of a run is, 0..1: its jobs done (out on the threads or the workers), else
    /// `arena::Run::generation_done`.
    pub fn generation_done(&self, l: &Loaded) -> f32 {
        if let Some(o) = self.out.as_ref().filter(|o| o.id == l.id && o.generation == l.run.generation() && self.validating.is_none() && self.rating.is_none()) {
            return o.done() as f32 / o.total().max(1) as f32;
        }
        l.run.generation_done()
    }

    /// The jobs out are of another batch than `id`'s `generation`: they are let go (their results are dropped as
    /// they come).
    fn out_for(&mut self, id: &str, generation: u32) -> bool {
        if self.out.as_ref().is_some_and(|o| o.id == id && o.generation == generation) {
            return true;
        }
        self.out = None;
        false
    }

    /// Hands `jobs` out as a new batch of `id`'s `generation`.
    fn hand_out(&mut self, id: &str, generation: u32, jobs: Vec<Job>) {
        self.batches += 1;
        self.out = Some(out::Out::new(id, generation, self.batches, jobs));
    }

    /// Jobs are out (the web's executor goes on with them, or falls back to slices without them).
    pub fn has_out(&self) -> bool {
        self.out.is_some()
    }

    /// One frame's work with the jobs out of the frame (GDR-0092 "The executor", ADR-0043): the open generation is
    /// bred on this thread within `budget_ms`, then its jobs go to `exec` a few at a time (the native compute threads,
    /// the web's workers, [`out`]); the results are taken as they come and handed to the run in job order once all
    /// are in, which closes the generation as the slices would (the same run). With no budget (in flight) no more
    /// jobs go out and the generation waits to close, so the cores are flight's once the jobs underway end (each
    /// under about 50 ms). A validation or a rating goes out in the run's place (as generation `u32::MAX` of its key).
    pub fn compute_out(&mut self, storage: &mut Storage, exec: &mut dyn out::Executor, budget_ms: f32, frame: f32) -> bool {
        if self.checkpoint_lost(storage) {
            return true;
        }
        let hold = budget_ms <= 0.0;
        if let Some((key, b)) = aside(&mut self.validating, &mut self.rating) {
            let jobs = b.jobs().to_vec();
            if !self.out_for(&key, u32::MAX) {
                if hold {
                    return false;
                }
                self.hand_out(&key, u32::MAX, jobs);
            }
            let o = self.out.as_mut().expect("the jobs are out");
            o.pump(exec, hold);
            if hold || !o.finished() {
                return false;
            }
            let results = self.out.take().expect("the jobs are in").take();
            if let Some((_, b)) = aside(&mut self.validating, &mut self.rating) {
                b.submit(results);
            }
            return self.aside_done(storage);
        }
        let Some(id) = self.running.clone() else {
            self.out = None;
            return false;
        };
        let ticks = self.rate.ticks_for(budget_ms);
        let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) else { return false };
        if !l.computes() {
            return false;
        }
        let generation = l.run.generation();
        if !self.out_for(&id, generation) {
            if hold {
                return false;
            }
            let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) else { return false };
            let (before, t0) = (l.run.worked(), Instant::now());
            self.started.get_or_insert(t0);
            let bred = l.run.breed(ticks);
            self.rate.note(l.run.worked() - before, t0.elapsed().as_secs_f32(), frame);
            if !bred {
                return false;
            }
            let jobs = l.run.jobs().to_vec();
            self.hand_out(&id, generation, jobs);
        }
        let o = self.out.as_mut().expect("the generation's jobs are out");
        let ticks_done = o.pump(exec, hold);
        if frame > 0.0 && ticks_done > 0 {
            self.rate.ticks_per_second += (ticks_done as f32 / frame - self.rate.ticks_per_second) * RATE_FOLLOW;
        }
        if hold || !o.finished() {
            return false;
        }
        let results = self.out.take().expect("the generation's jobs are in").take();
        let Some(l) = self.loaded.iter_mut().find(|l| l.id == id) else { return false };
        l.run.submit(results);
        self.closed(storage, &id)
    }

    /// The jobs out can't finish (the web's workers are gone): they are let go, and the run's slices do the
    /// generation from its start (its jobs are what `arena::Run::step` runs, so the run stays the same).
    pub fn take_back(&mut self) {
        self.out = None;
    }

    /// The work in the run's place is done: a validation's results into its run, or a rating written and the next
    /// one started. `true` (the pages follow).
    fn aside_done(&mut self, storage: &mut Storage) -> bool {
        if self.validating.is_some() {
            return self.validated(storage);
        }
        if let Some(r) = self.rating.take() {
            if let Err(e) = r.write(storage, now_unix()) {
                self.trouble = Some(format!("the rating of {} isn't saved: {}", r.name, not_saved(&e)));
            }
            self.rating = self.rate_queue.pop_front();
            self.started = None;
        }
        true
    }

    /// RATE ON THE LADDER (GDR-0092 step 9): `designs` (ids of YOUR DESIGNS) fight `lab.ron`'s ladder, one after the
    /// other, in the run's place (the run waits, as for VALIDATE); each rating is written to `ladder/<id>` when it is
    /// done. A design already waiting or being rated isn't queued again. How many were queued, or why none.
    pub fn rate(&mut self, storage: &Storage, content: &Content, behaviors: &PlayerBehaviors, working: &PlayerDesign, designs: &[String]) -> Result<usize, String> {
        let ladder = arena::evolve::ladder::Ladder::of(content).ok_or("the content has no ladder")?;
        let rules = self.today(content);
        let mut n = 0;
        for id in designs {
            if self.rating.iter().chain(&self.rate_queue).any(|r| r.design == *id) {
                continue;
            }
            let r = ladder::Rating::new(storage, content, behaviors, working, &ladder, rules, id)?;
            match self.rating {
                None => self.rating = Some(r),
                Some(_) => self.rate_queue.push_back(r),
            }
            n += 1;
        }
        self.started = None;
        Ok(n)
    }

    /// Saved design `id` waits to be rated.
    pub fn queued(&self, id: &str) -> bool {
        self.rate_queue.iter().any(|r| r.design == id)
    }

    /// The rating underway: the design's id and name, how far it is (0..1) and how many designs wait after it.
    pub fn rating(&self) -> Option<(&str, &str, f32, usize)> {
        self.rating.as_ref().map(|r| (r.design.as_str(), r.name.as_str(), r.done(), self.rate_queue.len()))
    }

    /// Today's rules' fingerprint (computed once).
    pub fn rules(&mut self, content: &Content) -> u64 {
        self.today(content)
    }
}

/// The work that computes in the run's place, if any: VALIDATE first, then a rating, with the key its jobs go out
/// under.
fn aside<'a>(validating: &'a mut Option<(String, Validation)>, rating: &'a mut Option<ladder::Rating>) -> Option<(String, &'a mut arena::evolve::JobBatch)> {
    match (validating, rating) {
        (Some((id, v)), _) => Some((id.clone(), &mut v.batch)),
        (None, Some(r)) => Some((format!("ladder/{}", r.design), &mut r.batch)),
        (None, None) => None,
    }
}

/// A failed write in words: a full storage says what makes room.
fn not_saved(e: &StorageError) -> String {
    match e {
        StorageError::Full => format!("not saved: {e}: delete runs you don't need to make room"),
        e => format!("not saved: {e}"),
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

/// Where SAVE AS DESIGN writes: a new design of this name, or over the saved design the genome's start is (REPLACE
/// <START>, GDR-0092: its id and name kept, so FLEET's escorts fly the new version).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveTo<'a> {
    New(&'a str),
    Replace,
}

/// The ids a population's starts go by in an origin (yours; THE ENEMY's in an arms race): a content ship's, a saved
/// design's (its blueprint's id; an unsaved working design's the slug of its name), a random start's faction.
fn start_ids(run: &Run, camp: Camp) -> Vec<String> {
    let starts: Vec<&Start> = match camp {
        Camp::Yours => run.config().all_starts().collect(),
        Camp::Enemy => run.config().enemy.iter().flat_map(|e| &e.starts).collect(),
    };
    starts
        .into_iter()
        .map(|s| match s {
            Start::Design { design: DesignRef::Ship(id), .. } => id.clone(),
            Start::Design { design: DesignRef::Blueprint(text), .. } => data::parse_blueprint(text).map_or_else(|_| String::new(), |b| if b.id.is_empty() { library::slug(&b.name) } else { b.id }),
            Start::Random { faction } => faction.clone(),
        })
        .collect()
}

/// What REPLACE <START> writes over for genome `genome`: its start, when that is one of YOUR DESIGNS still saved (its
/// id and name). A template or a random start has none.
pub fn replace_target(storage: &Storage, loaded: &Loaded, genome: u32) -> Option<(String, String)> {
    let root = loaded.run.table().get(genome as usize)?.root as usize;
    let Start::Design { design: DesignRef::Blueprint(_), .. } = loaded.run.config().all_starts().nth(root)? else { return None };
    let id = start_ids(&loaded.run, Camp::Yours).into_iter().nth(root).filter(|id| !id.is_empty())?;
    let doc = storage.read::<DesignDoc>(&id).ok()??;
    Some((id, doc.doc.0.name))
}

/// SAVE AS DESIGN: genome `genome` of run `loaded` into YOUR DESIGNS (as a new design, or over its start: [`SaveTo`]),
/// in its start's frame (as the drydock saves designs), tagged `evolved` (an arms race's enemy also `enemy`, its faction
/// kept: GDR-0092 "Saving evolved enemies"), with its `origin`. When its behavior evolved
/// (or its start flew a player behavior no longer saved), the behavior is saved beside it as a player behavior,
/// `<name> (evolved)`, and the design names it; else the design names its start's. `tidy`: an evolved behavior loses
/// the rules that never decided in its fights (TIDY, `arena::Run::tidied`). Returns the design's id and the problems
/// `validate_design` finds (none for a genome of a run: the pre-filter checked it).
#[allow(clippy::too_many_arguments)]
pub fn save_as_design(
    storage: &mut Storage,
    content: &Content,
    behaviors: &mut PlayerBehaviors,
    loaded: &Loaded,
    genome: u32,
    to: SaveTo,
    tidy: bool,
    now: u64,
) -> Result<(String, Vec<data::Problem>), String> {
    let run = &loaded.run;
    let g = run.genome(genome).ok_or_else(|| format!("genome {genome} doesn't rebuild"))?;
    let frame = run.frame(g.root).ok_or("its start is gone")?;
    let (generation, score) = match run.scored(genome) {
        Some((generation, child)) => (generation, run.state().records[generation as usize].children[child].score),
        None => (0, 0.0),
    };
    let root = run.start_config(g.root).ok_or("its start is gone")?;
    let start_text = run.start(g.root).map(|s| s.behavior_text().to_string()).unwrap_or_default();
    let tidied = if tidy && g.behavior_text() != start_text { run.tidied(genome).map(|(text, ..)| text) } else { None };
    let text = tidied.as_deref().unwrap_or(g.behavior_text());
    let evolved = text != start_text;
    let Start::Design { design, behavior: flew, .. } = root else { return Err("a random start has no design to keep".into()) };
    let (id, name) = match to {
        SaveTo::New(name) => (library::new_id(storage, content, name).map_err(|e| e.to_string())?, name.to_string()),
        SaveTo::Replace => replace_target(storage, loaded, genome).ok_or("its start is none of your designs")?,
    };
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
            set_text(&mut b, text);
            Some(behaviors.save(storage, content, &b, now)?)
        }
    };
    let camp = run.camp_of(g.root);
    let mut from = start_ids(run, camp);
    // Its own start's first (THE ENEMY's are numbered after yours).
    let own = g.root as usize - if camp == Camp::Enemy { run.starts().len() } else { 0 };
    if own < from.len() {
        let own = from.remove(own);
        from.insert(0, own);
    }
    let bp = Blueprint {
        id: id.clone(),
        name,
        behavior,
        origin: Some(Origin { run: loaded.id.clone(), genome, generation, from, score }),
        ..g.placed(frame)
    };
    let problems = validate_design(content, behaviors, &bp);
    storage.write(&id, &DesignDoc(bp), now).map_err(|e| e.to_string())?;
    Ok((id, problems))
}

/// The budget (ms) the run gets this frame (`lab.ron`'s): none in flight (it pauses), more while the layer is open.
pub fn budget(content: &Content, mode: AppMode, layer_open: bool) -> f32 {
    match (mode, content.lab.budget) {
        (AppMode::Flight, _) | (_, None) => 0.0,
        (_, Some(b)) if layer_open => b.layer_open,
        (_, Some(b)) => b.elsewhere,
    }
}

/// The lab has work: a run that computes, a validation or a rating.
fn busy(lab: &Lab) -> bool {
    lab.enabled && (lab.running.is_some() || lab.validating.is_some() || lab.rating.is_some())
}

/// Every frame at home and on the screens: the run that computes gets its budget (none in flight), its jobs on the
/// compute threads ([`Lab::compute_out`], [`out::Threads`]). Only a closed generation is news for the screens that show
/// the lab (they read the progress inside one themselves while they show it). A locked lab computes nothing.
#[cfg(not(target_arch = "wasm32"))]
pub fn compute(mode: Res<State<AppMode>>, time: Res<Time<Real>>, mut lab: ResMut<Lab>, mut storage: ResMut<Storage>, content: Res<GameContent>, mut threads: Local<Option<out::Threads>>) {
    let ms = budget(&content.0, *mode.get(), lab.layer_open);
    if !busy(&lab) {
        return;
    }
    let exec = threads.get_or_insert_with(|| out::Threads::new(&content.0));
    let (t0, l) = (Instant::now(), lab.bypass_change_detection());
    let changed = l.compute_out(&mut storage, exec, ms, time.delta_secs());
    l.frame_ms += t0.elapsed().as_secs_f32() * 1000.0;
    if changed {
        lab.set_changed();
    }
}

/// Every frame at home and on the screens: the run that computes gets its budget, its jobs in the lab's web workers
/// ([`workers`], started the first time there is work; ADR-0043) while any is there, else in slices on this thread
/// ([`Lab::compute`]), as when `?lab_workers=0`. None in flight (the jobs underway finish beside it), none while the
/// lab is locked.
#[cfg(target_arch = "wasm32")]
#[allow(clippy::too_many_arguments)]
pub fn compute(
    mode: Res<State<AppMode>>,
    time: Res<Time<Real>>,
    mut lab: ResMut<Lab>,
    mut storage: ResMut<Storage>,
    content: Res<GameContent>,
    params: Res<kit::params::Params>,
    mut pool: NonSendMut<LabWorkers>,
) {
    let ms = budget(&content.0, *mode.get(), lab.layer_open);
    if !busy(&lab) || (ms <= 0.0 && !lab.has_out()) {
        return;
    }
    let frame = time.delta_secs();
    let mut pool = match params.flag("lab_workers") {
        Some(false) => None,
        _ => Some(pool.0.get_or_insert_with(|| workers::Workers::start(arena::worker::content_key(&content.0)))),
    };
    let (t0, l) = (Instant::now(), lab.bypass_change_detection());
    if let Some(p) = pool.as_mut() {
        p.poll();
    }
    let changed = match pool {
        Some(p) if p.gone() => {
            l.take_back();
            ms > 0.0 && l.compute(&mut storage, ms, frame)
        }
        Some(p) if l.has_out() || out::Executor::width(p) > 0 => l.compute_out(&mut storage, p, ms, frame),
        _ => ms > 0.0 && l.compute(&mut storage, ms, frame),
    };
    l.frame_ms += t0.elapsed().as_secs_f32() * 1000.0;
    if changed {
        lab.set_changed();
    }
}

/// The web's workers, once started (a non-send resource: browser objects stay on the main thread).
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct LabWorkers(Option<workers::Workers>);

/// The lab opens with its facility: whenever the career's unlocks change (a research, `?unlock=all`).
fn unlock(granted: Res<Granted>, mut lab: ResMut<Lab>) {
    let open = granted.unlocks.has_facility(FACILITY);
    if granted.is_changed() && lab.enabled != open {
        lab.enabled = open;
    }
}

pub struct EvolutionPlugin;

impl Plugin for EvolutionPlugin {
    fn build(&self, app: &mut App) {
        let lab = {
            let world = app.world();
            Lab::load(world.resource::<Storage>(), &world.resource::<GameContent>().0)
        };
        if let Some(t) = &lab.trouble {
            warn!("lab: {t}");
        }
        app.insert_resource(lab).add_systems(Update, (unlock, compute).chain());
        #[cfg(target_arch = "wasm32")]
        app.init_non_send::<LabWorkers>();
    }
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;

    use super::*;
    use arena::RunSave;

    use crate::storage::MemoryBackend;
    use crate::storage::testing::Behind;

    /// The slices these tests compute in (ms): a frame's at home and with the layer open.
    const BUDGET_MS: f32 = 4.0;
    const BUDGET_OPEN_MS: f32 = 10.0;

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

    /// A stored run as its run file: what its parts restore to, with the game's head.
    fn file(storage: &Storage, content: &Content, id: &str) -> RunSave {
        let parts = store::read(storage, id).unwrap();
        let run = Run::restore_parts(content, &parts).unwrap();
        RunSave { name: parts.head.name.clone(), created: parts.head.created, status: parts.head.status, ..run.snapshot() }
    }

    /// A frame's work is the ticks its budget buys at the measured rate, no more (a generation that closes ends it
    /// early); the rate follows what frames measure.
    #[test]
    fn a_frame_works_within_its_budget() {
        let content = data::base_content();
        let mut storage = mem();
        let s = setup(&content);
        let mut lab = Lab::load(&storage, &content);
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
        // Budgets (lab.ron's): none in flight, more with the layer open.
        let b = content.lab.budget.unwrap();
        assert_eq!(budget(&content, AppMode::Flight, true), 0.0);
        assert_eq!((budget(&content, AppMode::Home, true), budget(&content, AppMode::Drydock, false), budget(&content, AppMode::Map, false)), (b.layer_open, b.elsewhere, b.elsewhere));
        let r = Rate { us_per_tick: 5.0, ..default() };
        assert_eq!((r.ticks_for(10.0), r.ticks_for(4.0)), (2000, 800));
    }

    fn app(storage: Storage, lab: Lab, mode: AppMode) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_state(mode)
            .insert_resource(storage)
            .insert_resource(lab)
            .insert_resource(Granted { unlocks: data::career::Unlocks::all(&data::base_content()), ..default() })
            .insert_resource(GameContent(data::base_content()))
            .add_systems(Update, (unlock, compute).chain());
        app
    }

    /// The run computes at home and on the screens (natively its fights on the threads), and waits in flight: no
    /// breeding, no generation closes, nothing lost.
    #[test]
    fn the_run_pauses_in_flight() {
        let content = data::base_content();
        let mut storage = mem();
        let s = setup(&content);
        let mut lab = Lab::load(&storage, &content);
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
        // A locked lab computes nothing, wherever the player is.
        app.world_mut().resource_mut::<Granted>().unlocks.facilities.clear();
        app.world_mut().resource_mut::<NextState<AppMode>>().set(AppMode::Drydock);
        app.update();
        app.update();
        assert!(!app.world().resource::<Lab>().enabled);
        assert_eq!(worked(&app), after.0, "locked: nothing");
    }

    /// The web's executor in a test: each chunk goes through the worker's message form and its answer
    /// (`arena::worker`) as a `lab-worker` gets and answers it, and answers come back a few frames later, in another
    /// order than they went out.
    struct Messages {
        content: Content,
        key: u64,
        width: usize,
        waiting: Vec<String>,
    }

    impl out::Executor for Messages {
        fn width(&self) -> usize {
            self.width
        }

        fn spawn(&mut self, batch: u64, first: usize, jobs: &[Job]) {
            let task = arena::worker::Task { batch, first: first as u32, content: self.key, jobs: jobs.to_vec() };
            let reply = arena::worker::answer(&self.content, self.key, &arena::worker::write(&task));
            self.waiting.push(arena::worker::write(&reply));
        }

        fn finished(&mut self) -> Vec<(u64, usize, Vec<arena::evolve::JobResult>)> {
            // The latest answer first, and only one a frame while more than two are out.
            let n = if self.waiting.len() > 2 { 1 } else { self.waiting.len() };
            (0..n)
                .filter_map(|_| self.waiting.pop())
                .map(|text| match arena::worker::read(&text).unwrap() {
                    arena::worker::Reply::Done { batch, first, results } => (batch, first as usize, results),
                    other => panic!("{other:?}"),
                })
                .collect()
        }
    }

    /// A generation's jobs out of the frame make the run the slices make, file for file: on the native threads, and
    /// through the web workers' messages (GDR-0092 step 9: a worker's results are the frame's).
    #[test]
    fn jobs_out_of_the_frame_make_the_run_the_slices_make() {
        let content = data::base_content();
        let s = setup(&content);
        let mut sliced = mem();
        let mut lab = Lab::load(&sliced, &content);
        let cfg = config(&content, &s, &sliced);
        let a = lab.create(&mut sliced, &content, cfg, "Small", 10).unwrap();
        while lab.running().is_some() {
            lab.compute(&mut sliced, BUDGET_MS, 0.016);
        }
        let doc = |st: &Storage, id: &str| file(st, &content, id).to_ron();
        let messages = Messages { content: content.clone(), key: arena::worker::content_key(&content), width: 3, waiting: Vec::new() };
        #[cfg(not(target_arch = "wasm32"))]
        bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
        #[cfg(not(target_arch = "wasm32"))]
        let execs: Vec<Box<dyn out::Executor>> = vec![Box::new(messages), Box::new(out::Threads::new(&content))];
        #[cfg(target_arch = "wasm32")]
        let execs: Vec<Box<dyn out::Executor>> = vec![Box::new(messages)];
        for mut exec in execs {
            let mut out = mem();
            let mut lab = Lab::load(&out, &content);
            let cfg = config(&content, &s, &out);
            let b = lab.create(&mut out, &content, cfg, "Small", 10).unwrap();
            let mut frames = 0;
            while lab.running().is_some() {
                // Now and then a frame in flight: no job goes out, the generation waits.
                let ms = if frames % 7 == 3 { 0.0 } else { BUDGET_MS };
                lab.compute_out(&mut out, &mut *exec, ms, 0.016);
                frames += 1;
                assert!(frames < 100_000, "the jobs never came back");
            }
            assert_eq!(doc(&out, &b), doc(&sliced, &a));
        }
    }

    /// RATE ON THE LADDER (GDR-0092 step 9): a saved design's rating computes in the run's place, the same in slices
    /// and out of the frame, and is kept in `ladder/<id>`; changing the design makes it a rating from before.
    #[test]
    fn a_design_is_rated_on_the_ladder() {
        let content = data::base_content();
        let (design, behaviors) = (PlayerDesign::starter(&content, "rim_skiff"), PlayerBehaviors::default());
        let skiff = content.ships.get("rim_skiff").unwrap().clone();
        let mut docs = Vec::new();
        for out in [false, true] {
            let mut storage = mem();
            storage.write("my_skiff", &DesignDoc(Blueprint { id: "my_skiff".into(), name: "My skiff".into(), ..skiff.clone() }), 5).unwrap();
            let mut lab = Lab::load(&storage, &content);
            assert_eq!(lab.rate(&storage, &content, &behaviors, &design, &["my_skiff".into(), "my_skiff".into()]), Ok(1));
            assert_eq!(lab.rating().map(|r| r.1), Some("My skiff"));
            let mut exec = Messages { content: content.clone(), key: arena::worker::content_key(&content), width: 2, waiting: Vec::new() };
            let mut frames = 0;
            while lab.rating().is_some() {
                if out {
                    lab.compute_out(&mut storage, &mut exec, BUDGET_MS, 0.016);
                } else {
                    lab.compute(&mut storage, BUDGET_MS, 0.016);
                }
                frames += 1;
                assert!(frames < 100_000, "the rating never finished");
            }
            let rules = lab.rules(&content);
            let ladder::Standing::Rated(doc) = ladder::Standing::of(&storage, &content, &behaviors, &design, rules, "my_skiff") else { panic!("not rated") };
            assert_eq!(doc.rating.of(), arena::evolve::ladder::Ladder::of(&content).unwrap().rungs().len());
            assert!(doc.code(&content, 0).is_some(), "its fights play again");
            docs.push(doc);
            let mut changed = skiff.clone();
            changed.parts.pop();
            storage.write("my_skiff", &DesignDoc(Blueprint { id: "my_skiff".into(), name: "My skiff".into(), ..changed }), 6).unwrap();
            assert!(matches!(ladder::Standing::of(&storage, &content, &behaviors, &design, rules, "my_skiff"), ladder::Standing::Changed(_)));
        }
        assert_eq!(docs[0], docs[1], "out of the frame, the same rating");
    }

    /// A worker refuses a task for other content, and a task that doesn't read.
    #[test]
    fn a_worker_fights_only_for_its_content() {
        let content = data::base_content();
        let key = arena::worker::content_key(&content);
        let task = arena::worker::Task { batch: 1, first: 0, content: key ^ 1, jobs: Vec::new() };
        assert!(matches!(arena::worker::answer(&content, key, &arena::worker::write(&task)), arena::worker::Reply::Refused(_)));
        assert!(matches!(arena::worker::answer(&content, key, "(nonsense"), arena::worker::Reply::Refused(_)));
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
        let mut lab = Lab::load(&storage, &content);
        let cfg = config(&content, &s, &storage);
        let id = lab.create(&mut storage, &content, cfg, "Small", 10).unwrap();
        run_to_end(&mut lab, &mut storage);
        let straight = file(&storage, &content, &id);
        assert_eq!(straight.history.len(), 3);
        assert_eq!(lab.fresh.as_deref(), Some(id.as_str()), "a finished run glows until it is opened");

        let mut storage = mem();
        let mut lab = Lab::load(&storage, &content);
        let cfg = config(&content, &s, &storage);
        let id = lab.create(&mut storage, &content, cfg, "Small", 10).unwrap();
        while lab.get(&id).unwrap().run.generation() < 1 {
            lab.compute(&mut storage, BUDGET_MS, 1.0 / 60.0);
        }
        for _ in 0..3 {
            lab.compute(&mut storage, BUDGET_MS, 1.0 / 60.0);
        }
        // The page reloads: the lab is read from the storage again and the run goes on.
        let mut lab = Lab::load(&storage, &content);
        let entry = lab.entry(&id).unwrap().clone();
        assert_eq!((entry.generation, entry.status, entry.stale, entry.name.as_str()), (1, RunStatus::Running, false, "Small"));
        assert_eq!(lab.running().map(|l| l.id.as_str()), Some(id.as_str()));
        run_to_end(&mut lab, &mut storage);
        let resumed = file(&storage, &content, &id);
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
        let mut lab = Lab::load(&storage, &content);
        assert!(lab.runs().is_empty() && lab.trouble.is_none() && lab.running().is_none());
        let s = setup(&content);
        let cfg = config(&content, &s, &storage);
        let a = lab.create(&mut storage, &content, cfg, "Run A", 10).unwrap();
        let cfg = config(&content, &s, &storage);
        let b = lab.create(&mut storage, &content, cfg, "Run A", 20).unwrap();
        assert_ne!(a, b);
        assert_eq!(lab.runs().iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), [b.as_str(), a.as_str()], "newest first");
        assert_eq!(lab.entry(&a).unwrap().status, RunStatus::Paused, "the new run paused the other");
        assert_eq!(file(&storage, &content, &a).status, Some(RunStatus::Paused));
        lab.resume(&mut storage, &content, &a).unwrap();
        assert_eq!((lab.entry(&b).unwrap().status, lab.running().unwrap().id.as_str()), (RunStatus::Paused, a.as_str()));
        lab.set_status(&mut storage, &a, RunStatus::Paused).unwrap();
        assert!(lab.running().is_none());
        assert!(!lab.compute(&mut storage, BUDGET_MS, 0.016));
        lab.delete(&mut storage, &a).unwrap();
        assert_eq!(store::ids(&storage).unwrap(), std::slice::from_ref(&b));
        assert!(lab.entry(&a).is_none() && lab.get(&a).is_none());
        // A stored run that doesn't read is listed with why.
        let mut text_store = MemoryBackend::default();
        crate::storage::Backend::write(&mut text_store, "lab", "broken", "// space-game: lab, format 1\n(format: 1").unwrap();
        let broken = Lab::load(&Storage::new(text_store), &content);
        assert!(broken.entry("broken").unwrap().problem.is_some());
    }

    /// Memory that notes every write: (folder, name, bytes).
    #[derive(Default)]
    struct Counting(MemoryBackend, std::sync::Arc<std::sync::Mutex<Vec<(String, String, usize)>>>);

    impl crate::storage::Backend for Counting {
        fn list(&self, folder: &str) -> Result<Vec<String>, StorageError> {
            self.0.list(folder)
        }
        fn read(&self, folder: &str, name: &str) -> Result<Option<String>, StorageError> {
            self.0.read(folder, name)
        }
        fn write(&mut self, folder: &str, name: &str, text: &str) -> Result<(), StorageError> {
            self.1.lock().unwrap().push((folder.into(), name.into(), text.len()));
            self.0.write(folder, name, text)
        }
        fn delete(&mut self, folder: &str, name: &str) -> Result<(), StorageError> {
            self.0.delete(folder, name)
        }
        fn location(&self) -> String {
            "counting".into()
        }
    }

    /// In parts: a new run writes its configuration and head; each generation writes its own part and the head, and
    /// nothing else (the head stays small); the parts read back as the run computed it; a lost part is named; DELETE
    /// removes every document of the run.
    #[test]
    fn a_generation_writes_its_part_and_the_head() {
        let content = data::base_content();
        let backend = Counting::default();
        let log = backend.1.clone();
        let written = || -> Vec<(String, usize)> { log.lock().unwrap().drain(..).map(|(f, n, b)| (format!("{f}/{n}"), b)).collect() };
        let mut storage = Storage::new(backend);
        let s = setup(&content);
        let mut lab = Lab::load(&storage, &content);
        let cfg = config(&content, &s, &storage);
        let id = lab.create(&mut storage, &content, cfg, "Small", 10).unwrap();
        let names = |w: &[(String, usize)]| w.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
        assert_eq!(names(&written()), [format!("lab/{id}/config"), format!("lab/{id}")], "made: its configuration and head");
        let mut generation = 0;
        while lab.running().is_some() {
            if lab.compute(&mut storage, BUDGET_OPEN_MS, 0.016) {
                let w = written();
                assert_eq!(names(&w), [format!("lab/{id}/g{generation}"), format!("lab/{id}")], "generation {generation}");
                assert!(w[1].1 < 1024, "the head stays small: {} bytes", w[1].1);
                generation += 1;
            }
        }
        assert_eq!(generation, 3);
        assert_eq!(storage.parts::<store::RunDoc>(&id).unwrap(), ["config", "g0", "g1", "g2"]);
        let l = lab.get(&id).unwrap();
        let computed = RunSave { name: Some(l.name.clone()), created: Some(l.created), status: Some(l.status), ..l.run.snapshot() };
        assert_eq!(file(&storage, &content, &id).to_ron(), computed.to_ron(), "the parts are the run");
        // A part gone: the run is listed with why.
        let mut damaged = Storage::new(MemoryBackend::default());
        store::write_new(&mut damaged, "r", &l.parts(), 1).unwrap();
        damaged.delete::<store::RunDoc>("r/g1").unwrap();
        let listed = Lab::load(&damaged, &content);
        assert!(listed.entry("r").unwrap().problem.as_deref().is_some_and(|p| p.contains("generation 1")));
        lab.delete(&mut storage, &id).unwrap();
        assert!(storage.parts::<store::RunDoc>(&id).unwrap().is_empty() && store::ids(&storage).unwrap().is_empty());
    }

    /// The web writes behind: a checkpoint the storage failed to keep (the quota) pauses the run with the words a
    /// failed write has (the storage is full, delete runs to make room). Until there is room, RESUME is refused the
    /// same way; then the unsaved documents go out again and the run goes on to its end, every part kept.
    #[test]
    fn a_checkpoint_the_storage_couldnt_keep_pauses_the_run() {
        let content = data::base_content();
        let s = setup(&content);
        let backend = Behind::default();
        let db = Behind(backend.0.clone());
        let mut storage = Storage::new(backend);
        let mut lab = Lab::load(&storage, &content);
        let cfg = config(&content, &s, &storage);
        let id = lab.create(&mut storage, &content, cfg, "Small", 10).unwrap();
        db.commit(false);
        while lab.get(&id).unwrap().run.generation() < 1 {
            lab.compute(&mut storage, BUDGET_MS, 0.016);
        }
        db.commit(true);
        assert!(lab.compute(&mut storage, BUDGET_MS, 0.016), "the page follows");
        assert!(lab.running().is_none());
        assert_eq!(lab.entry(&id).unwrap().status, RunStatus::Paused);
        let trouble = lab.trouble.clone().unwrap();
        assert!(trouble.contains("the storage is full") && trouble.contains("delete runs") && trouble.ends_with("the run is paused"), "{trouble}");
        assert!(lab.resume(&mut storage, &content, &id).unwrap_err().contains("the storage is full"));
        db.commit(false);
        lab.resume(&mut storage, &content, &id).unwrap();
        while lab.running().is_some() {
            lab.compute(&mut storage, BUDGET_OPEN_MS, 0.016);
            db.commit(false);
        }
        let kept: Vec<String> = db.0.lock().unwrap().1.keys().cloned().collect();
        assert_eq!(kept, [format!("lab/{id}"), format!("lab/{id}/config"), format!("lab/{id}/g0"), format!("lab/{id}/g1"), format!("lab/{id}/g2")]);
    }

    /// SAVE AS DESIGN: the best genome becomes a saved design in the run's frame with its origin and the EVOLVED tag;
    /// it reads back, validates and opens in the drydock as it was saved; an evolved behavior is saved beside it.
    #[test]
    fn save_as_design_keeps_its_origin_and_opens_in_the_drydock() {
        let content = data::base_content();
        let mut storage = mem();
        let s = setup(&content);
        let mut lab = Lab::load(&storage, &content);
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
        // A template start is none of YOUR DESIGNS: nothing to replace.
        assert_eq!(replace_target(&storage, loaded, best.id), None);
        assert!(save_as_design(&mut storage, &content, &mut behaviors, loaded, best.id, SaveTo::Replace, false, 99).is_err());
        let (saved, problems) = save_as_design(&mut storage, &content, &mut behaviors, loaded, best.id, SaveTo::New(&name), false, 99).unwrap();
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

    /// The ARMS RACE (GDR-0092 step 7): off by default; on, the setup's run has an enemy of the first enemy faction's
    /// ships (its boss only when picked) at half the children, it computes and stores like any run, and an evolved
    /// enemy saves as a design of its faction tagged `enemy`, with its origin.
    #[test]
    fn an_arms_race_runs_and_its_enemy_saves_as_a_design() {
        let content = data::base_content();
        let mut storage = mem();
        let mut s = setup(&content);
        let race = s.trials.iter().position(|t| t.preset == "arms_race").unwrap();
        let fresh = Setup::new(&content, vec![StartPick::Template("rim_skiff".into())], 7).trials.swap_remove(race);
        assert!(!fresh.on, "off by default");
        assert_eq!(fresh.picked(), ["automaton_gunner", "automaton_hunter", "automaton_raider"], "the warden is a boss");
        s.trials[race] = fresh;
        assert!(config(&content, &s, &storage).enemy.is_none());
        s.trials[race].on = true;
        assert_eq!(s.enemy_cost(&content).map(|(n, jobs, _)| (n, jobs)), Some((3, 12)), "3 children (one per picked ship), (3 best + 2 champions + 1 start) × 2 seeds");
        let cfg = config(&content, &s, &storage);
        let enemy = cfg.enemy.clone().unwrap();
        assert_eq!((enemy.starts.len(), enemy.population, enemy.palette.as_slice()), (3, 3, ["shield_emitter".to_string()].as_slice()));
        let mut lab = Lab::load(&storage, &content);
        let id = lab.create(&mut storage, &content, cfg, "Race", 10).unwrap();
        while lab.running().is_some() {
            lab.compute(&mut storage, BUDGET_OPEN_MS, 0.016);
        }
        let loaded = lab.get(&id).unwrap();
        assert_eq!(file(&storage, &content, &id).to_ron(), RunSave { name: Some("Race".into()), created: Some(10), status: Some(RunStatus::Running), ..loaded.run.snapshot() }.to_ron(), "its parts are its run");
        let theirs = loaded.run.enemy_archive().unwrap().best().unwrap().clone();
        let mut behaviors = PlayerBehaviors::default();
        let (saved, problems) = save_as_design(&mut storage, &content, &mut behaviors, loaded, theirs.id, SaveTo::New("Hunter evolved"), true, 99).unwrap();
        assert!(problems.iter().all(|p| !p.is_error()), "{problems:?}");
        let bp = storage.read::<DesignDoc>(&saved).unwrap().unwrap().doc.0;
        assert_eq!(bp.faction, "automatons");
        assert!(bp.tags.contains(&"enemy".to_string()) && bp.tags.contains(&"evolved".to_string()));
        let origin = bp.origin.unwrap();
        assert_eq!((origin.run.as_str(), origin.genome), (id.as_str(), theirs.id));
        assert!(origin.from[0].starts_with("automaton_") && origin.from.len() == 3, "its own start first: {:?}", origin.from);
        assert_eq!(replace_target(&storage, loaded, theirs.id), None, "an enemy replaces no design of yours");
    }

    /// REPLACE <START>: a genome of a saved design's run writes over that design, its id and name kept, with its
    /// origin; TIDY saves its evolved behavior without the rules that never decided in its fights.
    #[test]
    fn replace_writes_over_the_start_and_tidy_drops_rules() {
        let content = data::base_content();
        let mut storage = mem();
        let skiff = content.ships.get("rim_skiff").unwrap().clone();
        storage.write("my_skiff", &DesignDoc(Blueprint { id: "my_skiff".into(), name: "My skiff".into(), ..skiff }), 5).unwrap();
        let mut s = setup(&content);
        s.starts = vec![(StartPick::Design("my_skiff".into()), Locks::default())];
        let mut lab = Lab::load(&storage, &content);
        let cfg = config(&content, &s, &storage);
        let id = lab.create(&mut storage, &content, cfg, "Mine", 10).unwrap();
        while lab.running().is_some() {
            lab.compute(&mut storage, BUDGET_OPEN_MS, 0.016);
        }
        let loaded = lab.get(&id).unwrap();
        let start_text = loaded.run.starts()[0].behavior_text().to_string();
        // The best genome whose behavior evolved, else the best.
        let elites = loaded.run.results().elites;
        let best = elites.iter().find(|e| e.genome.behavior_text() != start_text).unwrap_or(&elites[0]).clone();
        assert_eq!(replace_target(&storage, loaded, best.id), Some(("my_skiff".into(), "My skiff".into())));
        let mut behaviors = PlayerBehaviors::default();
        let (saved, problems) = save_as_design(&mut storage, &content, &mut behaviors, loaded, best.id, SaveTo::Replace, true, 99).unwrap();
        assert!(problems.iter().all(|p| !p.is_error()), "{problems:?}");
        assert_eq!(saved, "my_skiff");
        let doc = storage.read::<DesignDoc>("my_skiff").unwrap().unwrap();
        let bp = doc.doc.0;
        assert_eq!((bp.name.as_str(), doc.saved), ("My skiff", Some(99)));
        let origin = bp.origin.unwrap();
        assert_eq!((origin.run.as_str(), origin.genome, origin.from.as_slice()), (id.as_str(), best.id, ["my_skiff".to_string()].as_slice()));
        let evolved = best.genome.behavior_text() != start_text;
        let want = match loaded.run.tidied(best.id) {
            Some((text, _, fights)) if evolved => {
                assert!(fights > 0);
                text
            }
            _ => best.genome.behavior_text().to_string(),
        };
        match bp.behavior.as_deref().and_then(|b| behaviors.get(b)) {
            Some(p) => assert_eq!(bare(&content, &behavior_text(&p.behavior)).unwrap(), want),
            None => assert!(!evolved, "an evolved behavior is saved beside it"),
        }
        assert_eq!(storage.list(<DesignDoc as crate::storage::Document>::NAMESPACE).unwrap(), ["my_skiff"], "no second design");
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
        let mut lab = Lab::load(&made, &content);
        let cfg = config(&content, &s, &made);
        let id = lab.create(&mut made, &content, cfg, "Small", 10).unwrap();
        while lab.running().is_some() {
            lab.compute(&mut made, BUDGET_OPEN_MS, 0.016);
        }
        // What `arena evolve --save` writes: the run file without the game's head.
        let full = file(&made, &content, &id);
        let cli = RunSave { name: None, created: None, status: None, ..full.clone() }.to_ron();
        let mut storage = mem();
        let mut lab = Lab::load(&storage, &content);
        let imported = lab.import(&mut storage, &content, "runs/deep_skiff.ron", &cli, 50).unwrap();
        let e = lab.entry(&imported).unwrap().clone();
        assert_eq!((e.name.as_str(), e.status, e.generation, e.stale), ("deep_skiff", RunStatus::Paused, 3, false));
        let back = file(&storage, &content, &imported);
        assert_eq!(RunSave { name: None, created: None, status: None, ..back }.to_ron(), cli, "it round-trips");
        assert!(lab.running().is_none(), "an imported run waits");
        assert!(lab.import(&mut storage, &content, "x.ron", "(format: 1", 50).is_err());
        lab.rename(&mut storage, &imported, "  Deep skiff  ").unwrap();
        assert_eq!(lab.entry(&imported).unwrap().name, "Deep skiff");
        assert_eq!(file(&storage, &content, &imported).name.as_deref(), Some("Deep skiff"));
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
        let save = file(&storage, &content, &imported);
        assert_eq!(Run::restore(&content, &save).unwrap().validated(best), Some(&v));
        // DUPLICATE: the same setup, a new seed, computing.
        let copy = lab.duplicate(&mut storage, &content, &imported, 60).unwrap();
        let c = lab.get(&copy).unwrap();
        assert_eq!((c.run.config().trials.clone(), c.status), (save.config.trials.clone(), RunStatus::Running));
        assert_ne!(c.run.config().seed, save.config.seed);
        assert_eq!(lab.entry(&copy).unwrap().name, "Deep skiff 2");
        // RE-SCORE: a run fought under other rules is stale; re-scored, a new run fights its elites again.
        let stale = RunSave { rules: save.rules ^ 1, ..save };
        store::write_new(&mut storage, "old_rules", &RunParts::of(&stale).unwrap(), 70).unwrap();
        let mut lab = Lab::load(&storage, &content);
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
        // GDR-0092 step 6: everything may change, and every fight is flown by the behavior on odd seeds and lab.ron's
        // first stand-in on even ones.
        assert_eq!(cfg.evolve, Evolve::MayChange(MayChange::default()));
        let casual = arena::Pilot { profile: "casual".into(), flies: content.lab.pilot("casual").unwrap().flies.clone() };
        assert!(cfg.trials.iter().all(|t| t.flown_by == FlownBy::Both(casual.clone())));
        Run::new(&content, cfg).unwrap();
        // A preset without the assists flies the behavior.
        let mut behavior = s.clone();
        behavior.may_change = MayChange::only(&content.lab.gene_presets.iter().find(|p| p.id == "behavior_only").unwrap().genes);
        behavior.flown_by = Flier::default_for(&behavior.may_change);
        let cfg = behavior.config(&content, std::slice::from_ref(&w), &granted, 4, &src).unwrap();
        assert!(cfg.trials.iter().all(|t| t.flown_by == FlownBy::Behavior));
        assert_eq!(cfg.archive.axes[0].measure, Measure::EngageDistance, "spread by how it fights");
        // Without an open voyage THE ENEMIES YOU MEET has nothing to fight; without a trial on, nothing scores.
        assert!(s.config(&content, std::slice::from_ref(&w), &granted, 4, &Sources::default()).unwrap_err().contains("no voyage is open"));
        let mut none = s.clone();
        none.trials.iter_mut().for_each(|t| t.on = false);
        assert!(none.config(&content, &[w], &granted, 4, &src).is_err());
        // The estimate: every trial's jobs a child (three encounters of the first voyage × 4 seeds, the waves 2, the
        // start 2, the hall of fame 4 × 2).
        assert_eq!(s.per_child(&content, &src).0, 12 + 2 + 2 + 8);
        assert!(s.ticks(&content, &src) > 0.0);
        let t = StartPick::Template("halvex_runner".into()).resolve(&storage, &content, &behaviors, &design).unwrap();
        assert!(matches!(t.start(Locks::default()), Start::Design { design: DesignRef::Ship(ref id), .. } if id == "halvex_runner"));
        assert_eq!(templates(&content, "ship_mind").first().map(String::as_str), Some("halvex_runner"));
        assert!(enemies(&content).contains(&"automaton_raider".to_string()));
    }
}
