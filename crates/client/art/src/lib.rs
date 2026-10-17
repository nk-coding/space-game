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

mod layout;
mod raster;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use resvg::{tiny_skia, usvg};

pub use layout::{Layout, RegionDef};
use layout::{Proc, Soft, Source};
pub use raster::{Raster, Rasterizer, STEP_TEXELS, rasterise};
use raster::{render_rows, svg_options};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

/// SVG user units per ship cell in all art sources.
pub const UNITS_PER_CELL: f32 = 20.0;
/// Pixel-density tiers (pixels per cell). Powers of two; the tiers below the top keep the atlas within 4096 pixels,
/// the top one within 8192 (the Grown's parts, 18 Oct 2026, grew the base atlas past 8 × 8 units; a GPU whose
/// largest texture is smaller caps the tier, `max_tier`).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::{CLEAR_BYTES, Stage, copy_region, procedural};

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
            let most = if tier == MAX_TIER { 8192 } else { 4096 };
            assert!(w <= most && h <= most, "tier {tier}: {w}x{h}");
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
            assert_eq!(data::content::svg::check_svg(svg), Ok(()), "{key}");
        }
        let layout = Arc::new(Layout::new(&with_drawings(&icons)));
        for (key, svg) in &icons {
            let r = layout.get(&format!("ship/{key}")).unwrap_or_else(|| panic!("{key}"));
            let [_, _, vw, vh] = data::content::svg::view_box(svg).unwrap();
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
