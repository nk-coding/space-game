//! The galaxy in the career (`data::career::Exploration`, save formats 3 and 4; galaxy plan E4, E7, E8, GDR-0093,
//! GDR-0096, GDR-0098) on a small hand-made skeleton: the jump range and its research, jumps from home and charted
//! clusters, launching into a cluster and ending there, data cores and logs, hearing, decoding and narrowing signals,
//! gates, the slice's voyages on their set pieces and kept in step (the Warden's gate opening), the trails' rewards,
//! the save. The migrations of a format 2 and a format 3 save with a generated galaxy are in the `galaxy` crate's
//! tests (`tests/career.rs`).

mod common;

use common::*;
use data::career::{Career, Haul, HaulItem, Lead, Outcome, Refusal, SignalState, TrailSignal, VoyageEnd};
use data::skeleton::{Area, Cluster, Place, Point, Signal, Skeleton, TrailPlan};
use data::Content;

/// Five rim clusters on a line and one off it (map units, 100 a S): home at 0, then 2, 3 and 4.5 S out; a gate 2.3 S
/// the other way. The slice's voyages lie on 1 to 4 (the Warden's on the gate). One trail: a hauler's log in cluster 1
/// leads to cluster 3, its fresh area holding 1, 2 and 3.
fn skeleton() -> Skeleton {
    let cluster = |name: &str, x: i32, y: i32, gate: bool| Cluster {
        name: name.into(),
        band: "rim".into(),
        kind: "wreck_drift".into(),
        at: Point { x, y },
        seed: x as u64 * 7 + y as u64,
        gate,
    };
    Skeleton {
        galaxy: "galaxy".into(),
        seed: 4207,
        version: 2,
        clusters: vec![
            cluster("Home", 0, 0, false),
            cluster("One", 200, 0, false),
            cluster("Two", 300, 0, false),
            cluster("Three", 450, 0, false),
            cluster("Gate", 0, 230, true),
        ],
        home: 0,
        pieces: [("yard_approach", 1), ("halvex_drift", 2), ("corran_reach", 3), ("wardens_gate", 4)].map(|(v, k)| (v.to_string(), k)).into(),
        trails: vec![TrailPlan {
            trail: "precursor_cache".into(),
            band: "rim".into(),
            lore: None,
            places: vec![
                Place { cluster: 1, encounter: Some("derelict_field".into()) },
                Place { cluster: 3, encounter: Some("precursor_wrecks".into()) },
            ],
            signals: vec![Signal {
                source: "hauler_log".into(),
                area: Area { centre: Point { x: 300, y: 50 }, radius: 300 },
            }],
        }],
    }
}

const LOG: TrailSignal = TrailSignal { trail: 0, signal: 0 };

/// A career from `fields` (RON, after the seed) with the skeleton above.
fn game(c: &Content, fields: &str) -> Career {
    let mut career = Career::from_ron(&format!("(start: \"rim_awakening\", seed: 4207, generator: 2{fields})")).unwrap();
    assert!(career.reconcile(c).is_empty());
    assert!(career.begin_galaxy(c, skeleton()).is_empty(), "a new game has no voyages to chart");
    career
}

fn core_haul() -> Haul {
    [(HaulItem::DataCore { trail: 0, signal: 0 }, 1), (HaulItem::Part { part: "thruster".into(), material: None }, 2)]
        .into_iter()
        .collect()
}

/// GDR-0093: 2.5 S at the start, 3.25 and 4 with the two propulsion nodes; jumps start from home and every charted
/// cluster.
#[test]
fn the_jump_range_grows_by_research_and_starts_from_charted_clusters() {
    let c = content();
    let mut career = game(&c, "");
    let g = career.exploration().unwrap();
    assert_eq!(g.charted().keys().copied().collect::<Vec<_>>(), [0], "home is charted");
    assert_eq!(career.jump_range(&c), 2.5);
    assert_eq!((career.jump_from(&c, 1), career.jump_from(&c, 2), career.jump_from(&c, 4)), (Some(0), None, Some(0)));
    assert_eq!(career.cluster_launched(&c, 2), Err(Refusal::OutOfReach(2)));
    assert_eq!(career.cluster_launched(&c, 9), Err(Refusal::UnknownCluster(9)));

    let one = career.clone();
    assert_eq!(game(&c, ", researched: [\"jump_calibration_1\"]").jump_range(&c), 3.25);
    let both = game(&c, ", researched: [\"jump_calibration_1\", \"jump_calibration_2\"]");
    assert_eq!(both.jump_range(&c), 4.0);
    assert_eq!(both.jump_from(&c, 3), None, "4.5 S from home");

    // Launching charts: from cluster 1 the next ones are in reach.
    let seed = career.cluster_seed(1);
    assert_eq!(career.cluster_launched(&c, 1), Ok(seed));
    assert_ne!(career.cluster_seed(1), seed, "every launch rolls anew");
    assert_eq!(career.cluster_launched(&c, 4), Err(Refusal::Underway("1".into())));
    assert_eq!(career.voyage_launched(&c, "yard_approach"), Err(Refusal::Underway("1".into())));
    let last = career.cluster_ended(&c, 1, VoyageEnd::Lost { ticks: 30 }).unwrap().clone();
    assert_eq!((last.voyage.as_str(), last.cluster, last.outcome), ("One", Some(1), Outcome::Lost));
    assert_eq!(career.cluster_ended(&c, 1, VoyageEnd::Lost { ticks: 30 }), Err(Refusal::NotUnderway("1".into())));
    let g = career.exploration().unwrap();
    assert!(g.is_charted(1), "arriving charts it, lost or not");
    assert_eq!(g.charted()[&1].launched, 1);
    assert_eq!((career.jump_from(&c, 2), career.jump_from(&c, 3)), (Some(1), Some(1)));
    assert_eq!(career.log().voyages, one.log().voyages + 1);
}

/// A data core takes the log's place home; lost, it stays in its wreck and the next launch rolls it again. Decoded,
/// it is a signal; a scan narrows its area towards the place until it is located.
#[test]
fn data_cores_become_logs_then_signals_that_narrow() {
    let c = content();
    let mut career = game(&c, "");
    let leads = career.leads(&c, 1);
    assert_eq!((leads.signals.as_slice(), leads.cores.as_slice()), (&[][..], &[(0, 0)][..]), "a hauler's log is a data core");

    career.cluster_launched(&c, 1).unwrap();
    career.cluster_ended(&c, 1, VoyageEnd::Lost { ticks: 10 }).unwrap();
    assert!(career.exploration().unwrap().logs().is_empty(), "lost with the haul");
    assert_eq!(career.leads(&c, 1).cores, [(0, 0)]);

    career.cluster_launched(&c, 1).unwrap();
    let report = career.cluster_ended(&c, 1, VoyageEnd::Recalled { haul: core_haul(), cleared: false, ticks: 10 }).unwrap().report.clone();
    assert_eq!((report.logs, report.items()), (1, 3));
    assert_eq!(career.exploration().unwrap().logs(), [LOG]);
    assert!(career.leads(&c, 1).cores.is_empty(), "the career has the log");

    assert_eq!(career.decode(1), None);
    assert_eq!(career.decode(0), Some(LOG));
    let g = career.exploration().unwrap();
    assert!(g.logs().is_empty());
    let s = g.signal(LOG).unwrap().clone();
    assert_eq!((s.cluster, s.state, s.area), (3, SignalState::Fresh, skeleton().trails[0].signals[0].area));
    assert!(!career.hear(LOG), "heard already");
    assert_eq!(career.leads(&c, 3).signals, [(0, 1)], "its place is a signal's site now");

    // A scan in cluster 2 shrinks the area to 60 % (`galaxy.ron`) towards cluster 3, which stays within 60 % of the new
    // radius of its centre; it still holds clusters 1 and 2, so it is only narrowed.
    let mut alt = career.clone();
    assert_eq!(career.narrow(&c, LOG, 2), Some(SignalState::Narrowed));
    let a = career.exploration().unwrap().signal(LOG).unwrap().area;
    assert_eq!(a, Area { centre: Point { x: 347, y: 35 }, radius: 180 });
    let three = skeleton().clusters[3].at;
    assert!(a.centre.dist2(three) <= 109 * 109, "the place stays within 60 % of the radius (to a unit)");
    // Again from cluster 1: 108, still holding cluster 2; once more, no other cluster is inside: located, on its cluster.
    assert_eq!(career.narrow(&c, LOG, 1), Some(SignalState::Narrowed));
    assert_eq!(career.exploration().unwrap().signal(LOG).unwrap().area.radius, 108);
    assert_eq!(career.narrow(&c, LOG, 2), Some(SignalState::Located));
    assert_eq!(career.exploration().unwrap().signal(LOG).unwrap().area, Area { centre: three, radius: 0 });
    assert_eq!(career.narrow(&c, LOG, 1), None, "located is final");
    // A scan in its own cluster locates it at once.
    assert_eq!(alt.narrow(&c, LOG, 3), Some(SignalState::Located));
}

#[test]
fn gates_open_once() {
    let c = content();
    let mut career = game(&c, "");
    assert!(!career.open_gate(1), "no gate");
    assert!(career.open_gate(4));
    assert!(!career.open_gate(4));
    assert_eq!(career.exploration().unwrap().gates().iter().copied().collect::<Vec<_>>(), [4]);
}

/// The slice's voyages (galaxy plan E7): on the skeleton's set pieces, the open one a located signal; flying one
/// charts its cluster, clearing it marks its signal done and the next voyage's located; clearing the Warden opens its
/// gate and the band beyond.
#[test]
fn the_slices_voyages_lie_on_their_set_pieces_and_the_warden_opens_its_gate() {
    let c = content();
    let mut career = game(&c, "");
    let g = career.exploration().unwrap();
    let pins: Vec<(&str, u32)> = g.pins().iter().map(|(v, &k)| (v.as_str(), k)).collect();
    assert_eq!(pins, [("corran_reach", 3), ("halvex_drift", 2), ("wardens_gate", 4), ("yard_approach", 1)]);
    let open: Vec<(&Lead, u32, SignalState)> = g.signals().iter().map(|s| (&s.lead, s.cluster, s.state)).collect();
    assert_eq!(open, [(&Lead::Voyage("yard_approach".into()), 1, SignalState::Located)]);

    career.voyage_launched(&c, "yard_approach").unwrap();
    assert!(career.exploration().unwrap().is_charted(1));
    career.voyage_ended(&c, "yard_approach", VoyageEnd::Recalled { haul: Haul::default(), cleared: true, ticks: 9 }).unwrap();
    let states: Vec<(String, SignalState)> = career
        .exploration()
        .unwrap()
        .signals()
        .iter()
        .map(|s| (format!("{:?}", s.lead), s.state))
        .collect();
    assert_eq!(
        states,
        [
            ("Voyage(\"yard_approach\")".to_string(), SignalState::Done),
            ("Voyage(\"halvex_drift\")".to_string(), SignalState::Located)
        ]
    );
    let clear = |career: &mut Career, v: &str| {
        career.voyage_launched(&c, v).unwrap();
        career.voyage_ended(&c, v, VoyageEnd::Recalled { haul: Haul::default(), cleared: true, ticks: 9 }).unwrap();
    };
    clear(&mut career, "halvex_drift");
    clear(&mut career, "corran_reach");
    assert!(career.exploration().unwrap().gates().is_empty() && !career.band_open(&c, "outer"));
    clear(&mut career, "wardens_gate");
    assert_eq!(career.exploration().unwrap().gates().iter().copied().collect::<Vec<_>>(), [4], "the Warden's gate");
    assert!(career.band_open(&c, "outer"));
}

/// Galaxy plan E8 (GDR-0098): the last signal of a trail done gives its reward, once: a cache's research data and
/// materials, a lore fragment for the memory log, a gate; found alone, or an earlier signal done, gives nothing.
#[test]
fn a_trails_end_gives_its_reward() {
    let c = content();
    let mut sk = skeleton();
    sk.trails[0].lore = Some("last_launch".into());
    sk.clusters[3].gate = true;
    let mut career = Career::from_ron("(start: \"rim_awakening\", seed: 4207, generator: 2)").unwrap();
    assert!(career.reconcile(&c).is_empty());
    career.begin_galaxy(&c, sk);
    assert!(career.hear(LOG));
    let store = |career: &Career| (career.data().get("logic").copied(), career.materials().get("programmable_matter").copied());
    assert_eq!(store(&career), (None, None));
    assert!(career.found(&c, LOG, false));
    assert_eq!(store(&career), (None, None), "found is no reward");
    assert!(career.found(&c, LOG, true));
    // The trail is a precursor cache: `trails.ron` gives logic 8, propulsion 8, programmable matter 4.
    assert_eq!(store(&career), (Some(8.0), Some(4)));
    let g = career.exploration().unwrap();
    assert_eq!(g.memories(), ["last_launch"]);
    assert_eq!(g.gates().iter().copied().collect::<Vec<_>>(), [3]);
    assert!(!career.found(&c, LOG, true), "once");
    assert_eq!(store(&career), (Some(8.0), Some(4)));
    // The memory log is saved and survives a reload; a fragment the content lost is dropped.
    let mut back = Career::from_ron(&career.to_ron()).unwrap();
    assert!(back.reconcile(&c).is_empty());
    assert_eq!(back, career);
}

/// The galaxy is saved with the career and reads back equal; a cluster still underway at load counts as lost.
#[test]
fn the_galaxy_is_saved_and_a_voyage_underway_is_lost_at_load() {
    let c = content();
    let mut career = game(&c, "");
    career.cluster_launched(&c, 1).unwrap();
    career.cluster_ended(&c, 1, VoyageEnd::Recalled { haul: core_haul(), cleared: false, ticks: 10 }).unwrap();
    career.decode(0);
    career.narrow(&c, LOG, 2);
    career.open_gate(4);
    career.cluster_launched(&c, 4).unwrap();
    let text = career.to_ron();
    assert!(text.contains("galaxy: (") && text.contains("skeleton: ("), "{text}");
    let mut back = Career::from_ron(&text).unwrap();
    assert_eq!(back, career);
    let notes = back.reconcile(&c);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].field.as_deref(), Some("galaxy.underway"));
    let g = back.exploration().unwrap();
    assert_eq!(g.underway(), None);
    assert_eq!(back.last_voyage().map(|v| (v.cluster, v.outcome)), Some((Some(4), Outcome::Lost)));
    assert_eq!(back.history().len(), 2, "the cluster voyages stay in the log");
    let again = back.clone();
    assert!(back.reconcile(&c).is_empty());
    assert_eq!(back, again);
}

/// Galaxy plan E6 (GDR-0097) on the skeleton with a free rim cluster, an outer one and a beacon's trail: a beacon
/// scanned is heard; a scan narrows the signals whose area holds the cluster, once a voyage; its site seen is found,
/// cleared done; blind jumps go to the nearest cluster in reach, never home or a slice voyage's, launching only into
/// an open band;
/// the warp takes 3 s + 2 s per S.
#[test]
fn beacons_scans_finds_and_blind_jumps() {
    let c = content();
    let mut sk = skeleton();
    let free = |name: &str, x: i32, y: i32, band: &str| Cluster { name: name.into(), band: band.into(), kind: "wreck_drift".into(), at: Point { x, y }, seed: 5, gate: false };
    sk.clusters.push(free("Five", -150, -150, "rim"));
    sk.clusters.push(free("Deep", 0, -150, "outer"));
    sk.trails.push(TrailPlan {
        trail: "memory".into(),
        band: "rim".into(),
        lore: None,
        places: vec![Place { cluster: 5, encounter: None }, Place { cluster: 2, encounter: Some("precursor_wrecks".into()) }],
        signals: vec![Signal { source: "old_beacon".into(), area: Area { centre: Point { x: 250, y: 0 }, radius: 200 } }],
    });
    let mut career = Career::from_ron("(start: \"rim_awakening\", seed: 4207, generator: 2)").unwrap();
    assert!(career.reconcile(&c).is_empty());
    career.begin_galaxy(&c, sk);
    assert_eq!(career.exploration().unwrap().pins().values().copied().collect::<std::collections::BTreeSet<_>>(), [1, 2, 3, 4].into());
    let beacon = TrailSignal { trail: 1, signal: 0 };
    assert_eq!((data::career::signal_to((1, 1)), data::career::signal_to((1, 0))), (Some(beacon), None));

    // A log's place sends no beacon; the beacon's is heard once.
    assert_eq!(career.beacon(&c, (0, 0)), None);
    assert_eq!(career.beacon(&c, (1, 0)), Some(beacon));
    assert_eq!(career.beacon(&c, (1, 0)), None, "heard already");

    // Scans: in cluster 1 (inside its area), once a voyage; not in cluster 5 (outside it).
    let mut far = career.clone();
    assert!(far.scanned(&c, 5, &[]).is_empty());
    let narrowed = career.scanned(&c, 1, &[]);
    assert_eq!(narrowed.iter().map(|n| n.0).collect::<Vec<_>>(), [beacon]);
    assert!(career.scanned(&c, 1, &[beacon]).is_empty());

    // Found, then done; never back.
    assert!(career.found(&c, beacon, false));
    assert!(!career.found(&c, beacon, false));
    let s = career.exploration().unwrap().signal(beacon).unwrap().clone();
    assert_eq!((s.state, s.area), (SignalState::Found, Area { centre: Point { x: 300, y: 0 }, radius: 0 }));
    assert!(career.found(&c, beacon, true));
    assert_eq!(career.exploration().unwrap().signal(beacon).unwrap().state, SignalState::Done);
    assert!(!career.found(&c, LOG, true), "not heard");
    assert!(career.scanned(&c, 1, &[]).is_empty(), "done");

    // Blind jumps: to the cluster nearest the spot, Five, or Deep in the outer band, which opens with the gate.
    assert_eq!(career.blind_target(&c, Point { x: -120, y: -150 }), Some(5));
    let spot = Point { x: -20, y: -140 };
    assert_eq!(career.blind_target(&c, spot), Some(6));
    assert!(!career.band_open(&c, "outer") && career.band_open(&c, "rim"));
    assert_eq!(career.cluster_launched(&c, 6), Err(Refusal::BandClosed(6)));
    assert!(career.open_gate(4));
    assert!(career.band_open(&c, "outer"));
    assert_eq!(career.warp_seconds(&c, 5).map(|s| (s * 100.0).round()), Some(724.0));
    assert_eq!(career.warp_seconds(&c, 3), None, "out of reach");
    assert_eq!(career.blind_target(&c, Point { x: -400, y: -400 }), None, "empty space: no cluster near");
}
