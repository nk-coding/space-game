//! Visual descriptions of ships: what each part looks like ([`DesignVisual`], once per blueprint, from `data`
//! content), what one live piece looks like ([`piece_visual`], from the `sim` state of that piece) and what a
//! whole blueprint looks like at rest ([`blueprint_visual`], the drydock: every part, from the ship grid alone).
//!
//! Visual interpretation only (ship drawing and icon, tile tone, barrels from the part's content, nav lights). Shape
//! and topology come from `data::design::geometry` and the sim: polygons, the piece's alive parts and frame origin, hull
//! edges, line of fire, shield radius.
//!
//! Machines are drawn as housings on a hull-toned cell (GDR-0069): [`HullTones`] picks the cell's armour tone.

use std::collections::BTreeMap;

use bevy::prelude::*;
use data::{Blueprint, Cell, Content, Dir, Placement, ShipGrid};
use sim::{Piece, Ship, Stat};
use kit::style::{Style, mix};

use crate::ship_view::{GunVisual, PartVisual, ShipVisual};

/// The muzzle glow of a gun without a barrel (its content has no `barrel`), at its front edge, in cells.
const BARE_MUZZLE: f32 = 0.12;
/// The armour tone of a hull part without a material, and of the cells under machines in a design without hull.
const DEFAULT_TONE: &str = "steel";

/// The armour tone the cell under a machine takes (GDR-0069), from what touches it: the tone with the most
/// `touching` hull cell sides wins, a tie going to the tone with more hull `area` in the design (then to the first
/// in name order); a machine that touches no hull takes the design's most used tone (by area; a tie goes to the
/// first in name order). `None` when the design has no hull at all.
pub fn hull_tone<'a>(touching: &BTreeMap<&'a str, u32>, area: &BTreeMap<&'a str, f32>) -> Option<&'a str> {
    let share = |t: &str| area.get(t).copied().unwrap_or(0.0);
    let mut best: Option<(&'a str, u32, f32)> = None;
    for (&t, &n) in touching {
        if best.is_none_or(|(_, bn, ba)| n > bn || (n == bn && share(t) > ba)) {
            best = Some((t, n, share(t)));
        }
    }
    if let Some((t, ..)) = best {
        return Some(t);
    }
    let mut top: Option<(&'a str, f32)> = None;
    for (&t, &a) in area {
        if top.is_none_or(|(_, ta)| a > ta) {
            top = Some((t, a));
        }
    }
    top.map(|(t, _)| t)
}

/// The armour tones of one design's hull, to find the tone under its machines ([`hull_tone`]).
pub struct HullTones<'a> {
    /// Per part: its armour tone (hull parts) or `None` (machines).
    part: Vec<Option<&'a str>>,
    /// Hull area per tone, in cells.
    area: BTreeMap<&'a str, f32>,
}

impl<'a> HullTones<'a> {
    pub fn new(content: &'a Content, ship: &'a Blueprint, grid: &ShipGrid) -> Self {
        let mut area = BTreeMap::new();
        let part = ship
            .parts
            .iter()
            .enumerate()
            .map(|(i, pl)| {
                let tone = content.parts.get(&pl.part).filter(|d| d.hull).map(|_| armor_tone(content, pl));
                if let Some(t) = tone {
                    *area.entry(t).or_insert(0.0) += grid.shape(i).area();
                }
                tone
            })
            .collect();
        Self { part, area }
    }

    /// The tone of the cell under a machine on `cells` of `grid` (the design's grid; cells of the machine itself
    /// are skipped, so a part being placed over others works too).
    pub fn under(&self, grid: &ShipGrid, cells: &[Cell]) -> &'a str {
        let mut touching = BTreeMap::new();
        for &(x, y) in cells {
            for d in Dir::ALL {
                let (dx, dy) = d.delta();
                let n = (x + dx, y + dy);
                if cells.contains(&n) {
                    continue;
                }
                if let Some(t) = grid.occupant(n).and_then(|j| self.part.get(j).copied().flatten()) {
                    *touching.entry(t).or_insert(0) += 1;
                }
            }
        }
        hull_tone(&touching, &self.area).unwrap_or(DEFAULT_TONE)
    }
}

/// The armour tone key of a hull placement (its material's).
fn armor_tone<'a>(content: &'a Content, pl: &Placement) -> &'a str {
    content.placement_material(pl).and_then(|m| m.armor.as_ref()).map_or(DEFAULT_TONE, |a| a.tone.as_str())
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
            GunVisual { clear: true, barrel, muzzle_radius, color }
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
/// gun's line of fire ([`ShipGrid::line_of_fire_clear`]), the hull edge around all parts, the content's shield
/// radius, nav lights on the main piece (the one with a logic core). No aura: the drydock is not space.
pub fn blueprint_visual(dv: &DesignVisual, grid: &ShipGrid) -> ShipVisual {
    let mut parts = Vec::with_capacity(dv.parts.len());
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for (i, p) in dv.parts.iter().enumerate() {
        let mut p = p.clone();
        if let Some(g) = p.gun.as_mut() {
            g.clear = grid.line_of_fire_clear(i);
        }
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

/// Navigation lights of player ships (halvex-runner.svg): coral on the port wing tip, teal on starboard, at the
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

#[cfg(test)]
mod tests {
    use super::*;

    /// GDR-0069's hull tone: the armour touching the machine first, the design's main tone without any.
    #[test]
    fn hull_tone_follows_the_touching_armour_then_the_design() {
        let sides = |v: &[(&'static str, u32)]| v.iter().copied().collect::<BTreeMap<_, _>>();
        let area = |v: &[(&'static str, f32)]| v.iter().copied().collect::<BTreeMap<_, _>>();
        let design = area(&[("composite", 10.0), ("steel", 4.0)]);
        // The most touching sides win, whatever the design uses most.
        assert_eq!(hull_tone(&sides(&[("steel", 3), ("composite", 1)]), &design), Some("steel"));
        // A tie goes to the tone the design uses more, then to name order.
        assert_eq!(hull_tone(&sides(&[("steel", 2), ("composite", 2)]), &design), Some("composite"));
        assert_eq!(hull_tone(&sides(&[("steel", 1), ("reactive", 1)]), &area(&[])), Some("reactive"));
        // Nothing touching: the design's most used tone (a tie: name order); no hull at all: none.
        assert_eq!(hull_tone(&sides(&[]), &design), Some("composite"));
        assert_eq!(hull_tone(&sides(&[]), &area(&[("steel", 2.0), ("reactive", 2.0)])), Some("reactive"));
        assert_eq!(hull_tone(&sides(&[]), &area(&[])), None);
    }

    /// On a design: a logic core with composite on two sides and steel on one sits on composite; a gyro touching
    /// no hull takes the design's main tone (steel, three cells against two).
    #[test]
    fn machines_take_the_tone_of_the_hull_around_them() {
        let content = data::base_content();
        let at = |part: &str, x: i32, y: i32, material: Option<&str>| Placement {
            part: part.into(),
            x,
            y,
            rot: 0,
            flip: false,
            material: material.map(str::to_string),
            power: None,
        };
        let ship = Blueprint {
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
            source: Default::default(),
        };
        let grid = content.ship_grid(&ship).unwrap();
        let tones = HullTones::new(&content, &ship, &grid);
        assert_eq!(tones.under(&grid, &grid.shape(1).cells()), "composite");
        assert_eq!(tones.under(&grid, &grid.shape(6).cells()), "steel");
        // A part about to be placed (the drydock's ghost) is judged the same way, its own cells skipped.
        assert_eq!(tones.under(&grid, &[(3, 1)]), "steel");
    }
}
