//! Drydock plumbing in `data`: checking a design on its own (`validate_ship`), the blueprint text format
//! (`blueprint_to_ron`, `parse_blueprint`), drydock frames as content, per-ship totals (`ship_totals`) and the
//! ship-level stat descriptors (`SHIP_STATS`).

mod common;

use common::*;
use data::limits::MAX_COORD;
use data::stats::{BUILTIN_STATS, SHIP_STATS};
use data::{
    Better, Blueprint, Content, Kind, Pack, Placement, PowerOverride, PowerTier, Problem, Severity,
    blueprint_to_ron, parse_blueprint, validate, validate_ship,
};

const MOD_MANIFEST: &str =
    r#"(id: "test_mod", name: "Test mod", version: "1.0.0", schema_version: 1, depends: ["base"])"#;

fn design(parts: Vec<Placement>) -> Blueprint {
    Blueprint {
        id: "my_design".into(),
        name: "My design".into(),
        summary: "A design from the drydock.".into(),
        description: None,
        lore: None,
        tags: Vec::new(),
        faction: "ship_mind".into(),
        size: (4, 4),
        parts,
        behavior: None,
        source: Default::default(),
    }
}

fn at(part: &str, x: i32, y: i32) -> Placement {
    Placement {
        part: part.into(),
        x,
        y,
        rot: 0,
        flip: false,
        material: None,
        power: None,
    }
}

fn hull(part: &str, x: i32, y: i32, material: &str) -> Placement {
    Placement {
        material: Some(material.into()),
        ..at(part, x, y)
    }
}

/// `(severity, field, message)` of every problem, for comparisons.
fn summary(problems: &[Problem]) -> Vec<(Severity, String, String)> {
    problems
        .iter()
        .map(|p| {
            (
                p.severity,
                p.field.clone().unwrap_or_default(),
                p.message.clone(),
            )
        })
        .collect()
}

fn has(problems: &[Problem], severity: Severity, field: &str, needle: &str) -> bool {
    problems.iter().any(|p| {
        p.severity == severity && p.field.as_deref() == Some(field) && p.message.contains(needle)
    })
}

#[test]
fn validate_ship_runs_the_same_checks_as_content_validation() {
    // Content ships with header errors, placement errors, warnings only, and no parts at all.
    let m = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        (
            "ships/bad.ron",
            r#"[
                (id: "bad", name: "", summary: "See [[part:nothing]].", tags: ["Bad Tag"], faction: "pirates",
                 size: (3, 3), behavior: "nope", parts: [
                    (part: "armor_block", x: 0, y: 0), (part: "logic_core", x: 0, y: 0, rot: 7),
                    (part: "warp_core", x: 1, y: 1), (part: "fusion_reactor", x: 2, y: 2),
                    (part: "fission_reactor", x: 1, y: 0, material: "steel", power: (weight: 0)),
                ]),
                (id: "warned", name: "Warned", summary: "Two pieces and a blocked gun.", faction: "ship_mind",
                 size: (3, 3), parts: [
                    (part: "pulse_laser", x: 0, y: 1), (part: "logic_core", x: 0, y: 0),
                    (part: "radiator", x: 2, y: 0), (part: "armor_block", x: 2, y: 1, material: "steel"),
                    (part: "thruster", x: 2, y: 2, power: ()),
                ]),
                (id: "empty", name: "Empty", summary: "No parts.", faction: "ship_mind", size: (0, 2), parts: []),
            ]"#,
        ),
    ])
    .unwrap_or_else(|p| panic!("{}", show(&p)));
    let c = Content::merge(&[data::base_pack().unwrap(), m]);
    let all = validate(&c);
    for ship in c.ships.iter() {
        let from_content: Vec<Problem> = all
            .iter()
            .filter(|p| p.kind == Some(Kind::Ship) && p.id.as_deref() == Some(ship.id.as_str()))
            .cloned()
            .collect();
        assert_eq!(validate_ship(&c, ship), from_content, "{}", ship.id);
    }
    let bad = validate_ship(&c, c.ships.get("bad").unwrap());
    for (field, needle) in [
        ("name", "must not be empty"),
        ("summary", "points at nothing"),
        ("tags[0]", "snake_case"),
        ("faction", "unknown faction `pirates`"),
        ("behavior", "unknown behavior `nope`"),
        ("parts[0].material", "needs a material"),
        ("parts[1]", "overlaps parts[0]"),
        ("parts[1].rot", "0 to 3"),
        ("parts[2].part", "unknown part `warp_core`"),
        ("parts[3]", "does not fit the 3×3 frame"),
        ("parts[4].material", "not a hull part"),
        ("parts[4].power", "draws no power"),
        ("parts[4].power.weight", "above 0"),
    ] {
        assert!(
            has(&bad, Severity::Error, field, needle),
            "{field}: {needle}\n{}",
            show(&bad)
        );
    }
    // Problems of a pack ship point at its file.
    assert!(
        bad.iter()
            .all(|p| p.source.as_ref().unwrap().file == "ships/bad.ron")
    );
    let warned = validate_ship(&c, c.ships.get("warned").unwrap());
    assert!(
        warned.iter().all(|p| p.severity == Severity::Warning),
        "{}",
        show(&warned)
    );
    for (field, needle) in [
        ("parts", "2 pieces"),
        ("parts[0]", "no clear line of fire"),
        ("parts[4].power", "changes nothing"),
    ] {
        assert!(
            has(&warned, Severity::Warning, field, needle),
            "{field}: {needle}\n{}",
            show(&warned)
        );
    }
    let empty = validate_ship(&c, c.ships.get("empty").unwrap());
    assert!(
        has(&empty, Severity::Error, "size", "at least 1×1"),
        "{}",
        show(&empty)
    );
    assert!(
        has(&empty, Severity::Error, "parts", "at least one part"),
        "{}",
        show(&empty)
    );
}

#[test]
fn a_design_outside_the_content_is_checked_with_field_paths() {
    let c = content();
    // A copy of a content ship under a new id is a clean design.
    let mut copy = c.ships.get("halvex_runner").unwrap().clone();
    copy.id = "my_runner".into();
    copy.source = Default::default();
    assert_eq!(validate_ship(&c, &copy), []);

    let mut d = design(vec![
        at("armor_block", 0, 0),
        Placement {
            rot: 5,
            ..at("logic_core", 1, 0)
        },
        at("fusion_reactor", 3, 3),
        at("thruster", 1, 0),
        hull("slope_1x1", 2, 0, "wood"),
    ]);
    d.id = "My Design".into();
    d.summary = " ".into();
    d.description = Some("Built from [[part:nothing]].".into());
    let p = validate_ship(&c, &d);
    assert!(
        p.iter().all(|p| p.kind == Some(Kind::Ship)
            && p.id.as_deref() == Some("My Design")
            && p.source.is_none()
            && p.is_error()),
        "{}",
        show(&p)
    );
    assert_eq!(
        summary(&p)
            .into_iter()
            .map(|(_, f, _)| f)
            .collect::<Vec<_>>(),
        [
            "id",
            "summary",
            "description",
            "parts[0].material",
            "parts[1].rot",
            "parts[2]",
            "parts[3]",
            "parts[4].material",
        ],
        "{}",
        show(&p)
    );
    assert!(
        p[5].message.contains("does not fit the 4×4 frame"),
        "{}",
        p[5]
    );
    assert!(p[6].message.contains("overlaps parts[1]"), "{}", p[6]);
    // Without a pack source the message names kind, id and field only.
    assert_eq!(
        p[3].to_string(),
        "error: ship `My Design`, field `parts[0].material`: hull part `armor_block` needs a material"
    );
}

#[test]
fn placement_errors_hold_back_the_ship_wide_checks() {
    let c = content();
    // A gun with a part in front of it, and two pieces: warnings once the placements are clean.
    let clean = design(vec![
        at("logic_core", 0, 1),
        at("pulse_laser", 1, 1),
        hull("armor_block", 1, 0, "steel"),
        at("thruster", 3, 3),
    ]);
    let w = validate_ship(&c, &clean);
    assert!(
        has(&w, Severity::Warning, "parts[1]", "no clear line of fire")
            && has(&w, Severity::Warning, "parts", "2 pieces"),
        "{}",
        show(&w)
    );
    assert!(
        w.iter().all(|p| p.severity == Severity::Warning),
        "{}",
        show(&w)
    );
    // One placement error (an unknown material) and the ship-wide checks report nothing.
    let mut broken = clean.clone();
    broken.parts[2].material = Some("wood".into());
    assert_eq!(
        summary(&validate_ship(&c, &broken)),
        [(
            Severity::Error,
            "parts[2].material".to_string(),
            "unknown material `wood`".to_string()
        )]
    );
    // Header errors do not hold them back.
    let mut unnamed = clean;
    unnamed.name = String::new();
    let p = validate_ship(&c, &unnamed);
    assert!(
        has(&p, Severity::Error, "name", "must not be empty"),
        "{}",
        show(&p)
    );
    assert!(
        has(&p, Severity::Warning, "parts", "2 pieces"),
        "{}",
        show(&p)
    );
}

/// Writes, reads back and writes again: the blueprint and the text must come back unchanged.
fn round_trip(bp: &Blueprint) -> String {
    let text = blueprint_to_ron(bp);
    let back = parse_blueprint(&text).unwrap_or_else(|p| panic!("{}\n{text}", show(&p)));
    assert_eq!(&back, bp, "{text}");
    assert_eq!(blueprint_to_ron(&back), text);
    text
}

#[test]
fn base_ship_files_are_written_back_exactly() {
    let c = content();
    let files: Vec<_> = data::base_pack_files()
        .iter()
        .filter(|(path, _)| path.starts_with("ships/"))
        .collect();
    assert_eq!(files.len(), c.ships.len());
    for (path, text) in files {
        // The file without its leading comment lines is exactly what the writer produces.
        let body: String = text
            .lines()
            .skip_while(|l| l.starts_with("//"))
            .map(|l| format!("{l}\n"))
            .collect();
        let ship = c.ships.iter().find(|s| s.source.file == *path).unwrap();
        let mut bare = ship.clone();
        bare.source = Default::default();
        assert_eq!(blueprint_to_ron(ship), body, "{path}");
        assert_eq!(round_trip(&bare), body, "{path}");
        // Comments are fine when reading.
        assert_eq!(parse_blueprint(text).unwrap(), bare, "{path}");
    }
}

#[test]
fn blueprint_text_format_and_defaults() {
    let mut bp = design(vec![
        hull("armor_block", 0, 0, "steel"),
        at("logic_core", 1, 0),
        Placement {
            rot: 3,
            flip: true,
            power: Some(PowerOverride {
                tier: Some(PowerTier::High),
                weight: Some(2.0),
            }),
            ..at("thruster", 2, 0)
        },
    ]);
    // Only what differs from the defaults is written; `source` never is.
    assert_eq!(
        round_trip(&bp),
        r#"(
    id: "my_design",
    name: "My design",
    summary: "A design from the drydock.",
    faction: "ship_mind",
    size: (4, 4),
    parts: [
        (part: "armor_block", x: 0, y: 0, material: "steel"),
        (part: "logic_core", x: 1, y: 0),
        (part: "thruster", x: 2, y: 0, rot: 3, flip: true, power: (tier: High, weight: 2)),
    ],
)
"#
    );
    bp.description = Some("Long text.".into());
    bp.lore = Some("Old text.".into());
    bp.tags = vec!["fast".into(), "cheap".into()];
    bp.behavior = Some("skirmisher".into());
    bp.parts.clear();
    assert_eq!(
        round_trip(&bp),
        r#"(
    id: "my_design",
    name: "My design",
    summary: "A design from the drydock.",
    description: "Long text.",
    lore: "Old text.",
    tags: ["fast", "cheap"],
    faction: "ship_mind",
    behavior: "skirmisher",
    size: (4, 4),
    parts: [],
)
"#
    );
    // A blueprint read from a pack writes the same text: `source` is not part of it.
    let mut loaded = bp.clone();
    loaded.source = data::Source {
        pack: "base".into(),
        file: "ships/x.ron".into(),
    };
    assert_eq!(blueprint_to_ron(&loaded), blueprint_to_ron(&bp));
}

#[test]
fn every_rotation_flip_and_power_override_round_trips() {
    let mut parts = Vec::new();
    for rot in [0, 1, 2, 3, 7, 255] {
        for flip in [false, true] {
            parts.push(Placement {
                rot,
                flip,
                ..hull("slope_1x2", rot as i32, -(flip as i32), "composite")
            });
        }
    }
    let tiers = PowerTier::ALL.into_iter().map(Some).chain([None]);
    for tier in tiers {
        for weight in [
            None,
            Some(1.0),
            Some(0.1),
            Some(1.0 / 3.0),
            Some(100.0),
            Some(f32::MAX),
            Some(f32::MIN_POSITIVE),
            Some(1e-45),
            Some(-0.0),
            Some(f32::INFINITY),
            Some(f32::NEG_INFINITY),
        ] {
            parts.push(Placement {
                power: Some(PowerOverride { tier, weight }),
                ..at("shield_emitter", -MAX_COORD, MAX_COORD)
            });
        }
    }
    let mut bp = design(parts);
    bp.size = (u32::MAX, 0);
    let text = round_trip(&bp);
    assert!(text.contains("power: ()"), "{text}");
    assert!(text.contains("power: (tier: Critical)"), "{text}");
    assert!(text.contains("power: (weight: 0.1)"), "{text}");
    assert!(text.contains("rot: 255, flip: true"), "{text}");
    // NaN is not equal to itself, so it is checked on its own.
    let nan = design(vec![Placement {
        power: Some(PowerOverride {
            tier: None,
            weight: Some(f32::NAN),
        }),
        ..at("shield_emitter", 0, 0)
    }]);
    let back = parse_blueprint(&blueprint_to_ron(&nan)).unwrap();
    assert!(back.parts[0].power.unwrap().weight.unwrap().is_nan());
}

#[test]
fn awkward_strings_round_trip() {
    let awkward = [
        "say \"hi\"",
        r"back\slash \n \u{41} \\",
        "two\nlines\r\nand\ttabs",
        "nul \0 bell \u{7} del \u{7f} c1 \u{9f}",
        "Kühler · 90° × 2 — 船 🚀 \u{2028} \u{200f}",
        "// not a comment /* nor this */ #![enable(implicit_some)]",
        "\"), parts: [], x: (",
        "",
        " ",
    ];
    for s in awkward {
        let mut bp = design(vec![Placement {
            material: Some(s.into()),
            ..at(s, 0, 0)
        }]);
        bp.id = s.into();
        bp.name = s.into();
        bp.summary = s.into();
        bp.description = Some(s.into());
        bp.lore = Some(s.into());
        bp.tags = vec![s.into(), s.into()];
        bp.faction = s.into();
        bp.behavior = Some(s.into());
        let text = round_trip(&bp);
        // One field per line: line breaks inside strings are escaped.
        assert_eq!(text.lines().count(), 14, "{text}");
    }
}

#[test]
fn parse_errors_name_line_and_column() {
    let text = blueprint_to_ron(&design(vec![at("logic_core", 0, 0)]));
    // Unknown fields, `source` included (it is never read from text).
    for (extra, needle) in [
        ("colour: \"red\",", "colour"),
        ("source: (pack: \"base\", file: \"x.ron\"),", "source"),
    ] {
        let bad = text.replacen("    size:", &format!("    {extra}\n    size:"), 1);
        let p = parse_blueprint(&bad).unwrap_err();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].kind, Some(Kind::Ship));
        assert_eq!(p[0].line.map(|(l, _)| l), Some(6), "{}", p[0]);
        assert!(p[0].message.contains(needle), "{}", p[0]);
        assert!(p[0].is_error() && p[0].source.is_none());
    }
    let p = parse_blueprint(&text.replace("    faction: \"ship_mind\",\n", "")).unwrap_err();
    assert!(p[0].message.contains("faction"), "{}", p[0]);
    // One blueprint, not a list.
    assert!(parse_blueprint(&format!("[{text}]")).is_err());
    // Explicit `Some(..)` and comments still read.
    let explicit = text.replace(
        "x: 0, y: 0)",
        "x: 0, y: 0, material: Some(\"steel\")) // note",
    );
    assert_eq!(
        parse_blueprint(&explicit).unwrap().parts[0]
            .material
            .as_deref(),
        Some("steel")
    );
}

#[test]
fn base_frames_load_in_order() {
    let c = content();
    let frames: Vec<(&str, (u32, u32))> =
        c.frames.iter().map(|f| (f.id.as_str(), f.size)).collect();
    assert_eq!(
        frames,
        [
            ("frigate", (24, 24)),
            ("cruiser", (40, 40)),
            ("capital", (64, 64))
        ]
    );
    assert_eq!(c.frames.get("frigate").unwrap().source.file, "frames.ron");
    // Every base ship fits the smallest frame.
    let (w, h) = c.frames.get("frigate").unwrap().size;
    assert!(c.ships.iter().all(|s| s.size.0 <= w && s.size.1 <= h));
    // A kind like any other: text keys, names, links.
    assert_eq!(Kind::from_dir("frames"), Some(Kind::Frame));
    assert_eq!(Kind::from_key("frame"), Some(Kind::Frame));
    assert_eq!(
        c.texts().get("frame.capital.name").map(String::as_str),
        Some("Capital")
    );
    assert!(c.exists(Kind::Frame, "cruiser"));
    assert_eq!(c.name_of(Kind::Frame, "cruiser"), Some("Cruiser"));
    assert_eq!(
        c.render_plain("A [[frame:frigate]] hull."),
        "A Frigate hull."
    );
}

#[test]
fn mods_add_and_override_frames() {
    let m = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        (
            "frames/big.ron",
            r#"[
                (id: "frigate", name: "Frigate", summary: "A roomier frigate frame.", size: (30, 20)),
                (id: "dreadnought", name: "Dreadnought", summary: "Beyond the [[frame:capital]].", size: (128, 96)),
            ]"#,
        ),
        ("wiki/frame/dreadnought.md", "# Dreadnought\n\nSee [[frame:frigate]]."),
    ])
    .unwrap_or_else(|p| panic!("{}", show(&p)));
    let c =
        Content::load(&[data::base_pack().unwrap(), m]).unwrap_or_else(|p| panic!("{}", show(&p)));
    let ids: Vec<&str> = c.frames.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, ["frigate", "cruiser", "capital", "dreadnought"]);
    let frigate = c.frames.get("frigate").unwrap();
    assert_eq!(
        (frigate.size, frigate.source.pack.as_str()),
        ((30, 20), "test_mod")
    );
    assert!(c.wiki_page(Kind::Frame, "dreadnought").is_some());
}

#[test]
fn frame_errors() {
    let m = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        (
            "frames.ron",
            r#"[
                (id: "flat", name: "Flat", summary: "No height.", size: (10, 0)),
                (id: "huge", name: "Huge", summary: "Too big.", size: (129, 10)),
                (id: "Bad", name: " ", summary: "See [[frame:nothing]].", tags: ["x y"], size: (5, 5)),
            ]"#,
        ),
    ])
    .unwrap();
    let e = Content::load(&[data::base_pack().unwrap(), m]).unwrap_err();
    let frame = |id: &str, field: &str, needle: &str| {
        assert!(
            e.iter().any(|p| p.is_error()
                && p.kind == Some(Kind::Frame)
                && p.id.as_deref() == Some(id)
                && p.field.as_deref() == Some(field)
                && p.message.contains(needle)),
            "{id} {field} {needle}\n{}",
            show(&e)
        )
    };
    frame("flat", "size", "1 to 128 cells; got 10×0");
    frame("huge", "size", "got 129×10");
    frame("Bad", "id", "snake_case");
    frame("Bad", "name", "must not be empty");
    frame("Bad", "summary", "points at nothing");
    frame("Bad", "tags[0]", "snake_case");
    // Unknown fields fail to parse; ids are unique within a pack.
    let e = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        (
            "frames.ron",
            r#"(id: "x", name: "X", summary: "S.", size: (1, 1), grid: 2)"#,
        ),
    ])
    .unwrap_err();
    assert!(
        e[0].kind == Some(Kind::Frame) && e[0].message.contains("grid"),
        "{}",
        e[0]
    );
    let one = r#"(id: "x", name: "X", summary: "S.", size: (1, 1))"#;
    let e = Pack::from_files([
        ("pack.ron", MOD_MANIFEST),
        ("frames/a.ron", one),
        ("frames/b.ron", one),
    ])
    .unwrap_err();
    assert!(
        e[0].kind == Some(Kind::Frame) && e[0].message.contains("duplicate id"),
        "{}",
        e[0]
    );
}

#[test]
fn ship_totals_are_the_sums_of_the_part_stat_lines() {
    let c = content();
    for s in c.ships.iter() {
        let t = c.ship_totals(s);
        let sum = |stat: &str| -> f32 {
            s.parts
                .iter()
                .flat_map(|pl| {
                    c.part_stats(c.parts.get(&pl.part).unwrap(), c.placement_material(pl))
                })
                .filter(|l| l.bonus.is_none() && l.stat == stat)
                .map(|l| l.value)
                .sum()
        };
        for (stat, value) in [
            ("build_points", t.build_points),
            ("mass", t.mass),
            ("hp", t.hp),
            ("power_output", t.power_output),
            ("power_draw", t.power_draw),
            ("thrust", t.thrust),
            ("cargo_capacity", t.cargo_capacity),
            ("rule_slots", t.rule_slots as f32),
            ("control_cost", t.control_cost as f32),
            ("control_capacity", t.control_capacity as f32),
        ] {
            assert_eq!(value, sum(stat), "{} {stat}", s.id);
        }
        // Base ships are one piece, so the sums equal the main piece's numbers.
        assert_eq!(
            c.ship_control(s),
            (t.control_cost, t.control_capacity),
            "{}",
            s.id
        );
        assert_eq!(c.ship_rule_slots(s), t.rule_slots, "{}", s.id);
    }
    let runner = c.ship_totals(c.ships.get("halvex_runner").unwrap());
    assert_eq!((runner.mass, runner.logic_cores), (106.5, 2));
}

#[test]
fn ship_totals_count_every_placement_of_any_design() {
    let c = content();
    let part = |id: &str| c.parts.get(id).unwrap();
    // Two pieces: the thruster at the corner is cut off, yet counts.
    let d = design(vec![
        at("logic_core", 0, 1),
        at("pulse_laser", 1, 1),
        hull("armor_block", 1, 0, "steel"),
        at("thruster", 3, 3),
    ]);
    let t = c.ship_totals(&d);
    assert_eq!(t.control_cost, 2);
    assert_eq!(c.ship_control(&d), (1, 16));
    assert_eq!(t.thrust, part("thruster").engine.as_ref().unwrap().thrust);
    assert_eq!(t.logic_cores, 1);
    // An invalid design: unknown parts are skipped, a hull part without material adds only its build points.
    let d = design(vec![
        at("armor_block", 0, 0),
        at("warp_core", 1, 0),
        at("logic_core", 2, 0),
    ]);
    let t = c.ship_totals(&d);
    let core = part("logic_core");
    assert_eq!(
        t.build_points,
        part("armor_block").build_points + core.build_points
    );
    assert_eq!((t.mass, t.hp), (core.mass.unwrap(), core.hp.unwrap()));
    assert_eq!(
        c.ship_totals(&design(Vec::new())),
        data::ShipTotals::default()
    );
}

#[test]
fn ship_stats_are_described_in_stats_ron() {
    let c = content();
    let want = [
        ("power_use", "Power", "MW", Better::Neutral),
        ("peak_heat", "Peak heat", "%", Better::Lower),
        ("acceleration", "Thrust/mass", "m/s²", Better::Higher),
        ("top_speed", "Top speed", "m/s", Better::Higher),
        ("turn_time", "Turn 90°", "s", Better::Lower),
        ("yaw", "Yaw", "", Better::Neutral),
        ("firepower", "Firepower", "/s", Better::Higher),
        ("control_load", "Control", "", Better::Neutral),
        ("device_load", "Device load", "", Better::Neutral),
    ];
    assert_eq!(SHIP_STATS, want.map(|w| w.0));
    for (id, label, unit, better) in want {
        let s = c.stats.get(id).unwrap();
        assert_eq!(
            (s.name.as_str(), s.unit.as_str(), s.better),
            (label, unit, better),
            "{id}"
        );
        assert!(!BUILTIN_STATS.contains(&id), "{id}");
    }
    // The panel shows the summed values under the part stat ids.
    for id in ["build_points", "mass", "hp", "cargo_capacity", "rule_slots"] {
        assert!(BUILTIN_STATS.contains(&id), "{id}");
    }
    // Formatted like part stat lines.
    let text = |id: &str, v: f32| c.stat_line(id, v).unwrap().text;
    assert_eq!(text("acceleration", 0.78), "0.78 m/s²");
    assert_eq!(text("peak_heat", 91.0), "91%");
    assert_eq!(text("turn_time", 3.5), "3.5 s");
    assert_eq!(text("firepower", 8.3), "8.3/s");
    assert_eq!(text("power_use", 44.0), "44 MW");
    assert_eq!(c.stat_line("control_load", 18.0).unwrap().label, "Control");
    assert!(c.stat_line("warp_factor", 1.0).is_none());
    // Required like the part stats: a pack without one fails validation.
    let files: Vec<(&str, String)> = data::base_pack_files()
        .iter()
        .map(|&(path, text)| {
            let text = match path {
                "stats.ron" => text
                    .lines()
                    .filter(|l| !l.contains("\"turn_time\""))
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => text.to_string(),
            };
            (path, text)
        })
        .collect();
    let e = Content::load(&[Pack::from_files(files).unwrap()]).unwrap_err();
    assert!(
        e.iter().any(|p| p.is_error()
            && p.kind == Some(Kind::Stat)
            && p.id.as_deref() == Some("turn_time")
            && p.message.contains("not defined in stats.ron")),
        "{}",
        show(&e)
    );
}
