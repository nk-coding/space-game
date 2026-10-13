//! Turning parts: gyro torque (gyros and logic cores), RCS nozzles on open sides, engine gimbal.

mod common;

use common::*;
use data::{Content, Dir, Footprint, GridPoint, Pack, PlacedShape, Severity, ShipGrid};

#[test]
fn turning_components_in_base_content() {
    let c = content();
    let gyro = c.parts.get("gyro").unwrap();
    assert_eq!(gyro.category, "propulsion");
    assert!(gyro.gyro.as_ref().unwrap().torque > 0.0);
    assert!(gyro.power.as_ref().unwrap().draw > 0.0);
    let rcs = c.parts.get("rcs_pod").unwrap();
    assert!(rcs.rcs.as_ref().unwrap().thrust > 0.0);
    assert!(rcs.engine.is_none(), "RCS pods are not rear engines");
    for core in ["logic_core", "automaton_core"] {
        assert!(
            c.parts.get(core).unwrap().gyro.is_some(),
            "{core} has built-in torque"
        );
    }
    for engine in ["thruster", "torch_drive"] {
        assert!(
            c.parts.get(engine).unwrap().engine.as_ref().unwrap().gimbal > 0.0,
            "{engine}"
        );
    }
}

#[test]
fn turning_stat_lines() {
    let c = content();
    let lines = |id: &str| -> Vec<(String, String)> {
        c.part_stats(c.parts.get(id).unwrap(), None)
            .into_iter()
            .filter(|l| l.stat != "control_cost") // checked in mods_and_validation.rs
            .map(|l| (l.stat, l.text))
            .collect()
    };
    let gyro = lines("gyro");
    assert!(
        gyro.contains(&("torque".into(), "1250 kN·m".into())),
        "{gyro:?}"
    );
    let rcs = lines("rcs_pod");
    assert!(
        rcs.contains(&("rcs_thrust".into(), "200 kN".into()))
            && rcs.contains(&("rcs_rated_speed".into(), "14 m/s".into())),
        "{rcs:?}"
    );
    let thruster = lines("thruster");
    let ids: Vec<&str> = thruster.iter().map(|(s, _)| s.as_str()).collect();
    // stats.ron order: thrust, rated speed, gimbal, then the plume.
    assert_eq!(
        ids,
        [
            "build_points",
            "mass",
            "hp",
            "power_draw",
            "heat_output",
            "heat_limit",
            "thrust",
            "rated_speed",
            "gimbal",
            "plume_length",
            "plume_damage",
            "plume_heat"
        ]
    );
    assert_eq!(thruster[7].1, "20 m/s");
    assert_eq!(thruster[8].1, "5°");
    assert_eq!(thruster[9].1, "3 m");
    assert!(lines("logic_core").iter().any(|(s, _)| s == "torque"));
}

fn block(x: i32, y: i32) -> PlacedShape {
    PlacedShape {
        footprint: Footprint::rect(1, 1),
        x,
        y,
        rot: 0,
        flip: false,
    }
}

#[test]
fn open_sides_are_rcs_nozzles() {
    // A pod at (1,1) with a block on its left and a slope face above it.
    let slope = PlacedShape {
        footprint: Footprint::slope(1),
        x: 1,
        y: 0,
        rot: 0,
        flip: false,
    }; // straight: Left, Down
    let flipped = PlacedShape {
        footprint: Footprint::slope(1),
        x: 1,
        y: 0,
        rot: 2,
        flip: false,
    }; // straight: Up, Right
    let g = ShipGrid::new(vec![block(1, 1), block(0, 1), slope]);
    assert_eq!(
        g.open_sides(0),
        vec![((1, 1), Dir::Down), ((1, 1), Dir::Right)]
    );
    // The slope's straight bottom links to the pod; either way the cell above is occupied, so no nozzle.
    let g2 = ShipGrid::new(vec![block(1, 1), block(0, 1), flipped]);
    assert_eq!(
        g2.open_sides(0),
        vec![((1, 1), Dir::Down), ((1, 1), Dir::Right)]
    );
    // Exposure (the venting rule) counts the side towards the unlinked slope face; open_sides does not.
    assert_eq!(g2.exposure((1, 1)), 3);
    // A lone pod has four nozzles; removing a neighbour opens a side.
    let mut g3 = ShipGrid::new(vec![block(1, 1), block(0, 1)]);
    assert_eq!(g3.open_sides(0).len(), 3);
    g3.remove(1);
    assert_eq!(g3.open_sides(0).len(), 4);
    assert!(g3.open_sides(1).is_empty(), "removed parts have no sides");
    // A 2×2 part has two sides per edge.
    let big = PlacedShape {
        footprint: Footprint::rect(2, 2),
        x: 0,
        y: 0,
        rot: 0,
        flip: false,
    };
    assert_eq!(ShipGrid::new(vec![big]).open_sides(0).len(), 8);
}

#[test]
fn side_helpers() {
    assert_eq!(Dir::Up.normal(), [0.0, -1.0]);
    assert_eq!(Dir::Left.side_center((2, 3)), [2.0, 3.5]);
    assert_eq!(Dir::Down.side_center((2, 3)), [2.5, 4.0]);
    assert_eq!(
        Dir::Right.side_segment((2, 3)),
        (GridPoint::new(3, 3), GridPoint::new(3, 4))
    );
    // Every side segment keeps its cell on the right, like the hull edges.
    for d in Dir::ALL {
        let (a, b) = d.side_segment((0, 0));
        let (dx, dy) = ((b.x - a.x) as f32, (b.y - a.y) as f32);
        assert_eq!([dy, -dx], d.normal(), "{d:?}");
    }
}

const MOD: &str = r#"(id: "m", name: "M", version: "1", schema_version: 1, depends: ["base"])"#;

fn load(path: &str, ron: &str) -> Result<Content, Vec<data::Problem>> {
    Content::load(&[
        data::base_pack().unwrap(),
        Pack::from_files([("pack.ron", MOD), (path, ron)]).unwrap(),
    ])
}

/// A harmless engine plume for test parts (the plume is required).
const PLUME: &str = "plume: (length: 1, damage: 0, heat: 0)";

#[test]
fn turning_validation() {
    let head = r#"id: "p", name: "P", summary: "S.", category: "propulsion", size: (1, 1), build_points: 1, mass: 1, hp: 1"#;
    let err = load(
        "parts/p.ron",
        &format!("({head}, engine: (thrust: 5, rated_speed: 10, gimbal: 60, {PLUME}))"),
    )
    .unwrap_err();
    assert!(
        err.iter()
            .any(|p| p.field.as_deref() == Some("engine.gimbal") && p.message.contains("45")),
        "{}",
        show(&err)
    );
    let err = load("parts/p.ron", &format!("({head}, gyro: (torque: 0))")).unwrap_err();
    assert!(
        err.iter()
            .any(|p| p.field.as_deref() == Some("gyro.torque")),
        "{}",
        show(&err)
    );
    let err = load("parts/p.ron", &format!("({head}, rcs: (thrust: -1, rated_speed: 10))")).unwrap_err();
    assert!(
        err.iter().any(|p| p.field.as_deref() == Some("rcs.thrust")),
        "{}",
        show(&err)
    );
    let err = load(
        "parts/p.ron",
        r#"(id: "p", name: "P", summary: "S.", category: "hull", size: (1, 1), build_points: 1, hull: true, rcs: (thrust: 1, rated_speed: 10))"#,
    )
    .unwrap_err();
    assert!(
        err.iter().any(|p| p.field.as_deref() == Some("rcs")),
        "{}",
        show(&err)
    );
    // Gimbal defaults to 0 and is optional.
    load("parts/p.ron", &format!("({head}, engine: (thrust: 5, rated_speed: 10, {PLUME}))")).unwrap();
    // The rated speed is required (a parse error) and positive (engines and RCS).
    let missing = format!("({head}, engine: (thrust: 5, {PLUME}))");
    let err = Pack::from_files([("pack.ron", MOD), ("parts/p.ron", missing.as_str())]).unwrap_err();
    assert!(err.iter().any(|p| p.message.contains("rated_speed")), "{}", show(&err));
    for block in [
        format!("engine: (thrust: 5, rated_speed: 0, {PLUME})"),
        "rcs: (thrust: 5, rated_speed: -3)".to_string(),
    ] {
        let err = load("parts/p.ron", &format!("({head}, {block})")).unwrap_err();
        assert!(
            err.iter()
                .any(|p| p.field.as_deref().is_some_and(|f| f.ends_with(".rated_speed"))),
            "{block}: {}",
            show(&err)
        );
    }
    // The plume is required too (GDR-0049); its numbers are not negative and its length at most 50 m.
    let missing = format!("({head}, engine: (thrust: 5, rated_speed: 10))");
    let err = Pack::from_files([("pack.ron", MOD), ("parts/p.ron", missing.as_str())]).unwrap_err();
    assert!(err.iter().any(|p| p.message.contains("plume")), "{}", show(&err));
    for (plume, field) in [
        ("(length: 1, damage: -1, heat: 0)", "engine.plume.damage"),
        ("(length: 1, damage: 0, heat: -2)", "engine.plume.heat"),
        ("(length: 80, damage: 1, heat: 1)", "engine.plume.length"),
    ] {
        let block = format!("({head}, engine: (thrust: 5, rated_speed: 10, plume: {plume}))");
        let err = load("parts/p.ron", &block).unwrap_err();
        assert!(
            err.iter().any(|p| p.field.as_deref() == Some(field)),
            "{plume}: {}",
            show(&err)
        );
    }
}

#[test]
fn boxed_rcs_and_radiators_warn() {
    let c = load(
        "ships/s.ron",
        r#"(id: "s", name: "S", summary: "S.", faction: "ship_mind", size: (3, 3), parts: [
            (part: "rcs_pod", x: 1, y: 1), (part: "logic_core", x: 1, y: 0), (part: "radiator", x: 0, y: 1),
            (part: "armor_block", x: 2, y: 1, material: "steel"), (part: "armor_block", x: 1, y: 2, material: "steel"),
        ])"#,
    )
    .unwrap();
    let w: Vec<_> = data::validate(&c)
        .into_iter()
        .filter(|p| p.id.as_deref() == Some("s"))
        .collect();
    assert!(w.iter().all(|p| p.severity == Severity::Warning));
    assert!(
        w.iter()
            .any(|p| p.message.contains("no side open to space")),
        "{}",
        show(&w)
    );
    assert!(
        !w.iter().any(|p| p.message.contains("boxed in")),
        "the radiator's left side is open: {}",
        show(&w)
    );
}
