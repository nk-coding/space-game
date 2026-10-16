//! The galaxy generator (ADR-0042, `docs/plans/galaxy-voyages.md` section 5): a game's galaxy **skeleton** from its
//! seed and the content's galaxy kinds (`galaxy.ron`, `bands.ron`, `cluster_types/`, `trails.ron`,
//! `signal_sources.ron`, `lore/`), and a cluster's **interior** from the skeleton when the fleet warps in.
//!
//! * The skeleton ([`Skeleton`], [`generate`]): the clusters (band, type, place on the map, seed, name), home, the
//!   gates and the trails with their places and signals. A new game generates it once and the save keeps it
//!   (ADR-0035), so content updates never reshuffle a running galaxy.
//! * An interior ([`interior`]): a cluster's radius and sites, rolled from its seed and type, with the trail places
//!   the skeleton put there.
//!
//! Generation, in order (each step on a PCG32 stream of its own, [`stream`]):
//!
//! 1. **Clusters** fill the galaxy's disc (out to the first band's edge) by Poisson-disc sampling at the galaxy's
//!    spacing, so no two lie closer than it and every spot lies within it of a cluster; each belongs to the band
//!    whose ring holds it. Ids run from the rim inwards, round each band by angle.
//! 2. **Home** is the first band's cluster nearest a random spot on the galaxy's edge with at least
//!    `home_neighbours` others of its band within the starting jump range.
//! 3. **Gates**: per band with gates, the cluster nearest its inner edge in each of `count` equal sectors of the disc.
//! 4. **Reachable**: within the starting jump range, hop by hop, every cluster of a band is reachable from its
//!    entrances (home for the first band, the gates of the band outside it for the others). Steps 1 to 4 start
//!    again on a new attempt when a band is empty, home or a gate cannot be placed, or a band is not reachable.
//! 5. **Types, seeds and names** per cluster in id order.
//! 6. **Trails** per band: one per gate, the band's caches and graveyards, one per lore fragment of the band. A
//!    trail's reward goes into a cluster of its own; its earlier places are walked back from there, a trail step at a
//!    time, towards the band's entrances where possible; each signal's area holds the place it leads to.
//!
//! **Determinism** (ADR-0010, ADR-0042): integer positions in hundredths of S ([`UNITS_PER_S`]), distance tests in
//! integers, `libm` for every root and angle, ordered containers only, PCG32 streams with fixed ids
//! ([`stream`], append only). [`Skeleton::hash`] is FNV-1a 64 of the skeleton's RON, the same on every platform;
//! `scripts/determinism-check.sh --galaxy` compares it native against wasm.

mod interior;
mod place;
mod trails;

use data::hash::{fnv1a64, splitmix64};
use data::rng::Pcg32;
use data::{Band, Content, Galaxy};
use serde::{Deserialize, Serialize};

pub use interior::{Interior, InteriorSite, interior};

/// The generator's version: saves store it next to the skeleton (ADR-0024). Raise it when the same seed and content
/// would generate a different skeleton or interior.
pub const GENERATOR_VERSION: u32 = 1;

/// Galaxy map positions are integers in hundredths of S.
pub const UNITS_PER_S: i32 = 100;

/// Placement attempts before generation gives up (each a fresh sampling of the disc).
const ATTEMPTS: u64 = 16;

/// The generator's random streams. **Append only, never renumber**: a stream's id is part of what a seed means.
#[derive(Clone, Copy, Debug)]
#[repr(u64)]
pub(crate) enum Stream {
    /// Cluster positions (per attempt).
    Place = 1,
    /// Home's spot on the edge (per attempt).
    Home = 2,
    /// The gates' sectors (per attempt).
    Gates = 3,
    /// Cluster types.
    Kinds = 4,
    /// Cluster seeds.
    Seeds = 5,
    /// Cluster names.
    Names = 6,
    /// Trails: lengths, places, sources, areas.
    Trails = 7,
    /// A cluster's interior (seeded by the cluster's own seed).
    Interior = 8,
}

/// The generator of `stream` for `seed` (and placement attempt `attempt`, 0 for the streams outside the attempts):
/// PCG32 seeded with `splitmix64(seed ^ splitmix64(id + attempt · 2³²))` on the stream's id as its sequence.
pub(crate) fn stream(seed: u64, stream: Stream, attempt: u64) -> Pcg32 {
    let id = stream as u64;
    Pcg32::new(splitmix64(seed ^ splitmix64(id.wrapping_add(attempt << 32))), id)
}

/// Uniform in `[0, 1)` with 32 random bits.
pub(crate) fn unit(rng: &mut Pcg32) -> f64 {
    rng.next_u32() as f64 * (1.0 / 4_294_967_296.0)
}

/// A point at distance `r` (map units) and angle `a` from `from`, rounded to map units.
pub(crate) fn polar(from: Point, r: f64, a: f64) -> Point {
    Point {
        x: libm::round(from.x as f64 + r * libm::cos(a)) as i32,
        y: libm::round(from.y as f64 + r * libm::sin(a)) as i32,
    }
}

/// A distance in S as map units, rounded.
pub(crate) fn units(s: f32) -> i64 {
    libm::round(s as f64 * UNITS_PER_S as f64) as i64
}

/// A place on the galaxy map in hundredths of S, the centre at `(0, 0)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    /// Squared distance in map units².
    pub fn dist2(self, o: Point) -> i64 {
        let (dx, dy) = ((self.x - o.x) as i64, (self.y - o.y) as i64);
        dx * dx + dy * dy
    }

    /// Squared distance from the centre.
    pub fn r2(self) -> i64 {
        self.dist2(Point::default())
    }

    /// Distance in S.
    pub fn distance_s(self, o: Point) -> f64 {
        libm::sqrt(self.dist2(o) as f64) / UNITS_PER_S as f64
    }

    /// Angle about the centre in radians, `-π..=π`.
    pub fn angle(self) -> f64 {
        libm::atan2(self.y as f64, self.x as f64)
    }
}

/// One cluster of the skeleton. Its id is its index in [`Skeleton::clusters`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cluster {
    pub name: String,
    /// The band (id) it lies in.
    pub band: String,
    /// Its cluster type (id).
    pub kind: String,
    pub at: Point,
    /// Its interior's seed.
    pub seed: u64,
    /// One of its band's gates to the next band.
    pub gate: bool,
}

/// A generated trail: `places` hold its signals' sources and targets (`places[k]` the source of `signals[k]`,
/// `places[k + 1]` where it leads; the last place holds the reward).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrailPlan {
    /// The trail template (id).
    pub trail: String,
    pub band: String,
    /// The lore fragment (id) at its end, for a lore trail.
    pub lore: Option<String>,
    pub places: Vec<Place>,
    pub signals: Vec<Signal>,
}

/// A place of a trail: a site put into a cluster's interior.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub cluster: u32,
    /// The encounter waiting there: a source's site, the trail's target; none for a beacon alone.
    pub encounter: Option<String>,
}

/// One signal of a trail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signal {
    /// The signal source (id): a log or a beacon at the place before it.
    pub source: String,
    /// Its fresh area on the map, holding the cluster it leads to.
    pub area: Area,
}

/// A circle on the galaxy map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Area {
    pub centre: Point,
    /// In map units.
    pub radius: i32,
}

/// A game's galaxy: what the save keeps (ADR-0035).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skeleton {
    /// The galaxy (id) it was generated from.
    pub galaxy: String,
    pub seed: u64,
    /// [`GENERATOR_VERSION`] at generation.
    pub version: u32,
    /// From the rim inwards, round each band by angle.
    pub clusters: Vec<Cluster>,
    /// The home cluster (id).
    pub home: u32,
    pub trails: Vec<TrailPlan>,
}

impl Skeleton {
    /// The generation hash: FNV-1a 64 of the skeleton's RON (identical on every platform).
    pub fn hash(&self) -> u64 {
        fnv1a64(ron::to_string(self).expect("a skeleton writes as RON").as_bytes())
    }

    /// The clusters (ids) of `band`.
    pub fn band<'a>(&'a self, band: &'a str) -> impl Iterator<Item = u32> + 'a {
        (0u32..).zip(&self.clusters).filter(move |(_, c)| c.band == band).map(|(k, _)| k)
    }

    /// The gate clusters (ids).
    pub fn gates(&self) -> impl Iterator<Item = u32> + '_ {
        (0u32..).zip(&self.clusters).filter(|(_, c)| c.gate).map(|(k, _)| k)
    }

    /// Where a band is entered from (ids): home for the first band, the gates of the band outside it for the others.
    pub fn entrances(&self, bands: &[String], band: usize) -> Vec<u32> {
        match band.checked_sub(1) {
            None => vec![self.home],
            Some(outer) => self.band(&bands[outer]).filter(|&k| self.clusters[k as usize].gate).collect(),
        }
    }
}

/// Why a galaxy could not be generated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenError(pub String);

impl std::fmt::Display for GenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GenError {}

/// The galaxy `galaxy` (id) and its bands, rim first.
pub(crate) fn resolve<'a>(c: &'a Content, galaxy: &str) -> Result<(&'a Galaxy, Vec<&'a Band>), GenError> {
    let g = c.galaxies.get(galaxy).ok_or_else(|| GenError(format!("unknown galaxy `{galaxy}`")))?;
    let bands = g
        .bands
        .iter()
        .map(|b| c.bands.get(b).ok_or_else(|| GenError(format!("unknown band `{b}`"))))
        .collect::<Result<Vec<_>, _>>()?;
    if bands.is_empty() {
        return Err(GenError(format!("galaxy `{galaxy}` has no bands")));
    }
    Ok((g, bands))
}

/// Generates the skeleton of `galaxy` (id) for a game's `seed`: a pure function of the seed, [`GENERATOR_VERSION`]
/// and the content.
pub fn generate(c: &Content, galaxy: &str, seed: u64) -> Result<Skeleton, GenError> {
    let (g, bands) = resolve(c, galaxy)?;
    let mut why = String::new();
    for attempt in 0..ATTEMPTS {
        match place::layout(g, &bands, seed, attempt) {
            Ok(layout) => {
                let mut s = place::skeleton(c, g, &bands, seed, layout)?;
                s.trails = trails::trails(c, g, &bands, &s)?;
                return Ok(s);
            }
            Err(e) => why = e.0,
        }
    }
    Err(GenError(format!("no galaxy after {ATTEMPTS} attempts: {why}")))
}
