//! The career model (`data::career`, `docs/career.md`): a new game, unlocks and mind capacity, unloading a haul,
//! research and its refusals, launching and ending voyages, voyage seeds (fixed values), voyage-readiness, the save
//! as RON, loading a save against changed content, and a scripted evening.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::*;
use data::career::{
    Career, DATA_EPSILON, FleetSlot, GENERATOR, Haul, HaulItem, HaulReport, Log, MAX_DATA,
    MAX_HISTORY, MAX_MATERIALS, Outcome, Readiness, Refusal, Shortfall, Unlocks, VoyageEnd,
    unload, voyage_ready,
};
use data::{Blueprint, Content, Kind, Pack, Problem, Severity};

const START: &str = "rim_awakening";
const FIRST: &str = "yard_approach";

/// A part that broke when stripped (GDR-0108): dismantled at once at home, so its data counts on arrival.
fn part(id: &str) -> HaulItem {
    HaulItem::Broken {
        part: id.into(),
        material: None,
    }
}

/// An intact part: it waits in the workshop's intake.
fn intact(id: &str) -> HaulItem {
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
    assert!(career.escorts().is_empty() && career.intake().is_empty());
    // The start's materials in store, its ship's parts in stock (GDR-0108).
    let start = c.starts.get(START).unwrap();
    assert_eq!(career.materials(), &start.materials);
    assert_eq!(career.stock(), &c.ship_parts(&ship(&c, &start.ship)));
    assert_eq!(career.stock()["pulse_laser"], 2);
    assert!(!career.stock().contains_key("armor_block"), "hull is not stocked");
    assert_eq!(career.log(), &Log::default());
    assert!(career.history().is_empty() && career.underway().is_none());
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
    // Broken parts give the workshop's `broken` share (0.25) of their recipes, hull its armour per cell, summed
    // over the haul and rounded to the nearest unit once: automaton plate 4 + 1.5, copper 1 + 0.5 + 0.5 + 0.5, steel
    // 2 + 1.5 + 1, ceramic 1, crystal 0.25 + 0.5, programmable matter 0.5 + 1.5 and 3 from the fragments; composite
    // 0.25 rounds to nothing.
    assert_eq!(
        report.materials,
        map(&[("automaton_plate", 6), ("ceramic", 1), ("copper", 3), ("crystal", 1), ("programmable_matter", 2 + 3), ("steel", 5)])
    );
    assert_eq!(
        report.broken,
        map(&[("automaton_cannon", 4), ("automaton_core", 2), ("logic_core", 1), ("thruster", 2)])
    );
    assert_eq!(report.parts, map(&[("armor_block", 3), ("slope_1x1", 2), ("slope_1x2", 1)]));
    assert_eq!(report.fragments, map(&[("automaton_core", 3)]));
    assert_eq!(report.items(), 15 + 3);
    // An intact part gives nothing yet: it waits in the workshop.
    let whole = unload(&c, &haul(&[(intact("thruster"), 2)]));
    assert!(whole.data.is_empty() && whole.materials.is_empty());
    assert_eq!(whole.parts, map(&[("thruster", 2)]));
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
            (part("pulse_laser"), 1), // broken: its own block and 0.25 of its recipe
            (part("radiator"), 0),    // nothing at all
        ]),
    );
    assert_eq!(report.data, map(&[("energy", 3.0)]));
    // Copper 0.75 and crystal 0.5 round up, steel 0.25 down.
    assert_eq!(report.materials, map(&[("copper", 1), ("crystal", 1)]));
    // Whatever came home is listed, worth something or not.
    assert_eq!(report.parts, map(&[("armor_block", 2)]));
    assert_eq!(report.broken, map(&[("armor_block", 1), ("pulse_laser", 1), ("warp_core", 2)]));
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
    assert_eq!(career.data()["kinetic"], 12.0);
    assert_eq!(career.researched(), ["automaton_cannon"]);
    assert!(career.is_researched("automaton_cannon"));
    // The node unlocks the player's copy of the gun; the automatons' own stays theirs.
    let u = career.unlocks(&c);
    assert!(u.has_part("reverse_engineered_cannon") && !u.has_part("automaton_cannon"));

    // Paying a store down to nothing removes it; materials are paid the same way.
    bring_home(
        &mut career,
        &c,
        &[
            (part("logic_core"), 1),
            (fragment("automaton_core"), 1),
            (part("automaton_cannon"), 1),
        ],
    );
    assert_eq!(career.data()["logic"], 3.0);
    // The fragment's 1 and a quarter of the broken core's 2, rounded up.
    assert_eq!(career.materials()["programmable_matter"], 2);
    career.research(&c, "tamed_core").unwrap();
    assert!(!career.data().contains_key("logic"));
    assert_eq!(career.data()["kinetic"], 15.0);
    assert_eq!(career.materials()["programmable_matter"], 1);
    assert_eq!(career.researched(), ["automaton_cannon", "tamed_core"]);
    assert!(career.unlocks(&c).has_part("automaton_core"));
}

#[test]
fn the_simulation_vault_opens_the_evolution_lab() {
    let c = content();
    let mut career = new_game(&c);
    assert!(!career.unlocks(&c).has_facility("evolution"), "dark in a new game");
    assert!(Unlocks::all(&c).has_facility("evolution"), "open in the dev mode");
    // Three logic data: a salvaged logic core and an automaton core's fragment.
    bring_home(&mut career, &c, &[(part("logic_core"), 1), (fragment("automaton_core"), 1)]);
    career.research(&c, "simulation_vault").unwrap();
    assert!(career.unlocks(&c).has_facility("evolution"));
    assert_eq!(career.mind_capacity(&c), new_game(&c).mind_capacity(&c), "it opens a building, nothing else");
}

#[test]
fn researched_parts_are_fresh_until_seen() {
    let c = content();
    let mut career = new_game(&c);
    assert!(career.fresh_parts(&c).is_empty());
    // A part a new game has is never fresh.
    assert!(!career.mark_seen(&c, "pulse_laser"));
    bring_home(
        &mut career,
        &c,
        &[(part("automaton_cannon"), 5), (part("automaton_core"), 2)],
    );
    career.research(&c, "automaton_cannon").unwrap();
    career.research(&c, "tamed_core").unwrap();
    let both = BTreeSet::from(["reverse_engineered_cannon".to_string(), "automaton_core".to_string()]);
    assert_eq!(career.fresh_parts(&c), both);
    // Seeing it once is enough; locked parts not researched yet are not fresh.
    assert!(career.mark_seen(&c, "reverse_engineered_cannon"));
    assert!(!career.mark_seen(&c, "reverse_engineered_cannon"));
    assert!(!career.mark_seen(&c, "beam_laser"));
    assert_eq!(
        career.fresh_parts(&c),
        BTreeSet::from(["automaton_core".to_string()])
    );
    // The save keeps what was seen.
    let text = career.to_ron();
    assert!(
        text.contains("seen: [\n        \"reverse_engineered_cannon\",\n    ],"),
        "{text}"
    );
    assert_eq!(Career::from_ron(&text).unwrap(), career);
    // A seen part the content no longer has goes without a note.
    let text = text.replace(
        "\"reverse_engineered_cannon\",\n    ],",
        "\"reverse_engineered_cannon\",\n        \"gone_gun\",\n    ],",
    );
    let mut back = Career::from_ron(&text).unwrap();
    assert!(back.reconcile(&c).is_empty());
    assert_eq!(back, career);
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
                cost: 3.0
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
        &[(part("thruster"), 1), (fragment("automaton_core"), 5)],
    );
    assert_eq!(
        career.can_pay(&c, "tamed_core"),
        Err(Refusal::CannotPay(vec![Shortfall::Data {
            family: "logic".into(),
            have: 0.0,
            cost: 3.0
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
            salvage_part("near", "kinetic", 3.0 - DATA_EPSILON / 2.0),
            salvage_part("short", "kinetic", 2.99)
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

/// Clearing a voyage's goal on a voyage that comes home opens the voyages after it (in content order); losing it,
/// or coming home without the goal, opens nothing.
#[test]
fn clearing_a_voyage_opens_the_next() {
    let c = content();
    let mut career = new_game(&c);
    let end = |career: &mut Career, id: &str, end: VoyageEnd| {
        career.voyage_launched(&c, id).unwrap();
        career.voyage_ended(&c, id, end).unwrap();
    };
    let home = |cleared| VoyageEnd::Recalled {
        haul: Haul::default(),
        cleared,
        ticks: 9000,
    };
    end(&mut career, FIRST, VoyageEnd::Lost { ticks: 10 });
    end(&mut career, FIRST, home(false));
    assert_eq!(career.open_voyages(&c), [FIRST]);
    end(&mut career, FIRST, home(true));
    assert_eq!(career.open_voyages(&c), [FIRST, "halvex_drift"]);
    assert!(career.is_open(&c, "halvex_drift") && !career.is_open(&c, "corran_reach"));
    end(&mut career, "halvex_drift", home(true));
    end(&mut career, "corran_reach", home(true));
    assert_eq!(
        career.open_voyages(&c),
        [FIRST, "halvex_drift", "corran_reach", "wardens_gate"]
    );

    // A voyage after two opens when both are cleared; one after none only through a start.
    let modded = Content::load(&[
        data::base_pack().unwrap(),
        Pack::from_files([
            (
                "pack.ron",
                r#"(id: "test_mod", name: "T", version: "1", schema_version: 1, depends: ["base"])"#,
            ),
            (
                "voyages/more.ron",
                r#"[(id: "both", name: "Both", summary: "S.", band: "rim",
                     opens_after: ["yard_approach", "halvex_drift"], radius: 800, spacing: (300, 600), minutes: (5, 9),
                     sites: [(encounter: "raider_den", goal: true)]),
                    (id: "elsewhere", name: "Elsewhere", summary: "S.", band: "rim",
                     radius: 800, spacing: (300, 600), minutes: (5, 9),
                     sites: [(encounter: "raider_den", goal: true)])]"#,
            ),
        ])
        .unwrap(),
    ])
    .unwrap();
    let mut career = new_game(&modded);
    let end = |career: &mut Career, id: &str| {
        career.voyage_launched(&modded, id).unwrap();
        career.voyage_ended(&modded, id, home(true)).unwrap();
    };
    end(&mut career, FIRST);
    assert!(!career.is_open(&modded, "both"));
    end(&mut career, "halvex_drift");
    assert!(career.is_open(&modded, "both"));
    assert!(!career.is_open(&modded, "elsewhere"));
}

/// A voyage's `gives` (the Warden's gate shard) comes with its first clear only, into store and the result's report.
#[test]
fn a_first_clear_gives_the_voyage_gift() {
    let c = content();
    let mut career = new_game(&c);
    let shard = |career: &Career| career.materials().get("gate_shard").copied().unwrap_or(0);
    let end = |career: &mut Career, cleared| {
        career.voyage_launched(&c, "wardens_gate").unwrap();
        let home = VoyageEnd::Recalled { haul: Haul::default(), cleared, ticks: 9000 };
        career.voyage_ended(&c, "wardens_gate", home).unwrap().report.materials.get("gate_shard").copied()
    };
    for id in [FIRST, "halvex_drift", "corran_reach"] {
        career.voyage_launched(&c, id).unwrap();
        career.voyage_ended(&c, id, VoyageEnd::Recalled { haul: Haul::default(), cleared: true, ticks: 9000 }).unwrap();
    }
    assert_eq!((end(&mut career, false), shard(&career)), (None, 0));
    assert_eq!((end(&mut career, true), shard(&career)), (Some(1), 1));
    assert_eq!((end(&mut career, true), shard(&career)), (None, 1));
}

#[test]
fn voyage_seeds_are_fixed() {
    // Values computed independently (FNV-1a 64 and SplitMix64 by their reference definitions); they are part of
    // the save format (the same in GENERATOR 1 and 2) and must never change.
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
    assert_eq!(voyage_ready(&c, &new_game_unlocks(&c), 2, None, &skiff, &[]), []);
}

#[test]
fn locked_parts_and_materials_are_named_once_per_ship() {
    let c = content();
    let u = new_game_unlocks(&c);
    let runner = ship(&c, "halvex_runner");
    let flag = FleetSlot::Flagship;
    let found = voyage_ready(&c, &u, 2, None, &runner, &[]);
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
    assert_eq!(voyage_ready(&c, &Unlocks::all(&c), 2, None, &runner, &[]), []);
    // A part the content doesn't have counts as locked (and a ship that cannot be built has no hold).
    let mut odd = ship(&c, "rim_skiff");
    odd.parts[2].part = "warp_core".into();
    assert_eq!(
        voyage_ready(&c, &u, 2, None, &odd, &[]),
        [
            Readiness::LockedPart {
                ship: flag,
                part: "warp_core".into()
            },
            Readiness::NoCargo
        ]
    );
}

#[test]
fn escorts_count_against_the_mind() {
    let c = content();
    let u = new_game_unlocks(&c);
    let skiff = ship(&c, "rim_skiff");
    assert_eq!(
        voyage_ready(&c, &u, 2, None, &skiff, std::slice::from_ref(&skiff)),
        [Readiness::TooManyCores {
            cores: 4,
            capacity: 2
        }]
    );
    assert_eq!(
        voyage_ready(&c, &u, 4, None, &skiff, std::slice::from_ref(&skiff)),
        []
    );
    // An escort's locked parts name the escort.
    let raider = ship(&c, "automaton_raider");
    let found = voyage_ready(&c, &u, 4, None, &skiff, &[skiff.clone(), raider]);
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
    let found = voyage_ready(&c, &all, 2, None, &raider, &[]);
    assert_eq!(found, [Readiness::NoCargo]);
    assert!(!found[0].blocks());
    // One ship with a hold is enough.
    assert_eq!(
        voyage_ready(&c, &all, 3, None, &raider, &[ship(&c, "rim_skiff")]),
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
        voyage_ready(&c, &u, 2, None, &big, &[]),
        [Readiness::LockedFrame {
            ship: flag,
            size: (30, 30),
            frame: Some("cruiser".into())
        }]
    );
    let mut huge = big.clone();
    huge.size = (100, 24);
    assert_eq!(
        voyage_ready(&c, &u, 2, None, &huge, &[]),
        [Readiness::LockedFrame {
            ship: flag,
            size: (100, 24),
            frame: None
        }]
    );
    u.frames.insert("cruiser".into());
    assert_eq!(voyage_ready(&c, &u, 2, None, &big, &[]), []);
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
    assert!(
        text.starts_with("(\n    packs: [\n") && text.ends_with("\n)\n"),
        "{text}"
    );
    assert_eq!(back.escorts(), ["drone_wing", "drone_wing"]);
    assert_eq!(back.underway(), Some(FIRST));
}

/// The history keeps the last [`MAX_HISTORY`] voyages, newest last, each new until the LOG opens; it round trips
/// through the save (a read entry writes no `new`), and a longer one written by hand loses its oldest.
#[test]
fn the_history_keeps_the_last_voyages() {
    let c = content();
    let mut career = new_game(&c);
    let n = MAX_HISTORY as u32;
    for k in 1..=n + 5 {
        bring_home(&mut career, &c, &[(part("thruster"), k)]);
    }
    let h = career.history();
    assert_eq!(h.len(), MAX_HISTORY);
    assert_eq!(h[0].report.broken["thruster"], 6, "the five oldest went");
    let last = career.last_voyage().unwrap();
    assert_eq!(last.report.broken["thruster"], n + 5);
    assert_eq!(career.log().recalled, n + 5, "the counts keep every voyage");
    assert_eq!(career.unread(), MAX_HISTORY);
    assert_eq!(Career::from_ron(&career.to_ron()).unwrap(), career);

    assert!(career.read_log());
    assert!(!career.read_log(), "nothing new the second time");
    assert_eq!(career.unread(), 0);
    let text = career.to_ron();
    assert!(
        text.contains("history: [") && !text.contains("new:"),
        "{text}"
    );
    bring_home(&mut career, &c, &[]);
    assert_eq!(career.unread(), 1);
    assert_eq!(Career::from_ron(&career.to_ron()).unwrap(), career);

    // Two more than it keeps, written by hand: the oldest two go, with a note.
    let entries = format!(
        "{}(voyage: \"{FIRST}\", outcome: Lost)",
        format!("(voyage: \"{FIRST}\", outcome: Recalled), ").repeat(MAX_HISTORY + 1)
    );
    let text = format!(
        "(start: \"{START}\", seed: 1, generator: {GENERATOR}, history: [{entries}])"
    );
    let mut long = Career::from_ron(&text).unwrap();
    let notes = long.reconcile(&c);
    assert!(
        note(&notes, "history").message.contains("oldest 2"),
        "{}",
        show(&notes)
    );
    assert_eq!(long.history().len(), MAX_HISTORY);
    assert_eq!(long.last_voyage().unwrap().outcome, Outcome::Lost);
}

#[test]
fn a_hand_written_save_reads() {
    // The shape of docs/career.md: comments, `Some` left out, fields with defaults left out.
    let text = r#"
// space-game: saves, format 5, saved 1760351234
(
    packs: [("base", "0.1.0")],
    start: "rim_awakening",
    seed: 4207,
    generator: 2,
    researched: ["automaton_cannon", "composite_plating"],
    data: {"kinetic": 4, "hull": 11.5, "logic": 2},
    materials: {"programmable_matter": 1},
    escorts: ["drone_wing"],
    voyages: {
        "yard_approach": (launched: 2, cleared: true, best_ticks: 13500),
    },
    log: (voyages: 3, lost: 1, salvaged: 34),
    history: [(voyage: "yard_approach", outcome: Recalled, ticks: 8000,
               report: (parts: {"automaton_cannon": 3, "thruster": 2}, data: {"kinetic": 9, "propulsion": 6}))],
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
    let last = career.last_voyage().unwrap();
    assert_eq!(last.report.data["propulsion"], 6.0);
    assert!(last.report.materials.is_empty());
    assert_eq!((career.history().len(), career.unread()), (1, 0));
    assert!(career.to_ron().contains("history: ["));
    let u = career.unlocks(&c);
    assert!(u.has_part("reverse_engineered_cannon") && u.has_material("composite"));
}

/// ADR-0024, ADR-0044: a save of another voyage roller version is refused, never migrated or reset.
#[test]
fn other_rollers_are_refused() {
    for generator in [1, GENERATOR + 1] {
        let text = format!(r#"(start: "rim_awakening", seed: 4207, generator: {generator})"#);
        let p = Career::from_ron(&text).unwrap_err();
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].severity, p[0].field.as_deref()), (Severity::Error, Some("generator")));
        assert!(p[0].message.contains(&format!("version {generator} of the roller")), "{}", show(&p));
    }
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
    generator: 2,
    researched: ["automaton_cannon", "warp_theory", "automaton_cannon"],
    data: {"kinetic": 4, "lasers": 2, "hull": NaN, "power": -3, "energy": 1e12, "thermal": 0, "shields": inf},
    materials: {"programmable_matter": 5000000, "unobtainium": 2, "steel": 0},
    voyages: {"yard_approach": (launched: 2), "nowhere": (launched: 1, cleared: true)},
    history: [(voyage: "nowhere", outcome: Cleared, report: (data: {"kinetic": 3}))],
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
        "history[0]",
    ] {
        note(&notes, field);
    }
    assert_eq!(note(&notes, "data.lasers").kind, Some(Kind::Family));
    assert_eq!(
        note(&notes, "materials.unobtainium").kind,
        Some(Kind::Material)
    );
    assert_eq!(notes.len(), 13, "{}", show(&notes));

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
    assert!(career.history().is_empty());
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
    let last = loaded.last_voyage().unwrap();
    assert_eq!((last.outcome, last.ticks, last.new), (Outcome::Lost, 0, true));
    assert_eq!(loaded.history().len(), 2);
    assert_eq!(loaded.data()["propulsion"], 3.0);
    // The next launch rolls a new voyage.
    assert_eq!(loaded.voyage(FIRST).unwrap().launched, 2);

    // Underway without a launch on record (a hand-edited save): just dropped.
    let mut odd = Career::from_ron(
        r#"(start: "rim_awakening", seed: 1, generator: 2, underway: "yard_approach")"#,
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
            "families/drones.ron",
            r#"(id: "drones", name: "Drones", summary: "Small craft.")"#,
        ),
        (
            "parts/rack.ron",
            &salvage_part("seeker_rack", "drones", 5.0),
        ),
        (
            "research/seeker.ron",
            r#"(id: "seeker", name: "Seeker", summary: "S.", family: "drones", kind: Refinement,
                cost: (data: {"drones": 5}), unlocks: [MindCapacity(1)])"#,
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
    assert_eq!(career.data()["drones"], 10.0);

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
    note(&notes, "data.drones");
    note(&notes, "history[0].report.data.drones");
    note(&notes, "history[1].report.data.drones");
    assert_eq!(note(&notes, "start").id.as_deref(), Some("alt"));
    assert_eq!(notes.len(), 6, "{}", show(&notes));
    assert_eq!(loaded.mind_capacity(&c), 2);
    assert!(loaded.researched().is_empty() && loaded.data().is_empty());
    // The debrief keeps what came home.
    assert_eq!(
        loaded.last_voyage().unwrap().report.broken,
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
        voyage_ready(c, &career.unlocks(c), career.mind_capacity(c), None, &skiff, &[]),
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
    // The start's materials, a quarter of the broken cannons' recipes and of the plating per cell (automaton plate
    // 4 + 2, copper 1, steel 2), the fragments' programmable matter.
    assert_eq!(
        career.materials(),
        &map(&[("automaton_plate", 6), ("ceramic", 2), ("copper", 5), ("crystal", 2), ("programmable_matter", 2), ("steel", 6)])
    );
    let last = career.last_voyage().unwrap();
    assert_eq!(
        (last.voyage.as_str(), last.outcome),
        (FIRST, Outcome::Recalled)
    );

    // The cannon.
    assert!(!career.unlocks(c).has_part("reverse_engineered_cannon"));
    career.research(c, "automaton_cannon").unwrap();
    assert!(career.unlocks(c).has_part("reverse_engineered_cannon"));
    assert_eq!(
        career.can_pay(c, "mind_expansion_1"),
        Err(Refusal::Requires(vec!["tamed_core".into()]))
    );

    // A core graveyard, goal cleared: five intact cores wait in the workshop.
    career.voyage_launched(c, FIRST).unwrap();
    let end = VoyageEnd::Recalled {
        haul: haul(&[(intact("automaton_core"), 5)]),
        cleared: true,
        ticks: 12000,
    };
    assert_eq!(
        career.voyage_ended(c, FIRST, end).unwrap().outcome,
        Outcome::Cleared
    );
    assert_eq!(career.intake(), &map(&[("automaton_core", 5)]));
    assert!(!career.data().contains_key("logic"));
    // The workshop suggests keeping two (none in stock), dismantling the rest; one is kept, four dismantled:
    // 4 logic data each and 0.4 of the recipe, rounded (programmable matter and automaton plate 4.8, copper and
    // crystal 1.6).
    let sort = career.suggested(c, &BTreeMap::new());
    assert_eq!(sort["automaton_core"], data::career::Sort { keep: 2, dismantle: 3 });
    career.keep("automaton_core", 1).unwrap();
    let got = career.dismantle(c, "automaton_core", 4).unwrap();
    assert_eq!(got.materials, map(&[("automaton_plate", 5), ("copper", 2), ("crystal", 2), ("programmable_matter", 5)]));
    assert_eq!(career.dismantle(c, "automaton_core", 1), Err(Refusal::NotInIntake("automaton_core".into())));
    assert!(career.intake().is_empty() && career.stock()["automaton_core"] == 1);
    assert_eq!(career.data()["logic"], 16.0);
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
    assert_eq!(
        career.data(),
        &map(&[("hull", 8.0), ("kinetic", 9.0), ("logic", 3.0)])
    );
    assert_eq!(
        career.materials(),
        &map(&[("automaton_plate", 11), ("ceramic", 2), ("copper", 7), ("crystal", 4), ("programmable_matter", 3), ("steel", 6)])
    );

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
        group: None,
    });
    let cap = career.mind_capacity(c);
    assert_eq!(voyage_ready(c, &unlocks, cap, None, &skiff, std::slice::from_ref(&drone)), []);
    assert_eq!(
        voyage_ready(c, &unlocks, cap, None, &skiff, std::slice::from_ref(&skiff)),
        [Readiness::TooManyCores {
            cores: 4,
            capacity: 3
        }]
    );
    // The stock holds one skiff and the kept core: the drone's other parts are missing, the core is not.
    let found = voyage_ready(c, &unlocks, cap, Some(career.stock()), &skiff, std::slice::from_ref(&drone));
    assert!(found.contains(&Readiness::MissingParts { part: "capacitor".into(), need: 4, have: 2 }), "{found:?}");
    assert!(!found.iter().any(|r| matches!(r, Readiness::MissingParts { part, .. } if part == "automaton_core" || part == "logic_core")));
    // The workshop fabricates a capacitor (ceramic 1, copper 3); two more take more than is left.
    career.fabricate(c, "capacitor", 1).unwrap();
    assert_eq!(career.stock()["capacitor"], 3);
    assert_eq!((career.materials()["ceramic"], career.materials()["copper"]), (1, 4));
    assert_eq!(
        career.fabricate(c, "capacitor", 2),
        Err(Refusal::CannotPay(vec![
            Shortfall::Material { material: "ceramic".into(), have: 1, cost: 2 },
            Shortfall::Material { material: "copper".into(), have: 4, cost: 6 },
        ]))
    );
    assert_eq!(career.can_fabricate(c, "beam_laser", 1), Err(Refusal::LockedPart("beam_laser".into())));
    assert_eq!(career.can_fabricate(c, "automaton_core", 1), Err(Refusal::NeedsFacility("automaton_forge".into())));
    assert_eq!(career.can_fabricate(c, "armor_block", 1), Err(Refusal::NoRecipe("armor_block".into())));

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

/// Research data that is not finite (a hand-edited or damaged save queried before `reconcile`) counts as none:
/// it never pays a node (13 Oct 2026 review: NaN paid one node, infinity every node forever).
#[test]
fn research_data_that_is_not_finite_pays_nothing() {
    let c = content();
    for store in ["NaN", "inf", "-inf"] {
        let text = format!(
            r#"(start: "rim_awakening", seed: 1, generator: 2, data: {{"kinetic": {store}}})"#
        );
        let mut career = Career::from_ron(&text).unwrap();
        match career.can_pay(&c, "automaton_cannon") {
            Err(Refusal::CannotPay(short)) => assert!(
                matches!(&short[..], [Shortfall::Data { have, .. }] if *have == 0.0),
                "{store}: {short:?}"
            ),
            other => panic!("{store}: {other:?}"),
        }
        assert!(career.research(&c, "automaton_cannon").is_err());
        assert!(career.researched().is_empty());
    }
}

/// The hold is the cargo of the main piece (the sim's controlling piece): a fleet whose only cargo bay sits on a
/// piece that falls off at spawn brings nothing home, so readiness warns.
#[test]
fn cargo_off_the_main_piece_is_no_cargo() {
    let c = content();
    let mut skiff = ship(&c, "rim_skiff");
    // The skiff's bay, moved two columns right of its hull: a piece of its own.
    let right = skiff.size.0 as i32;
    let mut moved = 0;
    for p in &mut skiff.parts {
        if p.part == "cargo_bay" {
            p.x = right + 1 + moved;
            p.y = 0;
            moved += 1;
        }
    }
    assert_eq!(moved, 1);
    skiff.size.0 += 4;
    assert_eq!(c.ship_hold(&skiff), 0);
    assert_eq!(c.ship_hold(&ship(&c, "rim_skiff")), 4);
    let all = Unlocks::all(&c);
    assert_eq!(voyage_ready(&c, &all, 2, None, &skiff, &[]), [Readiness::NoCargo]);
}

/// A save written by an editor that adds a byte order mark reads; one above the document size limit is refused
/// before it is parsed.
#[test]
fn saves_take_the_blueprints_input_limits() {
    let text = "\u{feff}(start: \"rim_awakening\", seed: 1, generator: 2)";
    assert!(Career::from_ron(text).is_ok());
    let huge = format!(
        "(start: \"rim_awakening\", seed: 1, generator: 2, escorts: [{}])",
        "\"x\",".repeat(data::content::limits::MAX_DOCUMENT_BYTES / 4 + 1)
    );
    let p = Career::from_ron(&huge).unwrap_err();
    assert!(p[0].message.contains("bytes"), "{}", show(&p));
}

// ---------------------------------------------------------------------------------------------------------------
// The workshop (GDR-0108)

/// Intact parts wait in the intake; the suggestion keeps up to the rules' two (or what the fleet needs) and
/// dismantles the rest; sorting as suggested empties the intake; unknown parts in stock or intake are dropped at load.
#[test]
fn the_workshop_sorts_the_intake() {
    let c = content();
    let mut career = new_game(&c);
    bring_home(&mut career, &c, &[(intact("thruster"), 5), (intact("gyro"), 1), (hull("armor_block", "steel"), 2)]);
    assert_eq!(career.intake(), &map(&[("gyro", 1), ("thruster", 5)]), "hull is never kept");
    // The skiff's stock: two thrusters, two gyros. Keep two: nothing more; the fleet needs four thrusters: two.
    let none = career.suggested(&c, &BTreeMap::new());
    assert_eq!((none["thruster"].keep, none["thruster"].dismantle, none["gyro"].keep), (0, 5, 0));
    let need = map(&[("thruster", 4)]);
    let sort = career.suggested(&c, &need);
    assert_eq!((sort["thruster"].keep, sort["thruster"].dismantle), (2, 3));
    let got = career.sort_suggested(&c, &need).unwrap();
    assert!(career.intake().is_empty());
    assert_eq!(career.stock()["thruster"], 4);
    // Three thrusters and a gyro at 0.4: steel 3.6 + 1.6, ceramic 2.4, copper 1.2 + 1.2, rounded per part kind.
    assert_eq!(got.materials, map(&[("ceramic", 2), ("copper", 2), ("steel", 6)]));
    assert_eq!(got.data["propulsion"], 3.0 * 3.0 + 4.0);
    assert_eq!(career.keep("thruster", 1), Err(Refusal::NotInIntake("thruster".into())));
    // What the report leaves unsorted is kept.
    bring_home(&mut career, &c, &[(intact("thruster"), 2), (intact("gyro"), 1)]);
    assert_eq!(career.keep_intake(), 3);
    assert!(career.intake().is_empty());
    assert_eq!((career.stock()["thruster"], career.stock()["gyro"]), (6, 3));
    assert_eq!(career.keep_intake(), 0);
    // Reading back keeps the stock; unknown or hull parts are dropped with a note.
    let text = career.to_ron().replace("stock: {", "stock: {\"warp_core\": 2, \"armor_block\": 1, ");
    let mut loaded = Career::from_ron(&text).unwrap();
    let notes = loaded.reconcile(&c);
    note(&notes, "stock.warp_core");
    note(&notes, "stock.armor_block");
    assert_eq!(loaded.stock(), career.stock());
}

/// Stock parts dismantle for the workshop's share of their recipe and no data, never below what the fleet carries;
/// the most the workshop can fabricate is what its scarcest material pays for.
#[test]
fn the_workshop_dismantles_stock_and_counts_what_it_can_make() {
    let c = content();
    let mut career = new_game(&c);
    let (data, before) = (career.data().clone(), career.materials().clone());
    assert_eq!(career.stock()["thruster"], 2);
    // The fleet carries two thrusters: none spare, dismantling one is refused and changes nothing.
    let need = map(&[("thruster", 2)]);
    assert_eq!(career.spare("thruster", &need), 0);
    assert_eq!(
        career.dismantle_stock(&c, "thruster", 1, &need),
        Err(Refusal::FleetNeeds { part: "thruster".into(), need: 2, stock: 2 })
    );
    assert_eq!(career.stock()["thruster"], 2);
    // The fleet carries one: one spare; two would leave it short.
    let need = map(&[("thruster", 1)]);
    assert_eq!(career.spare("thruster", &need), 1);
    assert!(matches!(career.dismantle_stock(&c, "thruster", 2, &need), Err(Refusal::FleetNeeds { .. })));
    // Without a fleet both go: a thruster's recipe (ceramic 2, copper 1, steel 3) times two at 0.4, rounded.
    let got = career.dismantle_stock(&c, "thruster", 2, &BTreeMap::new()).unwrap();
    assert_eq!(got.materials, map(&[("ceramic", 2), ("copper", 1), ("steel", 2)]));
    assert!(got.data.is_empty(), "a stock part gives no research data");
    assert_eq!(career.data(), &data);
    for (m, n) in &got.materials {
        assert_eq!(career.materials()[m], before.get(m).copied().unwrap_or(0) + n);
    }
    assert!(!career.stock().contains_key("thruster"));
    assert_eq!(career.dismantle_stock(&c, "thruster", 1, &BTreeMap::new()), Err(Refusal::NotInStock("thruster".into())));
    assert_eq!(career.dismantle_stock(&c, "gyro", 0, &BTreeMap::new()), Err(Refusal::NotInStock("gyro".into())));
    assert_eq!(career.dismantle_stock(&c, "armor_block", 1, &BTreeMap::new()), Err(Refusal::NoRecipe("armor_block".into())));
    // A capacitor costs ceramic 1 and copper 3: the most is what the scarcer pays for, and exactly that is made.
    let most = career.fabricable(&c, "capacitor");
    let have = |m: &str| career.materials().get(m).copied().unwrap_or(0);
    assert_eq!(most, have("ceramic").min(have("copper") / 3));
    assert!(most > 0);
    career.fabricate(&c, "capacitor", most).unwrap();
    assert_eq!(career.fabricable(&c, "capacitor"), 0);
    assert!(career.fabricate(&c, "capacitor", 1).is_err());
    assert_eq!(career.fabricable(&c, "beam_laser"), 0, "a locked part");
}

#[test]
fn ore_comes_home_and_the_smelter_refines_it() {
    let c = content();
    let mut career = new_game(&c);
    let ore = |m: &str| HaulItem::Ore { material: m.into() };
    // Ore is its material, a unit each (GDR-0114); the broken cargo bays give the smelter's utility data.
    let got = bring_home(&mut career, &c, &[(ore("nickel_iron"), 26), (ore("water_ice"), 5), (part("cargo_bay"), 2)]);
    assert_eq!((got.materials["nickel_iron"], got.materials["water_ice"]), (26, 5));
    let have = |career: &Career, m: &str| career.materials().get(m).copied().unwrap_or(0);
    // Without the smelter ore pays for nothing but itself.
    assert_eq!(career.available(&c, "steel"), have(&career, "steel"));
    assert_eq!(career.available(&c, "coolant"), 0);
    // The smelter's research takes nickel-iron; then two nickel-iron pay for one steel, two ice for one coolant.
    career.research(&c, "smelter").unwrap();
    assert_eq!(have(&career, "nickel_iron"), 20);
    assert!(career.unlocks(&c).has_facility("smelter"));
    let steel = have(&career, "steel");
    assert_eq!(career.available(&c, "steel"), steel + 10);
    assert_eq!(career.available(&c, "coolant"), 2);
    // A cargo hold takes 10 steel: the store's first, the rest refined (whole batches).
    assert_eq!(career.fabricable(&c, "cargo_hold"), (steel + 10) / 10);
    career.fabricate(&c, "cargo_hold", 1).unwrap();
    let refined = 10u32.saturating_sub(steel);
    assert_eq!(have(&career, "steel"), steel.saturating_sub(10));
    assert_eq!(have(&career, "nickel_iron"), 20 - refined * 2);
    // Short even with refining: the shortfall counts what refining would add.
    let left = career.available(&c, "steel");
    match career.can_fabricate(&c, "cargo_hold", 9) {
        Err(Refusal::CannotPay(s)) => {
            assert_eq!(s, [Shortfall::Material { material: "steel".into(), have: left, cost: 90 }]);
        }
        other => panic!("{other:?}"),
    }
}
