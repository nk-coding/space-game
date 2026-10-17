//! The evolution lab's content (`lab.ron`, GDR-0092 "Content and the wiki"): the goals a run's fitness weighs, their
//! one-click presets, the genes a run may change and their presets, the trial kinds and the trial presets the setup
//! offers, the stand-in pilots that fly a design in its fights, the size presets, the archive's measures and the
//! spreads over them, and the time a frame gives a run.
//! What a goal, a trial kind or a measure computes is the arena's (`arena::evolve`), how a stand-in flies the sim's
//! (`sim::flight::stand_in`); their words and numbers are here, so a pack can retune them. A run keeps everything it
//! was made with (resolved), so a change here changes no run. The mutation operators' weights stay the arena's: a run
//! breeds by them and its file doesn't carry them. The wiki generates the lab's page from this file and references its
//! entries as `[[lab:<list>.<id>]]` (`docs/wiki.md`); its texts are keyed `lab.<list>.<id>.<field>`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Source, StatDef};
use crate::content::table::Table;

/// The goals a run can weigh, by id: what the arena computes per job (`arena::evolve::GoalScores`).
pub const GOALS: [&str; 4] = ["win", "unhurt", "quick", "cheap"];
/// The genes a run may change (GDR-0092 "MAY CHANGE"), by id: what the arena's operators edit (`arena::evolve::Gene`,
/// in this order).
pub const GENES: [&str; 8] = ["layout", "parts", "armour", "power", "numbers", "rules", "assist_numbers", "assist_rules"];
/// The trial kinds the arena fights (`arena::evolve::Trial`), by id ([`TrialPresetKind::id`]).
pub const TRIAL_KINDS: [&str; 11] = ["encounters", "enemy_ships", "waves", "gauntlet", "beat_your_start", "fleet", "hall_of_fame", "champions", "voyages", "ladder", "co_evolution"];
/// What the archive measures a genome by (`arena::evolve::Measure`), by id.
pub const MEASURES: [&str; 10] = ["top_speed", "acceleration", "weapon_reach", "build_points", "mass", "firepower", "hit_points", "parts", "rules", "engage_distance"];
/// The archive's spreads (SPREAD BY): over what a design is, for runs whose design may change, else over how it fights.
pub const SPREADS: [&str; 2] = ["design", "behavior"];
/// The largest weight a goal or a trial may have (the setup's steppers run 0 to this).
pub const MAX_LAB_WEIGHT: f32 = 3.0;
/// Bounds of a preset's numbers: seeds per opponent, ships of an entry, waves of a target, the run's best fought.
pub const MAX_PRESET_SEEDS: u32 = 64;
pub const MAX_PRESET_SHIPS: u32 = 64;
pub const MAX_PRESET_WAVES: u32 = 50;
pub const MAX_PRESET_BEST: u32 = 16;
/// The longest stretch a stand-in holds or leaves the stick or the triggers (s).
pub const MAX_DUTY_SECONDS: f32 = 60.0;

/// `lab.ron`: one per pack (a later pack's entry of the same id replaces an earlier one, in its place).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lab {
    #[serde(default)]
    pub goals: Vec<LabGoal>,
    #[serde(default)]
    pub goal_presets: Vec<GoalPreset>,
    #[serde(default)]
    pub trials: Vec<TrialPreset>,
    /// The genes' words (MAY CHANGE's pills) and the one-click presets over them, the first the default.
    #[serde(default)]
    pub genes: Vec<LabGene>,
    #[serde(default)]
    pub gene_presets: Vec<GenePreset>,
    /// The stand-in pilots, the first the default.
    #[serde(default)]
    pub pilots: Vec<LabPilot>,
    /// What an enemy faction may build with in an arms race besides the parts its ships use (GDR-0092
    /// "Co-evolution": "`lab.ron` may widen a faction's palette").
    #[serde(default)]
    pub palettes: Vec<EnemyPalette>,
    /// The trial kinds' words (a run's trial without a preset shows its kind's name).
    #[serde(default)]
    pub kinds: Vec<LabKind>,
    /// The size presets (SIZE), one of them the default.
    #[serde(default)]
    pub sizes: Vec<LabSize>,
    /// The archive's measures: their words.
    #[serde(default)]
    pub measures: Vec<LabMeasure>,
    /// The archive's grids, one per [`SPREADS`].
    #[serde(default)]
    pub spreads: Vec<LabSpread>,
    /// The time a frame gives the run that computes (a later pack's replaces it).
    #[serde(default)]
    pub budget: Option<LabBudget>,
    /// Where it was read from (the last pack that has one).
    #[serde(skip_deserializing)]
    pub source: Source,
}

/// A goal (GDR-0092 "GOALS"): one of [`GOALS`], its words and its weight in a new run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabGoal {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub weight: f32,
}

/// A preset of goal weights (the pills WIN, WIN UNHURT, ...): goals left out weigh 0.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPreset {
    pub id: String,
    pub name: String,
    pub weights: BTreeMap<String, f32>,
}

/// A gene (GDR-0092 "MAY CHANGE"): one of [`GENES`] and its words.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabGene {
    pub id: String,
    pub name: String,
    pub summary: String,
}

/// A preset of what may change (the pills EVERYTHING, DESIGN ONLY, ...): the genes on, the others off.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenePreset {
    pub id: String,
    pub name: String,
    pub genes: Vec<String>,
}

/// A trial preset (GDR-0092 "Trials"): a kind with its data, the words the setup shows, its weight and seeds in a new
/// run, and whether a new run has it on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrialPreset {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub kind: TrialPresetKind,
    #[serde(default = "one")]
    pub weight: f32,
    #[serde(default = "four")]
    pub seeds: u32,
    /// Seconds a fight lasts at most (minutes for a voyage: [`TrialPresetKind::Voyages`]); left out: the arena's.
    #[serde(default)]
    pub time_limit: Option<f32>,
    /// In the setup's default selection.
    #[serde(default)]
    pub default: bool,
}

/// What a trial preset fights. The setup fills in what comes from the career and the lab (the open voyages, the
/// player's designs, earlier runs) when a run is made, so the run is self-contained.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrialPresetKind {
    /// THE ENEMIES YOU MEET: every encounter of the voyages open to the career, its ships at their largest counts.
    Encounters,
    /// ENEMY SHIPS: these content ships, each entry a fight.
    EnemyShips { ships: Vec<LabShips> },
    /// TEST-RANGE WAVES: waves as the test range sends them, sized from the design, one after the other; `target`
    /// waves cleared is a win.
    Waves { target: u32 },
    /// A gauntlet: these waves one after the other, each when the one before is out.
    Gauntlet { waves: Vec<Vec<LabShips>> },
    /// BEAT YOUR START: every start of the run under its own behavior.
    BeatYourStart,
    /// YOUR FLEET: the player's saved designs (FLEET's escorts preselected).
    Fleet,
    /// HALL OF FAME: the run's own best `count` so far.
    HallOfFame { count: u32 },
    /// CHAMPIONS: the best `count` designs of the lab's earlier runs.
    Champions { count: u32 },
    /// VOYAGES: the open voyages flown headless.
    Voyages,
    /// THE LADDER: fixed rungs of opponents, the weakest first, each a fight per seed; also a design's rating (the sum
    /// of its rungs' wins), shown on YOUR DESIGNS (GDR-0092 step 9).
    Ladder { rungs: Vec<Vec<LabShips>> },
    /// CO-EVOLUTION, the ARMS RACE: an enemy population evolves against yours in the run. `size` is its children per
    /// generation as a share of yours; your children fight its best `elites` and `hall` of its past champions, its
    /// children your best `elites`, `hall` of yours and (`anchor`) your starts; every `check_every` generations (0:
    /// never) both bests fight the other's champions of every `champions_every`th generation (PAST CHAMPIONS);
    /// `mirror` keeps its designs symmetric. Its starts are an enemy faction's ships, picked in the setup (its bosses
    /// only when picked).
    CoEvolution {
        size: f32,
        elites: u32,
        hall: u32,
        #[serde(default = "yes")]
        anchor: bool,
        check_every: u32,
        champions_every: u32,
        #[serde(default)]
        mirror: bool,
    },
}

/// A trial kind (one of [`TRIAL_KINDS`]) and its words.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabKind {
    pub id: String,
    pub name: String,
    pub summary: String,
}

/// A size preset (QUICK, STANDARD, DEEP): children per generation and generations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabSize {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub population: u32,
    pub generations: u32,
    /// A new run's size.
    #[serde(default)]
    pub default: bool,
}

/// A measure of the archive (one of [`MEASURES`]): its name and unit are a stat's (`stats.ron`) where one says the
/// same, else its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabMeasure {
    pub id: String,
    #[serde(default)]
    pub stat: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    pub summary: String,
}

/// A spread (one of [`SPREADS`]): the archive's grid, one to three axes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabSpread {
    pub id: String,
    pub axes: Vec<LabAxis>,
}

/// An axis of the archive: `cells` equal bins of a measure from `min` to `max` (values outside go to the end bins).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabAxis {
    pub measure: String,
    pub min: f32,
    pub max: f32,
    pub cells: u32,
}

/// The time a frame gives the run that computes (ms): with the EVOLUTION layer open, and anywhere else at home and
/// on the screens (in flight it pauses).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabBudget {
    pub layer_open: f32,
    pub elsewhere: f32,
}

impl TrialPresetKind {
    /// The kind's id (one of [`TRIAL_KINDS`]).
    pub fn id(&self) -> &'static str {
        match self {
            TrialPresetKind::Encounters => "encounters",
            TrialPresetKind::EnemyShips { .. } => "enemy_ships",
            TrialPresetKind::Waves { .. } => "waves",
            TrialPresetKind::Gauntlet { .. } => "gauntlet",
            TrialPresetKind::BeatYourStart => "beat_your_start",
            TrialPresetKind::Fleet => "fleet",
            TrialPresetKind::HallOfFame { .. } => "hall_of_fame",
            TrialPresetKind::Champions { .. } => "champions",
            TrialPresetKind::Voyages => "voyages",
            TrialPresetKind::Ladder { .. } => "ladder",
            TrialPresetKind::CoEvolution { .. } => "co_evolution",
        }
    }
}

/// The parts an enemy faction may build with in an arms race besides those its ships use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyPalette {
    pub faction: String,
    pub parts: Vec<String>,
}

/// A stand-in pilot (GDR-0092 "The stand-in pilot"): it flies the design in the fights FLOWN BY a stand-in, so its
/// pilot assists are scored (they act only on a piloted ship) and fill the channels it leaves idle, as for the player.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabPilot {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub flies: StandIn,
}

/// How a stand-in flies, every think: what a run carries of its pilot (`sim::flight::stand_in`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandIn {
    /// It locks the nearest enemy itself (LOCK TARGET's order) while it holds no lock; otherwise the lock is left to
    /// the assists (an auto-lock's pick is taken as a player's client takes it).
    #[serde(default)]
    pub lock: bool,
    /// It holds the stick towards the enemy while that one is farther than this share of its longest gun's reach.
    pub close_to: f32,
    /// When it holds the stick: none, never (it leaves thrust to the assists).
    #[serde(default)]
    pub stick: Option<Duty>,
    /// When it holds every trigger: none, never (it leaves firing to the assists).
    #[serde(default)]
    pub fire: Option<Duty>,
}

/// On for `on` seconds, then off for `off` (0: always on), from the fight's start.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Duty {
    pub on: f32,
    #[serde(default)]
    pub off: f32,
}

/// `count` ships of a content ship.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabShips {
    pub ship: String,
    #[serde(default = "one_ship")]
    pub count: u32,
}

fn one() -> f32 {
    1.0
}

fn four() -> u32 {
    4
}

fn one_ship() -> u32 {
    1
}

fn yes() -> bool {
    true
}

impl Lab {
    pub fn goal(&self, id: &str) -> Option<&LabGoal> {
        self.goals.iter().find(|g| g.id == id)
    }

    pub fn trial(&self, id: &str) -> Option<&TrialPreset> {
        self.trials.iter().find(|t| t.id == id)
    }

    pub fn gene(&self, id: &str) -> Option<&LabGene> {
        self.genes.iter().find(|g| g.id == id)
    }

    pub fn pilot(&self, id: &str) -> Option<&LabPilot> {
        self.pilots.iter().find(|p| p.id == id)
    }

    pub fn kind(&self, id: &str) -> Option<&LabKind> {
        self.kinds.iter().find(|k| k.id == id)
    }

    pub fn measure(&self, id: &str) -> Option<&LabMeasure> {
        self.measures.iter().find(|m| m.id == id)
    }

    pub fn spread(&self, id: &str) -> Option<&LabSpread> {
        self.spreads.iter().find(|s| s.id == id)
    }

    /// A measure's name and unit: its stat's (`stats`) or its own; none for a measure the file lacks.
    pub fn measure_words<'a>(&'a self, stats: &'a Table<StatDef>, id: &str) -> Option<(&'a str, &'a str)> {
        let m = self.measure(id)?;
        match m.stat.as_deref().and_then(|s| stats.get(s)) {
            Some(st) => Some((st.name.as_str(), st.unit.as_str())),
            None => Some((m.name.as_deref()?, m.unit.as_deref().unwrap_or(""))),
        }
    }

    /// A new run's size: the preset marked `default`, else the first.
    pub fn default_size(&self) -> Option<&LabSize> {
        self.sizes.iter().find(|s| s.default).or(self.sizes.first())
    }

    /// Every text with its key, `lab.<list>.<id>.<field>` (`lab.goal.win.name`), in file order.
    pub fn texts<'a>(&'a self) -> Vec<(String, &'a str)> {
        let mut out = Vec::new();
        let mut put = |list: &str, id: &str, fields: &[(&str, &'a str)]| {
            for (field, text) in fields {
                out.push((format!("lab.{list}.{id}.{field}"), *text));
            }
        };
        for g in &self.goals {
            put("goal", &g.id, &[("name", &g.name), ("summary", &g.summary)]);
        }
        for p in &self.goal_presets {
            put("goal_preset", &p.id, &[("name", &p.name)]);
        }
        for k in &self.kinds {
            put("kind", &k.id, &[("name", &k.name), ("summary", &k.summary)]);
        }
        for t in &self.trials {
            put("trial", &t.id, &[("name", &t.name), ("summary", &t.summary)]);
        }
        for g in &self.genes {
            put("gene", &g.id, &[("name", &g.name), ("summary", &g.summary)]);
        }
        for p in &self.gene_presets {
            put("gene_preset", &p.id, &[("name", &p.name)]);
        }
        for p in &self.pilots {
            put("pilot", &p.id, &[("name", &p.name), ("summary", &p.summary)]);
        }
        for s in &self.sizes {
            put("size", &s.id, &[("name", &s.name), ("summary", &s.summary)]);
        }
        for m in &self.measures {
            let mut fields = vec![("summary", m.summary.as_str())];
            fields.extend(m.name.as_deref().map(|n| ("name", n)));
            fields.extend(m.unit.as_deref().map(|u| ("unit", u)));
            put("measure", &m.id, &fields);
        }
        out
    }

    /// The parts `faction` may build with in an arms race besides its ships' own (none listed: none).
    pub fn palette(&self, faction: &str) -> &[String] {
        self.palettes.iter().find(|p| p.faction == faction).map_or(&[], |p| &p.parts)
    }

    /// Ids listed twice in this file: ("goal", id), ("goal_preset", id), ("trial", id), ("gene", id), ("gene_preset",
    /// id), ("pilot", id), ("palette", faction), ("kind", id), ("size", id), ("measure", id) or ("spread", id).
    pub(crate) fn repeated(&self) -> Vec<(&'static str, String)> {
        fn twice<'a>(what: &'static str, ids: impl Iterator<Item = &'a String>, out: &mut Vec<(&'static str, String)>) {
            let mut seen = std::collections::BTreeSet::new();
            for id in ids {
                if !seen.insert(id) {
                    out.push((what, id.clone()));
                }
            }
        }
        let mut out = Vec::new();
        twice("goal", self.goals.iter().map(|g| &g.id), &mut out);
        twice("goal_preset", self.goal_presets.iter().map(|g| &g.id), &mut out);
        twice("trial", self.trials.iter().map(|t| &t.id), &mut out);
        twice("gene", self.genes.iter().map(|g| &g.id), &mut out);
        twice("gene_preset", self.gene_presets.iter().map(|g| &g.id), &mut out);
        twice("pilot", self.pilots.iter().map(|p| &p.id), &mut out);
        twice("palette", self.palettes.iter().map(|p| &p.faction), &mut out);
        twice("kind", self.kinds.iter().map(|k| &k.id), &mut out);
        twice("size", self.sizes.iter().map(|s| &s.id), &mut out);
        twice("measure", self.measures.iter().map(|m| &m.id), &mut out);
        twice("spread", self.spreads.iter().map(|s| &s.id), &mut out);
        out
    }

    /// A later pack's lab merged in: an entry of the same id replaces the earlier one in its place, a new one goes
    /// at the end; its budget replaces the earlier one.
    pub(crate) fn merge(&mut self, other: &Lab) {
        fn upsert<T: Clone>(list: &mut Vec<T>, add: &[T], id: impl Fn(&T) -> &str) {
            for e in add {
                match list.iter().position(|x| id(x) == id(e)) {
                    Some(k) => list[k] = e.clone(),
                    None => list.push(e.clone()),
                }
            }
        }
        upsert(&mut self.goals, &other.goals, |g| &g.id);
        upsert(&mut self.goal_presets, &other.goal_presets, |g| &g.id);
        upsert(&mut self.trials, &other.trials, |t| &t.id);
        upsert(&mut self.genes, &other.genes, |g| &g.id);
        upsert(&mut self.gene_presets, &other.gene_presets, |g| &g.id);
        upsert(&mut self.pilots, &other.pilots, |p| &p.id);
        upsert(&mut self.palettes, &other.palettes, |p| &p.faction);
        upsert(&mut self.kinds, &other.kinds, |k| &k.id);
        upsert(&mut self.sizes, &other.sizes, |s| &s.id);
        upsert(&mut self.measures, &other.measures, |m| &m.id);
        upsert(&mut self.spreads, &other.spreads, |s| &s.id);
        if other.budget.is_some() {
            self.budget = other.budget;
        }
        self.source = other.source.clone();
    }
}
