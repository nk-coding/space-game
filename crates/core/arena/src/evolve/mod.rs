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
//! ```
//! let content = data::base_content();
//! let config = arena::RunConfig::from_ron(r#"(
//!     seed: 1,
//!     evolve: Behavior,
//!     generations: 1,
//!     population: 2,
//!     fitness: ArenaPool(opponents: [(design: Ship("automaton_raider"))], seeds: 1, time_limit: 5),
//!     start: Design(design: Ship("rim_skiff")),
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
mod lineage;
mod store;

use std::fmt;
use std::ops::Range;

use data::behavior::Program;
use data::{BehaviorEditor, Blueprint, Content};
use serde::{Deserialize, Serialize};
use sim::{Pcg32, SimConfig};

use crate::code::FightCode;
use crate::fight::{Fight, FightSetup, resolve_design};
use crate::rules::rules_fingerprint;
use crate::spec::{BehaviorRef, DesignRef, Entry, FightSpec, Formation};
use crate::voyage::{VoyageSpec, start_voyage};

pub use archive::{Archive, ArchiveShape, Axis, Elite, MAX_CELLS, Measure};
pub use config::{
    Bound, DEFAULT_CROSSOVER, DEFAULT_MUTATIONS, DEFAULT_POPULATION, DEFAULT_SEEDS, Evolve, Fitness,
    Genes, Limits, Locks, MAX_GENERATIONS, MAX_MUTATIONS, MAX_POPULATION, MAX_SEEDS, MAX_STARTS,
    RunConfig, Stage, Start, VoyagePilot,
};
pub use eval::{Job, JobEnd, JobRecord, JobResult, SAMPLE_TICKS};
pub use genome::Genome;
pub use lineage::Bred;
pub use store::{MAX_SAVE_PACKED, MAX_SAVE_PAYLOAD, RunSave, SAVE_FORMAT, SAVE_TAG};

use eval::Active;
use genome::{Kit, Locked, Root, Stats};

/// The PCG sequence of a run's random stream (`"evolve"` in ASCII), so it never repeats a sim stream.
const RUN_STREAM: u64 = 0x6576_6f6c_7665;
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
            RunError::Fitness(e) => write!(f, "the fitness is not usable: {e}"),
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

/// A run's results.
#[derive(Clone, Debug, PartialEq)]
pub struct Results {
    pub history: Vec<GenerationStats>,
    /// Every elite of the archive, best first (equal scores in cell order).
    pub elites: Vec<Elite>,
}

/// One child of the current generation.
struct Candidate {
    genome: Genome,
    id: u32,
    stats: Stats,
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
    extra: Vec<Entry>,
    candidates: Vec<Candidate>,
    /// Children given up (the pre-filter refused every try).
    failed: usize,
    rejected: u32,
    /// Each candidate's jobs.
    ranges: Vec<Range<usize>>,
    jobs: Vec<Job>,
    results: Vec<Option<JobResult>>,
    /// The next job to start.
    next: usize,
    active: Option<(usize, Active)>,
}

/// An evolution run (see the module docs).
pub struct Run {
    content: Content,
    config: RunConfig,
    kit: Kit,
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
fn entry_of(g: &Genome, count: u32) -> Entry {
    Entry {
        design: DesignRef::Blueprint(g.blueprint_text()),
        behavior: Some(BehaviorRef::Text(g.behavior.clone())),
        count,
        formation: Formation::Line,
    }
}

/// A start resolved against the content: its blueprint as written (none for a random one), its header and its
/// behavior text.
struct StartText {
    design: Option<Blueprint>,
    header: Blueprint,
    behavior: String,
    locks: Locks,
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

impl Run {
    /// Checks the configuration, resolves the starts (or builds random ones), opens each centred in its frame once
    /// and checks it against the limits, its locks and the pre-filter, and builds one job of each opponent or voyage
    /// with each start to check the fitness. Takes the rules' fingerprint (`rules_fingerprint`, a few ms).
    pub fn new(content: &Content, config: RunConfig) -> Result<Run, RunError> {
        config.check()?;
        let stages = config.schedule()?;
        let generations = stages.iter().map(|s| s.1).sum();
        let fights = matches!(config.fitness, Fitness::ArenaPool { .. });
        if !fights && config.archive.axes.iter().any(|a| a.measure.needs_fights()) {
            return Err(RunError::Config(
                "`EngageDistance` is measured in the arena pool's fights only".into(),
            ));
        }
        let mut rng = Pcg32::new(sim::rng::splitmix64(config.seed ^ RUN_STREAM), RUN_STREAM);
        let texts = config
            .all_starts()
            .map(|s| resolve_start(content, s))
            .collect::<Result<Vec<_>, _>>()
            .map_err(RunError::Start)?;
        // The run's frame: the configured one, else the smallest content frame every start fits.
        let frame = match &config.limits.frame {
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
        let mut kit = Kit::new(content, &config.limits);
        let mut headers = Vec::new();
        let mut starts = Vec::new();
        for (k, t) in texts.into_iter().enumerate() {
            let root = k as u32;
            let at = |e: String| RunError::Start(if k == 0 { e } else { format!("start {}: {e}", k + 1) });
            // The header every child carries: the start's, renamed; the behavior goes beside it as text.
            let header = Blueprint {
                id: "evolved".into(),
                name: format!("{} (evolved)", t.header.name),
                summary: format!("Evolved from {} in the evolution lab.", t.header.name),
                description: None,
                lore: None,
                tags: vec!["evolved".into()],
                behavior: None,
                source: Default::default(),
                ..t.header
            };
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
                        min_hold: None,
                        locks,
                    });
                    Genome::of(&dock, t.behavior, root)
                }
                None => {
                    kit.roots.push(Root {
                        frame: kit.frame_for(frame, (0, 0)),
                        build_points: None,
                        min_hold: None,
                        locks: Locked::default(),
                    });
                    kit.random_design(content, &header, &t.behavior, root, &mut rng)
                        .ok_or_else(|| at("no random design with the parts allowed flies".into()))?
                }
            };
            // The start's own cost and hold are its genomes' limits by default.
            let report = sim::design_report(content, &g.blueprint, &SimConfig::default()).map_err(at)?;
            let r = &mut kit.roots[k];
            r.build_points = config.limits.build_points.of(report.build_points);
            r.min_hold = config.limits.min_hold.of(content.ship_hold(&g.blueprint) as f32);
            kit.assess(content, &g).map_err(at)?;
            headers.push(header);
            starts.push(g);
        }
        let table = starts
            .iter()
            .map(|g| Bred {
                generation: 0,
                root: g.root,
                parent: None,
                partner: None,
                removed: Vec::new(),
                added: g.frame_parts().into_iter().enumerate().map(|(k, p)| (k as u32, p)).collect(),
                behavior: Some(g.behavior.clone()),
                size: g.blueprint.size,
            })
            .collect();
        let run = Run {
            content: content.clone(),
            archive: Archive::new(config.archive.clone()),
            config,
            kit,
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
        };
        // One job of every opponent or voyage with every start: the references resolve.
        for g in &run.starts {
            for job in run.jobs_of(g, &[]) {
                let ok = match &job {
                    Job::Fight { spec, .. } => Fight::new(content, spec).map(|_| ()),
                    Job::Voyage { spec, .. } => start_voyage(content, spec).map(|_| ()),
                };
                ok.map_err(|e| RunError::Fitness(e.to_string()))?;
            }
        }
        Ok(run)
    }

    pub fn config(&self) -> &RunConfig {
        &self.config
    }

    /// The starts (generation 0's first children, genomes 0 to n − 1), each opened centred in its frame.
    pub fn starts(&self) -> &[Genome] {
        &self.starts
    }

    /// The frame genomes of start `root` are edited in (`Genome::placed`).
    pub fn frame(&self, root: u32) -> Option<(u32, u32)> {
        self.kit.roots.get(root as usize).map(|r| r.frame)
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

    /// The stage of generation `g` (the last stage past the end).
    fn stage_of(&self, g: u32) -> usize {
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
        if self.bred(&b) && b.active.is_none() && b.next >= b.jobs.len() {
            self.close(b);
        } else {
            self.batch = Some(b);
        }
        self.progress()
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
        let record = self.records.get(generation as usize)?;
        let genome = self.genome(record.children.get(child)?.genome)?;
        let extra = record
            .opponents
            .iter()
            .map(|&id| self.genome(id).map(|g| entry_of(&g, 1)))
            .collect::<Option<Vec<_>>>()?;
        let mut jobs = self.jobs_of(&genome, &extra);
        for j in &mut jobs {
            j.set_candidate(child as u32);
        }
        Some(jobs)
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
        let build_points = FightSetup::build(&self.content, &spec).ok()?.referee.build_points();
        let result = record.fight_result(build_points)?;
        Some(FightCode::new(&self.content, self.rules, spec, result))
    }

    /// Generation `generation`'s best child (the first of equal scores), its index among the generation's children.
    pub fn best_child(&self, generation: u32) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (k, c) in self.records.get(generation as usize)?.children.iter().enumerate() {
            if best.is_none_or(|(_, s)| c.score > s) {
                best = Some((k, c.score));
            }
        }
        best.map(|(k, _)| k)
    }

    /// Where genome `id` was scored: its generation and its index among that generation's children (the record
    /// of its score, measures and fights).
    pub fn scored(&self, id: u32) -> Option<(u32, usize)> {
        let generation = self.table.get(id as usize)?.generation;
        let k = self
            .records
            .get(generation as usize)?
            .children
            .iter()
            .position(|c| c.genome == id)?;
        Some((generation, k))
    }

    /// The pool of opponents for this generation besides the configured ones: the best `hall_of_fame` elites.
    fn hall_of_fame(&self) -> Vec<&Elite> {
        let Fitness::ArenaPool { hall_of_fame, .. } = &self.config.fitness else {
            return Vec::new();
        };
        self.archive.top(*hall_of_fame as usize)
    }

    /// A genome's jobs: every opponent (the configured ones and `extra`) or voyage, every seed.
    fn jobs_of(&self, g: &Genome, extra: &[Entry]) -> Vec<Job> {
        let design = DesignRef::Blueprint(g.blueprint_text());
        let behavior = BehaviorRef::Text(g.behavior.clone());
        match &self.config.fitness {
            Fitness::ArenaPool {
                opponents,
                seeds,
                count,
                start_distance,
                time_limit,
                ..
            } => {
                let me = Entry {
                    design,
                    behavior: Some(behavior),
                    count: *count,
                    formation: Formation::Line,
                };
                let mut jobs = Vec::new();
                for o in opponents.iter().chain(extra) {
                    for seed in 1..=*seeds as u64 {
                        let mut spec = FightSpec::new(seed, vec![me.clone()], vec![o.clone()]);
                        spec.start_distance = *start_distance;
                        spec.time_limit = *time_limit;
                        jobs.push(Job::Fight { candidate: 0, spec });
                    }
                }
                jobs
            }
            Fitness::Voyages {
                ids,
                seeds,
                minutes,
                pilot,
            } => {
                let behavior = match pilot {
                    VoyagePilot::Behavior => Some(behavior),
                    VoyagePilot::Closing => None,
                };
                let mut jobs = Vec::new();
                for v in ids {
                    for seed in 1..=*seeds as u64 {
                        let spec = VoyageSpec {
                            voyage: v.clone(),
                            seed,
                            design: design.clone(),
                            behavior: behavior.clone(),
                            minutes: *minutes,
                        };
                        jobs.push(Job::Voyage { candidate: 0, spec });
                    }
                }
                jobs
            }
        }
    }

    /// One child: crossover (sometimes) and mutations of a random elite (generation 0: of a start, shared out
    /// evenly), under `genes`; its behavior cut to its design's rule slots. `None` when the pre-filter refused every
    /// try; `rejected` counts the refusals. With the child, how it was bred.
    fn child(&mut self, genes: Genes, made: usize, rejected: &mut u32) -> Option<(Genome, Stats, Bred)> {
        let content = &self.content;
        let n = self.starts.len();
        let elites: Vec<(u32, &Genome)> = if self.archive.cells().is_empty() {
            let k = made.saturating_sub(n) % n;
            vec![(k as u32, &self.starts[k])]
        } else {
            self.archive.cells().values().map(|e| (e.id, &e.genome)).collect()
        };
        for _ in 0..CHILD_TRIES {
            let rng = &mut self.rng;
            let (id, a) = elites[rng.below(elites.len() as u32) as usize];
            let mut g = a.clone();
            let mut partner = None;
            if elites.len() > 1 && rng.chance(self.config.crossover) {
                let (bid, b) = elites[rng.below(elites.len() as u32) as usize];
                if genes != Genes::Behavior
                    && let Some(c) = self.kit.cross_design(content, &g, b, rng)
                {
                    g = c;
                    partner = Some(bid);
                }
                if genes != Genes::Design {
                    let slots = content.ship_rule_slots(&g.blueprint);
                    if let Some(t) = self.kit.cross_behavior(&g.behavior, &b.behavior, slots, g.root, rng) {
                        g.behavior = t;
                        partner = Some(bid);
                    }
                }
            }
            let mutations = 1 + rng.below(self.config.mutations);
            for _ in 0..mutations {
                let design = match genes {
                    Genes::Design => true,
                    Genes::Behavior => false,
                    Genes::Both => rng.chance(0.5),
                };
                if design {
                    if let Some(m) = self.kit.mutate_design(content, &g, rng, 1) {
                        g = m;
                    }
                } else {
                    let slots = content.ship_rule_slots(&g.blueprint);
                    if let Some(t) = self.kit.mutate_behavior(&g.behavior, slots, g.root, rng, 1) {
                        g.behavior = t;
                    }
                }
            }
            // A design change may have cost rule slots: the lowest unlocked rules go.
            match self.kit.fit_rules(&g.behavior, content.ship_rule_slots(&g.blueprint), g.root) {
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
            match self.kit.assess(content, &g) {
                Ok(stats) => {
                    let (removed, added) = lineage::diff(&a.frame_parts(), &g.frame_parts());
                    let bred = Bred {
                        generation: self.history.len() as u32,
                        root: g.root,
                        parent: Some(id),
                        partner,
                        removed,
                        added,
                        behavior: (g.behavior != a.behavior).then(|| g.behavior.clone()),
                        size: g.blueprint.size,
                    };
                    return Some((g, stats, bred));
                }
                Err(_) => *rejected += 1,
            }
        }
        None
    }

    /// Opens the next generation: its stage, the random stream's state, the hall of fame it fights, and in
    /// generation 0 the starts as its first children.
    fn open(&mut self) -> Batch {
        let g = self.generation();
        let stage = self.stage_of(g);
        let top = self.hall_of_fame();
        let mut b = Batch {
            stage,
            genes: self.stages[stage].0,
            from: self.rng.clone(),
            first: self.table.len(),
            opponents: top.iter().map(|e| e.id).collect(),
            extra: top.iter().map(|e| entry_of(&e.genome, 1)).collect(),
            candidates: Vec::new(),
            failed: 0,
            rejected: 0,
            ranges: Vec::new(),
            jobs: Vec::new(),
            results: Vec::new(),
            next: 0,
            active: None,
        };
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

    /// Every child of the generation is bred (or given up).
    fn bred(&self, b: &Batch) -> bool {
        b.candidates.len() + b.failed >= self.config.population as usize
    }

    /// Breeds the generation's next child: into the table and the batch with its jobs, or given up.
    fn breed_child(&mut self, b: &mut Batch) {
        let made = b.candidates.len() + b.failed;
        match self.child(b.genes, made, &mut b.rejected) {
            Some((genome, stats, bred)) => {
                let id = self.table.len() as u32;
                self.table.push(bred);
                self.add_jobs(b, Candidate { genome, id, stats });
            }
            // A child whose every try was refused stays out: the generation is that much smaller.
            None => b.failed += 1,
        }
    }

    fn add_jobs(&self, b: &mut Batch, c: Candidate) {
        let k = b.candidates.len() as u32;
        let start = b.jobs.len();
        for mut j in self.jobs_of(&c.genome, &b.extra) {
            j.set_candidate(k);
            b.jobs.push(j);
            b.results.push(None);
        }
        b.ranges.push(start..b.jobs.len());
        b.candidates.push(c);
    }

    /// Scores the generation's children from their results, offers each to the archive in order, records the
    /// generation.
    fn close(&mut self, b: Batch) {
        let generation = self.generation();
        let mut best = 0.0f32;
        let mut sum = 0.0f64;
        let mut improved = 0;
        let mut ticks = 0;
        let mut children = Vec::with_capacity(b.candidates.len());
        for (c, range) in b.candidates.into_iter().zip(&b.ranges) {
            let rs: Vec<JobResult> = b.results[range.clone()]
                .iter()
                .map(|r| r.expect("every job done"))
                .collect();
            let score = (rs.iter().map(|r| r.score as f64).sum::<f64>() / rs.len().max(1) as f64) as f32;
            let samples: u32 = rs.iter().map(|r| r.engage_samples).sum();
            let engage = if samples > 0 {
                rs.iter().map(|r| r.engage_sum as f64).sum::<f64>() as f32 / samples as f32
            } else {
                0.0
            };
            ticks += rs.iter().map(|r| r.ticks).sum::<u64>();
            best = best.max(score);
            sum += score as f64;
            let s = c.stats;
            let measures: Vec<f32> = self
                .archive
                .shape()
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
                .collect();
            let elite = Elite {
                genome: c.genome,
                id: c.id,
                score,
                measures: measures.clone(),
                generation,
            };
            let took = self.archive.offer(elite);
            improved += took as u32;
            children.push(ChildRecord {
                genome: c.id,
                score,
                measures,
                took,
                jobs: rs.iter().map(JobRecord::from).collect(),
            });
        }
        let evaluated = b.ranges.len() as u32;
        self.history.push(GenerationStats {
            generation,
            stage: b.stage,
            genes: b.genes,
            evaluated,
            rejected: b.rejected,
            best,
            mean: if evaluated > 0 {
                (sum / evaluated as f64) as f32
            } else {
                0.0
            },
            archive_best: self.archive.best().map_or(0.0, |e| e.score),
            filled: self.archive.cells().len() as u32,
            improved,
            ticks,
        });
        self.records.push(GenerationRecord {
            opponents: b.opponents,
            children,
        });
    }
}
