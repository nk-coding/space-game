//! The galaxy (`galaxy.ron`, `bands.ron`, `cluster_types/*.ron`, `trails.ron`, `signal_sources.ron`, `lore/*.ron`):
//! what the `galaxy` crate generates a game's galaxy from (GDR-0026, GDR-0078, GDR-0080, `docs/plans/galaxy-voyages.md`).
//!
//! Distances on the galaxy map are in **S**, the spacing unit: about the distance between two neighbouring clusters
//! (the slice's voyages lie 0.9–1.3 S apart on the voyage map). Distances inside a cluster are metres.

use super::*;

/// `galaxy.ron`: one galaxy's generator numbers. A game's galaxy is generated once from its seed and these, and the
/// save keeps the result (ADR-0035).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Galaxy {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The bands (ids) from the rim inwards. The first is where the game begins; each band's outer edge lies inside
    /// the one before it.
    pub bands: Vec<String>,
    /// S: no two clusters closer than this. Clusters fill the disc as tightly as this allows, so every spot of a
    /// band lies within `spacing` of a cluster.
    pub spacing: f32,
    /// S: how far the drive jumps at the start (from home or any charted cluster, GDR-0093). Research grows it.
    pub jump_range: f32,
    /// Home is a cluster of the first band with at least this many others of it within the starting jump range that
    /// hold no set piece: free for the first blind jumps.
    pub home_neighbours: u32,
    /// Warp time: `(seconds, seconds per S)`, the first plus the second times the distance.
    pub warp: (f32, f32),
    /// A fresh signal's area on the map: its width `(min, max)` in S and how far off its centre the place may lie,
    /// as a share of its radius (0: always in the middle).
    pub signal_area: SignalArea,
    /// S: `(min, max)` between one place of a trail and the next.
    pub trail_step: (f32, f32),
    /// The parts of cluster names: a start and an end make the first word ("Hal" + "vex"), the cluster type gives
    /// the second ("Drift").
    pub names: NameParts,
    /// Hand-made voyages placed into the generated galaxy (the plan's step 5: the slice's four first), each on a
    /// cluster of its band that takes its name.
    #[serde(default)]
    pub set_pieces: Vec<SetPiece>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Galaxy, Kind::Galaxy);

/// A hand-made voyage in the galaxy ([`Galaxy::set_pieces`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetPiece {
    /// The voyage (id).
    pub voyage: String,
    /// S `(min, max)` from home. A gate's piece takes the cluster nearest its band's inner edge in it; the others the
    /// one nearest the way from the piece their voyage opens after (else home) to their band's gate piece, a trail
    /// step ([`Galaxy::trail_step`]) from the one they open after.
    pub distance: (f32, f32),
    /// It keeps one of its band's gates: clearing its goal opens the gate, and its voyages are the gate's trail (the
    /// Warden's gate, the rim's first, GDR-0078).
    #[serde(default)]
    pub gate: bool,
}

/// A fresh signal's area ([`Galaxy::signal_area`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalArea {
    /// S: `(min, max)` across.
    pub across: (f32, f32),
    /// Largest distance of the place from the area's centre, as a share of the area's radius (0 to below 1); a
    /// narrowed area keeps it.
    pub offset: f32,
    /// The share of its radius an area keeps when a scan from another cluster narrows it (above 0, below 1).
    pub narrow: f32,
}

/// Syllables of cluster names ([`Galaxy::names`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameParts {
    pub starts: Vec<String>,
    pub ends: Vec<String>,
}

/// `bands.ron`: a band of the galaxy (GDR-0026: rim, outer, middle, inner, core), a ring around the centre with its
/// clusters, the gates at its inner edge and the trails that run through it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Band {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// S from the galaxy's centre to the band's outer edge. Its inner edge is the next band's outer edge (the
    /// innermost band's is the centre).
    pub edge: f32,
    /// The cluster types (ids) of its clusters, each with a weight.
    pub clusters: Vec<Weighted>,
    /// Its gates to the next band inwards (none for the innermost band).
    #[serde(default)]
    pub gates: Option<Gates>,
    /// Further trails through the band: caches and graveyards (lore trails come from its [`LoreFragment`]s).
    #[serde(default)]
    pub trails: Vec<TrailCount>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Band, Kind::Band);

/// An id with a weight (a band's cluster types).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weighted {
    pub id: String,
    pub weight: u32,
    /// Rolled only for clusters within this many S of the band's outer edge (GDR-0103: the outer reach's rim
    /// stragglers near the border); `None` everywhere in the band.
    #[serde(default)]
    pub border: Option<f32>,
}

/// A band's gates ([`Band::gates`]): `count` clusters near its inner edge, spread around the disc, each the end of a
/// trail of its own (GDR-0078: any one of them opens the next band).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gates {
    pub count: u32,
    /// The trail (id) that leads to each; its reward is [`Reward::Gate`] and its target the gate's guardian.
    pub trail: String,
}

/// A trail template placed `count` times in a band.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrailCount {
    pub trail: String,
    pub count: u32,
}

/// `cluster_types/*.ron`: what kind of place a cluster is. Its interior is rolled from the cluster's seed when the
/// fleet warps in: a radius, and its sites from these entries, plus the trail places the galaxy put there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterType {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Metres `(min, max)`: the cluster's radius, rolled per cluster; at most 4 km (one `f32` space, ADR-0022).
    pub radius: (f32, f32),
    /// `(min, max)` metres between sites, as a voyage's [`Voyage::spacing`].
    pub spacing: (f32, f32),
    /// The second word of its clusters' names ("Drift", "Reach"), one picked per cluster.
    pub words: Vec<String>,
    /// Its ordinary sites: each encounter placed a rolled number of times.
    pub sites: Vec<ClusterSite>,
    /// A hot star near the cluster (GDR-0099 A5, GDR-0102): parts open to space take its heat.
    #[serde(default)]
    pub star: Option<StarSpec>,
    /// What fills its space between the sites: rocks, dust, debris, derelicts, landmarks, distant fights (GDR-0104).
    #[serde(default)]
    pub scenery: Scenery,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(ClusterType, Kind::ClusterType);

/// A hot star beside a cluster (GDR-0102): it lies `distance` times the cluster's radius from the centre, in a
/// direction rolled per cluster, and every part takes `heat` per second for each of its sides open to space at
/// the cluster's centre, more nearer the star (the square of the distance, at most [`STAR_HEAT_CAP`] times).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarSpec {
    /// Heat per second per open side at the cluster's centre (the units of `heat.output`).
    pub heat: f32,
    /// How far from the centre it lies, in cluster radii (above 1: outside the cluster).
    pub distance: f32,
}

/// Near a hot star the heat grows with the inverse square of the distance, but never past this many times its heat
/// at the cluster's centre.
pub const STAR_HEAT_CAP: f32 = 4.0;

/// Ordinary sites of a cluster type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterSite {
    pub encounter: String,
    /// `(min, max)`, both included, rolled per cluster.
    pub count: (u32, u32),
}

/// What waits at the end of a trail (GDR-0078).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Reward {
    /// A gate to the next band, kept by its guardian.
    Gate,
    /// A precursor cache: a research node or a rare part found nowhere else.
    Cache,
    /// A lore fragment for the memory log.
    Lore,
    /// Rich salvage: a graveyard or a wreck field with far more than usual.
    Salvage,
}

/// `trails.ron`: a trail template, a chain of signals ending at a reward. Each signal points from its source (a log
/// or a beacon at one place) to the next place; the first source lies in an ordinary cluster, each later one at the
/// place the signal before it led to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trail {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub reward: Reward,
    /// How many signals `(min, max)`, both included, 1 to 4.
    pub length: (u32, u32),
    /// The encounter at its last place: the gate's guardian, the cache, the graveyard, the wreck a fragment lies in.
    pub target: String,
    /// The signal sources (ids) its signals may come from, equally likely.
    pub sources: Vec<String>,
    /// What its signals say they lead to, the map's first line: "Something old" (GDR-0093: "SIGNAL · SOMETHING OLD").
    pub hint: String,
    /// What following it to its end gives besides what lies there: a cache's research data and materials, kept like
    /// every find of a voyage, lost or not.
    #[serde(default)]
    pub gives: ResearchCost,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Trail, Kind::Trail, hint);

/// How a signal is found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SourceForm {
    /// A data core in a wreck: salvaged, brought home and decoded there (free, GDR-0080).
    Log,
    /// A beacon, a distress call or an odd reading: scanned in the cluster, a signal at once.
    Beacon,
}

/// `signal_sources.ron`: where a signal comes from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalSource {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub form: SourceForm,
    /// The signal's second line on the map, what found it: "a log from a hauler's wreck" (GDR-0093).
    pub line: String,
    /// The encounter at the source's place: the wrecks a log lies in (required for a log), what keeps a beacon
    /// (none: the beacon alone).
    #[serde(default)]
    pub site: Option<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(SignalSource, Kind::SignalSource, line);

/// `lore/*.ron`: a lore fragment (GDR-0080), found at the end of a lore trail of its band and kept in the memory log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoreFragment {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The band (id) whose trail leads to it.
    pub band: String,
    /// The trail (id) that leads to it; its reward is [`Reward::Lore`].
    pub trail: String,
    /// The fragment as the memory log shows it.
    pub text: String,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(LoreFragment, Kind::Lore, text);
