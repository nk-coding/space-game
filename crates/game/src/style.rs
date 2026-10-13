//! Typed view of the style tokens (`art/tokens.json`, embedded and flattened by the `art` crate).
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
    pub plume: PlumeStyle,
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
    pub shield: ShieldStyle,
    pub particle: Color,
    pub dotted_ring: Color,
    pub ui_label: Color,
    /// The UI accents and text colours (`ui.*` in the tokens).
    pub ui: UiPalette,
}

/// UI colours (`ui.*`, GDR-0008): teal primary, gold, coral (danger and enemies), violet (logic), grey labels,
/// dim labels, white numbers. Pass them through [`ui_color`] for bevy_ui nodes.
#[derive(Clone, Copy, Debug)]
pub struct UiPalette {
    pub teal: Color,
    pub gold: Color,
    pub coral: Color,
    pub violet: Color,
    pub label: Color,
    pub dim: Color,
    pub number: Color,
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
            plume: PlumeStyle {
                cruise_length: atlas.token_f32("ship.plume.cruise.length"),
                cruise_flicker: atlas.token_f32("ship.plume.cruise.flicker"),
                cruise_trail: atlas.token_f32("ship.plume.cruise.trail"),
                core: [atlas.token_color("ship.plume.cruise.core_color"), atlas.token_color("ship.plume.cruise.core_color_enemy")],
                core_opacity: atlas.token_f32("ship.plume.cruise.core_opacity"),
                core_width: atlas.token_f32("ship.plume.cruise.core_width"),
                core_length: atlas.token_f32("ship.plume.cruise.core_length"),
            },
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
            shield: ShieldStyle::from_atlas(atlas),
            particle: atlas.token_color("ui.particle"),
            dotted_ring: atlas.token_color("ui.dotted_ring"),
            ui_label: atlas.token_color("ui.label"),
            ui: UiPalette {
                teal: atlas.token_color("ui.teal"),
                gold: atlas.token_color("ui.gold"),
                coral: atlas.token_color("ui.coral"),
                violet: atlas.token_color("ui.violet"),
                label: atlas.token_color("ui.label"),
                dim: atlas.token_color("ui.dim"),
                number: atlas.token_color("ui.number"),
            },
        }
    }

    pub fn armor_tone(&self, key: &str) -> Color {
        self.armor.get(key).copied().unwrap_or(self.armor["steel"])
    }

    pub fn system_color(&self, key: &str) -> Color {
        self.system.get(key).copied().unwrap_or(self.ui_label)
    }
}

/// Shield bubble look (`ship.shield_bubble.*`, GDR-0043). Widths in cells (= metres), times in seconds.
#[derive(Clone, Debug)]
pub struct ShieldStyle {
    pub color: Color,
    /// Rim line core and hit flashes.
    pub core: Color,
    pub fill_center_opacity: f32,
    pub fill_rim_opacity: f32,
    pub rim_opacity: f32,
    pub rim_width: f32,
    pub glow_opacity: f32,
    pub glow_width: f32,
    /// Depth of the shimmer (0 = none).
    pub shimmer: f32,
    pub shimmer_speed: f32,
    /// Share of capacity below which the bubble flickers.
    pub low_share: f32,
    pub hit_seconds: f32,
    pub collapse_seconds: f32,
}

impl ShieldStyle {
    fn from_atlas(atlas: &Atlas) -> Self {
        let f = |k: &str| atlas.token_f32(&format!("ship.shield_bubble.{k}"));
        Self {
            color: atlas.token_color("ship.shield_bubble.color"),
            core: atlas.token_color("ship.shield_bubble.core_color"),
            fill_center_opacity: f("fill_center_opacity"),
            fill_rim_opacity: f("fill_rim_opacity"),
            rim_opacity: f("rim_opacity"),
            rim_width: f("rim_width_cells"),
            glow_opacity: f("glow_opacity"),
            glow_width: f("glow_width_cells"),
            shimmer: f("shimmer"),
            shimmer_speed: f("shimmer_speed_cells"),
            low_share: f("low_share"),
            hit_seconds: f("hit_seconds"),
            collapse_seconds: f("collapse_seconds"),
        }
    }
}

/// The calm cruise flame of an engine holding a speed (`ship.plume.cruise` in the tokens, GDR-0041). Shares
/// are of the burning flame at the same level.
#[derive(Clone, Debug)]
pub struct PlumeStyle {
    /// Flame length while cruising.
    pub cruise_length: f32,
    /// Flicker amplitude while cruising (steadier).
    pub cruise_flicker: f32,
    /// Trail particle rate while cruising.
    pub cruise_trail: f32,
    /// Hot core colour: player (teal) and enemy (coral) plumes.
    pub core: [Color; 2],
    pub core_opacity: f32,
    /// Core width, share of the plume's width.
    pub core_width: f32,
    /// Core length, share of the flame's length.
    pub core_length: f32,
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

/// `a` → `b` by `t` (clamped to 0..1) in sRGB, the colour space of the tiles and the UI; alpha too.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    let t = t.clamp(0.0, 1.0);
    Color::srgba(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t, a.alpha + (b.alpha - a.alpha) * t)
}
