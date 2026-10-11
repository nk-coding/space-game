//! The base ships against their art sources: Halvex Runner tile-for-tile against
//! `art/ships/halvex-runner.layout.json` (including its `outline_path`), the raider against its SVG.

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

#[test]
fn halvex_runner_matches_layout_json_tile_for_tile() {
    let c = content();
    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("art/ships/halvex-runner.layout.json")).unwrap(),
    )
    .unwrap();
    let px = json["cell_px"].as_i64().unwrap() as i32;
    assert_eq!(px, 20);
    let ship = c.ships.get("halvex_runner").unwrap();
    assert_eq!(
        ship.size,
        (
            json["size_cells"][0].as_u64().unwrap() as u32,
            json["size_cells"][1].as_u64().unwrap() as u32
        )
    );
    let kinds = |k: &str| match k {
        "fis" => "fission_reactor",
        "fus" => "fusion_reactor",
        "cap" => "capacitor",
        "shd" => "shield_emitter",
        "rad" => "radiator",
        "cor" => "logic_core",
        "cgo" => "cargo_bay",
        "thr" => "thruster",
        "pls" => "pulse_laser",
        "bem" => "beam_laser",
        "tor" => "torch_drive",
        "blk" => "armor_block",
        other => panic!("unexpected tile kind {other}"),
    };
    let grid = c.ship_grid(ship).unwrap();

    // Every tile is exactly one rect placement of the mapped part covering exactly the tile's cells.
    let mut matched = BTreeSet::new();
    for t in json["tiles"].as_array().unwrap() {
        let v = |k: &str| t[k].as_i64().unwrap() as i32 / px;
        let (x, y, w, h) = (v("x"), v("y"), v("w"), v("h"));
        let part = kinds(t["kind"].as_str().unwrap());
        let idx = grid
            .occupant((x, y))
            .unwrap_or_else(|| panic!("no part at tile {x},{y}"));
        let pl = &ship.parts[idx];
        assert_eq!(pl.part, part, "tile at {x},{y}");
        if part == "armor_block" {
            assert_eq!(pl.material.as_deref(), Some("composite"));
        }
        let mut want: Vec<(i32, i32)> = (0..h)
            .flat_map(|j| (0..w).map(move |i| (x + i, y + j)))
            .collect();
        let mut got = grid.shape(idx).cells();
        want.sort();
        got.sort();
        assert_eq!(got, want, "tile {part} at {x},{y}");
        assert!(matched.insert(idx));
    }
    // Every slope triangle is exactly one slope placement's polygon.
    for tri in json["slopes"].as_array().unwrap() {
        let want: BTreeSet<GridPoint> = tri
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                GridPoint::new(
                    p[0].as_i64().unwrap() as i32 / px,
                    p[1].as_i64().unwrap() as i32 / px,
                )
            })
            .collect();
        let idx = (0..ship.parts.len())
            .find(|&i| {
                grid.shape(i).footprint.shape == Shape::Slope
                    && grid.shape(i).polygon().into_iter().collect::<BTreeSet<_>>() == want
            })
            .unwrap_or_else(|| panic!("no slope placement with polygon {want:?}"));
        assert_eq!(ship.parts[idx].material.as_deref(), Some("composite"));
        assert!(matched.insert(idx));
    }
    assert_eq!(
        matched.len(),
        ship.parts.len(),
        "placements without a tile or slope in the layout"
    );
}

#[test]
fn halvex_runner_is_one_piece_and_its_hull_edge_matches_outline_path() {
    let c = content();
    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("art/ships/halvex-runner.layout.json")).unwrap(),
    )
    .unwrap();
    let (pieces, outline) = main_outline(&c, "halvex_runner");
    assert_eq!(pieces, 1);
    let want = canonical(
        &parse_path(json["outline_path"].as_str().unwrap()),
        0.0,
        20.0,
    );
    assert_eq!(outline, want);
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
        if part.id == "beam_laser" {
            assert!(grid.line_of_fire_clear(i));
        }
        if part.id == "pulse_laser" {
            // A slope sits right in front of each wing-tip pulse laser.
            assert!(!grid.line_of_fire_clear(i));
        }
    }
}
