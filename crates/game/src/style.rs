//! Typed view of the style tokens (`art/tokens.json`, flattened into the atlas index by `atlas-tool`).
//! Client-only: the simulation never sees colours.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::atlas::Atlas;

#[derive(Resource, Clone, Debug)]
pub struct Style {
    pub machine_tile: Color,
    pub seam: Color,
    /// Seam width on each tile edge, in cells.
    pub seam_width: f32,
    /// Armour tone key (`ship.armor.<key>`) to tile colour.
    pub armor: HashMap<String, Color>,
    /// System colour key (`ship.system_colors.<key>`) to colour.
    pub system: HashMap<String, Color>,
    pub glow_player: Color,
    pub glow_enemy: Color,
    pub halo_opacity: f32,
    pub line_width: f32,
    pub line_opacity: f32,
    pub aura_opacity: f32,
    pub shield_color: Color,
    pub shield_opacity: f32,
    pub muzzle: Color,
    pub particle: Color,
    pub dotted_ring: Color,
    pub ui_label: Color,
    pub ui_dim: Color,
    pub ui_number: Color,
    pub ui_teal: Color,
}

impl Style {
    pub fn from_atlas(atlas: &Atlas) -> Self {
        let group = |prefix: &str, keys: &[&str]| -> HashMap<String, Color> {
            keys.iter().map(|k| (k.to_string(), atlas.token_color(&format!("{prefix}.{k}")))).collect()
        };
        Self {
            machine_tile: atlas.token_color("ship.machine_tile"),
            seam: atlas.token_color("ship.seam"),
            seam_width: atlas.token_f32("ship.seam_width_cells"),
            armor: group("ship.armor", &["steel", "composite", "reactive", "enemy"]),
            system: group(
                "ship.system_colors",
                &["power", "weapons", "shields", "heat", "propulsion", "logic", "cargo", "enemy"],
            ),
            glow_player: atlas.token_color("ship.hull_glow.color"),
            glow_enemy: atlas.token_color("ship.hull_glow.enemy_color"),
            halo_opacity: atlas.token_f32("ship.hull_glow.halo_opacity"),
            line_width: atlas.token_f32("ship.hull_glow.line_width_cells"),
            line_opacity: atlas.token_f32("ship.hull_glow.line_opacity"),
            aura_opacity: atlas.token_f32("ship.aura.opacity_center"),
            shield_color: atlas.token_color("ship.shield_ring.color"),
            shield_opacity: atlas.token_f32("ship.shield_ring.opacity"),
            muzzle: atlas.token_color("ship.muzzle"),
            particle: atlas.token_color("ui.particle"),
            dotted_ring: atlas.token_color("ui.dotted_ring"),
            ui_label: atlas.token_color("ui.label"),
            ui_dim: atlas.token_color("ui.dim"),
            ui_number: atlas.token_color("ui.number"),
            ui_teal: atlas.token_color("ui.teal"),
        }
    }

    pub fn armor_tone(&self, key: &str) -> Color {
        self.armor.get(key).copied().unwrap_or(self.armor["steel"])
    }

    pub fn system_color(&self, key: &str) -> Color {
        self.system.get(key).copied().unwrap_or(self.ui_label)
    }
}

/// Linear RGBA floats for a vertex colour, with an extra opacity factor.
pub fn vcol(c: Color, opacity: f32) -> [f32; 4] {
    let l = c.to_linear();
    [l.red, l.green, l.blue, l.alpha * opacity]
}

/// Colour for bevy_ui nodes. The world camera composites in sRGB space (like the browser and the concept
/// art) and bevy_ui does not encode for that space, so UI colours are passed pre-encoded.
pub fn ui_color(c: Color, opacity: f32) -> Color {
    let s = c.to_srgba();
    Color::LinearRgba(LinearRgba::new(s.red, s.green, s.blue, s.alpha * opacity))
}
