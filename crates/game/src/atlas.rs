//! The texture atlas, rasterised at runtime from vector sources (rule: no fixed raster graphics).
//!
//! `crates/art` embeds the SVG icons and style tokens, lays every region out in resolution-independent atlas
//! units and rasterises the layout at a *tier* (pixels per ship cell, a power of two). This module picks the
//! tier from what the screen needs (camera zoom, or the part icons in the UI when they are larger
//! ([`UiAtlasNeed`]), × window scale factor, capped by the GPU's largest texture),
//! rasterises a new atlas when that tier changes (regions and mip chain spread over frames within a time
//! budget, the old texture stays in use meanwhile; a job for a tier the zoom has already outgrown is dropped
//! for the new one) and switches the material to the new image a frame after adding it ([`rasterise_step`]).
//! Region UVs are normalised and identical at every tier, so meshes never need rebuilding. Every atlas mesh
//! draws with one [`AtlasMaterial`] (`atlas.wesl`: texture × vertex colour, sampled with a mip LOD bias so the
//! icons stay as sharp as the geometry). [`AtlasStatus`] reports what happens (F3 overlay).

use std::sync::Arc;

use bevy::asset::{AssetPath, RenderAssetUsages, embedded_asset, embedded_path};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::platform::collections::HashMap;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::render::renderer::RenderDevice;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin};
use bevy::window::PrimaryWindow;

use crate::camera::CameraRig;

/// Work per frame while re-rasterising (the old atlas stays on screen meanwhile).
const FRAME_BUDGET_MS: f64 = 6.0;
/// Work per frame while the atlas on screen is stretched more than [`BLURRY`] times: it looks blurry, so
/// finishing sooner beats a smooth frame rate.
const BLURRY_BUDGET_MS: f64 = 16.0;
const BLURRY: f32 = 2.0;
/// A new tier has to be wanted this long before the atlas is rebuilt (zooming through tiers costs nothing).
const SETTLE_SECONDS: f32 = 0.25;

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
    /// Shared material for every atlas mesh (vertex colours tint the texture).
    pub material: Handle<AtlasMaterial>,
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

/// The material of every atlas mesh: the atlas × the vertex colour, alpha-blended, like Bevy's `ColorMaterial`
/// but sampled with a mip LOD bias of −0.5 (`atlas.wesl` says why).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct AtlasMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
}

impl Material2d for AtlasMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(AssetPath::from_path_buf(embedded_path!("atlas.wesl")).with_source("embedded"))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// A re-rasterisation in progress.
#[derive(Resource, Default)]
struct RasterJob {
    job: Option<Job>,
    /// Tier wanted and since when (real seconds).
    wanted: Option<(u32, f32)>,
    /// A finished atlas, added as a new image one frame before the material switches to it (see
    /// [`rasterise_step`]), and its numbers for [`AtlasStatus::last`].
    ready: Option<(Handle<Image>, Rebuild)>,
}

struct Job {
    raster: art::Rasterizer,
    /// Milliseconds spent so far, frames, and the longest single step.
    spent: f64,
    frames: u32,
    longest: f64,
}

/// Logical pixels per ship cell the part icons in the UI need now (`ui_icons`; 0 when none are shown). The
/// atlas tier covers this as well as the world zoom.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct UiAtlasNeed(pub f32);

/// A finished rebuild: tier, milliseconds of work, frames, longest single step (ms).
pub type Rebuild = (u32, f64, u32, f64);

/// What the atlas does, for the debug overlay (F3).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct AtlasStatus {
    /// Pixels per cell the screen needs now (zoom × scale factor).
    pub need: f32,
    /// The highest tier the GPU's texture size limit allows.
    pub max_tier: u32,
    /// The tier being rasterised, if any.
    pub building: Option<u32>,
    /// The last rebuild (now on screen).
    pub last: Option<Rebuild>,
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
        embedded_asset!(app, "atlas.wesl");
        app.add_plugins(Material2dPlugin::<AtlasMaterial>::default());
        let world = app.world_mut();
        let image = world.resource_mut::<Assets<Image>>().add(image);
        let material = world.resource_mut::<Assets<AtlasMaterial>>().add(AtlasMaterial { texture: image });
        let atlas = Atlas { material, regions, tokens: art::tokens(), layout, tier };
        world.insert_resource(crate::style::Style::from_atlas(&atlas));
        world.insert_resource(atlas);
        app.init_resource::<RasterJob>()
            .init_resource::<AtlasStatus>()
            .init_resource::<UiAtlasNeed>()
            .add_systems(Update, (choose_tier, rasterise_step).chain());
    }
}

/// The highest tier whose atlas fits the GPU's largest 2D texture.
fn max_tier(layout: &art::Layout, limit: u32) -> u32 {
    let mut tier = art::MAX_TIER;
    while tier > art::MIN_TIER {
        let (w, h) = layout.pixel_size(tier);
        if w.max(h) <= limit {
            break;
        }
        tier /= 2;
    }
    tier
}

/// The tier the screen needs: camera zoom (logical px per metre = per cell, or the UI icons' when larger) × the
/// window's scale factor, capped by what the GPU can hold.
#[allow(clippy::too_many_arguments)]
fn choose_tier(
    time: Res<Time<Real>>,
    rig: Res<CameraRig>,
    ui_need: Res<UiAtlasNeed>,
    window: Single<&Window, With<PrimaryWindow>>,
    device: Option<Res<RenderDevice>>,
    atlas: Res<Atlas>,
    mut job: ResMut<RasterJob>,
    mut status: ResMut<AtlasStatus>,
) {
    if status.max_tier == 0
        && let Some(device) = device
    {
        status.max_tier = max_tier(&atlas.layout, device.limits().max_texture_dimension_2d);
        info!("atlas: tiers up to {} px/cell (GPU textures up to {} px)", status.max_tier, device.limits().max_texture_dimension_2d);
    }
    let cap = if status.max_tier > 0 { status.max_tier } else { art::MAX_TIER };
    let need = rig.zoom_target.max(rig.zoom).max(ui_need.0) * window.scale_factor();

    status.need = need;
    let want = art::tier_for(need).min(cap);
    let building = job.job.as_ref().map(|j| j.raster.tier());
    status.building = building;
    let cur = building.or(job.ready.as_ref().map(|r| r.1.0)).unwrap_or(atlas.tier);
    // Hysteresis: go up as soon as the screen needs more; go down only when half the tier is clearly enough.
    let target = if want > cur || (want < cur && need < cur as f32 * 0.5 * 0.75) { want } else { cur };
    let now = time.elapsed_secs();
    if target == cur {
        job.wanted = None;
        return;
    }
    match job.wanted {
        Some((t, since)) if t == target => {
            // A job for a lower tier than the screen now needs is dropped: finishing it first would only delay
            // the sharp one. A job for a higher tier runs on (zooming out is served by its mips).
            let free = building.is_none_or(|b| b < target);
            if now - since >= SETTLE_SECONDS && free {
                // Back to the tier on screen (a job for a lower one was running): just drop the job.
                let same = target == atlas.tier && job.ready.is_none();
                job.job = (!same).then(|| Job {
                    raster: art::Rasterizer::new(atlas.layout.clone(), target),
                    spent: 0.0,
                    frames: 0,
                    longest: 0.0,
                });
                job.wanted = None;
                status.building = (!same).then_some(target);
            }
        }
        _ => job.wanted = Some((target, now)),
    }
}

/// Renders regions and mip rows until the frame budget is used. A complete atlas becomes a new image; one frame
/// later, when the render world has uploaded it, the material switches to it.
///
/// Until 13 Oct 2026 the image was replaced in place and the material "touched" with `materials.get_mut(id)`.
/// In Bevy 0.20 that is a no-op (`get_mut` returns an `AssetMut` guard that only marks the asset modified when
/// it is written through), so the material's bind group kept the texture it was first prepared with: every
/// atlas mesh sampled the start atlas (64 or 32 px/cell) for the whole session, stretched 2.5–5× at 160 px/m,
/// while F3 showed the new tier. Now the material really changes (its texture handle). The new image is added
/// one frame before the switch because Bevy does not order a material's preparation after its images' within
/// `RenderSystems::PrepareAssets`: switching in the same frame could bind a texture that is not uploaded yet
/// (Bevy then retries the material, and the atlas meshes would vanish for a frame). A new image per tier also
/// avoids Bevy's in-place update for an unchanged texture size, which writes mip level 0 only.
fn rasterise_step(
    mut job: ResMut<RasterJob>,
    mut atlas: ResMut<Atlas>,
    mut status: ResMut<AtlasStatus>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<AtlasMaterial>>,
) {
    if let Some((image, last)) = job.ready.take() {
        // The material switches (written through `get_mut`, so Bevy re-prepares it); the old image goes with its
        // last handle.
        if let Some(mut m) = materials.get_mut(&atlas.material) {
            m.texture = image;
        }
        atlas.tier = last.0;
        status.last = Some(last);
    }
    let Some(j) = job.job.as_mut() else { return };
    let budget = if status.need > atlas.tier as f32 * BLURRY { BLURRY_BUDGET_MS } else { FRAME_BUDGET_MS };
    let t0 = Instant::now();
    let mut done = false;
    while !done && t0.elapsed().as_secs_f64() * 1e3 < budget {
        let t = Instant::now();
        match j.raster.step() {
            Ok(d) => done = d,
            Err(e) => {
                error!("atlas rasterisation failed: {e}");
                job.job = None;
                return;
            }
        }
        j.longest = j.longest.max(t.elapsed().as_secs_f64() * 1e3);
    }
    j.spent += t0.elapsed().as_secs_f64() * 1e3;
    j.frames += 1;
    if !done {
        return;
    }
    let Job { raster, spent, frames, longest } = job.job.take().expect("job");
    let tier = raster.tier();
    let raster = match raster.finish() {
        Ok(r) => r,
        Err(e) => {
            error!("atlas rasterisation failed: {e}");
            return;
        }
    };
    let (w, h, levels) = (raster.width, raster.height, raster.levels);
    job.ready = Some((images.add(image_from(raster)), (tier, spent, frames, longest)));
    status.building = None;
    info!("atlas: re-rasterised at {tier} px/cell ({w}x{h}, {levels} mips): {spent:.1} ms over {frames} frames, longest step {longest:.1} ms");
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
