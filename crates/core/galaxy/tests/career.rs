//! A new game's career with its generated galaxy (galaxy plan E4, E7, GDR-0096, GDR-0098): the base galaxy from its
//! game seed, the slice's voyages on their set pieces, home charted, the first voyage a located signal; clearing the
//! slice charts its clusters and the Warden opens its gate and the outer reach; read back, it keeps that galaxy; the dev
//! mode's career has everything open.
//! Here rather than in `data` because it needs the generator.

use data::career::{Career, Lead, SignalState, VoyageEnd};

const SEED: u64 = 1760351234;

fn new_game() -> (data::Content, Career) {
    let c = data::base_content();
    let mut career = Career::new(&c, "rim_awakening", SEED).unwrap();
    assert!(career.exploration().is_none());
    career.begin_galaxy(&c, galaxy::generate(&c, "galaxy", SEED).unwrap());
    (c, career)
}

#[test]
fn a_new_game_lies_in_its_galaxy_and_the_warden_opens_its_gate() {
    let (c, mut career) = new_game();
    let g = career.exploration().unwrap();
    let sk = g.skeleton().clone();
    assert_eq!(sk, galaxy::generate(&c, "galaxy", SEED).unwrap(), "the game seed's galaxy");

    // Every voyage on its set piece, the Warden's a gate; home charted, the first voyage a located signal.
    let pins = g.pins().clone();
    assert_eq!(pins, sk.pieces);
    assert_eq!(pins.keys().map(String::as_str).collect::<Vec<_>>(), ["corran_reach", "halvex_drift", "wardens_gate", "yard_approach"]);
    assert!(sk.clusters[pins["wardens_gate"] as usize].gate);
    assert_eq!(g.charted().keys().copied().collect::<Vec<_>>(), [sk.home]);
    let signals: Vec<(&Lead, u32, SignalState)> = g.signals().iter().map(|s| (&s.lead, s.cluster, s.state)).collect();
    assert_eq!(signals, [(&Lead::Voyage("yard_approach".into()), pins["yard_approach"], SignalState::Located)]);
    assert!(g.logs().is_empty() && g.gates().is_empty() && g.underway().is_none());
    assert_eq!(career.jump_range(&c), 2.5);
    assert!(!career.band_open(&c, "outer"));

    for v in ["yard_approach", "halvex_drift", "corran_reach", "wardens_gate"] {
        career.voyage_launched(&c, v).unwrap();
        career.voyage_ended(&c, v, VoyageEnd::Recalled { haul: Default::default(), cleared: true, ticks: 9 }).unwrap();
    }
    let g = career.exploration().unwrap();
    assert_eq!(g.charted().len(), 5, "home and the four voyages' clusters");
    assert!(g.signals().iter().all(|s| s.state == SignalState::Done));
    assert_eq!(g.gates().iter().copied().collect::<Vec<_>>(), [pins["wardens_gate"]]);
    assert!(career.band_open(&c, "outer"));
}

#[test]
fn a_career_keeps_its_galaxy() {
    let (c, mut career) = new_game();
    let before = career.clone();
    career.begin_galaxy(&c, galaxy::generate(&c, "galaxy", 1).unwrap());
    assert_eq!(career, before, "a career with a galaxy keeps it");
    let text = career.to_ron();
    let mut back = Career::from_ron(&text).unwrap();
    assert!(back.reconcile(&c).is_empty());
    assert_eq!(back, career);
    assert_eq!(back.to_ron(), text);
}

/// The dev mode's career (ADR-0046): every research node researched, every material and family in store up to the
/// bound, every cluster charted (a jump start) and every gate open, so every band is open and a jump reaches any
/// cluster; read back and opened again, it stays the same.
#[test]
fn the_dev_career_has_everything_open() {
    let (c, mut career) = new_game();
    career.open_everything(&c);
    assert!(c.research.iter().all(|n| career.is_researched(&n.id)));
    assert!(c.materials.iter().all(|m| career.materials().get(&m.id) == Some(&data::career::MAX_MATERIALS)));
    assert!(c.families.iter().all(|f| career.data().get(&f.id) == Some(&data::career::MAX_DATA)));
    let sk = career.exploration().unwrap().skeleton().clone();
    let bands = &c.galaxies.get("galaxy").unwrap().bands;
    assert!(bands.iter().all(|b| career.band_open(&c, b)), "every band open");
    for k in 0..sk.clusters.len() as u32 {
        assert!(career.exploration().unwrap().is_charted(k));
        assert!(career.jump_from(&c, k).is_some(), "cluster {k} in reach");
    }
    let deep = sk.clusters.iter().position(|x| x.band == *bands.last().unwrap()).expect("a cluster in the core") as u32;
    let mut jump = career.clone();
    assert!(jump.cluster_launched(&c, deep).is_ok());
    let mut back = Career::from_ron(&career.to_ron()).unwrap();
    assert!(back.reconcile(&c).is_empty());
    let before = back.clone();
    back.open_everything(&c);
    assert_eq!(back, before);
}
