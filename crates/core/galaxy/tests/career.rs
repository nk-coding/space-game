//! A slice career saved in format 2 (before the galaxy) gets its galaxy when the game loads it (save format 3,
//! galaxy plan E4, GDR-0096): the base galaxy generated from its game seed, its voyages pinned to rim clusters, the
//! launched ones charted, the open one a located signal; read back, it keeps that galaxy. Here rather than in
//! `data` because it needs the generator.

use data::career::{Career, Lead, SignalState};

const FORMAT_2: &str = include_str!("fixtures/career_format_2.ron");

fn migrated() -> (data::Content, Career, Vec<data::Problem>) {
    let c = data::base_content();
    let mut career = Career::from_ron(FORMAT_2).unwrap();
    assert!(career.reconcile(&c).is_empty(), "the fixture is a clean format 2 save");
    assert!(career.exploration().is_none());
    let skeleton = galaxy::generate(&c, "galaxy", career.seed()).unwrap();
    let notes = career.begin_galaxy(&c, skeleton);
    (c, career, notes)
}

#[test]
fn a_format_2_slice_career_is_charted_into_its_galaxy() {
    let (c, career, notes) = migrated();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(
        notes[0].message,
        "the galaxy is charted: 3 clusters from 3 voyages flown, 1 voyage open as a signal"
    );
    let g = career.exploration().unwrap();
    let sk = g.skeleton();
    assert_eq!(*sk, galaxy::generate(&c, "galaxy", 1760351234).unwrap(), "the game seed's galaxy");

    // Every voyage pinned to a rim cluster of its own, not home; the first to the rim cluster nearest home.
    let pins = g.pins();
    assert_eq!(pins.keys().map(String::as_str).collect::<Vec<_>>(), ["corran_reach", "halvex_drift", "wardens_gate", "yard_approach"]);
    let mut clusters: Vec<u32> = pins.values().copied().collect();
    clusters.sort();
    clusters.dedup();
    assert_eq!(clusters.len(), 4);
    assert!(clusters.iter().all(|&k| k != sk.home && sk.clusters[k as usize].band == "rim"));
    let home = sk.clusters[sk.home as usize].at;
    let nearest = sk.band("rim").filter(|&k| k != sk.home).min_by_key(|&k| (sk.clusters[k as usize].at.dist2(home), k));
    assert_eq!(Some(pins["yard_approach"]), nearest);

    // Flown voyages are charted (home too), the open one not cleared is a located signal; the Warden is not open.
    let mut charted: Vec<u32> = vec![sk.home, pins["yard_approach"], pins["halvex_drift"], pins["corran_reach"]];
    charted.sort();
    assert_eq!(g.charted().keys().copied().collect::<Vec<_>>(), charted);
    assert!(g.charted().values().all(|r| r.launched == 0), "slice voyages keep their own launch counts");
    let signals: Vec<(&Lead, u32, SignalState)> = g.signals().iter().map(|s| (&s.lead, s.cluster, s.state)).collect();
    assert_eq!(signals, [(&Lead::Voyage("corran_reach".into()), pins["corran_reach"], SignalState::Located)]);
    assert!(g.logs().is_empty() && g.gates().is_empty() && g.underway().is_none());
    assert_eq!(career.jump_range(&c), 2.5);
    // Everything else is as it was.
    assert_eq!(career.voyage("halvex_drift").map(|r| r.launched), Some(2));
    assert_eq!(career.history().len(), 2);
}

#[test]
fn a_migrated_career_keeps_its_galaxy() {
    let (c, mut career, _) = migrated();
    let before = career.clone();
    assert!(career.begin_galaxy(&c, galaxy::generate(&c, "galaxy", 1).unwrap()).is_empty());
    assert_eq!(career, before, "a career with a galaxy keeps it");
    let text = career.to_ron();
    let mut back = Career::from_ron(&text).unwrap();
    assert!(back.reconcile(&c).is_empty());
    assert_eq!(back, career);
    assert_eq!(back.to_ron(), text);
}
