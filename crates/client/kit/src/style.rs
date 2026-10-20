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
    /// The backdrop's twinkling stars.
    pub star: Color,
    pub dotted_ring: Color,
    /// The space backdrop (`ui.background`).
    pub background: RadialGradient,
    pub ui_label: Color,
    /// The UI accents and text colours (`ui.*` in the tokens).
    pub ui: UiPalette,
    /// The voyage map's bands, open and sealed (`map.bands.*`).
    pub bands: BandStyle,
}

/// The voyage map's look for open and sealed bands (`map.bands.*` in the tokens, GDR-0097): a tone and opacities
/// for each, edge widths, the sealed edge's dashes and its locks (galaxy px).
#[derive(Clone, Copy, Debug)]
pub struct BandStyle {
    pub open: Color,
    pub open_fill: f32,
    pub open_edge: f32,
    pub open_edge_width: f32,
    pub open_veil: f32,
    pub sealed: Color,
    pub sealed_tint: f32,
    pub sealed_edge: f32,
    pub sealed_edge_width: f32,
    pub sealed_dash: f32,
    pub sealed_gap: f32,
    pub sealed_veil: f32,
    /// The edges between sealed bands.
    pub deep_edge: f32,
    pub lock: f32,
    pub lock_size: f32,
}

impl BandStyle {
    fn from_atlas(atlas: &Atlas) -> Self {
        let f = |k: &str| atlas.token_f32(&format!("map.bands.{k}"));
        Self {
            open: atlas.token_color("map.bands.open"),
            open_fill: f("open_fill"),
            open_edge: f("open_edge"),
            open_edge_width: f("open_edge_width"),
            open_veil: f("open_veil"),
            sealed: atlas.token_color("map.bands.sealed"),
            sealed_tint: f("sealed_tint"),
            sealed_edge: f("sealed_edge"),
            sealed_edge_width: f("sealed_edge_width"),
            sealed_dash: f("sealed_dash"),
            sealed_gap: f("sealed_gap"),
            sealed_veil: f("sealed_veil"),
            deep_edge: f("deep_edge"),
            lock: f("lock"),
            lock_size: f("lock_size"),
        }
    }
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
            armor: group("ship.armor", &["steel", "composite", "reactive", "enemy", "tissue", "chitin", "crystal"]),
            system: group(
                "ship.system_colors",
                &["power", "weapons", "shields", "heat", "propulsion", "logic", "cargo", "enemy", "swarm", "phase", "lattice"],
            ),
            glow_player: atlas.token_color("ship.hull_glow.color"),
            glow_enemy: atlas.token_color("ship.hull_glow.enemy_color"),
            halo_opacity: atlas.token_f32("ship.hull_glow.halo_opacity"),
            line_width: atlas.token_f32("ship.hull_glow.line_width_cells"),
            line_opacity: atlas.token_f32("ship.hull_glow.line_opacity"),
            aura_opacity: atlas.token_f32("ship.aura.opacity_center"),
            shield: ShieldStyle::from_atlas(atlas),
            particle: atlas.token_color("ui.particle"),
            star: atlas.token_color("ui.star"),
            dotted_ring: atlas.token_color("ui.dotted_ring"),
            background: RadialGradient::parse(atlas.token("ui.background"))
                .unwrap_or_else(|e| panic!("style token `ui.background`: {e}")),
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
            bands: BandStyle::from_atlas(atlas),
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

/// A CSS `radial-gradient(ellipse RX% RY% at CX% CY%, colour [stop%], …)`, the form of `ui.background`.
#[derive(Clone, Debug, PartialEq)]
pub struct RadialGradient {
    /// The ellipse's radii as shares of the width and height.
    pub radii: Vec2,
    /// Its centre as shares of the width and height, from the top left.
    pub centre: Vec2,
    /// Colour stops by their share of the radius (ascending; the last one fills the rest).
    pub stops: Vec<(f32, Color)>,
}

impl RadialGradient {
    pub fn parse(css: &str) -> Result<Self, String> {
        let inner = css
            .trim()
            .strip_prefix("radial-gradient(")
            .and_then(|r| r.strip_suffix(')'))
            .ok_or_else(|| format!("not a radial-gradient(…): `{css}`"))?;
        let mut parts = inner.split(',').map(str::trim);
        let shape: Vec<&str> = parts.next().unwrap_or_default().split_whitespace().collect();
        let share = |t: &str| t.strip_suffix('%').and_then(|n| n.parse::<f32>().ok()).map(|n| n / 100.0);
        let [_ellipse, rx, ry, _at, cx, cy] = shape[..] else {
            return Err(format!("expected `ellipse RX% RY% at CX% CY%`, got `{}`", shape.join(" ")));
        };
        let (Some(rx), Some(ry), Some(cx), Some(cy)) = (share(rx), share(ry), share(cx), share(cy)) else {
            return Err(format!("expected percentages in `{}`", shape.join(" ")));
        };
        let mut stops: Vec<(Option<f32>, Color)> = Vec::new();
        for stop in parts {
            let mut words = stop.split_whitespace();
            let hex = words.next().unwrap_or_default();
            let color = Srgba::hex(hex).map_err(|e| format!("colour `{hex}`: {e}"))?;
            let at = match words.next() {
                Some(p) => Some(share(p).ok_or_else(|| format!("stop position `{p}`"))?),
                None => None,
            };
            stops.push((at, color.into()));
        }
        if stops.len() < 2 {
            return Err("fewer than two colour stops".into());
        }
        // CSS: the first stop defaults to 0, the last to 100 %, the ones between spread evenly between their
        // neighbours.
        let n = stops.len();
        stops[0].0.get_or_insert(0.0);
        stops[n - 1].0.get_or_insert(1.0);
        let mut k = 1;
        while k < n - 1 {
            if stops[k].0.is_some() {
                k += 1;
                continue;
            }
            let end = (k..n).find(|&j| stops[j].0.is_some()).unwrap_or(n - 1);
            let (a, b) = (stops[k - 1].0.unwrap_or(0.0), stops[end].0.unwrap_or(1.0));
            let span = (end - k + 1) as f32;
            for (i, stop) in stops[k..end].iter_mut().enumerate() {
                stop.0 = Some(a + (b - a) * (i + 1) as f32 / span);
            }
            k = end;
        }
        Ok(Self {
            radii: Vec2::new(rx, ry),
            centre: Vec2::new(cx, cy),
            stops: stops.into_iter().map(|(at, c)| (at.unwrap_or(0.0), c)).collect(),
        })
    }

    /// The colour at a share `t` of the radius.
    pub fn at(&self, t: f32) -> Color {
        let first = self.stops[0];
        if t <= first.0 {
            return first.1;
        }
        for w in self.stops.windows(2) {
            if t <= w[1].0 {
                let k = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
                let (a, b) = (w[0].1.to_srgba(), w[1].1.to_srgba());
                return Color::srgb(a.red + (b.red - a.red) * k, a.green + (b.green - a.green) * k, a.blue + (b.blue - a.blue) * k);
            }
        }
        self.outer()
    }

    /// The colour outside the last stop.
    pub fn outer(&self) -> Color {
        self.stops[self.stops.len() - 1].1
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

/// `a` → `b` by `t` (clamped to 0..1) in sRGB, the colour space of the tiles and the UI; alpha too.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    let t = t.clamp(0.0, 1.0);
    Color::srgba(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t, a.alpha + (b.alpha - a.alpha) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backdrop_token_parses_as_a_radial_gradient() {
        let g = RadialGradient::parse("radial-gradient(ellipse 75% 85% at 50% 45%, #0a1820, #050e15 55%, #010409)").unwrap();
        assert_eq!((g.radii, g.centre), (Vec2::new(0.75, 0.85), Vec2::new(0.5, 0.45)));
        let hex = |c: Color| c.to_srgba().to_hex();
        let stops: Vec<(f32, String)> = g.stops.iter().map(|&(t, c)| (t, hex(c))).collect();
        assert_eq!(stops, [(0.0, "#0A1820".into()), (0.55, "#050E15".into()), (1.0, "#010409".into())]);
        assert_eq!(hex(g.at(0.55)), "#050E15");
        assert_eq!(hex(g.at(2.0)), "#010409");
        assert_eq!(hex(g.outer()), "#010409");
        // Stops without a position spread evenly between their neighbours.
        let even = RadialGradient::parse("radial-gradient(ellipse 50% 50% at 50% 50%, #000, #111, #222, #fff 90%)").unwrap();
        for (got, want) in even.stops.iter().map(|s| s.0).zip([0.0, 0.3, 0.6, 0.9]) {
            assert!((got - want).abs() < 1e-6, "{got} vs {want}");
        }
        assert!(RadialGradient::parse("linear-gradient(#000, #fff)").is_err());
        assert!(RadialGradient::parse("radial-gradient(ellipse 75% 85% at 50% 45%, #000)").is_err());
    }
}
