//! The texture atlas, rasterised at runtime from vector sources (rule: no fixed raster graphics).
//!
//! `crates/client/art` takes the SVG icons from the content and the embedded style tokens, lays every region out in
//! resolution-independent atlas units and rasterises the layout at a *tier* (pixels per ship cell, a power of
//! two). This module picks the tier from what the screen needs (camera zoom, or the part icons in the UI when
//! they are larger ([`UiAtlasNeed`]), × window scale factor, capped by the GPU's largest texture),
//! rasterises a new atlas when that tier changes (regions and mip chain spread over frames within a time
//! budget, the old texture stays in use meanwhile; a job for a tier the zoom has already outgrown is dropped
//! for the new one), uploads it a band of rows at a time as the bands are finished ([`upload_bands`]) and
//! switches the materials to it once all of it is on the GPU ([`rasterise_step`]). Region UVs are normalised
//! and identical at every tier, so meshes never need rebuilding. Every atlas mesh draws with [`AtlasMaterial`]
//! (`atlas.wesl`: texture × vertex colour, sampled with a mip LOD bias so the icons stay as sharp as the
//! geometry): alpha-blended, opaque with depth writes for the lit ship tiles ([`Atlas::opaque`]), additive
//! ([`Atlas::additive`]) or premultiplied ([`Atlas::premultiplied`]).
//! [`AtlasStatus`] reports what happens (the debug overlay).

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bevy::asset::{AssetEventSystems, AssetPath, RenderAssetUsages, embedded_asset, embedded_path};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::platform::collections::HashMap;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, Extent3d, RenderPipelineDescriptor,
    SpecializedMeshPipelineError, TexelCopyBufferLayout, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPipeline, Material2dPlugin};
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;

use crate::view::ViewScale;

/// Work per frame while re-rasterising (the old atlas stays on screen meanwhile).
const FRAME_BUDGET_MS: f64 = 6.0;
/// Work per frame while the atlas on screen is stretched more than [`BLURRY`] times: it looks blurry, so
/// finishing sooner beats a smooth frame rate.
const BLURRY_BUDGET_MS: f64 = 16.0;
const BLURRY: f32 = 2.0;
/// Most bytes of a new atlas uploaded per frame, per millisecond of its time budget (2.1 MB at 6 ms, 5.6 MB at
/// 16 ms): the pull pass finishes rows faster than the time budget stops it.
const UPLOAD_BYTES_PER_MS: f64 = 350_000.0;
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
    /// Shared material for every atlas mesh (vertex colours tint the texture), alpha-blended.
    pub material: Handle<AtlasMaterial>,
    /// The same, drawn in the opaque pass with depth writes: the lit ship tiles, so that the shield bubbles of
    /// lower ships are hidden under them (`ship_view`, `shield`).
    pub opaque: Handle<AtlasMaterial>,
    /// The same, added onto what lies below (glints and embers on the rocks, `scenery::rocks`).
    pub additive: Handle<AtlasMaterial>,
    /// The same with premultiplied vertex colours: what lies below is dimmed by the alpha and the colour added, so
    /// one mesh both veils and glows (the sky's galaxy, `scenery::sky::galaxy`).
    pub premultiplied: Handle<AtlasMaterial>,
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

/// The material of every atlas mesh: the atlas × the vertex colour, like Bevy's `ColorMaterial` but sampled with
/// a mip LOD bias of −0.5 (`atlas.wesl` says why). Alpha-blended, opaque (no blending, writes depth), additive
/// (the colour times its alpha added onto what lies below) or premultiplied (the colour added onto what lies below
/// dimmed by the alpha; `tint` fades only its alpha, so it is drawn at full tint).
///
/// `tint`'s alpha fades everything drawn with it: a group of meshes that fades as one (a screen's world-space
/// elements, its UI icons, the shipyard while a screen covers it) shares a material of its own
/// ([`Atlas::group_material`], [`set_alpha`]), so a fade writes one uniform per frame and no mesh.
///
/// `aperture` cuts a group to a circle on screen ([`set_aperture`], the portal into the voyage map, `map::portal`):
/// the group draws only inside it, or only outside it. `clip` cuts it to a rectangle on screen ([`set_clip`]: a UI
/// icon in a list that scrolls, cut where the list cuts its row).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(AtlasKey)]
pub struct AtlasMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
    #[uniform(2)]
    tint: Vec4,
    /// The circle in framebuffer px: centre x, y, radius, and its soft edge (positive: inside shows, negative:
    /// outside shows, 0: no cut).
    #[uniform(3)]
    aperture: Vec4,
    /// The rectangle in framebuffer px: min x, min y, max x, max y (all 0: no cut).
    #[uniform(4)]
    clip: Vec4,
    opaque: bool,
    blend: Blend,
}

/// How a blended atlas material lays its colour over what lies below.
#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug)]
enum Blend {
    /// Over it by the alpha.
    Alpha,
    /// Added onto it (the colour times its alpha).
    Additive,
    /// The colour (premultiplied) added onto it dimmed by the alpha.
    Premultiplied,
}

/// What an atlas material's pipeline is specialised by: its blending.
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct AtlasKey {
    blend: Blend,
}

impl From<&AtlasMaterial> for AtlasKey {
    fn from(m: &AtlasMaterial) -> Self {
        AtlasKey { blend: m.blend }
    }
}

impl Atlas {
    /// A new alpha-blended material on the atlas (it follows every tier like the shared one), at alpha 1: for a
    /// group of meshes that fades as one ([`set_alpha`]).
    pub fn group_material(&self, materials: &mut Assets<AtlasMaterial>) -> Handle<AtlasMaterial> {
        let texture = materials.get(&self.material).map(|m| m.texture.clone()).unwrap_or_default();
        materials.add(AtlasMaterial { texture, tint: Vec4::ONE, aperture: Vec4::ZERO, clip: Vec4::ZERO, opaque: false, blend: Blend::Alpha })
    }
}

/// A cut of a group to a circle on screen (framebuffer px, y down): the group shows only inside it, or only outside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aperture {
    pub centre: Vec2,
    pub radius: f32,
    pub inside: bool,
}

/// The aperture's soft edge (px): wide enough that the map's opening reads as a soft iris with nothing drawn along
/// it (the portal, `map::portal`).
pub const APERTURE_EDGE: f32 = 18.0;

/// Cuts a group material to an aperture, or not (`None`) (written only when it changes).
pub fn set_aperture(materials: &mut Assets<AtlasMaterial>, material: &Handle<AtlasMaterial>, aperture: Option<Aperture>) {
    let want = aperture.map_or(Vec4::ZERO, |a| a.centre.extend(a.radius).extend(if a.inside { APERTURE_EDGE } else { -APERTURE_EDGE }));
    if materials.get(material).is_some_and(|m| m.aperture != want)
        && let Some(mut m) = materials.get_mut(material)
    {
        m.aperture = want;
    }
}

/// Makes `material` draw like `base` (its fade) cut to `clip` (framebuffer px, y down), or uncut (`None`); written
/// only when it changes.
pub fn set_clip(materials: &mut Assets<AtlasMaterial>, base: &Handle<AtlasMaterial>, material: &Handle<AtlasMaterial>, clip: Option<Rect>) {
    let clip = clip.map_or(Vec4::ZERO, |r| Vec4::new(r.min.x, r.min.y, r.max.x, r.max.y));
    let Some(tint) = materials.get(base).map(|m| m.tint) else { return };
    if materials.get(material).is_some_and(|m| m.tint != tint || m.clip != clip)
        && let Some(mut m) = materials.get_mut(material)
    {
        (m.tint, m.clip) = (tint, clip);
    }
}

/// Sets a group material's alpha (written only when it changes: a write re-prepares the material).
pub fn set_alpha(materials: &mut Assets<AtlasMaterial>, material: &Handle<AtlasMaterial>, alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    if materials.get(material).is_some_and(|m| (m.tint.w - a).abs() > 1e-4)
        && let Some(mut m) = materials.get_mut(material)
    {
        m.tint.w = a;
    }
}

impl Material2d for AtlasMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(AssetPath::from_path_buf(embedded_path!("atlas.wesl")).with_source("embedded"))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        if self.opaque { AlphaMode2d::Opaque } else { AlphaMode2d::Blend }
    }

    fn specialize(
        _: &Material2dPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let src = match key.bind_group_data.blend {
            Blend::Alpha => return Ok(()),
            Blend::Additive => BlendFactor::SrcAlpha,
            Blend::Premultiplied => BlendFactor::One,
        };
        let dst = if key.bind_group_data.blend == Blend::Additive { BlendFactor::One } else { BlendFactor::OneMinusSrcAlpha };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            let color = BlendComponent { src_factor: src, dst_factor: dst, operation: BlendOperation::Add };
            let alpha = BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState { color, alpha });
            }
        }
        Ok(())
    }
}

/// A re-rasterisation in progress.
#[derive(Resource, Default)]
struct RasterJob {
    job: Option<Job>,
    /// Tier wanted and since when (real seconds).
    wanted: Option<(u32, f32)>,
}

struct Job {
    raster: art::Rasterizer,
    /// The new tier's texture: created without data, filled a band at a time ([`upload_bands`]).
    image: Handle<Image>,
    /// Bands handed to the render world so far.
    bands: usize,
    done: bool,
    /// Milliseconds spent so far, frames, the longest single step, the most bytes uploaded in one frame.
    spent: f64,
    frames: u32,
    longest: f64,
    upload: usize,
}

impl Job {
    fn new(layout: Arc<art::Layout>, tier: u32, images: &mut Assets<Image>, uploads: &Uploads) -> Self {
        let raster = art::Rasterizer::new(layout, tier);
        let (w, h) = raster.size();
        let mut image = atlas_image(w, h, raster.levels(), None);
        // The first band written to a mip level makes wgpu clear the level: for a render target that is a cheap
        // clear pass instead of copies from a zero buffer.
        image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
        let image = images.add(image);
        uploads.start(Some(image.id()));
        Self { raster, image, bands: 0, done: false, spent: 0.0, frames: 0, longest: 0.0, upload: 0 }
    }
}

/// Rows of a mip level of the new atlas, on their way to the GPU.
struct Band {
    level: u32,
    y0: u32,
    width: u32,
    rows: u32,
    data: Vec<u8>,
}

/// The bands handed from [`rasterise_step`] to [`upload_bands`] (the render world); both worlds share it.
#[derive(Resource, Clone, Default)]
struct Uploads(Arc<Mutex<UploadQueue>>);

#[derive(Default)]
struct UploadQueue {
    /// The texture the bands are for (the job's).
    image: Option<AssetId<Image>>,
    bands: Vec<Band>,
    /// Bands written into it so far.
    written: usize,
}

impl Uploads {
    fn queue(&self) -> MutexGuard<'_, UploadQueue> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A new job's texture (the bands of an earlier one are dropped).
    fn start(&self, image: Option<AssetId<Image>>) {
        *self.queue() = UploadQueue { image, ..default() };
    }
}

/// Logical pixels per ship cell the part icons in the UI need now (`ui_icons`; 0 when none are shown). The
/// atlas tier covers this as well as the world zoom.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct UiAtlasNeed(pub f32);

/// A finished rebuild: tier, milliseconds of work, frames, longest single step (ms).
pub type Rebuild = (u32, f64, u32, f64);

/// What the atlas does, for the debug overlay.
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
        let image = atlas_image(raster.width, raster.height, raster.levels, Some(raster.data));
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
        // Mesh changes are announced after every PostUpdate system that replaces a mesh (they place their meshes, so
        // they run before the transforms are propagated). Announced in between, a mesh replaced in two frames in a
        // row got a second announcement a frame late, after its new data had already gone to the GPU: Bevy logged
        // "already extracted" and freed the mesh's GPU buffer, and it stayed invisible until it was replaced again
        // (Niklas, 15 Oct 2026: drydock icon buttons showed only once hovered, which rebuilds their glyph).
        app.configure_sets(PostUpdate, AssetEventSystems.after(TransformSystems::Propagate));
        app.add_plugins(Material2dPlugin::<AtlasMaterial>::default());
        let world = app.world_mut();
        let image = world.resource_mut::<Assets<Image>>().add(image);
        let mut materials = world.resource_mut::<Assets<AtlasMaterial>>();
        let base = AtlasMaterial { texture: image, tint: Vec4::ONE, aperture: Vec4::ZERO, clip: Vec4::ZERO, opaque: false, blend: Blend::Alpha };
        let material = materials.add(base.clone());
        let opaque = materials.add(AtlasMaterial { opaque: true, ..base.clone() });
        let additive = materials.add(AtlasMaterial { blend: Blend::Additive, ..base.clone() });
        let premultiplied = materials.add(AtlasMaterial { blend: Blend::Premultiplied, ..base });
        let atlas = Atlas { material, opaque, additive, premultiplied, regions, tokens: art::tokens(), layout, tier };
        world.insert_resource(crate::style::Style::from_atlas(&atlas));
        world.insert_resource(atlas);
        let uploads = Uploads::default();
        app.insert_resource(uploads.clone())
            .init_resource::<RasterJob>()
            .init_resource::<AtlasStatus>()
            .init_resource::<UiAtlasNeed>()
            .add_systems(Update, (choose_tier, rasterise_step).chain());
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.insert_resource(uploads).add_systems(Render, upload_bands.in_set(RenderSystems::PrepareResources));
        }
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
    view: Res<ViewScale>,
    ui_need: Res<UiAtlasNeed>,
    window: Single<&Window, With<PrimaryWindow>>,
    device: Option<Res<RenderDevice>>,
    atlas: Res<Atlas>,
    mut job: ResMut<RasterJob>,
    mut status: ResMut<AtlasStatus>,
    mut images: ResMut<Assets<Image>>,
    uploads: Res<Uploads>,
) {
    if status.max_tier == 0
        && let Some(device) = device
    {
        status.max_tier = max_tier(&atlas.layout, device.limits().max_texture_dimension_2d);
        info!("atlas: tiers up to {} px/cell (GPU textures up to {} px)", status.max_tier, device.limits().max_texture_dimension_2d);
    }
    let cap = if status.max_tier > 0 { status.max_tier } else { art::MAX_TIER };
    let need = view.zoom_target.max(view.zoom).max(ui_need.0) * window.scale_factor();

    status.need = need;
    let want = art::tier_for(need).min(cap);
    let building = job.job.as_ref().map(|j| j.raster.tier());
    status.building = building;
    let cur = building.unwrap_or(atlas.tier);
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
                let same = target == atlas.tier;
                if same {
                    uploads.start(None);
                }
                job.job = (!same).then(|| Job::new(atlas.layout.clone(), target, &mut images, &uploads));
                job.wanted = None;
                status.building = (!same).then_some(target);
            }
        }
        _ => job.wanted = Some((target, now)),
    }
}

/// Does steps of the job until the frame's time budget or its bytes ([`UPLOAD_BYTES_PER_MS`]) are used, and hands
/// the finished rows to the render world ([`upload_bands`]). Once every band is in the new texture, the materials
/// switch to it (written through `get_mut`, so Bevy re-prepares them) and the old image goes with its last handle.
/// No frame uploads the whole atlas (84 MB at tier 512, which took one frame until 13 Oct 2026).
///
/// Until 13 Oct 2026 the image was replaced in place and the material "touched" with `materials.get_mut(id)`.
/// In Bevy 0.20 that is a no-op (`get_mut` returns an `AssetMut` guard that only marks the asset modified when
/// it is written through), so the material's bind group kept the texture it was first prepared with: every
/// atlas mesh sampled the start atlas (64 or 32 px/cell) for the whole session, stretched 2.5–5× at 160 px/m,
/// while F3 showed the new tier. Now the materials really change (their texture handle), to a new image per tier
/// that is prepared on the GPU long before the switch (its bands come frames later).
fn rasterise_step(
    mut job: ResMut<RasterJob>,
    mut atlas: ResMut<Atlas>,
    mut status: ResMut<AtlasStatus>,
    mut materials: ResMut<Assets<AtlasMaterial>>,
    uploads: Res<Uploads>,
) {
    let Some(j) = job.job.as_mut() else { return };
    if !j.done {
        let budget = if status.need > atlas.tier as f32 * BLURRY { BLURRY_BUDGET_MS } else { FRAME_BUDGET_MS };
        let most = (UPLOAD_BYTES_PER_MS * budget) as usize;
        let t0 = Instant::now();
        let (mut bands, mut bytes) = (Vec::<Band>::new(), 0);
        while !j.done && bytes < most && t0.elapsed().as_secs_f64() * 1e3 < budget {
            let t = Instant::now();
            match j.raster.step() {
                Ok(d) => j.done = d,
                Err(e) => {
                    error!("atlas rasterisation failed: {e}");
                    uploads.start(None);
                    job.job = None;
                    return;
                }
            }
            j.longest = j.longest.max(t.elapsed().as_secs_f64() * 1e3);
            let Some((level, rows)) = j.raster.finished() else { continue };
            let (data, width) = j.raster.rows(level, rows.clone());
            bytes += data.len();
            // Rows that follow the last band of the same level join it: one texture write.
            match bands.last_mut() {
                Some(b) if b.level == level && b.y0 + b.rows == rows.start => {
                    b.data.extend_from_slice(data);
                    b.rows = rows.end - b.y0;
                }
                _ => bands.push(Band { level, y0: rows.start, width, rows: rows.len() as u32, data: data.to_vec() }),
            }
        }
        j.spent += t0.elapsed().as_secs_f64() * 1e3;
        j.frames += 1;
        j.upload = j.upload.max(bytes);
        j.bands += bands.len();
        uploads.queue().bands.extend(bands);
    }
    if !j.done || uploads.queue().written < j.bands {
        return;
    }
    let Job { raster, image, spent, frames, longest, upload, .. } = job.job.take().expect("job");
    // Every atlas material: the shared ones and the groups' (`Atlas::group_material`).
    let ids: Vec<_> = materials.ids().collect();
    for id in ids {
        if let Some(mut m) = materials.get_mut(id) {
            m.texture = image.clone();
        }
    }
    let tier = raster.tier();
    let ((w, h), levels) = (raster.size(), raster.levels());
    atlas.tier = tier;
    status.last = Some((tier, spent, frames, longest));
    status.building = None;
    info!(
        "atlas: re-rasterised at {tier} px/cell ({w}x{h}, {levels} mips): {spent:.1} ms over {frames} frames, longest step {longest:.1} ms, at most {:.1} MB uploaded per frame",
        upload as f64 / 1e6
    );
}

/// Render world: writes the bands handed over since the last frame into the new atlas texture (they wait until
/// its GPU texture exists, which is the frame after the job starts).
fn upload_bands(uploads: Res<Uploads>, images: Res<RenderAssets<GpuImage>>, queue: Res<RenderQueue>) {
    let mut q = uploads.queue();
    let Some(gpu) = q.image.and_then(|id| images.get(id)) else { return };
    let bands = std::mem::take(&mut q.bands);
    for b in &bands {
        let mut to = gpu.texture.as_image_copy();
        to.mip_level = b.level;
        to.origin.y = b.y0;
        let layout = TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(b.width * 4), rows_per_image: Some(b.rows) };
        queue.write_texture(to, &b.data, layout, Extent3d { width: b.width, height: b.rows, depth_or_array_layers: 1 });
    }
    q.written += bands.len();
}

/// An atlas texture with its mip levels (`data`: all of them, or `None` to fill it later).
fn atlas_image(width: u32, height: u32, levels: u32, data: Option<Vec<u8>>) -> Image {
    let mut image = Image::new_uninit(
        Extent3d { width, height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = data;
    image.texture_descriptor.mip_level_count = levels;
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
