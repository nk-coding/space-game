//! The career: the player's progression as a pure model (GDR-0062, GDR-0063, ADR-0033), as [`crate::design::edit`] is
//! for the drydock. No Bevy, no I/O, wasm-safe. The rules are documented in `docs/career.md`.
//!
//! A [`Career`] holds the save's **facts** only: the start, the game seed, the researched nodes, the research data
//! and materials in store, the escorts, a record per voyage, the voyage underway, a small log and the history of
//! the last voyages ([`MAX_HISTORY`]). Everything else is derived from the content on demand ([`Career::unlocks`],
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

mod save;
mod voyages;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::hash::{fnv1a64, splitmix64};
use crate::content::kind::Kind;
use crate::content::limits::MAX_DOCUMENT_BYTES;
use crate::content::load::{Content, parse_error, ron_options, strip_bom};
use crate::content::model::{Blueprint, Frame, ResearchNode, Unlock};
use crate::content::problem::Problem;

pub use voyages::{unload, voyage_ready};

/// The format version of the save's RON text ([`Career::to_ron`]). The game's storage writes it into the
/// document header (ADR-0031). Raise it when a field is added or its meaning changes, and migrate older formats
/// when reading: an older build refuses unknown fields, so only a new format lets its storage report the save as
/// newer instead of damaged.
///
/// * 1: the last voyage only, as `last_haul`.
/// * 2 (15 Oct 2026, GDR-0091): the last voyages as `history`; a format 1 `last_haul` is read and starts it
///   ([`Career::from_ron`]).
pub const FORMAT: u32 = 2;

/// How many voyages the history keeps (home's LOG, GDR-0091): the oldest go first. A placeholder.
pub const MAX_HISTORY: usize = 50;

/// The version of the voyage roller a new career uses (ADR-0024: old versions must stay reproducible or be
/// migrated). It covers [`Career::voyage_seed`] and the sim's `VoyagePlan::roll` (the draw order, the plan's own
/// generator, the wreck seeds): changing either needs a new version.
///
/// * 1 (13 Oct 2026): the plan drew on a generator seeded like the sim's encounter stream, and wrecks rolled their
///   missing parts on that stream when they spawned.
/// * 2 (13 Oct 2026, review): the plan's generator is salted (`sim::voyage::PLAN_STREAM`) and each wreck carries a
///   seed of its own in the plan.
///
/// An older save is migrated by [`Career::reconcile`]: nothing it stores refers to a rolled voyage (no mid-voyage
/// saves, GDR-0062; a launch rolls anew), so it simply rolls its next launches with this version. A save from a
/// newer build is refused by [`Career::from_ron`].
pub const GENERATOR: u32 = 2;

/// Most research data of one family a store holds (a sanity bound, placeholder): a hundred times the largest cost
/// content may ask for ([`crate::content::validate::MAX_RESEARCH_DATA`]).
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
    /// Parts research unlocked that the player has since placed or selected in the drydock. The others are
    /// fresh ([`Career::fresh_parts`]): the drydock tags them NEW.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    seen: BTreeSet<String>,
    /// One record per voyage ever launched, by voyage id.
    #[serde(default)]
    voyages: BTreeMap<String, VoyageRecord>,
    /// The voyage launched and not ended yet. A save loaded with one was closed mid-voyage: that voyage counts
    /// as lost (GDR-0062, [`Career::reconcile`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    underway: Option<String>,
    #[serde(default)]
    log: Log,
    /// The last voyages, oldest first, at most [`MAX_HISTORY`]: how each ended and what it brought home (the
    /// debrief, home's LOG).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    history: Vec<VoyageResult>,
    /// Format 1's `last_haul`: read only, it starts the history ([`Career::from_ron`]).
    #[serde(default, rename = "last_haul", skip_serializing)]
    old_last_haul: Option<VoyageResult>,
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

/// How a voyage ended, as stored in the history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// Came home without the goal cleared.
    Recalled,
    /// Came home with the goal cleared.
    Cleared,
    /// The last vessel was lost (or the game closed mid-voyage): the haul is gone.
    Lost,
}

/// How a voyage ended and what it brought home: one entry of the save's `history` (the debrief, home's LOG).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoyageResult {
    pub voyage: String,
    pub outcome: Outcome,
    /// How long the voyage took, in sim ticks (0 when the game was closed mid-voyage).
    #[serde(default)]
    pub ticks: u64,
    /// What came home and what it gave (empty when lost).
    #[serde(default)]
    pub report: HaulReport,
    /// Not looked at in the LOG yet: its mast glows until the LOG opens ([`Career::read_log`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub new: bool,
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

/// How many of a part the player has to build with ([`Career::available`], GDR-0082).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// As many as the player likes.
    Unlimited,
    /// This many in all: the parts in a design use them up.
    Count(u32),
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
    /// A warning only: no ship of the fleet has a hold (cargo on its main piece, `Content::ship_hold`), so nothing
    /// salvaged can come home.
    NoCargo,
}

impl Readiness {
    /// Does it keep the fleet from launching? Everything but [`Readiness::NoCargo`] does.
    pub fn blocks(&self) -> bool {
        !matches!(self, Readiness::NoCargo)
    }
}

impl Career {
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

    /// The research data of one family that can pay: an amount that is not finite or below 0 (a save not
    /// reconciled yet) counts as none.
    fn store(&self, family: &str) -> f32 {
        self.data
            .get(family)
            .copied()
            .filter(|x| x.is_finite() && *x > 0.0)
            .unwrap_or(0.0)
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

    /// The fresh parts: `locked` parts a researched node unlocked that the player has not placed or selected in
    /// the drydock since (its NEW tags), in id order.
    pub fn fresh_parts(&self, content: &Content) -> BTreeSet<String> {
        self.researched_unlocks(content)
            .filter_map(|u| match u {
                Unlock::Part(id) => Some(id),
                _ => None,
            })
            .filter(|id| {
                content.parts.get(id).is_some_and(|p| p.locked) && !self.seen.contains(*id)
            })
            .cloned()
            .collect()
    }

    /// The player placed or selected `part` in the drydock: it is no longer fresh. True when it was (the game
    /// writes the save then); any other part changes nothing.
    pub fn mark_seen(&mut self, content: &Content, part: &str) -> bool {
        self.fresh_parts(content).contains(part) && self.seen.insert(part.to_string())
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

    /// The last voyages, oldest first (at most [`MAX_HISTORY`]).
    pub fn history(&self) -> &[VoyageResult] {
        &self.history
    }

    /// The last voyage's result.
    pub fn last_voyage(&self) -> Option<&VoyageResult> {
        self.history.last()
    }

    /// How many voyage results the LOG has not shown yet.
    pub fn unread(&self) -> usize {
        self.history.iter().filter(|v| v.new).count()
    }

    /// The LOG was opened: every result has been looked at. True when one had not (the game writes the save then).
    pub fn read_log(&mut self) -> bool {
        let mut changed = false;
        for v in self.history.iter_mut().filter(|v| v.new) {
            v.new = false;
            changed = true;
        }
        changed
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

    /// How many of a part the player has to build with (GDR-0082). Derived like the unlocks, so the save holds no
    /// counts: every part is [`Availability::Unlimited`] until fabrication, spares and bonding fill the counts in
    /// (GDR-0029, GDR-0031, GDR-0063). The drydock shows it on each palette row,
    /// and its editing model refuses a placement when none is left ([`crate::design::edit::Drydock::set_available`]).
    pub fn available(&self, _part: &str) -> Availability {
        Availability::Unlimited
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

    /// The voyages the player may launch, in content order (the voyage map's): those the start lists, and every
    /// voyage with `opens_after` whose voyages have all been cleared ([`VoyageRecord::cleared`]). A voyage with no
    /// `opens_after` opens only through a start.
    pub fn open_voyages<'c>(&self, content: &'c Content) -> Vec<&'c str> {
        let start = content.starts.get(&self.start);
        let cleared = |id: &String| self.voyages.get(id).is_some_and(|r| r.cleared);
        content
            .voyages
            .iter()
            .filter(|v| {
                start.is_some_and(|s| s.voyages.contains(&v.id))
                    || (!v.opens_after.is_empty() && v.opens_after.iter().all(cleared))
            })
            .map(|v| v.id.as_str())
            .collect()
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
            let left = self.store(family) - cost;
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
            let have = self.store(family);
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
