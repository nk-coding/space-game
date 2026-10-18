//! Rules the base ships keep: the Halvex Runner's mirror symmetry, every ship one piece inside its frame, every gun,
//! engine and radiator of the Runner working, and the raider an automaton in automaton plate. (The checks against
//! the concept art went with the art itself, 14 Oct 2026: the content blueprints are the source of truth.) Also the
//! tone of the cells under machines (GDR-0069, `data::design::tone`).

mod common;

use std::collections::BTreeSet;

use common::*;
use data::Shape;

#[test]
fn halvex_runner_is_mirror_symmetric() {
    // drydock-model mirrorPart: x' = W - x - w, rot' = (4 - rot) % 4, flip' = !flip. Machines are placed
    // without flips, so for them only the position is mirrored.
    let c = content();
    let ship = c.ships.get("halvex_runner").unwrap();
    let key = |part: &str, x: i32, y: i32, rot: u8, flip: bool, m: &Option<String>| {
        (part.to_string(), x, y, rot, flip, m.clone())
    };
    let all: BTreeSet<_> = ship
        .parts
        .iter()
        .map(|p| key(&p.part, p.x, p.y, p.rot, p.flip, &p.material))
        .collect();
    for p in &ship.parts {
        let part = c.parts.get(&p.part).unwrap();
        let (w, _) = c.placed_shape(p).unwrap().dims();
        let mx = 12 - p.x - w as i32;
        let m = if part.shape == Shape::Slope {
            key(&p.part, mx, p.y, (4 - p.rot) % 4, !p.flip, &p.material)
        } else {
            key(&p.part, mx, p.y, p.rot, p.flip, &p.material)
        };
        assert!(
            all.contains(&m),
            "{} at ({}, {}) has no mirror partner",
            p.part,
            p.x,
            p.y
        );
    }
}

#[test]
fn automaton_raider_is_an_automaton_in_plate() {
    let c = content();
    let ship = c.ships.get("automaton_raider").unwrap();
    assert_eq!(ship.faction, "automatons");
    assert!(
        ship.parts
            .iter()
            .filter(|p| c.parts.get(&p.part).unwrap().hull)
            .all(|p| p.material.as_deref() == Some("automaton_plate"))
    );
}

#[test]
fn every_base_ship_is_one_piece_and_fits() {
    let c = content();
    for ship in c.ships.iter() {
        let grid = c.ship_grid(ship).unwrap();
        assert_eq!(grid.pieces().len(), 1, "{}", ship.id);
        let b = grid.bounds().unwrap();
        assert!(
            b.min_x >= 0
                && b.min_y >= 0
                && b.max_x <= ship.size.0 as i32
                && b.max_y <= ship.size.1 as i32,
            "{}",
            ship.id
        );
    }
    let halvex = c.ship_grid(c.ships.get("halvex_runner").unwrap()).unwrap();
    assert_eq!(halvex.bounds().unwrap().center(), [6.0, 3.5]);
}

#[test]
fn guns_and_engines_of_base_ships() {
    let c = content();
    let ship = c.ships.get("halvex_runner").unwrap();
    let grid = c.ship_grid(ship).unwrap();
    for (i, p) in ship.parts.iter().enumerate() {
        let part = c.parts.get(&p.part).unwrap();
        if part.weapon.is_some() {
            assert!(grid.line_of_fire_clear(i), "{} at {},{}", p.part, p.x, p.y);
        }
        if part.radiator.is_some() {
            assert!(
                grid.is_exposed(i),
                "{} at {},{} must vent",
                p.part,
                p.x,
                p.y
            );
        }
    }
}

/// On a design: a logic core with composite on two sides and steel on one sits on composite; a gyro touching
/// no hull takes the design's main tone (steel, three cells against two).
#[test]
fn machines_take_the_tone_of_the_hull_around_them() {
    let content = content();
    let at = |part: &str, x: i32, y: i32, material: Option<&str>| data::Placement {
        part: part.into(),
        x,
        y,
        rot: 0,
        flip: false,
        material: material.map(str::to_string),
        power: None,
        group: None,
    };
    let ship = data::Blueprint {
        id: "t".into(),
        name: String::new(),
        summary: String::new(),
        description: None,
        lore: None,
        tags: Vec::new(),
        faction: "ship_mind".into(),
        size: (5, 2),
        parts: vec![
            at("armor_block", 0, 0, Some("composite")),
            at("logic_core", 1, 0, None),
            at("armor_block", 2, 0, Some("composite")),
            at("armor_block", 0, 1, Some("steel")),
            at("armor_block", 1, 1, Some("steel")),
            at("armor_block", 2, 1, Some("steel")),
            at("gyro", 4, 1, None),
        ],
        behavior: None,
        assists: None,
        anchored: false,
        origin: None,
        source: Default::default(),
    };
    let grid = content.ship_grid(&ship).unwrap();
    let tones = data::design::tone::HullTones::new(&content, &ship, &grid);
    assert_eq!(tones.under(&grid, &grid.shape(1).cells()), "composite");
    assert_eq!(tones.under(&grid, &grid.shape(6).cells()), "steel");
    // A part about to be placed (the drydock's ghost) is judged the same way, its own cells skipped.
    assert_eq!(tones.under(&grid, &[(3, 1)]), "steel");
}
