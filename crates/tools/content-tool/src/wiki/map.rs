//! A generated galaxy as SVG (the `galaxy` crate's skeleton): the band edges as dotted rings, every cluster a dot
//! (home a diamond, gates gold diamonds), the starting jump range around home, and each trail as a path from its
//! first source to its reward, coloured by what it leads to; the first band's trails brighter and with their signals'
//! fresh areas. For the galaxy's
//! wiki page and `content-tool galaxy --svg`.

use std::fmt::Write as _;

use data::{Content, Reward};
use galaxy::{Point, Skeleton, UNITS_PER_S};

use super::draw::Style;
use super::markup::esc;

/// A number for SVG coordinates.
fn n(v: f64) -> String {
    data::design::stats::format_number(v as f32, 1)
}

/// The skeleton `s` of a galaxy of `c` drawn at `px_per_s` px per S. `labels`: the first band's cluster names.
pub fn svg(c: &Content, s: &Skeleton, style: &Style, px_per_s: f64, labels: bool) -> String {
    let ui = |k: &str| style.get(&format!("ui.{k}"));
    let g = c.galaxies.get(&s.galaxy);
    let bands: Vec<&data::Band> = g.map(|g| g.bands.iter().filter_map(|b| c.bands.get(b)).collect()).unwrap_or_default();
    let radius = bands.first().map_or(1.0, |b| b.edge as f64) + 0.3;
    let size = 2.0 * radius * px_per_s;
    let to = |p: Point| ((p.x as f64 / UNITS_PER_S as f64 + radius) * px_per_s, (p.y as f64 / UNITS_PER_S as f64 + radius) * px_per_s);
    let font = 11.0;
    let mut out = format!(
        "<svg class=\"galaxy-map\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {0} {0}\" width=\"{0}\" height=\"{0}\" role=\"img\" aria-label=\"{1}, seed {2}\">",
        n(size),
        esc(g.map_or(s.galaxy.as_str(), |g| g.name.as_str())),
        s.seed
    );
    let mid = radius * px_per_s;
    // The band edges, and each band's name just inside its edge.
    for b in &bands {
        let r = b.edge as f64 * px_per_s;
        write!(out, "<circle cx=\"{0}\" cy=\"{0}\" r=\"{1}\" fill=\"none\" stroke=\"{2}\" stroke-width=\"1\" stroke-dasharray=\"2 4\"/>", n(mid), n(r), ui("dotted_ring")).unwrap();
        write!(out, "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{font}\" fill=\"{}\" font-family=\"{}\">{}</text>", n(mid), n(mid - r + font + 2.0), ui("dim"), ui("font"), esc(&b.name.to_uppercase())).unwrap();
    }
    // The starting jump range around home, cut at the drawing's round edge.
    if let (Some(g), Some(home)) = (g, s.clusters.get(s.home as usize)) {
        let (x, y) = to(home.at);
        write!(out, "<clipPath id=\"galaxy-map-edge\"><circle cx=\"{0}\" cy=\"{0}\" r=\"{0}\"/></clipPath>", n(mid)).unwrap();
        write!(out, "<circle clip-path=\"url(#galaxy-map-edge)\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" fill-opacity=\"0.05\" stroke=\"{}\" stroke-opacity=\"0.5\" stroke-dasharray=\"3 3\"/>", n(x), n(y), n(g.jump_range as f64 * px_per_s), ui("teal"), ui("teal")).unwrap();
    }
    // Trails: their signals' areas, then the path through their places (faint, without areas, past the first band).
    let first = bands.first().map(|b| b.id.as_str());
    for t in &s.trails {
        let lit = Some(t.band.as_str()) == first;
        let colour = match c.trails.get(&t.trail).map(|t| t.reward) {
            Some(Reward::Gate) => ui("gold"),
            Some(Reward::Cache) => ui("violet"),
            Some(Reward::Lore) => ui("teal"),
            _ => ui("coral"),
        };
        for sig in t.signals.iter().filter(|_| lit) {
            let (x, y) = to(sig.area.centre);
            write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"{colour}\" stroke-opacity=\"0.25\" stroke-dasharray=\"1 3\"/>", n(x), n(y), n(sig.area.radius as f64 / UNITS_PER_S as f64 * px_per_s)).unwrap();
        }
        let d: Vec<String> = t
            .places
            .iter()
            .map(|p| {
                let (x, y) = to(s.clusters[p.cluster as usize].at);
                format!("{} {}", n(x), n(y))
            })
            .collect();
        let opacity = if lit { 0.7 } else { 0.25 };
        write!(out, "<path d=\"M{}\" fill=\"none\" stroke=\"{colour}\" stroke-opacity=\"{opacity}\" stroke-width=\"1.5\"/>", d.join("L")).unwrap();
        if let Some(end) = t.places.last() {
            let (x, y) = to(s.clusters[end.cluster as usize].at);
            write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"7\" fill=\"none\" stroke=\"{colour}\" stroke-width=\"1.5\"/>", n(x), n(y)).unwrap();
        }
    }
    // The clusters: the first band bright, the others dim; gates and home as diamonds.
    for (k, cl) in s.clusters.iter().enumerate() {
        let (x, y) = to(cl.at);
        let lit = Some(cl.band.as_str()) == first;
        if k as u32 == s.home || cl.gate {
            let (r, fill) = if cl.gate { (5.0, ui("gold")) } else { (7.0, ui("teal")) };
            write!(out, "<path d=\"M{} {}l{r} {r}l-{r} {r}l-{r} -{r}z\" fill=\"{fill}\"/>", n(x), n(y - r)).unwrap();
        } else {
            let fill = if lit { ui("star") } else { ui("dim") };
            write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{fill}\"/>", n(x), n(y), if lit { 3 } else { 2 }).unwrap();
        }
        if labels && lit {
            write!(out, "<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\" font-family=\"{}\">{}</text>", n(x + 6.0), n(y - 5.0), font - 2.0, ui("label"), ui("font"), esc(&cl.name)).unwrap();
        }
    }
    out + "</svg>"
}
