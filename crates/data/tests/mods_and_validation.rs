//! Mod packs overriding base content, and validation errors with precise locations.

mod common;

use common::*;
use data::{Content, Kind, Pack, Problem, Severity};

fn base() -> Pack {
    data::base_pack().unwrap()
}

fn pack(files: &[(&str, &str)]) -> Pack {
    Pack::from_files(files.iter().copied()).unwrap_or_else(|p| panic!("{}", show(&p)))
}

const MOD_MANIFEST: &str =
    r#"(id: "test_mod", name: "Test mod", version: "1.0.0", schema_version: 1, depends: ["base"])"#;

/// Loads base + one mod file and returns the errors.
fn errors_with(path: &str, contents: &str) -> Vec<Problem> {
    let m = Pack::from_files([("pack.ron", MOD_MANIFEST), (path, contents)]);
    let problems = match m {
        Err(p) => p,
        Ok(m) => match Content::load(&[base(), m]) {
            Ok(_) => Vec::new(),
            Err(p) => p,
        },
    };
    problems
        .into_iter()
        .filter(|p| p.severity == Severity::Error)
        .collect()
}

fn assert_error(problems: &[Problem], id: &str, field: &str, needle: &str) {
    assert!(
        problems.iter().any(|p| p.id.as_deref() == Some(id)
            && p.field.as_deref() == Some(field)
            && p.message.contains(needle)),
        "expected an error for `{id}` field `{field}` containing {needle:?}, got:\n{}",
        show(problems)
    );
}

#[test]
fn mod_overrides_a_part_in_place_and_adds_content() {
    let m = pack(&[
        ("pack.ron", MOD_MANIFEST),
        (
            "parts/lasers.ron",
            r#"[
                (id: "pulse_laser", name: "Pulse laser", summary: "Hotter, stronger pulses.", tags: ["laser"], category: "weapons",
                 icon: "pulse-laser", size: (1, 1), build_points: 4, mass: 2, hp: 1, power: (draw: 5),
                 heat: (output: 2, limit: 3), weapon: (family: Energy, damage: 1.6, range: 50)),
                (id: "twin_laser", name: "Twin laser", summary: "Two pulse lasers in one housing.", tags: ["laser"], category: "weapons",
                 icon: "pulse-laser", size: (2, 1), build_points: 6, mass: 4, hp: 2, power: (draw: 8), weapon: (family: Energy, damage: 2, range: 50)),
            ]"#,
        ),
        (
            "ships/twin.ron",
            r#"(id: "twin_boat", name: "Twin boat", summary: "A test ship using [[part:twin_laser]].", faction: "ship_mind", size: (2, 2),
                parts: [(part: "twin_laser", x: 0, y: 0), (part: "armor_block", x: 0, y: 1, material: "steel"), (part: "logic_core", x: 1, y: 1)])"#,
        ),
    ]);
    let base_only = Content::load(&[base()]).unwrap();
    let c = Content::load(&[base(), m]).unwrap_or_else(|p| panic!("{}", show(&p)));
    let laser = c.parts.get("pulse_laser").unwrap();
    assert_eq!(laser.weapon.as_ref().unwrap().damage, 1.6);
    assert_eq!(laser.source.pack, "test_mod");
    assert_eq!(laser.source.file, "parts/lasers.ron");
    // Same position in load order as before (overrides keep their place), new entries go last.
    assert_eq!(
        c.parts.position("pulse_laser"),
        base_only.parts.position("pulse_laser")
    );
    assert_eq!(c.parts.position("twin_laser"), Some(base_only.parts.len()));
    assert_eq!(
        c.packs.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        ["base", "test_mod"]
    );
    // Base ships pick up the override; the capacitor bonus targets the new laser by tag.
    assert!(c.ships.contains("halvex_runner") && c.ships.contains("twin_boat"));
    assert_eq!(
        c.render_plain(&c.ships.get("twin_boat").unwrap().summary),
        "A test ship using Twin laser."
    );
}

#[test]
fn dependency_order_and_schema_version() {
    let m = pack(&[("pack.ron", MOD_MANIFEST)]);
    let err = Content::load(&[m.clone(), base()]).unwrap_err();
    assert!(
        err.iter()
            .any(|p| p.field.as_deref() == Some("depends") && p.message.contains("`base`")),
        "{}",
        show(&err)
    );
    let newer = pack(&[(
        "pack.ron",
        r#"(id: "future", name: "F", version: "1", schema_version: 99)"#,
    )]);
    let err = Content::load(&[base(), newer]).unwrap_err();
    assert!(
        err.iter()
            .any(|p| p.field.as_deref() == Some("schema_version"))
    );
    let err = Content::load(&[base(), base()]).unwrap_err();
    assert!(err.iter().any(|p| p.message.contains("loaded twice")));
}

#[test]
fn parse_errors_have_file_and_line() {
    let err = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        (
            "parts/x.ron",
            "[\n  (id: \"x\", name: \"X\", summmary: \"typo\"),\n]",
        ),
    ])
    .unwrap_err();
    let p = &err[0];
    assert_eq!(p.source.as_ref().unwrap().file, "parts/x.ron");
    assert_eq!(p.kind, Some(Kind::Part));
    assert_eq!(p.line.unwrap().0, 2);
    assert!(p.message.contains("summmary"), "{p}");
    let text = p.to_string();
    assert!(text.starts_with("error: test_mod:parts/x.ron:2:"), "{text}");
}

#[test]
fn layout_errors() {
    let err = Pack::from_files([("pack.ron", MOD_MANIFEST), ("weapons/x.ron", "[]")]).unwrap_err();
    assert!(err[0].message.contains("unknown content file location"));
    let err =
        Pack::from_files([("pack.ron", MOD_MANIFEST), ("wiki/parts/x.md", "hi")]).unwrap_err();
    assert!(err[0].message.contains("wiki/<kind>/<id>.md"));
    let err = Pack::from_files([("stats.ron", "[]")]).unwrap_err();
    assert!(err[0].message.contains("missing pack.ron"));
    // README and other non-RON files are ignored; `stats.ron` and `stats/*.ron` are both fine.
    pack(&[
        ("pack.ron", MOD_MANIFEST),
        ("README.md", "# hi"),
        ("stats/extra.ron", "[]"),
        ("categories.ron", "[]"),
    ]);
}

const ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="5" y="5" width="10" height="10" fill="#8cefdb"/></svg>"##;

#[test]
fn mods_add_and_override_icons() {
    let m = pack(&[
        ("pack.ron", MOD_MANIFEST),
        ("icons/pulse-laser.svg", ICON),
        ("icons/twin-laser.svg", ICON),
        ("icons/README.md", "Icons of the test mod."),
        (
            "parts/twin.ron",
            r#"(id: "twin_laser", name: "Twin laser", summary: "Two lasers.", tags: ["laser"], category: "weapons",
                icon: "twin-laser", size: (2, 1), build_points: 6, mass: 4, hp: 2, power: (draw: 8),
                weapon: (family: Energy, damage: 2, range: 50))"#,
        ),
    ]);
    let base_only = Content::load(&[base()]).unwrap();
    let c = Content::load(&[base(), m]).unwrap_or_else(|p| panic!("{}", show(&p)));
    assert_eq!(c.icon_svg("pulse-laser"), Some(ICON));
    assert_eq!(c.icons["pulse-laser"].source.pack, "test_mod");
    assert_eq!(c.icon_svg("twin-laser"), Some(ICON));
    assert_ne!(c.icon_svg("thruster"), None);
    assert_eq!(c.icons.len(), base_only.icons.len() + 1);
}

#[test]
fn icon_errors_and_warnings() {
    // A part whose icon no pack provides.
    let e = errors_with(
        "parts/p.ron",
        &format!("({PART_HEAD}, category: \"power\", mass: 1, hp: 1, icon: \"warp-core\")"),
    );
    assert_error(&e, "p", "icon", "no loaded pack has icons/warp-core.svg");
    // SVG files belong in icons/ and need a kebab-case key.
    for path in ["icons/Bad_Key.svg", "parts/x.svg", "icons/sub/x.svg"] {
        let err = Pack::from_files([("pack.ron", MOD_MANIFEST), (path, ICON)]).unwrap_err();
        assert!(
            err[0].message.contains("icons/<key>.svg"),
            "{path}: {}",
            err[0]
        );
    }
    // A broken SVG is an error at its file; an icon nothing uses is a warning.
    let m = pack(&[
        ("pack.ron", MOD_MANIFEST),
        (
            "icons/broken.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"><g></svg>",
        ),
        ("icons/spare.svg", ICON),
    ]);
    let problems = data::validate(&Content::merge(&[base(), m]));
    let broken = problems
        .iter()
        .find(|p| p.message.contains("icon `broken`"))
        .unwrap_or_else(|| panic!("{}", show(&problems)));
    assert!(
        broken.is_error() && broken.message.contains("closes <g>"),
        "{broken}"
    );
    assert_eq!(broken.source.as_ref().unwrap().file, "icons/broken.svg");
    let spare = problems
        .iter()
        .find(|p| p.message.contains("icon `spare`"))
        .unwrap();
    assert_eq!(spare.severity, Severity::Warning);
    assert!(spare.message.contains("not used"), "{spare}");
}

#[test]
fn power_allocation_fields() {
    let c = Content::load(&[base()]).unwrap();
    let beam = c.parts.get("beam_laser").unwrap().power.as_ref().unwrap();
    assert_eq!(beam.tier, data::PowerTier::Low);
    assert_eq!(beam.weight, 1.0);
    assert_eq!(beam.min_level, 0.5);
    let lines = c.part_stats(c.parts.get("beam_laser").unwrap(), None);
    let min = lines.iter().find(|l| l.stat == "power_min_level").unwrap();
    assert_eq!(min.text, "50%");
    // Defaults: normal tier, weight 1, no minimum.
    let gyro = c.parts.get("gyro").unwrap().power.as_ref().unwrap();
    assert_eq!(
        (gyro.tier, gyro.weight, gyro.min_level),
        (data::PowerTier::Normal, 1.0, 0.0)
    );

    let e = errors_with(
        "parts/p.ron",
        &format!(
            "({PART_HEAD}, category: \"power\", mass: 1, hp: 1, power: (draw: 2, weight: 0, min_level: 1.5))"
        ),
    );
    assert_error(&e, "p", "power.weight", "above 0");
    assert_error(&e, "p", "power.min_level", "0 to 1");
    let e = errors_with(
        "parts/p.ron",
        &format!(
            "({PART_HEAD}, category: \"power\", mass: 1, hp: 1, power: (output: 5, tier: High))"
        ),
    );
    assert_error(&e, "p", "power", "for parts that draw power");
    let e = errors_with(
        "ships/s.ron",
        &format!(
            "({SHIP_HEAD}, parts: [(part: \"logic_core\", x: 0, y: 0, power: (tier: Off, weight: 500)), (part: \"fission_reactor\", x: 1, y: 0, power: (tier: High))])"
        ),
    );
    assert_error(&e, "s", "parts[0].power.weight", "at most 100");
    assert_error(&e, "s", "parts[1].power", "draws no power");
    // A valid override.
    let m = pack(&[
        ("pack.ron", MOD_MANIFEST),
        (
            "ships/s.ron",
            r#"(id: "s", name: "S", summary: "S.", faction: "ship_mind", size: (2, 1), parts: [
                (part: "logic_core", x: 0, y: 0, power: (tier: High, weight: 2)), (part: "fission_reactor", x: 1, y: 0),
            ])"#,
        ),
    ]);
    let c = Content::load(&[base(), m]).unwrap_or_else(|p| panic!("{}", show(&p)));
    let o = c.ships.get("s").unwrap().parts[0].power.unwrap();
    assert_eq!((o.tier, o.weight), (Some(data::PowerTier::High), Some(2.0)));
}

#[test]
fn duplicate_ids_within_a_pack() {
    let part = r#"(id: "dup", name: "D", summary: "S.", category: "hull", size: (1, 1), build_points: 1, hull: true)"#;
    let err = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        ("parts/a.ron", part),
        ("parts/b.ron", part),
    ])
    .unwrap_err();
    assert_eq!(err[0].id.as_deref(), Some("dup"));
    assert_eq!(err[0].source.as_ref().unwrap().file, "parts/b.ron");
    assert!(err[0].message.contains("parts/a.ron"), "{}", err[0]);
}

const PART_HEAD: &str = r#"id: "p", name: "P", summary: "S.", size: (1, 1), build_points: 1"#;

#[test]
fn part_errors() {
    let e = errors_with(
        "parts/p.ron",
        &format!("({PART_HEAD}, category: \"nope\", mass: 1, hp: 1)"),
    );
    assert_error(&e, "p", "category", "unknown category `nope`");
    let e = errors_with(
        "parts/p.ron",
        &format!("({PART_HEAD}, category: \"power\")"),
    );
    assert_error(&e, "p", "mass", "need");
    assert_error(&e, "p", "hp", "need");
    let e = errors_with(
        "parts/p.ron",
        &format!("({PART_HEAD}, category: \"hull\", hull: true, mass: 3)"),
    );
    assert_error(&e, "p", "mass", "material");
    let e = errors_with(
        "parts/p.ron",
        r#"(id: "p", name: "P", summary: "S.", category: "hull", size: (2, 2), shape: Slope, build_points: 1, hull: true)"#,
    );
    assert_error(&e, "p", "size", "one cell high");
    let e = errors_with(
        "parts/p.ron",
        r#"(id: "p", name: "P", summary: "S.", category: "hull", size: (2, 1), shape: Slope, build_points: 1, mass: 1, hp: 1)"#,
    );
    assert_error(&e, "p", "shape", "hull");
    let e = errors_with(
        "parts/p.ron",
        &format!(
            "({PART_HEAD}, category: \"power\", mass: 1, hp: 1, bonuses: [(target: Tag(\"lazer\"), stat: \"speed\", effect: Add(1))])"
        ),
    );
    assert_error(&e, "p", "bonuses[0].target", "no part has the tag `lazer`");
    assert_error(&e, "p", "bonuses[0].stat", "unknown stat `speed`");
    let e = errors_with(
        "parts/p.ron",
        &format!(
            "({PART_HEAD}, category: \"power\", mass: 1, hp: 1, color: \"pink\", icon: \"Bad_Icon\")"
        ),
    );
    assert_error(&e, "p", "color", "unknown system colour key `pink`");
    assert_error(&e, "p", "icon", "kebab-case");
    let e = errors_with(
        "parts/p.ron",
        &format!(
            "({PART_HEAD}, category: \"power\", mass: 1, hp: 1, description: \"See [[part:nothing]].\")"
        ),
    );
    assert_error(&e, "p", "description", "points at nothing");
    let e = errors_with(
        "parts/p.ron",
        r#"(id: "Bad-Id", name: "", summary: "S.", category: "power", size: (1, 1), build_points: 1, mass: 1, hp: 1)"#,
    );
    assert_error(&e, "Bad-Id", "id", "snake_case");
    assert_error(&e, "Bad-Id", "name", "empty");
}

const SHIP_HEAD: &str = r#"id: "s", name: "S", summary: "S.", faction: "ship_mind", size: (3, 3)"#;

#[test]
fn ship_errors() {
    let e = errors_with(
        "ships/s.ron",
        &format!("({SHIP_HEAD}, parts: [(part: \"armor_block\", x: 0, y: 0)])"),
    );
    assert_error(&e, "s", "parts[0].material", "needs a material");
    let e = errors_with(
        "ships/s.ron",
        &format!("({SHIP_HEAD}, parts: [(part: \"thruster\", x: 0, y: 0, material: \"steel\")])"),
    );
    assert_error(&e, "s", "parts[0].material", "not a hull part");
    let e = errors_with(
        "ships/s.ron",
        &format!("({SHIP_HEAD}, parts: [(part: \"armor_block\", x: 0, y: 0, material: \"wood\")])"),
    );
    assert_error(&e, "s", "parts[0].material", "unknown material `wood`");
    let e = errors_with(
        "ships/s.ron",
        &format!("({SHIP_HEAD}, parts: [(part: \"fusion_reactor\", x: 2, y: 0)])"),
    );
    assert_error(&e, "s", "parts[0]", "does not fit the 3×3 frame");
    let e = errors_with(
        "ships/s.ron",
        &format!(
            "({SHIP_HEAD}, parts: [(part: \"fusion_reactor\", x: 0, y: 0), (part: \"logic_core\", x: 1, y: 1)])"
        ),
    );
    assert_error(&e, "s", "parts[1]", "overlaps parts[0]");
    let e = errors_with(
        "ships/s.ron",
        &format!("({SHIP_HEAD}, parts: [(part: \"logic_core\", x: 0, y: 0, rot: 4)])"),
    );
    assert_error(&e, "s", "parts[0].rot", "0 to 3");
    let e = errors_with(
        "ships/s.ron",
        &format!("({SHIP_HEAD}, parts: [(part: \"warp_core\", x: 0, y: 0)])"),
    );
    assert_error(&e, "s", "parts[0].part", "unknown part `warp_core`");
    let e = errors_with(
        "ships/s.ron",
        r#"(id: "s", name: "S", summary: "S.", faction: "pirates", size: (1, 1), parts: [(part: "logic_core", x: 0, y: 0)])"#,
    );
    assert_error(&e, "s", "faction", "unknown faction `pirates`");
}

#[test]
fn ship_warnings() {
    let m = pack(&[
        ("pack.ron", MOD_MANIFEST),
        (
            "ships/s.ron",
            r#"(id: "s", name: "S", summary: "S.", faction: "ship_mind", size: (3, 3), parts: [
                (part: "pulse_laser", x: 0, y: 1), (part: "logic_core", x: 0, y: 0),
                (part: "thruster", x: 2, y: 0), (part: "armor_block", x: 2, y: 1, material: "steel"),
            ])"#,
        ),
    ]);
    let c = Content::load(&[base(), m]).unwrap();
    let w: Vec<Problem> = data::validate(&c)
        .into_iter()
        .filter(|p| p.id.as_deref() == Some("s"))
        .collect();
    assert!(w.iter().all(|p| p.severity == Severity::Warning));
    assert!(
        w.iter().any(|p| p.message.contains("2 pieces")),
        "{}",
        show(&w)
    );
    assert!(
        w.iter()
            .any(|p| p.message.contains("no clear line of fire")),
        "{}",
        show(&w)
    );
    assert!(
        w.iter().any(|p| p.message.contains("exhaust blocked")),
        "{}",
        show(&w)
    );
}

#[test]
fn wiki_errors() {
    let e = errors_with("wiki/part/warp_core.md", "Nothing here.");
    assert!(e.iter().any(|p| p.kind == Some(Kind::Part)
        && p.id.as_deref() == Some("warp_core")
        && p.message.contains("does not exist")));
    let e = errors_with(
        "wiki/part/radiator.md",
        "Works with [[part:fission_reactor]] and [[thing:x]] and [[part:nope]].",
    );
    assert_eq!(e.len(), 2, "{}", show(&e));
    assert!(
        e.iter()
            .all(|p| p.source.as_ref().unwrap().file == "wiki/part/radiator.md")
    );
    let e = errors_with("wiki/page/Bad.md", "# Bad");
    assert!(e.iter().any(|p| p.message.contains("snake_case")));
}

#[test]
fn single_entry_and_list_files_and_comments() {
    let one = "// comment\n/* block */\n#![enable(implicit_some)]\n(id: \"x\", name: \"X\", summary: \"S.\", glow: \"enemy\")";
    let list = "// comment\n[(id: \"y\", name: \"Y\", summary: \"S.\", glow: \"player\")]";
    let m = pack(&[
        ("pack.ron", MOD_MANIFEST),
        ("factions/x.ron", one),
        ("factions/y.ron", list),
    ]);
    assert_eq!(m.factions.len(), 2);
    // Explicit Some(...) still works next to the implicit form.
    let m = pack(&[
        ("pack.ron", MOD_MANIFEST),
        (
            "categories.ron",
            r#"[(id: "c", name: "C", summary: "S.", color: Some("power"))]"#,
        ),
    ]);
    assert_eq!(m.categories[0].color.as_deref(), Some("power"));
}

#[test]
fn control_cost_defaults_base_numbers_and_validation() {
    let c = content();
    let cost = |id: &str| c.parts.get(id).unwrap().control_cost();
    // Defaults by kind: hull and cores 0, parts that draw or make power 1, passive parts 0; big machines 2 in
    // content (lead placeholders).
    for (id, want) in [
        ("armor_block", 0),
        ("slope_1x3", 0),
        ("logic_core", 0),
        ("automaton_core", 0),
        ("radiator", 0),
        ("capacitor", 0),
        ("cargo_hold", 0),
        ("fission_reactor", 1),
        ("pulse_laser", 1),
        ("shield_emitter", 1),
        ("thruster", 1),
        ("gyro", 1),
        ("rcs_pod", 1),
        ("automaton_cannon", 1),
        ("fusion_reactor", 2),
        ("torch_drive", 2),
        ("beam_laser", 2),
    ] {
        assert_eq!(cost(id), want, "{id}");
    }
    // Every base ship runs all its parts with headroom.
    for (ship, load, capacity) in [
        ("halvex_runner", 18, 32),
        ("automaton_raider", 2, 4),
        ("sample_early", 8, 32),
        ("sample_mid", 16, 32),
    ] {
        assert_eq!(
            c.ship_control(c.ships.get(ship).unwrap()),
            (load, capacity),
            "{ship}"
        );
    }
    let lines = c.part_stats(c.parts.get("torch_drive").unwrap(), None);
    assert!(
        lines
            .iter()
            .any(|l| l.stat == "control_cost" && l.value == 2.0),
        "{lines:?}"
    );
    let lines = c.part_stats(c.parts.get("logic_core").unwrap(), None);
    assert!(
        lines
            .iter()
            .any(|l| l.stat == "control_capacity" && l.value == 16.0),
        "{lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.stat == "control_cost"),
        "cores cost nothing"
    );

    let e = errors_with(
        "parts/p.ron",
        &format!(
            "({PART_HEAD}, category: \"power\", mass: 1, hp: 1, control_cost: 101, power: (draw: 1))"
        ),
    );
    assert_error(&e, "p", "control_cost", "at most 100");
    // Warnings: a passive part with a cost, a ship over its cores' capacity.
    let m = pack(&[
        ("pack.ron", MOD_MANIFEST),
        (
            "parts/p.ron",
            &format!("({PART_HEAD}, category: \"power\", mass: 1, hp: 1, control_cost: 20)"),
        ),
        (
            "ships/s.ron",
            r#"(id: "s", name: "S", summary: "S.", faction: "ship_mind", size: (2, 1), parts: [
                (part: "logic_core", x: 0, y: 0), (part: "p", x: 1, y: 0),
            ])"#,
        ),
    ]);
    let c = Content::load(&[base(), m]).unwrap();
    let w: Vec<Problem> = data::validate(&c)
        .into_iter()
        .filter(|p| p.severity == Severity::Warning)
        .collect();
    assert!(
        w.iter()
            .any(|p| p.id.as_deref() == Some("p") && p.message.contains("never go idle")),
        "{}",
        show(&w)
    );
    assert!(
        w.iter()
            .any(|p| p.id.as_deref() == Some("s") && p.message.contains("control load 20 exceeds")),
        "{}",
        show(&w)
    );
}
