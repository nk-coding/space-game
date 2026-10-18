//! Voyages and encounters (GDR-0062, track 7E): loading, texts and links, validation (good and bad cases) and the
//! base pack's four voyages.

mod common;

use common::*;
use data::{Content, Kind, Pack, Problem, Severity};

const MOD_MANIFEST: &str =
    r#"(id: "test_mod", name: "Test mod", version: "1.0.0", schema_version: 1, depends: ["base"])"#;

fn pack(files: &[(&str, &str)]) -> Pack {
    let mut all = vec![("pack.ron", MOD_MANIFEST)];
    all.extend_from_slice(files);
    Pack::from_files(all).unwrap_or_else(|p| panic!("{}", show(&p)))
}

/// Base + a mod made of `files`: every problem `validate` finds (errors and warnings).
fn problems_with(files: &[(&str, &str)]) -> Vec<Problem> {
    data::validate(&Content::merge(&[data::base_pack().unwrap(), pack(files)]))
}

fn assert_problem(problems: &[Problem], s: Severity, id: &str, field: &str, needle: &str) {
    assert!(
        problems.iter().any(|p| p.severity == s
            && p.id.as_deref() == Some(id)
            && p.field.as_deref() == Some(field)
            && p.message.contains(needle)),
        "expected a {s:?} for `{id}` field `{field}` containing {needle:?}, got:\n{}",
        show(problems)
    );
}

fn assert_error(problems: &[Problem], id: &str, field: &str, needle: &str) {
    assert_problem(problems, Severity::Error, id, field, needle);
}

fn assert_clean(problems: &[Problem], id: &str) {
    let mine: Vec<&Problem> = problems
        .iter()
        .filter(|p| p.id.as_deref() == Some(id))
        .collect();
    assert!(mine.is_empty(), "`{id}`: {mine:?}");
}

/// A voyage `v` opened after `yard_approach`, with `fields` after the header.
fn voyage(id: &str, fields: &str) -> String {
    format!(r#"(id: "{id}", name: "V", summary: "S.", {fields})"#)
}

const GOOD_VOYAGE: &str = r#"band: "rim", opens_after: ["yard_approach"], radius: 800,
    spacing: (300, 600), minutes: (5, 10), sites: [(encounter: "raider_pair", count: 2),
    (encounter: "raider_den", goal: true)]"#;

fn encounter(id: &str, fields: &str) -> String {
    format!(r#"(id: "{id}", name: "E", summary: "S.", {fields})"#)
}

// ---------------------------------------------------------------------------------------------------------------
// Loading

#[test]
fn voyages_and_encounters_load_with_texts_links_and_wiki() {
    let m = pack(&[
        (
            "voyages/deep.ron",
            &voyage(
                "deep",
                r#"band: "outer", opens_after: ["wardens_gate"], radius: 1500,
                   spacing: (500, 900), minutes: (20, 30),
                   cleared: "Past [[encounter:ambush]], the reach opens.",
                   sites: [(encounter: "ambush", goal: true)]"#,
            ),
        ),
        (
            "encounters/extra.ron",
            r#"[(id: "ambush", name: "Ambush", summary: "Hunters waiting in [[voyage:deep]].",
                 ships: [(ship: "automaton_hunter", count: (3, 4))], wrecks: [(ship: "halvex_runner", count: (0, 1))],
                 wake_radius: 400)]"#,
        ),
        ("wiki/voyage/deep.md", "# Deep\n\n[[encounter:ambush]]"),
        ("wiki/encounter/ambush.md", "Waits."),
    ]);
    assert_eq!((m.voyages.len(), m.encounters.len()), (1, 1));
    let c =
        Content::load(&[data::base_pack().unwrap(), m]).unwrap_or_else(|p| panic!("{}", show(&p)));
    let deep = c.voyages.get("deep").unwrap();
    assert_eq!(deep.source.file, "voyages/deep.ron");
    assert_eq!(deep.opens_after, ["wardens_gate"]);
    assert_eq!(
        (deep.radius, deep.spacing, deep.minutes),
        (1500.0, (500.0, 900.0), (20, 30))
    );
    assert_eq!(deep.sites[0].count, 1, "count defaults to 1");
    assert_eq!(deep.site_count(), 1);
    assert_eq!(deep.goal().unwrap().encounter, "ambush");
    let ambush = c.encounters.get("ambush").unwrap();
    assert_eq!(ambush.wrecks[0].damage, 0.0, "damage defaults to 0");
    assert!(ambush.always_has_ships());
    // Kinds like any other: keys, dirs, names, links, text keys (the optional `cleared` too), wiki prose.
    for (kind, key, dir) in [
        (Kind::Voyage, "voyage", "voyages"),
        (Kind::Encounter, "encounter", "encounters"),
    ] {
        assert_eq!(Kind::from_key(key), Some(kind));
        assert_eq!(Kind::from_dir(dir), Some(kind));
    }
    assert!(c.exists(Kind::Voyage, "deep") && !c.exists(Kind::Encounter, "deep"));
    assert_eq!(
        c.render_plain("[[voyage:halvex_drift]] and [[encounter:core_graveyard|the graves]]"),
        "Halvex Drift and the graves"
    );
    let texts = c.texts();
    assert_eq!(
        texts["voyage.deep.cleared"],
        "Past [[encounter:ambush]], the reach opens."
    );
    assert_eq!(
        texts["encounter.ambush.summary"],
        "Hunters waiting in [[voyage:deep]]."
    );
    assert!(!texts.contains_key("voyage.yard_approach.lore"));
    assert!(c.wiki_page(Kind::Voyage, "deep").is_some());
    // A link to nothing is an error in the new kinds' texts too.
    let p = problems_with(&[(
        "voyages/v.ron",
        &voyage(
            "v",
            &format!(r#"cleared: "See [[voyage:nowhere]].", {GOOD_VOYAGE}"#),
        ),
    )]);
    assert_error(&p, "v", "cleared", "points at nothing");
}

#[test]
fn unknown_fields_and_duplicates_fail_loudly() {
    let e = encounter(
        "e",
        r#"ships: [(ship: "automaton_raider", count: (1, 1))], wake_radius: 200"#,
    );
    let err = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        ("encounters/a.ron", e.as_str()),
        ("encounters/b.ron", e.as_str()),
    ])
    .unwrap_err();
    assert!(err[0].message.contains("duplicate id"), "{}", err[0]);
    let err = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        (
            "voyages/x.ron",
            &voyage("x", &format!("{GOOD_VOYAGE}, seed: 4")),
        ),
    ])
    .unwrap_err();
    assert!(err[0].message.contains("seed"), "{}", err[0]);
    assert_eq!(err[0].kind, Some(Kind::Voyage));
}

// ---------------------------------------------------------------------------------------------------------------
// Encounters

#[test]
fn encounter_rules() {
    let good = encounter(
        "good",
        r#"ships: [(ship: "automaton_raider", count: (0, 2))], wrecks: [(ship: "sample_mid", count: (1, 1), damage: 0.4)],
           wake_radius: 200"#,
    );
    let p = problems_with(&[("encounters/good.ron", &good)]);
    assert_clean(&p, "good");

    let bad = encounter(
        "bad",
        r#"ships: [(ship: "nope", count: (1, 1)), (ship: "automaton_raider", count: (3, 2)),
                   (ship: "automaton_raider", count: (1, 40))],
           wrecks: [(ship: "gone", count: (1, 1)), (ship: "sample_early", count: (1, 1), damage: 1.0),
                    (ship: "sample_early", count: (1, 1), damage: -0.1)],
           wake_radius: 0"#,
    );
    let p = problems_with(&[("encounters/bad.ron", &bad)]);
    assert_error(&p, "bad", "wake_radius", "a distance above 0");
    assert_error(&p, "bad", "ships[0].ship", "unknown ship `nope`");
    assert_error(&p, "bad", "ships[1].count", "min 3 is above max 2");
    assert_error(&p, "bad", "ships[2].count", "at most 32, got 40");
    assert_error(&p, "bad", "wrecks[0].ship", "unknown ship `gone`");
    assert_error(&p, "bad", "wrecks[1].damage", "0 to below 1, got 1");
    assert_error(&p, "bad", "wrecks[2].damage", "0 to below 1, got -0.1");

    // Nothing at all; a ship without a logic core would spawn as salvage.
    let hulk = r#"(id: "hulk", name: "Hulk", summary: "S.", faction: "ship_mind", size: (1, 1), parts: [
        (part: "armor_block", x: 0, y: 0, material: "steel")])"#;
    let p = problems_with(&[
        ("ships/hulk.ron", hulk),
        (
            "encounters/e.ron",
            &format!(
                "[{}, {}]",
                encounter("empty", "wake_radius: 100"),
                encounter(
                    "hulks",
                    r#"ships: [(ship: "hulk", count: (1, 1))], wake_radius: 100"#
                )
            ),
        ),
    ]);
    assert_error(&p, "empty", "ships", "needs ships or wrecks");
    assert_error(&p, "hulks", "ships[0].ship", "list it under `wrecks`");

    // A ship of a faction without a behavior (and none of its own) would only hold (a warning).
    let p = problems_with(&[
        (
            "factions/drifters.ron",
            r#"(id: "drifters", name: "Drifters", summary: "S.", glow: "enemy")"#,
        ),
        (
            "ships/drifter.ron",
            r#"(id: "drifter", name: "Drifter", summary: "S.", faction: "drifters", size: (1, 1), parts: [
                (part: "automaton_core", x: 0, y: 0)])"#,
        ),
        (
            "encounters/e.ron",
            &encounter(
                "idle",
                r#"ships: [(ship: "drifter", count: (1, 1))], wake_radius: 100"#,
            ),
        ),
    ]);
    assert_problem(
        &p,
        Severity::Warning,
        "idle",
        "ships[0].ship",
        "never fight",
    );
    assert!(
        !p.iter()
            .any(|p| p.is_error() && p.id.as_deref() == Some("idle"))
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Voyages

#[test]
fn a_valid_voyage_is_clean() {
    let p = problems_with(&[("voyages/v.ron", &voyage("v", GOOD_VOYAGE))]);
    assert_clean(&p, "v");
}

#[test]
fn voyage_numbers_and_references() {
    let p = problems_with(&[(
        "voyages/v.ron",
        &voyage(
            "v",
            r#"band: "Rim", opens_after: ["nowhere", "yard_approach", "yard_approach"],
               radius: -5, spacing: (700, 400), minutes: (0, 900),
               sites: [(encounter: "who", count: 0), (encounter: "raider_pack", count: 70)]"#,
        ),
    )]);
    assert_error(&p, "v", "band", "snake_case");
    assert_error(&p, "v", "opens_after[0]", "unknown voyage `nowhere`");
    assert_error(&p, "v", "opens_after[2]", "listed twice");
    assert_error(&p, "v", "radius", "a distance above 0");
    assert_error(&p, "v", "spacing", "min 700 is above max 400");
    assert_error(&p, "v", "minutes", "at least 1 minute");
    assert_error(&p, "v", "minutes", "at most 600, got 900");
    assert_error(&p, "v", "sites[0].count", "at least 1");
    assert_error(&p, "v", "sites[0].encounter", "unknown encounter `who`");
    assert_error(&p, "v", "sites", "no site is the goal");
    assert_error(&p, "v", "sites", "at most 64 sites in all, got 70");

    // Sites that can never be that far apart inside the radius.
    let p = problems_with(&[(
        "voyages/v.ron",
        &voyage("v", &GOOD_VOYAGE.replace("radius: 800", "radius: 100")),
    )]);
    assert_error(&p, "v", "spacing", "never fit into a radius of 100 m");
    // No sites at all.
    let p = problems_with(&[(
        "voyages/v.ron",
        &voyage(
            "v",
            r#"band: "rim", opens_after: ["yard_approach"], radius: 800, spacing: (300, 600),
               minutes: (5, 10), sites: []"#,
        ),
    )]);
    assert_error(&p, "v", "sites", "at least one site");
}

#[test]
fn exactly_one_goal_that_can_be_cleared() {
    let sites = |s: &str| {
        voyage(
            "v",
            &format!(
                r#"band: "rim", opens_after: ["yard_approach"], radius: 800, spacing: (300, 600),
                   minutes: (5, 10), sites: [{s}]"#
            ),
        )
    };
    let p = problems_with(&[(
        "voyages/v.ron",
        &sites(r#"(encounter: "raider_pair", goal: true), (encounter: "raider_den", goal: true)"#),
    )]);
    assert_error(&p, "v", "sites[1].goal", "only one site can be the goal");
    let p = problems_with(&[(
        "voyages/v.ron",
        &sites(r#"(encounter: "raider_den", count: 2, goal: true)"#),
    )]);
    assert_error(
        &p,
        "v",
        "sites[0].count",
        "the goal is one site, got a count of 2",
    );
    // The derelict field may roll no raider: a goal there might never be cleared.
    let p = problems_with(&[(
        "voyages/v.ron",
        &sites(r#"(encounter: "derelict_field", goal: true)"#),
    )]);
    assert_error(&p, "v", "sites[0].encounter", "may roll no ships");
    // GDR-0068: a site clears once its enemies are destroyed or disarmed, so the goal must always roll an armed
    // ship: power and a weapon (or a ram prow and an engine, GDR-0109) on its main piece. A gun without a reactor
    // is not armed, nor is a ship without guns or a prow.
    let ship = |id: &str, parts: &str| {
        format!(
            r#"(id: "{id}", name: "S", summary: "S.", faction: "automatons", size: (3, 3), parts: [{parts}])"#
        )
    };
    let files = [
        (
            "ships/drone.ron",
            ship(
                "drone",
                r#"(part: "logic_core", x: 1, y: 1), (part: "fission_reactor", x: 1, y: 2),
                   (part: "thruster", x: 0, y: 2)"#,
            ),
        ),
        (
            "ships/dry_gun.ron",
            ship(
                "dry_gun",
                r#"(part: "pulse_laser", x: 1, y: 0), (part: "logic_core", x: 1, y: 1),
                   (part: "thruster", x: 1, y: 2)"#,
            ),
        ),
        (
            "encounters/e.ron",
            format!(
                "[{}, {}]",
                encounter(
                    "unarmed",
                    r#"ships: [(ship: "drone", count: (1, 2)), (ship: "dry_gun", count: (1, 1)),
                       (ship: "automaton_raider", count: (0, 2))], wake_radius: 200"#
                ),
                encounter(
                    "armed",
                    r#"ships: [(ship: "drone", count: (1, 2)), (ship: "automaton_raider", count: (1, 1))],
                       wake_radius: 200"#
                )
            ),
        ),
        (
            "voyages/v.ron",
            sites(r#"(encounter: "unarmed", goal: true)"#),
        ),
        (
            "voyages/w.ron",
            voyage(
                "w",
                r#"band: "rim", opens_after: ["yard_approach"], radius: 800, spacing: (300, 600),
                   minutes: (5, 10), sites: [(encounter: "unarmed"), (encounter: "armed", goal: true)]"#,
            ),
        ),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(f, s)| (*f, s.as_str())).collect();
    let p = problems_with(&files);
    assert_error(&p, "v", "sites[0].encounter", "may roll no armed ship");
    assert_clean(&p, "w");
    let c = data::base_content();
    assert!(c.ship_armed(c.ships.get("automaton_warden").unwrap()));
    assert!(c.ship_armed(c.ships.get("rim_skiff").unwrap()));
    // GDR-0109: a ram prow and an engine arm a ship without guns.
    assert!(c.ship_armed(c.ships.get("grown_seedling").unwrap()));
}

#[test]
fn opens_after_must_not_loop_and_every_voyage_must_open_somehow() {
    let file = format!(
        "[{}, {}, {}]",
        voyage(
            "a",
            &GOOD_VOYAGE.replace(r#"["yard_approach"]"#, r#"["b"]"#)
        ),
        voyage(
            "b",
            &GOOD_VOYAGE.replace(r#"["yard_approach"]"#, r#"["a"]"#)
        ),
        voyage(
            "me",
            &GOOD_VOYAGE.replace(r#"["yard_approach"]"#, r#"["me", "yard_approach"]"#)
        ),
    );
    let p = problems_with(&[("voyages/loop.ron", &file)]);
    let cycles: Vec<&Problem> = p.iter().filter(|p| p.message.contains("cycle")).collect();
    assert_eq!(cycles.len(), 2, "{}", show(&p));
    assert_error(
        &p,
        "b",
        "opens_after[0]",
        "a → b → a, each opens after the next",
    );
    assert_error(&p, "me", "opens_after[0]", "can ever open");

    // Opened by no start and after nothing: unreachable (a warning).
    let lonely = GOOD_VOYAGE.replace(r#"opens_after: ["yard_approach"], "#, "");
    let p = problems_with(&[("voyages/v.ron", &voyage("lonely", &lonely))]);
    assert_problem(
        &p,
        Severity::Warning,
        "lonely",
        "opens_after",
        "can never be launched",
    );
    // A start that lists it opens it.
    let start = r#"(id: "s", name: "S", summary: "S.", ship: "rim_skiff", mind_capacity: 2, voyages: ["lonely"])"#;
    let p = problems_with(&[
        ("voyages/v.ron", &voyage("lonely", &lonely)),
        ("starts/s.ron", start),
    ]);
    assert_clean(&p, "lonely");
    assert_clean(&p, "s");
    // Opened only after an unreachable voyage: it can never open either (a warning on it too).
    let after = GOOD_VOYAGE.replace(r#"["yard_approach"]"#, r#"["lonely", "yard_approach"]"#);
    let p = problems_with(&[(
        "voyages/v.ron",
        &format!(
            "[{}, {}]",
            voyage("lonely", &lonely),
            voyage("after", &after)
        ),
    )]);
    assert_problem(
        &p,
        Severity::Warning,
        "after",
        "opens_after",
        "it opens after `lonely`, which can never open",
    );
    assert!(
        !p.iter()
            .any(|x| x.id.as_deref() == Some("after") && x.message.contains("yard_approach")),
        "{}",
        show(&p)
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The base pack

#[test]
fn base_voyages_form_the_slice_chain() {
    let c = content();
    let ids: Vec<&str> = c.voyages.iter().map(|v| v.id.as_str()).collect();
    // One file in chain order: the voyage map's order.
    assert_eq!(
        ids,
        [
            "yard_approach",
            "halvex_drift",
            "corran_reach",
            "wardens_gate"
        ]
    );
    // The start opens the first; each other opens after the one before it (GDR-0062).
    assert_eq!(
        c.starts.get("rim_awakening").unwrap().voyages,
        ["yard_approach"]
    );
    let mut chain = vec!["yard_approach"];
    while let Some(next) = c
        .voyages
        .iter()
        .find(|v| v.opens_after.iter().any(|o| o == chain.last().unwrap()))
    {
        chain.push(&next.id);
    }
    assert_eq!(
        chain,
        [
            "yard_approach",
            "halvex_drift",
            "corran_reach",
            "wardens_gate"
        ]
    );
    let goal = |id: &str| c.voyages.get(id).unwrap().goal().unwrap().encounter.clone();
    assert_eq!(
        [
            goal("yard_approach"),
            goal("halvex_drift"),
            goal("corran_reach"),
            goal("wardens_gate")
        ],
        ["raiding_party", "gunner_den", "hunter_den", "the_warden"]
    );
    let sites: Vec<u32> = chain
        .iter()
        .map(|id| c.voyages.get(id).unwrap().site_count())
        .collect();
    assert_eq!(sites, [4, 4, 5, 3]);
    // Every encounter is used by a voyage, a kind of cluster or a trail's end (galaxy plan E8; the raider pair and
    // den only in clusters since GDR-0101), and the Warden waits at the end.
    for e in c.encounters.iter() {
        assert!(
            c.voyages
                .iter()
                .any(|v| v.sites.iter().any(|s| s.encounter == e.id))
                || c.cluster_types.iter().any(|t| t.sites.iter().any(|s| s.encounter == e.id))
                || c.trails.iter().any(|t| t.target == e.id),
            "{} is used by no voyage, no kind of cluster and no trail",
            e.id
        );
    }
    let warden = c.encounters.get("the_warden").unwrap();
    assert!(warden.ships.iter().any(|s| s.ship == "automaton_warden"));
    assert!(c.ships.get("automaton_hunter").is_some());
}
