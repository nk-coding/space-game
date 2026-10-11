#![allow(dead_code)]

use std::sync::Arc;

use data::{Blueprint, Content, Placement};
use sim::{Body, PartIdx, ShipCommand, ShipControls, ShipDesign, ShipId, Sim, TickInputs};

pub fn content() -> Content {
    data::base_content()
}

pub fn design(c: &Content, id: &str) -> Arc<ShipDesign> {
    let bp = c.ships.get(id).unwrap_or_else(|| panic!("no ship {id}"));
    Arc::new(ShipDesign::compile(c, bp).unwrap())
}

/// A placement: `(part, x, y, rot, flip, material)`.
pub type P<'a> = (&'a str, i32, i32, u8, bool, Option<&'a str>);

/// A test blueprint from placements (frame = bounds).
pub fn blueprint(parts: &[P]) -> Blueprint {
    let w = parts.iter().map(|p| p.1 + 3).max().unwrap_or(1);
    let h = parts.iter().map(|p| p.2 + 3).max().unwrap_or(1);
    Blueprint {
        id: "test".into(),
        name: "Test".into(),
        summary: "Test ship.".into(),
        description: None,
        lore: None,
        tags: vec![],
        faction: "ship_mind".into(),
        size: (w as u32, h as u32),
        parts: parts
            .iter()
            .map(|&(part, x, y, rot, flip, material)| Placement {
                part: part.into(),
                x,
                y,
                rot,
                flip,
                material: material.map(Into::into),
                power: None,
            })
            .collect(),
        behavior: None,
        source: Default::default(),
    }
}

pub fn custom(c: &Content, parts: &[P]) -> Arc<ShipDesign> {
    Arc::new(ShipDesign::compile(c, &blueprint(parts)).unwrap())
}

/// Index of the placement of `part` at `(x, y)`.
pub fn find(d: &ShipDesign, part: &str, x: i32, y: i32) -> PartIdx {
    d.parts
        .iter()
        .position(|p| p.id == part && p.shape.x == x && p.shape.y == y)
        .unwrap_or_else(|| panic!("no {part} at {x},{y}")) as PartIdx
}

pub fn world_with(design: Arc<ShipDesign>) -> (Sim, ShipId) {
    let mut w = Sim::new(1);
    let s = w.spawn_ship(design, Body::default());
    (w, s)
}

/// Puts a ship under its faction's content behavior (`raider_basic` for automatons), loading the content's
/// behavior library first if the world has none (that drops every ship's behavior).
pub fn fight(w: &mut Sim, c: &Content, ship: ShipId) {
    if w.behaviors().id("raider_basic").is_none() {
        w.set_behaviors(sim::BehaviorLibrary::new(c).expect("base behaviors compile"));
    }
    let b = w.behaviors().default_for(w.ship(ship).unwrap().design());
    w.command(ship, ShipCommand::Behavior(b));
}

pub fn controls(engines: usize, throttle: f32, fire: bool, shields: bool) -> ShipControls {
    ShipControls {
        throttle: vec![throttle; engines],
        fire,
        guns: Vec::new(),
        shields,
    }
}

pub fn inputs(ship: ShipId, c: ShipControls) -> TickInputs {
    TickInputs {
        controls: vec![(ship, c)],
        ..Default::default()
    }
}

pub fn close(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}
