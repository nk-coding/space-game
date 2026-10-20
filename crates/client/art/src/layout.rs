//! The atlas layout: every region (icons, ship drawings, glows, effect textures) packed into one sheet in cell units,
//! resolution independent; a tier scales it to pixels.

use super::*;

// ---------------------------------------------------------------------------------------------------------
// Layout

#[derive(Clone, Debug)]
pub(super) enum Source {
    /// SVG documents with the same viewBox, rendered to fill the region, the crisp one over the soft one. The soft
    /// layer is blurred (an icon's glow, a plume's glow); the crisp one is rendered at the tier a band of rows at
    /// a time (whole when it uses a filter, which a band would cut off).
    Svg { soft: Option<String>, crisp: Option<String> },
    Proc(Proc),
}

/// A soft layer rendered at [`SOFT_TIER`]: premultiplied RGBA8, `w × h` (the region's size at that tier).
#[derive(Clone)]
pub(super) struct Soft {
    pub(super) w: u32,
    pub(super) h: u32,
    pub(super) rgba: Vec<u8>,
}

impl std::fmt::Debug for Soft {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Soft({} x {})", self.w, self.h)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Proc {
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
    pub(super) source: Source,
    /// The soft layer at [`SOFT_TIER`], rendered the first time a tier at or above it needs it.
    pub(super) soft: OnceLock<Result<Soft, String>>,
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
    /// `data::content::svg::check_svg` (impossible in validated content) gets no region; one too big for the layout
    /// width is stored at a lower density.
    pub fn new(content: &data::Content) -> Layout {
        let t = tokens();
        let mut regs: Vec<(RegionDef, [f32; 2])> = Vec::new();
        let mut add = |name: String, size_u: [f32; 2], cells: [f32; 2], origin: [f32; 2], source: Source| {
            regs.push((RegionDef { name, rect: [0.0; 4], cells, origin, source, soft: OnceLock::new() }, size_u));
        };

        // A drawing and its glow layer (`<g filter="url(#gl)">` groups, rendered soft at half density).
        let mut drawing = |name: String, glow_name: String, src: &str, density: f32| {
            let Some([_, _, vw, vh]) = data::content::svg::view_box(src) else { return };
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
        // and lengths follow the concept board's flames at full thrust. `(colour, opacity, offset)` per stop.
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

        // Gun barrel (as in the concept art): 5 x 16 units, metal gradient, dark rim. Origin = muzzle end.
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
    pub(super) fn pixels(&self, tier: u32) -> [u32; 4] {
        let t = tier as f32;
        let [x, y, w, h] = self.rect;
        [(x * t).round() as u32, (y * t).round() as u32, (w * t).round().max(1.0) as u32, (h * t).round().max(1.0) as u32]
    }

    /// Whether a soft layer is rendered at `tier` itself rather than resampled from [`SOFT_TIER`]: below that tier,
    /// and for regions small enough to blur in one step even at the tier (the part icons' glows, which are at
    /// [`ICON_DENSITY`] and so only a few pixels across at [`SOFT_TIER`], too few to resample from).
    pub(super) fn soft_direct(&self, tier: u32) -> bool {
        let [_, _, w, h] = self.pixels(tier);
        tier < SOFT_TIER || (w * h) as usize <= STEP_TEXELS
    }

    /// The soft layer at [`SOFT_TIER`], rendered the first time it is asked for (`None`: the region has none).
    pub(super) fn soft_layer(&self) -> Option<&Result<Soft, String>> {
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

pub(super) fn snap(x: f32) -> f32 {
    (x / GUTTER).ceil() * GUTTER
}

/// Skyline packing in atlas units: tallest first, each region at the lowest spot of the skyline it fits (the
/// leftmost of equals). Positions are multiples of the gutter, the height is rounded up so every mip level has an
/// integer size at the smallest tier. A region wider or taller than [`LAYOUT_WIDTH_MIN`] (less the gutters) is
/// scaled down (keeping its aspect) to fit it. (Shelves until 13 Oct 2026: they left a third of the atlas empty
/// beside the tall regions, and the machines' ship drawings did not fit 4096 px at the top tier with them.)
///
/// The width adapts (20 Oct 2026): every width from [`LAYOUT_WIDTH_MIN`] to [`LAYOUT_SIDE_MAX`] in half units is
/// packed and the layout with the shortest longest side wins (the smaller area of equals), so the atlas stays near
/// square as content grows and its longest side, which decides the tiers a GPU can hold, grows with the square
/// root of the art's area. At a fixed 8 units it grew only downwards: 8 × 17.5 units with the Lattice's and the
/// Phase walkers' parts, 4480 px at tier 256.
pub(super) fn pack(mut regs: Vec<(RegionDef, [f32; 2])>) -> Layout {
    let most = LAYOUT_WIDTH_MIN - 2.0 * GUTTER;
    for (_, [w, h]) in &mut regs {
        let k = (most / w.max(*h)).min(1.0);
        (*w, *h) = (*w * k, *h * k);
    }
    let mut order: Vec<usize> = (0..regs.len()).collect();
    order.sort_by(|&a, &b| {
        regs[b].1[1].total_cmp(&regs[a].1[1]).then(regs[b].1[0].total_cmp(&regs[a].1[0])).then(regs[a].0.name.cmp(&regs[b].0.name))
    });
    let sizes: Vec<[f32; 2]> = regs.iter().map(|r| r.1).collect();
    // Widths in ascending order; once the width alone is no shorter than the best longest side, none can win.
    let mut best: Option<(f32, Vec<[f32; 2]>, f32)> = None;
    let mut width = LAYOUT_WIDTH_MIN;
    while width <= LAYOUT_SIDE_MAX && best.as_ref().is_none_or(|b| width < b.0.max(b.2)) {
        let (at, height) = skyline(&sizes, &order, width);
        let better = best.as_ref().is_none_or(|b| {
            let (side, b_side) = (width.max(height), b.0.max(b.2));
            side < b_side || (side == b_side && width * height < b.0 * b.2)
        });
        if better {
            best = Some((width, at, height));
        }
        width += 0.5;
    }
    let (width, at, height) = best.expect("the narrowest width is always packed");
    let regions: Vec<RegionDef> = regs
        .into_iter()
        .zip(at)
        .map(|((mut r, [w, h]), [x, y])| {
            r.rect = [x, y, w, h];
            r
        })
        .collect();
    let index = regions.iter().enumerate().map(|(i, r)| (r.name.clone(), i)).collect();
    Layout { regions, size: [width, height], index }
}

/// The skyline packing of regions of `sizes` (in `order`) into rows of `width` units: each region's top-left
/// corner, and the layout's height (a multiple of half a unit: integral mips down to 16 px).
fn skyline(sizes: &[[f32; 2]], order: &[usize], width: f32) -> (Vec<[f32; 2]>, f32) {
    const EPS: f32 = 1e-4;
    let mut at = vec![[0.0; 2]; sizes.len()];
    // The skyline: steps `[x, y]` by ascending x, each up to the next one (the last to the layout's width); a
    // region goes at a step's x, as high as the steps under its width (gutter included) reach.
    let mut sky: Vec<[f32; 2]> = vec![[GUTTER, GUTTER]];
    let mut bottom = 0.0f32;
    for &i in order {
        let [w, h] = sizes[i];
        let span = snap(w + GUTTER);
        let mut best: Option<[f32; 2]> = None;
        for (k, &[x, _]) in sky.iter().enumerate() {
            if x + w + GUTTER > width + EPS {
                break;
            }
            let y = sky[k..].iter().take_while(|s| s[0] < x + span - EPS).fold(0.0f32, |m, s| m.max(s[1]));
            if best.is_none_or(|b| y < b[1] - EPS) {
                best = Some([x, y]);
            }
        }
        // The first step starts at the gutter and every region fits the narrowest row.
        let [x, y] = best.expect("a region fits a row");
        at[i] = [x, y];
        bottom = bottom.max(y + h);
        let end = x + span;
        let resume = sky.iter().take_while(|s| s[0] < end + EPS).last().map_or(GUTTER, |s| s[1]);
        let mut next: Vec<[f32; 2]> = sky.iter().copied().filter(|s| s[0] < x - EPS).collect();
        next.push([x, snap(y + h + GUTTER)]);
        if end < width - EPS {
            next.push([end, resume]);
            next.extend(sky.iter().copied().filter(|s| s[0] > end + EPS));
        }
        next.dedup_by(|b, a| (a[1] - b[1]).abs() < EPS);
        sky = next;
    }
    (at, ((bottom + GUTTER) * 2.0).ceil() / 2.0)
}

/// Splits an icon SVG into (crisp glyph, glow-only) documents. The glow is every `<g filter="url(#gl)" ...>`
/// element (with its opacity) after `</defs>`.
pub(super) fn split_glow(src: &str) -> (String, Option<String>) {
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
pub(super) fn balanced_g_len(s: &str) -> usize {
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
