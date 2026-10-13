//! The career: the player's progression as a pure model (GDR-0062, GDR-0063, ADR-0033), as [`crate::edit`] is
//! for the drydock. No Bevy, no I/O, wasm-safe. The rules are documented in `docs/career.md`.
//!
//! A [`Career`] holds the save's **facts** only: the start, the game seed, the researched nodes, the research data
//! and materials in store, the escorts, a record per voyage, the voyage underway, a small log and the last
//! voyage's haul. Everything else is derived from the content on demand ([`Career::unlocks`],
//! [`Career::mind_capacity`], [`Career::open_voyages`], [`Career::can_pay`]), so a rebalance of the content
//! applies to old saves without a migration. The game stores it as RON ([`Career::to_ron`],
//! [`Career::from_ron`]) in its `saves/` namespace and calls [`Career::reconcile`] after loading, which drops what
//! the current content no longer has.
//!
//! ```
//! use data::career::{Career, Haul, HaulItem, VoyageEnd};
//! let content = data::base_content();
//! let mut career = Career::new(&content, "rim_awakening", 4207).unwrap();
//! let seed = career.voyage_launched(&content, "yard_approach").unwrap(); // the sim rolls the voyage from it
//! let haul: Haul = [(HaulItem::Part { part: "automaton_cannon".into(), material: None }, 4)].into_iter().collect();
//! career.voyage_ended(&content, "yard_approach", VoyageEnd::Recalled { haul, cleared: false, ticks: 9000 }).unwrap();
//! career.research(&content, "automaton_cannon").unwrap();
//! assert!(career.unlocks(&content).has_part("automaton_cannon"));
//! ```

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::kind::Kind;
use crate::load::{Content, parse_error, ron_options};
use crate::model::{Blueprint, Frame, ResearchNode, Unlock};
use crate::problem::Problem;

/// The format version of the save's RON text ([`Career::to_ron`]). The game's storage writes it into the
/// document header (ADR-0031). Raise it when the meaning of a field changes, and migrate older formats when
/// reading; new fields with a default need no new format.
pub const FORMAT: u32 = 1;

/// The version of the voyage roller a new career uses (ADR-0024: old versions must stay reproducible). It covers
/// [`Career::voyage_seed`] too: changing how voyage seeds are made needs a new version.
pub const GENERATOR: u32 = 1;

/// Most research data of one family a store holds (a sanity bound, placeholder): a hundred times the largest cost
/// content may ask for ([`crate::validate::MAX_RESEARCH_DATA`]).
pub const MAX_DATA: f32 = 1_000_000.0;

/// Most of one material a store holds (a sanity bound, placeholder).
pub const MAX_MATERIALS: u32 = 1_000_000;

/// Research data within this of a cost counts as enough: a sum of tenths may land a hair below the cost it was
/// meant to meet.
pub const DATA_EPSILON: f32 = 1e-3;

/// A career: the facts of one player's progression. Serialized as the save (format [`FORMAT`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Career {
    /// The content packs `(id, version)` it was last played with (informational, like share codes).
    #[serde(default)]
    packs: Vec<(String, String)>,
    /// The content start it began from.
    start: String,
    /// The game seed (ADR-0024); voyages roll their sites from it.
    seed: u64,
    /// The voyage roller's version ([`GENERATOR`]).
    generator: u32,
    /// Research node ids, in the order researched.
    #[serde(default)]
    researched: Vec<String>,
    /// Research data in store per family; only amounts above 0.
    #[serde(default)]
    data: BTreeMap<String, f32>,
    /// Materials in store; only amounts above 0.
    #[serde(default)]
    materials: BTreeMap<String, u32>,
    /// Saved design ids (`designs/` in the game's storage) flown as escorts. The flagship is the drydock's working
    /// design.
    #[serde(default)]
    escorts: Vec<String>,
    /// One record per voyage ever launched, by voyage id.
    #[serde(default)]
    voyages: BTreeMap<String, VoyageRecord>,
    /// The voyage launched and not ended yet. A save loaded with one was closed mid-voyage: that voyage counts
    /// as lost (GDR-0062, [`Career::reconcile`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    underway: Option<String>,
    #[serde(default)]
    log: Log,
    /// How the last voyage ended and what it brought home (the debrief).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_haul: Option<LastHaul>,
}

/// What a career remembers about one voyage.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoyageRecord {
    /// Launches so far; the next launch's seed mixes it in ([`Career::voyage_seed`]).
    #[serde(default)]
    pub launched: u32,
    /// The goal site was cleared on a voyage that came home. Clearing a voyage opens the next ones.
    #[serde(default)]
    pub cleared: bool,
    /// The quickest voyage that cleared the goal and came home, in sim ticks (30 per second).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_ticks: Option<u64>,
}

/// Counts over the whole career.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Log {
    /// Voyages launched.
    #[serde(default)]
    pub voyages: u32,
    /// Voyages that came home (recalled), with or without the goal cleared.
    #[serde(default)]
    pub recalled: u32,
    /// Voyages lost: the last vessel destroyed, or the game closed mid-voyage.
    #[serde(default)]
    pub lost: u32,
    /// Items brought home and unloaded (whole parts and fragments).
    #[serde(default)]
    pub salvaged: u32,
}

/// How a voyage ended, as stored in the debrief.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// Came home without the goal cleared.
    Recalled,
    /// Came home with the goal cleared.
    Cleared,
    /// The last vessel was lost (or the game closed mid-voyage): the haul is gone.
    Lost,
}

/// The last voyage's debrief: the save's `last_haul`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LastHaul {
    pub voyage: String,
    pub outcome: Outcome,
    /// How long the voyage took, in sim ticks (0 when the game was closed mid-voyage).
    #[serde(default)]
    pub ticks: u64,
    /// What came home and what it gave (empty when lost).
    #[serde(default)]
    pub report: HaulReport,
}

/// Something a ship carries home: the career's copy of the sim's cargo item (the game converts; `data` does not
/// depend on `sim`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HaulItem {
    /// A whole part (content part id); hull parts carry their armour material.
    Part {
        part: String,
        material: Option<String>,
    },
    /// What a destroyed part left (content id of that part), such as a core's programmable-matter fragment.
    Fragment { part: String },
}

/// The items a voyage brought home, with counts, in item order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Haul {
    pub items: BTreeMap<HaulItem, u32>,
}

impl Haul {
    /// Adds `count` of `item` (counts saturate).
    pub fn add(&mut self, item: HaulItem, count: u32) {
        let n = self.items.entry(item).or_default();
        *n = n.saturating_add(count);
    }
}

impl FromIterator<(HaulItem, u32)> for Haul {
    fn from_iter<I: IntoIterator<Item = (HaulItem, u32)>>(iter: I) -> Haul {
        let mut haul = Haul::default();
        for (item, count) in iter {
            haul.add(item, count);
        }
        haul
    }
}

/// What a haul is dismantled into at home ([`unload`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HaulReport {
    /// Whole parts by part id (every part that came home, worth something or not).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parts: BTreeMap<String, u32>,
    /// Fragments by the id of the part that left them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fragments: BTreeMap<String, u32>,
    /// Research data per family.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub data: BTreeMap<String, f32>,
    /// Materials by id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub materials: BTreeMap<String, u32>,
}

impl HaulReport {
    /// Items that came home: whole parts and fragments (saturating).
    pub fn items(&self) -> u32 {
        self.parts
            .values()
            .chain(self.fragments.values())
            .fold(0, |n, &k| n.saturating_add(k))
    }
}

/// How a voyage ended, as the game reports it to [`Career::voyage_ended`].
#[derive(Clone, Debug, PartialEq)]
pub enum VoyageEnd {
    /// Recalled home with every surviving vessel's haul. `cleared`: the goal site was cleared on this voyage.
    /// `ticks`: how long the voyage took.
    Recalled {
        haul: Haul,
        cleared: bool,
        ticks: u64,
    },
    /// The last vessel was lost: the haul stays behind, the goal does not count as cleared.
    Lost { ticks: u64 },
}

/// What the player may build with: parts, materials and frames. Derived ([`Career::unlocks`]), never stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unlocks {
    pub parts: BTreeSet<String>,
    pub materials: BTreeSet<String>,
    pub frames: BTreeSet<String>,
}

impl Unlocks {
    /// Every part, material and frame of the content, locked or not (the development switch `?unlock=all`).
    pub fn all(content: &Content) -> Unlocks {
        Unlocks {
            parts: content.parts.iter().map(|p| p.id.clone()).collect(),
            materials: content.materials.iter().map(|m| m.id.clone()).collect(),
            frames: content.frames.iter().map(|f| f.id.clone()).collect(),
        }
    }

    pub fn has_part(&self, id: &str) -> bool {
        self.parts.contains(id)
    }

    pub fn has_material(&self, id: &str) -> bool {
        self.materials.contains(id)
    }

    pub fn has_frame(&self, id: &str) -> bool {
        self.frames.contains(id)
    }
}

/// Why a career action is not possible.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// The content has no start with this id.
    UnknownStart(String),
    /// The content has no research node with this id.
    UnknownNode(String),
    /// The node is researched already.
    AlreadyResearched(String),
    /// These nodes must be researched first (in the node's `requires` order).
    Requires(Vec<String>),
    /// The stores hold less than the node costs: every cost that falls short, data first, each in id order.
    CannotPay(Vec<Shortfall>),
    /// The voyage is not open (unknown, or not opened yet).
    NotOpen(String),
    /// Another voyage is underway: it must end first.
    Underway(String),
    /// This voyage is not underway, so it cannot end.
    NotUnderway(String),
}

/// One cost of a research node that the stores can't meet.
#[derive(Clone, Debug, PartialEq)]
pub enum Shortfall {
    Data {
        family: String,
        have: f32,
        cost: f32,
    },
    Material {
        material: String,
        have: u32,
        cost: u32,
    },
}

/// Which ship of the fleet a [`Readiness`] entry is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FleetSlot {
    /// The drydock's working design.
    Flagship,
    /// An escort, by its index in the escort list.
    Escort(usize),
}

/// Why a fleet is not voyage-ready ([`voyage_ready`]), or a warning about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
    /// The ship is bigger than every unlocked frame. `frame`: the first frame in content order it would fit
    /// (a locked one), if any.
    LockedFrame {
        ship: FleetSlot,
        size: (u32, u32),
        frame: Option<String>,
    },
    /// The ship uses a part the player has not unlocked (or one the content no longer has).
    LockedPart { ship: FleetSlot, part: String },
    /// The ship uses an armour material the player has not unlocked (or one the content no longer has).
    LockedMaterial { ship: FleetSlot, material: String },
    /// The fleet has more logic cores than the ship-mind runs at once.
    TooManyCores { cores: u32, capacity: u32 },
    /// A warning only: no ship of the fleet has cargo room, so nothing salvaged can come home.
    NoCargo,
}

impl Readiness {
    /// Does it keep the fleet from launching? Everything but [`Readiness::NoCargo`] does.
    pub fn blocks(&self) -> bool {
        !matches!(self, Readiness::NoCargo)
    }
}

impl Career {
    /// A new game from the content's start `start`, with the game seed `seed`.
    pub fn new(content: &Content, start: &str, seed: u64) -> Result<Career, Refusal> {
        if !content.starts.contains(start) {
            return Err(Refusal::UnknownStart(start.to_string()));
        }
        Ok(Career {
            packs: packs_of(content),
            start: start.to_string(),
            seed,
            generator: GENERATOR,
            researched: Vec::new(),
            data: BTreeMap::new(),
            materials: BTreeMap::new(),
            escorts: Vec::new(),
            voyages: BTreeMap::new(),
            underway: None,
            log: Log::default(),
            last_haul: None,
        })
    }

    /// Reads a career written by [`Career::to_ron`] (or by hand) with the pack loader's options: `Some(..)` may
    /// be left out, unknown fields are errors. Only the syntax and the schema are checked; call
    /// [`Career::reconcile`] next. `Err` holds the parse error with its line and column (one problem, in a list
    /// like [`crate::parse_blueprint`]'s).
    pub fn from_ron(text: &str) -> Result<Career, Vec<Problem>> {
        ron_options()
            .from_str(text)
            .map_err(|e| vec![parse_error(&e)])
    }

    /// The career as RON: deterministic, four-space indent, reads back with [`Career::from_ron`] to an equal
    /// career.
    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new()
            .indentor("    ")
            .new_line("\n");
        ron_options()
            .to_string_pretty(self, pretty)
            .expect("a career is plain data and always serializes")
    }

    /// Brings a loaded career in line with the current content (it may have changed since the save was
    /// written): ids the content no longer has are dropped (researched nodes, families and materials in store
    /// and in the last haul, voyage records), an unknown start is replaced by the content's first start, numbers
    /// are made finite, non-negative and bounded ([`MAX_DATA`], [`MAX_MATERIALS`]), a voyage still underway
    /// counts as lost (the game was closed mid-voyage, GDR-0062), and the pack list is updated. Returns a
    /// warning for each change, for the status line. Never fails.
    pub fn reconcile(&mut self, content: &Content) -> Vec<Problem> {
        let mut notes = Vec::new();
        let now = packs_of(content);
        for (id, version) in &self.packs {
            if !now.iter().any(|(n, _)| n == id) {
                notes.push(
                    Problem::warning(format!(
                        "pack `{id}` {version} was used by this career and is not loaded now"
                    ))
                    .field("packs"),
                );
            }
        }
        self.packs = now;

        if !content.starts.contains(&self.start) {
            let old = std::mem::take(&mut self.start);
            let note = Problem::warning(match content.starts.iter().next() {
                Some(s) => {
                    self.start = s.id.clone();
                    format!(
                        "the content has no start `{old}` any more: the career continues from `{}`",
                        s.id
                    )
                }
                None => {
                    self.start = old.clone();
                    format!("the content has no start `{old}` and no other start")
                }
            });
            notes.push(note.entry(Kind::Start, &old).field("start"));
        }

        if !(1..=GENERATOR).contains(&self.generator) {
            notes.push(
                Problem::warning(format!(
                    "unknown voyage roller version {}: voyages roll with version {GENERATOR}",
                    self.generator
                ))
                .field("generator"),
            );
            self.generator = GENERATOR;
        }

        let mut kept: Vec<String> = Vec::new();
        for (k, id) in std::mem::take(&mut self.researched).into_iter().enumerate() {
            let field = format!("researched[{k}]");
            if !content.research.contains(&id) {
                notes.push(
                    Problem::warning(format!(
                        "the content has no research node `{id}` any more: dropped with its unlocks"
                    ))
                    .entry(Kind::Research, &id)
                    .field(field),
                );
            } else if kept.contains(&id) {
                notes.push(
                    Problem::warning(format!("`{id}` is listed twice: dropped"))
                        .entry(Kind::Research, &id)
                        .field(field),
                );
            } else {
                kept.push(id);
            }
        }
        self.researched = kept;

        clean_data(&mut self.data, content, "data", &mut notes);
        clean_materials(&mut self.materials, content, "materials", &mut notes);

        self.voyages.retain(|id, _| {
            let known = voyage_known(content, id);
            if !known {
                notes.push(
                    Problem::warning(format!(
                        "the content has no voyage `{id}` any more: its record is dropped"
                    ))
                    .field(format!("voyages.{id}")),
                );
            }
            known
        });

        if let Some(id) = self.underway.clone() {
            if self.voyages.get(&id).is_some_and(|r| r.launched > 0) {
                notes.push(
                    Problem::warning(format!(
                        "the game was closed during voyage `{id}`: it counts as lost"
                    ))
                    .field("underway"),
                );
                // Underway with a launch on record, so it ends.
                let _ = self.voyage_ended(content, &id, VoyageEnd::Lost { ticks: 0 });
            } else {
                notes.push(
                    Problem::warning(format!(
                        "voyage `{id}` was underway but has no launch on record: dropped"
                    ))
                    .field("underway"),
                );
                self.underway = None;
            }
        }

        if let Some(last) = &mut self.last_haul {
            if voyage_known(content, &last.voyage) {
                clean_data(
                    &mut last.report.data,
                    content,
                    "last_haul.report.data",
                    &mut notes,
                );
                clean_materials(
                    &mut last.report.materials,
                    content,
                    "last_haul.report.materials",
                    &mut notes,
                );
            } else {
                notes.push(
                    Problem::warning(format!(
                        "the content has no voyage `{}` any more: the last haul's report is dropped",
                        last.voyage
                    ))
                    .field("last_haul"),
                );
                self.last_haul = None;
            }
        }
        notes
    }

    /// The content packs `(id, version)` it was last played with.
    pub fn packs(&self) -> &[(String, String)] {
        &self.packs
    }

    /// The content start it began from.
    pub fn start(&self) -> &str {
        &self.start
    }

    /// The game seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The voyage roller's version.
    pub fn generator(&self) -> u32 {
        self.generator
    }

    /// Research node ids in the order researched.
    pub fn researched(&self) -> &[String] {
        &self.researched
    }

    pub fn is_researched(&self, node: &str) -> bool {
        self.researched.iter().any(|n| n == node)
    }

    /// Research data in store per family (amounts above 0 only).
    pub fn data(&self) -> &BTreeMap<String, f32> {
        &self.data
    }

    /// Materials in store (amounts above 0 only).
    pub fn materials(&self) -> &BTreeMap<String, u32> {
        &self.materials
    }

    /// Saved design ids flown as escorts.
    pub fn escorts(&self) -> &[String] {
        &self.escorts
    }

    /// FLEET: the saved designs (ids) to fly as escorts. Whether they fit the mind is [`voyage_ready`]'s
    /// question.
    pub fn set_escorts(&mut self, escorts: Vec<String>) {
        self.escorts = escorts;
    }

    /// The record of a voyage, if it was ever launched.
    pub fn voyage(&self, id: &str) -> Option<&VoyageRecord> {
        self.voyages.get(id)
    }

    /// The voyage launched and not ended yet.
    pub fn underway(&self) -> Option<&str> {
        self.underway.as_deref()
    }

    pub fn log(&self) -> &Log {
        &self.log
    }

    /// The last voyage's debrief.
    pub fn last_haul(&self) -> Option<&LastHaul> {
        self.last_haul.as_ref()
    }

    /// What the player may build with: every part, material and frame that is not `locked` in the content, plus
    /// every part, material and frame a researched node unlocks.
    pub fn unlocks(&self, content: &Content) -> Unlocks {
        let mut u = Unlocks {
            parts: open_ids(content.parts.iter().map(|p| (&p.id, p.locked))),
            materials: open_ids(content.materials.iter().map(|m| (&m.id, m.locked))),
            frames: open_ids(content.frames.iter().map(|f| (&f.id, f.locked))),
        };
        for unlock in self.researched_unlocks(content) {
            match unlock {
                Unlock::Part(id) => u.parts.insert(id.clone()),
                Unlock::Material(id) => u.materials.insert(id.clone()),
                Unlock::Frame(id) => u.frames.insert(id.clone()),
                Unlock::MindCapacity(_) => false,
            };
        }
        u
    }

    /// How many logic cores the ship-mind runs at once: the start's mind capacity plus every
    /// [`Unlock::MindCapacity`] of the researched nodes.
    pub fn mind_capacity(&self, content: &Content) -> u32 {
        let base = content
            .starts
            .get(&self.start)
            .map_or(0, |s| s.mind_capacity);
        self.researched_unlocks(content).fold(base, |n, u| match u {
            Unlock::MindCapacity(k) => n.saturating_add(*k),
            _ => n,
        })
    }

    /// The voyages the player may launch, in order: the start's list. The voyage kind (track 7E) adds here every
    /// voyage whose `opens_after` voyages are all cleared ([`VoyageRecord::cleared`]), and the private
    /// `voyage_known` (which [`Career::reconcile`] asks) checks its table.
    pub fn open_voyages<'c>(&self, content: &'c Content) -> Vec<&'c str> {
        content
            .starts
            .get(&self.start)
            .map(|s| s.voyages.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Is `voyage` one of [`Career::open_voyages`]?
    pub fn is_open(&self, content: &Content, voyage: &str) -> bool {
        self.open_voyages(content).contains(&voyage)
    }

    /// Can the player research `node` now? `Err` names the first reason not: an unknown node, one researched
    /// already, missing requirements, then every cost the stores can't meet.
    pub fn can_pay(&self, content: &Content, node: &str) -> Result<(), Refusal> {
        self.payable(content, node).map(|_| ())
    }

    /// Researches `node`: pays its cost from the stores and records it, which unlocks what it unlocks at once
    /// (GDR-0063). Returns the node, for the UI ("AUTOMATON CANNON · NOW IN THE DRYDOCK").
    pub fn research<'c>(
        &mut self,
        content: &'c Content,
        node: &str,
    ) -> Result<&'c ResearchNode, Refusal> {
        let n = self.payable(content, node)?;
        for (family, &cost) in &n.cost.data {
            let left = self.data.get(family).copied().unwrap_or(0.0) - cost;
            if left > DATA_EPSILON {
                self.data.insert(family.clone(), left);
            } else {
                self.data.remove(family);
            }
        }
        for (material, &cost) in &n.cost.materials {
            let left = self
                .materials
                .get(material)
                .copied()
                .unwrap_or(0)
                .saturating_sub(cost);
            if left > 0 {
                self.materials.insert(material.clone(), left);
            } else {
                self.materials.remove(material);
            }
        }
        self.researched.push(n.id.clone());
        Ok(n)
    }

    /// The seed of the next launch of `voyage`: the game seed, the voyage id (64-bit FNV-1a of its bytes) and the
    /// number of launches so far, mixed with SplitMix64:
    /// `splitmix64(seed ^ splitmix64(fnv1a64(id) ^ launched))`. The same save rolls the same voyage; every launch
    /// rolls a new one. Part of the save format: changing it needs a new [`GENERATOR`].
    pub fn voyage_seed(&self, voyage: &str) -> u64 {
        let launched = self.voyages.get(voyage).map_or(0, |r| r.launched);
        splitmix64(self.seed ^ splitmix64(fnv1a64(voyage.as_bytes()) ^ u64::from(launched)))
    }

    /// Records the launch of an open voyage and returns the seed the sim rolls it from (the
    /// [`Career::voyage_seed`] before this launch). The voyage is underway until [`Career::voyage_ended`]; write
    /// the save now, so closing the game mid-voyage counts as a loss.
    pub fn voyage_launched(&mut self, content: &Content, voyage: &str) -> Result<u64, Refusal> {
        if let Some(other) = &self.underway {
            return Err(Refusal::Underway(other.clone()));
        }
        if !self.is_open(content, voyage) {
            return Err(Refusal::NotOpen(voyage.to_string()));
        }
        let seed = self.voyage_seed(voyage);
        let record = self.voyages.entry(voyage.to_string()).or_default();
        record.launched = record.launched.saturating_add(1);
        self.log.voyages = self.log.voyages.saturating_add(1);
        self.underway = Some(voyage.to_string());
        Ok(seed)
    }

    /// Ends the voyage underway. Recalled: the haul is unloaded ([`unload`]) into the stores, and a cleared goal
    /// marks the voyage cleared (and its best time). Lost: the haul is gone and the goal does not count. Either way
    /// the debrief becomes the last haul, which is returned.
    pub fn voyage_ended(
        &mut self,
        content: &Content,
        voyage: &str,
        end: VoyageEnd,
    ) -> Result<&LastHaul, Refusal> {
        if self.underway.as_deref() != Some(voyage) {
            return Err(Refusal::NotUnderway(voyage.to_string()));
        }
        self.underway = None;
        let last = match end {
            VoyageEnd::Recalled {
                haul,
                cleared,
                ticks,
            } => {
                let report = unload(content, &haul);
                for (family, &x) in &report.data {
                    add_data(&mut self.data, family, x);
                }
                for (material, &x) in &report.materials {
                    add_materials(&mut self.materials, material, x);
                }
                let record = self.voyages.entry(voyage.to_string()).or_default();
                if cleared {
                    record.cleared = true;
                    record.best_ticks = Some(record.best_ticks.map_or(ticks, |b| b.min(ticks)));
                }
                self.log.recalled = self.log.recalled.saturating_add(1);
                self.log.salvaged = self.log.salvaged.saturating_add(report.items());
                LastHaul {
                    voyage: voyage.to_string(),
                    outcome: if cleared {
                        Outcome::Cleared
                    } else {
                        Outcome::Recalled
                    },
                    ticks,
                    report,
                }
            }
            VoyageEnd::Lost { ticks } => {
                self.log.lost = self.log.lost.saturating_add(1);
                LastHaul {
                    voyage: voyage.to_string(),
                    outcome: Outcome::Lost,
                    ticks,
                    report: HaulReport::default(),
                }
            }
        };
        Ok(self.last_haul.insert(last))
    }

    /// The node if it can be researched now (see [`Career::can_pay`]).
    fn payable<'c>(&self, content: &'c Content, node: &str) -> Result<&'c ResearchNode, Refusal> {
        let Some(n) = content.research.get(node) else {
            return Err(Refusal::UnknownNode(node.to_string()));
        };
        if self.is_researched(node) {
            return Err(Refusal::AlreadyResearched(node.to_string()));
        }
        let missing: Vec<String> = n
            .requires
            .iter()
            .filter(|r| !self.is_researched(r))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(Refusal::Requires(missing));
        }
        let mut short = Vec::new();
        for (family, &cost) in &n.cost.data {
            let have = self.data.get(family).copied().unwrap_or(0.0);
            if have + DATA_EPSILON < cost {
                short.push(Shortfall::Data {
                    family: family.clone(),
                    have,
                    cost,
                });
            }
        }
        for (material, &cost) in &n.cost.materials {
            let have = self.materials.get(material).copied().unwrap_or(0);
            if have < cost {
                short.push(Shortfall::Material {
                    material: material.clone(),
                    have,
                    cost,
                });
            }
        }
        if short.is_empty() {
            Ok(n)
        } else {
            Err(Refusal::CannotPay(short))
        }
    }

    /// Every unlock of the researched nodes the content has, in research order.
    fn researched_unlocks<'c>(&self, content: &'c Content) -> impl Iterator<Item = &'c Unlock> {
        self.researched
            .iter()
            .filter_map(|id| content.research.get(id))
            .flat_map(|n| &n.unlocks)
    }
}

/// What a haul is dismantled into at home, through the content's `salvage` blocks (`docs/content-model.md`
/// "Salvage"): a whole part gives its block's data and materials; a hull part gives its armour material's data
/// per cell of area (slopes count half a cell); a fragment gives its part's `fragment` materials. Items the content
/// doesn't know or that have no block give nothing (but whole parts are still listed). Pure: the career adds the
/// result to its stores in [`Career::voyage_ended`].
pub fn unload(content: &Content, haul: &Haul) -> HaulReport {
    let mut r = HaulReport::default();
    for (item, &n) in &haul.items {
        if n == 0 {
            continue;
        }
        match item {
            HaulItem::Part { part, material } => {
                let count = r.parts.entry(part.clone()).or_default();
                *count = count.saturating_add(n);
                let Some(p) = content.parts.get(part) else {
                    continue;
                };
                if p.hull {
                    let spec = material
                        .as_deref()
                        .and_then(|m| content.materials.get(m))
                        .and_then(|m| m.salvage.as_ref());
                    if let Some(s) = spec {
                        add_data(&mut r.data, &s.family, s.data * p.area() * n as f32);
                    }
                } else if let Some(s) = &p.salvage {
                    add_data(&mut r.data, &s.family, s.data * n as f32);
                    for (m, &a) in &s.materials {
                        add_materials(&mut r.materials, m, a.saturating_mul(n));
                    }
                }
            }
            HaulItem::Fragment { part } => {
                let count = r.fragments.entry(part.clone()).or_default();
                *count = count.saturating_add(n);
                if let Some(s) = content.parts.get(part).and_then(|p| p.salvage.as_ref()) {
                    for (m, &a) in &s.fragment {
                        add_materials(&mut r.materials, m, a.saturating_mul(n));
                    }
                }
            }
        }
    }
    r
}

/// Is the fleet (the flagship and the escorts) voyage-ready with these unlocks and this mind capacity? Empty:
/// yes. Entries, in order: per ship (flagship first) a frame too big for every unlocked frame, then the locked
/// parts and materials it uses (each once, in placement order); then too many logic cores for the mind; then the
/// warning that no ship has cargo room ([`Readiness::blocks`] is false only for that). The drydock and the map show
/// the first entry. Flight-readiness (power, thrust: the design report) is the sim's question, not this one.
pub fn voyage_ready(
    content: &Content,
    unlocks: &Unlocks,
    mind_capacity: u32,
    flagship: &Blueprint,
    escorts: &[Blueprint],
) -> Vec<Readiness> {
    let mut out = Vec::new();
    let fleet = std::iter::once((FleetSlot::Flagship, flagship)).chain(
        escorts
            .iter()
            .enumerate()
            .map(|(k, e)| (FleetSlot::Escort(k), e)),
    );
    let (mut cores, mut cargo) = (0u32, 0.0f32);
    for (slot, ship) in fleet {
        let (w, h) = ship.size;
        let fits = |f: &&Frame| f.size.0 >= w && f.size.1 >= h;
        if !content
            .frames
            .iter()
            .filter(fits)
            .any(|f| unlocks.has_frame(&f.id))
        {
            out.push(Readiness::LockedFrame {
                ship: slot,
                size: ship.size,
                frame: content.frames.iter().find(fits).map(|f| f.id.clone()),
            });
        }
        let mut parts: Vec<&str> = Vec::new();
        let mut materials: Vec<&str> = Vec::new();
        for p in &ship.parts {
            if !unlocks.has_part(&p.part) && !parts.contains(&p.part.as_str()) {
                parts.push(&p.part);
            }
            if let Some(m) = p.material.as_deref()
                && !unlocks.has_material(m)
                && !materials.contains(&m)
            {
                materials.push(m);
            }
        }
        out.extend(parts.into_iter().map(|part| Readiness::LockedPart {
            ship: slot,
            part: part.to_string(),
        }));
        out.extend(
            materials
                .into_iter()
                .map(|material| Readiness::LockedMaterial {
                    ship: slot,
                    material: material.to_string(),
                }),
        );
        let totals = content.ship_totals(ship);
        cores = cores.saturating_add(totals.logic_cores);
        cargo += totals.cargo_capacity;
    }
    if cores > mind_capacity {
        out.push(Readiness::TooManyCores {
            cores,
            capacity: mind_capacity,
        });
    }
    if cargo <= 0.0 {
        out.push(Readiness::NoCargo);
    }
    out
}

/// Does the content know this voyage? Until the voyage kind exists (track 7E, which checks its table here), the
/// only voyages the content names are the starts' lists.
fn voyage_known(content: &Content, voyage: &str) -> bool {
    content
        .starts
        .iter()
        .any(|s| s.voyages.iter().any(|v| v == voyage))
}

fn packs_of(content: &Content) -> Vec<(String, String)> {
    content
        .packs
        .iter()
        .map(|p| (p.id.clone(), p.version.clone()))
        .collect()
}

fn open_ids<'a>(entries: impl Iterator<Item = (&'a String, bool)>) -> BTreeSet<String> {
    entries
        .filter(|(_, locked)| !locked)
        .map(|(id, _)| id.clone())
        .collect()
}

/// Adds research data to a store, bounded by [`MAX_DATA`]; nothing (and no entry) for amounts not above 0.
fn add_data(store: &mut BTreeMap<String, f32>, family: &str, amount: f32) {
    if amount > 0.0 {
        let x = store.entry(family.to_string()).or_insert(0.0);
        *x = (*x + amount).min(MAX_DATA);
    }
}

/// Adds materials to a store, bounded by [`MAX_MATERIALS`]; nothing (and no entry) for 0.
fn add_materials(store: &mut BTreeMap<String, u32>, material: &str, amount: u32) {
    if amount > 0 {
        let x = store.entry(material.to_string()).or_insert(0);
        *x = x.saturating_add(amount).min(MAX_MATERIALS);
    }
}

/// Drops unknown families and amounts that are not finite or not above 0, and bounds the rest by [`MAX_DATA`].
fn clean_data(
    store: &mut BTreeMap<String, f32>,
    content: &Content,
    field: &str,
    notes: &mut Vec<Problem>,
) {
    store.retain(|family, x| {
        let note = |msg: String| {
            Problem::warning(msg)
                .entry(Kind::Family, family)
                .field(format!("{field}.{family}"))
        };
        if !content.families.contains(family) {
            notes.push(note(format!(
                "the content has no research family `{family}` any more: its data is dropped"
            )));
            return false;
        }
        if !x.is_finite() || *x < 0.0 {
            notes.push(note(format!(
                "research data {x} is not a finite amount >= 0: dropped"
            )));
            return false;
        }
        if *x > MAX_DATA {
            notes.push(note(format!(
                "research data {x} is above {MAX_DATA}: lowered to it"
            )));
            *x = MAX_DATA;
        }
        *x > 0.0
    });
}

/// Drops unknown materials and empty amounts, and bounds the rest by [`MAX_MATERIALS`].
fn clean_materials(
    store: &mut BTreeMap<String, u32>,
    content: &Content,
    field: &str,
    notes: &mut Vec<Problem>,
) {
    store.retain(|material, x| {
        if !content.materials.contains(material) {
            notes.push(
                Problem::warning(format!(
                    "the content has no material `{material}` any more: dropped"
                ))
                .entry(Kind::Material, material)
                .field(format!("{field}.{material}")),
            );
            return false;
        }
        if *x > MAX_MATERIALS {
            notes.push(
                Problem::warning(format!("{x} is above {MAX_MATERIALS}: lowered to it"))
                    .entry(Kind::Material, material)
                    .field(format!("{field}.{material}")),
            );
            *x = MAX_MATERIALS;
        }
        *x > 0
    });
}

/// 64-bit FNV-1a: a fixed, platform-independent hash (Rust's `DefaultHasher` may change between releases).
fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// SplitMix64 finaliser (Steele, Lea, Flood 2014), as the sim's RNG streams use it.
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference values of the FNV specification's test suite (and the offset basis for no bytes).
    #[test]
    fn fnv1a64_matches_the_reference() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    /// The reference outputs of SplitMix64 seeded with 0 (`next()` adds the increment first).
    #[test]
    fn splitmix64_matches_the_reference() {
        assert_eq!(splitmix64(0), 0xe220_a839_7b1d_cdaf);
        assert_eq!(splitmix64(0x9E37_79B9_7F4A_7C15), 0x6e78_9e6a_a1b9_65f4);
    }
}
