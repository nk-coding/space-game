//! Drydock plumbing in `data`: checking a design on its own (`validate_ship`).

mod common;

use common::*;
use data::{Blueprint, Content, Kind, Pack, Placement, Problem, Severity, validate, validate_ship};

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
