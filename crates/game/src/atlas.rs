//! The baked texture atlas and the style tokens that come with it.
//!
//! `crates/atlas-tool` rasterises `art/icons/*.icon-only.svg` (crisp glyphs and their glow layers), the FX
//! textures (plumes, halo profile, dots, aura, backdrop) and flattens `art/tokens.json` into
//! `assets/atlas/atlas.png` + `assets/atlas/atlas.txt`. Both are embedded in the binary: the atlas is needed
//! before the first frame, and embedding avoids a loading state and asset-meta requests on the web.
//!
//! Everything a ship draws samples this one texture, so a whole ship body is a single draw call.

use bevy::asset::RenderAssetUsages;
use bevy::image::{
    CompressedImageFormats, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor, ImageType,
};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::sprite_render::AlphaMode2d;

static ATLAS_PNG: &[u8] = include_bytes!("../assets/atlas/atlas.png");
static ATLAS_TXT: &str = include_str!("../assets/atlas/atlas.txt");

/// Mip levels generated at startup (the atlas height is a multiple of 64, so 6 levels stay integral).
const MIP_LEVELS: u32 = 6;

/// One region of the atlas.
#[derive(Clone, Copy, Debug)]
pub struct Region {
    /// Pixel rectangle (for `Sprite::rect`).
    pub px: Rect,
    /// Normalised UV rectangle.
    pub uv: Rect,
    /// Size of the art in ship cells (1 cell = 1 m).
    pub cells: Vec2,
    /// Anchor point (nozzle, muzzle, centre) in cells from the region's top-left corner.
    pub origin: Vec2,
}

impl Region {
    /// UV at a fractional position inside the region (0..1 from the top-left).
    pub fn uv_at(&self, f: Vec2) -> Vec2 {
        self.uv.min + (self.uv.max - self.uv.min) * f
    }
    /// UV at the region's centre (used to sample flat white).
    pub fn uv_center(&self) -> Vec2 {
        self.uv.center()
    }
    /// The sprite anchor (Bevy convention: -0.5..0.5, y up) that puts the region's origin at the transform.
    pub fn anchor(&self) -> Vec2 {
        Vec2::new(self.origin.x / self.cells.x - 0.5, 0.5 - self.origin.y / self.cells.y)
    }
}

#[derive(Resource)]
pub struct Atlas {
    pub image: Handle<Image>,
    /// Shared material for every atlas mesh (vertex colours tint the texture).
    pub material: Handle<ColorMaterial>,
    regions: HashMap<String, Region>,
    tokens: HashMap<String, String>,
}

impl Atlas {
    pub fn get(&self, name: &str) -> Option<&Region> {
        self.regions.get(name)
    }
    /// A region that must exist (FX regions the tool always writes).
    pub fn region(&self, name: &str) -> Region {
        *self.regions.get(name).unwrap_or_else(|| panic!("atlas region `{name}` missing; re-run atlas-tool"))
    }
    pub fn token(&self, path: &str) -> &str {
        self.tokens.get(path).unwrap_or_else(|| panic!("style token `{path}` missing; re-run atlas-tool"))
    }
    pub fn token_f32(&self, path: &str) -> f32 {
        self.token(path).parse().unwrap_or_else(|_| panic!("style token `{path}` is not a number"))
    }
    pub fn token_color(&self, path: &str) -> Color {
        parse_hex(self.token(path)).unwrap_or_else(|| panic!("style token `{path}` is not a #rrggbb colour"))
    }
}

pub fn parse_hex(s: &str) -> Option<Color> {
    Srgba::hex(s.trim()).ok().map(Color::from)
}

pub struct AtlasPlugin;

impl Plugin for AtlasPlugin {
    fn build(&self, app: &mut App) {
        let image = build_image();
        let (regions, tokens) = parse_index(ATLAS_TXT, image.width() as f32, image.height() as f32);
        let world = app.world_mut();
        let image = world.resource_mut::<Assets<Image>>().add(image);
        let material = world.resource_mut::<Assets<ColorMaterial>>().add(ColorMaterial {
            color: Color::WHITE,
            alpha_mode: AlphaMode2d::Blend,
            texture: Some(image.clone()),
            ..default()
        });
        let atlas = Atlas { image, material, regions, tokens };
        world.insert_resource(crate::style::Style::from_atlas(&atlas));
        world.insert_resource(atlas);
    }
}

fn parse_index(src: &str, w: f32, h: f32) -> (HashMap<String, Region>, HashMap<String, String>) {
    let mut regions = HashMap::default();
    let mut tokens = HashMap::default();
    for line in src.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("region ") {
            let mut it = rest.split_whitespace();
            let name = it.next().expect("region name").to_string();
            let n: Vec<f32> = it.map(|v| v.parse().expect("region number")).collect();
            assert_eq!(n.len(), 8, "bad region line: {line}");
            let px = Rect::new(n[0], n[1], n[0] + n[2], n[1] + n[3]);
            let uv = Rect::new(px.min.x / w, px.min.y / h, px.max.x / w, px.max.y / h);
            regions.insert(name, Region { px, uv, cells: Vec2::new(n[4], n[5]), origin: Vec2::new(n[6], n[7]) });
        } else if let Some(rest) = line.strip_prefix("token ") {
            if let Some((k, v)) = rest.split_once(' ') {
                tokens.insert(k.to_string(), v.trim().to_string());
            }
        }
    }
    (regions, tokens)
}

/// Decodes the embedded PNG and appends a box-filtered mip chain (alpha-weighted, so transparent texels
/// never darken their neighbours).
fn build_image() -> Image {
    let mut image = Image::from_buffer(
        ATLAS_PNG,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Descriptor(ImageSamplerDescriptor {
            label: Some("atlas".into()),
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            ..default()
        }),
        RenderAssetUsages::RENDER_WORLD,
    )
    .expect("decode embedded atlas.png");
    let (w, h) = (image.width() as usize, image.height() as usize);
    let base = image.data.take().expect("atlas pixels");
    assert_eq!(base.len(), w * h * 4, "atlas must decode to RGBA8");

    let levels = (MIP_LEVELS as usize).min(1 + w.min(h).ilog2() as usize);
    let mut data = Vec::with_capacity(base.len() * 4 / 3 + 64);
    data.extend_from_slice(&base);
    let (mut lw, mut lh, mut prev_start) = (w, h, 0usize);
    for _ in 1..levels {
        let (nw, nh) = (lw / 2, lh / 2);
        let start = data.len();
        data.resize(start + nw * nh * 4, 0);
        for y in 0..nh {
            for x in 0..nw {
                let mut sum = [0u32; 3];
                let mut plain = [0u32; 3];
                let mut a_sum = 0u32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let i = prev_start + ((2 * y + dy) * lw + 2 * x + dx) * 4;
                    let a = data[i + 3] as u32;
                    for c in 0..3 {
                        sum[c] += data[i + c] as u32 * a;
                        plain[c] += data[i + c] as u32;
                    }
                    a_sum += a;
                }
                let o = start + (y * nw + x) * 4;
                for c in 0..3 {
                    data[o + c] = if a_sum > 0 { (sum[c] / a_sum) as u8 } else { (plain[c] / 4) as u8 };
                }
                data[o + 3] = ((a_sum + 2) / 4) as u8;
            }
        }
        prev_start = start;
        lw = nw;
        lh = nh;
    }
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels as u32;
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon key used by content must have a baked atlas region (re-run atlas-tool when art changes).
    #[test]
    fn atlas_has_every_content_icon() {
        let (regions, tokens) = parse_index(ATLAS_TXT, 1.0, 1.0);
        let content = data::base_content();
        let mut missing = Vec::new();
        for part in content.parts.iter() {
            if let Some(icon) = &part.icon
                && !regions.contains_key(&format!("icon/{icon}"))
            {
                missing.push(format!("{} -> icon/{icon}", part.id));
            }
        }
        assert!(missing.is_empty(), "atlas is missing icons (run `cargo run -p atlas-tool --release`): {missing:?}");
        for tone in data::art_keys::ARMOR_TONES {
            assert!(tokens.contains_key(&format!("ship.armor.{tone}")), "token ship.armor.{tone} missing");
        }
        for key in data::art_keys::SYSTEM_COLORS {
            assert!(tokens.contains_key(&format!("ship.system_colors.{key}")), "token ship.system_colors.{key} missing");
        }
    }
}
