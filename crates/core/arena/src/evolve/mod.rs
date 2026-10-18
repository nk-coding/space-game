//! Evolution (track L3 of the evolution lab, GDR-0083, GDR-0032; GDR-0092 steps 2 and 3; `docs/arena.md`
//! "Evolution"): designs and behaviors bred through the editing models from one to four starts, scored by fights
//! against a pool or by the slice's voyages, kept in a MAP-Elites archive.
//!
//! A [`Run`] is built from a [`RunConfig`] (a RON text file). Each generation breeds `population` children from
//! the archive's elites (generation 0: the starts and their mutants), drops those the pre-filter refuses (the limits,
//! the starts' locks), evaluates the rest (fights or voyage runs: [`Job`]s) and offers each to its archive cell.
//! [`Run::step`] does a bounded slice of that work, breeding included (so a client can call it every frame);
//! [`Run::jobs`] and [`Run::submit`] hand a whole generation's jobs out and take their results back, so the native
//! CLI runs them on every core. Either way the run is the same: a job's result depends only on the job, results are
//! applied in job order, and every random draw comes from the run's own stream in a fixed order. Single-threaded, no
//! I/O, no wall clock.
//!
//! Every fight is kept (GDR-0092): every genome bred is numbered and stored as a change against its parent
//! ([`Bred`], [`Run::genome`], [`Run::lineage`]), every job as a small record ([`JobRecord`]), so any fight of any
//! child is found again by (generation, child, job) and becomes a fight code ([`Run::fight_code`]). Between two
//! generations a run is a [`RunSave`] ([`Run::snapshot`]); [`Run::restore`] goes on from it exactly as the run would
//! have, and marks the run stale when the rules it was fought under have changed.
//!
//! An ARMS RACE (GDR-0092 step 7, "Co-evolution", [`Population`]) has a second population, THE ENEMY, evolve in the
//! same run: its starts follow yours in the one genome table, its children follow yours in each generation's
//! records, it breeds on its own stream, and both sides fight frozen sets of the other's best and past champions
//! (your side always side 0), with a PAST CHAMPIONS check every few generations ([`Run::beaten`]).
//!
//! ```
//! let content = data::base_content();
//! let config = arena::RunConfig::from_ron(r#"(
//!     seed: 1,
//!     evolve: Behavior,
//!     generations: 1,
//!     population: 2,
//!     trials: [(kind: EnemyShips(opponents: [(design: Ship("automaton_raider"))]), seeds: 1, time_limit: 5)],
//!     starts: [Design(design: Ship("rim_skiff"))],
//! )"#).unwrap();
//! let mut run = arena::Run::new(&content, config).unwrap();
//! while !run.step(10_000).finished {}
//! let best = run.results().elites[0].clone();
//! assert!((0.0..=1.0).contains(&best.score));
//! let (blueprint, behavior) = (best.genome.blueprint_text(), best.genome.behavior_text()); // SAVE AS DESIGN
//! assert!(data::parse_blueprint(&blueprint).is_ok() && !behavior.is_empty());
//! let code = run.fight_code(0, 0, 0).unwrap(); // the start's first fight, to re-watch
//! assert!(code.encode().starts_with("SGF1:"));
//! ```

mod archive;
mod config;
mod eval;
mod genome;
pub mod ladder;
mod lineage;
mod store;
mod trial;

use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

use data::behavior::Program;
use data::{AssistList, BehaviorEditor, Blueprint, Content, Placement};
use serde::{Deserialize, Serialize};
use sim::{Pcg32, SimConfig};

use crate::code::{Code, FightCode};
use crate::fight::{Fight, resolve_design};
use crate::rules::rules_fingerprint;
use crate::spec::{BehaviorRef, DesignRef, Entry, FightSpec, Formation};
use crate::voyage::start_voyage;

pub use archive::{Archive, ArchiveShape, Axis, Elite, MAX_CELLS, Measure};
pub use config::{
    Bound, DEFAULT_CHAMPIONS_EVERY, DEFAULT_CHECK_EVERY, DEFAULT_CROSSOVER, DEFAULT_ELITES, DEFAULT_HALL,
    DEFAULT_MUTATIONS, DEFAULT_POPULATION, DEFAULT_SEEDS, Evolve, Gene, Genes, Limits, Locks, MayChange,
    MAX_GENERATIONS, MAX_MUTATIONS, MAX_PAIRING, MAX_POPULATION, MAX_SEEDS, MAX_STARTS, Population, RunConfig, Stage,
    Start, VoyagePilot,
};
pub use eval::{Job, JobBatch, JobEnd, JobRecord, JobResult, SAMPLE_TICKS, job_code};
pub use genome::{Genome, unmatched};
pub use lineage::Bred;
pub use store::{MAX_SAVE_PACKED, MAX_SAVE_PAYLOAD, RunHead, RunPart, RunParts, RunSave, RunStatus, SAVE_FORMAT, SAVE_TAG};
pub use trial::{BOSS_TAG, Contestant, DEFAULT_WAVES, FlownBy, GenCtx, GoalScores, Goals, MAX_WEIGHT, Sources, Trial, TrialKind, TrialSpec, encounters, enemy_starts, entry_of, pack};

use eval::Active;
use genome::{Kit, Locked, Root, Stats};

/// The PCG sequence of a run's random stream (`"evolve"` in ASCII), so it never repeats a sim stream.
const RUN_STREAM: u64 = 0x6576_6f6c_7665;
/// The sequence of THE ENEMY's stream in an arms race (`"enemy"`): its breeding and the generations' draws of past
/// champions, so yours draws as it would without it.
const ENEMY_STREAM: u64 = 0x65_6e65_6d79;
/// Breeding tries per child before it is given up (the pre-filter refused every one).
const CHILD_TRIES: u32 = 16;
/// What breeding one child counts against [`Run::step`]'s budget, in ticks: about 0.3 ms natively (10 ms per
/// generation of 32 designs, `docs/arena.md` "Performance"), at 5–6 µs a tick. PLACEHOLDER.
pub const BREED_COST: u64 = 60;

/// Why a run cannot be built or restored.
#[derive(Clone, Debug, PartialEq)]
pub enum RunError {
    /// The configuration does not read or a number is out of range.
    Config(String),
    /// A start is not usable: its design or behavior does not resolve, it breaks a limit or the pre-filter, a lock
    /// names something it lacks.
    Start(String),
    /// The fitness does not work with the starts (an unknown opponent or voyage, ...).
    Fitness(String),
    /// A run file does not read: damaged, of a newer format, or not this run's.
    Save(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Config(e) => write!(f, "the run configuration is not usable: {e}"),
            RunError::Start(e) => write!(f, "the start is not usable: {e}"),
            RunError::Fitness(e) => write!(f, "a trial is not usable: {e}"),
            RunError::Save(e) => write!(f, "the run file is not usable: {e}"),
        }
    }
}

impl std::error::Error for RunError {}

/// One generation's numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationStats {
    pub generation: u32,
    /// Index of its stage, and what it evolved.
    pub stage: usize,
    pub genes: Genes,
    /// Children evaluated.
    pub evaluated: u32,
    /// Children the pre-filter refused (each try counts).
    pub rejected: u32,
    /// Best and mean score of the children.
    pub best: f32,
    pub mean: f32,
    /// Best score in the archive after the generation.
    pub archive_best: f32,
    /// Occupied archive cells after the generation.
    pub filled: u32,
    /// Children that took a cell (a new one, or a better score).
    pub improved: u32,
    /// Ticks simulated.
    pub ticks: u64,
    /// A RE-SCORE (GDR-0092): the archive's elites fought again under new rules, the archive rebuilt from their new
    /// scores; it bred nothing and takes none of the run's generations (written only when it is one).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rescore: bool,
    /// THE ENEMY's numbers in an arms race (the ones above are yours; `ticks` counts both).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enemy: Option<EnemyStats>,
}

/// THE ENEMY's numbers of one generation of an arms race (as [`GenerationStats`]' for yours).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyStats {
    pub evaluated: u32,
    pub rejected: u32,
    pub best: f32,
    pub mean: f32,
    pub archive_best: f32,
    pub filled: u32,
    pub improved: u32,
}

/// What a run keeps of one generation besides its numbers: whom its children fought and how each of their jobs
/// ended, so every job is rebuilt ([`Run::job`]) and re-watched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationRecord {
    /// The hall of fame its children fought besides the configured opponents: genome numbers, best first.
    pub opponents: Vec<u32>,
    /// Its children in breeding order.
    pub children: Vec<ChildRecord>,
    /// The rules' fingerprint it was fought under, when it is not the run's (a generation before a RE-SCORE); its
    /// fight codes carry it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<u64>,
    /// An arms race's: whom each side fought, where THE ENEMY's children begin among `children`, PAST CHAMPIONS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enemy: Option<Rivalry>,
}

/// What a generation of an arms race keeps of the enemy population (GDR-0092 "Co-evolution"): the frozen sets both
/// sides fought, so every job rebuilds, and its PAST CHAMPIONS check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rivalry {
    /// The enemy genomes your children fought: its best, then its past champions drawn (genome numbers).
    pub opponents: Vec<u32>,
    /// Your genomes its children fought: your best, your past champions drawn, then your starts (the anchor).
    pub rivals: Vec<u32>,
    /// The generation's `children` from this one on are THE ENEMY's (yours before them).
    pub first: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<Check>,
}

/// PAST CHAMPIONS (GDR-0092): your best then fought THE ENEMY's champions of every `champions_every`th generation so
/// far, and its best yours. A champion is a generation's best child of its side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    /// Your archive's best and the enemy's when the generation opened.
    pub yours: u32,
    pub theirs: u32,
    /// The champions each one fought: the enemy's (yours fought them), then yours (its best fought them).
    pub their_champions: Vec<u32>,
    pub your_champions: Vec<u32>,
    /// The fights in order: your best against each of their champions (every seed), then their best against each
    /// of yours; each scored for the best that fought (side 0 for yours, side 1 for theirs).
    pub jobs: Vec<JobRecord>,
}

/// Which population of an arms race a genome is of: yours, or THE ENEMY (its starts are numbered after yours).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Camp {
    #[default]
    Yours,
    Enemy,
}

/// The PAST CHAMPIONS line of a checked generation: how many of the other side's champions each best beat (a mean
/// score above 0.5 over its fights), of how many.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Beaten {
    pub generation: u32,
    pub yours: (u32, u32),
    pub theirs: (u32, u32),
}

/// One child of a generation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildRecord {
    /// Its number in the genome table.
    pub genome: u32,
    pub score: f32,
    /// Its value of each archive axis's measure (its cell: `Archive::cell_of`).
    pub measures: Vec<f32>,
    /// It took a cell (a new one, or a better score).
    pub took: bool,
    /// Its jobs in order (every opponent × seed, or every voyage × seed).
    pub jobs: Vec<JobRecord>,
}

/// How far a run is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// The generation in progress (or the number done, once finished).
    pub generation: u32,
    pub generations: u32,
    pub finished: bool,
    /// The rules have changed since the run started ([`Run::restore`]): it is read-only and doesn't step.
    pub stale: bool,
}

/// The run as it stands, for a client to show.
#[derive(Clone, Copy, Debug)]
pub struct RunState<'a> {
    /// Generations done.
    pub generation: u32,
    pub generations: u32,
    /// The stage of the next generation (the last one once finished).
    pub stage: usize,
    pub history: &'a [GenerationStats],
    pub records: &'a [GenerationRecord],
    pub archive: &'a Archive,
}

/// A child's score taken apart (the run's page): per trial its score and its goals' means over its jobs, and the goals
/// over all trials as the score weighs them.
#[derive(Clone, Debug, PartialEq)]
pub struct Breakdown {
    pub trials: Vec<TrialScore>,
    pub goals: GoalScores,
}

/// One trial of a child's score.
#[derive(Clone, Debug, PartialEq)]
pub struct TrialScore {
    /// Its index in the run's trials (`Run::trials`).
    pub trial: usize,
    /// Its jobs; a trial without jobs (the hall of fame in generation 0) has no score and leaves the sums.
    pub jobs: usize,
    pub score: Option<f32>,
    pub goals: GoalScores,
}

/// A run's results.
#[derive(Clone, Debug, PartialEq)]
pub struct Results {
    pub history: Vec<GenerationStats>,
    /// Every elite of the archive, best first (equal scores in cell order).
    pub elites: Vec<Elite>,
}

/// VALIDATE's result for one genome (GDR-0092): its score on new seeds (the ones after each trial's own) with each
/// trial's part, and its recorded fights fought again under today's rules: how many still end as recorded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validated {
    pub genome: u32,
    /// New seeds per opponent.
    pub seeds: u32,
    pub score: f32,
    /// Per trial (`Run::trials`), its score on the new seeds (none without jobs).
    pub trials: Vec<Option<f32>>,
    /// Its recorded jobs that ended as recorded, of all of them.
    pub as_recorded: (u32, u32),
}

/// VALIDATE as work (GDR-0092): the jobs of some genomes on new seeds and their recorded jobs again, steppable like a
/// generation ([`Validation::step`], or [`Validation::jobs`] and [`Validation::submit`] on threads); [`Run::validated_by`]
/// takes the results into the run.
pub struct Validation {
    seeds: u32,
    /// Per genome: its number, its trials' tags of the new jobs, and its recorded jobs (the job's index and record).
    genomes: Vec<(u32, Vec<u32>, Vec<JobRecord>, f32)>,
    /// Per job: the genome's index and whether it is a recorded one.
    owner: Vec<(usize, bool)>,
    /// Its jobs and their results.
    pub batch: JobBatch,
}

/// One child of the current generation.
struct Candidate {
    genome: Genome,
    id: u32,
    stats: Stats,
}

/// The arms race of the current generation: the frozen sets, as genome numbers and as contestants, where THE ENEMY's
/// children begin (once yours are bred), its breeding so far, and PAST CHAMPIONS' jobs at the end of the batch.
struct Frozen {
    /// The enemy's stream before this generation's first draw (what a snapshot keeps, as `Batch::from`).
    from: Pcg32,
    opponents: Vec<u32>,
    foes: Vec<Contestant>,
    rivals: Vec<u32>,
    friends: Vec<Contestant>,
    first: Option<usize>,
    failed: usize,
    rejected: u32,
    /// The check's bests and champions, and its jobs' place in the batch.
    check: Option<(Check, Range<usize>)>,
}

/// THE ENEMY of an arms race as the run holds it: its breeding kit (its starts numbered after yours), its stream, its
/// starts and its archive.
struct Enemy {
    kit: Kit,
    rng: Pcg32,
    starts: Vec<Genome>,
    archive: Archive,
}

/// What breeding a child of one population reads: the content, that population's kit, starts and archive, and the
/// run's numbers.
struct Breeder<'a> {
    content: &'a Content,
    kit: &'a Kit,
    starts: &'a [Genome],
    archive: &'a Archive,
    crossover: f32,
    mutations: u32,
    generation: u32,
}

/// The current generation: its children so far, their jobs, the results so far.
struct Batch {
    stage: usize,
    genes: Genes,
    /// The run's random stream before this generation's first draw: what a snapshot keeps, so a restored run breeds
    /// the generation again from it.
    from: Pcg32,
    /// The genome table's length before this generation.
    first: usize,
    /// The hall of fame its children fight, and as fight entries.
    opponents: Vec<u32>,
    hall: Vec<Entry>,
    candidates: Vec<Candidate>,
    /// Children given up (the pre-filter refused every try).
    failed: usize,
    rejected: u32,
    /// Each candidate's jobs, and each job's trial.
    ranges: Vec<Range<usize>>,
    jobs: Vec<Job>,
    trial_of: Vec<u32>,
    results: Vec<Option<JobResult>>,
    /// The next job to start.
    next: usize,
    active: Option<(usize, Active)>,
    /// A RE-SCORE: the archive's elites again, nothing bred.
    rescore: bool,
    /// An arms race's.
    rivalry: Option<Frozen>,
}

/// An evolution run (see the module docs).
pub struct Run {
    content: Content,
    config: RunConfig,
    kit: Kit,
    /// The trials (`RunConfig::trials`) resolved against the content, in order.
    trials: Vec<(TrialSpec, Box<dyn TrialKind>)>,
    /// The starts as fight entries under their own behaviors (BEAT YOUR START).
    start_entries: Vec<Entry>,
    stages: Vec<(Genes, u32)>,
    generations: u32,
    /// The rules' fingerprint when the run started.
    rules: u64,
    stale: bool,
    rng: Pcg32,
    /// Per start, the header every genome of it carries.
    headers: Vec<Blueprint>,
    starts: Vec<Genome>,
    archive: Archive,
    history: Vec<GenerationStats>,
    records: Vec<GenerationRecord>,
    /// Every genome bred, numbered in breeding order (the starts first).
    table: Vec<Bred>,
    batch: Option<Batch>,
    /// The ticks of work [`Run::step`] has done since the run was built or restored.
    worked: u64,
    /// A RE-SCORE is due: the next generation fights the archive's elites again ([`Run::rescored`]).
    rescore: bool,
    /// VALIDATE's results, by genome.
    validated: Vec<Validated>,
    /// THE ENEMY of an arms race.
    enemy: Option<Enemy>,
}

/// VALIDATE's default: the archive's best this many, on this many new seeds each (GDR-0092, PLACEHOLDER).
pub const VALIDATE_BEST: usize = 8;
pub const VALIDATE_SEEDS: u32 = 20;

/// Where genome `id` (bred in generation `born`) was last scored: the latest RE-SCORE that fought it, else its own
/// generation; its generation and index among that generation's children.
fn scored_in(history: &[GenerationStats], records: &[GenerationRecord], born: u32, id: u32) -> Option<(u32, usize)> {
    (born as usize..records.len()).rev().filter(|&g| g == born as usize || history.get(g).is_some_and(|h| h.rescore)).find_map(|g| {
        let k = records[g].children.iter().position(|c| c.genome == id)?;
        Some((g as u32, k))
    })
}

/// The behavior a design flies by default: its own, else its faction's; as text.
fn default_behavior(content: &Content, bp: &Blueprint) -> Result<String, String> {
    let id = bp
        .behavior
        .clone()
        .or_else(|| content.factions.get(&bp.faction).and_then(|f| f.behavior.clone()))
        .ok_or_else(|| format!("`{}` has no default behavior; name one", bp.id))?;
    content_behavior(content, &id)
}

/// A behavior text in its canonical form without its notes: a note describes a rule as written, and evolution
/// rewrites the rules.
fn without_notes(content: &Content, text: &str) -> Result<String, String> {
    let mut ed = BehaviorEditor::from_text(&content.behavior_nodes, text)
        .map_err(|e| format!("the behavior text does not read: {e:?}"))?;
    let rules: Vec<_> = ed.rules().map(|r| r.id).collect();
    for id in rules {
        ed.set_rule_note(id, None).map_err(|e| e.to_string())?;
    }
    ed.set_target_note(None).map_err(|e| e.to_string())?;
    Ok(ed.text())
}

fn content_behavior(content: &Content, id: &str) -> Result<String, String> {
    let b = content
        .behaviors
        .get(id)
        .ok_or_else(|| format!("no behavior `{id}` in the content"))?;
    let program = b
        .program()
        .map_err(|e| format!("the behavior `{id}` does not read: {e:?}"))?;
    Ok(program.to_text())
}

/// A genome as a fight entry: its blueprint and behavior inlined as text.
fn genome_entry(g: &Genome, count: u32) -> Entry {
    Entry {
        design: DesignRef::Blueprint(g.blueprint_text()),
        behavior: Some(BehaviorRef::Text(g.behavior.clone())),
        count,
        formation: Formation::Line,
        pilot: None,
    }
}

/// THE ENEMY's score from its jobs' goals (GDR-0092 "Co-evolution": side 1's WIN, the mean over its fights), with its
/// breakdown: its one trial (`trial`, CO-EVOLUTION) and the goals' means.
fn enemy_score(trial: usize, goals: &[GoalScores]) -> (f32, Breakdown) {
    let n = goals.len().max(1) as f64;
    let mean = |k: usize| (goals.iter().map(|g| g.all()[k] as f64).sum::<f64>() / n) as f32;
    let means = GoalScores { win: mean(0), unhurt: mean(1), quick: mean(2), cheap: mean(3) };
    let score = (!goals.is_empty()).then_some(means.win);
    (means.win, Breakdown { trials: vec![TrialScore { trial, jobs: goals.len(), score, goals: means }], goals: means })
}

/// CHEAP: one less a design's build points over its limit, 0..1.
fn cheapness(build_points: f32, limit: f32) -> f32 {
    if limit > 0.0 { (1.0 - build_points / limit).clamp(0.0, 1.0) } else { 0.0 }
}

/// A start resolved against the content: its blueprint as written (none for a random one), its header and its
/// behavior text.
struct StartText {
    design: Option<Blueprint>,
    header: Blueprint,
    behavior: String,
    locks: Locks,
}

/// The header every child of a start carries: the start's, renamed, tagged `evolved` (THE ENEMY's `enemy` too, its
/// faction kept); the behavior goes beside it as text.
fn header_of(start: &Blueprint, camp: Camp) -> Blueprint {
    let tags = match camp {
        Camp::Yours => vec!["evolved".into()],
        Camp::Enemy => vec!["evolved".into(), sim::voyage::waves::ENEMY_TAG.into()],
    };
    Blueprint {
        id: "evolved".into(),
        name: format!("{} (evolved)", start.name),
        summary: format!("Evolved from {} in the evolution lab.", start.name),
        description: None,
        lore: None,
        tags,
        behavior: None,
        // A start that was evolved before is a start like any other here.
        origin: None,
        source: Default::default(),
        ..start.clone()
    }
}

/// The headers of a configuration's starts (yours, then THE ENEMY's), as a run gives its genomes.
fn headers_of(content: &Content, config: &RunConfig) -> Result<Vec<Blueprint>, RunError> {
    let yours = config.starts.iter().map(|s| (s, Camp::Yours));
    let enemy = config.enemy.iter().flat_map(|e| &e.starts).map(|s| (s, Camp::Enemy));
    yours.chain(enemy).map(|(s, camp)| resolve_start(content, s).map(|t| header_of(&t.header, camp)).map_err(RunError::Start)).collect()
}

fn resolve_start(content: &Content, start: &Start) -> Result<StartText, String> {
    match start {
        Start::Design {
            design,
            behavior,
            locks,
        } => {
            let (bp, _) = resolve_design(content, design, behavior.as_ref()).map_err(|p| match p {
                crate::fight::DesignProblem::UnknownShip(id) => format!("no ship `{id}` in the content"),
                crate::fight::DesignProblem::Blueprint(ps) => ps.join("; "),
            })?;
            let text = match behavior {
                None => default_behavior(content, &bp),
                Some(BehaviorRef::Content(id)) => content_behavior(content, id),
                Some(BehaviorRef::Text(t)) => BehaviorEditor::from_text(&content.behavior_nodes, t)
                    .map(|e| e.text())
                    .map_err(|e| format!("the behavior text does not read: {e:?}")),
            }?;
            Ok(StartText {
                header: bp.clone(),
                behavior: without_notes(content, &text)?,
                design: Some(bp),
                locks: locks.clone(),
            })
        }
        Start::Random { faction } => {
            let f = content
                .factions
                .get(faction)
                .ok_or_else(|| format!("no faction `{faction}` in the content"))?;
            let id = f
                .behavior
                .clone()
                .ok_or_else(|| format!("`{faction}` has no default behavior"))?;
            let header = Blueprint {
                id: "evolved".into(),
                name: "Random design".into(),
                summary: "A random design, evolved.".into(),
                description: None,
                lore: None,
                tags: Vec::new(),
                faction: faction.clone(),
                size: (0, 0),
                parts: Vec::new(),
                behavior: None,
                assists: None,
                origin: None,
                source: Default::default(),
            };
            Ok(StartText {
                header,
                behavior: without_notes(content, &content_behavior(content, &id)?)?,
                design: None,
                locks: Locks::default(),
            })
        }
    }
}

/// Opens a population's starts (yours, or THE ENEMY's numbered from `kit.first`) in its frame (the configured one,
/// else the smallest content frame every start fits) centred once, each checked against the limits, its locks and the
/// pre-filter, its own cost and hold its genomes' limits by default; with their headers.
fn open_starts(content: &Content, kit: &mut Kit, texts: Vec<StartText>, limits: &Limits, camp: Camp, rng: &mut Pcg32) -> Result<(Vec<Blueprint>, Vec<Genome>), RunError> {
    let frame = match &limits.frame {
        Some(id) => content
            .frames
            .get(id)
            .map(|f| f.size)
            .ok_or_else(|| RunError::Config(format!("no frame `{id}` in the content")))?,
        None => {
            let w = texts.iter().map(|t| t.header.size.0).max().unwrap_or(0);
            let h = texts.iter().map(|t| t.header.size.1).max().unwrap_or(0);
            content
                .frames
                .iter()
                .filter(|f| f.size.0 >= w && f.size.1 >= h)
                .min_by_key(|f| f.size.0 as u64 * f.size.1 as u64)
                .map(|f| f.size)
                .ok_or_else(|| RunError::Start("no content frame is big enough".into()))?
        }
    };
    let mut headers = Vec::new();
    let mut starts = Vec::new();
    for (k, t) in texts.into_iter().enumerate() {
        let root = kit.first + k as u32;
        let at = |e: String| {
            RunError::Start(match camp {
                Camp::Yours if k == 0 => e,
                Camp::Yours => format!("start {}: {e}", k + 1),
                Camp::Enemy => format!("the enemy's start {}: {e}", k + 1),
            })
        };
        let header = header_of(&t.header, camp);
        let g = match &t.design {
            Some(written) => {
                let bp = Blueprint {
                    parts: written.parts.clone(),
                    size: written.size,
                    ..header.clone()
                };
                // Opened centred once; from then on genomes keep their cells.
                let fits = |f| kit.dock(content, &bp, f, false);
                let size = fits(frame)
                    .ok_or_else(|| at("it does not fit the run's frame".into()))?
                    .export()
                    .size;
                let edit = kit.frame_for(frame, size);
                let dock = fits(edit).ok_or_else(|| at("it does not fit the run's frame".into()))?;
                let program = Program::parse(&t.behavior).map_err(|e| at(format!("{e:?}")))?;
                let locks = Locked::resolve(content, &t.locks, written, &dock, &program).map_err(at)?;
                kit.roots.push(Root {
                    frame: edit,
                    build_points: None,
                    start_bp: 0.0,
                    min_hold: None,
                    locks,
                });
                Genome::of(&dock, t.behavior, root)
            }
            None => {
                kit.roots.push(Root {
                    frame: kit.frame_for(frame, (0, 0)),
                    build_points: None,
                    start_bp: 0.0,
                    min_hold: None,
                    locks: Locked::default(),
                });
                kit.random_design(content, &header, &t.behavior, root, rng)
                    .ok_or_else(|| at("no random design with the parts allowed flies".into()))?
            }
        };
        let report = sim::design_report(content, &g.blueprint, &SimConfig::default()).map_err(at)?;
        let r = &mut kit.roots[k];
        r.build_points = limits.build_points.of(report.build_points);
        r.start_bp = report.build_points;
        r.min_hold = limits.min_hold.of(content.ship_hold(&g.blueprint) as f32);
        kit.assess(content, &g).map_err(at)?;
        headers.push(header);
        starts.push(g);
    }
    Ok((headers, starts))
}

/// One child of a population (GDR-0092 steps 2 and 3): crossover (sometimes) and mutations of a random elite of its
/// archive (generation 0: of a start, shared out evenly), of the genes `genes` may change; its behavior cut to its
/// design's rule slots. `None` when the pre-filter refused every try; `rejected` counts the refusals. With the child,
/// how it was bred.
fn breed(pop: &Breeder, rng: &mut Pcg32, genes: &Genes, made: usize, rejected: &mut u32) -> Option<(Genome, Stats, Bred)> {
    let may = genes.may_change();
    let content = pop.content;
    let n = pop.starts.len();
    let elites: Vec<(u32, &Genome)> = if pop.archive.cells().is_empty() {
        let k = made.saturating_sub(n) % n;
        vec![(pop.kit.first + k as u32, &pop.starts[k])]
    } else {
        pop.archive.cells().values().map(|e| (e.id, &e.genome)).collect()
    };
    for _ in 0..CHILD_TRIES {
        let (id, a) = elites[rng.below(elites.len() as u32) as usize];
        let mut g = a.clone();
        let mut partner = None;
        if elites.len() > 1 && rng.chance(pop.crossover) {
            let (bid, b) = elites[rng.below(elites.len() as u32) as usize];
            if may.design()
                && let Some(c) = pop.kit.cross_design(content, &g, b, rng)
            {
                g = c;
                partner = Some(bid);
            }
            if may.behavior() {
                let slots = content.ship_rule_slots(&g.blueprint);
                if let Some(t) = pop.kit.cross_behavior(&g.behavior, &b.behavior, slots, g.root, rng) {
                    g.behavior = t;
                    partner = Some(bid);
                }
            }
            if may.assists()
                && let Some(c) = pop.kit.cross_assists(&g, b, rng)
            {
                g = c;
                partner = Some(bid);
            }
        }
        let mutations = 1 + rng.below(pop.mutations);
        for _ in 0..mutations {
            if let Some(m) = pop.kit.mutate(content, &g, &may, rng) {
                g = m;
            }
        }
        // A design change may have cost rule slots: the lowest unlocked rules go.
        match pop.kit.fit_rules(&g.behavior, content.ship_rule_slots(&g.blueprint), g.root) {
            Some(t) => g.behavior = t,
            None => {
                *rejected += 1;
                continue;
            }
        }
        if g == *a {
            *rejected += 1;
            continue;
        }
        match pop.kit.assess(content, &g) {
            Ok(stats) => {
                let (removed, added) = lineage::diff(&a.frame_parts(), &g.frame_parts());
                let bred = Bred {
                    generation: pop.generation,
                    root: g.root,
                    parent: Some(id),
                    partner,
                    removed,
                    added,
                    behavior: (g.behavior != a.behavior).then(|| g.behavior.clone()),
                    assists: (g.blueprint.assists != a.blueprint.assists)
                        .then(|| g.blueprint.assists.clone().unwrap_or(AssistList { rules: Vec::new() })),
                    size: g.blueprint.size,
                };
                return Some((g, stats, bred));
            }
            Err(_) => *rejected += 1,
        }
    }
    None
}

impl Run {
    /// Checks the configuration, resolves the starts (or builds random ones), opens each centred in its frame once
    /// and checks it against the limits, its locks and the pre-filter, and builds one job of each opponent or voyage
    /// with each start to check the fitness. Takes the rules' fingerprint (`rules_fingerprint`, a few ms).
    pub fn new(content: &Content, config: RunConfig) -> Result<Run, RunError> {
        config.check()?;
        let stages = config.schedule()?;
        let generations = stages.iter().map(|s| s.1).sum();
        let specs = config.trials.clone();
        if !specs.iter().any(TrialSpec::fights) && config.archive.axes.iter().any(|a| a.measure.needs_fights()) {
            return Err(RunError::Config(
                "`EngageDistance` is measured in fights, and no trial fights".into(),
            ));
        }
        let trials = specs
            .into_iter()
            .map(|t| t.resolve(content).map(|k| (t, k)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut rng = Pcg32::new(sim::rng::splitmix64(config.seed ^ RUN_STREAM), RUN_STREAM);
        let texts = config
            .starts.iter()
            .map(|s| resolve_start(content, s))
            .collect::<Result<Vec<_>, _>>()
            .map_err(RunError::Start)?;
        let mut kit = Kit::new(content, &config.limits);
        let (mut headers, starts) = open_starts(content, &mut kit, texts, &config.limits, Camp::Yours, &mut rng)?;
        // THE ENEMY: its starts numbered after yours, built with the parts and armour its starts use and its palette.
        let enemy = match &config.enemy {
            None => None,
            Some(pop) => {
                let texts = pop
                    .starts
                    .iter()
                    .map(|s| resolve_start(content, s))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| RunError::Start(format!("the enemy: {e}")))?;
                let used = |f: &dyn Fn(&Placement) -> Option<String>| -> BTreeSet<String> {
                    texts.iter().flat_map(|t| t.design.iter().flat_map(|d| d.parts.iter().filter_map(f))).collect()
                };
                let limits = pop.limits(used(&|p| Some(p.part.clone())), used(&|p| p.material.clone()));
                let mut ekit = Kit::new(content, &limits);
                ekit.first = starts.len() as u32;
                let mut erng = Pcg32::new(sim::rng::splitmix64(config.seed ^ ENEMY_STREAM), ENEMY_STREAM);
                let (h, es) = open_starts(content, &mut ekit, texts, &limits, Camp::Enemy, &mut erng)?;
                headers.extend(h);
                Some(Enemy { kit: ekit, rng: erng, starts: es, archive: Archive::new(pop.archive.clone()) })
            }
        };
        let table = starts
            .iter()
            .chain(enemy.iter().flat_map(|e| &e.starts))
            .map(|g| Bred {
                generation: 0,
                root: g.root,
                parent: None,
                partner: None,
                removed: Vec::new(),
                added: g.frame_parts().into_iter().enumerate().map(|(k, p)| (k as u32, p)).collect(),
                behavior: Some(g.behavior.clone()),
                assists: None,
                size: g.blueprint.size,
            })
            .collect();
        let run = Run {
            content: content.clone(),
            archive: Archive::new(config.archive.clone()),
            config,
            kit,
            trials,
            start_entries: starts.iter().map(|g| genome_entry(g, 1)).collect(),
            stages,
            generations,
            rules: rules_fingerprint(content),
            stale: false,
            rng,
            headers,
            starts,
            history: Vec::new(),
            records: Vec::new(),
            table,
            batch: None,
            worked: 0,
            rescore: false,
            validated: Vec::new(),
            enemy,
        };
        // Every job of every start (generation 0's, with no hall of fame yet, against the other side's starts): the
        // references resolve.
        let (yours, theirs) = (run.start_contestants(Camp::Yours), run.start_contestants(Camp::Enemy));
        let mut jobs = Vec::new();
        for (k, g) in run.starts.iter().enumerate() {
            jobs.extend(run.jobs_of(g, run.kit.roots[k].start_bp, &[], &theirs));
        }
        for (g, e) in run.enemy_starts().iter().zip(&theirs) {
            jobs.extend(run.enemy_jobs_of(g, e.build_points, &yours));
        }
        for (_, job) in jobs {
            let ok = match &job {
                Job::Fight { spec, .. } => Fight::new(content, spec).map(|_| ()),
                Job::Voyage { spec, .. } => start_voyage(content, spec).map(|_| ()),
            };
            ok.map_err(|e| RunError::Fitness(e.to_string()))?;
        }
        Ok(run)
    }

    pub fn config(&self) -> &RunConfig {
        &self.config
    }

    /// The run's trials in order (`RunConfig::trials`): what a [`Breakdown`]'s trial index names.
    pub fn trials(&self) -> Vec<&TrialSpec> {
        self.trials.iter().map(|(t, _)| t).collect()
    }

    /// The starts (generation 0's first children, genomes 0 to n − 1), each opened centred in its frame.
    pub fn starts(&self) -> &[Genome] {
        &self.starts
    }

    /// THE ENEMY's starts in an arms race (genomes n to n + m − 1 after your n), each opened centred in its frame.
    pub fn enemy_starts(&self) -> &[Genome] {
        self.enemy.as_ref().map_or(&[], |e| &e.starts)
    }

    /// Start `root` (yours, else THE ENEMY's) as opened, and as configured.
    pub fn start(&self, root: u32) -> Option<&Genome> {
        self.starts.iter().chain(self.enemy_starts()).nth(root as usize)
    }

    pub fn start_config(&self, root: u32) -> Option<&Start> {
        self.config.starts.iter().chain(self.config.enemy.iter().flat_map(|e| &e.starts)).nth(root as usize)
    }

    /// The population start `root` is of.
    pub fn camp_of(&self, root: u32) -> Camp {
        if (root as usize) < self.starts.len() { Camp::Yours } else { Camp::Enemy }
    }

    /// The population genome `id` is of (`None`: no such genome).
    pub fn camp(&self, id: u32) -> Option<Camp> {
        self.table.get(id as usize).map(|b| self.camp_of(b.root))
    }

    /// THE ENEMY's archive in an arms race.
    pub fn enemy_archive(&self) -> Option<&Archive> {
        self.enemy.as_ref().map(|e| &e.archive)
    }

    /// A population's archive (yours; THE ENEMY's in an arms race).
    pub fn archive_of(&self, camp: Camp) -> Option<&Archive> {
        match camp {
            Camp::Yours => Some(&self.archive),
            Camp::Enemy => self.enemy_archive(),
        }
    }

    /// The kit start `root`'s genomes are bred with.
    fn kit_of(&self, root: u32) -> &Kit {
        match (self.camp_of(root), &self.enemy) {
            (Camp::Enemy, Some(e)) => &e.kit,
            _ => &self.kit,
        }
    }

    /// The frame genomes of start `root` are edited in (`Genome::placed`).
    pub fn frame(&self, root: u32) -> Option<(u32, u32)> {
        self.kit_of(root).root(root).map(|r| r.frame)
    }

    /// The rules' fingerprint when the run started (`rules_fingerprint`).
    pub fn rules(&self) -> u64 {
        self.rules
    }

    /// The rules have changed since the run started: its fights may no longer end as recorded, and it doesn't
    /// step (GDR-0092: it is kept read-only, or re-scored).
    pub fn stale(&self) -> bool {
        self.stale
    }

    /// Generations done.
    pub fn generation(&self) -> u32 {
        self.history.len() as u32
    }

    pub fn finished(&self) -> bool {
        self.generation() >= self.generations
    }

    /// The stage of generation `g` (the last stage past the end): RE-SCOREs before it take no stage's generations.
    fn stage_of(&self, g: u32) -> usize {
        let g = g - self.history.iter().take(g as usize).filter(|h| h.rescore).count() as u32;
        let mut end = 0;
        for (k, s) in self.stages.iter().enumerate() {
            end += s.1;
            if g < end {
                return k;
            }
        }
        self.stages.len() - 1
    }

    pub fn state(&self) -> RunState<'_> {
        RunState {
            generation: self.generation(),
            generations: self.generations,
            stage: self.stage_of(self.generation()),
            history: &self.history,
            records: &self.records,
            archive: &self.archive,
        }
    }

    pub fn archive(&self) -> &Archive {
        &self.archive
    }

    pub fn results(&self) -> Results {
        Results {
            history: self.history.clone(),
            elites: self.archive.top(usize::MAX).into_iter().cloned().collect(),
        }
    }

    pub fn progress(&self) -> Progress {
        Progress {
            generation: self.generation(),
            generations: self.generations,
            finished: self.finished(),
            stale: self.stale,
        }
    }

    /// The ticks of work [`Run::step`] has done since the run was built or restored, breeding counted at
    /// [`BREED_COST`] a child: a client measures its cost per tick against it (GDR-0092: the frame budget is
    /// converted to ticks with it).
    pub fn worked(&self) -> u64 {
        self.worked
    }

    /// How far the open generation is, 0..1: its jobs done of the jobs it will have (estimated from the children
    /// bred so far while it is still breeding). 0 between generations.
    pub fn generation_done(&self) -> f32 {
        let Some(b) = &self.batch else { return 0.0 };
        let made = b.candidates.len() + b.failed + b.rivalry.as_ref().map_or(0, |r| r.failed);
        if made == 0 && !b.rescore {
            return 0.0;
        }
        let population = self.config.population + self.config.enemy.as_ref().map_or(0, |e| e.population);
        let expected = if b.rescore { b.jobs.len() as f32 } else { b.jobs.len() as f32 * population as f32 / made as f32 };
        let done = b.results.iter().filter(|r| r.is_some()).count() as f32;
        (done / expected.max(1.0)).clamp(0.0, 1.0)
    }

    /// The genome table: how every genome so far was bred (the starts first; the open generation's children too).
    pub fn table(&self) -> &[Bred] {
        &self.table
    }

    /// Genome `id`, rebuilt from the table: the same genome the run bred.
    pub fn genome(&self, id: u32) -> Option<Genome> {
        lineage::rebuild(&self.table, &self.headers, id)
    }

    /// Genome `id`'s line from its start: the genome numbers of its start, each parent, and itself.
    pub fn lineage(&self, id: u32) -> Vec<u32> {
        let mut line = Vec::new();
        let mut k = self.table.get(id as usize).map(|_| id);
        while let Some(n) = k {
            line.push(n);
            k = self.table[n as usize].parent.filter(|&p| p < n);
        }
        line.reverse();
        line
    }

    /// Does up to `budget` ticks of work: breeding the next generation child by child ([`BREED_COST`] each) when
    /// needed, then its fights or voyages, closing the generation once its last job is done. Call it every frame
    /// with a small budget (the client converts its time budget with the measured cost per tick), or with a big
    /// one; the run is the same however it is sliced. A stale run doesn't step.
    pub fn step(&mut self, budget: u64) -> Progress {
        if self.finished() || self.stale {
            return self.progress();
        }
        let mut b = match self.batch.take() {
            Some(b) => b,
            None => self.open(),
        };
        let mut left = budget;
        while left > 0 && !self.bred(&b) {
            self.breed_child(&mut b);
            left = left.saturating_sub(BREED_COST);
        }
        let content = &self.content;
        while left > 0 && self.bred(&b) {
            if b.active.is_none() {
                if b.next >= b.jobs.len() {
                    break;
                }
                b.active = Some((b.next, b.jobs[b.next].start(content)));
                b.next += 1;
            }
            let (k, active) = b.active.as_mut().expect("a job is running");
            let (used, done) = active.advance(left);
            left -= used.min(left);
            if let Some(r) = done {
                b.results[*k] = Some(r);
                b.active = None;
            }
        }
        self.worked += budget - left;
        if self.bred(&b) && b.active.is_none() && b.next >= b.jobs.len() {
            self.close(b);
        } else {
            self.batch = Some(b);
        }
        self.progress()
    }

    /// Breeds the current generation's children within `budget` ticks ([`BREED_COST`] a child) and fights none of
    /// them: `true` once every child is bred, and [`Run::jobs`] then hands their jobs out without breeding more. A
    /// client that runs the jobs on threads breeds on its frame budget with it (GDR-0092: the native executor). A
    /// finished or stale run has nothing to breed.
    pub fn breed(&mut self, budget: u64) -> bool {
        if self.finished() || self.stale {
            return true;
        }
        let mut b = match self.batch.take() {
            Some(b) => b,
            None => self.open(),
        };
        let mut left = budget;
        while left > 0 && !self.bred(&b) {
            self.breed_child(&mut b);
            left = left.saturating_sub(BREED_COST);
        }
        self.worked += budget - left;
        let bred = self.bred(&b);
        self.batch = Some(b);
        bred
    }

    /// The current generation's jobs, breeding it first if needed (empty once the run is finished, or stale). Run
    /// them anywhere ([`Job::run`]) and hand the results back with [`Run::submit`], in the same order. Don't mix
    /// with [`Run::step`] within a generation.
    pub fn jobs(&mut self) -> &[Job] {
        if self.finished() || self.stale {
            return &[];
        }
        let mut b = match self.batch.take() {
            Some(b) => b,
            None => self.open(),
        };
        while !self.bred(&b) {
            self.breed_child(&mut b);
        }
        &self.batch.insert(b).jobs
    }

    /// Takes the results of [`Run::jobs`] (same order) and closes the generation. Panics if the count differs.
    pub fn submit(&mut self, results: Vec<JobResult>) {
        let mut b = self.batch.take().expect("submit after jobs()");
        assert!(self.bred(&b), "submit after jobs()");
        assert_eq!(results.len(), b.jobs.len(), "one result per job");
        b.results = results.into_iter().map(Some).collect();
        self.close(b);
    }

    /// The jobs of child `child` of generation `generation` (closed), rebuilt: the same jobs it ran.
    pub fn child_jobs(&self, generation: u32, child: usize) -> Option<Vec<Job>> {
        Some(self.tagged_jobs(generation, child)?.into_iter().map(|(_, j)| j).collect())
    }

    /// [`Run::child_jobs`] with each job's trial (its index in [`Run::trials`]).
    pub fn tagged_jobs(&self, generation: u32, child: usize) -> Option<Vec<(u32, Job)>> {
        let record = self.records.get(generation as usize)?;
        let genome = self.genome(record.children.get(child)?.genome)?;
        let bp = sim::design_report(&self.content, &genome.blueprint, &SimConfig::default()).ok()?.build_points;
        let mut jobs = match (&record.enemy, self.camp_of(genome.root)) {
            (Some(r), Camp::Enemy) => self.enemy_jobs_of(&genome, bp, &self.contestants(&r.rivals)?),
            (r, _) => {
                let hall = record
                    .opponents
                    .iter()
                    .map(|&id| self.genome(id).map(|g| genome_entry(&g, 1)))
                    .collect::<Option<Vec<_>>>()?;
                let foes = match r {
                    Some(r) => self.contestants(&r.opponents)?,
                    None => Vec::new(),
                };
                self.jobs_of(&genome, bp, &hall, &foes)
            }
        };
        for (_, j) in &mut jobs {
            j.set_candidate(child as u32);
        }
        Some(jobs)
    }

    /// Genomes as fight contestants (blueprint and behavior inlined, one ship, with their build points).
    fn contestants(&self, ids: &[u32]) -> Option<Vec<Contestant>> {
        ids.iter()
            .map(|&id| {
                let g = self.genome(id)?;
                let bp = sim::design_report(&self.content, &g.blueprint, &SimConfig::default()).ok()?.build_points;
                Some(Contestant { entry: genome_entry(&g, 1), build_points: bp })
            })
            .collect()
    }

    /// A population's starts as fight contestants (none for THE ENEMY of a run without one).
    fn start_contestants(&self, camp: Camp) -> Vec<Contestant> {
        let (starts, kit) = match (camp, &self.enemy) {
            (Camp::Yours, _) => (&self.starts, &self.kit),
            (Camp::Enemy, Some(e)) => (&e.starts, &e.kit),
            (Camp::Enemy, None) => return Vec::new(),
        };
        starts.iter().zip(&kit.roots).map(|(g, r)| Contestant { entry: genome_entry(g, 1), build_points: r.start_bp }).collect()
    }

    /// Child `child` of generation `generation`'s score taken apart by trial and goal, from its records (THE ENEMY's:
    /// its one trial, CO-EVOLUTION, scored by WIN).
    pub fn breakdown(&self, generation: u32, child: usize) -> Option<Breakdown> {
        let rec = self.records.get(generation as usize)?.children.get(child)?;
        let tags: Vec<u32> = self.tagged_jobs(generation, child)?.into_iter().map(|(t, _)| t).collect();
        let goals: Vec<GoalScores> = rec.jobs.iter().map(JobRecord::goal_scores).collect();
        Some(match self.camp(rec.genome)? {
            Camp::Yours => self.combine(&tags, &goals).1,
            Camp::Enemy => enemy_score(tags.first().copied().unwrap_or(0) as usize, &goals).1,
        })
    }

    /// A child's score from its jobs' goals and their trials (GDR-0092 "GOALS"): per trial the goals' means weighed
    /// by the run's goal weights, then the trials by theirs; trials without jobs and weights of 0 leave the sums. With
    /// the score, its breakdown.
    fn combine(&self, tags: &[u32], goals: &[GoalScores]) -> (f32, Breakdown) {
        let gw = self.config.goals.weights();
        let gsum: f64 = gw.iter().map(|w| *w as f64).sum();
        let (mut total, mut wsum) = (0.0f64, 0.0f64);
        let mut all = [0.0f64; 4];
        let mut trials = Vec::new();
        for (k, (spec, _)) in self.trials.iter().enumerate() {
            let mine: Vec<[f32; 4]> = goals.iter().zip(tags).filter(|(_, t)| **t == k as u32).map(|(g, _)| g.all()).collect();
            let n = mine.len();
            let mut mean = [0.0f64; 4];
            for (g, m) in mean.iter_mut().enumerate() {
                *m = mine.iter().map(|x| x[g] as f64).sum::<f64>() / n.max(1) as f64;
            }
            let score = (n > 0).then(|| {
                let mut s = 0.0f64;
                for (w, m) in gw.iter().zip(mean) {
                    if *w != 0.0 {
                        s += *w as f64 * m;
                    }
                }
                s / gsum
            });
            if let Some(s) = score
                && spec.weight != 0.0
            {
                total += spec.weight as f64 * s;
                wsum += spec.weight as f64;
                for (a, m) in all.iter_mut().zip(mean) {
                    *a += spec.weight as f64 * m;
                }
            }
            let [win, unhurt, quick, cheap] = mean.map(|m| m as f32);
            trials.push(TrialScore { trial: k, jobs: n, score: score.map(|s| s as f32), goals: GoalScores { win, unhurt, quick, cheap } });
        }
        let score = if wsum > 0.0 { (total / wsum) as f32 } else { 0.0 };
        let [win, unhurt, quick, cheap] = all.map(|a| if wsum > 0.0 { (a / wsum) as f32 } else { 0.0 });
        (score, Breakdown { trials, goals: GoalScores { win, unhurt, quick, cheap } })
    }

    /// Job `job` of child `child` of generation `generation`, rebuilt, with its record.
    pub fn job(&self, generation: u32, child: usize, job: usize) -> Option<(Job, JobRecord)> {
        let record = *self.records.get(generation as usize)?.children.get(child)?.jobs.get(job)?;
        Some((self.child_jobs(generation, child)?.into_iter().nth(job)?, record))
    }

    /// The fight code of job `job` of child `child` of generation `generation`, under the rules the run was fought
    /// under: it replays to the recorded end while they hold. `None` for a voyage job, or one that failed to build.
    pub fn fight_code(&self, generation: u32, child: usize, job: usize) -> Option<FightCode> {
        let (Job::Fight { spec, .. }, record) = self.job(generation, child, job)? else {
            return None;
        };
        self.code(generation, spec, record)
    }

    /// A fight of generation `generation` as a code, under the rules it was fought under.
    fn code(&self, generation: u32, spec: FightSpec, record: JobRecord) -> Option<FightCode> {
        match job_code(&self.content, self.rules_of(generation), Job::Fight { candidate: 0, side: 0, spec }, &record)? {
            Code::Fight(code) => Some(code),
            Code::Voyage(_) => None,
        }
    }

    /// The rules generation `generation` was fought under.
    fn rules_of(&self, generation: u32) -> u64 {
        self.records[generation as usize].rules.unwrap_or(self.rules)
    }

    /// Job `job` of child `child` of generation `generation` as a code to re-watch, under the rules it was fought
    /// under: a fight's code, or a voyage's (GDR-0092 step 9). `None` for a job that failed to build.
    pub fn replay_code(&self, generation: u32, child: usize, job: usize) -> Option<Code> {
        let (job, record) = self.job(generation, child, job)?;
        job_code(&self.content, self.rules_of(generation), job, &record)
    }

    /// Generation `generation`'s best child of yours (the first of equal scores), its index among the generation's
    /// children.
    pub fn best_child(&self, generation: u32) -> Option<usize> {
        self.best_of(generation, Camp::Yours)
    }

    /// Generation `generation`'s best child of a population (its champion), its index among the generation's
    /// children.
    pub fn best_of(&self, generation: u32, camp: Camp) -> Option<usize> {
        let children = &self.records.get(generation as usize)?.children;
        let mut best: Option<(usize, f32)> = None;
        for k in self.children_of(generation, camp) {
            if best.is_none_or(|(_, s)| children[k].score > s) {
                best = Some((k, children[k].score));
            }
        }
        best.map(|(k, _)| k)
    }

    /// Where a population's children of generation `generation` are among its `children` (yours first, then THE
    /// ENEMY's in an arms race).
    pub fn children_of(&self, generation: u32, camp: Camp) -> Range<usize> {
        let Some(r) = self.records.get(generation as usize) else { return 0..0 };
        let first = r.enemy.as_ref().map_or(r.children.len(), |e| (e.first as usize).min(r.children.len()));
        match camp {
            Camp::Yours => 0..first,
            Camp::Enemy => first..r.children.len(),
        }
    }

    /// A population's champions so far: each generation's best child of it (genome numbers, each once, in order).
    fn champions(&self, camp: Camp) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        for g in 0..self.records.len() as u32 {
            if let Some(k) = self.best_of(g, camp) {
                let id = self.records[g as usize].children[k].genome;
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }

    /// PAST CHAMPIONS (GDR-0092) of generation `generation`, if it checked.
    pub fn check(&self, generation: u32) -> Option<&Check> {
        self.records.get(generation as usize)?.enemy.as_ref()?.check.as_ref()
    }

    /// The PAST CHAMPIONS line: per checked generation, how many of the other side's champions each best beat.
    pub fn beaten(&self) -> Vec<Beaten> {
        let seeds = self.coevolution().map_or(1, |(_, t)| t.seeds.max(1)) as usize;
        let count = |jobs: &[JobRecord]| {
            let beat = jobs.chunks(seeds).filter(|c| c.iter().map(|j| j.score).sum::<f32>() / c.len() as f32 > 0.5).count();
            (beat as u32, jobs.len().div_ceil(seeds) as u32)
        };
        (0..self.records.len() as u32)
            .filter_map(|g| {
                let c = self.check(g)?;
                let (mine, theirs) = c.jobs.split_at((c.their_champions.len() * seeds).min(c.jobs.len()));
                Some(Beaten { generation: g, yours: count(mine), theirs: count(theirs) })
            })
            .collect()
    }

    /// The jobs of generation `generation`'s PAST CHAMPIONS check, rebuilt: the same jobs it ran.
    pub fn check_jobs(&self, generation: u32) -> Option<Vec<Job>> {
        self.jobs_of_check(self.check(generation)?)
    }

    /// A check's jobs: your best against each of their champions (every seed), then their best against each of yours.
    fn jobs_of_check(&self, c: &Check) -> Option<Vec<Job>> {
        let (_, kind) = &self.trials[self.coevolution()?.0];
        let one = |id: u32| self.contestants(&[id]).and_then(|v| v.into_iter().next());
        let theirs = self.contestants(&c.their_champions)?;
        let mut jobs = kind.jobs(&one(c.yours)?, &GenCtx { starts: &self.start_entries, hall: &[], enemies: &theirs, fresh: None });
        let ctx = GenCtx { starts: &self.start_entries, hall: &[], enemies: &[], fresh: None };
        jobs.extend(kind.enemy_jobs(&one(c.theirs)?, &self.contestants(&c.your_champions)?, &ctx));
        Some(jobs)
    }

    /// Job `job` of generation `generation`'s PAST CHAMPIONS check as a fight code (as [`Run::fight_code`]).
    pub fn check_fight_code(&self, generation: u32, job: usize) -> Option<FightCode> {
        let record = *self.check(generation)?.jobs.get(job)?;
        let Job::Fight { spec, .. } = self.check_jobs(generation)?.into_iter().nth(job)? else { return None };
        self.code(generation, spec, record)
    }

    /// The CO-EVOLUTION trial of an arms race: its index among the trials, and it.
    fn coevolution(&self) -> Option<(usize, &TrialSpec)> {
        self.trials.iter().enumerate().find(|(_, (t, _))| matches!(t.kind, Trial::CoEvolution)).map(|(k, (t, _))| (k, t))
    }

    /// Where genome `id` was last scored: its generation and its index among that generation's children (the
    /// record of its score, measures and fights): its own generation's, or a RE-SCORE's since.
    pub fn scored(&self, id: u32) -> Option<(u32, usize)> {
        scored_in(&self.history, &self.records, self.table.get(id as usize)?.generation, id)
    }

    /// TIDY at SAVE (GDR-0092): genome `id`'s behavior without the rules that never decided in any of its recorded
    /// fights its behavior flew (a stand-in's fights leave it alone; voyages keep no rules), its start's locked rules
    /// kept ([`Kit::tidy`]), with the dropped rules' indices and how many fights told. `None` when no fight told.
    pub fn tidied(&self, id: u32) -> Option<(String, Vec<usize>, usize)> {
        let genome = self.genome(id)?;
        let (mut decided, mut fights) = (0u64, 0usize);
        for (g, record) in self.records.iter().enumerate().skip(self.table.get(id as usize)?.generation as usize) {
            let Some(child) = record.children.iter().position(|c| c.genome == id) else { continue };
            let jobs = self.child_jobs(g as u32, child)?;
            for (job, rec) in jobs.iter().zip(&record.children[child].jobs) {
                if let (Job::Fight { spec, side, .. }, JobEnd::Fight { decided: d, .. }) = (job, &rec.end)
                    && spec.sides[(*side as usize).min(1)].entries.iter().all(|e| e.pilot.is_none())
                {
                    decided |= d;
                    fights += 1;
                }
            }
        }
        if fights == 0 {
            return None;
        }
        let (text, dropped) = self.kit_of(genome.root).tidy(genome.behavior_text(), decided, genome.root)?;
        Some((text, dropped, fights))
    }

    /// EXPORT TO CONTENT (GDR-0092 "Saving evolved enemies"): THE ENEMY's genome `id` in the content ship format
    /// (`data::export_text`) under the id `evolved_<faction>_<n>`, tagged `evolved` and `enemy`, with its `origin` (in
    /// run `run`, from its start's content ship first), naming its behavior of the same id, which follows as a content
    /// behavior file in a comment block. It goes into a pack's `ships/` (the behavior into `behaviors/`) by hand:
    /// curated, never into voyages by itself (GDR-0032). `None` for a genome of yours, or none.
    pub fn export_enemy(&self, id: u32, run: &str, n: u32) -> Option<String> {
        if self.camp(id)? != Camp::Enemy {
            return None;
        }
        let g = self.genome(id)?;
        let key = format!("evolved_{}_{n}", g.blueprint.faction);
        let (generation, score) = self.scored(id).map_or((0, 0.0), |(gn, k)| (gn, self.records[gn as usize].children[k].score));
        let own = self.start_config(g.root);
        let ids = |s: &Start| match s {
            Start::Design { design: DesignRef::Ship(id), .. } => id.clone(),
            Start::Design { design: DesignRef::Blueprint(text), .. } => data::parse_blueprint(text).map_or_else(|_| String::new(), |b| b.id),
            Start::Random { faction } => faction.clone(),
        };
        let mut from: Vec<String> = own.into_iter().map(ids).collect();
        from.extend(self.config.enemy.iter().flat_map(|e| &e.starts).filter(|s| Some(*s) != own).map(ids));
        let (target, rules) = Program::parse(&g.behavior).ok()?.to_parts();
        let behavior = data::Behavior {
            id: key.clone(),
            name: g.blueprint.name.clone(),
            summary: format!("Evolved in an arms race of the evolution lab: generation {generation}, score {score:.2}."),
            description: None,
            lore: None,
            tags: g.blueprint.tags.clone(),
            target,
            rules,
            source: Default::default(),
        };
        let ship = Blueprint {
            id: key.clone(),
            behavior: Some(key),
            origin: Some(data::Origin { run: run.to_string(), genome: id, generation, from, score }),
            ..g.blueprint
        };
        Some(data::export_text(&ship, Some(&behavior)))
    }

    /// VALIDATE (GDR-0092): `genomes` (the archive's best [`VALIDATE_BEST`], or the picked one) fight the run's trials
    /// on `seeds` new seeds each (the ones after each trial's own), against the starts, today's hall of fame and, in an
    /// arms race, THE ENEMY's best `elites + hall` today (no draws), and their recorded fights again under today's
    /// rules. `None` for a genome never scored, or one of THE ENEMY's (its designs are scored only against yours).
    pub fn validation(&self, genomes: &[u32], seeds: u32) -> Option<Validation> {
        let hall: Vec<Entry> = self.hall_of_fame().iter().map(|e| genome_entry(&e.genome, 1)).collect();
        let foes = match (&self.enemy, &self.config.enemy) {
            (Some(e), Some(pop)) if !e.archive.cells().is_empty() => {
                self.contestants(&e.archive.top((pop.elites + pop.hall) as usize).iter().map(|e| e.id).collect::<Vec<_>>())?
            }
            _ => self.start_contestants(Camp::Enemy),
        };
        let mut v = Validation { seeds, genomes: Vec::new(), owner: Vec::new(), batch: JobBatch::new(&self.content, Vec::new()) };
        let mut jobs = Vec::new();
        for &id in genomes {
            if self.camp(id)? == Camp::Enemy {
                return None;
            }
            let g = self.genome(id)?;
            let (generation, child) = self.scored(id)?;
            let stats = self.kit.assess(&self.content, &g).ok()?;
            let root = &self.kit.roots[g.root as usize];
            let cheap = cheapness(stats.build_points, root.build_points.unwrap_or(root.start_bp));
            let me = Contestant { entry: genome_entry(&g, 1), build_points: stats.build_points };
            let ctx = GenCtx { starts: &self.start_entries, hall: &hall, enemies: &foes, fresh: Some(seeds) };
            let k = v.genomes.len();
            let mut tags = Vec::new();
            for (t, (_, kind)) in self.trials.iter().enumerate().filter(|(_, (spec, _))| spec.weight != 0.0) {
                for j in kind.jobs(&me, &ctx) {
                    tags.push(t as u32);
                    v.owner.push((k, false));
                    jobs.push(j);
                }
            }
            let recorded = self.records[generation as usize].children[child].jobs.clone();
            for j in self.child_jobs(generation, child)? {
                v.owner.push((k, true));
                jobs.push(j);
            }
            v.genomes.push((id, tags, recorded, cheap));
        }
        v.batch = JobBatch::new(&self.content, jobs);
        Some(v)
    }

    /// Takes a finished validation's results into the run (kept in its file; a genome validated again replaces its
    /// earlier result). Panics if it isn't finished.
    pub fn validated_by(&mut self, v: Validation) {
        let results = v.batch.results();
        for (k, (id, tags, recorded, cheap)) in v.genomes.iter().enumerate() {
            let mine = || v.owner.iter().zip(&results).filter(move |((o, _), _)| *o == k);
            let goals: Vec<GoalScores> =
                mine().filter(|((_, rec), _)| !rec).map(|(_, r)| GoalScores { cheap: *cheap, ..r.goals }).collect();
            let (score, b) = self.combine(tags, &goals);
            let again: Vec<&JobResult> = mine().filter(|((_, rec), _)| *rec).map(|(_, r)| r).collect();
            let same = again.iter().zip(recorded).filter(|(r, rec)| r.end == rec.end && r.ticks == rec.ticks).count();
            let result = Validated {
                genome: *id,
                seeds: v.seeds,
                score,
                trials: b.trials.iter().map(|t| t.score).collect(),
                as_recorded: (same as u32, recorded.len() as u32),
            };
            self.validated.retain(|x| x.genome != *id);
            self.validated.push(result);
        }
    }

    /// VALIDATE's result for genome `id`, if it was validated.
    pub fn validated(&self, id: u32) -> Option<&Validated> {
        self.validated.iter().find(|v| v.genome == id)
    }

    /// RE-SCORE (GDR-0092): a stale run as a new run under today's rules: its history and fights kept (their
    /// generations keep the rules they were fought under), and its next generation fights the archive's elites again
    /// and rebuilds the archive from their new scores; then it goes on breeding where it was. Refused for a run whose
    /// rules are today's.
    pub fn rescored(content: &Content, save: &RunSave) -> Result<Run, RunError> {
        let mut run = Run::restore(content, save)?;
        if !run.stale {
            return Err(RunError::Save("its rules are today's: there is nothing to re-score".into()));
        }
        let old = run.rules;
        for r in &mut run.records {
            r.rules.get_or_insert(old);
        }
        run.rules = rules_fingerprint(content);
        run.stale = false;
        run.rescore = true;
        run.generations += 1;
        // What was validated was validated under the old rules.
        run.validated.clear();
        Ok(run)
    }

    /// The hall of fame this generation fights: the archive's best, as many as the trials want at most.
    fn hall_of_fame(&self) -> Vec<&Elite> {
        let n = self.trials.iter().map(|(_, t)| t.hall_of_fame()).max().unwrap_or(0);
        self.archive.top(n as usize)
    }

    /// A genome's jobs, each with its trial: every trial in order, each its opponents (or voyages) and seeds, against
    /// the run's starts, the generation's hall of fame `hall` and THE ENEMY's frozen set `foes`; `build_points` are the
    /// genome's.
    fn jobs_of(&self, g: &Genome, build_points: f32, hall: &[Entry], foes: &[Contestant]) -> Vec<(u32, Job)> {
        let me = Contestant { entry: genome_entry(g, 1), build_points };
        let ctx = GenCtx { starts: &self.start_entries, hall, enemies: foes, fresh: None };
        let mut jobs = Vec::new();
        // A trial of weight 0 counts nothing, so it is not fought.
        for (k, (_, t)) in self.trials.iter().enumerate().filter(|(_, (spec, _))| spec.weight != 0.0) {
            jobs.extend(t.jobs(&me, &ctx).into_iter().map(|j| (k as u32, j)));
        }
        jobs
    }

    /// An enemy's jobs (THE ENEMY's genome on side 1) against your frozen set `rivals`, tagged with the CO-EVOLUTION
    /// trial; `build_points` are the genome's.
    fn enemy_jobs_of(&self, g: &Genome, build_points: f32, rivals: &[Contestant]) -> Vec<(u32, Job)> {
        let Some((k, _)) = self.coevolution() else { return Vec::new() };
        let me = Contestant { entry: genome_entry(g, 1), build_points };
        let ctx = GenCtx { starts: &self.start_entries, hall: &[], enemies: &[], fresh: None };
        self.trials[k].1.enemy_jobs(&me, rivals, &ctx).into_iter().map(|j| (k as u32, j)).collect()
    }

    /// One child of a population (yours of the stage's genes `genes`, or THE ENEMY's of its own on its own stream), see
    /// [`breed`].
    fn child(&mut self, camp: Camp, genes: &Genes, made: usize, rejected: &mut u32) -> Option<(Genome, Stats, Bred)> {
        let generation = self.history.len() as u32;
        let (crossover, mutations) = (self.config.crossover, self.config.mutations);
        let content = &self.content;
        match (camp, &mut self.enemy, &self.config.enemy) {
            (Camp::Enemy, Some(e), Some(pop)) => {
                let b = Breeder { content, kit: &e.kit, starts: &e.starts, archive: &e.archive, crossover, mutations, generation };
                breed(&b, &mut e.rng, &Genes::MayChange(pop.evolve.clone()), made, rejected)
            }
            _ => {
                let b = Breeder { content, kit: &self.kit, starts: &self.starts, archive: &self.archive, crossover, mutations, generation };
                breed(&b, &mut self.rng, genes, made, rejected)
            }
        }
    }

    /// Opens the next generation: its stage, the random stream's state, the hall of fame it fights, an arms race's
    /// frozen sets, and in generation 0 the starts as its first children (THE ENEMY's come after your children).
    fn open(&mut self) -> Batch {
        let g = self.generation();
        let stage = self.stage_of(g);
        let top = self.hall_of_fame();
        let opponents = top.iter().map(|e| e.id).collect();
        let hall = top.iter().map(|e| genome_entry(&e.genome, 1)).collect();
        let mut b = Batch {
            stage,
            genes: self.stages[stage].0.clone(),
            from: self.rng.clone(),
            first: self.table.len(),
            opponents,
            hall,
            candidates: Vec::new(),
            failed: 0,
            rejected: 0,
            ranges: Vec::new(),
            jobs: Vec::new(),
            trial_of: Vec::new(),
            results: Vec::new(),
            next: 0,
            active: None,
            rescore: self.rescore,
            rivalry: self.freeze(),
        };
        if self.rescore {
            // Every elite again in cell order (yours, then THE ENEMY's); one that no longer builds or passes (a part
            // the content lost) is left out, counted as refused.
            let elites: Vec<Elite> = self.archive.cells().values().cloned().collect();
            for e in elites {
                match self.kit.assess(&self.content, &e.genome) {
                    Ok(stats) => self.add_jobs(&mut b, Candidate { genome: e.genome, id: e.id, stats }),
                    Err(_) => b.rejected += 1,
                }
            }
            let first = b.candidates.len();
            let theirs: Vec<Elite> = self.enemy.as_ref().map_or_else(Vec::new, |e| e.archive.cells().values().cloned().collect());
            let mut rejected = 0;
            for e in theirs {
                match self.kit_of(e.genome.root).assess(&self.content, &e.genome) {
                    Ok(stats) => self.add_jobs(&mut b, Candidate { genome: e.genome, id: e.id, stats }),
                    Err(_) => rejected += 1,
                }
            }
            if let Some(r) = &mut b.rivalry {
                r.first = Some(first);
                r.rejected = rejected;
            }
            return b;
        }
        if g == 0 {
            for (k, s) in self.starts.iter().enumerate() {
                let stats = self
                    .kit
                    .assess(&self.content, s)
                    .expect("the starts passed the pre-filter in Run::new");
                let c = Candidate {
                    genome: s.clone(),
                    id: k as u32,
                    stats,
                };
                self.add_jobs(&mut b, c);
            }
        }
        b
    }

    /// An arms race's frozen sets for the generation opening now (GDR-0092 "How they are paired"): for your children
    /// THE ENEMY's best `elites` and `hall` of its past champions, for its children your best `elites`, `hall` of
    /// yours and (`anchor`) your starts; a side's starts while its archive is empty (generation 0). The champions are
    /// drawn uniformly over the whole history on the enemy's stream (its first, then yours), each once and none
    /// that is among the best already.
    fn freeze(&mut self) -> Option<Frozen> {
        let pop = self.config.enemy.as_ref()?;
        let (elites, hall, anchor) = (pop.elites as usize, pop.hall as usize, pop.anchor);
        let (theirs, yours) = (self.champions(Camp::Enemy), self.champions(Camp::Yours));
        let n = self.starts.len() as u32;
        let e = self.enemy.as_mut()?;
        let from = e.rng.clone();
        let draw = |archive: &Archive, champions: Vec<u32>, rng: &mut Pcg32| -> Vec<u32> {
            let mut ids: Vec<u32> = archive.top(elites).iter().map(|e| e.id).collect();
            let mut pool: Vec<u32> = champions.into_iter().filter(|c| !ids.contains(c)).collect();
            for k in 0..hall.min(pool.len()) {
                let j = k + rng.below((pool.len() - k) as u32) as usize;
                pool.swap(k, j);
                ids.push(pool[k]);
            }
            ids
        };
        let opponents = match e.archive.cells().is_empty() {
            true => (n..n + e.starts.len() as u32).collect(),
            false => draw(&e.archive, theirs, &mut e.rng),
        };
        let mut rivals = match self.archive.cells().is_empty() {
            true => (0..n).collect(),
            false => draw(&self.archive, yours, &mut e.rng),
        };
        if anchor {
            for k in 0..n {
                if !rivals.contains(&k) {
                    rivals.push(k);
                }
            }
        }
        let foes = self.contestants(&opponents).expect("the enemy's genomes rebuild");
        let friends = self.contestants(&rivals).expect("your genomes rebuild");
        Some(Frozen { from, opponents, foes, rivals, friends, first: None, failed: 0, rejected: 0, check: None })
    }

    /// Your children of the generation are bred (or given up).
    fn yours_bred(&self, b: &Batch) -> bool {
        b.rivalry.as_ref().and_then(|r| r.first).unwrap_or(b.candidates.len()) + b.failed >= self.config.population as usize
    }

    /// Every child of the generation is bred (or given up), THE ENEMY's too; a RE-SCORE breeds none.
    fn bred(&self, b: &Batch) -> bool {
        let theirs = |r: &Frozen| {
            let pop = self.config.enemy.as_ref().map_or(0, |e| e.population as usize);
            r.first.is_some_and(|first| b.candidates.len() - first + r.failed >= pop)
        };
        b.rescore || (self.yours_bred(b) && b.rivalry.as_ref().is_none_or(theirs))
    }

    /// Breeds the generation's next child: into the table and the batch with its jobs, or given up. Yours first; then
    /// in an arms race THE ENEMY's (its starts first in generation 0), and once the last is bred PAST CHAMPIONS' jobs.
    fn breed_child(&mut self, b: &mut Batch) {
        let camp = if self.yours_bred(b) { Camp::Enemy } else { Camp::Yours };
        if camp == Camp::Enemy {
            let r = b.rivalry.as_mut().expect("only an arms race breeds after your children");
            if r.first.is_none() {
                r.first = Some(b.candidates.len());
                if self.history.is_empty() {
                    let starts = self.enemy.as_ref().map_or_else(Vec::new, |e| e.starts.clone());
                    for s in starts {
                        let stats = self.kit_of(s.root).assess(&self.content, &s).expect("the starts passed the pre-filter in Run::new");
                        self.add_jobs(b, Candidate { id: s.root, genome: s, stats });
                    }
                }
                if self.bred(b) {
                    self.add_check(b);
                }
                return;
            }
        }
        let made = match (camp, &b.rivalry) {
            (Camp::Enemy, Some(r)) => b.candidates.len() - r.first.unwrap_or(0) + r.failed,
            _ => b.candidates.len() + b.failed,
        };
        let mut rejected = 0;
        let child = self.child(camp, &b.genes, made, &mut rejected);
        let gave_up = child.is_none() as usize;
        match (camp, &mut b.rivalry) {
            (Camp::Enemy, Some(r)) => {
                r.rejected += rejected;
                r.failed += gave_up;
            }
            _ => {
                b.rejected += rejected;
                // A child whose every try was refused stays out: the generation is that much smaller.
                b.failed += gave_up;
            }
        }
        if let Some((genome, stats, bred)) = child {
            let id = self.table.len() as u32;
            self.table.push(bred);
            self.add_jobs(b, Candidate { genome, id, stats });
        }
        if camp == Camp::Enemy && self.bred(b) {
            self.add_check(b);
        }
    }

    fn add_jobs(&self, b: &mut Batch, c: Candidate) {
        let k = b.candidates.len() as u32;
        let start = b.jobs.len();
        let jobs = match (&b.rivalry, self.camp_of(c.genome.root)) {
            (Some(r), Camp::Enemy) => self.enemy_jobs_of(&c.genome, c.stats.build_points, &r.friends),
            (r, _) => self.jobs_of(&c.genome, c.stats.build_points, &b.hall, r.as_ref().map_or(&[], |r| &r.foes)),
        };
        for (t, mut j) in jobs {
            j.set_candidate(k);
            b.jobs.push(j);
            b.trial_of.push(t);
            b.results.push(None);
        }
        b.ranges.push(start..b.jobs.len());
        b.candidates.push(c);
    }

    /// PAST CHAMPIONS (GDR-0092): every `check_every`th generation (not the first, nor a RE-SCORE) both archives' bests
    /// fight the other side's champions of every `champions_every`th generation so far, after the children's jobs.
    fn add_check(&self, b: &mut Batch) {
        let g = self.generation();
        let (Some(pop), Some(e)) = (&self.config.enemy, &self.enemy) else { return };
        if b.rescore || g == 0 || pop.check_every == 0 || !g.is_multiple_of(pop.check_every) {
            return;
        }
        let (Some(yours), Some(theirs), Some((trial, _))) = (self.archive.best(), e.archive.best(), self.coevolution()) else { return };
        let every = |camp: Camp| -> Vec<u32> {
            (0..g).step_by(pop.champions_every as usize).filter_map(|k| self.best_of(k, camp).map(|c| self.records[k as usize].children[c].genome)).collect()
        };
        let check = Check { yours: yours.id, theirs: theirs.id, their_champions: every(Camp::Enemy), your_champions: every(Camp::Yours), jobs: Vec::new() };
        let Some(jobs) = self.jobs_of_check(&check) else { return };
        let start = b.jobs.len();
        let candidate = b.candidates.len() as u32;
        for mut j in jobs {
            j.set_candidate(candidate);
            b.jobs.push(j);
            b.trial_of.push(trial as u32);
            b.results.push(None);
        }
        let end = b.jobs.len();
        if let Some(r) = &mut b.rivalry {
            r.check = Some((check, start..end));
        }
    }

    /// Scores the generation's children from their results, offers each to its population's archive in order,
    /// records the generation.
    fn close(&mut self, b: Batch) {
        let generation = self.generation();
        if b.rescore {
            self.archive = Archive::new(self.config.archive.clone());
            if let (Some(e), Some(pop)) = (&mut self.enemy, &self.config.enemy) {
                e.archive = Archive::new(pop.archive.clone());
            }
            self.rescore = false;
        }
        let first = b.rivalry.as_ref().and_then(|r| r.first).unwrap_or(b.candidates.len());
        let trial = self.coevolution().map_or(0, |(k, _)| k);
        // Per population: the best and the sum of the scores, the children that took a cell.
        let (mut best, mut sum, mut improved) = ([0.0f32; 2], [0.0f64; 2], [0u32; 2]);
        let mut ticks = 0;
        let mut children = Vec::with_capacity(b.candidates.len());
        for (k, (c, range)) in b.candidates.into_iter().zip(&b.ranges).enumerate() {
            let side = (k >= first) as usize;
            // CHEAP is the design's: its build points against its start's limit (else the start's own).
            let root = self.kit_of(c.genome.root).root(c.genome.root).expect("a genome's start");
            let cheap = cheapness(c.stats.build_points, root.build_points.unwrap_or(root.start_bp));
            let rs: Vec<JobResult> = b.results[range.clone()]
                .iter()
                .map(|r| {
                    let mut r = r.expect("every job done");
                    r.goals.cheap = cheap;
                    r
                })
                .collect();
            let goals: Vec<GoalScores> = rs.iter().map(|r| r.goals).collect();
            let score = match side {
                0 => self.combine(&b.trial_of[range.clone()], &goals).0,
                _ => enemy_score(trial, &goals).0,
            };
            let samples: u32 = rs.iter().map(|r| r.engage_samples).sum();
            let engage = if samples > 0 {
                rs.iter().map(|r| r.engage_sum as f64).sum::<f64>() as f32 / samples as f32
            } else {
                0.0
            };
            ticks += rs.iter().map(|r| r.ticks).sum::<u64>();
            best[side] = best[side].max(score);
            sum[side] += score as f64;
            let archive = match (side, &mut self.enemy) {
                (1, Some(e)) => &mut e.archive,
                _ => &mut self.archive,
            };
            let measures = measures(archive.shape(), &c.stats, engage);
            let elite = Elite {
                genome: c.genome,
                id: c.id,
                score,
                measures: measures.clone(),
                // Bred then (a re-scored elite keeps its own generation).
                generation: self.table[c.id as usize].generation,
            };
            let took = archive.offer(elite);
            improved[side] += took as u32;
            children.push(ChildRecord {
                genome: c.id,
                score,
                measures,
                took,
                jobs: rs.iter().map(|r| JobRecord::of(r, true)).collect(),
            });
        }
        let check = b.rivalry.as_ref().and_then(|r| r.check.clone()).map(|(mut check, range)| {
            let rs: Vec<JobResult> = b.results[range].iter().map(|r| r.expect("every job done")).collect();
            ticks += rs.iter().map(|r| r.ticks).sum::<u64>();
            check.jobs = rs.iter().map(|r| JobRecord::of(r, true)).collect();
            check
        });
        let evaluated = [first as u32, (b.ranges.len() - first) as u32];
        let mean = |side: usize| if evaluated[side] > 0 { (sum[side] / evaluated[side] as f64) as f32 } else { 0.0 };
        let enemy = match (&b.rivalry, &self.enemy) {
            (Some(r), Some(e)) => Some(EnemyStats {
                evaluated: evaluated[1],
                rejected: r.rejected,
                best: best[1],
                mean: mean(1),
                archive_best: e.archive.best().map_or(0.0, |e| e.score),
                filled: e.archive.cells().len() as u32,
                improved: improved[1],
            }),
            _ => None,
        };
        self.history.push(GenerationStats {
            generation,
            stage: b.stage,
            genes: b.genes.clone(),
            evaluated: evaluated[0],
            rejected: b.rejected,
            best: best[0],
            mean: mean(0),
            archive_best: self.archive.best().map_or(0.0, |e| e.score),
            filled: self.archive.cells().len() as u32,
            improved: improved[0],
            ticks,
            rescore: b.rescore,
            enemy,
        });
        self.records.push(GenerationRecord {
            opponents: b.opponents,
            children,
            rules: None,
            enemy: b.rivalry.map(|r| Rivalry { opponents: r.opponents, rivals: r.rivals, first: first as u32, check }),
        });
    }
}

/// A design's value of each of an archive's measures.
fn measures(shape: &ArchiveShape, s: &Stats, engage: f32) -> Vec<f32> {
    shape
        .axes
        .iter()
        .map(|a| match a.measure {
            Measure::TopSpeed => s.top_speed,
            Measure::Acceleration => s.acceleration,
            Measure::WeaponReach => s.reach,
            Measure::BuildPoints => s.build_points,
            Measure::Mass => s.mass,
            Measure::Firepower => s.firepower,
            Measure::HitPoints => s.hit_points,
            Measure::Parts => s.parts as f32,
            Measure::Rules => s.rules as f32,
            Measure::EngageDistance => engage,
        })
        .collect()
}

impl Validation {
    /// The genomes it validates.
    pub fn genomes(&self) -> Vec<u32> {
        self.genomes.iter().map(|g| g.0).collect()
    }
}
