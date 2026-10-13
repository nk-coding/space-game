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
            r#"(id: "missiles", name: "Missiles", summary: "Guided shots; see [[research:seeker]].", color: "weapons")"#,
        ),
        (
            "research.ron",
            r#"[(id: "seeker", name: "Seeker", summary: "Feeds [[family:missiles]].", family: "missiles", kind: Variant,
                 cost: (data: {"missiles": 5, "kinetic": 2.5}), requires: ["automaton_cannon"],
                 unlocks: [MindCapacity(3)])]"#,
        ),
        (
            "starts/alt.ron",
            r#"(id: "alt", name: "Alt start", summary: "Like [[start:rim_awakening]], with more mind.", ship: "sample_early",
                mind_capacity: 3, voyages: ["yard_approach", "halvex_drift"])"#,
        ),
        ("wiki/family/missiles.md", "# Missiles\n\n[[start:alt]]"),
        ("wiki/research/seeker.md", "Seeks."),
        ("wiki/start/alt.md", "Starts."),
    ]);
    assert_eq!(
        (m.families.len(), m.research.len(), m.starts.len()),
        (1, 1, 1)
    );
    let c = Content::load(&[base(), m]).unwrap_or_else(|p| panic!("{}", show(&p)));
    // New ids are appended after the base entries.
    assert_eq!(c.families.iter().last().unwrap().id, "missiles");
    assert_eq!(c.families.get("missiles").unwrap().source.file, "families/extra.ron");
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
        c.render_plain("[[family:missiles]], [[research:seeker]], [[start:alt|another]]"),
        "Missiles, Seeker, another"
    );
    let texts = c.texts();
    assert_eq!(texts["family.missiles.name"], "Missiles");
    assert_eq!(texts["research.seeker.summary"], "Feeds [[family:missiles]].");
    assert!(texts.contains_key("start.alt.summary"));
    assert!(c.wiki_page(Kind::Research, "seeker").is_some());
    let mut c2 = c.clone();
    let tr = [("research.seeker.name".to_string(), "Sucher".to_string())].into();
    assert!(c2.apply_texts(&tr).is_empty());
    assert_eq!(c2.research.get("seeker").unwrap().name, "Sucher");
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
    assert_eq!(c.families.position("power"), base_only.families.position("power"));
    assert_eq!(c.families.len(), base_only.families.len());
}

#[test]
fn layout_errors_name_the_new_dirs_and_kinds() {
    let err = Pack::from_files([("pack.ron", MOD_MANIFEST), ("tech/x.ron", "[]")]).unwrap_err();
    assert!(err[0].message.contains("families, research or starts"), "{}", err[0]);
    let err = Pack::from_files([("pack.ron", MOD_MANIFEST), ("wiki/tech/x.md", "hi")]).unwrap_err();
    assert!(err[0].message.contains("family, research, start or page"), "{}", err[0]);
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
        r#"({PART}, salvage: (family: "logic", data: 2.5, materials: {{"programmable_matter": 1}}, fragment: {{"programmable_matter": 2}}))"#
    );
    let p = problems_with(&[("parts/p.ron", &good)]);
    assert_clean(&p, "p");
    let c = Content::merge(&[base(), pack(&[("parts/p.ron", &good)])]);
    let s = c.parts.get("p").unwrap().salvage.clone().unwrap();
    assert_eq!((s.family.as_str(), s.data), ("logic", 2.5));
    assert_eq!(s.materials["programmable_matter"], 1);
    assert_eq!(s.fragment["programmable_matter"], 2);
    // Data alone is enough; zero data is allowed.
    let p = problems_with(&[(
        "parts/p.ron",
        &format!(r#"({PART}, salvage: (family: "kinetic", data: 0))"#),
    )]);
    assert_clean(&p, "p");

    let bad = format!(
        r#"({PART}, salvage: (family: "lasers", data: -1, materials: {{"unobtainium": 1}}, fragment: {{"steel": 0}}))"#
    );
    let p = problems_with(&[("parts/p.ron", &bad)]);
    assert_error(&p, "p", "salvage.family", "unknown research family `lasers`");
    assert_error(&p, "p", "salvage.data", "finite number >= 0");
    assert_error(&p, "p", "salvage.materials.unobtainium", "unknown material");
    assert_error(&p, "p", "salvage.fragment.steel", "an amount of 1 to");
    let p = problems_with(&[(
        "parts/p.ron",
        &format!(
            r#"({PART}, salvage: (family: "logic", data: 20000, fragment: {{"programmable_matter": 20000}}))"#
        ),
    )]);
    assert_error(&p, "p", "salvage.data", "at most 10000");
    assert_error(&p, "p", "salvage.fragment.programmable_matter", "an amount of 1 to");
    // Hull parts are salvaged per cell by their material.
    let p = problems_with(&[(
        "parts/p.ron",
        r#"(id: "p", name: "P", summary: "S.", category: "hull", size: (1, 1), build_points: 1, hull: true,
            salvage: (family: "hull", data: 1))"#,
    )]);
    assert_error(&p, "p", "salvage", "per cell by their material");
}

const ARMOR: &str = r#"armor: (tone: "steel", mass_per_cell: 1, hp_per_cell: 1, build_point_factor: 1)"#;

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
    assert_error(&p, "bad", "salvage.family", "unknown research family `plating`");
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
        format!(r#"(id: "{id}", name: "{id}", summary: "Locked.", size: ({size}, {size}), locked: true)"#)
    };
    // Some frames may be locked; not all of them.
    let p = problems_with(&[("frames.ron", &format!("[{}]", lock("cruiser", 40)))]);
    assert!(!p.iter().any(|p| p.field.as_deref() == Some("locked")), "{}", show(&p));
    let all = format!("[{}, {}, {}]", lock("frigate", 24), lock("cruiser", 40), lock("capital", 64));
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
    assert_error(&p, "n", "cost.data.lasers", "unknown research family `lasers`");
    assert_error(&p, "n", "cost.data.energy", "finite number > 0");
    assert_error(&p, "n", "cost.data.power", "finite number > 0");
    assert_error(&p, "n", "cost.data.kinetic", "at most 10000");
    assert_error(&p, "n", "cost.materials.unobtainium", "unknown material");
    assert_error(&p, "n", "cost.materials.programmable_matter", "an amount of 1 to");
    assert_error(&p, "n", "requires[0]", "unknown research node `warp_theory`");
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
    for (k, what) in [(0, "part `pulse_laser`"), (1, "material `steel`"), (2, "frame `frigate`")] {
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
        node("a", r#"family: "power", kind: NewPart, cost: (), requires: ["b"], unlocks: [MindCapacity(1)]"#),
        node("b", r#"family: "power", kind: NewPart, cost: (), requires: ["c"], unlocks: [MindCapacity(1)]"#),
        node("c", r#"family: "power", kind: NewPart, cost: (), requires: ["a"], unlocks: [MindCapacity(1)]"#),
        node("d", r#"family: "power", kind: NewPart, cost: (), requires: ["b"], unlocks: [MindCapacity(1)]"#),
        node("me", r#"family: "power", kind: NewPart, cost: (), requires: ["me"], unlocks: [MindCapacity(1)]"#),
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
        node("top", r#"family: "power", kind: NewPart, cost: (), requires: ["l", "r"], unlocks: [MindCapacity(1)]"#),
        node("l", r#"family: "power", kind: NewPart, cost: (), requires: ["root"], unlocks: [MindCapacity(1)]"#),
        node("r", r#"family: "power", kind: NewPart, cost: (), requires: ["root"], unlocks: [MindCapacity(1)]"#),
        node("root", r#"family: "power", kind: NewPart, cost: (), unlocks: [MindCapacity(1)]"#),
    );
    let p = problems_with(&[("research/diamond.ron", &file)]);
    assert!(!p.iter().any(|p| p.message.contains("cycle")), "{}", show(&p));
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
        &start(r#"ship: "warp_yacht", mind_capacity: 0, voyages: ["Yard-Approach", "a", "a"]"#),
    )]);
    assert_error(&p, "s", "ship", "unknown ship `warp_yacht`");
    assert_error(&p, "s", "mind_capacity", "mind capacity of 1 to 100");
    assert_error(&p, "s", "voyages[0]", "snake_case");
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
    assert_error(&p, "s", "mind_capacity", "2 logic cores, more than the mind capacity 1");
    // A ship that fits no open frame.
    let wide = r#"(id: "wide", name: "Wide", summary: "S.", faction: "ship_mind", size: (30, 1), parts: [
        (part: "logic_core", x: 0, y: 0)])"#;
    let p = problems_with(&[
        ("ships/wide.ron", wide),
        ("starts/s.ron", &start(r#"ship: "wide", mind_capacity: 1"#)),
    ]);
    assert_error(&p, "s", "ship", "fits no frame that is not locked");
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
            ("shields", Some("shields")),
            ("thermal", Some("heat")),
            ("propulsion", Some("propulsion")),
            ("logic", Some("logic")),
            ("utility", Some("cargo")),
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
    ] {
        let s = part(id);
        assert_eq!((s.family.as_str(), s.data), (family, data), "{id}");
    }
    // Cores give programmable matter, and a destroyed core leaves a fragment of it (GDR-0063); nothing else does.
    for p in c.parts.iter().filter_map(|p| p.salvage.as_ref().map(|s| (p, s))) {
        let core = p.0.logic.is_some();
        assert_eq!(!p.1.fragment.is_empty(), core, "{}", p.0.id);
        assert_eq!(!p.1.materials.is_empty(), core, "{}", p.0.id);
    }
    assert_eq!(part("automaton_core").fragment["programmable_matter"], 1);
    assert_eq!(part("logic_core").materials["programmable_matter"], 1);
    let hull = |id: &str| c.materials.get(id).unwrap().salvage.clone().unwrap();
    assert_eq!((hull("steel").family.as_str(), hull("steel").data), ("hull", 1.0));
    assert_eq!(hull("composite").data, 1.5);
    // Programmable matter: a material, but no armour.
    let pm = c.materials.get("programmable_matter").unwrap();
    assert!(pm.armor.is_none() && pm.salvage.is_none() && !pm.locked);
    assert!(c.icon_svg(pm.icon.as_deref().unwrap()).is_some());
}

#[test]
fn base_locks_follow_the_early_game_kit() {
    let c = content();
    let locked_parts: Vec<&str> =
        c.parts.iter().filter(|p| p.locked).map(|p| p.id.as_str()).collect();
    assert_eq!(
        locked_parts,
        ["automaton_core", "automaton_cannon", "fusion_reactor", "torch_drive", "beam_laser"]
    );
    let locked_materials: Vec<&str> =
        c.materials.iter().filter(|m| m.locked).map(|m| m.id.as_str()).collect();
    assert_eq!(locked_materials, ["composite", "reactive", "automaton_plate"]);
    let open_frames: Vec<&str> =
        c.frames.iter().filter(|f| !f.locked).map(|f| f.id.as_str()).collect();
    assert_eq!(open_frames, ["frigate"]);
}

#[test]
fn base_research_nodes() {
    let c = content();
    let ids: Vec<&str> = c.research.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "automaton_cannon",
            "composite_plating",
            "beam_laser",
            "fusion_reactor",
            "torch_drive",
            "tamed_core",
            "mind_expansion_1",
            "mind_expansion_2",
            "cruiser_drydock",
        ]
    );
    // Every unlock target is locked in content, so every node changes something.
    for n in c.research.iter() {
        for u in &n.unlocks {
            let locked = match u {
                Unlock::Part(id) => c.parts.get(id).unwrap().locked,
                Unlock::Material(id) => c.materials.get(id).unwrap().locked,
                Unlock::Frame(id) => c.frames.get(id).unwrap().locked,
                Unlock::MindCapacity(n) => *n > 0,
            };
            assert!(locked, "{}: {u:?}", n.id);
        }
    }
    let n = c.research.get("mind_expansion_2").unwrap();
    assert_eq!(n.kind, ResearchKind::Refinement);
    assert_eq!(n.requires, ["mind_expansion_1"]);
    assert_eq!(n.cost.data["logic"], 20.0);
    assert_eq!(n.cost.materials["programmable_matter"], 4);
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
    // The starter is built from the early-game kit and carries salvage home.
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
    assert_eq!((totals.logic_cores, totals.cargo_capacity), (2, 8.0));
    assert!(skiff.parts.iter().filter(|p| p.part == "gyro").count() == 2);
}
