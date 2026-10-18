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

use std::collections::BTreeSet;

use data::{Content, Upgrades};
use serde::{Deserialize, Serialize};

use crate::spec::{BehaviorRef, DEFAULT_START_DISTANCE, DEFAULT_TIME_LIMIT, DesignRef, Entry, FightSpec, Formation, Pilot, Side};
use crate::voyage::{DEFAULT_VOYAGE_MINUTES, VoyageSpec};

use super::config::{DEFAULT_SEEDS, Population, Start, VoyagePilot};
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
    /// Who flies the contestant's first ship in its fights (a voyage has its own pilot).
    #[serde(default, skip_serializing_if = "FlownBy::is_behavior")]
    pub flown_by: FlownBy,
}

/// FLOWN BY (GDR-0092): who flies the contestant's first ship in a trial's fights. A stand-in pilot flies it as its
/// side's focus, with the design's pilot assists filling the channels it leaves idle, so the assists are scored;
/// the behavior is scored on the fights it flies.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum FlownBy {
    /// Its behavior, as before (nobody pilots).
    #[default]
    Behavior,
    /// The stand-in on every seed.
    StandIn(Pilot),
    /// The behavior on odd seeds, the stand-in on even ones (at no extra cost).
    Both(Pilot),
}

impl FlownBy {
    pub fn is_behavior(&self) -> bool {
        *self == FlownBy::Behavior
    }

    /// The pilot of the fight on `seed`, if a stand-in flies it.
    pub fn on(&self, seed: u64) -> Option<&Pilot> {
        match self {
            FlownBy::Behavior => None,
            FlownBy::StandIn(p) => Some(p),
            FlownBy::Both(p) => seed.is_multiple_of(2).then_some(p),
        }
    }
}

/// The trial kinds (GDR-0092's table). Player designs come inlined as text, content by id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Trial {
    /// ENEMY SHIPS: fixed opponents, each entry a fight per seed, and the run's best `hall_of_fame` with them in the
    /// same mean.
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
    /// VOYAGES: these voyages flown by the headless voyage runner (`docs/arena.md` "Voyage fitness").
    Voyages {
        ids: Vec<String>,
        #[serde(default = "default_minutes")]
        minutes: f32,
        #[serde(default)]
        pilot: VoyagePilot,
    },
    /// THE LADDER (GDR-0092 step 9, GDR-0083's benchmark): fixed rungs of opponents, from the weakest up, each a fight
    /// per seed; a design's rating is the sum of its rungs' WIN means ([`crate::evolve::ladder`]).
    Ladder { rungs: Vec<Vec<Entry>> },
    /// CO-EVOLUTION (the ARMS RACE): the run's enemy population (`RunConfig::enemy`), its best and past champions as
    /// the generation froze them, each in a pack sized to the contestant ([`pack`]). Its settings are the enemy's
    /// fights too, seen from side 1.
    CoEvolution,
}

impl Trial {
    /// The kind's id, as `lab.ron` names it (`data::TRIAL_KINDS`, `data::TrialPresetKind::id`).
    pub fn kind_id(&self) -> &'static str {
        match self {
            Trial::EnemyShips { .. } => "enemy_ships",
            Trial::Encounters { .. } => "encounters",
            Trial::Waves { .. } => "waves",
            Trial::Gauntlet { .. } => "gauntlet",
            Trial::BeatYourStart => "beat_your_start",
            Trial::Fleet { .. } => "fleet",
            Trial::HallOfFame { .. } => "hall_of_fame",
            Trial::Champions { .. } => "champions",
            Trial::Voyages { .. } => "voyages",
            Trial::Ladder { .. } => "ladder",
            Trial::CoEvolution => "co_evolution",
        }
    }
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
#[derive(Clone)]
pub struct Contestant {
    pub entry: Entry,
    pub build_points: f32,
}

/// What a generation's jobs fight besides the fixed opponents, frozen when it opens: the run's starts (each as an
/// entry under its own behavior), its hall of fame (the archive's best, best first) and, in an arms race, THE ENEMY's
/// frozen set (its best, then its past champions drawn); and the seeds.
pub struct GenCtx<'a> {
    pub starts: &'a [Entry],
    pub hall: &'a [Entry],
    pub enemies: &'a [Contestant],
    /// VALIDATE: this many new seeds, the ones after each trial's own (`Some(20)`: seeds n + 1 to n + 20 of a trial
    /// of n); `None`: the trial's own seeds 1 to n, as a run fights them.
    pub fresh: Option<u32>,
    /// The run's part-stat steps ([`super::RunConfig::upgrades`]): every fight's side 0 and voyage's fleet, and side 1
    /// when it is yours (your starts, hall of fame, fleet and champions).
    pub upgrades: &'a Upgrades,
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
    /// The jobs of an enemy (CO-EVOLUTION: the enemy population's child, side 1) against your side's frozen `rivals`,
    /// in a fixed order (every rival, then every seed); no other kind has any.
    fn enemy_jobs(&self, _enemy: &Contestant, _rivals: &[Contestant], _ctx: &GenCtx) -> Vec<Job> {
        Vec::new()
    }
}

/// How many ships of an enemy design of `enemy_bp` build points fight a side of `side_bp` (GDR-0092 "Fair pairing"):
/// the test range's wave rule with that one design (`sim::voyage::waves::wave_mix`), so a bigger enemy comes in fewer
/// copies; at least one.
pub fn pack(side_bp: f32, enemy_bp: f32) -> u32 {
    sim::voyage::waves::wave_mix(side_bp, &[enemy_bp])[0].max(1)
}

/// The settings every fight of a trial shares.
#[derive(Clone)]
struct Fights {
    seeds: u32,
    count: u32,
    start_distance: f32,
    time_limit: f32,
    escorts: Vec<Entry>,
    flown_by: FlownBy,
}

impl Fights {
    /// The fights of `me` against one opponent (and its waves, a gauntlet), one per seed, each flown as `flown_by`
    /// says.
    /// `yours`: the opponent is one of your designs (built with the run's upgrades too).
    fn against(&self, me: &Contestant, ctx: &GenCtx, side: &[Entry], waves: &[Side], yours: bool, jobs: &mut Vec<Job>) {
        self.fight(me, ctx, (side, yours), waves, 0, jobs);
    }

    /// The fights of `ours` (side 0, with the escorts beside it, built with the run's upgrades) against `theirs` (with
    /// them when it is yours), one per seed; the candidate is side `candidate` (1: an enemy of the arms race, scored
    /// as side 1).
    fn fight(&self, ours: &Contestant, ctx: &GenCtx, theirs: (&[Entry], bool), waves: &[Side], candidate: u32, jobs: &mut Vec<Job>) {
        let (theirs, yours) = theirs;
        let mut side = vec![Entry { count: self.count, ..ours.entry.clone() }];
        side.extend(self.escorts.iter().cloned());
        for seed in seeds(self.seeds, ctx) {
            side[0].pilot = self.flown_by.on(seed).cloned();
            let mut spec = FightSpec::new(seed, side.clone(), theirs.to_vec());
            spec.sides[0].upgrades = ctx.upgrades.clone();
            if yours {
                spec.sides[1].upgrades = ctx.upgrades.clone();
            }
            spec.start_distance = self.start_distance;
            spec.time_limit = self.time_limit;
            spec.waves = waves.to_vec();
            jobs.push(Job::Fight { candidate: 0, side: candidate, spec });
        }
    }

    /// An enemy design as side 1 against `ours`: in a pack sized to `ours`' build points (the contestant's copies).
    fn pack_of(&self, ours: &Contestant, enemy: &Contestant) -> Entry {
        Entry { count: pack(ours.build_points * self.count as f32, enemy.build_points), ..enemy.entry.clone() }
    }
}

/// Fixed opponents, each a fight (ENEMY SHIPS with its hall of fame, THE ENEMIES YOU MEET, YOUR FLEET, CHAMPIONS, THE
/// LADDER's rungs, a gauntlet's first wave with the rest after it).
struct Opponents {
    fights: Fights,
    sides: Vec<(Vec<Entry>, Vec<Side>)>,
    /// The sides are your designs (YOUR FLEET, CHAMPIONS).
    yours: bool,
    hall_of_fame: u32,
}

impl TrialKind for Opponents {
    fn jobs(&self, me: &Contestant, ctx: &GenCtx) -> Vec<Job> {
        let mut jobs = Vec::new();
        for (side, waves) in &self.sides {
            self.fights.against(me, ctx, side, waves, self.yours, &mut jobs);
        }
        for o in ctx.hall.iter().take(self.hall_of_fame as usize) {
            self.fights.against(me, ctx, std::slice::from_ref(o), &[], true, &mut jobs);
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
        let rest = vec![Side::new(entries.clone()); self.target.saturating_sub(1) as usize];
        let mut jobs = Vec::new();
        self.fights.against(me, ctx, &entries, &rest, false, &mut jobs);
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
            self.fights.against(me, ctx, std::slice::from_ref(s), &[], true, &mut jobs);
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
            self.fights.against(me, ctx, std::slice::from_ref(o), &[], true, &mut jobs);
        }
        jobs
    }

    fn hall_of_fame(&self) -> u32 {
        self.count
    }
}

/// CO-EVOLUTION: THE ENEMY's frozen set, and from its side your frozen set.
struct CoEvolution {
    fights: Fights,
}

impl TrialKind for CoEvolution {
    fn jobs(&self, me: &Contestant, ctx: &GenCtx) -> Vec<Job> {
        let mut jobs = Vec::new();
        for e in ctx.enemies {
            self.fights.against(me, ctx, &[self.fights.pack_of(me, e)], &[], false, &mut jobs);
        }
        jobs
    }

    fn enemy_jobs(&self, enemy: &Contestant, rivals: &[Contestant], ctx: &GenCtx) -> Vec<Job> {
        let mut jobs = Vec::new();
        for r in rivals {
            self.fights.fight(r, ctx, (&[self.fights.pack_of(r, enemy)], false), &[], 1, &mut jobs);
        }
        jobs
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
                    upgrades: ctx.upgrades.clone(),
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
            flown_by: FlownBy::Behavior,
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
            Trial::Ladder { rungs } if rungs.is_empty() || rungs.iter().any(Vec::is_empty) => empty("a rung"),
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
            flown_by: self.flown_by.clone(),
        };
        let each = |list: &[Entry]| list.iter().map(|e| (vec![e.clone()], Vec::new())).collect::<Vec<_>>();
        Ok(match &self.kind {
            Trial::EnemyShips { opponents, hall_of_fame } => {
                Box::new(Opponents { fights, sides: each(opponents), yours: false, hall_of_fame: *hall_of_fame })
            }
            Trial::Fleet { designs } | Trial::Champions { designs } => {
                Box::new(Opponents { fights, sides: each(designs), yours: true, hall_of_fame: 0 })
            }
            Trial::Encounters { voyages } => {
                let sides = encounters(content, voyages).map_err(bad)?;
                Box::new(Opponents { fights, sides: sides.into_iter().map(|(_, s)| (s, Vec::new())).collect(), yours: false, hall_of_fame: 0 })
            }
            Trial::Ladder { rungs } => {
                Box::new(Opponents { fights, sides: rungs.iter().map(|r| (r.clone(), Vec::new())).collect(), yours: false, hall_of_fame: 0 })
            }
            Trial::Gauntlet { waves } => {
                let rest = waves[1..].iter().map(|w| Side::new(w.clone())).collect();
                Box::new(Opponents { fights, sides: vec![(waves[0].clone(), rest)], yours: false, hall_of_fame: 0 })
            }
            Trial::Waves { target } => {
                let enemies = sim::voyage::waves::wave_enemies(content);
                if enemies.is_empty() {
                    return Err(bad("no content ship is tagged `enemy`: the waves have nothing to send".into()));
                }
                Box::new(Waves { fights, enemies, target: *target })
            }
            Trial::BeatYourStart => Box::new(Starts { fights }),
            Trial::CoEvolution => Box::new(CoEvolution { fights }),
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
            K::Ladder { rungs } => Trial::Ladder { rungs: rungs.iter().map(|r| ships(r)).collect() },
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
            K::CoEvolution { .. } => Trial::CoEvolution,
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

/// The ships tagged as a voyage's bosses (the guardians): an arms race starts from them only when they are picked.
pub const BOSS_TAG: &str = "boss";

/// The ships an ARMS RACE starts THE ENEMY from by default (GDR-0092 "Co-evolution", lead default): the content ships
/// tagged `enemy` of the first faction that has any, its bosses and stations (GDR-0110: they cannot fly) left out, in
/// content order (at most four).
pub fn enemy_starts(content: &Content) -> Vec<String> {
    let enemy = |s: &&data::Blueprint| s.tags.iter().any(|t| t == sim::voyage::waves::ENEMY_TAG);
    let Some(faction) = content.ships.iter().find(enemy).map(|s| s.faction.clone()) else { return Vec::new() };
    content
        .ships
        .iter()
        .filter(|s| enemy(s) && s.faction == faction && !s.anchored && !s.tags.iter().any(|t| t == BOSS_TAG))
        .take(super::config::MAX_STARTS)
        .map(|s| s.id.clone())
        .collect()
}

impl Population {
    /// THE ENEMY of a `lab.ron` ARMS RACE preset (`CoEvolution`): `ships` as its starts (content ships of one enemy
    /// faction, under their own behaviors), the faction's `lab.ron` palette, `size` × `yours` children (at least its
    /// starts) and the preset's pairing. Refused in words without a ship, with ships of two factions or with more
    /// than four.
    pub fn from_preset(content: &Content, p: &data::TrialPreset, ships: &[String], yours: u32) -> Result<Population, String> {
        let data::TrialPresetKind::CoEvolution { size, elites, hall, anchor, check_every, champions_every, mirror } = p.kind else {
            return Err(format!("{}: no arms race", p.name));
        };
        let factions: BTreeSet<&str> = ships.iter().filter_map(|id| content.ships.get(id)).map(|s| s.faction.as_str()).collect();
        let faction = match (factions.len(), ships.len()) {
            (_, 0) => return Err(format!("{}: pick an enemy ship to start the enemy from", p.name)),
            (1, n) if n <= super::config::MAX_STARTS => factions.into_iter().next().expect("one faction"),
            (1, _) => return Err(format!("{}: at most {} enemy ships", p.name, super::config::MAX_STARTS)),
            _ => return Err(format!("{}: the enemy's ships are of one faction", p.name)),
        };
        Ok(Population {
            starts: ships.iter().map(|id| Start::Design { design: DesignRef::Ship(id.clone()), behavior: None, locks: Default::default() }).collect(),
            evolve: super::config::enemy_genes(),
            population: ((yours as f32 * size).round() as u32).max(ships.len() as u32),
            frame: None,
            palette: content.lab.palette(faction).to_vec(),
            mirror,
            archive: Default::default(),
            elites,
            hall,
            anchor,
            check_every,
            champions_every,
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
    Entry { design, behavior, count: 1, formation: Formation::Line, pilot: None }
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
