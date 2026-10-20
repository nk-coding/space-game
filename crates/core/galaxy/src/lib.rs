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
//! 2. **Home and the set pieces**: home is the first band's cluster nearest a random spot on the galaxy's edge where
//!    the galaxy's `set_pieces` (hand-made voyages, the slice's four) find their clusters at their distances from
//!    home, a gate's piece nearest its band's inner edge, the others along the way to it a trail step apart, and at least
//!    `home_neighbours` others of its band without a set piece lie within the starting jump range.
//! 3. **Gates**: per band with gates, the cluster nearest its inner edge in each of `count` equal sectors of the disc;
//!    a band's gate piece is its first gate, the sectors turned round it.
//! 4. **Reachable**: within the starting jump range, hop by hop, every cluster of a band is reachable from its
//!    entrances (home for the first band, the gates of the band outside it for the others). Steps 1 to 4 start
//!    again on a new attempt when a band is empty (or a band with a centre holds fewer than three clusters, GDR-0130),
//!    home or a gate cannot be placed, or a band is not reachable.
//! 5. **Types, seeds and names** per cluster in id order; a set piece's cluster takes its voyage's name; a band's
//!    centre (GDR-0130), its cluster nearest the galaxy's centre, takes the centre's type instead of the one it rolled.
//! 6. **Trails** per band: the centre's (GDR-0130), one per gate but a gate piece's (its voyages are its trail), the band's bosses, caches and
//!    graveyards, one per lore fragment of the band, then its spare caches and graveyards. No trail place lies at
//!    home or a set piece. A trail's reward goes into a cluster of its own (of its `ends_in` types, if it names any);
//!    with none left free, the trail is left out of that galaxy. Its earlier places are walked back from there, a
//!    trail step at a time, towards the band's entrances where possible; each signal's area holds the place it leads
//!    to.
//!
//! **Determinism** (ADR-0010, ADR-0042): integer positions in hundredths of S ([`UNITS_PER_S`]), distance tests in
//! integers, `libm` for every root and angle, ordered containers only, PCG32 streams with fixed ids
//! ([`stream`], append only). [`Skeleton::hash`] is FNV-1a 64 of the skeleton's RON, the same on every platform;
//! `scripts/determinism-check.sh --galaxy` compares it native against wasm.

mod interior;
mod place;
mod trails;

use data::hash::splitmix64;
use data::rng::Pcg32;
use data::{Band, Content, Galaxy};

pub use data::skeleton::{Area, Cluster, Place, Point, Signal, Skeleton, TrailPlan, UNITS_PER_S};
pub use interior::{Interior, InteriorSite, InteriorStar, interior};

/// The generator's version: saves store it next to the skeleton (ADR-0024). Raise it when the same seed and content
/// would generate a different skeleton or interior. 2: the set pieces (galaxy plan E7). 3: cluster types rolled only
/// near their band's outer edge (`Weighted::border`, GDR-0103).
pub const GENERATOR_VERSION: u32 = 3;

/// Placement attempts before generation gives up (each a fresh sampling of the disc).
const ATTEMPTS: u64 = 32;

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

/// The galaxy's set pieces resolved: their voyages' names and bands, their distances in map units², the piece each
/// voyage opens after.
fn pieces(c: &Content, g: &Galaxy, bands: &[&Band]) -> Result<Vec<place::Piece>, GenError> {
    g.set_pieces
        .iter()
        .map(|p| {
            let v = c.voyages.get(&p.voyage).ok_or_else(|| GenError(format!("unknown voyage `{}`", p.voyage)))?;
            let band = bands.iter().position(|b| b.id == v.band).ok_or_else(|| GenError(format!("voyage `{}` lies outside the galaxy", v.id)))?;
            let after = v.opens_after.first().and_then(|a| g.set_pieces.iter().position(|q| q.voyage == *a));
            Ok(place::Piece {
                voyage: v.id.clone(),
                name: v.name.clone(),
                band,
                within: (units(p.distance.0).pow(2), units(p.distance.1).pow(2)),
                gate: p.gate,
                after,
            })
        })
        .collect()
}

/// Generates the skeleton of `galaxy` (id) for a game's `seed`: a pure function of the seed, [`GENERATOR_VERSION`]
/// and the content.
pub fn generate(c: &Content, galaxy: &str, seed: u64) -> Result<Skeleton, GenError> {
    let (g, bands) = resolve(c, galaxy)?;
    let pieces = pieces(c, g, &bands)?;
    let mut why = String::new();
    for attempt in 0..ATTEMPTS {
        match place::layout(g, &bands, &pieces, seed, attempt) {
            Ok(layout) => {
                let mut s = place::skeleton(c, g, &bands, &pieces, seed, layout)?;
                s.trails = trails::trails(c, g, &bands, &s)?;
                return Ok(s);
            }
            Err(e) => why = e.0,
        }
    }
    Err(GenError(format!("no galaxy after {ATTEMPTS} attempts: {why}")))
}
