//! A slice career saved in format 2 (before the galaxy) gets its galaxy when the game loads it (galaxy plan E4,
//! GDR-0096): the base galaxy generated from its game seed, its voyages on their set pieces (E7, GDR-0098), the
//! launched ones charted, the open one a located signal; read back, it keeps that galaxy. A format 3 career (a
//! skeleton from before the set pieces, the slice pinned by E4) keeps its pins, and its Warden's cluster becomes a
//! gate that clearing the Warden opens. Here rather than in `data` because it needs the generator.

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

    // Every voyage on its set piece, the Warden's a gate.
    let pins = g.pins();
    assert_eq!(*pins, sk.pieces);
    assert_eq!(pins.keys().map(String::as_str).collect::<Vec<_>>(), ["corran_reach", "halvex_drift", "wardens_gate", "yard_approach"]);
    assert!(sk.clusters[pins["wardens_gate"] as usize].gate);

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

/// A format 3 save as E4 wrote it: its skeleton without set pieces (generator version 1), the slice pinned by the
/// career, the Warden's cluster no gate. At load it keeps its pins and the Warden's cluster becomes a gate (a note);
/// clearing the Warden then opens it, and the outer reach with it.
#[test]
fn a_format_3_career_keeps_its_pins_and_its_warden_opens_a_gate() {
    let (c, career, _) = migrated();
    let warden = career.exploration().unwrap().pins()["wardens_gate"];
    // E4's text: no `pieces`, the Warden's cluster not a gate.
    let mut text = career.to_ron();
    let at = text.find("pieces: {").unwrap();
    let (from, to) = (text[..at].rfind('\n').unwrap() + 1, at + text[at..].find("},\n").unwrap() + "},\n".len());
    text.replace_range(from..to, "");
    let seed = format!("seed: {},", career.exploration().unwrap().skeleton().clusters[warden as usize].seed);
    let at = text.find(&seed).unwrap();
    let gate = at + text[at..].find("gate: true").unwrap();
    text.replace_range(gate..gate + "gate: true".len(), "gate: false");
    let mut old = Career::from_ron(&text).unwrap();
    let g = old.exploration().unwrap();
    assert!(g.skeleton().pieces.is_empty() && !g.skeleton().clusters[warden as usize].gate);

    let notes = old.reconcile(&c);
    assert_eq!(notes.iter().map(|n| n.message.as_str()).collect::<Vec<_>>(), ["The Warden's gate keeps a gate now: clearing its goal opens the next band"]);
    let g = old.exploration().unwrap();
    assert_eq!(g.pins()["wardens_gate"], warden, "its pins kept");
    assert!(g.skeleton().clusters[warden as usize].gate && g.gates().is_empty());
    assert!(old.reconcile(&c).is_empty(), "once");

    for v in ["corran_reach", "wardens_gate"] {
        old.voyage_launched(&c, v).unwrap();
        old.voyage_ended(&c, v, data::career::VoyageEnd::Recalled { haul: Default::default(), cleared: true, ticks: 9 }).unwrap();
    }
    assert_eq!(old.exploration().unwrap().gates().iter().copied().collect::<Vec<_>>(), [warden]);
    assert!(old.band_open(&c, "outer"));
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
