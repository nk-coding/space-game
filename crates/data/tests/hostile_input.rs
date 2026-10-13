//! Hostile and damaged input from outside the build (share codes, `.ron` files, stored documents, mods): every
//! case ends in a clean error or a usable result, never in a panic, an overflow or a huge allocation. Debug
//! builds check integer overflow, so these tests also catch arithmetic that would wrap in release builds.

mod common;

use common::*;
use data::edit::{Drydock, OpenError, Refusal};
use data::limits::MAX_COORD;
use data::validate::MAX_FRAME_SIZE;
use data::{Blueprint, Content, Pack, Placement, ShipGrid, parse_blueprint, validate_ship};

fn block(x: i32, y: i32) -> Placement {
    Placement {
        part: "armor_block".into(),
        x,
        y,
        rot: 0,
        flip: false,
        material: Some("steel".into()),
        power: None,
    }
}

fn design(size: (u32, u32), parts: Vec<Placement>) -> Blueprint {
    Blueprint {
        id: "x".into(),
        name: "x".into(),
        summary: "x".into(),
        description: None,
        lore: None,
        tags: Vec::new(),
        faction: "ship_mind".into(),
        size,
        parts,
        behavior: None,
        source: Default::default(),
    }
}

/// Every check a design from outside goes through; none may panic.
fn every_check(c: &Content, bp: &Blueprint) {
    let _ = validate_ship(c, bp);
    let _ = c.ship_grid(bp);
    let _ = c.ship_rule_slots(bp);
    let _ = c.ship_control(bp);
    let _ = c.ship_totals(bp);
    for frame in [(24, 24), (MAX_FRAME_SIZE, MAX_FRAME_SIZE)] {
        if let Ok(dock) = Drydock::open(c, bp, frame) {
            let _ = validate_ship(c, &dock.export());
        }
    }
}

// ---- placement coordinates near the ends of the i32 range (security review, finding 1) ----

/// The payload of the 200-character share code of the untrusted-input review: three blocks, two of them near
/// `i32::MIN` and `i32::MAX`. Its import crashed `Drydock::open` (debug: overflow; release: a part stamped
/// outside the frame).
const REVIEW_PAYLOAD_SHIP: &str = r#"(id: "x", name: "x", summary: "x", faction: "ship_mind", size: (1, 1), parts: [(part: "armor_block", x: -2147483647, y: 0, material: "steel"), (part: "armor_block", x: 2147483646, y: 0, material: "steel"), (part: "armor_block", x: 0, y: 0, material: "steel")])"#;

#[test]
fn reading_refuses_coordinates_beyond_the_limit() {
    let err = parse_blueprint(REVIEW_PAYLOAD_SHIP).unwrap_err();
    assert!(err[0].message.contains("out of range"), "{}", show(&err));
    // The same check runs wherever a blueprint is read with the content options (a share code's payload, a
    // stored document), not only in `parse_blueprint`.
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct Payload {
        packs: Vec<(String, String)>,
        ship: Blueprint,
    }
    let payload = format!("(packs: [], ship: {REVIEW_PAYLOAD_SHIP})");
    let e = data::ron_options().from_str::<Payload>(&payload).err().unwrap();
    assert!(e.to_string().contains("out of range"), "{e}");
    for (x, ok) in [
        (MAX_COORD, true),
        (-MAX_COORD, true),
        (MAX_COORD + 1, false),
        (-MAX_COORD - 1, false),
        (i32::MAX, false),
        (i32::MIN, false),
    ] {
        let text = format!(
            r#"(id: "x", name: "x", summary: "x", faction: "ship_mind", size: (1, 1), parts: [(part: "armor_block", x: 0, y: {x}, material: "steel")])"#
        );
        assert_eq!(parse_blueprint(&text).is_ok(), ok, "y = {x}");
    }
    // Beyond i32 altogether.
    let text = REVIEW_PAYLOAD_SHIP.replace("2147483646", "99999999999999999999");
    assert!(parse_blueprint(&text).is_err());
}

#[test]
fn far_apart_parts_do_not_open_and_never_overflow() {
    let c = content();
    // Built in code, so nothing refused the coordinates on the way in.
    let review = design((1, 1), vec![block(-2147483647, 0), block(2147483646, 0), block(0, 0)]);
    assert_eq!(
        Drydock::open(&c, &review, (24, 24)).err(),
        Some(OpenError::DoesNotFit { design: (4294967294, 1) })
    );
    for (a, b, wide) in [
        (block(0, 0), block(i32::MAX, 0), (1 << 31, 1)),
        (block(i32::MIN, 0), block(i32::MAX, 0), (u32::MAX, 1)),
        (block(0, i32::MIN), block(0, i32::MAX), (1, u32::MAX)),
        (block(i32::MIN, i32::MIN), block(i32::MAX, i32::MAX), (u32::MAX, u32::MAX)),
        (block(MAX_COORD, 0), block(-MAX_COORD, 0), (2 * MAX_COORD as u32 + 1, 1)),
    ] {
        let bp = design((24, 24), vec![a, b]);
        assert_eq!(Drydock::open(&c, &bp, (24, 24)).err(), Some(OpenError::DoesNotFit { design: wide }));
        every_check(&c, &bp);
        assert!(c.ship_grid(&bp).is_err());
        assert_eq!((c.ship_rule_slots(&bp), c.ship_control(&bp)), (0, (0, 0)));
        let problems = validate_ship(&c, &bp);
        assert!(problems.iter().any(|p| p.message.contains("does not fit the 24×24 frame")), "{}", show(&problems));
    }
    every_check(&c, &review);
}

#[test]
fn a_lone_far_part_opens_centred() {
    let c = content();
    for (x, y) in [(i32::MAX, i32::MAX), (i32::MIN, i32::MIN), (i32::MAX, i32::MIN), (MAX_COORD, -MAX_COORD)] {
        let bp = design((1, 1), vec![block(x, y)]);
        let dock = Drydock::open(&c, &bp, (24, 24)).unwrap();
        assert_eq!((dock.parts()[0].x, dock.parts()[0].y), (11, 11), "({x}, {y})");
        assert_eq!(dock.export().parts[0], block(0, 0));
        every_check(&c, &bp);
    }
    // The grid of the content view refuses it (it is outside the coordinate range), validation says why.
    let bp = design((24, 24), vec![block(i32::MAX, 0)]);
    assert!(c.ship_grid(&bp).is_err());
    let problems = validate_ship(&c, &bp);
    assert!(problems.iter().any(|p| p.field.as_deref() == Some("parts[0]")), "{}", show(&problems));
}

#[test]
fn ship_grids_refuse_far_or_wide_parts() {
    let c = content();
    let shape = |x, y| c.placed_shape(&block(x, y)).unwrap();
    let side = MAX_FRAME_SIZE as i32;
    assert!(ShipGrid::new(vec![shape(0, 0), shape(side - 1, side - 1)]).is_some());
    for far in [
        vec![shape(0, 0), shape(side, 0)],
        vec![shape(0, 0), shape(0, side)],
        vec![shape(i32::MAX, 0)],
        vec![shape(i32::MIN, i32::MIN)],
        vec![shape(MAX_COORD, 0)],
        vec![shape(i32::MIN, 0), shape(i32::MAX, 0)],
    ] {
        assert!(ShipGrid::new(far).is_none());
    }
    // A huge footprint (a broken mod part) is refused before any cell is stamped.
    let huge = data::PlacedShape {
        footprint: data::Footprint::rect(u32::MAX, u32::MAX),
        x: 0,
        y: 0,
        rot: 1,
        flip: true,
    };
    assert!(ShipGrid::new(vec![huge]).is_none());
    assert_eq!(huge.area(), u32::MAX as f32 * u32::MAX as f32);
}

#[test]
fn drydock_edits_with_extreme_arguments_are_refused() {
    let c = content();
    let mut dock = Drydock::open(&c, &design((3, 3), vec![block(0, 0)]), (24, 24)).unwrap();
    dock.mirror = true;
    for (x, y) in [(i32::MAX, 0), (i32::MIN, 0), (0, i32::MAX), (0, i32::MIN), (i32::MAX, i32::MIN)] {
        assert_eq!(dock.check(&block(x, y)), Err(Refusal::Outside));
        assert_eq!(dock.place(block(x, y)), Err(Refusal::Outside));
        assert_eq!(dock.paint((x, y), "steel").err(), Some(Refusal::Outside));
        assert_eq!(dock.erase((x, y)).err(), Some(Refusal::Outside));
        assert_eq!(dock.place_slope("slope_1x1", (x, y), "steel").err(), Some(Refusal::Outside));
        let _ = dock.mirror_cell((x, y));
        let _ = dock.mirror_of(&block(x, y));
        assert_eq!(dock.move_part(0, x, y).err(), Some(Refusal::Outside));
        assert_eq!(dock.preview_move(0, x, y).err(), Some(Refusal::Outside));
    }
    // A fill between extreme corners is clipped to the frame.
    dock.fill((i32::MIN, i32::MIN), (i32::MAX, i32::MAX), "steel").unwrap();
    assert_eq!(dock.parts().len(), 24 * 24);
    assert!(dock.set_frame((MAX_FRAME_SIZE + 1, 24)).is_err());
    assert!(dock.set_frame((u32::MAX, u32::MAX)).is_err());
}

#[test]
fn blueprint_sizes_beyond_the_frame_limit_are_errors() {
    let c = content();
    for size in [(MAX_FRAME_SIZE + 1, 1), (1, u32::MAX), (u32::MAX, u32::MAX), (100_000, 100_000)] {
        let bp = design(size, vec![block(0, 0), block(99_999, 99_999)]);
        let problems = validate_ship(&c, &bp);
        assert!(
            problems.iter().any(|p| p.field.as_deref() == Some("size") && p.message.contains("at most")),
            "{size:?}: {}",
            show(&problems)
        );
        every_check(&c, &bp);
    }
    // As a mod ship: the pack loads with an error, without building a 100000 × 100000 grid.
    let ship = r#"(id: "huge", name: "Huge", summary: "x", faction: "ship_mind", size: (100000, 100000),
        parts: [(part: "logic_core", x: 0, y: 0), (part: "logic_core", x: 65536, y: 65536)])"#;
    let m = Pack::from_files([
        ("pack.ron", r#"(id: "m", name: "M", version: "1", schema_version: 1, depends: ["base"])"#),
        ("ships/huge.ron", ship),
    ])
    .unwrap();
    let e = Content::load(&[data::base_pack().unwrap(), m]).unwrap_err();
    assert!(e.iter().any(|p| p.id.as_deref() == Some("huge") && p.field.as_deref() == Some("size")), "{}", show(&e));
}

// ---- RON from outside: byte order marks, deep nesting, size ----

#[test]
fn byte_order_marks_are_skipped() {
    let bom = "\u{feff}";
    let m = Pack::from_files([
        ("pack.ron", format!(r#"{bom}(id: "m", name: "M", version: "1", schema_version: 1, depends: ["base"])"#)),
        ("frames.ron", format!(r#"{bom}[(id: "dock", name: "Dock", summary: "A frame.", size: (8, 8))]"#)),
        ("factions/x.ron", format!("{bom}// a comment first\n(id: \"x\", name: \"X\", summary: \"X.\", glow: \"enemy\")")),
    ])
    .unwrap_or_else(|p| panic!("{}", show(&p)));
    let c = Content::load(&[data::base_pack().unwrap(), m]).unwrap_or_else(|p| panic!("{}", show(&p)));
    assert!(c.frames.contains("dock") && c.factions.contains("x"));
    let ship = data::blueprint_to_ron(c.ships.get("halvex_runner").unwrap());
    assert!(parse_blueprint(&format!("{bom}{ship}")).is_ok());
}

#[test]
fn deep_nesting_and_huge_texts_give_clean_errors() {
    let ship = data::blueprint_to_ron(content().ships.get("halvex_runner").unwrap());
    for deep in [
        "(".repeat(100_000),
        "[".repeat(100_000),
        format!("{}{}", "(id: ".repeat(10_000), ")".repeat(10_000)),
        ship.replace("faction: ", &format!("tags: {}", "[".repeat(10_000))),
        ship.replace("parts: [", &format!("parts: [{}", "(".repeat(10_000))),
        ship.replace("parts: [", &format!("parts: [(part: {}", "Some(".repeat(10_000))),
        format!("{}{ship}", "/*".repeat(10_000)),
    ] {
        assert!(parse_blueprint(&deep).is_err());
        let _ = Pack::from_files([("pack.ron", deep.as_str()), ("ships/x.ron", deep.as_str())]);
    }
    let huge = format!("{ship}{}", " ".repeat(data::limits::MAX_DOCUMENT_BYTES));
    let err = parse_blueprint(&huge).unwrap_err();
    assert!(err[0].message.contains("too long"), "{}", show(&err));
}
