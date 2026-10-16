//! The evolution lab's content (`lab.ron`, GDR-0092 "Content and the wiki"): the goals a run's fitness weighs, their
//! one-click presets, the genes a run may change and their presets, the trial presets the setup offers and the stand-in
//! pilots that fly a design in its fights.
//! What a goal or a trial kind computes is the arena's (`arena::evolve`), how a stand-in flies the sim's
//! (`sim::flight::stand_in`); their words and numbers are here, so a pack can retune them. A run keeps the trials and
//! the pilot it was made with (resolved), so a change here changes no run. GDR-0092 step 8 completes the file (size
//! presets, measures, operator weights, budgets) and gives it a wiki page.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Source;

/// The goals a run can weigh, by id: what the arena computes per job (`arena::evolve::GoalScores`).
pub const GOALS: [&str; 4] = ["win", "unhurt", "quick", "cheap"];
/// The genes a run may change (GDR-0092 "MAY CHANGE"), by id: what the arena's operators edit (`arena::evolve::Gene`,
/// in this order).
pub const GENES: [&str; 8] = ["layout", "parts", "armour", "power", "numbers", "rules", "assist_numbers", "assist_rules"];
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

    /// Ids listed twice in this file: ("goal", id), ("goal_preset", id), ("trial", id), ("gene", id), ("gene_preset",
    /// id) or ("pilot", id).
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
        out
    }

    /// A later pack's lab merged in: an entry of the same id replaces the earlier one in its place, a new one goes
    /// at the end.
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
        self.source = other.source.clone();
    }
}
