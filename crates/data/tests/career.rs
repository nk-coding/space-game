//! The career model (`data::career`, `docs/career.md`): a new game, unlocks and mind capacity, unloading a haul,
//! research and its refusals, launching and ending voyages, voyage seeds (fixed values), voyage-readiness, the save
//! as RON, loading a save against changed content, and a scripted evening.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::*;
use data::career::{
    Career, DATA_EPSILON, FleetSlot, GENERATOR, Haul, HaulItem, HaulReport, Log, MAX_DATA,
    MAX_MATERIALS, Outcome, Readiness, Refusal, Shortfall, Unlocks, VoyageEnd, unload,
    voyage_ready,
};
use data::{Blueprint, Content, Kind, Pack, Problem, Severity};

const START: &str = "rim_awakening";
const FIRST: &str = "yard_approach";

fn part(id: &str) -> HaulItem {
    HaulItem::Part {
        part: id.into(),
        material: None,
    }
}

fn hull(id: &str, material: &str) -> HaulItem {
    HaulItem::Part {
        part: id.into(),
        material: Some(material.into()),
    }
}

fn fragment(id: &str) -> HaulItem {
    HaulItem::Fragment { part: id.into() }
}

fn haul(items: &[(HaulItem, u32)]) -> Haul {
    items.iter().cloned().collect()
}

fn map<V: Copy>(entries: &[(&str, V)]) -> BTreeMap<String, V> {
    entries.iter().map(|&(k, v)| (k.to_string(), v)).collect()
}

fn new_game(c: &Content) -> Career {
    Career::new(c, START, 4207).unwrap()
}

/// Flies the first voyage and recalls home with `items` (goal not cleared).
fn bring_home(career: &mut Career, c: &Content, items: &[(HaulItem, u32)]) -> HaulReport {
    career.voyage_launched(c, FIRST).unwrap();
    let end = VoyageEnd::Recalled {
        haul: haul(items),
        cleared: false,
        ticks: 9000,
    };
    career.voyage_ended(c, FIRST, end).unwrap().report.clone()
}

fn ship(c: &Content, id: &str) -> Blueprint {
    c.ships.get(id).unwrap().clone()
}

fn new_game_unlocks(c: &Content) -> Unlocks {
    new_game(c).unlocks(c)
}

/// The notes about one field.
fn note<'a>(notes: &'a [Problem], field: &str) -> &'a Problem {
    notes
        .iter()
        .find(|p| p.field.as_deref() == Some(field))
        .unwrap_or_else(|| panic!("no note for `{field}`:\n{}", show(notes)))
}

const MOD_MANIFEST: &str =
    r#"(id: "test_mod", name: "Test mod", version: "1.0.0", schema_version: 1, depends: ["base"])"#;

/// Base + a mod made of `files`.
fn with_mod(files: &[(&str, &str)]) -> Content {
    let mut all = vec![("pack.ron", MOD_MANIFEST)];
    all.extend_from_slice(files);
    let m = Pack::from_files(all).unwrap_or_else(|p| panic!("{}", show(&p)));
    Content::load(&[data::base_pack().unwrap(), m]).unwrap_or_else(|p| panic!("{}", show(&p)))
}

/// A 1×1 logic-category part `id` whose salvage block gives `data` of `family`.
fn salvage_part(id: &str, family: &str, data: f32) -> String {
    format!(
        r#"(id: "{id}", name: "{id}", summary: "S.", category: "logic", size: (1, 1), build_points: 1, mass: 1, hp: 1,
            salvage: (family: "{family}", data: {data}))"#
    )
}

// ---------------------------------------------------------------------------------------------------------------
// A new game

#[test]
fn a_new_game_has_the_early_game_kit() {
    let c = content();
    let career = new_game(&c);
    assert_eq!(
        (career.start(), career.seed(), career.generator()),
        (START, 4207, GENERATOR)
    );
    assert_eq!(career.packs(), [("base".to_string(), "0.1.0".to_string())]);
    assert!(career.researched().is_empty() && career.data().is_empty());
    assert!(career.materials().is_empty() && career.escorts().is_empty());
    assert_eq!(career.log(), &Log::default());
    assert!(career.last_haul().is_none() && career.underway().is_none());
    assert!(career.voyage(FIRST).is_none());

    // Everything the content does not lock, nothing it does.
    let u = career.unlocks(&c);
    let open = |ids: Vec<(&String, bool)>| -> BTreeSet<String> {
        ids.into_iter()
            .filter(|(_, locked)| !locked)
            .map(|(id, _)| id.clone())
            .collect()
    };
    assert_eq!(
        u.parts,
        open(c.parts.iter().map(|p| (&p.id, p.locked)).collect())
    );
    assert_eq!(
        u.materials,
        open(c.materials.iter().map(|m| (&m.id, m.locked)).collect())
    );
    assert!(u.has_part("pulse_laser") && u.has_part("logic_core") && u.has_part("cargo_hold"));
    assert!(!u.has_part("beam_laser") && !u.has_part("automaton_cannon"));
    assert!(u.has_material("steel") && u.has_material("programmable_matter"));
    assert!(!u.has_material("composite"));
    assert_eq!(u.frames, BTreeSet::from(["frigate".to_string()]));
    assert!(u.has_frame("frigate") && !u.has_frame("cruiser"));
    assert_eq!(career.mind_capacity(&c), 2);
    assert_eq!(career.open_voyages(&c), [FIRST]);
    assert!(career.is_open(&c, FIRST) && !career.is_open(&c, "halvex_drift"));

    assert_eq!(
        Career::new(&c, "drone_dawn", 1),
        Err(Refusal::UnknownStart("drone_dawn".into()))
    );
}

#[test]
fn unlock_all_is_everything_in_the_content() {
    let c = content();
    let u = Unlocks::all(&c);
    assert_eq!(u.parts.len(), c.parts.len());
    assert_eq!(u.materials.len(), c.materials.len());
    assert_eq!(u.frames.len(), c.frames.len());
    assert!(u.has_part("automaton_core") && u.has_material("reactive") && u.has_frame("capital"));
}

// ---------------------------------------------------------------------------------------------------------------
// Unloading a haul

#[test]
fn unload_turns_a_haul_into_data_and_materials() {
    let c = content();
    let report = unload(
        &c,
        &haul(&[
            (part("automaton_cannon"), 4),
            (part("thruster"), 2),
            (part("logic_core"), 1),
            (part("automaton_core"), 2),
            // Hull: per cell of area in its material; slopes count half a cell.
            (hull("armor_block", "steel"), 3),
            (hull("slope_1x1", "steel"), 2),
            (hull("slope_1x2", "composite"), 1),
            (fragment("automaton_core"), 3),
        ]),
    );
    assert_eq!(
        report.data,
        map(&[
            ("hull", 3.0 + 2.0 * 0.5 + 1.5),
            ("kinetic", 12.0),
            ("logic", 3.0 + 8.0),
            ("propulsion", 6.0),
        ])
    );
    // Cores give programmable matter, and so do the fragments destroyed cores leave (but no data).
    assert_eq!(report.materials, map(&[("programmable_matter", 3 + 3)]));
    assert_eq!(
        report.parts,
        map(&[
            ("armor_block", 3),
            ("automaton_cannon", 4),
            ("automaton_core", 2),
            ("logic_core", 1),
            ("slope_1x1", 2),
            ("slope_1x2", 1),
            ("thruster", 2),
        ])
    );
    assert_eq!(report.fragments, map(&[("automaton_core", 3)]));
    assert_eq!(report.items(), 15 + 3);
    // The same items in a different order make the same haul and the same report.
    let mut again = Haul::default();
    again.add(fragment("automaton_core"), 1);
    again.add(hull("slope_1x2", "composite"), 1);
    again.add(fragment("automaton_core"), 2);
    let a = unload(&c, &again);
    let b = unload(
        &c,
        &haul(&[
            (hull("slope_1x2", "composite"), 1),
            (fragment("automaton_core"), 3),
        ]),
    );
    assert_eq!(a, b);
}

#[test]
fn unload_gives_nothing_for_what_it_cannot_value() {
    let c = content();
    let report = unload(
        &c,
        &haul(&[
            (part("warp_core"), 2),                          // unknown to the content
            (fragment("warp_core"), 1),                      // unknown too
            (part("armor_block"), 1),                        // a hull part without its material
            (hull("armor_block", "wood"), 1),                // an unknown material
            (hull("armor_block", "programmable_matter"), 1), // not an armour material: no salvage block
            (fragment("thruster"), 4),                       // leaves no fragment materials
            (hull("pulse_laser", "steel"), 1), // not a hull part: its own block, no material
            (part("radiator"), 0),             // nothing at all
        ]),
    );
    assert_eq!(report.data, map(&[("energy", 3.0)]));
    assert!(report.materials.is_empty());
    // Whatever came home is listed, worth something or not.
    assert_eq!(
        report.parts,
        map(&[("armor_block", 3), ("pulse_laser", 1), ("warp_core", 2)])
    );
    assert_eq!(report.fragments, map(&[("thruster", 4), ("warp_core", 1)]));
}

#[test]
fn amounts_stay_bounded() {
    let c = content();
    let huge = [(part("automaton_core"), u32::MAX)];
    let report = unload(&c, &haul(&huge));
    assert_eq!(report.data["logic"], MAX_DATA);
    assert_eq!(report.materials["programmable_matter"], MAX_MATERIALS);
    let mut career = new_game(&c);
    bring_home(&mut career, &c, &huge);
    bring_home(&mut career, &c, &huge);
    assert_eq!(career.data()["logic"], MAX_DATA);
    assert_eq!(career.materials()["programmable_matter"], MAX_MATERIALS);
    assert_eq!(career.log().salvaged, u32::MAX);
    // Counts saturate in a haul too.
    let mut h = Haul::default();
    h.add(part("thruster"), u32::MAX);
    h.add(part("thruster"), 5);
    assert_eq!(h.items[&part("thruster")], u32::MAX);
}

// ---------------------------------------------------------------------------------------------------------------
// Research

#[test]
fn research_pays_and_unlocks_at_once() {
    let c = content();
    let mut career = new_game(&c);
    bring_home(&mut career, &c, &[(part("automaton_cannon"), 5)]);
    assert_eq!(career.data()["kinetic"], 15.0);
    assert_eq!(career.can_pay(&c, "automaton_cannon"), Ok(()));
    let node = career.research(&c, "automaton_cannon").unwrap();
    assert_eq!(node.id, "automaton_cannon");
    assert_eq!(career.data()["kinetic"], 3.0);
    assert_eq!(career.researched(), ["automaton_cannon"]);
    assert!(career.is_researched("automaton_cannon"));
    assert!(career.unlocks(&c).has_part("automaton_cannon"));

    // Paying a store down to nothing removes it; materials are paid the same way.
    bring_home(
        &mut career,
        &c,
        &[(part("automaton_core"), 2), (part("automaton_cannon"), 1)],
    );
    assert_eq!(career.data()["logic"], 8.0);
    assert_eq!(career.materials()["programmable_matter"], 2);
    career.research(&c, "tamed_core").unwrap();
    assert!(!career.data().contains_key("logic"));
    assert_eq!(career.data()["kinetic"], 6.0);
    assert_eq!(career.materials()["programmable_matter"], 1);
    assert_eq!(career.researched(), ["automaton_cannon", "tamed_core"]);
    assert!(career.unlocks(&c).has_part("automaton_core"));
}

#[test]
fn research_refusals() {
    let c = content();
    let mut career = new_game(&c);
    assert_eq!(
        career.can_pay(&c, "warp_theory"),
        Err(Refusal::UnknownNode("warp_theory".into()))
    );
    assert_eq!(
        career.can_pay(&c, "mind_expansion_1"),
        Err(Refusal::Requires(vec!["tamed_core".into()]))
    );
    assert_eq!(
        career.can_pay(&c, "mind_expansion_2"),
        Err(Refusal::Requires(vec!["mind_expansion_1".into()]))
    );
    // Every cost that falls short, data first.
    assert_eq!(
        career.can_pay(&c, "tamed_core"),
        Err(Refusal::CannotPay(vec![
            Shortfall::Data {
                family: "logic".into(),
                have: 0.0,
                cost: 8.0
            },
            Shortfall::Material {
                material: "programmable_matter".into(),
                have: 0,
                cost: 1
            },
        ]))
    );
    bring_home(
        &mut career,
        &c,
        &[(part("logic_core"), 1), (fragment("automaton_core"), 5)],
    );
    assert_eq!(
        career.can_pay(&c, "tamed_core"),
        Err(Refusal::CannotPay(vec![Shortfall::Data {
            family: "logic".into(),
            have: 3.0,
            cost: 8.0
        }]))
    );
    // A refused research changes nothing.
    let before = career.clone();
    assert!(career.research(&c, "tamed_core").is_err());
    assert!(career.research(&c, "warp_theory").is_err());
    assert_eq!(career, before);

    bring_home(&mut career, &c, &[(part("logic_core"), 2)]);
    career.research(&c, "tamed_core").unwrap();
    assert_eq!(
        career.research(&c, "tamed_core").unwrap_err(),
        Refusal::AlreadyResearched("tamed_core".into())
    );
    // The requirement is met now; only the cost is missing.
    assert!(matches!(
        career.can_pay(&c, "mind_expansion_1"),
        Err(Refusal::CannotPay(_))
    ));
}

#[test]
fn data_within_epsilon_of_a_cost_pays_it() {
    let c = with_mod(&[(
        "parts/dust.ron",
        &format!(
            "[{}, {}]",
            salvage_part("near", "kinetic", 12.0 - DATA_EPSILON / 2.0),
            salvage_part("short", "kinetic", 11.99)
        ),
    )]);
    let mut career = new_game(&c);
    bring_home(&mut career, &c, &[(part("short"), 1)]);
    assert!(matches!(
        career.can_pay(&c, "automaton_cannon"),
        Err(Refusal::CannotPay(_))
    ));
    let mut career = new_game(&c);
    bring_home(&mut career, &c, &[(part("near"), 1)]);
    career.research(&c, "automaton_cannon").unwrap();
    assert!(career.data().is_empty(), "{:?}", career.data());
}

// ---------------------------------------------------------------------------------------------------------------
// Voyages

#[test]
fn voyages_launch_when_open_and_end_once() {
    let c = content();
    let mut career = new_game(&c);
    assert_eq!(
        career.voyage_launched(&c, "halvex_drift"),
        Err(Refusal::NotOpen("halvex_drift".into()))
    );
    assert_eq!(
        career.voyage_ended(&c, FIRST, VoyageEnd::Lost { ticks: 1 }),
        Err(Refusal::NotUnderway(FIRST.into()))
    );
    let next = career.voyage_seed(FIRST);
    assert_eq!(career.voyage_launched(&c, FIRST), Ok(next));
    assert_eq!(career.underway(), Some(FIRST));
    assert_eq!(career.voyage(FIRST).unwrap().launched, 1);
    assert_eq!(career.log().voyages, 1);
    assert_ne!(career.voyage_seed(FIRST), next, "every launch rolls anew");
    assert_eq!(
        career.voyage_launched(&c, FIRST),
        Err(Refusal::Underway(FIRST.into()))
    );
    assert_eq!(
        career.voyage_ended(&c, "halvex_drift", VoyageEnd::Lost { ticks: 1 }),
        Err(Refusal::NotUnderway("halvex_drift".into()))
    );

    // Lost: the haul is gone (nothing to unload), the goal does not count.
    let last = career
        .voyage_ended(&c, FIRST, VoyageEnd::Lost { ticks: 600 })
        .unwrap();
    assert_eq!(
        (last.voyage.as_str(), last.outcome, last.ticks),
        (FIRST, Outcome::Lost, 600)
    );
    assert_eq!(last.report, HaulReport::default());
    assert_eq!(career.underway(), None);
    assert!(career.data().is_empty());
    assert_eq!((career.log().lost, career.log().recalled), (1, 0));
    assert!(!career.voyage(FIRST).unwrap().cleared);
    assert_eq!(
        career.voyage_ended(&c, FIRST, VoyageEnd::Lost { ticks: 1 }),
        Err(Refusal::NotUnderway(FIRST.into()))
    );

    // Recalled: cleared once stays cleared; the best time is the quickest clear.
    let fly = |career: &mut Career, cleared: bool, ticks: u64| {
        career.voyage_launched(&c, FIRST).unwrap();
        let end = VoyageEnd::Recalled {
            haul: Haul::default(),
            cleared,
            ticks,
        };
        career.voyage_ended(&c, FIRST, end).unwrap().outcome
    };
    assert_eq!(fly(&mut career, false, 5000), Outcome::Recalled);
    assert_eq!(fly(&mut career, true, 12000), Outcome::Cleared);
    assert_eq!(fly(&mut career, true, 15000), Outcome::Cleared);
    assert_eq!(fly(&mut career, false, 3000), Outcome::Recalled);
    let record = career.voyage(FIRST).unwrap();
    assert_eq!(
        (record.launched, record.cleared, record.best_ticks),
        (5, true, Some(12000))
    );
    assert_eq!(fly(&mut career, true, 9000), Outcome::Cleared);
    assert_eq!(career.voyage(FIRST).unwrap().best_ticks, Some(9000));
    assert_eq!(
        career.log(),
        &Log {
            voyages: 6,
            recalled: 5,
            lost: 1,
            salvaged: 0
        }
    );
}

#[test]
fn voyage_seeds_are_fixed() {
    // Values computed independently (FNV-1a 64 and SplitMix64 by their reference definitions); they are part of
    // the save format (GENERATOR 1) and must never change.
    let c = content();
    let mut career = new_game(&c);
    assert_eq!(career.voyage_seed(FIRST), 9_993_844_787_240_758_234);
    assert_eq!(
        career.voyage_seed("halvex_drift"),
        6_951_312_451_671_714_856
    );
    assert_eq!(
        Career::new(&c, START, 4208).unwrap().voyage_seed(FIRST),
        1_757_091_447_161_648_217
    );
    assert_eq!(
        Career::new(&c, START, 0).unwrap().voyage_seed(""),
        2_448_385_507_222_971_125
    );
    assert_eq!(
        career.voyage_launched(&c, FIRST),
        Ok(9_993_844_787_240_758_234)
    );
    assert_eq!(career.voyage_seed(FIRST), 8_810_720_374_438_629_928);
    // The save carries everything the seed needs.
    let back = Career::from_ron(&career.to_ron()).unwrap();
    assert_eq!(back.voyage_seed(FIRST), 8_810_720_374_438_629_928);
}

// ---------------------------------------------------------------------------------------------------------------
// Voyage-readiness

#[test]
fn the_starter_ship_is_voyage_ready() {
    let c = content();
    let skiff = ship(&c, "rim_skiff");
    assert_eq!(voyage_ready(&c, &new_game_unlocks(&c), 2, &skiff, &[]), []);
}

#[test]
fn locked_parts_and_materials_are_named_once_per_ship() {
    let c = content();
    let u = new_game_unlocks(&c);
    let runner = ship(&c, "halvex_runner");
    let flag = FleetSlot::Flagship;
    let found = voyage_ready(&c, &u, 2, &runner, &[]);
    assert_eq!(
        found,
        [
            Readiness::LockedPart {
                ship: flag,
                part: "beam_laser".into()
            },
            Readiness::LockedPart {
                ship: flag,
                part: "fusion_reactor".into()
            },
            Readiness::LockedPart {
                ship: flag,
                part: "torch_drive".into()
            },
            Readiness::LockedMaterial {
                ship: flag,
                material: "composite".into()
            },
        ]
    );
    assert!(found.iter().all(Readiness::blocks));
    assert_eq!(voyage_ready(&c, &Unlocks::all(&c), 2, &runner, &[]), []);
    // A part the content doesn't have counts as locked.
    let mut odd = ship(&c, "rim_skiff");
    odd.parts[2].part = "warp_core".into();
    assert_eq!(
        voyage_ready(&c, &u, 2, &odd, &[]),
        [Readiness::LockedPart {
            ship: flag,
            part: "warp_core".into()
        }]
    );
}

#[test]
fn escorts_count_against_the_mind() {
    let c = content();
    let u = new_game_unlocks(&c);
    let skiff = ship(&c, "rim_skiff");
    assert_eq!(
        voyage_ready(&c, &u, 2, &skiff, std::slice::from_ref(&skiff)),
        [Readiness::TooManyCores {
            cores: 4,
            capacity: 2
        }]
    );
    assert_eq!(
        voyage_ready(&c, &u, 4, &skiff, std::slice::from_ref(&skiff)),
        []
    );
    // An escort's locked parts name the escort.
    let raider = ship(&c, "automaton_raider");
    let found = voyage_ready(&c, &u, 4, &skiff, &[skiff.clone(), raider]);
    let escort = FleetSlot::Escort(1);
    assert_eq!(
        found,
        [
            Readiness::LockedPart {
                ship: escort,
                part: "automaton_cannon".into()
            },
            Readiness::LockedPart {
                ship: escort,
                part: "automaton_core".into()
            },
            Readiness::LockedMaterial {
                ship: escort,
                material: "automaton_plate".into()
            },
            Readiness::TooManyCores {
                cores: 5,
                capacity: 4
            },
        ]
    );
}

#[test]
fn no_cargo_is_only_a_warning() {
    let c = content();
    let raider = ship(&c, "automaton_raider");
    let all = Unlocks::all(&c);
    let found = voyage_ready(&c, &all, 2, &raider, &[]);
    assert_eq!(found, [Readiness::NoCargo]);
    assert!(!found[0].blocks());
    // One ship with a hold is enough.
    assert_eq!(
        voyage_ready(&c, &all, 3, &raider, &[ship(&c, "rim_skiff")]),
        []
    );
}

#[test]
fn a_ship_must_fit_an_unlocked_frame() {
    let c = content();
    let mut u = new_game_unlocks(&c);
    let mut big = ship(&c, "rim_skiff");
    big.size = (30, 30);
    let flag = FleetSlot::Flagship;
    assert_eq!(
        voyage_ready(&c, &u, 2, &big, &[]),
        [Readiness::LockedFrame {
            ship: flag,
            size: (30, 30),
            frame: Some("cruiser".into())
        }]
    );
    let mut huge = big.clone();
    huge.size = (100, 24);
    assert_eq!(
        voyage_ready(&c, &u, 2, &huge, &[]),
        [Readiness::LockedFrame {
            ship: flag,
            size: (100, 24),
            frame: None
        }]
    );
    u.frames.insert("cruiser".into());
    assert_eq!(voyage_ready(&c, &u, 2, &big, &[]), []);
}

// ---------------------------------------------------------------------------------------------------------------
// The save

#[test]
fn the_save_round_trips_through_ron() {
    let c = content();
    let fresh = new_game(&c);
    assert_eq!(Career::from_ron(&fresh.to_ron()).unwrap(), fresh);

    let mut career = an_evening(&c);
    career.set_escorts(vec!["drone_wing".into(), "drone_wing".into()]);
    career.voyage_launched(&c, FIRST).unwrap();
    let text = career.to_ron();
    let back = Career::from_ron(&text).unwrap();
    assert_eq!(back, career);
    assert_eq!(back.to_ron(), text, "writing is deterministic");
    assert_eq!(back.escorts(), ["drone_wing", "drone_wing"]);
    assert_eq!(back.underway(), Some(FIRST));
}

#[test]
fn a_hand_written_save_reads() {
    // The shape of docs/career.md: comments, `Some` left out, fields with defaults left out.
    let text = r#"
// space-game: saves, format 1, saved 1760351234
(
    packs: [("base", "0.1.0")],
    start: "rim_awakening",
    seed: 4207,
    generator: 1,
    researched: ["automaton_cannon", "composite_plating"],
    data: {"kinetic": 4, "hull": 11.5, "logic": 2},
    materials: {"programmable_matter": 1},
    escorts: ["drone_wing"],
    voyages: {
        "yard_approach": (launched: 2, cleared: true, best_ticks: 13500),
    },
    log: (voyages: 3, lost: 1, salvaged: 34),
    last_haul: (voyage: "yard_approach", outcome: Recalled, ticks: 8000,
                report: (parts: {"automaton_cannon": 3, "thruster": 2}, data: {"kinetic": 9, "propulsion": 6})),
)
"#;
    let c = content();
    let mut career = Career::from_ron(text).unwrap_or_else(|p| panic!("{}", show(&p)));
    assert!(career.reconcile(&c).is_empty());
    assert_eq!(
        career.researched(),
        ["automaton_cannon", "composite_plating"]
    );
    assert_eq!(career.data()["hull"], 11.5);
    assert_eq!(career.voyage(FIRST).unwrap().best_ticks, Some(13500));
    assert_eq!(career.log().recalled, 0);
    let last = career.last_haul().unwrap();
    assert_eq!(last.report.data["propulsion"], 6.0);
    assert!(last.report.materials.is_empty());
    let u = career.unlocks(&c);
    assert!(u.has_part("automaton_cannon") && u.has_material("composite"));
}

#[test]
fn a_broken_save_is_an_error_with_its_place() {
    let p = Career::from_ron(r#"(start: "rim_awakening", seed: 1, generator: 1, credits: 5)"#)
        .unwrap_err();
    assert_eq!(p.len(), 1);
    assert!(p[0].message.contains("credits"), "{}", show(&p));
    assert_eq!(p[0].severity, Severity::Error);
    assert_eq!(p[0].line, Some((1, 49)));
    let p = Career::from_ron(r#"(start: "rim_awakening", generator: 1)"#).unwrap_err();
    assert!(p[0].message.contains("seed"), "{}", show(&p));
    assert!(Career::from_ron("(start: ").is_err());
}

// ---------------------------------------------------------------------------------------------------------------
// Loading against changed content

#[test]
fn reconcile_drops_what_the_content_no_longer_has() {
    let text = r#"(
    packs: [("base", "0.1.0"), ("old_mod", "2.0.0")],
    start: "drone_dawn",
    seed: 9,
    generator: 7,
    researched: ["automaton_cannon", "warp_theory", "automaton_cannon"],
    data: {"kinetic": 4, "lasers": 2, "hull": NaN, "power": -3, "energy": 1e12, "thermal": 0, "shields": inf},
    materials: {"programmable_matter": 5000000, "unobtainium": 2, "steel": 0},
    voyages: {"yard_approach": (launched: 2), "nowhere": (launched: 1, cleared: true)},
    last_haul: (voyage: "nowhere", outcome: Cleared, report: (data: {"kinetic": 3})),
)"#;
    let c = content();
    let mut career = Career::from_ron(text).unwrap_or_else(|p| panic!("{}", show(&p)));
    let notes = career.reconcile(&c);
    assert!(notes.iter().all(|n| n.severity == Severity::Warning));
    assert!(note(&notes, "packs").message.contains("old_mod"));
    let start = note(&notes, "start");
    assert_eq!(
        (start.kind, start.id.as_deref()),
        (Some(Kind::Start), Some("drone_dawn"))
    );
    assert!(start.message.contains("`rim_awakening`"), "{start}");
    assert!(note(&notes, "generator").message.contains('7'));
    assert_eq!(
        note(&notes, "researched[1]").id.as_deref(),
        Some("warp_theory")
    );
    assert!(note(&notes, "researched[2]").message.contains("twice"));
    for field in [
        "data.lasers",
        "data.hull",
        "data.power",
        "data.shields",
        "data.energy",
        "materials.unobtainium",
        "materials.programmable_matter",
        "voyages.nowhere",
        "last_haul",
    ] {
        note(&notes, field);
    }
    assert_eq!(note(&notes, "data.lasers").kind, Some(Kind::Family));
    assert_eq!(
        note(&notes, "materials.unobtainium").kind,
        Some(Kind::Material)
    );
    assert_eq!(notes.len(), 14, "{}", show(&notes));

    assert_eq!(career.packs(), [("base".to_string(), "0.1.0".to_string())]);
    assert_eq!((career.start(), career.generator()), (START, GENERATOR));
    assert_eq!(career.researched(), ["automaton_cannon"]);
    assert_eq!(
        career.data(),
        &map(&[("energy", MAX_DATA), ("kinetic", 4.0)])
    );
    assert_eq!(
        career.materials(),
        &map(&[("programmable_matter", MAX_MATERIALS)])
    );
    assert!(career.voyage("nowhere").is_none());
    assert_eq!(career.voyage(FIRST).unwrap().launched, 2);
    assert!(career.last_haul().is_none());
    // A second pass finds nothing more.
    assert!(career.reconcile(&c).is_empty());
}

#[test]
fn closing_the_game_mid_voyage_loses_it() {
    let c = content();
    let mut career = new_game(&c);
    bring_home(&mut career, &c, &[(part("thruster"), 1)]);
    career.voyage_launched(&c, FIRST).unwrap();
    // The game writes the save at launch, then closes.
    let mut loaded = Career::from_ron(&career.to_ron()).unwrap();
    let notes = loaded.reconcile(&c);
    assert!(note(&notes, "underway").message.contains("counts as lost"));
    assert_eq!(notes.len(), 1);
    assert_eq!(loaded.underway(), None);
    assert_eq!((loaded.log().lost, loaded.log().recalled), (1, 1));
    let last = loaded.last_haul().unwrap();
    assert_eq!((last.outcome, last.ticks), (Outcome::Lost, 0));
    assert_eq!(loaded.data()["propulsion"], 3.0);
    // The next launch rolls a new voyage.
    assert_eq!(loaded.voyage(FIRST).unwrap().launched, 2);

    // Underway without a launch on record (a hand-edited save): just dropped.
    let mut odd = Career::from_ron(
        r#"(start: "rim_awakening", seed: 1, generator: 1, underway: "yard_approach")"#,
    )
    .unwrap();
    let notes = odd.reconcile(&c);
    assert!(note(&notes, "underway").message.contains("no launch"));
    assert_eq!((odd.underway(), odd.log().lost), (None, 0));
}

#[test]
fn a_removed_mod_takes_its_research_with_it() {
    let modded = with_mod(&[
        (
            "families/missiles.ron",
            r#"(id: "missiles", name: "Missiles", summary: "Guided shots.")"#,
        ),
        (
            "parts/rack.ron",
            &salvage_part("seeker_rack", "missiles", 5.0),
        ),
        (
            "research/seeker.ron",
            r#"(id: "seeker", name: "Seeker", summary: "S.", family: "missiles", kind: Refinement,
                cost: (data: {"missiles": 5}), unlocks: [MindCapacity(1)])"#,
        ),
        (
            "starts/alt.ron",
            r#"(id: "alt", name: "Alt", summary: "S.", ship: "rim_skiff", mind_capacity: 3, voyages: ["yard_approach"])"#,
        ),
    ]);
    let mut career = Career::new(&modded, "alt", 5).unwrap();
    assert_eq!(career.packs().len(), 2);
    bring_home(&mut career, &modded, &[(part("seeker_rack"), 1)]);
    career.research(&modded, "seeker").unwrap();
    assert_eq!(career.mind_capacity(&modded), 4);
    bring_home(&mut career, &modded, &[(part("seeker_rack"), 2)]);
    assert_eq!(career.data()["missiles"], 10.0);

    // The same save against the base content alone: the queries cope before it is reconciled...
    let c = content();
    let mut loaded = Career::from_ron(&career.to_ron()).unwrap();
    assert_eq!(loaded.mind_capacity(&c), 0);
    assert!(loaded.open_voyages(&c).is_empty());
    assert!(!loaded.unlocks(&c).has_part("beam_laser"));
    // ...and reconciling drops the mod's node, family and start.
    let notes = loaded.reconcile(&c);
    assert!(note(&notes, "packs").message.contains("test_mod"));
    assert_eq!(note(&notes, "researched[0]").id.as_deref(), Some("seeker"));
    note(&notes, "data.missiles");
    note(&notes, "last_haul.report.data.missiles");
    assert_eq!(note(&notes, "start").id.as_deref(), Some("alt"));
    assert_eq!(notes.len(), 5, "{}", show(&notes));
    assert_eq!(loaded.mind_capacity(&c), 2);
    assert!(loaded.researched().is_empty() && loaded.data().is_empty());
    // The debrief keeps what came home.
    assert_eq!(
        loaded.last_haul().unwrap().report.parts,
        map(&[("seeker_rack", 2)])
    );
}

// ---------------------------------------------------------------------------------------------------------------
// An evening

/// New game → unload a haul → research the cannon → it is unlocked → mind expansion → capacity 3.
fn an_evening(c: &Content) -> Career {
    let mut career = new_game(c);
    let skiff = ship(c, "rim_skiff");
    assert_eq!(
        voyage_ready(c, &career.unlocks(c), career.mind_capacity(c), &skiff, &[]),
        []
    );

    // Shipyard approach: raider wrecks.
    let report = bring_home(
        &mut career,
        c,
        &[
            (part("automaton_cannon"), 4),
            (hull("armor_block", "automaton_plate"), 6),
            (hull("slope_1x1", "automaton_plate"), 4),
            (fragment("automaton_core"), 2),
        ],
    );
    assert_eq!(report.data, map(&[("hull", 8.0), ("kinetic", 12.0)]));
    assert_eq!(career.materials(), &map(&[("programmable_matter", 2)]));
    let last = career.last_haul().unwrap();
    assert_eq!(
        (last.voyage.as_str(), last.outcome),
        (FIRST, Outcome::Recalled)
    );

    // The cannon.
    assert!(!career.unlocks(c).has_part("automaton_cannon"));
    career.research(c, "automaton_cannon").unwrap();
    assert!(career.unlocks(c).has_part("automaton_cannon"));
    assert_eq!(
        career.can_pay(c, "mind_expansion_1"),
        Err(Refusal::Requires(vec!["tamed_core".into()]))
    );

    // A core graveyard, goal cleared.
    career.voyage_launched(c, FIRST).unwrap();
    let end = VoyageEnd::Recalled {
        haul: haul(&[(part("automaton_core"), 5)]),
        cleared: true,
        ticks: 12000,
    };
    assert_eq!(
        career.voyage_ended(c, FIRST, end).unwrap().outcome,
        Outcome::Cleared
    );
    assert_eq!(career.data()["logic"], 20.0);
    assert_eq!(career.materials()["programmable_matter"], 7);

    // A tamed core, then room in the mind for an escort.
    career.research(c, "tamed_core").unwrap();
    assert_eq!(career.mind_capacity(c), 2);
    career.research(c, "mind_expansion_1").unwrap();
    assert_eq!(career.mind_capacity(c), 3);
    assert_eq!(
        career.researched(),
        ["automaton_cannon", "tamed_core", "mind_expansion_1"]
    );
    assert_eq!(career.data(), &map(&[("hull", 8.0), ("logic", 2.0)]));
    assert_eq!(career.materials(), &map(&[("programmable_matter", 4)]));

    // Three cores: the skiff and a one-core escort fit, two skiffs don't.
    let unlocks = career.unlocks(c);
    let mut drone = skiff.clone();
    drone.parts.retain(|p| p.part != "logic_core");
    drone.parts.push(data::Placement {
        part: "automaton_core".into(),
        x: 3,
        y: 2,
        rot: 0,
        flip: false,
        material: None,
        power: None,
    });
    let cap = career.mind_capacity(c);
    assert_eq!(voyage_ready(c, &unlocks, cap, &skiff, &[drone]), []);
    assert_eq!(
        voyage_ready(c, &unlocks, cap, &skiff, std::slice::from_ref(&skiff)),
        [Readiness::TooManyCores {
            cores: 4,
            capacity: 3
        }]
    );

    let record = career.voyage(FIRST).unwrap();
    assert_eq!(
        (record.launched, record.cleared, record.best_ticks),
        (2, true, Some(12000))
    );
    assert_eq!(
        career.log(),
        &Log {
            voyages: 2,
            recalled: 2,
            lost: 0,
            salvaged: 4 + 6 + 4 + 2 + 5
        }
    );
    career
}

#[test]
fn an_evening_of_play() {
    an_evening(&content());
}
