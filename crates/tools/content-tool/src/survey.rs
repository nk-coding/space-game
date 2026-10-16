//! `content-tool galaxy --seeds N`: the content's galaxy over many seeds, for tuning (galaxy plan section 8, risk 1:
//! "how many scans and voyages a trail takes"): how many free clusters home has within the starting jump range, and
//! how many voyages a player takes to find a signal of the first band.
//!
//! The search is played by a simple player on the career's own rules (`Career::scanned`): while the signal is an
//! area, a voyage goes to a cluster of the place's band inside it, picked at random (a blind jump into the area: the
//! map shows no uncharted cluster), and scans there, which
//! finds the place when it is that cluster and narrows the area otherwise; once located, the next voyage finds it.

use data::Career;
use data::career::{SignalState, TrailSignal};
use data::rng::Pcg32;
use data::skeleton::Skeleton;

/// Voyages to find one signal, at most this many (a guard; the area always ends up holding the place alone).
const MOST: u32 = 12;

/// Prints the survey of seeds `1..=seeds`.
pub fn survey(c: &data::Content, seeds: u64) -> Result<(), String> {
    let g = c.galaxies.iter().next().ok_or("the content has no galaxy (`galaxy.ron`)")?;
    let start = c.starts.iter().next().ok_or("the content has no start")?;
    let rim = &g.bands[0];
    let (mut free, mut voyages, mut inside, mut first_scan) = (Vec::new(), Vec::new(), Vec::new(), (0u32, 0u32));
    let mut failed = 0;
    // The set pieces: the step from each to the piece its voyage opens after (else home).
    let mut steps: Vec<f64> = Vec::new();
    for seed in 1..=seeds {
        let Ok(s) = galaxy::generate(c, &g.id, seed) else {
            failed += 1;
            continue;
        };
        let home = s.clusters[s.home as usize].at;
        for (v, &k) in &s.pieces {
            let after = c.voyages.get(v).and_then(|x| x.opens_after.first()).and_then(|a| s.pieces.get(a)).map_or(home, |&a| s.clusters[a as usize].at);
            steps.push(s.clusters[k as usize].at.distance_s(after));
        }
        let reach = (g.jump_range as f64 * data::skeleton::UNITS_PER_S as f64) as i64;
        let pieces: Vec<u32> = s.pieces.values().copied().collect();
        free.push(s.band(rim).filter(|&k| k != s.home && !pieces.contains(&k) && s.clusters[k as usize].at.dist2(home) <= reach * reach).count());
        let mut rng = Pcg32::new(seed, 1);
        for (t, plan) in (0u32..).zip(&s.trails).filter(|(_, p)| p.band == *rim) {
            for k in 0..plan.signals.len() as u32 {
                let signal = TrailSignal { trail: t, signal: k };
                let mut career = Career::new(c, &start.id, seed).map_err(|e| format!("{e:?}"))?;
                career.begin_galaxy(c, s.clone());
                career.hear(signal);
                inside.push(within(&s, &career, signal).len());
                let (n, located_first) = search(c, &mut career, signal, &mut rng);
                voyages.push(n);
                if let Some(first) = located_first {
                    first_scan.0 += u32::from(first);
                    first_scan.1 += 1;
                }
            }
        }
    }
    free.sort();
    let share = |n: &[u32], f: &dyn Fn(u32) -> bool| 100.0 * n.iter().filter(|&&x| f(x)).count() as f64 / n.len().max(1) as f64;
    let mean = |n: &[u32]| n.iter().sum::<u32>() as f64 / n.len().max(1) as f64;
    println!("{seeds} seeds of galaxy `{}`, {failed} failed", g.id);
    if !free.is_empty() {
        let f: Vec<u32> = free.iter().map(|&x| x as u32).collect();
        println!(
            "home's free clusters within {} S (no set piece): min {} / median {} / max {}, mean {:.1}; 3 or more {:.0} %, 4 or more {:.0} %",
            g.jump_range,
            f[0],
            f[f.len() / 2],
            f[f.len() - 1],
            mean(&f),
            share(&f, &|x| x >= 3),
            share(&f, &|x| x >= 4)
        );
    }
    if !steps.is_empty() {
        steps.sort_by(f64::total_cmp);
        let far = steps.iter().filter(|&&x| x > g.jump_range as f64).count();
        println!(
            "the set pieces' steps (from the piece each opens after, or home): min {:.2} / median {:.2} / max {:.2} S; {:.0} % beyond {} S",
            steps[0],
            steps[steps.len() / 2],
            steps[steps.len() - 1],
            100.0 * far as f64 / steps.len() as f64,
            g.jump_range
        );
    }
    let inside: Vec<u32> = inside.iter().map(|&x| x as u32).collect();
    println!("clusters in a fresh area of `{rim}`: mean {:.1}", mean(&inside));
    let dist: Vec<String> = (1..=5).map(|k| format!("{k}{}: {:.0} %", if k == 5 { "+" } else { "" }, share(&voyages, &|x| if k == 5 { x >= 5 } else { x == k }))).collect();
    println!("voyages to find a signal of `{rim}`: mean {:.2}; {}", mean(&voyages), dist.join(", "));
    println!(
        "the first scan from another cluster locates it: {:.0} % ({} of {})",
        100.0 * first_scan.0 as f64 / first_scan.1.max(1) as f64,
        first_scan.0,
        first_scan.1
    );
    Ok(())
}

/// The clusters inside the signal's area now (none once it is located).
fn within(s: &Skeleton, career: &Career, signal: TrailSignal) -> Vec<u32> {
    let Some(r) = career.exploration().and_then(|g| g.signal(signal)).filter(|r| r.state < SignalState::Located) else {
        return Vec::new();
    };
    let r2 = i64::from(r.area.radius).pow(2);
    (0u32..).zip(&s.clusters).filter(|(_, c)| c.at.dist2(r.area.centre) <= r2).map(|(k, _)| k).collect()
}

/// Voyages until `signal` is found, and whether the first scan away from its place located it (`None` when the first
/// voyage found it outright).
fn search(c: &data::Content, career: &mut Career, signal: TrailSignal, rng: &mut Pcg32) -> (u32, Option<bool>) {
    let s = career.exploration().expect("a galaxy").skeleton().clone();
    let target = s.trails[signal.trail as usize].places[signal.signal as usize + 1].cluster;
    let mut first = None;
    for n in 1..=MOST {
        // A blind jump never goes home or to a set piece, nor past a gate not open (the place's band is).
        let band = &s.clusters[target as usize].band;
        let inside: Vec<u32> = within(&s, career, signal)
            .into_iter()
            .filter(|k| *k != s.home && !s.pieces.values().any(|p| p == k) && s.clusters[*k as usize].band == *band)
            .collect();
        if inside.is_empty() {
            return (n, first);
        }
        let to = inside[rng.below(inside.len() as u32) as usize];
        if to == target {
            return (n, first);
        }
        let after = career.scanned(c, to, &[]);
        let located = after.iter().any(|&(x, state)| x == signal && state == SignalState::Located);
        first.get_or_insert(located);
    }
    (MOST, first)
}
