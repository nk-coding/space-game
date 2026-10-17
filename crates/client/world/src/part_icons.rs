//! Part and ship icons in UI nodes ([`UiIcon::Part`], [`UiIcon::Ship`]): drawn with the flight tiles, by the same
//! code as the ships (`ship_model`, `ship_view::build_body`). The content implements `ui_icons::IconShapes`, so the
//! icons (`ui_icons`) know nothing about ships.

use bevy::prelude::*;
use data::{Blueprint, Placement};
use kit::atlas::Atlas;
use kit::style::Style;
use kit::ui_icons::{IconShapes, UiIcon};

use crate::ship_model::{blueprint_visual, design_visual};
use crate::ship_view::{ShipVisual, build_body};
use crate::sim_bridge::GameContent;

impl IconShapes for GameContent {
    fn shape(&self, icon: &UiIcon, style: &Style, atlas: &Atlas, glow: bool) -> Option<(Mesh, Vec2)> {
        match icon {
            UiIcon::Part { part, material } => part_tile(&self.0, style, atlas, part, material.as_deref(), glow),
            UiIcon::Ship(bp) => ship_icon(&self.0, style, atlas, bp, glow),
            UiIcon::Glyph { .. } | UiIcon::Scrim { .. } | UiIcon::Sketch(_) => None,
        }
    }
}

/// One part as a single tile, centred on the origin in cells; and its footprint. No hull edge, aura, barrel or
/// muzzle: an icon, not a ship. A machine shows its icon on the machine tile colour (on ships it is its drawing on
/// a hull-toned cell, GDR-0069).
fn part_tile(
    content: &data::Content,
    style: &Style,
    atlas: &Atlas,
    part: &str,
    material: Option<&str>,
    glow: bool,
) -> Option<(Mesh, Vec2)> {
    let def = content.parts.get(part)?;
    let bp = Blueprint {
        id: String::new(),
        name: String::new(),
        summary: String::new(),
        description: None,
        lore: None,
        tags: Vec::new(),
        faction: String::new(),
        size: def.size,
        parts: vec![Placement {
            part: part.into(),
            x: 0,
            y: 0,
            rot: 0,
            flip: false,
            material: material.filter(|_| def.hull).map(str::to_string),
            power: None,
            group: None,
        }],
        behavior: None,
        assists: None,
        origin: None,
        source: Default::default(),
    };
    let dv = design_visual(content, &bp, style).ok()?;
    let mut p = dv.parts.into_iter().next()?;
    p.gun = None;
    if !def.hull {
        (p.fill, p.art, p.overhang) = (style.machine_tile, None, 0.0);
    }
    let dims = Vec2::new(def.size.0 as f32, def.size.1 as f32);
    let v = ShipVisual {
        parts: vec![p],
        hull_loops: Vec::new(),
        pivot: dims / 2.0,
        bounds_min: Vec2::ZERO,
        bounds_max: dims,
        glow: dv.glow,
        enemy: false,
        nav_lights: Vec::new(),
        aura: false,
    };
    Some((build_body(&v, atlas, style, glow, true), dims))
}

/// A whole design as the drydock draws it (`blueprint_visual`), centred on the origin in cells; and its extent.
fn ship_icon(content: &data::Content, style: &Style, atlas: &Atlas, bp: &Blueprint, glow: bool) -> Option<(Mesh, Vec2)> {
    let dv = design_visual(content, bp, style).ok()?;
    let grid = content.ship_grid(bp).ok()?;
    let mut v = blueprint_visual(&dv, &grid);
    if v.parts.is_empty() {
        return None;
    }
    v.pivot = (v.bounds_min + v.bounds_max) / 2.0;
    Some((build_body(&v, atlas, style, glow, true), v.bounds_max - v.bounds_min))
}
