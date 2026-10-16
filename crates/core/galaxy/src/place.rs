//! Steps 1 to 5 of generation: clusters on the disc, their bands, home, the gates, the reachability check, then
//! the clusters' types, seeds and names.

use std::collections::BTreeSet;
use std::f64::consts::TAU;

use data::{Band, Content, Galaxy};

use crate::{Cluster, GENERATOR_VERSION, GenError, Point, Skeleton, Stream, polar, stream, unit, units};

/// Candidates tried around a cluster before it counts as surrounded (Bridson's k).
const TRIES: u32 = 30;
/// Spots on the edge tried for home before the attempt fails.
const HOME_TRIES: u32 = 16;
/// Name rolls before a name gets a number.
const NAME_TRIES: u32 = 32;

/// Where the clusters lie, sorted (band, then angle), with home and the gates.
pub(crate) struct Layout {
    points: Vec<Point>,
    /// Index into the galaxy's bands.
    band: Vec<usize>,
    home: usize,
    gate: Vec<bool>,
}

fn fail<T>(why: impl Into<String>) -> Result<T, GenError> {
    Err(GenError(why.into()))
}

/// Poisson-disc sampling of the disc of `radius` with no two points closer than `spacing` (map units; Bridson
/// 2007): every point of the disc ends within about `spacing` of a sample.
fn sample(radius: i64, spacing: i64, rng: &mut crate::Pcg32) -> Vec<Point> {
    // Grid cells of side spacing / √2 hold at most one sample each.
    let cell = ((spacing as f64 / std::f64::consts::SQRT_2) as i64).max(1);
    let n = (2 * radius / cell + 1) as usize;
    let mut grid: Vec<Option<u32>> = vec![None; n * n];
    let at = |p: Point| (((p.x as i64 + radius) / cell) as usize, ((p.y as i64 + radius) / cell) as usize);
    let (r2, s2) = (radius * radius, spacing * spacing);
    let mut points: Vec<Point> = Vec::new();
    let fits = |p: Point, points: &[Point], grid: &[Option<u32>]| {
        if p.r2() > r2 {
            return false;
        }
        let (gx, gy) = at(p);
        for y in gy.saturating_sub(2)..(gy + 3).min(n) {
            for x in gx.saturating_sub(2)..(gx + 3).min(n) {
                if grid[y * n + x].is_some_and(|k| points[k as usize].dist2(p) < s2) {
                    return false;
                }
            }
        }
        true
    };
    let first = polar(Point::default(), radius as f64 * libm::sqrt(unit(rng)), TAU * unit(rng));
    let mut active = vec![0usize];
    let (gx, gy) = at(first);
    grid[gy * n + gx] = Some(0);
    points.push(first);
    while !active.is_empty() {
        let k = rng.below(active.len() as u32) as usize;
        let from = points[active[k]];
        let mut found = false;
        for _ in 0..TRIES {
            let p = polar(from, spacing as f64 * (1.0 + unit(rng)), TAU * unit(rng));
            if fits(p, &points, &grid) {
                let (gx, gy) = at(p);
                grid[gy * n + gx] = Some(points.len() as u32);
                active.push(points.len());
                points.push(p);
                found = true;
                break;
            }
        }
        if !found {
            active.swap_remove(k);
        }
    }
    points
}

/// Steps 1 to 4 for one attempt.
pub(crate) fn layout(g: &Galaxy, bands: &[&Band], seed: u64, attempt: u64) -> Result<Layout, GenError> {
    let edges: Vec<i64> = bands.iter().map(|b| units(b.edge)).collect();
    let mut points = sample(edges[0], units(g.spacing), &mut stream(seed, Stream::Place, attempt));
    // The band of a point: the innermost whose edge still holds it.
    let band_of = |p: &Point| edges.iter().rposition(|&e| p.r2() <= e * e).unwrap_or(0);
    points.sort_by(|a, b| band_of(a).cmp(&band_of(b)).then(a.angle().total_cmp(&b.angle())).then(a.cmp(b)));
    let band: Vec<usize> = points.iter().map(band_of).collect();
    if let Some(b) = (0..bands.len()).find(|&b| !band.contains(&b)) {
        return fail(format!("band `{}` holds no cluster", bands[b].id));
    }
    let band_ref = &band;
    let of = |b: usize| (0..points.len()).filter(move |&k| band_ref[k] == b);
    let jump2 = units(g.jump_range).pow(2);

    // Home: the first band's cluster nearest a spot on the edge, with enough neighbours in range.
    let mut rng = stream(seed, Stream::Home, attempt);
    let home = (0..HOME_TRIES)
        .find_map(|_| {
            let spot = polar(Point::default(), edges[0] as f64, TAU * unit(&mut rng));
            let home = of(0).min_by_key(|&k| (points[k].dist2(spot), k))?;
            let near = of(0).filter(|&k| k != home && points[k].dist2(points[home]) <= jump2).count();
            (near >= g.home_neighbours as usize).then_some(home)
        })
        .ok_or_else(|| GenError(format!("no spot for home with {} neighbours in range", g.home_neighbours)))?;

    // Gates: per band, the cluster nearest its inner edge in each sector.
    let mut rng = stream(seed, Stream::Gates, attempt);
    let mut gate = vec![false; points.len()];
    for (b, band_entry) in bands.iter().enumerate() {
        let Some(gates) = &band_entry.gates else { continue };
        let turn = TAU * unit(&mut rng);
        let width = TAU / gates.count as f64;
        for s in 0..gates.count {
            let lo = turn + s as f64 * width;
            let inside = |k: &usize| (points[*k].angle() - lo).rem_euclid(TAU) < width;
            let pick = of(b).filter(|k| *k != home && inside(k)).min_by_key(|&k| (points[k].r2(), k));
            match pick {
                Some(k) => gate[k] = true,
                None => return fail(format!("no cluster for gate {} of band `{}`", s + 1, band_entry.id)),
            }
        }
    }

    // Reachable: every cluster of a band, hop by hop within the jump range, from its entrances.
    for (b, band_entry) in bands.iter().enumerate() {
        let entrances: Vec<usize> = if b == 0 { vec![home] } else { of(b - 1).filter(|&k| gate[k]).collect() };
        let mut seen: BTreeSet<usize> = entrances.iter().copied().collect();
        let mut todo = entrances;
        while let Some(k) = todo.pop() {
            for j in of(b) {
                if !seen.contains(&j) && points[j].dist2(points[k]) <= jump2 {
                    seen.insert(j);
                    todo.push(j);
                }
            }
        }
        if let Some(k) = of(b).find(|k| !seen.contains(k)) {
            return fail(format!("a cluster of band `{}` at {:?} is out of reach", band_entry.id, points[k]));
        }
    }
    Ok(Layout { points, band, home, gate })
}

/// Step 5: the skeleton of a layout, with each cluster's type, seed and name (no trails yet).
pub(crate) fn skeleton(c: &Content, g: &Galaxy, bands: &[&Band], seed: u64, l: Layout) -> Result<Skeleton, GenError> {
    let (mut kinds, mut seeds, mut names) =
        (stream(seed, Stream::Kinds, 0), stream(seed, Stream::Seeds, 0), stream(seed, Stream::Names, 0));
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let mut clusters = Vec::with_capacity(l.points.len());
    for (k, &at) in l.points.iter().enumerate() {
        let band = bands[l.band[k]];
        let total: u32 = band.clusters.iter().map(|w| w.weight).sum();
        let mut roll = kinds.below(total);
        let kind = band
            .clusters
            .iter()
            .find(|w| {
                let hit = roll < w.weight;
                roll = roll.saturating_sub(w.weight);
                hit
            })
            .ok_or_else(|| GenError(format!("band `{}` has no cluster types", band.id)))?;
        let t = c.cluster_types.get(&kind.id).ok_or_else(|| GenError(format!("unknown cluster type `{}`", kind.id)))?;
        let mut pick = |parts: &[String]| parts[names.below(parts.len() as u32) as usize].clone();
        let mut roll_name = || format!("{}{} {}", pick(&g.names.starts), pick(&g.names.ends), pick(&t.words));
        let mut name = roll_name();
        for _ in 0..NAME_TRIES {
            if !taken.contains(&name) {
                break;
            }
            name = roll_name();
        }
        let base = name.clone();
        let mut n = 2;
        while taken.contains(&name) {
            name = format!("{base} {n}");
            n += 1;
        }
        taken.insert(name.clone());
        clusters.push(Cluster {
            name,
            band: band.id.clone(),
            kind: kind.id.clone(),
            at,
            seed: seeds.next_u64(),
            gate: l.gate[k],
        });
    }
    Ok(Skeleton {
        galaxy: g.id.clone(),
        seed,
        version: GENERATOR_VERSION,
        clusters,
        home: l.home as u32,
        trails: Vec::new(),
    })
}
