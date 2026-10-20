//! Visual descriptions of ships: what each part looks like ([`DesignVisual`], once per blueprint, from `data`
//! content), what one live piece looks like ([`piece_visual`], from the `sim` state of that piece) and what a
//! whole blueprint looks like at rest ([`blueprint_visual`], the drydock: every part, from the ship grid alone).
//!
//! Visual interpretation only (ship drawing and icon, tile tone, barrels from the part's content, nav lights). Shape
//! and topology come from `data::design::geometry` and the sim: polygons, the piece's alive parts and frame origin, hull
//! edges, line of fire, shield radius.
//!
//! Machines are drawn as housings on a hull-toned cell (GDR-0069): `data::design::tone::HullTones` picks the cell's
//! armour tone (the wiki's ship drawings use the same rule).

use bevy::prelude::*;
use data::design::tone::{HullTones, armor_tone};
use data::{Blueprint, Content, ShipGrid};
use sim::{Piece, Ship, Stat};
use kit::style::{Style, mix};

use crate::ship_view::{GunVisual, PartVisual, ShipVisual};

/// The muzzle glow of a gun without a barrel (its content has no `barrel`), at its front edge, in cells.
const BARE_MUZZLE: f32 = 0.12;

/// How a ship is drawn: as one of the player's (teal glow) or as an enemy's (coral). By default its design's faction
/// decides (`Faction::glow`); a replayed fight decides by side (`SimWorld::looks`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Look {
    Player,
    Enemy,
}

/// The look of every part of one blueprint (blueprint frame), computed once per design.
#[derive(Clone, Debug)]
pub struct DesignVisual {
    pub parts: Vec<PartVisual>,
    /// Full-rectangle (non-slope) parts, for nav lights.
    pub rect_part: Vec<bool>,
    /// Logic cores (the static view finds the main piece with them).
    pub logic_part: Vec<bool>,
    pub enemy: bool,
    /// Hull edge / aura / plume / trail tint.
    pub glow: Color,
    /// The port navigation light (coral; starboard takes the glow).
    pub port_light: Color,
    /// Weapon colour (muzzle flashes, beams, pulses).
    pub weapon_color: Color,
}

pub fn design_visual(content: &Content, ship: &Blueprint, style: &Style) -> Result<DesignVisual, String> {
    let grid = content.ship_grid(ship)?;
    let tones = HullTones::new(content, ship, &grid);
    let enemy = content.factions.get(&ship.faction).map(|f| f.glow == "enemy").unwrap_or(false);
    let glow = if enemy { style.glow_enemy } else { style.glow_player };
    let mut parts = Vec::with_capacity(ship.parts.len());
    let mut rect_part = Vec::with_capacity(ship.parts.len());
    let mut logic_part = Vec::with_capacity(ship.parts.len());
    let mut weapon_color = style.system_color(if enemy { "enemy" } else { "weapons" });
    for (i, pl) in ship.parts.iter().enumerate() {
        let def = content.parts.get(&pl.part).ok_or_else(|| format!("unknown part `{}`", pl.part))?;
        let shape = grid.shape(i);
        let polygon: Vec<Vec2> = shape.polygon().iter().map(|g| Vec2::new(g.x as f32, g.y as f32)).collect();
        let [cx, cy] = shape.center();
        let (w, h) = shape.dims();
        let (fx, fy) = shape.facing().delta();
        let color_key = content.part_color(def);
        // Machines: the housing drawing on a cell in the hull's tone (GDR-0069); the icon is for UI tiles.
        let (fill, debug_color, icon) = if def.hull {
            let c = style.armor_tone(armor_tone(content, pl));
            (c, mix(c, Color::WHITE, 0.12), None)
        } else {
            let sys = style.system_color(color_key.unwrap_or("cargo"));
            (style.armor_tone(tones.under(&grid, &shape.cells())), mix(style.machine_tile, sys, 0.55), def.icon.clone())
        };
        let drawing = icon.as_deref().and_then(|k| content.ship_svg(k).map(|svg| (k, svg)));
        let overhang = drawing
            .and_then(|(_, svg)| data::content::svg::view_box(svg))
            .and_then(|vb| data::content::svg::ship_overhang(vb, def.size).ok())
            .unwrap_or(0.0);
        let gun = def.weapon.as_ref().map(|_| {
            let (barrel, muzzle_radius) = match def.barrel {
                Some(b) => (Some((b.width, b.length)), b.muzzle),
                None => (None, BARE_MUZZLE),
            };
            let color = style.system_color(color_key.unwrap_or("weapons"));
            weapon_color = color;
            // At rest: its line of fire, or its beam's through the prisms that relay it (GDR-0126).
            GunVisual { clear: content.beam_path(&ship.parts, &grid, i).clear(), barrel, muzzle_radius, color }
        });
        parts.push(PartVisual {
            index: i as u32,
            polygon,
            fill,
            debug_color,
            art: drawing.map(|(k, _)| k.to_string()),
            overhang,
            icon,
            center: Vec2::new(cx, cy),
            dims: Vec2::new(w as f32, h as f32),
            rot: pl.rot,
            flip: pl.flip,
            facing: Vec2::new(fx as f32, fy as f32),
            gun,
            shield_radius: def.shield.as_ref().map(|s| s.radius),
        });
        rect_part.push(def.shape == data::Shape::Rect);
        logic_part.push(def.logic.is_some());
    }
    Ok(DesignVisual { parts, rect_part, logic_part, enemy, glow, port_light: style.glow_enemy, weapon_color })
}

impl DesignVisual {
    /// The same parts with the hull glow, aura and trails of `look`.
    pub fn with_look(mut self, look: Look, style: &Style) -> DesignVisual {
        self.enemy = look == Look::Enemy;
        self.glow = if self.enemy { style.glow_enemy } else { style.glow_player };
        self
    }
}

/// The visual of one live piece: its alive parts, drawn in the sim's piece frame (pivot = `piece.origin`), with
/// the sim's line-of-fire flags and shield radii (bonuses included).
pub fn piece_visual(dv: &DesignVisual, ship: &Ship, piece: &Piece) -> ShipVisual {
    let mut parts = Vec::with_capacity(piece.parts.len());
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for &i in &piece.parts {
        let mut p = dv.parts[i as usize].clone();
        if let Some(g) = p.gun.as_mut() {
            g.clear = piece.guns.iter().find(|m| m.part == i).is_some_and(|m| m.clear);
        }
        if p.shield_radius.is_some() {
            p.shield_radius = Some(ship.stats(i).get(Stat::ShieldRadius));
        }
        for q in &p.polygon {
            min = min.min(*q);
            max = max.max(*q);
        }
        parts.push(p);
    }
    let hull_loops = data::design::geometry::hull_loops(&ship.piece_hull_edges(piece.id))
        .into_iter()
        .map(|l| (l.points.iter().map(|g| Vec2::new(g.x as f32, g.y as f32)).collect(), l.closed))
        .collect();

    // Navigation lights: only the controlling piece carries them.
    let nav_lights = if piece.controlled { nav_lights(dv, ship.grid(), &piece.parts) } else { Vec::new() };

    ShipVisual {
        parts,
        hull_loops,
        pivot: Vec2::new(piece.origin[0], piece.origin[1]),
        bounds_min: min,
        bounds_max: max,
        glow: dv.glow,
        enemy: dv.enemy,
        nav_lights,
        aura: true,
    }
}

/// A whole blueprint at rest, in the blueprint frame (pivot at the blueprint origin, so local = `(x, −y)`):
/// every part, with the tiles, icons and glows of flight. What the sim would decide is taken from the grid: a
/// gun's line of fire (the design's, [`Content::beam_path`]: through the prisms that relay it), the hull edge around all parts, the content's shield
/// radius, nav lights on the main piece (the one with a logic core). No aura: the drydock is not space.
pub fn blueprint_visual(dv: &DesignVisual, grid: &ShipGrid) -> ShipVisual {
    let mut parts = Vec::with_capacity(dv.parts.len());
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for p in &dv.parts {
        let p = p.clone();
        for q in &p.polygon {
            min = min.min(*q);
            max = max.max(*q);
        }
        parts.push(p);
    }
    if parts.is_empty() {
        (min, max) = (Vec2::ZERO, Vec2::ZERO);
    }
    let hull_loops = data::design::geometry::hull_loops(&grid.hull_edges(|_| true))
        .into_iter()
        .map(|l| (l.points.iter().map(|g| Vec2::new(g.x as f32, g.y as f32)).collect(), l.closed))
        .collect();
    let pieces = grid.pieces();
    let nav = grid
        .main_piece(&pieces, |i| dv.logic_part[i])
        .filter(|&m| pieces.parts[m].iter().any(|&i| dv.logic_part[i]))
        .map(|m| {
            let main: Vec<u32> = pieces.parts[m].iter().map(|&i| i as u32).collect();
            nav_lights(dv, grid, &main)
        })
        .unwrap_or_default();
    ShipVisual {
        parts,
        hull_loops,
        pivot: Vec2::ZERO,
        bounds_min: min,
        bounds_max: max,
        glow: dv.glow,
        enemy: dv.enemy,
        nav_lights: nav,
        aura: false,
    }
}

/// Navigation lights of player ships (as in the concept art): coral on the port wing tip, teal on starboard, at the
/// outer bottom corner of the outermost full cell of `parts`.
fn nav_lights(dv: &DesignVisual, grid: &ShipGrid, parts: &[u32]) -> Vec<(Vec2, Color)> {
    if dv.enemy {
        return Vec::new();
    }
    let mut best_l: Option<(i32, i32)> = None;
    let mut best_r: Option<(i32, i32)> = None;
    for &i in parts {
        if !dv.rect_part[i as usize] {
            continue;
        }
        for c in grid.shape(i as usize).cells() {
            if best_l.is_none_or(|b| (-c.0, c.1) > (-b.0, b.1)) {
                best_l = Some(c);
            }
            if best_r.is_none_or(|b| (c.0, c.1) > (b.0, b.1)) {
                best_r = Some(c);
            }
        }
    }
    match (best_l, best_r) {
        (Some(l), Some(r)) => vec![
            (Vec2::new(l.0 as f32 + 0.2, l.1 as f32 + 0.85), dv.port_light),
            (Vec2::new(r.0 as f32 + 0.8, r.1 as f32 + 0.85), dv.glow),
        ],
        _ => Vec::new(),
    }
}
