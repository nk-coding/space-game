//! A game's galaxy skeleton (ADR-0035, ADR-0042): the clusters with their bands, types, places, seeds, names and gate
//! flags, home, and the trails with their places and signals. The `galaxy` crate generates it from a seed and the
//! content; the career keeps it in the save ([`crate::career::Exploration`]), so content updates never reshuffle a
//! running galaxy. The types live here, beside the career that stores them, and the generator re-exports them.
//!
//! Positions are integers in hundredths of S ([`UNITS_PER_S`]); [`Skeleton::hash`] is FNV-1a 64 of its RON, the same
//! on every platform.

use serde::{Deserialize, Serialize};

use crate::hash::fnv1a64;

/// Galaxy map positions are integers in hundredths of S.
pub const UNITS_PER_S: i32 = 100;

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
    /// The generator's version (`galaxy::GENERATOR_VERSION`) at generation.
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
