//! The content of the career (GDR-0062, GDR-0063): research families, research nodes, starts, salvage blocks on
//! parts and armour materials, and `locked` frames. Loading, validation (good and bad cases) and the base pack.

mod common;

use common::*;
use data::{Content, Kind, Pack, Problem, ResearchKind, Severity, Unlock};

fn base() -> Pack {
    data::base_pack().unwrap()
}

const MOD_MANIFEST: &str =
    r#"(id: "test_mod", name: "Test mod", version: "1.0.0", schema_version: 1, depends: ["base"])"#;

fn pack(files: &[(&str, &str)]) -> Pack {
    let mut all = vec![("pack.ron", MOD_MANIFEST)];
    all.extend_from_slice(files);
    Pack::from_files(all).unwrap_or_else(|p| panic!("{}", show(&p)))
}

/// Base + a mod made of `files`: every problem `validate` finds (errors and warnings).
fn problems_with(files: &[(&str, &str)]) -> Vec<Problem> {
    data::validate(&Content::merge(&[base(), pack(files)]))
}

/// Base + a mod: the problems of severity `s` about entry `id`.
fn of(problems: &[Problem], id: &str, s: Severity) -> Vec<Problem> {
    problems
        .iter()
        .filter(|p| p.severity == s && p.id.as_deref() == Some(id))
        .cloned()
        .collect()
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
    let mine: Vec<Problem> = problems
        .iter()
        .filter(|p| p.id.as_deref() == Some(id))
        .cloned()
        .collect();
    assert!(mine.is_empty(), "`{id}`:\n{}", show(&mine));
}

// ---------------------------------------------------------------------------------------------------------------
// Loading

#[test]
fn new_kinds_load_from_their_dirs_with_texts_links_and_wiki() {
    let m = pack(&[
        (
            "families/extra.ron",
            r#"(id: "drones", name: "Drones", summary: "Small craft; see [[research:seeker]].", color: "weapons")"#,
        ),
        (
            "research.ron",
            r#"[(id: "seeker", name: "Seeker", summary: "Feeds [[family:drones]].", family: "drones", kind: Variant,
                 cost: (data: {"drones": 5, "kinetic": 2.5}), requires: ["automaton_cannon"],
                 unlocks: [MindCapacity(3)])]"#,
        ),
        (
            "starts/alt.ron",
            r#"(id: "alt", name: "Alt start", summary: "Like [[start:rim_awakening]], with more mind.", ship: "sample_early",
                mind_capacity: 3, voyages: ["yard_approach", "halvex_drift"])"#,
        ),
        ("wiki/family/drones.md", "# Drones\n\n[[start:alt]]"),
        ("wiki/research/seeker.md", "Seeks."),
        ("wiki/start/alt.md", "Starts."),
    ]);
    assert_eq!(
        (m.families.len(), m.research.len(), m.starts.len()),
        (1, 1, 1)
    );
    let c = Content::load(&[base(), m]).unwrap_or_else(|p| panic!("{}", show(&p)));
    // New ids are appended after the base entries.
    assert_eq!(c.families.iter().last().unwrap().id, "drones");
    assert_eq!(
        c.families.get("drones").unwrap().source.file,
        "families/extra.ron"
    );
    let seeker = c.research.get("seeker").unwrap();
    assert_eq!(seeker.kind, ResearchKind::Variant);
    assert_eq!(seeker.cost.data["kinetic"], 2.5);
    assert!(seeker.cost.materials.is_empty());
    assert_eq!(seeker.unlocks, [Unlock::MindCapacity(3)]);
    let alt = c.starts.get("alt").unwrap();
    assert_eq!(alt.voyages, ["yard_approach", "halvex_drift"]);
    // Kinds like any other: keys, dirs, names, links, text keys, wiki prose.
    for (kind, key, dir) in [
        (Kind::Family, "family", "families"),
        (Kind::Research, "research", "research"),
        (Kind::Start, "start", "starts"),
    ] {
        assert_eq!(Kind::from_key(key), Some(kind));
        assert_eq!(Kind::from_dir(dir), Some(kind));
    }
    assert!(c.exists(Kind::Research, "seeker") && !c.exists(Kind::Research, "nope"));
    assert_eq!(c.name_of(Kind::Start, "alt"), Some("Alt start"));
    assert_eq!(
        c.render_plain("[[family:drones]], [[research:seeker]], [[start:alt|another]]"),
        "Drones, Seeker, another"
    );
    let texts = c.texts();
    assert_eq!(texts["family.drones.name"], "Drones");
    assert_eq!(
        texts["research.seeker.summary"],
        "Feeds [[family:drones]]."
    );
    assert!(texts.contains_key("start.alt.summary"));
    assert!(c.wiki_page(Kind::Research, "seeker").is_some());
}

#[test]
fn mods_override_new_kinds_in_place() {
    let base_only = Content::load(&[base()]).unwrap();
    let m = pack(&[(
        "families.ron",
        r#"[(id: "power", name: "Power and storage", summary: "Reactors.", color: "power")]"#,
    )]);
    let c = Content::load(&[base(), m]).unwrap_or_else(|p| panic!("{}", show(&p)));
    assert_eq!(c.families.get("power").unwrap().name, "Power and storage");
    assert_eq!(
        c.families.position("power"),
        base_only.families.position("power")
    );
    assert_eq!(c.families.len(), base_only.families.len());
}

#[test]
fn layout_errors_name_the_new_dirs_and_kinds() {
    let err = Pack::from_files([("pack.ron", MOD_MANIFEST), ("tech/x.ron", "[]")]).unwrap_err();
    assert!(
        err[0]
            .message
            .contains("facilities, starts, voyages, encounters, galaxy, bands, cluster_types, rocks, landmarks, planets, trails, signal_sources, lore or controls"),
        "{}",
        err[0]
    );
    let err = Pack::from_files([("pack.ron", MOD_MANIFEST), ("wiki/tech/x.md", "hi")]).unwrap_err();
    assert!(
        err[0]
            .message
            .contains("start, voyage, encounter, galaxy, band, cluster_type, rock, landmark, planet, trail, signal_source, lore, control or page"),
        "{}",
        err[0]
    );
    // Duplicate ids within a pack, as for every kind.
    let start = r#"(id: "s", name: "S", summary: "S.", ship: "sample_early", mind_capacity: 2)"#;
    let err = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        ("starts/a.ron", start),
        ("starts/b.ron", start),
    ])
    .unwrap_err();
    assert!(err[0].message.contains("duplicate id"), "{}", err[0]);
    // Unknown fields fail loudly; a node needs a kind, a cost and its unlocks.
    let err = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        (
            "research/x.ron",
            r#"(id: "x", name: "X", summary: "S.", family: "power", kind: NewPart, unlocks: [])"#,
        ),
    ])
    .unwrap_err();
    assert!(err[0].message.contains("cost"), "{}", err[0]);
    assert_eq!(err[0].kind, Some(Kind::Research));
}

// ---------------------------------------------------------------------------------------------------------------
// Families

#[test]
fn family_colors_are_system_colour_keys() {
    let p = problems_with(&[(
        "families.ron",
        r#"[(id: "a", name: "A", summary: "S.", color: "pink"), (id: "b", name: "B", summary: "S.", color: "heat"),
            (id: "c", name: "C", summary: "S.")]"#,
    )]);
    assert_error(&p, "a", "color", "unknown system colour key `pink`");
    assert_clean(&p, "b");
    assert_clean(&p, "c");
}

// ---------------------------------------------------------------------------------------------------------------
// Salvage blocks

const PART: &str = r#"id: "p", name: "P", summary: "S.", category: "logic", size: (1, 1), build_points: 3, mass: 1, hp: 1"#;

#[test]
fn salvage_on_parts() {
    let good = format!(
        r#"({PART}, salvage: (family: "logic", data: 2.5, fragment: {{"programmable_matter": 2}}), recipe: (materials: {{"programmable_matter": 1}}))"#
    );
    let p = problems_with(&[("parts/p.ron", &good)]);
    assert_clean(&p, "p");
    let c = Content::merge(&[base(), pack(&[("parts/p.ron", &good)])]);
    let s = c.parts.get("p").unwrap().salvage.clone().unwrap();
    assert_eq!((s.family.as_str(), s.data), ("logic", 2.5));
    assert_eq!(s.fragment["programmable_matter"], 2);
    assert_eq!(c.parts.get("p").unwrap().recipe.as_ref().unwrap().materials["programmable_matter"], 1);
    // Data alone is enough; zero data is allowed.
    let p = problems_with(&[(
        "parts/p.ron",
        &format!(r#"({PART}, salvage: (family: "kinetic", data: 0))"#),
    )]);
    assert_clean(&p, "p");

    let bad = format!(
        r#"({PART}, salvage: (family: "lasers", data: -1, fragment: {{"steel": 0}}), recipe: (materials: {{"unobtainium": 1}}, needs: "evolution"))"#
    );
    let p = problems_with(&[("parts/p.ron", &bad)]);
    assert_error(
        &p,
        "p",
        "salvage.family",
        "unknown research family `lasers`",
    );
    assert_error(&p, "p", "salvage.data", "finite number >= 0");
    assert_error(&p, "p", "recipe.materials.unobtainium", "unknown material");
    assert_error(&p, "p", "recipe.needs", "no workshop upgrade");
    assert_error(&p, "p", "salvage.fragment.steel", "an amount of 1 to");
    let p = problems_with(&[(
        "parts/p.ron",
        &format!(
            r#"({PART}, salvage: (family: "logic", data: 20000, fragment: {{"programmable_matter": 20000}}))"#
        ),
    )]);
    assert_error(&p, "p", "salvage.data", "at most 10000");
    assert_error(
        &p,
        "p",
        "salvage.fragment.programmable_matter",
        "an amount of 1 to",
    );
    // Hull parts are salvaged per cell by their material.
    let p = problems_with(&[(
        "parts/p.ron",
        r#"(id: "p", name: "P", summary: "S.", category: "hull", size: (1, 1), build_points: 1, hull: true,
            salvage: (family: "hull", data: 1))"#,
    )]);
    assert_error(&p, "p", "salvage", "per cell by their material");
}

const ARMOR: &str =
    r#"armor: (tone: "steel", mass_per_cell: 1, hp_per_cell: 1, build_point_factor: 1)"#;

#[test]
fn salvage_on_materials() {
    let p = problems_with(&[(
        "materials/m.ron",
        &format!(
            r#"[(id: "good", name: "G", summary: "S.", {ARMOR}, salvage: (family: "hull", data: 1.5)),
                (id: "bad", name: "B", summary: "S.", {ARMOR}, salvage: (family: "plating", data: -2)),
                (id: "loose", name: "L", summary: "S.", salvage: (family: "hull", data: 1))]"#
        ),
    )]);
    assert_clean(&p, "good");
    assert_error(
        &p,
        "bad",
        "salvage.family",
        "unknown research family `plating`",
    );
    assert_error(&p, "bad", "salvage.data", "finite number >= 0");
    assert_error(&p, "loose", "salvage", "only armour materials");
    // A hull cell gives data only: materials and fragments are part fields (a parse error here).
    let err = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        (
            "materials/m.ron",
            &*format!(
                r#"(id: "m", name: "M", summary: "S.", {ARMOR}, salvage: (family: "hull", data: 1, fragment: {{"steel": 1}}))"#
            ),
        ),
    ])
    .unwrap_err();
    assert!(err[0].message.contains("fragment"), "{}", err[0]);
}

// ---------------------------------------------------------------------------------------------------------------
// Frames

#[test]
fn locked_frames() {
    let c = content();
    assert!(!c.frames.get("frigate").unwrap().locked);
    let lock = |id: &str, size: u32| {
        format!(
            r#"(id: "{id}", name: "{id}", summary: "Locked.", size: ({size}, {size}), locked: true)"#
        )
    };
    // Some frames may be locked; not all of them.
    let p = problems_with(&[("frames.ron", &format!("[{}]", lock("cruiser", 40)))]);
    assert!(
        !p.iter().any(|p| p.field.as_deref() == Some("locked")),
        "{}",
        show(&p)
    );
    let all = format!(
        "[{}, {}, {}]",
        lock("frigate", 24),
        lock("cruiser", 40),
        lock("capital", 64)
    );
    let p = problems_with(&[("frames.ron", &all)]);
    assert_error(&p, "frigate", "locked", "every frame is locked");
}

// ---------------------------------------------------------------------------------------------------------------
// Research nodes

/// A node with the given extra fields (family, kind and cost given unless overridden in `fields`).
fn node(id: &str, fields: &str) -> String {
    format!(r#"(id: "{id}", name: "{id}", summary: "S.", {fields})"#)
}

#[test]
fn a_valid_node_is_clean() {
    let n = node(
        "n",
        r#"family: "energy", kind: NewPart, cost: (data: {"energy": 5, "power": 1}, materials: {"programmable_matter": 1}),
           requires: ["beam_laser"], unlocks: [Part("automaton_cannon"), Material("automaton_plate"), Frame("capital"), MindCapacity(2)]"#,
    );
    let p = problems_with(&[("research/n.ron", &n)]);
    assert_clean(&p, "n");
}

#[test]
fn node_references_and_amounts() {
    let n = node(
        "n",
        r#"family: "lasers", kind: NewPart, cost: (data: {"lasers": 5, "energy": 0, "power": -1, "kinetic": 50000},
           materials: {"unobtainium": 1, "programmable_matter": 0}),
           requires: ["warp_theory", "beam_laser", "beam_laser"],
           unlocks: [Part("warp_core"), Material("wood"), Frame("dreadnought"), MindCapacity(0), MindCapacity(500)]"#,
    );
    let p = problems_with(&[("research/n.ron", &n)]);
    assert_error(&p, "n", "family", "unknown research family `lasers`");
    assert_error(
        &p,
        "n",
        "cost.data.lasers",
        "unknown research family `lasers`",
    );
    assert_error(&p, "n", "cost.data.energy", "finite number > 0");
    assert_error(&p, "n", "cost.data.power", "finite number > 0");
    assert_error(&p, "n", "cost.data.kinetic", "at most 10000");
    assert_error(&p, "n", "cost.materials.unobtainium", "unknown material");
    assert_error(
        &p,
        "n",
        "cost.materials.programmable_matter",
        "an amount of 1 to",
    );
    assert_error(
        &p,
        "n",
        "requires[0]",
        "unknown research node `warp_theory`",
    );
    assert_error(&p, "n", "requires[2]", "listed twice");
    assert_error(&p, "n", "unlocks[0]", "unknown part `warp_core`");
    assert_error(&p, "n", "unlocks[1]", "unknown material `wood`");
    assert_error(&p, "n", "unlocks[2]", "unknown frame `dreadnought`");
    assert_error(&p, "n", "unlocks[3]", "mind capacity of 1 to 100");
    assert_error(&p, "n", "unlocks[4]", "mind capacity of 1 to 100");
}

#[test]
fn unlocking_what_is_open_or_nothing_is_a_warning() {
    let n = node(
        "n",
        r#"family: "energy", kind: NewPart, cost: (data: {"energy": 1}),
           unlocks: [Part("pulse_laser"), Material("steel"), Frame("frigate")]"#,
    );
    let empty = node(
        "e",
        r#"family: "energy", kind: Refinement, cost: (), unlocks: []"#,
    );
    let p = problems_with(&[("research/n.ron", &format!("[{n}, {empty}]"))]);
    assert!(of(&p, "n", Severity::Error).is_empty(), "{}", show(&p));
    for (k, what) in [
        (0, "part `pulse_laser`"),
        (1, "material `steel`"),
        (2, "frame `frigate`"),
    ] {
        assert_problem(&p, Severity::Warning, "n", &format!("unlocks[{k}]"), what);
    }
    assert_problem(&p, Severity::Warning, "e", "unlocks", "unlocks nothing");
    assert!(of(&p, "e", Severity::Error).is_empty(), "{}", show(&p));
}

#[test]
fn requires_must_not_loop() {
    // a → b → c → a, plus d → b (into the loop, not part of it) and a node that requires itself.
    let file = format!(
        "[{}, {}, {}, {}, {}]",
        node(
            "a",
            r#"family: "power", kind: NewPart, cost: (), requires: ["b"], unlocks: [MindCapacity(1)]"#
        ),
        node(
            "b",
            r#"family: "power", kind: NewPart, cost: (), requires: ["c"], unlocks: [MindCapacity(1)]"#
        ),
        node(
            "c",
            r#"family: "power", kind: NewPart, cost: (), requires: ["a"], unlocks: [MindCapacity(1)]"#
        ),
        node(
            "d",
            r#"family: "power", kind: NewPart, cost: (), requires: ["b"], unlocks: [MindCapacity(1)]"#
        ),
        node(
            "me",
            r#"family: "power", kind: NewPart, cost: (), requires: ["me"], unlocks: [MindCapacity(1)]"#
        ),
    );
    let p = problems_with(&[("research/loop.ron", &file)]);
    let cycles: Vec<&Problem> = p.iter().filter(|p| p.message.contains("cycle")).collect();
    assert_eq!(cycles.len(), 2, "{}", show(&p));
    assert_error(&p, "c", "requires[0]", "a → b → c → a");
    assert_error(&p, "me", "requires[0]", "me → me");
    assert!(of(&p, "d", Severity::Error).is_empty(), "{}", show(&p));
    // A diamond (two paths to the same node) is no cycle.
    let file = format!(
        "[{}, {}, {}, {}]",
        node(
            "top",
            r#"family: "power", kind: NewPart, cost: (), requires: ["l", "r"], unlocks: [MindCapacity(1)]"#
        ),
        node(
            "l",
            r#"family: "power", kind: NewPart, cost: (), requires: ["root"], unlocks: [MindCapacity(1)]"#
        ),
        node(
            "r",
            r#"family: "power", kind: NewPart, cost: (), requires: ["root"], unlocks: [MindCapacity(1)]"#
        ),
        node(
            "root",
            r#"family: "power", kind: NewPart, cost: (), unlocks: [MindCapacity(1)]"#
        ),
    );
    let p = problems_with(&[("research/diamond.ron", &file)]);
    assert!(
        !p.iter().any(|p| p.message.contains("cycle")),
        "{}",
        show(&p)
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Starts

fn start(fields: &str) -> String {
    format!(r#"(id: "s", name: "S", summary: "S.", {fields})"#)
}

#[test]
fn a_start_needs_a_ship_a_new_game_can_fly() {
    let p = problems_with(&[(
        "starts/s.ron",
        &start(r#"ship: "sample_early", mind_capacity: 2, voyages: ["yard_approach"]"#),
    )]);
    assert_clean(&p, "s");
    let p = problems_with(&[(
        "starts/s.ron",
        &start(
            r#"ship: "warp_yacht", mind_capacity: 0, voyages: ["nowhere", "halvex_drift", "halvex_drift"]"#,
        ),
    )]);
    assert_error(&p, "s", "ship", "unknown ship `warp_yacht`");
    assert_error(&p, "s", "mind_capacity", "mind capacity of 1 to 100");
    assert_error(&p, "s", "voyages[0]", "unknown voyage `nowhere`");
    assert_error(&p, "s", "voyages[2]", "listed twice");
    // The Halvex Runner is built from locked parts and composite.
    let p = problems_with(&[(
        "starts/s.ron",
        &start(r#"ship: "halvex_runner", mind_capacity: 2"#),
    )]);
    assert_error(&p, "s", "ship", "part `beam_laser`");
    assert_error(&p, "s", "ship", "material `composite`");
    // Two logic cores need a mind capacity of two.
    let p = problems_with(&[(
        "starts/s.ron",
        &start(r#"ship: "sample_early", mind_capacity: 1"#),
    )]);
    assert_error(
        &p,
        "s",
        "mind_capacity",
        "2 logic cores, more than the mind capacity 1",
    );
    // A ship that fits no open frame.
    let wide = r#"(id: "wide", name: "Wide", summary: "S.", faction: "ship_mind", size: (30, 1), parts: [
        (part: "logic_core", x: 0, y: 0)])"#;
    let p = problems_with(&[
        ("ships/wide.ron", wide),
        ("starts/s.ron", &start(r#"ship: "wide", mind_capacity: 1"#)),
    ]);
    assert_error(&p, "s", "ship", "fits no frame that is not locked");
    // Without a logic core nothing flies it; without a hold (a warning) nothing salvaged comes home.
    let hulk = r#"(id: "hulk", name: "Hulk", summary: "S.", faction: "ship_mind", size: (2, 1), parts: [
        (part: "armor_block", x: 0, y: 0, material: "steel"), (part: "cargo_bay", x: 1, y: 0)])"#;
    let dry = r#"(id: "dry", name: "Dry", summary: "S.", faction: "ship_mind", size: (3, 1), parts: [
        (part: "logic_core", x: 0, y: 0), (part: "fission_reactor", x: 1, y: 0), (part: "thruster", x: 2, y: 0)])"#;
    let p = problems_with(&[
        ("ships/hulk.ron", hulk),
        ("ships/dry.ron", dry),
        ("starts/s.ron", &start(r#"ship: "hulk", mind_capacity: 1"#)),
        ("starts/t.ron", &start(r#"ship: "dry", mind_capacity: 1"#).replace("\"s\"", "\"t\"")),
    ]);
    assert_error(&p, "s", "ship", "has no logic core");
    assert_problem(&p, Severity::Warning, "t", "ship", "has no hold");
    assert!(of(&p, "t", Severity::Error).is_empty(), "{p:?}");
}

/// Content without any start can't begin a career: an error (13 Oct 2026 review).
#[test]
fn content_needs_a_start() {
    let mut c = Content::merge(&[base()]);
    assert!(
        data::validate(&c)
            .iter()
            .all(|p| p.field.as_deref() != Some("starts"))
    );
    c.starts = Default::default();
    let p = data::validate(&c);
    assert!(
        p.iter().any(|p| p.severity == Severity::Error
            && p.field.as_deref() == Some("starts")
            && p.message.contains("no start")),
        "{p:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The base pack

#[test]
fn base_families() {
    let c = content();
    let families: Vec<(&str, Option<&str>)> = c
        .families
        .iter()
        .map(|f| (f.id.as_str(), f.color.as_deref()))
        .collect();
    assert_eq!(
        families,
        [
            ("hull", None),
            ("power", Some("power")),
            ("energy", Some("weapons")),
            ("kinetic", Some("weapons")),
            ("missiles", Some("weapons")),
            ("shields", Some("shields")),
            ("thermal", Some("heat")),
            ("propulsion", Some("propulsion")),
            ("logic", Some("logic")),
            ("utility", Some("cargo")),
            ("grown", Some("enemy")),
            ("swarm", Some("swarm")),
        ]
    );
}

#[test]
fn base_salvage_blocks() {
    let c = content();
    // Every non-hull part and every armour material is salvageable; hull parts go by their material.
    for p in c.parts.iter() {
        assert_eq!(p.salvage.is_some(), !p.hull, "{}", p.id);
    }
    for m in c.materials.iter() {
        assert_eq!(m.salvage.is_some(), m.armor.is_some(), "{}", m.id);
    }
    let part = |id: &str| c.parts.get(id).unwrap().salvage.clone().unwrap();
    for (id, family, data) in [
        ("automaton_cannon", "kinetic", 3.0),
        ("thruster", "propulsion", 3.0),
        ("automaton_core", "logic", 4.0),
        ("logic_core", "logic", 3.0),
        ("pulse_laser", "energy", 3.0),
        ("beam_laser", "energy", 7.0),
        ("fission_reactor", "power", 4.0),
        ("fusion_reactor", "power", 10.0),
        ("torch_drive", "propulsion", 9.0),
        ("shield_emitter", "shields", 3.0),
        ("radiator", "thermal", 2.0),
        ("capacitor", "power", 2.0),
        ("grown_heart", "grown", 6.0),
        ("ram_prow", "grown", 4.0),
        ("thorn_gun", "grown", 3.0),
        ("grown_ganglion", "grown", 5.0),
    ] {
        let s = part(id);
        assert_eq!((s.family.as_str(), s.data), (family, data), "{id}");
    }
    // A destroyed core leaves a fragment of programmable matter (GDR-0063), and its recipe takes some (GDR-0031:
    // it hosts a copy of you); nothing else leaves one, and the Grown's ganglion neither (GDR-0103: the Grown are not
    // precursor work). A Grown heart leaves heartwood instead (18 Oct 2026). The Swarm's nerve node is no precursor
    // core either: it leaves a nerve core, and their brood hangar resin (GDR-0117).
    for p in c
        .parts
        .iter()
        .filter_map(|p| p.salvage.as_ref().map(|s| (p, s)))
    {
        let race = p.0.tags.iter().any(|t| t == "grown" || t == "swarm");
        let core = p.0.logic.is_some() && !race;
        let heart = p.0.heart.is_some();
        let swarm = matches!(p.0.id.as_str(), "swarm_node" | "brood_hangar");
        assert_eq!(!p.1.fragment.is_empty(), core || heart || swarm, "{}", p.0.id);
        let pm = p.0.recipe.as_ref().is_some_and(|r| r.materials.contains_key("programmable_matter"));
        assert!(!core || pm, "{}", p.0.id);
        // Every part but hull has a recipe (GDR-0108).
        assert!(p.0.recipe.is_some(), "{}", p.0.id);
    }
    assert_eq!(part("automaton_core").fragment["programmable_matter"], 1);
    assert_eq!(part("grown_heart").fragment.get("heartwood"), Some(&1));
    assert_eq!(part("swarm_node").fragment.get("nerve_core"), Some(&1));
    assert_eq!(part("brood_hangar").fragment.get("resin"), Some(&2));
    let hull = |id: &str| c.materials.get(id).unwrap().salvage.clone().unwrap();
    assert_eq!(
        (hull("steel").family.as_str(), hull("steel").data),
        ("hull", 1.0)
    );
    assert_eq!(hull("composite").data, 1.5);
    // Programmable matter: a material, but no armour.
    let pm = c.materials.get("programmable_matter").unwrap();
    assert!(pm.armor.is_none() && pm.salvage.is_none() && !pm.locked);
    assert!(c.icon_svg(pm.icon.as_deref().unwrap()).is_some());
}

#[test]
fn base_locks_follow_the_early_game_kit() {
    let c = content();
    let locked_parts: Vec<&str> = c
        .parts
        .iter()
        .filter(|p| p.locked)
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(
        locked_parts,
        [
            "automaton_core",
            "automaton_cannon",
            "prism_ward", // the Spire's (GDR-0116): enemy only, no research unlocks it
            "wide_shield_emitter",
            "ion_cannon", // the logic research's (GDR-0125)
            "grown_heart",
            "ram_prow",
            "thorn_gun",
            "grown_ganglion",
            "spore_radiator",
            "heat_sink",
            "rocket_pod",
            "torpedo_launcher",
            "point_defence",
            "flare_launcher",
            "fusion_reactor",
            "compact_reactor",
            "torch_drive",
            "swarm_node", // the Swarm's own (GDR-0117), like the ganglion, the jet and the brood hangar
            "swarm_jet",
            "stinger", // the Swarm family unlocks it and the drone bay (GDR-0117)
            "brood_hangar",
            "drone_bay",
            "beam_laser",
            "reverse_engineered_cannon",
            "burst_laser",
            "long_cannon",
            "ammo_feeder"
        ]
    );
    let locked_materials: Vec<&str> = c
        .materials
        .iter()
        .filter(|m| m.locked)
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(
        locked_materials,
        ["composite", "reactive", "automaton_plate", "grown_tissue", "chitin"]
    );
    let open_frames: Vec<&str> = c
        .frames
        .iter()
        .filter(|f| !f.locked)
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(open_frames, ["frigate"]);
}

#[test]
fn base_research_nodes() {
    let c = content();
    let ids: Vec<&str> = c.research.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "regrowing_tissue",
            "living_weapons",
            "grown_heart",
            "spore_lining",
            "smelter",
            "rocket_pod",
            "point_defence",
            "torpedo",
            "flares",
            "automaton_cannon",
            "composite_plating",
            "beam_laser",
            "fusion_reactor",
            "torch_drive",
            "jump_calibration_1",
            "jump_calibration_2",
            "tamed_core",
            "mind_expansion_1",
            "mind_expansion_2",
            "simulation_vault",
            "ion_cannon",
            "cruiser_drydock",
            "reactive_plating",
            "automaton_forge",
            "capital_drydock",
            "wide_bubble",
            "heat_sink",
            "ammo_feeder",
            "burst_laser",
            "long_cannon",
            "compact_reactor",
            "chitin_plating",
            "stingers",
            "drone_bay",
            "shield_regen_1",
            "shield_regen_2",
            "shield_regen_3",
            "radiator_vent_1",
            "radiator_vent_2",
            "radiator_vent_3",
            "thruster_speed_1",
            "thruster_speed_2",
            "thruster_speed_3",
            "pulse_rate_1",
            "pulse_rate_2",
            "pulse_rate_3",
        ]
    );
    // The gate shards open the rim's three dead ends (GDR-0031): only the automaton plate stays the automatons' own.
    let unlocked = |u: Unlock| c.research.iter().any(|n| n.unlocks.contains(&u));
    for m in c.materials.iter().filter(|m| m.locked && m.id != "automaton_plate") {
        assert!(unlocked(Unlock::Material(m.id.clone())), "{}", m.id);
    }
    for f in c.frames.iter().filter(|f| f.locked) {
        assert!(unlocked(Unlock::Frame(f.id.clone())), "{}", f.id);
    }
    for f in c.facilities.iter().filter(|f| f.locked) {
        assert!(unlocked(Unlock::Facility(f.id.clone())), "{}", f.id);
    }
    for id in ["reactive_plating", "automaton_forge", "capital_drydock"] {
        assert!(c.research.get(id).unwrap().cost.materials.contains_key("gate_shard"), "{id}");
    }
    // The ammo feeder speeds guns tagged `cannon` (GDR-0018): the player's cannons, never the automatons' own.
    let tagged = |id: &str| c.parts.get(id).unwrap().tags.iter().any(|t| t == "cannon");
    assert!(tagged("reverse_engineered_cannon") && tagged("long_cannon") && !tagged("automaton_cannon"));
    let feeder = &c.parts.get("ammo_feeder").unwrap().bonuses[0];
    assert_eq!((feeder.stat.as_str(), feeder.max_stacks), ("fire_rate", Some(2)));
    // Every unlock target is locked in content, so every node changes something.
    for n in c.research.iter() {
        for u in &n.unlocks {
            let locked = match u {
                Unlock::Part(id) => c.parts.get(id).unwrap().locked,
                Unlock::Material(id) => c.materials.get(id).unwrap().locked,
                Unlock::Frame(id) => c.frames.get(id).unwrap().locked,
                Unlock::Facility(id) => c.facilities.get(id).unwrap().locked,
                Unlock::MindCapacity(n) => *n > 0,
                Unlock::JumpRange(s) => *s > 0.0,
                Unlock::PartStat { step: data::Effect::Add(x) | data::Effect::Percent(x), .. } => *x > 0.0,
            };
            assert!(locked, "{}: {u:?}", n.id);
        }
    }
    let n = c.research.get("mind_expansion_2").unwrap();
    assert_eq!(n.kind, ResearchKind::Refinement);
    assert_eq!(n.requires, ["mind_expansion_1"]);
    assert_eq!(n.cost.data["logic"], 16.0);
    assert_eq!(n.cost.materials["programmable_matter"], 5);
    assert_eq!(
        c.research.get("cruiser_drydock").unwrap().unlocks,
        [Unlock::Frame("cruiser".into())]
    );
}

#[test]
fn base_start() {
    let c = content();
    let ids: Vec<&str> = c.starts.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["rim_awakening"]);
    let s = c.starts.get("rim_awakening").unwrap();
    assert_eq!((s.ship.as_str(), s.mind_capacity), ("rim_skiff", 2));
    assert_eq!(s.voyages, ["yard_approach"]);
    // The mind grows to 4 by research in the first playable loop (GDR-0062).
    let more: u32 = c
        .research
        .iter()
        .flat_map(|n| &n.unlocks)
        .map(|u| match u {
            Unlock::MindCapacity(n) => *n,
            _ => 0,
        })
        .sum();
    assert_eq!(s.mind_capacity + more, 4);
    // The starter is built from the early-game kit, carries salvage home and a scanner (GDR-0093: in place of a
    // cargo bay).
    let skiff = c.ships.get("rim_skiff").unwrap();
    for pl in &skiff.parts {
        assert!(!c.parts.get(&pl.part).unwrap().locked, "{}", pl.part);
        assert!(
            c.placement_material(pl).is_none_or(|m| !m.locked),
            "{}",
            pl.part
        );
    }
    let totals = c.ship_totals(skiff);
    assert_eq!((totals.logic_cores, totals.cargo_capacity), (2, 4.0));
    assert!(skiff.parts.iter().filter(|p| p.part == "gyro").count() == 2);
    assert!(skiff.parts.iter().filter(|p| p.part == "scanner").count() == 1);
}

// ---------------------------------------------------------------------------------------------------------------
// Refinement: part-stat steps (GDR-0024)

#[test]
fn part_stat_steps_validate_like_bonuses() {
    let node = |id: &str, unlock: &str| {
        format!(r#"(id: "{id}", name: "X", summary: "X.", family: "power", kind: Refinement, cost: (), unlocks: [{unlock}])"#)
    };
    let nodes = [
        node("good", r#"PartStat(target: Tag("radiator"), stat: "heat_vent", step: Percent(5))"#),
        node("not_bonus", r#"PartStat(target: Part("thruster"), stat: "mass", step: Add(1))"#),
        node("no_part", r#"PartStat(target: Part("warp_core"), stat: "thrust", step: Percent(5))"#),
        node("lands_nowhere", r#"PartStat(target: Tag("radiator"), stat: "shield_regen", step: Percent(5))"#),
        node("too_big", r#"PartStat(target: Part("thruster"), stat: "thrust", step: Percent(-100))"#),
    ];
    let p = problems_with(&[("research.ron", &format!("[{}]", nodes.join(",")))]);
    assert_clean(&p, "good");
    assert_error(&p, "not_bonus", "unlocks[0].stat", "research steps cannot change `mass`");
    assert_error(&p, "no_part", "unlocks[0].target", "unknown part `warp_core`");
    assert_error(&p, "lands_nowhere", "unlocks[0]", "would change nothing");
    assert_error(&p, "too_big", "unlocks[0].step", "above -100");
}

#[test]
fn a_careers_steps_change_its_stat_lines() {
    let c = data::base_content();
    // Content order, whatever order the nodes were researched in; unknown ids are skipped.
    let up = data::Upgrades::of_research(&c, ["pulse_rate_2", "nonsense", "radiator_vent_1", "pulse_rate_1"]);
    let stats: Vec<&str> = up.0.iter().map(|s| s.stat.as_str()).collect();
    assert_eq!(stats, ["heat_vent", "fire_rate", "fire_rate"]);
    let line = |part: &str, stat: &str, up: &data::Upgrades| {
        let p = c.parts.get(part).unwrap();
        c.part_stats_upgraded(p, None, up).into_iter().find(|l| l.stat == stat && l.bonus.is_none()).unwrap()
    };
    let rad = line("radiator", "heat_vent", &up);
    let base = rad.upgraded_from.unwrap();
    assert!((rad.value - base * 1.08).abs() < 1e-5, "{rad:?}");
    // A fire-rate step shows on the shot rate and the damage per second; untouched lines stay as they are.
    let rate = line("pulse_laser", "shot_rate", &up);
    assert!((rate.value - rate.upgraded_from.unwrap() * 1.06).abs() < 1e-5, "{rate:?}");
    assert!(line("pulse_laser", "damage", &up).upgraded_from.is_some());
    assert_eq!(line("pulse_laser", "range", &up).upgraded_from, None);
    assert_eq!(line("burst_laser", "shot_rate", &up).upgraded_from, None);
    // No steps: the content's lines.
    let p = c.parts.get("radiator").unwrap();
    assert_eq!(c.part_stats_upgraded(p, None, &data::Upgrades::NONE), c.part_stats(p, None));
}

/// A part research unlocks is fabricated once the node is researched (GDR-0108's upgrades): the node, or one it
/// requires, installs the workshop upgrade its recipe needs. Two wait for another node, as GDR-0108 has it: the
/// automatons' core and cannon for the forge (it takes a gate shard, GDR-0031), the torch drive for the fusion reactor's.
#[test]
fn researched_parts_can_be_fabricated() {
    let c = content();
    let installs = |id: &str, f: &str| -> bool {
        let mut open = vec![id];
        while let Some(n) = open.pop() {
            let n = c.research.get(n).unwrap();
            if n.unlocks.contains(&Unlock::Facility(f.into())) {
                return true;
            }
            open.extend(n.requires.iter().map(String::as_str));
        }
        false
    };
    let mut gated = Vec::new();
    for n in c.research.iter() {
        for u in &n.unlocks {
            let Unlock::Part(p) = u else { continue };
            let Some(f) = c.parts.get(p).unwrap().recipe.as_ref().and_then(|r| r.needs.as_deref()) else { continue };
            if let Some(by) = [("tamed_core", "automaton_forge"), ("automaton_cannon", "automaton_forge"), ("torch_drive", "fusion_reactor")]
                .iter()
                .find_map(|&(x, by)| (x == n.id).then_some(by))
            {
                assert!(installs(by, f), "{}: {p} needs {f}", n.id);
                continue;
            }
            assert!(installs(&n.id, f), "{}: {p} needs {f}", n.id);
            gated.push((p.as_str(), f));
        }
    }
    for (p, f) in [
        ("rocket_pod", "munitions_bay"),
        ("torpedo_launcher", "munitions_bay"),
        ("flare_launcher", "munitions_bay"),
        ("stinger", "hive_press"),
        ("drone_bay", "hive_press"),
    ] {
        assert!(gated.contains(&(p, f)), "{p} needs {f}");
    }
    assert!(c.parts.get("point_defence").unwrap().recipe.as_ref().unwrap().needs.is_none(), "it fires pulses");
    assert!(c.parts.get("ion_cannon").unwrap().recipe.as_ref().unwrap().needs.is_none(), "it fires pulses");
}
