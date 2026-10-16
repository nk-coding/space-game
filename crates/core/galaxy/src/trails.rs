//! Step 6 of generation: the trails of every band, their places, signal sources and fresh areas.

use std::collections::BTreeSet;
use std::f64::consts::TAU;

use data::{Band, Content, Galaxy, Trail};

use crate::{Area, GenError, Place, Point, Signal, Skeleton, Stream, TrailPlan, polar, stream, unit, units};

/// The trails of a skeleton whose clusters, home and gates are set: per band (rim first) one per gate, then the
/// band's `trails`, then one per lore fragment of the band.
pub(crate) fn trails(c: &Content, g: &Galaxy, bands: &[&Band], s: &Skeleton) -> Result<Vec<TrailPlan>, GenError> {
    let mut rng = stream(s.seed, Stream::Trails, 0);
    let get = |id: &str| c.trails.get(id).ok_or_else(|| GenError(format!("unknown trail `{id}`")));
    // Clusters holding a reward already: no two rewards share a cluster.
    let mut ends: BTreeSet<u32> = BTreeSet::new();
    let mut out = Vec::new();
    for (b, band) in bands.iter().enumerate() {
        // (template, fixed end, lore fragment)
        let mut wanted: Vec<(&Trail, Option<u32>, Option<String>)> = Vec::new();
        if let Some(gates) = &band.gates {
            let t = get(&gates.trail)?;
            wanted.extend(s.band(&band.id).filter(|&k| s.clusters[k as usize].gate).map(|k| (t, Some(k), None)));
        }
        for tc in &band.trails {
            let t = get(&tc.trail)?;
            wanted.extend((0..tc.count).map(|_| (t, None, None)));
        }
        for l in c.lore.iter().filter(|l| l.band == band.id) {
            wanted.push((get(&l.trail)?, None, Some(l.id.clone())));
        }
        let entrances: Vec<Point> = s.entrances(&g.bands, b).iter().map(|&k| s.clusters[k as usize].at).collect();
        let from_start = |k: u32| entrances.iter().map(|e| e.dist2(s.clusters[k as usize].at)).min().unwrap_or(0);
        // Places a trail may use: the band's clusters but home and the gates.
        let open: Vec<u32> = s.band(&band.id).filter(|&k| k != s.home && !s.clusters[k as usize].gate).collect();
        for (t, fixed, lore) in wanted {
            let end = match fixed {
                Some(k) => k,
                None => {
                    let free: Vec<u32> = open.iter().copied().filter(|k| !ends.contains(k)).collect();
                    if free.is_empty() {
                        return Err(GenError(format!("band `{}` has no cluster left for the end of a `{}` trail", band.id, t.id)));
                    }
                    free[rng.below(free.len() as u32) as usize]
                }
            };
            ends.insert(end);
            let length = t.length.0 + rng.below(t.length.1 - t.length.0 + 1);
            // Walk back from the end: each earlier place a trail step from the next, nearer the entrances if any is.
            let (near, far) = (units(g.trail_step.0).pow(2), units(g.trail_step.1).pow(2));
            let mut path = vec![end];
            for _ in 0..length {
                let here = *path.last().unwrap();
                let at = s.clusters[here as usize].at;
                let step: Vec<u32> = open
                    .iter()
                    .copied()
                    .filter(|k| !path.contains(k) && (near..=far).contains(&s.clusters[*k as usize].at.dist2(at)))
                    .collect();
                let closer: Vec<u32> = step.iter().copied().filter(|&k| from_start(k) < from_start(here)).collect();
                let pool = if closer.is_empty() { step } else { closer };
                // None a trail step away (a thin band): the nearest place left, however far.
                let next = if pool.is_empty() {
                    open.iter().copied().filter(|k| !path.contains(k)).min_by_key(|&k| (s.clusters[k as usize].at.dist2(at), k))
                } else {
                    Some(pool[rng.below(pool.len() as u32) as usize])
                };
                let Some(next) = next else { break };
                path.push(next);
            }
            if path.len() < 2 {
                return Err(GenError(format!("no place left for a `{}` trail in band `{}`", t.id, band.id)));
            }
            path.reverse();
            let (places, signals) = plan(c, g, t, &path, s, &mut rng)?;
            out.push(TrailPlan { trail: t.id.clone(), band: band.id.clone(), lore, places, signals });
        }
    }
    Ok(out)
}

/// The places and signals of a trail along `path` (first source to reward): each signal's source and its fresh area
/// around the next place.
fn plan(c: &Content, g: &Galaxy, t: &Trail, path: &[u32], s: &Skeleton, rng: &mut crate::Pcg32) -> Result<(Vec<Place>, Vec<Signal>), GenError> {
    let mut places = Vec::with_capacity(path.len());
    let mut signals = Vec::with_capacity(path.len() - 1);
    for (k, &cluster) in path.iter().enumerate() {
        let Some(&next) = path.get(k + 1) else {
            places.push(Place { cluster, encounter: Some(t.target.clone()) });
            break;
        };
        let id = &t.sources[rng.below(t.sources.len() as u32) as usize];
        let source = c.signal_sources.get(id).ok_or_else(|| GenError(format!("unknown signal source `{id}`")))?;
        places.push(Place { cluster, encounter: source.site.clone() });
        let (lo, hi) = g.signal_area.across;
        let radius = (lo as f64 + (hi - lo) as f64 * unit(rng)) * crate::UNITS_PER_S as f64 / 2.0;
        let off = g.signal_area.offset as f64 * radius * libm::sqrt(unit(rng));
        let a = TAU * unit(rng);
        let centre = polar(s.clusters[next as usize].at, off, a);
        signals.push(Signal { source: source.id.clone(), area: Area { centre, radius: libm::round(radius) as i32 } });
    }
    Ok((places, signals))
}
