//! Vector art, rasterised at runtime (rule: no fixed raster graphics).
//!
//! * **Sources**: the icons and the machines' ship drawings are content (`content/base/icons/<key>.svg` and
//!   `<key>.ship.svg`, `data::Content::icon_svg` / `ship_svg`; mods override them by key); the style tokens (`art/tokens.json`) are embedded at build time ([`tokens`]). FX
//!   textures (glow dots, halo profile, aura, plumes, barrel) are defined here as SVG or as functions of `(u, v)`.
//! * **Layout** ([`Layout`]): every region has a fixed rectangle in *atlas units* (1 unit = 1 ship cell at full
//!   density). A texture for a pixel density `tier` (pixels per cell) is the layout scaled by `tier`, so the
//!   normalised UV of a region never changes between tiers: meshes keep their UVs when the texture is swapped.
//!   Soft regions (glows, plumes, aura) are stored at half density (they are blurry anyway), the part icons at
//!   [`ICON_DENSITY`] (only the UI draws them, small; ships show the drawings, GDR-0069).
//! * **Rasteriser** ([`Rasterizer`]): clears its buffer, renders the regions, then builds an alpha-weighted mip
//!   chain and bleeds colour into transparent texels, every stage a band of rows at a time (`step`, about
//!   [`STEP_TEXELS`] texels), so a client can spread all the work over frames and upload each band as soon as
//!   it is final ([`Rasterizer::finished`]). Soft layers (icon glows, the plumes' blurred glow) look the same at
//!   every tier: they are rendered once at [`SOFT_TIER`] (kept in the [`Layout`]) and resampled for the higher
//!   tiers, so no step blurs a large region. Output: straight-alpha sRGB RGBA8, mip levels concatenated.
//!
//! No Bevy, no threads, no filesystem; builds for wasm32 and Android.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use resvg::{tiny_skia, usvg};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

/// SVG user units per ship cell in all art sources.
pub const UNITS_PER_CELL: f32 = 20.0;
/// Pixel-density tiers (pixels per cell). Powers of two; the top tier keeps the atlas within 4096 pixels.
pub const MIN_TIER: u32 = 32;
pub const MAX_TIER: u32 = 512;
/// Soft layers are rendered at this tier once and resampled for higher ones (blurred by 5 px or more here, so a
/// bilinear upsample is indistinguishable from rendering them at the tier, which blurred by up to 41 px at 512).
pub const SOFT_TIER: u32 = 128;
/// Density of the part icons in the atlas (texels per cell = `ICON_DENSITY × tier`): they are drawn in the UI only,
/// so a client asks for `1 / ICON_DENSITY` times the pixels per cell its icons need.
pub const ICON_DENSITY: f32 = 0.5;
/// Layout width in atlas units (the atlas is `LAYOUT_WIDTH × tier` pixels wide).
const LAYOUT_WIDTH: f32 = 8.0;
/// Gutter between regions, atlas units.
const GUTTER: f32 = 1.0 / 16.0;
/// Most mip levels generated.
const MAX_MIPS: u32 = 7;

/// The tier (pixels per cell) for a needed pixel density, rounded up to a power of two and clamped.
pub fn tier_for(px_per_cell: f32) -> u32 {
    let need = px_per_cell.max(1.0).ceil() as u32;
    need.next_power_of_two().clamp(MIN_TIER, MAX_TIER)
}

// ---------------------------------------------------------------------------------------------------------
// Tokens

/// `art/tokens.json`, flattened to `dotted.path → value` (arrays by index).
#[derive(Clone, Debug, Default)]
pub struct Tokens {
    map: BTreeMap<String, String>,
}

impl Tokens {
    pub fn get(&self, path: &str) -> Option<&str> {
        self.map.get(path).map(String::as_str)
    }
    pub fn f32(&self, path: &str) -> Option<f32> {
        self.get(path).and_then(|v| v.parse().ok())
    }
    fn must(&self, path: &str) -> &str {
        self.get(path).unwrap_or_else(|| panic!("style token `{path}` missing in art/tokens.json"))
    }
    fn must_f32(&self, path: &str) -> f32 {
        self.f32(path).unwrap_or_else(|| panic!("style token `{path}` is not a number"))
    }
}

/// The embedded style tokens.
pub fn tokens() -> Tokens {
    let v: serde_json::Value = serde_json::from_str(embedded::TOKENS_JSON).expect("art/tokens.json parses");
    let mut map = BTreeMap::new();
    flatten("", &v, &mut map);
    Tokens { map }
}

fn flatten(prefix: &str, v: &serde_json::Value, out: &mut BTreeMap<String, String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, v) in m {
                if !k.starts_with('_') {
                    flatten(&if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") }, v, out);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                flatten(&format!("{prefix}.{i}"), v, out);
            }
        }
        serde_json::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        other => {
            out.insert(prefix.to_string(), other.to_string());
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Layout

#[derive(Clone, Debug)]
enum Source {
    /// SVG documents with the same viewBox, rendered to fill the region, the crisp one over the soft one. The soft
    /// layer is blurred (an icon's glow, a plume's glow); the crisp one is rendered at the tier a band of rows at
    /// a time (whole when it uses a filter, which a band would cut off).
    Svg { soft: Option<String>, crisp: Option<String> },
    Proc(Proc),
}

/// A soft layer rendered at [`SOFT_TIER`]: premultiplied RGBA8, `w × h` (the region's size at that tier).
#[derive(Clone)]
struct Soft {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

impl std::fmt::Debug for Soft {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Soft({} x {})", self.w, self.h)
    }
}

#[derive(Clone, Copy, Debug)]
enum Proc {
    White,
    DotSoft,
    DotHard,
    /// Radial alpha: 1 → `mid` at 0.55 → 0.
    Aura { mid: f32 },
    AuraLinear,
    /// Box of half-width `hw` blurred by `sigma`, over ±`half` (cells).
    Halo { hw: f32, sigma: f32, half: f32 },
}

/// One region of the atlas.
#[derive(Clone, Debug)]
pub struct RegionDef {
    pub name: String,
    /// `[x, y, w, h]` in atlas units.
    pub rect: [f32; 4],
    /// Size of the art in ship cells (1 cell = 1 m).
    pub cells: [f32; 2],
    /// Anchor (nozzle, muzzle, centre) in cells from the region's top-left corner.
    pub origin: [f32; 2],
    source: Source,
    /// The soft layer at [`SOFT_TIER`], rendered the first time a tier at or above it needs it.
    soft: OnceLock<Result<Soft, String>>,
}

/// Where everything sits in the atlas, independent of the pixel density.
#[derive(Clone, Debug)]
pub struct Layout {
    pub regions: Vec<RegionDef>,
    /// `[width, height]` in atlas units.
    pub size: [f32; 2],
    index: BTreeMap<String, usize>,
}

impl Layout {
    /// Every content icon, its glow layer and the FX textures, packed into shelves. An icon that fails
    /// `data::svg::check_svg` (impossible in validated content) gets no region; one too big for the layout
    /// width is stored at a lower density.
    pub fn new(content: &data::Content) -> Layout {
        let t = tokens();
        let mut regs: Vec<(RegionDef, [f32; 2])> = Vec::new();
        let mut add = |name: String, size_u: [f32; 2], cells: [f32; 2], origin: [f32; 2], source: Source| {
            regs.push((RegionDef { name, rect: [0.0; 4], cells, origin, source, soft: OnceLock::new() }, size_u));
        };

        // A drawing and its glow layer (`<g filter="url(#gl)">` groups, rendered soft at half density).
        let mut drawing = |name: String, glow_name: String, src: &str, density: f32| {
            let Some([_, _, vw, vh]) = data::svg::view_box(src) else { return };
            let cells = [vw / UNITS_PER_CELL, vh / UNITS_PER_CELL];
            let (crisp, glow) = split_glow(src);
            // The glow is clipped to the region anyway: limit the blur's filter region to the viewBox (the sources
            // use 220 % of the bounding box, ~5x the pixels).
            let glow = glow.map(|g| {
                g.replace(
                    "<filter id=\"gl\" x=\"-60%\" y=\"-60%\" width=\"220%\" height=\"220%\">",
                    &format!("<filter id=\"gl\" filterUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"{vw}\" height=\"{vh}\">"),
                )
            });
            let size = [cells[0] * density, cells[1] * density];
            add(name, size, cells, [0.0; 2], Source::Svg { soft: None, crisp: Some(crisp) });
            if let Some(g) = glow {
                let half = [size[0] / 2.0, size[1] / 2.0];
                add(glow_name, half, cells, [0.0; 2], Source::Svg { soft: Some(g), crisp: None });
            }
        };
        // Machines on ships (GDR-0069), at full density: the world zooms in on them.
        for (key, art) in &content.ship_art {
            drawing(format!("ship/{key}"), format!("ship-glow/{key}"), &art.svg, 1.0);
        }
        // Part icons: only the part tiles in the UI draw them (the palette, the inspector), never larger than
        // about 40 px per cell, so they are stored at [`ICON_DENSITY`]. Material and category icons are for the
        // wiki: the game never draws them.
        let part_icons: std::collections::BTreeSet<&str> = content.parts.iter().filter_map(|p| p.icon.as_deref()).collect();
        for (key, icon) in content.icons.iter().filter(|(k, _)| part_icons.contains(k.as_str())) {
            drawing(format!("icon/{key}"), format!("glow/{key}"), &icon.svg, ICON_DENSITY);
        }

        add("fx/white".into(), [0.125; 2], [0.125; 2], [0.0; 2], Source::Proc(Proc::White));
        add("fx/dot_soft".into(), [0.5; 2], [0.5; 2], [0.25; 2], Source::Proc(Proc::DotSoft));
        add("fx/dot_hard".into(), [0.5; 2], [0.5; 2], [0.25; 2], Source::Proc(Proc::DotHard));
        let c = t.must_f32("ship.aura.opacity_center");
        let mid = t.must_f32("ship.aura.opacity_mid") / c;
        add("fx/aura".into(), [0.75; 2], [1.0; 2], [0.5; 2], Source::Proc(Proc::Aura { mid }));
        add("fx/aura_linear".into(), [0.75; 2], [1.0; 2], [0.5; 2], Source::Proc(Proc::AuraLinear));
        let hw = t.must_f32("ship.hull_glow.halo_width_cells") / 2.0;
        let sigma = t.must_f32("ship.hull_glow.halo_blur_std_cells");
        let half = hw + 3.0 * sigma;
        add("fx/halo".into(), [0.5, 0.0625], [2.0 * half, 2.0 * half / 8.0], [half, 0.0], Source::Proc(Proc::Halo { hw, sigma, half }));

        // Engine plumes: gradient triangle over a blurred copy, nozzle at the origin, pointing down (+y). Shapes
        // and lengths follow drawFlame() in docs/concept/reference/concept-board-script.js at full thrust.
        // `(colour, opacity, offset)` per stop; the enemy plume is gPlR in art/ships/automaton-raider.svg.
        let stops = |gradient: &str| -> Vec<(String, String, String)> {
            let stop = |i: usize, j: usize| t.must(&format!("ship.plume.{gradient}.{i}.{j}")).to_string();
            (0..3).map(|i| (stop(i, 0), stop(i, 1), stop(i, 2))).collect()
        };
        let (teal_stops, coral_stops) = (stops("gradient"), stops("gradient_enemy"));
        let glow_op = t.must_f32("ship.plume.glow_opacity");
        let glow_sigma = t.must_f32("ship.plume.glow_blur_std_cells") * UNITS_PER_CELL;
        let teal = t.must("ship.hull_glow.color").to_string();
        let coral = t.must("ship.hull_glow.enemy_color").to_string();
        for (size, half_w, len) in [("small", 0.3f32, 1.15f32), ("large", 0.75, 2.05)] {
            for (tint, stops, glow_col) in [("teal", &teal_stops, &teal), ("coral", &coral_stops, &coral)] {
                let (hw, l) = (half_w * UNITS_PER_CELL, len * UNITS_PER_CELL);
                let m = (3.0 * glow_sigma).ceil();
                let (x0, y0, vw, vh) = (-hw - m, -m, 2.0 * (hw + m), l + 2.0 * m);
                let mut stop_xml = String::new();
                for (col, op, off) in stops.iter() {
                    let _ = write!(stop_xml, "<stop offset=\"{off}\" stop-color=\"{col}\" stop-opacity=\"{op}\"/>");
                }
                // The blurred glow (soft) and the gradient triangle over it (crisp), as two documents.
                let open = format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{vw}\" height=\"{vh}\" viewBox=\"{x0} {y0} {vw} {vh}\">"
                );
                let points = format!("{a},0 {hw},0 0,{l}", a = -hw);
                let soft = format!(
                    "{open}<defs><filter id=\"b\" filterUnits=\"userSpaceOnUse\" x=\"{x0}\" y=\"{y0}\" width=\"{vw}\" height=\"{vh}\"><feGaussianBlur stdDeviation=\"{glow_sigma}\"/></filter></defs>\
                     <polygon points=\"{points}\" fill=\"{glow_col}\" opacity=\"{glow_op}\" filter=\"url(#b)\"/></svg>"
                );
                let crisp = format!(
                    "{open}<defs><linearGradient id=\"p\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\">{stop_xml}</linearGradient></defs>\
                     <polygon points=\"{points}\" fill=\"url(#p)\"/></svg>"
                );
                let cells = [vw / UNITS_PER_CELL, vh / UNITS_PER_CELL];
                add(
                    format!("fx/plume_{size}_{tint}"),
                    [cells[0] / 2.0, cells[1] / 2.0],
                    cells,
                    [-x0 / UNITS_PER_CELL, -y0 / UNITS_PER_CELL],
                    Source::Svg { soft: Some(soft), crisp: Some(crisp) },
                );
            }
        }

        // Gun barrel (halvex-runner.svg): 5 x 16 units, metal gradient, dark rim. Origin = muzzle end.
        let offsets = [0.0, 0.45, 0.7, 1.0];
        let mut stop_xml = String::new();
        for (i, o) in offsets.iter().enumerate() {
            let _ = write!(stop_xml, "<stop offset=\"{o}\" stop-color=\"{}\"/>", t.must(&format!("ship.barrel_gradient.{i}")));
        }
        let (bx0, by0, bw, bh) = (-3.0f32, -0.5f32, 6.0f32, 17.0f32);
        let stroke = t.must("ship.barrel_stroke");
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{bw}\" height=\"{bh}\" viewBox=\"{bx0} {by0} {bw} {bh}\">\
             <defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"0\">{stop_xml}</linearGradient></defs>\
             <rect x=\"-2.5\" y=\"0\" width=\"5\" height=\"16\" rx=\"1\" fill=\"url(#g)\" stroke=\"{stroke}\" stroke-width=\".6\"/></svg>"
        );
        let cells = [bw / UNITS_PER_CELL, bh / UNITS_PER_CELL];
        let origin = [-bx0 / UNITS_PER_CELL, -by0 / UNITS_PER_CELL];
        add("fx/barrel".into(), cells, cells, origin, Source::Svg { soft: None, crisp: Some(svg) });

        pack(regs)
    }

    pub fn get(&self, name: &str) -> Option<&RegionDef> {
        self.index.get(name).map(|&i| &self.regions[i])
    }

    /// Normalised `[u0, v0, u1, v1]` of a region (the same at every tier).
    pub fn uv(&self, r: &RegionDef) -> [f32; 4] {
        let [x, y, w, h] = r.rect;
        [x / self.size[0], y / self.size[1], (x + w) / self.size[0], (y + h) / self.size[1]]
    }

    /// Atlas size in pixels at a tier.
    pub fn pixel_size(&self, tier: u32) -> (u32, u32) {
        ((self.size[0] * tier as f32).round() as u32, (self.size[1] * tier as f32).round() as u32)
    }
}

impl RegionDef {
    /// `[x, y, w, h]` in pixels at a tier.
    fn pixels(&self, tier: u32) -> [u32; 4] {
        let t = tier as f32;
        let [x, y, w, h] = self.rect;
        [(x * t).round() as u32, (y * t).round() as u32, (w * t).round().max(1.0) as u32, (h * t).round().max(1.0) as u32]
    }

    /// Whether a soft layer is rendered at `tier` itself rather than resampled from [`SOFT_TIER`]: below that tier,
    /// and for regions small enough to blur in one step even at the tier (the part icons' glows, which are at
    /// [`ICON_DENSITY`] and so only a few pixels across at [`SOFT_TIER`], too few to resample from).
    fn soft_direct(&self, tier: u32) -> bool {
        let [_, _, w, h] = self.pixels(tier);
        tier < SOFT_TIER || (w * h) as usize <= STEP_TEXELS
    }

    /// The soft layer at [`SOFT_TIER`], rendered the first time it is asked for (`None`: the region has none).
    fn soft_layer(&self) -> Option<&Result<Soft, String>> {
        let Source::Svg { soft: Some(svg), .. } = &self.source else { return None };
        Some(self.soft.get_or_init(|| {
            let tree = usvg::Tree::from_str(svg, &svg_options()).map_err(|e| e.to_string())?;
            let [_, _, w, h] = self.pixels(SOFT_TIER);
            let mut pm = tiny_skia::Pixmap::new(w, h).ok_or("empty region")?;
            render_rows(&tree, &mut pm.as_mut(), w, h, 0);
            Ok(Soft { w, h, rgba: pm.take() })
        }))
    }
}

fn snap(x: f32) -> f32 {
    (x / GUTTER).ceil() * GUTTER
}

/// Skyline packing in atlas units: tallest first, each region at the lowest spot of the skyline it fits (the
/// leftmost of equals) within rows of `LAYOUT_WIDTH`. Positions are multiples of the gutter, the height is rounded
/// up so every mip level has an integer size at the smallest tier. A region wider or taller than a row is scaled
/// down (keeping its aspect) to fit one. (Shelves until 13 Oct 2026: they left a third of the atlas empty beside
/// the tall regions, and the machines' ship drawings did not fit 4096 px at the top tier with them.)
fn pack(mut regs: Vec<(RegionDef, [f32; 2])>) -> Layout {
    const EPS: f32 = 1e-4;
    let most = LAYOUT_WIDTH - 2.0 * GUTTER;
    for (_, [w, h]) in &mut regs {
        let k = (most / w.max(*h)).min(1.0);
        (*w, *h) = (*w * k, *h * k);
    }
    let mut order: Vec<usize> = (0..regs.len()).collect();
    order.sort_by(|&a, &b| {
        regs[b].1[1].total_cmp(&regs[a].1[1]).then(regs[b].1[0].total_cmp(&regs[a].1[0])).then(regs[a].0.name.cmp(&regs[b].0.name))
    });
    // The skyline: steps `[x, y]` by ascending x, each up to the next one (the last to the layout's width); a
    // region goes at a step's x, as high as the steps under its width (gutter included) reach.
    let mut sky: Vec<[f32; 2]> = vec![[GUTTER, GUTTER]];
    let mut bottom = 0.0f32;
    for &i in &order {
        let [w, h] = regs[i].1;
        let span = snap(w + GUTTER);
        let mut best: Option<[f32; 2]> = None;
        for (k, &[x, _]) in sky.iter().enumerate() {
            if x + w + GUTTER > LAYOUT_WIDTH + EPS {
                break;
            }
            let y = sky[k..].iter().take_while(|s| s[0] < x + span - EPS).fold(0.0f32, |m, s| m.max(s[1]));
            if best.is_none_or(|b| y < b[1] - EPS) {
                best = Some([x, y]);
            }
        }
        // The first step starts at the gutter and every region fits a row.
        let [x, y] = best.expect("a region fits a row");
        regs[i].0.rect = [x, y, w, h];
        bottom = bottom.max(y + h);
        let end = x + span;
        let resume = sky.iter().take_while(|s| s[0] < end + EPS).last().map_or(GUTTER, |s| s[1]);
        let mut next: Vec<[f32; 2]> = sky.iter().copied().filter(|s| s[0] < x - EPS).collect();
        next.push([x, snap(y + h + GUTTER)]);
        if end < LAYOUT_WIDTH - EPS {
            next.push([end, resume]);
            next.extend(sky.iter().copied().filter(|s| s[0] > end + EPS));
        }
        next.dedup_by(|b, a| (a[1] - b[1]).abs() < EPS);
        sky = next;
    }
    let height = ((bottom + GUTTER) * 2.0).ceil() / 2.0; // multiple of 0.5 unit: integral mips down to 16 px
    let regions: Vec<RegionDef> = regs.into_iter().map(|r| r.0).collect();
    let index = regions.iter().enumerate().map(|(i, r)| (r.name.clone(), i)).collect();
    Layout { regions, size: [LAYOUT_WIDTH, height], index }
}

/// Splits an icon SVG into (crisp glyph, glow-only) documents. The glow is every `<g filter="url(#gl)" ...>`
/// element (with its opacity) after `</defs>`.
fn split_glow(src: &str) -> (String, Option<String>) {
    let defs_end = src.find("</defs>").map(|i| i + "</defs>".len()).unwrap_or_else(|| src.find('>').map_or(0, |i| i + 1));
    // A `</defs>` in a comment after the root must not cut the body backwards.
    let svg_end = src.rfind("</svg>").filter(|&e| e >= defs_end).unwrap_or(src.len());
    let head = &src[..defs_end];
    let body = &src[defs_end..svg_end];
    let mut crisp = String::new();
    let mut glow = String::new();
    let mut rest = body;
    while let Some(start) = rest.find("<g filter=\"url(#gl)\"") {
        crisp.push_str(&rest[..start]);
        let len = balanced_g_len(&rest[start..]);
        glow.push_str(&rest[start..start + len]);
        rest = &rest[start + len..];
    }
    crisp.push_str(rest);
    (format!("{head}{crisp}</svg>"), (!glow.is_empty()).then(|| format!("{head}{glow}</svg>")))
}

/// The length in bytes of the `<g ...>` element `s` starts with, up to its matching `</g>` (all of `s` if it is
/// never closed). Steps through bytes, so any UTF-8 text inside is fine; every returned length ends after an
/// ASCII `>` or at the end of `s`, a char boundary.
fn balanced_g_len(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"</g>") {
            depth -= 1;
            i += 4;
            if depth == 0 {
                return i;
            }
            continue;
        }
        if bytes[i..].starts_with(b"<g") && matches!(bytes.get(i + 2), Some(b' ' | b'>')) {
            let Some(close) = bytes[i..].iter().position(|&b| b == b'>').map(|c| c + i) else {
                return s.len();
            };
            if bytes[close - 1] != b'/' {
                depth += 1;
            }
            i = close + 1;
            continue;
        }
        i += 1;
    }
    s.len()
}

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
    width: u32,
    height: u32,
    /// Every mip level, concatenated; grows to its full size while [`Stage::Clear`] zeroes it.
    rgba: Vec<u8>,
    /// The full size of `rgba` in bytes.
    total: usize,
    /// `(offset in bytes, width, height)` per mip level.
    levels: Vec<(usize, usize, usize)>,
    stage: Stage,
    /// The SVG layers of the region in progress, parsed at its first band.
    parsed: Option<Parsed>,
    /// One band's pixels, premultiplied (tiny-skia's format), reused.
    band: Vec<u8>,
    /// The rows the last step made final: `(mip level, rows)`.
    finished: Option<(u32, Range<u32>)>,
    failed: Vec<String>,
}

/// The parsed layers of the region in progress.
struct Parsed {
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
enum Stage {
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
const CLEAR_BYTES: usize = 1 << 21;
/// Texels below this alpha take their colour from the coarser level (see [`Rasterizer::pull_rows`]).
const SOLID: u8 = 24;

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
    fn after_region(&self, index: Option<usize>) -> Stage {
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
    fn region(&mut self, index: usize, row: u32) -> Result<Option<u32>, String> {
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
    fn parse(&mut self, index: usize, r: &RegionDef) -> Parsed {
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
    fn down_rows(&mut self, level: usize, y0: usize, y1: usize) {
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
    fn pull_rows(&mut self, level: usize, y0: usize, y1: usize) {
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
/// and decodes `data:` URLs (raster images are not allowed). `data::svg::check_svg` refuses such icons anyway.
fn svg_options() -> usvg::Options<'static> {
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
fn render_rows(tree: &usvg::Tree, pm: &mut tiny_skia::PixmapMut, w: u32, h: u32, y0: u32) {
    let size = tree.size();
    let tf = tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height())
        .post_translate(0.0, -(y0 as f32));
    resvg::render(tree, tf, pm);
}

/// The soft layer (rendered at [`SOFT_TIER`]) resampled bilinearly into `pm`, which holds rows from `y0` on of the
/// region at `w × h` pixels. Pixel centres line up; at the same size it is an exact copy. Premultiplied in and out,
/// so the result stays valid premultiplied colour.
fn resample_rows(soft: &Soft, pm: &mut tiny_skia::PixmapMut, w: u32, h: u32, y0: u32) {
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
fn copy_region(src: &[u8], w: usize, h: usize, atlas: &mut [u8], atlas_w: u32, px: u32, py: u32) {
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

fn to_u8(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Procedural regions, written premultiplied (white × alpha) like tiny-skia output: rows from `y0` on of a region
/// `pm.width() × h` pixels large.
fn procedural(p: Proc, pm: &mut tiny_skia::PixmapMut, y0: u32, h: u32) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon key used by content has an SVG, and the whole atlas rasterises at two tiers: every part's icon
    /// (at [`ICON_DENSITY`]) and every machine's ship drawing (GDR-0069, at full density, an engine's with its
    /// overhang) are there and not empty. Icons only the wiki shows (materials) are not in the atlas.
    #[test]
    fn content_icons_and_ship_drawings_rasterise_at_two_tiers() {
        let content = data::base_content();
        let mut missing = Vec::new();
        for part in content.parts.iter() {
            if let Some(icon) = &part.icon
                && content.icon_svg(icon).is_none()
            {
                missing.push(format!("{} -> {icon}", part.id));
            }
        }
        assert!(missing.is_empty(), "icons without SVG: {missing:?}");
        let layout = Arc::new(Layout::new(&content));
        let mut regions = Vec::new();
        for part in content.parts.iter() {
            let Some(icon) = &part.icon else { continue };
            let reg = layout.get(&format!("icon/{icon}")).expect("icon region");
            assert_eq!(reg.rect[2..], reg.cells.map(|c| c * ICON_DENSITY), "{icon}");
            regions.push(reg);
            if part.hull {
                continue;
            }
            let ship = layout.get(&format!("ship/{icon}")).unwrap_or_else(|| panic!("ship drawing of {}", part.id));
            assert_eq!(ship.rect[2..], ship.cells, "{icon}");
            let over = ship.cells[1] - part.size.1 as f32;
            assert_eq!(over > 0.0, part.engine.is_some(), "{}: only engines reach past the hull", part.id);
            regions.push(ship);
        }
        assert!(layout.get("icon/armor-steel").is_none(), "material icons are for the wiki");
        for tier in [64, 256] {
            let r = rasterise(&layout, tier).expect("rasterises");
            assert_eq!((r.width, r.height), layout.pixel_size(tier));
            assert!(r.levels >= 3, "mips at tier {tier}");
            for reg in &regions {
                let [x, y, w, h] = reg.pixels(tier);
                let mut alpha = 0u64;
                for yy in y..y + h {
                    for xx in x..x + w {
                        alpha += r.data[((yy * r.width + xx) * 4 + 3) as usize] as u64;
                    }
                }
                assert!(alpha > 0, "{} is empty at tier {tier}", reg.name);
            }
        }
    }

    /// Every step does a bounded piece of work (a structural bound, not a timing): [`CLEAR_BYTES`] of zeroes, or
    /// at most [`STEP_TEXELS`] texels (one row when a row is wider) of a region, a soft layer or a mip level. Every
    /// row of every mip level is reported final exactly once, coarsest level first, with the bytes it ends with.
    #[test]
    fn steps_are_bounded_and_report_every_row_once() {
        let layout = Arc::new(Layout::new(&data::base_content()));
        let tier = 256;
        let mut r = Rasterizer::new(layout.clone(), tier);
        let mut reported: Vec<(u32, Range<u32>, Vec<u8>)> = Vec::new();
        loop {
            let (before, len) = (r.stage, r.rgba.len());
            let done = r.step().unwrap();
            match before {
                Stage::Clear => assert!(r.rgba.len() - len <= CLEAR_BYTES),
                Stage::Region { index, row } => {
                    let reg = &layout.regions[index];
                    let [_, _, w, h] = reg.pixels(tier);
                    let end = match r.stage {
                        Stage::Region { index: i, row } if i == index => row,
                        _ => h,
                    };
                    // No rows: the step rendered the soft layer at SOFT_TIER.
                    let [_, _, sw, sh] = reg.pixels(SOFT_TIER);
                    let (texels, width) = if end == row { (sw * sh, sw) } else { ((end - row) * w, w) };
                    assert!(texels as usize <= STEP_TEXELS.max(width as usize), "{} rows {row}..{end}", reg.name);
                }
                Stage::Down { .. } | Stage::Pull { .. } | Stage::Done => {}
            }
            if let Some((level, rows)) = r.finished() {
                let (data, width) = r.rows(level, rows.clone());
                assert!(data.len() / 4 <= STEP_TEXELS.max(width as usize));
                reported.push((level, rows, data.to_vec()));
            }
            if done {
                break;
            }
        }
        assert!(reported.windows(2).all(|w| w[0].0 >= w[1].0), "coarsest level first");
        let raster = r.finish().unwrap();
        let mut offset = 0;
        let (mut w, mut h) = (raster.width as usize, raster.height as usize);
        for level in 0..raster.levels {
            let mut next = 0;
            for (_, rows, data) in reported.iter().filter(|b| b.0 == level) {
                assert_eq!(rows.start as usize, next, "level {level}");
                next = rows.end as usize;
                let at = offset + rows.start as usize * w * 4;
                assert!(raster.data[at..at + data.len()] == data[..], "level {level} rows {rows:?} changed");
            }
            assert_eq!(next, h, "level {level}");
            offset += w * h * 4;
            (w, h) = (w / 2, h / 2);
        }
        assert_eq!(offset, raster.data.len());
    }

    /// Regions rendered a band at a time, and soft layers resampled from [`SOFT_TIER`], look like each region
    /// rendered whole at the tier (as until 13 Oct 2026): soft layers within a few levels of 255, the rest except a
    /// few texels along curves (tiny-skia flattens a curve cut at a band edge a little differently).
    #[test]
    fn bands_and_resampled_soft_layers_match_a_whole_render() {
        let layout = Arc::new(Layout::new(&data::base_content()));
        let tier = 256;
        let mut r = Rasterizer::new(layout.clone(), tier);
        while !matches!(r.stage, Stage::Down { .. }) {
            r.step().unwrap();
        }
        let premultiplied = |p: &[u8]| -> [u32; 4] {
            let a = p[3] as u32;
            [(p[0] as u32 * a + 127) / 255, (p[1] as u32 * a + 127) / 255, (p[2] as u32 * a + 127) / 255, a]
        };
        for reg in &layout.regions {
            let [px, py, w, h] = reg.pixels(tier);
            let mut pm = tiny_skia::Pixmap::new(w, h).unwrap();
            match &reg.source {
                Source::Proc(p) => procedural(*p, &mut pm.as_mut(), 0, h),
                Source::Svg { soft, crisp } => {
                    for svg in [soft, crisp].into_iter().flatten() {
                        render_rows(&usvg::Tree::from_str(svg, &svg_options()).unwrap(), &mut pm.as_mut(), w, h, 0);
                    }
                }
            }
            let mut whole = vec![0; (w * h * 4) as usize];
            copy_region(pm.data(), w as usize, h as usize, &mut whole, w, 0, 0);
            let (mut max, mut sum, mut off) = (0u32, 0u64, 0usize);
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let i = ((py as usize + y) * r.width as usize + px as usize + x) * 4;
                    let a = premultiplied(&r.rgba[i..i + 4]);
                    let b = premultiplied(&whole[(y * w as usize + x) * 4..][..4]);
                    for c in 0..4 {
                        let d = a[c].abs_diff(b[c]);
                        (max, sum, off) = (max.max(d), sum + d as u64, off + (d > 2) as usize);
                    }
                }
            }
            let n = (w * h * 4) as f64;
            let soft_only = matches!(reg.source, Source::Svg { soft: Some(_), crisp: None });
            assert!(!soft_only || max <= 4, "{}: max {max}", reg.name);
            assert!(sum as f64 / n <= 0.5, "{}: mean {}", reg.name, sum as f64 / n);
            assert!(off as f64 / n <= 0.01, "{}: {off} of {n} values off by more than 2", reg.name);
        }
    }

    #[test]
    fn uv_is_tier_independent_and_tiers_fit() {
        let layout = Layout::new(&data::base_content());
        for tier in [MIN_TIER, 64, 128, 256, MAX_TIER] {
            let (w, h) = layout.pixel_size(tier);
            assert!(w <= 4096 && h <= 4096, "tier {tier}: {w}x{h}");
        }
        assert_eq!(tier_for(10.0), MIN_TIER);
        assert_eq!(tier_for(100.0), 128);
        assert_eq!(tier_for(5000.0), MAX_TIER);
    }

    #[test]
    fn tokens_parse() {
        let t = tokens();
        assert_eq!(t.get("ship.machine_tile"), Some("#0f1c25"));
    }

    /// Content (unvalidated) with these SVGs as ship drawings (the atlas holds every drawing at full density).
    fn with_drawings(icons: &[(&str, String)]) -> data::Content {
        let mut files = vec![(
            "pack.ron".to_string(),
            r#"(id: "m", name: "M", version: "1", schema_version: 1)"#.to_string(),
        )];
        files.extend(icons.iter().map(|(k, svg)| (format!("icons/{k}.ship.svg"), svg.clone())));
        data::Content::merge(&[data::Pack::from_files(files).unwrap()])
    }

    /// Drawings that pass `check_svg` but used to stop the layout (review finding 3): another quote style, commas,
    /// a viewBox wider than a layout row, non-ASCII text inside a glow group, `</defs>` in a trailing comment.
    #[test]
    fn every_drawing_that_checks_lays_out_and_rasterises() {
        let glow = r##"<defs><filter id="gl" x="-60%" y="-60%" width="220%" height="220%"><feGaussianBlur stdDeviation="1"/></filter></defs>"##;
        let icons = [
            ("quotes", "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 20 20'><rect width='5' height='5'/></svg>".to_string()),
            ("commas", r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0,0,20,20"><rect width="5" height="5"/></svg>"#.into()),
            ("wide", r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 20"><rect width="400" height="5"/></svg>"#.into()),
            ("tall", r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10000"><rect width="5" height="5"/></svg>"#.into()),
            ("tiny", r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 0.001 0.001"><rect width="1" height="1"/></svg>"#.into()),
            (
                "umlaut",
                format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20">{glow}<g filter="url(#gl)" data-label="Ebene ä — ✓"><!-- ü --><g><rect width="5" height="5"/></g>ö</g><rect width="2" height="2"/></svg>"##
                ),
            ),
            ("unclosed-glow", format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20">{glow}<g filter="url(#gl)">é</g></svg><!-- </defs> é -->"##)),
        ];
        for (key, svg) in &icons {
            assert_eq!(data::svg::check_svg(svg), Ok(()), "{key}");
        }
        let layout = Arc::new(Layout::new(&with_drawings(&icons)));
        for (key, svg) in &icons {
            let r = layout.get(&format!("ship/{key}")).unwrap_or_else(|| panic!("{key}"));
            let [_, _, vw, vh] = data::svg::view_box(svg).unwrap();
            assert_eq!(r.cells, [vw / UNITS_PER_CELL, vh / UNITS_PER_CELL], "{key}");
            assert!(r.rect[2] <= LAYOUT_WIDTH - 2.0 * GUTTER && r.rect[3] <= LAYOUT_WIDTH - 2.0 * GUTTER, "{key}");
        }
        let r = rasterise(&layout, MIN_TIER).unwrap();
        assert_eq!(r.failed, Vec::<String>::new());
    }

    /// An icon the XML parser refuses although `check_svg` passes it (an undeclared namespace prefix) stays blank;
    /// the rest of the atlas is fine.
    #[test]
    fn a_region_that_does_not_parse_stays_blank() {
        let icons = [
            ("prefix", r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect inkscape:label="x" width="5" height="5"/></svg>"#.to_string()),
            ("fine", r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect width="5" height="5"/></svg>"#.into()),
        ];
        let layout = Arc::new(Layout::new(&with_drawings(&icons)));
        let r = rasterise(&layout, MIN_TIER).unwrap();
        assert_eq!(r.failed.len(), 1);
        assert!(r.failed[0].starts_with("ship/prefix: "), "{:?}", r.failed);
    }

    /// Icons that fail `check_svg` get no region instead of a panic (unvalidated content only).
    #[test]
    fn unusable_icons_are_left_out() {
        let icons = [
            ("no-view-box", r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="5" height="5"/></svg>"#.to_string()),
            ("broken", "<svg".into()),
            ("not-svg", "é".repeat(100)),
        ];
        let layout = Layout::new(&with_drawings(&icons));
        for (key, _) in &icons {
            assert!(layout.get(&format!("ship/{key}")).is_none(), "{key}");
        }
    }

    /// usvg never reads a file or decodes an embedded image for the art (review finding 6).
    #[test]
    fn svg_images_resolve_to_nothing() {
        let dir = std::env::temp_dir().join(format!("art-resolver-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("inner.svg");
        std::fs::write(&file, r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4"/></svg>"#).unwrap();
        // The same 4 × 4 black square, base64.
        let embedded = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnIHdpZHRoPSc0JyBoZWlnaHQ9JzQnPjxyZWN0IHdpZHRoPSc0JyBoZWlnaHQ9JzQnLz48L3N2Zz4=";
        for href in [file.to_string_lossy().into_owned(), embedded.to_string()] {
            for element in [
                format!(r#"<image href="{href}" width="4" height="4"/>"#),
                format!(r#"<filter id="f"><feImage href="{href}"/></filter><rect width="4" height="4" filter="url(#f)"/>"#),
            ] {
                let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">{element}</svg>"#);
                let loaded = |options: &usvg::Options| {
                    let tree = usvg::Tree::from_str(&svg, options).unwrap();
                    let mut pm = tiny_skia::Pixmap::new(4, 4).unwrap();
                    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pm.as_mut());
                    pm.pixels().iter().any(|p| p.alpha() > 0)
                };
                assert!(loaded(&usvg::Options::default()), "usvg's default loads {element}");
                assert!(!loaded(&svg_options()), "{element}");
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
