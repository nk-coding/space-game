//! The texture atlas, rasterised at runtime from vector sources (rule: no fixed raster graphics).
//!
//! `crates/art` embeds the SVG icons and style tokens, lays every region out in resolution-independent atlas
//! units and rasterises the layout at a *tier* (pixels per ship cell, a power of two). This module picks the
//! tier from what the screen needs (camera zoom × window scale factor), rasterises a new atlas when that tier
//! changes (spread over frames, the old texture stays in use meanwhile) and swaps the image in place. Region
//! UVs are normalised and identical at every tier, so meshes never need rebuilding.

use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::platform::collections::HashMap;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::AlphaMode2d;
use bevy::window::PrimaryWindow;

use crate::camera::CameraRig;

/// Work per frame while re-rasterising (the old atlas stays on screen meanwhile).
const FRAME_BUDGET_MS: f64 = 6.0;
/// A new tier has to be wanted this long before the atlas is rebuilt (zooming through tiers costs nothing).
const SETTLE_SECONDS: f32 = 0.35;

/// One region of the atlas.
#[derive(Clone, Copy, Debug)]
pub struct Region {
    /// Normalised UV rectangle (the same at every tier).
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
    /// Anchor (Bevy convention: -0.5..0.5, y up) that puts the region's origin at the transform.
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
    tokens: art::Tokens,
    layout: Arc<art::Layout>,
    /// Pixels per cell of the atlas on screen now.
    pub tier: u32,
}

impl Atlas {
    pub fn get(&self, name: &str) -> Option<&Region> {
        self.regions.get(name)
    }
    /// A region that must exist (FX regions the art crate always defines).
    pub fn region(&self, name: &str) -> Region {
        *self.regions.get(name).unwrap_or_else(|| panic!("atlas region `{name}` missing"))
    }
    pub fn token(&self, path: &str) -> &str {
        self.tokens.get(path).unwrap_or_else(|| panic!("style token `{path}` missing in art/tokens.json"))
    }
    pub fn token_f32(&self, path: &str) -> f32 {
        self.tokens.f32(path).unwrap_or_else(|| panic!("style token `{path}` is not a number"))
    }
    pub fn token_color(&self, path: &str) -> Color {
        parse_hex(self.token(path)).unwrap_or_else(|| panic!("style token `{path}` is not a #rrggbb colour"))
    }
}

pub fn parse_hex(s: &str) -> Option<Color> {
    Srgba::hex(s.trim()).ok().map(Color::from)
}

/// A re-rasterisation in progress.
#[derive(Resource, Default)]
struct RasterJob {
    job: Option<(art::Rasterizer, f64, u32)>,
    /// Tier wanted and since when (real seconds).
    wanted: Option<(u32, f32)>,
}

pub struct AtlasPlugin;

impl Plugin for AtlasPlugin {
    fn build(&self, app: &mut App) {
        let layout = Arc::new(art::Layout::new(&data::base_content()));
        // Start at the tier the default zoom needs on a 2x display; refined on the first frames.
        let tier = 64;
        let t0 = Instant::now();
        let raster = art::rasterise(&layout, tier).expect("rasterise atlas");
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        info!(
            "atlas: {} regions rasterised at {tier} px/cell ({}x{}, {} mips) in {ms:.1} ms",
            layout.regions.len(),
            raster.width,
            raster.height,
            raster.levels
        );
        let image = image_from(raster);
        let regions = layout
            .regions
            .iter()
            .map(|r| {
                let [u0, v0, u1, v1] = layout.uv(r);
                (
                    r.name.clone(),
                    Region {
                        uv: Rect::new(u0, v0, u1, v1),
                        cells: Vec2::from(r.cells),
                        origin: Vec2::from(r.origin),
                    },
                )
            })
            .collect();
        let world = app.world_mut();
        let image = world.resource_mut::<Assets<Image>>().add(image);
        let material = world.resource_mut::<Assets<ColorMaterial>>().add(ColorMaterial {
            color: Color::WHITE,
            alpha_mode: AlphaMode2d::Blend,
            texture: Some(image.clone()),
            ..default()
        });
        let atlas = Atlas { image, material, regions, tokens: art::tokens(), layout, tier };
        world.insert_resource(crate::style::Style::from_atlas(&atlas));
        world.insert_resource(atlas);
        app.init_resource::<RasterJob>().add_systems(Update, (choose_tier, rasterise_step).chain());
    }
}

/// The tier the screen needs: camera zoom (logical px per metre = per cell) × the window's scale factor.
fn choose_tier(
    time: Res<Time<Real>>,
    rig: Res<CameraRig>,
    window: Single<&Window, With<PrimaryWindow>>,
    atlas: Res<Atlas>,
    mut job: ResMut<RasterJob>,
) {
    let need = rig.zoom_target.max(rig.zoom) * window.scale_factor();
    let want = art::tier_for(need);
    let cur = job.job.as_ref().map(|j| j.0.tier()).unwrap_or(atlas.tier);
    // Hysteresis: go up as soon as the screen needs more; go down only when half the tier is clearly enough.
    let target = if want > cur || (want < cur && need < cur as f32 * 0.5 * 0.75) { want } else { cur };
    let now = time.elapsed_secs();
    if target == cur {
        job.wanted = None;
        return;
    }
    match job.wanted {
        Some((t, since)) if t == target => {
            if now - since >= SETTLE_SECONDS && job.job.is_none() {
                let r = art::Rasterizer::new(atlas.layout.clone(), target);
                job.job = Some((r, 0.0, 0));
                job.wanted = None;
            }
        }
        _ => job.wanted = Some((target, now)),
    }
}

/// Renders regions until the frame budget is used, then finishes and swaps the image.
fn rasterise_step(
    mut job: ResMut<RasterJob>,
    mut atlas: ResMut<Atlas>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Some((r, spent, frames)) = job.job.as_mut() else { return };
    let t0 = Instant::now();
    let mut done = false;
    while t0.elapsed().as_secs_f64() * 1e3 < FRAME_BUDGET_MS {
        match r.step() {
            Ok(true) => {
                done = true;
                break;
            }
            Ok(false) => {}
            Err(e) => {
                error!("atlas rasterisation failed: {e}");
                job.job = None;
                return;
            }
        }
    }
    *spent += t0.elapsed().as_secs_f64() * 1e3;
    *frames += 1;
    if !done {
        return;
    }
    let (r, spent, frames) = job.job.take().expect("job");
    let tier = r.tier();
    let t1 = Instant::now();
    let raster = r.finish();
    let finish_ms = t1.elapsed().as_secs_f64() * 1e3;
    let (w, h) = (raster.width, raster.height);
    let _ = images.insert(&atlas.image, image_from(raster));
    atlas.tier = tier;
    // Re-bind every material that samples the atlas (they cache the old texture view).
    let ids: Vec<AssetId<ColorMaterial>> =
        materials.iter().filter(|(_, m)| m.texture.as_ref() == Some(&atlas.image)).map(|(id, _)| id).collect();
    for id in ids {
        let _ = materials.get_mut(id);
    }
    info!(
        "atlas: re-rasterised at {tier} px/cell ({w}x{h}): regions {spent:.1} ms over {frames} frames, mips {finish_ms:.1} ms"
    );
}

fn image_from(r: art::Raster) -> Image {
    let mut image = Image::new_uninit(
        Extent3d { width: r.width, height: r.height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(r.data);
    image.texture_descriptor.mip_level_count = r.levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        label: Some("atlas".into()),
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}
