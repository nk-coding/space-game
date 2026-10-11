//! Builds the render-ready [`ShipVisual`] of a blueprint from `data` content (parts, materials, factions,
//! blueprint geometry). This is the only place where the renderer touches content types.
//!
//! What lives here is purely visual interpretation (which icon, which tile tone, barrel sizes per gun art);
//! everything about shape and topology (polygons, pieces, hull edges, line of fire, exhaust) comes from
//! `data::geometry`.

use bevy::prelude::*;
use data::{Blueprint, Content, ShipGrid};

use crate::ship_view::{EngineVisual, GunVisual, PartVisual, ShipVisual};
use crate::style::Style;

/// Barrel and muzzle look per gun icon, taken from the reference renders
/// (`art/ships/halvex-runner.svg`, `art/ships/automaton-raider.svg`): `(barrel (width, protrusion), muzzle radius)`
/// in cells. Guns without an entry get no barrel and a small muzzle glow at their front edge.
fn gun_art(icon: Option<&str>) -> (Option<(f32, f32)>, f32) {
    match icon {
        Some("beam-laser") => (Some((0.25, 0.7)), 0.15),
        Some("automaton-cannon") => (Some((0.2, 0.6)), 0.15),
        _ => (None, 0.12),
    }
}

/// Builds the visual description of `ship`. Errors only for content that failed validation.
pub fn ship_visual(content: &Content, ship: &Blueprint, style: &Style) -> Result<ShipVisual, String> {
    let grid: ShipGrid = content.ship_grid(ship)?;
    let parts_def: Vec<&data::Part> = ship
        .parts
        .iter()
        .map(|p| content.parts.get(&p.part).ok_or_else(|| format!("unknown part `{}`", p.part)))
        .collect::<Result<_, _>>()?;

    let pieces = grid.pieces();
    let main = grid.main_piece(&pieces, |i| parts_def[i].logic.is_some());
    let is_main = |i: usize| main.is_some() && pieces.piece_of_part.get(i).copied().flatten() == main;

    let enemy = content.factions.get(&ship.faction).map(|f| f.glow == "enemy").unwrap_or(false);
    let glow = if enemy { style.glow_enemy } else { style.glow_player };

    let mut parts = Vec::with_capacity(ship.parts.len());
    // Centre of mass as the pivot (the sim will own the real one; the same data helpers feed both).
    let (mut mass_sum, mut mass_pos) = (0.0f32, Vec2::ZERO);
    for (i, (pl, def)) in ship.parts.iter().zip(&parts_def).enumerate() {
        let shape = grid.shape(i);
        let polygon: Vec<Vec2> = shape.polygon().iter().map(|g| Vec2::new(g.x as f32, g.y as f32)).collect();
        let [cx, cy] = shape.center();
        let center = Vec2::new(cx, cy);
        let (w, h) = shape.dims();
        let (fx, fy) = shape.facing().delta();
        let facing = Vec2::new(fx as f32, fy as f32);

        let material = content.placement_material(pl);
        let color_key = content.part_color(def);
        let (fill, debug_color, icon) = if def.hull {
            let tone = material.and_then(|m| m.armor.as_ref()).map(|a| a.tone.as_str()).unwrap_or("steel");
            let c = style.armor_tone(tone);
            (c, lighten(c, 0.12), None)
        } else {
            let sys = style.system_color(color_key.unwrap_or("cargo"));
            (style.machine_tile, mix(style.machine_tile, sys, 0.55), def.icon.clone())
        };

        let mass = content.placement_mass(pl).unwrap_or(0.0);
        let [mx, my] = shape.centroid();
        mass_sum += mass;
        mass_pos += Vec2::new(mx, my) * mass;

        let engine = def.engine.as_ref().map(|_| EngineVisual { large: w.min(h) >= 2, clear: grid.exhaust_clear(i) });
        let gun = def.weapon.as_ref().map(|_| {
            let (barrel, muzzle_radius) = gun_art(def.icon.as_deref());
            GunVisual {
                clear: grid.line_of_fire_clear(i),
                barrel,
                muzzle_radius,
                color: style.system_color(color_key.unwrap_or("weapons")),
            }
        });

        parts.push(PartVisual {
            polygon,
            fill,
            debug_color,
            icon,
            center,
            dims: Vec2::new(w as f32, h as f32),
            rot: pl.rot,
            flip: pl.flip,
            facing,
            engine,
            gun,
            shield_radius: def.shield.as_ref().map(|s| s.radius),
            main: is_main(i),
        });
    }

    let hull_loops = match main {
        Some(m) => data::geometry::hull_loops(&grid.piece_hull_edges(&pieces, m))
            .into_iter()
            .map(|l| (l.points.iter().map(|g| Vec2::new(g.x as f32, g.y as f32)).collect(), l.closed))
            .collect(),
        None => Vec::new(),
    };
    let b = grid.bounds().ok_or("empty ship")?;
    let pivot = if mass_sum > 0.0 { mass_pos / mass_sum } else { Vec2::new(b.center()[0], b.center()[1]) };

    // Navigation lights on player ships (halvex-runner.svg): coral on the port wing tip, teal on starboard,
    // at the outer bottom corner of the outermost full cell of the main piece.
    let mut nav_lights = Vec::new();
    if !enemy {
        let mut cells: Vec<(i32, i32)> = Vec::new();
        for (i, def) in parts_def.iter().enumerate() {
            if is_main(i) && def.shape == data::Shape::Rect {
                cells.extend(grid.shape(i).cells());
            }
        }
        let pick = |key: fn(&(i32, i32)) -> (i32, i32)| cells.iter().copied().max_by_key(key);
        if let (Some(l), Some(r)) = (pick(|c| (-c.0, c.1)), pick(|c| (c.0, c.1))) {
            nav_lights.push((Vec2::new(l.0 as f32 + 0.2, l.1 as f32 + 0.85), style.system_color("heat")));
            nav_lights.push((Vec2::new(r.0 as f32 + 0.8, r.1 as f32 + 0.85), style.glow_player));
        }
    }

    Ok(ShipVisual {
        nav_lights,
        parts,
        hull_loops,
        pivot,
        bounds_min: Vec2::new(b.min_x as f32, b.min_y as f32),
        bounds_max: Vec2::new(b.max_x as f32, b.max_y as f32),
        glow,
        enemy,
    })
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    Color::srgb(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t)
}

fn lighten(c: Color, t: f32) -> Color {
    mix(c, Color::WHITE, t)
}
