//! Evolution (track L3 of the evolution lab, GDR-0083, GDR-0032; `docs/arena.md` "Evolution"): designs and
//! behaviors bred through the editing models, scored by fights against a pool or by the slice's voyages, kept in
//! a MAP-Elites archive.
//!
//! A [`Run`] is built from a [`RunConfig`] (a RON text file). Each generation breeds `population` children from
//! the archive's elites (generation 0: the start and its mutants), drops those the pre-filter refuses, evaluates
//! the rest (fights or voyage runs: [`Job`]s) and offers each to its archive cell. [`Run::step`] does a bounded
//! slice of that work (so a client can call it every frame); [`Run::jobs`] and [`Run::submit`] hand a whole
//! generation's jobs out and take their results back, so the native CLI runs them on every core. Either way the
//! run is the same: a job's result depends only on the job, results are applied in job order, and every random
//! draw comes from the run's own stream in a fixed order. Single-threaded, no I/O, no wall clock.
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
//! ```

mod archive;
mod config;
mod eval;
mod genome;

use std::fmt;
use std::ops::Range;

use data::{BehaviorEditor, Blueprint, Content};
use sim::Pcg32;

use crate::fight::{Fight, resolve_design};
use crate::spec::{BehaviorRef, DesignRef, Entry, FightSpec, Formation};
use crate::voyage::{VoyageSpec, start_voyage};

pub use archive::{Archive, ArchiveShape, Axis, Elite, MAX_CELLS, Measure};
pub use config::{
    DEFAULT_CROSSOVER, DEFAULT_MUTATIONS, DEFAULT_POPULATION, DEFAULT_SEEDS, Evolve, Fitness, Genes,
    Limits, MAX_GENERATIONS, MAX_MUTATIONS, MAX_POPULATION, MAX_SEEDS, RunConfig, Stage, Start,
    VoyagePilot,
};
pub use eval::{Job, JobResult, SAMPLE_TICKS};
pub use genome::Genome;

use eval::Active;
use genome::{Kit, Stats};

/// The PCG sequence of a run's random stream (`"evolve"` in ASCII), so it never repeats a sim stream.
const RUN_STREAM: u64 = 0x6576_6f6c_7665;
/// Breeding tries per child before it is given up (the pre-filter refused every one).
const CHILD_TRIES: u32 = 16;

/// Why a run cannot be built.
#[derive(Clone, Debug, PartialEq)]
pub enum RunError {
    /// The configuration does not read or a number is out of range.
    Config(String),
    /// The start is not usable: its design or behavior does not resolve, it breaks a limit or the pre-filter.
    Start(String),
    /// The fitness does not work with the start (an unknown opponent or voyage, ...).
    Fitness(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Config(e) => write!(f, "the run configuration is not usable: {e}"),
            RunError::Start(e) => write!(f, "the start is not usable: {e}"),
            RunError::Fitness(e) => write!(f, "the fitness is not usable: {e}"),
        }
    }
}

impl std::error::Error for RunError {}

/// One generation's numbers.
#[derive(Clone, Debug, PartialEq)]
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

/// How far a run is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// The generation in progress (or the number done, once finished).
    pub generation: u32,
    pub generations: u32,
    /// Jobs of the current generation done, and how many it has.
    pub jobs_done: usize,
    pub jobs: usize,
    pub finished: bool,
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
    stats: Stats,
}

/// The current generation: its children, their jobs, the results so far.
struct Batch {
    stage: usize,
    genes: Genes,
    candidates: Vec<Candidate>,
    /// Each candidate's jobs.
    ranges: Vec<Range<usize>>,
    jobs: Vec<Job>,
    results: Vec<Option<JobResult>>,
    /// The next job to start.
    next: usize,
    active: Option<(usize, Active)>,
    rejected: u32,
}

/// An evolution run (see the module docs).
pub struct Run {
    content: Content,
    config: RunConfig,
    kit: Kit,
    stages: Vec<(Genes, u32)>,
    generations: u32,
    rng: Pcg32,
    start: Genome,
    archive: Archive,
    history: Vec<GenerationStats>,
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

impl Run {
    /// Checks the configuration, resolves the start (or builds a random one) and checks it against the limits
    /// and the pre-filter, and builds one job of each opponent or voyage to check the fitness.
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
        let start_err = RunError::Start;
        let (header, behavior, design) = match &config.start {
            Start::Design { design, behavior } => {
                let (bp, _) = resolve_design(content, design).map_err(|p| {
                    start_err(match p {
                        crate::fight::DesignProblem::UnknownShip(id) => {
                            format!("no ship `{id}` in the content")
                        }
                        crate::fight::DesignProblem::Blueprint(ps) => ps.join("; "),
                    })
                })?;
                let text = match behavior {
                    None => default_behavior(content, &bp),
                    Some(BehaviorRef::Content(id)) => content_behavior(content, id),
                    Some(BehaviorRef::Text(t)) => BehaviorEditor::from_text(&content.behavior_nodes, t)
                        .map(|e| e.text())
                        .map_err(|e| format!("the behavior text does not read: {e:?}")),
                }
                .map_err(start_err)?;
                (bp.clone(), text, Some(bp))
            }
            Start::Random { faction } => {
                let f = content
                    .factions
                    .get(faction)
                    .ok_or_else(|| start_err(format!("no faction `{faction}` in the content")))?;
                let id = f
                    .behavior
                    .clone()
                    .ok_or_else(|| start_err(format!("`{faction}` has no default behavior")))?;
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
                    source: Default::default(),
                };
                (header, content_behavior(content, &id).map_err(start_err)?, None)
            }
        };
        // The frame: the configured one, else the smallest content frame the start fits.
        let frame = match &config.limits.frame {
            Some(id) => content
                .frames
                .get(id)
                .map(|f| f.size)
                .ok_or_else(|| RunError::Config(format!("no frame `{id}` in the content")))?,
            None => {
                let (w, h) = header.size;
                content
                    .frames
                    .iter()
                    .filter(|f| f.size.0 >= w && f.size.1 >= h)
                    .min_by_key(|f| f.size.0 as u64 * f.size.1 as u64)
                    .map(|f| f.size)
                    .ok_or_else(|| start_err("no content frame is big enough".into()))?
            }
        };
        let kit = Kit::new(content, &config.limits, frame);
        // The header every child carries: the start's, renamed; the behavior goes beside it as text.
        let header = Blueprint {
            id: "evolved".into(),
            name: format!("{} (evolved)", header.name),
            behavior: None,
            source: Default::default(),
            ..header
        };
        let start = match design {
            Some(bp) => Genome {
                blueprint: Blueprint {
                    parts: bp.parts,
                    size: bp.size,
                    ..header
                },
                behavior,
            },
            None => kit
                .random_design(content, &header, &behavior, &mut rng)
                .ok_or_else(|| start_err("no random design with the parts allowed flies".into()))?,
        };
        kit.assess(content, &start).map_err(start_err)?;
        let run = Run {
            content: content.clone(),
            archive: Archive::new(config.archive.clone()),
            config,
            kit,
            stages,
            generations,
            rng,
            start,
            history: Vec::new(),
            batch: None,
        };
        // One job of every opponent or voyage with the start: the references resolve.
        for job in run.jobs_of(&run.start, &[]) {
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

    /// The start genome (generation 0's first child).
    pub fn start(&self) -> &Genome {
        &self.start
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
        let (done, jobs) = self.batch.as_ref().map_or((0, 0), |b| {
            (b.results.iter().filter(|r| r.is_some()).count(), b.jobs.len())
        });
        Progress {
            generation: self.generation(),
            generations: self.generations,
            jobs_done: done,
            jobs,
            finished: self.finished(),
        }
    }

    /// Does up to `budget` ticks of fights or voyages (breeding the next generation first when needed, and
    /// closing it once its last job is done). Call it every frame with a small budget, or with a big one.
    pub fn step(&mut self, budget: u64) -> Progress {
        if self.finished() {
            return self.progress();
        }
        if self.batch.is_none() {
            self.breed();
        }
        let mut left = budget;
        let content = &self.content;
        let b = self.batch.as_mut().expect("bred");
        while left > 0 {
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
        if b.active.is_none() && b.next >= b.jobs.len() {
            self.close();
        }
        self.progress()
    }

    /// The current generation's jobs, breeding it first if needed (empty once the run is finished). Run them
    /// anywhere ([`Job::run`]) and hand the results back with [`Run::submit`], in the same order. Don't mix
    /// with [`Run::step`] within a generation.
    pub fn jobs(&mut self) -> &[Job] {
        if self.finished() {
            return &[];
        }
        if self.batch.is_none() {
            self.breed();
        }
        &self.batch.as_ref().expect("bred").jobs
    }

    /// Takes the results of [`Run::jobs`] (same order) and closes the generation. Panics if the count differs.
    pub fn submit(&mut self, results: Vec<JobResult>) {
        let b = self.batch.as_mut().expect("submit after jobs()");
        assert_eq!(results.len(), b.jobs.len(), "one result per job");
        b.results = results.into_iter().map(Some).collect();
        b.active = None;
        b.next = b.jobs.len();
        self.close();
    }

    /// The pool of opponents for this generation: the configured ones plus the best `hall_of_fame` elites.
    fn hall_of_fame(&self) -> Vec<Entry> {
        let Fitness::ArenaPool { hall_of_fame, .. } = &self.config.fitness else {
            return Vec::new();
        };
        self.archive
            .top(*hall_of_fame as usize)
            .into_iter()
            .map(|e| Entry {
                design: DesignRef::Blueprint(e.genome.blueprint_text()),
                behavior: Some(BehaviorRef::Text(e.genome.behavior.clone())),
                count: 1,
                formation: Formation::Line,
            })
            .collect()
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

    /// One child: crossover (sometimes) and mutations of a random elite (generation 0: of the start), under
    /// `genes`. `None` when the pre-filter refused every try; `rejected` counts the refusals.
    fn child(&mut self, genes: Genes, rejected: &mut u32) -> Option<Candidate> {
        let content = &self.content;
        let elites: Vec<&Genome> = if self.archive.cells().is_empty() {
            vec![&self.start]
        } else {
            self.archive.cells().values().map(|e| &e.genome).collect()
        };
        for _ in 0..CHILD_TRIES {
            let rng = &mut self.rng;
            let a = elites[rng.below(elites.len() as u32) as usize];
            let mut g = a.clone();
            if elites.len() > 1 && rng.chance(self.config.crossover) {
                let b = elites[rng.below(elites.len() as u32) as usize];
                if genes != Genes::Behavior
                    && let Some(bp) = self.kit.cross_design(content, &g.blueprint, &b.blueprint, rng)
                {
                    g.blueprint = bp;
                }
                if genes != Genes::Design {
                    let slots = content.ship_rule_slots(&g.blueprint);
                    if let Some(t) = self.kit.cross_behavior(&g.behavior, &b.behavior, slots, rng) {
                        g.behavior = t;
                    }
                }
            }
            let n = 1 + rng.below(self.config.mutations);
            for _ in 0..n {
                let design = match genes {
                    Genes::Design => true,
                    Genes::Behavior => false,
                    Genes::Both => rng.chance(0.5),
                };
                if design {
                    if let Some(bp) = self.kit.mutate_design(content, &g.blueprint, rng, 1) {
                        g.blueprint = bp;
                    }
                } else {
                    let slots = content.ship_rule_slots(&g.blueprint);
                    if let Some(t) = self.kit.mutate_behavior(&g.behavior, slots, rng, 1) {
                        g.behavior = t;
                    }
                }
            }
            if g == *a {
                *rejected += 1;
                continue;
            }
            match self.kit.assess(content, &g) {
                Ok(stats) => return Some(Candidate { genome: g, stats }),
                Err(_) => *rejected += 1,
            }
        }
        None
    }

    /// Breeds the next generation and lists its jobs.
    fn breed(&mut self) {
        let g = self.generation();
        let stage = self.stage_of(g);
        let genes = self.stages[stage].0;
        let mut rejected = 0;
        let mut candidates = Vec::new();
        if g == 0 {
            let stats = self
                .kit
                .assess(&self.content, &self.start)
                .expect("the start passed the pre-filter in Run::new");
            candidates.push(Candidate {
                genome: self.start.clone(),
                stats,
            });
        }
        // A child whose every try was refused stays out: the generation is that much smaller.
        let mut failed = 0;
        while candidates.len() + failed < self.config.population as usize {
            match self.child(genes, &mut rejected) {
                Some(c) => candidates.push(c),
                None => failed += 1,
            }
        }
        let extra = self.hall_of_fame();
        let mut jobs = Vec::new();
        let mut ranges = Vec::new();
        for (k, c) in candidates.iter().enumerate() {
            let start = jobs.len();
            for mut j in self.jobs_of(&c.genome, &extra) {
                match &mut j {
                    Job::Fight { candidate, .. } | Job::Voyage { candidate, .. } => {
                        *candidate = k as u32
                    }
                }
                jobs.push(j);
            }
            ranges.push(start..jobs.len());
        }
        self.batch = Some(Batch {
            stage,
            genes,
            candidates,
            ranges,
            results: vec![None; jobs.len()],
            jobs,
            next: 0,
            active: None,
            rejected,
        });
    }

    /// Scores the generation's children from their results, offers each to the archive in order, records the
    /// generation.
    fn close(&mut self) {
        let b = self.batch.take().expect("a generation to close");
        let generation = self.generation();
        let mut best = 0.0f32;
        let mut sum = 0.0f64;
        let mut improved = 0;
        let mut ticks = 0;
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
            let measures = self
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
                score,
                measures,
                generation,
            };
            improved += self.archive.offer(elite) as u32;
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
    }
}
