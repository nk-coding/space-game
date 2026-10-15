//! Trials and goals (GDR-0092 "Trials", "GOALS"): what a genome is scored against, and what a score weighs.
//!
//! A run has any number of [`TrialSpec`]s, each a kind ([`Trial`]) with its data and the settings every trial shares
//! (weight, seeds, copies of the contestant, start distance, time limit, escorts beside it). At `Run::new` each spec
//! is resolved against the content into a [`TrialKind`]: the one abstraction a kind implements, turning a contestant
//! (a genome as a fight entry) and the generation's frozen opponents ([`GenCtx`]: the starts, the hall of fame) into
//! jobs. A kind that reduces to fights is a small impl with no job type of its own; a gauntlet is a fight spec with
//! waves, a voyage a voyage spec.
//!
//! Every job scores four goals, 0..1 each ([`GoalScores`]): WIN (the arena score; a gauntlet's waves cleared of all;
//! the voyage score), UNHURT (one less the share of side 0's build points lost; of the hull, on a voyage), QUICK (one
//! less the share of the time limit a win took) and CHEAP (one less the design's build points over its limit). A
//! child's score is, per trial, the goals' means over its jobs weighed by [`Goals`], then the trials weighed by their
//! weights; trials without jobs (the hall of fame in generation 0) and weights of 0 leave the sums. With one trial and
//! WIN alone that is the mean arena score of all its jobs, bit for bit as before.

use data::Content;
use serde::{Deserialize, Serialize};

use crate::spec::{BehaviorRef, DEFAULT_START_DISTANCE, DEFAULT_TIME_LIMIT, DesignRef, Entry, FightSpec, Formation, Side};
use crate::voyage::{DEFAULT_VOYAGE_MINUTES, VoyageSpec};

use super::config::{DEFAULT_SEEDS, Fitness, VoyagePilot};
use super::eval::Job;
use super::RunError;

/// TEST-RANGE WAVES: waves cleared out of this many is a win when a run leaves it out. PLACEHOLDER (`lab.ron`).
pub const DEFAULT_WAVES: u32 = 5;
/// The largest weight of a trial or a goal, a sanity bound (the lab's steppers run 0 to 3).
pub const MAX_WEIGHT: f32 = 100.0;

/// One trial of a run (GDR-0092 "Shared settings of every trial").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrialSpec {
    /// The `lab.ron` preset it was made from: its name on the run's page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    pub kind: Trial,
    /// How much it counts against the run's other trials.
    #[serde(default = "one_f")]
    pub weight: f32,
    /// Each opponent (or voyage) is fought on seeds 1..=`seeds`.
    #[serde(default = "default_seeds")]
    pub seeds: u32,
    /// Ships of the contestant on side 0.
    #[serde(default = "one")]
    pub count: u32,
    #[serde(default = "default_start_distance")]
    pub start_distance: f32,
    /// Seconds a fight lasts at most (a voyage's limit is its kind's `minutes`).
    #[serde(default = "default_time_limit")]
    pub time_limit: f32,
    /// WITH YOUR ESCORTS: these fly beside the contestant on side 0 (FLEET's escorts, inlined), as a voyage
    /// launches them; on a voyage their designs come along under the content's escort behavior.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub escorts: Vec<Entry>,
}

/// The trial kinds (GDR-0092's table). Player designs come inlined as text, content by id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Trial {
    /// ENEMY SHIPS: fixed opponents, each entry a fight per seed, and the run's best `hall_of_fame` with them in the
    /// same mean (the arena pool of the runs before GDR-0092 step 5: their `fitness: ArenaPool` reads as this).
    EnemyShips {
        opponents: Vec<Entry>,
        #[serde(default)]
        hall_of_fame: u32,
    },
    /// THE ENEMIES YOU MEET: every encounter of these voyages (in their sites' order, each once) that has ships,
    /// its ships at their largest counts, a fight each; the guardian with its voyage.
    Encounters { voyages: Vec<String> },
    /// TEST-RANGE WAVES: a gauntlet of `target` waves as the test range sends them (`sim::voyage::waves`), each
    /// sized from the contestant's build points; WIN is the waves cleared of `target`.
    Waves {
        #[serde(default = "default_waves")]
        target: u32,
    },
    /// A gauntlet: these waves one after the other (the first is side 1, each next when the one before is out).
    Gauntlet { waves: Vec<Vec<Entry>> },
    /// BEAT YOUR START: every start of the run under its own behavior.
    BeatYourStart,
    /// YOUR FLEET: the player's designs, each a fight.
    Fleet { designs: Vec<Entry> },
    /// HALL OF FAME: the run's own best `count` so far (frozen per generation).
    HallOfFame { count: u32 },
    /// CHAMPIONS: the best designs of earlier runs, each a fight.
    Champions { designs: Vec<Entry> },
    /// VOYAGES: these voyages flown by the headless voyage runner (`docs/arena.md` "Voyage fitness"); the runs
    /// before GDR-0092 step 5 had them as `fitness: Voyages`.
    Voyages {
        ids: Vec<String>,
        #[serde(default = "default_minutes")]
        minutes: f32,
        #[serde(default)]
        pilot: VoyagePilot,
    },
}

/// The goals' weights (GDR-0092 "GOALS", the setup's sliders). Left out: WIN alone.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Goals {
    #[serde(default = "one_f")]
    pub win: f32,
    #[serde(default)]
    pub unhurt: f32,
    #[serde(default)]
    pub quick: f32,
    #[serde(default)]
    pub cheap: f32,
}

impl Default for Goals {
    fn default() -> Goals {
        Goals { win: 1.0, unhurt: 0.0, quick: 0.0, cheap: 0.0 }
    }
}

impl Goals {
    /// WIN alone (written only when it is not).
    pub fn is_default(&self) -> bool {
        *self == Goals::default()
    }

    /// The weights in [`data::GOALS`]' order.
    pub fn weights(&self) -> [f32; 4] {
        [self.win, self.unhurt, self.quick, self.cheap]
    }

    /// Weights by goal id (`data::GOALS`), others 0: a `lab.ron` goal preset.
    pub fn of(weights: impl IntoIterator<Item = (String, f32)>) -> Goals {
        let mut g = Goals { win: 0.0, ..Goals::default() };
        for (id, w) in weights {
            match id.as_str() {
                "win" => g.win = w,
                "unhurt" => g.unhurt = w,
                "quick" => g.quick = w,
                "cheap" => g.cheap = w,
                _ => {}
            }
        }
        g
    }
}

/// One job's score per goal, 0..1 each (in [`data::GOALS`]' order as [`GoalScores::all`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalScores {
    pub win: f32,
    pub unhurt: f32,
    pub quick: f32,
    pub cheap: f32,
}

impl GoalScores {
    pub fn all(&self) -> [f32; 4] {
        [self.win, self.unhurt, self.quick, self.cheap]
    }
}

/// The contestant a trial makes jobs for: the genome as a fight entry (blueprint and behavior inlined, one ship) and
/// its build points (TEST-RANGE WAVES size their waves from them).
pub struct Contestant {
    pub entry: Entry,
    pub build_points: f32,
}

/// What a generation's jobs fight besides the fixed opponents, frozen when it opens: the run's starts (each as an
/// entry under its own behavior) and its hall of fame (the archive's best, best first); and the seeds.
pub struct GenCtx<'a> {
    pub starts: &'a [Entry],
    pub hall: &'a [Entry],
    /// VALIDATE: this many new seeds, the ones after each trial's own (`Some(20)`: seeds n + 1 to n + 20 of a trial
    /// of n); `None`: the trial's own seeds 1 to n, as a run fights them.
    pub fresh: Option<u32>,
}

/// The seeds a trial of `own` seeds fights in `ctx`.
fn seeds(own: u32, ctx: &GenCtx) -> std::ops::RangeInclusive<u64> {
    match ctx.fresh {
        Some(n) => own as u64 + 1..=own as u64 + n as u64,
        None => 1..=own as u64,
    }
}

/// One kind of trial, resolved against the content ([`TrialSpec::resolve`]): it holds everything it needs, so the run
/// is self-contained, and makes a contestant's jobs of a generation in a fixed order. A job's goals follow from the
/// job and how it ended (a fight, a gauntlet or a voyage, `eval.rs`), so a kind needs no scoring of its own.
pub trait TrialKind: Send + Sync {
    /// The jobs of one contestant in one generation, in a fixed order (every opponent, then every seed).
    fn jobs(&self, me: &Contestant, ctx: &GenCtx) -> Vec<Job>;
    /// How many of the run's best it fights: the generation's frozen hall of fame holds the most any trial wants.
    fn hall_of_fame(&self) -> u32 {
        0
    }
}

/// The settings every fight of a trial shares.
#[derive(Clone)]
struct Fights {
    seeds: u32,
    count: u32,
    start_distance: f32,
    time_limit: f32,
    escorts: Vec<Entry>,
}

impl Fights {
    /// The fights of `me` against one opponent (and its waves, a gauntlet), one per seed.
    fn against(&self, me: &Contestant, ctx: &GenCtx, side: &[Entry], waves: &[Side], jobs: &mut Vec<Job>) {
        let mut ours = vec![Entry { count: self.count, ..me.entry.clone() }];
        ours.extend(self.escorts.iter().cloned());
        for seed in seeds(self.seeds, ctx) {
            let mut spec = FightSpec::new(seed, ours.clone(), side.to_vec());
            spec.start_distance = self.start_distance;
            spec.time_limit = self.time_limit;
            spec.waves = waves.to_vec();
            jobs.push(Job::Fight { candidate: 0, spec });
        }
    }
}

/// Fixed opponents, each a fight (ENEMY SHIPS with its hall of fame, THE ENEMIES YOU MEET, YOUR FLEET, CHAMPIONS, a
/// gauntlet's first wave with the rest after it).
struct Opponents {
    fights: Fights,
    sides: Vec<(Vec<Entry>, Vec<Side>)>,
    hall_of_fame: u32,
}

impl TrialKind for Opponents {
    fn jobs(&self, me: &Contestant, ctx: &GenCtx) -> Vec<Job> {
        let mut jobs = Vec::new();
        for (side, waves) in &self.sides {
            self.fights.against(me, ctx, side, waves, &mut jobs);
        }
        for o in ctx.hall.iter().take(self.hall_of_fame as usize) {
            self.fights.against(me, ctx, std::slice::from_ref(o), &[], &mut jobs);
        }
        jobs
    }

    fn hall_of_fame(&self) -> u32 {
        self.hall_of_fame
    }
}

/// TEST-RANGE WAVES: each seed a gauntlet of `target` equal waves sized from the contestant (the test range's rule).
struct Waves {
    fights: Fights,
    /// The content's enemy ships and their build points, smallest first.
    enemies: Vec<(String, f32)>,
    target: u32,
}

impl TrialKind for Waves {
    fn jobs(&self, me: &Contestant, ctx: &GenCtx) -> Vec<Job> {
        let ships = sim::voyage::waves::wave(me.build_points * self.fights.count as f32, &self.enemies);
        // Ships of one kind next to each other are one entry: `2 × hunter, 1 × raider`.
        let mut entries: Vec<Entry> = Vec::new();
        for id in ships {
            match entries.last_mut() {
                Some(e) if e.design == DesignRef::Ship(id.clone()) => e.count += 1,
                _ => entries.push(Entry::ship(&id)),
            }
        }
        let rest = vec![Side { entries: entries.clone() }; self.target.saturating_sub(1) as usize];
        let mut jobs = Vec::new();
        self.fights.against(me, ctx, &entries, &rest, &mut jobs);
        jobs
    }
}

/// BEAT YOUR START: the run's starts.
struct Starts {
    fights: Fights,
}

impl TrialKind for Starts {
    fn jobs(&self, me: &Contestant, ctx: &GenCtx) -> Vec<Job> {
        let mut jobs = Vec::new();
        for s in ctx.starts {
            self.fights.against(me, ctx, std::slice::from_ref(s), &[], &mut jobs);
        }
        jobs
    }
}

/// HALL OF FAME: the run's best so far.
struct Hall {
    fights: Fights,
    count: u32,
}

impl TrialKind for Hall {
    fn jobs(&self, me: &Contestant, ctx: &GenCtx) -> Vec<Job> {
        let mut jobs = Vec::new();
        for o in ctx.hall.iter().take(self.count as usize) {
            self.fights.against(me, ctx, std::slice::from_ref(o), &[], &mut jobs);
        }
        jobs
    }

    fn hall_of_fame(&self) -> u32 {
        self.count
    }
}

/// VOYAGES: each voyage on every seed, flown by the genome's behavior or the runner's closing pilot.
struct Voyages {
    ids: Vec<String>,
    seeds: u32,
    minutes: f32,
    pilot: VoyagePilot,
    escorts: Vec<DesignRef>,
}

impl TrialKind for Voyages {
    fn jobs(&self, me: &Contestant, ctx: &GenCtx) -> Vec<Job> {
        let behavior = match self.pilot {
            VoyagePilot::Behavior => me.entry.behavior.clone(),
            VoyagePilot::Closing => None,
        };
        let mut jobs = Vec::new();
        for v in &self.ids {
            for seed in seeds(self.seeds, ctx) {
                let spec = VoyageSpec {
                    voyage: v.clone(),
                    seed,
                    design: me.entry.design.clone(),
                    behavior: behavior.clone(),
                    minutes: self.minutes,
                    escorts: self.escorts.clone(),
                };
                jobs.push(Job::Voyage { candidate: 0, spec });
            }
        }
        jobs
    }
}

impl TrialSpec {
    /// A trial of this kind with the settings' defaults (weight 1, seeds 4, one ship, the fight defaults, no escorts).
    pub fn new(kind: Trial) -> TrialSpec {
        TrialSpec {
            preset: None,
            kind,
            weight: 1.0,
            seeds: DEFAULT_SEEDS,
            count: 1,
            start_distance: DEFAULT_START_DISTANCE,
            time_limit: DEFAULT_TIME_LIMIT,
            escorts: Vec::new(),
        }
    }

    /// A run's `fitness` before GDR-0092 step 5, as the one trial it is.
    pub fn of_fitness(f: &Fitness) -> TrialSpec {
        match f {
            Fitness::ArenaPool { opponents, seeds, hall_of_fame, count, start_distance, time_limit } => TrialSpec {
                seeds: *seeds,
                count: *count,
                start_distance: *start_distance,
                time_limit: *time_limit,
                ..TrialSpec::new(Trial::EnemyShips { opponents: opponents.clone(), hall_of_fame: *hall_of_fame })
            },
            Fitness::Voyages { ids, seeds, minutes, pilot } => TrialSpec {
                seeds: *seeds,
                ..TrialSpec::new(Trial::Voyages { ids: ids.clone(), minutes: *minutes, pilot: *pilot })
            },
        }
    }

    /// Fights (some or all of its jobs): the archive's `EngageDistance` measure is taken from fights.
    pub fn fights(&self) -> bool {
        !matches!(self.kind, Trial::Voyages { .. })
    }

    /// The numbers are in range and the lists not empty (references are checked when the run builds its jobs).
    pub(crate) fn check(&self) -> Result<(), String> {
        if !(self.weight.is_finite() && (0.0..=MAX_WEIGHT).contains(&self.weight)) {
            return Err(format!("a weight of {}: 0 to {MAX_WEIGHT}", self.weight));
        }
        if !(1..=super::config::MAX_SEEDS).contains(&self.seeds) {
            return Err(format!("`seeds` must be 1 to {}", super::config::MAX_SEEDS));
        }
        if self.count == 0 {
            return Err("`count` must be at least 1".into());
        }
        let empty = |what: &str| Err(format!("{what} is empty"));
        match &self.kind {
            Trial::EnemyShips { opponents, .. } if opponents.is_empty() => empty("the opponents' list"),
            Trial::Encounters { voyages } if voyages.is_empty() => empty("the voyages' list"),
            Trial::Gauntlet { waves } if waves.is_empty() || waves.iter().any(Vec::is_empty) => empty("a wave"),
            Trial::Fleet { designs } | Trial::Champions { designs } if designs.is_empty() => empty("the designs' list"),
            Trial::Voyages { ids, .. } if ids.is_empty() => empty("the voyages' list"),
            Trial::Waves { target: 0 } => Err("`target` must be at least 1".into()),
            Trial::HallOfFame { count: 0 } => Err("`count` must be at least 1".into()),
            _ => Ok(()),
        }
    }

    /// Resolves the trial against the content: the encounters of its voyages, the enemy ships the waves draw from,
    /// the voyages it flies (checked). What it fights is then fixed for the run.
    pub fn resolve(&self, content: &Content) -> Result<Box<dyn TrialKind>, RunError> {
        let bad = |m: String| RunError::Fitness(m);
        let fights = Fights {
            seeds: self.seeds,
            count: self.count,
            start_distance: self.start_distance,
            time_limit: self.time_limit,
            escorts: self.escorts.clone(),
        };
        let each = |list: &[Entry]| list.iter().map(|e| (vec![e.clone()], Vec::new())).collect::<Vec<_>>();
        Ok(match &self.kind {
            Trial::EnemyShips { opponents, hall_of_fame } => {
                Box::new(Opponents { fights, sides: each(opponents), hall_of_fame: *hall_of_fame })
            }
            Trial::Fleet { designs } | Trial::Champions { designs } => {
                Box::new(Opponents { fights, sides: each(designs), hall_of_fame: 0 })
            }
            Trial::Encounters { voyages } => {
                let sides = encounters(content, voyages).map_err(bad)?;
                Box::new(Opponents { fights, sides: sides.into_iter().map(|(_, s)| (s, Vec::new())).collect(), hall_of_fame: 0 })
            }
            Trial::Gauntlet { waves } => {
                let rest = waves[1..].iter().map(|w| Side { entries: w.clone() }).collect();
                Box::new(Opponents { fights, sides: vec![(waves[0].clone(), rest)], hall_of_fame: 0 })
            }
            Trial::Waves { target } => {
                let enemies = sim::voyage::waves::wave_enemies(content);
                if enemies.is_empty() {
                    return Err(bad("no content ship is tagged `enemy`: the waves have nothing to send".into()));
                }
                Box::new(Waves { fights, enemies, target: *target })
            }
            Trial::BeatYourStart => Box::new(Starts { fights }),
            Trial::HallOfFame { count } => Box::new(Hall { fights, count: *count }),
            Trial::Voyages { ids, minutes, pilot } => {
                if let Some(v) = ids.iter().find(|v| !content.voyages.contains(v)) {
                    return Err(bad(format!("no voyage `{v}` in the content")));
                }
                Box::new(Voyages {
                    ids: ids.clone(),
                    seeds: self.seeds,
                    minutes: *minutes,
                    pilot: *pilot,
                    escorts: self.escorts.iter().map(|e| e.design.clone()).collect(),
                })
            }
        })
    }
}

/// What a `lab.ron` preset needs filled in when a run is made (the game and the CLI fill it): the voyages open to the
/// career, the player's designs for YOUR FLEET (FLEET's escorts first), the best designs of earlier runs for CHAMPIONS
/// (best first), and FLEET's escorts for WITH YOUR ESCORTS; designs inlined as fight entries.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    pub open_voyages: Vec<String>,
    pub fleet: Vec<Entry>,
    pub champions: Vec<Entry>,
    pub escorts: Vec<Entry>,
}

impl TrialSpec {
    /// A run's trial from a `lab.ron` preset, its weight and seeds as given, its kind filled in from `src` (the
    /// escorts beside the contestant when `escorts`). Refused, in words, when the preset has nothing to fight: no open
    /// voyage, no design, no earlier run.
    pub fn from_preset(p: &data::TrialPreset, weight: f32, seeds: u32, escorts: bool, src: &Sources) -> Result<TrialSpec, String> {
        use data::TrialPresetKind as K;
        let ships = |list: &[data::LabShips]| list.iter().map(|s| Entry { count: s.count, ..Entry::ship(&s.ship) }).collect::<Vec<_>>();
        let none = |what: &str| Err(format!("{}: {what}", p.name));
        let kind = match &p.kind {
            K::Encounters if src.open_voyages.is_empty() => return none("no voyage is open"),
            K::Encounters => Trial::Encounters { voyages: src.open_voyages.clone() },
            K::EnemyShips { ships: list } => Trial::EnemyShips { opponents: ships(list), hall_of_fame: 0 },
            K::Waves { target } => Trial::Waves { target: *target },
            K::Gauntlet { waves } => Trial::Gauntlet { waves: waves.iter().map(|w| ships(w)).collect() },
            K::BeatYourStart => Trial::BeatYourStart,
            K::Fleet if src.fleet.is_empty() => return none("no saved design to fight"),
            K::Fleet => Trial::Fleet { designs: src.fleet.clone() },
            K::HallOfFame { count } => Trial::HallOfFame { count: *count },
            K::Champions { .. } if src.champions.is_empty() => return none("no earlier run has a best design yet"),
            K::Champions { count } => Trial::Champions { designs: src.champions.iter().take(*count as usize).cloned().collect() },
            K::Voyages if src.open_voyages.is_empty() => return none("no voyage is open"),
            K::Voyages => Trial::Voyages {
                ids: src.open_voyages.clone(),
                minutes: p.time_limit.unwrap_or(DEFAULT_VOYAGE_MINUTES),
                pilot: VoyagePilot::Behavior,
            },
        };
        let voyage = matches!(kind, Trial::Voyages { .. });
        Ok(TrialSpec {
            preset: Some(p.id.clone()),
            weight,
            seeds,
            time_limit: if voyage { DEFAULT_TIME_LIMIT } else { p.time_limit.unwrap_or(DEFAULT_TIME_LIMIT) },
            escorts: if escorts { src.escorts.clone() } else { Vec::new() },
            ..TrialSpec::new(kind)
        })
    }
}

/// THE ENEMIES YOU MEET: every encounter of these voyages that has ships, each once in the order its voyages' sites
/// list them, with its ships at their largest counts (content ids, under their own behavior). Unknown voyages are an
/// error, and so is no encounter with ships at all.
pub fn encounters(content: &Content, voyages: &[String]) -> Result<Vec<(String, Vec<Entry>)>, String> {
    let mut out: Vec<(String, Vec<Entry>)> = Vec::new();
    for id in voyages {
        let v = content.voyages.get(id).ok_or_else(|| format!("no voyage `{id}` in the content"))?;
        for site in &v.sites {
            if out.iter().any(|(e, _)| *e == site.encounter) {
                continue;
            }
            let Some(enc) = content.encounters.get(&site.encounter) else { continue };
            let entries: Vec<Entry> =
                enc.ships.iter().filter(|s| s.count.1 > 0).map(|s| Entry { count: s.count.1, ..Entry::ship(&s.ship) }).collect();
            if !entries.is_empty() {
                out.push((enc.id.clone(), entries));
            }
        }
    }
    if out.is_empty() {
        return Err("the voyages hold no encounter with ships".into());
    }
    Ok(out)
}

/// A design and behavior as a fight entry of one ship in a line (a player design or a genome, inlined).
pub fn entry_of(design: DesignRef, behavior: Option<BehaviorRef>) -> Entry {
    Entry { design, behavior, count: 1, formation: Formation::Line }
}

fn one() -> u32 {
    1
}
fn one_f() -> f32 {
    1.0
}
fn default_seeds() -> u32 {
    DEFAULT_SEEDS
}
fn default_waves() -> u32 {
    DEFAULT_WAVES
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
