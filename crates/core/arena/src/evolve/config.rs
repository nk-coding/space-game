//! The run configuration: what evolves, how it is scored, the limits, where it starts and what it keeps (locks), how
//! big it is. Plain data with a RON text form, so a run is a text file (`crates/core/arena/runs/`, `docs/arena.md`
//! "Evolution").

use std::collections::{BTreeMap, BTreeSet};

use data::Upgrades;
use serde::{Deserialize, Serialize};

use crate::spec::{BehaviorRef, DesignRef};

use super::RunError;
use super::archive::ArchiveShape;
use super::trial::{Goals, MAX_WEIGHT, Trial, TrialSpec};

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
/// THE ENEMY's pairing when a run leaves it out (GDR-0092 "Co-evolution", PLACEHOLDERS): your children fight its best
/// 3 elites and 2 of its past champions, its children your best 3, 2 of yours and your starts; every 10 generations
/// both bests fight the other's champions of every 5th generation.
pub const DEFAULT_ELITES: u32 = 3;
pub const DEFAULT_HALL: u32 = 2;
pub const DEFAULT_CHECK_EVERY: u32 = 10;
pub const DEFAULT_CHAMPIONS_EVERY: u32 = 5;
/// The most elites or champions a generation's frozen set draws from each side (a sanity bound).
pub const MAX_PAIRING: u32 = 16;

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
    /// What a genome is scored against (at least one): each trial with its weight (`trial.rs`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trials: Vec<TrialSpec>,
    /// What a score weighs: WIN alone when left out (the score of the runs before step 5).
    #[serde(default, skip_serializing_if = "Goals::is_default")]
    pub goals: Goals,
    #[serde(default)]
    pub limits: Limits,
    /// The starts (one to [`MAX_STARTS`]). Generation 0 is every start and mutants of each, shared
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
    /// THE ENEMY (GDR-0092 "Co-evolution", the ARMS RACE): a second population that evolves against yours in the same
    /// run, scored by its fights against your best; your side fights it in a `CoEvolution` trial. Written only when
    /// there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enemy: Option<Population>,
    /// The researched part-stat steps (GDR-0024's refinement: the career's) your side is built with: your genomes,
    /// starts, hall of fame and fleet in every fight and voyage, and the measures; never THE ENEMY or the content's
    /// opponents. Written only when there are any.
    #[serde(default, skip_serializing_if = "Upgrades::is_empty")]
    pub upgrades: Upgrades,
}

/// THE ENEMY of an arms race (GDR-0092 "Co-evolution"): its own starts, genes, limits, archive, size and random
/// stream (the run's seed on a sequence of its own). Each generation both populations fight frozen sets of the
/// other's from the end of the one before: your children its best `elites` and `hall` of its past champions, its
/// children your best `elites`, `hall` of your past champions and (`anchor`) your starts; generation 0 the starts of
/// each side. An enemy fights as side 1 in a pack sized by the test range's wave rule to your side's build points
/// (`sim::voyage::waves::wave_mix`), so it needs no build-point cap: a bigger design comes in fewer copies. Its
/// score is side 1's WIN.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Population {
    /// One to [`MAX_STARTS`] designs (the lab's: an enemy faction's content ships under their own behaviors).
    pub starts: Vec<Start>,
    /// What may change (left out: the design's and the behavior's genes; enemies are never piloted, so their assists
    /// don't change).
    #[serde(default = "enemy_genes", skip_serializing_if = "is_enemy_genes")]
    pub evolve: MayChange,
    /// Its children per generation (generation 0: its starts and their mutants).
    pub population: u32,
    /// A content frame id; left out: the smallest content frame every start fits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    /// The parts it builds with besides those its starts use (`lab.ron`'s palette of the faction, resolved when the
    /// run is made); the armour materials are its starts'.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<String>,
    /// Its designs stay symmetric (every edit mirrored; its starts must be symmetric).
    #[serde(default)]
    pub mirror: bool,
    #[serde(default)]
    pub archive: ArchiveShape,
    #[serde(default = "default_elites")]
    pub elites: u32,
    #[serde(default = "default_hall")]
    pub hall: u32,
    /// Its children also fight your starts, so it stays able to beat the ships you actually have.
    #[serde(default = "yes")]
    pub anchor: bool,
    /// PAST CHAMPIONS: every this many generations (0: never) both bests fight the other's champions of every
    /// `champions_every`th generation so far.
    #[serde(default = "default_check_every")]
    pub check_every: u32,
    #[serde(default = "default_champions_every")]
    pub champions_every: u32,
}

impl Population {
    /// What it builds with: its limits as a [`Kit`](super::genome::Kit) takes them (only the parts and armour
    /// materials its starts use and its palette, no build-point cap, no hold or core limit).
    pub(crate) fn limits(&self, parts: BTreeSet<String>, materials: BTreeSet<String>) -> Limits {
        Limits {
            frame: self.frame.clone(),
            build_points: Bound::Off,
            min_hold: Bound::Off,
            max_cores: None,
            counts: None,
            parts: Some(parts.into_iter().chain(self.palette.iter().cloned()).collect()),
            materials: Some(materials),
            mirror: self.mirror,
        }
    }

    fn check(&self) -> Result<(), String> {
        if !(1..=MAX_STARTS).contains(&self.starts.len()) {
            return Err(format!("it needs one to {MAX_STARTS} starts"));
        }
        if !(1..=MAX_POPULATION).contains(&self.population) || (self.population as usize) < self.starts.len() {
            return Err(format!("`population` must be its {} starts to {MAX_POPULATION}", self.starts.len()));
        }
        if self.evolve.assists() {
            return Err("enemies are never piloted: their assists don't change".into());
        }
        if Gene::ALL.iter().all(|g| !self.evolve.has(*g)) {
            return Err("nothing may change".into());
        }
        if self.elites == 0 || self.elites > MAX_PAIRING || self.hall > MAX_PAIRING {
            return Err(format!("`elites` must be 1 to {MAX_PAIRING}, `hall` 0 to {MAX_PAIRING}"));
        }
        if self.check_every > 0 && self.champions_every == 0 {
            return Err("`champions_every` must be at least 1".into());
        }
        Ok(())
    }
}

/// What evolves: the behavior only (on a fixed design), the design only (under a fixed behavior), both together,
/// the genes a run may change (GDR-0092 "MAY CHANGE": `MayChange(())` is everything, the assists included), or a
/// sequence of stages (e.g. behavior for 30 generations, then design for 30, then both).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Evolve {
    Behavior,
    Design,
    Both,
    MayChange(MayChange),
    Stages(Vec<Stage>),
}

/// What one stage evolves: `Design` is the design's genes (layout, parts, armour, power), `Behavior` the behavior's
/// (its numbers and its rules), `Both` both; `MayChange` the genes it names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Genes {
    Behavior,
    Design,
    Both,
    MayChange(MayChange),
}

impl Genes {
    /// The genes it changes.
    pub fn may_change(&self) -> MayChange {
        let both = MayChange { assist_numbers: false, assist_rules: false, ..MayChange::default() };
        match self {
            Genes::Design => MayChange { numbers: false, rules: false, ..both },
            Genes::Behavior => MayChange { layout: false, parts: false, armour: false, power: false, ..both },
            Genes::Both => both,
            Genes::MayChange(m) => m.clone(),
        }
    }
}

/// A gene of a genome (GDR-0092 "MAY CHANGE"): what a mutation may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gene {
    /// Move, rotate and flip parts.
    Layout,
    /// Add, remove and swap parts (of the categories not [`MayChange::fixed`]).
    Parts,
    /// A hull part's material.
    Armour,
    /// A part's power tier and weight.
    Power,
    /// The behavior's thresholds, parameters and choices.
    Numbers,
    /// The behavior's structure: its rules and their order, their actions and conditions, the target line.
    Rules,
    /// The pilot assists' numbers and choices, and whether each is on at the start.
    AssistNumbers,
    /// The pilot assists' structure: their rules and order, their assists and conditions.
    AssistRules,
}

impl Gene {
    /// In the order of `data::GENES`, whose ids they have.
    pub const ALL: [Gene; 8] = [
        Gene::Layout,
        Gene::Parts,
        Gene::Armour,
        Gene::Power,
        Gene::Numbers,
        Gene::Rules,
        Gene::AssistNumbers,
        Gene::AssistRules,
    ];

    /// Its id in `lab.ron` (`data::GENES`).
    pub fn id(self) -> &'static str {
        data::GENES[Gene::ALL.iter().position(|g| *g == self).expect("every gene is in ALL")]
    }

    pub fn of(id: &str) -> Option<Gene> {
        Gene::ALL.into_iter().find(|g| g.id() == id)
    }
}

/// The genes a run may change (GDR-0092 "MAY CHANGE"; each left out: on, so `()` is everything). The locks still
/// hold within them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MayChange {
    pub layout: bool,
    pub parts: bool,
    /// Part categories (`categories.ron` ids) whose parts are neither added, removed nor swapped while `parts` is on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fixed: Vec<String>,
    pub armour: bool,
    pub power: bool,
    pub numbers: bool,
    pub rules: bool,
    pub assist_numbers: bool,
    pub assist_rules: bool,
}

impl Default for MayChange {
    /// Everything.
    fn default() -> MayChange {
        MayChange {
            layout: true,
            parts: true,
            fixed: Vec::new(),
            armour: true,
            power: true,
            numbers: true,
            rules: true,
            assist_numbers: true,
            assist_rules: true,
        }
    }
}

impl MayChange {
    pub fn has(&self, gene: Gene) -> bool {
        match gene {
            Gene::Layout => self.layout,
            Gene::Parts => self.parts,
            Gene::Armour => self.armour,
            Gene::Power => self.power,
            Gene::Numbers => self.numbers,
            Gene::Rules => self.rules,
            Gene::AssistNumbers => self.assist_numbers,
            Gene::AssistRules => self.assist_rules,
        }
    }

    pub fn set(&mut self, gene: Gene, on: bool) {
        *match gene {
            Gene::Layout => &mut self.layout,
            Gene::Parts => &mut self.parts,
            Gene::Armour => &mut self.armour,
            Gene::Power => &mut self.power,
            Gene::Numbers => &mut self.numbers,
            Gene::Rules => &mut self.rules,
            Gene::AssistNumbers => &mut self.assist_numbers,
            Gene::AssistRules => &mut self.assist_rules,
        } = on;
    }

    /// These genes (`lab.ron` ids, a gene preset's) on, the others off; no category kept.
    pub fn only<'a>(ids: impl IntoIterator<Item = &'a String>) -> MayChange {
        let mut m = MayChange::default();
        for g in Gene::ALL {
            m.set(g, false);
        }
        for g in ids.into_iter().filter_map(|id| Gene::of(id)) {
            m.set(g, true);
        }
        m
    }

    /// A gene of the design is on (layout, parts, armour, power).
    pub fn design(&self) -> bool {
        self.layout || self.parts || self.armour || self.power
    }

    /// A gene of the behavior is on.
    pub fn behavior(&self) -> bool {
        self.numbers || self.rules
    }

    /// A gene of the pilot assists is on.
    pub fn assists(&self) -> bool {
        self.assist_numbers || self.assist_rules
    }

    /// Parts of this content part may be added, removed or swapped.
    pub(crate) fn changes(&self, content: &data::Content, part: &str) -> bool {
        self.parts && content.parts.get(part).is_some_and(|p| !self.fixed.contains(&p.category))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub evolve: Genes,
    pub generations: u32,
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
    /// number (`build_points: At(60)`), or `Off`.
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

/// A limit taken from the start (the default), a number (`At(60)`), or none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Bound {
    /// The start's own value (each genome its own start's).
    #[default]
    Start,
    Off,
    At(f32),
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
    /// Rules by index in the start's pilot assists (0 = the top): never changed, moved or deleted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assists: Vec<u32>,
    /// LOCK ORDER: the locked rules also keep their order among themselves (the behavior's and the assists' each).
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
fn default_crossover() -> f32 {
    DEFAULT_CROSSOVER
}
fn default_mutations() -> u32 {
    DEFAULT_MUTATIONS
}
fn yes() -> bool {
    true
}
fn default_elites() -> u32 {
    DEFAULT_ELITES
}
fn default_hall() -> u32 {
    DEFAULT_HALL
}
fn default_check_every() -> u32 {
    DEFAULT_CHECK_EVERY
}
fn default_champions_every() -> u32 {
    DEFAULT_CHAMPIONS_EVERY
}
/// THE ENEMY's genes: the design's and the behavior's.
pub(crate) fn enemy_genes() -> MayChange {
    Genes::Both.may_change()
}
fn is_enemy_genes(m: &MayChange) -> bool {
    *m == enemy_genes()
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
            (Evolve::Stages(s), None) => s.iter().map(|s| (s.evolve.clone(), s.generations)).collect(),
            (_, None) => return bad("`generations` is missing"),
            (Evolve::Behavior, Some(g)) => vec![(Genes::Behavior, g)],
            (Evolve::Design, Some(g)) => vec![(Genes::Design, g)],
            (Evolve::Both, Some(g)) => vec![(Genes::Both, g)],
            (Evolve::MayChange(m), Some(g)) => vec![(Genes::MayChange(m.clone()), g)],
        };
        let total: u64 = stages.iter().map(|s: &(Genes, u32)| s.1 as u64).sum();
        if stages.is_empty() || stages.iter().any(|s| s.1 == 0) {
            return bad("every stage needs at least one generation");
        }
        if stages.iter().any(|s| Gene::ALL.iter().all(|g| !s.0.may_change().has(*g))) {
            return bad("nothing may change");
        }
        if total > MAX_GENERATIONS as u64 {
            return Err(RunError::Config(format!(
                "{total} generations; at most {MAX_GENERATIONS}"
            )));
        }
        Ok(stages)
    }

    /// The numbers are in range (references are checked when the run is built).
    pub(crate) fn check(&self) -> Result<(), RunError> {
        let bad = |m: String| Err(RunError::Config(m));
        let starts = self.starts.len();
        if !(1..=MAX_STARTS).contains(&starts) {
            return bad(format!("a run needs one to {MAX_STARTS} starts"));
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
        if let Err(e) = self.upgrades.check() {
            return bad(format!("`upgrades`: {e}"));
        }
        let trials = &self.trials;
        if trials.is_empty() {
            return bad("a run needs at least one trial".into());
        }
        for (k, t) in trials.iter().enumerate() {
            t.check().map_err(|e| RunError::Config(format!("trial {}: {e}", k + 1)))?;
        }
        if trials.iter().all(|t| t.weight == 0.0) {
            return bad("every trial weighs 0".into());
        }
        let goals = self.goals.weights();
        if goals.iter().any(|w| !(w.is_finite() && (0.0..=MAX_WEIGHT).contains(w))) || goals.iter().all(|w| *w == 0.0) {
            return bad(format!("the goals' weights must be 0 to {MAX_WEIGHT}, not all 0"));
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
        let coevolution = trials.iter().filter(|t| matches!(t.kind, Trial::CoEvolution)).count();
        match (&self.enemy, coevolution) {
            (Some(e), 1) => {
                e.check().map_err(|m| RunError::Config(format!("the enemy: {m}")))?;
                e.archive.check()?;
            }
            (None, 0) => {}
            (None, _) => return bad("a `CoEvolution` trial needs an `enemy`".into()),
            (Some(_), _) => return bad("an `enemy` needs one `CoEvolution` trial: it scores your designs against it".into()),
        }
        self.schedule().map(|_| ())
    }
}
