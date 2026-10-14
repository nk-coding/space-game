//! The rasteriser: the atlas's regions drawn at a tier (pixels per cell) from their SVG, in bands of rows so a frame
//! never waits long ([`Rasterizer`]), or at once ([`rasterise`]); soft glows resampled, procedural textures drawn.

use super::*;

// ---------------------------------------------------------------------------------------------------------
// Rasteriser

/// A finished atlas: straight-alpha sRGB RGBA8, mip levels concatenated (level 0 first).
pub struct Raster {
    pub tier: u32,
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    pub data: Vec<u8>,
    /// Regions whose SVG did not parse (`name: why`); they are left transparent, the rest of the atlas is fine.
    pub failed: Vec<String>,
}

/// Renders a layout at one tier in small steps ([`Rasterizer::step`], each about [`STEP_TEXELS`] texels of work):
/// zeroes its buffer a band at a time (no large zeroed allocation at once), renders every region a band of rows
/// at a time, then the mip chain a band at a time (downsampling level by level, then the colour pull, coarse to
/// fine). After its pull a band never changes again ([`Rasterizer::finished`]), so a client can spread all of it
/// over frames and upload as it goes. [`Rasterizer::finish`] hands the whole result over.
pub struct Rasterizer {
    layout: Arc<Layout>,
    tier: u32,
    pub(super) width: u32,
    height: u32,
    /// Every mip level, concatenated; grows to its full size while [`Stage::Clear`] zeroes it.
    pub(super) rgba: Vec<u8>,
    /// The full size of `rgba` in bytes.
    total: usize,
    /// `(offset in bytes, width, height)` per mip level.
    levels: Vec<(usize, usize, usize)>,
    pub(super) stage: Stage,
    /// The SVG layers of the region in progress, parsed at its first band.
    parsed: Option<Parsed>,
    /// One band's pixels, premultiplied (tiny-skia's format), reused.
    band: Vec<u8>,
    /// The rows the last step made final: `(mip level, rows)`.
    finished: Option<(u32, Range<u32>)>,
    failed: Vec<String>,
}

/// The parsed layers of the region in progress.
pub(super) struct Parsed {
    index: usize,
    /// The soft layer, when it is rendered at the tier ([`RegionDef::soft_direct`]); else it is resampled.
    soft: Option<usvg::Tree>,
    crisp: Option<usvg::Tree>,
    /// Rendered in one step: a blur can't be cut into bands (a soft layer rendered at the tier, or a crisp layer
    /// that uses a filter, which no base icon does).
    whole: bool,
}

/// Where a [`Rasterizer`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    /// Zeroing the buffer (`rgba.len()` bytes so far).
    Clear,
    /// Region `index`, from pixel row `row`.
    Region { index: usize, row: u32 },
    /// Downsampling into `level` (1..), from output row `row`.
    Down { level: usize, row: usize },
    /// Pulling colour into the nearly transparent texels of `level` (from the coarser level; the coarsest has
    /// none), from row `row`. The rows are final after it.
    Pull { level: usize, row: usize },
    Done,
}

/// Texels per step (a millisecond natively at most, for the busiest icon).
pub const STEP_TEXELS: usize = 1 << 15;
/// Bytes zeroed per step while clearing (zeroing is much cheaper per byte than the rest).
pub(super) const CLEAR_BYTES: usize = 1 << 21;
/// Texels below this alpha take their colour from the coarser level (see [`Rasterizer::pull_rows`]).
pub(super) const SOLID: u8 = 24;

impl Rasterizer {
    pub fn new(layout: Arc<Layout>, tier: u32) -> Self {
        let (width, height) = layout.pixel_size(tier);
        let (mut lw, mut lh) = (width as usize, height as usize);
        let mut levels = vec![(0, lw, lh)];
        let mut total = lw * lh * 4;
        while (levels.len() as u32) < MAX_MIPS && lw % 2 == 0 && lh % 2 == 0 && lw.min(lh) >= 32 {
            lw /= 2;
            lh /= 2;
            levels.push((total, lw, lh));
            total += lw * lh * 4;
        }
        Self {
            layout,
            tier,
            width,
            height,
            rgba: Vec::with_capacity(total),
            total,
            levels,
            stage: Stage::Clear,
            parsed: None,
            band: Vec::new(),
            finished: None,
            failed: Vec::new(),
        }
    }

    pub fn tier(&self) -> u32 {
        self.tier
    }

    /// Mip level 0 in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// How many mip levels the atlas has.
    pub fn levels(&self) -> u32 {
        self.levels.len() as u32
    }

    /// The rows of a mip level the last [`Rasterizer::step`] made final, if any: they never change again. Every
    /// row of every level is reported exactly once, coarsest level first.
    pub fn finished(&self) -> Option<(u32, Range<u32>)> {
        self.finished.clone()
    }

    /// Rows of mip `level` (straight-alpha sRGB RGBA8) and the level's width in pixels.
    pub fn rows(&self, level: u32, rows: Range<u32>) -> (&[u8], u32) {
        let (o, w, _) = self.levels[level as usize];
        (&self.rgba[o + rows.start as usize * w * 4..o + rows.end as usize * w * 4], w as u32)
    }

    /// Does the next piece of work: a band of zeroes, of a region or of mip rows. Returns `true` when everything
    /// is done.
    pub fn step(&mut self) -> Result<bool, String> {
        self.finished = None;
        match self.stage {
            Stage::Clear => {
                let end = (self.rgba.len() + CLEAR_BYTES).min(self.total);
                self.rgba.resize(end, 0);
                if end == self.total {
                    self.stage = self.after_region(None);
                }
            }
            Stage::Region { index, row } => {
                self.stage = match self.region(index, row)? {
                    Some(row) => Stage::Region { index, row },
                    None => self.after_region(Some(index)),
                };
            }
            Stage::Down { level, row } => {
                let (_, w, h) = self.levels[level];
                let end = (row + (STEP_TEXELS / w).max(1)).min(h);
                self.down_rows(level, row, end);
                self.stage = if end < h {
                    Stage::Down { level, row: end }
                } else if level + 1 < self.levels.len() {
                    Stage::Down { level: level + 1, row: 0 }
                } else {
                    Stage::Pull { level, row: 0 }
                };
            }
            Stage::Pull { level, row } => {
                let (_, w, h) = self.levels[level];
                let end = (row + (STEP_TEXELS / w).max(1)).min(h);
                if level + 1 < self.levels.len() {
                    self.pull_rows(level, row, end);
                }
                self.finished = Some((level as u32, row as u32..end as u32));
                self.stage = if end < h {
                    Stage::Pull { level, row: end }
                } else if level > 0 {
                    Stage::Pull { level: level - 1, row: 0 }
                } else {
                    Stage::Done
                };
            }
            Stage::Done => {}
        }
        Ok(self.stage == Stage::Done)
    }

    /// The stage after region `index` (`None`: the first region).
    pub(super) fn after_region(&self, index: Option<usize>) -> Stage {
        let next = index.map_or(0, |i| i + 1);
        if next < self.layout.regions.len() {
            Stage::Region { index: next, row: 0 }
        } else if self.levels.len() > 1 {
            Stage::Down { level: 1, row: 0 }
        } else {
            Stage::Pull { level: 0, row: 0 }
        }
    }

    /// Renders a band of region `index` from pixel row `row` into level 0 and returns the row after it (`None`:
    /// the region is done). The first time a tier at or above [`SOFT_TIER`] needs a soft layer, rendering it at
    /// that tier is the step. An SVG that does not parse leaves its layer transparent and is listed in
    /// [`Raster::failed`].
    pub(super) fn region(&mut self, index: usize, row: u32) -> Result<Option<u32>, String> {
        let layout = self.layout.clone();
        let r = &layout.regions[index];
        let resample = !r.soft_direct(self.tier);
        if resample && matches!(r.source, Source::Svg { soft: Some(_), .. }) && r.soft.get().is_none() {
            let _ = r.soft_layer();
            return Ok(Some(row));
        }
        if row == 0 {
            self.parsed = Some(self.parse(index, r));
        }
        let parsed = self.parsed.as_ref().filter(|p| p.index == index).ok_or("region out of order")?;
        let [px, py, pw, ph] = r.pixels(self.tier);
        let rows = if parsed.whole { ph - row } else { (STEP_TEXELS as u32 / pw).clamp(1, ph - row) };
        self.band.clear();
        self.band.resize((pw * rows * 4) as usize, 0);
        {
            let mut pm = tiny_skia::PixmapMut::from_bytes(&mut self.band, pw, rows).ok_or("empty region")?;
            match &r.source {
                Source::Proc(p) => procedural(*p, &mut pm, row, ph),
                Source::Svg { .. } => {
                    if let Some(tree) = &parsed.soft {
                        render_rows(tree, &mut pm, pw, ph, row);
                    } else if resample && let Some(Ok(soft)) = r.soft_layer() {
                        resample_rows(soft, &mut pm, pw, ph, row);
                    }
                    if let Some(tree) = &parsed.crisp {
                        render_rows(tree, &mut pm, pw, ph, row);
                    }
                }
            }
        }
        copy_region(&self.band, pw as usize, rows as usize, &mut self.rgba, self.width, px, py + row);
        let next = row + rows;
        Ok((next < ph).then_some(next))
    }

    /// Parses a region's SVG layers (at its first band).
    pub(super) fn parse(&mut self, index: usize, r: &RegionDef) -> Parsed {
        let Source::Svg { soft, crisp } = &r.source else {
            return Parsed { index, soft: None, crisp: None, whole: false };
        };
        let direct = r.soft_direct(self.tier);
        let mut parse = |svg: &str| match usvg::Tree::from_str(svg, &svg_options()) {
            Ok(tree) => Some(tree),
            Err(e) => {
                self.failed.push(format!("{}: {e}", r.name));
                None
            }
        };
        let soft = if direct { soft.as_deref().and_then(&mut parse) } else { None };
        let crisp = crisp.as_deref().and_then(&mut parse);
        if !direct && let Some(Err(e)) = r.soft_layer() {
            self.failed.push(format!("{}: {e}", r.name));
        }
        let whole = soft.is_some() || crisp.as_ref().is_some_and(|t| !t.filters().is_empty());
        Parsed { index, soft, crisp, whole }
    }

    /// Rows `y0..y1` of mip `level`: each texel the alpha-weighted average of its four texels one level finer.
    pub(super) fn down_rows(&mut self, level: usize, y0: usize, y1: usize) {
        let (o, nw, _) = self.levels[level];
        let (prev, lw, _) = self.levels[level - 1];
        let (fine, coarse) = self.rgba.split_at_mut(o);
        for y in y0..y1 {
            for x in 0..nw {
                let mut sum = [0u32; 3];
                let mut plain = [0u32; 3];
                let mut a_sum = 0u32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let i = prev + ((2 * y + dy) * lw + 2 * x + dx) * 4;
                    let a = fine[i + 3] as u32;
                    for c in 0..3 {
                        sum[c] += fine[i + c] as u32 * a;
                        plain[c] += fine[i + c] as u32;
                    }
                    a_sum += a;
                }
                let k = (y * nw + x) * 4;
                for c in 0..3 {
                    coarse[k + c] = sum[c].checked_div(a_sum).unwrap_or(plain[c] / 4) as u8;
                }
                coarse[k + 3] = ((a_sum + 2) / 4) as u8;
            }
        }
    }

    /// Pull, rows `y0..y1` of `level`: nearly transparent texels take the (alpha-weighted) colour of their parent
    /// texel, coarse to fine, so bilinear filtering never blends towards black or noisy colours at shape edges.
    pub(super) fn pull_rows(&mut self, level: usize, y0: usize, y1: usize) {
        let (o, lw, _) = self.levels[level];
        let (po, pw, ph) = self.levels[level + 1];
        let (fine, coarse) = self.rgba.split_at_mut(po);
        for y in y0..y1 {
            let py = (y / 2).min(ph - 1);
            for x in 0..lw {
                let i = o + (y * lw + x) * 4;
                if fine[i + 3] < SOLID {
                    let p = ((py * pw) + (x / 2).min(pw - 1)) * 4;
                    fine[i..i + 3].copy_from_slice(&coarse[p..p + 3]);
                }
            }
        }
    }

    /// Does everything that is left.
    pub fn run(&mut self) -> Result<(), String> {
        while !self.step()? {}
        Ok(())
    }

    /// The finished atlas (runs whatever work is left first).
    pub fn finish(mut self) -> Result<Raster, String> {
        self.run()?;
        Ok(Raster { tier: self.tier, width: self.width, height: self.height, levels: self.levels.len() as u32, data: self.rgba, failed: self.failed })
    }
}

/// usvg's options for content SVG: images (`<image>`, `<feImage>`) resolve to nothing. usvg's default resolver
/// reads any path it is given from the disk (the art must never touch files; a mod's `/dev/zero` would never end)
/// and decodes `data:` URLs (raster images are not allowed). `data::content::svg::check_svg` refuses such icons anyway.
pub(super) fn svg_options() -> usvg::Options<'static> {
    usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    }
}

/// Renders a whole layout at once.
pub fn rasterise(layout: &Arc<Layout>, tier: u32) -> Result<Raster, String> {
    Rasterizer::new(layout.clone(), tier).finish()
}

/// Renders `tree` scaled to fill a `w × h` region into `pm`, which holds the region's rows from `y0` on. Over what
/// is there already (source-over), as resvg always draws.
pub(super) fn render_rows(tree: &usvg::Tree, pm: &mut tiny_skia::PixmapMut, w: u32, h: u32, y0: u32) {
    let size = tree.size();
    let tf = tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height())
        .post_translate(0.0, -(y0 as f32));
    resvg::render(tree, tf, pm);
}

/// The soft layer (rendered at [`SOFT_TIER`]) resampled bilinearly into `pm`, which holds rows from `y0` on of the
/// region at `w × h` pixels. Pixel centres line up; at the same size it is an exact copy. Premultiplied in and out,
/// so the result stays valid premultiplied colour.
pub(super) fn resample_rows(soft: &Soft, pm: &mut tiny_skia::PixmapMut, w: u32, h: u32, y0: u32) {
    let (sw, sh) = (soft.w as usize, soft.h as usize);
    // The two source texels around a destination texel's centre and the weight of the second (of 256).
    let taps = |d: u32, dn: u32, sn: usize| {
        let s = ((d as f32 + 0.5) * sn as f32 / dn as f32 - 0.5).clamp(0.0, (sn - 1) as f32);
        let i = s as usize;
        (i, (i + 1).min(sn - 1), ((s - i as f32) * 256.0).round() as u32)
    };
    let cols: Vec<(usize, usize, u32)> = (0..w).map(|x| taps(x, w, sw)).collect();
    let rows = pm.height() as usize;
    let out = pm.data_mut();
    for y in 0..rows {
        let (r0, r1, fy) = taps(y0 + y as u32, h, sh);
        let top = &soft.rgba[r0 * sw * 4..(r0 + 1) * sw * 4];
        let bottom = &soft.rgba[r1 * sw * 4..(r1 + 1) * sw * 4];
        let line = &mut out[y * w as usize * 4..(y + 1) * w as usize * 4];
        for (px, &(c0, c1, fx)) in line.as_chunks_mut::<4>().0.iter_mut().zip(&cols) {
            for c in 0..4 {
                let t = top[c0 * 4 + c] as u32 * (256 - fx) + top[c1 * 4 + c] as u32 * fx;
                let b = bottom[c0 * 4 + c] as u32 * (256 - fx) + bottom[c1 * 4 + c] as u32 * fx;
                px[c] = ((t * (256 - fy) + b * fy + (1 << 15)) >> 16) as u8;
            }
        }
    }
}

/// Copies premultiplied pixels (`w × h`) into the straight-alpha atlas at `(px, py)` (colour fringes are fixed
/// later by the pull pass of the mip steps).
pub(super) fn copy_region(src: &[u8], w: usize, h: usize, atlas: &mut [u8], atlas_w: u32, px: u32, py: u32) {
    for y in 0..h {
        let row = ((py as usize + y) * atlas_w as usize + px as usize) * 4;
        for x in 0..w {
            let i = (y * w + x) * 4;
            let o = row + x * 4;
            let a = src[i + 3] as u32;
            for c in 0..3 {
                if let Some(v) = (src[i + c] as u32 * 255 + a / 2).checked_div(a) {
                    atlas[o + c] = v.min(255) as u8;
                }
            }
            atlas[o + 3] = a as u8;
        }
    }
}

pub(super) fn to_u8(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Procedural regions, written premultiplied (white × alpha) like tiny-skia output: rows from `y0` on of a region
/// `pm.width() × h` pixels large.
pub(super) fn procedural(p: Proc, pm: &mut tiny_skia::PixmapMut, y0: u32, h: u32) {
    let (w, rows) = (pm.width(), pm.height());
    let data = pm.data_mut();
    for y in 0..rows {
        for x in 0..w {
            let (u, v) = ((x as f32 + 0.5) / w as f32, ((y0 + y) as f32 + 0.5) / h as f32);
            let a = match p {
                Proc::White => 1.0,
                Proc::DotSoft => {
                    let r2 = ((u - 0.5) * (u - 0.5) + (v - 0.5) * (v - 0.5)) / 0.25;
                    if r2 >= 1.0 { 0.0 } else { (-(r2 * 9.0) / 2.0).exp() }
                }
                Proc::DotHard => {
                    // Anti-aliased disc of radius 0.46, supersampled 4x4.
                    let (pxu, pxv) = (1.0 / w as f32, 1.0 / h as f32);
                    let mut cov = 0.0;
                    for sy in 0..4 {
                        for sx in 0..4 {
                            let dx = u - 0.5 + (sx as f32 - 1.5) * pxu / 4.0;
                            let dy = v - 0.5 + (sy as f32 - 1.5) * pxv / 4.0;
                            if dx * dx + dy * dy <= 0.46 * 0.46 {
                                cov += 1.0 / 16.0;
                            }
                        }
                    }
                    cov
                }
                Proc::Aura { mid } => {
                    let t = ((u - 0.5).hypot(v - 0.5) / 0.5).min(1.0);
                    if t < 0.55 { 1.0 + (mid - 1.0) * t / 0.55 } else { mid * (1.0 - (t - 0.55) / 0.45) }
                }
                Proc::AuraLinear => 1.0 - ((u - 0.5).hypot(v - 0.5) / 0.5).min(1.0),
                Proc::Halo { hw, sigma, half } => {
                    let xc = (u - 0.5) * 2.0 * half;
                    let n = 48;
                    let mut s = 0.0;
                    for i in 0..n {
                        let t = -hw + (i as f32 + 0.5) / n as f32 * 2.0 * hw;
                        let d = (xc - t) / sigma;
                        s += (-0.5 * d * d).exp();
                    }
                    s * (2.0 * hw / n as f32) / (sigma * (2.0 * std::f32::consts::PI).sqrt())
                }
            };
            let a = to_u8(a);
            let i = ((y * w + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&[a, a, a, a]);
        }
    }
}
