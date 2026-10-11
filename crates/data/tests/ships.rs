//! The base ships against their art sources. Halvex Runner: same cells and spine as the concept art
//! (`art/ships/halvex-runner.layout.json`), its outline equal to `outline_path` except the wing shoulders, mirror
//! symmetry, and every gun, engine and radiator working. The raider: its outline equals its SVG.

mod common;

use std::collections::BTreeSet;

use common::*;
use data::geometry::hull_loops;
use data::{Content, GridPoint, Shape};

/// Parses the subset of SVG path syntax used by the art (M, L, H, V, Z, absolute, implicit repeats).
fn parse_path(d: &str) -> Vec<(f64, f64)> {
    let mut tokens = Vec::new();
    let mut num = String::new();
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            if !num.is_empty() {
                tokens.push(num.clone());
                num.clear();
            }
            tokens.push(ch.to_string());
        } else if ch == ' ' || ch == ',' {
            if !num.is_empty() {
                tokens.push(num.clone());
                num.clear();
            }
        } else if ch == '-' && !num.is_empty() {
            tokens.push(num.clone());
            num = "-".into();
        } else {
            num.push(ch);
        }
    }
    if !num.is_empty() {
        tokens.push(num);
    }
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let (mut cmd, mut i, mut cur) = ('M', 0, (0.0, 0.0));
    while i < tokens.len() {
        let t = &tokens[i];
        if t.chars().next().unwrap().is_ascii_alphabetic() {
            cmd = t.chars().next().unwrap();
            i += 1;
            if cmd == 'Z' {
                break;
            }
            continue;
        }
        let n = |k: usize| tokens[k].parse::<f64>().unwrap();
        match cmd {
            'M' | 'L' => {
                cur = (n(i), n(i + 1));
                i += 2;
            }
            'H' => {
                cur.0 = n(i);
                i += 1;
            }
            'V' => {
                cur.1 = n(i);
                i += 1;
            }
            c => panic!("unsupported path command {c}"),
        }
        pts.push(cur);
    }
    pts
}

/// Closed polygon in cells, collinear points dropped, starting at the top-most, left-most vertex.
fn canonical(points: &[(f64, f64)], offset: f64, cell: f64) -> Vec<GridPoint> {
    let mut pts: Vec<GridPoint> = points
        .iter()
        .map(|&(x, y)| {
            let (cx, cy) = ((x + offset) / cell, (y + offset) / cell);
            assert!(
                cx.fract() == 0.0 && cy.fract() == 0.0,
                "outline vertex off the grid"
            );
            GridPoint::new(cx as i32, cy as i32)
        })
        .collect();
    loop {
        let n = pts.len();
        let drop = (0..n).find(|&k| {
            let (a, b, c) = (pts[(k + n - 1) % n], pts[k], pts[(k + 1) % n]);
            let (ux, uy, vx, vy) = (b.x - a.x, b.y - a.y, c.x - b.x, c.y - b.y);
            ux * vy - uy * vx == 0 && ux * vx + uy * vy > 0
        });
        match drop {
            Some(k) => {
                pts.remove(k);
            }
            None => break,
        }
    }
    let start = (0..pts.len())
        .min_by_key(|&k| (pts[k].y, pts[k].x))
        .unwrap();
    pts.rotate_left(start);
    pts
}

fn main_outline(c: &Content, ship_id: &str) -> (usize, Vec<GridPoint>) {
    let ship = c.ships.get(ship_id).unwrap();
    let grid = c.ship_grid(ship).unwrap();
    let pieces = grid.pieces();
    let main = grid
        .main_piece(&pieces, |i| {
            c.parts.get(&ship.parts[i].part).unwrap().logic.is_some()
        })
        .unwrap();
    let loops = hull_loops(&grid.piece_hull_edges(&pieces, main));
    assert_eq!(
        loops.len(),
        1,
        "{ship_id}: expected one outline loop, got {loops:?}"
    );
    assert!(loops[0].closed);
    (pieces.len(), loops[0].points.clone())
}

fn layout_json() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("art/ships/halvex-runner.layout.json")).unwrap(),
    )
    .unwrap()
}

/// Occupied cells of the concept-art layout: tiles plus the cells under each slope triangle.
fn art_cells(json: &serde_json::Value) -> BTreeSet<(i32, i32)> {
    let px = json["cell_px"].as_i64().unwrap() as i32;
    let mut cells = BTreeSet::new();
    for t in json["tiles"].as_array().unwrap() {
        let v = |k: &str| t[k].as_i64().unwrap() as i32 / px;
        for j in 0..v("h") {
            for i in 0..v("w") {
                cells.insert((v("x") + i, v("y") + j));
            }
        }
    }
    for tri in json["slopes"].as_array().unwrap() {
        let pts: Vec<(i32, i32)> = tri
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p[0].as_i64().unwrap() as i32 / px,
                    p[1].as_i64().unwrap() as i32 / px,
                )
            })
            .collect();
        let (x0, y0) = (
            pts.iter().map(|p| p.0).min().unwrap(),
            pts.iter().map(|p| p.1).min().unwrap(),
        );
        let (x1, y1) = (
            pts.iter().map(|p| p.0).max().unwrap(),
            pts.iter().map(|p| p.1).max().unwrap(),
        );
        for y in y0..y1 {
            for x in x0..x1 {
                cells.insert((x, y));
            }
        }
    }
    cells
}

/// The content blueprint is the source of truth; art/ships/halvex-runner.* stays the original concept art. The
/// layout was adjusted (Niklas, 11 Oct 2026) to work under the drydock rules while keeping the silhouette, the
/// symmetry and the look. These checks pin what was kept.
#[test]
fn halvex_runner_keeps_the_concept_silhouette() {
    let c = content();
    let json = layout_json();
    let ship = c.ships.get("halvex_runner").unwrap();
    let grid = c.ship_grid(ship).unwrap();
    assert_eq!(ship.size, (12, 7));

    // Same occupied cells as the concept art.
    let cells: BTreeSet<(i32, i32)> = (0..ship.parts.len())
        .flat_map(|i| grid.shape(i).cells())
        .collect();
    assert_eq!(cells, art_cells(&json));

    // Same outline, except at the wing shoulders, where the pulse lasers (full cells) replaced the 1×1 slopes
    // that blocked their line of fire: the art's vertices (0,3) and (12,3) move up to (0,2) and (12,2).
    let (pieces, outline) = main_outline(&c, "halvex_runner");
    assert_eq!(pieces, 1);
    let want: Vec<GridPoint> = canonical(
        &parse_path(json["outline_path"].as_str().unwrap()),
        0.0,
        20.0,
    )
    .into_iter()
    .map(|p| match (p.x, p.y) {
        (0, 3) => GridPoint::new(0, 2),
        (12, 3) => GridPoint::new(12, 2),
        _ => p,
    })
    .collect();
    assert_eq!(outline, want);

    // The spine and the engines sit exactly where the art has them.
    let px = json["cell_px"].as_i64().unwrap() as i32;
    for t in json["tiles"].as_array().unwrap() {
        let part = match t["kind"].as_str().unwrap() {
            "bem" => "beam_laser",
            "fus" => "fusion_reactor",
            "tor" => "torch_drive",
            "thr" => "thruster",
            _ => continue,
        };
        let (x, y) = (
            t["x"].as_i64().unwrap() as i32 / px,
            t["y"].as_i64().unwrap() as i32 / px,
        );
        let i = grid.occupant((x, y)).unwrap();
        assert_eq!(
            (
                ship.parts[i].part.as_str(),
                ship.parts[i].x,
                ship.parts[i].y
            ),
            (part, x, y)
        );
    }
    // All slopes left of the art are still there, in composite.
    let slopes = ship
        .parts
        .iter()
        .filter(|p| c.parts.get(&p.part).unwrap().shape == Shape::Slope)
        .count();
    assert_eq!(slopes, json["slopes"].as_array().unwrap().len() - 2);
    for p in ship
        .parts
        .iter()
        .filter(|p| c.parts.get(&p.part).unwrap().hull)
    {
        assert_eq!(p.material.as_deref(), Some("composite"));
    }
}

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
fn automaton_raider_matches_its_svg() {
    let c = content();
    let (pieces, outline) = main_outline(&c, "automaton_raider");
    assert_eq!(pieces, 1);
    // The hull glow path from art/ships/automaton-raider.svg (cells of 20 px centred on 0,0).
    let svg = std::fs::read_to_string(repo_root().join("art/ships/automaton-raider.svg")).unwrap();
    let d = svg
        .split("<path d=\"")
        .filter_map(|s| s.split('"').next())
        .find(|d| d.starts_with("M-10 -30"))
        .unwrap();
    assert_eq!(outline, canonical(&parse_path(d), 30.0, 20.0));
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
        if part.engine.is_some() {
            assert!(grid.exhaust_clear(i), "{} at {},{}", p.part, p.x, p.y);
        }
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
