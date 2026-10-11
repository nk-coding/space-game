//! Visual descriptions of ships: what each part looks like ([`DesignVisual`], once per blueprint, from `data`
//! content) and what one live piece looks like ([`piece_visual`], from the `sim` state of that piece).
//!
//! Visual interpretation only (icon, tile tone, barrel sizes per gun art, nav lights). Shape and topology come
//! from `data::geometry` and the sim: polygons, the piece's alive parts and frame origin, hull edges, line of
//! fire, shield radius.

use bevy::prelude::*;
use data::{Blueprint, Content};
use sim::{Piece, Ship, Stat};

use crate::ship_view::{GunVisual, PartVisual, ShipVisual};
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

/// The look of every part of one blueprint (blueprint frame), computed once per design.
#[derive(Clone, Debug)]
pub struct DesignVisual {
    pub id: String,
    pub name: String,
    pub parts: Vec<PartVisual>,
    /// Full-rectangle (non-slope) parts, for nav lights.
    pub rect_part: Vec<bool>,
    pub enemy: bool,
    /// Hull edge / aura / plume / trail tint.
    pub glow: Color,
    /// Weapon colour (muzzle flashes, beams, pulses).
    pub weapon_color: Color,
}

pub fn design_visual(content: &Content, ship: &Blueprint, style: &Style) -> Result<DesignVisual, String> {
    let grid = content.ship_grid(ship)?;
    let enemy = content.factions.get(&ship.faction).map(|f| f.glow == "enemy").unwrap_or(false);
    let glow = if enemy { style.glow_enemy } else { style.glow_player };
    let mut parts = Vec::with_capacity(ship.parts.len());
    let mut rect_part = Vec::with_capacity(ship.parts.len());
    let mut weapon_color = style.system_color(if enemy { "enemy" } else { "weapons" });
    for (i, pl) in ship.parts.iter().enumerate() {
        let def = content.parts.get(&pl.part).ok_or_else(|| format!("unknown part `{}`", pl.part))?;
        let shape = grid.shape(i);
        let polygon: Vec<Vec2> = shape.polygon().iter().map(|g| Vec2::new(g.x as f32, g.y as f32)).collect();
        let [cx, cy] = shape.center();
        let (w, h) = shape.dims();
        let (fx, fy) = shape.facing().delta();
        let material = content.placement_material(pl);
        let color_key = content.part_color(def);
        let (fill, debug_color, icon) = if def.hull {
            let tone = material.and_then(|m| m.armor.as_ref()).map(|a| a.tone.as_str()).unwrap_or("steel");
            let c = style.armor_tone(tone);
            (c, mix(c, Color::WHITE, 0.12), None)
        } else {
            let sys = style.system_color(color_key.unwrap_or("cargo"));
            (style.machine_tile, mix(style.machine_tile, sys, 0.55), def.icon.clone())
        };
        let gun = def.weapon.as_ref().map(|_| {
            let (barrel, muzzle_radius) = gun_art(def.icon.as_deref());
            let color = style.system_color(color_key.unwrap_or("weapons"));
            weapon_color = color;
            GunVisual { clear: true, barrel, muzzle_radius, color }
        });
        parts.push(PartVisual {
            index: i as u32,
            polygon,
            fill,
            debug_color,
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
    }
    Ok(DesignVisual { id: ship.id.clone(), name: ship.name.clone(), parts, rect_part, enemy, glow, weapon_color })
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
    let hull_loops = data::geometry::hull_loops(&ship.piece_hull_edges(piece.id))
        .into_iter()
        .map(|l| (l.points.iter().map(|g| Vec2::new(g.x as f32, g.y as f32)).collect(), l.closed))
        .collect();

    // Navigation lights on player ships (halvex-runner.svg): coral on the port wing tip, teal on starboard, at
    // the outer bottom corner of the outermost full cell. Only the controlling piece carries them.
    let mut nav_lights = Vec::new();
    if !dv.enemy && piece.controlled {
        let mut best_l: Option<(i32, i32)> = None;
        let mut best_r: Option<(i32, i32)> = None;
        for &i in &piece.parts {
            if !dv.rect_part[i as usize] {
                continue;
            }
            for c in ship.grid().shape(i as usize).cells() {
                if best_l.is_none_or(|b| (-c.0, c.1) > (-b.0, b.1)) {
                    best_l = Some(c);
                }
                if best_r.is_none_or(|b| (c.0, c.1) > (b.0, b.1)) {
                    best_r = Some(c);
                }
            }
        }
        if let (Some(l), Some(r)) = (best_l, best_r) {
            nav_lights.push((Vec2::new(l.0 as f32 + 0.2, l.1 as f32 + 0.85), Color::srgb_u8(0xec, 0x8f, 0x7c)));
            nav_lights.push((Vec2::new(r.0 as f32 + 0.8, r.1 as f32 + 0.85), dv.glow));
        }
    }

    ShipVisual {
        parts,
        hull_loops,
        pivot: Vec2::new(piece.origin[0], piece.origin[1]),
        bounds_min: min,
        bounds_max: max,
        glow: dv.glow,
        enemy: dv.enemy,
        nav_lights,
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    Color::srgb(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t)
}
