//! A cluster's interior (ADR-0035: generated on demand from the cluster's seed and type): its radius and sites,
//! the trail places the skeleton put there among them. Where the sites lie in the cluster is the voyage director's
//! roll (`sim::voyage`), as for a voyage.

use data::Content;
use serde::{Deserialize, Serialize};

use crate::{GenError, Skeleton, Stream, stream, unit};

/// What a cluster holds when the fleet warps in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interior {
    /// Its cluster type (id): the voyage roll places the type's scenery (its rocks, GDR-0104).
    pub kind: String,
    /// Metres, whole.
    pub radius: f32,
    /// `(min, max)` metres between sites (the cluster type's).
    pub spacing: (f32, f32),
    /// The ordinary sites in the type's order, then the trail places in trail order.
    pub sites: Vec<InteriorSite>,
    /// The hot star beside it, if its type has one (GDR-0102). Left out of the RON when none, so interiors without
    /// one hash as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub star: Option<InteriorStar>,
    /// Radiation everywhere in it (GDR-0130, its type's `radiation` at the cluster's distance from the galaxy's
    /// centre): heat per second per side open to space. Left out of the RON when none, as `star`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radiation: Option<f32>,
}

/// A cluster's hot star (the type's `star`): where it lies, whole metres from the cluster's centre, in a direction
/// rolled after the sites, and its heat per open side at the centre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InteriorStar {
    pub x: f32,
    pub y: f32,
    pub heat: f32,
}

/// One site of an interior.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteriorSite {
    /// The encounter waiting there; none for a beacon alone.
    pub encounter: Option<String>,
    /// `(trail, place)` indices into the skeleton when the site is a trail's place.
    pub place: Option<(u32, u32)>,
}

/// The interior of cluster `cluster` (id) of `s`.
pub fn interior(c: &Content, s: &Skeleton, cluster: u32) -> Result<Interior, GenError> {
    let k = s.clusters.get(cluster as usize).ok_or_else(|| GenError(format!("no cluster {cluster}")))?;
    let t = c.cluster_types.get(&k.kind).ok_or_else(|| GenError(format!("unknown cluster type `{}`", k.kind)))?;
    let mut rng = stream(k.seed, Stream::Interior, 0);
    let (lo, hi) = t.radius;
    let radius = libm::round(lo as f64 + (hi - lo) as f64 * unit(&mut rng)) as f32;
    let mut sites = Vec::new();
    for e in &t.sites {
        let n = e.count.0 + rng.below(e.count.1 - e.count.0 + 1);
        sites.extend((0..n).map(|_| InteriorSite { encounter: Some(e.encounter.clone()), place: None }));
    }
    for (i, trail) in (0u32..).zip(&s.trails) {
        for (j, p) in (0u32..).zip(&trail.places) {
            if p.cluster == cluster {
                sites.push(InteriorSite { encounter: p.encounter.clone(), place: Some((i, j)) });
            }
        }
    }
    let star = t.star.as_ref().map(|st| {
        let a = core::f64::consts::TAU * unit(&mut rng);
        let d = radius as f64 * st.distance as f64;
        InteriorStar { x: libm::round(d * libm::cos(a)) as f32, y: libm::round(d * libm::sin(a)) as f32, heat: st.heat }
    });
    // No draw: types without radiation roll as before.
    let radiation = t.radiation.as_ref().map(|r| {
        let edge = c.bands.get(&k.band).map_or(0.0, |b| b.edge as f64);
        r.at(libm::sqrt(k.at.r2() as f64) / crate::UNITS_PER_S as f64, edge)
    });
    Ok(Interior { kind: t.id.clone(), radius, spacing: t.spacing, sites, star, radiation })
}
