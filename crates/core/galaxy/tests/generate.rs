//! The base content's galaxy keeps the generator's rules on many seeds (ADR-0042, `docs/plans/galaxy-voyages.md`
//! section 5): spacing, bands, home, the set pieces, gates, reachability, trails and their signals, names, interiors;
//! and the same seed always gives the same galaxy.

use std::collections::BTreeSet;

use data::{Content, Reward};
use galaxy::{GENERATOR_VERSION, Skeleton, UNITS_PER_S, generate, interior};

const SEEDS: u64 = 40;

fn units(s: f32) -> i64 {
    (s as f64 * UNITS_PER_S as f64).round() as i64
}

#[test]
fn the_same_seed_gives_the_same_galaxy_and_seeds_differ() {
    let c = data::base_content();
    let a = generate(&c, "galaxy", 7).unwrap();
    assert_eq!(a, generate(&c, "galaxy", 7).unwrap());
    assert_eq!(a.hash(), generate(&c, "galaxy", 7).unwrap().hash());
    let hashes: BTreeSet<u64> = (1..=5).map(|s| generate(&c, "galaxy", s).unwrap().hash()).collect();
    assert_eq!(hashes.len(), 5);
    assert_eq!((a.seed, a.version, a.galaxy.as_str()), (7, GENERATOR_VERSION, "galaxy"));
    let text = ron::to_string(&a).unwrap();
    assert_eq!(ron::from_str::<Skeleton>(&text).unwrap(), a, "a skeleton reads back from its RON (the save keeps it)");
    assert!(generate(&c, "no_such_galaxy", 1).unwrap_err().0.contains("unknown galaxy"));
}

/// The rules of section 5 on every seed.
#[test]
fn every_seed_keeps_the_rules() {
    let c = data::base_content();
    let g = c.galaxies.get("galaxy").unwrap();
    for seed in 1..=SEEDS {
        let s = generate(&c, "galaxy", seed).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        check(&c, &s);
    }
    let total: usize = (1..=SEEDS).map(|seed| generate(&c, "galaxy", seed).unwrap().clusters.len()).sum();
    let mean = total as f64 / SEEDS as f64;
    assert!((70.0..=95.0).contains(&mean), "about 80 clusters (GDR-0026), {mean} on average");
    // The outer reach's border types turn up (GDR-0103: rim stragglers near the border).
    let bordered = (1..=SEEDS)
        .flat_map(|seed| generate(&c, "galaxy", seed).unwrap().clusters)
        .filter(|k| c.bands.get(&k.band).unwrap().clusters.iter().any(|w| w.id == k.kind && w.border.is_some()))
        .count();
    assert!(bordered > 0, "no cluster of a border type in {SEEDS} seeds");
    assert!(g.bands.len() == 5);
}

fn check(c: &Content, s: &Skeleton) {
    let g = c.galaxies.get(&s.galaxy).unwrap();
    let seed = s.seed;
    let spacing2 = units(g.spacing).pow(2);
    let jump2 = units(g.jump_range).pow(2);
    // Spacing, bands by radius, ids from the rim inwards.
    let order: Vec<usize> = s.clusters.iter().map(|k| g.bands.iter().position(|b| *b == k.band).unwrap()).collect();
    assert!(order.windows(2).all(|w| w[0] <= w[1]), "seed {seed}: ids run from the rim inwards");
    for (i, a) in s.clusters.iter().enumerate() {
        for b in &s.clusters[i + 1..] {
            assert!(a.at.dist2(b.at) >= spacing2, "seed {seed}: {} and {} closer than the spacing", a.name, b.name);
        }
        let k = order[i];
        let outer = units(c.bands.get(&g.bands[k]).unwrap().edge);
        let inner = g.bands.get(k + 1).map_or(0, |b| units(c.bands.get(b).unwrap().edge));
        assert!(a.at.r2() <= outer * outer && a.at.r2() > inner * inner, "seed {seed}: {} outside its band", a.name);
        let band = c.bands.get(&a.band).unwrap();
        let w = band.clusters.iter().find(|w| w.id == a.kind);
        // A band's centre has the centre's type (GDR-0130), the only cluster that has it.
        let centre = band.centre.as_ref().map(|x| x.cluster.as_str());
        let nearest = s.band(&a.band).min_by_key(|&j| (s.clusters[j as usize].at.r2(), j)) == Some(i as u32);
        assert_eq!(centre.is_some_and(|x| x == a.kind), centre.is_some() && nearest, "seed {seed}: {} is a centre or not", a.name);
        assert!(w.is_some() || centre == Some(a.kind.as_str()), "seed {seed}: {} has a type of another band", a.name);
        // A type with a border only that near the band's outer edge (GDR-0103).
        if let Some(d) = w.and_then(|w| w.border) {
            assert!(a.at.r2() >= units(band.edge - d).pow(2), "seed {seed}: {} ({}) too far in", a.name, a.kind);
        }
    }
    let names: BTreeSet<&str> = s.clusters.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(names.len(), s.clusters.len(), "seed {seed}: names are unique");
    // Home and its neighbours.
    let home = &s.clusters[s.home as usize];
    assert_eq!(home.band, g.bands[0]);
    assert!(!home.gate);
    let piece = |k: u32| s.pieces.values().any(|&p| p == k);
    let near = s.band(&g.bands[0]).filter(|&k| k != s.home && !piece(k) && s.clusters[k as usize].at.dist2(home.at) <= jump2).count();
    assert!(near >= g.home_neighbours as usize, "seed {seed}: home has {near} free neighbours");
    // The set pieces: each voyage on a cluster of its band at its distance from home, named after it; a gate's on a
    // gate; no two on one cluster.
    assert_eq!(s.pieces.len(), g.set_pieces.len(), "seed {seed}: every set piece placed");
    for p in &g.set_pieces {
        let v = c.voyages.get(&p.voyage).unwrap();
        let k = &s.clusters[s.pieces[&p.voyage] as usize];
        let d = k.at.dist2(home.at);
        assert!((units(p.distance.0).pow(2)..=units(p.distance.1).pow(2)).contains(&d), "seed {seed}: {} at {d}", v.id);
        assert_eq!((k.band.as_str(), k.name.as_str(), k.gate), (v.band.as_str(), v.name.as_str(), p.gate), "seed {seed}");
    }
    let pieces: BTreeSet<u32> = s.pieces.values().copied().collect();
    assert!(pieces.len() == s.pieces.len() && !pieces.contains(&s.home));
    // Gates per band, and every band reachable hop by hop from its entrances.
    for (b, id) in g.bands.iter().enumerate() {
        let band = c.bands.get(id).unwrap();
        let gates = s.band(id).filter(|&k| s.clusters[k as usize].gate).count();
        assert_eq!(gates as u32, band.gates.as_ref().map_or(0, |x| x.count), "seed {seed}: gates of {id}");
        let mut seen: BTreeSet<u32> = s.entrances(&g.bands, b).into_iter().collect();
        assert!(!seen.is_empty());
        let mut todo: Vec<u32> = seen.iter().copied().collect();
        while let Some(k) = todo.pop() {
            for j in s.band(id) {
                if !seen.contains(&j) && s.clusters[j as usize].at.dist2(s.clusters[k as usize].at) <= jump2 {
                    seen.insert(j);
                    todo.push(j);
                }
            }
        }
        assert!(s.band(id).all(|k| seen.contains(&k)), "seed {seed}: band {id} is not reachable");
    }
    // Trails: as many as the bands and lore ask for, each well formed. Any but a gate's is left out only where its
    // band has no cluster free for its end (GDR-0127: of its `ends_in` types; GDR-0128: a small band full).
    let ends_of: BTreeSet<u32> = s.trails.iter().map(|t| t.places.last().unwrap().cluster).collect();
    let none_free = |band: &str, tpl: &data::Trail| {
        s.band(band).all(|k| {
            let kc = &s.clusters[k as usize];
            k == s.home || kc.gate || piece(k) || ends_of.contains(&k) || !(tpl.ends_in.is_empty() || tpl.ends_in.contains(&kc.kind))
        })
    };
    for id in &g.bands {
        let band = c.bands.get(id).unwrap();
        let placed = |trail: &str| s.trails.iter().filter(|t| t.trail == trail && t.band == *id).count() as u32;
        // A gate piece's gate has the slice for its trail.
        let gated = g.set_pieces.iter().filter(|p| p.gate && c.voyages.get(&p.voyage).unwrap().band == *id).count() as u32;
        if let Some(gates) = &band.gates {
            assert_eq!(placed(&gates.trail), gates.count - gated, "seed {seed}: gate trails of {id}");
        }
        for tc in &band.trails {
            let tpl = c.trails.get(&tc.trail).unwrap();
            let n = placed(&tc.trail);
            assert!(n <= tc.count, "seed {seed}: {} trails", tc.trail);
            assert!(n == tc.count || none_free(id, tpl), "seed {seed}: a {} trail left out of {id} with room for it", tc.trail);
        }
        for l in c.lore.iter().filter(|l| l.band == *id) {
            let n = s.trails.iter().filter(|t| t.lore.as_deref() == Some(l.id.as_str())).count();
            assert!(n <= 1, "seed {seed}: fragment {} twice", l.id);
            let tpl = c.trails.get(&l.trail).unwrap();
            assert!(n == 1 || none_free(id, tpl), "seed {seed}: fragment {} left out of {id} with room for it", l.id);
        }
        // A band's centre (GDR-0130): at least three clusters, its trail ending at the cluster nearest the galaxy's
        // centre, which no other trail passes through.
        if let Some(x) = &band.centre {
            assert!(s.band(id).count() >= 3, "seed {seed}: {id} too small round its centre");
            let k = s.band(id).min_by_key(|&j| (s.clusters[j as usize].at.r2(), j)).unwrap();
            let ends: Vec<_> = s.trails.iter().filter(|t| t.trail == x.trail && t.band == *id).collect();
            assert!(ends.len() == 1 && ends[0].places.last().unwrap().cluster == k, "seed {seed}: the centre's trail");
            let through = s.trails.iter().flat_map(|t| &t.places).filter(|p| p.cluster == k).count();
            assert_eq!(through, 1, "seed {seed}: another trail passes the centre");
        }
        // A band's lore comes before its spare trails (GDR-0128).
        let lore_out = c.lore.iter().any(|l| l.band == *id && !s.trails.iter().any(|t| t.lore.as_deref() == Some(l.id.as_str())));
        let spares = band.trails.iter().filter(|tc| tc.spare).map(|tc| placed(&tc.trail)).sum::<u32>();
        assert!(!lore_out || spares == 0, "seed {seed}: a spare trail in {id} while a fragment is left out");
    }
    let mut ends = BTreeSet::new();
    for t in &s.trails {
        let tpl = c.trails.get(&t.trail).unwrap();
        let n = t.signals.len() as u32;
        assert!((tpl.length.0..=tpl.length.1).contains(&n), "seed {seed}: a {} trail of {n} signals", t.trail);
        assert_eq!(t.places.len(), t.signals.len() + 1);
        let end = t.places.last().unwrap();
        assert_eq!(end.encounter.as_deref(), Some(tpl.target.as_str()));
        assert!(ends.insert(end.cluster), "seed {seed}: two rewards in one cluster");
        let kind = &s.clusters[end.cluster as usize].kind;
        assert!(tpl.ends_in.is_empty() || tpl.ends_in.contains(kind), "seed {seed}: a {} trail ends in a {kind}", t.trail);
        assert_eq!(s.clusters[end.cluster as usize].gate, tpl.reward == Reward::Gate, "seed {seed}: gate trails end at gates");
        for p in &t.places {
            assert_eq!(s.clusters[p.cluster as usize].band, t.band, "seed {seed}: a trail stays in its band");
            assert_ne!(p.cluster, s.home, "seed {seed}: no trail place at home");
            assert!(!piece(p.cluster), "seed {seed}: no trail place at a set piece");
        }
        for (k, sig) in t.signals.iter().enumerate() {
            assert!(tpl.sources.contains(&sig.source));
            assert_eq!(t.places[k].encounter, c.signal_sources.get(&sig.source).unwrap().site, "a source's place holds its site");
            let to = s.clusters[t.places[k + 1].cluster as usize].at;
            let r = sig.area.radius as i64;
            assert!(to.dist2(sig.area.centre) <= r * r, "seed {seed}: a signal's area holds where it leads");
            let (lo, hi) = g.signal_area.across;
            assert!((units(lo) / 2 - 1..=units(hi) / 2 + 1).contains(&r));
        }
        assert_eq!(t.lore.is_some(), tpl.reward == Reward::Lore);
    }
}

#[test]
fn interiors_follow_their_cluster_types_and_hold_the_trail_places() {
    let c = data::base_content();
    let s = generate(&c, "galaxy", 3).unwrap();
    let mut places = 0;
    for (k, cluster) in (0u32..).zip(&s.clusters) {
        let i = interior(&c, &s, k).unwrap();
        assert_eq!(i, interior(&c, &s, k).unwrap(), "an interior is a function of its seed");
        let t = c.cluster_types.get(&cluster.kind).unwrap();
        assert!(i.radius >= t.radius.0 && i.radius <= t.radius.1 && i.radius.fract() == 0.0);
        // A hot cluster's star lies its type's distance out, whole metres (GDR-0102); others have none.
        assert_eq!(i.star.is_some(), t.star.is_some());
        if let (Some(star), Some(spec)) = (&i.star, &t.star) {
            let d = (star.x as f64).hypot(star.y as f64);
            assert!((d - (i.radius * spec.distance) as f64).abs() < 1.0 && star.x.fract() == 0.0 && star.heat == spec.heat);
        }
        // A radiant cluster's radiation lies between its type's edge and centre values (GDR-0130); others have none.
        assert_eq!(i.radiation.is_some(), t.radiation.is_some());
        if let (Some(r), Some(spec)) = (i.radiation, &t.radiation) {
            assert!(r >= spec.edge && r <= spec.centre, "{} radiates {r}", cluster.name);
        }
        let ordinary = i.sites.iter().filter(|x| x.place.is_none()).count() as u32;
        let (lo, hi) = t.sites.iter().fold((0, 0), |(a, b), x| (a + x.count.0, b + x.count.1));
        assert!((lo..=hi).contains(&ordinary));
        for site in i.sites.iter().filter_map(|x| x.place) {
            let p = &s.trails[site.0 as usize].places[site.1 as usize];
            assert_eq!(p.cluster, k);
            places += 1;
        }
    }
    assert_eq!(places, s.trails.iter().map(|t| t.places.len()).sum::<usize>(), "every trail place is in its interior");
    assert!(interior(&c, &s, s.clusters.len() as u32).is_err());
}

/// The base content's galaxy for seed 1. When this changes on purpose (the generator or the galaxy's content),
/// update it; when the generator changed, also raise `GENERATOR_VERSION`.
#[test]
fn the_generation_hash_is_pinned() {
    let c = data::base_content();
    assert_eq!(format!("{:016x}", generate(&c, "galaxy", 1).unwrap().hash()), "7eb322ae41afb209");
}
