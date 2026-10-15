mod common;

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
        "rim_skiff",
    ] {
        assert!(c.ships.contains(id), "missing ship {id}");
    }
}

/// The embed is not stale: it holds exactly the pack's `.ron` and `.svg` files outside `wiki/`, as on disk
/// (see `build.rs`; a moved file keeps its old mtime).
#[test]
fn embedded_pack_matches_disk() {
    let mut disk: Vec<(String, String)> = read_dir_files(&repo_root().join("content/base"))
        .into_iter()
        .filter(|(path, _)| {
            !path.starts_with("wiki/")
                && !path.split('/').any(|c| c.starts_with('.'))
                && (path.ends_with(".ron") || path.ends_with(".svg"))
        })
        .collect();
    disk.sort();
    let embedded: Vec<(String, String)> = data::base_pack_files()
        .iter()
        .map(|(p, s)| (p.to_string(), s.to_string()))
        .collect();
    let names = |v: &[(String, String)]| v.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>();
    assert_eq!(
        names(&embedded),
        names(&disk),
        "stale embed: touch crates/core/data/build.rs"
    );
    for ((path, a), (_, b)) in embedded.iter().zip(&disk) {
        assert!(a == b, "stale embed of {path}: touch crates/core/data/build.rs");
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
fn icons_are_embedded_with_the_pack() {
    let c = content();
    // Every non-hull part and every material has an icon, served from the pack as SVG text.
    for p in c.parts.iter().filter(|p| !p.hull) {
        let key = p
            .icon
            .as_deref()
            .unwrap_or_else(|| panic!("{} has no icon", p.id));
        let svg = c
            .icon_svg(key)
            .unwrap_or_else(|| panic!("no icons/{key}.svg"));
        assert_eq!(data::content::svg::check_svg(svg), Ok(()), "{key}");
    }
    for m in c.materials.iter() {
        assert!(c.icon_svg(m.icon.as_deref().unwrap()).is_some(), "{}", m.id);
    }
    let icon = &c.icons["pulse-laser"];
    assert_eq!(icon.source.file, "icons/pulse-laser.svg");
    assert!(
        data::base_pack_files()
            .iter()
            .any(|(path, _)| *path == "icons/pulse-laser.svg")
    );
    // The disk pack (tools) loads the same icons.
    assert_eq!(
        data::Content::merge(&[base_pack_with_wiki()]).icons.len(),
        c.icons.len()
    );
}

#[test]
fn base_warnings_are_known() {
    let problems = data::validate(&content());
    let warnings: Vec<String> = problems
        .iter()
        .filter(|p| p.severity == Severity::Warning)
        .map(|p| p.to_string())
        .collect();
    // Every base ship works under the drydock rules: guns can fire, engines and RCS have open space,
    // radiators vent, ships are one piece.
    assert!(warnings.is_empty(), "{warnings:#?}");
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
            "control_cost",
            "mass",
            "hp",
            "power_output",
            "heat_output",
            "heat_limit",
            "blast_damage",
            "blast_radius"
        ]
    );
    assert_eq!(lines[7].text, "1.5 HP");
    assert_eq!(lines[8].text, "1.5 m");
    assert_eq!(lines[1].text, "1");
    assert_eq!(lines[1].label, "Control cost");
    assert_eq!(lines[4].text, "10 MW");
    assert_eq!(lines[4].label, "Power output");

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
        ["2 t", "1 HP", "1×", "0.5×"]
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
    assert!(c.render_plain(&logic).contains("Salvaged cores give programmable matter"));
    assert_eq!(c.render_plain("x [[part:radiator]] y"), "x Radiator y");
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
    assert_eq!(mass, 106.5);
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

/// The content's fingerprint (GDR-0092) is the same for equal content, changes with what a fight reads (a part's
/// mass) and not with what it doesn't (a research node's name, wiki prose).
#[test]
fn the_fingerprint_follows_what_fights_read() {
    let c = content();
    let base = c.fingerprint();
    assert_eq!(content().fingerprint(), base);
    let mut heavier = c.clone();
    let part = heavier.parts.iter_mut().find(|p| p.mass.is_some()).unwrap();
    part.mass = part.mass.map(|m| m + 0.5);
    assert_ne!(heavier.fingerprint(), base);
    let mut renamed = c.clone();
    renamed.research.iter_mut().next().unwrap().name.push_str(" II");
    renamed.wiki.clear();
    assert_eq!(renamed.fingerprint(), base);
}
