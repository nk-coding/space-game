mod common;

use std::collections::BTreeSet;

use common::*;
use data::{Kind, Severity};

#[test]
fn embedded_base_pack_loads_and_validates() {
    let c = content();
    assert_eq!(c.packs.len(), 1);
    assert_eq!(c.packs[0].id, "base");
    assert!(
        c.wiki.is_empty(),
        "the game build must not embed wiki prose"
    );
    for id in ["steel", "composite", "reactive", "automaton_plate"] {
        assert!(c.materials.get(id).unwrap().armor.is_some(), "{id}");
    }
    let parts: Vec<&str> = c.parts.iter().map(|p| p.id.as_str()).collect();
    for id in [
        "armor_block",
        "slope_1x1",
        "slope_1x2",
        "slope_1x3",
        "fission_reactor",
        "fusion_reactor",
        "capacitor",
        "pulse_laser",
        "beam_laser",
        "shield_emitter",
        "radiator",
        "thruster",
        "torch_drive",
        "logic_core",
        "cargo_bay",
        "cargo_hold",
        "automaton_core",
        "automaton_cannon",
        "gyro",
        "rcs_pod",
    ] {
        assert!(parts.contains(&id), "missing part {id}");
    }
    for id in [
        "halvex_runner",
        "automaton_raider",
        "sample_early",
        "sample_mid",
    ] {
        assert!(c.ships.contains(id), "missing ship {id}");
    }
}

#[test]
fn disk_pack_with_wiki_validates() {
    let c =
        data::Content::load(&[base_pack_with_wiki()]).unwrap_or_else(|p| panic!("{}", show(&p)));
    assert!(c.wiki_page(Kind::Part, "fission_reactor").is_some());
    assert_eq!(
        c.name_of(Kind::Page, "drydock_rules"),
        Some("Drydock rules")
    );
}

#[test]
fn icon_keys_exist_in_art() {
    let icons: BTreeSet<String> = std::fs::read_dir(repo_root().join("art/icons"))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .split('.')
                .next()
                .unwrap()
                .to_string()
        })
        .collect();
    let opts = data::ValidateOptions {
        icon_keys: Some(icons),
    };
    let problems = data::validate_with(&content(), &opts);
    let errors: Vec<_> = problems.iter().filter(|p| p.is_error()).cloned().collect();
    assert!(errors.is_empty(), "{}", show(&errors));
    // Every non-hull part has an icon.
    for p in content().parts.iter().filter(|p| !p.hull) {
        assert!(p.icon.is_some(), "{} has no icon", p.id);
    }
}

#[test]
fn base_warnings_are_known() {
    let problems = data::validate(&content());
    let warnings: Vec<String> = problems
        .iter()
        .filter(|p| p.severity == Severity::Warning)
        .map(|p| p.to_string())
        .collect();
    // The original Halvex Runner layout: wing-tip pulse lasers behind 1×1 slopes, all six radiators boxed in.
    // Fixed in the runner layout commit (Niklas, 11 Oct 2026).
    assert_eq!(warnings.len(), 8, "{warnings:#?}");
    assert!(
        warnings.iter().all(|w| w.contains("halvex_runner")),
        "{warnings:#?}"
    );
}

#[test]
fn stat_lines_follow_stats_ron() {
    let c = content();
    let reactor = c.parts.get("fission_reactor").unwrap();
    let lines = c.part_stats(reactor, None);
    let ids: Vec<&str> = lines.iter().map(|l| l.stat.as_str()).collect();
    assert_eq!(
        ids,
        [
            "build_points",
            "mass",
            "hp",
            "power_output",
            "heat_output",
            "heat_limit"
        ]
    );
    assert_eq!(lines[3].text, "10 MW");
    assert_eq!(lines[3].label, "Power output");

    let cap = c.parts.get("capacitor").unwrap();
    let bonus = c
        .part_stats(cap, None)
        .into_iter()
        .find(|l| l.bonus.is_some())
        .unwrap();
    assert_eq!(bonus.stat, "fire_rate");
    assert_eq!(bonus.text, "+15%");
    assert_eq!(bonus.bonus.as_ref().unwrap().max_stacks, Some(3));

    let rad = c.parts.get("radiator").unwrap();
    let bonus = c
        .part_stats(rad, None)
        .into_iter()
        .find(|l| l.bonus.is_some())
        .unwrap();
    assert_eq!(bonus.text, "+2 MW");

    // Hull parts: mass and HP from the material, slopes count half.
    let slope = c.parts.get("slope_1x2").unwrap();
    let lines = c.part_stats(slope, c.materials.get("composite"));
    let get = |id: &str| lines.iter().find(|l| l.stat == id).unwrap().value;
    assert_eq!(get("mass"), 1.5);
    assert_eq!(get("hp"), 1.4);
    assert_eq!(get("build_points"), 1.5);
    assert!(c.part_stats(slope, None).iter().all(|l| l.stat != "mass"));

    let steel = c.material_stats(c.materials.get("steel").unwrap());
    assert_eq!(
        steel.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
        ["2 t", "1 HP", "1×"]
    );
}

#[test]
fn text_keys_and_plain_rendering() {
    let c = content();
    let texts = c.texts();
    assert_eq!(
        texts.get("part.fission_reactor.name").map(String::as_str),
        Some("Fission reactor")
    );
    assert_eq!(texts.get("stat.mass.unit").map(String::as_str), Some("t"));
    assert!(texts.contains_key("ship.halvex_runner.summary"));
    let desc = c.parts.get("capacitor").unwrap();
    assert!(!desc.summary.contains("[["));
    let logic = c
        .parts
        .get("logic_core")
        .unwrap()
        .description
        .clone()
        .unwrap();
    assert!(c.render_plain(&logic).contains("salvaged automaton cores"));
    assert_eq!(c.render_plain("x [[part:radiator]] y"), "x Radiator y");

    let mut c2 = c.clone();
    let mut tr = std::collections::BTreeMap::new();
    tr.insert("part.radiator.name".to_string(), "Kühler".to_string());
    tr.insert("part.nope.name".to_string(), "?".to_string());
    let unused = c2.apply_texts(&tr);
    assert_eq!(unused, ["part.nope.name"]);
    assert_eq!(c2.parts.get("radiator").unwrap().name, "Kühler");
}

#[test]
fn part_colors() {
    let c = content();
    let color = |id: &str| c.part_color(c.parts.get(id).unwrap()).map(str::to_string);
    assert_eq!(color("fission_reactor").as_deref(), Some("power"));
    assert_eq!(color("shield_emitter").as_deref(), Some("shields"));
    assert_eq!(color("automaton_core").as_deref(), Some("enemy"));
    assert_eq!(color("armor_block"), None);
}

#[test]
fn placement_mass_and_centroid() {
    let c = content();
    let ship = c.ships.get("halvex_runner").unwrap();
    let grid = c.ship_grid(ship).unwrap();
    let mass: f32 = ship
        .parts
        .iter()
        .map(|p| c.placement_mass(p).unwrap())
        .sum();
    assert_eq!(mass, 103.0);
    // slope_1x2 at (3,0) flipped: corners (5,0), (5,1), (3,1).
    let i = ship
        .parts
        .iter()
        .position(|p| p.part == "slope_1x2" && p.x == 3)
        .unwrap();
    assert_eq!(c.placement_mass(&ship.parts[i]), Some(1.5));
    assert_eq!(c.placement_hp(&ship.parts[i]), Some(1.4));
    let [x, y] = grid.shape(i).centroid();
    assert!((x - 13.0 / 3.0).abs() < 1e-6 && (y - 2.0 / 3.0).abs() < 1e-6);
    let j = ship
        .parts
        .iter()
        .position(|p| p.part == "fusion_reactor")
        .unwrap();
    assert_eq!(grid.shape(j).centroid(), [6.0, 3.0]);
    assert_eq!(c.placement_mass(&ship.parts[j]), Some(12.0));
}
