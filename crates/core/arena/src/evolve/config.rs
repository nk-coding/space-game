//! The run configuration: what evolves, how it is scored, the limits, where it starts and what it keeps (locks), how
//! big it is. Plain data with a RON text form, so a run is a text file (`crates/core/arena/runs/`, `docs/arena.md`
//! "Evolution").

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::spec::{BehaviorRef, DEFAULT_START_DISTANCE, DEFAULT_TIME_LIMIT, DesignRef, Entry};
use crate::voyage::DEFAULT_VOYAGE_MINUTES;

use super::RunError;
use super::archive::ArchiveShape;

/// Children per generation when a run leaves it out. PLACEHOLDER (the plan's 64 halved, so a run is quick).
pub const DEFAULT_POPULATION: u32 = 32;
/// Seeds per opponent or voyage when a run leaves it out (coop-arena 3.8: 4). PLACEHOLDER.
pub const DEFAULT_SEEDS: u32 = 4;
/// Share of children bred by crossover of two elites (then mutated as well). PLACEHOLDER.
pub const DEFAULT_CROSSOVER: f32 = 0.2;
/// A child gets 1 to this many mutations. PLACEHOLDER.
pub const DEFAULT_MUTATIONS: u32 = 3;
/// Sanity bounds of a run.
pub const MAX_POPULATION: u32 = 4096;
pub const MAX_GENERATIONS: u32 = 100_000;
pub const MAX_SEEDS: u32 = 1000;
pub const MAX_MUTATIONS: u32 = 16;
/// Most starts a run may have (GDR-0092, lead default).
pub const MAX_STARTS: usize = 4;

/// A whole run. See `docs/arena.md` "Evolution" for an example.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunConfig {
    /// The run's own random stream (parents, mutations, crossover); fights and voyages use their own seeds.
    pub seed: u64,
    pub evolve: Evolve,
    /// Generations for `Behavior`, `Design` and `Both`; left out with `Stages` (each stage has its own).
    #[serde(default)]
    pub generations: Option<u32>,
    /// Children per generation (generation 0: the start and its mutants).
    #[serde(default = "default_population")]
    pub population: u32,
    pub fitness: Fitness,
    #[serde(default)]
    pub limits: Limits,
    /// One start, the form of runs before GDR-0092 (still read): taken as the first of the starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Start>,
    /// The starts (with `start`, one to [`MAX_STARTS`]). Generation 0 is every start and mutants of each, shared
    /// out evenly; every genome descends from one of them (its root) and keeps that start's locks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub starts: Vec<Start>,
    #[serde(default)]
    pub archive: ArchiveShape,
    /// Share of children bred by crossover, 0..1.
    #[serde(default = "default_crossover")]
    pub crossover: f32,
    /// A child gets 1 to this many mutations.
    #[serde(default = "default_mutations")]
    pub mutations: u32,
}

/// What evolves: the behavior only (on a fixed design), the design only (under a fixed behavior), both together,
/// or a sequence of stages (e.g. behavior for 30 generations, then design for 30, then both).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Evolve {
    Behavior,
    Design,
    Both,
    Stages(Vec<Stage>),
}

/// What one stage evolves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Genes {
    Behavior,
    Design,
    Both,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub evolve: Genes,
    pub generations: u32,
}

/// How a genome is scored, 0..1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Fitness {
    /// Fights against a pool: the fixed opponents (archetypes) plus the best `hall_of_fame` elites so far, each
    /// over `seeds` seeds (1, 2, ...).
    /// The score is the mean arena score of the genome's side.
    ArenaPool {
        opponents: Vec<Entry>,
        #[serde(default = "default_seeds")]
        seeds: u32,
        #[serde(default)]
        hall_of_fame: u32,
        /// Ships of the genome on its side.
        #[serde(default = "one")]
        count: u32,
        #[serde(default = "default_start_distance")]
        start_distance: f32,
        #[serde(default = "default_time_limit")]
        time_limit: f32,
    },
    /// The slice's voyages flown by the headless voyage runner, each over `seeds` seeds (1, 2, ...); the score is
    /// the mean voyage score (`arena::voyage_score`).
    Voyages {
        ids: Vec<String>,
        #[serde(default = "default_seeds")]
        seeds: u32,
        #[serde(default = "default_minutes")]
        minutes: f32,
        #[serde(default)]
        pilot: VoyagePilot,
    },
}

/// Who flies a voyage's fights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoyagePilot {
    /// The genome's behavior.
    #[default]
    Behavior,
    /// The runner's scripted closing pilot (the behavior is not used).
    Closing,
}

/// What a design may be. Every limit may be left out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// A content frame id (`frigate`, `cruiser`, `capital`); left out: the smallest content frame every start fits.
    #[serde(default)]
    pub frame: Option<String>,
    /// The most build points a design may cost: by default its start's ("same cost, better ship", GDR-0092), a
    /// number (`build_points: 60`), or `Off`.
    #[serde(default)]
    pub build_points: Bound,
    /// The smallest cargo hold a design may have (`Content::ship_hold`, cargo units): by default its start's, so
    /// every result still brings salvage home (VOYAGE-READY), a number, or `Off`.
    #[serde(default)]
    pub min_hold: Bound,
    /// The most logic cores a design may have: the career's mind capacity less the cores of FLEET's escorts
    /// (VOYAGE-READY); left out: any number.
    #[serde(default)]
    pub max_cores: Option<u32>,
    /// How many of a part a design may hold (`Career::available`, for the parts with a count); others: any number.
    #[serde(default)]
    pub counts: Option<BTreeMap<String, u32>>,
    /// Only these parts (e.g. a career's unlocked parts, `data::career::Unlocks::parts`); left out: all parts.
    #[serde(default)]
    pub parts: Option<BTreeSet<String>>,
    /// Only these armour materials; left out: all.
    #[serde(default)]
    pub materials: Option<BTreeSet<String>>,
    /// Mirror lock: every design edit is mirrored across the frame's centre line, and designs stay symmetric.
    #[serde(default)]
    pub mirror: bool,
}

/// A limit taken from the start (the default), a number, or none. A plain number reads as `At` (`build_points:
/// 60`, the form of runs before GDR-0092).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "BoundText")]
pub enum Bound {
    /// The start's own value (each genome its own start's).
    #[default]
    Start,
    Off,
    At(f32),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BoundText {
    Number(f32),
    Named(BoundName),
}

#[derive(Deserialize)]
enum BoundName {
    Start,
    Off,
    At(f32),
}

impl From<BoundText> for Bound {
    fn from(t: BoundText) -> Bound {
        match t {
            BoundText::Number(v) | BoundText::Named(BoundName::At(v)) => Bound::At(v),
            BoundText::Named(BoundName::Start) => Bound::Start,
            BoundText::Named(BoundName::Off) => Bound::Off,
        }
    }
}

impl Bound {
    /// The limit for a genome whose start has `start`.
    pub fn of(self, start: f32) -> Option<f32> {
        match self {
            Bound::Start => Some(start),
            Bound::Off => None,
            Bound::At(v) => Some(v),
        }
    }
}

/// Where a run starts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Start {
    /// A design (a content ship or blueprint text) and a behavior (left out: the design's own, else its
    /// faction's), and what the run keeps of them.
    Design {
        design: DesignRef,
        #[serde(default)]
        behavior: Option<BehaviorRef>,
        #[serde(default, skip_serializing_if = "Locks::is_empty")]
        locks: Locks,
    },
    /// A random design that flies (a logic core, a power source, an engine and a gun, then random parts) of this
    /// faction, under the faction's behavior.
    Random { faction: String },
}

/// What a run keeps of a start (GDR-0092 "LOCKS"). The operators never pick what is locked, and the pre-filter
/// refuses a child that lost any of it, so crossover cannot break a lock either. Under the mirror lock a locked part
/// or cell locks its mirror image too.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Locks {
    /// Parts by index in the start's blueprint (`parts[i]`): they never move, turn, change material or power, or go.
    #[serde(default)]
    pub parts: Vec<u32>,
    /// Part categories (`categories.ron` ids): every part of them in the start is locked (`["weapons"]`: KEEP ALL
    /// WEAPONS, each gun where it is).
    #[serde(default)]
    pub categories: Vec<String>,
    /// KEEP EMPTY: cells `(x, y)` in the start blueprint's coordinates that no part may cover (a clear line of fire).
    #[serde(default)]
    pub cells: Vec<(i32, i32)>,
    /// Rules by index in the start's behavior (0 = the top rule): never changed, moved or deleted. Rules may still
    /// be added around them.
    #[serde(default)]
    pub rules: Vec<u32>,
    /// The behavior's target line stays as it is.
    #[serde(default)]
    pub target: bool,
    /// LOCK ORDER: the locked rules also keep their order among themselves.
    #[serde(default)]
    pub order: bool,
}

impl Locks {
    pub fn is_empty(&self) -> bool {
        *self == Locks::default()
    }
}

fn default_population() -> u32 {
    DEFAULT_POPULATION
}
fn default_seeds() -> u32 {
    DEFAULT_SEEDS
}
fn default_crossover() -> f32 {
    DEFAULT_CROSSOVER
}
fn default_mutations() -> u32 {
    DEFAULT_MUTATIONS
}
fn default_start_distance() -> f32 {
    DEFAULT_START_DISTANCE
}
fn default_time_limit() -> f32 {
    DEFAULT_TIME_LIMIT
}
fn default_minutes() -> f32 {
    DEFAULT_VOYAGE_MINUTES
}
fn one() -> u32 {
    1
}

impl RunConfig {
    /// Reads a run from its RON text (`Some(..)` may be left out, unknown fields are errors, comments are fine).
    pub fn from_ron(text: &str) -> Result<RunConfig, RunError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        data::ron_options()
            .from_str(text)
            .map_err(|e| RunError::Config(e.to_string()))
    }

    /// The run as RON text; reads back with [`RunConfig::from_ron`] to an equal run.
    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new().escape_strings(false);
        data::ron_options()
            .to_string_pretty(self, pretty)
            .expect("a run configuration always serialises")
    }

    /// The stages in order: what evolves and for how many generations.
    pub fn schedule(&self) -> Result<Vec<(Genes, u32)>, RunError> {
        let bad = |m: &str| Err(RunError::Config(m.to_string()));
        let stages = match (&self.evolve, self.generations) {
            (Evolve::Stages(_), Some(_)) => {
                return bad("`generations` goes into each stage with `Stages`");
            }
            (Evolve::Stages(s), None) => s.iter().map(|s| (s.evolve, s.generations)).collect(),
            (_, None) => return bad("`generations` is missing"),
            (Evolve::Behavior, Some(g)) => vec![(Genes::Behavior, g)],
            (Evolve::Design, Some(g)) => vec![(Genes::Design, g)],
            (Evolve::Both, Some(g)) => vec![(Genes::Both, g)],
        };
        let total: u64 = stages.iter().map(|s: &(Genes, u32)| s.1 as u64).sum();
        if stages.is_empty() || stages.iter().any(|s| s.1 == 0) {
            return bad("every stage needs at least one generation");
        }
        if total > MAX_GENERATIONS as u64 {
            return Err(RunError::Config(format!(
                "{total} generations; at most {MAX_GENERATIONS}"
            )));
        }
        Ok(stages)
    }

    /// Every start: `start` (the old form), then `starts`.
    pub fn all_starts(&self) -> impl Iterator<Item = &Start> + '_ {
        self.start.iter().chain(&self.starts)
    }

    /// The numbers are in range (references are checked when the run is built).
    pub(crate) fn check(&self) -> Result<(), RunError> {
        let bad = |m: String| Err(RunError::Config(m));
        let starts = self.all_starts().count();
        if !(1..=MAX_STARTS).contains(&starts) {
            return bad(format!("a run needs one to {MAX_STARTS} starts (`start` or `starts`)"));
        }
        if !(1..=MAX_POPULATION).contains(&self.population) {
            return bad(format!("`population` must be 1 to {MAX_POPULATION}"));
        }
        if (self.population as usize) < starts {
            return bad(format!("`population` must be at least the {starts} starts"));
        }
        if !(0.0..=1.0).contains(&self.crossover) {
            return bad("`crossover` must be 0 to 1".into());
        }
        if !(1..=MAX_MUTATIONS).contains(&self.mutations) {
            return bad(format!("`mutations` must be 1 to {MAX_MUTATIONS}"));
        }
        let seeds = match &self.fitness {
            Fitness::ArenaPool {
                opponents, seeds, count, ..
            } => {
                if opponents.is_empty() {
                    return bad("the arena pool needs at least one opponent".into());
                }
                if *count == 0 {
                    return bad("`count` must be at least 1".into());
                }
                *seeds
            }
            Fitness::Voyages { ids, seeds, .. } => {
                if ids.is_empty() {
                    return bad("`Voyages` needs at least one voyage id".into());
                }
                *seeds
            }
        };
        if !(1..=MAX_SEEDS).contains(&seeds) {
            return bad(format!("`seeds` must be 1 to {MAX_SEEDS}"));
        }
        if let Bound::At(b) = self.limits.build_points
            && !(b.is_finite() && b > 0.0)
        {
            return bad("`build_points` must be above 0".into());
        }
        if let Bound::At(h) = self.limits.min_hold
            && !(h.is_finite() && h >= 0.0)
        {
            return bad("`min_hold` must be 0 or more".into());
        }
        self.archive.check()?;
        self.schedule().map(|_| ())
    }
}
