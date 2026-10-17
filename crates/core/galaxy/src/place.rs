//! Steps 1 to 5 of generation: clusters on the disc, their bands, home and the set pieces, the gates, the
//! reachability check, then the clusters' types, seeds and names.

use std::collections::BTreeSet;
use std::f64::consts::TAU;

use data::{Band, Content, Galaxy};

use crate::{Cluster, GENERATOR_VERSION, GenError, Point, Skeleton, Stream, polar, stream, unit, units};

/// Candidates tried around a cluster before it counts as surrounded (Bridson's k).
const TRIES: u32 = 30;
/// Spots on the edge tried for home before the attempt fails.
const HOME_TRIES: u32 = 64;
/// Name rolls before a name gets a number.
const NAME_TRIES: u32 = 32;

/// Where the clusters lie, sorted (band, then angle), with home, the set pieces and the gates.
pub(crate) struct Layout {
    points: Vec<Point>,
    /// Index into the galaxy's bands.
    band: Vec<usize>,
    home: usize,
    /// The cluster of each of [`Piece`]s, in order.
    pieces: Vec<usize>,
    gate: Vec<bool>,
}

/// A set piece to place (the galaxy's `set_pieces`, resolved against the content).
pub(crate) struct Piece {
    pub voyage: String,
    pub name: String,
    /// Index into the galaxy's bands.
    pub band: usize,
    /// Map units² from home, `(min, max)`.
    pub within: (i64, i64),
    pub gate: bool,
    /// The piece (index) its voyage opens after, if it is one.
    pub after: Option<usize>,
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

/// The clusters of `pieces` for home `home`, or `None` when one finds no cluster: a gate's piece first, the cluster
/// of its band nearest the band's inner edge (the smallest radius) within its distance from home; then the others
/// in order, within theirs and a trail step (`step`, map units²) from the piece their voyage opens after (else
/// home), the one nearest the way from there to their band's gate piece (if any); a gate's piece must lie a trail
/// step from the piece it opens after too. Never home, never one cluster twice.
fn place_pieces(points: &[Point], band: &[usize], home: usize, pieces: &[Piece], step: (i64, i64)) -> Option<Vec<usize>> {
    let mut at: Vec<Option<usize>> = vec![None; pieces.len()];
    let free = |k: usize, p: &Piece, at: &[Option<usize>]| {
        band[k] == p.band && k != home && !at.contains(&Some(k)) && (p.within.0..=p.within.1).contains(&points[k].dist2(points[home]))
    };
    for (i, p) in pieces.iter().enumerate().filter(|(_, p)| p.gate) {
        at[i] = Some((0..points.len()).filter(|&k| free(k, p, &at)).min_by_key(|&k| (points[k].r2(), k))?);
    }
    let dist = |a: usize, b: usize| libm::sqrt(points[a].dist2(points[b]) as f64);
    for (i, p) in pieces.iter().enumerate().filter(|(_, p)| !p.gate) {
        let from = p.after.and_then(|j| at[j]).unwrap_or(home);
        let to = pieces.iter().zip(&at).find(|(q, _)| q.gate && q.band == p.band).and_then(|(_, k)| *k);
        let cost = |k: usize| dist(from, k) + to.map_or(0.0, |t| dist(k, t));
        let near = |k: usize| (step.0..=step.1).contains(&points[k].dist2(points[from]));
        at[i] = Some((0..points.len()).filter(|&k| free(k, p, &at) && near(k)).min_by(|&a, &b| cost(a).total_cmp(&cost(b)).then(a.cmp(&b)))?);
    }
    let at: Vec<usize> = at.into_iter().collect::<Option<_>>()?;
    let stepped = |(i, p): (usize, &Piece)| p.after.is_none_or(|j| (step.0..=step.1).contains(&points[at[i]].dist2(points[at[j]])));
    pieces.iter().enumerate().filter(|(_, p)| p.gate).all(stepped).then_some(at)
}

/// Steps 1 to 4 for one attempt.
pub(crate) fn layout(g: &Galaxy, bands: &[&Band], pieces: &[Piece], seed: u64, attempt: u64) -> Result<Layout, GenError> {
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

    // Home: the first band's cluster nearest a spot on the edge, its set pieces placed, with enough free neighbours
    // in range (not set pieces: the first blind jumps go there).
    let mut rng = stream(seed, Stream::Home, attempt);
    let (home, placed) = (0..HOME_TRIES)
        .find_map(|_| {
            let spot = polar(Point::default(), edges[0] as f64, TAU * unit(&mut rng));
            let home = of(0).min_by_key(|&k| (points[k].dist2(spot), k))?;
            let placed = place_pieces(&points, &band, home, pieces, (units(g.trail_step.0).pow(2), units(g.trail_step.1).pow(2)))?;
            let near = of(0).filter(|&k| k != home && !placed.contains(&k) && points[k].dist2(points[home]) <= jump2).count();
            (near >= g.home_neighbours as usize).then_some((home, placed))
        })
        .ok_or_else(|| GenError(format!("no spot for home with its set pieces and {} free neighbours in range", g.home_neighbours)))?;

    // Gates: per band, the cluster nearest its inner edge in each sector; a band's gate piece is its first gate, in
    // the middle of the first sector.
    let mut rng = stream(seed, Stream::Gates, attempt);
    let mut gate = vec![false; points.len()];
    for (b, band_entry) in bands.iter().enumerate() {
        let Some(gates) = &band_entry.gates else { continue };
        let width = TAU / gates.count as f64;
        let fixed = pieces.iter().zip(&placed).find(|(p, _)| p.gate && p.band == b).map(|(_, &k)| k);
        let roll = TAU * unit(&mut rng);
        let turn = fixed.map_or(roll, |k| points[k].angle() - width / 2.0);
        for s in 0..gates.count {
            let lo = turn + s as f64 * width;
            let inside = |k: &usize| (points[*k].angle() - lo).rem_euclid(TAU) < width;
            let pick = match fixed.filter(|_| s == 0) {
                Some(k) => Some(k),
                None => of(b).filter(|k| *k != home && !placed.contains(k) && inside(k)).min_by_key(|&k| (points[k].r2(), k)),
            };
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
    Ok(Layout { points, band, home, pieces: placed, gate })
}

/// Step 5: the skeleton of a layout, with each cluster's type, seed and name (a set piece's its voyage's; no trails
/// yet).
pub(crate) fn skeleton(c: &Content, g: &Galaxy, bands: &[&Band], pieces: &[Piece], seed: u64, l: Layout) -> Result<Skeleton, GenError> {
    let (mut kinds, mut seeds, mut names) =
        (stream(seed, Stream::Kinds, 0), stream(seed, Stream::Seeds, 0), stream(seed, Stream::Names, 0));
    let mut taken: BTreeSet<String> = pieces.iter().map(|p| p.name.clone()).collect();
    let mut clusters = Vec::with_capacity(l.points.len());
    for (k, &at) in l.points.iter().enumerate() {
        let band = bands[l.band[k]];
        // A type with a `border` only within that many S of the band's outer edge.
        let fits = |w: &&data::Weighted| w.border.is_none_or(|d| at.r2() >= units(band.edge - d).max(0).pow(2));
        let total: u32 = band.clusters.iter().filter(fits).map(|w| w.weight).sum();
        let mut roll = kinds.below(total);
        let kind = band
            .clusters
            .iter()
            .filter(fits)
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
        if let Some(j) = l.pieces.iter().position(|&p| p == k) {
            name = pieces[j].name.clone();
        }
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
        pieces: pieces.iter().zip(&l.pieces).map(|(p, &k)| (p.voyage.clone(), k as u32)).collect(),
        trails: Vec::new(),
    })
}
